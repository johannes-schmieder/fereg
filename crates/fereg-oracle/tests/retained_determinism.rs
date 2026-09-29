//! Qualification diagnostic for the retained dependency, not a relaxed engine contract.
use fereg_engine::threads::ThreadTeam;
use within::{Effect, LocalSolverConfig, LsmrOptions, PreconditionerConfig, ReductionStrategy};

#[test]
#[ignore = "known pinned dependency rounding diagnostic; M3.5 acceptance requires an upstream decision"]
fn pinned_lsmr_thread_rounding_is_reproduced() {
    let n = 20_000;
    let a: Vec<u32> = (0..n).map(|i| (i % 311) as u32).collect();
    let b: Vec<u32> = (0..n).map(|i| ((i / 3 + i % 7) % 97) as u32).collect();
    let c: Vec<u32> = (0..n).map(|i| ((i / 19) % 11) as u32).collect();
    let y: Vec<f64> = (0..n)
        .map(|i| (i as f64 * 0.123).sin() + (i % 17) as f64 / 19.)
        .collect();
    let weights: Vec<f64> = (0..n).map(|i| 0.7 + (i % 13) as f64 / 7.).collect();
    for additive in [false, true] {
        let mut reference: Option<Vec<f64>> = None;
        let mut observed_difference = false;
        let mut fitted_reference: Option<Vec<f64>> = None;
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
            if let Some(reference) = &fitted_reference {
                let changed = reference
                    .iter()
                    .zip(&fitted)
                    .filter(|(a, b)| a.to_bits() != b.to_bits())
                    .count();
                let maximum = reference
                    .iter()
                    .zip(&fitted)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0., f64::max);
                println!("additive={additive} workers={workers}: {changed}/{n} fitted sums changed; max_abs={maximum:.16e}");
                observed_difference |= changed > 0;
            } else {
                fitted_reference = Some(fitted);
            }

            if let Some(reference) = &reference {
                let changed = reference
                    .iter()
                    .zip(&result.x)
                    .filter(|(a, b)| a.to_bits() != b.to_bits())
                    .count();
                let maximum = reference
                    .iter()
                    .zip(&result.x)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0., f64::max);
                println!("additive={additive} workers={workers}: {changed}/{} changed; max_abs={maximum:.16e}", reference.len());
                assert!(
                    maximum < 1e-10,
                    "the known rounding discrepancy became a larger numerical error"
                );
            } else {
                reference = Some(result.x);
            }
        }
        assert!(
            observed_difference,
            "reassess the pinned-dependency blocker: this diagnostic no longer reproduces it"
        );
    }
}
