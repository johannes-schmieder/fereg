//! Prerequisites for optional coefficient mean correction on a sealed fit.
//! Support is a necessary screen; it does not authorize a correction calculation.
mod anchor;
mod certificate;
mod information;
pub mod philox;
mod support;
mod units;
use crate::error::{reserved, ErrorCode, FeregError, Result};
pub use information::{with_information, Diagnostics, Information};
pub use support::{screen, Summary, Support};

fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>> {
    let mut values = reserved(n)?;
    values.resize(n, value);
    Ok(values)
}
fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
