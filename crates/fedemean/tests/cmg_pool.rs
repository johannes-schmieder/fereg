//! CMG local solves on the caller's pool ([`Control::with_pool`]): a block
//! graph large enough for CMG's planned threshold builds and applies on the
//! pool as a borrowed team, with residuals and coefficients bitwise equal to
//! the serial solves at every worker count, repeatably.
#![cfg(feature = "cmg")]

use fedemean::{
    BlockSpec, Choice, CoefficientLayout, CoefficientsMut, ColumnsMut, Control, Design, Effect,
    Fallback, LocalSolver, LocalUsed, Outcome, Preconditioner, PreconditionerUsed, Projector,
    Report, Request, Settings, Weights,
};

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
}

/// Workers (eliminated) at five distinct firms each, so the firm block's
/// CMG graph has ten clique edges per worker: 37,000 workers give 370,000
/// edges, above CMG's planned threshold of 350,000.
struct Case {
    n: usize,
    levels: [usize; 2],
    codes: [Vec<u32>; 2],
    data: Vec<f64>,
    weights: Vec<f64>,
}

const COLUMNS: usize = 3;

impl Case {
    fn new() -> Self {
        let (workers, firms) = (37_000, 6_000);
        let mut rng = Rng(5);
        let mut codes = [Vec::new(), Vec::new()];
        for worker in 0..workers {
            let mut chosen: Vec<u32> = Vec::new();
            while chosen.len() < 5 {
                // Skewed firm sizes; every firm appears via the first pass.
                let firm = if worker * 5 + chosen.len() < firms {
                    (worker * 5 + chosen.len()) as u32
                } else {
                    rng.below(firms).min(rng.below(firms)) as u32
                };
                if !chosen.contains(&firm) {
                    chosen.push(firm);
                }
            }
            for firm in chosen {
                codes[0].push(worker as u32);
                codes[1].push(firm);
            }
        }
        let n = codes[0].len();
        let data = (0..n * COLUMNS).map(|_| rng.uniform() - 0.5).collect();
        let weights = (0..n).map(|_| 0.25 + rng.uniform()).collect();
        Self {
            n,
            levels: [workers, firms],
            codes,
            data,
            weights,
        }
    }

    fn design(&self) -> Design<'_> {
        let effects = self
            .codes
            .iter()
            .zip(self.levels)
            .map(|(codes, levels)| Effect::new(codes, levels))
            .collect();
        Design::new(self.n, effects).unwrap()
    }
}

fn settings() -> Settings {
    Settings {
        choice: Choice {
            preconditioner: Preconditioner::BlockJacobi(BlockSpec {
                large: LocalSolver::Cmg,
                dense_max_levels: None,
            }),
            fallback: Fallback::Decline,
            ..Choice::default()
        },
        lanes: None,
    }
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

/// Residual bits, coefficient bits and the report of one projection.
fn project(
    projector: &mut Projector<'_>,
    case: &Case,
    control: &Control<'_>,
) -> (Vec<u64>, Vec<u64>, Report) {
    let mut data = case.data.clone();
    let per_rhs = case.levels[0] + case.levels[1];
    let offsets = [0, case.levels[0]];
    let mut coefficients = vec![0.0; per_rhs * COLUMNS];
    let outcome = projector
        .project(
            Weights::Values(&case.weights),
            ColumnsMut::new(&mut data, case.n, COLUMNS).unwrap(),
            Some(CoefficientsMut::new(
                &mut coefficients,
                CoefficientLayout::new(per_rhs, &offsets, &case.levels),
            )),
            &Request::new(1e-10, 500),
            control,
        )
        .unwrap();
    let Outcome::Projected(report) = outcome else {
        panic!("declined: {outcome:?}");
    };
    let PreconditionerUsed::Block(blocks) = report.route.preconditioner else {
        panic!("not a block route");
    };
    assert_eq!(blocks[0].map(|block| block.local), Some(LocalUsed::Cmg));
    (bits(&data), bits(&coefficients), report)
}

#[test]
fn pooled_cmg_blocks_reproduce_serial_bits_at_every_worker_count() {
    // The elimination's own lanes follow the pool's worker count, so each
    // pool is compared with a pool-free projection on that same pool.
    let case = Case::new();
    let mut previous: Option<Vec<u64>> = None;
    for threads in [1, 2, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            let serial = {
                let mut projector = Projector::new(case.design(), settings());
                project(&mut projector, &case, &Control::default())
            };
            assert!(serial.2.certificate.passed);
            let never = || false;
            let control = Control::with_cancel(&never).with_pool(&pool);
            let mut projector = Projector::new(case.design(), settings());
            let first = project(&mut projector, &case, &control);
            assert_eq!(first.0, serial.0, "{threads} workers: residuals");
            assert_eq!(first.1, serial.1, "{threads} workers: coefficients");
            assert_eq!(
                first.2.facts.iterations, serial.2.facts.iterations,
                "{threads} workers"
            );
            // A multi-worker team keeps a plan (counted in the blocks'
            // bytes); one worker has no team levels and keeps none.
            if threads > 1 {
                assert!(first.2.workspace_bytes > serial.2.workspace_bytes);
            } else {
                assert_eq!(first.2.workspace_bytes, serial.2.workspace_bytes);
            }
            // The retained blocks repeat, and serve a pool-free call.
            let again = project(&mut projector, &case, &control);
            assert_eq!((&again.0, &again.1), (&first.0, &first.1));
            let unpooled = project(&mut projector, &case, &Control::default());
            assert_eq!((&unpooled.0, &unpooled.1), (&serial.0, &serial.1));
            // Across worker counts the residuals agree far inside the
            // tolerance.
            if let Some(previous) = &previous {
                let scale = case.data.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
                for (a, b) in first.0.iter().zip(previous) {
                    let (a, b) = (f64::from_bits(*a), f64::from_bits(*b));
                    assert!(
                        (a - b).abs() <= 1e-9 * scale,
                        "{threads} workers: {a} vs {b}"
                    );
                }
            }
            previous = Some(first.0);
        });
    }
}
