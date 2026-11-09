//! Varying slopes on the full formulation ([`Effect::with_slopes`]): PCG and
//! modified LSMR with the levels' channel blocks, additive Schwarz (pairs
//! with slopes acted on by their block elimination) or no preconditioner,
//! and the elimination with slopes of two effects, against an independent
//! dense weighted least-squares reference on the
//! explicit design (one column per effect, channel and level: the level
//! dummy times the channel's value; modified Gram-Schmidt on `sqrt(W) D`),
//! singular level blocks, unit invariance, independence of the worker count,
//! warm starts, routing and validation.

use fedemean::{
    Choice, CoefficientLayout, CoefficientsMut, ColumnsMut, Control, Design, Effect, Error,
    Fallback, Method, MethodUsed, Outcome, Preconditioner, PreconditionerUsed, Projector,
    Reduction, ReductionUsed, Report, Request, SchwarzSpec, Settings, Weights,
};

const TOLERANCE: f64 = 1e-10;
const MAX_ITERATIONS: usize = 5000;

/// SplitMix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }

    fn normal(&mut self) -> f64 {
        let u = self.uniform().max(1e-300);
        let v = self.uniform();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// One effect: its level count, whether it has an intercept and its number
/// of slopes.
#[derive(Clone, Copy, Debug)]
struct Spec {
    levels: usize,
    intercept: bool,
    slopes: usize,
}

const fn spec(levels: usize, intercept: bool, slopes: usize) -> Spec {
    Spec {
        levels,
        intercept,
        slopes,
    }
}

/// A design in which every level is observed, with slope values.
struct Case {
    n: usize,
    specs: Vec<Spec>,
    codes: Vec<Vec<u32>>,
    /// Per effect, per slope, one value per observation.
    slopes: Vec<Vec<Vec<f64>>>,
    /// A common offset of the slopes that the generated columns do not
    /// carry (absorbed by the intercepts), and one of a single effect's
    /// slopes on every `every`-th level (effect, every, offset).
    offset: f64,
    levels_offset: Option<(usize, u32, f64)>,
}

impl Case {
    /// Independent uniform codes per effect, each level at least once;
    /// slopes `center + spread * normal` with a center and spread per
    /// slope (a time trend-like variable is far from zero).
    fn random(seed: u64, n: usize, specs: &[Spec]) -> Self {
        let mut rng = Rng(seed);
        let codes = specs
            .iter()
            .map(|spec| {
                assert!(spec.levels <= n);
                let mut codes: Vec<u32> = (0..n)
                    .map(|i| {
                        if i < spec.levels {
                            i as u32
                        } else {
                            rng.below(spec.levels) as u32
                        }
                    })
                    .collect();
                for i in (1..n).rev() {
                    codes.swap(i, rng.below(i + 1));
                }
                codes
            })
            .collect();
        let slopes = specs
            .iter()
            .map(|spec| {
                (0..spec.slopes)
                    .map(|s| {
                        let center = [10.0, -1.0, 0.0][s % 3];
                        let spread = [3.0, 0.5, 2.0][s % 3];
                        (0..n).map(|_| center + spread * rng.normal()).collect()
                    })
                    .collect()
            })
            .collect();
        Self {
            n,
            specs: specs.to_vec(),
            codes,
            slopes,
            offset: 0.0,
            levels_offset: None,
        }
    }

    /// Add `offset` to every slope value (the columns stay as they were).
    fn shifted(mut self, offset: f64) -> Self {
        for value in self.slopes.iter_mut().flatten().flatten() {
            *value += offset;
        }
        self.offset = offset;
        self
    }

    /// Add `offset` to effect `e`'s slopes on every `every`-th level only
    /// (the columns stay as they were).
    fn shifted_levels(mut self, e: usize, every: u32, offset: f64) -> Self {
        for (i, &code) in self.codes[e].iter().enumerate() {
            if code % every == 0 {
                for slope in &mut self.slopes[e] {
                    slope[i] += offset;
                }
            }
        }
        self.levels_offset = Some((e, every, offset));
        self
    }

    /// The offset of effect `e`'s slopes at row `i` (absent from the
    /// columns).
    fn offset_at(&self, e: usize, i: usize) -> f64 {
        match self.levels_offset {
            Some((effect, every, offset)) if effect == e && self.codes[e][i] % every == 0 => {
                self.offset + offset
            }
            _ => self.offset,
        }
    }

    fn design(&self) -> Design<'_> {
        let effects = self
            .specs
            .iter()
            .enumerate()
            .map(|(e, spec)| {
                Effect::new(&self.codes[e], spec.levels)
                    .with_slopes(spec.intercept, self.slopes[e].iter().map(Vec::as_slice))
                    .unwrap()
            })
            .collect();
        Design::new(self.n, effects).unwrap()
    }

    fn channels(&self, e: usize) -> usize {
        usize::from(self.specs[e].intercept) + self.specs[e].slopes
    }

    fn channel_counts(&self) -> Vec<usize> {
        (0..self.specs.len()).map(|e| self.channels(e)).collect()
    }

    fn level_counts(&self) -> Vec<usize> {
        self.specs.iter().map(|s| s.levels).collect()
    }

    fn per_rhs(&self) -> usize {
        (0..self.specs.len())
            .map(|e| self.specs[e].levels * self.channels(e))
            .sum()
    }

    fn offsets(&self) -> Vec<usize> {
        let mut next = 0;
        (0..self.specs.len())
            .map(|e| {
                let offset = next;
                next += self.specs[e].levels * self.channels(e);
                offset
            })
            .collect()
    }

    /// Channel `c` of effect `e` at row `i`.
    fn value(&self, e: usize, c: usize, i: usize) -> f64 {
        let lead = usize::from(self.specs[e].intercept);
        if c < lead {
            1.0
        } else {
            self.slopes[e][c - lead][i]
        }
    }

    /// Positive, non-constant weights with a few heavy observations.
    fn weights(&self, seed: u64) -> Vec<f64> {
        let mut rng = Rng(seed);
        (0..self.n)
            .map(|_| {
                if rng.below(50) == 0 {
                    6.0
                } else {
                    0.2 + 1.8 * rng.uniform()
                }
            })
            .collect()
    }

    /// `ncols` columns: each a random coefficient per effect, channel and
    /// level, a constant and (for some) noise.
    fn columns(&self, seed: u64, ncols: usize) -> Vec<f64> {
        let mut rng = Rng(seed);
        let mut data = vec![0.0; self.n * ncols];
        for j in 0..ncols {
            let alpha: Vec<f64> = (0..self.per_rhs()).map(|_| rng.normal()).collect();
            let noise = [1.0, 0.0, 2.0][j % 3];
            let constant = 4.0 * j as f64 - 3.0;
            for i in 0..self.n {
                data[i * ncols + j] = constant + noise * rng.normal() + self.fitted_less(&alpha, i);
            }
        }
        data
    }

    /// `D b` at row `i` with every slope less its offset.
    fn fitted_less(&self, coefficients: &[f64], i: usize) -> f64 {
        let offsets = self.offsets();
        let mut fit = 0.0;
        for e in 0..self.specs.len() {
            let (levels, lead) = (self.specs[e].levels, usize::from(self.specs[e].intercept));
            let code = self.codes[e][i] as usize;
            for c in 0..self.channels(e) {
                let shift = if c < lead { 0.0 } else { self.offset_at(e, i) };
                let v = self.value(e, c, i) - shift;
                fit += v * coefficients[offsets[e] + c * levels + code];
            }
        }
        fit
    }

    /// `D b` at row `i` of channel-major coefficients (column `j` of a
    /// buffer, or `coefficients` itself).
    fn fitted(&self, coefficients: &[f64], j: Option<usize>, i: usize) -> f64 {
        let base = j.map_or(0, |j| j * self.per_rhs());
        let offsets = self.offsets();
        let mut fit = 0.0;
        for e in 0..self.specs.len() {
            let levels = self.specs[e].levels;
            let code = self.codes[e][i] as usize;
            for c in 0..self.channels(e) {
                fit += self.value(e, c, i) * coefficients[base + offsets[e] + c * levels + code];
            }
        }
        fit
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn subtract_projections(basis: &[Vec<f64>], v: &mut [f64]) {
    // Two passes of modified Gram-Schmidt (reorthogonalization).
    for _ in 0..2 {
        for q in basis {
            let d = dot(q, v);
            for (x, &qi) in v.iter_mut().zip(q) {
                *x -= d * qi;
            }
        }
    }
}

/// Dense weighted least-squares reference: an orthonormal basis of the
/// column space of `sqrt(W) D`, dropping explicit columns whose remaining
/// norm after projection is below `1e-9` of their own.
struct Oracle {
    root_weights: Vec<f64>,
    basis: Vec<Vec<f64>>,
}

impl Oracle {
    fn new(case: &Case, weights: Option<&[f64]>) -> Self {
        let root_weights: Vec<f64> = (0..case.n)
            .map(|i| weights.map_or(1.0, |w| w[i].sqrt()))
            .collect();
        let mut basis: Vec<Vec<f64>> = Vec::new();
        for (e, spec) in case.specs.iter().enumerate() {
            for c in 0..case.channels(e) {
                for level in 0..spec.levels {
                    let mut v: Vec<f64> = (0..case.n)
                        .map(|i| {
                            if case.codes[e][i] as usize == level {
                                root_weights[i] * case.value(e, c, i)
                            } else {
                                0.0
                            }
                        })
                        .collect();
                    let original = dot(&v, &v).sqrt();
                    if original == 0.0 {
                        continue;
                    }
                    subtract_projections(&basis, &mut v);
                    let remaining = dot(&v, &v).sqrt();
                    if remaining > 1e-9 * original {
                        for x in &mut v {
                            *x /= remaining;
                        }
                        basis.push(v);
                    }
                }
            }
        }
        Self {
            root_weights,
            basis,
        }
    }

    fn residual(&self, y: &[f64]) -> Vec<f64> {
        let mut v: Vec<f64> = y
            .iter()
            .zip(&self.root_weights)
            .map(|(y, root)| y * root)
            .collect();
        subtract_projections(&self.basis, &mut v);
        v.iter()
            .zip(&self.root_weights)
            .map(|(v, root)| v / root)
            .collect()
    }
}

fn column(data: &[f64], ncols: usize, j: usize) -> Vec<f64> {
    data.iter().skip(j).step_by(ncols).copied().collect()
}

fn inf_norm(values: &[f64]) -> f64 {
    values.iter().fold(0.0_f64, |m, v| m.max(v.abs()))
}

fn as_weights(weights: Option<&[f64]>) -> Weights<'_> {
    weights.map_or(Weights::Unit, Weights::Values)
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

fn projected(outcome: Outcome) -> Report {
    match outcome {
        Outcome::Projected(report) => report,
        Outcome::Declined { reason, facts, .. } => {
            panic!("unexpected decline: {reason:?} {facts:?}")
        }
        _ => panic!("unexpected outcome"),
    }
}

fn full(method: Method, preconditioner: Preconditioner) -> Settings {
    Settings {
        choice: Choice {
            reduction: Reduction::Full,
            method,
            preconditioner,
            ..Choice::default()
        },
        lanes: None,
    }
}

const SCHWARZ: Preconditioner = Preconditioner::Schwarz(SchwarzSpec {
    subdomains: fedemean::Subdomains::Star,
    local: fedemean::SchwarzLocal::Auto,
});

/// The routes this build supports for `case` (Schwarz on two or more
/// effects).
fn routes(case: &Case) -> Vec<(Method, Preconditioner)> {
    let mut routes = vec![
        (Method::Auto, Preconditioner::Auto),
        (Method::Pcg, Preconditioner::Jacobi),
        (Method::Pcg, Preconditioner::Identity),
    ];
    if cfg!(feature = "schwarz") {
        routes.push((Method::Lsmr, Preconditioner::Jacobi));
        routes.push((Method::Lsmr, Preconditioner::Identity));
        if case.specs.len() >= 2 {
            routes.push((Method::Pcg, SCHWARZ));
            routes.push((Method::Lsmr, SCHWARZ));
        }
    }
    routes
}

/// Whether `used` is what `preconditioner` resolves to for `case`.
fn resolved(case: &Case, preconditioner: Preconditioner, used: PreconditionerUsed) -> bool {
    let schwarz = matches!(used, PreconditionerUsed::Schwarz { .. });
    match preconditioner {
        Preconditioner::Identity => used == PreconditionerUsed::Identity,
        Preconditioner::Schwarz(_) => schwarz,
        Preconditioner::Auto if cfg!(feature = "schwarz") && case.specs.len() >= 2 => schwarz,
        _ => matches!(used, PreconditionerUsed::ChannelBlocks { .. }),
    }
}

/// Project `data` in place (from `start` when given), filling and returning
/// a zeroed coefficient buffer of the channel-major layout.
fn project(
    projector: &mut Projector<'_>,
    case: &Case,
    weights: Option<&[f64]>,
    (data, ncols): (&mut [f64], usize),
    start: Option<&[f64]>,
    request: &Request,
) -> (Outcome, Vec<f64>) {
    let (offsets, levels, channels) = (case.offsets(), case.level_counts(), case.channel_counts());
    let mut coefficients = vec![0.0; case.per_rhs() * ncols];
    let layout = CoefficientLayout::new(case.per_rhs(), &offsets, &levels).with_channels(&channels);
    let columns = ColumnsMut::new(data, case.n, ncols).unwrap();
    let out = Some(CoefficientsMut::new(&mut coefficients, layout));
    let control = Control::default();
    let outcome = match start {
        Some(start) => {
            projector.project_from(as_weights(weights), columns, start, out, request, &control)
        }
        None => projector.project(as_weights(weights), columns, out, request, &control),
    };
    (outcome.unwrap(), coefficients)
}

/// Residuals against the oracle, coefficients against `y - r`, the route,
/// the reports and `certify_residual`; returns the reported preconditioner.
fn check(
    case: &Case,
    weights: Option<&[f64]>,
    oracle: &Oracle,
    (method, preconditioner): (Method, Preconditioner),
    columns: (usize, u64),
) -> PreconditionerUsed {
    let route = (Reduction::Full, method, preconditioner);
    check_route(case, weights, oracle, route, columns)
}

/// The effect with the most levels (the first of equals): the elimination's.
fn largest(case: &Case) -> usize {
    (0..case.specs.len())
        .max_by_key(|&e| (case.specs[e].levels, std::cmp::Reverse(e)))
        .unwrap()
}

/// [`check`] for any reduction (the elimination: its reduced system is the
/// smaller effect's coefficients).
fn check_route(
    case: &Case,
    weights: Option<&[f64]>,
    oracle: &Oracle,
    route: (Reduction, Method, Preconditioner),
    columns: (usize, u64),
) -> PreconditionerUsed {
    check_at(case, weights, oracle, route, (columns, TOLERANCE))
}

/// [`check_route`] at `tolerance`.
fn check_at(
    case: &Case,
    weights: Option<&[f64]>,
    oracle: &Oracle,
    (reduction, method, preconditioner): (Reduction, Method, Preconditioner),
    ((ncols, seed), tolerance): ((usize, u64), f64),
) -> PreconditionerUsed {
    let original = case.columns(seed, ncols);
    let mut data = original.clone();
    let mut settings = full(method, preconditioner);
    settings.choice.reduction = reduction;
    let mut projector = Projector::new(case.design(), settings);
    let request = Request::new(tolerance, MAX_ITERATIONS);
    let (outcome, coefficients) = project(
        &mut projector,
        case,
        weights,
        (&mut data, ncols),
        None,
        &request,
    );
    let label = format!(
        "{:?} {reduction:?}/{method:?}/{preconditioner:?} {ncols} cols weighted {}",
        case.specs,
        weights.is_some()
    );
    if let Outcome::Declined { reason, facts, .. } = &outcome {
        panic!("{label}: declined {reason:?} {facts:?}");
    }
    let report = projected(outcome);
    assert!(
        report.certificate.passed,
        "{label}: {:?}",
        report.certificate
    );
    let method_used = if method == Method::Lsmr {
        MethodUsed::Lsmr
    } else {
        MethodUsed::Pcg
    };
    assert_eq!(report.route.method, method_used, "{label}");
    assert_eq!(report.route.escalations, 0, "{label}");
    let used = report.route.preconditioner;
    if reduction == Reduction::EliminateLargest {
        let effect = largest(case);
        let other = 1 - effect;
        let reduced = case.specs[other].levels * case.channels(other);
        assert_eq!(
            report.route.reduction,
            ReductionUsed::Eliminated { effect },
            "{label}"
        );
        assert_eq!(report.facts.reduced, reduced, "{label}");
        assert!(
            matches!(used, PreconditionerUsed::ChannelBlocks { .. }),
            "{label}: {used:?}"
        );
    } else {
        assert_eq!(report.route.reduction, ReductionUsed::Full, "{label}");
        assert_eq!(report.facts.reduced, case.per_rhs(), "{label}");
        assert!(resolved(case, preconditioner, used), "{label}: {used:?}");
    }
    assert_eq!(projector.columns().len(), ncols);
    for j in 0..ncols {
        let y = column(&original, ncols, j);
        let r = column(&data, ncols, j);
        let bound = 1e-7 * inf_norm(&y).max(1.0);
        for (i, (&actual, &reference)) in r.iter().zip(&oracle.residual(&y)).enumerate() {
            assert!(
                (actual - reference).abs() <= bound,
                "{label}, column {j}, row {i}: {actual} vs oracle {reference}"
            );
        }
        let bound = 1e-8 * inf_norm(&y).max(1.0);
        for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
            let fit = case.fitted(&coefficients, Some(j), i);
            assert!(
                (yi - fit - ri).abs() <= bound,
                "{label}, column {j}, row {i}: y - Db = {} vs residual {ri}",
                yi - fit
            );
        }
        let report = projector.columns()[j];
        assert!(report.relative_moment <= tolerance, "{label}");
        // Recomputed at the projection's own scale.
        if report.residual_norm > 0.0 {
            let amplification = (report.scale / report.residual_norm).max(1.0);
            let certificate = projector
                .certify_residual(as_weights(weights), &r, tolerance, amplification)
                .unwrap()
                .expect("certificate");
            assert!(certificate.passed, "{label}, column {j}: {certificate:?}");
        }
    }
    used
}

/// Every supported route (without `identity`, the preconditioned ones),
/// unit and varying weights, 1/3/11 columns.
fn check_case(seed: u64, n: usize, specs: &[Spec], identity: bool) {
    let case = Case::random(seed, n, specs);
    let weights = case.weights(seed + 1);
    for weights in [None, Some(weights.as_slice())] {
        let oracle = Oracle::new(&case, weights);
        for route in routes(&case)
            .into_iter()
            .filter(|&(_, p)| identity || p != Preconditioner::Identity)
        {
            for (ncols, offset) in [(1, 2), (3, 3), (11, 4)] {
                check(&case, weights, &oracle, route, (ncols, seed + offset));
            }
        }
        if case.specs.len() == 2 {
            for preconditioner in [Preconditioner::Auto, Preconditioner::Jacobi] {
                let route = (Reduction::EliminateLargest, Method::Pcg, preconditioner);
                for (ncols, offset) in [(1, 5), (3, 6), (11, 7)] {
                    check_route(&case, weights, &oracle, route, (ncols, seed + offset));
                }
            }
        }
    }
}

#[test]
fn one_effect_with_slopes_matches_dense_oracle() {
    check_case(11, 120, &[spec(7, true, 1)], true);
    check_case(12, 150, &[spec(9, false, 2)], true);
    check_case(13, 150, &[spec(5, true, 3)], true);
}

#[test]
fn two_effects_with_slopes_match_dense_oracle() {
    check_case(21, 240, &[spec(40, true, 0), spec(9, true, 1)], true);
    check_case(22, 240, &[spec(31, true, 1), spec(8, false, 1)], true);
}

#[test]
fn three_effects_with_slopes_match_dense_oracle() {
    check_case(
        31,
        300,
        &[spec(30, true, 0), spec(6, true, 2), spec(5, false, 1)],
        true,
    );
}

/// Unpreconditioned CG is slow on these badly scaled channels (slopes
/// about ten times the intercepts): its weighted eleven-column batch stalls
/// and declines, so only the preconditioned routes are checked here.
#[test]
fn four_effects_with_slopes_match_dense_oracle() {
    check_case(
        41,
        360,
        &[
            spec(25, true, 1),
            spec(12, true, 0),
            spec(7, true, 1),
            spec(4, false, 1),
        ],
        false,
    );
}

/// A design with singular level blocks: an intercept-and-slope effect whose
/// level 0 has a constant slope, level 1 a zero slope and level 2 a single
/// row, and a slope-only effect whose level 0 has a zero slope.
fn singular_case(scale: f64) -> Case {
    let mut case = Case::random(
        51,
        400,
        &[spec(35, true, 0), spec(8, true, 1), spec(6, false, 1)],
    );
    // Effect 1: level 2 keeps one row (moved to level 3 otherwise).
    let mut kept = false;
    for code in &mut case.codes[1] {
        if *code == 2 {
            if kept {
                *code = 3;
            }
            kept = true;
        }
    }
    for i in 0..case.n {
        match case.codes[1][i] {
            0 => case.slopes[1][0][i] = 5.0,
            1 => case.slopes[1][0][i] = 0.0,
            _ => {}
        }
        if case.codes[2][i] == 0 {
            case.slopes[2][0][i] = 0.0;
        }
        case.slopes[1][0][i] *= scale;
        case.slopes[2][0][i] *= scale;
    }
    case
}

/// Singular blocks are null directions: the projection matches the oracle
/// (which drops dependent columns), every route reports them, the rule does
/// not depend on the slopes' units, and a zero slope's coefficient stays
/// zero.
#[test]
fn singular_level_blocks_are_null_directions() {
    for scale in [1.0, 1e6, 1e-5] {
        let case = singular_case(scale);
        let weights = case.weights(52);
        for weights in [None, Some(weights.as_slice())] {
            let oracle = Oracle::new(&case, weights);
            for route in routes(&case) {
                let used = check(&case, weights, &oracle, route, (3, 53));
                if route.1 == Preconditioner::Jacobi {
                    assert_eq!(
                        used,
                        PreconditionerUsed::ChannelBlocks { singular: 4 },
                        "scale {scale} {route:?}"
                    );
                }
            }
        }
    }
    let case = singular_case(1.0);
    let ncols = 2;
    let data = case.columns(54, ncols);
    for (method, preconditioner) in routes(&case) {
        let mut projector = Projector::new(case.design(), full(method, preconditioner));
        let request = Request::new(TOLERANCE, MAX_ITERATIONS);
        let mut copy = data.clone();
        let (outcome, coefficients) = project(
            &mut projector,
            &case,
            None,
            (&mut copy, ncols),
            None,
            &request,
        );
        projected(outcome);
        let offsets = case.offsets();
        for j in 0..ncols {
            let base = j * case.per_rhs();
            // Effect 1, slope channel, level 1; effect 2, level 0.
            assert_eq!(coefficients[base + offsets[1] + 8 + 1], 0.0, "{method:?}");
            assert_eq!(coefficients[base + offsets[2]], 0.0, "{method:?}");
        }
    }
}

/// Larger designs (a gathered effect with slopes and scattered ones) are
/// bitwise independent of the worker count and satisfy every channel's
/// normal equation.
#[test]
fn slope_projections_are_pool_independent() {
    let case = Case::random(
        61,
        40_000,
        &[
            spec(2500, true, 1),
            spec(1300, true, 0),
            spec(40, true, 2),
            spec(9, false, 1),
        ],
    );
    let weights = case.weights(62);
    let ncols = 4;
    let original = case.columns(63, ncols);
    for (method, preconditioner) in routes(&case)
        .into_iter()
        .filter(|&(_, p)| p != Preconditioner::Identity)
    {
        for weights in [None, Some(weights.as_slice())] {
            let run = |threads: usize| {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                pool.install(|| {
                    let mut projector = Projector::new(case.design(), full(method, preconditioner));
                    let mut data = original.clone();
                    let request = Request::new(1e-9, MAX_ITERATIONS);
                    let (outcome, coefficients) = project(
                        &mut projector,
                        &case,
                        weights,
                        (&mut data, ncols),
                        None,
                        &request,
                    );
                    assert!(projected(outcome).certificate.passed);
                    (data, coefficients)
                })
            };
            let (four, coefficients) = run(4);
            for threads in [1, 3] {
                let (other, other_coefficients) = run(threads);
                assert_eq!(bits(&four), bits(&other), "{method:?} {threads}");
                assert_eq!(bits(&coefficients), bits(&other_coefficients));
            }
            // Every channel of every level: |sum w v r| / sqrt(sum w v^2)
            // relative to the column's weighted norm.
            for j in 0..ncols {
                let y = column(&original, ncols, j);
                let r = column(&four, ncols, j);
                let w = |i: usize| weights.map_or(1.0, |w| w[i]);
                let norm = (0..case.n).map(|i| w(i) * y[i] * y[i]).sum::<f64>().sqrt();
                for (e, spec) in case.specs.iter().enumerate() {
                    for c in 0..case.channels(e) {
                        let mut moment = vec![0.0; spec.levels];
                        let mut mass = vec![0.0; spec.levels];
                        for (i, &ri) in r.iter().enumerate() {
                            let l = case.codes[e][i] as usize;
                            let v = case.value(e, c, i);
                            moment[l] += w(i) * v * ri;
                            mass[l] += w(i) * v * v;
                        }
                        for (m, mass) in moment.iter().zip(&mass) {
                            assert!(m.abs() <= 1e-8 * mass.sqrt() * norm, "{e} {c}");
                        }
                    }
                }
                let bound = 1e-8 * inf_norm(&y).max(1.0);
                for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                    let fit = case.fitted(&coefficients, Some(j), i);
                    assert!((yi - fit - ri).abs() <= bound, "column {j}, row {i}");
                }
            }
        }
    }
}

