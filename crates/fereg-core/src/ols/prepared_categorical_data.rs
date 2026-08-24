//! Integrated prepared-data path for numeric categorical absorbed interactions.
//!
//! The existing `prepare_ols_data` boundary remains the authority for request
//! validation, candidate-sample construction, missing-value accounting, weight
//! semantics, and ordinary model recipes. This module uses the categorical
//! absorbed-request planner to run that boundary with singleton deletion
//! deliberately disabled, then applies the true single-source/interaction
//! singleton fixed point exactly once and constructs the final deduplicated FE
//! topology. Public estimator dispatch remains disabled.

use crate::data::{DenseMatrix, DropReason, PreparedNumericSample, PreparedWeights, WeightKind};
use crate::error::{ErrorCode, FeregError, Result};
use crate::factorize::{factorize_f64, EncodedFactor};
use crate::fe::{
    absorb_finalized_categorical_intercept_slope_map_certified,
    absorb_finalized_categorical_topology_map_certified, finalize_categorical_topology,
    plan_categorical_absorbed_request, prune_categorical_partition_singletons, BorrowedMapWeights,
    BoundCategoricalAbsorbEffect, CategoricalCodeView, CertifiedBorrowedMap,
    FinalizedCategoricalTopology, MapConfig, SingletonMultiplicity,
};
use crate::metadata::SourceId;
use crate::prepared_categorical_rebase::{
    rebase_categorical_model, PreparedCategoricalOlsReportPlan,
};
use crate::topology_dof::{
    compute_categorical_topology_absorbed_dof, CategoricalTopologyDofEffectSpec,
    CategoricalTopologyDofPolicy, CategoricalTopologyDofReport, CategoricalTopologyDofSlopeSpec,
};

use crate::ols::prepare_ols_data_with_cluster_sources;

use super::{
    LoadedOlsInput, OlsRequestV1, PreparedCategoricalSource, PreparedOlsCoefficientStatus,
    PreparedOlsColumnKind, PreparedOlsData, PreparedOlsModel, PreparedOlsSingletons,
};

