use super::{Attempt, Backend, RhsDiagnostics, StopReason};
use crate::error::{reserved, Result};

/// Auxiliary setup RHSs have independent budgets; their work is never charged
/// to every model RHS or counted again when this frame's factors are reused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupDiagnostics {
    pub backend: Backend,
    pub right_hand_sides: Vec<Attempt>,
    pub stop: StopReason,
}
pub(super) fn copy_rhs(source: &[RhsDiagnostics]) -> Result<Vec<RhsDiagnostics>> {
    let mut result = reserved(source.len())?;
    for row in source {
        let mut attempts = reserved(row.attempts.len())?;
        attempts.extend_from_slice(&row.attempts);
        result.push(RhsDiagnostics {
            attempts,
            requested: row.requested,
            selected: row.selected,
            used: row.used,
            checkpoint_backend: row.checkpoint_backend,
            iterations: row.iterations,
            stop: row.stop,
            fallback: row.fallback,
        });
    }
    Ok(result)
}
pub(super) fn copy_setup(source: &[SetupDiagnostics]) -> Result<Vec<SetupDiagnostics>> {
    let mut result = reserved(source.len())?;
    for setup in source {
        let mut right_hand_sides = reserved(setup.right_hand_sides.len())?;
        right_hand_sides.extend_from_slice(&setup.right_hand_sides);
        result.push(SetupDiagnostics {
            backend: setup.backend,
            right_hand_sides,
            stop: setup.stop,
        });
    }
    Ok(result)
}