/// Warm starts in the channel-major layout: from the solution at most one
/// iteration; from a perturbed start the same projection, with coefficients
/// that are the full solution.
#[test]
fn slope_warm_starts_continue_from_their_coefficients() {
    let case = Case::random(
        71,
        500,
        &[spec(60, true, 0), spec(11, true, 1), spec(6, false, 2)],
    );
    let weights = case.weights(72);
    let ncols = 3;
    let original = case.columns(73, ncols);
    for (method, preconditioner) in routes(&case)
        .into_iter()
        .filter(|&(_, p)| p != Preconditioner::Identity)
    {
        for weights in [None, Some(weights.as_slice())] {
            let oracle = Oracle::new(&case, weights);
            let mut projector = Projector::new(case.design(), full(method, preconditioner));
            let tight = Request::new(1e-12, MAX_ITERATIONS);
            let mut solved = original.clone();
            let (outcome, solution) = project(
                &mut projector,
                &case,
                weights,
                (&mut solved, ncols),
                None,
                &tight,
            );
            assert!(projected(outcome).certificate.passed);
            let mut rng = Rng(74);
            let perturbed: Vec<f64> = solution.iter().map(|x| x + 0.1 * rng.normal()).collect();
            let request = Request::new(TOLERANCE, MAX_ITERATIONS);
            for (start, few) in [(&solution, true), (&perturbed, false)] {
                let mut data = original.clone();
                let (outcome, coefficients) = project(
                    &mut projector,
                    &case,
                    weights,
                    (&mut data, ncols),
                    Some(start),
                    &request,
                );
                let report = projected(outcome);
                assert!(report.certificate.passed);
                if few {
                    assert!(
                        report.facts.iterations <= 1,
                        "{method:?}: {:?}",
                        report.facts
                    );
                }
                for j in 0..ncols {
                    let y = column(&original, ncols, j);
                    let r = column(&data, ncols, j);
                    let bound = 1e-7 * inf_norm(&y).max(1.0);
                    let expected = oracle.residual(&y);
                    assert!(r.iter().zip(&expected).all(|(a, b)| (a - b).abs() <= bound));
                    let bound = 1e-8 * inf_norm(&y).max(1.0);
                    for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                        let fit = case.fitted(&coefficients, Some(j), i);
                        assert!((yi - fit - ri).abs() <= bound, "{method:?} {j} {i}");
                    }
                }
            }
        }
    }
}

