//! PPML-owned prepared categorical model generations.
//!
//! Numeric/domain preparation is authoritative exactly once.  This owner then
//! keeps the raw loaded source registry and rebuilds only row-scoped finite,
//! fixed-effect, and cluster topology after separation commits.  Rebuilds are
//! keyed by stable original-row identifiers and always run the shared OLS
//! categorical preparation seam with singleton deletion disabled.  PPML's
//! separation coordinator is the sole owner of singleton/separation deletion.

use crate::linalg::neumaier_add;
use crate::memory::{fallible_copy, fallible_filled};
use std::sync::Arc;

use super::preparation_trace::PreparationTrace;
#[cfg(test)]
use super::prepared::PreparedPpmlNumericData;
use super::prepared::{
    canonicalize_probability_weights, checked_frequency_effective_observations,
    PpmlNumericSelection, PreparedPpmlAdjustmentReport, PreparedPpmlWeightReport,
};
use crate::data::{
    DropReason, PreparedAdjustment, PreparedNumericSample, PreparedWeights, WeightKind,
};
use crate::error::{allocation, invalid_argument, ErrorCode, FeregError, Result};
use crate::ols::prepared_categorical_data::{
    prepare_ppml_categorical_ols_data_with_load_hint, PreparedOlsData,
};
use crate::ols::{
    prepare_ppml_categorical_ols_clustered_data, LoadedNumericColumn, LoadedOlsInput,
    OlsClusterTermV1, OlsClusteredRequestV1, OlsRequestV1, PreparedOlsClusterDesign,
    PreparedOlsModel, PreparedOlsSingletons,
};
use crate::ppml_separation::{PreparedSeparationRebuilder, SeparationDropReason};

const MAX_PPML_CLUSTER_DIMENSIONS: usize = 5;

pub(crate) use crate::ols::prepared_fit_view::SharedPreparedOlsData as PreparedPpmlModel;

/// Final compact generation consumed by separation detectors, IRLS, and
/// inference.  Every observation-scale vector is aligned to
/// `original_rows()` and no vector borrows the raw SPI-loaded owner.
#[derive(Debug, PartialEq)]
pub(crate) struct PreparedPpmlModelGeneration {
    generation: usize,
    prepared: PreparedPpmlModel,
    offset: Vec<f64>,
    unit_prior_weights: Option<Vec<f64>>,
    original_observations: usize,
    requested_candidate_observations: usize,
    /// Immutable capacity reserved by numeric preparation for the owner-side
    /// canonical offset (including a logged exposure).  Every prepared
    /// generation owns a separate compact `offset`, so this candidate-axis
    /// buffer remains live beside the generation-local N-vector throughout
    /// separation and fitting.  Zero means the request had no adjustment.
    retained_base_adjustment_capacity_elements: usize,
    /// Length of the owner's immutable raw pweight subset on the
    /// generation-zero canonical active axis. It remains at that initial
    /// active N after separation generations shrink.
    retained_raw_preseparation_probability_weight_elements: usize,
    initial_drop_reasons: Arc<Vec<DropReason>>,
    weight_report: PreparedPpmlWeightReport,
    adjustment_report: PreparedPpmlAdjustmentReport,
}

/// Retain the exact immutable owner, including its original-row axis, beside
/// numerical artifacts. Equal-shaped or stale generations cannot substitute.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PpmlGenerationBinding {
    generation: u64,
    prepared: PreparedPpmlModel,
}

impl PpmlGenerationBinding {
    pub(super) fn capture(generation: &PreparedPpmlModelGeneration) -> Result<Self> {
        Ok(Self {
            generation: u64::try_from(generation.generation())
                .map_err(|_| invalid_argument("PPML model generation is not representable"))?,
            prepared: generation.prepared().clone(),
        })
    }
    pub(super) const fn generation(&self) -> u64 {
        self.generation
    }
    pub(super) fn validate(
        &self,
        generation: &PreparedPpmlModelGeneration,
        rows: usize,
    ) -> Result<()> {
        if self.generation
            != u64::try_from(generation.generation())
                .map_err(|_| invalid_argument("PPML model generation is not representable"))?
            || !Arc::ptr_eq(&self.prepared, generation.prepared())
            || rows != generation.original_rows().len()
        {
            return Err(invalid_argument(
                "PPML numerical artifact belongs to another prepared generation",
            ));
        }
        Ok(())
    }
}

impl PreparedPpmlModelGeneration {
    /// All generation-owned heap capacities. The shared initial-drop-reason
    /// Arc belongs to the immutable raw owner and is counted there exactly once.
    pub(super) fn owned_buffer_bytes(&self) -> u128 {
        let prepared = std::mem::size_of::<(usize, usize, PreparedOlsData)>() as u128
            + self.fit_view().owned_buffer_bytes();
        prepared
            + crate::memory::vec_capacity_bytes(&self.offset)
            + self
                .unit_prior_weights
                .as_ref()
                .map_or(0, crate::memory::vec_capacity_bytes)
    }

    fn validate_memory_bound(&self, bound: Option<u128>) -> Result<()> {
        if bound.is_some_and(|bound| self.owned_buffer_bytes() > bound) {
            return Err(allocation(
                "PPML prepared generation capacities exceed admission",
            ));
        }
        Ok(())
    }

    pub(crate) const fn generation(&self) -> usize {
        self.generation
    }

    pub(crate) const fn prepared(&self) -> &PreparedPpmlModel {
        &self.prepared
    }

    pub(crate) fn fit_view(&self) -> &PreparedOlsData {
        &self.prepared
    }

    pub(crate) fn model(&self) -> &PreparedOlsModel {
        self.prepared.model()
    }

    pub(crate) fn original_rows(&self) -> &[usize] {
        self.prepared
            .sample()
            .retained_rows()
            .selected()
            .expect("PPML preparation owns explicit rows")
    }

    pub(crate) fn outcome(&self) -> &[f64] {
        &self.prepared.sample().outcome
    }

    pub(crate) fn prior_weights(&self) -> &[f64] {
        match &self.prepared.sample().weights {
            PreparedWeights::None => self
                .unit_prior_weights
                .as_deref()
                .expect("certified unweighted PPML generation has unit weights"),
            PreparedWeights::Provided { values, .. } => values,
        }
    }

    pub(crate) fn offset(&self) -> &[f64] {
        &self.offset
    }

    pub(crate) const fn original_observations(&self) -> usize {
        self.original_observations
    }

    pub(crate) const fn requested_candidate_observations(&self) -> usize {
        self.requested_candidate_observations
    }

    pub(crate) const fn retained_base_adjustment_capacity_elements(&self) -> usize {
        self.retained_base_adjustment_capacity_elements
    }

    pub(crate) const fn retained_raw_preseparation_probability_weight_elements(&self) -> usize {
        self.retained_raw_preseparation_probability_weight_elements
    }

    pub(crate) const fn weight_report(&self) -> PreparedPpmlWeightReport {
        self.weight_report
    }

    pub(crate) const fn adjustment_report(&self) -> PreparedPpmlAdjustmentReport {
        self.adjustment_report
    }

    pub(crate) fn cluster_design(&self) -> Option<&PreparedOlsClusterDesign> {
        self.prepared.cluster_design()
    }

    pub(crate) fn cluster_codes(&self, dimension: usize) -> Result<&[u32]> {
        self.prepared.cluster_codes(dimension)
    }

