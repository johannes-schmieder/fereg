//! Block-Jacobi preconditioning and escalation against an independent dense
//! weighted least-squares reference (rank-revealing modified Gram-Schmidt
//! with reorthogonalization on `sqrt(W) D`, the explicit dummy matrix, as in
//! `tests/oracle.rs`), and the Jacobi routes' bitwise agreement.

#[cfg(not(feature = "cmg"))]
use fedemean::Error;
use fedemean::{
    BlockSpec, BlockUsed, Choice, CoefficientLayout, CoefficientsMut, ColumnsMut, Control, Decline,
    Design, Effect, Facts, Fallback, InputState, LocalSolver, LocalUsed, Outcome, Preconditioner,
    PreconditionerUsed, Projector, Report, Request, Settings, Weights,
};

const TOLERANCE: f64 = 1e-10;

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

    /// Workers (the eliminated effect) on a chain of `firms` firms:
    /// `stayers` workers with three years at each firm and one mover with
    /// two years at each of two neighbouring firms. With `years`, a year
    /// effect of four levels. Jacobi needs about `firms` iterations on the
    /// chain and stalls on long ones; an exact firm block solves it at once.
    /// Without stayers the firms outnumber the workers and are eliminated
    /// instead, and the movers' block is the chain.
    fn chain(firms: usize, stayers: usize, years: bool) -> Self {
        let mut codes = vec![Vec::new(); if years { 3 } else { 2 }];
        let mut worker = 0_u32;
        let push = |codes: &mut Vec<Vec<u32>>, worker: u32, firm: usize, year: usize| {
            codes[0].push(worker);
            codes[1].push(firm as u32);
            if let Some(year_codes) = codes.get_mut(2) {
                year_codes.push(year as u32);
            }
        };
        for firm in 0..firms {
            for s in 0..stayers {
                for t in 0..3 {
                    push(&mut codes, worker, firm, (s + t) % 4);
                }
                worker += 1;
            }
            if firm + 1 < firms {
                for t in 0..4 {
                    push(&mut codes, worker, firm + t / 2, t);
                }
                worker += 1;
            }
        }
        let mut levels = vec![worker as usize, firms];
        if years {
            levels.push(4);
        }
        Self {
            n: codes[0].len(),
            levels,
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

    /// Observation-major columns cycling through noise dominated, in the
    /// design span, inside the eliminated span and mixed.
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

    fn fitted(&self, coefficients: &[f64], offsets: &[usize], j: usize, i: usize) -> f64 {
        let base = j * self.per_rhs();
        self.codes
            .iter()
            .zip(offsets)
            .map(|(codes, &offset)| coefficients[base + offset + codes[i] as usize])
            .sum()
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn subtract_projections(basis: &[Vec<f64>], v: &mut [f64]) {
    for _ in 0..2 {
        for q in basis {
            let d = dot(q, v);
            for (x, &qi) in v.iter_mut().zip(q) {
                *x -= d * qi;
            }
        }
    }
}

/// Dense weighted least-squares reference.
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

fn projected(outcome: Outcome) -> Report {
    match outcome {
        Outcome::Projected(report) => report,
        Outcome::Declined { reason, .. } => panic!("unexpected decline: {reason:?}"),
        _ => panic!("unexpected outcome"),
    }
}

fn settings(preconditioner: Preconditioner, fallback: Fallback) -> Settings {
    Settings {
        choice: Choice {
            preconditioner,
            fallback,
            ..Choice::default()
        },
        lanes: None,
    }
}

fn blocks(dense_max_levels: Option<usize>) -> Settings {
    let spec = BlockSpec {
        large: LocalSolver::Jacobi,
        dense_max_levels,
    };
    settings(Preconditioner::BlockJacobi(spec), Fallback::Decline)
}

/// Project `data` in place, filling and returning a zeroed coefficient
/// buffer of the documented layout.
fn project(
    projector: &mut Projector<'_>,
    case: &Case,
    weights: Option<&[f64]>,
    data: &mut [f64],
    ncols: usize,
    request: &Request,
) -> (Outcome, Vec<f64>) {
    let offsets = case.offsets();
    let mut coefficients = vec![0.0; case.per_rhs() * ncols];
    let layout = CoefficientLayout::new(case.per_rhs(), &offsets, &case.levels);
    let outcome = projector
        .project(
            as_weights(weights),
            ColumnsMut::new(data, case.n, ncols).unwrap(),
            Some(CoefficientsMut::new(&mut coefficients, layout)),
            request,
            &Control::default(),
        )
        .unwrap();
    (outcome, coefficients)
}

/// Residuals against the oracle, coefficients reproducing the fit, the
/// certificate and the per-column reports of one projection.
fn check_against_oracle(
    case: &Case,
    weights: Option<&[f64]>,
    oracle: &Oracle,
    projector: &mut Projector<'_>,
    ncols: usize,
    seed: u64,
    request: &Request,
) -> Report {
    let original = case.columns(seed, ncols);
    let mut data = original.clone();
    let (outcome, coefficients) = project(projector, case, weights, &mut data, ncols, request);
    let report = projected(outcome);
    assert!(report.certificate.passed, "{:?}", report.certificate);
    assert!(report.certificate.relative_infinity_norm <= request.tolerance);
    let offsets = case.offsets();
    assert_eq!(projector.columns().len(), ncols);
    for j in 0..ncols {
        let y = column(&original, ncols, j);
        let r = column(&data, ncols, j);
        let bound = 1e-7 * inf_norm(&y).max(1.0);
        for (i, (&actual, &reference)) in r.iter().zip(&oracle.residual(&y)).enumerate() {
            assert!(
                (actual - reference).abs() <= bound,
                "column {j} of {ncols}, row {i}: {actual} vs oracle {reference}"
            );
        }
        let bound = 1e-9 * inf_norm(&y).max(1.0);
        for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
            let fit = case.fitted(&coefficients, &offsets, j, i);
            assert!((yi - fit - ri).abs() <= bound, "column {j}, row {i}");
        }
        assert!(projector.columns()[j].relative_moment <= request.tolerance);
    }
    report
}

/// The retained blocks of a block route, in effect order.
fn used_blocks(report: &Report) -> Vec<BlockUsed> {
    match report.route.preconditioner {
        PreconditionerUsed::Block(blocks) => {
            let used: Vec<BlockUsed> = blocks.iter().flatten().copied().collect();
            assert!(blocks[used.len()..].iter().all(Option::is_none));
            used
        }
        other => panic!("not a block route: {other:?}"),
    }
}

fn check_blocks(seed: u64, n: usize, levels: &[usize], limit: Option<usize>) {
    let case = Case::random(seed, n, levels);
    let weights = case.weights(seed + 1);
    for weights in [None, Some(weights.as_slice())] {
        let oracle = Oracle::new(&case, weights);
        let mut projector = Projector::new(case.design(), blocks(limit));
        for (k, ncols) in [1, 3, 11].into_iter().enumerate() {
            let seed = seed + 10 + k as u64;
            let request = Request::new(TOLERANCE, 1000);
            let report = check_against_oracle(
                &case,
                weights,
                &oracle,
                &mut projector,
                ncols,
                seed,
                &request,
            );
            assert_eq!(report.route.escalations, 0);
            let retained: Vec<usize> = (0..levels.len()).filter(|&e| e != case.largest()).collect();
            let used = used_blocks(&report);
            assert_eq!(used.len(), retained.len());
            for (block, &effect) in used.iter().zip(&retained) {
                assert_eq!((block.effect, block.levels), (effect, levels[effect]));
                if let Some(limit) = limit {
                    let dense = levels[effect] <= limit;
                    assert_eq!(block.local == LocalUsed::Dense, dense, "{block:?}");
                }
            }
        }
    }
}

#[test]
fn two_effect_blocks_match_dense_oracle() {
    check_blocks(11, 500, &[120, 17], Some(12));
    check_blocks(11, 500, &[120, 17], Some(20));
    check_blocks(17, 450, &[150, 200], Some(160));
}

#[test]
fn three_effect_blocks_match_dense_oracle() {
    check_blocks(12, 600, &[150, 23, 6], Some(12));
    check_blocks(16, 600, &[30, 4, 3], None);
}

#[test]
fn four_effect_blocks_match_dense_oracle() {
    check_blocks(13, 700, &[150, 23, 6, 11], Some(12));
    check_blocks(13, 700, &[150, 23, 6, 11], None);
}

#[test]
fn five_effect_blocks_match_dense_oracle() {
    check_blocks(14, 900, &[150, 23, 6, 11, 4], Some(12));
    check_blocks(14, 900, &[150, 23, 6, 11, 4], Some(0));
    check_blocks(14, 900, &[150, 23, 6, 11, 4], None);
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

#[test]
fn jacobi_routes_are_bitwise_identical() {
    // Explicit Jacobi and every Auto choice without an escalation run the
    // same recurrence; Jacobi never escalates.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    pool.install(|| {
        for (seed, n, levels) in [
            (14, 900, &[150, 23, 6, 11, 4][..]),
            (51, 24_000, &[2400, 600, 12][..]),
        ] {
            let case = Case::random(seed, n, levels);
            let weights = case.weights(seed + 1);
            for weights in [None, Some(weights.as_slice())] {
                for ncols in [3, 11] {
                    let mut runs = Vec::new();
                    for (preconditioner, fallback) in [
                        (Preconditioner::Auto, Fallback::Decline),
                        (Preconditioner::Jacobi, Fallback::Decline),
                        (Preconditioner::Auto, Fallback::Escalate),
                        (Preconditioner::Jacobi, Fallback::Escalate),
                    ] {
                        let mut settings = settings(preconditioner, fallback);
                        settings.lanes = Some(4);
                        let mut projector = Projector::new(case.design(), settings);
                        let mut data = case.columns(seed + 2, ncols);
                        let request = Request::new(TOLERANCE, 1000);
                        let (outcome, coefficients) =
                            project(&mut projector, &case, weights, &mut data, ncols, &request);
                        let report = projected(outcome);
                        assert_eq!(report.route.preconditioner, PreconditionerUsed::Jacobi);
                        assert_eq!((report.route.escalations, report.facts.escalations), (0, 0));
                        runs.push((bits(&data), bits(&coefficients), report.facts.iterations));
                    }
                    assert!(runs.windows(2).all(|pair| pair[0] == pair[1]));
                }
            }
        }
    });
}

/// Firms of a chain of movers (no stayers) on which the Jacobi recurrence
/// stalls in every batch of these tests' columns. The firms are eliminated,
/// and the retained block of `STALLING_CHAIN - 1` movers (effect 0) is
/// within the levels the automatic rule always solves densely.
const STALLING_CHAIN: usize = 256;

/// The reason, input state and facts of a declined projection.
fn declined(outcome: Outcome) -> (Decline, InputState, Facts) {
    match outcome {
        Outcome::Declined {
            reason,
            input,
            facts,
        } => (reason, input, facts),
        other => panic!("unexpected outcome: {other:?}"),
    }
}

/// The iterations the Jacobi recurrence (of the first batch) spends until it
/// stalls and declines untouched, also with an escalation fallback (Jacobi
/// never escalates).
fn jacobi_stall(
    case: &Case,
    weights: Option<&[f64]>,
    original: &[f64],
    ncols: usize,
    request: &Request,
) -> usize {
    let mut spent = Vec::new();
    for (preconditioner, fallback) in [
        (Preconditioner::Auto, Fallback::Decline),
        (Preconditioner::Jacobi, Fallback::Escalate),
    ] {
        let mut data = original.to_vec();
        let mut projector = Projector::new(case.design(), settings(preconditioner, fallback));
        let (outcome, coefficients) =
            project(&mut projector, case, weights, &mut data, ncols, request);
        let (reason, input, facts) = declined(outcome);
        assert_eq!(reason, Decline::Numerical("stalled reduced recurrence"));
        assert_eq!(input, InputState::Untouched);
        assert_eq!(facts.escalations, 0);
        assert_eq!(bits(&data), bits(original));
        assert!(coefficients.iter().all(|&c| c == 0.0));
        spent.push(facts.iterations);
    }
    assert_eq!(spent[0], spent[1]);
    assert!(0 < spent[0] && spent[0] < request.max_iterations);
    spent[0]
}

/// Run `check(years, weighted)` for the four variants of a chain test, each
/// on its own thread: their dense references dominate the time.
fn each_variant(check: impl Fn(bool, bool) + Sync) {
    let check = &check;
    std::thread::scope(|scope| {
        for years in [false, true] {
            for weighted in [false, true] {
                scope.spawn(move || check(years, weighted));
            }
        }
    });
}

#[test]
fn escalation_recovers_a_stalled_recurrence() {
    let cap = 1500;
    each_variant(|years, weighted| {
        let case = Case::chain(STALLING_CHAIN, 0, years);
        assert_eq!(case.largest(), 1);
        let weights = case.weights(5);
        let weights = weighted.then_some(weights.as_slice());
        let oracle = Oracle::new(&case, weights);
        let request = Request::new(TOLERANCE, cap);
        for ncols in [3, 11] {
            let original = case.columns(6, ncols);
            let stalled = jacobi_stall(&case, weights, &original, ncols, &request);
            // Every batch escalates from its stalled Jacobi iterate to an
            // exact movers' block (and an exact year block).
            let escalate = settings(Preconditioner::Auto, Fallback::Escalate);
            let mut projector = Projector::new(case.design(), escalate);
            let report =
                check_against_oracle(&case, weights, &oracle, &mut projector, ncols, 6, &request);
            let batches = ncols.div_ceil(8);
            assert_eq!(usize::from(report.route.escalations), batches);
            assert_eq!(report.facts.escalations, batches);
            let total = report.facts.iterations;
            assert!(stalled < total && total <= cap, "{:?}", report.facts);
            let used = used_blocks(&report);
            assert_eq!(used.len(), case.levels.len() - 1);
            assert_eq!((used[0].effect, used[0].levels), (0, STALLING_CHAIN - 1));
            assert!(used.iter().all(|block| block.local == LocalUsed::Dense));
            if !years && ncols <= 8 {
                // One exact block is the reduced system's pseudo-inverse.
                assert!(total <= stalled + 2, "{:?}", report.facts);
            }
            // The reported iterations are the batches' whole work: a budget
            // of exactly that many gives the same projection.
            let mut runs = Vec::new();
            for cap in [cap, total] {
                let mut projector = Projector::new(case.design(), escalate);
                let mut data = original.clone();
                let request = Request::new(TOLERANCE, cap);
                let (outcome, coefficients) =
                    project(&mut projector, &case, weights, &mut data, ncols, &request);
                let report = projected(outcome);
                runs.push((bits(&data), bits(&coefficients), report.facts.iterations));
            }
            assert_eq!(runs[0], runs[1]);
            assert_eq!(runs[0].2, total);
            // Explicit blocks from the first iteration agree and finish
            // before the Jacobi recurrence stalls.
            let mut projector = Projector::new(case.design(), blocks(None));
            let report =
                check_against_oracle(&case, weights, &oracle, &mut projector, ncols, 6, &request);
            assert_eq!(report.route.escalations, 0);
            assert!(report.facts.iterations < stalled, "{:?}", report.facts);
        }
    });
}

#[test]
fn escalation_needs_budget_left() {
    // A Jacobi recurrence that spends the whole iteration budget declines
    // untouched also with the escalation fallback: at the ceiling (before
    // the first stall window) and when it stalls at its last iteration.
    let case = Case::chain(STALLING_CHAIN, 0, true);
    let weights = case.weights(5);
    for weights in [None, Some(weights.as_slice())] {
        for ncols in [3, 11] {
            let original = case.columns(6, ncols);
            let stalled = jacobi_stall(
                &case,
                weights,
                &original,
                ncols,
                &Request::new(TOLERANCE, 1500),
            );
            for (cap, reason) in [
                (30, "iteration ceiling"),
                (stalled, "stalled reduced recurrence"),
            ] {
                for (preconditioner, fallback) in [
                    (Preconditioner::Auto, Fallback::Decline),
                    (Preconditioner::Jacobi, Fallback::Escalate),
                    (Preconditioner::Auto, Fallback::Escalate),
                ] {
                    let mut data = original.clone();
                    let mut projector =
                        Projector::new(case.design(), settings(preconditioner, fallback));
                    let request = Request::new(TOLERANCE, cap);
                    let (outcome, coefficients) =
                        project(&mut projector, &case, weights, &mut data, ncols, &request);
                    let (got, input, facts) = declined(outcome);
                    let run = format!("{preconditioner:?}, {fallback:?}, cap {cap}");
                    assert_eq!(got, Decline::Numerical(reason), "{run}");
                    assert_eq!(input, InputState::Untouched, "{run}");
                    assert_eq!((facts.iterations, facts.escalations), (cap, 0), "{run}");
                    assert_eq!(bits(&data), bits(&original));
                    assert!(coefficients.iter().all(|&c| c == 0.0));
                }
            }
        }
    }
}

#[test]
fn escalation_respects_the_memory_limit() {
    let case = Case::chain(STALLING_CHAIN, 0, false);
    let request = Request::new(TOLERANCE, 1500);
    let original = case.columns(6, 3);
    let escalate = settings(Preconditioner::Auto, Fallback::Escalate);
    // The workspace of the Jacobi route admits the projection but leaves no
    // room for the blocks.
    let mut projector = Projector::new(case.design(), escalate);
    assert!(projector.prepare(&Control::default()).unwrap());
    let admission = projector.admission_bytes(3).unwrap();
    let mut data = original.clone();
    let (outcome, _) = project(&mut projector, &case, None, &mut data, 3, &request);
    let report = projected(outcome);
    assert_eq!(report.facts.escalations, 1);
    assert!(report.workspace_bytes > admission);
    let mut limited = request;
    limited.memory_limit = Some(admission);
    let mut projector = Projector::new(case.design(), escalate);
    let mut data = original.clone();
    let (outcome, _) = project(&mut projector, &case, None, &mut data, 3, &limited);
    // The stalled Jacobi recurrence declines untouched.
    let (reason, input, facts) = declined(outcome);
    assert_eq!(reason, Decline::Numerical("stalled reduced recurrence"));
    assert_eq!(input, InputState::Untouched);
    assert_eq!(facts.escalations, 0);
    assert_eq!(bits(&data), bits(&original));
    // Explicit blocks count their bound at admission.
    let mut projector = Projector::new(case.design(), blocks(None));
    assert!(projector.prepare(&Control::default()).unwrap());
    let explicit = projector.admission_bytes(3).unwrap();
    assert!(explicit > admission);
    let (outcome, _) = project(&mut projector, &case, None, &mut data, 3, &limited);
    assert!(matches!(
        outcome,
        Outcome::Declined {
            reason: Decline::MemoryLimit,
            ..
        }
    ));
}

#[test]
fn escalation_fails_declined_when_blocks_cannot_finish() {
    // Jacobi blocks only (no dense block): the escalated recurrence is
    // Jacobi again and reaches the ceiling too.
    let case = Case::chain(60, 4, false);
    let request = Request::new(TOLERANCE, 10);
    let mut data = case.columns(8, 3);
    let original = data.clone();
    let spec = BlockSpec {
        large: LocalSolver::Jacobi,
        dense_max_levels: Some(0),
    };
    let mut projector = Projector::new(
        case.design(),
        settings(Preconditioner::BlockJacobi(spec), Fallback::Escalate),
    );
    let (outcome, _) = project(&mut projector, &case, None, &mut data, 3, &request);
    assert!(matches!(
        outcome,
        Outcome::Declined {
            reason: Decline::Numerical("iteration ceiling"),
            input: InputState::Untouched,
            ..
        }
    ));
    assert_eq!(bits(&data), bits(&original));
}

#[cfg(not(feature = "cmg"))]
#[test]
fn explicit_cmg_local_solves_are_unsupported() {
    let case = Case::random(21, 300, &[60, 7]);
    let spec = BlockSpec {
        large: LocalSolver::Cmg,
        dense_max_levels: None,
    };
    let mut projector = Projector::new(
        case.design(),
        settings(Preconditioner::BlockJacobi(spec), Fallback::Decline),
    );
    let mut data = case.columns(22, 2);
    let error = projector
        .project(
            Weights::Unit,
            ColumnsMut::new(&mut data, case.n, 2).unwrap(),
            None,
            &Request::new(TOLERANCE, 100),
            &Control::default(),
        )
        .unwrap_err();
    assert_eq!(
        error,
        Error::Unsupported("CMG local solves are not available in this build")
    );
    assert_eq!(BlockSpec::default().large, LocalSolver::Auto);
    assert_eq!(BlockSpec::default().dense_max_levels, None);
}

#[test]
fn block_projections_are_repeatable_and_follow_new_weights() {
    let case = Case::random(31, 900, &[150, 23, 6, 11, 4]);
    let request = Request::new(TOLERANCE, 1000);
    let mut projector = Projector::new(case.design(), blocks(Some(12)));
    let run = |projector: &mut Projector<'_>, weights: Option<&[f64]>| {
        let mut data = case.columns(32, 5);
        let (outcome, coefficients) = project(projector, &case, weights, &mut data, 5, &request);
        let report = projected(outcome);
        (bits(&data), bits(&coefficients), report.workspace_bytes)
    };
    let mut weights = case.weights(33);
    let first = run(&mut projector, Some(&weights));
    // Retained blocks and a fresh projector give the same bits.
    assert_eq!(run(&mut projector, Some(&weights)), first);
    let mut fresh = Projector::new(case.design(), blocks(Some(12)));
    assert_eq!(run(&mut fresh, Some(&weights)), first);
    // New values in the same buffer rebuild the blocks, at the same size.
    for w in weights.iter_mut().step_by(3) {
        *w *= 1.7;
    }
    let oracle = Oracle::new(&case, Some(&weights));
    let report = check_against_oracle(
        &case,
        Some(&weights),
        &oracle,
        &mut projector,
        5,
        34,
        &request,
    );
    assert_eq!(report.workspace_bytes, first.2);
}

