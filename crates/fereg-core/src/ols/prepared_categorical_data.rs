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
use crate::error::{allocation, invalid_argument as invalid, invalid_data, Result};
use crate::factorize::{factorize_f64, EncodedFactor};
use crate::fe::{
    finalize_categorical_topology, plan_categorical_absorbed_request,
    prune_categorical_partition_singletons, BorrowedMapWeights, CategoricalAbsorbedEffectView,
    CategoricalCodeView, FeCoefficientBatch, FinalizedCategoricalTopology, LsmrConfig, MapConfig,
    RecoveredCategoricalEffects, SingletonMultiplicity, SingletonResult,
};
use crate::memory::push_reserved;
use crate::memory::{estimate_intercept_recovery_memory, FeSolverMemoryBudget};
use crate::metadata::SourceId;
use crate::prepared_categorical_rebase::{rebase_prepared_model, PreparedCategoricalOlsReportPlan};
use crate::topology_dof::{
    compute_categorical_topology_absorbed_dof_with_slopes, CategoricalTopologyDofEffectSpec,
    CategoricalTopologyDofPolicy, CategoricalTopologyDofReport, CategoricalTopologyDofSlopeSpec,
};
use rayon::prelude::*;

use crate::ols::{
    factorize_preparation_sources, prepare_categorical_numeric_sample,
    prepare_categorical_numeric_sample_owned, prepare_ols_numeric_sample,
};

use super::{
    LoadedOlsInput, OlsRequestV1, PreparedCategoricalSource, PreparedOlsColumnKind,
    PreparedOlsModel, PreparedOlsSingletons,
};

// Keep the owned sample behind a module-private field: even the surrounding
// preparation code cannot forge this proof or obtain mutable access. Source
// values and row/weight metadata therefore retain their constructor checks.
pub(crate) mod validated_sample {
    use super::{validate_numeric_sample, PreparedNumericSample, Result};

    #[derive(Debug, PartialEq)]
    pub(crate) struct ValidatedSample(PreparedNumericSample);

    impl ValidatedSample {
        pub(crate) fn new(sample: PreparedNumericSample) -> Result<Self> {
            validate_numeric_sample(&sample)?;
            Ok(Self(sample))
        }

        pub(crate) const fn get(&self) -> &PreparedNumericSample {
            &self.0
        }
    }

    // Unit fixtures may poison a sealed owner to exercise downstream failures.
    // This implementation is absent from library/plugin builds.
    #[cfg(test)]
    impl std::ops::DerefMut for ValidatedSample {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl std::ops::Deref for ValidatedSample {
        type Target = PreparedNumericSample;

        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
}

/// One finalized model/sample/source owner. Grammar controls mathematical
/// operations at cold boundaries; it never owns a parallel preparation bank.
#[derive(Debug, PartialEq)]
pub struct PreparedModel {
    pub(crate) model: PreparedOlsModel,
    pub(crate) sample: validated_sample::ValidatedSample,
    pub(crate) categorical_sources: Vec<PreparedCategoricalModelSource>,
    pub(crate) fe_topology: crate::fe::ValidatedFinalizedTopology,
    pub(crate) effect_specs: Vec<CategoricalTopologyDofEffectSpec>,
    pub(crate) structural_intercept: bool,
    pub(crate) report_intercept: bool,
    pub(crate) singletons: PreparedOlsSingletons,
    pub(crate) report_plan: PreparedCategoricalOlsReportPlan,
    pub(crate) grammar: PreparationGrammar,
    pub(crate) singleton_policy: super::PreparedOlsFeSingletonPolicy,
    pub(crate) clusters: Option<super::PreparedOlsClusterDesign>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreparationGrammar {
    Ordinary,
    Categorical,
}

/// Compatibility name; all finalized preparations have the same owner.
pub type PreparedCategoricalOlsData = PreparedModel;

impl PreparedModel {
    #[allow(clippy::too_many_arguments)] // One owner publication boundary; all fields are required.
    pub(crate) fn finish(
        model: PreparedOlsModel,
        sample: PreparedNumericSample,
        categorical_sources: Vec<PreparedCategoricalModelSource>,
        fe_topology: crate::fe::ValidatedFinalizedTopology,
        effect_specs: Vec<CategoricalTopologyDofEffectSpec>,
        structural_intercept: bool,
        report_intercept: bool,
        singletons: PreparedOlsSingletons,
        grammar: PreparationGrammar,
    ) -> Result<Self> {
        let report_plan = PreparedCategoricalOlsReportPlan::new(model.coefficients.len());
        let prepared = Self {
            model,
            sample: validated_sample::ValidatedSample::new(sample)?,
            categorical_sources,
            fe_topology,
            effect_specs,
            structural_intercept,
            report_intercept,
            singletons,
            report_plan,
            grammar,
            singleton_policy: if singletons.kept_by_request {
                super::PreparedOlsFeSingletonPolicy::KeptByRequest
            } else {
                super::PreparedOlsFeSingletonPolicy::VerifiedFixedPoint
            },
            clusters: None,
        };
        if prepared.uses_categorical_preparation() {
            prepared.validate_after_topology()?;
        } else {
            prepared.validate_fe_bindings()?;
            prepared.fe_topology.registry().total_levels()?;
        }
        Ok(prepared)
    }

