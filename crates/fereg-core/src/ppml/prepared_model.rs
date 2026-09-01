//! PPML-owned prepared categorical model generations.
//!
//! Numeric/domain preparation is authoritative exactly once.  This owner then
//! keeps the raw loaded source registry and rebuilds only row-scoped finite,
//! fixed-effect, and cluster topology after separation commits.  Rebuilds are
//! keyed by stable original-row identifiers and always run the shared OLS
//! categorical preparation seam with singleton deletion disabled.  PPML's
//! separation coordinator is the sole owner of singleton/separation deletion.

use std::sync::Arc;

use super::prepared::{
    canonicalize_probability_weights, checked_frequency_effective_observations,
    PreparedPpmlAdjustmentReport, PreparedPpmlNumericData, PreparedPpmlWeightReport,
};
use crate::data::{
    DropReason, PreparedAdjustment, PreparedNumericSample, PreparedWeights, WeightKind,
};
use crate::error::{ErrorCode, FeregError, Result};
use crate::ols::{
    prepare_ppml_categorical_ols_clustered_data, LoadedNumericColumn, LoadedOlsInput,
    OlsClusterTermV1, OlsClusteredRequestV1, OlsRequestV1, PreparedCategoricalOlsClusteredData,
    PreparedOlsClusterDesign, PreparedOlsModel, PreparedOlsSingletons,
};
use crate::ppml_separation::{PreparedSeparationRebuilder, SeparationDropReason};
use crate::prepared_categorical_data::{
    prepare_ppml_categorical_ols_data, PreparedCategoricalOlsData,
};

const MAX_PPML_CLUSTER_DIMENSIONS: usize = 5;

/// One immutable, sample-bound finite/FE/cluster artifact.
///
/// Both variants use the categorical prepared-data seam, including the empty
/// topology used by no-FE PPML, and differ only in whether cluster metadata is
/// owned beside the fit view. No variant converts topology to `FeDesign`.
#[derive(Debug, PartialEq)]
pub(crate) enum PreparedPpmlModel {
    Categorical(PreparedCategoricalOlsData),
    CategoricalClustered(PreparedCategoricalOlsClusteredData),
}

impl PreparedPpmlModel {
    /// Resolve the PPML model variant once to the one concrete prepared
    /// layout used by every production generation. Keeping this return type
    /// concrete is performance-critical: the shared linear session then
    /// monomorphizes its observation loops instead of calling the prepared
    /// view through a vtable.
    pub(crate) fn fit_view(&self) -> &PreparedCategoricalOlsData {
        match self {
            Self::Categorical(prepared) => prepared,
            Self::CategoricalClustered(prepared) => prepared.ols(),
        }
    }

    pub(crate) fn model(&self) -> &PreparedOlsModel {
        match self {
            Self::Categorical(prepared) => prepared.model(),
            Self::CategoricalClustered(prepared) => prepared.ols().model(),
        }
    }

    pub(crate) fn sample(&self) -> &PreparedNumericSample {
        match self {
            Self::Categorical(prepared) => prepared.sample(),
            Self::CategoricalClustered(prepared) => prepared.ols().sample(),
        }
    }

    pub(crate) fn singletons(&self) -> PreparedOlsSingletons {
        match self {
            Self::Categorical(prepared) => prepared.singletons(),
            Self::CategoricalClustered(prepared) => prepared.ols().singletons(),
        }
    }

    pub(crate) fn categorical(&self) -> Option<&PreparedCategoricalOlsData> {
        match self {
            Self::Categorical(prepared) => Some(prepared),
            Self::CategoricalClustered(prepared) => Some(prepared.ols()),
        }
    }

    pub(crate) fn clusters(&self) -> Option<&PreparedOlsClusterDesign> {
        match self {
            Self::CategoricalClustered(prepared) => Some(prepared.clusters()),
            Self::Categorical(_) => None,
        }
    }

    pub(crate) fn cluster_codes(&self, dimension: usize) -> Result<&[u32]> {
        match self {
            Self::CategoricalClustered(prepared) => prepared.cluster_codes(dimension),
            Self::Categorical(_) => Err(invalid_argument(
                "prepared PPML model has no declared cluster dimensions",
            )),
        }
    }
}

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
    /// Capacity-bearing length of the owner's immutable raw pweight subset.
    /// It remains candidate-sized after separation generations shrink.
    retained_raw_candidate_probability_weight_elements: usize,
    initial_drop_reasons: Arc<Vec<DropReason>>,
    weight_report: PreparedPpmlWeightReport,
    adjustment_report: PreparedPpmlAdjustmentReport,
}

impl PreparedPpmlModelGeneration {
    pub(crate) const fn generation(&self) -> usize {
        self.generation
    }

    pub(crate) const fn prepared(&self) -> &PreparedPpmlModel {
        &self.prepared
    }

