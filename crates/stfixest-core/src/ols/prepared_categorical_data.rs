//! Integrated prepared-data path for numeric categorical absorbed interactions.
//!
//! This module leaves the existing single-source `PreparedOlsData` contract
//! unchanged. It provides a parallel, fail-closed path for intercept-only
//! numeric categorical effects such as `worker` and `firm#year`, carrying the
//! request from authoritative sample construction through interaction-aware
//! singleton pruning, final topology, absorbed DoF, RHS generation, and
//! certified MAP residualization. Public estimator dispatch remains disabled.

use crate::data::{
    prepare_numeric_sample_with_source_uses, AdjustmentInput, DenseMatrix, DomainDecision,
    DropReason, NumericColumnView, NumericDomainPolicy, NumericPreparationInput,
    NumericSourceUse, PreparedAdjustment, PreparedNumericSample, PreparedWeights, WeightInput,
    WeightKind,
};
use crate::error::{ErrorCode, Result, StFixestError};
use crate::factorize::factorize_f64;
use crate::fe::{
    absorb_finalized_categorical_topology_map_certified, finalize_categorical_topology,
    prune_categorical_partition_singletons, BorrowedMapWeights, CategoricalCodeView,
    CertifiedBorrowedMap, FinalizedCategoricalTopology, MapConfig, SingletonMultiplicity,
};
use crate::metadata::{
    AbsorbedTerm, CoefficientSlot, ComponentKind, LevelState, MetadataFlag, RawTermKind,
    SourceId, TimeSeriesState,
};
use crate::ols::{
    CategoricalTopologyDofEffectSpec, CategoricalTopologyDofPolicy,
    CategoricalTopologyDofReport, LoadedNumericColumn, LoadedOlsInput, OlsInputRole,
    OlsRequestV1, PreparedCategoricalSource, PreparedOlsCategoricalComponent,
    PreparedOlsCoefficient, PreparedOlsCoefficientStatus, PreparedOlsColumn,
    PreparedOlsColumnKind, PreparedOlsModel, PreparedOlsSingletons,
};
use crate::topology_dof::compute_categorical_topology_absorbed_dof;

/// Complete numeric categorical-interaction preparation result.
#[derive(Debug, PartialEq)]
pub struct PreparedCategoricalOlsData {
    model: PreparedOlsModel,
    sample: PreparedNumericSample,
    /// Final-sample factorization required only by ordinary categorical model
    /// columns. FE-only raw codes live in `fe_topology`, avoiding a duplicate
    /// observation-scale copy.
    categorical_sources: Vec<PreparedCategoricalSource>,
    fe_topology: FinalizedCategoricalTopology,
    effect_specs: Vec<CategoricalTopologyDofEffectSpec>,
    structural_intercept: bool,
    report_intercept: bool,
    singletons: PreparedOlsSingletons,
}

impl PreparedCategoricalOlsData {
    #[must_use]
    pub const fn model(&self) -> &PreparedOlsModel {
        &self.model
    }

    #[must_use]
    pub const fn sample(&self) -> &PreparedNumericSample {
        &self.sample
    }

    #[must_use]
    pub fn categorical_sources(&self) -> &[PreparedCategoricalSource] {
        &self.categorical_sources
    }

    #[must_use]
    pub const fn fe_topology(&self) -> &FinalizedCategoricalTopology {
        &self.fe_topology
    }

    #[must_use]
    pub fn effect_specs(&self) -> &[CategoricalTopologyDofEffectSpec] {
        &self.effect_specs
    }

    #[must_use]
    pub const fn structural_intercept(&self) -> bool {
        self.structural_intercept
    }

    #[must_use]
    pub const fn report_intercept(&self) -> bool {
        self.report_intercept
    }

    #[must_use]
    pub const fn singletons(&self) -> PreparedOlsSingletons {
        self.singletons
    }

    /// Evaluate one generated ordinary model column on the final sample.
    pub fn column_value(&self, column: usize, active_row: usize) -> Result<f64> {
        if active_row >= self.sample.n_active() {
            return Err(invalid("prepared categorical OLS row is out of range"));
        }
        let column = self
            .model
            .columns
            .get(column)
            .ok_or_else(|| invalid("prepared categorical OLS column is out of range"))?;
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
            } => categorical_indicator(
                &self.categorical_sources,
                *categorical_source_index,
                *level_code,
                active_row,
            ),
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                for component in components {
                    if categorical_indicator(
                        &self.categorical_sources,
                        component.categorical_source_index,
                        component.level_code,
                        active_row,
                    )? == 0.0
                    {
                        return Ok(0.0);
                    }
                }
                Ok(1.0)
            }
        }
    }

    /// Build one observation-major `[Y, X]` batch without intermediate model
    /// columns.
    pub fn build_rhs(&self) -> Result<PreparedCategoricalOlsRhs> {
        let mut reservation = SystemCategoricalPreparationReservation;
        build_rhs_with_reservation(self, &mut reservation)
    }

    /// Residualize the complete prepared batch through the strict borrowed MAP
    /// solver. No caller-visible partially transformed state escapes failure.
    pub fn absorb_rhs_map(&self, config: MapConfig) -> Result<CertifiedBorrowedMap> {
        self.validate()?;
        let rhs = self.build_rhs()?.into_values();
        let weights = match &self.sample.weights {
            PreparedWeights::None => BorrowedMapWeights::Unit,
            PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
        };
        absorb_finalized_categorical_topology_map_certified(
            &self.fe_topology,
            weights,
            rhs,
            config,
        )
    }

    /// Compute absorbed rank directly from the finalized interaction-aware
    /// topology. Cluster dimensions, when supplied, must already be compact
    /// final-sample codes.
    pub fn absorbed_dof(
        &self,
        policy: CategoricalTopologyDofPolicy,
        cluster_dimensions: &[&[u32]],
    ) -> Result<CategoricalTopologyDofReport> {
        self.validate()?;
        compute_categorical_topology_absorbed_dof(
            &self.fe_topology,
            &self.effect_specs,
            self.structural_intercept,
            policy,
            cluster_dimensions,
        )
    }

    pub fn validate(&self) -> Result<()> {
        validate_numeric_sample(&self.sample)?;
        self.fe_topology.validate()?;
        if self.fe_topology.retained_rows().len() != self.sample.n_active() {
            return Err(invalid(
                "prepared categorical topology is not aligned with the final sample",
            ));
        }
        if self.effect_specs.len() != self.fe_topology.effect_to_topology().len() {
            return Err(invalid(
                "prepared categorical effect metadata and topology map differ in length",
            ));
        }
        if !self.structural_intercept {
            return Err(invalid(
                "numeric categorical absorbed effects must contain a structural intercept",
            ));
        }
        if self.report_intercept && !self.structural_intercept {
            return Err(invalid(
                "reported categorical OLS intercept lacks a structural intercept",
            ));
        }
        validate_final_categorical_sources(&self.sample, &self.categorical_sources)?;
        validate_model_recipes(self)?;
        Ok(())
    }
}

