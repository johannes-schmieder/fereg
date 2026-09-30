//! Immutable detector views and atomic sample generations (SPEC §14).
mod cascade;
mod exact;
mod ir;
mod ledger;
mod methods;
mod mu;
mod numerical;
mod simplex;
mod verify;
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