/// Complete native preparation result for numeric categorical absorbed
/// effects, including interactions and one guarded intercept-plus-slope term.
#[derive(Debug, PartialEq)]
pub struct PreparedCategoricalOlsData {
    model: PreparedOlsModel,
    sample: PreparedNumericSample,
    /// Ordinary categorical sources on the final sample. Sources that also
    /// participate in an absorbed term borrow their N-row codes from
    /// `fe_topology`; only their small candidate-level table is retained here.
    categorical_sources: Vec<PreparedCategoricalModelSource>,
    fe_topology: FinalizedCategoricalTopology,
    effect_specs: Vec<CategoricalTopologyDofEffectSpec>,
    structural_intercept: bool,
    report_intercept: bool,
    singletons: PreparedOlsSingletons,
    report_plan: PreparedCategoricalOlsReportPlan,
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
    pub fn categorical_sources(&self) -> &[PreparedCategoricalModelSource] {
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
    pub fn has_absorbed_slope(&self) -> bool {
        self.effect_specs
            .iter()
            .any(|effect| effect.slope.is_some())
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

    #[must_use]
    pub const fn report_plan(&self) -> &PreparedCategoricalOlsReportPlan {
        &self.report_plan
    }

    /// Evaluate one generated ordinary model column on the final compact row
    /// axis without materializing an observation-scale model column.
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
            } => self.categorical_indicator(*categorical_source_index, *level_code, active_row),
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                for component in components {
                    if self.categorical_indicator(
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

    /// Build one observation-major `[Y, X]` batch directly from prepared
    /// recipes. No intermediate N-row ordinary columns are allocated.
    pub fn build_rhs(&self) -> Result<PreparedCategoricalOlsRhs> {
        self.validate()?;
        let nrows = self.sample.n_active();
        let ncols = self
            .model
            .columns
            .len()
            .checked_add(1)
            .ok_or_else(|| allocation("categorical OLS RHS column count overflows"))?;
        let elements = nrows
            .checked_mul(ncols)
            .ok_or_else(|| allocation("categorical OLS RHS dimensions overflow"))?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(elements)
            .map_err(|_| allocation("unable to reserve categorical OLS RHS"))?;
        if values.capacity() < elements {
            return Err(allocation(
                "categorical OLS RHS reservation returned insufficient capacity",
            ));
        }
        values.resize(elements, 0.0);
        for (row, destination) in values.chunks_exact_mut(ncols).enumerate() {
            destination[0] = self.sample.outcome[row];
            for generated_column in 0..self.model.columns.len() {
                destination[generated_column + 1] = self.column_value(generated_column, row)?;
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

    /// Residualize the complete prepared batch through the strict borrowed MAP
    /// solver. A failure returns no caller-visible partially transformed batch.
    pub fn absorb_rhs_map(&self, config: MapConfig) -> Result<CertifiedBorrowedMap> {
        self.validate()?;
        let rhs = self.build_rhs()?.into_values();
        let weights = match &self.sample.weights {
            PreparedWeights::None => BorrowedMapWeights::Unit,
            PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
        };
        if self.has_absorbed_slope() {
            let slopes = self.slope_views()?;
            absorb_finalized_categorical_intercept_slope_map_certified(
                &self.fe_topology,
                &slopes,
                weights,
                rhs,
                config,
            )
        } else {
            absorb_finalized_categorical_topology_map_certified(
                &self.fe_topology,
                weights,
                rhs,
                config,
            )
        }
    }

    /// Compute absorbed rank from the finalized interaction-aware topology.
    /// Cluster dimensions, when supplied, are already compact final-sample
    /// interaction codes.
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

    /// Revalidate the complete prepared artifact before numerical use.
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
        let slopes = self.slope_views()?;
        if slopes.iter().flatten().count() > 1 {
            return Err(invalid(
                "prepared categorical OLS contains more than one absorbed slope",
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
        validate_categorical_model_sources(
            &self.sample,
            &self.fe_topology,
            &self.categorical_sources,
        )?;
        validate_model_recipes(&self.model, &self.categorical_sources, &self.fe_topology)?;
        validate_report_plan(&self.model, &self.report_plan)?;
        Ok(())
    }

    pub(crate) fn slope_views(&self) -> Result<Vec<Option<&[f64]>>> {
        let mut output = Vec::new();
        output
            .try_reserve_exact(self.effect_specs.len())
            .map_err(|_| allocation("unable to reserve prepared absorbed-slope views"))?;
        for effect in &self.effect_specs {
            let slope = effect
                .slope
                .as_ref()
                .map(|slope| {
                    let source_index = usize::try_from(slope.source.get() - 1)
                        .map_err(|_| invalid("absorbed-slope source index overflows"))?;
                    self.sample
                        .regressors
                        .get(source_index)
                        .map(Vec::as_slice)
                        .ok_or_else(|| invalid("absorbed-slope source is out of range"))
                })
                .transpose()?;
            if slope.is_some_and(|values| {
                values.len() != self.sample.n_active()
                    || values.iter().any(|value| !value.is_finite())
            }) {
                return Err(invalid_data(
                    "prepared absorbed-slope source is not finite and aligned",
                ));
            }
            output.push(slope);
        }
        Ok(output)
    }

    fn categorical_indicator(
        &self,
        source_index: usize,
        level_code: Option<u32>,
        row: usize,
    ) -> Result<f64> {
        let Some(level_code) = level_code else {
            return Ok(0.0);
        };
        let source = self
            .categorical_sources
            .get(source_index)
            .ok_or_else(|| invalid("ordinary categorical source is out of range"))?;
        let code = source
            .codes(&self.fe_topology)?
            .get(row)
            .copied()
            .ok_or_else(|| invalid("ordinary categorical source row is out of range"))?;
        Ok(f64::from(code == level_code))
    }
}

/// One ordinary categorical source on the final sample.
#[derive(Debug, PartialEq)]
pub struct PreparedCategoricalModelSource {
    source: SourceId,
    prepared_source_index: usize,
    storage: PreparedCategoricalModelSourceStorage,
}

impl PreparedCategoricalModelSource {
    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn prepared_source_index(&self) -> usize {
        self.prepared_source_index
    }

    #[must_use]
    pub const fn is_topology_backed(&self) -> bool {
        matches!(
            &self.storage,
            PreparedCategoricalModelSourceStorage::Topology { .. }
        )
    }

    pub fn codes<'a>(&'a self, topology: &'a FinalizedCategoricalTopology) -> Result<&'a [u32]> {
        match &self.storage {
            PreparedCategoricalModelSourceStorage::Topology {
                candidate_source_index,
                ..
            } => Ok(topology
                .source_for_candidate(*candidate_source_index)?
                .codes()),
            PreparedCategoricalModelSourceStorage::Owned { encoded } => Ok(&encoded.codes),
        }
    }

    pub fn n_levels(&self, topology: &FinalizedCategoricalTopology) -> Result<usize> {
        match &self.storage {
            PreparedCategoricalModelSourceStorage::Topology {
                candidate_source_index,
                ..
            } => Ok(topology
                .source_for_candidate(*candidate_source_index)?
                .n_levels()),
            PreparedCategoricalModelSourceStorage::Owned { encoded } => Ok(encoded.n_levels()),
        }
    }

    pub(crate) fn resolve_level(
        &self,
        topology: &FinalizedCategoricalTopology,
        requested_level: f64,
    ) -> Result<Option<u32>> {
        let requested_level = canonical_factor_value(requested_level);
        match &self.storage {
            PreparedCategoricalModelSourceStorage::Topology {
                candidate_source_index,
                candidate_levels,
            } => {
                let Some(candidate_level) = candidate_levels
                    .binary_search_by(|value| value.total_cmp(&requested_level))
                    .ok()
                else {
                    return Ok(None);
                };
                let candidate_level = u32::try_from(candidate_level)
                    .map_err(|_| invalid("candidate categorical level exceeds u32"))?;
                let final_source = topology.source_for_candidate(*candidate_source_index)?;
                final_source
                    .original_levels()
                    .binary_search(&candidate_level)
                    .ok()
                    .map(|index| {
                        u32::try_from(index)
                            .map_err(|_| invalid("final categorical level exceeds u32"))
                    })
                    .transpose()
            }
            PreparedCategoricalModelSourceStorage::Owned { encoded } => encoded
                .levels
                .binary_search_by(|value| value.total_cmp(&requested_level))
                .ok()
                .map(|index| {
                    u32::try_from(index)
                        .map_err(|_| invalid("ordinary categorical level exceeds u32"))
                })
                .transpose(),
        }
    }

    fn level_value(
        &self,
        topology: &FinalizedCategoricalTopology,
        final_level: u32,
    ) -> Result<f64> {
        match &self.storage {
            PreparedCategoricalModelSourceStorage::Topology {
                candidate_source_index,
                candidate_levels,
            } => {
                let final_source = topology.source_for_candidate(*candidate_source_index)?;
                let candidate_level = final_source
                    .original_levels()
                    .get(final_level as usize)
                    .copied()
                    .ok_or_else(|| invalid("final categorical level is out of range"))?;
                candidate_levels
                    .get(candidate_level as usize)
                    .copied()
                    .ok_or_else(|| invalid("candidate categorical level is out of range"))
            }
            PreparedCategoricalModelSourceStorage::Owned { encoded } => encoded
                .levels
                .get(final_level as usize)
                .copied()
                .ok_or_else(|| invalid("ordinary categorical level is out of range")),
        }
    }
}

#[derive(Debug, PartialEq)]
enum PreparedCategoricalModelSourceStorage {
    /// Codes and final compact level identities are owned once by the FE
    /// topology. Candidate levels are O(number of levels), not O(N).
    Topology {
        candidate_source_index: usize,
        candidate_levels: Vec<f64>,
    },
    /// An ordinary categorical source not used by any absorbed effect.
    Owned { encoded: EncodedFactor<f64> },
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

/// Prepare numeric categorical intercept effects, including interactions and
/// the guarded single intercept-plus-slope channel.
///
/// This is a developer/core boundary. It does not activate any public estimator
/// or Stata result-transport action.
pub fn prepare_categorical_ols_data(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<PreparedCategoricalOlsData> {
    prepare_categorical_ols_data_with_cluster_sources(request, loaded, &[])
}

pub(crate) fn prepare_categorical_ols_data_with_cluster_sources(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<PreparedCategoricalOlsData> {
    let plan = plan_categorical_absorbed_request(request)?;
    let availability = prepare_ols_data_with_cluster_sources(
        plan.availability_request(),
        loaded,
        cluster_sources,
    )?;
    if !availability.singletons.kept_by_request
        || availability.singletons.num_singletons != 0
        || availability.singletons.cascade_events != 0
    {
        return Err(invalid(
            "categorical source-availability preparation applied singleton deletion",
        ));
    }

    let bound_effects = plan.bind_effects(&availability.categorical_sources)?;
    let candidate_views = categorical_views(&availability.categorical_sources)?;
    let recipe_views = effect_recipe_views(&bound_effects)?;
    let initially_active = fallible_filled_bool(availability.sample.n_active(), true)?;
    let multiplicity = match &availability.sample.weights {
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values,
        } => SingletonMultiplicity::Frequency(values.as_slice()),
        PreparedWeights::None | PreparedWeights::Provided { .. } => SingletonMultiplicity::Unit,
    };
    let singleton_result = prune_categorical_partition_singletons(
        &candidate_views,
        &recipe_views,
        &initially_active,
        multiplicity,
        plan.requested_keep_singletons(),
    )?;
    let (singleton, _, _) = singleton_result.into_parts();
    let fe_topology =
        finalize_categorical_topology(&candidate_views, &recipe_views, &singleton.active)?;
    let num_singletons = singleton.dropped_observations.len();
    let cascade_events = singleton.cascade_events;

    let PreparedOlsData {
        model: availability_model,
        sample,
        categorical_sources: candidate_sources,
        ..
    } = availability;
    let final_sources = final_categorical_sources(&availability_model, cluster_sources)?;
    let model_source_plans = plan_model_sources(candidate_sources, &final_sources, &fe_topology)?;
    let sample = sample.compact_active_rows(&singleton.active, DropReason::Singleton)?;
    if fe_topology.retained_rows().len() != sample.n_active() {
        return Err(invalid(
            "final categorical topology retained-row count differs from compact sample",
        ));
    }
    let categorical_sources = build_final_model_sources(model_source_plans, &sample, &fe_topology)?;
    let rebased = rebase_categorical_model(
        &request.model,
        &availability_model,
        &categorical_sources,
        &fe_topology,
        request.include_intercept,
    )?;
    let effect_specs = effect_specs(&bound_effects, &request.model)?;
    let prepared = PreparedCategoricalOlsData {
        model: rebased.model,
        sample,
        categorical_sources,
        fe_topology,
        effect_specs,
        structural_intercept: true,
        report_intercept: request.include_intercept,
        singletons: PreparedOlsSingletons {
            kept_by_request: plan.requested_keep_singletons(),
            num_singletons,
            cascade_events,
        },
        report_plan: rebased.report_plan,
    };
    prepared.validate()?;
    Ok(prepared)
}

#[derive(Debug)]
struct CategoricalModelSourcePlan {
    source: SourceId,
    prepared_source_index: usize,
    storage: CategoricalModelSourcePlanStorage,
}

#[derive(Debug)]
enum CategoricalModelSourcePlanStorage {
    Topology {
        candidate_source_index: usize,
        candidate_levels: Vec<f64>,
    },
    Owned,
}

fn plan_model_sources(
    candidate_sources: Vec<PreparedCategoricalSource>,
    ordinary_sources: &[SourceId],
    topology: &FinalizedCategoricalTopology,
) -> Result<Vec<CategoricalModelSourcePlan>> {
    if topology.candidate_to_compacted_source().len() != candidate_sources.len() {
        return Err(invalid(
            "categorical topology source map and candidate source table differ in length",
        ));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(ordinary_sources.len())
        .map_err(|_| allocation("unable to reserve categorical model-source plan"))?;
    for (candidate_source_index, candidate) in candidate_sources.into_iter().enumerate() {
        if !ordinary_sources.contains(&candidate.source) {
            continue;
        }
        let storage = if topology.candidate_to_compacted_source()[candidate_source_index].is_some()
        {
            CategoricalModelSourcePlanStorage::Topology {
                candidate_source_index,
                candidate_levels: candidate.encoded.levels,
            }
        } else {
            CategoricalModelSourcePlanStorage::Owned
        };
        push_reserved(
            &mut output,
            CategoricalModelSourcePlan {
                source: candidate.source,
                prepared_source_index: candidate.prepared_source_index,
                storage,
            },
            "categorical model-source plan capacity exhausted",
        )?;
    }
    if output.len() != ordinary_sources.len()
        || ordinary_sources
            .iter()
            .any(|source| !output.iter().any(|plan| plan.source == *source))
    {
        return Err(invalid(
            "candidate categorical source table is missing an ordinary model source",
        ));
    }
    Ok(output)
}

fn build_final_model_sources(
    plans: Vec<CategoricalModelSourcePlan>,
    sample: &PreparedNumericSample,
    topology: &FinalizedCategoricalTopology,
) -> Result<Vec<PreparedCategoricalModelSource>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(plans.len())
        .map_err(|_| allocation("unable to reserve final categorical model sources"))?;
    for plan in plans {
        let storage = match plan.storage {
            CategoricalModelSourcePlanStorage::Topology {
                candidate_source_index,
                candidate_levels,
            } => {
                topology.source_for_candidate(candidate_source_index)?;
                PreparedCategoricalModelSourceStorage::Topology {
                    candidate_source_index,
                    candidate_levels,
                }
            }
            CategoricalModelSourcePlanStorage::Owned => {
                let values = sample
                    .regressors
                    .get(plan.prepared_source_index)
                    .ok_or_else(|| invalid("ordinary categorical raw source is out of range"))?;
                PreparedCategoricalModelSourceStorage::Owned {
                    encoded: factorize_f64(values)?,
                }
            }
        };
        push_reserved(
            &mut output,
            PreparedCategoricalModelSource {
                source: plan.source,
                prepared_source_index: plan.prepared_source_index,
                storage,
            },
            "final categorical model-source capacity exhausted",
        )?;
    }
    Ok(output)
}

fn ordinary_categorical_sources(model: &PreparedOlsModel) -> Result<Vec<SourceId>> {
    let capacity = model.columns.iter().try_fold(0_usize, |total, column| {
        let additional = match &column.kind {
            PreparedOlsColumnKind::Categorical { .. } => 1,
            PreparedOlsColumnKind::CategoricalInteraction { components } => components.len(),
            PreparedOlsColumnKind::Intercept | PreparedOlsColumnKind::Continuous { .. } => 0,
        };
        total
            .checked_add(additional)
            .ok_or_else(|| allocation("ordinary categorical recipe arity overflows"))
    })?;
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(capacity)
        .map_err(|_| allocation("unable to reserve ordinary categorical source identities"))?;
    for column in &model.columns {
        match &column.kind {
            PreparedOlsColumnKind::Categorical { source, .. } => {
                push_unique_source(&mut sources, *source)?;
            }
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                for component in components {
                    push_unique_source(&mut sources, component.source)?;
                }
            }
            PreparedOlsColumnKind::Intercept | PreparedOlsColumnKind::Continuous { .. } => {}
        }
    }
    Ok(sources)
}

fn final_categorical_sources(
    model: &PreparedOlsModel,
    cluster_sources: &[SourceId],
) -> Result<Vec<SourceId>> {
    let mut sources = ordinary_categorical_sources(model)?;
    let additional = cluster_sources
        .iter()
        .filter(|source| !sources.contains(source))
        .count();
    sources
        .try_reserve_exact(additional)
        .map_err(|_| allocation("unable to reserve final categorical cluster sources"))?;
    for source in cluster_sources {
        push_unique_source(&mut sources, *source)?;
    }
    Ok(sources)
}

fn push_unique_source(sources: &mut Vec<SourceId>, source: SourceId) -> Result<()> {
    if sources.contains(&source) {
        return Ok(());
    }
    if sources.len() == sources.capacity() {
        return Err(allocation(
            "ordinary categorical source-identity capacity exhausted",
        ));
    }
    sources.push(source);
    Ok(())
}

fn effect_specs(
    effects: &[BoundCategoricalAbsorbEffect],
    model: &crate::metadata::CompiledTermGraph,
) -> Result<Vec<CategoricalTopologyDofEffectSpec>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(effects.len())
        .map_err(|_| allocation("unable to reserve categorical absorbed DoF metadata"))?;
    for effect in effects {
        let slope = effect
            .slope_source
            .map(|source| {
                let source_name = model
                    .sources
                    .iter()
                    .find(|entry| entry.id == source)
                    .map(|entry| entry.canonical_name.as_str())
                    .ok_or_else(|| invalid("absorbed-slope source identity is absent"))?;
                let categorical_name = model
                    .sources
                    .iter()
                    .find(|entry| effect.sources.first() == Some(&entry.id))
                    .map(|entry| entry.canonical_name.as_str())
                    .ok_or_else(|| invalid("absorbed intercept source identity is absent"))?;
                Ok(CategoricalTopologyDofSlopeSpec {
                    source,
                    label: absorbed_slope_label(categorical_name, source_name)?,
                })
            })
            .transpose()?;
        let intercept_label = if slope.is_some() {
            let source = effect
                .sources
                .first()
                .ok_or_else(|| invalid("absorbed slope lacks a categorical source"))?;
            model
                .sources
                .iter()
                .find(|entry| entry.id == *source)
                .map(|entry| entry.canonical_name.as_str())
                .ok_or_else(|| invalid("absorbed intercept source identity is absent"))?
        } else {
            effect.label.as_str()
        };
        push_reserved(
            &mut output,
            CategoricalTopologyDofEffectSpec {
                term_id: effect.term_id,
                sources: fallible_copy_slice(
                    &effect.sources,
                    "unable to reserve categorical absorbed DoF source identities",
                )?,
                label: fallible_string_copy(
                    intercept_label,
                    "unable to reserve categorical absorbed DoF label",
                )?,
                slope,
            },
            "categorical absorbed DoF metadata capacity exhausted",
        )?;
    }
    Ok(output)
}

fn absorbed_slope_label(categorical: &str, slope: &str) -> Result<String> {
    let capacity = categorical
        .len()
        .checked_add(slope.len())
        .and_then(|value| value.checked_add(3))
        .ok_or_else(|| allocation("categorical absorbed-slope label length overflows"))?;
    let mut output = String::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| allocation("unable to reserve categorical absorbed-slope label"))?;
    output.push_str(categorical);
    output.push_str("#c.");
    output.push_str(slope);
    Ok(output)
}

fn categorical_views(
    sources: &[PreparedCategoricalSource],
) -> Result<Vec<CategoricalCodeView<'_>>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(sources.len())
        .map_err(|_| allocation("unable to reserve candidate categorical source views"))?;
    for source in sources {
        push_reserved(
            &mut output,
            CategoricalCodeView::new(&source.encoded.codes, source.encoded.n_levels()),
            "candidate categorical source-view capacity exhausted",
        )?;
    }
    Ok(output)
}

fn effect_recipe_views(effects: &[BoundCategoricalAbsorbEffect]) -> Result<Vec<&[usize]>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(effects.len())
        .map_err(|_| allocation("unable to reserve categorical effect recipe views"))?;
    for effect in effects {
        if effect.categorical_source_indices.is_empty() {
            return Err(invalid(
                "bound categorical absorbed effect has an empty source recipe",
            ));
        }
        push_reserved(
            &mut output,
            effect.categorical_source_indices.as_slice(),
            "categorical effect recipe-view capacity exhausted",
        )?;
    }
    Ok(output)
}

fn validate_categorical_model_sources(
    sample: &PreparedNumericSample,
    topology: &FinalizedCategoricalTopology,
    sources: &[PreparedCategoricalModelSource],
) -> Result<()> {
    let mut seen = Vec::new();
    seen.try_reserve_exact(sources.len())
        .map_err(|_| allocation("unable to reserve categorical source validation identities"))?;
    for source in sources {
        if seen.contains(&source.source) {
            return Err(invalid(
                "ordinary categorical source table contains duplicate source identities",
            ));
        }
        push_reserved(
            &mut seen,
            source.source,
            "categorical source validation identity capacity exhausted",
        )?;
        let expected_prepared_index = usize::try_from(source.source.get() - 1)
            .map_err(|_| invalid("ordinary categorical source id does not fit usize"))?;
        if source.prepared_source_index != expected_prepared_index {
            return Err(invalid(
                "ordinary categorical source index does not match its source id",
            ));
        }
        let raw = sample
            .regressors
            .get(source.prepared_source_index)
            .ok_or_else(|| invalid("ordinary categorical raw source is out of range"))?;
        let codes = source.codes(topology)?;
        let n_levels = source.n_levels(topology)?;
        if codes.len() != sample.n_active() || raw.len() != sample.n_active() {
            return Err(invalid(
                "ordinary categorical source is not aligned with the final sample",
            ));
        }
        if !codes.is_empty() && n_levels == 0 {
            return Err(invalid(
                "nonempty ordinary categorical source declares zero levels",
            ));
        }
        for (row, code) in codes.iter().copied().enumerate() {
            if code as usize >= n_levels {
                return Err(invalid_data(
                    "ordinary categorical source contains an out-of-range code",
                ));
            }
            let level = source.level_value(topology, code)?;
            if level.to_bits() != canonical_factor_value(raw[row]).to_bits() {
                return Err(invalid(
                    "ordinary categorical source codes do not represent their raw values",
                ));
            }
        }
    }
    Ok(())
}

fn validate_model_recipes(
    model: &PreparedOlsModel,
    sources: &[PreparedCategoricalModelSource],
    topology: &FinalizedCategoricalTopology,
) -> Result<()> {
    let mut generated = 0_usize;
    for coefficient in &model.coefficients {
        match coefficient.status {
            PreparedOlsCoefficientStatus::Active => {
                if coefficient.generated_column_index != Some(generated)
                    || model.columns.get(generated).is_none_or(|column| {
                        column.coefficient_slot != coefficient.coefficient_slot
                    })
                {
                    return Err(invalid(
                        "categorical OLS coefficient-to-column map is inconsistent",
                    ));
                }
                generated = generated.checked_add(1).ok_or_else(|| {
                    allocation("categorical OLS generated-column count overflows")
                })?;
            }
            PreparedOlsCoefficientStatus::SyntaxOmitted => {
                if coefficient.generated_column_index.is_some() {
                    return Err(invalid(
                        "syntax-omitted categorical OLS coefficient has a generated column",
                    ));
                }
            }
        }
    }
    if generated != model.columns.len() {
        return Err(invalid(
            "categorical OLS generated columns are not covered exactly once",
        ));
    }
    for column in &model.columns {
        match &column.kind {
            PreparedOlsColumnKind::Intercept => {}
            PreparedOlsColumnKind::Continuous {
                source,
                prepared_source_index,
            } => {
                let expected = usize::try_from(source.get() - 1)
                    .map_err(|_| invalid("continuous source id does not fit usize"))?;
                if expected != *prepared_source_index {
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
            } => {
                let categorical = sources
                    .get(*categorical_source_index)
                    .ok_or_else(|| invalid("categorical recipe source is out of range"))?;
                if categorical.source != *source
                    || categorical.prepared_source_index != *prepared_source_index
                    || categorical.resolve_level(topology, f64::from_bits(*level_bits))?
                        != *level_code
                {
                    return Err(invalid("categorical OLS factor recipe is inconsistent"));
                }
            }
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                if components.len() < 2 {
                    return Err(invalid(
                        "categorical ordinary interaction has fewer than two components",
                    ));
                }
                for component in components {
                    let categorical = sources
                        .get(component.categorical_source_index)
                        .ok_or_else(|| invalid("interaction recipe source is out of range"))?;
                    if categorical.source != component.source
                        || categorical
                            .resolve_level(topology, f64::from_bits(component.level_bits))?
                            != component.level_code
                    {
                        return Err(invalid(
                            "categorical OLS interaction recipe is inconsistent",
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_report_plan(
    model: &PreparedOlsModel,
    report_plan: &PreparedCategoricalOlsReportPlan,
) -> Result<()> {
    if report_plan.public_coefficient_count() != model.coefficients.len()
        || report_plan
            .original_slots()
            .windows(2)
            .any(|pair| pair[0].get() >= pair[1].get())
        || report_plan
            .original_slots()
            .last()
            .is_some_and(|slot| slot.get() as usize > report_plan.original_coefficient_count())
    {
        return Err(invalid(
            "categorical OLS final-sample report plan is inconsistent",
        ));
    }
    for (index, coefficient) in model.coefficients.iter().enumerate() {
        if coefficient.coefficient_slot.get() as usize != index + 1 {
            return Err(invalid(
                "categorical OLS rebased coefficient slots are not contiguous",
            ));
        }
    }
    Ok(())
}

fn validate_numeric_sample(sample: &PreparedNumericSample) -> Result<()> {
    let nrows = sample.n_active();
    if sample.n_candidate > sample.n_original
        || sample.drop_reasons.len() != sample.n_original
        || sample.outcome.len() != nrows
        || sample.regressors.iter().any(|source| source.len() != nrows)
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

fn fallible_filled_bool(len: usize, value: bool) -> Result<Vec<bool>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(len)
        .map_err(|_| allocation("unable to reserve categorical singleton active mask"))?;
    if output.capacity() < len {
        return Err(allocation(
            "categorical singleton active-mask reservation returned insufficient capacity",
        ));
    }
    output.resize(len, value);
    Ok(output)
}

fn fallible_copy_slice<T: Copy>(input: &[T], label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| allocation(label))?;
    if output.capacity() < input.len() {
        return Err(allocation(
            "categorical OLS vector reservation returned insufficient capacity",
        ));
    }
    output.extend_from_slice(input);
    Ok(output)
}

fn fallible_string_copy(input: &str, label: &'static str) -> Result<String> {
    let mut output = String::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| allocation(label))?;
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

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}
