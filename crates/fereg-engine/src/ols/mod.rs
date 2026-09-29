//! OLS orchestration over the shared data, projection and finite-fit components.
mod prepare;
pub use prepare::prepare;
mod report;
pub use report::{report, Reported};
