//! Lockstep modified LSMR has the outcome of its columns solved one at a
//! time in column order, on declines and escalations too: the iterations it
//! reports (which a caller charges to its budget), its decline, the columns
//! written before the declining one, and every column's bits.

use fedemean::{
    Choice, CoefficientLayout, CoefficientsMut, ColumnsMut, Control, Decline, Design, Effect,
    Facts, Fallback, InputState, Method, Outcome, Preconditioner, Projector, Reduction, Request,
    Settings, Weights,
};

const TOLERANCE: f64 = 1e-10;

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

    fn normal(&mut self) -> f64 {
        let u = ((self.next() >> 11) as f64 / (1_u64 << 53) as f64).max(1e-300);
        let v = (self.next() >> 11) as f64 / (1_u64 << 53) as f64;
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// Categorical effects (codes, levels), optional slopes on the first.
struct Case {
    n: usize,
    codes: Vec<Vec<u32>>,
    levels: Vec<usize>,
    slopes: Vec<Vec<f64>>,
    weights: Option<Vec<f64>>,
}

impl Case {
    /// A two-way panel whose firms form a path (Jacobi stalls on it):
    /// `workers` workers with `years` observations each at `workers / 10`
    /// firms, the first firms' workers moving to the next firm halfway.
    #[cfg(feature = "schwarz")]
    fn chain(workers: usize, years: usize) -> Self {
        let firms = workers / 10;
        let mut codes = vec![Vec::new(), Vec::new()];
        for w in 0..workers {
            for t in 0..years {
                let home = w % firms;
                let firm = if w + 1 < firms && t >= years / 2 {
                    home + 1
                } else {
                    home
                };
                codes[0].push(w as u32);
                codes[1].push(firm as u32);
            }
        }
        Self {
            n: workers * years,
            codes,
            levels: vec![workers, firms],
            slopes: Vec::new(),
            weights: None,
        }
    }

    fn random(seed: u64, n: usize, levels: &[usize]) -> Self {
        let mut rng = Rng(seed);
        let codes = levels
            .iter()
            .map(|&count| {
                (0..n)
                    .map(|i| if i < count { i } else { rng.below(count) } as u32)
                    .collect()
            })
            .collect();
        Self {
            n,
            codes,
            levels: levels.to_vec(),
            slopes: Vec::new(),
            weights: None,
        }
    }

    /// fereg's offset-slope boundary fixture (case 2): one effect of four
    /// levels with an intercept and two slopes near `±2^24` that differ by
    /// units within each level, weights 2 to 7, and the columns `y`, `q`,
    /// `v`. The unpreconditioned recurrence's stopping tests and the
    /// certificate part ways on it.
    fn boundary() -> (Self, Vec<f64>) {
        let mut data = Vec::with_capacity(3 * BOUNDARY_ROWS);
        for i in 0..BOUNDARY_ROWS {
            data.extend([Y[i], Q[i], V[i]]);
        }
        let case = Self {
            n: BOUNDARY_ROWS,
            codes: vec![G.to_vec()],
            levels: vec![4],
            slopes: vec![Z1.to_vec(), Z2.to_vec()],
            weights: Some(W.to_vec()),
        };
        (case, data)
    }

    fn design(&self) -> Design<'_> {
        let effects = self
            .codes
            .iter()
            .zip(&self.levels)
            .enumerate()
            .map(|(e, (codes, &levels))| {
                let effect = Effect::new(codes, levels);
                if e == 0 && !self.slopes.is_empty() {
                    effect
                        .with_slopes(true, self.slopes.iter().map(Vec::as_slice))
                        .unwrap()
                } else {
                    effect
                }
            })
            .collect();
        Design::new(self.n, effects).unwrap()
    }

    fn channels(&self) -> Vec<usize> {
        (0..self.levels.len())
            .map(|e| if e == 0 { 1 + self.slopes.len() } else { 1 })
            .collect()
    }

    fn per_rhs(&self) -> usize {
        self.levels
            .iter()
            .zip(self.channels())
            .map(|(l, c)| l * c)
            .sum()
    }

    /// Columns `j`: FE structure plus noise, `j % 3` choosing the mix.
    fn columns(&self, seed: u64, ncols: usize) -> Vec<f64> {
        let mut rng = Rng(seed);
        let mut data = vec![0.0; self.n * ncols];
        for j in 0..ncols {
            let effects: Vec<Vec<f64>> = self
                .levels
                .iter()
                .map(|&levels| (0..levels).map(|_| rng.normal()).collect())
                .collect();
            for i in 0..self.n {
                let fe: f64 = self
                    .codes
                    .iter()
                    .zip(&effects)
                    .map(|(codes, values)| values[codes[i] as usize])
                    .sum();
                let slope = self.slopes.first().map_or(0.0, |s| 1e-7 * s[i]);
                data[i * ncols + j] = 3.0 * fe + slope + (1 + j % 3) as f64 * rng.normal();
            }
        }
        data
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

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

fn column(data: &[f64], ncols: usize, j: usize) -> Vec<f64> {
    data.iter().skip(j).step_by(ncols).copied().collect()
}

/// A projection's outcome, data and coefficients, and the per-column reports.
struct Run {
    outcome: Outcome,
    data: Vec<f64>,
    coefficients: Vec<f64>,
    iterations: Vec<usize>,
}

fn project(
    projector: &mut Projector<'_>,
    case: &Case,
    (data, ncols): (&[f64], usize),
    request: &Request,
) -> Run {
    let mut data = data.to_vec();
    let offsets: Vec<usize> = case
        .levels
        .iter()
        .zip(case.channels())
        .scan(0, |next, (l, c)| {
            let offset = *next;
            *next += l * c;
            Some(offset)
        })
        .collect();
    let mut coefficients = vec![0.0; case.per_rhs() * ncols];
    let channels = case.channels();
    let layout =
        CoefficientLayout::new(case.per_rhs(), &offsets, &case.levels).with_channels(&channels);
    let columns = ColumnsMut::new(&mut data, case.n, ncols).unwrap();
    let out = Some(CoefficientsMut::new(&mut coefficients, layout));
    let weights = case
        .weights
        .as_deref()
        .map_or(Weights::Unit, Weights::Values);
    let outcome = projector
        .project(weights, columns, out, request, &Control::default())
        .unwrap();
    let iterations = projector.columns().iter().map(|c| c.iterations).collect();
    Run {
        outcome,
        data,
        coefficients,
        iterations,
    }
}

/// A decline's reason, input state and facts.
fn declined(outcome: &Outcome) -> (Decline, InputState, Facts) {
    match outcome {
        Outcome::Declined {
            reason,
            input,
            facts,
        } => (*reason, input.clone(), *facts),
        Outcome::Projected(_) => panic!("unexpected projection"),
        _ => panic!("unexpected outcome"),
    }
}

fn scale_bits(input: &InputState) -> Option<Vec<u64>> {
    match input {
        InputState::Untouched => None,
        InputState::SameProjection { reference_scales } => Some(bits(reference_scales)),
        _ => panic!("unexpected input state"),
    }
}

/// The projection one column at a time (the memory limit admits one column).
fn one_at_a_time(
    projector: &mut Projector<'_>,
    case: &Case,
    data: (&[f64], usize),
    request: &Request,
) -> Run {
    let mut narrow = *request;
    narrow.memory_limit = projector.admission_bytes(1);
    assert!(narrow.memory_limit < projector.admission_bytes(2));
    project(projector, case, data, &narrow)
}

/// `batch` and `alone` decline alike: reason, input state, the iterations a
/// caller charges, the solved columns' iterations, data and coefficients.
fn assert_same_decline(batch: &Run, alone: &Run, label: &str) -> Facts {
    let (reason, input, facts) = declined(&batch.outcome);
    let (alone_reason, alone_input, alone_facts) = declined(&alone.outcome);
    assert_eq!(reason, alone_reason, "{label}");
    assert_eq!(scale_bits(&input), scale_bits(&alone_input), "{label}");
    assert_eq!(facts.iterations, alone_facts.iterations, "{label}");
    assert_eq!(
        facts.column_iterations, alone_facts.column_iterations,
        "{label}"
    );
    assert_eq!(facts.accepted, alone_facts.accepted, "{label}");
    assert_eq!(facts.escalations, alone_facts.escalations, "{label}");
    assert_eq!(bits(&batch.data), bits(&alone.data), "{label}");
    assert_eq!(
        bits(&batch.coefficients),
        bits(&alone.coefficients),
        "{label}"
    );
    facts
}

/// A column whose setup declines (its scale is outside the plain-sum range)
/// ends the batch after the columns before it, which are solved and
/// written: the decline reports their iterations and their scales, and the
/// remaining budget hands the projection on exactly as one column at a time.
#[test]
fn a_setup_decline_ends_the_batch_after_the_columns_before_it() {
    let case = Case::random(501, 4000, &[600, 90, 7]);
    let ncols = 6;
    let mut original = case.columns(502, ncols);
    for i in 0..case.n {
        original[i * ncols + 2] *= 1e160;
    }
    let cap = 400;
    let request = Request::new(TOLERANCE, cap);
    let choice = settings(Method::Lsmr, Preconditioner::Jacobi, Fallback::Decline);
    let mut projector = Projector::new(case.design(), choice);
    let batch = project(&mut projector, &case, (&original, ncols), &request);
    let alone = one_at_a_time(&mut projector, &case, (&original, ncols), &request);
    let facts = assert_same_decline(&batch, &alone, "setup decline");
    let (reason, input, _) = declined(&batch.outcome);
    assert_eq!(
        reason,
        Decline::Numerical("RHS scale outside the plain-sum range")
    );
    let scales = scale_bits(&input).expect("two columns were written");
    assert!(scales[..2].iter().all(|&s| f64::from_bits(s).is_finite()));
    assert!(scales[2..].iter().all(|&s| f64::from_bits(s).is_nan()));
    assert!(facts.iterations > 0 && facts.column_iterations >= facts.iterations);
    for j in 2..ncols {
        assert_eq!(
            bits(&column(&batch.data, ncols, j)),
            bits(&column(&original, ncols, j))
        );
    }
    // The hand-off: the caller continues within the rest of its budget.
    let rest = Request::new(TOLERANCE, cap - facts.iterations);
    let fallback = settings(Method::Pcg, Preconditioner::Jacobi, Fallback::Decline);
    let mut after = Vec::new();
    for run in [&batch, &alone] {
        let mut fixed = run.data.clone();
        for i in 0..case.n {
            fixed[i * ncols + 2] = original[i * ncols + 2] * 1e-160;
        }
        let mut projector = Projector::new(case.design(), fallback);
        after.push(project(&mut projector, &case, (&fixed, ncols), &rest));
    }
    let [first, second] = &after[..] else {
        unreachable!()
    };
    assert!(matches!(first.outcome, Outcome::Projected(_)));
    assert_eq!(bits(&first.data), bits(&second.data));
    assert_eq!(bits(&first.coefficients), bits(&second.coefficients));
}

/// On fereg's offset-slope boundary fixture the unpreconditioned passes
/// stop on their estimates without the certificate: the batch declines as
/// one column at a time does at every tolerance and budget, charging the
/// first declining column's iterations and its predecessors', never a
/// later column's, so the caller's next route gets the same rest of its
/// budget.
#[test]
fn a_declining_batch_charges_what_one_column_at_a_time_charges() {
    let (case, original) = Case::boundary();
    let ncols = 3;
    let choice = settings(Method::Lsmr, Preconditioner::Identity, Fallback::Decline);
    let mut declines = 0;
    for tolerance in [1e-10, 1e-11] {
        for cap in [10, 21, 40, 72, 100] {
            let request = Request::new(tolerance, cap);
            let mut projector = Projector::new(case.design(), choice);
            let batch = project(&mut projector, &case, (&original, ncols), &request);
            let alone = one_at_a_time(&mut projector, &case, (&original, ncols), &request);
            let label = format!("tolerance {tolerance:e}, cap {cap}");
            if matches!(batch.outcome, Outcome::Projected(_)) {
                assert!(matches!(alone.outcome, Outcome::Projected(_)), "{label}");
                assert_eq!(bits(&batch.data), bits(&alone.data), "{label}");
                assert_eq!(batch.iterations, alone.iterations, "{label}");
                continue;
            }
            declines += 1;
            let facts = assert_same_decline(&batch, &alone, &label);
            assert!(facts.iterations <= cap, "{label}");
        }
    }
    assert!(declines > 0);
}

/// An escalating projection whose first column certifies on Jacobi: the
/// later columns run in lockstep, and the first of them in column order to
/// stall escalates; the columns after it start on Schwarz, as one column at
/// a time (here: each column alone, in order, on one projector).
#[cfg(feature = "schwarz")]
#[test]
fn a_later_escalation_has_the_one_column_route() {
    let case = Case::chain(1500, 8);
    let ncols = 5;
    let mut original = case.columns(521, ncols);
    for i in 0..case.n {
        // Explained by the cold start exactly: certifies on Jacobi at once.
        original[i * ncols] = 2.5;
    }
    let request = Request::new(TOLERANCE, 5000);
    let choice = settings(Method::Lsmr, Preconditioner::Jacobi, Fallback::Escalate);
    let mut projector = Projector::new(case.design(), choice);
    let batch = project(&mut projector, &case, (&original, ncols), &request);
    let Outcome::Projected(report) = &batch.outcome else {
        panic!("the batch declined");
    };
    assert_eq!(report.facts.escalations, 1);
    let mut alone = Projector::new(case.design(), choice);
    let mut escalations = 0;
    for j in 0..ncols {
        let single = column(&original, ncols, j);
        let one = project(&mut alone, &case, (&single, 1), &request);
        let Outcome::Projected(report) = one.outcome else {
            panic!("column {j} declined");
        };
        escalations += report.facts.escalations;
        assert_eq!(
            bits(&column(&batch.data, ncols, j)),
            bits(&one.data),
            "column {j}"
        );
        let per_rhs = case.per_rhs();
        let lockstep = &batch.coefficients[j * per_rhs..(j + 1) * per_rhs];
        assert_eq!(bits(lockstep), bits(&one.coefficients), "column {j}");
        assert_eq!(batch.iterations[j], one.iterations[0], "column {j}");
    }
    assert_eq!(escalations, 1);
}

const BOUNDARY_ROWS: usize = 128;

// fereg's `tests/fixtures/ols/ols_offset_slope_boundary.csv`, case 2 (exact).
const G: [u32; 128] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
];
const W: [f64; 128] = [
    2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 5.0, 5.0, 5.0,
    5.0, 5.0, 5.0, 5.0, 5.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0,
    2.0, 2.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 7.0,
    7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0,
    3.0, 3.0, 3.0, 3.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0,
    7.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 5.0, 5.0,
    5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0,
];
const Z1: [f64; 128] = [
    16777215.0, 16777217.0, 16777215.0, 16777217.0, 16777215.0, 16777217.0, 16777215.0, 16777217.0,
    16777215.0, 16777217.0, 16777215.0, 16777217.0, 16777215.0, 16777217.0, 16777215.0, 16777217.0,
    16777215.0, 16777217.0, 16777215.0, 16777217.0, 16777215.0, 16777217.0, 16777215.0, 16777217.0,
    16777215.0, 16777217.0, 16777215.0, 16777217.0, 16777215.0, 16777217.0, 16777215.0, 16777217.0,
    16777214.0, 16777218.0, 16777214.0, 16777218.0, 16777214.0, 16777218.0, 16777214.0, 16777218.0,
    16777214.0, 16777218.0, 16777214.0, 16777218.0, 16777214.0, 16777218.0, 16777214.0, 16777218.0,
    16777214.0, 16777218.0, 16777214.0, 16777218.0, 16777214.0, 16777218.0, 16777214.0, 16777218.0,
    16777214.0, 16777218.0, 16777214.0, 16777218.0, 16777214.0, 16777218.0, 16777214.0, 16777218.0,
    16777213.0, 16777219.0, 16777213.0, 16777219.0, 16777213.0, 16777219.0, 16777213.0, 16777219.0,
    16777213.0, 16777219.0, 16777213.0, 16777219.0, 16777213.0, 16777219.0, 16777213.0, 16777219.0,
    16777213.0, 16777219.0, 16777213.0, 16777219.0, 16777213.0, 16777219.0, 16777213.0, 16777219.0,
    16777213.0, 16777219.0, 16777213.0, 16777219.0, 16777213.0, 16777219.0, 16777213.0, 16777219.0,
    16777212.0, 16777220.0, 16777212.0, 16777220.0, 16777212.0, 16777220.0, 16777212.0, 16777220.0,
    16777212.0, 16777220.0, 16777212.0, 16777220.0, 16777212.0, 16777220.0, 16777212.0, 16777220.0,
    16777212.0, 16777220.0, 16777212.0, 16777220.0, 16777212.0, 16777220.0, 16777212.0, 16777220.0,
    16777212.0, 16777220.0, 16777212.0, 16777220.0, 16777212.0, 16777220.0, 16777212.0, 16777220.0,
];
const Z2: [f64; 128] = [
    -16777217.0000099,
    -16777215.0000099,
    -16777216.9999901,
    -16777214.9999901,
    -16777217.0000099,
    -16777215.0000099,
    -16777216.9999901,
    -16777214.9999901,
    -16777217.0000099,
    -16777215.0000099,
    -16777216.9999901,
    -16777214.9999901,
    -16777217.0000099,
    -16777215.0000099,
    -16777216.9999901,
    -16777214.9999901,
    -16777217.0000099,
    -16777215.0000099,
    -16777216.9999901,
    -16777214.9999901,
    -16777217.0000099,
    -16777215.0000099,
    -16777216.9999901,
    -16777214.9999901,
    -16777217.0000099,
    -16777215.0000099,
    -16777216.9999901,
    -16777214.9999901,
    -16777217.0000099,
    -16777215.0000099,
    -16777216.9999901,
    -16777214.9999901,
    -16777218.0000198,
    -16777214.0000198,
    -16777217.9999802,
    -16777213.9999802,
    -16777218.0000198,
    -16777214.0000198,
    -16777217.9999802,
    -16777213.9999802,
    -16777218.0000198,
    -16777214.0000198,
    -16777217.9999802,
    -16777213.9999802,
    -16777218.0000198,
    -16777214.0000198,
    -16777217.9999802,
    -16777213.9999802,
    -16777218.0000198,
    -16777214.0000198,
    -16777217.9999802,
    -16777213.9999802,
    -16777218.0000198,
    -16777214.0000198,
    -16777217.9999802,
    -16777213.9999802,
    -16777218.0000198,
    -16777214.0000198,
    -16777217.9999802,
    -16777213.9999802,
    -16777218.0000198,
    -16777214.0000198,
    -16777217.9999802,
    -16777213.9999802,
    -16777219.0000297,
    -16777213.0000297,
    -16777218.9999703,
    -16777212.9999703,
    -16777219.0000297,
    -16777213.0000297,
    -16777218.9999703,
    -16777212.9999703,
    -16777219.0000297,
    -16777213.0000297,
    -16777218.9999703,
    -16777212.9999703,
    -16777219.0000297,
    -16777213.0000297,
    -16777218.9999703,
    -16777212.9999703,
    -16777219.0000297,
    -16777213.0000297,
    -16777218.9999703,
    -16777212.9999703,
    -16777219.0000297,
    -16777213.0000297,
    -16777218.9999703,
    -16777212.9999703,
    -16777219.0000297,
    -16777213.0000297,
    -16777218.9999703,
    -16777212.9999703,
    -16777219.0000297,
    -16777213.0000297,
    -16777218.9999703,
    -16777212.9999703,
    -16777220.0000396,
    -16777212.0000396,
    -16777219.9999604,
    -16777211.9999604,
    -16777220.0000396,
    -16777212.0000396,
    -16777219.9999604,
    -16777211.9999604,
    -16777220.0000396,
    -16777212.0000396,
    -16777219.9999604,
    -16777211.9999604,
    -16777220.0000396,
    -16777212.0000396,
    -16777219.9999604,
    -16777211.9999604,
    -16777220.0000396,
    -16777212.0000396,
    -16777219.9999604,
    -16777211.9999604,
    -16777220.0000396,
    -16777212.0000396,
    -16777219.9999604,
    -16777211.9999604,
    -16777220.0000396,
    -16777212.0000396,
    -16777219.9999604,
    -16777211.9999604,
    -16777220.0000396,
    -16777212.0000396,
    -16777219.9999604,
    -16777211.9999604,
];
const Y: [f64; 128] = [
    -0.20000000000000007,
    -1.0,
    -1.2,
    0.7999999999999999,
    0.6,
    -0.19999999999999996,
    -0.3999999999999999,
    1.6,
    -0.20000000000000007,
    -1.0,
    -1.2,
    0.7999999999999999,
    0.6,
    -0.19999999999999996,
    -0.3999999999999999,
    1.6,
    -0.20000000000000007,
    -1.0,
    -1.2,
    0.7999999999999999,
    0.6,
    -0.19999999999999996,
    -0.3999999999999999,
    1.6,
    -0.20000000000000007,
    -1.0,
    -1.2,
    0.7999999999999999,
    0.6,
    -0.19999999999999996,
    -0.3999999999999999,
    1.6,
    -0.07500000000000007,
    -0.875,
    -1.075,
    0.9249999999999999,
    0.725,
    -0.07499999999999996,
    -0.2749999999999999,
    1.7249999999999999,
    -0.07500000000000007,
    -0.875,
    -1.075,
    0.9249999999999999,
    0.725,
    -0.07499999999999996,
    -0.2749999999999999,
    1.7249999999999999,
    -0.07500000000000007,
    -0.875,
    -1.075,
    0.9249999999999999,
    0.725,
    -0.07499999999999996,
    -0.2749999999999999,
    1.7249999999999999,
    -0.07500000000000007,
    -0.875,
    -1.075,
    0.9249999999999999,
    0.725,
    -0.07499999999999996,
    -0.2749999999999999,
    1.7249999999999999,
    0.04999999999999993,
    -0.75,
    -0.95,
    1.0499999999999998,
    0.85,
    0.050000000000000044,
    -0.1499999999999999,
    1.8499999999999999,
    0.04999999999999993,
    -0.75,
    -0.95,
    1.0499999999999998,
    0.85,
    0.050000000000000044,
    -0.1499999999999999,
    1.8499999999999999,
    0.04999999999999993,
    -0.75,
    -0.95,
    1.0499999999999998,
    0.85,
    0.050000000000000044,
    -0.1499999999999999,
    1.8499999999999999,
    0.04999999999999993,
    -0.75,
    -0.95,
    1.0499999999999998,
    0.85,
    0.050000000000000044,
    -0.1499999999999999,
    1.8499999999999999,
    0.17499999999999993,
    -0.625,
    -0.825,
    1.1749999999999998,
    0.975,
    0.17500000000000004,
    -0.02499999999999991,
    1.9749999999999999,
    0.17499999999999993,
    -0.625,
    -0.825,
    1.1749999999999998,
    0.975,
    0.17500000000000004,
    -0.02499999999999991,
    1.9749999999999999,
    0.17499999999999993,
    -0.625,
    -0.825,
    1.1749999999999998,
    0.975,
    0.17500000000000004,
    -0.02499999999999991,
    1.9749999999999999,
    0.17499999999999993,
    -0.625,
    -0.825,
    1.1749999999999998,
    0.975,
    0.17500000000000004,
    -0.02499999999999991,
    1.9749999999999999,
];
const Q: [f64; 128] = [
    1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0,
    -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0,
    -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0,
    1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0,
    1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0,
    -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0,
    -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0,
    1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0,
];
const V: [f64; 128] = [
    -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0,
    -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0,
    1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0,
    1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0,
    -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0,
    -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0,
    1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0,
    1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0,
];
