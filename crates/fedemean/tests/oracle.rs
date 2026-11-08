//! Independent checks of the projection: a dense weighted least-squares
//! reference written here (rank-revealing modified Gram-Schmidt with
//! reorthogonalization on `sqrt(W) D`, the explicit dummy matrix), an
//! independent moment (normal-equation) check on a larger design, and the
//! documented declines, errors and determinism guarantees.

use fedemean::{
    Choice, CoefficientLayout, CoefficientsMut, ColumnsMut, Control, Decline, Design, Effect,
    Error, InputState, Members, Outcome, Projector, Reduction, ReductionUsed, Report, Request,
    Settings, Weights,
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

    fn effects(&self) -> Vec<Effect<'_>> {
        self.codes
            .iter()
            .zip(&self.levels)
            .map(|(codes, &levels)| Effect::new(codes, levels))
            .collect()
    }

    fn design(&self) -> Design<'_> {
        Design::new(self.n, self.effects()).unwrap()
    }

    /// The effect with the most levels (the first of equals).
    fn largest(&self) -> usize {
        let most = *self.levels.iter().max().unwrap();
        self.levels.iter().position(|&l| l == most).unwrap()
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

    /// `ncols` observation-major columns. Column kinds cycle through: noise
    /// dominated, exactly in the design span, inside the eliminated span,
    /// and mixed; every column carries its own constant.
    fn columns(&self, seed: u64, ncols: usize) -> Vec<f64> {
        let mut rng = Rng(seed);
        let largest = self.largest();
        let mut data = vec![0.0; self.n * ncols];
        for j in 0..ncols {
            let effects: Vec<Vec<f64>> = self
                .levels
                .iter()
                .map(|&levels| (0..levels).map(|_| rng.normal()).collect())
                .collect();
            let (eliminated, rest, noise) = match j % 4 {
                0 => (4.0, 0.7, 1.0),
                1 => (2.0, 3.0, 0.0),
                2 => (3.0, 0.0, 0.0),
                _ => (1.0, 1.5, 2.0),
            };
            let constant = 10.0 * j as f64 - 3.0;
            for i in 0..self.n {
                let mut value = constant + noise * rng.normal();
                for (e, (codes, alpha)) in self.codes.iter().zip(&effects).enumerate() {
                    let scale = if e == largest { eliminated } else { rest };
                    value += scale * alpha[codes[i] as usize];
                }
                data[i * ncols + j] = value;
            }
        }
        data
    }

    /// `D b` of column `j`'s coefficients at observation `i`.
    fn fitted(&self, coefficients: &[f64], offsets: &[usize], j: usize, i: usize) -> f64 {
        let base = j * self.per_rhs();
        self.codes
            .iter()
            .zip(offsets)
            .map(|(codes, &offset)| coefficients[base + offset + codes[i] as usize])
            .sum()
    }
}

