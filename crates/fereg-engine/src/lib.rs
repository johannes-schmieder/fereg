//! Shared safe numerical engine for OLS and PPML.
#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]

pub mod error;
pub mod spec;
pub mod threads;

pub use error::{ErrorCode, FeregError, Result};
pub use spec::metadata::*;
pub use spec::ModelSpec;