/// One directly materialized observation-major categorical OLS batch.
#[derive(Debug, PartialEq)]
pub struct PreparedCategoricalOlsRhs {
    values: DenseMatrix,
}

impl PreparedCategoricalOlsRhs {
    #[must_use]
    pub const fn values(&self) -> &DenseMatrix {
        &self.values
    }

    #[must_use]
    pub fn into_values(self) -> DenseMatrix {
        self.values
    }
}

/// Prepare intercept-only numeric categorical effects, including interactions.
pub fn prepare_categorical_ols_data(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<PreparedCategoricalOlsData> {
    let mut reservation = SystemCategoricalPreparationReservation;
    prepare_with_reservation(request, loaded, &mut reservation)
}

trait CategoricalPreparationReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;

    fn reserve_string(
        &mut self,
        target: &mut String,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemCategoricalPreparationReservation;

impl CategoricalPreparationReservation for SystemCategoricalPreparationReservation {
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

    fn reserve_string(
        &mut self,
        target: &mut String,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

fn prepare_with_reservation<R: CategoricalPreparationReservation>(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    reservation: &mut R,
) -> Result<PreparedCategoricalOlsData> {
    validate_request(request)?;
    validate_bindings(request)?;
    validate_loaded(request, loaded)?;
    validate_candidate_ordinary_categorical_domain(request, loaded)?;
    validate_candidate_weight_domain(loaded)?;

    let ordinary_categorical = ordered_ordinary_categorical_sources(request, reservation)?;
    let fixed_effect_sources = ordered_fixed_effect_sources(request, reservation)?;
    let mut model = compile_model(request, &ordinary_categorical, reservation)?;
    let source_uses = build_source_uses(request, reservation)?;
    let regressor_views = loaded_regressor_views(loaded, reservation)?;
    let weights = loaded_weight_input(loaded)?;
    let input = NumericPreparationInput {
        candidate: &loaded.candidate,
        outcome: NumericColumnView::with_missing(&loaded.outcome.values, &loaded.outcome.missing),
        regressors: &regressor_views,
        weights,
        adjustment: AdjustmentInput::None,
    };
    let candidate_sample =
        prepare_numeric_sample_with_source_uses(&input, &source_uses, &OlsPreparationPolicy)?;

    let candidate_fe_sources = factorize_sources(
        request,
        &candidate_sample,
        &fixed_effect_sources,
        reservation,
        "unable to reserve candidate FE categorical sources",
    )?;
    let candidate_views = categorical_views(&candidate_fe_sources, reservation)?;
    let (effect_recipes, effect_specs) =
        build_effect_recipes(request, &candidate_fe_sources, reservation)?;
    let recipe_views = recipe_views(&effect_recipes, reservation)?;
    let initially_active = filled_bool(
        candidate_sample.n_active(),
        true,
        reservation,
        "unable to reserve categorical singleton active mask",
    )?;
    let multiplicity = match &candidate_sample.weights {
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values,
        } => SingletonMultiplicity::Frequency(values),
        PreparedWeights::None | PreparedWeights::Provided { .. } => SingletonMultiplicity::Unit,
    };
    let partition_singletons = prune_categorical_partition_singletons(
        &candidate_views,
        &recipe_views,
        &initially_active,
        multiplicity,
        request.keep_singletons,
    )?;
    let singleton = partition_singletons.singleton();
    let fe_topology =
        finalize_categorical_topology(&candidate_views, &recipe_views, &singleton.active)?;
    let num_singletons = singleton.dropped_observations.len();
    let cascade_events = singleton.cascade_events;
    let final_sample = candidate_sample.compact_active_rows(&singleton.active, DropReason::Singleton)?;
    if fe_topology.retained_rows().len() != final_sample.n_active() {
        return Err(invalid(
            "final categorical topology retained-row count differs from compact sample",
        ));
    }

    let categorical_sources = factorize_sources(
        request,
        &final_sample,
        &ordinary_categorical,
        reservation,
        "unable to reserve final ordinary categorical sources",
    )?;
    resolve_model_levels(&mut model, &categorical_sources)?;
    let prepared = PreparedCategoricalOlsData {
        model,
        sample: final_sample,
        categorical_sources,
        fe_topology,
        effect_specs,
        structural_intercept: true,
        report_intercept: request.include_intercept,
        singletons: PreparedOlsSingletons {
            kept_by_request: request.keep_singletons,
            num_singletons,
            cascade_events,
        },
    };
    prepared.validate()?;
    Ok(prepared)
}

fn compile_model<R: CategoricalPreparationReservation>(
    request: &OlsRequestV1,
    ordinary_categorical: &[SourceId],
    reservation: &mut R,
) -> Result<PreparedOlsModel> {
    request.model.validate_executable_metadata()?;
    let report_intercept = request.include_intercept;
    let capacity = request
        .model
        .terms
        .len()
        .checked_add(usize::from(report_intercept))
        .ok_or_else(|| allocation("categorical OLS model-column count overflows"))?;
    let mut columns = Vec::new();
    reservation.reserve_vec(
        &mut columns,
        capacity,
        "unable to reserve categorical OLS model columns",
    )?;
    let mut coefficients = Vec::new();
    reservation.reserve_vec(
        &mut coefficients,
        capacity,
        "unable to reserve categorical OLS coefficient slots",
    )?;

    for term in &request.model.terms {
        let kind = match (term.raw_kind, term.components.as_slice()) {
            (RawTermKind::Variable, [component])
                if component.kind == ComponentKind::Continuous
                    && component.level_state == LevelState::Absent
                    && component.base == MetadataFlag::False
                    && component.time_series_state == TimeSeriesState::None
                    && matches!(component.raw_operator.as_str(), "" | "c" | "co") =>
            {
                PreparedOlsColumnKind::Continuous {
                    source: component.source,
                    prepared_source_index: model_source_index(request, component.source)?,
                }
            }
            (RawTermKind::Factor, [component])
                if component.kind == ComponentKind::Categorical
                    && component.level_state == LevelState::Present
                    && component.time_series_state == TimeSeriesState::None =>
            {
                validate_ordinary_categorical_value(
                    component.level_value,
                    "factor metadata level",
                )?;
                PreparedOlsColumnKind::Categorical {
                    source: component.source,
                    prepared_source_index: model_source_index(request, component.source)?,
                    categorical_source_index: ordered_source_index(
                        ordinary_categorical,
                        component.source,
                        "ordinary categorical source",
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
                reservation.reserve_vec(
                    &mut prepared_components,
                    components.len(),
                    "unable to reserve categorical ordinary-interaction recipe",
                )?;
                for component in components {
                    validate_ordinary_categorical_value(
                        component.level_value,
                        "categorical interaction metadata level",
                    )?;
                    push_reserved(
                        &mut prepared_components,
                        PreparedOlsCategoricalComponent {
                            source: component.source,
                            categorical_source_index: ordered_source_index(
                                ordinary_categorical,
                                component.source,
                                "ordinary categorical interaction source",
                            )?,
                            level_bits: canonical_factor_value(component.level_value).to_bits(),
                            level_code: None,
                        },
                        "categorical ordinary-interaction recipe capacity exhausted",
                    )?;
                }
                PreparedOlsColumnKind::CategoricalInteraction {
                    components: prepared_components,
                }
            }
            _ => {
                return Err(unsupported(
                    "categorical OLS preparation supports only non-time-series continuous terms, numeric factor main effects, and numeric all-categorical ordinary interactions",
                ));
            }
        };

        if term.whole_column_omitted {
            push_reserved(
                &mut coefficients,
                PreparedOlsCoefficient {
                    coefficient_slot: term.coefficient_slot,
                    status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                    generated_column_index: None,
                },
                "categorical OLS coefficient capacity exhausted",
            )?;
        } else {
            let generated_column_index = columns.len();
            push_reserved(
                &mut columns,
                PreparedOlsColumn {
                    coefficient_slot: term.coefficient_slot,
                    kind,
                },
                "categorical OLS model-column capacity exhausted",
            )?;
            push_reserved(
                &mut coefficients,
                PreparedOlsCoefficient {
                    coefficient_slot: term.coefficient_slot,
                    status: PreparedOlsCoefficientStatus::Active,
                    generated_column_index: Some(generated_column_index),
                },
                "categorical OLS coefficient capacity exhausted",
            )?;
        }
    }

    if report_intercept {
        let slot = request
            .model
            .terms
            .len()
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| invalid("categorical OLS intercept slot exceeds u32"))?;
        let coefficient_slot = CoefficientSlot::new(slot)?;
        let generated_column_index = columns.len();
        push_reserved(
            &mut columns,
            PreparedOlsColumn {
                coefficient_slot,
                kind: PreparedOlsColumnKind::Intercept,
            },
            "categorical OLS model-column capacity exhausted",
        )?;
        push_reserved(
            &mut coefficients,
            PreparedOlsCoefficient {
                coefficient_slot,
                status: PreparedOlsCoefficientStatus::Active,
                generated_column_index: Some(generated_column_index),
            },
            "categorical OLS coefficient capacity exhausted",
        )?;
    }

    Ok(PreparedOlsModel {
        coefficients,
        columns,
    })
}

fn validate_request(request: &OlsRequestV1) -> Result<()> {
    if request.absorbed_terms.is_empty() {
        return Err(unsupported(
            "categorical OLS preparation requires at least one explicit absorbed term",
        ));
    }
    for term in &request.absorbed_terms {
        validate_absorbed_term(request, term)?;
    }
    Ok(())
}

fn validate_absorbed_term(request: &OlsRequestV1, term: &AbsorbedTerm) -> Result<()> {
    if !term.intercept
        || term.categorical_sources.is_empty()
        || !term.slope_sources.is_empty()
        || !term.saved_name.is_empty()
    {
        return Err(unsupported(
            "categorical OLS preparation supports only unnamed intercept-only numeric categorical absorbed terms",
        ));
    }
    if term.label.is_empty() {
        return Err(invalid("categorical absorbed term label is empty"));
    }
    for source in &term.categorical_sources {
        model_source_index(request, *source)?;
    }
    Ok(())
}

fn validate_bindings(request: &OlsRequestV1) -> Result<()> {
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(request.bindings.len())
        .map_err(|_| allocation("unable to reserve categorical OLS binding positions"))?;
    let mut bound_sources = Vec::new();
    bound_sources
        .try_reserve_exact(request.model.sources.len())
        .map_err(|_| allocation("unable to reserve categorical OLS bound sources"))?;
    let mut candidate_count = 0_usize;
    let mut outcome_count = 0_usize;
    let mut analytic_count = 0_usize;
    let mut frequency_count = 0_usize;
    let mut probability_count = 0_usize;
    for binding in &request.bindings {
        if binding.stata_var_position == 0 || positions.contains(&binding.stata_var_position) {
            return Err(invalid(
                "categorical OLS bindings require unique positive Stata positions",
            ));
        }
        positions.push(binding.stata_var_position);
        match binding.role {
            OlsInputRole::Candidate => candidate_count += 1,
            OlsInputRole::Outcome => outcome_count += 1,
            OlsInputRole::NumericSource(source) => {
                if bound_sources.contains(&source) {
                    return Err(invalid("categorical OLS source is bound more than once"));
                }
                bound_sources.push(source);
            }
            OlsInputRole::AnalyticWeight => analytic_count += 1,
            OlsInputRole::FrequencyWeight => frequency_count += 1,
            OlsInputRole::ProbabilityWeight => probability_count += 1,
        }
    }
    if candidate_count != 1 || outcome_count != 1 {
        return Err(invalid(
            "categorical OLS request requires one candidate and one outcome binding",
        ));
    }
    if analytic_count + frequency_count + probability_count > 1 {
        return Err(invalid(
            "categorical OLS request permits at most one weight binding",
        ));
    }
    if bound_sources.len() != request.model.sources.len()
        || request
            .model
            .sources
            .iter()
            .any(|source| !bound_sources.contains(&source.id))
    {
        return Err(invalid(
            "categorical OLS request must bind every model source exactly once",
        ));
    }
    Ok(())
}

fn validate_loaded(request: &OlsRequestV1, loaded: &LoadedOlsInput) -> Result<()> {
    let n = loaded.candidate.len();
    validate_loaded_column("outcome", &loaded.outcome, n)?;
    if loaded.numeric_sources.len() != request.model.sources.len() {
        return Err(invalid(
            "loaded categorical OLS source count differs from the model registry",
        ));
    }
    for (index, (source, column)) in loaded.numeric_sources.iter().enumerate() {
        if *source != request.model.sources[index].id {
            return Err(invalid(
                "loaded categorical OLS sources are not in model-registry order",
            ));
        }
        validate_loaded_column("numeric source", column, n)?;
    }
    validate_weight_binding(
        request,
        OlsInputRole::AnalyticWeight,
        loaded.analytic_weight.as_ref(),
        "analytic weight",
        n,
    )?;
    validate_weight_binding(
        request,
        OlsInputRole::FrequencyWeight,
        loaded.frequency_weight.as_ref(),
        "frequency weight",
        n,
    )?;
    validate_weight_binding(
        request,
        OlsInputRole::ProbabilityWeight,
        loaded.probability_weight.as_ref(),
        "probability weight",
        n,
    )?;
    if usize::from(loaded.analytic_weight.is_some())
        + usize::from(loaded.frequency_weight.is_some())
        + usize::from(loaded.probability_weight.is_some())
        > 1
    {
        return Err(invalid(
            "loaded categorical OLS input has more than one weight column",
        ));
    }
    Ok(())
}

fn validate_weight_binding(
    request: &OlsRequestV1,
    role: OlsInputRole,
    loaded: Option<&LoadedNumericColumn>,
    label: &str,
    n: usize,
) -> Result<()> {
    let requested = request.bindings.iter().any(|binding| binding.role == role);
    if requested != loaded.is_some() {
        return Err(invalid(format!(
            "loaded {label} does not match the categorical OLS binding"
        )));
    }
    if let Some(column) = loaded {
        validate_loaded_column(label, column, n)?;
    }
    Ok(())
}

fn validate_loaded_column(name: &str, column: &LoadedNumericColumn, n: usize) -> Result<()> {
    if column.values.len() != n || column.missing.len() != n {
        return Err(invalid(format!(
            "loaded {name} values and missing mask must both have length {n}"
        )));
    }
    Ok(())
}

fn validate_candidate_ordinary_categorical_domain(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<()> {
    let ordinary = ordinary_categorical_source_set(request);
    for (source, column) in &loaded.numeric_sources {
        if !ordinary.contains(source) {
            continue;
        }
        for (row, candidate) in loaded.candidate.iter().copied().enumerate() {
            if !candidate || column.missing[row] || !column.values[row].is_finite() {
                continue;
            }
            validate_ordinary_categorical_value(
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

fn validate_ordinary_categorical_value(value: f64, label: &str) -> Result<()> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
        return Err(invalid_data(format!(
            "{label} must be a finite nonnegative integer"
        )));
    }
    Ok(())
}

fn validate_candidate_weight_domain(loaded: &LoadedOlsInput) -> Result<()> {
    let Some((kind, column)) = loaded_weight(loaded) else {
        return Ok(());
    };
    for (row, candidate) in loaded.candidate.iter().copied().enumerate() {
        if !candidate || column.missing[row] {
            continue;
        }
        let value = column.values[row];
        match kind {
            WeightKind::Analytic if value < 0.0 => {
                return Err(StFixestError::new(
                    ErrorCode::NegativeWeight,
                    format!("analytic weight at observation {} is negative", row + 1),
                ));
            }
            WeightKind::Frequency
                if !value.is_finite()
                    || value <= 0.0
                    || value.fract() != 0.0
                    || value > f64::from(u32::MAX) =>
            {
                return Err(StFixestError::new(
                    ErrorCode::InvalidFrequencyWeight,
                    format!(
                        "frequency weight at observation {} must be a positive u32 integer",
                        row + 1
                    ),
                ));
            }
            WeightKind::Probability if !value.is_finite() || value <= 0.0 => {
                return Err(invalid_data(format!(
                    "probability weight at observation {} must be positive and finite",
                    row + 1
                )));
            }
            WeightKind::Analytic | WeightKind::Frequency | WeightKind::Probability => {}
            WeightKind::Importance => {
                return Err(unsupported(
                    "categorical OLS preparation received an unsupported weight kind",
                ));
            }
        }
    }
    Ok(())
}

fn loaded_regressor_views<'a, R: CategoricalPreparationReservation>(
    loaded: &'a LoadedOlsInput,
    reservation: &mut R,
) -> Result<Vec<NumericColumnView<'a>>> {
    let mut views = Vec::new();
    reservation.reserve_vec(
        &mut views,
        loaded.numeric_sources.len(),
        "unable to reserve categorical OLS numeric source views",
    )?;
    for (_, column) in &loaded.numeric_sources {
        push_reserved(
            &mut views,
            NumericColumnView::with_missing(&column.values, &column.missing),
            "categorical OLS numeric source-view capacity exhausted",
        )?;
    }
    Ok(views)
}

fn loaded_weight_input(loaded: &LoadedOlsInput) -> Result<WeightInput<'_>> {
    match (
        &loaded.analytic_weight,
        &loaded.frequency_weight,
        &loaded.probability_weight,
    ) {
        (None, None, None) => Ok(WeightInput::None),
        (Some(column), None, None) => Ok(WeightInput::Provided {
            kind: WeightKind::Analytic,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        }),
        (None, Some(column), None) => Ok(WeightInput::Provided {
            kind: WeightKind::Frequency,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        }),
        (None, None, Some(column)) => Ok(WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        }),
        _ => Err(invalid(
            "loaded categorical OLS input has more than one weight column",
        )),
    }
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

fn build_source_uses<R: CategoricalPreparationReservation>(
    request: &OlsRequestV1,
    reservation: &mut R,
) -> Result<Vec<NumericSourceUse>> {
    let ordinary = ordinary_source_set(request);
    let fixed_effect = fixed_effect_source_set(request);
    let mut uses = Vec::new();
    reservation.reserve_vec(
        &mut uses,
        request.model.sources.len(),
        "unable to reserve categorical OLS source-use plan",
    )?;
    for source in &request.model.sources {
        let usage = NumericSourceUse {
            ordinary: ordinary.contains(&source.id),
            fixed_effect: fixed_effect.contains(&source.id),
            cluster: false,
        };
        if !usage.ordinary && !usage.fixed_effect {
            return Err(invalid(format!(
                "numeric source {} is unused by ordinary and absorbed terms",
                source.id.get()
            )));
        }
        push_reserved(
            &mut uses,
            usage,
            "categorical OLS source-use capacity exhausted",
        )?;
    }
    Ok(uses)
}

fn ordered_ordinary_categorical_sources<R: CategoricalPreparationReservation>(
    request: &OlsRequestV1,
    reservation: &mut R,
) -> Result<Vec<SourceId>> {
    let selected = ordinary_categorical_source_set(request);
    ordered_sources(request, &selected, reservation, "ordinary categorical sources")
}

fn ordered_fixed_effect_sources<R: CategoricalPreparationReservation>(
    request: &OlsRequestV1,
    reservation: &mut R,
) -> Result<Vec<SourceId>> {
    let selected = fixed_effect_source_set(request);
    ordered_sources(request, &selected, reservation, "fixed-effect categorical sources")
}

fn ordered_sources<R: CategoricalPreparationReservation>(
    request: &OlsRequestV1,
    selected: &[SourceId],
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<SourceId>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, selected.len(), label)?;
    for source in &request.model.sources {
        if selected.contains(&source.id) {
            push_reserved(
                &mut output,
                source.id,
                "ordered categorical source capacity exhausted",
            )?;
        }
    }
    Ok(output)
}

fn ordinary_source_set(request: &OlsRequestV1) -> Vec<SourceId> {
    request
        .model
        .terms
        .iter()
        .flat_map(|term| term.components.iter().map(|component| component.source))
        .fold(Vec::new(), |mut sources, source| {
            if !sources.contains(&source) {
                sources.push(source);
            }
            sources
        })
}

fn ordinary_categorical_source_set(request: &OlsRequestV1) -> Vec<SourceId> {
    request
        .model
        .terms
        .iter()
        .flat_map(|term| &term.components)
        .filter(|component| component.kind == ComponentKind::Categorical)
        .map(|component| component.source)
        .fold(Vec::new(), |mut sources, source| {
            if !sources.contains(&source) {
                sources.push(source);
            }
            sources
        })
}

fn fixed_effect_source_set(request: &OlsRequestV1) -> Vec<SourceId> {
    request
        .absorbed_terms
        .iter()
        .flat_map(|term| term.categorical_sources.iter().copied())
        .fold(Vec::new(), |mut sources, source| {
            if !sources.contains(&source) {
                sources.push(source);
            }
            sources
        })
}

fn factorize_sources<R: CategoricalPreparationReservation>(
    request: &OlsRequestV1,
    sample: &PreparedNumericSample,
    sources: &[SourceId],
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<PreparedCategoricalSource>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, sources.len(), label)?;
    for source in sources {
        let prepared_source_index = model_source_index(request, *source)?;
        let values = sample
            .regressors
            .get(prepared_source_index)
            .ok_or_else(|| invalid("categorical source is absent from the numeric sample"))?;
        push_reserved(
            &mut output,
            PreparedCategoricalSource {
                source: *source,
                prepared_source_index,
                encoded: factorize_f64(values)?,
            },
            "prepared categorical source capacity exhausted",
        )?;
    }
    Ok(output)
}

fn categorical_views<'a, R: CategoricalPreparationReservation>(
    sources: &'a [PreparedCategoricalSource],
    reservation: &mut R,
) -> Result<Vec<CategoricalCodeView<'a>>> {
    let mut views = Vec::new();
    reservation.reserve_vec(
        &mut views,
        sources.len(),
        "unable to reserve categorical FE source views",
    )?;
    for source in sources {
        push_reserved(
            &mut views,
            CategoricalCodeView::new(&source.encoded.codes, source.encoded.n_levels()),
            "categorical FE source-view capacity exhausted",
        )?;
    }
    Ok(views)
}

