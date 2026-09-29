//! Data-independent model specification; no SPI positions or borrowed Stata memory.
pub mod metadata;
pub mod options;
pub mod protocol;

use crate::error::{ErrorCode, FeregError, Result};
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
        let decoded = wire.decode_specification()?;
        let specification = Self {
            model: decoded.model,
            absorbed_terms: decoded.absorbed_terms,
            clusters: Vec::new(),
            include_intercept: true,
            keep_singletons: false,
        };
        specification.validate()?;
        Ok(specification)
    }

    pub fn validate(&self) -> Result<()> {
        self.model.validate_executable_metadata()?;
        metadata::validate_absorbed_terms(&self.absorbed_terms, self.model.sources.len())?;
        if self.clusters.len() > 10 {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "at most ten cluster dimensions are supported",
            ));
        }
        for term in &self.clusters {
            if term.label.trim().is_empty()
                || term.sources.is_empty()
                || term
                    .sources
                    .iter()
                    .any(|source| source.get() as usize > self.model.sources.len())
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidArgument,
                    "invalid cluster source graph",
                ));
            }
            if term
                .sources
                .iter()
                .enumerate()
                .any(|(index, source)| term.sources[..index].contains(source))
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidArgument,
                    "cluster interaction repeats a source",
                ));
            }
        }
        Ok(())
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
