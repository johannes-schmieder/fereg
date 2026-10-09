pub(crate) use crate::memory::SystemVecReservation as SystemFeTopologyReservation;
pub(crate) use crate::memory::VecReservation as FeTopologyReservation;
#[path = "prepared_topology.rs"]
mod topology;
use topology::build_borrowed_intercept_fe_design_with_reservation;

use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

use crate::data::{
    classify_numeric_sample_into_mask, prepare_numeric_sample_with_source_uses, AdjustmentInput,
    DomainDecision, DropReason, NumericColumnView, NumericDomainPolicy, NumericPreparationInput,
    NumericSourceUse, PreparedNumericSample, PreparedWeights, WeightInput, WeightKind,
};
use crate::error::{
    allocation, invalid_argument as invalid, invalid_data, unsupported, ErrorCode, FeregError,
    Result,
};
use crate::factorize::{factorize_f64, EncodedFactor};
use crate::fe::{BorrowedInterceptFeDesign, ValidatedBorrowedInterceptSource};
use crate::memory::push_reserved;
use crate::metadata::{
    AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, LevelState, MetadataFlag,
    RawTermKind, SourceId, TimeSeriesState,
};

/// Role of one Stata plugin-varlist input in the developer OLS preparation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OlsInputRole {
    Candidate,
    Outcome,
    /// One numeric model source. A single source may be used by both `c.x`
    /// and `i.x`; the binding therefore describes storage, not term meaning.
    NumericSource(SourceId),
    AnalyticWeight,
    FrequencyWeight,
    ProbabilityWeight,
}

/// One-based binding from semantic input role to Stata's plugin varlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OlsInputBinding {
    pub stata_var_position: u32,
    pub role: OlsInputRole,
}

/// Version-one request for the deliberately narrow OLS preparation boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct OlsRequestV1 {
    pub model: CompiledTermGraph,
    pub absorbed_terms: Vec<AbsorbedTerm>,
    pub bindings: Vec<OlsInputBinding>,
    pub include_intercept: bool,
    pub keep_singletons: bool,
}

/// Raw numeric source read by the plugin, retaining Stata missing metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedNumericColumn {
    pub values: Vec<f64>,
    pub missing: Vec<bool>,
}

