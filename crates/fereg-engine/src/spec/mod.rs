//! Data-independent model specification; no SPI positions or borrowed Stata memory.
pub mod metadata;
pub mod options;
pub mod protocol;

use crate::{
    error::{ErrorCode, FeregError, Result},
    execution,
};
use metadata::{AbsorbedTerm, CompiledTermGraph, ModelWireV2, SourceId};
pub use options::{Estimator, Options, Solver};

/// Harvested from legacy OlsClusterTermV1; interactions remain one dimension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterTerm {
    pub label: String,
    pub sources: Vec<SourceId>,
}

/// Shared request shape harvested from legacy OlsRequestV1 and OlsClusteredRequestV1.
/// Source IDs refer to the single loaded bank, independent of a host's varlist.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelSpec {
    pub model: CompiledTermGraph,
    pub absorbed_terms: Vec<AbsorbedTerm>,
    pub clusters: Vec<ClusterTerm>,
    pub include_intercept: bool,
    pub keep_singletons: bool,
}

impl ModelSpec {
    pub fn from_wire(wire: &ModelWireV2) -> Result<Self> {
        Self::from_wire_with(wire, &mut || Ok(()))
    }
    /// Native hosts decode on their caller before accessing observations.
    pub fn from_wire_with(
        wire: &ModelWireV2,
        interrupt: &mut impl FnMut() -> Result<()>,
    ) -> Result<Self> {
        let decoded = wire.decode_specification_with(interrupt)?;
        let specification = Self {
            model: decoded.model,
            absorbed_terms: decoded.absorbed_terms,
            clusters: Vec::new(),
            include_intercept: true,
            keep_singletons: false,
        };
        specification.validate_polled(interrupt)?;
        Ok(specification)
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_polled(&mut || Ok(()))
    }
    pub(crate) fn validate_polled(&self, poll: &mut impl FnMut() -> Result<()>) -> Result<()> {
        self.model.validate_executable_metadata_polled(poll)?;
        metadata::validate_absorbed_terms_polled(
            &self.absorbed_terms,
            self.model.sources.len(),
            poll,
        )?;
        if self.clusters.len() > 10 {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "at most ten cluster dimensions are supported",
            ));
        }
        for (index, term) in self.clusters.iter().enumerate() {
            execution::check_chunk_with(index, poll)?;
            if term.label.trim().is_empty()
                || term.sources.is_empty()
                || execution::any_with(
                    &term.sources,
                    |source| source.get() as usize > self.model.sources.len(),
                    poll,
                )?
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidArgument,
                    "invalid cluster source graph",
                ));
            }
            for (position, source) in term.sources.iter().enumerate() {
                execution::check_chunk_with(position, poll)?;
                if execution::any_with(&term.sources[..position], |prior| prior == source, poll)? {
                    return Err(FeregError::new(
                        ErrorCode::InvalidArgument,
                        "cluster interaction repeats a source",
                    ));
                }
            }
        }
        poll()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metadata::SourceEntry;

    #[test]
    fn cluster_sources_share_the_model_registry_and_reject_duplicates() {
        let source = SourceId::new(1).unwrap();
        let mut specification = ModelSpec {
            model: CompiledTermGraph {
                sources: vec![SourceEntry {
                    id: source,
                    canonical_name: "g".into(),
                }],
                terms: vec![],
            },
            absorbed_terms: vec![],
            clusters: vec![ClusterTerm {
                label: "g".into(),
                sources: vec![source],
            }],
            include_intercept: true,
            keep_singletons: false,
        };
        specification.validate().unwrap();
        specification.clusters[0].sources.push(source);
        assert_eq!(
            specification.validate().unwrap_err().code,
            ErrorCode::InvalidArgument
        );
        specification.clusters[0].sources = vec![SourceId::new(2).unwrap()];
        assert!(specification.validate().is_err());
    }
}

#[cfg(test)]
mod preflight_tests;
