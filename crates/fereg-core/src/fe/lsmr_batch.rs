//! Bounded transport around the pinned independent-RHS solver.
use std::time::Instant;

use crate::error::{ErrorCode, FeregError};
use crate::memory::LsmrBatchPolicy;
use within::{BatchSolveResult, Effect, LsmrOptions, PreconditionerConfig, Solver, WithinError};

#[derive(Debug)]
pub(super) enum BatchError {
    Upstream(WithinError),
    Allocation(FeregError),
}

impl BatchError {
    pub(super) fn preconditioner_breakdown(&self) -> bool {
        matches!(self, Self::Upstream(WithinError::Solve(within::SolveError::InvalidInput {
            context, message
        })) if *context == "mlsmr" && message.contains("preconditioner not positive definite"))
    }

    pub(super) fn into_fereg(self, context: &str) -> FeregError {
        match self {
            Self::Allocation(error) => error,
            Self::Upstream(error) => {
                FeregError::new(ErrorCode::Internal, format!("{context}: {error}"))
            }
        }
    }
}

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<(), BatchError> {
    values.try_reserve_exact(additional).map_err(|_| {
        BatchError::Allocation(FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve bounded LSMR transport",
        ))
    })
}

fn cells(rows: usize, columns: usize) -> Result<usize, BatchError> {
    rows.checked_mul(columns).ok_or_else(|| {
        BatchError::Allocation(FeregError::new(
            ErrorCode::Allocation,
            "bounded LSMR transport dimensions overflow",
        ))
    })
}

pub(super) fn solve_batch<'a>(
    effects: Vec<Effect<'a>>,
    rhs: &[&[f64]],
    weights: Option<&[f64]>,
    options: &LsmrOptions,
    preconditioner: PreconditionerConfig,
) -> Result<BatchSolveResult, BatchError> {
    solve_batch_with_policy(effects, rhs, weights, options, preconditioner, LsmrBatchPolicy::Default)
}

pub(super) fn solve_batch_with_policy<'a>(
    effects: Vec<Effect<'a>>,
    rhs: &[&[f64]],
    weights: Option<&[f64]>,
    options: &LsmrOptions,
    preconditioner: PreconditionerConfig,
    batch_policy: LsmrBatchPolicy,
) -> Result<BatchSolveResult, BatchError> {
    // Keep the upstream construction/transport lifetime for narrow batches.
    // Only a windowed batch needs to retain its solver across several calls.
    if rhs.len() <= batch_policy.rhs_window(rhs.len(), rayon::current_num_threads()) {
        return within::solve_batch(effects, rhs, weights, options, preconditioner)
            .map_err(BatchError::Upstream);
    }
    let started = Instant::now();
    let weights = weights
        .map(|weights| {
            let mut copy = Vec::new();
            reserve(&mut copy, weights.len())?;
            copy.extend_from_slice(weights);
            Ok(copy)
        })
        .transpose()?;
    let solver = Solver::new(effects, weights, preconditioner)
        .map_err(|error| BatchError::Upstream(error.into()))?;
    let setup = started.elapsed().as_secs_f64();
    let mut result = solve_prepared_batch(&solver, rhs, options, batch_policy)?;
    result.time_setup += setup;
    result.time_total = started.elapsed().as_secs_f64();
    Ok(result)
}