/// Block-Jacobi with CMG on the blocks above the dense limit.
#[cfg(feature = "cmg")]
fn cmg_blocks(dense_max_levels: Option<usize>) -> Settings {
    let spec = BlockSpec {
        large: LocalSolver::Cmg,
        dense_max_levels,
    };
    settings(Preconditioner::BlockJacobi(spec), Fallback::Decline)
}

/// The chain's block (effect and levels) uses CMG and a year block the
/// exact solve.
#[cfg(feature = "cmg")]
fn assert_cmg_chain(report: &Report, (effect, levels): (usize, usize)) {
    let used = used_blocks(report);
    assert_eq!(
        (used[0].effect, used[0].levels, used[0].local),
        (effect, levels, LocalUsed::Cmg)
    );
    assert!(used[1..]
        .iter()
        .all(|block| block.local == LocalUsed::Dense));
}

#[cfg(feature = "cmg")]
#[test]
fn cmg_escalation_matches_the_dense_oracle() {
    // A chain of 260 firms and 259 movers: the movers' block is above the
    // automatic dense limit, so the escalated block takes CMG (one exact
    // terminal factor at this size), and the year block the dense solve.
    // The Jacobi recurrence stalls in every batch.
    let (cap, firms) = (1500, 260);
    each_variant(|years, weighted| {
        let case = Case::chain(firms, 0, years);
        assert_eq!(case.largest(), 1);
        let weights = case.weights(41);
        let weights = weighted.then_some(weights.as_slice());
        let oracle = Oracle::new(&case, weights);
        let request = Request::new(TOLERANCE, cap);
        for ncols in [3, 11] {
            let original = case.columns(42, ncols);
            let stalled = jacobi_stall(&case, weights, &original, ncols, &request);
            let mut projector = Projector::new(
                case.design(),
                settings(Preconditioner::Auto, Fallback::Escalate),
            );
            let report =
                check_against_oracle(&case, weights, &oracle, &mut projector, ncols, 42, &request);
            let batches = ncols.div_ceil(8);
            assert_eq!(usize::from(report.route.escalations), batches);
            assert_eq!(report.facts.escalations, batches);
            assert_cmg_chain(&report, (0, firms - 1));
            assert!(report.facts.iterations <= cap, "{:?}", report.facts);
            if ncols <= 8 {
                // Few iterations follow the stall of the one batch.
                let after = report.facts.iterations - stalled;
                eprintln!(
                    "chain of {firms} firms (years {years}, weighted {weighted}): \
                     {stalled} Jacobi and {after} CMG iterations"
                );
                assert!(after <= if years { 12 } else { 2 }, "{:?}", report.facts);
            }
            assert!(report.facts.block_seconds > 0.0);
        }
    });
}