    pub(crate) fn fit_view(&self) -> &PreparedCategoricalOlsData {
        self.prepared.fit_view()
    }

    pub(crate) fn model(&self) -> &PreparedOlsModel {
        self.prepared.model()
    }

    pub(crate) fn original_rows(&self) -> &[usize] {
        self.prepared.sample().retained_rows()
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

    pub(crate) const fn retained_raw_candidate_probability_weight_elements(&self) -> usize {
        self.retained_raw_candidate_probability_weight_elements
    }

    pub(crate) const fn weight_report(&self) -> PreparedPpmlWeightReport {
        self.weight_report
    }

    pub(crate) const fn adjustment_report(&self) -> PreparedPpmlAdjustmentReport {
        self.adjustment_report
    }

    pub(crate) fn cluster_design(&self) -> Option<&PreparedOlsClusterDesign> {
        self.prepared.clusters()
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
    pub(crate) fn new(
        mut request: OlsRequestV1,
        clusters: Vec<OlsClusterTermV1>,
        loaded: LoadedOlsInput,
        numeric: PreparedPpmlNumericData,
    ) -> Result<Self> {
        if clusters.len() > MAX_PPML_CLUSTER_DIMENSIONS {
            return Err(invalid_argument(
                "prepared PPML supports at most five declared cluster dimensions",
            ));
        }
        let weight_report = numeric.weights();
        let adjustment_report = numeric.adjustment();
        let sample = numeric.into_sample();
        validate_loaded_numeric_pair(&loaded, &sample, weight_report)?;
        let base_probability_weights = capture_raw_probability_weights(&loaded, &sample)?;

        let base_offset = match sample.adjustment {
            PreparedAdjustment::None => None,
            PreparedAdjustment::Offset(values) => Some(values),
            PreparedAdjustment::Exposure(_) => {
                return Err(invalid_argument(
                    "prepared PPML owner received an unlogged exposure",
                ));
            }
        };
        let original_observations = sample.n_original;
        let requested_candidate_observations = sample.n_candidate;
        let candidate_original_rows = Arc::new(sample.original_row_by_active);
        let initial_drop_reasons = Arc::new(sample.drop_reasons);
        // Shared categorical preparation must only establish source
        // availability and topology. PPML's coordinator applies the requested
        // singleton policy as part of its certified fixed point.
        request.keep_singletons = true;
        Ok(Self {
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

    fn build_prepared_model(&self) -> Result<PreparedPpmlModel> {
        // PPML always enters through the categorical preparation seam, even
        // with no absorbed effects.  The plain OLS view represents its
        // structural intercept as a virtual one-level absorbed effect; that
        // is correct for OLS's plain/categorical unification but would turn a
        // no-FE PPML `_cons` into a normalized structural constant.  An empty
        // categorical topology instead leaves `_cons` in the finite design,
        // while using the identical production view once effects are present.
        if self.request.clusters.is_empty() {
            Ok(PreparedPpmlModel::Categorical(
                prepare_ppml_categorical_ols_data(&self.request.ols, &self.loaded, &[])?,
            ))
        } else {
            Ok(PreparedPpmlModel::CategoricalClustered(
                prepare_ppml_categorical_ols_clustered_data(&self.request, &self.loaded)?,
            ))
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
        let mut prior_bits = Vec::new();
        prior_bits
            .try_reserve_exact(retained_original_rows.len())
            .map_err(|_| allocation("unable to reserve PPML pweight rollback state"))?;
        for &original_row in retained_original_rows {
            prior_bits.push(ProbabilityWeightBits {
                original_row,
                value: target.values[original_row],
                missing: target.missing[original_row],
            });
        }

        // No fallible operation occurs after publication to the loaded owner;
        // `rebuild_generation` restores these exact bits immediately after its
        // build attempt, irrespective of that attempt's result.
        let target = self.loaded.probability_weight.as_mut().ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "prepared PPML probability-weight column vanished during staging",
            )
        })?;
        for ((&original_row, value), bits) in retained_original_rows
            .iter()
            .zip(staged_values)
            .zip(&prior_bits)
        {
            debug_assert_eq!(original_row, bits.original_row);
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
            prior_bits,
        }))
    }

    fn restore_generation_probability_weights(
        &mut self,
        stage: Option<ProbabilityGenerationStage>,
    ) -> Result<()> {
        let Some(stage) = stage else {
            return Ok(());
        };
        let target = self.loaded.probability_weight.as_mut().ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "prepared PPML probability-weight column vanished before rollback",
            )
        })?;
        for bits in stage.prior_bits {
            target.values[bits.original_row] = bits.value;
            target.missing[bits.original_row] = bits.missing;
        }
        Ok(())
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
    prior_bits: Vec<ProbabilityWeightBits>,
}

