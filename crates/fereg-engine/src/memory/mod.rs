//! One request-wide storage estimate; no numerical or per-backend admission.
mod checked;
mod layout;
mod loaded;
mod phases;
mod solver;
use crate::{
    error::Result,
    ppml::bias,
    spec::{Estimator, Solver},
};
pub use layout::Layout;
pub use phases::{Estimate, Phase};

/// Bounds supplied before loading. Active rows and FE levels may be upper
/// bounds; original-axis outputs always use the complete original row count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inputs {
    pub original_rows: usize,
    pub active_rows: usize,
    pub sources: usize,
    pub string_sources: usize,
    /// Payload upper bounds; neither establishes unavoidable storage.
    pub string_bytes: u128,
    pub metadata_bytes: u128,
    pub weighted: bool,
    pub adjustment: bool,
    /// Existing call-owned storage outside the declared input banks.
    pub parent_bytes: u128,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effect {
    pub levels: usize,
    pub channels: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    pub inputs: Inputs,
    pub effects: &'a [Effect],
    pub finite_columns: usize,
    pub coefficient_slots: usize,
    pub estimator: Estimator,
    pub solver: Solver,
    pub clusters: usize,
    /// Ordinary requested banks, excluding the optional eighteen bias banks.
    pub observation_outputs: usize,
    /// Exact cells in native result/shadow banks that coexist with outputs.
    pub publication_matrix_cells: usize,
    pub bias: Option<bias::Request>,
    /// The separately specified simplex resource cap, zero when not requested.
    pub simplex_cells: usize,
}
#[derive(Debug)]
pub struct Plan {
    estimate: Estimate,
}
/// Only a successful whole-call decision constructs this token. It contains
/// no route, solver override, shrinking allowance or nested admission API.
#[derive(Debug)]
pub struct Admitted {
    estimate: Estimate,
}
impl Plan {
    /// Model the loaded owner as live through validation/publication. Callers
    /// include worker/host owners outside these banks in `parent_bytes`.
    pub fn new(request: Request<'_>) -> Result<Self> {
        Ok(Self {
            estimate: phases::estimate(request)?,
        })
    }
    #[must_use]
    pub const fn estimate(&self) -> &Estimate {
        &self.estimate
    }
    /// `usable_bytes` is the allowance for all call-owned storage, including
    /// any already loaded parent/input owners, not remaining free bytes alone.
    /// Only unavoidable storage proves insufficient capacity. Sparse fill and
    /// dependency construction estimates do not prove that a supported solver
    /// cannot fit. Unknown/uncertain capacity relies on fallible allocation.
    pub fn admit(self, usable_bytes: Option<u128>) -> Result<Admitted> {
        if usable_bytes.is_some_and(|bytes| self.estimate.minimum_bytes() > bytes) {
            return Err(crate::FeregError::new(crate::ErrorCode::Allocation,
                format!("whole-call required storage {} bytes exceeds usable capacity {} bytes (estimated peak {} bytes)",
                    self.estimate.minimum_bytes(), usable_bytes.unwrap(), self.estimate.peak_bytes())));
        }
        Ok(Admitted {
            estimate: self.estimate,
        })
    }
}
impl Admitted {
    #[must_use]
    pub const fn estimate(&self) -> &Estimate {
        &self.estimate
    }
}
