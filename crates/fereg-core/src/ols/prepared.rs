use std::collections::{BTreeMap, BTreeSet};

use crate::data::{
    prepare_numeric_sample_with_source_uses, AdjustmentInput, DomainDecision, NumericColumnView,
    NumericDomainPolicy, NumericPreparationInput, NumericSourceUse, PreparedNumericSample,
    PreparedWeights, WeightInput, WeightKind,
};
use crate::error::{ErrorCode, FeregError, Result};
use crate::factorize::{factorize_f64, EncodedFactor};
use crate::fe::{
    prune_singletons_with_multiplicity, BorrowedInterceptFeDesign, LevelIndex,
    SingletonMultiplicity, SingletonResult, ValidatedBorrowedInterceptSource,
};
use crate::metadata::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, LevelState,
    MetadataFlag, RawTermKind, SourceId, TimeSeriesState,
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

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedCategoricalSource {
    pub source: SourceId,
    pub prepared_source_index: usize,
    pub encoded: EncodedFactor<f64>,
}

/// One prepared single-source, intercept-only absorbed term.
///
/// Codes remain owned by the deduplicated categorical-source table. The term
/// points to one unique topology source, which in turn points back to that
/// categorical source. Neither layer copies the observation-scale codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOlsEffect {
    pub term_id: AbsorbTermId,
    pub label: String,
    pub source: SourceId,
    pub categorical_source_index: usize,
    pub topology_source_index: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct PreparedOlsEffectRecipe {
    term_id: AbsorbTermId,
    label: String,
    source: SourceId,
    categorical_source_index: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PreparedOlsFeTopologySource {
    source: SourceId,
    categorical_source_index: usize,
    n_levels: usize,
    level_index: LevelIndex,
}

impl PreparedOlsFeTopologySource {
    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn categorical_source_index(&self) -> usize {
        self.categorical_source_index
    }

    #[must_use]
    pub const fn n_levels(&self) -> usize {
        self.n_levels
    }

    #[must_use]
    pub fn nobs(&self) -> usize {
        self.level_index.nobs()
    }

    pub fn members(&self, level: usize) -> Result<&[usize]> {
        self.level_index.checked_members(level)
    }

    #[must_use]
    pub(crate) const fn level_index(&self) -> &LevelIndex {
        &self.level_index
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsFeSingletonPolicy {
    VerifiedFixedPoint,
    KeptByRequest,
}

/// Structural fixed-effect topology on the final compact observation axis.
///
/// This is deliberately not the numerical solver `FeDesign`: it owns only
/// small term/source mappings plus one fallibly constructed membership index
/// per unique FE source. Final codes and level metadata remain single-owned by
/// `PreparedOlsData::categorical_sources`.
#[derive(Debug, PartialEq, Eq)]
pub struct PreparedOlsFeTopology {
    nobs: usize,
    sources: Vec<PreparedOlsFeTopologySource>,
    effects: Vec<PreparedOlsEffect>,
    total_levels: usize,
    singleton_policy: PreparedOlsFeSingletonPolicy,
}

impl PreparedOlsFeTopology {
    #[must_use]
    pub const fn nobs(&self) -> usize {
        self.nobs
    }

    #[must_use]
    pub fn sources(&self) -> &[PreparedOlsFeTopologySource] {
        &self.sources
    }

    #[must_use]
    pub fn effects(&self) -> &[PreparedOlsEffect] {
        &self.effects
    }

    #[must_use]
    pub const fn total_levels(&self) -> usize {
        self.total_levels
    }

    #[must_use]
    pub const fn singleton_policy(&self) -> PreparedOlsFeSingletonPolicy {
        self.singleton_policy
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsSingletons {
    pub kept_by_request: bool,
    pub num_singletons: usize,
    pub cascade_events: usize,
}

#[derive(Debug, PartialEq)]
pub struct PreparedOlsData {
    pub model: PreparedOlsModel,
    pub sample: PreparedNumericSample,
    pub categorical_sources: Vec<PreparedCategoricalSource>,
    pub fe_topology: PreparedOlsFeTopology,
    /// Whether the model contains a structural constant direction. With no
    /// explicit absorb terms, the pinned reference synthesizes a virtual
    /// one-level absorbed constant even under `noconstant`.
    pub structural_intercept: bool,
    /// Whether the structural constant is exposed as the `_cons` coefficient.
    pub report_intercept: bool,
    pub singletons: PreparedOlsSingletons,
}

impl PreparedOlsData {
    /// Borrow the current intercept-only FE design without copying codes or
    /// level membership. This structural adapter invokes no numerical solver.
    pub fn borrowed_intercept_fe_design(&self) -> Result<BorrowedInterceptFeDesign<'_>> {
        let mut reservation = SystemFeTopologyReservation;
        build_borrowed_intercept_fe_design_with_reservation(self, &mut reservation)
    }

    /// Evaluate one prepared finite-dimensional recipe at one active row.
    /// This is a testable preparation primitive, not an estimator dispatch.
    pub fn column_value(&self, column: usize, active_row: usize) -> Result<f64> {
        if active_row >= self.sample.n_active() {
            return Err(invalid("prepared OLS active row is out of range"));
        }
        let column = self
            .model
            .columns
            .get(column)
            .ok_or_else(|| invalid("prepared OLS column is out of range"))?;
        match &column.kind {
            PreparedOlsColumnKind::Intercept => Ok(1.0),
            PreparedOlsColumnKind::Continuous {
                prepared_source_index,
                ..
            } => self
                .sample
                .regressors
                .get(*prepared_source_index)
                .and_then(|source| source.get(active_row))
                .copied()
                .ok_or_else(|| invalid("prepared continuous source is out of range")),
            PreparedOlsColumnKind::Categorical {
                categorical_source_index,
                level_code,
                ..
            } => {
                let Some(level_code) = *level_code else {
                    return Ok(0.0);
                };
                let code = self
                    .categorical_sources
                    .get(*categorical_source_index)
                    .and_then(|source| source.encoded.codes.get(active_row))
                    .copied()
                    .ok_or_else(|| invalid("prepared categorical source is out of range"))?;
                Ok(if code == level_code { 1.0 } else { 0.0 })
            }
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                for component in components {
                    let Some(level_code) = component.level_code else {
                        return Ok(0.0);
                    };
                    let code = self
                        .categorical_sources
                        .get(component.categorical_source_index)
                        .and_then(|source| source.encoded.codes.get(active_row))
                        .copied()
                        .ok_or_else(|| {
                            invalid("prepared categorical interaction source is out of range")
                        })?;
                    if code != level_code {
                        return Ok(0.0);
                    }
                }
                Ok(1.0)
            }
        }
    }
}

fn build_borrowed_intercept_fe_design_with_reservation<'a, R: FeTopologyReservation>(
    prepared: &'a PreparedOlsData,
    reservation: &mut R,
) -> Result<BorrowedInterceptFeDesign<'a>> {
    let mut validation = SystemBorrowedInterceptSourceValidation;
    build_borrowed_intercept_fe_design_with_controls(prepared, reservation, &mut validation)
}

fn build_borrowed_intercept_fe_design_with_controls<
    'a,
    R: FeTopologyReservation,
    V: BorrowedInterceptSourceValidation,
>(
    prepared: &'a PreparedOlsData,
    reservation: &mut R,
    validation: &mut V,
) -> Result<BorrowedInterceptFeDesign<'a>> {
    if prepared.fe_topology.nobs() != prepared.sample.n_active() {
        return Err(invalid(
            "borrowed FE design topology is not aligned with the final sample",
        ));
    }

    let mut validated_sources = Vec::new();
    reservation.reserve_vec(
        &mut validated_sources,
        prepared.fe_topology.sources().len(),
        "unable to reserve validated borrowed FE sources",
    )?;
    for topology_source in prepared.fe_topology.sources() {
        let categorical = prepared
            .categorical_sources
            .get(topology_source.categorical_source_index())
            .ok_or_else(|| invalid("borrowed FE source categorical index is out of range"))?;
        if categorical.source != topology_source.source()
            || categorical.encoded.n_levels() != topology_source.n_levels()
        {
            return Err(invalid(
                "borrowed FE source identity or level count is inconsistent",
            ));
        }
        let validated = validation.validate(
            &categorical.encoded.codes,
            topology_source.n_levels(),
            topology_source.level_index(),
        )?;
        push_reserved(
            &mut validated_sources,
            validated,
            "validated borrowed FE source capacity exhausted",
        )?;
    }

    let mut effects = Vec::new();
    reservation.reserve_vec(
        &mut effects,
        prepared.fe_topology.effects().len(),
        "unable to reserve borrowed FE design effects",
    )?;
    for effect in prepared.fe_topology.effects() {
        let topology_source = prepared
            .fe_topology
            .sources()
            .get(effect.topology_source_index)
            .ok_or_else(|| invalid("borrowed FE design topology source is out of range"))?;
        let categorical = prepared
            .categorical_sources
            .get(effect.categorical_source_index)
            .ok_or_else(|| invalid("borrowed FE design categorical source is out of range"))?;
        let validated = validated_sources
            .get(effect.topology_source_index)
            .ok_or_else(|| invalid("borrowed FE validated source is out of range"))?;
        if topology_source.categorical_source_index() != effect.categorical_source_index
            || topology_source.source() != effect.source
            || categorical.source != effect.source
            || categorical.encoded.n_levels() != topology_source.n_levels()
            || validated.n_levels() != topology_source.n_levels()
            || !std::ptr::eq(validated.codes(), categorical.encoded.codes.as_slice())
            || !std::ptr::eq(validated.level_index(), topology_source.level_index())
        {
            return Err(invalid(
                "borrowed FE design source identities are inconsistent",
            ));
        }
        push_reserved(
            &mut effects,
            validated.effect(),
            "borrowed FE design effect capacity exhausted",
        )?;
    }
    BorrowedInterceptFeDesign::new(prepared.sample.n_active(), effects)
}