struct ProbabilityWeightBits {
    original_row: usize,
    value: f64,
    missing: bool,
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
        let prepared_result = self.build_prepared_model();
        self.loaded.candidate = baseline_candidate;
        self.restore_generation_probability_weights(probability_stage)?;
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
        let retained_raw_candidate_probability_weight_elements =
            self.base_probability_weights.as_ref().map_or(0, Vec::len);
        Ok(PreparedPpmlModelGeneration {
            generation,
            prepared,
            offset,
            unit_prior_weights,
            original_observations: self.original_observations,
            requested_candidate_observations: self.requested_candidate_observations,
            retained_raw_candidate_probability_weight_elements,
            initial_drop_reasons: Arc::clone(&self.initial_drop_reasons),
            weight_report,
            adjustment_report: self.adjustment_report,
        })
    }
}

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
            .windows(2)
            .any(|rows| rows[0] >= rows[1])
    {
        return Err(invalid_argument(
            "prepared PPML numeric data is not aligned with its loaded source owner",
        ));
    }
    for (active_row, &original_row) in sample.original_row_by_active.iter().enumerate() {
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
    sample: &PreparedNumericSample,
) -> Result<Option<Vec<f64>>> {
    match &sample.weights {
        PreparedWeights::None => Ok(None),
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            ..
        } => {
            if loaded.frequency_weight.is_none() {
                return Err(invalid_argument(
                    "prepared PPML frequency-weight column is absent from the loaded owner",
                ));
            }
            Ok(None)
        }
        PreparedWeights::Provided {
            kind: WeightKind::Probability,
            ..
        } => {
            let raw = loaded.probability_weight.as_ref().ok_or_else(|| {
                invalid_argument(
                    "prepared PPML probability-weight column is absent from the loaded owner",
                )
            })?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(sample.original_row_by_active.len())
                .map_err(|_| allocation("unable to reserve raw prepared PPML pweights"))?;
            for &original_row in &sample.original_row_by_active {
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
        PreparedWeights::Provided {
            kind: WeightKind::Analytic | WeightKind::Importance,
            ..
        } => Err(FeregError::new(
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
    if sample.retained_rows() != retained_original_rows
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
    if let Some(clusters) = prepared.clusters() {
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

fn fallible_filled<T: Clone>(len: usize, value: T, label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(len)
        .map_err(|_| allocation(label))?;
    if output.capacity() < len {
        return Err(allocation(label));
    }
    output.resize(len, value);
    Ok(output)
}

fn fallible_copy<T: Copy>(input: &[T], label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| allocation(label))?;
    if output.capacity() < input.len() {
        return Err(allocation(label));
    }
    output.extend_from_slice(input);
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
        let next = sum + value;
        correction += if sum.abs() >= value.abs() {
            (sum - next) + value
        } else {
            (value - next) + sum
        };
        sum = next;
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

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
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
        fit_prepared_categorical_ols_coefficients_map, OlsInputBinding, OlsInputRole,
        PreparedOlsFitConfig,
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
    fn categorical_clustered_generations_preserve_original_rows_and_defer_singletons() {
        let fixture = categorical_clustered_fixture();
        let numeric = fixture.prepare_numeric();
        let owner = fixture.into_owner(numeric);
        let mut coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");

        let initial = coordinator.view();
        assert_eq!(initial.original_rows, &[0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(initial.prepared.generation(), 0);
        assert!(matches!(
            initial.prepared.prepared(),
            PreparedPpmlModel::CategoricalClustered(_)
        ));
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
            .transact_validated_proposal(SeparationDropReason::Simplex, &[1], |_| {
                Ok(PreparedSeparationCascade {
                    singleton_rows: vec![4],
                    exact_fe_rows: Vec::new(),
                })
            })
            .expect("rebased transaction");
        let outcome = coordinator.into_outcome();
        assert_eq!(outcome.prepared.generation(), 1);
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

        let error = owner
            .rebuild_generation(&[0, 2, 4, 6], 1)
            .expect_err("one cluster group must fail");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert_eq!(owner.loaded.candidate, baseline);
        assert_eq!(owner.loaded.probability_weight, baseline_probability);

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

        owner
            .restore_generation_probability_weights(Some(stage))
            .expect("rollback");
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
        assert!(matches!(
            generation.prepared(),
            PreparedPpmlModel::Categorical(_)
        ));
        let categorical = generation
            .prepared()
            .categorical()
            .expect("categorical PPML view");
        assert!(categorical.effect_specs().is_empty());
        let concrete_fit: &PreparedCategoricalOlsData = generation.fit_view();
        assert!(!concrete_fit.has_absorbed_intercept());
        assert!(concrete_fit.report_intercept());
        let fit = fit_prepared_categorical_ols_coefficients_map(
            categorical,
            PreparedOlsFitConfig::default(),
        )
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
        let fit = fit_prepared_categorical_ols_coefficients_map(
            generation
                .prepared()
                .categorical()
                .expect("categorical PPML view"),
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