    /// Merge the immutable numeric/domain ledger with PPML separation reasons
    /// on the full original observation axis.
    pub(crate) fn final_drop_reasons(
        &self,
        separation_reasons: &[Option<SeparationDropReason>],
    ) -> Result<Vec<DropReason>> {
        if separation_reasons.len() != self.original_observations {
            return Err(invalid_argument(
                "prepared PPML separation reasons do not match the original observation axis",
            ));
        }
        let mut final_reasons = fallible_copy(
            &self.initial_drop_reasons,
            "unable to reserve final prepared PPML drop reasons",
        )?;
        let mut final_position = 0_usize;
        for original_row in 0..self.original_observations {
            let initially_retained = final_reasons[original_row] == DropReason::Retained;
            let finally_retained = self
                .original_rows()
                .get(final_position)
                .is_some_and(|row| *row == original_row);
            if finally_retained {
                final_position += 1;
            }
            match (
                initially_retained,
                finally_retained,
                separation_reasons[original_row],
            ) {
                (false, false, None) | (true, true, None) => {}
                (true, false, Some(reason)) => {
                    final_reasons[original_row] = separation_drop_reason(reason);
                }
                _ => {
                    return Err(invalid_argument(
                        "prepared PPML numeric, separation, and final row ledgers disagree",
                    ));
                }
            }
        }
        if final_position != self.original_rows().len() {
            return Err(invalid_argument(
                "prepared PPML final row map is not strictly increasing",
            ));
        }
        Ok(final_reasons)
    }
}

/// Sole owner of the raw request/source registry across prepared generations.
///
/// The owner intentionally does not expose its mutable candidate mask.  A
/// rebuild swaps in a newly allocated mask, constructs a wholly owned result,
/// and restores the baseline mask before returning either success or error.
pub(crate) struct PreparedPpmlModelOwner {
    load_hint: Option<crate::ols::PreparedOlsLoadHint>,
    membership_scratch: u128,
    generation_memory_bound: Option<u128>,
    request: OlsClusteredRequestV1,
    loaded: LoadedOlsInput,
    candidate_original_rows: Arc<Vec<usize>>,
    base_offset: Option<Vec<f64>>,
    /// Raw finite-positive pweights, aligned to `candidate_original_rows`.
    /// Each generation canonicalizes its retained raw subset directly, so
    /// subnormal raw totals are not reconstructed through rounded logarithms
    /// or an already-rounded earlier normalization.
    base_probability_weights: Option<Vec<f64>>,
    initial_drop_reasons: Arc<Vec<DropReason>>,
    original_observations: usize,
    requested_candidate_observations: usize,
    weight_report: PreparedPpmlWeightReport,
    adjustment_report: PreparedPpmlAdjustmentReport,
}

impl PreparedPpmlModelOwner {
    pub(crate) fn with_load_hint(mut self, hint: Option<crate::ols::PreparedOlsLoadHint>) -> Self {
        self.load_hint = hint;
        self
    }

    pub(crate) fn with_membership_scratch(mut self, bytes: u128) -> Self {
        self.membership_scratch = bytes;
        self
    }

    pub(crate) fn with_generation_memory_bound(mut self, bytes: u128) -> Self {
        self.generation_memory_bound = Some(bytes);
        self
    }

    pub(crate) fn from_selection(
        request: OlsRequestV1,
        clusters: Vec<OlsClusterTermV1>,
        loaded: LoadedOlsInput,
        numeric: PpmlNumericSelection,
        adjustment: Option<LoadedNumericColumn>,
    ) -> Result<Self> {
        // The selector borrowed this loaded owner directly. No separately
        // supplied compact outcome/source copies require pair validation.
        let base_probability_weights = capture_raw_probability_weights(
            &loaded,
            &numeric.original_row_by_active,
            numeric.weights.kind,
        )?;
        let (adjustment, adjustment_report) =
            numeric.take_adjustment(adjustment.map(|column| column.values))?;
        Self::from_parts(
            request,
            clusters,
            loaded,
            numeric.original_row_by_active,
            numeric.drop_reasons,
            numeric.classification.n_candidate,
            adjustment,
            base_probability_weights,
            numeric.weights,
            adjustment_report,
        )
    }