impl OlsRequestV1 {
    /// Validate the entire request and compile the currently supported plain
    /// continuous, numeric categorical main-effect, and all-categorical
    /// interaction recipes. This performs no observation reads; categorical
    /// level codes are resolved later.
    pub fn validate_and_compile(&self) -> Result<PreparedOlsModel> {
        self.validate_and_compile_with_cluster_sources(&[])
    }

    pub(super) fn validate_and_compile_with_cluster_sources(
        &self,
        cluster_sources: &[SourceId],
    ) -> Result<PreparedOlsModel> {
        self.model.validate_executable_metadata()?;
        validate_absorbed_preparation(self)?;
        validate_bindings(self, cluster_sources)?;
        let structural_intercept =
            self.absorbed_terms.is_empty() || self.absorbed_terms.iter().any(|term| term.intercept);
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
            let kind = match (term.raw_kind, term.components.as_slice()) {
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
                            &categorical_source_indices,
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
                        .map_err(|_| {
                            allocation("unable to reserve categorical interaction recipe")
                        })?;
                    for component in components {
                        validate_categorical_value(
                            component.level_value,
                            "categorical interaction metadata level",
                        )?;
                        prepared_components.push(PreparedOlsCategoricalComponent {
                            source: component.source,
                            categorical_source_index: categorical_source_index(
                                &categorical_source_indices,
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
            };

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

/// Finish the Stata-independent boundary after the SPI layer has loaded raw inputs.
pub fn prepare_ols_data(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<PreparedOlsData> {
    prepare_ols_data_with_cluster_sources(request, loaded, &[])
}

/// Finish the common complete, unweighted boundary by taking ownership of the
/// plugin input buffers.  This avoids copying every outcome and regressor
/// column before the row-major solver batch is built.  Inputs with exclusions,
/// missing values, or weights retain the fully general borrowed preparation
/// path and its authoritative classification semantics.
pub fn prepare_ols_data_owned(
    request: &OlsRequestV1,
    loaded: LoadedOlsInput,
) -> Result<PreparedOlsData> {
    if !is_full_retained_unweighted_sample(&loaded) {
        return prepare_ols_data(request, &loaded);
    }
    let model = request.validate_and_compile_with_cluster_sources(&[])?;
    validate_loaded(request, &loaded)?;
    validate_candidate_categorical_domain(request, &loaded, &[])?;
    validate_candidate_weight_domain(&loaded)?;
    let sample = take_full_retained_unweighted_sample(loaded)?;
    finish_prepare_ols_data(request, &[], model, sample)
}

pub(crate) fn prepare_ols_data_with_cluster_sources(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<PreparedOlsData> {
    let model = request.validate_and_compile_with_cluster_sources(cluster_sources)?;
    validate_loaded(request, loaded)?;
    validate_candidate_categorical_domain(request, loaded, cluster_sources)?;

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
    finish_prepare_ols_data(request, cluster_sources, model, sample)
}

fn finish_prepare_ols_data(
    request: &OlsRequestV1,
    cluster_sources: &[SourceId],
    mut model: PreparedOlsModel,
    sample: PreparedNumericSample,
) -> Result<PreparedOlsData> {
    // Cluster-only sources are deliberately absent from this pre-singleton
    // factorization. They affect availability (reason 6), never FE pruning;
    // cluster coding is created only on the final singleton fixed point below.
    let pruning_categorical_sources = factorize_categorical_sources(request, &sample, &[])?;
    let pruning_fixed_effects = prepare_fixed_effects(request, &pruning_categorical_sources, &[])?;
    let singleton_result = prune_prepared_singletons(
        request,
        &sample,
        &pruning_categorical_sources,
        &pruning_fixed_effects,
    )?;
    let num_singletons = singleton_result.dropped_observations.len();
    let cascade_events = singleton_result.cascade_events;
    let sample =
        sample.compact_active_rows(&singleton_result.active, crate::data::DropReason::Singleton)?;

    // With no singleton deletion, the pruning codes already live on the final
    // compact axis. Reusing them avoids a second observation-scale
    // factorization. Cluster-only sources still require the complete final
    // factorization because they were deliberately excluded from pruning.
    let pruning_covers_final_sources = cluster_sources.iter().all(|source| {
        pruning_categorical_sources
            .iter()
            .any(|candidate| candidate.source == *source)
    });
    let (categorical_sources, fixed_effects) = if num_singletons == 0
        && pruning_covers_final_sources
    {
        (pruning_categorical_sources, pruning_fixed_effects)
    } else {
        let categorical_sources = factorize_categorical_sources(request, &sample, cluster_sources)?;
        let fixed_effects = prepare_fixed_effects(request, &categorical_sources, cluster_sources)?;
        (categorical_sources, fixed_effects)
    };
    resolve_categorical_level_codes(&mut model, &categorical_sources)?;
    let fe_topology =
        build_prepared_fe_topology(request, &sample, &categorical_sources, fixed_effects)?;
    Ok(PreparedOlsData {
        model,
        sample,
        categorical_sources,
        fe_topology,
        structural_intercept: request.absorbed_terms.is_empty()
            || request.absorbed_terms.iter().any(|term| term.intercept),
        report_intercept: request.include_intercept
            && (request.absorbed_terms.is_empty()
                || request.absorbed_terms.iter().any(|term| term.intercept)),
        singletons: PreparedOlsSingletons {
            kept_by_request: request.keep_singletons,
            num_singletons,
            cascade_events,
        },
    })
}

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
        && loaded.numeric_sources.iter().all(|(_, column)| {
            column.missing.iter().all(|missing| !*missing)
                && column.values.iter().all(|value| value.is_finite())
        })
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

fn prune_prepared_singletons(
    request: &OlsRequestV1,
    sample: &PreparedNumericSample,
    categorical_sources: &[PreparedCategoricalSource],
    fixed_effects: &[PreparedOlsEffectRecipe],
) -> Result<SingletonResult> {
    let mut codes_by_effect = Vec::new();
    codes_by_effect
        .try_reserve_exact(fixed_effects.len())
        .map_err(|_| allocation("unable to reserve singleton fixed-effect code table"))?;
    let mut level_counts = Vec::new();
    level_counts
        .try_reserve_exact(fixed_effects.len())
        .map_err(|_| allocation("unable to reserve singleton level-count table"))?;
    for effect in fixed_effects {
        let source = categorical_sources
            .get(effect.categorical_source_index)
            .ok_or_else(|| invalid("singleton fixed-effect source index is out of range"))?;
        if source.encoded.codes.len() != sample.n_active() {
            return Err(invalid(
                "singleton fixed-effect source is not aligned with the numeric sample",
            ));
        }
        codes_by_effect.push(source.encoded.codes.as_slice());
        level_counts.push(source.encoded.n_levels());
    }

    let mut initially_active = Vec::new();
    initially_active
        .try_reserve_exact(sample.n_active())
        .map_err(|_| allocation("unable to reserve singleton active mask"))?;
    initially_active.resize(sample.n_active(), true);
    let multiplicity = match &sample.weights {
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values,
        } => SingletonMultiplicity::Frequency(values),
        PreparedWeights::None | PreparedWeights::Provided { .. } => SingletonMultiplicity::Unit,
    };
    prune_singletons_with_multiplicity(
        &codes_by_effect,
        &level_counts,
        &initially_active,
        multiplicity,
        request.keep_singletons,
    )
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

fn validate_candidate_categorical_domain(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    _cluster_sources: &[SourceId],
) -> Result<()> {
    // Ordinary factor-variable metadata requires canonical nonnegative integer
    // levels. FE and cluster factorization, like the pinned Factor behavior,
    // accepts every finite numeric category (including negative/fractional
    // values) and therefore must not inherit this ordinary-factor restriction.
    let categorical = ordinary_categorical_sources(&request.model);
    for (source, column) in &loaded.numeric_sources {
        if !categorical.contains(source) {
            continue;
        }
        for (row, candidate) in loaded.candidate.iter().copied().enumerate() {
            if !candidate || column.missing[row] || !column.values[row].is_finite() {
                continue;
            }
            validate_categorical_value(
                column.values[row],
                &format!(
                    "numeric categorical source {} at observation {}",
                    source.get(),
                    row + 1
                ),
            )?;
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

fn factorize_categorical_sources(
    request: &OlsRequestV1,
    sample: &PreparedNumericSample,
    cluster_sources: &[SourceId],
) -> Result<Vec<PreparedCategoricalSource>> {
    let categorical = categorical_source_indices(request, cluster_sources)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(categorical.len())
        .map_err(|_| allocation("unable to reserve prepared categorical sources"))?;
    for source in &request.model.sources {
        if !categorical.contains_key(&source.id) {
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
    Ok(output)
}

fn prepare_fixed_effects(
    request: &OlsRequestV1,
    categorical_sources: &[PreparedCategoricalSource],
    cluster_sources: &[SourceId],
) -> Result<Vec<PreparedOlsEffectRecipe>> {
    let indices = categorical_source_indices(request, cluster_sources)?;
    let mut fixed_effects = Vec::new();
    fixed_effects
        .try_reserve_exact(request.absorbed_terms.len())
        .map_err(|_| allocation("unable to reserve prepared fixed-effect terms"))?;
    for term in &request.absorbed_terms {
        let source = term.categorical_sources[0];
        let categorical_source_index = categorical_source_index(&indices, source)?;
        if categorical_source_index >= categorical_sources.len() {
            return Err(invalid(
                "prepared fixed-effect categorical source is out of range",
            ));
        }
        fixed_effects.push(PreparedOlsEffectRecipe {
            term_id: term.id,
            label: fallible_string_copy(&term.label, "unable to reserve fixed-effect label")?,
            source,
            categorical_source_index,
        });
    }
    Ok(fixed_effects)
}

trait FeTopologyReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

trait BorrowedInterceptSourceValidation {
    fn validate<'a>(
        &mut self,
        codes: &'a [u32],
        n_levels: usize,
        level_index: &'a LevelIndex,
    ) -> Result<ValidatedBorrowedInterceptSource<'a>>;
}

struct SystemBorrowedInterceptSourceValidation;

impl BorrowedInterceptSourceValidation for SystemBorrowedInterceptSourceValidation {
    fn validate<'a>(
        &mut self,
        codes: &'a [u32],
        n_levels: usize,
        level_index: &'a LevelIndex,
    ) -> Result<ValidatedBorrowedInterceptSource<'a>> {
        ValidatedBorrowedInterceptSource::new(codes, n_levels, level_index)
    }
}

struct SystemFeTopologyReservation;

impl FeTopologyReservation for SystemFeTopologyReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn build_prepared_fe_topology(
    request: &OlsRequestV1,
    sample: &PreparedNumericSample,
    categorical_sources: &[PreparedCategoricalSource],
    recipes: Vec<PreparedOlsEffectRecipe>,
) -> Result<PreparedOlsFeTopology> {
    let mut reservation = SystemFeTopologyReservation;
    build_prepared_fe_topology_with_reservation(
        request,
        sample,
        categorical_sources,
        recipes,
        &mut reservation,
    )
}

fn build_prepared_fe_topology_with_reservation<R: FeTopologyReservation>(
    request: &OlsRequestV1,
    sample: &PreparedNumericSample,
    categorical_sources: &[PreparedCategoricalSource],
    recipes: Vec<PreparedOlsEffectRecipe>,
    reservation: &mut R,
) -> Result<PreparedOlsFeTopology> {
    let mut sources = Vec::new();
    reservation.reserve_vec(
        &mut sources,
        recipes.len(),
        "unable to reserve prepared FE topology sources",
    )?;
    let mut effects = Vec::new();
    reservation.reserve_vec(
        &mut effects,
        recipes.len(),
        "unable to reserve prepared FE topology effects",
    )?;
    let mut total_levels = 0_usize;

    for recipe in recipes {
        let topology_source_index = if let Some(index) =
            sources
                .iter()
                .position(|source: &PreparedOlsFeTopologySource| {
                    source.categorical_source_index == recipe.categorical_source_index
                }) {
            index
        } else {
            let categorical = categorical_sources
                .get(recipe.categorical_source_index)
                .ok_or_else(|| invalid("prepared FE topology source index is out of range"))?;
            if categorical.source != recipe.source {
                return Err(invalid(
                    "prepared FE topology source identity does not match its categorical source",
                ));
            }
            if categorical.encoded.codes.len() != sample.n_active() {
                return Err(invalid(
                    "prepared FE topology source is not aligned with the final sample",
                ));
            }
            let n_levels = categorical.encoded.n_levels();
            total_levels = total_levels
                .checked_add(n_levels)
                .ok_or_else(|| allocation("prepared FE topology total level count overflows"))?;
            let level_index = LevelIndex::from_codes(&categorical.encoded.codes, n_levels)?;
            let index = sources.len();
            push_reserved(
                &mut sources,
                PreparedOlsFeTopologySource {
                    source: recipe.source,
                    categorical_source_index: recipe.categorical_source_index,
                    n_levels,
                    level_index,
                },
                "prepared FE topology source capacity exhausted",
            )?;
            index
        };

        push_reserved(
            &mut effects,
            PreparedOlsEffect {
                term_id: recipe.term_id,
                label: recipe.label,
                source: recipe.source,
                categorical_source_index: recipe.categorical_source_index,
                topology_source_index,
            },
            "prepared FE topology effect capacity exhausted",
        )?;
    }

    let topology = PreparedOlsFeTopology {
        nobs: sample.n_active(),
        sources,
        effects,
        total_levels,
        singleton_policy: if request.keep_singletons {
            PreparedOlsFeSingletonPolicy::KeptByRequest
        } else {
            PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
        },
    };
    topology.validate(request, sample, categorical_sources)?;
    Ok(topology)
}

impl PreparedOlsFeTopology {
    /// Recheck the complete structural contract against the final prepared data.
    pub fn validate(
        &self,
        request: &OlsRequestV1,
        sample: &PreparedNumericSample,
        categorical_sources: &[PreparedCategoricalSource],
    ) -> Result<()> {
        if self.nobs != sample.n_active() || self.effects.len() != request.absorbed_terms.len() {
            return Err(invalid(
                "prepared FE topology dimensions do not match the final sample and request",
            ));
        }
        let expected_policy = if request.keep_singletons {
            PreparedOlsFeSingletonPolicy::KeptByRequest
        } else {
            PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
        };
        if self.singleton_policy != expected_policy {
            return Err(invalid(
                "prepared FE topology singleton policy does not match the request",
            ));
        }

        let mut checked_total_levels = 0_usize;
        for (source_index, source) in self.sources.iter().enumerate() {
            if self.sources[..source_index]
                .iter()
                .any(|earlier| earlier.categorical_source_index == source.categorical_source_index)
            {
                return Err(invalid(
                    "prepared FE topology contains a duplicate categorical source",
                ));
            }
            let categorical = categorical_sources
                .get(source.categorical_source_index)
                .ok_or_else(|| {
                    invalid("prepared FE topology categorical source is out of range")
                })?;
            if categorical.source != source.source
                || categorical.encoded.codes.len() != self.nobs
                || categorical.encoded.n_levels() != source.n_levels
                || source.level_index.nobs() != self.nobs
                || source.level_index.n_levels() != source.n_levels
            {
                return Err(invalid(
                    "prepared FE topology source is inconsistent with final categorical storage",
                ));
            }
            if (self.nobs == 0) != (source.n_levels == 0) {
                return Err(invalid(
                    "prepared FE topology empty-sample and level counts disagree",
                ));
            }
            checked_total_levels = checked_total_levels
                .checked_add(source.n_levels)
                .ok_or_else(|| allocation("prepared FE topology total level count overflows"))?;
            let mut membership_count = 0_usize;
            for level in 0..source.n_levels {
                let members = source.level_index.checked_members(level)?;
                if members.is_empty() {
                    return Err(invalid(
                        "prepared FE topology final categorical codes are not contiguous",
                    ));
                }
                membership_count = membership_count
                    .checked_add(members.len())
                    .ok_or_else(|| allocation("prepared FE topology membership count overflows"))?;
                for observation in members {
                    if *observation >= self.nobs
                        || categorical.encoded.codes[*observation] as usize != level
                    {
                        return Err(invalid(
                            "prepared FE topology membership does not match final codes",
                        ));
                    }
                }
                if self.singleton_policy == PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
                    && level_multiplicity(sample, members)? == 1
                {
                    return Err(invalid(
                        "prepared FE topology contains a singleton after fixed-point pruning",
                    ));
                }
            }
            if membership_count != self.nobs {
                return Err(invalid(
                    "prepared FE topology membership does not cover the final sample exactly",
                ));
            }
        }
        if checked_total_levels != self.total_levels {
            return Err(invalid(
                "prepared FE topology cached level count is inconsistent",
            ));
        }

        for (index, (effect, term)) in self.effects.iter().zip(&request.absorbed_terms).enumerate()
        {
            if effect.term_id != term.id
                || effect.label != term.label
                || term.categorical_sources.as_slice() != [effect.source]
            {
                return Err(invalid(format!(
                    "prepared FE topology term identity changed at effect {}",
                    index + 1
                )));
            }
            let source = self
                .sources
                .get(effect.topology_source_index)
                .ok_or_else(|| invalid("prepared FE topology effect source is out of range"))?;
            if source.source != effect.source
                || source.categorical_source_index != effect.categorical_source_index
            {
                return Err(invalid(
                    "prepared FE topology effect does not reference its declared source",
                ));
            }
        }
        if self.sources.iter().any(|source| {
            !self.effects.iter().any(|effect| {
                effect.topology_source_index < self.sources.len()
                    && self.sources[effect.topology_source_index].categorical_source_index
                        == source.categorical_source_index
            })
        }) {
            return Err(invalid(
                "prepared FE topology contains an unreferenced source",
            ));
        }
        Ok(())
    }
}

fn level_multiplicity(sample: &PreparedNumericSample, members: &[usize]) -> Result<u64> {
    match &sample.weights {
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values,
        } => members.iter().try_fold(0_u64, |total, observation| {
            let value = values
                .get(*observation)
                .copied()
                .ok_or_else(|| invalid("prepared FE topology frequency weights are not aligned"))?;
            if !value.is_finite()
                || value <= 0.0
                || value.fract() != 0.0
                || value > f64::from(u32::MAX)
            {
                return Err(invalid_data(
                    "prepared FE topology frequency weight is outside its validated domain",
                ));
            }
            total.checked_add(value as u64).ok_or_else(|| {
                invalid_data("prepared FE topology frequency-weight multiplicity overflows")
            })
        }),
        PreparedWeights::None | PreparedWeights::Provided { .. } => u64::try_from(members.len())
            .map_err(|_| allocation("prepared FE topology physical multiplicity exceeds u64")),
    }
}

fn fallible_string_copy(value: &str, label: &'static str) -> Result<String> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| allocation(label))?;
    output.push_str(value);
    Ok(output)
}

fn resolve_categorical_level_codes(
    model: &mut PreparedOlsModel,
    categorical_sources: &[PreparedCategoricalSource],
) -> Result<()> {
    for column in &mut model.columns {
        match &mut column.kind {
            PreparedOlsColumnKind::Categorical {
                categorical_source_index,
                level_bits,
                level_code,
                ..
            } => {
                *level_code = resolve_categorical_level_code(
                    categorical_sources,
                    *categorical_source_index,
                    *level_bits,
                )?;
            }
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                for component in components {
                    component.level_code = resolve_categorical_level_code(
                        categorical_sources,
                        component.categorical_source_index,
                        component.level_bits,
                    )?;
                }
            }
            PreparedOlsColumnKind::Intercept | PreparedOlsColumnKind::Continuous { .. } => {}
        }
    }
    Ok(())
}

fn resolve_categorical_level_code(
    categorical_sources: &[PreparedCategoricalSource],
    categorical_source_index: usize,
    level_bits: u64,
) -> Result<Option<u32>> {
    let source = categorical_sources
        .get(categorical_source_index)
        .ok_or_else(|| invalid("prepared categorical source plan is out of range"))?;
    let level = f64::from_bits(level_bits);
    source
        .encoded
        .levels
        .binary_search_by(|candidate| candidate.total_cmp(&level))
        .ok()
        .map(|index| {
            u32::try_from(index).map_err(|_| invalid("prepared categorical level code exceeds u32"))
        })
        .transpose()
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
        .flat_map(|term| term.categorical_sources.iter().copied())
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

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

fn negative_weight(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NegativeWeight, message)
}

fn invalid_frequency_weight(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidFrequencyWeight, message)
}

fn unsupported(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Unsupported, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
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

    #[derive(Default)]
    struct CountingBorrowedSourceValidation {
        calls: usize,
        inspected_rows: usize,
    }

    impl BorrowedInterceptSourceValidation for CountingBorrowedSourceValidation {
        fn validate<'a>(
            &mut self,
            codes: &'a [u32],
            n_levels: usize,
            level_index: &'a LevelIndex,
        ) -> Result<ValidatedBorrowedInterceptSource<'a>> {
            self.calls = self.calls.checked_add(1).expect("small validation count");
            self.inspected_rows = self
                .inspected_rows
                .checked_add(codes.len())
                .expect("small inspected-row count");
            ValidatedBorrowedInterceptSource::new(codes, n_levels, level_index)
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
        assert_eq!(prepared.categorical_sources.len(), 1);
        assert_eq!(
            prepared.categorical_sources[0].encoded.levels,
            vec![0.0, 1.0, 2.0]
        );
        assert_eq!(prepared.categorical_sources[0].encoded.codes, vec![0, 1, 2]);
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
    fn categorical_domain_is_candidate_scoped_and_fails_before_sample_construction() {
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
            assert_eq!(error.code, ErrorCode::InvalidData);
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
        assert_eq!(prepared.categorical_sources.len(), 3);
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
        assert_eq!(prepared.fe_topology.nobs, 0);
        assert!(prepared.fe_topology.sources.is_empty());
        assert!(prepared.fe_topology.effects.is_empty());
        assert_eq!(prepared.fe_topology.total_levels, 0);
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
        assert_eq!(prepared.categorical_sources.len(), 2);
        assert_eq!(prepared.fe_topology.effects.len(), 3);
        assert_eq!(prepared.fe_topology.sources.len(), 2);
        assert_eq!(prepared.fe_topology.nobs, 2);
        assert_eq!(prepared.fe_topology.total_levels, 4);
        assert_eq!(
            prepared.fe_topology.singleton_policy,
            PreparedOlsFeSingletonPolicy::KeptByRequest
        );
        assert_eq!(
            prepared.fe_topology.effects[0].topology_source_index,
            prepared.fe_topology.effects[2].topology_source_index
        );
        assert_eq!(prepared.fe_topology.effects[0].label, "g");
        assert_eq!(prepared.fe_topology.effects[1].label, "h");
        assert_eq!(prepared.fe_topology.effects[2].label, "g_again");
        prepared
            .fe_topology
            .validate(&request, &prepared.sample, &prepared.categorical_sources)
            .expect("structural certificate");
        let design = prepared
            .borrowed_intercept_fe_design()
            .expect("borrowed FE design");
        assert_eq!(design.nobs(), 2);
        assert_eq!(design.effects().len(), 3);
        assert_eq!(design.total_term_slots(), 6);
        assert!(std::ptr::eq(
            design.effects()[0].codes(),
            prepared.categorical_sources[0].encoded.codes.as_slice()
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
            prepared.categorical_sources[1].encoded.levels,
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
        assert_eq!(prepared.categorical_sources.len(), 2);
        assert_eq!(prepared.fe_topology.nobs, 4);
        assert_eq!(prepared.fe_topology.effects.len(), 2);
        assert_eq!(prepared.fe_topology.sources.len(), 2);
        assert_eq!(prepared.fe_topology.total_levels, 4);
        assert_eq!(
            prepared.fe_topology.singleton_policy,
            PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
        );
        assert_eq!(
            prepared.fe_topology.sources[0]
                .members(0)
                .expect("worker level zero"),
            &[0, 1]
        );
        assert_eq!(
            prepared.fe_topology.sources[1]
                .members(1)
                .expect("firm level one"),
            &[1, 3]
        );
        prepared
            .fe_topology
            .validate(&request, &prepared.sample, &prepared.categorical_sources)
            .expect("post-singleton topology certificate");
        assert_eq!(
            prepared.categorical_sources[0].encoded.levels,
            vec![4.0, 5.0]
        );
        assert_eq!(
            prepared.categorical_sources[0].encoded.codes,
            vec![0, 0, 1, 1]
        );
        assert_eq!(
            prepared.categorical_sources[1].encoded.levels,
            vec![4.0, 5.0]
        );
        assert_eq!(
            prepared.categorical_sources[1].encoded.codes,
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
            kept.fe_topology.singleton_policy,
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
            weighted.fe_topology.singleton_policy,
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
            probability.fe_topology.singleton_policy,
            PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
        );
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
        let weighted = prepare_ols_data(&request, &loaded).expect("frequency singleton");
        assert_eq!(weighted.sample.retained_rows(), &[0]);
        assert_eq!(
            weighted.sample.drop_reasons,
            vec![crate::DropReason::Retained, crate::DropReason::Singleton]
        );
        assert_eq!(weighted.categorical_sources[0].encoded.levels, vec![10.0]);
        assert_eq!(weighted.singletons.num_singletons, 1);
        assert_eq!(weighted.fe_topology.total_levels, 1);
        assert_eq!(
            weighted.fe_topology.singleton_policy,
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
        assert_eq!(prepared.categorical_sources[0].encoded.levels, vec![2.0]);
        assert_eq!(prepared.categorical_sources[0].encoded.codes, vec![0, 0]);
        assert_eq!(prepared.fe_topology.sources.len(), 1);
        assert_eq!(
            prepared.fe_topology.effects[0].categorical_source_index,
            prepared.fe_topology.sources[0].categorical_source_index
        );
        assert_eq!(
            prepared.fe_topology.sources[0]
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
        assert!(prepared.categorical_sources[0].encoded.levels.is_empty());
        assert!(prepared.categorical_sources[0].encoded.codes.is_empty());
        assert_eq!(prepared.fe_topology.nobs, 0);
        assert_eq!(prepared.fe_topology.effects.len(), 1);
        assert_eq!(prepared.fe_topology.sources.len(), 1);
        assert_eq!(prepared.fe_topology.sources[0].n_levels, 0);
        assert_eq!(prepared.fe_topology.sources[0].nobs(), 0);
        assert_eq!(prepared.fe_topology.total_levels, 0);
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
        for (effect, prepared_effect) in design.effects().iter().zip(&prepared.fe_topology.effects)
        {
            let topology_source =
                &prepared.fe_topology.sources[prepared_effect.topology_source_index];
            let categorical =
                &prepared.categorical_sources[prepared_effect.categorical_source_index];
            assert!(std::ptr::eq(
                effect.codes(),
                categorical.encoded.codes.as_slice()
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
        assert_eq!(successful.calls, 2);

        for fail_at in 0..successful.calls {
            let mut failed = InjectedFeTopologyReservation {
                fail_at: Some(fail_at),
                ..InjectedFeTopologyReservation::default()
            };
            let error = build_borrowed_intercept_fe_design_with_reservation(&prepared, &mut failed)
                .expect_err("adapter reservation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        for skipped in [
            "unable to reserve validated borrowed FE sources",
            "unable to reserve borrowed FE design effects",
        ] {
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
    fn repeated_fe_terms_validate_their_unique_source_once() {
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
        assert_eq!(prepared.fe_topology.sources().len(), 1);
        assert_eq!(prepared.fe_topology.effects().len(), TERM_COUNT);

        let mut reservation = SystemFeTopologyReservation;
        let mut validation = CountingBorrowedSourceValidation::default();
        let design = build_borrowed_intercept_fe_design_with_controls(
            &prepared,
            &mut reservation,
            &mut validation,
        )
        .expect("borrowed repeated FE design");
        assert_eq!(validation.calls, 1);
        assert_eq!(validation.inspected_rows, prepared.sample.n_active());
        assert_eq!(design.effects().len(), TERM_COUNT);
        assert_eq!(design.total_term_slots(), TERM_COUNT * 3);
        for effect in design.effects() {
            assert!(std::ptr::eq(
                effect.codes(),
                prepared.categorical_sources[0].encoded.codes.as_slice()
            ));
            assert!(std::ptr::eq(
                effect.level_index(),
                prepared.fe_topology.sources()[0].level_index()
            ));
        }
    }

    #[test]
    fn borrowed_fe_design_rejects_corrupt_prepared_links_without_panicking() {
        let request = two_way_singleton_request(false);

        let mut bad_effect =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared topology");
        bad_effect.fe_topology.effects[0].topology_source_index = usize::MAX;
        assert_eq!(
            bad_effect
                .borrowed_intercept_fe_design()
                .expect_err("bad topology source")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut short_codes =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared topology");
        short_codes.categorical_sources[0].encoded.codes.pop();
        assert_eq!(
            short_codes
                .borrowed_intercept_fe_design()
                .expect_err("short categorical codes")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut bad_categorical_index =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared topology");
        bad_categorical_index.fe_topology.effects[0].categorical_source_index = usize::MAX;
        assert_eq!(
            bad_categorical_index
                .borrowed_intercept_fe_design()
                .expect_err("bad categorical source")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut bad_levels =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared topology");
        bad_levels.categorical_sources[0].encoded.levels.push(99.0);
        assert_eq!(
            bad_levels
                .borrowed_intercept_fe_design()
                .expect_err("level mismatch")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut bad_membership =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared topology");
        bad_membership.fe_topology.sources[0].level_index =
            LevelIndex::from_codes(&[0, 1, 0, 1], 2).expect("wrong membership");
        assert_eq!(
            bad_membership
                .borrowed_intercept_fe_design()
                .expect_err("membership mismatch")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn every_topology_table_reservation_and_capacity_guard_fails_closed() {
        let request = two_way_singleton_request(true);
        let prepared =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared source data");
        let PreparedOlsData {
            sample,
            categorical_sources,
            ..
        } = prepared;

        let mut successful = InjectedFeTopologyReservation::default();
        let recipes = prepare_fixed_effects(&request, &categorical_sources, &[]).expect("recipes");
        build_prepared_fe_topology_with_reservation(
            &request,
            &sample,
            &categorical_sources,
            recipes,
            &mut successful,
        )
        .expect("record topology reservations");
        assert_eq!(successful.calls, 2);

        for fail_at in 0..successful.calls {
            let recipes =
                prepare_fixed_effects(&request, &categorical_sources, &[]).expect("recipes");
            let mut injected = InjectedFeTopologyReservation {
                fail_at: Some(fail_at),
                ..InjectedFeTopologyReservation::default()
            };
            let error = build_prepared_fe_topology_with_reservation(
                &request,
                &sample,
                &categorical_sources,
                recipes,
                &mut injected,
            )
            .expect_err("injected topology reservation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        for skipped in [
            "unable to reserve prepared FE topology sources",
            "unable to reserve prepared FE topology effects",
        ] {
            let recipes =
                prepare_fixed_effects(&request, &categorical_sources, &[]).expect("recipes");
            let mut undersized = InjectedFeTopologyReservation {
                skip_label: Some(skipped),
                ..InjectedFeTopologyReservation::default()
            };
            let error = build_prepared_fe_topology_with_reservation(
                &request,
                &sample,
                &categorical_sources,
                recipes,
                &mut undersized,
            )
            .expect_err("topology capacity guard must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "{skipped}");
        }
    }

    #[test]
    fn topology_validator_rejects_corrupt_identity_membership_and_level_metadata() {
        let request = two_way_singleton_request(false);

        let mut bad_effect =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared topology");
        bad_effect.fe_topology.effects[0].topology_source_index = usize::MAX;
        assert_eq!(
            bad_effect
                .fe_topology
                .validate(
                    &request,
                    &bad_effect.sample,
                    &bad_effect.categorical_sources,
                )
                .expect_err("bad effect source")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut bad_membership =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared topology");
        bad_membership.fe_topology.sources[0].level_index =
            LevelIndex::from_codes(&[0, 1, 0, 1], 2).expect("wrong membership");
        assert_eq!(
            bad_membership
                .fe_topology
                .validate(
                    &request,
                    &bad_membership.sample,
                    &bad_membership.categorical_sources,
                )
                .expect_err("membership mismatch")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut noncontiguous =
            prepare_ols_data(&request, &two_way_singleton_input()).expect("prepared topology");
        noncontiguous.categorical_sources[0]
            .encoded
            .levels
            .push(99.0);
        noncontiguous.fe_topology.sources[0].n_levels = 3;
        noncontiguous.fe_topology.sources[0].level_index =
            LevelIndex::from_codes(&[0, 0, 1, 1], 3).expect("hole-bearing membership");
        noncontiguous.fe_topology.total_levels += 1;
        assert_eq!(
            noncontiguous
                .fe_topology
                .validate(
                    &request,
                    &noncontiguous.sample,
                    &noncontiguous.categorical_sources,
                )
                .expect_err("empty final level")
                .code,
            ErrorCode::InvalidArgument
        );
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