/// Auto routes two effects with slopes to the slope elimination; block-Jacobi,
/// exact Schwarz local solves with slopes, the elimination of three effects
/// or with another method or preconditioner, and too many channels are
/// unsupported; inputs are validated.
#[test]
fn slope_routes_and_validation() {
    let case = Case::random(81, 200, &[spec(20, true, 0), spec(6, true, 1)]);
    let original = case.columns(82, 2);
    let request = Request::new(TOLERANCE, MAX_ITERATIONS);
    let run = |choice: Choice| {
        let settings = Settings {
            choice,
            lanes: None,
        };
        let mut projector = Projector::new(case.design(), settings);
        let mut data = original.clone();
        let columns = ColumnsMut::new(&mut data, case.n, 2).unwrap();
        projector.project(Weights::Unit, columns, None, &request, &Control::default())
    };
    for fallback in [Fallback::Decline, Fallback::Escalate] {
        let report = projected(
            run(Choice {
                fallback,
                ..Choice::default()
            })
            .unwrap(),
        );
        // Two effects with slopes: the slope elimination.
        assert_eq!(
            report.route.reduction,
            ReductionUsed::Eliminated { effect: 0 }
        );
        assert_eq!(report.route.method, MethodUsed::Pcg);
        let used = report.route.preconditioner;
        assert!(
            matches!(used, PreconditionerUsed::ChannelBlocks { .. }),
            "{used:?}"
        );
        assert_eq!(report.route.escalations, 0);
    }
    let unsupported = |choice: Choice| {
        assert!(
            matches!(run(choice), Err(Error::Unsupported(_))),
            "{choice:?}"
        );
    };

    unsupported(Choice {
        preconditioner: Preconditioner::BlockJacobi(fedemean::BlockSpec::default()),
        ..Choice::default()
    });
    // Seventeen constant channels: no cap; every slope is constant, so each
    // level's block is singular in every slope.
    let many: Vec<Vec<f64>> = (0..16).map(|s| vec![s as f64; case.n]).collect();
    let wide = Effect::new(&case.codes[1], 6)
        .with_slopes(true, many.iter().map(Vec::as_slice))
        .unwrap();
    let design = Design::new(case.n, vec![wide]).unwrap();
    let mut data = original.clone();
    let columns = ColumnsMut::new(&mut data, case.n, 2).unwrap();
    let result = Projector::new(design, Settings::default()).project(
        Weights::Unit,
        columns,
        None,
        &request,
        &Control::default(),
    );
    let report = projected(result.unwrap());
    assert_eq!(
        report.route.preconditioner,
        PreconditionerUsed::ChannelBlocks { singular: 6 }
    );
    // Exact Schwarz local solves with slopes; the elimination beyond two
    // effects or with another method or preconditioner.
    unsupported(Choice {
        preconditioner: Preconditioner::Schwarz(SchwarzSpec {
            local: fedemean::SchwarzLocal::Exact,
            ..SchwarzSpec::default()
        }),
        ..Choice::default()
    });
    for (method, preconditioner) in [
        (Method::Lsmr, Preconditioner::Jacobi),
        (Method::Pcg, Preconditioner::Identity),
        (Method::Pcg, SCHWARZ),
    ] {
        unsupported(Choice {
            reduction: Reduction::EliminateLargest,
            method,
            preconditioner,
            ..Choice::default()
        });
    }
    let three = Case::random(
        83,
        200,
        &[spec(20, true, 0), spec(6, true, 1), spec(4, true, 0)],
    );
    let mut data = three.columns(84, 2);
    let columns = ColumnsMut::new(&mut data, three.n, 2).unwrap();
    let mut settings = Settings::default();
    settings.choice.reduction = Reduction::EliminateLargest;
    let result = Projector::new(three.design(), settings).project(
        Weights::Unit,
        columns,
        None,
        &request,
        &Control::default(),
    );
    assert!(matches!(result, Err(Error::Unsupported(_))), "{result:?}");

    // Slope validation.
    let codes = &case.codes[1];
    let short = vec![1.0; case.n - 1];
    let mut nonfinite = case.slopes[1][0].clone();
    nonfinite[7] = f64::NAN;
    assert!(matches!(
        Effect::new(codes, 6).with_slopes(true, [short.as_slice()]),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        Effect::new(codes, 6).with_slopes(true, [nonfinite.as_slice()]),
        Err(Error::InvalidData(_))
    ));
    assert!(matches!(
        Effect::new(codes, 6).with_slopes(false, std::iter::empty()),
        Err(Error::InvalidInput(_))
    ));
    let plain = Effect::new(codes, 6)
        .with_slopes(true, std::iter::empty())
        .unwrap();
    assert_eq!(plain, Effect::new(codes, 6));
    assert_eq!(plain.channels(), 1);

    // A coefficient layout without the channel counts of a design with
    // slopes, or with wrong ones, is invalid.
    let (offsets, levels) = (case.offsets(), case.level_counts());
    let mut coefficients = vec![0.0; case.per_rhs() * 2];
    for channels in [None, Some(&[1_usize, 3][..])] {
        let mut layout = CoefficientLayout::new(case.per_rhs(), &offsets, &levels);
        if let Some(channels) = channels {
            layout = layout.with_channels(channels);
        }
        let mut projector = Projector::new(case.design(), Settings::default());
        let mut data = original.clone();
        let columns = ColumnsMut::new(&mut data, case.n, 2).unwrap();
        let result = projector.project(
            Weights::Unit,
            columns,
            Some(CoefficientsMut::new(&mut coefficients, layout)),
            &request,
            &Control::default(),
        );
        assert!(
            matches!(result, Err(Error::InvalidInput(_))),
            "{channels:?}"
        );
    }
    // A warm start must have every channel's coefficients (the full
    // formulation); the elimination has none.
    for (settings, short) in [
        (full(Method::Pcg, Preconditioner::Jacobi), true),
        (Settings::default(), false),
    ] {
        let mut projector = Projector::new(case.design(), settings);
        let mut data = original.clone();
        let columns = ColumnsMut::new(&mut data, case.n, 2).unwrap();
        let start = vec![0.0; if short { 20 + 6 } else { case.per_rhs() } * 2];
        let result = projector.project_from(
            Weights::Unit,
            columns,
            &start,
            None,
            &request,
            &Control::default(),
        );
        if short {
            assert!(matches!(result, Err(Error::InvalidInput(_))), "{result:?}");
        } else {
            assert!(matches!(result, Err(Error::Unsupported(_))), "{result:?}");
        }
    }
}