    #[must_use]
    pub const fn uses_categorical_preparation(&self) -> bool {
        matches!(self.grammar, PreparationGrammar::Categorical)
    }

    /// Retained heap buffers, with topology-backed codes counted only at their
    /// owner. Construction scratch and numerical/dependency frames are separate.
    #[must_use]
    pub fn owned_buffer_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        self.sample.owned_buffer_bytes()
            + self.model.owned_buffer_bytes()
            + self.effect_metadata_capacity_bytes()
            + bytes(&self.categorical_sources)
            + self
                .categorical_sources
                .iter()
                .map(|source| source.owned_buffer_bytes())
                .sum::<u128>()
            + self.fe_topology.owned_buffer_bytes()
            + self
                .clusters
                .as_ref()
                .map_or(0, super::PreparedOlsClusterDesign::owned_buffer_bytes)
    }

    /// Executable recipe payloads and FE metadata; excludes numeric topology
    /// and top-level finite descriptors, charged separately by the planner.
    pub(crate) fn nested_metadata_capacity_bytes(&self) -> u128 {
        self.model.nested_recipe_capacity_bytes() + self.effect_metadata_capacity_bytes()
    }

    fn effect_metadata_capacity_bytes(&self) -> u128 {
        use std::mem::size_of;
        self.effect_specs.capacity() as u128 * size_of::<CategoricalTopologyDofEffectSpec>() as u128
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
            memory_budget,
            None,
        )
    }