/// Complete raw input for the numeric-source, optionally weighted preparation
/// slice. At most one weight column may be present. Fixed-effect terms reuse
/// entries in `numeric_sources`; no separate observation-scale FE copy exists.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedOlsInput {
    pub candidate: Vec<bool>,
    pub outcome: LoadedNumericColumn,
    pub numeric_sources: Vec<(SourceId, LoadedNumericColumn)>,
    pub analytic_weight: Option<LoadedNumericColumn>,
    pub frequency_weight: Option<LoadedNumericColumn>,
    pub probability_weight: Option<LoadedNumericColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedOlsColumnKind {
    Intercept,
    Continuous {
        source: SourceId,
        prepared_source_index: usize,
    },
    Categorical {
        source: SourceId,
        prepared_source_index: usize,
        categorical_source_index: usize,
        /// Canonicalized Stata level value, retained as bits so the recipe is
        /// exactly comparable without making an `f64` enum non-`Eq`.
        level_bits: u64,
        /// `None` means Stata expanded this level in the candidate universe,
        /// but the level disappeared after authoritative sample preparation.
        /// The executable column is then identically zero.
        level_code: Option<u32>,
    },
    /// An ordinary all-categorical interaction. The small component recipe is
    /// retained per coefficient column; no observation-scale interaction
    /// column or tuple table is materialized.
    CategoricalInteraction {
        components: Vec<PreparedOlsCategoricalComponent>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsCategoricalComponent {
    pub source: SourceId,
    pub categorical_source_index: usize,
    pub level_bits: u64,
    /// `None` means that Stata expanded the requested level in the candidate
    /// universe but it disappeared after authoritative sample preparation.
    pub level_code: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOlsColumn {
    pub coefficient_slot: CoefficientSlot,
    pub kind: PreparedOlsColumnKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsCoefficientStatus {
    Active,
    SyntaxOmitted,
}

/// One Stata-facing coefficient slot. Generated columns are stored separately
/// so an omitted base remains represented without entering numerical work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsCoefficient {
    pub coefficient_slot: CoefficientSlot,
    pub status: PreparedOlsCoefficientStatus,
    pub generated_column_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOlsModel {
    pub coefficients: Vec<PreparedOlsCoefficient>,
    pub columns: Vec<PreparedOlsColumn>,
}

impl PreparedOlsModel {
    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(&self.coefficients)
            + crate::memory::vec_capacity_bytes(&self.columns)
            + self.nested_recipe_capacity_bytes()
    }

    pub(crate) fn nested_recipe_capacity_bytes(&self) -> u128 {
        self.columns
            .iter()
            .map(|column| match &column.kind {
                PreparedOlsColumnKind::CategoricalInteraction { components } => {
                    components.capacity() as u128
                        * std::mem::size_of::<PreparedOlsCategoricalComponent>() as u128
                }
                _ => 0,
            })
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedCategoricalSource {
    pub source: SourceId,
    pub prepared_source_index: usize,
    pub encoded: EncodedFactor<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsFeSingletonPolicy {
    VerifiedFixedPoint,
    KeptByRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsSingletons {
    pub kept_by_request: bool,
    pub num_singletons: usize,
    pub cascade_events: usize,
}

/// Compatibility name for the single finalized preparation owner.
pub type PreparedOlsData = crate::prepared_categorical_data::PreparedModel;

impl PreparedOlsData {
    #[must_use]
    pub fn fe_effects(&self) -> &[crate::topology_dof::CategoricalTopologyDofEffectSpec] {
        &self.effect_specs
    }

    #[must_use]
    pub const fn fe_singleton_policy(&self) -> PreparedOlsFeSingletonPolicy {
        self.singleton_policy
    }

    pub(crate) fn categorical_codes(&self, source: usize) -> Result<&[u32]> {
        self.categorical_sources()
            .get(source)
            .ok_or_else(|| invalid("prepared categorical source is out of range"))?
            .codes(self.fe_topology())
    }

    pub fn borrowed_intercept_fe_design(&self) -> Result<BorrowedInterceptFeDesign<'_>> {
        let mut reservation = SystemFeTopologyReservation;
        build_borrowed_intercept_fe_design_with_reservation(self, &mut reservation)
    }
}

impl OlsRequestV1 {
    /// Retained request heap, including nested containers and unused capacity.
    /// Borrowed SPI inputs and construction scratch belong to separate owners.
    pub fn owned_buffer_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        bytes(&self.model.sources)
            + bytes(&self.model.terms)
            + bytes(&self.absorbed_terms)
            + bytes(&self.bindings)
            + self
                .model
                .sources
                .iter()
                .map(|source| source.canonical_name.capacity() as u128)
                .sum::<u128>()
            + self
                .model
                .terms
                .iter()
                .map(|term| {
                    term.canonical_token.capacity() as u128
                        + bytes(&term.components)
                        + term
                            .components
                            .iter()
                            .map(|component| {
                                component.raw_operator.capacity() as u128
                                    + component.raw_time_series_operator.capacity() as u128
                            })
                            .sum::<u128>()
                })
                .sum::<u128>()
            + self
                .absorbed_terms
                .iter()
                .map(|term| {
                    bytes(&term.categorical_sources)
                        + bytes(&term.slope_sources)
                        + term.label.capacity() as u128
                        + term.saved_name.capacity() as u128
                })
                .sum::<u128>()
    }

    /// Validate the entire request and compile the currently supported plain
    /// continuous, numeric categorical main-effect, and all-categorical
    /// interaction recipes. This performs no observation reads; categorical
    /// level codes are resolved later.
    pub fn validate_and_compile(&self) -> Result<PreparedOlsModel> {
        self.validate_and_compile_with_cluster_sources(&[])
    }

    /// Validate PPML's ordinary model plus its categorical absorbed topology
    /// without applying the developer OLS boundary's intercept-only limit to
    /// the original absorbed request. The synthetic availability request is
    /// still compiled by that boundary, so ordinary terms, bindings, and
    /// source uses retain the same fail-closed metadata checks.
    pub fn validate_ppml_and_compile(&self) -> Result<PreparedOlsModel> {
        self.validate_ppml_and_compile_with_cluster_sources(&[])
    }

    pub(super) fn validate_ppml_and_compile_with_cluster_sources(
        &self,
        cluster_sources: &[SourceId],
    ) -> Result<PreparedOlsModel> {
        if self.absorbed_terms.is_empty() {
            return self.validate_and_compile_with_cluster_sources(cluster_sources);
        }
        let _plan = crate::fe::plan_categorical_absorbed_request(self)?;
        self.compile_model(cluster_sources, true)
    }

    pub(super) fn validate_and_compile_with_cluster_sources(
        &self,
        cluster_sources: &[SourceId],
    ) -> Result<PreparedOlsModel> {
        self.compile_model(cluster_sources, false)
    }

    pub(super) fn compile_model(
        &self,
        cluster_sources: &[SourceId],
        categorical: bool,
    ) -> Result<PreparedOlsModel> {
        self.model.validate_executable_metadata()?;
        if !categorical {
            validate_absorbed_preparation(self)?;
        }
        validate_bindings(self, cluster_sources)?;
        let structural_intercept = categorical
            || self.absorbed_terms.is_empty()
            || self.absorbed_terms.iter().any(|term| term.intercept);
        let report_intercept = self.include_intercept && structural_intercept;

        let categorical_source_indices = categorical_source_indices(self, cluster_sources)?;
        let mut columns = Vec::new();
        let mut coefficients = Vec::new();
        columns
            .try_reserve_exact(
                self.model
                    .terms
                    .len()
                    .checked_add(usize::from(report_intercept))
                    .ok_or_else(|| allocation("prepared OLS column count overflows"))?,
            )
            .map_err(|_| allocation("unable to reserve prepared OLS columns"))?;
        coefficients
            .try_reserve_exact(
                self.model
                    .terms
                    .len()
                    .checked_add(usize::from(report_intercept))
                    .ok_or_else(|| allocation("prepared OLS coefficient count overflows"))?,
            )
            .map_err(|_| allocation("unable to reserve prepared OLS coefficients"))?;

        for term in &self.model.terms {
            let kind = compile_column_kind(term, &categorical_source_indices)?;

            // Never infer whole-column omission from component metadata. Stata's
            // term flag is the sole authority for whether this recipe is emitted.
            if !term.whole_column_omitted {
                let generated_column_index = columns.len();
                columns.push(PreparedOlsColumn {
                    coefficient_slot: term.coefficient_slot,
                    kind,
                });
                coefficients.push(PreparedOlsCoefficient {
                    coefficient_slot: term.coefficient_slot,
                    status: PreparedOlsCoefficientStatus::Active,
                    generated_column_index: Some(generated_column_index),
                });
            } else {
                coefficients.push(PreparedOlsCoefficient {
                    coefficient_slot: term.coefficient_slot,
                    status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                    generated_column_index: None,
                });
            }
        }

        if report_intercept {
            let next = self
                .model
                .terms
                .len()
                .checked_add(1)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| invalid("intercept coefficient slot exceeds u32"))?;
            columns.push(PreparedOlsColumn {
                coefficient_slot: CoefficientSlot::new(next)?,
                kind: PreparedOlsColumnKind::Intercept,
            });
            coefficients.push(PreparedOlsCoefficient {
                coefficient_slot: CoefficientSlot::new(next)?,
                status: PreparedOlsCoefficientStatus::Active,
                generated_column_index: Some(columns.len() - 1),
            });
        }
        Ok(PreparedOlsModel {
            coefficients,
            columns,
        })
    }
}

fn compile_column_kind(
    term: &crate::metadata::ExpandedTerm,
    categorical_source_indices: &BTreeMap<SourceId, usize>,
) -> Result<PreparedOlsColumnKind> {
    Ok(match (term.raw_kind, term.components.as_slice()) {
        (RawTermKind::Variable, [component])
            if component.kind == ComponentKind::Continuous
                && component.level_state == LevelState::Absent
                && component.base == MetadataFlag::False
                && matches!(
                    component.time_series_state,
                    TimeSeriesState::None | TimeSeriesState::Materialized
                )
                && matches!(component.raw_operator.as_str(), "" | "c" | "co") =>
        {
            PreparedOlsColumnKind::Continuous {
                source: component.source,
                prepared_source_index: source_index(component.source)?,
            }
        }
        (RawTermKind::Factor, [component])
            if component.kind == ComponentKind::Categorical
                && component.level_state == LevelState::Present
                && matches!(
                    component.time_series_state,
                    TimeSeriesState::None | TimeSeriesState::Materialized
                ) =>
        {
            validate_categorical_value(component.level_value, "factor metadata level")?;
            PreparedOlsColumnKind::Categorical {
                source: component.source,
                prepared_source_index: source_index(component.source)?,
                categorical_source_index: categorical_source_index(
                    categorical_source_indices,
                    component.source,
                )?,
                level_bits: canonical_factor_value(component.level_value).to_bits(),
                level_code: None,
            }
        }
        (RawTermKind::Interaction | RawTermKind::Product, components)
            if components.len() >= 2
                && components.iter().all(|component| {
                    component.kind == ComponentKind::Categorical
                        && component.level_state == LevelState::Present
                        && component.time_series_state == TimeSeriesState::None
                }) =>
        {
            let mut prepared_components = Vec::new();
            prepared_components
                .try_reserve_exact(components.len())
                .map_err(|_| allocation("unable to reserve categorical interaction recipe"))?;
            for component in components {
                validate_categorical_value(
                    component.level_value,
                    "categorical interaction metadata level",
                )?;
                prepared_components.push(PreparedOlsCategoricalComponent {
                    source: component.source,
                    categorical_source_index: categorical_source_index(
                        categorical_source_indices,
                        component.source,
                    )?,
                    level_bits: canonical_factor_value(component.level_value).to_bits(),
                    level_code: None,
                });
            }
            PreparedOlsColumnKind::CategoricalInteraction {
                components: prepared_components,
            }
        }
        _ => {
            return Err(unsupported(
                        "prepare-ols-test supports only non-time-series continuous terms, numeric factor main effects, and numeric all-categorical interactions",
                    ));
        }
    })
}

/// Finish the Stata-independent boundary after the SPI layer has loaded raw inputs.
pub fn prepare_ols_data(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<PreparedOlsData> {
    prepare_ols_data_with_cluster_sources(request, loaded, &[])
}

/// Prepare observations in owned plugin buffers. The authoritative classifier
/// determines the shared sample before stable column compaction; supplied
/// weights retain their kind and unmodified reference values.
pub fn prepare_ols_data_owned(
    request: &OlsRequestV1,
    loaded: LoadedOlsInput,
) -> Result<PreparedOlsData> {
    prepare_ols_data_owned_with_cluster_sources(request, loaded, &[])
}

pub(crate) fn prepare_ols_data_owned_with_cluster_sources(
    request: &OlsRequestV1,
    loaded: LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<PreparedOlsData> {
    let (model, sample) = prepare_ols_numeric_sample_owned(request, loaded, cluster_sources)?;
    crate::prepared_categorical_data::finish_prepared_model(
        request,
        cluster_sources,
        model,
        sample,
        false,
        false,
        true,
    )
}

/// Authoritative metadata and numeric-sample stage, before FE factorization.
/// Categorical slope preparation may discard synthetic availability categories
/// only after this stage has classified every continuous slope source.
pub(crate) fn prepare_ols_numeric_sample_owned(
    request: &OlsRequestV1,
    loaded: LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<(PreparedOlsModel, PreparedNumericSample)> {
    prepare_numeric_sample_owned(request, loaded, cluster_sources, false)
}

pub(crate) fn prepare_categorical_numeric_sample_owned(
    request: &OlsRequestV1,
    loaded: LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<(PreparedOlsModel, PreparedNumericSample)> {
    prepare_numeric_sample_owned(request, loaded, cluster_sources, true)
}

fn prepare_numeric_sample_owned(
    request: &OlsRequestV1,
    loaded: LoadedOlsInput,
    cluster_sources: &[SourceId],
    categorical: bool,
) -> Result<(PreparedOlsModel, PreparedNumericSample)> {
    let model = request.compile_model(cluster_sources, categorical)?;
    validate_loaded(request, &loaded)?;
    validate_candidate_weight_domain(&loaded)?;
    let sample = if is_full_retained_unweighted_sample(&loaded) {
        take_full_retained_unweighted_sample(loaded)?
    } else {
        take_classified_sample(request, loaded, cluster_sources)?
    };
    validate_available_categorical_domain(request, &sample)?;
    Ok((model, sample))
}

pub(crate) fn prepare_ols_data_with_cluster_sources(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<PreparedOlsData> {
    let (model, sample) = prepare_ols_numeric_sample(request, loaded, cluster_sources)?;
    finish_prepare_ols_data(request, cluster_sources, model, sample)
}

/// Borrowed counterpart of the owned classification boundary. All source uses
/// are resolved before a caller may discard synthetic availability categories.
pub(crate) fn prepare_ols_numeric_sample(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<(PreparedOlsModel, PreparedNumericSample)> {
    prepare_numeric_sample(request, loaded, cluster_sources, false)
}

pub(crate) fn prepare_categorical_numeric_sample(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<(PreparedOlsModel, PreparedNumericSample)> {
    prepare_numeric_sample(request, loaded, cluster_sources, true)
}

fn prepare_numeric_sample(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    cluster_sources: &[SourceId],
    categorical: bool,
) -> Result<(PreparedOlsModel, PreparedNumericSample)> {
    let model = request.compile_model(cluster_sources, categorical)?;
    validate_loaded(request, loaded)?;

    let regressor_views = loaded
        .numeric_sources
        .iter()
        .map(|(_, column)| NumericColumnView::with_missing(&column.values, &column.missing))
        .collect::<Vec<_>>();
    validate_candidate_weight_domain(loaded)?;
    let weights = match (
        &loaded.analytic_weight,
        &loaded.frequency_weight,
        &loaded.probability_weight,
    ) {
        (None, None, None) => WeightInput::None,
        (Some(column), None, None) => WeightInput::Provided {
            kind: WeightKind::Analytic,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        },
        (None, Some(column), None) => WeightInput::Provided {
            kind: WeightKind::Frequency,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        },
        (None, None, Some(column)) => WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        },
        _ => {
            return Err(invalid(
                "loaded OLS input cannot contain more than one weight column",
            ));
        }
    };
    let input = NumericPreparationInput {
        candidate: &loaded.candidate,
        outcome: NumericColumnView::with_missing(&loaded.outcome.values, &loaded.outcome.missing),
        regressors: &regressor_views,
        weights,
        adjustment: AdjustmentInput::None,
    };
    let source_uses = source_uses(request, cluster_sources)?;
    let sample = match prepare_full_retained_unweighted_sample(loaded)? {
        Some(sample) => sample,
        None => {
            prepare_numeric_sample_with_source_uses(&input, &source_uses, &OlsPreparationPolicy)?
        }
    };
    validate_available_categorical_domain(request, &sample)?;
    Ok((model, sample))
}

fn finish_prepare_ols_data(
    request: &OlsRequestV1,
    cluster_sources: &[SourceId],
    model: PreparedOlsModel,
    sample: PreparedNumericSample,
) -> Result<PreparedOlsData> {
    crate::prepared_categorical_data::finish_prepared_model(
        request,
        cluster_sources,
        model,
        sample,
        false,
        false,
        true,
    )
}

#[cfg(test)]
fn finish_prepare_ols_data_with_membership(
    request: &OlsRequestV1,
    cluster_sources: &[SourceId],
    model: PreparedOlsModel,
    sample: PreparedNumericSample,
    reuse_membership: bool,
) -> Result<PreparedOlsData> {
    crate::prepared_categorical_data::finish_prepared_model(
        request,
        cluster_sources,
        model,
        sample,
        false,
        false,
        reuse_membership,
    )
}

pub(crate) fn factorize_preparation_sources(
    request: &OlsRequestV1,
    sample: &PreparedNumericSample,
) -> Result<Vec<PreparedCategoricalSource>> {
    factorize_categorical_sources_reusing(request, sample, &[], Vec::new())
}

// A one-effect unit sample is a singleton fixed point precisely when no final
// level has one member. Build the ordinary final index once and reuse it for
// that check. Final topology validation still independently checks every code,
// membership and singleton invariant. A failed shortcut drops the index before
// ordinary pruning allocates; its lifetime fits the existing topology peak.
/// Materialize the overwhelmingly common complete, unweighted sample without
/// paying the per-row classification and per-column `push` cost. The scan is
/// still authoritative: every candidate flag, missing mask, and finite-value
/// condition required by the generic OLS policy is checked before this route
/// is selected. Any exclusion or weight returns to the fully general path.
fn prepare_full_retained_unweighted_sample(
    loaded: &LoadedOlsInput,
) -> Result<Option<PreparedNumericSample>> {
    if !is_full_retained_unweighted_sample(loaded) {
        return Ok(None);
    }

    let n = loaded.candidate.len();
    let mut original_row_by_active = Vec::new();
    original_row_by_active
        .try_reserve_exact(n)
        .map_err(|_| allocation("unable to reserve complete-sample observation map"))?;
    original_row_by_active.extend(0..n);

    let mut outcome = Vec::new();
    outcome
        .try_reserve_exact(n)
        .map_err(|_| allocation("unable to reserve complete-sample outcome"))?;
    outcome.extend_from_slice(&loaded.outcome.values);

    let mut regressors = Vec::new();
    regressors
        .try_reserve_exact(loaded.numeric_sources.len())
        .map_err(|_| allocation("unable to reserve complete-sample regressor table"))?;
    for (_, source) in &loaded.numeric_sources {
        let mut destination = Vec::new();
        destination
            .try_reserve_exact(n)
            .map_err(|_| allocation("unable to reserve complete-sample regressor column"))?;
        destination.extend_from_slice(&source.values);
        regressors.push(destination);
    }

    let mut drop_reasons = Vec::new();
    drop_reasons
        .try_reserve_exact(n)
        .map_err(|_| allocation("unable to reserve complete-sample drop-reason mask"))?;
    drop_reasons.resize(n, crate::data::DropReason::Retained);

    Ok(Some(PreparedNumericSample {
        n_original: n,
        n_candidate: n,
        original_row_by_active,
        outcome,
        regressors,
        weights: PreparedWeights::None,
        adjustment: crate::data::PreparedAdjustment::None,
        drop_reasons,
    }))
}

fn is_full_retained_unweighted_sample(loaded: &LoadedOlsInput) -> bool {
    loaded.analytic_weight.is_none()
        && loaded.frequency_weight.is_none()
        && loaded.probability_weight.is_none()
        && loaded.candidate.iter().all(|candidate| *candidate)
        && loaded.outcome.missing.iter().all(|missing| !*missing)
        && loaded.outcome.values.iter().all(|value| value.is_finite())
        && {
            let complete = |(_, column): &(SourceId, LoadedNumericColumn)| {
                column.missing.iter().all(|missing| !*missing)
                    && column.values.iter().all(|value| value.is_finite())
            };
            // This predicate chooses an ownership shortcut, not error order.
            // Failed checks still enter the authoritative classification
            // path. Never create Rayon's global pool for standalone callers.
            if rayon::current_thread_index().is_some()
                && crate::parallel::StagePlan::current(
                    loaded.candidate.len() as u128 * loaded.numeric_sources.len() as u128 * 2,
                    loaded.numeric_sources.len(),
                )
                .lanes
                    > 1
            {
                loaded.numeric_sources.par_iter().all(complete)
            } else {
                loaded.numeric_sources.iter().all(complete)
            }
        }
}

fn take_full_retained_unweighted_sample(loaded: LoadedOlsInput) -> Result<PreparedNumericSample> {
    let n = loaded.candidate.len();
    let mut original_row_by_active = Vec::new();
    original_row_by_active
        .try_reserve_exact(n)
        .map_err(|_| allocation("unable to reserve complete-sample observation map"))?;
    original_row_by_active.extend(0..n);

    let mut regressors = Vec::new();
    regressors
        .try_reserve_exact(loaded.numeric_sources.len())
        .map_err(|_| allocation("unable to reserve complete-sample regressor table"))?;
    regressors.extend(
        loaded
            .numeric_sources
            .into_iter()
            .map(|(_, column)| column.values),
    );

    let mut drop_reasons = Vec::new();
    drop_reasons
        .try_reserve_exact(n)
        .map_err(|_| allocation("unable to reserve complete-sample drop-reason mask"))?;
    drop_reasons.resize(n, crate::data::DropReason::Retained);

    Ok(PreparedNumericSample {
        n_original: n,
        n_candidate: n,
        original_row_by_active,
        outcome: loaded.outcome.values,
        regressors,
        weights: PreparedWeights::None,
        adjustment: crate::data::PreparedAdjustment::None,
        drop_reasons,
    })
}

fn take_classified_sample(
    request: &OlsRequestV1,
    mut loaded: LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<PreparedNumericSample> {
    let n_original = loaded.candidate.len();
    let n_candidate = loaded
        .candidate
        .iter()
        .filter(|candidate| **candidate)
        .count();
    let mut drop_reasons = Vec::new();
    drop_reasons
        .try_reserve_exact(n_original)
        .map_err(|_| allocation("unable to reserve owned-sample drop-reason mask"))?;
    drop_reasons.resize(n_original, DropReason::Retained);
    let mut original_row_by_active = Vec::new();
    original_row_by_active
        .try_reserve_exact(n_candidate)
        .map_err(|_| allocation("unable to reserve owned-sample observation map"))?;
    let mut regressor_views = Vec::new();
    regressor_views
        .try_reserve_exact(loaded.numeric_sources.len())
        .map_err(|_| allocation("unable to reserve owned-sample source views"))?;
    regressor_views.extend(
        loaded
            .numeric_sources
            .iter()
            .map(|(_, column)| NumericColumnView::with_missing(&column.values, &column.missing)),
    );
    let input = NumericPreparationInput {
        candidate: &loaded.candidate,
        outcome: NumericColumnView::with_missing(&loaded.outcome.values, &loaded.outcome.missing),
        regressors: &regressor_views,
        weights: loaded_weight(&loaded).map_or(WeightInput::None, |(kind, column)| {
            WeightInput::Provided {
                kind,
                column: NumericColumnView::with_missing(&column.values, &column.missing),
            }
        }),
        adjustment: AdjustmentInput::None,
    };
    classify_numeric_sample_into_mask(
        &input,
        &source_uses(request, cluster_sources)?,
        &OlsPreparationPolicy,
        &mut drop_reasons,
    )?;
    original_row_by_active.extend(
        drop_reasons
            .iter()
            .enumerate()
            .filter_map(|(row, reason)| (*reason == DropReason::Retained).then_some(row)),
    );
    drop(regressor_views);

    let compact = |column: &mut LoadedNumericColumn| -> Result<()> {
        // Reuse storage only up to the general path's candidate-axis bound.
        // Restricted samples must not retain full-data column capacity.
        if column.values.capacity() <= n_candidate {
            if original_row_by_active.len() != n_original {
                for (active, original) in original_row_by_active.iter().copied().enumerate() {
                    column.values[active] = column.values[original];
                }
            }
            column.values.truncate(original_row_by_active.len());
        } else {
            let mut values = Vec::new();
            values
                .try_reserve_exact(original_row_by_active.len())
                .map_err(|_| allocation("unable to reserve compact owned-sample column"))?;
            values.extend(original_row_by_active.iter().map(|row| column.values[*row]));
            column.values = values;
        }
        Ok(())
    };
    if rayon::current_thread_index().is_some()
        && crate::parallel::StagePlan::current(
            n_original as u128 * loaded.numeric_sources.len() as u128,
            loaded.numeric_sources.len(),
        )
        .lanes
            > 1
    {
        loaded
            .numeric_sources
            .par_iter_mut()
            .try_for_each(|(_, column)| compact(column))?;
    } else {
        loaded
            .numeric_sources
            .iter_mut()
            .try_for_each(|(_, column)| compact(column))?;
    }
    compact(&mut loaded.outcome)?;
    let mut weight = match (
        loaded.analytic_weight.take(),
        loaded.frequency_weight.take(),
        loaded.probability_weight.take(),
    ) {
        (None, None, None) => None,
        (Some(column), None, None) => Some((WeightKind::Analytic, column)),
        (None, Some(column), None) => Some((WeightKind::Frequency, column)),
        (None, None, Some(column)) => Some((WeightKind::Probability, column)),
        _ => {
            return Err(invalid(
                "loaded OLS input cannot contain more than one weight column",
            ))
        }
    };
    if let Some((_, column)) = &mut weight {
        compact(column)?;
    }
    let mut regressors = Vec::new();
    regressors
        .try_reserve_exact(loaded.numeric_sources.len())
        .map_err(|_| allocation("unable to reserve owned-sample regressor table"))?;
    regressors.extend(
        loaded
            .numeric_sources
            .into_iter()
            .map(|(_, column)| column.values),
    );
    Ok(PreparedNumericSample {
        n_original,
        n_candidate,
        original_row_by_active,
        outcome: loaded.outcome.values,
        regressors,
        weights: weight.map_or(PreparedWeights::None, |(kind, column)| {
            PreparedWeights::Provided {
                kind,
                values: column.values,
            }
        }),
        adjustment: crate::data::PreparedAdjustment::None,
        drop_reasons,
    })
}

fn ordinary_categorical_sources(model: &CompiledTermGraph) -> BTreeSet<SourceId> {
    model
        .terms
        .iter()
        .flat_map(|term| &term.components)
        .filter(|component| component.kind == ComponentKind::Categorical)
        .map(|component| component.source)
        .collect()
}

fn categorical_source_indices(
    request: &OlsRequestV1,
    cluster_sources: &[SourceId],
) -> Result<BTreeMap<SourceId, usize>> {
    let mut categorical = ordinary_categorical_sources(&request.model);
    for term in &request.absorbed_terms {
        categorical.extend(term.categorical_sources.iter().copied());
    }
    let mut indices = BTreeMap::new();
    for source in &request.model.sources {
        if categorical.contains(&source.id) || cluster_sources.contains(&source.id) {
            indices.insert(source.id, indices.len());
        }
    }
    Ok(indices)
}

fn categorical_source_index(
    indices: &BTreeMap<SourceId, usize>,
    source: SourceId,
) -> Result<usize> {
    indices
        .get(&source)
        .copied()
        .ok_or_else(|| invalid("categorical source plan disappeared"))
}

fn validate_available_categorical_domain(
    request: &OlsRequestV1,
    sample: &PreparedNumericSample,
) -> Result<()> {
    // Ordinary factors alone require nonnegative integers. Validate after all
    // availability exclusions but before singleton/rank decisions; omitted
    // terms remain in the source registry. FE/cluster domains are unchanged.
    for source in ordinary_categorical_sources(&request.model) {
        let column = &sample.regressors[source_index(source)?];
        for (row, &value) in column.iter().enumerate() {
            if value < 0.0 || value.fract() != 0.0 || !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidFactor,
                    format!(
                        "ordinary factor source {} at observation {} must be a finite nonnegative integer",
                        source.get(), sample.original_row_by_active[row] + 1,
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_categorical_value(value: f64, label: &str) -> Result<()> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
        return Err(invalid_data(format!(
            "{label} must be a finite nonnegative integer",
        )));
    }
    Ok(())
}

fn canonical_factor_value(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn factorize_categorical_sources_reusing(
    request: &OlsRequestV1,
    sample: &PreparedNumericSample,
    cluster_sources: &[SourceId],
    previous: Vec<PreparedCategoricalSource>,
) -> Result<Vec<PreparedCategoricalSource>> {
    let categorical = categorical_source_indices(request, cluster_sources)?;
    let mut previous = previous.into_iter().peekable();
    let mut output = Vec::new();
    output
        .try_reserve_exact(categorical.len())
        .map_err(|_| allocation("unable to reserve prepared categorical sources"))?;
    for source in &request.model.sources {
        if !categorical.contains_key(&source.id) {
            continue;
        }
        if previous
            .peek()
            .is_some_and(|candidate| candidate.source == source.id)
        {
            output.push(previous.next().expect("peeked retained categorical source"));
            continue;
        }
        let prepared_source_index = source_index(source.id)?;
        let values = sample
            .regressors
            .get(prepared_source_index)
            .ok_or_else(|| invalid("prepared categorical source index is out of range"))?;
        output.push(PreparedCategoricalSource {
            source: source.id,
            prepared_source_index,
            encoded: factorize_f64(values)?,
        });
    }
    if previous.next().is_some() {
        return Err(invalid(
            "retained categorical source is absent from the final source plan",
        ));
    }
    Ok(output)
}

fn source_index(source: SourceId) -> Result<usize> {
    usize::try_from(source.get() - 1).map_err(|_| invalid("source id does not fit usize"))
}

fn validate_absorbed_preparation(request: &OlsRequestV1) -> Result<()> {
    for term in &request.absorbed_terms {
        if term.categorical_sources.len() != 1 || !term.intercept || !term.slope_sources.is_empty()
        {
            return Err(unsupported(
                "prepare-ols-test supports only single-source intercept-only numeric fixed effects",
            ));
        }
    }
    Ok(())
}

fn source_uses(
    request: &OlsRequestV1,
    cluster_sources: &[SourceId],
) -> Result<Vec<NumericSourceUse>> {
    let ordinary = request
        .model
        .terms
        .iter()
        .flat_map(|term| &term.components)
        .map(|component| component.source)
        .collect::<BTreeSet<_>>();
    let fixed_effect = request
        .absorbed_terms
        .iter()
        .flat_map(|term| {
            term.categorical_sources
                .iter()
                .chain(&term.slope_sources)
                .copied()
        })
        .collect::<BTreeSet<_>>();
    let mut uses = Vec::new();
    uses.try_reserve_exact(request.model.sources.len())
        .map_err(|_| allocation("unable to reserve numeric source-use plan"))?;
    for source in &request.model.sources {
        let usage = NumericSourceUse {
            ordinary: ordinary.contains(&source.id),
            fixed_effect: fixed_effect.contains(&source.id),
            cluster: cluster_sources.contains(&source.id),
        };
        if !usage.ordinary && !usage.fixed_effect && !usage.cluster {
            return Err(invalid(format!(
                "source {} is not used by an ordinary, fixed-effect, or cluster term",
                source.id.get()
            )));
        }
        uses.push(usage);
    }
    Ok(uses)
}

fn validate_bindings(request: &OlsRequestV1, cluster_sources: &[SourceId]) -> Result<()> {
    let mut positions = BTreeSet::new();
    let mut candidate_count = 0_usize;
    let mut outcome_count = 0_usize;
    let mut analytic_weight_count = 0_usize;
    let mut frequency_weight_count = 0_usize;
    let mut probability_weight_count = 0_usize;
    let mut sources = BTreeMap::new();
    for binding in &request.bindings {
        if binding.stata_var_position == 0 || !positions.insert(binding.stata_var_position) {
            return Err(invalid(
                "OLS input bindings require unique positive Stata varlist positions",
            ));
        }
        match binding.role {
            OlsInputRole::Candidate => candidate_count += 1,
            OlsInputRole::Outcome => outcome_count += 1,
            OlsInputRole::NumericSource(source) => {
                if sources.insert(source, binding.stata_var_position).is_some() {
                    return Err(invalid("numeric source is bound more than once"));
                }
            }
            OlsInputRole::AnalyticWeight => analytic_weight_count += 1,
            OlsInputRole::FrequencyWeight => frequency_weight_count += 1,
            OlsInputRole::ProbabilityWeight => probability_weight_count += 1,
        }
    }
    if candidate_count != 1 || outcome_count != 1 {
        return Err(invalid(
            "OLS request requires exactly one candidate and one outcome binding",
        ));
    }
    if analytic_weight_count + frequency_weight_count + probability_weight_count > 1 {
        return Err(invalid("OLS request permits at most one weight binding"));
    }
    if sources.len() != request.model.sources.len()
        || request
            .model
            .sources
            .iter()
            .any(|source| !sources.contains_key(&source.id))
    {
        return Err(invalid(
            "OLS request must bind every model source exactly once",
        ));
    }
    source_uses(request, cluster_sources)?;
    Ok(())
}

fn validate_loaded(request: &OlsRequestV1, loaded: &LoadedOlsInput) -> Result<()> {
    let n = loaded.candidate.len();
    validate_loaded_column("outcome", &loaded.outcome, n)?;
    if loaded.numeric_sources.len() != request.model.sources.len() {
        return Err(invalid(
            "loaded numeric source count does not match the model registry",
        ));
    }
    for (index, (source, column)) in loaded.numeric_sources.iter().enumerate() {
        if *source != request.model.sources[index].id {
            return Err(invalid(
                "loaded numeric sources are not in model-registry order",
            ));
        }
        validate_loaded_column("numeric source", column, n)?;
    }
    let request_has_analytic_weight = request
        .bindings
        .iter()
        .any(|binding| binding.role == OlsInputRole::AnalyticWeight);
    if request_has_analytic_weight != loaded.analytic_weight.is_some() {
        return Err(invalid(
            "loaded analytic weight does not match the OLS request binding",
        ));
    }
    if let Some(column) = &loaded.analytic_weight {
        validate_loaded_column("analytic weight", column, n)?;
    }
    let request_has_frequency_weight = request
        .bindings
        .iter()
        .any(|binding| binding.role == OlsInputRole::FrequencyWeight);
    if request_has_frequency_weight != loaded.frequency_weight.is_some() {
        return Err(invalid(
            "loaded frequency weight does not match the OLS request binding",
        ));
    }
    if let Some(column) = &loaded.frequency_weight {
        validate_loaded_column("frequency weight", column, n)?;
    }
    let request_has_probability_weight = request
        .bindings
        .iter()
        .any(|binding| binding.role == OlsInputRole::ProbabilityWeight);
    if request_has_probability_weight != loaded.probability_weight.is_some() {
        return Err(invalid(
            "loaded probability weight does not match the OLS request binding",
        ));
    }
    if let Some(column) = &loaded.probability_weight {
        validate_loaded_column("probability weight", column, n)?;
    }
    if usize::from(loaded.analytic_weight.is_some())
        + usize::from(loaded.frequency_weight.is_some())
        + usize::from(loaded.probability_weight.is_some())
        > 1
    {
        return Err(invalid(
            "loaded OLS input cannot contain more than one weight column",
        ));
    }
    Ok(())
}

fn validate_candidate_weight_domain(loaded: &LoadedOlsInput) -> Result<()> {
    let Some((kind, weight)) = loaded_weight(loaded) else {
        return Ok(());
    };
    for (row, candidate) in loaded.candidate.iter().copied().enumerate() {
        if !candidate || weight.missing[row] {
            continue;
        }
        let value = weight.values[row];
        match kind {
            WeightKind::Analytic if value < 0.0 => {
                return Err(negative_weight(format!(
                    "analytic weight at observation {} is negative",
                    row + 1,
                )));
            }
            WeightKind::Frequency
                if !value.is_finite()
                    || value <= 0.0
                    || value.fract() != 0.0
                    || value > f64::from(u32::MAX) =>
            {
                return Err(invalid_frequency_weight(format!(
                    "frequency weight at observation {} must be a positive integer representable as u32",
                    row + 1,
                )));
            }
            WeightKind::Probability if !value.is_finite() || value <= 0.0 => {
                return Err(invalid_data(format!(
                    "probability weight at observation {} must be positive and finite",
                    row + 1,
                )));
            }
            WeightKind::Analytic | WeightKind::Frequency | WeightKind::Probability => {}
            WeightKind::Importance => {
                return Err(unsupported(
                    "OLS preparation received an unsupported weight kind",
                ));
            }
        }
    }
    Ok(())
}

fn loaded_weight(loaded: &LoadedOlsInput) -> Option<(WeightKind, &LoadedNumericColumn)> {
    match (
        &loaded.analytic_weight,
        &loaded.frequency_weight,
        &loaded.probability_weight,
    ) {
        (Some(column), None, None) => Some((WeightKind::Analytic, column)),
        (None, Some(column), None) => Some((WeightKind::Frequency, column)),
        (None, None, Some(column)) => Some((WeightKind::Probability, column)),
        _ => None,
    }
}

fn validate_loaded_column(name: &str, column: &LoadedNumericColumn, n: usize) -> Result<()> {
    if column.values.len() != n || column.missing.len() != n {
        return Err(invalid(format!(
            "loaded {name} values and missing mask must both have length {n}",
        )));
    }
    Ok(())
}

struct OlsPreparationPolicy;

impl NumericDomainPolicy for OlsPreparationPolicy {
    fn outcome(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }

    fn weight(&self, _row: usize, kind: WeightKind, value: f64) -> Result<DomainDecision> {
        match kind {
            WeightKind::Analytic if value == 0.0 => Ok(DomainDecision::Drop),
            WeightKind::Analytic if value < 0.0 => {
                Err(negative_weight("analytic weight is negative"))
            }
            WeightKind::Analytic => Ok(DomainDecision::Retain),
            WeightKind::Frequency
                if value > 0.0 && value.fract() == 0.0 && value <= f64::from(u32::MAX) =>
            {
                Ok(DomainDecision::Retain)
            }
            WeightKind::Frequency => Err(invalid_frequency_weight(
                "frequency weight must be a positive integer representable as u32",
            )),
            WeightKind::Probability if value > 0.0 => Ok(DomainDecision::Retain),
            WeightKind::Probability => Err(invalid_data(
                "probability weight must be positive and finite",
            )),
            WeightKind::Importance => Err(unsupported(
                "OLS preparation received an unsupported weight kind",
            )),
        }
    }

    fn offset(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
        Err(invalid("OLS preparation received an offset"))
    }

    fn exposure(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
        Err(invalid("OLS preparation received an exposure"))
    }
}

fn negative_weight(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NegativeWeight, message)
}

fn invalid_frequency_weight(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidFrequencyWeight, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, ComponentOrder, ExpandedTerm, SourceEntry, TermComponent,
        TermId, TermOrder,
    };

    #[derive(Default)]
    struct InjectedFeTopologyReservation {
        fail_at: Option<usize>,
        calls: usize,
        skip_label: Option<&'static str>,
    }

    impl FeTopologyReservation for InjectedFeTopologyReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected prepared FE topology allocation failure",
                ));
            }
            if self.skip_label == Some(label) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test prepared FE topology reservation failed"))
        }
    }

    fn request(source_count: usize, include_intercept: bool) -> OlsRequestV1 {
        let sources = (0..source_count)
            .map(|index| SourceEntry {
                id: SourceId::new(u32::try_from(index + 1).expect("small fixture")).expect("id"),
                canonical_name: format!("x{}", index + 1),
            })
            .collect::<Vec<_>>();
        let terms = sources
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let one_based = u32::try_from(index + 1).expect("small fixture");
                ExpandedTerm {
                    id: TermId::new(one_based).expect("term id"),
                    order: TermOrder::new(one_based).expect("term order"),
                    coefficient_slot: CoefficientSlot::new(one_based).expect("slot"),
                    raw_kind: RawTermKind::Variable,
                    whole_column_omitted: false,
                    canonical_token: format!("c.x{}", index + 1),
                    components: vec![TermComponent {
                        order: ComponentOrder::new(1).expect("component order"),
                        source: source.id,
                        kind: ComponentKind::Continuous,
                        level_state: LevelState::Absent,
                        level_value: 0.0,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::Unavailable,
                        raw_operator: "c".to_owned(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    }],
                }
            })
            .collect::<Vec<_>>();
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ];
        bindings.extend(
            sources
                .iter()
                .enumerate()
                .map(|(index, source)| OlsInputBinding {
                    stata_var_position: u32::try_from(index + 3).expect("small fixture"),
                    role: OlsInputRole::NumericSource(source.id),
                }),
        );
        OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            absorbed_terms: Vec::new(),
            bindings,
            include_intercept,
            keep_singletons: false,
        }
    }

    fn column(values: &[f64], missing: &[bool]) -> LoadedNumericColumn {
        LoadedNumericColumn {
            values: values.to_vec(),
            missing: missing.to_vec(),
        }
    }

    fn mixed_numeric_factor_request() -> OlsRequestV1 {
        let source = SourceEntry {
            id: SourceId::new(1).expect("source"),
            canonical_name: "g".to_owned(),
        };
        let mut terms = Vec::new();
        terms.push(ExpandedTerm {
            id: TermId::new(1).expect("term"),
            order: TermOrder::new(1).expect("order"),
            coefficient_slot: CoefficientSlot::new(1).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: "c.g".to_owned(),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source: source.id,
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.0,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".to_owned(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        });
        for (index, level, base, omitted, token, raw_operator) in [
            (2, 0.0, true, true, "0b.g", "0b"),
            (3, 1.0, false, false, "1.g", "1"),
            (4, 2.0, false, false, "2.g", "2"),
            (5, 3.0, false, false, "3.g", "3"),
        ] {
            terms.push(ExpandedTerm {
                id: TermId::new(index).expect("term"),
                order: TermOrder::new(index).expect("order"),
                coefficient_slot: CoefficientSlot::new(index).expect("slot"),
                raw_kind: RawTermKind::Factor,
                whole_column_omitted: omitted,
                canonical_token: token.to_owned(),
                components: vec![TermComponent {
                    order: ComponentOrder::new(1).expect("component"),
                    source: source.id,
                    kind: ComponentKind::Categorical,
                    level_state: LevelState::Present,
                    level_value: level,
                    base: if base {
                        MetadataFlag::True
                    } else {
                        MetadataFlag::False
                    },
                    omitted: if omitted {
                        MetadataFlag::True
                    } else {
                        MetadataFlag::False
                    },
                    raw_operator: raw_operator.to_owned(),
                    time_series_state: TimeSeriesState::None,
                    raw_time_series_operator: String::new(),
                }],
            });
        }
        OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![source.clone()],
                terms,
            },
            absorbed_terms: Vec::new(),
            bindings: vec![
                OlsInputBinding {
                    stata_var_position: 1,
                    role: OlsInputRole::Candidate,
                },
                OlsInputBinding {
                    stata_var_position: 2,
                    role: OlsInputRole::Outcome,
                },
                OlsInputBinding {
                    stata_var_position: 3,
                    role: OlsInputRole::NumericSource(source.id),
                },
            ],
            include_intercept: true,
            keep_singletons: false,
        }
    }

    fn categorical_component(order: u32, source: SourceId, level: f64) -> TermComponent {
        TermComponent {
            order: ComponentOrder::new(order).expect("component"),
            source,
            kind: ComponentKind::Categorical,
            level_state: LevelState::Present,
            level_value: level,
            base: MetadataFlag::False,
            omitted: MetadataFlag::False,
            raw_operator: format!("{level}"),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }
    }

    fn all_categorical_interaction_request() -> OlsRequestV1 {
        let sources = ["g", "h", "j"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| SourceEntry {
                id: SourceId::new(u32::try_from(index + 1).expect("small fixture"))
                    .expect("source"),
                canonical_name: name.to_owned(),
            })
            .collect::<Vec<_>>();
        let specifications: Vec<(RawTermKind, bool, &str, Vec<TermComponent>)> = vec![
            (
                RawTermKind::Factor,
                true,
                "1b.g",
                vec![categorical_component(1, sources[0].id, 1.0)],
            ),
            (
                RawTermKind::Factor,
                false,
                "2.g",
                vec![categorical_component(1, sources[0].id, 2.0)],
            ),
            (
                RawTermKind::Interaction,
                false,
                "1.g#1.h",
                vec![
                    categorical_component(1, sources[0].id, 1.0),
                    categorical_component(2, sources[1].id, 1.0),
                ],
            ),
            (
                RawTermKind::Product,
                false,
                "2.g#2.h",
                vec![
                    categorical_component(1, sources[0].id, 2.0),
                    categorical_component(2, sources[1].id, 2.0),
                ],
            ),
            (
                RawTermKind::Interaction,
                false,
                "3.g#1.h",
                vec![
                    categorical_component(1, sources[0].id, 3.0),
                    categorical_component(2, sources[1].id, 1.0),
                ],
            ),
            (
                RawTermKind::Interaction,
                false,
                "1.g#2.h#2.j",
                vec![
                    categorical_component(1, sources[0].id, 1.0),
                    categorical_component(2, sources[1].id, 2.0),
                    categorical_component(3, sources[2].id, 2.0),
                ],
            ),
        ];
        let terms = specifications
            .into_iter()
            .enumerate()
            .map(|(index, (raw_kind, omitted, token, components))| {
                let one_based = u32::try_from(index + 1).expect("small fixture");
                ExpandedTerm {
                    id: TermId::new(one_based).expect("term"),
                    order: TermOrder::new(one_based).expect("order"),
                    coefficient_slot: CoefficientSlot::new(one_based).expect("slot"),
                    raw_kind,
                    whole_column_omitted: omitted,
                    canonical_token: token.to_owned(),
                    components,
                }
            })
            .collect();
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ];
        bindings.extend(
            sources
                .iter()
                .enumerate()
                .map(|(index, source)| OlsInputBinding {
                    stata_var_position: u32::try_from(index + 3).expect("small fixture"),
                    role: OlsInputRole::NumericSource(source.id),
                }),
        );
        OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            absorbed_terms: Vec::new(),
            bindings,
            include_intercept: true,
            keep_singletons: false,
        }
    }

    #[test]
    fn exact_row_map_drop_precedence_bits_and_intercept_are_preserved() {
        let request = request(2, true);
        let negative_zero = -0.0_f64;
        let tiny = f64::from_bits(1);
        let loaded = LoadedOlsInput {
            candidate: vec![false, true, true, true, true, true],
            outcome: column(
                &[99.0, negative_zero, 2.0, 3.0, 4.0, tiny],
                &[false, false, true, false, false, false],
            ),
            numeric_sources: vec![
                (
                    SourceId::new(1).expect("id"),
                    column(
                        &[88.0, 10.0, 20.0, 30.0, 40.0, tiny],
                        &[false, false, true, true, false, false],
                    ),
                ),
                (
                    SourceId::new(2).expect("id"),
                    column(
                        &[77.0, 11.0, 21.0, 31.0, 41.0, negative_zero],
                        &[false, false, false, false, true, false],
                    ),
                ),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("prepare");
        assert_eq!(prepared.sample.retained_rows(), &[1, 5]);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::ExcludedByIfIn,
                crate::DropReason::Retained,
                crate::DropReason::MissingOutcome,
                crate::DropReason::MissingRegressor,
                crate::DropReason::MissingRegressor,
                crate::DropReason::Retained,
            ]
        );
        assert_eq!(
            prepared.sample.outcome[0].to_bits(),
            negative_zero.to_bits()
        );
        assert_eq!(prepared.sample.outcome[1].to_bits(), tiny.to_bits());
        assert_eq!(
            prepared.sample.regressors[1][1].to_bits(),
            negative_zero.to_bits()
        );
        assert_eq!(prepared.model.columns.len(), 3);
        assert_eq!(
            prepared.model.columns[2],
            PreparedOlsColumn {
                coefficient_slot: CoefficientSlot::new(3).expect("slot"),
                kind: PreparedOlsColumnKind::Intercept,
            }
        );
    }

    #[test]
    fn owned_complete_sample_fast_path_matches_borrowed_preparation_exactly() {
        let request = request(2, true);
        let loaded = LoadedOlsInput {
            candidate: vec![true; 5],
            outcome: column(&[1.0, -0.0, 3.5, f64::from_bits(1), -7.0], &[false; 5]),
            numeric_sources: vec![
                (
                    SourceId::new(1).expect("id"),
                    column(&[2.0, 4.0, -6.0, 8.0, 10.0], &[false; 5]),
                ),
                (
                    SourceId::new(2).expect("id"),
                    column(&[-1.0, 3.0, 5.0, 7.0, 9.0], &[false; 5]),
                ),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let borrowed = prepare_ols_data(&request, &loaded).expect("borrowed preparation");
        let owned = prepare_ols_data_owned(&request, loaded).expect("owned preparation");
        assert_eq!(owned, borrowed);
    }

    #[test]
    fn parallel_owned_preparation_preserves_serial_sample_and_error_precedence() {
        for width in [7, 8, 11, 16] {
            let request = request(width, true);
            let n = 32_771;
            let full = LoadedOlsInput {
                candidate: vec![true; n],
                outcome: LoadedNumericColumn {
                    values: vec![1.0; n],
                    missing: vec![false; n],
                },
                numeric_sources: (1..=width)
                    .map(|source| {
                        (
                            SourceId::new(source as u32).expect("source"),
                            LoadedNumericColumn {
                                values: (0..n).map(|row| (row % 13) as f64).collect(),
                                missing: vec![false; n],
                            },
                        )
                    })
                    .collect(),
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            };
            for case in 0..7 {
                let mut loaded = full.clone();
                if case > 0 {
                    loaded.numeric_sources[width - 1].1.values[71] = f64::NAN;
                    loaded.numeric_sources[width - 1].1.missing[71] = case == 1;
                    loaded.candidate[71] = case != 3;
                }
                if case == 4 {
                    loaded.candidate.fill(false);
                }
                if case == 5 {
                    loaded.outcome.missing.fill(true);
                }
                if case == 6 {
                    for row in 0..n {
                        loaded.candidate[row] = row % 13 == 0;
                    }
                }
                let serial = prepare_ols_data(&request, &loaded);
                for threads in [1, 4] {
                    let input = loaded.clone();
                    let team = crate::ThreadTeam::new(
                        crate::ThreadConfig::from_requested(threads, true).expect("threads"),
                    );
                    let parallel = team
                        .install(|| prepare_ols_data_owned(&request, input))
                        .expect("worker team");
                    match (&serial, parallel) {
                        (Ok(expected), Ok(actual)) => assert_eq!(*expected, actual),
                        (Err(expected), Err(actual)) => {
                            assert_eq!(expected.code, actual.code);
                            assert_eq!(expected.message, actual.message);
                        }
                        other => panic!("preparation outcome changed: {other:?}"),
                    }
                }
            }
        }
    }

    #[test]
    fn owned_incomplete_compaction_preserves_value_bits_and_candidate_capacity() {
        let request = request(1, true);
        for restricted in [false, true] {
            let loaded = LoadedOlsInput {
                candidate: vec![!restricted, true, true, true, !restricted, true],
                outcome: column(
                    &[0.0, -0.0, 3.0, f64::from_bits(1), 5.0, f64::MAX],
                    &[false; 6],
                ),
                numeric_sources: vec![(
                    SourceId::new(1).unwrap(),
                    column(
                        &[0.0, -0.0, f64::NAN, f64::from_bits(1), 5.0, f64::MAX],
                        &[false; 6],
                    ),
                )],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            };
            let prepared_expected = prepare_ols_data(&request, &loaded).unwrap();
            let expected = prepared_expected.sample();
            let actual = take_classified_sample(&request, loaded, &[]).unwrap();
            assert_eq!(&actual, expected);
            for (a, b) in actual
                .outcome
                .iter()
                .zip(&expected.outcome)
                .chain(actual.regressors[0].iter().zip(&expected.regressors[0]))
            {
                assert_eq!(a.to_bits(), b.to_bits());
            }
            assert!(actual.outcome.capacity() <= actual.n_candidate);
            assert!(actual.regressors[0].capacity() <= actual.n_candidate);
        }
    }

    #[test]
    fn owned_weights_preserve_samples_errors_bits_and_capacity() {
        let n = 32_771;
        for (kind, role) in [
            (WeightKind::Analytic, OlsInputRole::AnalyticWeight),
            (WeightKind::Frequency, OlsInputRole::FrequencyWeight),
            (WeightKind::Probability, OlsInputRole::ProbabilityWeight),
        ] {
            let mut request = request(16, true);
            request.bindings.push(OlsInputBinding {
                stata_var_position: 19,
                role,
            });
            for case in 0..5 {
                let mut loaded = LoadedOlsInput {
                    candidate: (0..n).map(|row| case != 2 || row % 3 == 0).collect(),
                    outcome: LoadedNumericColumn {
                        values: vec![-0.0; n],
                        missing: vec![false; n],
                    },
                    numeric_sources: (1..=16)
                        .map(|source| {
                            (
                                SourceId::new(source).unwrap(),
                                LoadedNumericColumn {
                                    values: vec![f64::from_bits(1); n],
                                    missing: vec![false; n],
                                },
                            )
                        })
                        .collect(),
                    analytic_weight: None,
                    frequency_weight: None,
                    probability_weight: None,
                };
                let mut weight = LoadedNumericColumn {
                    values: (0..n)
                        .map(|row| {
                            if kind == WeightKind::Frequency {
                                (row % 3 + 1) as f64
                            } else if row % 2 == 0 {
                                1e-100
                            } else {
                                1e100
                            }
                        })
                        .collect(),
                    missing: vec![false; n],
                };
                if case > 0 {
                    loaded.outcome.missing[2] = true;
                    loaded.numeric_sources[15].1.missing[16_385] = true;
                    weight.missing[4] = true;
                    if kind == WeightKind::Analytic {
                        weight.values[5] = 0.0;
                    }
                }
                if case >= 3 {
                    // Weight-domain validation precedes availability, including
                    // missing outcomes, and chooses the first original row.
                    weight.values[2] = -1.0;
                    weight.values[16_386] = -2.0;
                    loaded.candidate[2] = case != 4;
                }
                match kind {
                    WeightKind::Analytic => loaded.analytic_weight = Some(weight),
                    WeightKind::Frequency => loaded.frequency_weight = Some(weight),
                    WeightKind::Probability => loaded.probability_weight = Some(weight),
                    WeightKind::Importance => unreachable!(),
                }
                let expected = prepare_ols_data(&request, &loaded);
                for threads in [1, 4] {
                    let input = loaded.clone();
                    let actual = crate::ThreadTeam::new(
                        crate::ThreadConfig::from_requested(threads, true).unwrap(),
                    )
                    .install(|| prepare_ols_data_owned(&request, input))
                    .unwrap();
                    match (&expected, actual) {
                        (Ok(expected), Ok(actual)) => {
                            assert_eq!(*expected, actual);
                            let PreparedWeights::Provided {
                                kind: actual_kind,
                                values,
                            } = &actual.sample.weights
                            else {
                                panic!("weight lost")
                            };
                            assert_eq!(kind, *actual_kind);
                            assert!(values.capacity() <= actual.sample.n_candidate);
                            assert!(actual.sample.outcome.capacity() <= actual.sample.n_candidate);
                            assert!(actual
                                .sample
                                .regressors
                                .iter()
                                .all(|column| column.capacity() <= actual.sample.n_candidate));
                            assert!(actual
                                .sample
                                .outcome
                                .iter()
                                .all(|value| value.to_bits() == (-0.0_f64).to_bits()));
                            assert!(actual
                                .sample
                                .regressors
                                .iter()
                                .flatten()
                                .all(|value| value.to_bits() == 1));
                        }
                        (Err(expected), Err(actual)) => {
                            assert_eq!(expected.code, actual.code);
                            assert_eq!(expected.message, actual.message);
                        }
                        other => panic!("owned weight outcome changed: {other:?}"),
                    }
                }
            }
        }
    }

    #[test]
    fn owned_incomplete_sample_preserves_singleton_cascades() {
        for keep in [false, true] {
            let request = two_way_singleton_request(keep);
            for excluded in [false, true] {
                let mut loaded = two_way_singleton_input();
                loaded.candidate[0] = !excluded;
                loaded.outcome.missing[1] = true;
                assert_eq!(
                    prepare_ols_data(&request, &loaded).unwrap(),
                    prepare_ols_data_owned(&request, loaded).unwrap()
                );
            }
        }
    }

    #[test]
    fn numeric_factor_main_effects_preserve_slots_factorize_once_and_allow_dual_use() {
        let request = mixed_numeric_factor_request();
        let loaded = LoadedOlsInput {
            candidate: vec![false, true, true, true, true, true],
            outcome: column(
                &[99.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                &[false, false, false, true, false, false],
            ),
            numeric_sources: vec![(
                SourceId::new(1).expect("source"),
                column(
                    &[-7.5, -0.0, 1.0, 3.0, 2.0, 2.0],
                    &[false, false, false, false, true, false],
                ),
            )],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("factor preparation");

        assert_eq!(prepared.sample.retained_rows(), &[1, 2, 5]);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::ExcludedByIfIn,
                crate::DropReason::Retained,
                crate::DropReason::Retained,
                crate::DropReason::MissingOutcome,
                crate::DropReason::MissingRegressor,
                crate::DropReason::Retained,
            ]
        );
        assert_eq!(prepared.categorical_sources().len(), 1);
        assert_eq!(
            prepared.categorical_sources()[0].level_values(),
            vec![0.0, 1.0, 2.0]
        );
        assert_eq!(
            prepared.categorical_sources()[0]
                .codes(prepared.fe_topology())
                .unwrap(),
            vec![0, 1, 2]
        );
        assert_eq!(prepared.model.coefficients.len(), 6);
        assert_eq!(
            prepared.model.coefficients[1],
            PreparedOlsCoefficient {
                coefficient_slot: CoefficientSlot::new(2).expect("slot"),
                status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                generated_column_index: None,
            }
        );
        assert_eq!(prepared.model.columns.len(), 5);
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(0, row).expect("continuous"))
                .collect::<Vec<_>>(),
            vec![-0.0, 1.0, 2.0]
        );
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(1, row).expect("level one"))
                .collect::<Vec<_>>(),
            vec![0.0, 1.0, 0.0]
        );
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(2, row).expect("level two"))
                .collect::<Vec<_>>(),
            vec![0.0, 0.0, 1.0]
        );
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(3, row).expect("vanished level"))
                .collect::<Vec<_>>(),
            vec![0.0, 0.0, 0.0]
        );
        let PreparedOlsColumnKind::Categorical { level_code, .. } = &prepared.model.columns[3].kind
        else {
            panic!("fourth generated column must be categorical");
        };
        assert_eq!(*level_code, None);
    }

    #[test]
    fn categorical_domain_follows_availability_before_singletons() {
        for invalid_value in [-1.0, 1.5] {
            let request = mixed_numeric_factor_request();
            let loaded = LoadedOlsInput {
                candidate: vec![false, true],
                outcome: column(&[1.0, 2.0], &[false, false]),
                numeric_sources: vec![(
                    SourceId::new(1).expect("source"),
                    column(&[invalid_value, 1.0], &[false, false]),
                )],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            };
            prepare_ols_data(&request, &loaded).expect("off-candidate invalid factor value");

            let mut candidate_invalid = loaded;
            candidate_invalid.candidate[0] = true;
            let error = prepare_ols_data(&request, &candidate_invalid)
                .expect_err("candidate invalid factor value");
            assert_eq!(error.code, ErrorCode::InvalidFactor);
            candidate_invalid.outcome.missing[0] = true;
            for owned in [false, true] {
                let prepared = if owned {
                    prepare_ols_data_owned(&request, candidate_invalid.clone())
                } else {
                    prepare_ols_data(&request, &candidate_invalid)
                }
                .expect("unavailable invalid factor is excluded");
                assert_eq!(prepared.sample.retained_rows(), &[1]);
            }
        }
    }

    #[test]
    fn all_categorical_interactions_preserve_slots_and_evaluate_without_materialization() {
        let request = all_categorical_interaction_request();
        let loaded = LoadedOlsInput {
            candidate: vec![true; 5],
            outcome: column(
                &[10.0, 11.0, 12.0, 13.0, 14.0],
                &[false, false, false, true, true],
            ),
            numeric_sources: vec![
                (
                    SourceId::new(1).expect("g"),
                    column(&[1.0, 1.0, 2.0, 2.0, 3.0], &[false; 5]),
                ),
                (
                    SourceId::new(2).expect("h"),
                    column(&[1.0, 2.0, 1.0, 2.0, 1.0], &[false; 5]),
                ),
                (
                    SourceId::new(3).expect("j"),
                    column(&[1.0, 2.0, 2.0, 1.0, 1.0], &[false; 5]),
                ),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("interaction preparation");

        assert_eq!(prepared.sample.retained_rows(), &[0, 1, 2]);
        assert_eq!(prepared.categorical_sources().len(), 3);
        assert_eq!(prepared.model.coefficients.len(), 7);
        assert_eq!(
            prepared.model.coefficients[0],
            PreparedOlsCoefficient {
                coefficient_slot: CoefficientSlot::new(1).expect("slot"),
                status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                generated_column_index: None,
            }
        );
        assert_eq!(prepared.model.columns.len(), 6);
        let evaluated = (0..prepared.model.columns.len())
            .map(|column| {
                (0..prepared.sample.n_active())
                    .map(|row| prepared.column_value(column, row).expect("column value"))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            evaluated,
            vec![
                vec![0.0, 0.0, 1.0],
                vec![1.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![1.0, 1.0, 1.0],
            ]
        );

        let PreparedOlsColumnKind::CategoricalInteraction { components } =
            &prepared.model.columns[2].kind
        else {
            panic!("third generated column must be a categorical interaction");
        };
        assert!(components
            .iter()
            .all(|component| component.level_code.is_some()));
        let PreparedOlsColumnKind::CategoricalInteraction { components } =
            &prepared.model.columns[3].kind
        else {
            panic!("fourth generated column must be a categorical interaction");
        };
        assert_eq!(components[0].level_code, None);
    }

    #[test]
    fn categorical_interaction_values_are_stable_under_row_permutation() {
        let request = all_categorical_interaction_request();
        let make_loaded = |order: &[usize]| LoadedOlsInput {
            candidate: vec![true; order.len()],
            outcome: column(
                &order
                    .iter()
                    .map(|row| [10.0, 11.0, 12.0][*row])
                    .collect::<Vec<_>>(),
                &vec![false; order.len()],
            ),
            numeric_sources: [[1.0, 1.0, 2.0], [1.0, 2.0, 1.0], [1.0, 2.0, 2.0]]
                .into_iter()
                .enumerate()
                .map(|(source, values)| {
                    (
                        SourceId::new(u32::try_from(source + 1).expect("small fixture"))
                            .expect("source"),
                        column(
                            &order.iter().map(|row| values[*row]).collect::<Vec<_>>(),
                            &vec![false; order.len()],
                        ),
                    )
                })
                .collect(),
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let original = prepare_ols_data(&request, &make_loaded(&[0, 1, 2])).expect("original");
        let permuted = prepare_ols_data(&request, &make_loaded(&[2, 0, 1])).expect("permuted");
        for column in 0..original.model.columns.len() {
            let original_values =
                [0, 1, 2].map(|row| original.column_value(column, row).expect("original value"));
            let permuted_values =
                [1, 2, 0].map(|row| permuted.column_value(column, row).expect("permuted value"));
            assert_eq!(original_values, permuted_values);
        }
    }

    #[test]
    fn mixed_and_time_series_interactions_remain_unsupported() {
        let mut mixed = all_categorical_interaction_request();
        let component = &mut mixed.model.terms[2].components[1];
        component.kind = ComponentKind::Continuous;
        component.level_state = LevelState::Absent;
        component.level_value = 0.0;
        component.base = MetadataFlag::False;
        component.raw_operator = "c".to_owned();
        assert_eq!(
            mixed
                .validate_and_compile()
                .expect_err("mixed interaction")
                .code,
            ErrorCode::Unsupported
        );

        let mut pending = all_categorical_interaction_request();
        pending.model.terms[2].components[0].time_series_state =
            TimeSeriesState::PendingMaterialization;
        pending.model.terms[2].components[0].raw_time_series_operator = "L".to_owned();
        assert_eq!(
            pending
                .validate_and_compile()
                .expect_err("pending time-series interaction")
                .code,
            ErrorCode::Unsupported
        );
    }

    #[test]
    fn empty_and_all_dropped_samples_are_valid_preparation_results() {
        let no_fe = LoadedOlsInput {
            candidate: vec![true, true],
            outcome: column(&[1.0, 2.0], &[false; 2]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request(0, false), &no_fe).expect("nonempty no-FE data");
        let design = prepared
            .borrowed_intercept_fe_design()
            .expect("nonempty no-FE design");
        assert_eq!(design.nobs(), 2);
        assert!(design.effects().is_empty());

        let empty = LoadedOlsInput {
            candidate: Vec::new(),
            outcome: column(&[], &[]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request(0, false), &empty).expect("empty");
        assert_eq!(prepared.sample.n_original, 0);
        assert_eq!(prepared.sample.n_active(), 0);
        assert!(prepared.model.columns.is_empty());

        let dropped = LoadedOlsInput {
            candidate: vec![true, false],
            outcome: column(&[1.0, 2.0], &[true, false]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request(0, true), &dropped).expect("all dropped");
        assert_eq!(prepared.sample.n_candidate, 1);
        assert_eq!(prepared.sample.n_active(), 0);
        assert_eq!(prepared.model.columns.len(), 1);
        assert_eq!(prepared.fe_topology().nobs(), 0);
        assert!(prepared.fe_topology().registry().sources().is_empty());
        assert!(prepared.fe_effects().is_empty());
        assert_eq!(prepared.fe_topology().registry().total_levels().unwrap(), 0);
        let design = prepared
            .borrowed_intercept_fe_design()
            .expect("empty no-FE design");
        assert_eq!(design.nobs(), 0);
        assert!(design.effects().is_empty());
        assert_eq!(design.total_term_slots(), 0);
    }

    #[test]
    fn malformed_bindings_and_unsupported_terms_fail_closed() {
        let mut malformed = request(1, false);
        malformed.bindings.pop();
        assert_eq!(
            malformed
                .validate_and_compile()
                .expect_err("missing source binding")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut unsupported = request(1, false);
        unsupported.model.terms[0].raw_kind = RawTermKind::Factor;
        assert_eq!(
            unsupported
                .validate_and_compile()
                .expect_err("factor unsupported")
                .code,
            ErrorCode::Unsupported
        );
    }

    #[test]
    fn whole_column_flag_is_authoritative_for_omission() {
        let mut request = request(1, false);
        request.model.terms[0].components[0].omitted = MetadataFlag::True;
        assert_eq!(
            request
                .validate_and_compile()
                .expect("component flag is inapplicable")
                .columns
                .len(),
            1
        );
        request.model.terms[0].whole_column_omitted = true;
        let model = request
            .validate_and_compile()
            .expect("whole-column omission");
        assert!(model.columns.is_empty());
        assert_eq!(
            model.coefficients[0].status,
            PreparedOlsCoefficientStatus::SyntaxOmitted
        );
    }

    #[test]
    fn analytic_weights_preserve_positive_bits_and_lock_drop_semantics() {
        let mut request = request(1, false);
        request.bindings.push(OlsInputBinding {
            stata_var_position: 4,
            role: OlsInputRole::AnalyticWeight,
        });
        let tiny = f64::from_bits(1);
        let huge = 1.0e300_f64;
        let loaded = LoadedOlsInput {
            candidate: vec![false, true, true, true, true, true],
            outcome: column(&[9.0, 1.0, 2.0, 3.0, 4.0, 5.0], &[false; 6]),
            numeric_sources: vec![(
                SourceId::new(1).expect("id"),
                column(&[9.0, 10.0, 20.0, 30.0, 40.0, 50.0], &[false; 6]),
            )],
            analytic_weight: Some(column(
                &[-1.0, tiny, huge, 0.0, -0.0, 7.0],
                &[false, false, false, false, false, true],
            )),
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("analytic-weight prepare");
        assert_eq!(prepared.sample.retained_rows(), &[1, 2]);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::ExcludedByIfIn,
                crate::DropReason::Retained,
                crate::DropReason::Retained,
                crate::DropReason::InvalidOrMissingWeight,
                crate::DropReason::InvalidOrMissingWeight,
                crate::DropReason::InvalidOrMissingWeight,
            ]
        );
        let crate::data::PreparedWeights::Provided { kind, values } = &prepared.sample.weights
        else {
            panic!("analytic weights must remain present");
        };
        assert_eq!(*kind, WeightKind::Analytic);
        assert_eq!(values[0].to_bits(), tiny.to_bits());
        assert_eq!(values[1].to_bits(), huge.to_bits());
    }

    #[test]
    fn candidate_negative_weight_fails_but_off_candidate_negative_is_ignored() {
        let mut request = request(0, false);
        request.bindings.push(OlsInputBinding {
            stata_var_position: 3,
            role: OlsInputRole::AnalyticWeight,
        });
        let off_candidate = LoadedOlsInput {
            candidate: vec![false, true],
            outcome: column(&[1.0, 2.0], &[false; 2]),
            numeric_sources: Vec::new(),
            analytic_weight: Some(column(&[-1.0, 2.0], &[false; 2])),
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &off_candidate).expect("off-candidate weight");
        assert_eq!(prepared.sample.retained_rows(), &[1]);

        let mut candidate_negative = off_candidate;
        candidate_negative.candidate = vec![true, true];
        let error = prepare_ols_data(&request, &candidate_negative)
            .expect_err("candidate-negative analytic weight");
        assert_eq!(error.code, ErrorCode::NegativeWeight);
    }

    #[test]
    fn analytic_weight_binding_and_loaded_column_must_match_exactly() {
        let mut duplicate = request(0, false);
        duplicate.bindings.extend([
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::AnalyticWeight,
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::AnalyticWeight,
            },
        ]);
        assert_eq!(
            duplicate
                .validate_and_compile()
                .expect_err("duplicate analytic weight")
                .code,
            ErrorCode::InvalidArgument
        );

        let unbound_loaded = LoadedOlsInput {
            candidate: vec![true],
            outcome: column(&[1.0], &[false]),
            numeric_sources: Vec::new(),
            analytic_weight: Some(column(&[1.0], &[false])),
            frequency_weight: None,
            probability_weight: None,
        };
        assert_eq!(
            prepare_ols_data(&request(0, false), &unbound_loaded)
                .expect_err("unbound loaded weight")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn frequency_weights_preserve_positive_integer_bits_and_candidate_scope() {
        let mut request = request(0, false);
        request.bindings.push(OlsInputBinding {
            stata_var_position: 3,
            role: OlsInputRole::FrequencyWeight,
        });
        let maximum = f64::from(u32::MAX);
        let loaded = LoadedOlsInput {
            candidate: vec![true, true, true, true, false],
            outcome: column(&[1.0, 2.0, 3.0, 4.0, 5.0], &[false; 5]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: Some(column(
                &[1.0, 2.0, maximum, f64::NAN, 1.5],
                &[false, false, false, true, false],
            )),
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("frequency-weight prepare");
        assert_eq!(prepared.sample.retained_rows(), &[0, 1, 2]);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::Retained,
                crate::DropReason::Retained,
                crate::DropReason::Retained,
                crate::DropReason::InvalidOrMissingWeight,
                crate::DropReason::ExcludedByIfIn,
            ]
        );
        let crate::data::PreparedWeights::Provided { kind, values } = &prepared.sample.weights
        else {
            panic!("frequency weights must remain present");
        };
        assert_eq!(*kind, WeightKind::Frequency);
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].to_bits(), 1.0_f64.to_bits());
        assert_eq!(values[1].to_bits(), 2.0_f64.to_bits());
        assert_eq!(values[2].to_bits(), maximum.to_bits());
    }

    #[test]
    fn invalid_candidate_frequency_weights_fail_closed() {
        for invalid_weight in [
            0.0,
            -1.0,
            1.5,
            f64::from(u32::MAX) + 1.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            let mut request = request(0, false);
            request.bindings.push(OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::FrequencyWeight,
            });
            let loaded = LoadedOlsInput {
                candidate: vec![true],
                outcome: column(&[1.0], &[false]),
                numeric_sources: Vec::new(),
                analytic_weight: None,
                frequency_weight: Some(column(&[invalid_weight], &[false])),
                probability_weight: None,
            };
            let error = prepare_ols_data(&request, &loaded)
                .expect_err("invalid candidate frequency weight");
            assert_eq!(error.code, ErrorCode::InvalidFrequencyWeight);
        }
    }

    #[test]
    fn frequency_weight_binding_is_unique_and_matches_loaded_input() {
        let mut duplicate = request(0, false);
        duplicate.bindings.extend([
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::FrequencyWeight,
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::FrequencyWeight,
            },
        ]);
        assert_eq!(
            duplicate
                .validate_and_compile()
                .expect_err("duplicate frequency-weight bindings")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut mixed = request(0, false);
        mixed.bindings.extend([
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::AnalyticWeight,
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::FrequencyWeight,
            },
        ]);
        assert_eq!(
            mixed
                .validate_and_compile()
                .expect_err("mixed weight bindings")
                .code,
            ErrorCode::InvalidArgument
        );

        let unbound_loaded = LoadedOlsInput {
            candidate: vec![true],
            outcome: column(&[1.0], &[false]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: Some(column(&[1.0], &[false])),
            probability_weight: None,
        };
        assert_eq!(
            prepare_ols_data(&request(0, false), &unbound_loaded)
                .expect_err("unbound loaded frequency weight")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn probability_weights_preserve_positive_bits_and_candidate_scope() {
        let mut request = request(0, false);
        request.bindings.push(OlsInputBinding {
            stata_var_position: 3,
            role: OlsInputRole::ProbabilityWeight,
        });
        let tiny = f64::from_bits(1);
        let huge = 1.0e300_f64;
        let loaded = LoadedOlsInput {
            candidate: vec![false, true, true, true],
            outcome: column(&[1.0, 2.0, 3.0, 4.0], &[false; 4]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: Some(column(
                &[-1.0, tiny, huge, f64::NAN],
                &[false, false, false, true],
            )),
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("probability-weight prepare");
        assert_eq!(prepared.sample.retained_rows(), &[1, 2]);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::ExcludedByIfIn,
                crate::DropReason::Retained,
                crate::DropReason::Retained,
                crate::DropReason::InvalidOrMissingWeight,
            ]
        );
        let crate::data::PreparedWeights::Provided { kind, values } = &prepared.sample.weights
        else {
            panic!("probability weights must remain present");
        };
        assert_eq!(*kind, WeightKind::Probability);
        assert_eq!(values[0].to_bits(), tiny.to_bits());
        assert_eq!(values[1].to_bits(), huge.to_bits());
    }

    #[test]
    fn invalid_candidate_probability_weights_fail_closed() {
        for invalid_weight in [0.0, -0.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let mut request = request(0, false);
            request.bindings.push(OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::ProbabilityWeight,
            });
            let loaded = LoadedOlsInput {
                candidate: vec![true],
                outcome: column(&[1.0], &[false]),
                numeric_sources: Vec::new(),
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: Some(column(&[invalid_weight], &[false])),
            };
            let error = prepare_ols_data(&request, &loaded)
                .expect_err("invalid candidate probability weight");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn probability_weight_binding_is_unique_and_mutually_exclusive() {
        let mut duplicate = request(0, false);
        duplicate.bindings.extend([
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::ProbabilityWeight,
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::ProbabilityWeight,
            },
        ]);
        assert_eq!(
            duplicate
                .validate_and_compile()
                .expect_err("duplicate probability-weight bindings")
                .code,
            ErrorCode::InvalidArgument
        );

        for other in [OlsInputRole::AnalyticWeight, OlsInputRole::FrequencyWeight] {
            let mut mixed = request(0, false);
            mixed.bindings.extend([
                OlsInputBinding {
                    stata_var_position: 3,
                    role: OlsInputRole::ProbabilityWeight,
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: other,
                },
            ]);
            assert_eq!(
                mixed
                    .validate_and_compile()
                    .expect_err("mixed weight bindings")
                    .code,
                ErrorCode::InvalidArgument
            );
        }

        let unbound_loaded = LoadedOlsInput {
            candidate: vec![true],
            outcome: column(&[1.0], &[false]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: Some(column(&[1.0], &[false])),
        };
        assert_eq!(
            prepare_ols_data(&request(0, false), &unbound_loaded)
                .expect_err("unbound loaded probability weight")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn fixed_effect_preparation_deduplicates_shared_sources_and_preserves_reason_five() {
        let mut request = mixed_numeric_factor_request();
        let h = SourceEntry {
            id: SourceId::new(2).expect("source"),
            canonical_name: "h".to_owned(),
        };
        request.model.sources.push(h.clone());
        request.bindings.push(OlsInputBinding {
            stata_var_position: 4,
            role: OlsInputRole::NumericSource(h.id),
        });
        request.absorbed_terms = vec![
            AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorb term"),
                intercept: true,
                categorical_sources: vec![SourceId::new(1).expect("g")],
                slope_sources: Vec::new(),
                label: "g".to_owned(),
                saved_name: String::new(),
            },
            AbsorbedTerm {
                id: AbsorbTermId::new(2).expect("absorb term"),
                intercept: true,
                categorical_sources: vec![h.id],
                slope_sources: Vec::new(),
                label: "h".to_owned(),
                saved_name: String::new(),
            },
            AbsorbedTerm {
                id: AbsorbTermId::new(3).expect("absorb term"),
                intercept: true,
                categorical_sources: vec![SourceId::new(1).expect("g")],
                slope_sources: Vec::new(),
                label: "g_again".to_owned(),
                saved_name: String::new(),
            },
        ];
        request.keep_singletons = true;
        let loaded = LoadedOlsInput {
            candidate: vec![false, true, true, true, true, true],
            outcome: column(&[0.0, f64::NAN, 2.0, 3.0, 4.0, 5.0], &[false; 6]),
            numeric_sources: vec![
                (
                    SourceId::new(1).expect("g"),
                    column(&[f64::NAN, 1.0, f64::NAN, 2.0, 2.0, 3.0], &[false; 6]),
                ),
                (
                    h.id,
                    column(&[f64::NAN, 1.5, 2.5, f64::NAN, -3.25, 2.5], &[false; 6]),
                ),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("FE preparation");
        assert_eq!(
            prepared,
            prepare_ols_data_owned(&request, loaded).expect("owned FE preparation")
        );
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::ExcludedByIfIn,
                crate::DropReason::MissingOutcome,
                crate::DropReason::MissingRegressor,
                crate::DropReason::MissingFixedEffect,
                crate::DropReason::Retained,
                crate::DropReason::Retained,
            ]
        );
        assert_eq!(prepared.sample.retained_rows(), &[4, 5]);
        assert_eq!(prepared.categorical_sources().len(), 2);
        assert_eq!(prepared.fe_effects().len(), 3);
        assert_eq!(prepared.fe_topology().registry().sources().len(), 2);
        assert_eq!(prepared.fe_topology().nobs(), 2);
        assert_eq!(prepared.fe_topology().registry().total_levels().unwrap(), 4);
        assert_eq!(
            prepared.fe_singleton_policy(),
            PreparedOlsFeSingletonPolicy::KeptByRequest
        );
        assert_eq!(
            prepared.fe_topology().effect_to_topology()[0],
            prepared.fe_topology().effect_to_topology()[2]
        );
        assert_eq!(prepared.fe_effects()[0].label, "g");
        assert_eq!(prepared.fe_effects()[1].label, "h");
        assert_eq!(prepared.fe_effects()[2].label, "g_again");
        prepared
            .validate_fe_bindings()
            .expect("structural certificate");
        let design = prepared
            .borrowed_intercept_fe_design()
            .expect("borrowed FE design");
        assert_eq!(design.nobs(), 2);
        assert_eq!(design.effects().len(), 3);
        assert_eq!(design.total_term_slots(), 6);
        assert!(std::ptr::eq(
            design.effects()[0].codes(),
            prepared.categorical_sources()[0]
                .codes(prepared.fe_topology())
                .unwrap()
        ));
        assert!(std::ptr::eq(
            design.effects()[0].codes(),
            design.effects()[2].codes()
        ));
        assert!(std::ptr::eq(
            design.effects()[0].level_index(),
            design.effects()[2].level_index()
        ));
        assert_eq!(
            prepared.categorical_sources()[1].level_values(),
            vec![-3.25, 2.5]
        );
    }

    fn two_way_singleton_request(keep_singletons: bool) -> OlsRequestV1 {
        let mut request = request(3, true);
        request.absorbed_terms = [(1, 2, "worker"), (2, 3, "firm")]
            .into_iter()
            .map(|(term, source, label)| AbsorbedTerm {
                id: AbsorbTermId::new(term).expect("absorbed term"),
                intercept: true,
                categorical_sources: vec![SourceId::new(source).expect("FE source")],
                slope_sources: Vec::new(),
                label: label.to_owned(),
                saved_name: String::new(),
            })
            .collect();
        request.keep_singletons = keep_singletons;
        request
    }

    fn two_way_singleton_input() -> LoadedOlsInput {
        LoadedOlsInput {
            candidate: vec![true; 9],
            outcome: column(&[1.2, 1.8, 2.4, 3.1, 3.7, 4.0, 4.5, 5.0, 5.6], &[false; 9]),
            numeric_sources: vec![
                (
                    SourceId::new(1).expect("x"),
                    column(
                        &[-1.0, -0.5, 0.0, 0.5, 1.0, -0.8, -0.2, 0.3, 0.9],
                        &[false; 9],
                    ),
                ),
                (
                    SourceId::new(2).expect("worker"),
                    column(&[1.0, 1.0, 2.0, 2.0, 3.0, 4.0, 4.0, 5.0, 5.0], &[false; 9]),
                ),
                (
                    SourceId::new(3).expect("firm"),
                    column(&[1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 5.0, 4.0, 5.0], &[false; 9]),
                ),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        }
    }

    #[test]
    fn two_way_singletons_match_the_sealed_reference_and_compact_every_source() {
        let request = two_way_singleton_request(false);
        let prepared =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("singleton preparation");
        assert_eq!(prepared.singletons.num_singletons, 5);
        assert_eq!(prepared.singletons.cascade_events, 5);
        assert!(!prepared.singletons.kept_by_request);
        assert_eq!(
            prepared.sample.drop_reasons,
            [
                vec![crate::DropReason::Singleton; 5],
                vec![crate::DropReason::Retained; 4],
            ]
            .concat()
        );
        assert_eq!(prepared.sample.retained_rows(), &[5, 6, 7, 8]);
        assert_eq!(prepared.sample.outcome, vec![4.0, 4.5, 5.0, 5.6]);
        assert_eq!(prepared.sample.regressors[0], vec![-0.8, -0.2, 0.3, 0.9]);
        assert_eq!(prepared.sample.regressors[1], vec![4.0, 4.0, 5.0, 5.0]);
        assert_eq!(prepared.sample.regressors[2], vec![4.0, 5.0, 4.0, 5.0]);
        assert_eq!(prepared.categorical_sources().len(), 2);
        assert_eq!(prepared.fe_topology().nobs(), 4);
        assert_eq!(prepared.fe_effects().len(), 2);
        assert_eq!(prepared.fe_topology().registry().sources().len(), 2);
        assert_eq!(prepared.fe_topology().registry().total_levels().unwrap(), 4);
        assert_eq!(
            prepared.fe_singleton_policy(),
            PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
        );
        assert_eq!(
            prepared.fe_topology().registry().sources()[0]
                .members(0)
                .expect("worker level zero"),
            &[0, 1]
        );
        assert_eq!(
            prepared.fe_topology().registry().sources()[1]
                .members(1)
                .expect("firm level one"),
            &[1, 3]
        );
        prepared
            .validate_fe_bindings()
            .expect("post-singleton topology certificate");
        assert_eq!(
            prepared.categorical_sources()[0].level_values(),
            vec![4.0, 5.0]
        );
        assert_eq!(
            prepared.categorical_sources()[0]
                .codes(prepared.fe_topology())
                .unwrap(),
            vec![0, 0, 1, 1]
        );
        assert_eq!(
            prepared.categorical_sources()[1].level_values(),
            vec![4.0, 5.0]
        );
        assert_eq!(
            prepared.categorical_sources()[1]
                .codes(prepared.fe_topology())
                .unwrap(),
            vec![0, 1, 0, 1]
        );
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(0, row).expect("aligned x"))
                .collect::<Vec<_>>(),
            vec![-0.8, -0.2, 0.3, 0.9]
        );
    }

    #[test]
    fn singleton_fixture_omits_additive_slope_before_residual_dof() {
        for keep_singletons in [false, true] {
            let mut request = two_way_singleton_request(keep_singletons);
            request.model.terms.truncate(1);
            let mut loaded = two_way_singleton_input();
            for value in &mut loaded.outcome.values {
                *value = (*value as f32) as f64;
            }
            for (_, column) in &mut loaded.numeric_sources {
                for value in &mut column.values {
                    *value = (*value as f32) as f64;
                }
            }
            let prepared = prepare_ols_data(&request, &loaded).expect("singleton preparation");
            let inference = super::super::fit_prepared_ols_unadjusted_map(
                &prepared,
                super::super::PreparedOlsUnadjustedConfig::default(),
            )
            .expect("rank-zero singleton inference");
            assert_eq!(inference.fit().finite_rank(), 0);
            // The FE-collinear slope has no finite score coordinate, while the
            // deferred reported constant retains its raw score-mutation check.
            assert_eq!(inference.fit().score_certificate().moments, 1);
            assert_eq!(
                inference.fit().coefficients()[0].status,
                super::super::PreparedOlsFitStatus::CollinearWithFixedEffects
            );
            assert_eq!(inference.dof().finite_model_dof, 0);
            assert_eq!(
                inference.dof().absorbed_rank,
                if keep_singletons { 8 } else { 3 }
            );
            assert_eq!(inference.dof().residual_dof, 1);
            assert!(inference.statistics().f_statistic.is_none());
            assert_eq!(inference.covariance().dimension(), 1);
            assert_eq!(inference.fit().bread().coordinates().len(), 1);
            assert_eq!(
                inference.fit().bread().values(),
                &[1.0 / prepared.sample.n_active() as f64]
            );
            assert_eq!(
                prepared.singletons.num_singletons,
                if keep_singletons { 0 } else { 5 }
            );
            if !keep_singletons {
                assert!(
                    (inference.fit().coefficients()[1]
                        .value
                        .expect("normalized intercept")
                        - 4.774_999_976_158_142)
                        .abs()
                        <= 1e-15
                );
                assert!((inference.statistics().rss - 0.002_499_995_231_630_682_4).abs() <= 2e-15);
                assert!(
                    (inference.covariance().values()[0] - 0.000_624_998_807_907_670_6).abs()
                        <= 5e-16
                );
            }
        }
    }

    #[test]
    fn keepsingletons_and_aweights_preserve_reference_identities() {
        let kept = prepare_ols_data(&two_way_singleton_request(true), &two_way_singleton_input())
            .expect("keep singletons");
        assert_eq!(kept.sample.n_active(), 9);
        assert_eq!(kept.singletons.num_singletons, 0);
        assert!(kept.singletons.kept_by_request);
        assert_eq!(
            kept.fe_singleton_policy(),
            PreparedOlsFeSingletonPolicy::KeptByRequest
        );
        assert!(kept
            .sample
            .drop_reasons
            .iter()
            .all(|reason| *reason == crate::DropReason::Retained));

        let mut request = two_way_singleton_request(false);
        request.bindings.push(OlsInputBinding {
            stata_var_position: 6,
            role: OlsInputRole::AnalyticWeight,
        });
        let mut loaded = two_way_singleton_input();
        loaded.analytic_weight = Some(column(
            &[
                2020.0, 2021.0, 2020.0, 2021.0, 2020.0, 2020.0, 2021.0, 2020.0, 2021.0,
            ],
            &[false; 9],
        ));
        let weighted = prepare_ols_data(&request, &loaded).expect("weighted singleton preparation");
        assert_eq!(weighted.sample.retained_rows(), &[5, 6, 7, 8]);
        assert_eq!(weighted.singletons.num_singletons, 5);
        assert_eq!(
            weighted.fe_singleton_policy(),
            PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
        );
        assert_eq!(
            weighted.sample.weights,
            PreparedWeights::Provided {
                kind: WeightKind::Analytic,
                values: vec![2020.0, 2021.0, 2020.0, 2021.0],
            }
        );

        let mut probability_request = two_way_singleton_request(false);
        probability_request.bindings.push(OlsInputBinding {
            stata_var_position: 6,
            role: OlsInputRole::ProbabilityWeight,
        });
        let mut probability_input = two_way_singleton_input();
        probability_input.probability_weight = Some(column(&[0.5; 9], &[false; 9]));
        let probability = prepare_ols_data(&probability_request, &probability_input)
            .expect("probability-weighted singleton preparation");
        assert_eq!(probability.sample.retained_rows(), &[5, 6, 7, 8]);
        assert_eq!(probability.singletons.num_singletons, 5);
        assert_eq!(
            probability.fe_singleton_policy(),
            PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
        );
    }

    #[test]
    fn shared_factor_preparation_preserves_owned_sample_codes_and_fallback() {
        use crate::{ThreadConfig, ThreadTeam};
        for width in [7, 8, 16, 17] {
            for n in [16383, 16384, 32769] {
                let mut request = request(width, false);
                request.absorbed_terms = vec![AbsorbedTerm {
                    id: AbsorbTermId::new(1).unwrap(),
                    intercept: true,
                    categorical_sources: vec![SourceId::new(width as u32).unwrap()],
                    slope_sources: Vec::new(),
                    label: "g".into(),
                    saved_name: String::new(),
                }];
                let input = LoadedOlsInput {
                    candidate: vec![true; n],
                    outcome: column(&vec![1.0; n], &vec![false; n]),
                    numeric_sources: (1..=width as u32)
                        .map(|j| {
                            (
                                SourceId::new(j).unwrap(),
                                column(
                                    &(0..n)
                                        .map(|r| {
                                            if j == width as u32 {
                                                (r / 5) as f64 - 0.25
                                            } else {
                                                ((r * j as usize) as f64).sin()
                                            }
                                        })
                                        .collect::<Vec<_>>(),
                                    &vec![false; n],
                                ),
                            )
                        })
                        .collect(),
                    analytic_weight: None,
                    frequency_weight: None,
                    probability_weight: None,
                };
                for case in 0..6 {
                    let mut input = input.clone();
                    match case {
                        1 => input.candidate[0] = false,
                        2 => input.outcome.missing[1] = true,
                        3 => input.numeric_sources[width - 1].1.values[1] = f64::NAN,
                        4 => input.numeric_sources[width - 1].1.values[n / 2] = 1e12,
                        5 => input.numeric_sources[width - 1].1.values.reverse(),
                        _ => (),
                    }
                    let run = |threads| {
                        ThreadTeam::new(ThreadConfig::from_requested(threads, false).unwrap())
                            .install(|| prepare_ols_data_owned(&request, input.clone()))
                            .unwrap()
                    };
                    let expected = run(1).expect("serial prepared fixture");
                    for workers in [4, 8, 16] {
                        assert_eq!(
                            expected,
                            run(workers).expect("shared prepared fixture"),
                            "N={n}, width={width}, workers={workers}, case={case}"
                        );
                    }
                }
            }
        }
    }

    fn classified_slope_role_fixture() -> (OlsRequestV1, LoadedOlsInput) {
        let first = SourceId::new(1).unwrap();
        let factor = SourceId::new(2).unwrap();
        let partition = SourceId::new(3).unwrap();
        let mut request = mixed_numeric_factor_request();
        let full = self::request(3, true);
        request.model.sources = full.model.sources;
        request.bindings = full.bindings;
        for term in &mut request.model.terms {
            for component in &mut term.components {
                component.source = factor;
            }
        }
        request.absorbed_terms = vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).unwrap(),
            intercept: true,
            categorical_sources: vec![partition],
            slope_sources: vec![first, factor],
            label: "g##c.(z f)".into(),
            saved_name: String::new(),
        }];
        let mut interaction = request.model.terms[2].clone();
        interaction.id = TermId::new(6).unwrap();
        interaction.order = TermOrder::new(6).unwrap();
        interaction.coefficient_slot = CoefficientSlot::new(6).unwrap();
        interaction.raw_kind = RawTermKind::Interaction;
        interaction.canonical_token = "1.f#0b.g".into();
        interaction.whole_column_omitted = true;
        let mut base = categorical_component(2, partition, 0.0);
        base.base = MetadataFlag::True;
        base.omitted = MetadataFlag::True;
        base.raw_operator = "0b".into();
        interaction.components = vec![categorical_component(1, factor, 1.0), base];
        let mut kept = interaction.clone();
        kept.id = TermId::new(7).unwrap();
        kept.order = TermOrder::new(7).unwrap();
        kept.coefficient_slot = CoefficientSlot::new(7).unwrap();
        kept.canonical_token = "1.f#1.g".into();
        kept.whole_column_omitted = false;
        kept.components[1] = categorical_component(2, partition, 1.0);
        request.model.terms.extend([interaction, kept]);
        let n = 97;
        let mut input = LoadedOlsInput {
            candidate: vec![true; n],
            outcome: column(&vec![1.0; n], &vec![false; n]),
            numeric_sources: vec![
                (
                    first,
                    column(
                        &(0..n).map(|r| (r as f64).sin()).collect::<Vec<_>>(),
                        &vec![false; n],
                    ),
                ),
                (
                    factor,
                    column(
                        &(0..n).map(|r| (r % 3) as f64).collect::<Vec<_>>(),
                        &vec![false; n],
                    ),
                ),
                (
                    partition,
                    column(
                        &(0..n).map(|r| (r / 5) as f64).collect::<Vec<_>>(),
                        &vec![false; n],
                    ),
                ),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        input.candidate[0] = false;
        input.numeric_sources[1].1.values[0] = -1.5;
        input.numeric_sources[0].1.missing[3] = true;
        (request, input)
    }

    #[test]
    fn classified_slope_roles_preserve_factors_clusters_and_missing_reasons() {
        let first = SourceId::new(1).unwrap();
        let factor = SourceId::new(2).unwrap();
        let partition = SourceId::new(3).unwrap();
        let (request, input) = classified_slope_role_fixture();
        for cluster_first in [false, true] {
            let clusters = if cluster_first {
                vec![first, factor]
            } else {
                vec![factor]
            };
            let _plan = crate::fe::plan_categorical_absorbed_request(&request).unwrap();
            let (model, sample) =
                prepare_categorical_numeric_sample(&request, &input, &clusters).unwrap();
            assert_eq!(sample.drop_reasons[0], crate::DropReason::ExcludedByIfIn);
            assert_eq!(
                sample.drop_reasons[3],
                crate::DropReason::MissingFixedEffect
            );
            let sources = factorize_preparation_sources(&request, &sample).unwrap();
            assert_eq!(
                sources
                    .iter()
                    .map(|source| source.source)
                    .collect::<Vec<_>>(),
                vec![factor, partition]
            );
            let available_sample = sample;
            assert!(!model.columns.is_empty());
            let prepared = crate::prepared_categorical_data::prepare_categorical_ols_data_with_cluster_sources(
                &request, &input, &clusters).unwrap();
            assert_eq!(prepared.sample(), &available_sample);
            let interaction = prepared
                .model()
                .columns
                .iter()
                .position(|column| {
                    matches!(
                        column.kind,
                        PreparedOlsColumnKind::CategoricalInteraction { .. }
                    )
                })
                .unwrap();
            for row in 0..prepared.sample().n_active() {
                let original = prepared.sample().retained_rows()[row];
                assert_eq!(
                    prepared.column_value(0, row).unwrap(),
                    (original % 3) as f64
                );
                assert_eq!(
                    prepared.column_value(interaction, row).unwrap(),
                    f64::from(original % 3 == 1 && original / 5 == 1)
                );
            }
        }
    }

    #[test]
    fn categorical_availability_retains_only_consumable_membership() {
        for n in [97, 16384] {
            let mut request = request(1, false);
            request.keep_singletons = true;
            request.absorbed_terms = vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![SourceId::new(1).unwrap()],
                slope_sources: Vec::new(),
                label: "group".into(),
                saved_name: String::new(),
            }];
            let loaded = LoadedOlsInput {
                candidate: vec![true; n],
                outcome: column(&vec![1.0; n], &vec![false; n]),
                numeric_sources: vec![(
                    SourceId::new(1).unwrap(),
                    column(
                        &(0..n).map(|r| (r / 5) as f64).collect::<Vec<_>>(),
                        &vec![false; n],
                    ),
                )],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            };
            let expected = prepare_ols_data(&request, &loaded).unwrap();
            for reusable in [false, true] {
                let (model, sample) = prepare_ols_numeric_sample(&request, &loaded, &[]).unwrap();
                let available = crate::prepared_categorical_data::finish_prepared_model(
                    &request,
                    &[],
                    model,
                    sample,
                    false,
                    false,
                    reusable,
                )
                .unwrap();
                assert_eq!(available, expected);
            }
        }
    }

    #[test]
    fn one_fe_membership_reuse_matches_generic_fixed_point_and_sample_ledger() {
        for n in [16_383, 16_384, 32_769] {
            for shuffled in [false, true] {
                for singleton in [false, true] {
                    let mut request = request(1, false);
                    request.absorbed_terms = vec![AbsorbedTerm {
                        id: AbsorbTermId::new(1).unwrap(),
                        intercept: true,
                        categorical_sources: vec![SourceId::new(1).unwrap()],
                        slope_sources: Vec::new(),
                        label: "group".into(),
                        saved_name: String::new(),
                    }];
                    let mut codes: Vec<_> = (0..n)
                        .map(|r| {
                            let row = if shuffled { (r * 7919) % n } else { r };
                            (row / 5) as f64 - 0.25
                        })
                        .collect();
                    if singleton {
                        codes[n / 2] = 1e12;
                    }
                    let mut loaded = LoadedOlsInput {
                        candidate: vec![true; n],
                        outcome: column(
                            &(0..n).map(|r| (r % 97) as f64).collect::<Vec<_>>(),
                            &vec![false; n],
                        ),
                        numeric_sources: vec![(
                            SourceId::new(1).unwrap(),
                            column(&codes, &vec![false; n]),
                        )],
                        analytic_weight: None,
                        frequency_weight: None,
                        probability_weight: None,
                    };
                    for exclusions in [false, true] {
                        if exclusions {
                            loaded.candidate[0] = false;
                            loaded.outcome.missing[1] = true;
                        }
                        let finish = |reuse| {
                            let sample = if is_full_retained_unweighted_sample(&loaded) {
                                take_full_retained_unweighted_sample(loaded.clone()).unwrap()
                            } else {
                                take_classified_sample(&request, loaded.clone(), &[]).unwrap()
                            };
                            let model = request
                                .validate_and_compile_with_cluster_sources(&[])
                                .unwrap();
                            finish_prepare_ols_data_with_membership(
                                &request,
                                &[],
                                model,
                                sample,
                                reuse,
                            )
                            .unwrap()
                        };
                        let expected = finish(false);
                        let actual = finish(true);
                        assert!(expected == actual, "N={n}, shuffled={shuffled}, singleton={singleton}, exclusions={exclusions}");
                    }
                }
            }
        }
    }

    #[test]
    fn one_fe_and_multiway_cluster_reuse_preserves_source_order_weights_and_sample_changes() {
        for effect_count in [1, 2] {
            for weight in [
                None,
                Some(WeightKind::Analytic),
                Some(WeightKind::Frequency),
                Some(WeightKind::Probability),
            ] {
                for cluster_ids in [vec![2], vec![1], vec![9], vec![1, 9]] {
                    for (changed_sample, keep_singletons) in
                        [(false, false), (true, false), (false, true), (true, true)]
                    {
                        let n = 16_387;
                        let mut request = request(9, true);
                        request.keep_singletons = keep_singletons;
                        request.absorbed_terms = vec![AbsorbedTerm {
                            id: AbsorbTermId::new(1).unwrap(),
                            intercept: true,
                            categorical_sources: vec![SourceId::new(2).unwrap()],
                            slope_sources: Vec::new(),
                            label: "group".into(),
                            saved_name: String::new(),
                        }];
                        if effect_count == 2 {
                            request.absorbed_terms.push(AbsorbedTerm {
                                id: AbsorbTermId::new(2).unwrap(),
                                intercept: true,
                                categorical_sources: vec![SourceId::new(3).unwrap()],
                                slope_sources: Vec::new(),
                                label: "second group".into(),
                                saved_name: String::new(),
                            });
                        }
                        let clusters: Vec<_> = cluster_ids
                            .iter()
                            .copied()
                            .map(|id| SourceId::new(id).unwrap())
                            .collect();
                        let mut loaded = LoadedOlsInput {
                            candidate: vec![true; n],
                            outcome: column(
                                &(0..n).map(|r| (r % 97) as f64).collect::<Vec<_>>(),
                                &vec![false; n],
                            ),
                            numeric_sources: (1..=9)
                                .map(|id| {
                                    let values: Vec<_> = (0..n)
                                        .map(|r| {
                                            if id == 2 {
                                                (r / 5) as f64 - 0.25
                                            } else {
                                                (r % (id as usize * 17)) as f64
                                            }
                                        })
                                        .collect();
                                    (SourceId::new(id).unwrap(), column(&values, &vec![false; n]))
                                })
                                .collect(),
                            analytic_weight: None,
                            frequency_weight: None,
                            probability_weight: None,
                        };
                        if let Some(kind) = weight {
                            let values = column(
                                &(0..n).map(|r| (r % 3 + 1) as f64).collect::<Vec<_>>(),
                                &vec![false; n],
                            );
                            let role = match kind {
                                WeightKind::Analytic => {
                                    loaded.analytic_weight = Some(values);
                                    OlsInputRole::AnalyticWeight
                                }
                                WeightKind::Frequency => {
                                    loaded.frequency_weight = Some(values);
                                    OlsInputRole::FrequencyWeight
                                }
                                WeightKind::Probability => {
                                    loaded.probability_weight = Some(values);
                                    OlsInputRole::ProbabilityWeight
                                }
                                WeightKind::Importance => {
                                    unreachable!("fixture selects supported OLS weights")
                                }
                            };
                            request.bindings.push(OlsInputBinding {
                                stata_var_position: 12,
                                role,
                            });
                        }
                        if changed_sample {
                            loaded.candidate[0] = false;
                            loaded.outcome.missing[1] = true;
                            loaded.numeric_sources[0].1.missing[3] = true;
                            loaded.numeric_sources[1].1.values[10] = 1e12;
                        }
                        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(16, true).unwrap()).install(|| {
                        let sample = if is_full_retained_unweighted_sample(&loaded) {
                            take_full_retained_unweighted_sample(loaded.clone()).unwrap()
                        } else { take_classified_sample(&request, loaded.clone(), &clusters).unwrap() };
                        let model = request.validate_and_compile_with_cluster_sources(&clusters).unwrap();
                        let expected = finish_prepare_ols_data_with_membership(&request, &clusters, model, sample, false).unwrap();
                        let actual = prepare_ols_data_owned_with_cluster_sources(&request, loaded, &clusters).unwrap();
                        assert_eq!(actual, expected, "effects={effect_count}, weight={weight:?}, clusters={clusters:?}, changed={changed_sample}, keepsingletons={keep_singletons}");
                    }).unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn frequency_multiplicity_and_prior_drop_reasons_are_preserved() {
        let mut request = request(1, false);
        request.absorbed_terms = vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("term"),
            intercept: true,
            categorical_sources: vec![SourceId::new(1).expect("source")],
            slope_sources: Vec::new(),
            label: "group".to_owned(),
            saved_name: String::new(),
        }];
        request.bindings.push(OlsInputBinding {
            stata_var_position: 4,
            role: OlsInputRole::FrequencyWeight,
        });
        let loaded = LoadedOlsInput {
            candidate: vec![true, true],
            outcome: column(&[1.0, 2.0], &[false; 2]),
            numeric_sources: vec![(
                SourceId::new(1).expect("source"),
                column(&[10.0, 20.0], &[false; 2]),
            )],
            analytic_weight: None,
            frequency_weight: Some(column(&[2.0, 1.0], &[false; 2])),
            probability_weight: None,
        };
        let mut weighted = prepare_ols_data(&request, &loaded).expect("frequency singleton");
        assert_eq!(
            weighted,
            prepare_ols_data_owned(&request, loaded.clone()).unwrap()
        );
        assert_eq!(weighted.sample.retained_rows(), &[0]);
        assert_eq!(
            weighted.sample.drop_reasons,
            vec![crate::DropReason::Retained, crate::DropReason::Singleton]
        );
        assert_eq!(weighted.categorical_sources()[0].level_values(), vec![10.0]);
        assert_eq!(weighted.singletons.num_singletons, 1);
        assert_eq!(weighted.fe_topology().registry().total_levels().unwrap(), 1);
        assert_eq!(
            weighted.fe_singleton_policy(),
            PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
        );
        weighted.singletons.kept_by_request = true;
        assert_eq!(
            weighted.fe_singleton_policy(),
            PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
        );

        let mut precedence = loaded.clone();
        precedence.outcome.values[1] = f64::NAN;
        let prepared = prepare_ols_data(&request, &precedence).expect("reason precedence");
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::Retained,
                crate::DropReason::MissingOutcome
            ]
        );
        assert_eq!(prepared.singletons.num_singletons, 0);
    }

    #[test]
    fn singleton_compaction_rebuilds_shared_ordinary_factor_recipes() {
        let mut request = mixed_numeric_factor_request();
        request.absorbed_terms = vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("term"),
            intercept: true,
            categorical_sources: vec![SourceId::new(1).expect("source")],
            slope_sources: Vec::new(),
            label: "g".to_owned(),
            saved_name: String::new(),
        }];
        let loaded = LoadedOlsInput {
            candidate: vec![true; 3],
            outcome: column(&[1.0, 2.0, 3.0], &[false; 3]),
            numeric_sources: vec![(
                SourceId::new(1).expect("source"),
                column(&[1.0, 2.0, 2.0], &[false; 3]),
            )],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("shared factor and FE");

        assert_eq!(prepared.sample.retained_rows(), &[1, 2]);
        assert_eq!(prepared.categorical_sources()[0].level_values(), vec![2.0]);
        assert_eq!(
            prepared.categorical_sources()[0]
                .codes(prepared.fe_topology())
                .unwrap(),
            vec![0, 0]
        );
        assert_eq!(prepared.fe_topology().registry().sources().len(), 1);
        assert_eq!(
            prepared
                .categorical_sources()
                .iter()
                .position(|s| s.source() == prepared.fe_effects()[0].sources[0])
                .unwrap(),
            prepared.fe_topology().compacted_sources()[prepared.fe_topology().registry().sources()
                [0]
            .partition()
            .categorical_source_indices()[0]]
                .candidate_source_index()
        );
        assert_eq!(
            prepared.fe_topology().registry().sources()[0]
                .members(0)
                .expect("shared final level"),
            &[0, 1]
        );
        let PreparedOlsColumnKind::Categorical {
            level_code: vanished,
            ..
        } = &prepared.model.columns[1].kind
        else {
            panic!("level-one recipe must remain categorical");
        };
        assert_eq!(*vanished, None);
        let PreparedOlsColumnKind::Categorical {
            level_code: retained,
            ..
        } = &prepared.model.columns[2].kind
        else {
            panic!("level-two recipe must remain categorical");
        };
        assert_eq!(*retained, Some(0));
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(1, row).expect("vanished recipe"))
                .collect::<Vec<_>>(),
            vec![0.0, 0.0]
        );
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(2, row).expect("retained recipe"))
                .collect::<Vec<_>>(),
            vec![1.0, 1.0]
        );
    }

    #[test]
    fn all_singletons_remain_a_diagnostic_preparation_result() {
        let mut request = request(1, false);
        request.absorbed_terms = vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("term"),
            intercept: true,
            categorical_sources: vec![SourceId::new(1).expect("source")],
            slope_sources: Vec::new(),
            label: "group".to_owned(),
            saved_name: String::new(),
        }];
        let loaded = LoadedOlsInput {
            candidate: vec![true, true],
            outcome: column(&[1.0, 2.0], &[false; 2]),
            numeric_sources: vec![(
                SourceId::new(1).expect("source"),
                column(&[10.0, 20.0], &[false; 2]),
            )],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("all singleton preparation");
        assert_eq!(prepared.sample.n_active(), 0);
        assert_eq!(prepared.singletons.num_singletons, 2);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![crate::DropReason::Singleton; 2]
        );
        assert!(prepared.categorical_sources()[0].level_values().is_empty());
        assert!(prepared.categorical_sources()[0]
            .codes(prepared.fe_topology())
            .unwrap()
            .is_empty());
        assert_eq!(prepared.fe_topology().nobs(), 0);
        assert_eq!(prepared.fe_effects().len(), 1);
        assert_eq!(prepared.fe_topology().registry().sources().len(), 1);
        assert_eq!(prepared.fe_topology().registry().sources()[0].n_levels(), 0);
        assert_eq!(prepared.fe_topology().registry().sources()[0].nobs(), 0);
        assert_eq!(prepared.fe_topology().registry().total_levels().unwrap(), 0);
        let design = prepared
            .borrowed_intercept_fe_design()
            .expect("empty borrowed FE design");
        assert_eq!(design.nobs(), 0);
        assert_eq!(design.effects().len(), 1);
        assert_eq!(design.effects()[0].n_levels(), 0);
        assert_eq!(design.total_term_slots(), 0);
    }

    #[test]
    fn borrowed_fe_design_uses_final_singleton_axis_and_existing_membership() {
        let request = two_way_singleton_request(false);
        let prepared =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared topology");
        let design = prepared
            .borrowed_intercept_fe_design()
            .expect("borrowed FE design");
        assert_eq!(design.nobs(), 4);
        assert_eq!(design.effects().len(), 2);
        assert_eq!(design.total_term_slots(), 4);
        for (effect_index, effect) in design.effects().iter().enumerate() {
            let topology_source = &prepared.fe_topology().registry().sources()
                [prepared.fe_topology().effect_to_topology()[effect_index]];
            let categorical = &prepared.categorical_sources()[prepared
                .categorical_sources()
                .iter()
                .position(|s| s.source() == prepared.fe_effects()[effect_index].sources[0])
                .unwrap()];
            assert!(std::ptr::eq(
                effect.codes(),
                categorical.codes(prepared.fe_topology()).unwrap()
            ));
            assert!(std::ptr::eq(
                effect.level_index(),
                topology_source.level_index()
            ));
            for level in 0..effect.n_levels() {
                assert_eq!(
                    effect
                        .level_index()
                        .checked_members(level)
                        .expect("borrowed members"),
                    topology_source.members(level).expect("prepared members")
                );
            }
        }
    }

    #[test]
    fn borrowed_fe_design_reservation_and_capacity_guard_fail_closed() {
        let request = two_way_singleton_request(true);
        let prepared =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared topology");

        let mut successful = InjectedFeTopologyReservation::default();
        build_borrowed_intercept_fe_design_with_reservation(&prepared, &mut successful)
            .expect("record adapter reservations");
        assert_eq!(successful.calls, 1);

        for fail_at in 0..successful.calls {
            let mut failed = InjectedFeTopologyReservation {
                fail_at: Some(fail_at),
                ..InjectedFeTopologyReservation::default()
            };
            let error = build_borrowed_intercept_fe_design_with_reservation(&prepared, &mut failed)
                .expect_err("adapter reservation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        {
            let skipped = "unable to reserve borrowed FE design effects";
            let mut undersized = InjectedFeTopologyReservation {
                skip_label: Some(skipped),
                ..InjectedFeTopologyReservation::default()
            };
            let error =
                build_borrowed_intercept_fe_design_with_reservation(&prepared, &mut undersized)
                    .expect_err("adapter capacity guard must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "{skipped}");
        }
    }

    #[test]
    fn shared_partition_pruning_preserves_duplicate_order_and_frequency_decisions() {
        for order in [vec![0, 1], vec![1, 0], vec![0, 1, 0], vec![1, 1, 0, 1]] {
            for frequency in [false, true] {
                let mut request = two_way_singleton_request(false);
                let original = request.absorbed_terms.clone();
                request.absorbed_terms = order
                    .iter()
                    .enumerate()
                    .map(|(index, &position)| {
                        let mut term = original[position].clone();
                        term.id = AbsorbTermId::new((index + 1) as u32).unwrap();
                        term
                    })
                    .collect();
                let mut loaded = two_way_singleton_input();
                if frequency {
                    request.bindings.push(OlsInputBinding {
                        stata_var_position: u32::try_from(request.bindings.len() + 1).unwrap(),
                        role: OlsInputRole::FrequencyWeight,
                    });
                    let mut values = vec![1.; loaded.candidate.len()];
                    values[0] = 2.;
                    loaded.frequency_weight = Some(column(&values, &vec![false; values.len()]));
                }
                let (_, sample) = prepare_ols_numeric_sample(&request, &loaded, &[]).unwrap();
                let encoded: Vec<_> = request
                    .absorbed_terms
                    .iter()
                    .map(|term| {
                        factorize_f64(
                            &sample.regressors[(term.categorical_sources[0].get() - 1) as usize],
                        )
                        .unwrap()
                    })
                    .collect();
                let codes: Vec<_> = encoded
                    .iter()
                    .map(|source| source.codes.as_slice())
                    .collect();
                let levels: Vec<_> = encoded.iter().map(EncodedFactor::n_levels).collect();
                let multiplicity = match &sample.weights {
                    PreparedWeights::Provided {
                        kind: WeightKind::Frequency,
                        values,
                    } => crate::fe::SingletonMultiplicity::Frequency(values),
                    _ => crate::fe::SingletonMultiplicity::Unit,
                };
                let expected = crate::fe::prune_singletons_with_multiplicity(
                    &codes,
                    &levels,
                    &vec![true; sample.n_active()],
                    multiplicity,
                    false,
                )
                .unwrap();
                let dropped = expected.dropped_observations.len();
                let cascade = expected.cascade_events;
                let expected_sample = sample
                    .compact_active_rows(&expected.active, DropReason::Singleton)
                    .unwrap();
                let prepared = prepare_ols_data(&request, &loaded).unwrap();
                assert_eq!(prepared.sample(), &expected_sample, "{order:?}/{frequency}");
                assert_eq!(prepared.singletons.num_singletons, dropped);
                assert_eq!(prepared.singletons.cascade_events, cascade);
            }
        }
    }

    #[test]
    fn repeated_fe_terms_borrow_the_immutable_unique_source() {
        const TERM_COUNT: usize = 128;
        let source = SourceId::new(1).expect("source");
        let mut request = request(1, false);
        request.absorbed_terms = (1..=TERM_COUNT)
            .map(|term| AbsorbedTerm {
                id: AbsorbTermId::new(u32::try_from(term).expect("small term")).expect("term"),
                intercept: true,
                categorical_sources: vec![source],
                slope_sources: Vec::new(),
                label: format!("g_{term}"),
                saved_name: String::new(),
            })
            .collect();
        request.keep_singletons = true;
        let loaded = LoadedOlsInput {
            candidate: vec![true; 6],
            outcome: column(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[false; 6]),
            numeric_sources: vec![(source, column(&[1.0, 1.0, 2.0, 2.0, 3.0, 3.0], &[false; 6]))],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("repeated FE preparation");
        assert_eq!(prepared.fe_topology().registry().sources().len(), 1);
        assert_eq!(prepared.fe_effects().len(), TERM_COUNT);

        let mut reservation = SystemFeTopologyReservation;
        let design =
            build_borrowed_intercept_fe_design_with_reservation(&prepared, &mut reservation)
                .expect("borrowed repeated FE design");
        assert_eq!(design.effects().len(), TERM_COUNT);
        assert_eq!(design.total_term_slots(), TERM_COUNT * 3);
        for effect in design.effects() {
            assert!(std::ptr::eq(
                effect.codes(),
                prepared.categorical_sources()[0]
                    .codes(prepared.fe_topology())
                    .unwrap()
            ));
            assert!(std::ptr::eq(
                effect.level_index(),
                prepared.fe_topology().registry().sources()[0].level_index()
            ));
        }
    }

    #[test]
    fn borrowed_fe_design_rejects_changed_outer_bindings_without_panicking() {
        let request = two_way_singleton_request(false);
        for fault in 0..4 {
            let mut prepared = prepare_ols_data(&request, &two_way_singleton_input()).unwrap();
            match fault {
                0 => prepared.effect_specs[0].intercept = false,
                1 => {
                    prepared.sample.original_row_by_active.pop();
                }
                2 => prepared.effect_specs[0].sources[0] = SourceId::new(999).unwrap(),
                _ => {
                    let source = prepared.categorical_sources[prepared
                        .categorical_sources()
                        .iter()
                        .position(|s| s.source() == prepared.fe_effects()[0].sources[0])
                        .unwrap()]
                    .prepared_source_index();
                    prepared.sample.regressors[source].pop();
                }
            }
            assert_eq!(
                prepared
                    .borrowed_intercept_fe_design()
                    .expect_err(&format!("fault {fault}"))
                    .code,
                ErrorCode::InvalidArgument,
                "fault {fault}"
            );
        }
    }

    #[test]
    fn final_owner_metadata_reservations_and_capacity_guards_fail_closed() {
        let request = two_way_singleton_request(true);
        let metadata = |reservation: &mut InjectedFeTopologyReservation| {
            let prepared = prepare_ols_data(&request, &two_way_singleton_input()).unwrap();
            let sources = factorize_preparation_sources(&request, prepared.sample()).unwrap();
            let required: Vec<_> = sources.iter().map(|source| source.source).collect();
            crate::prepared_categorical_data::retain_model_sources_with_reservation(
                sources,
                &required,
                prepared.fe_topology(),
                true,
                reservation,
            )?;
            crate::prepared_categorical_data::effect_specs_with_reservation(
                &request.absorbed_terms,
                &request.model,
                false,
                reservation,
            )
        };
        let mut successful = InjectedFeTopologyReservation::default();
        metadata(&mut successful).unwrap();
        assert_eq!(successful.calls, 6);
        for fail_at in 0..successful.calls {
            let mut failed = InjectedFeTopologyReservation {
                fail_at: Some(fail_at),
                ..Default::default()
            };
            assert_eq!(
                metadata(&mut failed).unwrap_err().code,
                ErrorCode::Allocation,
                "call {fail_at}"
            );
        }
        for label in [
            "unable to reserve categorical absorbed DoF metadata",
            "unable to reserve final categorical model sources",
        ] {
            let mut failed = InjectedFeTopologyReservation {
                skip_label: Some(label),
                ..Default::default()
            };
            assert_eq!(
                metadata(&mut failed).unwrap_err().code,
                ErrorCode::Allocation,
                "{label}"
            );
        }
    }

    #[test]
    fn legacy_compile_rejects_unsupported_categorical_shapes_before_loading() {
        let source = SourceId::new(1).expect("source");
        for term in [
            AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("term"),
                intercept: true,
                categorical_sources: vec![source, source],
                slope_sources: Vec::new(),
                label: "g#g".to_owned(),
                saved_name: String::new(),
            },
            AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("term"),
                intercept: false,
                categorical_sources: vec![source],
                slope_sources: vec![source],
                label: "g#c.g".to_owned(),
                saved_name: String::new(),
            },
        ] {
            for include_intercept in [true, false] {
                let mut request = mixed_numeric_factor_request();
                request.include_intercept = include_intercept;
                request.absorbed_terms = vec![term.clone()];
                assert_eq!(
                    request
                        .validate_and_compile()
                        .expect_err("legacy compile rejects unsupported FE shape")
                        .code,
                    ErrorCode::Unsupported
                );
            }
        }
    }

    #[test]
    fn saved_single_source_categorical_effect_compiles_before_loading() {
        let source = SourceId::new(1).expect("source");
        for include_intercept in [true, false] {
            let mut request = mixed_numeric_factor_request();
            request.include_intercept = include_intercept;
            request.absorbed_terms = vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("term"),
                intercept: true,
                categorical_sources: vec![source],
                slope_sources: Vec::new(),
                label: "saved=g".to_owned(),
                saved_name: "saved".to_owned(),
            }];
            let compiled = request
                .validate_and_compile()
                .expect("saved categorical FE shape compiles before loading");
            assert_eq!(
                compiled.coefficients.len(),
                if include_intercept { 6 } else { 5 }
            );
        }
    }
}