/// Solve the small dense system `a x = b` (row-major, `c x c`) by Gaussian
/// elimination with partial pivoting.
fn dense_solve(mut a: Vec<f64>, mut b: Vec<f64>, c: usize) -> Vec<f64> {
    for k in 0..c {
        let p = (k..c)
            .max_by(|&x, &y| a[x * c + k].abs().total_cmp(&a[y * c + k].abs()))
            .unwrap();
        for j in 0..c {
            a.swap(k * c + j, p * c + j);
        }
        b.swap(k, p);
        for i in k + 1..c {
            let f = a[i * c + k] / a[k * c + k];
            for j in k..c {
                a[i * c + j] -= f * a[k * c + j];
            }
            b[i] -= f * b[k];
        }
    }
    let mut x = vec![0.0; c];
    for k in (0..c).rev() {
        let s: f64 = (k + 1..c).map(|j| a[k * c + j] * x[j]).sum();
        x[k] = (b[k] - s) / a[k * c + k];
    }
    x
}

/// With coefficient stopping, every level's block estimate of the
/// coefficient correction, `|G_l^-1 m_l|` (normalized weights), is within
/// the tolerance of the column scale over the root total weight.
#[test]
fn slope_coefficient_stopping_bounds_the_block_estimate() {
    let case = Case::random(
        91,
        600,
        &[spec(50, true, 0), spec(9, true, 1), spec(5, false, 2)],
    );
    let weights = case.weights(92);
    let weight_max = weights.iter().fold(0.0_f64, |m, &v| m.max(v));
    let w = |i: usize| weights[i] / weight_max;
    let total: f64 = (0..case.n).map(w).sum();
    let ncols = 3;
    let original = case.columns(93, ncols);
    for (method, preconditioner) in routes(&case)
        .into_iter()
        .filter(|&(_, p)| p == Preconditioner::Jacobi)
    {
        let mut projector = Projector::new(case.design(), full(method, preconditioner));
        let mut request = Request::new(TOLERANCE, MAX_ITERATIONS);
        request.coefficient_stopping = true;
        let mut data = original.clone();
        let (outcome, _) = project(
            &mut projector,
            &case,
            Some(&weights),
            (&mut data, ncols),
            None,
            &request,
        );
        assert!(projected(outcome).certificate.passed);
        for j in 0..ncols {
            let r = column(&data, ncols, j);
            let scale = projector.columns()[j].scale;
            let mut largest = 0.0_f64;
            for (e, spec) in case.specs.iter().enumerate() {
                let c = case.channels(e);
                let mut gram = vec![vec![0.0; c * c]; spec.levels];
                let mut moment = vec![vec![0.0; c]; spec.levels];
                for (i, &ri) in r.iter().enumerate() {
                    let l = case.codes[e][i] as usize;
                    for a in 0..c {
                        moment[l][a] += w(i) * case.value(e, a, i) * ri;
                        for b in 0..c {
                            gram[l][a * c + b] += w(i) * case.value(e, a, i) * case.value(e, b, i);
                        }
                    }
                }
                for (g, m) in gram.into_iter().zip(moment) {
                    let x = dense_solve(g, m, c);
                    largest = x.iter().fold(largest, |acc, v| acc.max(v.abs()));
                }
            }
            let bound = TOLERANCE * scale / total.sqrt();
            assert!(
                largest <= bound * (1.0 + 1e-6),
                "{method:?} column {j}: {largest} > {bound}"
            );
        }
    }
}