    #[cfg(test)]
    pub(crate) fn new(
        request: OlsRequestV1,
        clusters: Vec<OlsClusterTermV1>,
        loaded: LoadedOlsInput,
        numeric: PreparedPpmlNumericData,
    ) -> Result<Self> {
        let weight_report = numeric.weights();
        let adjustment_report = numeric.adjustment();
        let sample = numeric.into_sample();
        validate_loaded_numeric_pair(&loaded, &sample, weight_report)?;
        let rows = sample.original_row_by_active.into_vec()?;
        let base_probability_weights =
            capture_raw_probability_weights(&loaded, &rows, weight_report.kind)?;
        Self::from_parts(
            request,
            clusters,
            loaded,
            rows,
            sample.drop_reasons,
            sample.n_candidate,
            sample.adjustment,
            base_probability_weights,
            weight_report,
            adjustment_report,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        mut request: OlsRequestV1,
        clusters: Vec<OlsClusterTermV1>,
        loaded: LoadedOlsInput,
        original_row_by_active: Vec<usize>,
        drop_reasons: Vec<DropReason>,
        requested_candidate_observations: usize,
        adjustment: PreparedAdjustment,
        base_probability_weights: Option<Vec<f64>>,
        weight_report: PreparedPpmlWeightReport,
        adjustment_report: PreparedPpmlAdjustmentReport,
    ) -> Result<Self> {
        if clusters.len() > MAX_PPML_CLUSTER_DIMENSIONS {
            return Err(invalid_argument(
                "prepared PPML supports at most five declared cluster dimensions",
            ));
        }
        let base_offset = match adjustment {
            PreparedAdjustment::None => None,
            PreparedAdjustment::Offset(values) => Some(values),
            PreparedAdjustment::Exposure(_) => {
                return Err(invalid_argument(
                    "prepared PPML owner received an unlogged exposure",
                ));
            }
        };
        let original_observations = loaded.candidate.len();
        let candidate_original_rows = Arc::new(original_row_by_active);
        let initial_drop_reasons = Arc::new(drop_reasons);
        // Shared categorical preparation must only establish source
        // availability and topology. PPML's coordinator applies the requested
        // singleton policy as part of its certified fixed point.
        request.keep_singletons = true;
        Ok(Self {
            load_hint: None,
            membership_scratch: 0,
            generation_memory_bound: None,
            request: OlsClusteredRequestV1 {
                ols: request,
                clusters,
            },
            loaded,
            candidate_original_rows,
            base_offset,
            base_probability_weights,
            initial_drop_reasons,
            original_observations,
            requested_candidate_observations,
            weight_report,
            adjustment_report,
        })
    }

    fn build_prepared_model(
        &self,
        hint: Option<crate::ols::PreparedOlsLoadHint>,
        membership_scratch: u128,
    ) -> Result<PreparedPpmlModel> {
        // PPML always enters through the categorical preparation seam, even
        // with no absorbed effects.  The plain OLS view represents its
        // structural intercept as a virtual one-level absorbed effect; that
        // is correct for OLS's plain/categorical unification but would turn a
        // no-FE PPML `_cons` into a normalized structural constant.  An empty
        // categorical topology instead leaves `_cons` in the finite design,
        // while using the identical production view once effects are present.
        if self.request.clusters.is_empty() {
            Ok(Arc::new(prepare_ppml_categorical_ols_data_with_load_hint(
                &self.request.ols,
                &self.loaded,
                &[],
                hint,
                membership_scratch,
            )?))
        } else {
            Ok(Arc::new(prepare_ppml_categorical_ols_clustered_data(
                &self.request,
                &self.loaded,
                hint,
                membership_scratch,
            )?))
        }
    }

    fn compact_offset(&self, retained_original_rows: &[usize]) -> Result<Vec<f64>> {
        let mut output = fallible_filled(
            retained_original_rows.len(),
            0.0,
            "unable to reserve prepared PPML generation offset",
        )?;
        let Some(base_offset) = &self.base_offset else {
            return Ok(output);
        };
        let mut base_position = 0_usize;
        for (output_position, &original_row) in retained_original_rows.iter().enumerate() {
            while self
                .candidate_original_rows
                .get(base_position)
                .is_some_and(|candidate| *candidate < original_row)
            {
                base_position += 1;
            }
            if self.candidate_original_rows.get(base_position) != Some(&original_row) {
                return Err(invalid_argument(
                    "prepared PPML generation contains a row outside the numeric sample",
                ));
            }
            output[output_position] = base_offset[base_position];
        }
        Ok(output)
    }

    fn stage_generation_probability_weights(
        &mut self,
        retained_original_rows: &[usize],
    ) -> Result<Option<ProbabilityGenerationStage>> {
        let Some(base_weights) = &self.base_probability_weights else {
            return Ok(None);
        };
        let target = self.loaded.probability_weight.as_ref().ok_or_else(|| {
            invalid_argument("prepared PPML probability-weight owner lost its loaded column")
        })?;
        if base_weights.len() != self.candidate_original_rows.len()
            || target.values.len() != self.original_observations
            || target.missing.len() != self.original_observations
        {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "prepared PPML probability-weight rollback owners are misaligned",
            ));
        }
        let mut positions = fallible_filled(
            retained_original_rows.len(),
            0_usize,
            "unable to reserve prepared PPML pweight subset positions",
        )?;
        let mut base_position = 0_usize;
        for (output_position, &original_row) in retained_original_rows.iter().enumerate() {
            while self
                .candidate_original_rows
                .get(base_position)
                .is_some_and(|candidate| *candidate < original_row)
            {
                base_position += 1;
            }
            if self.candidate_original_rows.get(base_position) != Some(&original_row) {
                return Err(invalid_argument(
                    "prepared PPML pweight generation contains a nonnumeric row",
                ));
            }
            positions[output_position] = base_position;
        }
        let physical = retained_original_rows.len() as f64;
        let mut staged_values = Vec::new();
        staged_values
            .try_reserve_exact(retained_original_rows.len())
            .map_err(|_| allocation("unable to reserve staged PPML pweight values"))?;
        for &position in &positions {
            staged_values.push(base_weights[position]);
        }
        let subset_scale = canonicalize_probability_weights(&mut staged_values, physical)?;
        // No fallible operation occurs after publication to the loaded owner;
        // `rebuild_generation` restores the immutable raw values addressed by
        // `positions` immediately after its build attempt, irrespective of
        // that attempt's result.  Retaining the compact positions rather than
        // a padded `(row, value, missing)` rollback record also makes the
        // accepted-rebase peak exactly one usize per staged probability row.
        let target = self.loaded.probability_weight.as_mut().ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "prepared PPML probability-weight column vanished during staging",
            )
        })?;
        for ((&original_row, value), &position) in retained_original_rows
            .iter()
            .zip(staged_values)
            .zip(&positions)
        {
            debug_assert_eq!(
                self.candidate_original_rows.get(position),
                Some(&original_row)
            );
            target.values[original_row] = value;
            target.missing[original_row] = false;
        }
        Ok(Some(ProbabilityGenerationStage {
            scale: ProbabilityGenerationScale {
                raw_sum: subset_scale.raw_sum,
                log_raw_sum: subset_scale.log_raw_sum,
                raw_to_prepared_multiplier: subset_scale.raw_to_prepared_multiplier,
                log_raw_to_prepared_multiplier: subset_scale.log_raw_to_prepared_multiplier,
            },
            positions,
        }))
    }

    fn restore_generation_probability_weights(
        &mut self,
        stage: Option<ProbabilityGenerationStage>,
    ) {
        let Some(stage) = stage else {
            return;
        };
        // All three owners and every position were certified before the first
        // loaded value was changed. `build_prepared_model` borrows `self`
        // immutably, so those invariants cannot change before this rollback.
        // Restoration is consequently allocation-free and structurally
        // infallible after publication of the temporary normalized values.
        let base_weights = self
            .base_probability_weights
            .as_deref()
            .expect("staged PPML pweights retain immutable raw values");
        let target = self
            .loaded
            .probability_weight
            .as_mut()
            .expect("staged PPML pweights retain their loaded target");
        for position in stage.positions {
            let raw = *base_weights
                .get(position)
                .expect("staged PPML pweight position was prevalidated");
            let original_row = *self
                .candidate_original_rows
                .get(position)
                .expect("staged PPML pweight row position was prevalidated");
            *target
                .values
                .get_mut(original_row)
                .expect("staged PPML pweight value target was prevalidated") = raw;
            *target
                .missing
                .get_mut(original_row)
                .expect("staged PPML pweight missing target was prevalidated") = false;
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ProbabilityGenerationScale {
    raw_sum: Option<f64>,
    log_raw_sum: f64,
    raw_to_prepared_multiplier: Option<f64>,
    log_raw_to_prepared_multiplier: f64,
}

struct ProbabilityGenerationStage {
    scale: ProbabilityGenerationScale,
    /// Positions on the immutable generation-zero numeric row axis.  These
    /// address both the owner row map and its raw pweight vector for exact,
    /// allocation-free restoration after a generation build attempt.
    positions: Vec<usize>,
}

impl PreparedSeparationRebuilder for PreparedPpmlModelOwner {
    type Generation = PreparedPpmlModelGeneration;

    fn original_observations(&self) -> usize {
        self.original_observations
    }

    fn candidate_original_rows(&self) -> Arc<Vec<usize>> {
        Arc::clone(&self.candidate_original_rows)
    }

    fn rebuild_generation(
        &mut self,
        retained_original_rows: &[usize],
        generation: usize,
    ) -> Result<Self::Generation> {
        let mut preparation_trace = PreparationTrace::new();
        // A failed first attempt consumes the optional work as well. No later
        // separation generation can inherit even an equal-shaped capsule.
        let load_hint = self.load_hint.take().filter(|_| generation == 0);
        // Histogram scratch is admitted only for generation-zero preparation;
        // replacement generations keep their existing memory envelope.
        let membership_scratch = std::mem::take(&mut self.membership_scratch);
        let membership_scratch = if generation == 0 {
            membership_scratch
        } else {
            0
        };
        validate_retained_subset(
            retained_original_rows,
            &self.candidate_original_rows,
            self.original_observations,
        )?;
        let staged_candidate = original_axis_mask(
            self.original_observations,
            retained_original_rows,
            "unable to reserve prepared PPML generation candidate mask",
        )?;
        let probability_stage =
            self.stage_generation_probability_weights(retained_original_rows)?;
        let probability_scale = probability_stage.as_ref().map(|stage| stage.scale);
        let baseline_candidate = std::mem::replace(&mut self.loaded.candidate, staged_candidate);
        let staging_seconds = preparation_trace.lap();
        // The shared OLS boundary includes sample materialization, source
        // factorization, membership, finalized topology and optional clusters.
        // Keep it an inclusive aggregate rather than relabeling its internals.
        let prepared_result = self.build_prepared_model(load_hint, membership_scratch);
        let shared_model_seconds = preparation_trace.lap();
        self.loaded.candidate = baseline_candidate;
        self.restore_generation_probability_weights(probability_stage);
        let restoration_seconds = preparation_trace.lap();
        let prepared = prepared_result?;

        validate_prepared_generation(&prepared, retained_original_rows)?;
        if !prepared
            .sample()
            .outcome
            .iter()
            .any(|outcome| *outcome > 0.0)
        {
            return Err(FeregError::new(
                ErrorCode::InsufficientObservations,
                "prepared PPML generation has no positive outcome",
            ));
        }
        let offset = self.compact_offset(retained_original_rows)?;
        let unit_prior_weights = matches!(prepared.sample().weights, PreparedWeights::None)
            .then(|| {
                fallible_filled(
                    retained_original_rows.len(),
                    1.0,
                    "unable to reserve prepared PPML unit prior weights",
                )
            })
            .transpose()?;
        let weight_report =
            generation_weight_report(self.weight_report, prepared.sample(), probability_scale)?;
        let retained_raw_preseparation_probability_weight_elements =
            self.base_probability_weights.as_ref().map_or(0, Vec::len);
        let retained_base_adjustment_capacity_elements = self
            .base_offset
            .as_ref()
            .map_or(0, |_| self.requested_candidate_observations);
        let prepared = PreparedPpmlModelGeneration {
            generation,
            prepared,
            offset,
            unit_prior_weights,
            original_observations: self.original_observations,
            requested_candidate_observations: self.requested_candidate_observations,
            retained_base_adjustment_capacity_elements,
            retained_raw_preseparation_probability_weight_elements,
            initial_drop_reasons: Arc::clone(&self.initial_drop_reasons),
            weight_report,
            adjustment_report: self.adjustment_report,
        };
        prepared.validate_memory_bound(self.generation_memory_bound)?;
        let products_seconds = preparation_trace.lap();
        if preparation_trace.enabled() {
            eprintln!(
                "FEREG_PHASE|ppml_prepare_generation|generation={generation}|staging={staging_seconds:.9}|shared_model={shared_model_seconds:.9}|restoration={restoration_seconds:.9}|products={products_seconds:.9}|rows={}|cluster_dimensions={}|team_workers={}",
                retained_original_rows.len(),
                self.request.clusters.len(),
                PreparationTrace::team_workers(),
            );
        }
        Ok(prepared)
    }
}

#[cfg(test)]
fn validate_loaded_numeric_pair(
    loaded: &LoadedOlsInput,
    sample: &PreparedNumericSample,
    weight_report: PreparedPpmlWeightReport,
) -> Result<()> {
    let original = loaded.candidate.len();
    validate_loaded_column(&loaded.outcome, original, "outcome")?;
    for (_, source) in &loaded.numeric_sources {
        validate_loaded_column(source, original, "numeric source")?;
    }
    for (label, weight) in [
        ("analytic weight", loaded.analytic_weight.as_ref()),
        ("frequency weight", loaded.frequency_weight.as_ref()),
        ("probability weight", loaded.probability_weight.as_ref()),
    ] {
        if let Some(weight) = weight {
            validate_loaded_column(weight, original, label)?;
        }
    }
    if sample.n_original != original
        || sample.drop_reasons.len() != original
        || sample.n_candidate
            != loaded
                .candidate
                .iter()
                .filter(|candidate| **candidate)
                .count()
        || sample.outcome.len() != sample.n_active()
        || sample.regressors.len() != loaded.numeric_sources.len()
        || sample
            .regressors
            .iter()
            .any(|source| source.len() != sample.n_active())
        || sample
            .original_row_by_active
            .iter()
            .zip(sample.original_row_by_active.iter().skip(1))
            .any(|(left, right)| left >= right)
    {
        return Err(invalid_argument(
            "prepared PPML numeric data is not aligned with its loaded source owner",
        ));
    }
    for (active_row, original_row) in sample.original_row_by_active.iter().enumerate() {
        if !loaded.candidate[original_row]
            || sample.drop_reasons[original_row] != DropReason::Retained
            || loaded.outcome.missing[original_row]
            || loaded.outcome.values[original_row].to_bits() != sample.outcome[active_row].to_bits()
        {
            return Err(invalid_argument(
                "prepared PPML outcome row map does not match the loaded source owner",
            ));
        }
        for (prepared_source, (_, loaded_source)) in
            sample.regressors.iter().zip(&loaded.numeric_sources)
        {
            if loaded_source.missing[original_row]
                || loaded_source.values[original_row].to_bits()
                    != prepared_source[active_row].to_bits()
            {
                return Err(invalid_argument(
                    "prepared PPML regressor row map does not match the loaded source owner",
                ));
            }
        }
    }
    if weight_report.kind != sample_weight_kind(&sample.weights) {
        return Err(invalid_argument(
            "prepared PPML weight report does not match its numeric sample",
        ));
    }
    Ok(())
}

#[cfg(test)]
fn validate_loaded_column(column: &LoadedNumericColumn, rows: usize, label: &str) -> Result<()> {
    if column.values.len() != rows || column.missing.len() != rows {
        return Err(invalid_argument(format!(
            "loaded PPML {label} is not aligned with the original observation axis"
        )));
    }
    Ok(())
}

fn capture_raw_probability_weights(
    loaded: &LoadedOlsInput,
    original_row_by_active: &[usize],
    kind: Option<WeightKind>,
) -> Result<Option<Vec<f64>>> {
    match kind {
        None => Ok(None),
        Some(WeightKind::Frequency) => {
            if loaded.frequency_weight.is_none() {
                return Err(invalid_argument(
                    "prepared PPML frequency-weight column is absent from the loaded owner",
                ));
            }
            Ok(None)
        }
        Some(WeightKind::Probability) => {
            let raw = loaded.probability_weight.as_ref().ok_or_else(|| {
                invalid_argument(
                    "prepared PPML probability-weight column is absent from the loaded owner",
                )
            })?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(original_row_by_active.len())
                .map_err(|_| allocation("unable to reserve raw prepared PPML pweights"))?;
            for &original_row in original_row_by_active {
                let value = raw.values.get(original_row).copied().ok_or_else(|| {
                    invalid_argument(
                        "prepared PPML probability-weight row map exceeds the loaded owner",
                    )
                })?;
                if raw.missing.get(original_row).copied() != Some(false)
                    || !value.is_finite()
                    || value <= 0.0
                {
                    return Err(invalid_argument(
                        "prepared PPML probability-weight row map contains an invalid raw value",
                    ));
                }
                values.push(value);
            }
            Ok(Some(values))
        }
        Some(WeightKind::Analytic | WeightKind::Importance) => Err(FeregError::new(
            ErrorCode::UnsupportedWeight,
            "prepared PPML owner supports only frequency and probability weights",
        )),
    }
}

fn validate_retained_subset(
    retained: &[usize],
    numeric_rows: &[usize],
    original_observations: usize,
) -> Result<()> {
    if retained.is_empty() {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "prepared PPML generation cannot be empty",
        ));
    }
    if retained.windows(2).any(|rows| rows[0] >= rows[1])
        || retained.iter().any(|row| *row >= original_observations)
    {
        return Err(invalid_argument(
            "prepared PPML generation rows must be sorted, unique, and in range",
        ));
    }
    let mut numeric_position = 0_usize;
    for &row in retained {
        while numeric_rows
            .get(numeric_position)
            .is_some_and(|numeric_row| *numeric_row < row)
        {
            numeric_position += 1;
        }
        if numeric_rows.get(numeric_position) != Some(&row) {
            return Err(invalid_argument(
                "prepared PPML generation contains a row outside the numeric sample",
            ));
        }
    }
    Ok(())
}

