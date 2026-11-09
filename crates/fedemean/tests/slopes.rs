//! Varying slopes on the full formulation ([`Effect::with_slopes`]): PCG and
//! modified LSMR with the levels' channel blocks, the hybrid additive
//! Schwarz (factor-pair subdomains on the intercept channels plus the level
//! blocks) or no preconditioner against an independent dense weighted
//! least-squares reference on the
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

    fn intercepts(&self) -> usize {
        self.specs.iter().filter(|s| s.intercept).count()
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
                data[i * ncols + j] =
                    constant + noise * rng.normal() + self.fitted(&alpha, None, i);
            }
        }
        data
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

/// The routes this build supports for `case` (Schwarz needs two effects
/// with intercepts).
fn routes(case: &Case) -> Vec<(Method, Preconditioner)> {
    let mut routes = vec![
        (Method::Auto, Preconditioner::Auto),
        (Method::Pcg, Preconditioner::Jacobi),
        (Method::Pcg, Preconditioner::Identity),
    ];
    if cfg!(feature = "schwarz") {
        routes.push((Method::Lsmr, Preconditioner::Jacobi));
        routes.push((Method::Lsmr, Preconditioner::Identity));
        if case.intercepts() >= 2 {
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
        Preconditioner::Auto if cfg!(feature = "schwarz") && case.intercepts() >= 2 => schwarz,
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
    (ncols, seed): (usize, u64),
) -> PreconditionerUsed {
    let original = case.columns(seed, ncols);
    let mut data = original.clone();
    let mut projector = Projector::new(case.design(), full(method, preconditioner));
    let request = Request::new(TOLERANCE, MAX_ITERATIONS);
    let (outcome, coefficients) = project(
        &mut projector,
        case,
        weights,
        (&mut data, ncols),
        None,
        &request,
    );
    let label = format!(
        "{:?} {method:?}/{preconditioner:?} {ncols} cols weighted {}",
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
    assert_eq!(report.route.reduction, ReductionUsed::Full, "{label}");
    let method_used = if method == Method::Lsmr {
        MethodUsed::Lsmr
    } else {
        MethodUsed::Pcg
    };
    assert_eq!(report.route.method, method_used, "{label}");
    assert_eq!(report.route.escalations, 0, "{label}");
    assert_eq!(report.facts.reduced, case.per_rhs(), "{label}");
    let used = report.route.preconditioner;
    assert!(resolved(case, preconditioner, used), "{label}: {used:?}");
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
        assert!(report.relative_moment <= TOLERANCE, "{label}");
        // Recomputed at the projection's own scale.
        if report.residual_norm > 0.0 {
            let amplification = (report.scale / report.residual_norm).max(1.0);
            let certificate = projector
                .certify_residual(as_weights(weights), &r, TOLERANCE, amplification)
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

/// Auto routes slopes to the full formulation: Schwarz when it builds, or
/// with escalation the channel blocks first (an easy design keeps them);
/// the elimination, block-Jacobi, Schwarz without two intercept effects and
/// too many channels are unsupported; inputs are validated.
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
        assert_eq!(report.route.reduction, ReductionUsed::Full);
        assert_eq!(report.route.method, MethodUsed::Pcg);
        let schwarz = cfg!(feature = "schwarz") && fallback == Fallback::Decline;
        let used = report.route.preconditioner;
        if schwarz {
            assert!(
                matches!(used, PreconditionerUsed::Schwarz { .. }),
                "{used:?}"
            );
        } else {
            assert_eq!(used, PreconditionerUsed::ChannelBlocks { singular: 0 });
        }
        assert_eq!(report.route.escalations, 0);
    }
    let unsupported = |choice: Choice| {
        assert!(
            matches!(run(choice), Err(Error::Unsupported(_))),
            "{choice:?}"
        );
    };
    unsupported(Choice {
        reduction: Reduction::EliminateLargest,
        ..Choice::default()
    });
    unsupported(Choice {
        preconditioner: Preconditioner::BlockJacobi(fedemean::BlockSpec::default()),
        ..Choice::default()
    });
    // Seventeen channels.
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
    assert!(matches!(result, Err(Error::Unsupported(_))));
    // Schwarz with one intercept effect.
    let single = Effect::new(&case.codes[1], 6)
        .with_slopes(false, [case.slopes[1][0].as_slice()])
        .unwrap();
    let design = Design::new(case.n, vec![Effect::new(&case.codes[0], 20), single]).unwrap();
    let mut data = original.clone();
    let columns = ColumnsMut::new(&mut data, case.n, 2).unwrap();
    let result = Projector::new(design, full(Method::Pcg, SCHWARZ)).project(
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
    // A warm start must have every channel's coefficients.
    let mut projector = Projector::new(case.design(), Settings::default());
    let mut data = original.clone();
    let columns = ColumnsMut::new(&mut data, case.n, 2).unwrap();
    let start = vec![0.0; (20 + 6) * 2];
    let result = projector.project_from(
        Weights::Unit,
        columns,
        &start,
        None,
        &request,
        &Control::default(),
    );
    assert!(matches!(result, Err(Error::InvalidInput(_))));
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