/// Two effects for the elimination with singular levels: the effect with
/// slopes (`sloped`, intercept and one slope) has a constant slope on level
/// 0, a zero slope on level 1 and a single row on level 2; with the slopes
/// on the smaller effect, its level 3 is also nested in its own levels of
/// the larger effect (a Schur block that cancels).
fn singular_pair(sloped: usize, scale: f64) -> Case {
    let specs = if sloped == 0 {
        [spec(30, true, 1), spec(8, true, 0)]
    } else {
        [spec(30, true, 0), spec(8, true, 1)]
    };
    let mut case = Case::random(95, 420, &specs);
    let mut kept = false;
    for code in &mut case.codes[sloped] {
        if *code == 2 {
            if kept {
                *code = 4;
            }
            kept = true;
        }
    }
    if sloped == 1 {
        // Larger levels 28 and 29 only at smaller level 3, and level 3 only
        // there.
        for i in 0..case.n {
            if case.codes[1][i] == 3 {
                case.codes[0][i] = 28 + (i % 2) as u32;
            } else if case.codes[0][i] >= 28 {
                case.codes[0][i] = (i % 28) as u32;
            }
        }
    }
    for i in 0..case.n {
        match case.codes[sloped][i] {
            0 => case.slopes[sloped][0][i] = 5.0,
            1 => case.slopes[sloped][0][i] = 0.0,
            _ => {}
        }
        case.slopes[sloped][0][i] *= scale;
    }
    case
}