fn validate_prepared_generation(
    prepared: &PreparedPpmlModel,
    retained_original_rows: &[usize],
) -> Result<()> {
    let sample = prepared.sample();
    if !sample
        .retained_rows()
        .iter()
        .eq(retained_original_rows.iter().copied())
        || sample.n_active() != retained_original_rows.len()
        || prepared.singletons()
            != (PreparedOlsSingletons {
                kept_by_request: true,
                num_singletons: 0,
                cascade_events: 0,
            })
    {
        return Err(invalid_argument(
            "shared preparation changed the PPML-owned retained sample",
        ));
    }
    if let Some(clusters) = prepared.cluster_design() {
        for dimension in 0..clusters.len() {
            if prepared.cluster_codes(dimension)?.len() != sample.n_active() {
                return Err(invalid_argument(
                    "prepared PPML cluster codes are not aligned with the final sample",
                ));
            }
        }
    }
    Ok(())
}

fn generation_weight_report(
    _base: PreparedPpmlWeightReport,
    sample: &PreparedNumericSample,
    probability_scale: Option<ProbabilityGenerationScale>,
) -> Result<PreparedPpmlWeightReport> {
    let physical = sample.n_active();
    match &sample.weights {
        PreparedWeights::None => Ok(PreparedPpmlWeightReport {
            kind: None,
            physical_observations: physical,
            effective_observations: physical,
            raw_weight_sum: None,
            log_raw_weight_sum: None,
            prepared_weight_sum: physical as f64,
            raw_to_prepared_multiplier: Some(1.0),
            log_raw_to_prepared_multiplier: 0.0,
        }),
        PreparedWeights::Provided { kind, values } => {
            let prepared_sum = compensated_sum(values.iter().copied())?;
            let (effective, raw_sum) = match kind {
                WeightKind::Frequency => {
                    let total = checked_frequency_effective_observations(values)?;
                    (total, Some(prepared_sum))
                }
                WeightKind::Probability => {
                    let scale = probability_scale.ok_or_else(|| {
                        invalid_argument("prepared PPML generation pweight scale is absent")
                    })?;
                    let physical_f64 = physical as f64;
                    if (prepared_sum - physical_f64).abs()
                        > 64.0 * f64::EPSILON * physical_f64.max(1.0)
                    {
                        return Err(invalid_argument(
                            "prepared PPML generation pweights do not sum to final physical N",
                        ));
                    }
                    (physical, scale.raw_sum)
                }
                WeightKind::Analytic | WeightKind::Importance => {
                    return Err(FeregError::new(
                        ErrorCode::UnsupportedWeight,
                        "prepared PPML generation contains an unsupported weight kind",
                    ));
                }
            };
            let (log_raw_weight_sum, raw_to_prepared_multiplier, log_raw_to_prepared_multiplier) =
                match kind {
                    WeightKind::Probability => {
                        let scale = probability_scale.expect("probability scale checked above");
                        (
                            Some(scale.log_raw_sum),
                            scale.raw_to_prepared_multiplier,
                            scale.log_raw_to_prepared_multiplier,
                        )
                    }
                    WeightKind::Frequency => (Some(prepared_sum.ln()), Some(1.0), 0.0),
                    WeightKind::Analytic | WeightKind::Importance => unreachable!(),
                };
            Ok(PreparedPpmlWeightReport {
                kind: Some(*kind),
                physical_observations: physical,
                effective_observations: effective,
                raw_weight_sum: raw_sum,
                log_raw_weight_sum,
                prepared_weight_sum: prepared_sum,
                raw_to_prepared_multiplier,
                log_raw_to_prepared_multiplier,
            })
        }
    }
}

