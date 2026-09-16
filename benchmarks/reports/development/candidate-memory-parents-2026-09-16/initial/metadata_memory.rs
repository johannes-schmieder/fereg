//! Scalar metadata admission. These charges supplement the numerical banks;
//! raw request/input owners belong to the embedding-resident charge.

use std::mem::size_of;

use super::{PpmlClusterDimensionResult, PpmlCoefficientStatus, PpmlFixedEffectResult};
use crate::error::{ErrorCode, FeregError, Result};
use crate::metadata::{RawTermKind, SourceId};
use crate::ols::{
    OlsClusterTermV1, OlsRequestV1, PreparedOlsCategoricalComponent, PreparedOlsClusterDimension,
    PreparedOlsCoefficient, PreparedOlsColumn, PreparedOlsFittedCoefficient,
};
use crate::topology_dof::{CategoricalTopologyDofEffectSpec, CategoricalTopologyDofSlopeSpec};

/// Frozen generation-zero metadata capacities, independent of observation N.
/// Rebased generations retain the original reporting-axis reservations.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PpmlMetadataMemory {
    pub prepared_nested_bytes: u128,
    pub preparation_transient_bytes: u128,
    pub result_bytes: u128,
}

impl PpmlMetadataMemory {
    pub(crate) fn from_request(
        request: &OlsRequestV1,
        clusters: &[OlsClusterTermV1],
    ) -> Result<Self> {
        let mut recipes = 0_u128;
        for term in &request.model.terms {
            if term.raw_kind == RawTermKind::Interaction {
                add(
                    &mut recipes,
                    bytes::<PreparedOlsCategoricalComponent>(term.components.len())?,
                )?;
            }
        }
        let mut prepared = recipes;
        add(
            &mut prepared,
            bytes::<CategoricalTopologyDofEffectSpec>(request.absorbed_terms.len())?,
        )?;
        add(
            &mut prepared,
            bytes::<PreparedOlsClusterDimension>(clusters.len())?,
        )?;
        let mut result = bytes::<PpmlClusterDimensionResult>(clusters.len())?;
        for effect in &request.absorbed_terms {
            let source_bytes = bytes::<SourceId>(effect.categorical_sources.len())?;
            add(&mut prepared, source_bytes)?;
            add(
                &mut prepared,
                bytes::<CategoricalTopologyDofSlopeSpec>(effect.slope_sources.len())?,
            )?;
            let mut label = effect.categorical_sources.len().saturating_sub(1) as u128;
            for id in &effect.categorical_sources {
                add(&mut label, source_name_bytes(request, *id)?)?;
            }
            add(&mut prepared, label)?;
            for id in &effect.slope_sources {
                add(
                    &mut prepared,
                    sum(&[label, 3, source_name_bytes(request, *id)?])?,
                )?;
            }
            // Every declared channel has a separate identity tuple, even
            // when a partial save retains just one observation vector.
            let channels = effect
                .slope_sources
                .len()
                .checked_add(usize::from(effect.intercept))
                .ok_or_else(overflow)?;
            add(
                &mut result,
                (channels as u128)
                    .checked_mul(sum(&[
                        size_of::<PpmlFixedEffectResult>() as u128,
                        source_bytes,
                        size_of::<&[f64]>() as u128, // plugin saved-channel views
                        size_of::<bool>() as u128,   // requested-channel registry
                        size_of::<Vec<f64>>() as u128,
                        size_of::<usize>() as u128,
                    ])?)
                    .ok_or_else(overflow)?,
            )?;
        }
        for cluster in clusters {
            let nested = sum(&[
                cluster.label.len() as u128,
                bytes::<SourceId>(cluster.sources.len())?,
            ])?;
            add(&mut prepared, nested)?;
            add(&mut result, nested)?;
        }
        let slots = request
            .model
            .terms
            .len()
            .checked_add(usize::from(request.include_intercept))
            .ok_or_else(overflow)?;
        // The accepted IRLS coefficient records, the final public axis, and
        // its temporary identified-slot validator coexist with the fit.
        add(
            &mut result,
            (slots as u128)
                .checked_mul(sum(&[
                    size_of::<PreparedOlsFittedCoefficient>() as u128,
                    size_of::<f64>() as u128,
                    size_of::<PpmlCoefficientStatus>() as u128,
                    size_of::<Option<usize>>() as u128,
                    size_of::<usize>() as u128,
                    size_of::<bool>() as u128,
                ])?)
                .ok_or_else(overflow)?,
        )?;
        // The candidate model remains alive while the compact model and its
        // nested interaction recipes are rebuilt. Their main descriptors
        // are otherwise counted once in prepared_model_metadata_bytes.
        let transient = sum(&[
            recipes,
            bytes::<PreparedOlsCoefficient>(slots)?,
            bytes::<PreparedOlsColumn>(slots)?,
        ])?;
        Ok(Self {
            prepared_nested_bytes: prepared,
            preparation_transient_bytes: transient,
            result_bytes: result,
        })
    }
}

fn source_name_bytes(request: &OlsRequestV1, id: SourceId) -> Result<u128> {
    request
        .model
        .sources
        .iter()
        .find(|source| source.id == id)
        .map(|source| source.canonical_name.len() as u128)
        .ok_or_else(|| {
            FeregError::new(ErrorCode::InvalidArgument, "PPML metadata source is absent")
        })
}

fn bytes<T>(count: usize) -> Result<u128> {
    (count as u128)
        .checked_mul(size_of::<T>() as u128)
        .ok_or_else(overflow)
}

fn add(total: &mut u128, value: u128) -> Result<()> {
    *total = total.checked_add(value).ok_or_else(overflow)?;
    Ok(())
}

fn sum(values: &[u128]) -> Result<u128> {
    values.iter().try_fold(0_u128, |total, value| {
        total.checked_add(*value).ok_or_else(overflow)
    })
}

fn overflow() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "PPML nested metadata memory overflows",
    )
}