fn build_effect_recipes<R: CategoricalPreparationReservation>(
    request: &OlsRequestV1,
    candidate_sources: &[PreparedCategoricalSource],
    reservation: &mut R,
) -> Result<(Vec<Vec<usize>>, Vec<CategoricalTopologyDofEffectSpec>)> {
    let mut recipes = Vec::new();
    reservation.reserve_vec(
        &mut recipes,
        request.absorbed_terms.len(),
        "unable to reserve categorical absorbed recipes",
    )?;
    let mut specs = Vec::new();
    reservation.reserve_vec(
        &mut specs,
        request.absorbed_terms.len(),
        "unable to reserve categorical absorbed effect metadata",
    )?;
    for term in &request.absorbed_terms {
        let mut recipe = Vec::new();
        reservation.reserve_vec(
            &mut recipe,
            term.categorical_sources.len(),
            "unable to reserve categorical absorbed source recipe",
        )?;
        for source in &term.categorical_sources {
            let index = candidate_sources
                .iter()
                .position(|candidate| candidate.source == *source)
                .ok_or_else(|| invalid("absorbed categorical source plan disappeared"))?;
            push_reserved(
                &mut recipe,
                index,
                "categorical absorbed source-recipe capacity exhausted",
            )?;
        }
        let mut spec_sources = Vec::new();
        reservation.reserve_vec(
            &mut spec_sources,
            term.categorical_sources.len(),
            "unable to reserve categorical absorbed source identities",
        )?;
        for source in &term.categorical_sources {
            push_reserved(
                &mut spec_sources,
                *source,
                "categorical absorbed source-identity capacity exhausted",
            )?;
        }
        let label = fallible_string_copy(
            &term.label,
            reservation,
            "unable to reserve categorical absorbed label",
        )?;
        push_reserved(
            &mut recipes,
            recipe,
            "categorical absorbed recipe capacity exhausted",
        )?;
        push_reserved(
            &mut specs,
            CategoricalTopologyDofEffectSpec {
                term_id: term.id,
                sources: spec_sources,
                label,
            },
            "categorical absorbed effect-metadata capacity exhausted",
        )?;
    }
    Ok((recipes, specs))
}

