//! The full formulation ([`Reduction::Full`]): PCG and modified LSMR with
//! Jacobi or no preconditioner (Schwarz: `tests/schwarz.rs`) against an
//! independent dense weighted least-squares reference (modified
//! Gram-Schmidt on `sqrt(W) D`, copied from `tests/oracle.rs`), independent
//! normal-equation checks on larger
//! designs, warm starts, the Auto routing, unsupported combinations,
//! cancellation, memory admission and determinism.

use fedemean::{
    Choice, CoefficientLayout, CoefficientsMut, ColumnsMut, Control, Decline, Design, Effect,
    Error, Fallback, InputState, Method, MethodUsed, Outcome, Preconditioner, PreconditionerUsed,
    Projector, Reduction, ReductionUsed, Report, Request, SchwarzSpec, Settings, Weights,
};
use std::sync::atomic::{AtomicUsize, Ordering};

const TOLERANCE: f64 = 1e-10;
const MAX_ITERATIONS: usize = 1000;

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

/// A categorical design in which every level is observed.
struct Case {
    n: usize,
    levels: Vec<usize>,
    codes: Vec<Vec<u32>>,
}

impl Case {
    /// Independent uniform codes per effect, each level at least once.
    fn random(seed: u64, n: usize, levels: &[usize]) -> Self {
        let mut rng = Rng(seed);
        let codes = levels
            .iter()
            .map(|&count| {
                assert!(count <= n);
                let mut codes: Vec<u32> = (0..n)
                    .map(|i| {
                        if i < count {
                            i as u32
                        } else {
                            rng.below(count) as u32
                        }
                    })
                    .collect();
                for i in (1..n).rev() {
                    codes.swap(i, rng.below(i + 1));
                }
                codes
            })
            .collect();
        Self {
            n,
            levels: levels.to_vec(),
            codes,
        }
    }