    /// PPML and OLS share fallible coefficient recovery and independent
    /// reconstruction against the original target before publication.
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
            None,
            Some(map.tolerance),
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
        memory_budget: Option<FeSolverMemoryBudget>,
        reconstruction_tolerance: Option<f64>,
    ) -> Result<RecoveredCategoricalEffects> {
        self.validate_with_reusable_topology(true)?;
        let effects = self.effect_views()?;
        crate::fe::FeSession::recover(crate::fe::session::RecoveryRequest {
            topology: &self.fe_topology,
            effects: &effects,
            weights,
            target,
            requested_channels,
            retained,
            known_connected,
            materialize_absorbed_sum,
            lsmr,
            map,
            memory_budget,
            reconstruction_tolerance,
        })
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

    pub(crate) fn validate_after_topology(&self) -> Result<()> {
        if self.fe_topology.nobs() != self.sample.n_active() {
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
        crate::ols::validate_prepared_model_metadata(self)?;
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

/// One ordinary categorical source on the final sample.
#[derive(Debug, PartialEq)]
pub struct PreparedCategoricalModelSource {
    source: SourceId,
    prepared_source_index: usize,
    storage: PreparedCategoricalModelSourceStorage,
}

impl PreparedCategoricalModelSource {
    pub(crate) fn references_topology_candidate(&self, candidate: usize) -> bool {
        matches!(self.storage, PreparedCategoricalModelSourceStorage::Topology {candidate_source_index,..} if candidate_source_index==candidate)
    }

    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        match &self.storage {
            PreparedCategoricalModelSourceStorage::Topology { levels, .. } => bytes(levels),
            PreparedCategoricalModelSourceStorage::Owned { encoded } => {
                bytes(&encoded.codes) + bytes(&encoded.levels)
            }
        }
    }

    /// Sorted level values on the final sample's code axis.
    #[must_use]
    pub fn level_values(&self) -> &[f64] {
        match &self.storage {
            PreparedCategoricalModelSourceStorage::Topology { levels, .. } => levels,
            PreparedCategoricalModelSourceStorage::Owned { encoded } => &encoded.levels,
        }
    }

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
        _topology: &FinalizedCategoricalTopology,
        requested_level: f64,
    ) -> Result<Option<u32>> {
        let requested_level = canonical_factor_value(requested_level);
        self.level_values()
            .binary_search_by(|value| value.total_cmp(&requested_level))
            .ok()
            .map(|index| {
                u32::try_from(index).map_err(|_| invalid("ordinary categorical level exceeds u32"))
            })
            .transpose()
    }

    pub(crate) fn level_value(
        &self,
        _topology: &FinalizedCategoricalTopology,
        final_level: u32,
    ) -> Result<f64> {
        self.level_values()
            .get(final_level as usize)
            .copied()
            .ok_or_else(|| invalid("ordinary categorical level is out of range"))
    }
}

#[derive(Debug, PartialEq)]
enum PreparedCategoricalModelSourceStorage {
    /// Codes and final compact level identities are owned once by the FE
    /// topology. Values use its final sorted code axis.
    Topology {
        candidate_source_index: usize,
        levels: Vec<f64>,
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
    let _plan = plan_categorical_absorbed_request(request)?;
    let (model, sample) =
        prepare_categorical_numeric_sample_owned(request, loaded, cluster_sources)?;
    finish_prepared_model(request, cluster_sources, model, sample, true, false, true)
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

    let (model, sample) = prepare_ols_numeric_sample(request, loaded, cluster_sources)?;
    finish_prepared_model(request, cluster_sources, model, sample, true, true, true)
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
    let _plan = plan_categorical_absorbed_request(request)?;
    let (model, sample) = prepare_categorical_numeric_sample(request, loaded, cluster_sources)?;
    finish_prepared_model(
        request,
        cluster_sources,
        model,
        sample,
        true,
        ppml_structural_constant,
        true,
    )
}

#[allow(clippy::too_many_arguments)] // The one shared preparation boundary carries cold grammar policies.
pub(crate) fn finish_prepared_model(
    request: &OlsRequestV1,
    cluster_sources: &[SourceId],
    model: PreparedOlsModel,
    sample: PreparedNumericSample,
    categorical: bool,
    ppml_constant: bool,
    reuse_membership: bool,
) -> Result<PreparedModel> {
    let candidate_sources = factorize_preparation_sources(request, &sample)?;
    let recipes = crate::fe::bind_categorical_effect_recipes(request, &candidate_sources)?;
    let recipe_views: Vec<_> = recipes.iter().map(Vec::as_slice).collect();
    let mut membership =
        preparation_memberships(&candidate_sources, &recipe_views, &sample, reuse_membership)?;
    let fixed_point = recipe_views.is_empty()
        || (!membership.is_empty()
            && (request.keep_singletons
                || membership.iter().all(|(_, index)| {
                    (0..index.n_levels()).all(|level| {
                        index
                            .checked_members(level)
                            .is_ok_and(|rows| rows.len() > 1)
                    })
                })));
    let singleton = if fixed_point {
        SingletonResult {
            active: Vec::new(),
            dropped_observations: Vec::new(),
            cascade_events: 0,
        }
    } else {
        let views = categorical_views(&candidate_sources)?;
        let active = fallible_filled_bool(sample.n_active(), true)?;
        let multiplicity = match &sample.weights {
            PreparedWeights::Provided {
                kind: WeightKind::Frequency,
                values,
            } => SingletonMultiplicity::Frequency(values),
            _ => SingletonMultiplicity::Unit,
        };
        prune_categorical_partition_singletons(
            &views,
            &recipe_views,
            &active,
            multiplicity,
            request.keep_singletons,
        )?
        .into_parts()
        .0
    };
    let unchanged = fixed_point || singleton.active.iter().all(|&active| active);
    let num_singletons = singleton.dropped_observations.len();
    let cascade_events = singleton.cascade_events;
    drop(singleton.dropped_observations);
    let (topology, candidate_sources) = finalize_preparation_sources(
        candidate_sources,
        &recipe_views,
        &sample,
        &singleton.active,
        fixed_point,
        &mut membership,
    )?;
    drop(membership);
    let mut required = ordinary_categorical_sources(&model)?;
    if !categorical {
        required
            .try_reserve_exact(candidate_sources.len())
            .map_err(|_| allocation("unable to reserve final FE source identities"))?;
        for source in &candidate_sources {
            push_unique_source(&mut required, source.source)?;
        }
    }
    let mut categorical_sources =
        retain_model_sources(candidate_sources, &required, &topology, unchanged)?;
    let sample = if unchanged {
        sample
    } else {
        sample.compact_active_rows(&singleton.active, DropReason::Singleton)?
    };
    if !unchanged {
        reencode_model_sources(&mut categorical_sources, &sample)?;
    }
    append_cluster_sources(&mut categorical_sources, cluster_sources, &sample)?;
    let structural_intercept = if categorical && request.absorbed_terms.is_empty() {
        request.include_intercept
    } else {
        ppml_constant
            || request.absorbed_terms.is_empty()
            || request.absorbed_terms.iter().any(|term| term.intercept)
    };
    let report_intercept = request.include_intercept && structural_intercept;
    let rebased = rebase_prepared_model(
        &request.model,
        model,
        &categorical_sources,
        &topology,
        report_intercept,
        categorical,
    )?;
    PreparedModel::finish(
        rebased,
        sample,
        categorical_sources,
        crate::fe::ValidatedFinalizedTopology::new(topology)?,
        effect_specs(&request.absorbed_terms, &request.model, categorical)?,
        structural_intercept,
        report_intercept,
        PreparedOlsSingletons {
            kept_by_request: request.keep_singletons,
            num_singletons,
            cascade_events,
        },
        if categorical {
            PreparationGrammar::Categorical
        } else {
            PreparationGrammar::Ordinary
        },
    )
}

fn preparation_memberships(
    sources: &[PreparedCategoricalSource],
    recipes: &[&[usize]],
    sample: &PreparedNumericSample,
    reuse: bool,
) -> Result<Vec<(usize, crate::fe::LevelIndex)>> {
    let mut output = Vec::new();
    if !reuse
        || sample.n_active() < 16_384
        || !matches!(sample.weights, PreparedWeights::None)
        || !recipes.iter().all(|recipe| recipe.len() == 1)
    {
        return Ok(output);
    }
    output
        .try_reserve_exact(recipes.len())
        .map_err(|_| allocation("unable to reserve prepared FE membership sources"))?;
    for recipe in recipes {
        let index = recipe[0];
        if output.iter().any(|(prior, _)| *prior == index) {
            continue;
        }
        let source = &sources[index];
        let membership = match crate::fe::LevelIndex::try_from_ordered_codes(
            &source.encoded.codes,
            source.encoded.n_levels(),
        )? {
            Some(index) => index,
            None => {
                crate::fe::LevelIndex::from_codes(&source.encoded.codes, source.encoded.n_levels())?
            }
        };
        output.push((index, membership));
    }
    Ok(output)
}

fn finalize_preparation_sources(
    mut sources: Vec<PreparedCategoricalSource>,
    recipes: &[&[usize]],
    sample: &PreparedNumericSample,
    active: &[bool],
    transfer: bool,
    membership: &mut Vec<(usize, crate::fe::LevelIndex)>,
) -> Result<(FinalizedCategoricalTopology, Vec<PreparedCategoricalSource>)> {
    let topology = if transfer {
        FinalizedCategoricalTopology::from_single_source_indexes(
            sources.len(),
            sample.n_active(),
            recipes,
            |index| {
                let position = membership
                    .iter()
                    .position(|(i, _)| *i == index)
                    .ok_or_else(|| invalid("transferred topology source is missing"))?;
                let membership = membership.swap_remove(position).1;
                let source = &mut sources[index];
                Ok((
                    std::mem::take(&mut source.encoded.codes),
                    source.encoded.n_levels(),
                    membership,
                ))
            },
        )?
    } else {
        finalize_categorical_topology(&categorical_views(&sources)?, recipes, active)?
    };
    Ok((topology, sources))
}

fn append_cluster_sources(
    sources: &mut Vec<PreparedCategoricalModelSource>,
    cluster_sources: &[SourceId],
    sample: &PreparedNumericSample,
) -> Result<()> {
    let additional = cluster_sources
        .iter()
        .filter(|id| !sources.iter().any(|source| source.source == **id))
        .count();
    sources
        .try_reserve_exact(additional)
        .map_err(|_| allocation("unable to reserve final categorical cluster sources"))?;
    for id in cluster_sources {
        if sources.iter().any(|source| source.source == *id) {
            continue;
        }
        let prepared_source_index = usize::try_from(id.get() - 1)
            .map_err(|_| invalid("cluster source id does not fit usize"))?;
        let values = sample
            .regressors
            .get(prepared_source_index)
            .ok_or_else(|| invalid("ordinary categorical raw source is out of range"))?;
        sources.push(PreparedCategoricalModelSource {
            source: *id,
            prepared_source_index,
            storage: PreparedCategoricalModelSourceStorage::Owned {
                encoded: factorize_f64(values)?,
            },
        });
    }
    sources.sort_unstable_by_key(|source| source.source);
    Ok(())
}

/// Construct the final source bank directly. A changed row axis releases old
/// owned encodings before sample compaction; topology-backed levels survive to
/// resolve original factor recipes against the final topology.
pub(super) fn retain_model_sources(
    candidate_sources: Vec<PreparedCategoricalSource>,
    required: &[SourceId],
    topology: &FinalizedCategoricalTopology,
    unchanged_axis: bool,
) -> Result<Vec<PreparedCategoricalModelSource>> {
    retain_model_sources_with_reservation(
        candidate_sources,
        required,
        topology,
        unchanged_axis,
        &mut crate::ols::SystemFeTopologyReservation,
    )
}

pub(crate) fn retain_model_sources_with_reservation<R: crate::ols::FeTopologyReservation>(
    candidate_sources: Vec<PreparedCategoricalSource>,
    required: &[SourceId],
    topology: &FinalizedCategoricalTopology,
    unchanged_axis: bool,
    reservation: &mut R,
) -> Result<Vec<PreparedCategoricalModelSource>> {
    if topology.candidate_to_compacted_source().len() != candidate_sources.len() {
        return Err(invalid(
            "categorical topology source map and candidate source table differ in length",
        ));
    }
    let mut output = Vec::new();
    reservation.reserve_vec(
        &mut output,
        required.len(),
        "unable to reserve final categorical model sources",
    )?;
    for (candidate_source_index, candidate) in candidate_sources.into_iter().enumerate() {
        if !required.contains(&candidate.source) {
            continue;
        }
        let storage = if topology.candidate_to_compacted_source()[candidate_source_index].is_some()
        {
            let finalized = topology.source_for_candidate(candidate_source_index)?;
            let mut levels = candidate.encoded.levels;
            // Original indices are monotone and never smaller than their final
            // positions, so this compaction cannot overwrite a later source.
            for level in 0..finalized.n_levels() {
                let original = finalized
                    .original_level(level as u32)
                    .ok_or_else(|| invalid("final categorical level is out of range"))?
                    as usize;
                levels[level] = *levels
                    .get(original)
                    .ok_or_else(|| invalid("candidate categorical level is out of range"))?;
            }
            levels.truncate(finalized.n_levels());
            PreparedCategoricalModelSourceStorage::Topology {
                candidate_source_index,
                levels,
            }
        } else {
            PreparedCategoricalModelSourceStorage::Owned {
                encoded: if unchanged_axis {
                    candidate.encoded
                } else {
                    EncodedFactor {
                        codes: Vec::new(),
                        levels: Vec::new(),
                    }
                },
            }
        };
        push_reserved(
            &mut output,
            PreparedCategoricalModelSource {
                source: candidate.source,
                prepared_source_index: candidate.prepared_source_index,
                storage,
            },
            "final categorical model-source capacity exhausted",
        )?;
    }
    if output.len() != required.len()
        || required
            .iter()
            .any(|source| !output.iter().any(|entry| entry.source == *source))
    {
        return Err(invalid(
            "candidate categorical source table is missing an ordinary model source",
        ));
    }
    Ok(output)
}

fn reencode_model_sources(
    sources: &mut [PreparedCategoricalModelSource],
    sample: &PreparedNumericSample,
) -> Result<()> {
    for source in sources {
        if let PreparedCategoricalModelSourceStorage::Owned { encoded } = &mut source.storage {
            let values = sample
                .regressors
                .get(source.prepared_source_index)
                .ok_or_else(|| invalid("ordinary categorical raw source is out of range"))?;
            *encoded = factorize_f64(values)?;
        }
    }
    Ok(())
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
    effects: &[crate::metadata::AbsorbedTerm],
    model: &crate::metadata::CompiledTermGraph,
    categorical: bool,
) -> Result<Vec<CategoricalTopologyDofEffectSpec>> {
    effect_specs_with_reservation(
        effects,
        model,
        categorical,
        &mut crate::ols::SystemFeTopologyReservation,
    )
}

pub(crate) fn effect_specs_with_reservation<R: crate::ols::FeTopologyReservation>(
    effects: &[crate::metadata::AbsorbedTerm],
    model: &crate::metadata::CompiledTermGraph,
    categorical: bool,
    reservation: &mut R,
) -> Result<Vec<CategoricalTopologyDofEffectSpec>> {
    let mut output = Vec::new();
    reservation.reserve_vec(
        &mut output,
        effects.len(),
        "unable to reserve categorical absorbed DoF metadata",
    )?;
    for effect in effects {
        let categorical_label = if categorical {
            absorbed_categorical_label(&effect.categorical_sources, model)?
        } else {
            fallible_string_copy(&effect.label, "unable to reserve fixed-effect label")?
        };
        let mut slopes = Vec::new();
        reservation.reserve_vec(
            &mut slopes,
            effect.slope_sources.len(),
            "unable to reserve categorical absorbed-slope metadata",
        )?;
        let mut sources = Vec::new();
        reservation.reserve_vec(
            &mut sources,
            effect.categorical_sources.len(),
            "unable to reserve categorical absorbed DoF source identities",
        )?;
        for source in &effect.categorical_sources {
            push_reserved(
                &mut sources,
                *source,
                "categorical absorbed-source identity capacity exhausted",
            )?;
        }
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
                term_id: effect.id,
                sources,
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

fn validate_report_plan(
    model: &PreparedOlsModel,
    report_plan: &PreparedCategoricalOlsReportPlan,
) -> Result<()> {
    if report_plan.public_coefficient_count() != model.coefficients.len()
        || report_plan.public_coefficient_count() != report_plan.original_coefficient_count()
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
        || !matches!(sample.adjustment, crate::data::PreparedAdjustment::None)
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

fn fallible_string_copy(value: &str, label: &'static str) -> Result<String> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| allocation(label))?;
    output.push_str(value);
    Ok(output)
}

fn canonical_factor_value(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
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
            let mut sources = retain_model_sources(
                vec![candidate],
                &[SourceId::new(1).unwrap()],
                &topology,
                unchanged,
            )
            .unwrap();
            let sample = sample
                .compact_active_rows(&keep, DropReason::Singleton)
                .unwrap();
            if !unchanged {
                reencode_model_sources(&mut sources, &sample).unwrap();
            }
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