#[test]
fn slope_elimination_handles_singular_levels() {
    for sloped in [0, 1] {
        for scale in [1.0, 1e6] {
            let case = singular_pair(sloped, scale);
            let weights = case.weights(96);
            for weights in [None, Some(weights.as_slice())] {
                let oracle = Oracle::new(&case, weights);
                for preconditioner in [Preconditioner::Auto, Preconditioner::Jacobi] {
                    let route = (Reduction::EliminateLargest, Method::Pcg, preconditioner);
                    let used = check_route(&case, weights, &oracle, route, (3, 97));
                    let PreconditionerUsed::ChannelBlocks { singular } = used else {
                        panic!("{used:?}");
                    };
                    // The nested smaller level's Schur block cancels.
                    assert!(sloped == 0 || singular >= 1, "{sloped} {scale}: {used:?}");
                }
            }
        }
    }
}

/// The elimination with slopes (a gathered larger effect) is bitwise
/// independent of the worker count and satisfies every channel's normal
/// equation, with the slopes on either effect.
#[test]
fn slope_elimination_is_pool_independent() {
    for specs in [
        [spec(2500, true, 0), spec(400, true, 1)],
        [spec(2500, true, 2), spec(1200, true, 0)],
    ] {
        let case = Case::random(98, 30_000, &specs);
        let weights = case.weights(99);
        let ncols = 4;
        let original = case.columns(100, ncols);
        let mut settings = Settings::default();
        settings.choice.reduction = Reduction::EliminateLargest;
        for weights in [None, Some(weights.as_slice())] {
            let run = |threads: usize| {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                pool.install(|| {
                    let mut projector = Projector::new(case.design(), settings);
                    let mut data = original.clone();
                    let request = Request::new(1e-9, MAX_ITERATIONS);
                    let (outcome, coefficients) = project(
                        &mut projector,
                        &case,
                        weights,
                        (&mut data, ncols),
                        None,
                        &request,
                    );
                    let report = projected(outcome);
                    assert!(report.certificate.passed);
                    assert_eq!(
                        report.route.reduction,
                        ReductionUsed::Eliminated { effect: 0 }
                    );
                    (data, coefficients)
                })
            };
            let (four, coefficients) = run(4);
            for threads in [1, 3] {
                let (other, other_coefficients) = run(threads);
                assert_eq!(bits(&four), bits(&other), "{specs:?} {threads}");
                assert_eq!(bits(&coefficients), bits(&other_coefficients));
            }
            let w = |i: usize| weights.map_or(1.0, |w| w[i]);
            for j in 0..ncols {
                let y = column(&original, ncols, j);
                let r = column(&four, ncols, j);
                let norm = (0..case.n).map(|i| w(i) * y[i] * y[i]).sum::<f64>().sqrt();
                for (e, spec) in case.specs.iter().enumerate() {
                    for c in 0..case.channels(e) {
                        let (mut moment, mut mass) =
                            (vec![0.0; spec.levels], vec![0.0; spec.levels]);
                        for (i, &ri) in r.iter().enumerate() {
                            let l = case.codes[e][i] as usize;
                            let v = case.value(e, c, i);
                            moment[l] += w(i) * v * ri;
                            mass[l] += w(i) * v * v;
                        }
                        for (m, mass) in moment.iter().zip(&mass) {
                            assert!(m.abs() <= 1e-8 * mass.sqrt() * norm, "{e} {c}");
                        }
                    }
                }
                let bound = 1e-8 * inf_norm(&y).max(1.0);
                for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                    let fit = case.fitted(&coefficients, Some(j), i);
                    assert!((yi - fit - ri).abs() <= bound, "column {j}, row {i}");
                }
            }
        }
    }
}