#[cfg(feature = "cmg")]
#[test]
fn cmg_escalation_solves_a_long_chain() {
    // 3000 firms in a chain: a multilevel CMG hierarchy. The Jacobi
    // recurrence stalls; the escalated projection agrees with explicit
    // exact (dense) firm blocks.
    let (cap, firms, ncols) = (1500, 3000, 3);
    each_variant(|years, weighted| {
        let case = Case::chain(firms, 4, years);
        let weights = case.weights(43);
        let weights = weighted.then_some(weights.as_slice());
        let original = case.columns(44, ncols);
        let request = Request::new(TOLERANCE, cap);
        let stalled = jacobi_stall(&case, weights, &original, ncols, &request);
        let mut projector = Projector::new(
            case.design(),
            settings(Preconditioner::Auto, Fallback::Escalate),
        );
        let mut escalated = original.clone();
        let (outcome, coefficients) = project(
            &mut projector,
            &case,
            weights,
            &mut escalated,
            ncols,
            &request,
        );
        let report = projected(outcome);
        assert_eq!((report.route.escalations, report.facts.escalations), (1, 1));
        assert_cmg_chain(&report, (1, firms));
        let after = report.facts.iterations - stalled;
        // Explicit CMG blocks from the first iteration.
        let mut explicit = Projector::new(case.design(), cmg_blocks(None));
        let mut from_zero = original.clone();
        let (outcome, _) = project(
            &mut explicit,
            &case,
            weights,
            &mut from_zero,
            ncols,
            &request,
        );
        let explicit_report = projected(outcome);
        assert_cmg_chain(&explicit_report, (1, firms));
        eprintln!(
            "chain of {firms} firms (years {years}, weighted {weighted}): escalated {} = \
             {stalled} Jacobi + {after} CMG; explicit CMG {}",
            report.facts.iterations, explicit_report.facts.iterations
        );
        // Fewer than one stall window of iterations follow the stall.
        assert!(after < 40, "{:?}", report.facts);
        // Exact firm blocks agree (weighted runs; the unweighted ones with
        // the explicit CMG run). The dense factor of 3000 levels dominates
        // the test's time.
        let exact = if weighted {
            let mut dense = Projector::new(case.design(), blocks(Some(firms)));
            let mut exact = original.clone();
            let (outcome, _) = project(&mut dense, &case, weights, &mut exact, ncols, &request);
            let dense_report = projected(outcome);
            assert!(used_blocks(&dense_report)
                .iter()
                .all(|block| block.local == LocalUsed::Dense));
            exact
        } else {
            from_zero.clone()
        };
        let offsets = case.offsets();
        for j in 0..ncols {
            let y = column(&original, ncols, j);
            let magnitude = inf_norm(&y).max(1.0);
            let reference = column(&exact, ncols, j);
            for (k, residual) in [&escalated, &from_zero].into_iter().enumerate() {
                let r = column(residual, ncols, j);
                for (i, (a, b)) in r.iter().zip(&reference).enumerate() {
                    assert!(
                        (a - b).abs() <= 1e-7 * magnitude,
                        "run {k}, column {j}, row {i}: {a} vs {b}"
                    );
                }
            }
            let r = column(&escalated, ncols, j);
            for (i, (&yi, &ri)) in y.iter().zip(&r).enumerate() {
                let fit = case.fitted(&coefficients, &offsets, j, i);
                assert!(
                    (yi - fit - ri).abs() <= 1e-9 * magnitude,
                    "column {j}, row {i}"
                );
            }
        }
    });
}