fn recipe_views<'a, R: CategoricalPreparationReservation>(
    recipes: &'a [Vec<usize>],
    reservation: &mut R,
) -> Result<Vec<&'a [usize]>> {
    let mut views = Vec::new();
    reservation.reserve_vec(
        &mut views,
        recipes.len(),
        "unable to reserve categorical absorbed recipe views",
    )?;
    for recipe in recipes {
        push_reserved(
            &mut views,
            recipe.as_slice(),
            "categorical absorbed recipe-view capacity exhausted",
        )?;
    }
    Ok(views)
}

fn resolve_model_levels(
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
                *level_code = resolve_level(
                    categorical_sources,
                    *categorical_source_index,
                    *level_bits,
                )?;
            }
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                for component in components {
                    component.level_code = resolve_level(
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

fn resolve_level(
    sources: &[PreparedCategoricalSource],
    source_index: usize,
    level_bits: u64,
) -> Result<Option<u32>> {
    let source = sources
        .get(source_index)
        .ok_or_else(|| invalid("ordinary categorical source is out of range"))?;
    let level = f64::from_bits(level_bits);
    source
        .encoded
        .levels
        .binary_search_by(|candidate| candidate.total_cmp(&level))
        .ok()
        .map(|index| {
            u32::try_from(index)
                .map_err(|_| invalid("ordinary categorical level code exceeds u32"))
        })
        .transpose()
}

fn build_rhs_with_reservation<R: CategoricalPreparationReservation>(
    prepared: &PreparedCategoricalOlsData,
    reservation: &mut R,
) -> Result<PreparedCategoricalOlsRhs> {
    prepared.validate()?;
    let nrows = prepared.sample.n_active();
    let ncols = prepared
        .model
        .columns
        .len()
        .checked_add(1)
        .ok_or_else(|| allocation("categorical OLS RHS column count overflows"))?;
    let elements = nrows
        .checked_mul(ncols)
        .ok_or_else(|| allocation("categorical OLS RHS dimensions overflow"))?;
    let mut values = Vec::new();
    reservation.reserve_vec(
        &mut values,
        elements,
        "unable to reserve categorical OLS RHS",
    )?;
    if values.capacity() < elements {
        return Err(allocation(
            "categorical OLS RHS reservation returned insufficient capacity",
        ));
    }
    values.resize(elements, 0.0);
    for (row, destination) in values.chunks_exact_mut(ncols).enumerate() {
        destination[0] = prepared.sample.outcome[row];
        for generated_column in 0..prepared.model.columns.len() {
            destination[generated_column + 1] = prepared.column_value(generated_column, row)?;
        }
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(invalid_data(
            "categorical OLS RHS construction produced a nonfinite value",
        ));
    }
    Ok(PreparedCategoricalOlsRhs {
        values: DenseMatrix::from_row_major(nrows, ncols, values)?,
    })
}

fn validate_numeric_sample(sample: &PreparedNumericSample) -> Result<()> {
    let nrows = sample.n_active();
    if sample.n_candidate > sample.n_original
        || sample.drop_reasons.len() != sample.n_original
        || sample.outcome.len() != nrows
        || sample.regressors.iter().any(|source| source.len() != nrows)
        || !matches!(sample.adjustment, PreparedAdjustment::None)
    {
        return Err(invalid(
            "prepared categorical OLS numeric sample is not aligned",
        ));
    }
    if sample.outcome.iter().any(|value| !value.is_finite())
        || sample
            .regressors
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
    {
        return Err(invalid_data(
            "prepared categorical OLS numeric values must be finite",
        ));
    }
    if let PreparedWeights::Provided { kind, values } = &sample.weights {
        if values.len() != nrows || *kind == WeightKind::Importance {
            return Err(invalid(
                "prepared categorical OLS weights are not aligned or supported",
            ));
        }
        for value in values {
            if !value.is_finite() || *value <= 0.0 {
                return Err(invalid_data(
                    "prepared categorical OLS weights must be positive and finite",
                ));
            }
            if *kind == WeightKind::Frequency
                && (value.fract() != 0.0 || *value > f64::from(u32::MAX))
            {
                return Err(invalid_data(
                    "prepared categorical OLS frequency weights must be u32 integers",
                ));
            }
        }
    }
    let retained = sample
        .drop_reasons
        .iter()
        .filter(|reason| **reason == DropReason::Retained)
        .count();
    if retained != nrows
        || sample
            .original_row_by_active
            .windows(2)
            .any(|rows| rows[0] >= rows[1])
        || sample.original_row_by_active.iter().any(|row| {
            *row >= sample.n_original || sample.drop_reasons[*row] != DropReason::Retained
        })
    {
        return Err(invalid(
            "prepared categorical OLS retained-row map is inconsistent",
        ));
    }
    Ok(())
}

fn validate_final_categorical_sources(
    sample: &PreparedNumericSample,
    sources: &[PreparedCategoricalSource],
) -> Result<()> {
    for source in sources {
        let raw = sample
            .regressors
            .get(source.prepared_source_index)
            .ok_or_else(|| invalid("ordinary categorical source is out of range"))?;
        if source.encoded.codes.len() != sample.n_active() || raw.len() != sample.n_active() {
            return Err(invalid(
                "ordinary categorical source is not aligned with the final sample",
            ));
        }
        for (row, code) in source.encoded.codes.iter().copied().enumerate() {
            let level = source
                .encoded
                .levels
                .get(code as usize)
                .ok_or_else(|| invalid("ordinary categorical code is out of range"))?;
            if level.to_bits() != canonical_factor_value(raw[row]).to_bits() {
                return Err(invalid(
                    "ordinary categorical codes do not represent their numeric source",
                ));
            }
        }
    }
    Ok(())
}

fn validate_model_recipes(prepared: &PreparedCategoricalOlsData) -> Result<()> {
    let mut generated = 0_usize;
    for coefficient in &prepared.model.coefficients {
        match coefficient.status {
            PreparedOlsCoefficientStatus::Active => {
                if coefficient.generated_column_index != Some(generated)
                    || prepared
                        .model
                        .columns
                        .get(generated)
                        .is_none_or(|column| column.coefficient_slot != coefficient.coefficient_slot)
                {
                    return Err(invalid(
                        "categorical OLS coefficient-to-column map is inconsistent",
                    ));
                }
                generated = generated
                    .checked_add(1)
                    .ok_or_else(|| allocation("categorical OLS generated-column count overflows"))?;
            }
            PreparedOlsCoefficientStatus::SyntaxOmitted => {
                if coefficient.generated_column_index.is_some() {
                    return Err(invalid(
                        "syntax-omitted categorical OLS coefficient has a column",
                    ));
                }
            }
        }
    }
    if generated != prepared.model.columns.len() {
        return Err(invalid(
            "categorical OLS generated columns are not covered exactly once",
        ));
    }
    for column in &prepared.model.columns {
        match &column.kind {
            PreparedOlsColumnKind::Intercept => {}
            PreparedOlsColumnKind::Continuous {
                source,
                prepared_source_index,
            } => {
                if model_source_index_from_id(*source)? != *prepared_source_index
                    || prepared.sample.regressors.get(*prepared_source_index).is_none()
                {
                    return Err(invalid(
                        "categorical OLS continuous recipe identity is inconsistent",
                    ));
                }
            }
            PreparedOlsColumnKind::Categorical {
                source,
                prepared_source_index,
                categorical_source_index,
                level_bits,
                level_code,
            } => validate_categorical_recipe(
                prepared,
                *source,
                *prepared_source_index,
                *categorical_source_index,
                *level_bits,
                *level_code,
            )?,
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                if components.len() < 2 {
                    return Err(invalid(
                        "categorical ordinary interaction has fewer than two components",
                    ));
                }
                for component in components {
                    let categorical = prepared
                        .categorical_sources
                        .get(component.categorical_source_index)
                        .ok_or_else(|| invalid("ordinary interaction source is out of range"))?;
                    validate_categorical_recipe(
                        prepared,
                        component.source,
                        categorical.prepared_source_index,
                        component.categorical_source_index,
                        component.level_bits,
                        component.level_code,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_categorical_recipe(
    prepared: &PreparedCategoricalOlsData,
    source: SourceId,
    prepared_source_index: usize,
    categorical_source_index: usize,
    level_bits: u64,
    level_code: Option<u32>,
) -> Result<()> {
    let categorical = prepared
        .categorical_sources
        .get(categorical_source_index)
        .ok_or_else(|| invalid("ordinary categorical recipe source is out of range"))?;
    if categorical.source != source
        || categorical.prepared_source_index != prepared_source_index
        || model_source_index_from_id(source)? != prepared_source_index
    {
        return Err(invalid(
            "ordinary categorical recipe identity is inconsistent",
        ));
    }
    let requested = canonical_factor_value(f64::from_bits(level_bits));
    if !requested.is_finite() || requested < 0.0 || requested.fract() != 0.0 {
        return Err(invalid(
            "ordinary categorical recipe level is invalid",
        ));
    }
    let resolved = categorical
        .encoded
        .levels
        .binary_search_by(|candidate| candidate.total_cmp(&requested))
        .ok()
        .and_then(|index| u32::try_from(index).ok());
    if resolved != level_code {
        return Err(invalid(
            "ordinary categorical recipe level code is inconsistent",
        ));
    }
    Ok(())
}

fn categorical_indicator(
    sources: &[PreparedCategoricalSource],
    source_index: usize,
    level_code: Option<u32>,
    row: usize,
) -> Result<f64> {
    let Some(level_code) = level_code else {
        return Ok(0.0);
    };
    let code = sources
        .get(source_index)
        .and_then(|source| source.encoded.codes.get(row))
        .copied()
        .ok_or_else(|| invalid("ordinary categorical indicator source is out of range"))?;
    Ok(f64::from(code == level_code))
}

fn model_source_index(request: &OlsRequestV1, source: SourceId) -> Result<usize> {
    let index = model_source_index_from_id(source)?;
    if request.model.sources.get(index).is_none_or(|entry| entry.id != source) {
        return Err(invalid("model source registry is not contiguous and ordered"));
    }
    Ok(index)
}

fn model_source_index_from_id(source: SourceId) -> Result<usize> {
    usize::try_from(source.get() - 1).map_err(|_| invalid("source id does not fit usize"))
}

fn ordered_source_index(sources: &[SourceId], source: SourceId, label: &str) -> Result<usize> {
    sources
        .iter()
        .position(|candidate| *candidate == source)
        .ok_or_else(|| invalid(format!("{label} is absent")))
}

fn filled_bool<R: CategoricalPreparationReservation>(
    len: usize,
    value: bool,
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<bool>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, len, label)?;
    if output.capacity() < len {
        return Err(allocation(
            "categorical OLS boolean reservation returned insufficient capacity",
        ));
    }
    output.resize(len, value);
    Ok(output)
}

fn fallible_string_copy<R: CategoricalPreparationReservation>(
    input: &str,
    reservation: &mut R,
    label: &'static str,
) -> Result<String> {
    let mut output = String::new();
    reservation.reserve_string(&mut output, input.len(), label)?;
    if output.capacity() < input.len() {
        return Err(allocation(
            "categorical OLS string reservation returned insufficient capacity",
        ));
    }
    output.push_str(input);
    Ok(output)
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn canonical_factor_value(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

struct OlsPreparationPolicy;

impl NumericDomainPolicy for OlsPreparationPolicy {
    fn outcome(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }

    fn weight(&self, _row: usize, kind: WeightKind, value: f64) -> Result<DomainDecision> {
        match kind {
            WeightKind::Analytic if value == 0.0 => Ok(DomainDecision::Drop),
            WeightKind::Analytic if value < 0.0 => Err(StFixestError::new(
                ErrorCode::NegativeWeight,
                "analytic weight is negative",
            )),
            WeightKind::Analytic => Ok(DomainDecision::Retain),
            WeightKind::Frequency
                if value > 0.0 && value.fract() == 0.0 && value <= f64::from(u32::MAX) =>
            {
                Ok(DomainDecision::Retain)
            }
            WeightKind::Frequency => Err(StFixestError::new(
                ErrorCode::InvalidFrequencyWeight,
                "frequency weight must be a positive u32 integer",
            )),
            WeightKind::Probability if value > 0.0 => Ok(DomainDecision::Retain),
            WeightKind::Probability => Err(invalid_data(
                "probability weight must be positive and finite",
            )),
            WeightKind::Importance => Err(unsupported(
                "categorical OLS preparation received an unsupported weight kind",
            )),
        }
    }

    fn offset(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
        Err(invalid("categorical OLS preparation received an offset"))
    }

    fn exposure(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
        Err(invalid(
            "categorical OLS preparation received an exposure",
        ))
    }
}

fn invalid(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidData, message)
}

fn unsupported(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Unsupported, message)
}

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}
