//! Sample-bound cluster preparation for categorical-interaction OLS.
//!
//! Cluster-only numeric sources participate in reason-6 missing-value deletion
//! before the categorical singleton fixed point. Simple cluster dimensions
//! borrow their final categorical source codes without an N-row copy. A true
//! cluster interaction owns exactly one collision-free `u32` code per retained
//! observation.

use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::{factorize_code_interaction, CategoricalCodeView};
use crate::prepared_categorical_data::{
    prepare_categorical_ols_data_with_cluster_sources, PreparedCategoricalOlsData,
};

use super::prepared_clusters::{
    validate_request, PreparedOlsClusterCodes, PreparedOlsClusterDesign,
    PreparedOlsClusterDimension,
};
use super::{LoadedOlsInput, OlsClusteredRequestV1};

/// Owning categorical-interaction OLS-plus-cluster preparation result. The OLS
/// artifact and every cluster dimension share one final compact sample axis.
#[derive(Debug, PartialEq)]
pub struct PreparedCategoricalOlsClusteredData {
    ols: PreparedCategoricalOlsData,
    clusters: PreparedOlsClusterDesign,
}

impl PreparedCategoricalOlsClusteredData {
    #[must_use]
    pub const fn ols(&self) -> &PreparedCategoricalOlsData {
        &self.ols
    }

    #[must_use]
    pub const fn clusters(&self) -> &PreparedOlsClusterDesign {
        &self.clusters
    }

    /// Borrow one declared cluster dimension on the final compact sample axis.
    pub fn cluster_codes(&self, dimension: usize) -> Result<&[u32]> {
        let dimension = self
            .clusters
            .dimensions
            .get(dimension)
            .ok_or_else(|| invalid_argument("categorical cluster dimension is out of range"))?;
        let codes = match &dimension.codes {
            PreparedOlsClusterCodes::CategoricalSource {
                categorical_source_index,
            } => self
                .ols
                .categorical_sources()
                .get(*categorical_source_index)
                .ok_or_else(|| {
                    invalid_argument("categorical simple cluster source is out of range")
                })?
                .codes(self.ols.fe_topology())?,
            PreparedOlsClusterCodes::Interaction(codes) => codes,
        };
        if codes.len() != self.ols.sample().n_active() {
            return Err(invalid_argument(
                "categorical cluster codes are not aligned with the final sample",
            ));
        }
        Ok(codes)
    }
}

trait CategoricalClusterReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;

    fn reserve_string(
        &mut self,
        target: &mut String,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemCategoricalClusterReservation;

impl CategoricalClusterReservation for SystemCategoricalClusterReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }

    fn reserve_string(
        &mut self,
        target: &mut String,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }
}

