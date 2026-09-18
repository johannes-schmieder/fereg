//! Scalar metadata admission. These charges supplement the numerical banks;
//! raw request/input owners belong to the embedding-resident charge.

use std::mem::size_of;

use super::{PpmlClusterDimensionResult, PpmlCoefficientStatus, PpmlFixedEffectResult};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{BoundCategoricalAbsorbEffect, CategoricalAbsorbEffectIdentity};
use crate::metadata::{
    AbsorbedTerm, ExpandedTerm, RawTermKind, SourceEntry, SourceId, TermComponent,
};
use crate::ols::{
    OlsClusterTermV1, OlsInputBinding, OlsRequestV1, PreparedOlsCategoricalComponent,
    PreparedOlsClusterDimension, PreparedOlsCoefficient, PreparedOlsColumn,
    PreparedOlsFittedCoefficient,
};
use crate::topology_dof::{CategoricalTopologyDofEffectSpec, CategoricalTopologyDofSlopeSpec};

/// Frozen generation-zero metadata capacities, independent of observation N.
/// Rebased generations retain the original reporting-axis reservations.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PpmlMetadataMemory {
    /// Capacity sum of the transferred raw request and input owner. The
    /// embedding observation may already include it; only a shortfall is added.
    pub input_owner_bytes: u128,
    pub prepared_nested_bytes: u128,
    pub preparation_transient_bytes: u128,
    pub result_bytes: u128,
    /// Retained categorical/topology descriptors outside the model metadata.
    pub generation_fixed_bytes: u128,
    /// Codes, level identities and topology row maps per retained observation.
    /// Membership offsets are charged separately at the admitted FE level axis.
    pub generation_bytes_per_row: u128,
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
            availability_request_metadata_bytes(request)?,
        ])?;
        let (generation_fixed_bytes, generation_bytes_per_row) =
            generation_storage(request, clusters)?;
        Ok(Self {
            input_owner_bytes: 0,
            generation_fixed_bytes,
            generation_bytes_per_row,
            prepared_nested_bytes: prepared,
            preparation_transient_bytes: transient,
            result_bytes: result,
        })
    }
}

/// Complete retained categorical storage, including finite-only and cluster-only
/// sources. All source-set counting is metadata-sized and allocation-free.
fn generation_storage(
    request: &OlsRequestV1,
    clusters: &[OlsClusterTermV1],
) -> Result<(u128, u128)> {
    use crate::fe::{CategoricalTopologySource, CompactedCategoricalSource};
    use crate::metadata::ComponentKind;
    use crate::prepared_categorical_data::PreparedCategoricalModelSource;
    let mut categories = 0;
    let mut compacted = 0;
    let mut ordinary = 0;
    let mut row_bytes = size_of::<usize>() as u128; // topology retained-row map
    for source in &request.model.sources {
        let absorbed = request.absorbed_terms.iter().any(|effect| {
            effect.categorical_sources.contains(&source.id)
        });
        let modeled = request.model.terms.iter().any(|term| {
            term.components.iter().any(|component| {
                component.source == source.id && component.kind == ComponentKind::Categorical
            })
        }) || clusters.iter().any(|cluster| cluster.sources.contains(&source.id));
        if absorbed || modeled { categories += 1; }
        if absorbed {
            compacted += 1;
            // Compact codes and original-level identities; each has <= N entries.
            add(&mut row_bytes, 2 * size_of::<u32>() as u128)?;
        }
        if modeled {
            ordinary += 1;
            // A topology-backed source owns only its candidate f64 level table.
            // An ordinary source owns an additional compact u32 code vector.
            add(&mut row_bytes, size_of::<f64>() as u128
                + if absorbed { 0 } else { size_of::<u32>() as u128 })?;
        }
    }
    let effects = request.absorbed_terms.len();
    // Registry growth uses try_reserve(1): geometric capacity < 2*len, with
    // Rust's four-element minimum. No registry allocation for zero effects.
    let registry_capacity = if effects == 0 { 0 } else {
        effects.checked_mul(2).ok_or_else(overflow)?.max(4)
    };
    let mut fixed = sum(&[
        bytes::<CompactedCategoricalSource>(compacted)?,
        bytes::<PreparedCategoricalModelSource>(ordinary)?,
        bytes::<Option<usize>>(categories)?,
        bytes::<Vec<usize>>(effects)?,
        bytes::<usize>(effects)?, // effect -> topology
        bytes::<CategoricalTopologySource>(registry_capacity)?,
        bytes::<usize>(effects)?, // terminal membership offsets
    ])?;
    for effect in &request.absorbed_terms {
        add(&mut fixed, bytes::<usize>(effect.categorical_sources.len())?)?;
        add(&mut row_bytes, size_of::<usize>() as u128)?; // memberships
        if effect.categorical_sources.len() > 1 {
            // Canonical partition key and one interaction code per observation.
            add(&mut fixed, bytes::<usize>(effect.categorical_sources.len())?)?;
            add(&mut row_bytes, size_of::<u32>() as u128)?;
        }
    }
    for cluster in clusters {
        if cluster.sources.len() > 1 {
            add(&mut row_bytes, size_of::<u32>() as u128)?;
        }
    }
    Ok((fixed, row_bytes))
}