/// Stable counting sort of one effect's codes into level members.
fn members_of(codes: &[u32], levels: usize) -> (Vec<usize>, Vec<usize>) {
    let mut offsets = vec![0_usize; levels + 1];
    for &code in codes {
        offsets[code as usize + 1] += 1;
    }
    for level in 0..levels {
        offsets[level + 1] += offsets[level];
    }
    let mut cursor = offsets[..levels].to_vec();
    let mut rows = vec![0_usize; codes.len()];
    for (row, &code) in codes.iter().enumerate() {
        rows[cursor[code as usize]] = row;
        cursor[code as usize] += 1;
    }
    (offsets, rows)
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
/// after projection is below `1e-10` of their own (the rank deficiency of
/// multiway designs).
struct Oracle {
    root_weights: Vec<f64>,
    basis: Vec<Vec<f64>>,
}

impl Oracle {
    fn new(case: &Case, weights: Option<&[f64]>) -> Self {
        let n = case.n;
        let root_weights: Vec<f64> = (0..n)
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

    fn rank(&self) -> usize {
        self.basis.len()
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

fn request() -> Request {
    Request::new(TOLERANCE, MAX_ITERATIONS)
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

/// Project `data` in place; with `coefficients`, a zeroed buffer of the
/// documented layout is filled and returned.
fn project(
    projector: &mut Projector<'_>,
    case: &Case,
    weights: Option<&[f64]>,
    data: &mut [f64],
    ncols: usize,
    with_coefficients: bool,
    request: &Request,
) -> (Outcome, Option<Vec<f64>>) {
    let offsets = case.offsets();
    let mut coefficients = vec![0.0; case.per_rhs() * ncols];
    let layout = CoefficientLayout::new(case.per_rhs(), &offsets, &case.levels);
    let columns = ColumnsMut::new(data, case.n, ncols).unwrap();
    let outcome = projector
        .project(
            as_weights(weights),
            columns,
            with_coefficients.then(|| CoefficientsMut::new(&mut coefficients, layout)),
            request,
            &Control::default(),
        )
        .unwrap();
    (outcome, with_coefficients.then_some(coefficients))
}

/// Every documented property of one certified projection against the
/// dense reference.
fn check_against_oracle(
    case: &Case,
    weights: Option<&[f64]>,
    oracle: &Oracle,
    projector: &mut Projector<'_>,
    ncols: usize,
    seed: u64,
) {
    let original = case.columns(seed, ncols);
    let mut data = original.clone();
    let mut req = request();
    req.residual_gram = true;
    let (outcome, coefficients) = project(projector, case, weights, &mut data, ncols, true, &req);
    let report = projected(outcome);
    let coefficients = coefficients.unwrap();
    assert!(report.certificate.passed, "{:?}", report.certificate);
    assert!(report.certificate.relative_infinity_norm <= TOLERANCE);
    assert!(matches!(
        report.route.reduction,
        ReductionUsed::Eliminated { effect } if effect == case.largest()
    ));
    let weight_max = weights.map_or(1.0, |w| w.iter().fold(0.0_f64, |m, &v| m.max(v)));
    assert_eq!(report.weight_scale, weight_max);

    let offsets = case.offsets();
    let reports = projector.columns();
    let norms = projector.residual_norms();
    assert_eq!(reports.len(), ncols);
    assert_eq!(norms.len(), ncols);
    let mut residuals = Vec::with_capacity(ncols);
    for j in 0..ncols {
        let y = column(&original, ncols, j);
        let r = column(&data, ncols, j);
        let bound = 1e-7 * inf_norm(&y).max(1.0);
        let expected = oracle.residual(&y);
        for (i, (&actual, &reference)) in r.iter().zip(&expected).enumerate() {
            assert!(
                (actual - reference).abs() <= bound,
                "column {j} of {ncols}, row {i}: {actual} vs oracle {reference}"
            );
        }
        let bound = 1e-9 * inf_norm(&y).max(1.0);
        for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
            let fit = case.fitted(&coefficients, &offsets, j, i);
            assert!(
                (yi - fit - ri).abs() <= bound,
                "column {j} of {ncols}, row {i}: y - Db = {} vs residual {ri}",
                yi - fit
            );
        }
        let direct = r
            .iter()
            .enumerate()
            .map(|(i, &ri)| weights.map_or(1.0, |w| w[i] / weight_max) * ri * ri)
            .sum::<f64>()
            .sqrt();
        assert!(
            (norms[j] - direct).abs() <= 1e-12 * direct.max(f64::MIN_POSITIVE),
            "residual norm {j}: {} vs {direct}",
            norms[j]
        );
        let column_report = reports[j];
        assert!(
            column_report.relative_moment.is_finite() && column_report.relative_moment <= TOLERANCE,
            "column {j}: {column_report:?}"
        );
        assert_eq!(column_report.residual_norm, norms[j]);
        residuals.push(r);
    }

    match projector.residual_gram() {
        Some(gram) => {
            assert!(ncols <= 8, "no Gram is formed for {ncols} columns");
            assert_eq!(gram.columns(), ncols);
            assert_eq!(gram.values().len(), ncols * ncols);
            let raw = |i: usize| weights.map_or(1.0, |w| w[i]);
            let direct: Vec<f64> = (0..ncols * ncols)
                .map(|k| {
                    let (a, b) = (k / ncols, k % ncols);
                    (0..case.n)
                        .map(|i| raw(i) * residuals[a][i] * residuals[b][i])
                        .sum()
                })
                .collect();
            for k in 0..ncols * ncols {
                let (a, b) = (k / ncols, k % ncols);
                let scale = (direct[a * ncols + a] * direct[b * ncols + b]).sqrt();
                assert!(
                    (gram.values()[k] - direct[k]).abs() <= 1e-12 * scale.max(f64::MIN_POSITIVE),
                    "Gram ({a}, {b}): {} vs {}",
                    gram.values()[k],
                    direct[k]
                );
            }
        }
        None => assert!(ncols > 8, "the Gram of {ncols} columns is missing"),
    }

    // Column 0 is noise dominated, so its moments certify at its own norm.
    let certificate = projector
        .certify_residual(as_weights(weights), &residuals[0], TOLERANCE, 1.0)
        .unwrap()
        .expect("same weights certify");
    assert!(certificate.passed, "{certificate:?}");
    let other: Vec<f64> = case.weights(seed ^ 0xabcd);
    assert!(projector
        .certify_residual(Weights::Values(&other), &residuals[0], TOLERANCE, 1.0)
        .unwrap()
        .is_none());
    if weights.is_some() {
        assert!(projector
            .certify_residual(Weights::Unit, &residuals[0], TOLERANCE, 1.0)
            .unwrap()
            .is_none());
    }
}

fn check_case(seed: u64, n: usize, levels: &[usize]) {
    let case = Case::random(seed, n, levels);
    let weights = case.weights(seed + 1);
    for weights in [None, Some(weights.as_slice())] {
        let oracle = Oracle::new(&case, weights);
        assert!(
            oracle.rank() < case.per_rhs(),
            "multiway designs are deficient"
        );
        let mut projector = Projector::new(case.design(), Settings::default());
        for (k, ncols) in [1, 3, 11].into_iter().enumerate() {
            check_against_oracle(
                &case,
                weights,
                &oracle,
                &mut projector,
                ncols,
                seed + 10 + k as u64,
            );
        }
    }
}

#[test]
fn two_effects_match_dense_oracle() {
    check_case(11, 500, &[120, 17]);
}

#[test]
fn three_effects_match_dense_oracle() {
    check_case(12, 600, &[150, 23, 6]);
}

#[test]
fn four_effects_match_dense_oracle() {
    check_case(13, 700, &[150, 23, 6, 11]);
}

#[test]
fn five_effects_match_dense_oracle() {
    check_case(14, 900, &[150, 23, 6, 11, 4]);
}

/// Rows that repeat a remaining-code combination within their eliminated
/// group, and the cell count of one projection.
fn repeats_and_cells(seed: u64, n: usize, levels: &[usize]) -> (usize, usize) {
    let case = Case::random(seed, n, levels);
    let largest = case.largest();
    let mut keys: Vec<Vec<u32>> = (0..n)
        .map(|i| {
            let mut key = vec![case.codes[largest][i]];
            key.extend(
                (0..levels.len())
                    .filter(|&e| e != largest)
                    .map(|e| case.codes[e][i]),
            );
            key
        })
        .collect();
    keys.sort_unstable();
    let repeats = keys.windows(2).filter(|pair| pair[0] == pair[1]).count();
    let mut data = case.columns(seed + 10, 1);
    let mut projector = Projector::new(case.design(), Settings::default());
    let (outcome, _) = project(&mut projector, &case, None, &mut data, 1, false, &request());
    (repeats, projected(outcome).facts.cells)
}

#[test]
fn eliminated_groups_with_repeated_rows_match_dense_oracle() {
    // Each eliminated group repeats its few remaining-code combinations, so
    // repeated rows merge into weighted cells.
    for (seed, levels) in [(15, &[40, 4][..]), (16, &[30, 4, 3][..])] {
        let (repeats, cells) = repeats_and_cells(seed, 600, levels);
        assert!(repeats > 100 && cells == 600 - repeats, "{repeats} {cells}");
        check_case(seed, 600, levels);
    }
}

#[test]
fn sparse_two_effects_with_rare_repeats_match_dense_oracle() {
    // A sparse, possibly disconnected design whose rare repeated
    // combinations stay below the merge threshold (one cell per row), with
    // the largest effect listed second.
    let (repeats, cells) = repeats_and_cells(17, 450, &[150, 200]);
    assert!(repeats > 0 && cells == 450, "{repeats} {cells}");
    check_case(17, 450, &[150, 200]);
}

#[test]
fn fresh_projectors_are_bitwise_repeatable() {
    let case = Case::random(21, 900, &[150, 23, 6, 11, 4]);
    let weights = case.weights(22);
    for weights in [None, Some(weights.as_slice())] {
        let run = || {
            let mut data = case.columns(23, 11);
            let mut projector = Projector::new(case.design(), Settings::default());
            let (outcome, coefficients) = project(
                &mut projector,
                &case,
                weights,
                &mut data,
                11,
                true,
                &request(),
            );
            projected(outcome);
            (data, coefficients.unwrap())
        };
        let (first, first_coefficients) = run();
        let (second, second_coefficients) = run();
        assert!(first
            .iter()
            .zip(&second)
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(first_coefficients
            .iter()
            .zip(&second_coefficients)
            .all(|(a, b)| a.to_bits() == b.to_bits()));
    }
}

#[test]
fn lane_settings_inside_a_pool_match_dense_oracle() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let case = Case::random(31, 900, &[150, 23, 6, 11, 4]);
    let weights = case.weights(32);
    pool.install(|| {
        for weights in [None, Some(weights.as_slice())] {
            let oracle = Oracle::new(&case, weights);
            for lanes in [1, 4] {
                let settings = Settings {
                    lanes: Some(lanes),
                    ..Settings::default()
                };
                let mut projector = Projector::new(case.design(), settings);
                check_against_oracle(&case, weights, &oracle, &mut projector, 3, 33);
                check_against_oracle(&case, weights, &oracle, &mut projector, 11, 34);
            }
        }
    });
}

#[test]
fn supplied_members_give_bitwise_the_same_projection() {
    let case = Case::random(41, 700, &[150, 23, 6, 11]);
    let weights = case.weights(42);
    let members: Vec<(Vec<usize>, Vec<usize>)> = case
        .codes
        .iter()
        .zip(&case.levels)
        .map(|(codes, &levels)| members_of(codes, levels))
        .collect();
    for weights in [None, Some(weights.as_slice())] {
        let with_members = Design::new(
            case.n,
            case.effects()
                .into_iter()
                .zip(&members)
                .map(|(effect, (offsets, rows))| {
                    effect
                        .with_members(Members::new(offsets, rows).unwrap())
                        .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let mut a = case.columns(43, 5);
        let mut b = a.clone();
        let mut c = a.clone();
        let mut plain = Projector::new(case.design(), Settings::default());
        let mut supplied = Projector::new(with_members, Settings::default());
        let (outcome, coefficients_a) =
            project(&mut plain, &case, weights, &mut a, 5, true, &request());
        projected(outcome);
        let (outcome, coefficients_b) =
            project(&mut supplied, &case, weights, &mut b, 5, true, &request());
        projected(outcome);
        let (outcome, _) = project(&mut supplied, &case, weights, &mut c, 5, false, &request());
        projected(outcome);
        assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
        assert!(b.iter().zip(&c).all(|(x, y)| x.to_bits() == y.to_bits()));
        assert!(coefficients_a
            .unwrap()
            .iter()
            .zip(&coefficients_b.unwrap())
            .all(|(x, y)| x.to_bits() == y.to_bits()));
    }
}

/// Every level's relative weighted moment of `r` (all effects, the
/// eliminated one included) at the column's original weighted norm.
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

#[test]
fn parallel_lanes_satisfy_normal_equations_and_are_pool_independent() {
    // Large enough that the structure, weight, operator and row passes split
    // into lanes.
    let case = Case::random(51, 24_000, &[2400, 600, 12]);
    let weights = case.weights(52);
    let ncols = 5;
    let original = case.columns(53, ncols);
    let offsets = case.offsets();
    let run = |threads: usize, lanes: usize, weights: Option<&[f64]>| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            let settings = Settings {
                lanes: Some(lanes),
                ..Settings::default()
            };
            let mut projector = Projector::new(case.design(), settings);
            let mut data = original.clone();
            let (outcome, coefficients) = project(
                &mut projector,
                &case,
                weights,
                &mut data,
                ncols,
                true,
                &request(),
            );
            let report = projected(outcome);
            assert!(report.certificate.passed);
            (data, coefficients.unwrap(), report.facts.lanes)
        })
    };
    for weights in [None, Some(weights.as_slice())] {
        let (four, coefficients, lanes) = run(4, 4, weights);
        assert!(lanes > 1, "the operator ran in {lanes} lane");
        for j in 0..ncols {
            let y = column(&original, ncols, j);
            let r = column(&four, ncols, j);
            let moment = largest_relative_moment(&case, weights, &y, &r);
            assert!(moment <= TOLERANCE, "column {j}: moment {moment}");
            let bound = 1e-9 * inf_norm(&y).max(1.0);
            for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                let fit = case.fitted(&coefficients, &offsets, j, i);
                assert!((yi - fit - ri).abs() <= bound, "column {j}, row {i}");
            }
        }
        // A fixed lane count is bitwise reproducible whatever the pool width.
        for threads in [1, 3] {
            let (other, other_coefficients, _) = run(threads, 4, weights);
            assert!(four
                .iter()
                .zip(&other)
                .all(|(a, b)| a.to_bits() == b.to_bits()));
            assert!(coefficients
                .iter()
                .zip(&other_coefficients)
                .all(|(a, b)| a.to_bits() == b.to_bits()));
        }
        // Another lane count agrees scientifically.
        let (one, _, lanes) = run(4, 1, weights);
        assert_eq!(lanes, 1);
        for j in 0..ncols {
            let y = column(&original, ncols, j);
            let bound = 1e-7 * inf_norm(&y).max(1.0);
            for i in 0..case.n {
                let (a, b) = (four[i * ncols + j], one[i * ncols + j]);
                assert!((a - b).abs() <= bound, "column {j}, row {i}: {a} vs {b}");
            }
        }
    }
}

fn assert_untouched(outcome: Outcome, reason: Decline, data: &[f64], original: &[f64]) {
    let (actual, input) = declined(outcome);
    assert_eq!(actual, reason);
    assert_eq!(input, InputState::Untouched);
    assert!(data
        .iter()
        .zip(original)
        .all(|(a, b)| a.to_bits() == b.to_bits()));
}

#[test]
fn designs_outside_the_route_decline_untouched() {
    // The explicit elimination keeps its two-to-five-effect range (Auto
    // projects these designs with the full formulation).
    let eliminate = Settings {
        choice: Choice {
            reduction: Reduction::EliminateLargest,
            ..Choice::default()
        },
        ..Settings::default()
    };
    for levels in [vec![40], vec![30, 5, 4, 3, 3, 2]] {
        let case = Case::random(61, 300, &levels);
        let original = case.columns(62, 3);
        let mut data = original.clone();
        let mut projector = Projector::new(case.design(), eliminate);
        assert!(!projector.prepare(&Control::default()).unwrap());
        let (outcome, coefficients) =
            project(&mut projector, &case, None, &mut data, 3, true, &request());
        assert_untouched(outcome, Decline::OutsideRoute, &data, &original);
        assert!(coefficients.unwrap().iter().all(|&c| c == 0.0));
        assert!(projector.columns().is_empty() && projector.residual_norms().is_empty());
    }
}

#[test]
fn shape_memory_and_iteration_declines_leave_columns_untouched() {
    let case = Case::random(71, 600, &[150, 23, 6]);
    let weights = case.weights(72);
    let mut projector = Projector::new(case.design(), Settings::default());

    // Observation count differs from the design's.
    let original = case.columns(73, 3);
    let mut short = original[..(case.n - 1) * 3].to_vec();
    let outcome = projector
        .project(
            Weights::Unit,
            ColumnsMut::new(&mut short, case.n - 1, 3).unwrap(),
            None,
            &request(),
            &Control::default(),
        )
        .unwrap();
    assert_untouched(
        outcome,
        Decline::Shape,
        &short,
        &original[..(case.n - 1) * 3],
    );

    // No columns.
    let outcome = projector
        .project(
            Weights::Unit,
            ColumnsMut::new(&mut [], case.n, 0).unwrap(),
            None,
            &request(),
            &Control::default(),
        )
        .unwrap();
    assert_untouched(outcome, Decline::Shape, &[], &[]);

    // Workspace above the memory limit.
    let mut data = original.clone();
    let mut limited = request();
    limited.memory_limit = Some(1);
    let (outcome, _) = project(
        &mut projector,
        &case,
        Some(&weights),
        &mut data,
        3,
        false,
        &limited,
    );
    assert_untouched(outcome, Decline::MemoryLimit, &data, &original);

    // An unreachable tolerance within one iteration.
    let mut capped = Request::new(1e-14, 1);
    capped.residual_gram = true;
    for weights in [None, Some(weights.as_slice())] {
        let mut data = original.clone();
        let (outcome, coefficients) =
            project(&mut projector, &case, weights, &mut data, 3, true, &capped);
        let (reason, input) = declined(outcome);
        assert!(matches!(reason, Decline::Numerical(_)), "{reason:?}");
        assert_eq!(input, InputState::Untouched);
        assert!(data
            .iter()
            .zip(&original)
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(coefficients.unwrap().iter().all(|&c| c == 0.0));
        assert!(projector.columns().is_empty() && projector.residual_norms().is_empty());
        assert!(projector.residual_gram().is_none());
    }

    // The projector still projects afterwards.
    let mut data = original.clone();
    let (outcome, _) = project(
        &mut projector,
        &case,
        Some(&weights),
        &mut data,
        3,
        false,
        &request(),
    );
    assert!(projected(outcome).certificate.passed);
}

fn expect_error(case: &Case, weights: Option<&[f64]>, request: &Request) -> Error {
    let mut rng = Rng(81);
    let mut data: Vec<f64> = (0..case.n * 2).map(|_| rng.normal()).collect();
    let mut projector = Projector::new(case.design(), Settings::default());
    projector
        .project(
            as_weights(weights),
            ColumnsMut::new(&mut data, case.n, 2).unwrap(),
            None,
            request,
            &Control::default(),
        )
        .expect_err("projection must fail")
}

#[test]
fn invalid_requests_codes_and_weights_are_errors() {
    let case = Case::random(82, 500, &[120, 17]);
    for tolerance in [0.0, -1e-8, f64::NAN, f64::INFINITY] {
        let error = expect_error(&case, None, &Request::new(tolerance, 100));
        assert!(matches!(error, Error::InvalidInput(_)), "{error:?}");
    }

    // A code at its effect's level count, in a retained effect and in the
    // eliminated effect (whose members the projector builds).
    for effect in [1, 0] {
        let mut bad = Case::random(83, 500, &[120, 17]);
        bad.codes[effect][7] = bad.levels[effect] as u32;
        let error = expect_error(&bad, None, &request());
        assert!(matches!(error, Error::InvalidInput(_)), "{error:?}");
    }

    let weights = case.weights(84);
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mut bad = weights.clone();
        bad[13] = value;
        let error = expect_error(&case, Some(&bad), &request());
        assert!(matches!(error, Error::InvalidData(_)), "{value}: {error:?}");
    }
}

fn project_with_cancel(
    projector: &mut Projector<'_>,
    case: &Case,
    cancel: &(dyn Fn() -> bool + Sync),
) -> Result<Outcome, Error> {
    let mut data = case.columns(92, 3);
    projector.project(
        Weights::Unit,
        ColumnsMut::new(&mut data, case.n, 3).unwrap(),
        None,
        &request(),
        &Control::with_cancel(cancel),
    )
}

#[test]
fn cancellation_stops_structure_and_iterations() {
    let case = Case::random(91, 900, &[150, 23, 6, 11, 4]);
    let always = || true;
    let polls = AtomicUsize::new(0);
    let later = || polls.fetch_add(1, Ordering::Relaxed) >= 3;

    // During the structure pass.
    let mut projector = Projector::new(case.design(), Settings::default());
    let error = project_with_cancel(&mut projector, &case, &always).unwrap_err();
    assert_eq!(error, Error::Cancelled);

    // During the iteration, with the structure prepared.
    let mut projector = Projector::new(case.design(), Settings::default());
    assert!(projector.prepare(&Control::default()).unwrap());
    let error = project_with_cancel(&mut projector, &case, &always).unwrap_err();
    assert_eq!(error, Error::Cancelled);
    let error = project_with_cancel(&mut projector, &case, &later).unwrap_err();
    assert_eq!(error, Error::Cancelled);
    assert_eq!(polls.load(Ordering::Relaxed), 4);

    // The projector recovers after a cancelled call.
    let report = projected(project_with_cancel(&mut projector, &case, &|| false).unwrap());
    assert!(report.certificate.passed && report.facts.iterations > 3);
}
