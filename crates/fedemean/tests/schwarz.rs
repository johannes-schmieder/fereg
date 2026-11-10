//! Additive Schwarz on the full formulation (feature `schwarz`): PCG and
//! modified LSMR with every subdomain rule and local solve against an
//! independent dense weighted least-squares reference (modified
//! Gram-Schmidt on `sqrt(W) D`, as in `tests/full.rs`), on random, chain and
//! low-mobility designs; the exact two-effect inverse; escalation from a
//! stalling Jacobi recurrence; bitwise independence of the pool; memory
//! admission; cancellation.

#![cfg(feature = "schwarz")]

use fedemean::{
    Choice, CoefficientLayout, CoefficientsMut, ColumnsMut, Control, Decline, Design, Effect,
    Error, Fallback, InputState, Method, MethodUsed, Outcome, Preconditioner, PreconditionerUsed,
    Projector, Reduction, ReductionUsed, Report, Request, SchwarzLocal, SchwarzSolves, SchwarzSpec,
    Settings, Subdomains, Weights,
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

    fn chance(&mut self, p: f64) -> bool {
        self.uniform() < p
    }

    fn normal(&mut self) -> f64 {
        let u = self.uniform().max(1e-300);
        let v = self.uniform();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// Worker mobility of a panel.
#[derive(Clone, Copy, Debug)]
enum Mobility {
    /// Firms on a line; a move (at this rate per year) goes one or two
    /// firms along it.
    Chain(f64),
    /// A chain at rate 0.3 whose first workers each bridge two neighbouring
    /// firms, so that the firm line is connected.
    Line,
    /// Rare moves between size-skewed firms: many small components.
    Low,
}

/// A categorical design in which every level is observed.
struct Case {
    n: usize,
    levels: Vec<usize>,
    codes: Vec<Vec<u32>>,
}

/// Dense codes `0..levels` of observed identifiers, in sorted order.
fn dense(raw: &[usize]) -> (Vec<u32>, usize) {
    let mut levels = raw.to_vec();
    levels.sort_unstable();
    levels.dedup();
    let codes = raw
        .iter()
        .map(|id| levels.binary_search(id).unwrap() as u32)
        .collect();
    (codes, levels.len())
}

impl Case {
    /// Independent uniform codes per effect, each level at least once.
    fn random(seed: u64, n: usize, levels: &[usize]) -> Self {
        let mut rng = Rng(seed);
        let codes = levels
            .iter()
            .map(|&count| {
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

    /// A worker-firm-year panel of `n` rows with `firms` firms, plus uniform
    /// effects with `extra` levels.
    fn panel(seed: u64, n: usize, firms: usize, mobility: Mobility, extra: &[usize]) -> Self {
        let mut rng = Rng(seed);
        let mut raw: Vec<Vec<usize>> = vec![Vec::with_capacity(n); 3 + extra.len()];
        let mut worker = 0;
        let skewed = |rng: &mut Rng| (firms as f64 * rng.uniform() * rng.uniform()) as usize;
        if matches!(mobility, Mobility::Line) {
            for firm in 0..2 * (firms - 1) {
                raw[0].push(firm / 2);
                raw[1].push(firm / 2 + firm % 2);
                raw[2].push(firm % 2);
                for (e, &levels) in extra.iter().enumerate() {
                    raw[3 + e].push(rng.below(levels));
                }
            }
            worker = firms - 1;
        }
        let mobility = match mobility {
            Mobility::Line => Mobility::Chain(0.3),
            other => other,
        };
        while raw[0].len() < n {
            let spell = 2 + rng.below(6);
            let entry = rng.below(20 - spell + 1);
            let mut firm = match mobility {
                Mobility::Low => skewed(&mut rng),
                _ => rng.below(firms),
            };
            for t in 0..spell.min(n - raw[0].len()) {
                if t > 0 {
                    match mobility {
                        Mobility::Chain(rate) if rng.chance(rate) => {
                            let step = 1 + rng.below(2);
                            firm = if rng.chance(0.5) {
                                (firm + step).min(firms - 1)
                            } else {
                                firm.saturating_sub(step)
                            };
                        }
                        Mobility::Low if rng.chance(0.03) => firm = skewed(&mut rng),
                        _ => {}
                    }
                }
                raw[0].push(worker);
                raw[1].push(firm);
                raw[2].push(entry + t);
                for (e, &levels) in extra.iter().enumerate() {
                    raw[3 + e].push(rng.below(levels));
                }
            }
            worker += 1;
        }
        let (codes, levels) = raw.iter().map(|ids| dense(ids)).unzip();
        Self { n, levels, codes }
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

    /// `ncols` observation-major columns mixing the effects and noise.
    fn columns(&self, seed: u64, ncols: usize) -> Vec<f64> {
        let mut rng = Rng(seed);
        let mut data = vec![0.0; self.n * ncols];
        for j in 0..ncols {
            let effects: Vec<Vec<f64>> = self
                .levels
                .iter()
                .map(|&levels| (0..levels).map(|_| rng.normal()).collect())
                .collect();
            let noise = [1.0, 0.0, 2.0][j % 3];
            let constant = 10.0 * j as f64 - 3.0;
            for i in 0..self.n {
                let mut value = constant + noise * rng.normal();
                for (e, (codes, alpha)) in self.codes.iter().zip(&effects).enumerate() {
                    value += (1.0 + e as f64) * alpha[codes[i] as usize];
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

/// Dense weighted least-squares reference: an orthonormal basis of the
/// column space of `sqrt(W) D` (two passes of modified Gram-Schmidt),
/// dropping dummy columns whose remaining norm is below `1e-10` of theirs.
struct Oracle {
    root_weights: Vec<f64>,
    basis: Vec<Vec<f64>>,
}

impl Oracle {
    fn subtract(&self, v: &mut [f64]) {
        for _ in 0..2 {
            for q in &self.basis {
                let d = dot(q, v);
                for (x, &qi) in v.iter_mut().zip(q) {
                    *x -= d * qi;
                }
            }
        }
    }

    fn new(case: &Case, weights: Option<&[f64]>) -> Self {
        let root_weights = (0..case.n)
            .map(|i| weights.map_or(1.0, |w| w[i].sqrt()))
            .collect();
        let mut oracle = Self {
            root_weights,
            basis: Vec::new(),
        };
        for (codes, &levels) in case.codes.iter().zip(&case.levels) {
            for level in 0..levels {
                let mut v: Vec<f64> = codes
                    .iter()
                    .zip(&oracle.root_weights)
                    .map(|(&code, &root)| if code as usize == level { root } else { 0.0 })
                    .collect();
                let original = dot(&v, &v).sqrt();
                oracle.subtract(&mut v);
                let remaining = dot(&v, &v).sqrt();
                if remaining > 1e-10 * original {
                    v.iter_mut().for_each(|x| *x /= remaining);
                    oracle.basis.push(v);
                }
            }
        }
        oracle
    }

    fn residual(&self, y: &[f64]) -> Vec<f64> {
        let mut v: Vec<f64> = y
            .iter()
            .zip(&self.root_weights)
            .map(|(y, root)| y * root)
            .collect();
        self.subtract(&mut v);
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

fn declined(outcome: Outcome) -> (Decline, InputState) {
    match outcome {
        Outcome::Declined { reason, input, .. } => (reason, input),
        Outcome::Projected(_) => panic!("unexpected projection"),
        _ => panic!("unexpected outcome"),
    }
}

fn settings(method: Method, preconditioner: Preconditioner, fallback: Fallback) -> Settings {
    Settings {
        choice: Choice {
            reduction: Reduction::Full,
            method,
            preconditioner,
            fallback,
        },
        lanes: None,
    }
}

fn schwarz(subdomains: Subdomains, local: SchwarzLocal) -> Preconditioner {
    Preconditioner::Schwarz(SchwarzSpec { subdomains, local })
}

/// Project `data` in place, filling and returning a zeroed coefficient
/// buffer of the canonical layout.
fn project(
    projector: &mut Projector<'_>,
    case: &Case,
    weights: Option<&[f64]>,
    (data, ncols): (&mut [f64], usize),
    request: &Request,
    control: &Control<'_>,
) -> Result<(Outcome, Vec<f64>), Error> {
    let offsets = case.offsets();
    let mut coefficients = vec![0.0; case.per_rhs() * ncols];
    let layout = CoefficientLayout::new(case.per_rhs(), &offsets, &case.levels);
    let columns = ColumnsMut::new(data, case.n, ncols).unwrap();
    let out = Some(CoefficientsMut::new(&mut coefficients, layout));
    let outcome = projector.project(as_weights(weights), columns, out, request, control)?;
    Ok((outcome, coefficients))
}

/// The Schwarz route of a report: its subdomain count and local solves.
fn schwarz_used(report: &Report) -> (u32, SchwarzSolves) {
    match report.route.preconditioner {
        PreconditionerUsed::Schwarz { subdomains, local } => (subdomains, local),
        other => panic!("not Schwarz: {other:?}"),
    }
}

/// Residuals (against the oracle), coefficients (`y - D b = r`) and the
/// certificates of one projection.
fn check_projection(
    case: &Case,
    (weights, oracle): (Option<&[f64]>, &Oracle),
    original: &[f64],
    (data, coefficients): (&[f64], &[f64]),
    ncols: usize,
    label: &str,
) {
    for j in 0..ncols {
        let y = column(original, ncols, j);
        let r = column(data, ncols, j);
        let bound = 1e-7 * inf_norm(&y).max(1.0);
        for (i, (&actual, &reference)) in r.iter().zip(&oracle.residual(&y)).enumerate() {
            assert!(
                (actual - reference).abs() <= bound,
                "{label}, column {j}, row {i}: {actual} vs oracle {reference}"
            );
        }
        let bound = 1e-9 * inf_norm(&y).max(1.0);
        for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
            let fit = case.fitted(coefficients, j, i);
            assert!(
                (yi - fit - ri).abs() <= bound,
                "{label}, column {j}, row {i}"
            );
        }
    }
    let _ = weights;
}

/// Every method, subdomain rule and local solve; unit weights, weights and
/// unit weights again on one projector (the preconditioner is rebuilt for
/// each new weight state).
fn check_case(case: &Case, seed: u64, kinds: impl Fn(SchwarzLocal, SchwarzSolves)) {
    let weights = case.weights(seed);
    let oracles = [Oracle::new(case, None), Oracle::new(case, Some(&weights))];
    let ncols = 3;
    let original = case.columns(seed + 1, ncols);
    for method in [Method::Pcg, Method::Lsmr] {
        for subdomains in [Subdomains::Star, Subdomains::AllPairs] {
            for local in [
                SchwarzLocal::Auto,
                SchwarzLocal::Exact,
                SchwarzLocal::Approximate,
            ] {
                let precond = schwarz(subdomains, local);
                let mut projector =
                    Projector::new(case.design(), settings(method, precond, Fallback::Decline));
                for (pass, weighted) in [false, true, false].into_iter().enumerate() {
                    let weights = weighted.then_some(weights.as_slice());
                    let label = format!(
                        "{:?} {method:?} {subdomains:?} {local:?} pass {pass}",
                        case.levels
                    );
                    let mut data = original.clone();
                    let (outcome, coefficients) = project(
                        &mut projector,
                        case,
                        weights,
                        (&mut data, ncols),
                        &Request::new(TOLERANCE, MAX_ITERATIONS),
                        &Control::default(),
                    )
                    .unwrap();
                    let report = projected(outcome);
                    assert!(report.certificate.passed, "{label}");
                    assert_eq!(report.route.reduction, ReductionUsed::Full);
                    let used = if method == Method::Lsmr {
                        MethodUsed::Lsmr
                    } else {
                        MethodUsed::Pcg
                    };
                    assert_eq!(report.route.method, used);
                    assert_eq!(report.route.escalations, 0);
                    let (count, solves) = schwarz_used(&report);
                    let pairs = case.levels.len() - 1;
                    assert!(count as usize >= pairs, "{label}: {count}");
                    let total = solves.trivial + solves.dense + solves.skyline + solves.approximate;
                    assert_eq!(total, count, "{label}");
                    kinds(local, solves);
                    let oracle = &oracles[usize::from(weighted)];
                    let outputs = (&data[..], &coefficients[..]);
                    check_projection(case, (weights, oracle), &original, outputs, ncols, &label);
                    for report in projector.columns() {
                        assert!(report.relative_moment <= TOLERANCE, "{label}");
                    }
                }
            }
        }
    }
}

/// Explicit local solves use only their kinds.
fn explicit_kinds(local: SchwarzLocal, solves: SchwarzSolves) {
    match local {
        SchwarzLocal::Exact => assert_eq!(solves.approximate, 0, "{solves:?}"),
        SchwarzLocal::Approximate => {
            assert_eq!((solves.dense, solves.skyline), (0, 0), "{solves:?}");
        }
        _ => {}
    }
}

#[test]
fn three_effects_match_dense_oracle() {
    check_case(&Case::random(301, 600, &[150, 23, 6]), 302, explicit_kinds);
}

#[test]
fn five_effects_match_dense_oracle() {
    check_case(
        &Case::random(311, 900, &[150, 23, 6, 11, 4]),
        312,
        explicit_kinds,
    );
}

#[test]
fn seven_effects_match_dense_oracle() {
    let case = Case::random(321, 1000, &[150, 23, 6, 11, 4, 9, 30]);
    check_case(&case, 322, explicit_kinds);
}

/// A chain whose firm side (the smaller side of the worker-firm component)
/// has more than 128 levels: the automatic and exact rules factor its
/// banded Schur complement by its envelope.
#[test]
fn chain_designs_match_dense_oracle() {
    let case = Case::panel(331, 900, 140, Mobility::Line, &[5]);
    assert!(case.levels[1] > 128, "{:?}", case.levels);
    check_case(&case, 332, |local, solves| {
        explicit_kinds(local, solves);
        if local != SchwarzLocal::Approximate {
            assert!(solves.skyline >= 1, "{local:?}: {solves:?}");
        }
    });
}

/// Low mobility: many small worker-firm components (one-firm components are
/// trivial).
#[test]
fn low_mobility_designs_match_dense_oracle() {
    let case = Case::panel(341, 900, 60, Mobility::Low, &[7, 3]);
    check_case(&case, 342, |local, solves| {
        explicit_kinds(local, solves);
        assert!(solves.trivial >= 10, "{solves:?}");
    });
}

/// With two effects the star has one pair, whose components are the whole
/// system: exact local solves invert `D'WD` on its range, so each recurrence
/// takes one iteration (two for modified LSMR's bidiagonalization).
#[test]
fn exact_schwarz_on_two_effects_is_the_inverse() {
    let case = Case::random(351, 700, &[120, 40]);
    let ncols = 3;
    for method in [Method::Pcg, Method::Lsmr] {
        let precond = schwarz(Subdomains::Star, SchwarzLocal::Exact);
        let mut projector =
            Projector::new(case.design(), settings(method, precond, Fallback::Decline));
        let mut data = case.columns(352, ncols);
        let request = Request::new(TOLERANCE, MAX_ITERATIONS);
        let (outcome, _) = project(
            &mut projector,
            &case,
            None,
            (&mut data, ncols),
            &request,
            &Control::default(),
        )
        .unwrap();
        let report = projected(outcome);
        assert!(
            report.facts.iterations <= 2,
            "{method:?}: {:?}",
            report.facts
        );
    }
}

/// A chain on which Jacobi stalls: without escalation PCG declines with the
/// columns untouched; with it the recurrence continues on Schwarz within
/// the remaining budget, satisfies independently recomputed normal
/// equations, agrees with the (escalating) elimination, and later
/// projections start on Schwarz.
#[test]
fn escalation_continues_a_stalled_jacobi_recurrence_on_schwarz() {
    let case = Case::panel(361, 10_000, 250, Mobility::Chain(0.3), &[]);
    let ncols = 3;
    let original = case.columns(362, ncols);
    let request = Request::new(TOLERANCE, MAX_ITERATIONS);
    let control = Control::default();
    let plain = settings(Method::Pcg, Preconditioner::Jacobi, Fallback::Decline);
    let mut data = original.clone();
    let mut projector = Projector::new(case.design(), plain);
    let (outcome, _) = project(
        &mut projector,
        &case,
        None,
        (&mut data, ncols),
        &request,
        &control,
    )
    .unwrap();
    let (reason, input) = declined(outcome);
    assert!(matches!(reason, Decline::Numerical(_)), "{reason:?}");
    assert_eq!(input, InputState::Untouched);
    assert_eq!(bits(&data), bits(&original));
    let mut eliminated = Settings::default();
    eliminated.choice.reduction = Reduction::EliminateLargest;
    eliminated.choice.fallback = Fallback::Escalate;
    let mut reference = original.clone();
    let mut projector = Projector::new(case.design(), eliminated);
    let (outcome, _) = project(
        &mut projector,
        &case,
        None,
        (&mut reference, ncols),
        &request,
        &control,
    )
    .unwrap();
    projected(outcome);
    for method in [Method::Pcg, Method::Lsmr] {
        let escalating = settings(method, Preconditioner::Jacobi, Fallback::Escalate);
        let mut projector = Projector::new(case.design(), escalating);
        for round in 0..2 {
            let mut data = original.clone();
            let (outcome, coefficients) = project(
                &mut projector,
                &case,
                None,
                (&mut data, ncols),
                &request,
                &control,
            )
            .unwrap();
            let report = projected(outcome);
            schwarz_used(&report);
            let label = format!("{method:?} round {round}");
            assert_eq!(report.route.escalations, u8::from(round == 0), "{label}");
            assert_eq!(report.facts.escalations, usize::from(round == 0), "{label}");
            for j in 0..ncols {
                let y = column(&original, ncols, j);
                let r = column(&data, ncols, j);
                let moment = largest_relative_moment(&case, None, &y, &r);
                assert!(moment <= TOLERANCE, "{label} column {j}: {moment}");
                let bound = 1e-9 * inf_norm(&y).max(1.0);
                for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                    let fit = case.fitted(&coefficients, j, i);
                    assert!(
                        (yi - fit - ri).abs() <= bound,
                        "{label}, column {j}, row {i}"
                    );
                }
                let bound = 1e-7 * inf_norm(&y).max(1.0);
                let elim = column(&reference, ncols, j);
                assert!(
                    r.iter().zip(&elim).all(|(a, b)| (a - b).abs() <= bound),
                    "{label}"
                );
            }
        }
    }
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

/// A design large enough for the parallel passes (components above the
/// parallel threshold and the fixed sum chunks, more than 1024 levels on
/// both sides of the worker-firm pair) and for approximate Cholesky under
/// the automatic rule: independent normal-equation checks and bitwise
/// identity across pool widths.
#[test]
fn schwarz_is_bitwise_independent_of_the_pool() {
    let case = Case::random(371, 24_000, &[6000, 2500, 12, 7]);
    let weights = case.weights(372);
    let ncols = 5;
    let original = case.columns(373, ncols);
    let run = |method: Method, threads: usize, weights: Option<&[f64]>| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            let precond = Preconditioner::Schwarz(SchwarzSpec::default());
            let mut projector =
                Projector::new(case.design(), settings(method, precond, Fallback::Decline));
            let mut data = original.clone();
            let request = Request::new(TOLERANCE, MAX_ITERATIONS);
            let (outcome, coefficients) = project(
                &mut projector,
                &case,
                weights,
                (&mut data, ncols),
                &request,
                &Control::default(),
            )
            .unwrap();
            let report = projected(outcome);
            (data, coefficients, report)
        })
    };
    for weights in [None, Some(weights.as_slice())] {
        for method in [Method::Pcg, Method::Lsmr] {
            let (four, coefficients, report) = run(method, 4, weights);
            let (_, solves) = schwarz_used(&report);
            assert!(solves.approximate >= 1, "{solves:?}");
            for j in 0..ncols {
                let y = column(&original, ncols, j);
                let r = column(&four, ncols, j);
                let moment = largest_relative_moment(&case, weights, &y, &r);
                assert!(moment <= TOLERANCE, "{method:?} column {j}: {moment}");
                let bound = 1e-9 * inf_norm(&y).max(1.0);
                for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                    let fit = case.fitted(&coefficients, j, i);
                    assert!((yi - fit - ri).abs() <= bound, "column {j}, row {i}");
                }
            }
            for threads in [1, 2, 3] {
                let (other, other_coefficients, other_report) = run(method, threads, weights);
                assert_eq!(bits(&four), bits(&other), "{method:?} {threads}");
                assert_eq!(bits(&coefficients), bits(&other_coefficients));
                assert_eq!(report.route, other_report.route);
                assert_eq!(report.facts.iterations, other_report.facts.iterations);
            }
        }
    }
}

/// Lockstep modified LSMR gives every column the bits, coefficients and
/// iterations of its own one-column projection, under Jacobi, the identity
/// and Schwarz, unit and varying weights, over two batches (eight and three
/// columns), and at the narrower width a memory limit leaves.
#[test]
fn lockstep_lsmr_columns_have_their_single_column_bits() {
    let case = Case::random(391, 3000, &[700, 150, 9, 5]);
    let weights = case.weights(392);
    let ncols = 11;
    let original = case.columns(393, ncols);
    let per_rhs = case.per_rhs();
    let request = Request::new(TOLERANCE, MAX_ITERATIONS);
    let preconditioners = [
        Preconditioner::Jacobi,
        Preconditioner::Identity,
        Preconditioner::Schwarz(SchwarzSpec::default()),
    ];
    for weights in [None, Some(weights.as_slice())] {
        for preconditioner in preconditioners {
            let choice = settings(Method::Lsmr, preconditioner, Fallback::Decline);
            let mut projector = Projector::new(case.design(), choice);
            let mut data = original.clone();
            let (outcome, coefficients) = project(
                &mut projector,
                &case,
                weights,
                (&mut data, ncols),
                &request,
                &Control::default(),
            )
            .unwrap();
            assert!(projected(outcome).certificate.passed);
            let iterations: Vec<usize> = projector.columns().iter().map(|c| c.iterations).collect();
            let mut alone = Projector::new(case.design(), choice);
            for j in 0..ncols {
                let mut single = column(&original, ncols, j);
                let (outcome, one) = project(
                    &mut alone,
                    &case,
                    weights,
                    (&mut single, 1),
                    &request,
                    &Control::default(),
                )
                .unwrap();
                assert!(projected(outcome).certificate.passed);
                let label = format!("{preconditioner:?} column {j}");
                assert_eq!(bits(&column(&data, ncols, j)), bits(&single), "{label}");
                let lockstep = &coefficients[j * per_rhs..(j + 1) * per_rhs];
                assert_eq!(bits(lockstep), bits(&one), "{label}");
                assert_eq!(iterations[j], alone.columns()[0].iterations, "{label}");
            }
            let mut narrow = request;
            narrow.memory_limit = projector.admission_bytes(2);
            let mut limited = original.clone();
            let (outcome, limited_coefficients) = project(
                &mut projector,
                &case,
                weights,
                (&mut limited, ncols),
                &narrow,
                &Control::default(),
            )
            .unwrap();
            let report = projected(outcome);
            assert!(report.workspace_bytes <= narrow.memory_limit.unwrap());
            assert_eq!(bits(&limited), bits(&data), "{preconditioner:?} narrow");
            assert_eq!(bits(&limited_coefficients), bits(&coefficients));
        }
    }
}

/// Explicit Schwarz beyond the memory limit declines with the columns
/// untouched; the automatic choice keeps Jacobi within it; the admitted
/// bytes count a built preconditioner.
#[test]
fn memory_limits_decline_explicit_schwarz_and_keep_automatic_jacobi() {
    let case = Case::random(381, 3000, &[700, 150, 9, 5]);
    let ncols = 3;
    let original = case.columns(382, ncols);
    let request = Request::new(TOLERANCE, MAX_ITERATIONS);
    for method in [Method::Pcg, Method::Lsmr] {
        let explicit = Preconditioner::Schwarz(SchwarzSpec::default());
        let mut projector =
            Projector::new(case.design(), settings(method, explicit, Fallback::Decline));
        assert!(projector.prepare(&Control::default()).unwrap());
        // Modified LSMR admits one column's workspace; a wider batch runs
        // only where it fits beside the preconditioner.
        let needed = if method == Method::Lsmr { 1 } else { ncols };
        let base = projector.admission_bytes(needed).unwrap();
        let mut limited = request;
        limited.memory_limit = Some(base + 1024);
        let mut data = original.clone();
        let (outcome, coefficients) = project(
            &mut projector,
            &case,
            None,
            (&mut data, ncols),
            &limited,
            &Control::default(),
        )
        .unwrap();
        let (reason, input) = declined(outcome);
        assert_eq!(
            (reason, input),
            (Decline::MemoryLimit, InputState::Untouched)
        );
        assert_eq!(bits(&data), bits(&original));
        assert!(coefficients.iter().all(|&c| c == 0.0));
        let mut auto = Projector::new(
            case.design(),
            settings(method, Preconditioner::Auto, Fallback::Decline),
        );
        let mut data = original.clone();
        let (outcome, _) = project(
            &mut auto,
            &case,
            None,
            (&mut data, ncols),
            &limited,
            &Control::default(),
        )
        .unwrap();
        let report = projected(outcome);
        assert_eq!(report.route.preconditioner, PreconditionerUsed::Jacobi);
        assert!(report.workspace_bytes <= base + 1024);
        // Without a limit, the built preconditioner is admitted and counted.
        let mut data = original.clone();
        let (outcome, _) = project(
            &mut projector,
            &case,
            None,
            (&mut data, ncols),
            &request,
            &Control::default(),
        )
        .unwrap();
        let report = projected(outcome);
        schwarz_used(&report);
        let admitted = projector.admission_bytes(ncols).unwrap();
        assert!(
            admitted > base && report.workspace_bytes <= admitted,
            "{method:?}"
        );
        let mut exact = request;
        exact.memory_limit = Some(admitted);
        let mut data = original.clone();
        let (outcome, _) = project(
            &mut projector,
            &case,
            None,
            (&mut data, ncols),
            &exact,
            &Control::default(),
        )
        .unwrap();
        assert!(projected(outcome).certificate.passed);
    }
}

/// A cold projection's reported workspace is what Schwarz is admitted at:
/// its build's peak with the transients charged before each stage. At a
/// limit of exactly those bytes the projection runs; one byte below, it
/// declines with the columns untouched (every local solve kind, the star
/// and all pairs, weighted).
#[test]
fn schwarz_is_admitted_at_its_reported_workspace() {
    let cases = [
        Case::random(391, 4000, &[900, 120, 7]),
        Case::random(392, 3000, &[400, 300]),
        Case::random(393, 2500, &[60, 40, 6]),
    ];
    let ncols = 2;
    for (c, case) in cases.iter().enumerate() {
        let weights: Vec<f64> = (0..case.n).map(|i| 0.5 + (i % 7) as f64 * 0.25).collect();
        let original = case.columns(394 + c as u64, ncols);
        for (subdomains, local) in [
            (Subdomains::Star, SchwarzLocal::Auto),
            (Subdomains::Star, SchwarzLocal::Approximate),
            (Subdomains::Star, SchwarzLocal::Exact),
            (Subdomains::AllPairs, SchwarzLocal::Auto),
        ] {
            let pre = schwarz(subdomains, local);
            let run = |limit: Option<u128>| {
                let mut projector =
                    Projector::new(case.design(), settings(Method::Pcg, pre, Fallback::Decline));
                let mut request = Request::new(TOLERANCE, MAX_ITERATIONS);
                request.memory_limit = limit;
                let mut data = original.clone();
                let (outcome, _) = project(
                    &mut projector,
                    case,
                    Some(&weights),
                    (&mut data, ncols),
                    &request,
                    &Control::default(),
                )
                .unwrap();
                (outcome, data, projector.admission_bytes(ncols))
            };
            let label = format!("case {c} {subdomains:?} {local:?}");
            let (outcome, _, admitted) = run(None);
            let reported = projected(outcome).workspace_bytes;
            assert_eq!(admitted, Some(reported), "{label}");
            let (outcome, _, _) = run(Some(reported));
            assert!(projected(outcome).certificate.passed, "{label}");
            let (outcome, data, _) = run(Some(reported - 1));
            let (reason, input) = declined(outcome);
            assert_eq!(
                (reason, input),
                (Decline::MemoryLimit, InputState::Untouched),
                "{label}"
            );
            assert_eq!(bits(&data), bits(&original), "{label}");
        }
    }
}

/// Cancellation stops the Schwarz build and its recurrences.
#[test]
fn cancellation_stops_schwarz_builds_and_recurrences() {
    let case = Case::panel(391, 4000, 120, Mobility::Chain(0.3), &[6]);
    for method in [Method::Pcg, Method::Lsmr] {
        let polls = AtomicUsize::new(0);
        let later = || polls.fetch_add(1, Ordering::Relaxed) >= 12;
        let always = || true;
        let precond = Preconditioner::Schwarz(SchwarzSpec::default());
        let mut projector =
            Projector::new(case.design(), settings(method, precond, Fallback::Decline));
        for cancel in [&always as &(dyn Fn() -> bool + Sync), &later] {
            let mut data = case.columns(392, 3);
            let error = project(
                &mut projector,
                &case,
                None,
                (&mut data, 3),
                &Request::new(TOLERANCE, MAX_ITERATIONS),
                &Control::with_cancel(cancel),
            )
            .unwrap_err();
            assert_eq!(error, Error::Cancelled, "{method:?}");
        }
        assert!(polls.load(Ordering::Relaxed) >= 13);
        let mut data = case.columns(392, 3);
        let (outcome, _) = project(
            &mut projector,
            &case,
            None,
            (&mut data, 3),
            &Request::new(TOLERANCE, MAX_ITERATIONS),
            &Control::with_cancel(&|| false),
        )
        .unwrap();
        assert!(projected(outcome).certificate.passed);
    }
}

/// The automatic preconditioner with escalation starts on Jacobi: a design
/// whose Jacobi recurrence is fast (uniform codes) keeps it, one whose
/// Jacobi recurrence is slow (rare moves between size-skewed firms) hands
/// over to Schwarz after its pilot, and later projections start on Schwarz.
/// Both certify and reproduce their columns from their coefficients.
#[test]
fn automatic_escalation_keeps_fast_jacobi_and_hands_slow_recurrences_to_schwarz() {
    let easy = Case::random(381, 20_000, &[500, 40, 7]);
    let hard = Case::panel(382, 20_000, 400, Mobility::Low, &[9]);
    let ncols = 2;
    let request = Request::new(TOLERANCE, MAX_ITERATIONS);
    let control = Control::default();
    for method in [Method::Pcg, Method::Lsmr] {
        let automatic = settings(method, Preconditioner::Auto, Fallback::Escalate);
        for (case, hard_design) in [(&easy, false), (&hard, true)] {
            let original = case.columns(383, ncols);
            let mut projector = Projector::new(case.design(), automatic);
            for round in 0..2 {
                let mut data = original.clone();
                let (outcome, coefficients) = project(
                    &mut projector,
                    case,
                    None,
                    (&mut data, ncols),
                    &request,
                    &control,
                )
                .unwrap();
                let report = projected(outcome);
                let label = format!("{method:?} hard={hard_design} round {round}");
                if hard_design {
                    schwarz_used(&report);
                    assert_eq!(report.facts.escalations > 0, round == 0, "{label}");
                } else {
                    assert_eq!(
                        report.route.preconditioner,
                        PreconditionerUsed::Jacobi,
                        "{label}"
                    );
                    assert_eq!(report.facts.escalations, 0, "{label}");
                }
                for j in 0..ncols {
                    let y = column(&original, ncols, j);
                    let r = column(&data, ncols, j);
                    let moment = largest_relative_moment(case, None, &y, &r);
                    assert!(moment <= TOLERANCE, "{label} column {j}: {moment}");
                    let bound = 1e-9 * inf_norm(&y).max(1.0);
                    for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                        let fit = case.fitted(&coefficients, j, i);
                        assert!(
                            (yi - fit - ri).abs() <= bound,
                            "{label}, column {j}, row {i}"
                        );
                    }
                }
            }
        }
    }
}