    fn design(&self) -> Design<'_> {
        let effects = self
            .codes
            .iter()
            .zip(&self.levels)
            .map(|(codes, &levels)| Effect::new(codes, levels))
            .collect();
        Design::new(self.n, effects).unwrap()
    }

    fn per_rhs(&self) -> usize {
        self.levels.iter().sum()
    }

    fn offsets(&self) -> Vec<usize> {
        self.levels
            .iter()
            .scan(0, |next, &levels| {
                let offset = *next;
                *next += levels;
                Some(offset)
            })
            .collect()
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

    /// `ncols` observation-major columns: noise dominated, exactly in the
    /// design span, inside the first effect's span, and mixed; each with its
    /// own constant.
    fn columns(&self, seed: u64, ncols: usize) -> Vec<f64> {
        let mut rng = Rng(seed);
        let mut data = vec![0.0; self.n * ncols];
        for j in 0..ncols {
            let effects: Vec<Vec<f64>> = self
                .levels
                .iter()
                .map(|&levels| (0..levels).map(|_| rng.normal()).collect())
                .collect();
            let (first, rest, noise) = match j % 4 {
                0 => (4.0, 0.7, 1.0),
                1 => (2.0, 3.0, 0.0),
                2 => (3.0, 0.0, 0.0),
                _ => (1.0, 1.5, 2.0),
            };
            let constant = 10.0 * j as f64 - 3.0;
            for i in 0..self.n {
                let mut value = constant + noise * rng.normal();
                for (e, (codes, alpha)) in self.codes.iter().zip(&effects).enumerate() {
                    let scale = if e == 0 { first } else { rest };
                    value += scale * alpha[codes[i] as usize];
                }
                data[i * ncols + j] = value;
            }
        }
        data
    }

    /// `D b` of column `j`'s coefficients (canonical layout) at row `i`.
    fn fitted(&self, coefficients: &[f64], j: usize, i: usize) -> f64 {
        let base = j * self.per_rhs();
        self.codes
            .iter()
            .zip(self.offsets())
            .map(|(codes, offset)| coefficients[base + offset + codes[i] as usize])
            .sum()
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
/// column space of `sqrt(W) D`, dropping dummy columns whose remaining norm
/// after projection is below `1e-10` of their own.
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
        for (codes, &levels) in case.codes.iter().zip(&case.levels) {
            for level in 0..levels {
                let mut v: Vec<f64> = codes
                    .iter()
                    .zip(&root_weights)
                    .map(|(&code, &root)| if code as usize == level { root } else { 0.0 })
                    .collect();
                let original = dot(&v, &v).sqrt();
                subtract_projections(&basis, &mut v);
                let remaining = dot(&v, &v).sqrt();
                if remaining > 1e-10 * original {
                    for x in &mut v {
                        *x /= remaining;
                    }
                    basis.push(v);
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
        Outcome::Declined { reason, .. } => panic!("unexpected decline: {reason:?}"),
        _ => panic!("unexpected outcome"),
    }
}

fn declined(outcome: Outcome) -> (Decline, InputState) {
    match outcome {
        Outcome::Declined { reason, input, .. } => (reason, input),
        Outcome::Projected(_) => panic!("unexpected projection"),
        _ => panic!("unexpected outcome"),
    }
}

fn settings(reduction: Reduction, method: Method, preconditioner: Preconditioner) -> Settings {
    Settings {
        choice: Choice {
            reduction,
            method,
            preconditioner,
            ..Choice::default()
        },
        lanes: None,
    }
}

fn full(method: Method, preconditioner: Preconditioner) -> Settings {
    settings(Reduction::Full, method, preconditioner)
}

/// The full-formulation routes this build supports.
fn routes() -> Vec<(Method, Preconditioner)> {
    vec![
        (Method::Pcg, Preconditioner::Jacobi),
        (Method::Pcg, Preconditioner::Identity),
        (Method::Lsmr, Preconditioner::Jacobi),
        (Method::Lsmr, Preconditioner::Identity),
    ]
}

fn request() -> Request {
    Request::new(TOLERANCE, MAX_ITERATIONS)
}

/// Project `data` in place (from `start` when given), filling and returning
/// a zeroed coefficient buffer of the canonical layout.
fn project(
    projector: &mut Projector<'_>,
    case: &Case,
    weights: Option<&[f64]>,
    (data, ncols): (&mut [f64], usize),
    start: Option<&[f64]>,
    request: &Request,
) -> (Outcome, Vec<f64>) {
    let offsets = case.offsets();
    let mut coefficients = vec![0.0; case.per_rhs() * ncols];
    let layout = CoefficientLayout::new(case.per_rhs(), &offsets, &case.levels);
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

fn expected_route(report: &Report, method: Method, preconditioner: Preconditioner) {
    assert_eq!(report.route.reduction, ReductionUsed::Full);
    let method_used = match method {
        Method::Lsmr => MethodUsed::Lsmr,
        _ => MethodUsed::Pcg,
    };
    assert_eq!(report.route.method, method_used);
    let used = match preconditioner {
        Preconditioner::Identity => PreconditionerUsed::Identity,
        _ => PreconditionerUsed::Jacobi,
    };
    assert_eq!(report.route.preconditioner, used);
    assert_eq!(report.route.escalations, 0);
}

/// Residuals, coefficients, norms, reports, Gram and `certify_residual` of
/// one projection against the dense reference.
fn check_against_oracle(
    case: &Case,
    weights: Option<&[f64]>,
    oracle: &Oracle,
    projector: &mut Projector<'_>,
    (method, preconditioner): (Method, Preconditioner),
    (ncols, seed): (usize, u64),
) {
    let original = case.columns(seed, ncols);
    let mut data = original.clone();
    let mut req = request();
    req.residual_gram = true;
    let (outcome, coefficients) = project(projector, case, weights, (&mut data, ncols), None, &req);
    let report = projected(outcome);
    let label = format!(
        "{:?} {method:?}/{preconditioner:?} {ncols} cols",
        case.levels
    );
    assert!(
        report.certificate.passed,
        "{label}: {:?}",
        report.certificate
    );
    expected_route(&report, method, preconditioner);
    assert_eq!(report.facts.reduced, case.per_rhs());
    let weight_max = weights.map_or(1.0, |w| w.iter().fold(0.0_f64, |m, &v| m.max(v)));
    assert_eq!(report.weight_scale, weight_max);
    let (reports, norms) = (projector.columns(), projector.residual_norms());
    assert_eq!((reports.len(), norms.len()), (ncols, ncols));
    let mut residuals = Vec::with_capacity(ncols);
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
        let bound = 1e-9 * inf_norm(&y).max(1.0);
        for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
            let fit = case.fitted(&coefficients, j, i);
            assert!(
                (yi - fit - ri).abs() <= bound,
                "{label}, column {j}, row {i}: y - Db = {} vs residual {ri}",
                yi - fit
            );
        }
        let direct = r
            .iter()
            .enumerate()
            .map(|(i, &ri)| weights.map_or(1.0, |w| w[i] / weight_max) * ri * ri)
            .sum::<f64>()
            .sqrt();
        assert!((norms[j] - direct).abs() <= 1e-12 * direct.max(f64::MIN_POSITIVE));
        assert!(
            reports[j].relative_moment <= TOLERANCE,
            "{label}: {:?}",
            reports[j]
        );
        assert_eq!(reports[j].residual_norm, norms[j]);
        residuals.push(r);
    }
    match projector.residual_gram() {
        Some(gram) => {
            assert!(ncols <= 8);
            let raw = |i: usize| weights.map_or(1.0, |w| w[i]);
            for a in 0..ncols {
                for b in 0..ncols {
                    let direct: f64 = (0..case.n)
                        .map(|i| raw(i) * residuals[a][i] * residuals[b][i])
                        .sum();
                    let scale =
                        gram.values()[a * ncols + a].sqrt() * gram.values()[b * ncols + b].sqrt();
                    let value = gram.values()[a * ncols + b];
                    assert!((value - direct).abs() <= 1e-12 * scale.max(f64::MIN_POSITIVE));
                }
            }
        }
        None => assert!(ncols > 8, "{label}: the Gram is missing"),
    }
    let certificate = projector
        .certify_residual(as_weights(weights), &residuals[0], TOLERANCE, 1.0)
        .unwrap()
        .expect("same weights certify");
    assert!(certificate.passed, "{label}: {certificate:?}");
    let other = case.weights(seed ^ 0xabcd);
    let other = projector
        .certify_residual(Weights::Values(&other), &residuals[0], TOLERANCE, 1.0)
        .unwrap();
    assert!(other.is_none());
}

/// Every supported full route, unit and varying weights, 1/3/11 columns.
fn check_case(seed: u64, n: usize, levels: &[usize]) {
    let case = Case::random(seed, n, levels);
    let weights = case.weights(seed + 1);
    for weights in [None, Some(weights.as_slice())] {
        let oracle = Oracle::new(&case, weights);
        for route in routes() {
            let mut projector = Projector::new(case.design(), full(route.0, route.1));
            for (k, ncols) in [1, 3, 11].into_iter().enumerate() {
                let seed = seed + 10 + k as u64;
                check_against_oracle(
                    &case,
                    weights,
                    &oracle,
                    &mut projector,
                    route,
                    (ncols, seed),
                );
            }
        }
    }
}

#[test]
fn one_effect_matches_dense_oracle() {
    check_case(101, 300, &[40]);
}

#[test]
fn two_effects_match_dense_oracle() {
    check_case(102, 500, &[120, 17]);
}

#[test]
fn three_effects_match_dense_oracle() {
    check_case(103, 600, &[150, 23, 6]);
}

#[test]
fn four_effects_match_dense_oracle() {
    check_case(104, 700, &[23, 150, 6, 11]);
}

#[test]
fn five_effects_match_dense_oracle() {
    check_case(105, 900, &[150, 23, 6, 11, 4]);
}

#[test]
fn six_effects_match_dense_oracle() {
    check_case(106, 900, &[150, 23, 6, 11, 4, 9]);
}

#[test]
fn seven_effects_match_dense_oracle() {
    check_case(107, 1000, &[150, 23, 6, 11, 4, 9, 30]);
}

/// Every level's relative weighted moment of `r` at `max(||y||, ||r||)`.
fn largest_relative_moment(case: &Case, weights: Option<&[f64]>, y: &[f64], r: &[f64]) -> f64 {
    let weight_max = weights.map_or(1.0, |w| w.iter().fold(0.0_f64, |m, &v| m.max(v)));
    let w = |i: usize| weights.map_or(1.0, |w| w[i] / weight_max);
    let norm = |v: &[f64]| {
        v.iter()
            .enumerate()
            .map(|(i, x)| w(i) * x * x)
            .sum::<f64>()
            .sqrt()
    };
    let scale = norm(y).max(norm(r));
    let mut largest = 0.0_f64;
    for (codes, &levels) in case.codes.iter().zip(&case.levels) {
        let mut moment = vec![0.0; levels];
        let mut mass = vec![0.0; levels];
        for (i, &code) in codes.iter().enumerate() {
            moment[code as usize] += w(i) * r[i];
            mass[code as usize] += w(i);
        }
        for (m, mass) in moment.iter().zip(&mass) {
            largest = largest.max(m.abs() / mass.sqrt() / scale);
        }
    }
    largest
}

/// A larger design (gathered and scattered effects, parallel passes):
/// independent normal-equation checks, agreement with the elimination,
/// bitwise identity across pool widths and lane settings (the full
/// formulation does not depend on either).
#[test]
fn large_designs_satisfy_normal_equations_and_are_pool_independent() {
    let case = Case::random(111, 12_000, &[1500, 1100, 12, 7]);
    let weights = case.weights(112);
    let ncols = 5;
    let original = case.columns(113, ncols);
    let run = |settings: Settings, threads: usize, lanes: usize, weights: Option<&[f64]>| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            let settings = Settings {
                lanes: Some(lanes),
                ..settings
            };
            let mut projector = Projector::new(case.design(), settings);
            let mut data = original.clone();
            let (outcome, coefficients) = project(
                &mut projector,
                &case,
                weights,
                (&mut data, ncols),
                None,
                &request(),
            );
            assert!(projected(outcome).certificate.passed);
            (data, coefficients)
        })
    };
    let eliminated = settings(
        Reduction::EliminateLargest,
        Method::Auto,
        Preconditioner::Auto,
    );
    for weights in [None, Some(weights.as_slice())] {
        let (reference, _) = run(eliminated, 4, 4, weights);
        for (method, preconditioner) in routes()
            .into_iter()
            .filter(|&(_, p)| p == Preconditioner::Jacobi)
        {
            let settings = full(method, preconditioner);
            let (four, coefficients) = run(settings, 4, 4, weights);
            for j in 0..ncols {
                let y = column(&original, ncols, j);
                let r = column(&four, ncols, j);
                let moment = largest_relative_moment(&case, weights, &y, &r);
                assert!(
                    moment <= TOLERANCE,
                    "{method:?} column {j}: moment {moment}"
                );
                let bound = 1e-9 * inf_norm(&y).max(1.0);
                for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                    let fit = case.fitted(&coefficients, j, i);
                    assert!((yi - fit - ri).abs() <= bound, "column {j}, row {i}");
                }
                let bound = 1e-7 * inf_norm(&y).max(1.0);
                let elim = column(&reference, ncols, j);
                assert!(r.iter().zip(&elim).all(|(a, b)| (a - b).abs() <= bound));
            }
            for (threads, lanes) in [(1, 1), (3, 4), (4, 1)] {
                let (other, other_coefficients) = run(settings, threads, lanes, weights);
                assert_eq!(bits(&four), bits(&other), "{method:?} {threads} {lanes}");
                assert_eq!(bits(&coefficients), bits(&other_coefficients));
            }
        }
    }
}

