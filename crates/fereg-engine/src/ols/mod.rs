//! OLS orchestration over the shared data, projection and finite-fit components.
mod prepare;
pub use prepare::prepare;
mod report;
pub use report::{report, Reported};
mod call;
mod fit;
pub use crate::result::{Diagnostics, FitData, OlsFit, Sample, Timings, WeightSummary};
pub use call::{Call, Phase, Request, Resources};
pub use fit::{fit, fit_requested};