/// Slopes sharing a large common offset (near `+-2^24`, `3 * 2^20` and
/// `1e5`, varying by units on each level): the operator, the moments, the
/// blocks and the certificate work in each level's centered basis, so no
/// offset cancels the slopes' variation, and every route (no
/// preconditioner, Jacobi, Schwarz and the elimination; unit and varying
/// weights) certifies at `1e-10` and matches the oracle in the raw basis.
#[test]
fn slopes_with_large_offsets_match_dense_oracle() {
    let two = [spec(30, true, 0), spec(8, true, 1)];
    let other = [spec(30, true, 1), spec(8, true, 0)];
    let three = [spec(25, true, 1), spec(9, true, 2), spec(4, true, 0)];
    for offset in [16_777_216.0, -16_777_216.0, 3_145_728.0, 1e5] {
        for specs in [&two[..], &other[..], &three[..]] {
            let case = Case::random(101, 360, specs).shifted(offset);
            let weights = case.weights(102);
            for weights in [None, Some(weights.as_slice())] {
                let oracle = Oracle::new(&case, weights);
                let mut routes: Vec<_> = routes(&case)
                    .into_iter()
                    .map(|(m, p)| (Reduction::Full, m, p))
                    .collect();
                if specs.len() == 2 {
                    routes.push((
                        Reduction::EliminateLargest,
                        Method::Pcg,
                        Preconditioner::Jacobi,
                    ));
                }
                for route in routes {
                    check_at(&case, weights, &oracle, route, ((3, 103), TOLERANCE));
                }
            }
        }
    }
}

