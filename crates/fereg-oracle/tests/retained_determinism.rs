//! The formerly failing retained-dependency probe now enforces exact worker identity.
use fereg_engine::threads::ThreadTeam;
use within::{Effect, LocalSolverConfig, LsmrOptions, PreconditionerConfig, ReductionStrategy};

#[test]
fn pinned_lsmr_thread_rounding_regression_is_fixed() {
    let n = 20_000;
    let a: Vec<u32> = (0..n).map(|i| (i % 311) as u32).collect();
    let b: Vec<u32> = (0..n).map(|i| ((i / 3 + i % 7) % 97) as u32).collect();
    let c: Vec<u32> = (0..n).map(|i| ((i / 19) % 11) as u32).collect();
    let y: Vec<f64> = (0..n)
        .map(|i| (i as f64 * 0.123).sin() + (i % 17) as f64 / 19.)
        .collect();
    let weights: Vec<f64> = (0..n).map(|i| 0.7 + (i % 13) as f64 / 7.).collect();
    for additive in [false, true] {
        let mut reference = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let result = team.install(|| {
                let effects = vec![
                    Effect::new(&a, true, []).unwrap(),
                    Effect::new(&b, true, []).unwrap(),
                    Effect::new(&c, true, []).unwrap(),
                ];
                let preconditioner = if additive {
                    PreconditionerConfig::Additive {
                        local_solver: LocalSolverConfig::default(),
                        reduction: ReductionStrategy::ParallelReduction,
                    }
                } else {
                    PreconditionerConfig::Diagonal
                };
                within::solve_batch(
                    effects,
                    &[y.as_slice()],
                    Some(&weights),
                    &LsmrOptions {
                        tol: 1e-12,
                        maxiter: 1000,
                        local_size: None,
                    },
                    preconditioner,
                )
                .unwrap()
            });
            assert!(result.converged.iter().all(|&value| value));
            let fitted: Vec<_> = (0..n)
                .map(|row| {
                    result.x[a[row] as usize]
                        + result.x[311 + b[row] as usize]
                        + result.x[408 + c[row] as usize]
                })
                .collect();
            let bits = |values: Vec<f64>| values.into_iter().map(f64::to_bits).collect::<Vec<_>>();
            let actual = (
                bits(result.x),
                bits(fitted),
                bits(result.demeaned),
                result.iterations,
            );
            if let Some(expected) = &reference {
                assert!(
                    expected == &actual,
                    "additive={additive}, workers={workers}"
                );
            } else {
                reference = Some(actual);
            }
        }
    }
}
