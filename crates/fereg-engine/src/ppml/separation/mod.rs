//! Immutable detector views and atomic sample generations (SPEC §14).
mod cascade;
mod exact;
mod ledger;
mod verify;
pub use exact::{ExactCertificate, ExactLevel, ExactProposal, ExactStatus};
pub use ledger::{Commit, Ledger, Policy, View};

use crate::error::{ErrorCode, FeregError};
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
