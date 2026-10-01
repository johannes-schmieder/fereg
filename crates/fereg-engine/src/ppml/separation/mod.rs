//! Immutable detector views and atomic sample generations (SPEC §14).
mod cascade;
mod coordinator;
mod exact;
mod ir;
mod ledger;
mod methods;
mod mu;
mod numerical;
mod simplex;
mod verify;
pub use coordinator::{
    run, run_with, Event, EventKind, FinalSample, MethodSummary, Report, RunControls, TRACE_LIMIT,
};
pub use exact::{ExactCertificate, ExactLevel, ExactProposal, Status};
pub use ir::{IrFixedPoint, IrReport, IrTerminal};
pub use ledger::{Commit, Ledger, Policy, View};
pub use methods::{Method, Methods};
pub use mu::{MuReport, MuTerminal, DEFAULT_MU_TOLERANCE};
pub use numerical::{
    Controls, DirectionCertificate, DirectionProposal, Model, NumericalCommit, SpanValidation,
};
pub use simplex::{
    simplex_kernel_cells, SimplexCapacity, SimplexDualCertificate, SimplexEvidence, SimplexLimits,
    SimplexReport, SimplexTerminal,
};

use crate::error::{ErrorCode, FeregError};
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

pub(crate) use coordinator::run_polled;

#[cfg(test)]
mod generation_tests;
#[cfg(test)]
mod numerical_interruption_tests;