/// Offset slopes on some levels only (the rest raw), one effect with two
/// slopes: every route certifies at `1e-10` and matches the oracle, and a
/// warm start in the raw basis from the solution needs no more iterations
/// than the cold start (the raw coefficients of an offset slope carry
/// their offset's rounding, about `1e-9` of the column here), from a
/// perturbed start the same projection.
#[test]
fn offset_slopes_mix_with_raw_levels_and_warm_starts() {
    let specs = [spec(28, true, 0), spec(9, true, 2)];
    let case = Case::random(111, 400, &specs).shifted_levels(1, 2, 16_777_216.0);
    let weights = case.weights(112);
    let ncols = 3;
    let original = case.columns(113, ncols);
    for weights in [None, Some(weights.as_slice())] {
        let oracle = Oracle::new(&case, weights);
        let mut routes: Vec<_> = routes(&case)
            .into_iter()
            .map(|(m, p)| (Reduction::Full, m, p))
            .collect();
        routes.push((
            Reduction::EliminateLargest,
            Method::Pcg,
            Preconditioner::Jacobi,
        ));
        for route in routes {
            check_at(&case, weights, &oracle, route, ((ncols, 113), TOLERANCE));
            let (reduction, method, preconditioner) = route;
            if reduction == Reduction::EliminateLargest {
                continue;
            }
            let mut projector = Projector::new(case.design(), full(method, preconditioner));
            let request = Request::new(TOLERANCE, MAX_ITERATIONS);
            let mut solved = original.clone();
            let (outcome, solution) = project(
                &mut projector,
                &case,
                weights,
                (&mut solved, ncols),
                None,
                &request,
            );
            let cold = projected(outcome);
            assert!(cold.certificate.passed);
            let mut rng = Rng(114);
            let perturbed: Vec<f64> = solution.iter().map(|x| x + 0.1 * rng.normal()).collect();
            for (start, few) in [(&solution, true), (&perturbed, false)] {
                let mut data = original.clone();
                let (outcome, coefficients) = project(
                    &mut projector,
                    &case,
                    weights,
                    (&mut data, ncols),
                    Some(start),
                    &request,
                );
                let report = projected(outcome);
                assert!(report.certificate.passed, "{route:?}");
                if few {
                    let (warm, cold) = (report.facts.iterations, cold.facts.iterations);
                    assert!(warm <= cold, "{route:?}: {:?}", report.facts);
                }
                for j in 0..ncols {
                    let y = column(&original, ncols, j);
                    let r = column(&data, ncols, j);
                    let expected = oracle.residual(&y);
                    let bound = 1e-7 * inf_norm(&y).max(1.0);
                    assert!(r.iter().zip(&expected).all(|(a, b)| (a - b).abs() <= bound));
                    let bound = 1e-8 * inf_norm(&y).max(1.0) * 16_777_216.0;
                    for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                        let fit = case.fitted(&coefficients, Some(j), i);
                        assert!((yi - fit - ri).abs() <= bound, "{route:?} {j} {i}");
                    }
                }
            }
        }
    }
}

/// Schwarz pairs with slopes need no intercept: two slope-only effects.
#[test]
fn schwarz_pairs_need_no_intercept() {
    let case = Case::random(105, 300, &[spec(20, false, 1), spec(7, false, 2)]);
    let weights = case.weights(106);
    for weights in [None, Some(weights.as_slice())] {
        let oracle = Oracle::new(&case, weights);
        for route in routes(&case)
            .into_iter()
            .filter(|&(_, p)| p != Preconditioner::Identity)
        {
            check(&case, weights, &oracle, route, (3, 107));
        }
    }
}

/// No channel cap: an effect with 17 or 33 channels (intercept and 16 or
/// 32 slopes) under Jacobi, Schwarz and the elimination, with the wide
/// effect on either side of the pair.
#[test]
fn wide_effects_match_dense_oracle() {
    for slopes in [16, 32] {
        for specs in [
            [spec(30, true, 0), spec(4, true, slopes)],
            [spec(12, true, slopes), spec(5, true, 0)],
        ] {
            let case = Case::random(111 + slopes as u64, 900, &specs);
            let weights = case.weights(112);
            for weights in [None, Some(weights.as_slice())] {
                let oracle = Oracle::new(&case, weights);
                let mut routes: Vec<_> = routes(&case)
                    .into_iter()
                    .filter(|&(_, p)| p != Preconditioner::Identity)
                    .map(|(m, p)| (Reduction::Full, m, p))
                    .collect();
                routes.push((
                    Reduction::EliminateLargest,
                    Method::Pcg,
                    Preconditioner::Jacobi,
                ));
                for route in routes {
                    check_route(&case, weights, &oracle, route, (3, 113));
                }
            }
        }
    }
}

/// A wide effect's blocks are admitted by memory: a limit below them is a
/// memory decline, not an unsupported request.
#[test]
fn wide_effects_are_admitted_by_memory() {
    let case = Case::random(115, 400, &[spec(30, true, 0), spec(4, true, 32)]);
    let mut data = case.columns(116, 2);
    let mut request = Request::new(TOLERANCE, MAX_ITERATIONS);
    request.memory_limit = Some(4096);
    for reduction in [Reduction::Full, Reduction::EliminateLargest] {
        let mut settings = full(Method::Pcg, Preconditioner::Jacobi);
        settings.choice.reduction = reduction;
        let columns = ColumnsMut::new(&mut data, case.n, 2).unwrap();
        let outcome = Projector::new(case.design(), settings)
            .project(Weights::Unit, columns, None, &request, &Control::default())
            .unwrap();
        assert!(
            matches!(
                outcome,
                Outcome::Declined {
                    reason: fedemean::Decline::MemoryLimit,
                    ..
                }
            ),
            "{reduction:?}: {outcome:?}"
        );
    }
}
