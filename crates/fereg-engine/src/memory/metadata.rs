//! Existing request metadata stays alive while its compiled views are used.
use super::checked::{array, sum};
use crate::{error::Result, execution, ModelSpec};

pub(crate) fn model_bytes(
    model: &ModelSpec,
    poll: &mut impl FnMut() -> Result<()>,
) -> Result<u128> {
    poll()?;
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
    for (index, source) in model.model.sources.iter().enumerate() {
        execution::check_chunk_with(index, poll)?;
        bytes = sum(&[bytes, source.canonical_name.capacity() as u128])?;
    }
    for (index, term) in model.model.terms.iter().enumerate() {
        execution::check_chunk_with(index, poll)?;
        bytes = sum(&[
            bytes,
            term.canonical_token.capacity() as u128,
            array(
                term.components.capacity(),
                size_of::<crate::TermComponent>(),
            )?,
        ])?;
        for (position, component) in term.components.iter().enumerate() {
            execution::check_chunk_with(position, poll)?;
            bytes = sum(&[
                bytes,
                component.raw_operator.capacity() as u128,
                component.raw_time_series_operator.capacity() as u128,
            ])?;
        }
    }
    for (index, effect) in model.absorbed_terms.iter().enumerate() {
        execution::check_chunk_with(index, poll)?;
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
    for (index, cluster) in model.clusters.iter().enumerate() {
        execution::check_chunk_with(index, poll)?;
        bytes = sum(&[
            bytes,
            cluster.label.capacity() as u128,
            array(cluster.sources.capacity(), size_of::<crate::SourceId>())?,
        ])?;
    }
    poll()?;
    Ok(bytes)
}

impl super::Inputs {
    pub(crate) fn with_model_polled(
        mut self,
        model: &ModelSpec,
        requested_bytes: u128,
        poll: &mut impl FnMut() -> Result<()>,
    ) -> Result<Self> {
        let metadata = model_bytes(model, poll)?;
        self.parent_bytes = sum(&[self.parent_bytes, metadata, requested_bytes])?;
        self.metadata_bytes = self.metadata_bytes.max(super::checked::mul(metadata, 4)?);
        Ok(self)
    }
}