#[cfg(feature = "cmg")]
#[test]
fn explicit_cmg_blocks_are_repeatable_and_follow_new_weights() {
    let firms = 3000;
    let case = Case::chain(firms, 4, true);
    let request = Request::new(TOLERANCE, 1500);
    let mut projector = Projector::new(case.design(), cmg_blocks(None));
    let run = |projector: &mut Projector<'_>, weights: &[f64]| {
        let mut data = case.columns(51, 5);
        let (outcome, coefficients) =
            project(projector, &case, Some(weights), &mut data, 5, &request);
        let report = projected(outcome);
        assert_cmg_chain(&report, (1, firms));
        (bits(&data), bits(&coefficients), report.workspace_bytes)
    };
    let mut weights = case.weights(52);
    let first = run(&mut projector, &weights);
    // The retained hierarchy and a fresh one give the same bits.
    assert_eq!(run(&mut projector, &weights), first);
    let mut fresh = Projector::new(case.design(), cmg_blocks(None));
    assert_eq!(run(&mut fresh, &weights), first);
    // New values in the same buffer rebuild the hierarchy: the projection
    // follows them and agrees with exact firm blocks.
    for w in weights.iter_mut().step_by(3) {
        *w *= 1.7;
    }
    let mut cmg = case.columns(53, 5);
    let (outcome, _) = project(&mut projector, &case, Some(&weights), &mut cmg, 5, &request);
    assert_cmg_chain(&projected(outcome), (1, firms));
    let mut dense = Projector::new(case.design(), blocks(Some(firms)));
    let mut exact = case.columns(53, 5);
    let (outcome, _) = project(&mut dense, &case, Some(&weights), &mut exact, 5, &request);
    projected(outcome);
    let original = case.columns(53, 5);
    for j in 0..5 {
        let bound = 1e-7 * inf_norm(&column(&original, 5, j)).max(1.0);
        for (a, b) in column(&cmg, 5, j).iter().zip(&column(&exact, 5, j)) {
            assert!((a - b).abs() <= bound, "column {j}: {a} vs {b}");
        }
    }
}