#[test]
fn fresh_full_projectors_are_bitwise_repeatable() {
    let case = Case::random(121, 900, &[150, 23, 6, 11, 4, 9]);
    let weights = case.weights(122);
    for (method, preconditioner) in routes() {
        for weights in [None, Some(weights.as_slice())] {
            let run = || {
                let mut data = case.columns(123, 11);
                let mut projector = Projector::new(case.design(), full(method, preconditioner));
                let (outcome, coefficients) = project(
                    &mut projector,
                    &case,
                    weights,
                    (&mut data, 11),
                    None,
                    &request(),
                );
                projected(outcome);
                (bits(&data), bits(&coefficients))
            };
            assert_eq!(run(), run());
        }
    }
}

/// Warm starts: from a tighter solution none (PCG) or one (LSMR) iteration;
/// along a null direction the same; from a perturbed start the same
/// projection, with coefficients that are the full solution.
#[test]
fn warm_starts_continue_from_their_coefficients() {
    let case = Case::random(131, 700, &[150, 23, 6, 11, 4, 9]);
    let weights = case.weights(132);
    let ncols = 5;
    let original = case.columns(133, ncols);
    let per = case.per_rhs();
    for (method, preconditioner) in routes() {
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
            // The same solution shifted along a null direction (a constant
            // moved from the first effect to the second).
            let mut shifted = solution.clone();
            for j in 0..ncols {
                for l in 0..case.levels[0] {
                    shifted[j * per + l] += 0.75;
                }
                for l in 0..case.levels[1] {
                    shifted[j * per + case.levels[0] + l] -= 0.75;
                }
            }
            let mut rng = Rng(134);
            let perturbed: Vec<f64> = solution.iter().map(|x| x + 0.1 * rng.normal()).collect();
            let label = format!(
                "{method:?}/{preconditioner:?} weighted {}",
                weights.is_some()
            );
            for (start, few) in [(&solution, true), (&shifted, true), (&perturbed, false)] {
                let mut data = original.clone();
                let (outcome, coefficients) = project(
                    &mut projector,
                    &case,
                    weights,
                    (&mut data, ncols),
                    Some(start),
                    &request(),
                );
                let report = projected(outcome);
                assert!(report.certificate.passed, "{label}");
                if few {
                    let limit = usize::from(method == Method::Lsmr);
                    assert!(
                        report.facts.iterations <= limit,
                        "{label}: {:?}",
                        report.facts
                    );
                }
                for j in 0..ncols {
                    let y = column(&original, ncols, j);
                    let r = column(&data, ncols, j);
                    let bound = 1e-7 * inf_norm(&y).max(1.0);
                    let expected = oracle.residual(&y);
                    assert!(r.iter().zip(&expected).all(|(a, b)| (a - b).abs() <= bound));
                    let bound = 1e-9 * inf_norm(&y).max(1.0);
                    for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                        let fit = case.fitted(&coefficients, j, i);
                        assert!(
                            (yi - fit - ri).abs() <= bound,
                            "{label}: column {j}, row {i}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn warm_starts_are_validated() {
    let case = Case::random(141, 300, &[40, 7, 5, 3, 2, 2]);
    let mut data = case.columns(142, 2);
    let original = data.clone();
    let mut projector = Projector::new(case.design(), Settings::default());
    for start in [vec![0.0; case.per_rhs() * 2 - 1], {
        let mut start = vec![0.0; case.per_rhs() * 2];
        start[3] = f64::NAN;
        start
    }] {
        let error = projector
            .project_from(
                Weights::Unit,
                ColumnsMut::new(&mut data, case.n, 2).unwrap(),
                &start,
                None,
                &request(),
                &Control::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            Error::InvalidInput(_) | Error::InvalidData(_)
        ));
        assert_eq!(bits(&data), bits(&original));
    }
    // The elimination takes no warm start.
    let case = Case::random(143, 300, &[40, 7, 5]);
    let mut data = case.columns(144, 2);
    let start = vec![0.0; case.per_rhs() * 2];
    let error = Projector::new(case.design(), Settings::default())
        .project_from(
            Weights::Unit,
            ColumnsMut::new(&mut data, case.n, 2).unwrap(),
            &start,
            None,
            &request(),
            &Control::default(),
        )
        .unwrap_err();
    assert!(matches!(error, Error::Unsupported(_)), "{error:?}");
}

/// The route of one default-request projection of three columns.
fn route_of(case: &Case, settings: Settings) -> Result<Outcome, Error> {
    let mut data = case.columns(151, 3);
    Projector::new(case.design(), settings).project(
        Weights::Unit,
        ColumnsMut::new(&mut data, case.n, 3).unwrap(),
        None,
        &request(),
        &Control::default(),
    )
}

#[test]
fn auto_eliminates_two_to_five_effects_and_uses_the_full_system_otherwise() {
    let auto = Settings::default();
    for levels in [
        &[40, 7][..],
        &[40, 7, 5][..],
        &[40, 7, 5, 3][..],
        &[40, 7, 5, 3, 2][..],
    ] {
        let case = Case::random(152, 400, levels);
        let report = projected(route_of(&case, auto).unwrap());
        assert_eq!(
            report.route.reduction,
            ReductionUsed::Eliminated { effect: 0 }
        );
        assert_eq!(report.route.method, MethodUsed::Pcg);
        // Auto is the explicit elimination, bit for bit.
        let explicit = settings(
            Reduction::EliminateLargest,
            Method::Pcg,
            Preconditioner::Auto,
        );
        let (mut a, mut b) = (case.columns(153, 3), case.columns(153, 3));
        for (data, settings) in [(&mut a, auto), (&mut b, explicit)] {
            let mut projector = Projector::new(case.design(), settings);
            let (outcome, _) = project(&mut projector, &case, None, (data, 3), None, &request());
            projected(outcome);
        }
        assert_eq!(bits(&a), bits(&b));
    }
    for levels in [
        &[40][..],
        &[40, 7, 5, 3, 2, 2][..],
        &[40, 7, 5, 3, 2, 2, 3][..],
    ] {
        let case = Case::random(154, 400, levels);
        let report = projected(route_of(&case, auto).unwrap());
        assert_eq!(report.route.reduction, ReductionUsed::Full);
        assert_eq!(report.route.method, MethodUsed::Pcg);
        // The full formulation's Auto: Schwarz (feature schwarz) for two or
        // more effects, Jacobi for one (where it is exact).
        let schwarz = cfg!(feature = "schwarz") && levels.len() > 1;
        assert_eq!(
            matches!(
                report.route.preconditioner,
                PreconditionerUsed::Schwarz { .. }
            ),
            schwarz,
            "{levels:?}: {:?}",
            report.route.preconditioner
        );
        if !schwarz {
            assert_eq!(report.route.preconditioner, PreconditionerUsed::Jacobi);
        }
        // Block-Jacobi always means the elimination, which declines.
        let blocks = settings(
            Reduction::Auto,
            Method::Auto,
            Preconditioner::BlockJacobi(Default::default()),
        );
        let (reason, input) = declined(route_of(&case, blocks).unwrap());
        assert_eq!(
            (reason, input),
            (Decline::OutsideRoute, InputState::Untouched)
        );
    }
    // An explicit method or preconditioner of the full system selects it.
    let case = Case::random(155, 400, &[40, 7, 5]);
    let identity = settings(Reduction::Auto, Method::Auto, Preconditioner::Identity);
    let report = projected(route_of(&case, identity).unwrap());
    assert_eq!(report.route.reduction, ReductionUsed::Full);
    assert_eq!(report.route.preconditioner, PreconditionerUsed::Identity);
    let lsmr = settings(Reduction::Auto, Method::Lsmr, Preconditioner::Auto);
    let report = projected(route_of(&case, lsmr).unwrap());
    assert_eq!(report.route.reduction, ReductionUsed::Full);
    assert_eq!(report.route.method, MethodUsed::Lsmr);
}

#[test]
fn unsupported_combinations_are_errors_before_any_change() {
    let case = Case::random(161, 400, &[40, 7, 5]);
    let schwarz = Preconditioner::Schwarz(SchwarzSpec::default());
    let blocks = Preconditioner::BlockJacobi(Default::default());
    let mut combinations = vec![
        settings(
            Reduction::EliminateLargest,
            Method::Lsmr,
            Preconditioner::Auto,
        ),
        settings(
            Reduction::EliminateLargest,
            Method::Auto,
            Preconditioner::Identity,
        ),
        settings(Reduction::EliminateLargest, Method::Auto, schwarz),
        settings(Reduction::Full, Method::Pcg, blocks),
    ];
    if !cfg!(feature = "schwarz") {
        combinations.push(settings(Reduction::Full, Method::Auto, schwarz));
        combinations.push(settings(Reduction::Auto, Method::Pcg, schwarz));
    }
    for settings in combinations {
        let mut data = case.columns(162, 3);
        let original = data.clone();
        let mut projector = Projector::new(case.design(), settings);
        assert!(matches!(
            projector.prepare(&Control::default()),
            Err(Error::Unsupported(_))
        ));
        let error = projector
            .project(
                Weights::Unit,
                ColumnsMut::new(&mut data, case.n, 3).unwrap(),
                None,
                &request(),
                &Control::default(),
            )
            .unwrap_err();
        assert!(
            matches!(error, Error::Unsupported(_)),
            "{settings:?}: {error:?}"
        );
        assert_eq!(bits(&data), bits(&original));
    }
}

#[test]
fn declines_leave_the_columns_untouched() {
    let case = Case::random(171, 600, &[150, 23, 6, 11, 4, 9]);
    let weights = case.weights(172);
    let original = case.columns(173, 3);
    for (method, preconditioner) in routes() {
        let mut projector = Projector::new(case.design(), full(method, preconditioner));
        assert!(projector.admission_bytes(3).is_none());
        assert!(projector.prepare(&Control::default()).unwrap());
        assert!(projector.admission_bytes(3).unwrap() > 0);
        let mut limited = request();
        limited.memory_limit = Some(1);
        let mut data = original.clone();
        let (outcome, coefficients) = project(
            &mut projector,
            &case,
            Some(&weights),
            (&mut data, 3),
            None,
            &limited,
        );
        let (reason, input) = declined(outcome);
        assert_eq!(
            (reason, input),
            (Decline::MemoryLimit, InputState::Untouched)
        );
        assert_eq!(bits(&data), bits(&original));
        assert!(coefficients.iter().all(|&c| c == 0.0));
        // An unreachable tolerance within one iteration; escalation has no
        // stronger preconditioner to try.
        let mut escalating = Projector::new(
            case.design(),
            Settings {
                choice: Choice {
                    fallback: Fallback::Escalate,
                    ..full(method, preconditioner).choice
                },
                lanes: None,
            },
        );
        for projector in [&mut projector, &mut escalating] {
            let mut data = original.clone();
            let capped = Request::new(1e-14, 1);
            let (outcome, coefficients) =
                project(projector, &case, None, (&mut data, 3), None, &capped);
            let (reason, input) = declined(outcome);
            assert!(matches!(reason, Decline::Numerical(_)), "{reason:?}");
            assert_eq!(input, InputState::Untouched);
            assert_eq!(bits(&data), bits(&original));
            assert!(coefficients.iter().all(|&c| c == 0.0));
            assert!(projector.columns().is_empty() && projector.residual_norms().is_empty());
        }
        let mut data = original.clone();
        let (outcome, _) = project(
            &mut projector,
            &case,
            None,
            (&mut data, 3),
            None,
            &request(),
        );
        assert!(projected(outcome).certificate.passed);
    }
    // A level without observations has no mass.
    let mut codes = case.codes.clone();
    codes[2].iter_mut().for_each(|c| *c = (*c).min(4));
    let effects = codes
        .iter()
        .zip(&case.levels)
        .map(|(codes, &levels)| Effect::new(codes, levels))
        .collect();
    let design = Design::new(case.n, effects).unwrap();
    let mut data = original.clone();
    let outcome = Projector::new(design, Settings::default())
        .project(
            Weights::Unit,
            ColumnsMut::new(&mut data, case.n, 3).unwrap(),
            None,
            &request(),
            &Control::default(),
        )
        .unwrap();
    let (reason, input) = declined(outcome);
    assert_eq!((reason, input), (Decline::Weights, InputState::Untouched));
    assert_eq!(bits(&data), bits(&original));
}

#[test]
fn invalid_codes_and_weights_are_errors() {
    // Scattered (small) and gathered (more than 1024 levels) effects.
    let case = Case::random(181, 1500, &[40, 1200, 5, 3, 2, 2]);
    let original = case.columns(182, 2);
    let project_codes = |codes: &[Vec<u32>], weights: Weights<'_>| {
        let effects = codes
            .iter()
            .zip(&case.levels)
            .map(|(codes, &levels)| Effect::new(codes, levels))
            .collect();
        let mut data = original.clone();
        Projector::new(Design::new(case.n, effects).unwrap(), Settings::default()).project(
            weights,
            ColumnsMut::new(&mut data, case.n, 2).unwrap(),
            None,
            &request(),
            &Control::default(),
        )
    };
    for effect in [0, 1, 5] {
        let mut codes = case.codes.clone();
        codes[effect][9] = case.levels[effect] as u32;
        let error = project_codes(&codes, Weights::Unit).unwrap_err();
        assert!(matches!(error, Error::InvalidInput(_)), "{error:?}");
    }
    let mut weights = case.weights(183);
    weights[17] = 0.0;
    let error = project_codes(&case.codes, Weights::Values(&weights)).unwrap_err();
    assert!(matches!(error, Error::InvalidData(_)), "{error:?}");
}

#[test]
fn cancellation_stops_the_iterations() {
    let case = Case::random(191, 900, &[150, 23, 6, 11, 4, 9]);
    for (method, preconditioner) in routes() {
        let polls = AtomicUsize::new(0);
        let later = || polls.fetch_add(1, Ordering::Relaxed) >= 3;
        let always = || true;
        let mut projector = Projector::new(case.design(), full(method, preconditioner));
        for cancel in [&always as &(dyn Fn() -> bool + Sync), &later] {
            let mut data = case.columns(192, 3);
            let error = projector
                .project(
                    Weights::Unit,
                    ColumnsMut::new(&mut data, case.n, 3).unwrap(),
                    None,
                    &request(),
                    &Control::with_cancel(cancel),
                )
                .unwrap_err();
            assert_eq!(error, Error::Cancelled, "{method:?}");
        }
        assert!(polls.load(Ordering::Relaxed) >= 4);
        let mut data = case.columns(192, 3);
        let report = projected(
            projector
                .project(
                    Weights::Unit,
                    ColumnsMut::new(&mut data, case.n, 3).unwrap(),
                    None,
                    &request(),
                    &Control::with_cancel(&|| false),
                )
                .unwrap(),
        );
        assert!(report.certificate.passed && report.facts.iterations > 3);
    }
}

/// With coefficient stopping, every level's Jacobi estimate `|m_l| / mass_l`
/// of the returned residual is within the tolerance of the column's
/// root-mean-square scale (`scale / sqrt(total mass)`, with slack for the
/// recurrence's drift from the formed residual).
#[test]
fn coefficient_stopping_bounds_the_jacobi_estimate() {
    let case = Case::random(201, 800, &[150, 23, 6, 11, 4, 9]);
    let weights = case.weights(202);
    let weight_max = weights.iter().fold(0.0_f64, |m, &v| m.max(v));
    let w = |i: usize| weights[i] / weight_max;
    let total: f64 = (0..case.n).map(w).sum();
    let ncols = 3;
    let original = case.columns(203, ncols);
    for (method, preconditioner) in routes() {
        let mut projector = Projector::new(case.design(), full(method, preconditioner));
        let mut stopping = request();
        stopping.coefficient_stopping = true;
        let mut data = original.clone();
        let (outcome, _) = project(
            &mut projector,
            &case,
            Some(&weights),
            (&mut data, ncols),
            None,
            &stopping,
        );
        assert!(projected(outcome).certificate.passed);
        for j in 0..ncols {
            let r = column(&data, ncols, j);
            let scale = projector.columns()[j].scale;
            let mut largest = 0.0_f64;
            for (codes, &levels) in case.codes.iter().zip(&case.levels) {
                let (mut moment, mut mass) = (vec![0.0; levels], vec![0.0; levels]);
                for (i, &code) in codes.iter().enumerate() {
                    moment[code as usize] += w(i) * r[i];
                    mass[code as usize] += w(i);
                }
                for (m, mass) in moment.iter().zip(&mass) {
                    largest = largest.max(m.abs() / mass);
                }
            }
            let bound = TOLERANCE * scale / total.sqrt();
            assert!(
                largest <= bound,
                "{method:?} column {j}: {largest} > {bound}"
            );
        }
    }
}
