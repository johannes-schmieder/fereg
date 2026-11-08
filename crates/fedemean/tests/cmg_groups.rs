//! CMG local solves below CMG's planned threshold on the caller's pool
//! ([`Control::with_pool`]): the block's lanes split into concurrent groups,
//! with residuals, coefficients and iterations bitwise equal to the
//! pool-free solves at every worker count, repeatably.
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

/// Low mobility: firms in a chain, each linked to the next by one mover and
/// to a nearby firm by another, plus stayers. Workers (eliminated) at two
/// firms give the firm block's CMG graph one edge each: about 6,000 edges
/// on 3,000 firms, between the group threshold and CMG's planned one.
struct Case {
    n: usize,
    levels: [usize; 2],
    codes: [Vec<u32>; 2],
    data: Vec<f64>,
    weights: Vec<f64>,
}

const COLUMNS: usize = 5;

impl Case {
    fn new() -> Self {
        let firms = 3_000;
        let mut rng = Rng(11);
        let mut codes = [Vec::new(), Vec::new()];
        let mut workers = 0_u32;
        let mut hire = |firm_list: &[usize], rows: usize| {
            for &firm in firm_list {
                for _ in 0..rows {
                    codes[0].push(workers);
                    codes[1].push(firm as u32);
                }
            }
            workers += 1;
        };
        for firm in 0..firms {
            for _ in 0..3 {
                hire(&[firm], 2);
            }
            if firm + 1 < firms {
                hire(&[firm, firm + 1], 1);
            }
            let near = (firm + 2 + rng.below(5)).min(firms - 1);
            if near != firm {
                hire(&[firm, near], 1);
            }
        }
        let n = codes[0].len();
        let data = (0..n * COLUMNS).map(|_| rng.uniform() - 0.5).collect();
        let weights = (0..n).map(|_| 0.25 + rng.uniform()).collect();
        Self {
            n,
            levels: [workers as usize, firms],
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
            &Request::new(1e-10, 2_000),
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
fn pooled_cmg_lane_groups_reproduce_serial_bits_at_every_worker_count() {
    let case = Case::new();
    assert!(case.levels[0] > case.levels[1]);
    for threads in [1, 2, 4, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        // The elimination's own lanes follow the pool's worker count, so
        // each pool is compared with a pool-free projection on that pool.
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
            assert_eq!(first.2.facts.iterations, serial.2.facts.iterations);
            // Several workers hold one group per worker (other buffers and
            // CMG workspaces); one worker holds the serial group.
            let grouped = first.2.workspace_bytes != serial.2.workspace_bytes;
            assert_eq!(grouped, threads > 1, "{threads} workers");
            // The retained groups repeat, and serve a pool-free call.
            let again = project(&mut projector, &case, &control);
            assert_eq!((&again.0, &again.1), (&first.0, &first.1));
            let unpooled = project(&mut projector, &case, &Control::default());
            assert_eq!((&unpooled.0, &unpooled.1), (&serial.0, &serial.1));
        });
    }
}