#[cfg(test)]
fn sample_weight_kind(weights: &PreparedWeights) -> Option<WeightKind> {
    match weights {
        PreparedWeights::None => None,
        PreparedWeights::Provided { kind, .. } => Some(*kind),
    }
}

fn separation_drop_reason(reason: SeparationDropReason) -> DropReason {
    match reason {
        SeparationDropReason::InitialSingleton | SeparationDropReason::SingletonCascade => {
            DropReason::Singleton
        }
        SeparationDropReason::FixedEffectZeroLevel => DropReason::FixedEffectSeparation,
        SeparationDropReason::Simplex => DropReason::SimplexSeparation,
        SeparationDropReason::IteratedRectifier => DropReason::IteratedRectifierSeparation,
        SeparationDropReason::Mu => DropReason::MuSeparation,
    }
}

fn original_axis_mask(rows: usize, retained: &[usize], label: &'static str) -> Result<Vec<bool>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(rows)
        .map_err(|_| allocation(label))?;
    if output.capacity() < rows {
        return Err(allocation(label));
    }
    output.resize(rows, false);
    for &row in retained {
        output[row] = true;
    }
    Ok(output)
}

fn compensated_sum(values: impl IntoIterator<Item = f64>) -> Result<f64> {
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for value in values {
        if !value.is_finite() || value <= 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "prepared PPML generation weights must be finite and positive",
            ));
        }
        neumaier_add(value, &mut sum, &mut correction);
    }
    let total = sum + correction;
    if !total.is_finite() || total <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "prepared PPML generation weight sum is not finite and positive",
        ));
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{NumericColumnView, NumericSourceUse, WeightInput};
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind,
        ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId,
        TermComponent, TermId, TermOrder, TimeSeriesState,
    };
    use crate::ols::{
        fit_prepared_model_coefficients, OlsInputBinding, OlsInputRole, PreparedOlsFitConfig,
    };
    use crate::ppml::prepared::{prepare_ppml_numeric_data, PpmlNumericPreparationInput};
    use crate::ppml_separation::{
        PreparedSeparationCascade, PreparedSeparationCoordinator, SeparationDropReason,
    };

    struct Fixture {
        request: OlsRequestV1,
        clusters: Vec<OlsClusterTermV1>,
        loaded: LoadedOlsInput,
        offset: LoadedNumericColumn,
        uses: Vec<NumericSourceUse>,
    }

    impl Fixture {
        fn prepare_numeric(&self) -> PreparedPpmlNumericData {
            let source_views = self
                .loaded
                .numeric_sources
                .iter()
                .map(|(_, column)| NumericColumnView::with_missing(&column.values, &column.missing))
                .collect::<Vec<_>>();
            let probability = self
                .loaded
                .probability_weight
                .as_ref()
                .expect("fixture probability weight");
            prepare_ppml_numeric_data(&PpmlNumericPreparationInput {
                candidate: &self.loaded.candidate,
                outcome: NumericColumnView::with_missing(
                    &self.loaded.outcome.values,
                    &self.loaded.outcome.missing,
                ),
                numeric_sources: &source_views,
                source_uses: &self.uses,
                weights: WeightInput::Provided {
                    kind: WeightKind::Probability,
                    column: NumericColumnView::with_missing(
                        &probability.values,
                        &probability.missing,
                    ),
                },
                offset: Some(NumericColumnView::with_missing(
                    &self.offset.values,
                    &self.offset.missing,
                )),
                exposure: None,
            })
            .expect("numeric preparation")
        }

        fn into_owner(self, numeric: PreparedPpmlNumericData) -> PreparedPpmlModelOwner {
            PreparedPpmlModelOwner::new(self.request, self.clusters, self.loaded, numeric)
                .expect("prepared owner")
        }
    }

    #[test]
    fn axis_ppml_complete_and_selected_generation_seams_own_explicit_rows() {
        for absorbed in [false, true] {
            for clustered in [false, true] {
                for excluded in [false, true] {
                    let mut fixture = categorical_clustered_fixture();
                    fixture.request.keep_singletons = true;
                    if !absorbed {
                        fixture.request.absorbed_terms.clear();
                    }
                    fixture.loaded.candidate.fill(true);
                    if excluded {
                        fixture.loaded.candidate[7] = false;
                    }
                    fixture.loaded.probability_weight = None;
                    // Use the second source for both optional grammars so source
                    // IDs stay contiguous even when the absorbed term is absent.
                    fixture.clusters[0].sources = vec![SourceId::new(2).unwrap()];
                    let used = |id| id == 1 || (id == 2 && (absorbed || clustered));
                    fixture
                        .request
                        .model
                        .sources
                        .retain(|source| used(source.id.get()));
                    fixture
                        .loaded
                        .numeric_sources
                        .retain(|(source, _)| used(source.get()));
                    fixture.request.bindings.retain(|binding| {
                        !matches!(binding.role, OlsInputRole::NumericSource(source) if !used(source.get()))
                    });
                    fixture
                        .request
                        .bindings
                        .retain(|binding| binding.role != OlsInputRole::ProbabilityWeight);
                    let prepared = if clustered {
                        prepare_ppml_categorical_ols_clustered_data(
                            &OlsClusteredRequestV1 {
                                ols: fixture.request,
                                clusters: fixture.clusters,
                            },
                            &fixture.loaded,
                            None,
                            0,
                        )
                    } else {
                        prepare_ppml_categorical_ols_data_with_load_hint(
                            &fixture.request,
                            &fixture.loaded,
                            &[],
                            None,
                            0,
                        )
                    }
                    .unwrap();
                    let rows = prepared
                        .sample()
                        .retained_rows()
                        .selected()
                        .expect("PPML owns row vector even for identity");
                    assert!(rows.iter().copied().eq(0..if excluded { 7 } else { 8 }));
                    assert!(
                        prepared
                            .sample()
                            .original_row_by_active
                            .owned_buffer_bytes()
                            >= std::mem::size_of_val(rows) as u128
                    );
                    assert_eq!(prepared.singletons().num_singletons, 0);
                }
            }
        }
    }

    #[test]
    fn preparation_options_are_consumed_once_and_preserve_pweight_rollback() {
        for first_generation in [0, 1] {
            let mut fixture = categorical_clustered_fixture();
            fixture.loaded.candidate.fill(true);
            let ids = crate::ols::PreparedOlsLoadHint::source_ids(&fixture.request).unwrap();
            let hint = crate::ols::PreparedOlsLoadHint::prepare(
                ids.iter().map(|id| {
                    let (_, column) = fixture
                        .loaded
                        .numeric_sources
                        .iter()
                        .find(|(source, _)| source == id)
                        .unwrap();
                    (*id, column.values.as_slice(), column.missing.as_slice())
                }),
                fixture.loaded.candidate.len(),
                u128::MAX,
            )
            .unwrap();
            let numeric = fixture.prepare_numeric();
            let mut owner = fixture
                .into_owner(numeric)
                .with_load_hint(Some(hint))
                .with_membership_scratch(4096);
            let baseline_candidate = owner.loaded.candidate.clone();
            let baseline_weights = owner.loaded.probability_weight.clone();
            let rows = owner.candidate_original_rows.to_vec();
            owner.generation_memory_bound = Some(1);
            assert_eq!(
                owner
                    .rebuild_generation(&rows, first_generation)
                    .unwrap_err()
                    .code,
                ErrorCode::Allocation
            );
            assert!(owner.load_hint.is_none());
            assert_eq!(owner.membership_scratch, 0);
            assert_eq!(owner.loaded.candidate, baseline_candidate);
            assert_eq!(owner.loaded.probability_weight, baseline_weights);
            owner.generation_memory_bound = None;
            let retry = owner.rebuild_generation(&rows, 0).unwrap();
            let later = owner.rebuild_generation(&rows, 1).unwrap();
            assert_eq!(retry.prepared().as_ref(), later.prepared().as_ref());
            assert_eq!(retry.prior_weights(), later.prior_weights());
            let subset = owner
                .rebuild_generation(&rows[..rows.len() - 2], 2)
                .unwrap();
            assert_eq!(subset.original_rows(), &rows[..rows.len() - 2]);
            assert_eq!(owner.loaded.candidate, baseline_candidate);
            assert_eq!(owner.loaded.probability_weight, baseline_weights);
        }
    }

    #[test]
    fn generation_capacity_guard_counts_spare_capacity_and_preserves_rollback() {
        let fixture = categorical_clustered_fixture();
        let numeric = fixture.prepare_numeric();
        let mut owner = fixture.into_owner(numeric);
        let rows = owner.candidate_original_rows.to_vec();
        let initial = owner.rebuild_generation(&rows, 0).unwrap();
        let exact = initial.owned_buffer_bytes();
        initial.validate_memory_bound(Some(exact)).unwrap();
        assert_eq!(
            initial
                .validate_memory_bound(Some(exact - 1))
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
        let candidate = owner.loaded.candidate.clone();
        let probability = owner.loaded.probability_weight.clone();
        owner.generation_memory_bound = Some(exact - 1);
        assert_eq!(
            owner.rebuild_generation(&rows, 1).unwrap_err().code,
            ErrorCode::Allocation
        );
        assert_eq!(owner.loaded.candidate, candidate);
        assert_eq!(owner.loaded.probability_weight, probability);
        owner.generation_memory_bound = Some(exact);
        for generation in 1..=3 {
            let replacement = owner.rebuild_generation(&rows, generation).unwrap();
            assert_eq!(replacement.owned_buffer_bytes(), exact);
            assert_eq!(replacement.outcome(), initial.outcome());
        }
        let mut spare = initial;
        spare.offset.reserve_exact(37);
        assert!(spare.owned_buffer_bytes() > exact);
        assert_eq!(
            spare.validate_memory_bound(Some(exact)).unwrap_err().code,
            ErrorCode::Allocation
        );
    }

    #[test]
    fn slope_generation_compaction_preserves_pweight_rebase_and_rollback() {
        let mut fixture = categorical_clustered_fixture();
        let x = SourceId::new(1).unwrap();
        let h = SourceId::new(3).unwrap();
        fixture.request.absorbed_terms[0].slope_sources = vec![x];
        fixture.request.absorbed_terms.push(AbsorbedTerm {
            id: AbsorbTermId::new(2).unwrap(),
            intercept: true,
            categorical_sources: vec![h],
            slope_sources: vec![x],
            label: "h##c.x".into(),
            saved_name: String::new(),
        });
        let numeric = fixture.prepare_numeric();
        let mut owner = fixture.into_owner(numeric);
        let candidate = owner.loaded.candidate.clone();
        let raw_weights = owner.loaded.probability_weight.clone();
        let all_rows = owner.candidate_original_rows.to_vec();
        let initial = owner.rebuild_generation(&all_rows, 0).unwrap();
        let subset = [0, 2, 3, 6];
        let rebased = owner.rebuild_generation(&subset, 1).unwrap();
        assert_eq!(rebased.original_rows(), subset);
        assert_eq!(rebased.fit_view().sample().n_active(), subset.len());
        assert_eq!(rebased.fit_view().effect_specs().len(), 2);
        assert!((rebased.prior_weights().iter().sum::<f64>() - subset.len() as f64).abs() < 1e-12);
        for (position, &row) in subset.iter().enumerate() {
            assert_eq!(
                rebased.outcome()[position],
                owner.loaded.outcome.values[row]
            );
            assert_eq!(
                rebased.fit_view().sample().regressors[0][position],
                owner.loaded.numeric_sources[0].1.values[row]
            );
        }
        assert_eq!(owner.loaded.candidate, candidate);
        assert_eq!(owner.loaded.probability_weight, raw_weights);
        owner.generation_memory_bound = Some(rebased.owned_buffer_bytes() - 1);
        assert_eq!(
            owner.rebuild_generation(&subset, 2).unwrap_err().code,
            ErrorCode::Allocation
        );
        assert_eq!(owner.loaded.candidate, candidate);
        assert_eq!(owner.loaded.probability_weight, raw_weights);
        owner.generation_memory_bound = Some(initial.owned_buffer_bytes());
        let restored = owner.rebuild_generation(&all_rows, 3).unwrap();
        assert_eq!(restored.original_rows(), initial.original_rows());
        assert_eq!(restored.prior_weights(), initial.prior_weights());
        assert_eq!(restored.fit_view().sample(), initial.fit_view().sample());
    }

    #[test]
    fn numerical_artifact_binding_rejects_substitution_and_rebase() {
        let fixture = categorical_clustered_fixture();
        let numeric = fixture.prepare_numeric();
        let mut owner = fixture.into_owner(numeric);
        let first = owner.rebuild_generation(&[0, 1, 2, 3, 4, 5, 6], 0).unwrap();
        let binding = PpmlGenerationBinding::capture(&first).unwrap();
        assert_eq!(binding.generation(), 0);
        binding.validate(&first, 7).unwrap();
        assert!(binding.validate(&first, 6).is_err());
        let equivalent = owner.rebuild_generation(first.original_rows(), 0).unwrap();
        assert_eq!(first.original_rows(), equivalent.original_rows());
        assert_eq!(first.fit_view(), equivalent.fit_view());
        assert!(binding.validate(&equivalent, 7).is_err());
        let rebased = owner.rebuild_generation(&[0, 2, 3, 5, 6], 1).unwrap();
        assert!(binding.validate(&rebased, 5).is_err());
        PpmlGenerationBinding::capture(&rebased)
            .unwrap()
            .validate(&rebased, 5)
            .unwrap();
    }

    #[test]
    fn categorical_clustered_generations_preserve_original_rows_and_defer_singletons() {
        let fixture = categorical_clustered_fixture();
        let numeric = fixture.prepare_numeric();
        let owner = fixture.into_owner(numeric);
        let mut coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");

        let initial = coordinator.view();
        assert_eq!(initial.original_rows, &[0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(initial.prepared.generation(), 0);
        assert_eq!(
            initial
                .prepared
                .retained_base_adjustment_capacity_elements(),
            7,
            "the owner-side offset keeps the immutable if/in candidate capacity",
        );
        assert!(initial.prepared.prepared().cluster_design().is_some());
        assert_eq!(
            initial.prepared.prepared().singletons(),
            PreparedOlsSingletons {
                kept_by_request: true,
                num_singletons: 0,
                cascade_events: 0,
            }
        );
        assert_eq!(
            initial.prepared.cluster_design().expect("clusters").len(),
            1
        );
        assert_eq!(initial.prepared.cluster_codes(0).expect("codes").len(), 7);
        assert!((initial.prepared.prior_weights().iter().sum::<f64>() - 7.0).abs() < 1e-12);
        assert_eq!(
            initial.prepared.offset(),
            &[0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6]
        );

        coordinator
            .transact_validated_proposal(
                SeparationDropReason::Simplex,
                vec![1],
                PreparedSeparationCascade {
                    singleton_rows: vec![4],
                    exact_fe_rows: Vec::new(),
                },
            )
            .expect("rebased transaction");
        let outcome = coordinator.into_outcome();
        assert_eq!(outcome.prepared.generation(), 1);
        assert_eq!(
            outcome
                .prepared
                .retained_base_adjustment_capacity_elements(),
            7,
            "separation must not shrink the owner-side adjustment reservation",
        );
        assert_eq!(outcome.prepared.original_rows(), &[0, 2, 3, 5, 6]);
        assert_eq!(outcome.prepared.offset(), &[0.0, 0.2, 0.3, 0.5, 0.6]);
        assert_eq!(outcome.prepared.outcome(), &[1.0, 2.0, 0.0, 0.0, 4.0]);
        assert_eq!(outcome.prepared.cluster_codes(0).expect("codes").len(), 5);
        let final_reasons = outcome
            .prepared
            .final_drop_reasons(&outcome.reasons)
            .expect("merged reasons");
        assert_eq!(final_reasons[1], DropReason::SimplexSeparation);
        assert_eq!(final_reasons[4], DropReason::Singleton);
        assert_eq!(final_reasons[7], DropReason::ExcludedByIfIn);
        assert_eq!(outcome.prepared.weight_report().physical_observations, 5);
    }

    #[test]
    fn failed_cluster_rebase_restores_the_raw_owner_candidate_mask() {
        let fixture = categorical_clustered_fixture();
        let numeric = fixture.prepare_numeric();
        let mut owner = fixture.into_owner(numeric);
        let baseline = owner.loaded.candidate.clone();
        let baseline_probability = owner.loaded.probability_weight.clone();
        let baseline_probability_bits = baseline_probability
            .as_ref()
            .expect("fixture probability weights")
            .values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>();

        let error = owner
            .rebuild_generation(&[0, 2, 4, 6], 1)
            .expect_err("one cluster group must fail");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert_eq!(owner.loaded.candidate, baseline);
        assert_eq!(owner.loaded.probability_weight, baseline_probability);
        assert_eq!(
            owner
                .loaded
                .probability_weight
                .as_ref()
                .expect("restored probability weights")
                .values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            baseline_probability_bits,
            "a failed build restores every supplied probability weight bit-for-bit",
        );

        let recovered = owner
            .rebuild_generation(&[0, 1, 2, 3, 4, 5, 6], 2)
            .expect("subsequent full rebuild");
        assert_eq!(recovered.original_rows(), &[0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(owner.loaded.candidate, baseline);
        assert_eq!(owner.loaded.probability_weight, baseline_probability);
    }

    #[test]
    fn probability_weights_renormalize_to_each_final_generation_n() {
        let fixture = categorical_clustered_fixture();
        let numeric = fixture.prepare_numeric();
        let mut owner = fixture.into_owner(numeric);
        let retained = [0, 1, 2, 3, 5, 6];
        let generation = owner
            .rebuild_generation(&retained, 1)
            .expect("rebased pweight generation");
        assert!(Arc::ptr_eq(
            &owner.initial_drop_reasons,
            &generation.initial_drop_reasons,
        ));
        let report = generation.weight_report();
        assert_eq!(report.physical_observations, retained.len());
        assert!((report.prepared_weight_sum - 6.0).abs() < 1e-12);
        assert!(
            (generation.prior_weights().iter().sum::<f64>() - retained.len() as f64).abs() < 1e-12
        );
        assert!((report.raw_weight_sum.expect("raw sum") - 23.0).abs() < 1e-12);
        assert!(
            (report.raw_to_prepared_multiplier.expect("multiplier") - 6.0 / 23.0).abs() < 1e-12
        );
        assert!((report.log_raw_weight_sum.expect("log raw sum") - 23.0_f64.ln()).abs() < 1e-12);
        assert!((report.log_raw_to_prepared_multiplier - (6.0_f64 / 23.0).ln()).abs() < 1e-12);

        let mut rescaled_fixture = categorical_clustered_fixture();
        for value in &mut rescaled_fixture
            .loaded
            .probability_weight
            .as_mut()
            .expect("pweights")
            .values
        {
            *value *= 8.0;
        }
        let rescaled_numeric = rescaled_fixture.prepare_numeric();
        let mut rescaled_owner = rescaled_fixture.into_owner(rescaled_numeric);
        let rescaled = rescaled_owner
            .rebuild_generation(&retained, 1)
            .expect("rescaled pweight generation");
        assert_eq!(generation.prior_weights(), rescaled.prior_weights());
        assert!(
            (rescaled.weight_report().prepared_weight_sum - report.prepared_weight_sum).abs()
                < 1e-12
        );
        assert!((rescaled.weight_report().raw_weight_sum.expect("raw sum") - 184.0).abs() < 1e-12);
        assert!(
            (rescaled
                .weight_report()
                .raw_to_prepared_multiplier
                .expect("rescaled multiplier")
                - 6.0 / 184.0)
                .abs()
                < 1e-12
        );
        assert!(
            (rescaled
                .weight_report()
                .log_raw_weight_sum
                .expect("rescaled log raw sum")
                - report.log_raw_weight_sum.expect("log raw sum")
                - 8.0_f64.ln())
            .abs()
                < 1e-12
        );
    }

    #[test]
    fn tiny_equal_pweights_rebase_locally_without_an_overflowing_scalar_multiplier() {
        let mut fixture = categorical_clustered_fixture();
        let tiny = f64::from_bits(1);
        let values = &mut fixture
            .loaded
            .probability_weight
            .as_mut()
            .expect("pweights")
            .values;
        values.fill(1.0);
        values[1] = tiny;
        values[2] = tiny;

        let numeric = fixture.prepare_numeric();
        let mut owner = fixture.into_owner(numeric);
        let baseline = owner.loaded.probability_weight.clone();
        let stage = owner
            .stage_generation_probability_weights(&[1, 2])
            .expect("equal tiny pweights have a representable local normalization")
            .expect("probability stage");
        let staged = owner
            .loaded
            .probability_weight
            .as_ref()
            .expect("staged pweights");
        assert_eq!(staged.values[1], 1.0);
        assert_eq!(staged.values[2], 1.0);
        assert_eq!(stage.scale.raw_sum, Some(f64::from_bits(2)));
        assert!(stage.scale.log_raw_sum.is_finite());
        assert!(stage.scale.log_raw_to_prepared_multiplier.is_finite());
        assert_eq!(stage.positions, [1, 2]);
        assert_eq!(
            stage.positions.capacity(),
            2,
            "rollback retains only one exact-capacity source position per staged row",
        );

        owner.restore_generation_probability_weights(Some(stage));
        assert_eq!(owner.loaded.probability_weight, baseline);
    }

    #[test]
    fn no_absorb_generation_keeps_the_intercept_in_the_categorical_finite_design() {
        let mut fixture = categorical_clustered_fixture();
        fixture.request.absorbed_terms.clear();
        fixture.clusters.clear();
        fixture.request.keep_singletons = true;
        fixture.request.model.sources.truncate(1);
        fixture.request.bindings.retain(|binding| {
            !matches!(binding.role, OlsInputRole::NumericSource(source) if source.get() > 1)
        });
        fixture.loaded.numeric_sources.truncate(1);
        fixture.uses.truncate(1);
        let numeric = fixture.prepare_numeric();
        let mut owner = fixture.into_owner(numeric);
        let generation = owner
            .rebuild_generation(&[0, 1, 2, 3, 4, 5, 6], 0)
            .expect("plain generation");
        assert!(generation.prepared().cluster_design().is_none());
        let categorical = generation.prepared().as_ref();
        assert!(categorical.effect_specs().is_empty());
        let concrete_fit: &PreparedOlsData = generation.fit_view();
        assert!(!concrete_fit.has_absorbed_intercept());
        assert!(concrete_fit.report_intercept());
        let fit = fit_prepared_model_coefficients(categorical, PreparedOlsFitConfig::default())
            .expect("no-absorb categorical finite fit");
        let intercept = fit.coefficients().last().expect("finite intercept slot");
        assert_eq!(intercept.status, crate::ols::PreparedOlsFitStatus::Kept);
        assert!(intercept.value.is_some());
        assert!(fit.bread().coordinates().iter().any(|coordinate| {
            coordinate.generated_column_index == intercept.generated_column_index.expect("index")
                && coordinate.kind == crate::ols::PreparedOlsBreadCoordinateKind::FiniteRegressor
        }));
        assert!(generation.cluster_design().is_none());
        assert_eq!(generation.fit_view().sample().n_active(), 7);
        let fit = fit_prepared_model_coefficients(
            generation.prepared().as_ref(),
            PreparedOlsFitConfig::default(),
        )
        .expect("no-FE finite fit");
        assert_eq!(fit.finite_rank(), 2);
        assert_eq!(fit.kept_generated_columns().len(), 2);
    }

    #[test]
    fn owner_rejects_numeric_data_from_a_different_loaded_source() {
        let mut fixture = categorical_clustered_fixture();
        let numeric = fixture.prepare_numeric();
        fixture.loaded.outcome.values[2] = 99.0;
        let error =
            PreparedPpmlModelOwner::new(fixture.request, fixture.clusters, fixture.loaded, numeric)
                .err()
                .expect("mismatched owner must fail");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn owner_rejects_more_than_five_cluster_dimensions() {
        let mut fixture = categorical_clustered_fixture();
        let numeric = fixture.prepare_numeric();
        let term = fixture.clusters[0].clone();
        fixture.clusters = vec![term; 6];
        let error =
            PreparedPpmlModelOwner::new(fixture.request, fixture.clusters, fixture.loaded, numeric)
                .err()
                .expect("six clusters must fail");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }

    fn categorical_clustered_fixture() -> Fixture {
        let x = SourceId::new(1).expect("x");
        let g = SourceId::new(2).expect("g");
        let h = SourceId::new(3).expect("h");
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![source(x, "x"), source(g, "g"), source(h, "h")],
                terms: vec![continuous_term(x)],
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorb"),
                intercept: true,
                categorical_sources: vec![g],
                slope_sources: Vec::new(),
                label: "g".to_owned(),
                saved_name: String::new(),
            }],
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
                    role: OlsInputRole::NumericSource(x),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(g),
                },
                OlsInputBinding {
                    stata_var_position: 5,
                    role: OlsInputRole::NumericSource(h),
                },
                OlsInputBinding {
                    stata_var_position: 6,
                    role: OlsInputRole::ProbabilityWeight,
                },
            ],
            include_intercept: true,
            keep_singletons: false,
        };
        Fixture {
            request,
            clusters: vec![OlsClusterTermV1 {
                label: "h".to_owned(),
                sources: vec![h],
            }],
            loaded: LoadedOlsInput {
                candidate: vec![true, true, true, true, true, true, true, false],
                outcome: column(&[1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0, 0.0]),
                numeric_sources: vec![
                    (x, column(&[0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0])),
                    (g, column(&[1.0, 1.0, 2.0, 2.0, 3.0, 4.0, 4.0, 5.0])),
                    (h, column(&[10.0, 20.0, 10.0, 20.0, 10.0, 20.0, 10.0, 20.0])),
                ],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: Some(column(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0])),
            },
            offset: column(&[0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]),
            uses: vec![
                NumericSourceUse::ordinary(),
                NumericSourceUse::fixed_effect(),
                NumericSourceUse {
                    ordinary: false,
                    fixed_effect: false,
                    cluster: true,
                },
            ],
        }
    }

    fn source(id: SourceId, name: &str) -> SourceEntry {
        SourceEntry {
            id,
            canonical_name: name.to_owned(),
        }
    }

    fn continuous_term(source: SourceId) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(1).expect("term"),
            order: TermOrder::new(1).expect("order"),
            coefficient_slot: CoefficientSlot::new(1).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: "c.x".to_owned(),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component order"),
                source,
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
    }

    fn column(values: &[f64]) -> LoadedNumericColumn {
        LoadedNumericColumn {
            values: values.to_vec(),
            missing: vec![false; values.len()],
        }
    }
}
