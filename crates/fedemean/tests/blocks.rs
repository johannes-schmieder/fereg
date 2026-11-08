//! Block-Jacobi preconditioning and escalation against an independent dense
//! weighted least-squares reference (rank-revealing modified Gram-Schmidt
//! with reorthogonalization on `sqrt(W) D`, the explicit dummy matrix, as in
//! `tests/oracle.rs`), and the Jacobi routes' bitwise agreement.

use fedemean::{
    BlockSpec, BlockUsed, Choice, CoefficientLayout, CoefficientsMut, ColumnsMut, Control, Decline,
    Design, Effect, Error, Fallback, InputState, LocalSolver, LocalUsed, Outcome, Preconditioner,
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
    /// chain; an exact firm block solves it at once.
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

#[test]
fn escalation_recovers_the_iteration_ceiling() {
    for (years, cap) in [(false, 10), (true, 20)] {
        let case = Case::chain(60, 4, years);
        assert_eq!(case.largest(), 0);
        let weights = case.weights(5);
        for weights in [None, Some(weights.as_slice())] {
            let oracle = Oracle::new(&case, weights);
            let request = Request::new(TOLERANCE, cap);
            for ncols in [3, 11] {
                let original = case.columns(6, ncols);
                // Jacobi declines untouched, also with an escalation fallback.
                for (preconditioner, fallback) in [
                    (Preconditioner::Auto, Fallback::Decline),
                    (Preconditioner::Jacobi, Fallback::Escalate),
                ] {
                    let mut data = original.clone();
                    let mut projector =
                        Projector::new(case.design(), settings(preconditioner, fallback));
                    let (outcome, coefficients) =
                        project(&mut projector, &case, weights, &mut data, ncols, &request);
                    match outcome {
                        Outcome::Declined { reason, input, .. } => {
                            assert_eq!(reason, Decline::Numerical("iteration ceiling"));
                            assert_eq!(input, InputState::Untouched);
                        }
                        other => panic!("{preconditioner:?}: {other:?}"),
                    }
                    assert_eq!(bits(&data), bits(&original));
                    assert!(coefficients.iter().all(|&c| c == 0.0));
                }
                // Escalation continues from the Jacobi iterate with an exact
                // firm block (and an exact year block).
                let mut projector = Projector::new(
                    case.design(),
                    settings(Preconditioner::Auto, Fallback::Escalate),
                );
                let report = check_against_oracle(
                    &case,
                    weights,
                    &oracle,
                    &mut projector,
                    ncols,
                    6,
                    &request,
                );
                let batches = ncols.div_ceil(8);
                assert_eq!(usize::from(report.route.escalations), batches);
                assert_eq!(report.facts.escalations, batches);
                assert!(report.facts.iterations > cap, "{:?}", report.facts);
                let used = used_blocks(&report);
                assert_eq!(used.len(), case.levels.len() - 1);
                assert_eq!((used[0].effect, used[0].levels), (1, 60));
                assert!(used.iter().all(|block| block.local == LocalUsed::Dense));
                if !years {
                    // One exact block is the reduced system's pseudo-inverse.
                    assert!(report.facts.iterations <= cap + 2, "{:?}", report.facts);
                }
                // Explicit blocks from the first iteration agree.
                let mut projector = Projector::new(case.design(), blocks(None));
                let report = check_against_oracle(
                    &case,
                    weights,
                    &oracle,
                    &mut projector,
                    ncols,
                    6,
                    &request,
                );
                assert_eq!(report.route.escalations, 0);
                assert!(report.facts.iterations <= cap, "{:?}", report.facts);
            }
        }
    }
}

#[test]
fn escalation_respects_the_memory_limit() {
    let case = Case::chain(60, 4, false);
    let request = Request::new(TOLERANCE, 10);
    let original = case.columns(7, 3);
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
    match outcome {
        Outcome::Declined { reason, facts, .. } => {
            assert_eq!(reason, Decline::Numerical("iteration ceiling"));
            assert_eq!(facts.escalations, 0);
        }
        other => panic!("{other:?}"),
    }
    // Explicit blocks count their bound at admission.
    let mut projector = Projector::new(case.design(), blocks(None));
    assert!(projector.prepare(&Control::default()).unwrap());
    let explicit = projector.admission_bytes(3).unwrap();
    assert!(explicit > admission);
    let mut limited = Request::new(TOLERANCE, 10);
    limited.memory_limit = Some(admission);
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
