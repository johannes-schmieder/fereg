//! Existing request metadata stays alive while its compiled views are used.
use super::checked::{array, sum};
use crate::{error::Result, ModelSpec};

pub(crate) fn model_bytes(model: &ModelSpec) -> Result<u128> {
    let mut bytes = sum(&[
        array(
            model.model.sources.capacity(),
            size_of::<crate::SourceEntry>(),
        )?,
        array(
            model.model.terms.capacity(),
            size_of::<crate::ExpandedTerm>(),
        )?,
        array(
            model.absorbed_terms.capacity(),
            size_of::<crate::AbsorbedTerm>(),
        )?,
        array(
            model.clusters.capacity(),
            size_of::<crate::spec::ClusterTerm>(),
        )?,
    ])?;
    for source in &model.model.sources {
        bytes = sum(&[bytes, source.canonical_name.capacity() as u128])?;
    }
    for term in &model.model.terms {
        bytes = sum(&[
            bytes,
            term.canonical_token.capacity() as u128,
            array(
                term.components.capacity(),
                size_of::<crate::TermComponent>(),
            )?,
        ])?;
        for component in &term.components {
            bytes = sum(&[
                bytes,
                component.raw_operator.capacity() as u128,
                component.raw_time_series_operator.capacity() as u128,
            ])?;
        }
    }
    for effect in &model.absorbed_terms {
        bytes = sum(&[
            bytes,
            effect.label.capacity() as u128,
            effect.saved_name.capacity() as u128,
            array(
                effect.categorical_sources.capacity(),
                size_of::<crate::SourceId>(),
            )?,
            array(
                effect.slope_sources.capacity(),
                size_of::<crate::SourceId>(),
            )?,
        ])?;
    }
    for cluster in &model.clusters {
        bytes = sum(&[
            bytes,
            cluster.label.capacity() as u128,
            array(cluster.sources.capacity(), size_of::<crate::SourceId>())?,
        ])?;
    }
    Ok(bytes)
}

impl super::Inputs {
    pub(crate) fn with_model(mut self, model: &ModelSpec, requested_bytes: u128) -> Result<Self> {
        let metadata = model_bytes(model)?;
        self.parent_bytes = sum(&[self.parent_bytes, metadata, requested_bytes])?;
        self.metadata_bytes = self.metadata_bytes.max(super::checked::mul(metadata, 4)?);
        Ok(self)
    }
}