#[cfg(feature = "cmg")]
#[test]
fn cmg_blocks_are_admitted_within_the_memory_limit() {
    let case = Case::chain(3000, 4, false);
    let original = case.columns(61, 3);
    let run = |settings: Settings, limit: Option<u128>| {
        let mut projector = Projector::new(case.design(), settings);
        let mut data = original.clone();
        let mut request = Request::new(TOLERANCE, 1500);
        request.memory_limit = limit;
        let outcome = projector
            .project(
                Weights::Unit,
                ColumnsMut::new(&mut data, case.n, 3).unwrap(),
                None,
                &request,
                &Control::default(),
            )
            .unwrap();
        (outcome, data)
    };
    let escalate = settings(Preconditioner::Auto, Fallback::Escalate);
    let mut explicit = Projector::new(case.design(), cmg_blocks(None));
    assert!(explicit.prepare(&Control::default()).unwrap());
    // The projection's workspace and the shared Laplacian, without room for
    // a hierarchy.
    let tight = Some(explicit.admission_bytes(3).unwrap() + (64 << 10));
    for limit in [None, Some(1 << 30)] {
        let (outcome, _) = run(escalate, limit);
        let report = projected(outcome);
        assert_eq!(report.facts.escalations, 1);
        assert_cmg_chain(&report, (1, 3000));
        let (outcome, _) = run(cmg_blocks(None), limit);
        assert_cmg_chain(&projected(outcome), (1, 3000));
    }
    // An explicit request declines untouched ...
    let (outcome, data) = run(cmg_blocks(None), tight);
    assert!(matches!(
        outcome,
        Outcome::Declined {
            reason: Decline::MemoryLimit,
            input: InputState::Untouched,
            ..
        }
    ));
    assert_eq!(bits(&data), bits(&original));
    // ... while the escalation keeps Jacobi on the firm block, which stalls
    // again within the budget.
    let (outcome, data) = run(escalate, tight);
    let (reason, input, facts) = declined(outcome);
    assert_eq!(reason, Decline::Numerical("stalled reduced recurrence"));
    assert_eq!(input, InputState::Untouched);
    assert_eq!(facts.escalations, 1);
    assert!(facts.iterations < 1500, "{facts:?}");
    assert_eq!(bits(&data), bits(&original));
}