pub(super) fn solve_prepared_batch(
    solver: &Solver<'_>,
    rhs: &[&[f64]],
    options: &LsmrOptions,
    batch_policy: LsmrBatchPolicy,
) -> Result<BatchSolveResult, BatchError> {
    // An explicit outer window bounds live recurrences even when nested Rayon
    // suspends one RHS and starts another. Use the installed team, including
    // checkpoints whose LsmrConfig retains the machine-default thread setting.
    let window = batch_policy.rhs_window(rhs.len(), rayon::current_num_threads());
    if rhs.len() <= window {
        return solver
            .solve_batch(rhs, options)
            .map_err(|error| BatchError::Upstream(error.into()));
    }
    let started = Instant::now();
    let mut windows = rhs.chunks(window);
    let mut result = solver
        .solve_batch(windows.next().unwrap_or(&[]), options)
        .map_err(|error| BatchError::Upstream(error.into()))?;
    let remaining = rhs.len() - result.iterations.len();
    reserve(&mut result.x, cells(result.n_dofs, remaining)?)?;
    reserve(&mut result.demeaned, cells(result.n_obs, remaining)?)?;
    reserve(&mut result.converged, remaining)?;
    reserve(&mut result.iterations, remaining)?;
    reserve(&mut result.residual, remaining)?;
    reserve(&mut result.time_solve, remaining)?;
    for rhs in windows {
        let mut next = solver
            .solve_batch(rhs, options)
            .map_err(|error| BatchError::Upstream(error.into()))?;
        // Every window uses the same solver: layout, unidentified columns and
        // build warnings are invariant. Append in original RHS order. On any
        // error, drop all earlier windows before the caller retries the batch.
        result.x.append(&mut next.x);
        result.demeaned.append(&mut next.demeaned);
        result.converged.append(&mut next.converged);
        result.iterations.append(&mut next.iterations);
        result.residual.append(&mut next.residual);
        result.time_solve.append(&mut next.time_solve);
        result.time_setup += next.time_setup;
    }
    result.time_total = started.elapsed().as_secs_f64();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_preserve_all_outputs_against_the_upstream_full_batch() {
        let rows = 1024;
        let left: Vec<u32> = (0..rows).map(|row| (row % 32) as u32).collect();
        let right: Vec<u32> = (0..rows).map(|row| ((row / 7) % 27) as u32).collect();
        let weights: Vec<f64> = (0..rows)
            .map(|row| 0.5 + (row % 19) as f64 / 19.0)
            .collect();
        let columns: Vec<Vec<f64>> = (0..67)
            .map(|column| {
                (0..rows)
                    .map(|row| ((row * 17 + column * 31) as f64 * 0.0031).sin())
                    .collect()
            })
            .collect();
        let rhs: Vec<&[f64]> = columns.iter().map(Vec::as_slice).collect();
        let options = LsmrOptions {
            tol: 1e-10,
            maxiter: 500,
            local_size: Some(4),
        };
        for threads in [1, 3, 4, 9] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    for preconditioner in [
                        PreconditionerConfig::Off,
                        PreconditionerConfig::Diagonal,
                        PreconditionerConfig::default(),
                    ] {
                        let solver = Solver::new(
                            vec![
                                Effect::new(&left, true, []).unwrap(),
                                Effect::new(&right, true, []).unwrap(),
                            ],
                            Some(weights.clone()),
                            preconditioner,
                        )
                        .unwrap();
                        let oracle = solver.solve_batch(&rhs, &options).unwrap();
                        assert!(oracle.converged.iter().all(|&converged| converged));
                        let result = solve_prepared_batch(&solver, &rhs, &options, LsmrBatchPolicy::Default).unwrap();
                        assert_eq!(result.x, oracle.x);
                        assert_eq!(result.demeaned, oracle.demeaned);
                        assert_eq!(result.unidentified, oracle.unidentified);
                        assert_eq!(result.converged, oracle.converged);
                        assert_eq!(result.iterations, oracle.iterations);
                        assert_eq!(result.residual, oracle.residual);
                        assert_eq!((result.n_dofs, result.n_obs), (oracle.n_dofs, oracle.n_obs));
                    }
                });
        }
    }

    #[test]
    fn retained_serial_ols_windows_match_full_batches_and_fail_atomically() {
        let rows = 512;
        let left: Vec<u32> = (0..rows).map(|r| (r % 32) as u32).collect();
        let right: Vec<u32> = (0..rows).map(|r| ((r / 7) % 19) as u32).collect();
        let weights: Vec<f64> = (0..rows).map(|r| 0.5 + (r % 13) as f64 / 13.0).collect();
        let columns: Vec<Vec<f64>> = (0..64).map(|c| (0..rows)
            .map(|r| ((r * 17 + c * 31) as f64 * 0.0031).sin()).collect()).collect();
        let views: Vec<&[f64]> = columns.iter().map(Vec::as_slice).collect();
        let options = LsmrOptions { tol: 1e-10, maxiter: 500, local_size: Some(4) };
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                for weighted in [false, true] {
                    for preconditioner in [PreconditionerConfig::Off, PreconditionerConfig::Diagonal,
                        PreconditionerConfig::default()]
                    {
                        let effects = || vec![Effect::new(&left, true, []).unwrap(),
                            Effect::new(&right, true, []).unwrap()];
                        let w = weighted.then_some(weights.as_slice());
                        let solver = Solver::new(effects(), w.map(<[f64]>::to_vec), preconditioner.clone()).unwrap();
                        for width in [16, 52, 64] {
                            let rhs = &views[..width];
                            let expected = solver.solve_batch(rhs, &options).unwrap();
                            assert!(expected.converged.iter().all(|&v| v));
                            for actual in [
                                solve_prepared_batch(&solver, rhs, &options, LsmrBatchPolicy::RetainedSerialOls).unwrap(),
                                solve_batch_with_policy(effects(), rhs, w, &options, preconditioner.clone(),
                                    LsmrBatchPolicy::RetainedSerialOls).unwrap(),
                            ] {
                                assert_eq!(actual.x, expected.x);
                                assert_eq!(actual.demeaned, expected.demeaned);
                                assert_eq!(actual.unidentified, expected.unidentified);
                                assert_eq!(actual.converged, expected.converged);
                                assert_eq!(actual.iterations, expected.iterations);
                                assert_eq!(actual.residual, expected.residual);
                                assert_eq!((actual.n_dofs, actual.n_obs), (expected.n_dofs, expected.n_obs));
                            }
                        }
                        if threads == 1 {
                            let mut malformed = views[..16].to_vec();
                            malformed[15] = &columns[15][..rows - 1];
                            for attempt in [
                                solve_prepared_batch(&solver, &malformed, &options, LsmrBatchPolicy::RetainedSerialOls),
                                solve_batch_with_policy(effects(), &malformed, w, &options, preconditioner.clone(),
                                    LsmrBatchPolicy::RetainedSerialOls),
                            ] {
                                assert!(matches!(attempt, Err(BatchError::Upstream(_))));
                            }
                        }
                    }
                }
            });
        }
    }

    #[test]
    fn a_later_window_failure_returns_no_partial_batch() {
        let levels = vec![0_u32, 0, 1, 1];
        let solver = Solver::new(
            vec![Effect::new(&levels, true, []).unwrap()],
            None,
            PreconditionerConfig::Off,
        )
        .unwrap();
        let rhs = [1.0, 2.0, 3.0, 4.0];
        let mut batch: Vec<&[f64]> = vec![&rhs; 65];
        batch[64] = &rhs[..3];
        let error = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap()
            .install(|| solve_prepared_batch(&solver, &batch, &LsmrOptions::default(), LsmrBatchPolicy::Default))
            .unwrap_err();
        assert!(matches!(
            &error,
            BatchError::Upstream(WithinError::Solve(_))
        ));
        assert!(!error.preconditioner_breakdown());
        assert_eq!(error.into_fereg("test failure").code, ErrorCode::Internal);
    }

    #[test]
    fn transport_capacity_errors_keep_allocation_classification() {
        let mut data = vec![1.0];
        let error = reserve(&mut data, usize::MAX).unwrap_err();
        assert_eq!(data, [1.0]);
        assert_eq!(
            error.into_fereg("test reservation").code,
            ErrorCode::Allocation
        );
        assert_eq!(
            cells(usize::MAX, 2)
                .unwrap_err()
                .into_fereg("test shape")
                .code,
            ErrorCode::Allocation
        );
    }
}
