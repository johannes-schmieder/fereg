//! Cooperative cancellation of long native work.
//!
//! An embedding that owns the host's interrupt signal (the Stata plugin polls
//! Break on Stata's main thread) calls [`request_cancellation`] while native
//! work runs on worker threads. Long loops call [`checkpoint`] at iteration
//! boundaries, before they start the next unit of work, and return
//! [`ErrorCode::Interrupted`]; the failed fit is then dropped atomically like
//! any other error. A checkpoint is one relaxed atomic load and never changes
//! arithmetic, allocation or iteration order.
//!
//! Stata runs one native action at a time, so one process-wide flag suffices.
//! Embeddings clear it with [`reset_cancellation`] before and after each
//! action. Tests that request cancellation run in an isolated process, since
//! concurrently running in-process tests would observe the flag.

use crate::error::{ErrorCode, FeregError, Result};
use std::sync::atomic::{AtomicBool, Ordering};

static CANCELLATION_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Ask every subsequent [`checkpoint`] to fail with `Interrupted`.
pub fn request_cancellation() {
    CANCELLATION_REQUESTED.store(true, Ordering::Release);
}

/// Clear a cancellation request at an action boundary.
pub fn reset_cancellation() {
    CANCELLATION_REQUESTED.store(false, Ordering::Release);
}

/// Whether cancellation has been requested for the current action.
#[must_use]
pub fn cancellation_requested() -> bool {
    CANCELLATION_REQUESTED.load(Ordering::Relaxed)
}

/// Fail with `Interrupted` once cancellation has been requested.
#[inline]
pub fn checkpoint() -> Result<()> {
    if cancellation_requested() {
        Err(FeregError::new(
            ErrorCode::Interrupted,
            "native estimation was interrupted at a cancellation checkpoint",
        ))
    } else {
        Ok(())
    }
}

/// Rows between checkpoints in long serial row passes. A power of two keeps
/// the stride test to one mask.
pub(crate) const CHECKPOINT_ROW_STRIDE: usize = 1024;

/// Checkpoint at the first row of every [`CHECKPOINT_ROW_STRIDE`] rows of a
/// serial pass whose rows each cost O(K) or more.
#[inline]
pub(crate) fn row_checkpoint(row: usize) -> Result<()> {
    if row % CHECKPOINT_ROW_STRIDE == 0 {
        checkpoint()
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DenseMatrix;
    use crate::fe::{
        absorb_fixed_effects, absorb_map, AbsorbConfig, EffectTerm, FeDesign, FeSolverKind,
        MapConfig,
    };

    fn two_way() -> (FeDesign, Vec<f64>, DenseMatrix) {
        let rows = 64;
        let first = EffectTerm::new(
            (0..rows).map(|row| (row % 8) as u32).collect(),
            8,
            true,
            vec![],
        )
        .expect("first effect");
        let second = EffectTerm::new(
            (0..rows).map(|row| (row % 5) as u32).collect(),
            5,
            true,
            vec![],
        )
        .expect("second effect");
        let design = FeDesign::new(rows, vec![first, second]).expect("design");
        let values =
            DenseMatrix::from_columns(&[(0..rows).map(|row| (row as f64).sin()).collect()])
                .expect("values");
        (design, vec![1.0; rows], values)
    }

    /// The flag is process-wide, so this runs in a child process.
    #[test]
    fn requested_cancellation_stops_solver_entry_points() {
        const CHILD_ENV: &str = "FEREG_CORE_CANCELLATION_TEST_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .env(CHILD_ENV, "1")
                .args([
                    "interrupt::tests::requested_cancellation_stops_solver_entry_points",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .expect("launch isolated cancellation test");
            assert!(status.success(), "isolated cancellation test failed");
            return;
        }
        let (design, weights, values) = two_way();
        let mut unaffected = values.clone();
        absorb_map(&design, &weights, &mut unaffected, MapConfig::default())
            .expect("no request leaves solvers untouched");

        request_cancellation();
        assert!(checkpoint().is_err());
        assert!(row_checkpoint(0).is_err());
        assert!(row_checkpoint(1).is_ok());
        assert!(row_checkpoint(CHECKPOINT_ROW_STRIDE - 1).is_ok());
        assert!(row_checkpoint(CHECKPOINT_ROW_STRIDE).is_err());
        let mut map_values = values.clone();
        let error = absorb_map(&design, &weights, &mut map_values, MapConfig::default())
            .expect_err("MAP stops at its first sweep");
        assert_eq!(error.code, ErrorCode::Interrupted);
        for solver in [FeSolverKind::Cg, FeSolverKind::Lsmr] {
            let mut solver_values = values.clone();
            let config = AbsorbConfig {
                solver,
                ..AbsorbConfig::default()
            };
            let error = absorb_fixed_effects(&design, &weights, &mut solver_values, config)
                .expect_err("solver stops at a checkpoint");
            assert_eq!(error.code, ErrorCode::Interrupted, "{solver:?}");
        }
        reset_cancellation();
        assert!(checkpoint().is_ok());
        let mut resumed = values;
        absorb_map(&design, &weights, &mut resumed, MapConfig::default())
            .expect("a reset flag lets the next action run");
        assert_eq!(resumed.as_slice(), unaffected.as_slice());
    }
}