/// `plan_categorical_absorbed_request` keeps a fallible model copy, synthetic
/// availability terms and requested identities alive while bound effects and
/// the final compact model are built. Source count bounds the deduplicated
/// synthetic terms without allocating a marker solely for reporting.
fn availability_request_metadata_bytes(request: &OlsRequestV1) -> Result<u128> {
    if request.absorbed_terms.is_empty() {
        return Ok(0);
    }
    let sources = request.model.sources.len();
    let mut retained = sum(&[
        bytes::<SourceEntry>(sources)?,
        bytes::<ExpandedTerm>(request.model.terms.len())?,
        bytes::<OlsInputBinding>(request.bindings.len())?,
        bytes::<AbsorbedTerm>(sources)?,
        bytes::<SourceId>(sources)?, // required-source registry
        bytes::<SourceId>(sources)?, // one source per synthetic term
        bytes::<CategoricalAbsorbEffectIdentity>(request.absorbed_terms.len())?,
        bytes::<BoundCategoricalAbsorbEffect>(request.absorbed_terms.len())?,
    ])?;
    for source in &request.model.sources {
        // Model copy and synthetic availability label have distinct owners.
        add(
            &mut retained,
            sum(&[
                source.canonical_name.len() as u128,
                source.canonical_name.len() as u128,
            ])?,
        )?;
    }
    for term in &request.model.terms {
        add(&mut retained, term.canonical_token.len() as u128)?;
        add(
            &mut retained,
            bytes::<TermComponent>(term.components.len())?,
        )?;
        for component in &term.components {
            add(
                &mut retained,
                sum(&[
                    component.raw_operator.len() as u128,
                    component.raw_time_series_operator.len() as u128,
                ])?,
            )?;
        }
    }
    for effect in &request.absorbed_terms {
        let identity = sum(&[
            effect.label.len() as u128,
            bytes::<SourceId>(effect.categorical_sources.len())?,
            bytes::<SourceId>(effect.slope_sources.len())?,
        ])?;
        add(
            &mut retained,
            sum(&[
                identity,
                identity,
                bytes::<usize>(effect.categorical_sources.len())?,
            ])?,
        )?;
    }
    // Validation, construction markers and source binding are sequential.
    let validation = sum(&[bytes::<u32>(request.bindings.len())?, bytes::<u8>(sources)?])?;
    let scratch = validation
        .max(bytes::<bool>(sources)?)
        .max(bytes::<Option<usize>>(sources)?);
    sum(&[retained, scratch])
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
