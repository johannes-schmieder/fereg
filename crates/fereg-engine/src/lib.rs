//! Shared safe numerical engine for OLS and PPML.
#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]

pub mod data;
pub mod dof;
pub mod error;
pub mod factor;
pub mod fe;
mod linalg;
pub mod ols;
pub mod outputs;
pub mod spec;
pub mod statistics;
pub mod threads;
pub mod topology;
pub mod vce;
pub mod wls;

pub use error::{ErrorCode, FeregError, Result};
pub use spec::metadata::*;
pub use spec::ModelSpec;
