//! Immutable detector views and atomic sample generations (SPEC §14).
mod cascade;
mod exact;
mod ir;
mod ledger;
mod methods;
mod numerical;
mod verify;
pub use exact::{ExactCertificate, ExactLevel, ExactProposal, Status};
pub use ir::{IrFixedPoint, IrReport, IrTerminal};
pub use ledger::{Commit, Ledger, Policy, View};
pub use methods::{Method, Methods};
pub use numerical::{
    Controls, DirectionCertificate, DirectionProposal, Model, NumericalCommit, SpanValidation,
};

use crate::error::{ErrorCode, FeregError};
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