/// Prepare categorical-interaction OLS and declared clusters through one
/// authoritative sample pass.
pub fn prepare_categorical_ols_clustered_data(
    request: &OlsClusteredRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<PreparedCategoricalOlsClusteredData> {
    let cluster_sources = validate_request(request)?;
    let ols = prepare_categorical_ols_data_with_cluster_sources(
        &request.ols,
        loaded,
        &cluster_sources,
    )?;
    let mut reservation = SystemCategoricalClusterReservation;
    let clusters = build_cluster_design(request, &ols, &mut reservation)?;
    let prepared = PreparedCategoricalOlsClusteredData { ols, clusters };
    certify_prepared_clusters(&prepared)?;
    Ok(prepared)
}

fn build_cluster_design<R: CategoricalClusterReservation>(
    request: &OlsClusteredRequestV1,
    ols: &PreparedCategoricalOlsData,
    reservation: &mut R,
) -> Result<PreparedOlsClusterDesign> {
    let mut dimensions = reserved_empty(
        reservation,
        request.clusters.len(),
        "unable to reserve categorical cluster dimension table",
    )?;
    for term in &request.clusters {
        let categorical_indices = resolve_source_indices(ols, &term.sources, reservation)?;
        let (codes, groups) = if let [index] = categorical_indices.as_slice() {
            let source = ols
                .categorical_sources()
                .get(*index)
                .ok_or_else(|| invalid_argument("categorical simple cluster source disappeared"))?;
            (
                PreparedOlsClusterCodes::CategoricalSource {
                    categorical_source_index: *index,
                },
                source.n_levels(ols.fe_topology())?,
            )
        } else {
            let encoded = factorize_interaction(ols, &categorical_indices, reservation)?;
            let groups = encoded.n_levels();
            (PreparedOlsClusterCodes::Interaction(encoded.into_codes()), groups)
        };
        if groups < 2 {
            return Err(invalid_data(
                "each categorical cluster dimension must contain at least two final-sample groups",
            ));
        }
        let label = fallible_string_copy(
            &term.label,
            reservation,
            "unable to reserve categorical cluster label",
        )?;
        let mut sources = reserved_empty(
            reservation,
            term.sources.len(),
            "unable to reserve categorical cluster source identities",
        )?;
        for source in &term.sources {
            push_reserved(
                &mut sources,
                *source,
                "categorical cluster source-identity capacity exhausted",
            )?;
        }
        push_reserved(
            &mut dimensions,
            PreparedOlsClusterDimension {
                label,
                sources,
                codes,
                groups,
            },
            "categorical cluster dimension capacity exhausted",
        )?;
    }
    Ok(PreparedOlsClusterDesign { dimensions })
}

fn resolve_source_indices<R: CategoricalClusterReservation>(
    ols: &PreparedCategoricalOlsData,
    sources: &[crate::metadata::SourceId],
    reservation: &mut R,
) -> Result<Vec<usize>> {
    let mut indices = reserved_empty(
        reservation,
        sources.len(),
        "unable to reserve categorical cluster source-index recipe",
    )?;
    for source in sources {
        let index = ols
            .categorical_sources()
            .iter()
            .position(|categorical| categorical.source() == *source)
            .ok_or_else(|| invalid_argument("categorical cluster source disappeared"))?;
        push_reserved(
            &mut indices,
            index,
            "categorical cluster source-index capacity exhausted",
        )?;
    }
    Ok(indices)
}

fn factorize_interaction<R: CategoricalClusterReservation>(
    ols: &PreparedCategoricalOlsData,
    categorical_indices: &[usize],
    reservation: &mut R,
) -> Result<crate::fe::EncodedInteraction> {
    if categorical_indices.len() < 2 {
        return Err(invalid_argument(
            "categorical cluster interaction requires at least two sources",
        ));
    }
    let mut views = reserved_empty(
        reservation,
        categorical_indices.len(),
        "unable to reserve categorical cluster interaction views",
    )?;
    for index in categorical_indices {
        let source = ols
            .categorical_sources()
            .get(*index)
            .ok_or_else(|| invalid_argument("categorical cluster interaction source disappeared"))?;
        push_reserved(
            &mut views,
            CategoricalCodeView::new(
                source.codes(ols.fe_topology())?,
                source.n_levels(ols.fe_topology())?,
            ),
            "categorical cluster interaction-view capacity exhausted",
        )?;
    }
    factorize_code_interaction(&views)
}

fn certify_prepared_clusters(prepared: &PreparedCategoricalOlsClusteredData) -> Result<()> {
    prepared.ols.validate()?;
    let nobs = prepared.ols.sample().n_active();
    if !(1..=10).contains(&prepared.clusters.len()) {
        return Err(invalid_argument(
            "categorical cluster dimension count is outside the certified range",
        ));
    }
    let mut reservation = SystemCategoricalClusterReservation;
    for (dimension_index, dimension) in prepared.clusters.dimensions.iter().enumerate() {
        if dimension.label.is_empty() || dimension.sources.is_empty() || dimension.groups < 2 {
            return Err(invalid_argument(
                "categorical cluster metadata is structurally incomplete",
            ));
        }
        let source_indices =
            resolve_source_indices(&prepared.ols, &dimension.sources, &mut reservation)?;
        let codes = prepared.cluster_codes(dimension_index)?;
        if codes.len() != nobs
            || codes
                .iter()
                .any(|code| *code as usize >= dimension.groups)
        {
            return Err(invalid_data(
                "categorical cluster codes or group count failed certification",
            ));
        }
        match (&dimension.codes, source_indices.as_slice()) {
            (
                PreparedOlsClusterCodes::CategoricalSource {
                    categorical_source_index,
                },
                [expected],
            ) if categorical_source_index == expected => {
                let source = prepared
                    .ols
                    .categorical_sources()
                    .get(*expected)
                    .ok_or_else(|| invalid_argument("certified simple cluster source disappeared"))?;
                if source.n_levels(prepared.ols.fe_topology())? != dimension.groups
                    || !std::ptr::eq(
                        codes.as_ptr(),
                        source.codes(prepared.ols.fe_topology())?.as_ptr(),
                    )
                {
                    return Err(invalid_argument(
                        "categorical simple cluster is not the authoritative zero-copy source",
                    ));
                }
            }
            (PreparedOlsClusterCodes::Interaction(_), indices) if indices.len() >= 2 => {
                let expected = factorize_interaction(&prepared.ols, indices, &mut reservation)?;
                if expected.n_levels() != dimension.groups || expected.codes() != codes {
                    return Err(invalid_data(
                        "categorical cluster interaction differs from its source tuple partition",
                    ));
                }
            }
            _ => {
                return Err(invalid_argument(
                    "categorical cluster storage kind and source recipe disagree",
                ));
            }
        }
        let mut seen = reserved_filled(
            &mut reservation,
            dimension.groups,
            false,
            "unable to reserve categorical cluster certificate groups",
        )?;
        for code in codes {
            seen[*code as usize] = true;
        }
        if seen.iter().any(|present| !present) {
            return Err(invalid_data(
                "categorical cluster codes are not contiguous on the final sample",
            ));
        }
    }
    Ok(())
}

fn reserved_empty<T, R: CategoricalClusterReservation>(
    reservation: &mut R,
    elements: usize,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "categorical cluster reservation returned insufficient capacity",
        ));
    }
    Ok(output)
}

fn reserved_filled<T: Clone, R: CategoricalClusterReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = reserved_empty(reservation, elements, label)?;
    output.resize(elements, value);
    Ok(output)
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn fallible_string_copy<R: CategoricalClusterReservation>(
    value: &str,
    reservation: &mut R,
    label: &'static str,
) -> Result<String> {
    let mut output = String::new();
    reservation.reserve_string(&mut output, value.len(), label)?;
    if output.capacity() < value.len() {
        return Err(allocation(
            "categorical cluster string reservation returned insufficient capacity",
        ));
    }
    output.push_str(value);
    Ok(output)
}

fn invalid_argument(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidData, message)
}

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}
