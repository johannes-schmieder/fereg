//! Integrated prepared-data path for numeric categorical absorbed interactions.
//!
//! The numeric sample-preparation boundary remains the authority for request
//! validation, candidate-sample construction, missing-value accounting, weight
//! semantics, and ordinary model recipes. This module uses the categorical
//! absorbed-request planner to classify complete source uses, then applies the
//! true single-source/interaction
//! singleton fixed point exactly once and constructs the final deduplicated FE
//! topology. This boundary is shared by public estimation and developer gates.

use crate::data::{DenseMatrix, DropReason, PreparedNumericSample, PreparedWeights, WeightKind};
use crate::error::{ErrorCode, FeregError, Result};
use crate::factorize::{factorize_f64, EncodedFactor};
use crate::fe::{
    absorb_finalized_categorical_slope_map_certified,
    absorb_finalized_categorical_topology_map_certified, finalize_categorical_topology,
    plan_categorical_absorbed_request, prune_categorical_partition_singletons,
    recover_finalized_categorical_effects_cg_certified,
    recover_finalized_categorical_effects_from_coefficients_selected_certified,
    recover_finalized_categorical_effects_lsmr_certified,
    recover_finalized_categorical_effects_map_certified,
    recover_finalized_categorical_effects_sd_lsmr_certified,
    recover_finalized_two_effects_graph_certified, BorrowedMapWeights,
    BoundCategoricalAbsorbEffect, CategoricalAbsorbRequestPlan, CategoricalAbsorbedEffectView,
    CategoricalCodeView, CertifiedBorrowedMap, FeCoefficientBatch, FinalizedCategoricalTopology,
    LsmrConfig, MapConfig, RecoveredCategoricalEffects, SingletonMultiplicity, SingletonResult,
};
use crate::memory::{estimate_intercept_recovery_memory, FeSolverMemoryBudget};
use crate::metadata::SourceId;
use crate::prepared_categorical_rebase::{
    rebase_categorical_model, PreparedCategoricalOlsReportPlan,
};
use crate::topology_dof::{
    compute_categorical_topology_absorbed_dof_with_slopes, CategoricalTopologyDofEffectSpec,
    CategoricalTopologyDofPolicy, CategoricalTopologyDofReport, CategoricalTopologyDofSlopeSpec,
};
use rayon::prelude::*;

use crate::ols::{
    finish_categorical_availability, prepare_ols_data_with_cluster_sources, CategoricalAvailability,
};

use super::{
    LoadedOlsInput, OlsRequestV1, PreparedCategoricalSource, PreparedOlsCoefficientStatus,
    PreparedOlsColumnKind, PreparedOlsData, PreparedOlsModel, PreparedOlsSingletons,
};

const CATEGORICAL_SLOPE_MAP_TOLERANCE_CEILING: f64 = 1e-12;
const CATEGORICAL_RECOVERY_CG_ITERATION_CAP: usize = 256;

// Keep the owned sample behind a module-private field: even the surrounding
// preparation code cannot forge this proof or obtain mutable access. Source
// values and row/weight metadata therefore retain their constructor checks.
mod validated_sample {
    use super::{validate_numeric_sample, PreparedNumericSample, Result};

    #[derive(Debug, PartialEq)]
    pub(super) struct ValidatedSample(PreparedNumericSample);

    impl ValidatedSample {
        pub(super) fn new(sample: PreparedNumericSample) -> Result<Self> {
            validate_numeric_sample(&sample)?;
            Ok(Self(sample))
        }

        pub(super) const fn get(&self) -> &PreparedNumericSample {
            &self.0
        }
    }

    impl std::ops::Deref for ValidatedSample {
        type Target = PreparedNumericSample;

        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
}

/// Complete native preparation result for numeric categorical absorbed
/// effects, including interactions and arbitrary ordered absorbed slopes.
#[derive(Debug, PartialEq)]
pub struct PreparedCategoricalOlsData {
    model: PreparedOlsModel,
    sample: validated_sample::ValidatedSample,
    /// Ordinary categorical sources on the final sample. Sources that also
    /// participate in an absorbed term borrow their N-row codes from
    /// `fe_topology`; only their small candidate-level table is retained here.
    categorical_sources: Vec<PreparedCategoricalModelSource>,
    fe_topology: crate::fe::ValidatedFinalizedTopology,
    effect_specs: Vec<CategoricalTopologyDofEffectSpec>,
    structural_intercept: bool,
    report_intercept: bool,
    singletons: PreparedOlsSingletons,
    report_plan: PreparedCategoricalOlsReportPlan,
}

/// Only the allocation-free topology check may overlap fitting. The immutable
/// owner keeps the validated sample and topology fixed until DoF creation;
/// metadata checks and their allocations still run at the original boundary.
pub(crate) struct ValidatedCategoricalDofInputs<'a> {
    prepared: &'a PreparedCategoricalOlsData,
    topology: crate::fe::ValidatedSingleTopology<'a>,
}

impl ValidatedCategoricalDofInputs<'_> {
    pub(crate) fn compute(
        self,
        policy: CategoricalTopologyDofPolicy,
        cluster_dimensions: &[&[u32]],
    ) -> Result<CategoricalTopologyDofReport> {
        self.prepared.validate_after_topology()?;
        compute_categorical_topology_absorbed_dof_with_slopes(
            &self.prepared.fe_topology,
            &self.prepared.effect_specs,
            self.prepared.structural_intercept,
            policy,
            cluster_dimensions,
            &self.prepared.sample.regressors,
            Some(self.topology),
        )
    }
}

