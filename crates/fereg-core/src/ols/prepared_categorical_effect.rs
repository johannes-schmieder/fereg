//! Identity and source recipes for prepared categorical absorbed terms.

use crate::error::{ErrorCode, FeregError, Result};
use crate::metadata::{AbsorbTermId, SourceId};

/// Requested identity and numerical recipe for one categorical absorbed term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCategoricalEffectRequest {
    pub term_id: AbsorbTermId,
    pub label: String,
    /// Wire identity in requested order. This tuple is never canonicalized.
    pub sources: Vec<SourceId>,
    /// Indices into the prepared categorical-source table, also in requested
    /// order. Numerical topology construction canonicalizes a temporary copy.
    pub categorical_source_indices: Vec<usize>,
}

impl PreparedCategoricalEffectRequest {
    pub fn new(
        term_id: AbsorbTermId,
        label: String,
        sources: Vec<SourceId>,
        categorical_source_indices: Vec<usize>,
    ) -> Result<Self> {
        let request = Self {
            term_id,
            label,
            sources,
            categorical_source_indices,
        };
        request.validate(usize::MAX)?;
        Ok(request)
    }

    pub(crate) fn validate(&self, source_count: usize) -> Result<()> {
        if self.label.is_empty() {
            return Err(invalid(
                "prepared categorical absorbed effect has an empty label",
            ));
        }
        if self.sources.is_empty() {
            return Err(invalid(
                "prepared categorical absorbed effect has an empty source tuple",
            ));
        }
        if self.sources.len() != self.categorical_source_indices.len() {
            return Err(invalid(
                "prepared categorical absorbed effect source identities and indices differ in length",
            ));
        }
        if self
            .categorical_source_indices
            .iter()
            .any(|index| *index >= source_count)
        {
            return Err(invalid(
                "prepared categorical absorbed effect source index is out of range",
            ));
        }
        Ok(())
    }
}

/// One requested absorbed term after numerical topology deduplication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCategoricalEffect {
    pub term_id: AbsorbTermId,
    pub label: String,
    pub sources: Vec<SourceId>,
    pub categorical_source_indices: Vec<usize>,
    pub topology_source_index: usize,
}

impl PreparedCategoricalEffect {
    pub(crate) fn validate(&self, source_count: usize) -> Result<()> {
        if self.label.is_empty() {
            return Err(invalid(
                "prepared categorical effect metadata has an empty label",
            ));
        }
        if self.sources.is_empty()
            || self.sources.len() != self.categorical_source_indices.len()
            || self
                .categorical_source_indices
                .iter()
                .any(|index| *index >= source_count)
        {
            return Err(invalid(
                "prepared categorical effect metadata is structurally invalid",
            ));
        }
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