impl PreparedCategoricalOlsData {
    /// Retained heap buffers, with topology-backed codes counted only at their
    /// owner. Construction scratch and numerical/dependency frames are separate.
    #[must_use]
    pub fn owned_buffer_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        self.sample.owned_buffer_bytes()
            + bytes(&self.model.columns)
            + bytes(&self.model.coefficients)
            + self.nested_metadata_capacity_bytes()
            + bytes(&self.categorical_sources)
            + self
                .categorical_sources
                .iter()
                .map(|source| match &source.storage {
                    PreparedCategoricalModelSourceStorage::Topology {
                        candidate_levels, ..
                    } => bytes(candidate_levels),
                    PreparedCategoricalModelSourceStorage::Owned { encoded } => {
                        bytes(&encoded.codes) + bytes(&encoded.levels)
                    }
                })
                .sum::<u128>()
            + self.fe_topology.owned_buffer_bytes()
            + self.report_plan.owned_buffer_bytes()
    }

    /// Executable recipe payloads and FE metadata; excludes numeric topology
    /// and top-level finite descriptors, charged separately by the planner.
    pub(crate) fn nested_metadata_capacity_bytes(&self) -> u128 {
        use std::mem::size_of;
        self.model.nested_recipe_capacity_bytes()
            + self.effect_specs.capacity() as u128
                * size_of::<CategoricalTopologyDofEffectSpec>() as u128
            + self
                .effect_specs
                .iter()
                .map(|effect| {
                    effect.sources.capacity() as u128 * size_of::<SourceId>() as u128
                        + effect.label.capacity() as u128
                        + effect.slopes.capacity() as u128
                            * size_of::<CategoricalTopologyDofSlopeSpec>() as u128
                        + effect
                            .slopes
                            .iter()
                            .map(|slope| slope.label.capacity() as u128)
                            .sum::<u128>()
                })
                .sum::<u128>()
    }

    #[must_use]
    pub const fn model(&self) -> &PreparedOlsModel {
        &self.model
    }

    #[must_use]
    pub const fn sample(&self) -> &PreparedNumericSample {
        self.sample.get()
    }

    #[must_use]
    pub fn categorical_sources(&self) -> &[PreparedCategoricalModelSource] {
        &self.categorical_sources
    }

    #[must_use]
    pub const fn fe_topology(&self) -> &FinalizedCategoricalTopology {
        self.fe_topology.get()
    }

    /// Borrow one already validated partition, including categorical interactions.
    /// The immutable owner binds its codes and membership for this entire borrow.
    pub(crate) fn validated_single_effect(&self) -> Option<crate::fe::BorrowedInterceptEffect<'_>> {
        crate::fe::ValidatedBorrowedInterceptSource::from_finalized_single_effect(&self.fe_topology)
            .map(|source| source.effect())
    }

    pub(crate) fn validated_single_topology(
        &self,
    ) -> Result<Option<crate::fe::ValidatedSingleTopology<'_>>> {
        self.fe_topology.single_source()
    }

    #[must_use]
    pub fn effect_specs(&self) -> &[CategoricalTopologyDofEffectSpec] {
        &self.effect_specs
    }

    #[must_use]
    pub fn has_absorbed_slope(&self) -> bool {
        self.effect_specs
            .iter()
            .any(|effect| !effect.slopes.is_empty())
    }

    #[must_use]
    pub fn has_absorbed_intercept(&self) -> bool {
        self.effect_specs.iter().any(|effect| effect.intercept)
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
        self.build_rhs_with_response(&self.sample.outcome)
    }

    /// Build one observation-major `[response, X]` batch while borrowing a
    /// caller-supplied working response. The generated-column fill and sole
    /// dense allocation are identical to [`Self::build_rhs`].
    pub(crate) fn build_rhs_with_response(
        &self,
        response: &[f64],
    ) -> Result<PreparedCategoricalOlsRhs> {
        self.validate_with_reusable_topology(false)?;
        let nrows = self.sample.n_active();
        if response.len() != nrows {
            return Err(invalid(
                "categorical OLS response is not aligned on the active observation axis",
            ));
        }
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
        if let Some(columns) = crate::ols::NumericOlsColumns::bind(&self.model, &self.sample)? {
            columns.fill_reserved_rhs(response, &mut values)?;
        } else {
            values.resize(elements, 0.0);
            for (row, destination) in values.chunks_exact_mut(ncols).enumerate() {
                destination[0] = response[row];
                for generated_column in 0..self.model.columns.len() {
                    destination[generated_column + 1] = self.column_value(generated_column, row)?;
                }
            }
        }
        if values.par_iter().any(|value| !value.is_finite()) {
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
        self.validate_with_reusable_topology(false)?;
        let rhs = self.build_rhs()?.into_values();
        let weights = match &self.sample.weights {
            PreparedWeights::None => BorrowedMapWeights::Unit,
            PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
        };
        if self.has_absorbed_slope() {
            let effects = self.effect_views()?;
            absorb_finalized_categorical_slope_map_certified(
                &self.fe_topology,
                &effects,
                weights,
                rhs,
                categorical_slope_map_config(config),
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

    pub(crate) fn slope_map_config(&self, config: MapConfig) -> MapConfig {
        categorical_slope_map_config(config)
    }

    /// Recover the absorbed sum and selected normalized FE channels from an
    /// observation-aligned absorbed target.
    pub fn recover_absorbed_effects_map(
        &self,
        target: &[f64],
        requested_channels: &[bool],
        config: MapConfig,
    ) -> Result<RecoveredCategoricalEffects> {
        let weights = match &self.sample.weights {
            PreparedWeights::None => BorrowedMapWeights::Unit,
            PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
        };
        self.recover_absorbed_effects_map_with_weights(weights, target, requested_channels, config)
    }

    /// Crate-private recovery boundary for estimators whose final curvature
    /// weights differ from the preparation weights. Public OLS continues to
    /// call the wrapper above with the prepared weights.
    pub(crate) fn recover_absorbed_effects_map_with_weights(
        &self,
        weights: BorrowedMapWeights<'_>,
        target: &[f64],
        requested_channels: &[bool],
        config: MapConfig,
    ) -> Result<RecoveredCategoricalEffects> {
        self.validate_with_reusable_topology(false)?;
        let effects = self.effect_views()?;
        recover_finalized_categorical_effects_map_certified(
            &self.fe_topology,
            &effects,
            weights,
            target,
            requested_channels,
            categorical_slope_map_config(config),
        )
    }

    /// Recover saved FE channels without repeating symmetric observation-space
    /// MAP. Retained fit coefficients are the zero-extra-solve path.
    /// Eligible unit-weight two-effect targets next try certified graph
    /// propagation. Iterative recovery uses reconstruction-aware CG, then a
    /// deterministic four-sweep SD-Kaczmarz pilot plus LSMR remainder solve;
    /// one slope partition first tries independent group solves, and other
    /// slopes use strict LSMR. Strict MAP remains the final certified fallback.
    #[allow(clippy::too_many_arguments)]
    pub fn recover_absorbed_effects_fast(
        &self,
        target: &[f64],
        requested_channels: &[bool],
        retained: Option<&FeCoefficientBatch>,
        known_connected: bool,
        materialize_absorbed_sum: bool,
        lsmr: LsmrConfig,
        map: MapConfig,
        memory_budget: Option<FeSolverMemoryBudget>,
    ) -> Result<RecoveredCategoricalEffects> {
        let weights = match &self.sample.weights {
            PreparedWeights::None => BorrowedMapWeights::Unit,
            PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
        };
        self.recover_absorbed_effects_fast_with_weights(
            weights,
            target,
            requested_channels,
            retained,
            known_connected,
            materialize_absorbed_sum,
            lsmr,
            map,
            memory_budget,
        )
    }

    /// Repair a failed fitted-model span recovery using one fresh, complete
    /// coefficient product. The original model scale controls subtraction
    /// roundoff; output finishing still uses the unchanged strict span check.
    #[allow(clippy::too_many_arguments)]
    pub fn recover_model_effects(
        &self,
        target: &[f64],
        residuals: &[f64],
        model_scale: f64,
        requested: &[bool],
        known_connected: bool,
        materialize_absorbed_sum: bool,
        map: MapConfig,
        memory_budget: Option<FeSolverMemoryBudget>,
    ) -> Result<RecoveredCategoricalEffects> {
        if !self.has_absorbed_intercept() || self.has_absorbed_slope() {
            return Err(invalid("model FE recovery requires intercept-only effects"));
        }
        let validation_bytes = self.fe_topology.validation_workspace_bytes()?;
        if memory_budget.is_some_and(|budget| !budget.fits(validation_bytes)) {
            return Err(allocation(
                "model FE recovery topology exceeds the memory budget",
            ));
        }
        let _validated = self.validate_with_reusable_topology(true)?;
        let effects = self.effect_views()?;
        let mut levels = Vec::new();
        levels
            .try_reserve_exact(effects.len())
            .map_err(|_| allocation("model recovery level counts"))?;
        for &source in self.fe_topology.effect_to_topology() {
            levels.push(self.fe_topology.registry().source(source)?.n_levels());
        }
        let memory = estimate_intercept_recovery_memory(
            self.sample.n_active(),
            &levels,
            requested.iter().filter(|&&value| value).count(),
            rayon::current_num_threads(),
            None,
        )?;
        if memory_budget.is_some_and(|budget| !budget.fits(memory.cg_peak_bytes)) {
            return Err(allocation(
                "model FE recovery exceeds the joint memory budget",
            ));
        }
        let weights = match &self.sample.weights {
            PreparedWeights::None => BorrowedMapWeights::Unit,
            PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
        };
        let recovered = crate::fe::recover_finalized_model_effects_certified(
            &self.fe_topology,
            &effects,
            weights,
            target,
            residuals,
            model_scale,
            requested,
            known_connected,
            materialize_absorbed_sum,
            map,
        )?;
        Ok(
            recovered.with_solver_workspace_peak(crate::memory::SolverWorkspacePeak::from_bytes(
                memory.cg_peak_bytes,
            )?),
        )
    }

    /// Explicit-weight counterpart retaining the ordinary OLS recovery policy,
    /// including the shared LSMR routes.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn recover_absorbed_effects_fast_with_weights(
        &self,
        weights: BorrowedMapWeights<'_>,
        target: &[f64],
        requested_channels: &[bool],
        retained: Option<&FeCoefficientBatch>,
        known_connected: bool,
        materialize_absorbed_sum: bool,
        lsmr: LsmrConfig,
        map: MapConfig,
        memory_budget: Option<FeSolverMemoryBudget>,
    ) -> Result<RecoveredCategoricalEffects> {
        self.recover_absorbed_effects_fast_with_weights_inner(
            weights,
            target,
            requested_channels,
            retained,
            known_connected,
            materialize_absorbed_sum,
            lsmr,
            map,
            false,
            true,
            memory_budget,
        )
    }

    /// PPML saved-FE recovery boundary. Allocation failures from a preferred
    /// recovery route are terminal r909 errors rather than reasons to attempt a
    /// second solver whose allocator behavior could mask the original failure.
    /// PPML also skips the pinned external LSMR implementation, whose upstream
    /// workspaces are not yet fallibly allocated, and uses the certified
    /// repository-owned MAP recovery fallback instead. Ordinary OLS retains
    /// the existing LSMR-first recovery policy above.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn recover_ppml_absorbed_effects_fast_with_weights(
        &self,
        weights: BorrowedMapWeights<'_>,
        target: &[f64],
        requested_channels: &[bool],
        retained: Option<&FeCoefficientBatch>,
        known_connected: bool,
        materialize_absorbed_sum: bool,
        lsmr: LsmrConfig,
        map: MapConfig,
    ) -> Result<RecoveredCategoricalEffects> {
        self.recover_absorbed_effects_fast_with_weights_inner(
            weights,
            target,
            requested_channels,
            retained,
            known_connected,
            materialize_absorbed_sum,
            lsmr,
            map,
            true,
            false,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn recover_absorbed_effects_fast_with_weights_inner(
        &self,
        weights: BorrowedMapWeights<'_>,
        target: &[f64],
        requested_channels: &[bool],
        retained: Option<&FeCoefficientBatch>,
        known_connected: bool,
        materialize_absorbed_sum: bool,
        lsmr: LsmrConfig,
        map: MapConfig,
        propagate_allocation: bool,
        allow_lsmr: bool,
        memory_budget: Option<FeSolverMemoryBudget>,
    ) -> Result<RecoveredCategoricalEffects> {
        let validation_bytes = self.fe_topology.validation_workspace_bytes()?;
        if memory_budget.is_some_and(|budget| !budget.fits(validation_bytes)) {
            return Err(allocation(
                "saved-FE topology validation exceeds the joint memory budget",
            ));
        }
        let validated_single = self.validate_with_reusable_topology(true)?;
        let effects = self.effect_views()?;
        let recovery_memory = if !effects.is_empty()
            && effects
                .iter()
                .all(|effect| effect.has_intercept() && effect.slopes().is_empty())
        {
            let mut levels = Vec::new();
            levels.try_reserve_exact(effects.len()).map_err(|_| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "unable to reserve recovery level counts",
                )
            })?;
            for &source in self.fe_topology.effect_to_topology() {
                levels.push(self.fe_topology.registry().source(source)?.n_levels());
            }
            Some(estimate_intercept_recovery_memory(
                self.sample.n_active(),
                &levels,
                requested_channels
                    .iter()
                    .filter(|&&requested| requested)
                    .count(),
                rayon::current_num_threads(),
                lsmr.local_size,
            )?)
        } else if allow_lsmr && !effects.is_empty() {
            // PPML admits recovery in its parent/frame planner and disables this
            // LSMR route. Keep its existing admission and failure policy.
            let mut shapes = Vec::new();
            shapes
                .try_reserve_exact(effects.len())
                .map_err(|_| allocation("unable to reserve slope recovery shapes"))?;
            for (effect, &source) in effects.iter().zip(self.fe_topology.effect_to_topology()) {
                shapes.push((
                    self.fe_topology.registry().source(source)?.n_levels(),
                    effect.has_intercept(),
                    effect.slopes().len(),
                ));
            }
            Some(crate::memory::estimate_slope_recovery_memory(
                self.sample.n_active(),
                &shapes,
                requested_channels
                    .iter()
                    .filter(|&&requested| requested)
                    .count(),
                self.fe_topology.compacted_sources().len(),
                validation_bytes,
                rayon::current_num_threads(),
                lsmr.local_size,
            )?)
        } else {
            None
        };
        let fits = |bytes| memory_budget.is_none_or(|budget| budget.fits(bytes));
        let mut attempted_peak = crate::memory::SolverWorkspacePeak::ZERO;
        fn record_attempt(
            peak: &mut crate::memory::SolverWorkspacePeak,
            bytes: Option<u128>,
        ) -> Result<()> {
            *peak = peak.max(match bytes {
                Some(bytes) => crate::memory::SolverWorkspacePeak::from_bytes(bytes)?,
                None => crate::memory::SolverWorkspacePeak::UNMODELED,
            });
            Ok(())
        }

        // A budget excludes only that attempted route. In particular, a valid
        // retained product or the bounded MAP fallback need not reserve an
        // unused Schwarz hierarchy. Each failed attempt drops its workspace
        // before the next admitted route starts.
        if let Some(coefficients) = retained
            .filter(|_| recovery_memory.is_none_or(|memory| fits(memory.retained_peak_bytes)))
        {
            record_attempt(
                &mut attempted_peak,
                recovery_memory.map(|memory| memory.retained_peak_bytes),
            )?;
            match recover_finalized_categorical_effects_from_coefficients_selected_certified(
                &self.fe_topology,
                &effects,
                weights,
                target,
                requested_channels,
                coefficients,
                map.tolerance,
                known_connected,
                materialize_absorbed_sum,
            ) {
                Ok(recovered) => return Ok(recovered.with_solver_workspace_peak(attempted_peak)),
                Err(error) if propagate_allocation && error.code == ErrorCode::Allocation => {
                    return Err(error);
                }
                Err(_) => {}
            }
        }
        // OLS may solve independent slope groups directly. The common finishing
        // certificate remains authoritative; failed attempts leave target intact.
        if allow_lsmr
            && effects.len() == 1
            && !effects[0].slopes().is_empty()
            && effects[0].channels() <= 4
            && target.len() >= 16_384
            && map.max_iterations > 0
        {
            let levels = self
                .fe_topology
                .registry()
                .source(self.fe_topology.effect_to_topology()[0])?
                .n_levels();
            let extra = crate::fe::single_slope_recovery_extra_bytes(
                target.len(),
                levels,
                effects[0].channels(),
                rayon::current_num_threads(),
            )?;
            let direct_bytes = recovery_memory
                .and_then(|memory| memory.retained_peak_bytes.checked_add(extra))
                .ok_or_else(|| allocation("single slope recovery envelope overflows"))?;
            for cache_geometry in [true, false] {
                if cache_geometry && rayon::current_num_threads() != 1 {
                    continue;
                }
                let bytes = if cache_geometry {
                    direct_bytes
                        .checked_add(crate::fe::single_slope_recovery_geometry_bytes(
                            levels,
                            effects[0].channels(),
                        )?)
                        .ok_or_else(|| allocation("cached recovery geometry envelope overflows"))?
                } else {
                    direct_bytes
                };
                if !fits(bytes) {
                    continue;
                }
                record_attempt(&mut attempted_peak, Some(bytes))?;
                match crate::fe::recover_finalized_single_slope_direct_certified(
                    &self.fe_topology,
                    &effects,
                    weights,
                    target,
                    requested_channels,
                    map.tolerance,
                    materialize_absorbed_sum,
                    cache_geometry,
                ) {
                    Ok(recovered) => {
                        return Ok(recovered.with_solver_workspace_peak(attempted_peak))
                    }
                    Err(error) if propagate_allocation && error.code == ErrorCode::Allocation => {
                        return Err(error)
                    }
                    Err(_) => {}
                }
            }
        }
        if effects
            .iter()
            .all(|effect| effect.has_intercept() && effect.slopes().is_empty())
        {
            if allow_lsmr
                && effects.len() == 2
                && target.len() >= 16_384
                && rayon::current_num_threads() >= 8
                && recovery_memory.is_some_and(|memory| fits(memory.retained_peak_bytes))
            {
                record_attempt(
                    &mut attempted_peak,
                    recovery_memory.map(|memory| memory.retained_peak_bytes),
                )?;
                match recover_finalized_two_effects_graph_certified(
                    &self.fe_topology,
                    &effects,
                    weights,
                    target,
                    requested_channels,
                    map.tolerance,
                    known_connected,
                    materialize_absorbed_sum,
                ) {
                    Ok(recovered) => {
                        return Ok(recovered.with_solver_workspace_peak(attempted_peak))
                    }
                    Err(error) if propagate_allocation && error.code == ErrorCode::Allocation => {
                        return Err(error)
                    }
                    Err(_) => {}
                }
            }
            if effects.len() > 1 && recovery_memory.is_none_or(|memory| fits(memory.cg_peak_bytes))
            {
                record_attempt(
                    &mut attempted_peak,
                    recovery_memory.map(|memory| memory.cg_peak_bytes),
                )?;
                match recover_finalized_categorical_effects_cg_certified(
                    &self.fe_topology,
                    &effects,
                    weights,
                    target,
                    requested_channels,
                    MapConfig {
                        max_iterations: map
                            .max_iterations
                            .min(CATEGORICAL_RECOVERY_CG_ITERATION_CAP),
                        ..categorical_slope_map_config(map)
                    },
                ) {
                    Ok(recovered) => {
                        return Ok(recovered.with_solver_workspace_peak(attempted_peak))
                    }
                    Err(error) if propagate_allocation && error.code == ErrorCode::Allocation => {
                        return Err(error);
                    }
                    Err(_) => {}
                }
            }
            if allow_lsmr && recovery_memory.is_none_or(|memory| fits(memory.sd_lsmr_peak_bytes)) {
                record_attempt(
                    &mut attempted_peak,
                    recovery_memory.map(|memory| memory.sd_lsmr_peak_bytes),
                )?;
                match recover_finalized_categorical_effects_sd_lsmr_certified(
                    &self.fe_topology,
                    &effects,
                    weights,
                    target,
                    requested_channels,
                    lsmr,
                    categorical_slope_map_config(map),
                    validated_single,
                ) {
                    Ok(recovered) => {
                        return Ok(recovered.with_solver_workspace_peak(attempted_peak))
                    }
                    Err(error) if propagate_allocation && error.code == ErrorCode::Allocation => {
                        return Err(error);
                    }
                    Err(_) => {}
                }
            }
        }
        if allow_lsmr && recovery_memory.is_none_or(|memory| fits(memory.lsmr_peak_bytes)) {
            record_attempt(
                &mut attempted_peak,
                recovery_memory.map(|memory| memory.lsmr_peak_bytes),
            )?;
            match recover_finalized_categorical_effects_lsmr_certified(
                &self.fe_topology,
                &effects,
                weights,
                target,
                requested_channels,
                lsmr,
                map.tolerance,
            ) {
                Ok(recovered) => return Ok(recovered.with_solver_workspace_peak(attempted_peak)),
                Err(error) if propagate_allocation && error.code == ErrorCode::Allocation => {
                    return Err(error);
                }
                Err(_) => {}
            }
        }
        if let Some(memory) = recovery_memory.filter(|memory| !fits(memory.map_peak_bytes)) {
            return Err(FeregError::new(
                ErrorCode::Allocation,
                format!(
                    "saved-FE recovery requires at least {} incremental bytes and does not fit the joint execution/recovery memory plan",
                    memory.map_peak_bytes,
                ),
            ));
        }
        record_attempt(
            &mut attempted_peak,
            recovery_memory.map(|memory| memory.map_peak_bytes),
        )?;
        recover_finalized_categorical_effects_map_certified(
            &self.fe_topology,
            &effects,
            weights,
            target,
            requested_channels,
            categorical_slope_map_config(map),
        )
        .map(|recovered| recovered.with_solver_workspace_peak(attempted_peak))
    }

    /// Compute absorbed rank from the finalized interaction-aware topology.
    /// Cluster dimensions, when supplied, are already compact final-sample
    /// interaction codes.
    pub fn absorbed_dof(
        &self,
        policy: CategoricalTopologyDofPolicy,
        cluster_dimensions: &[&[u32]],
    ) -> Result<CategoricalTopologyDofReport> {
        let validated_single = self.validate_with_reusable_topology(true)?;
        compute_categorical_topology_absorbed_dof_with_slopes(
            &self.fe_topology,
            &self.effect_specs,
            self.structural_intercept,
            policy,
            cluster_dimensions,
            &self.sample.regressors,
            validated_single,
        )
    }

    /// Explicitly revalidate the complete prepared artifact, including the
    /// immutable sample checked at construction.
    pub fn validate(&self) -> Result<()> {
        validate_numeric_sample(&self.sample)?;
        self.fe_topology.validate()?;
        self.validate_after_topology()
    }

    pub(crate) fn validate_dof_inputs_for_overlap(
        &self,
    ) -> Result<Option<ValidatedCategoricalDofInputs<'_>>> {
        if self.fe_topology.compacted_sources().len() != 1
            || self.fe_topology.registry().sources().len() != 1
            || self.fe_topology.effect_to_topology().len() != 1
            || self.fe_topology.registry().sources()[0]
                .partition()
                .is_interaction()
        {
            return Ok(None);
        }
        Ok(self
            .validated_single_topology()?
            .map(|topology| ValidatedCategoricalDofInputs {
                prepared: self,
                topology,
            }))
    }

    fn validate_with_reusable_topology(
        &self,
        reuse_single: bool,
    ) -> Result<Option<crate::fe::ValidatedSingleTopology<'_>>> {
        let validated_single = if reuse_single {
            self.validated_single_topology()?
        } else {
            None
        };
        self.validate_after_topology()?;
        Ok(validated_single)
    }

    fn validate_after_topology(&self) -> Result<()> {
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
        let _effects = self.effect_views()?;
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

    pub(crate) fn effect_views(&self) -> Result<Vec<CategoricalAbsorbedEffectView<'_>>> {
        let mut output = Vec::new();
        output
            .try_reserve_exact(self.effect_specs.len())
            .map_err(|_| allocation("unable to reserve prepared absorbed-effect views"))?;
        for effect in &self.effect_specs {
            let mut slopes = Vec::new();
            slopes
                .try_reserve_exact(effect.slopes.len())
                .map_err(|_| allocation("unable to reserve prepared absorbed-slope views"))?;
            for slope in &effect.slopes {
                let values = self.numeric_source_values(slope.source)?;
                if slopes.len() == slopes.capacity() {
                    return Err(allocation(
                        "prepared absorbed-slope view capacity exhausted",
                    ));
                }
                slopes.push(values);
            }
            if output.len() == output.capacity() {
                return Err(allocation(
                    "prepared absorbed-effect view capacity exhausted",
                ));
            }
            output.push(CategoricalAbsorbedEffectView::new(
                effect.intercept,
                slopes,
            )?);
        }
        Ok(output)
    }

    pub(crate) fn numeric_source_values(&self, source: SourceId) -> Result<&[f64]> {
        let source_index = usize::try_from(source.get() - 1)
            .map_err(|_| invalid("prepared numeric source index overflows"))?;
        let values = self
            .sample
            .regressors
            .get(source_index)
            .map(Vec::as_slice)
            .ok_or_else(|| invalid("prepared numeric source is out of range"))?;
        // Alignment and finiteness belong to the immutable sample owner;
        // only the requested source identity still needs a lookup here.
        Ok(values)
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

fn categorical_slope_map_config(mut config: MapConfig) -> MapConfig {
    // Group-specific slopes can amplify the final raw finite-score moment even
    // after every FE moment has reached the generic MAP tolerance. Keep the
    // registered finite-score gate unchanged and require a tighter upstream
    // projection certificate for this path.
    config.tolerance = config
        .tolerance
        .min(CATEGORICAL_SLOPE_MAP_TOLERANCE_CEILING);
    config
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

/// Consume source columns before constructing the unchanged interaction/slope
/// topology and applying its singleton fixed point.
pub fn prepare_categorical_ols_data_owned(
    request: &OlsRequestV1,
    loaded: LoadedOlsInput,
) -> Result<PreparedCategoricalOlsData> {
    prepare_categorical_ols_data_owned_with_cluster_sources(request, loaded, &[])
}

pub(crate) fn prepare_categorical_ols_data_owned_with_cluster_sources(
    request: &OlsRequestV1,
    loaded: LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<PreparedCategoricalOlsData> {
    let mut plan = plan_categorical_absorbed_request(request)?;
    let (model, sample, early_factor) = crate::ols::prepare_ols_numeric_sample_owned(
        plan.availability_request(),
        loaded,
        cluster_sources,
    )?;
    plan.discard_classified_slope_categories()?;
    let availability = finish_categorical_availability(
        plan.availability_request(),
        cluster_sources,
        model,
        sample,
        request
            .absorbed_terms
            .iter()
            .all(|term| term.categorical_sources.len() == 1),
        early_factor,
    )?;
    finish_prepare_categorical_ols_data(request, cluster_sources, false, plan, availability)
}

/// PPML preparation boundary that also admits an empty absorbed topology.
///
/// The public categorical OLS constructor continues to require explicit
/// absorbed terms. With no absorbed terms this path preserves `_cons` as the
/// ordinary generated intercept column produced by categorical rebasing; it
/// never synthesizes an absorbed constant or reclassifies the coefficient
/// after fitting.
pub(crate) fn prepare_ppml_categorical_ols_data(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<PreparedCategoricalOlsData> {
    if !request.absorbed_terms.is_empty() {
        return prepare_categorical_ols_data_with_cluster_sources_policy(
            request,
            loaded,
            cluster_sources,
            true,
        );
    }

    let availability = prepare_ols_data_with_cluster_sources(request, loaded, cluster_sources)?;
    if availability.singletons.num_singletons != 0 || availability.singletons.cascade_events != 0 {
        return Err(invalid(
            "empty categorical PPML topology unexpectedly applied singleton deletion",
        ));
    }
    let candidate_views = categorical_views(&availability.categorical_sources)?;
    let active = fallible_filled_bool(availability.sample.n_active(), true)?;
    let fe_topology = finalize_categorical_topology(&candidate_views, &[], &active)?;

    let PreparedOlsData {
        model: availability_model,
        sample,
        categorical_sources: candidate_sources,
        singletons,
        ..
    } = availability;
    let final_sources = final_categorical_sources(&availability_model, cluster_sources)?;
    let model_source_plans =
        plan_model_sources(candidate_sources, &final_sources, &fe_topology, true)?;
    let categorical_sources = build_final_model_sources(model_source_plans, &sample, &fe_topology)?;
    let report_intercept = request.include_intercept;
    let rebased = rebase_categorical_model(
        &request.model,
        &availability_model,
        &categorical_sources,
        &fe_topology,
        report_intercept,
    )?;
    let sample = validated_sample::ValidatedSample::new(sample)?;
    let fe_topology = crate::fe::ValidatedFinalizedTopology::new(fe_topology)?;
    let prepared = PreparedCategoricalOlsData {
        model: rebased.model,
        sample,
        categorical_sources,
        fe_topology,
        effect_specs: Vec::new(),
        structural_intercept: report_intercept,
        report_intercept,
        singletons,
        report_plan: rebased.report_plan,
    };
    prepared.validate_with_reusable_topology(false)?;
    Ok(prepared)
}

pub(crate) fn prepare_categorical_ols_data_with_cluster_sources(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    cluster_sources: &[SourceId],
) -> Result<PreparedCategoricalOlsData> {
    prepare_categorical_ols_data_with_cluster_sources_policy(
        request,
        loaded,
        cluster_sources,
        false,
    )
}

fn prepare_categorical_ols_data_with_cluster_sources_policy(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    cluster_sources: &[SourceId],
    ppml_structural_constant: bool,
) -> Result<PreparedCategoricalOlsData> {
    let mut plan = plan_categorical_absorbed_request(request)?;
    let (model, sample) = crate::ols::prepare_ols_numeric_sample(
        plan.availability_request(),
        loaded,
        cluster_sources,
    )?;
    plan.discard_classified_slope_categories()?;
    let availability = finish_categorical_availability(
        plan.availability_request(),
        cluster_sources,
        model,
        sample,
        request
            .absorbed_terms
            .iter()
            .all(|term| term.categorical_sources.len() == 1),
        None,
    )?;
    finish_prepare_categorical_ols_data(
        request,
        cluster_sources,
        ppml_structural_constant,
        plan,
        availability,
    )
}

fn finish_prepare_categorical_ols_data(
    request: &OlsRequestV1,
    cluster_sources: &[SourceId],
    ppml_structural_constant: bool,
    plan: CategoricalAbsorbRequestPlan,
    availability: CategoricalAvailability,
) -> Result<PreparedCategoricalOlsData> {
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
    // No initial singleton in any partition implies the pruning fixed point.
    // Unchanged single-source partitions can then move their validated owners;
    // interactions and weighted samples keep the generic pruning/finalization.
    let fixed_point = if availability.sample.n_active() >= 16_384
        && matches!(availability.sample.weights, PreparedWeights::None)
        && !recipe_views.is_empty()
        && recipe_views.iter().all(|recipe| recipe.len() == 1)
    {
        recipe_views
            .iter()
            .try_fold(true, |clear, recipe| -> Result<bool> {
                let Some(source) = availability
                    .membership
                    .iter()
                    .find(|source| source.categorical_source_index() == recipe[0])
                else {
                    return Ok(false);
                };
                Ok(clear
                    && (plan.requested_keep_singletons()
                        || (0..source.n_levels()).try_fold(
                            true,
                            |clear, level| -> Result<bool> {
                                Ok(clear && source.members(level)?.len() > 1)
                            },
                        )?))
            })?
    } else {
        false
    };
    let singleton = if fixed_point {
        SingletonResult {
            active: initially_active,
            dropped_observations: Vec::new(),
            cascade_events: 0,
        }
    } else {
        let singleton = prune_categorical_partition_singletons(
            &candidate_views,
            &recipe_views,
            &initially_active,
            multiplicity,
            plan.requested_keep_singletons(),
        )?
        .into_parts()
        .0;
        drop(initially_active);
        singleton
    };
    let num_singletons = singleton.dropped_observations.len();
    let cascade_events = singleton.cascade_events;
    drop(singleton.dropped_observations);
    let rebuilt_topology = if fixed_point {
        None
    } else {
        Some(finalize_categorical_topology(
            &candidate_views,
            &recipe_views,
            &singleton.active,
        )?)
    };
    drop(candidate_views);

    let CategoricalAvailability {
        model: availability_model,
        sample,
        categorical_sources: mut candidate_sources,
        membership: mut membership_sources,
    } = availability;
    // A fixed point preserves the availability row and level axes. Transfer
    // their existing owners; rebuilding would copy codes and membership only
    // to discard the originals. The new owner revalidates exact membership.
    let fe_topology = if let Some(topology) = rebuilt_topology {
        drop(membership_sources);
        topology
    } else {
        FinalizedCategoricalTopology::from_single_source_indexes(
            candidate_sources.len(),
            sample.n_active(),
            &recipe_views,
            |source_index| {
                let position = membership_sources
                    .iter()
                    .position(|source| source.categorical_source_index() == source_index)
                    .ok_or_else(|| invalid("transferred topology source is missing"))?;
                let index = membership_sources.swap_remove(position).into_level_index();
                let source = candidate_sources
                    .get_mut(source_index)
                    .ok_or_else(|| invalid("transferred categorical source is out of range"))?;
                Ok((
                    std::mem::take(&mut source.encoded.codes),
                    source.encoded.n_levels(),
                    index,
                ))
            },
        )?
    };
    drop(recipe_views);
    let final_sources = final_categorical_sources(&availability_model, cluster_sources)?;
    // Only this complete mask proves that candidate codes still use the final row axis.
    let unchanged_axis = singleton.active.iter().all(|&active| active);
    let model_source_plans = plan_model_sources(
        candidate_sources,
        &final_sources,
        &fe_topology,
        unchanged_axis,
    )?;
    let sample = sample.compact_active_rows(&singleton.active, DropReason::Singleton)?;
    drop(singleton.active);
    if fe_topology.retained_rows().len() != sample.n_active() {
        return Err(invalid(
            "final categorical topology retained-row count differs from compact sample",
        ));
    }
    let categorical_sources = build_final_model_sources(model_source_plans, &sample, &fe_topology)?;
    let structural_intercept = ppml_structural_constant
        || request.absorbed_terms.is_empty()
        || request.absorbed_terms.iter().any(|term| term.intercept);
    let report_intercept = request.include_intercept && structural_intercept;
    let rebased = rebase_categorical_model(
        &request.model,
        &availability_model,
        &categorical_sources,
        &fe_topology,
        report_intercept,
    )?;
    let effect_specs = effect_specs(&bound_effects, &request.model)?;
    let sample = validated_sample::ValidatedSample::new(sample)?;
    let fe_topology = crate::fe::ValidatedFinalizedTopology::new(fe_topology)?;
    let prepared = PreparedCategoricalOlsData {
        model: rebased.model,
        sample,
        categorical_sources,
        fe_topology,
        effect_specs,
        structural_intercept,
        report_intercept,
        singletons: PreparedOlsSingletons {
            kept_by_request: plan.requested_keep_singletons(),
            num_singletons,
            cascade_events,
        },
        report_plan: rebased.report_plan,
    };
    prepared.validate_with_reusable_topology(false)?;
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
    Owned(Option<EncodedFactor<f64>>),
}

fn plan_model_sources(
    candidate_sources: Vec<PreparedCategoricalSource>,
    ordinary_sources: &[SourceId],
    topology: &FinalizedCategoricalTopology,
    unchanged_axis: bool,
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
            CategoricalModelSourcePlanStorage::Owned(unchanged_axis.then_some(candidate.encoded))
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
            CategoricalModelSourcePlanStorage::Owned(encoded) => {
                let values = sample
                    .regressors
                    .get(plan.prepared_source_index)
                    .ok_or_else(|| invalid("ordinary categorical raw source is out of range"))?;
                PreparedCategoricalModelSourceStorage::Owned {
                    encoded: match encoded {
                        Some(encoded) => encoded,
                        None => factorize_f64(values)?,
                    },
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
        let categorical_label = absorbed_categorical_label(&effect.sources, model)?;
        let mut slopes = Vec::new();
        slopes
            .try_reserve_exact(effect.slope_sources.len())
            .map_err(|_| allocation("unable to reserve categorical absorbed-slope metadata"))?;
        for source in &effect.slope_sources {
            let source_name = model
                .sources
                .iter()
                .find(|entry| entry.id == *source)
                .map(|entry| entry.canonical_name.as_str())
                .ok_or_else(|| invalid("absorbed-slope source identity is absent"))?;
            push_reserved(
                &mut slopes,
                CategoricalTopologyDofSlopeSpec {
                    source: *source,
                    label: absorbed_slope_label(&categorical_label, source_name)?,
                },
                "categorical absorbed-slope metadata capacity exhausted",
            )?;
        }
        push_reserved(
            &mut output,
            CategoricalTopologyDofEffectSpec {
                term_id: effect.term_id,
                sources: fallible_copy_slice(
                    &effect.sources,
                    "unable to reserve categorical absorbed DoF source identities",
                )?,
                intercept: effect.intercept,
                label: categorical_label,
                slopes,
            },
            "categorical absorbed DoF metadata capacity exhausted",
        )?;
    }
    Ok(output)
}

fn absorbed_categorical_label(
    sources: &[SourceId],
    model: &crate::metadata::CompiledTermGraph,
) -> Result<String> {
    if sources.is_empty() {
        return Err(invalid("absorbed effect lacks a categorical source"));
    }
    let mut capacity = sources.len().saturating_sub(1);
    for source in sources {
        let name = model
            .sources
            .iter()
            .find(|entry| entry.id == *source)
            .map(|entry| entry.canonical_name.as_str())
            .ok_or_else(|| invalid("absorbed categorical source identity is absent"))?;
        capacity = capacity
            .checked_add(name.len())
            .ok_or_else(|| allocation("categorical absorbed label length overflows"))?;
    }
    let mut output = String::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| allocation("unable to reserve categorical absorbed label"))?;
    for (index, source) in sources.iter().enumerate() {
        if index > 0 {
            output.push('#');
        }
        let name = model
            .sources
            .iter()
            .find(|entry| entry.id == *source)
            .map(|entry| entry.canonical_name.as_str())
            .ok_or_else(|| invalid("absorbed categorical source identity is absent"))?;
        output.push_str(name);
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
        || report_plan.public_coefficient_count() != report_plan.original_coefficient_count()
        || report_plan
            .original_slots()
            .iter()
            .enumerate()
            .any(|(index, slot)| slot.get() as usize != index + 1)
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
        || !crate::ols::numeric_sources_are_finite(sample)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn numeric_sample() -> PreparedNumericSample {
        PreparedNumericSample {
            n_original: 4,
            n_candidate: 4,
            original_row_by_active: vec![0, 1, 2, 3],
            outcome: vec![0.0, -0.0, 2.0, 3.0],
            regressors: vec![vec![1.0, 2.0, 3.0, 4.0]],
            weights: PreparedWeights::None,
            adjustment: crate::data::PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; 4],
        }
    }

    #[test]
    fn final_model_sources_transfer_only_on_an_unchanged_axis() {
        for keep in [[true; 4], [true, false, true, false]] {
            let mut sample = numeric_sample();
            sample.regressors[0] = vec![2.0, 1.0, 2.0, 3.0];
            let encoded = factorize_f64(&sample.regressors[0]).unwrap();
            let pointers = (encoded.codes.as_ptr(), encoded.levels.as_ptr());
            let candidate = PreparedCategoricalSource {
                source: SourceId::new(1).unwrap(),
                prepared_source_index: 0,
                encoded,
            };
            let views = categorical_views(std::slice::from_ref(&candidate)).unwrap();
            let topology = finalize_categorical_topology(&views, &[], &keep).unwrap();
            let unchanged = keep.iter().all(|&value| value);
            let plans = plan_model_sources(
                vec![candidate],
                &[SourceId::new(1).unwrap()],
                &topology,
                unchanged,
            )
            .unwrap();
            let sample = sample
                .compact_active_rows(&keep, DropReason::Singleton)
                .unwrap();
            let sources = build_final_model_sources(plans, &sample, &topology).unwrap();
            let PreparedCategoricalModelSourceStorage::Owned { encoded } = &sources[0].storage
            else {
                panic!("non-FE source must retain an independent owner");
            };
            assert_eq!(*encoded, factorize_f64(&sample.regressors[0]).unwrap());
            if unchanged {
                assert_eq!(pointers, (encoded.codes.as_ptr(), encoded.levels.as_ptr()));
            } else {
                assert_eq!(encoded.levels, vec![2.0]);
                assert_eq!(encoded.codes, vec![0, 0]);
            }
        }
    }

    #[test]
    fn validated_sample_owner_preserves_storage_and_rejects_invalid_inputs() {
        let sample = numeric_sample();
        let pointers = (
            sample.outcome.as_ptr(),
            sample.regressors[0].as_ptr(),
            sample.original_row_by_active.as_ptr(),
            sample.drop_reasons.as_ptr(),
        );
        let bytes = sample.owned_buffer_bytes();
        let owned = validated_sample::ValidatedSample::new(sample).unwrap();
        assert_eq!(
            pointers,
            (
                owned.outcome.as_ptr(),
                owned.regressors[0].as_ptr(),
                owned.original_row_by_active.as_ptr(),
                owned.drop_reasons.as_ptr()
            )
        );
        assert_eq!(owned.owned_buffer_bytes(), bytes);
        assert_eq!(
            std::mem::size_of_val(&owned),
            std::mem::size_of::<PreparedNumericSample>()
        );
        assert_eq!(owned.outcome[1].to_bits(), (-0.0_f64).to_bits());

        let corruptions: &[fn(&mut PreparedNumericSample)] = &[
            |s| s.n_candidate = 5,
            |s| {
                s.drop_reasons.pop();
            },
            |s| {
                s.outcome.pop();
            },
            |s| {
                s.regressors[0].pop();
            },
            |s| s.outcome[2] = f64::NAN,
            |s| s.regressors[0][2] = f64::INFINITY,
            |s| s.original_row_by_active.swap(0, 1),
            |s| s.original_row_by_active[3] = 4,
            |s| s.drop_reasons[3] = DropReason::MissingOutcome,
            |s| s.original_row_by_active[3] = 2,
        ];
        for corrupt in corruptions {
            let mut sample = numeric_sample();
            corrupt(&mut sample);
            let expected = validate_numeric_sample(&sample).unwrap_err();
            assert_eq!(
                validated_sample::ValidatedSample::new(sample).unwrap_err(),
                expected
            );
        }
        for kind in [
            WeightKind::Analytic,
            WeightKind::Frequency,
            WeightKind::Probability,
            WeightKind::Importance,
        ] {
            for values in [
                vec![1.0, 2.0, 3.0, 4.0],
                vec![1.0; 3],
                vec![0.0; 4],
                vec![-1.0; 4],
                vec![f64::NAN; 4],
                vec![0.5; 4],
                vec![f64::from(u32::MAX) + 1.0; 4],
            ] {
                let mut sample = numeric_sample();
                sample.weights = PreparedWeights::Provided { kind, values };
                let expected = validate_numeric_sample(&sample);
                assert_eq!(
                    validated_sample::ValidatedSample::new(sample).map(|_| ()),
                    expected
                );
            }
        }
    }
}
