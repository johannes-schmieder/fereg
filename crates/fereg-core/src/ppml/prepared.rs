use crate::data::{
    classify_numeric_sample_with_source_uses, prepare_numeric_sample_with_source_uses,
    AdjustmentInput, DomainDecision, DropReason, NumericColumnView, NumericDomainPolicy,
    NumericPreparationInput, NumericSampleClassification, NumericSourceUse, PreparedAdjustment,
    PreparedNumericSample, PreparedWeights, WeightInput, WeightKind,
};
use crate::error::{ErrorCode, FeregError, Result};

/// Borrowed PPML inputs before authoritative sample construction.
///
/// Offset and exposure remain separate here because the caller-facing syntax
/// can supply both. The preparation boundary rejects that ambiguity before it
/// constructs the generic mutually exclusive `AdjustmentInput`.
#[derive(Debug)]
pub struct PpmlNumericPreparationInput<'a> {
    pub candidate: &'a [bool],
    pub outcome: NumericColumnView<'a>,
    /// Deduplicated numeric model sources in registry order.
    pub numeric_sources: &'a [NumericColumnView<'a>],
    /// One entry per numeric source, preserving ordinary/FE/cluster precedence.
    pub source_uses: &'a [NumericSourceUse],
    pub weights: WeightInput<'a>,
    pub offset: Option<NumericColumnView<'a>>,
    pub exposure: Option<NumericColumnView<'a>>,
}

/// The user-scale adjustment supplied to PPML preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedPpmlAdjustmentKind {
    None,
    Offset,
    Exposure,
}

/// How the retained adjustment is represented on the canonical PPML axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedPpmlAdjustmentReport {
    pub input_kind: PreparedPpmlAdjustmentKind,
    /// Offset values are canonical. Exposure values are logged exactly once
    /// and then stored as offsets, so later stages must never log them again.
    pub prepared_as_offset: bool,
    pub exposure_log_transformed: bool,
}

/// Prior-weight scale and effective observation experiment after compaction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedPpmlWeightReport {
    pub kind: Option<WeightKind>,
    pub physical_observations: usize,
    /// Physical N for unweighted/pweighted data; checked literal multiplicity
    /// for frequency-weighted data.
    pub effective_observations: usize,
    /// Sum on the supplied scale when it has a finite `f64` representation.
    /// This is absent for an unweighted model and may also be absent for
    /// pweights whose individually finite values have an overflowing sum.
    pub raw_weight_sum: Option<f64>,
    /// Natural logarithm of the supplied-scale sum. This is absent exactly
    /// for an unweighted model and remains finite when `raw_weight_sum`
    /// cannot be represented.
    pub log_raw_weight_sum: Option<f64>,
    /// Sum of the weights stored in `PreparedNumericSample` (or physical N
    /// when the sample has no materialized weight column).
    pub prepared_weight_sum: f64,
    /// Multiplier from supplied values to the canonical computational scale
    /// when that positive value has a finite `f64` representation. This is
    /// one for unweighted/fweighted data and N/sum(raw) for pweights; extreme
    /// pweight scales may make the scalar absent without narrowing the valid
    /// pweight domain.
    pub raw_to_prepared_multiplier: Option<f64>,
    /// Natural logarithm of the raw-to-prepared multiplier. This is always
    /// finite, equals zero for unweighted/fweighted data, and is authoritative
    /// whenever the scalar multiplier is not representable.
    pub log_raw_to_prepared_multiplier: f64,
}

/// Result of one exact-domain probability-weight normalization attempt.
///
/// Raw pweights may have a finite-positive domain that is wider than one
/// normalized `f64` vector can represent.  The log fields remain available
/// across that entire raw domain, while successful construction of this value
/// additionally certifies that every normalized computational weight is
/// strictly positive and finite.  Failure of that latter numerical
/// materialization is nonconvergence, never an invalid-data classification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CanonicalProbabilityWeightScale {
    pub(crate) raw_sum: Option<f64>,
    pub(crate) log_raw_sum: f64,
    pub(crate) prepared_sum: f64,
    pub(crate) raw_to_prepared_multiplier: Option<f64>,
    pub(crate) log_raw_to_prepared_multiplier: f64,
}

/// PPML-owned numeric preparation result before categorical topology,
/// singleton pruning, separation, or any estimator iteration.
#[derive(Debug, PartialEq)]
pub struct PreparedPpmlNumericData {
    sample: PreparedNumericSample,
    weights: PreparedPpmlWeightReport,
    adjustment: PreparedPpmlAdjustmentReport,
}

/// Scalar shape certified by the same row-domain policy as canonical numeric
/// preparation, without allocating an observation-sized ledger or compacted
/// column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PpmlNumericClassification {
    pub(crate) n_original: usize,
    pub(crate) n_candidate: usize,
    pub(crate) n_active: usize,
}

impl PreparedPpmlNumericData {
    #[must_use]
    pub const fn sample(&self) -> &PreparedNumericSample {
        &self.sample
    }

    #[must_use]
    pub const fn weights(&self) -> PreparedPpmlWeightReport {
        self.weights
    }

    #[must_use]
    pub const fn adjustment(&self) -> PreparedPpmlAdjustmentReport {
        self.adjustment
    }

    /// Canonical retained offset. Exposure inputs have already been logged.
    #[must_use]
    pub fn offset(&self) -> Option<&[f64]> {
        match &self.sample.adjustment {
            PreparedAdjustment::Offset(values) => Some(values),
            PreparedAdjustment::None | PreparedAdjustment::Exposure(_) => None,
        }
    }

    #[must_use]
    pub fn into_sample(self) -> PreparedNumericSample {
        self.sample
    }
}

/// Validate and count the authoritative PPML numeric sample without
/// materializing it.
///
/// This pass exists solely so fit-lifetime memory routing can fail closed
/// before canonical compaction. It invokes the shared row classifier and the
/// same PPML domain policy as [`prepare_ppml_numeric_data`]. The sole retained
/// state is a few scalar counts and the first deferred nonfinite-weight row.
pub(crate) fn classify_ppml_numeric_data(
    input: &PpmlNumericPreparationInput<'_>,
) -> Result<PpmlNumericClassification> {
    let (_, adjustment) = ppml_adjustment(input)?;
    validate_ppml_weight_kind(input.weights)?;
    let generic = NumericPreparationInput {
        candidate: input.candidate,
        outcome: input.outcome,
        regressors: input.numeric_sources,
        weights: input.weights,
        adjustment,
    };
    let mut has_positive_outcome = false;
    let mut first_nonfinite_domain = None;
    let mut first_nonfinite_weight = None;
    let NumericSampleClassification {
        n_original,
        n_candidate,
        n_active,
    } = classify_numeric_sample_with_source_uses(
        &generic,
        input.source_uses,
        &PpmlNumericDomainPolicy,
        |original_row, reason| {
            if first_nonfinite_domain.is_none()
                && is_unmasked_nonfinite_ppml_domain_row(input, original_row, reason)
            {
                first_nonfinite_domain = Some((original_row, reason));
            }
            if reason == DropReason::Retained && input.outcome.values[original_row] > 0.0 {
                has_positive_outcome = true;
            }
            if first_nonfinite_weight.is_none()
                && reason == DropReason::InvalidOrMissingWeight
                && matches!(
                    input.weights,
                    WeightInput::Provided { column, .. }
                        if !column.missing.is_some_and(|mask| mask[original_row])
                            && !column.values[original_row].is_finite()
                )
            {
                first_nonfinite_weight = Some(original_row);
            }
            Ok(())
        },
    )?;
    if let Some((original_row, reason)) = first_nonfinite_domain {
        validate_unmasked_nonfinite_ppml_domain_row(input, original_row, reason)?;
        unreachable!("recorded nonfinite PPML domain must fail validation");
    }
    if let Some(original_row) = first_nonfinite_weight {
        validate_unmasked_nonfinite_weight_row(input.weights, original_row)?;
        unreachable!("recorded nonfinite PPML weight must fail validation");
    }
    if n_active == 0 || !has_positive_outcome {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "PPML final numeric sample has no positive weighted outcome",
        ));
    }
    Ok(PpmlNumericClassification {
        n_original,
        n_candidate,
        n_active,
    })
}

fn ppml_adjustment<'a>(
    input: &PpmlNumericPreparationInput<'a>,
) -> Result<(PreparedPpmlAdjustmentKind, AdjustmentInput<'a>)> {
    match (input.offset, input.exposure) {
        (Some(_), Some(_)) => Err(FeregError::new(
            ErrorCode::Syntax,
            "PPML offset and exposure are mutually exclusive",
        )),
        (Some(offset), None) => Ok((
            PreparedPpmlAdjustmentKind::Offset,
            AdjustmentInput::Offset(offset),
        )),
        (None, Some(exposure)) => Ok((
            PreparedPpmlAdjustmentKind::Exposure,
            AdjustmentInput::Exposure(exposure),
        )),
        (None, None) => Ok((PreparedPpmlAdjustmentKind::None, AdjustmentInput::None)),
    }
}

fn validate_ppml_weight_kind(weights: WeightInput<'_>) -> Result<()> {
    if matches!(
        weights,
        WeightInput::Provided {
            kind: WeightKind::Analytic | WeightKind::Importance,
            ..
        }
    ) {
        return Err(FeregError::new(
            ErrorCode::UnsupportedWeight,
            "PPML numeric preparation supports only fweights and pweights",
        ));
    }
    Ok(())
}

/// Construct the PPML numeric sample without building categorical topology.
///
/// The generic preparation seam remains authoritative for row order, missing
/// data precedence, compact-axis alignment, and the original-row drop ledger.
/// This wrapper supplies only PPML scientific-domain decisions and canonical
/// post-compaction weight/adjustment representations.
pub fn prepare_ppml_numeric_data(
    input: &PpmlNumericPreparationInput<'_>,
) -> Result<PreparedPpmlNumericData> {
    let (adjustment_kind, adjustment) = ppml_adjustment(input)?;
    validate_ppml_weight_kind(input.weights)?;
    let generic = NumericPreparationInput {
        candidate: input.candidate,
        outcome: input.outcome,
        regressors: input.numeric_sources,
        weights: input.weights,
        adjustment,
    };
    let mut sample = prepare_numeric_sample_with_source_uses(
        &generic,
        input.source_uses,
        &PpmlNumericDomainPolicy,
    )?;
    validate_unmasked_nonfinite_ppml_domains(input, &sample)?;
    validate_unmasked_nonfinite_weights(input.weights, &sample)?;

    if sample.n_active() == 0 || !sample.outcome.iter().any(|value| *value > 0.0) {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "PPML final numeric sample has no positive weighted outcome",
        ));
    }

    let weights = canonicalize_weights(&mut sample)?;
    let adjustment = canonicalize_adjustment(&mut sample, adjustment_kind)?;
    Ok(PreparedPpmlNumericData {
        sample,
        weights,
        adjustment,
    })
}

/// The shared sample seam deliberately treats nonfinite values like missing
/// values so OLS and Stata's explicit missing masks retain their established
/// semantics. PPML has a narrower public domain: an unmasked NaN/Inf outcome,
/// offset, or exposure on a row that reaches that field is invalid data. Use
/// the final precedence ledger to distinguish those values from explicit
/// missingness and from values on rows excluded by an earlier rule.
fn validate_unmasked_nonfinite_ppml_domains(
    input: &PpmlNumericPreparationInput<'_>,
    sample: &PreparedNumericSample,
) -> Result<()> {
    for (original_row, reason) in sample.drop_reasons.iter().copied().enumerate() {
        validate_unmasked_nonfinite_ppml_domain_row(input, original_row, reason)?;
    }
    Ok(())
}

fn validate_unmasked_nonfinite_ppml_domain_row(
    input: &PpmlNumericPreparationInput<'_>,
    original_row: usize,
    reason: DropReason,
) -> Result<()> {
    if reason == DropReason::MissingOutcome
        && !input.outcome.missing.is_some_and(|mask| mask[original_row])
        && !input.outcome.values[original_row].is_finite()
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            format!(
                "PPML outcome at observation {} must be finite and nonnegative",
                original_row + 1
            ),
        ));
    }

    if reason != DropReason::MissingOffsetOrExposure {
        return Ok(());
    }
    let (kind, column) = match (input.offset, input.exposure) {
        (Some(column), None) => ("offset", column),
        (None, Some(column)) => ("exposure", column),
        (None, None) => return Ok(()),
        (Some(_), Some(_)) => unreachable!("mutual exclusion checked before preparation"),
    };
    if column.missing.is_some_and(|mask| mask[original_row])
        || column.values[original_row].is_finite()
    {
        return Ok(());
    }
    Err(FeregError::new(
        ErrorCode::InvalidData,
        format!(
            "PPML {kind} at observation {} must be finite",
            original_row + 1
        ),
    ))
}

fn is_unmasked_nonfinite_ppml_domain_row(
    input: &PpmlNumericPreparationInput<'_>,
    original_row: usize,
    reason: DropReason,
) -> bool {
    if reason == DropReason::MissingOutcome {
        return !input.outcome.missing.is_some_and(|mask| mask[original_row])
            && !input.outcome.values[original_row].is_finite();
    }
    if reason != DropReason::MissingOffsetOrExposure {
        return false;
    }
    let column = match (input.offset, input.exposure) {
        (Some(column), None) | (None, Some(column)) => column,
        (None, None) | (Some(_), Some(_)) => return false,
    };
    !column.missing.is_some_and(|mask| mask[original_row])
        && !column.values[original_row].is_finite()
}

fn validate_unmasked_nonfinite_weights(
    input: WeightInput<'_>,
    sample: &PreparedNumericSample,
) -> Result<()> {
    let WeightInput::Provided { column, .. } = input else {
        return Ok(());
    };
    for (original_row, reason) in sample.drop_reasons.iter().copied().enumerate() {
        let explicitly_missing = column.missing.is_some_and(|mask| mask[original_row]);
        if reason != crate::data::DropReason::InvalidOrMissingWeight
            || explicitly_missing
            || column.values[original_row].is_finite()
        {
            continue;
        }
        return validate_unmasked_nonfinite_weight_row(input, original_row);
    }
    Ok(())
}

fn validate_unmasked_nonfinite_weight_row(
    input: WeightInput<'_>,
    original_row: usize,
) -> Result<()> {
    let WeightInput::Provided { kind, .. } = input else {
        return Ok(());
    };
    match kind {
        WeightKind::Frequency => Err(FeregError::new(
            ErrorCode::InvalidFrequencyWeight,
            format!(
                "PPML frequency weight at observation {} must be finite",
                original_row + 1
            ),
        )),
        WeightKind::Probability => Err(FeregError::new(
            ErrorCode::InvalidData,
            format!(
                "PPML probability weight at observation {} must be finite",
                original_row + 1
            ),
        )),
        WeightKind::Analytic | WeightKind::Importance => Err(FeregError::new(
            ErrorCode::UnsupportedWeight,
            "PPML numeric preparation supports only fweights and pweights",
        )),
    }
}

struct PpmlNumericDomainPolicy;

impl NumericDomainPolicy for PpmlNumericDomainPolicy {
    fn outcome(&self, original_row: usize, value: f64) -> Result<DomainDecision> {
        if value >= 0.0 {
            Ok(DomainDecision::Retain)
        } else {
            Err(FeregError::new(
                ErrorCode::InvalidData,
                format!(
                    "PPML outcome at observation {} must be nonnegative",
                    original_row + 1
                ),
            ))
        }
    }

    fn weight(&self, original_row: usize, kind: WeightKind, value: f64) -> Result<DomainDecision> {
        match kind {
            WeightKind::Frequency
                if value > 0.0
                    && value.fract() == 0.0
                    && value <= f64::from(u32::MAX) =>
            {
                Ok(DomainDecision::Retain)
            }
            WeightKind::Frequency => Err(FeregError::new(
                ErrorCode::InvalidFrequencyWeight,
                format!(
                    "PPML frequency weight at observation {} must be a positive integer representable as u32",
                    original_row + 1
                ),
            )),
            WeightKind::Probability if value > 0.0 => Ok(DomainDecision::Retain),
            WeightKind::Probability => Err(FeregError::new(
                ErrorCode::InvalidData,
                format!(
                    "PPML probability weight at observation {} must be finite and positive",
                    original_row + 1
                ),
            )),
            WeightKind::Analytic | WeightKind::Importance => Err(FeregError::new(
                ErrorCode::UnsupportedWeight,
                "PPML numeric preparation supports only fweights and pweights",
            )),
        }
    }

    fn offset(&self, _original_row: usize, _value: f64) -> Result<DomainDecision> {
        // Numeric preparation invokes the policy only for finite available
        // values. The common seam classifies availability; PPML's wrapper then
        // distinguishes explicit missingness from an unmasked nonfinite value.
        Ok(DomainDecision::Retain)
    }

    fn exposure(&self, original_row: usize, value: f64) -> Result<DomainDecision> {
        if value > 0.0 {
            Ok(DomainDecision::Retain)
        } else {
            Err(FeregError::new(
                ErrorCode::InvalidData,
                format!(
                    "PPML exposure at observation {} must be finite and positive",
                    original_row + 1
                ),
            ))
        }
    }
}

fn canonicalize_weights(sample: &mut PreparedNumericSample) -> Result<PreparedPpmlWeightReport> {
    let physical = sample.n_active();
    let physical_f64 = physical as f64;
    match &mut sample.weights {
        PreparedWeights::None => Ok(PreparedPpmlWeightReport {
            kind: None,
            physical_observations: physical,
            effective_observations: physical,
            raw_weight_sum: None,
            log_raw_weight_sum: None,
            prepared_weight_sum: physical_f64,
            raw_to_prepared_multiplier: Some(1.0),
            log_raw_to_prepared_multiplier: 0.0,
        }),
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values,
        } => {
            let effective_observations = checked_frequency_effective_observations(values)?;
            let sum = effective_observations as f64;
            Ok(PreparedPpmlWeightReport {
                kind: Some(WeightKind::Frequency),
                physical_observations: physical,
                effective_observations,
                raw_weight_sum: Some(sum),
                log_raw_weight_sum: Some(sum.ln()),
                prepared_weight_sum: sum,
                raw_to_prepared_multiplier: Some(1.0),
                log_raw_to_prepared_multiplier: 0.0,
            })
        }
        PreparedWeights::Provided {
            kind: WeightKind::Probability,
            values,
        } => {
            let scale = canonicalize_probability_weights(values, physical_f64)?;
            Ok(PreparedPpmlWeightReport {
                kind: Some(WeightKind::Probability),
                physical_observations: physical,
                effective_observations: physical,
                raw_weight_sum: scale.raw_sum,
                log_raw_weight_sum: Some(scale.log_raw_sum),
                prepared_weight_sum: scale.prepared_sum,
                raw_to_prepared_multiplier: scale.raw_to_prepared_multiplier,
                log_raw_to_prepared_multiplier: scale.log_raw_to_prepared_multiplier,
            })
        }
        PreparedWeights::Provided {
            kind: WeightKind::Analytic | WeightKind::Importance,
            ..
        } => Err(FeregError::new(
            ErrorCode::UnsupportedWeight,
            "prepared PPML data contains an unsupported weight kind",
        )),
    }
}

/// Exact checked literal-replication count shared by PPML preparation,
/// generation rebasing, and inference metadata validation. Every failure is a
/// frequency-weight domain/range failure and therefore maps to Stata r(401).
pub(crate) fn checked_frequency_effective_observations(values: &[f64]) -> Result<usize> {
    checked_frequency_effective_observations_with_limit(values, usize::MAX as u128)
}

fn checked_frequency_effective_observations_with_limit(
    values: &[f64],
    supported_maximum: u128,
) -> Result<usize> {
    let mut effective = 0_u128;
    for value in values.iter().copied() {
        if !value.is_finite() || value <= 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidFrequencyWeight,
                "prepared PPML frequency weights are not positive u32 integers",
            ));
        }
        effective = effective
            .checked_add(u128::from(value as u32))
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::InvalidFrequencyWeight,
                    "PPML frequency-weight effective N overflowed",
                )
            })?;
        if effective > supported_maximum {
            return Err(FeregError::new(
                ErrorCode::InvalidFrequencyWeight,
                "PPML frequency-weight effective N exceeds the supported range",
            ));
        }
    }
    usize::try_from(effective).map_err(|_| {
        FeregError::new(
            ErrorCode::InvalidFrequencyWeight,
            "PPML frequency-weight effective N exceeds the supported range",
        )
    })
}

/// Normalize finite-positive pweights to `target_sum` without first forming a
/// potentially overflowing raw sum or multiplier.
///
/// Ratios that underflow while computing the log/raw scale are harmless for
/// the correctly rounded aggregate and are therefore allowed in that pass.
/// They become a hard numerical publication boundary only if the corresponding
/// normalized row weight itself cannot be represented as a strictly positive
/// `f64`.  The input slice is not mutated unless the entire normalized vector,
/// including its final sum correction, has passed those checks.
pub(crate) fn canonicalize_probability_weights(
    values: &mut [f64],
    target_sum: f64,
) -> Result<CanonicalProbabilityWeightScale> {
    if values.is_empty()
        || !target_sum.is_finite()
        || target_sum <= 0.0
        || values
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "prepared PPML probability weights have no finite positive scale",
        ));
    }
    let maximum = values.iter().copied().fold(0.0_f64, f64::max);
    let scaled_sum = compensated_nonnegative_sum(values.iter().map(|value| *value / maximum))?;
    let log_raw_sum = maximum.ln() + scaled_sum.ln();
    if !log_raw_sum.is_finite() {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML probability-weight log raw sum is not finite",
        ));
    }
    let scaled_multiplier = target_sum / scaled_sum;
    if !scaled_multiplier.is_finite() || scaled_multiplier <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML probability weights cannot be normalized on the finite computational scale",
        ));
    }
    let first_sum = compensated_scaled_probability_sum(values, scaled_multiplier, maximum)?;
    let correction = target_sum / first_sum;
    let final_multiplier = scaled_multiplier * correction;
    if !correction.is_finite()
        || correction <= 0.0
        || !final_multiplier.is_finite()
        || final_multiplier <= 0.0
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML probability-weight sum correction is outside the finite computational scale",
        ));
    }
    // Validate the complete corrected vector before mutating the source.  The
    // multiply-first/divide-first helper is essential here: for example,
    // `MIN_SUBNORMAL / 2 * 2` underflows prematurely even though the exact
    // normalized value is representable as `MIN_SUBNORMAL`.
    let prepared_sum = compensated_scaled_probability_sum(values, final_multiplier, maximum)?;
    if (prepared_sum - target_sum).abs() > 64.0 * f64::EPSILON * target_sum.max(1.0) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML probability weights cannot certify their normalized computational sum",
        ));
    }
    let raw_sum_value = maximum * scaled_sum;
    let raw_sum = (raw_sum_value.is_finite() && raw_sum_value > 0.0).then_some(raw_sum_value);
    let log_multiplier = target_sum.ln() - log_raw_sum;
    if !log_multiplier.is_finite() {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML probability-weight log normalization multiplier is not finite",
        ));
    }
    let multiplier_value = log_multiplier.exp();
    let multiplier =
        (multiplier_value.is_finite() && multiplier_value > 0.0).then_some(multiplier_value);

    // Every fallible calculation is complete. Re-evaluating the same pure
    // ratio can no longer fail, so mutation is an atomic publication step.
    for value in values.iter_mut() {
        *value = positive_scaled_ratio(*value, final_multiplier, maximum)
            .expect("validated PPML probability-weight ratio remains representable");
    }
    Ok(CanonicalProbabilityWeightScale {
        raw_sum,
        log_raw_sum,
        prepared_sum,
        raw_to_prepared_multiplier: multiplier,
        log_raw_to_prepared_multiplier: log_multiplier,
    })
}

fn compensated_scaled_probability_sum(
    values: &[f64],
    multiplier: f64,
    divisor: f64,
) -> Result<f64> {
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for &value in values {
        let normalized = positive_scaled_ratio(value, multiplier, divisor).ok_or_else(|| {
            FeregError::new(
                ErrorCode::NonConvergence,
                "PPML probability-weight normalization underflowed outside the finite computational scale",
            )
        })?;
        let next = sum + normalized;
        correction += if sum.abs() >= normalized.abs() {
            (sum - next) + normalized
        } else {
            (normalized - next) + sum
        };
        sum = next;
    }
    let total = sum + correction;
    if total.is_finite() && total > 0.0 {
        Ok(total)
    } else {
        Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML normalized probability-weight sum is outside the finite computational scale",
        ))
    }
}

fn positive_scaled_ratio(value: f64, multiplier: f64, divisor: f64) -> Option<f64> {
    let multiplied = value * multiplier;
    if multiplied.is_finite() && multiplied > 0.0 {
        let result = multiplied / divisor;
        if result.is_finite() && result > 0.0 {
            return Some(result);
        }
    }
    let divided = value / divisor;
    if divided.is_finite() && divided > 0.0 {
        let result = divided * multiplier;
        if result.is_finite() && result > 0.0 {
            return Some(result);
        }
    }
    None
}

fn compensated_nonnegative_sum(values: impl IntoIterator<Item = f64>) -> Result<f64> {
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for value in values {
        if !value.is_finite() || value < 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "PPML scaled probability-weight sum received an invalid value",
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
    if total.is_finite() && total > 0.0 {
        Ok(total)
    } else {
        Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML scaled probability-weight sum is not finite and positive",
        ))
    }
}

fn canonicalize_adjustment(
    sample: &mut PreparedNumericSample,
    input_kind: PreparedPpmlAdjustmentKind,
) -> Result<PreparedPpmlAdjustmentReport> {
    let adjustment = std::mem::replace(&mut sample.adjustment, PreparedAdjustment::None);
    let (canonical, prepared_as_offset, exposure_log_transformed) = match (input_kind, adjustment) {
        (PreparedPpmlAdjustmentKind::None, PreparedAdjustment::None) => {
            (PreparedAdjustment::None, false, false)
        }
        (PreparedPpmlAdjustmentKind::Offset, PreparedAdjustment::Offset(values)) => {
            (PreparedAdjustment::Offset(values), true, false)
        }
        (PreparedPpmlAdjustmentKind::Exposure, PreparedAdjustment::Exposure(mut values)) => {
            for value in &mut values {
                *value = value.ln();
                if !value.is_finite() {
                    return Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "PPML logged exposure is not finite",
                    ));
                }
            }
            (PreparedAdjustment::Offset(values), true, true)
        }
        _ => {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML adjustment kind and prepared sample disagree",
            ));
        }
    };
    sample.adjustment = canonical;
    Ok(PreparedPpmlAdjustmentReport {
        input_kind,
        prepared_as_offset,
        exposure_log_transformed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DropReason;

    fn unweighted_input<'a>(
        candidate: &'a [bool],
        outcome: &'a [f64],
    ) -> PpmlNumericPreparationInput<'a> {
        PpmlNumericPreparationInput {
            candidate,
            outcome: NumericColumnView::new(outcome),
            numeric_sources: &[],
            source_uses: &[],
            weights: WeightInput::None,
            offset: None,
            exposure: None,
        }
    }

    fn assert_close(left: f64, right: f64) {
        assert!(
            (left - right).abs() <= 64.0 * f64::EPSILON * left.abs().max(right.abs()).max(1.0),
            "{left:.17e} != {right:.17e}"
        );
    }

    #[test]
    fn allocation_free_classification_matches_materialized_shape_and_error_precedence() {
        let candidate = [true, true, false, true, true];
        let outcome = [1.0, 9.0, -7.0, 0.0, 3.0];
        let outcome_missing = [false, true, false, false, false];
        let regressor = [2.0, 3.0, f64::NAN, 4.0, 5.0];
        let regressor_missing = [false, false, false, true, false];
        let sources = [NumericColumnView::with_missing(
            &regressor,
            &regressor_missing,
        )];
        let uses = [NumericSourceUse::ordinary()];
        let input = PpmlNumericPreparationInput {
            candidate: &candidate,
            outcome: NumericColumnView::with_missing(&outcome, &outcome_missing),
            numeric_sources: &sources,
            source_uses: &uses,
            weights: WeightInput::None,
            offset: None,
            exposure: None,
        };
        let classification =
            classify_ppml_numeric_data(&input).expect("allocation-free classification");
        let prepared = prepare_ppml_numeric_data(&input).expect("materialized sample");
        assert_eq!(classification.n_original, prepared.sample().n_original);
        assert_eq!(classification.n_candidate, prepared.sample().n_candidate);
        assert_eq!(classification.n_active, prepared.sample().n_active());
        assert_eq!(prepared.sample().retained_rows(), &[0, 4]);

        // Historical preparation validates unmasked outcome/adjustment
        // domains before deferred unmasked nonfinite-weight diagnostics. The
        // early classifier must retain that exact failure ordering.
        let candidate = [true, true];
        let outcome = [1.0, 1.0];
        let weight = [f64::NAN, 1.0];
        let offset = [0.0, f64::NAN];
        let invalid = PpmlNumericPreparationInput {
            candidate: &candidate,
            outcome: NumericColumnView::new(&outcome),
            numeric_sources: &[],
            source_uses: &[],
            weights: WeightInput::Provided {
                kind: WeightKind::Probability,
                column: NumericColumnView::new(&weight),
            },
            offset: Some(NumericColumnView::new(&offset)),
            exposure: None,
        };
        let classified_error = classify_ppml_numeric_data(&invalid)
            .expect_err("classification must reject the nonfinite offset first");
        let prepared_error = prepare_ppml_numeric_data(&invalid)
            .expect_err("materialization must reject the same nonfinite offset first");
        assert_eq!(classified_error.code, prepared_error.code);
        assert_eq!(classified_error.message, prepared_error.message);
        assert!(classified_error.message.contains("offset at observation 2"));

        // Policy failures raised during the authoritative row scan precede
        // the post-scan diagnostics for an earlier unmasked nonfinite value.
        // The count pass must defer the latter just as materialization does.
        let outcome = [f64::NAN, -1.0];
        let invalid = unweighted_input(&candidate, &outcome);
        let classified_error = classify_ppml_numeric_data(&invalid)
            .expect_err("classification must reject the later negative outcome first");
        let prepared_error = prepare_ppml_numeric_data(&invalid)
            .expect_err("materialization must reject the same negative outcome first");
        assert_eq!(classified_error.code, prepared_error.code);
        assert_eq!(classified_error.message, prepared_error.message);
        assert!(classified_error.message.contains("observation 2"));
    }

    #[test]
    fn unweighted_fractional_outcomes_are_retained_with_physical_effective_n() {
        let candidate = [true, true, true];
        let outcome = [0.0, 0.25, 2.5];
        let prepared = prepare_ppml_numeric_data(&unweighted_input(&candidate, &outcome))
            .expect("fractional PPML sample");
        assert_eq!(prepared.sample().outcome, outcome);
        assert_eq!(prepared.sample().retained_rows(), &[0, 1, 2]);
        assert_eq!(
            prepared.weights(),
            PreparedPpmlWeightReport {
                kind: None,
                physical_observations: 3,
                effective_observations: 3,
                raw_weight_sum: None,
                log_raw_weight_sum: None,
                prepared_weight_sum: 3.0,
                raw_to_prepared_multiplier: Some(1.0),
                log_raw_to_prepared_multiplier: 0.0,
            }
        );
        assert_eq!(
            prepared.adjustment(),
            PreparedPpmlAdjustmentReport {
                input_kind: PreparedPpmlAdjustmentKind::None,
                prepared_as_offset: false,
                exposure_log_transformed: false,
            }
        );
    }

    #[test]
    fn negative_outcome_is_invalid_but_off_candidate_value_is_not_inspected() {
        let candidate = [true, true];
        let negative = [1.0, -0.5];
        assert_eq!(
            prepare_ppml_numeric_data(&unweighted_input(&candidate, &negative))
                .expect_err("negative outcome")
                .code,
            ErrorCode::InvalidData
        );

        let restricted = [false, true];
        let prepared = prepare_ppml_numeric_data(&unweighted_input(&restricted, &negative))
            .expect_err("remaining outcome is negative");
        assert_eq!(prepared.code, ErrorCode::InvalidData);

        let off_candidate_negative = [-1.0, 0.5];
        let prepared =
            prepare_ppml_numeric_data(&unweighted_input(&restricted, &off_candidate_negative))
                .expect("off-candidate negative outcome");
        assert_eq!(prepared.sample().retained_rows(), &[1]);
    }

    #[test]
    fn unmasked_nonfinite_outcome_is_invalid_but_masked_or_off_candidate_is_missing() {
        let candidate = [true, true];
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let outcome = [invalid, 1.0];
            let error = prepare_ppml_numeric_data(&unweighted_input(&candidate, &outcome))
                .expect_err("unmasked nonfinite outcome");
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert_eq!(error.return_code(), 459);

            let missing = [true, false];
            let mut input = unweighted_input(&candidate, &outcome);
            input.outcome = NumericColumnView::with_missing(&outcome, &missing);
            let prepared = prepare_ppml_numeric_data(&input).expect("masked outcome is missing");
            assert_eq!(prepared.sample().retained_rows(), &[1]);
            assert_eq!(
                prepared.sample().drop_reasons,
                [DropReason::MissingOutcome, DropReason::Retained]
            );

            let restricted = [false, true];
            let prepared = prepare_ppml_numeric_data(&unweighted_input(&restricted, &outcome))
                .expect("off-candidate nonfinite outcome is ignored");
            assert_eq!(prepared.sample().retained_rows(), &[1]);
        }
    }

    #[test]
    fn empty_and_zero_positive_weight_samples_are_insufficient() {
        let all_zero = [0.0, 0.0];
        let all_candidate = [true, true];
        assert_eq!(
            prepare_ppml_numeric_data(&unweighted_input(&all_candidate, &all_zero))
                .expect_err("all-zero outcome")
                .code,
            ErrorCode::InsufficientObservations
        );
        let no_candidate = [false, false];
        let positive = [1.0, 2.0];
        assert_eq!(
            prepare_ppml_numeric_data(&unweighted_input(&no_candidate, &positive))
                .expect_err("empty final sample")
                .code,
            ErrorCode::InsufficientObservations
        );

        let tiny_positive = [0.0, f64::from_bits(1)];
        prepare_ppml_numeric_data(&unweighted_input(&all_candidate, &tiny_positive))
            .expect("positive subnormal outcome must not underflow the existence check");

        for (kind, values) in [
            (WeightKind::Frequency, [2.0, 3.0]),
            (WeightKind::Probability, [0.25, 9.0]),
        ] {
            let mut input = unweighted_input(&all_candidate, &all_zero);
            input.weights = WeightInput::Provided {
                kind,
                column: NumericColumnView::new(&values),
            };
            assert_eq!(
                prepare_ppml_numeric_data(&input)
                    .expect_err("weighted all-zero outcome")
                    .code,
                ErrorCode::InsufficientObservations
            );
        }
    }

    #[test]
    fn frequency_weights_preserve_multiplicity_and_report_effective_n() {
        let candidate = [true, true, true];
        let outcome = [0.0, 0.5, 2.0];
        let frequency = [2.0, 3.0, 5.0];
        let mut input = unweighted_input(&candidate, &outcome);
        input.weights = WeightInput::Provided {
            kind: WeightKind::Frequency,
            column: NumericColumnView::new(&frequency),
        };
        let prepared = prepare_ppml_numeric_data(&input).expect("frequency weights");
        assert_eq!(
            prepared.weights(),
            PreparedPpmlWeightReport {
                kind: Some(WeightKind::Frequency),
                physical_observations: 3,
                effective_observations: 10,
                raw_weight_sum: Some(10.0),
                log_raw_weight_sum: Some(10.0_f64.ln()),
                prepared_weight_sum: 10.0,
                raw_to_prepared_multiplier: Some(1.0),
                log_raw_to_prepared_multiplier: 0.0,
            }
        );
        let PreparedWeights::Provided { kind, values } = &prepared.sample().weights else {
            panic!("frequency weights must remain materialized");
        };
        assert_eq!(*kind, WeightKind::Frequency);
        assert_eq!(values, &frequency);
    }

    #[test]
    fn every_invalid_fweight_form_fails_with_frequency_code() {
        let candidate = [true];
        let outcome = [1.0];
        for invalid_weight in [0.0, -1.0, 1.5, f64::from(u32::MAX) + 1.0] {
            let weights = [invalid_weight];
            let mut input = unweighted_input(&candidate, &outcome);
            input.weights = WeightInput::Provided {
                kind: WeightKind::Frequency,
                column: NumericColumnView::new(&weights),
            };
            assert_eq!(
                prepare_ppml_numeric_data(&input)
                    .expect_err("invalid fweight")
                    .code,
                ErrorCode::InvalidFrequencyWeight
            );
        }

        for invalid_weight in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let weights = [invalid_weight];
            let mut input = unweighted_input(&candidate, &outcome);
            input.weights = WeightInput::Provided {
                kind: WeightKind::Frequency,
                column: NumericColumnView::new(&weights),
            };
            assert_eq!(
                prepare_ppml_numeric_data(&input)
                    .expect_err("unmasked nonfinite fweight")
                    .code,
                ErrorCode::InvalidFrequencyWeight
            );
        }
    }

    #[test]
    fn frequency_effective_n_supported_range_failure_is_r401() {
        let error = checked_frequency_effective_observations_with_limit(&[2.0, 3.0], 4)
            .expect_err("checked literal multiplicity must respect the supported maximum");
        assert_eq!(error.code, ErrorCode::InvalidFrequencyWeight);
        assert_eq!(error.return_code(), 401);
        assert!(error.message.contains("supported range"));
    }

    #[test]
    fn probability_weights_normalize_to_physical_n_and_keep_raw_sum() {
        let candidate = [true, true, true];
        let outcome = [0.0, 1.0, 2.0];
        let raw = [1.0, 2.0, 3.0];
        let mut input = unweighted_input(&candidate, &outcome);
        input.weights = WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::new(&raw),
        };
        let prepared = prepare_ppml_numeric_data(&input).expect("pweight sample");
        let report = prepared.weights();
        assert_eq!(report.kind, Some(WeightKind::Probability));
        assert_eq!(report.physical_observations, 3);
        assert_eq!(report.effective_observations, 3);
        assert_eq!(report.raw_weight_sum, Some(6.0));
        assert_close(
            report.log_raw_weight_sum.expect("log raw sum"),
            6.0_f64.ln(),
        );
        assert_close(report.prepared_weight_sum, 3.0);
        assert_close(report.raw_to_prepared_multiplier.expect("multiplier"), 0.5);
        assert_close(report.log_raw_to_prepared_multiplier, 0.5_f64.ln());
        let PreparedWeights::Provided { values, .. } = &prepared.sample().weights else {
            panic!("pweights must remain materialized");
        };
        for (actual, expected) in values.iter().zip([0.5, 1.0, 1.5]) {
            assert_close(*actual, expected);
        }
    }

    #[test]
    fn common_pweight_rescaling_leaves_canonical_weights_unchanged() {
        let candidate = [true, true, true, true];
        let outcome = [0.0, 0.25, 2.0, 4.0];
        let raw = [0.5, 2.0, 3.5, 9.0];
        let scaled = raw.map(|value| value * 17.0);
        let prepare = |weights: &[f64]| {
            let mut input = unweighted_input(&candidate, &outcome);
            input.weights = WeightInput::Provided {
                kind: WeightKind::Probability,
                column: NumericColumnView::new(weights),
            };
            prepare_ppml_numeric_data(&input).expect("scaled pweights")
        };
        let first = prepare(&raw);
        let second = prepare(&scaled);
        let PreparedWeights::Provided {
            values: first_values,
            ..
        } = &first.sample().weights
        else {
            panic!("first pweights");
        };
        let PreparedWeights::Provided {
            values: second_values,
            ..
        } = &second.sample().weights
        else {
            panic!("second pweights");
        };
        for (left, right) in first_values.iter().zip(second_values) {
            assert_close(*left, *right);
        }
        assert_close(
            second.weights().raw_weight_sum.expect("scaled raw sum"),
            17.0 * first.weights().raw_weight_sum.expect("raw sum"),
        );
        assert_close(
            second
                .weights()
                .log_raw_weight_sum
                .expect("scaled log raw sum"),
            first.weights().log_raw_weight_sum.expect("log raw sum") + 17.0_f64.ln(),
        );
        assert_close(
            second.weights().log_raw_to_prepared_multiplier,
            first.weights().log_raw_to_prepared_multiplier - 17.0_f64.ln(),
        );
    }

    #[test]
    fn extreme_finite_pweights_normalize_even_when_raw_sum_and_multiplier_overflow() {
        let candidate = [true, true];
        let outcome = [1.0, 2.0];

        let huge = [f64::MAX, f64::MAX];
        let mut huge_input = unweighted_input(&candidate, &outcome);
        huge_input.weights = WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::new(&huge),
        };
        let huge_prepared = prepare_ppml_numeric_data(&huge_input)
            .expect("finite pweight sum overflow must not narrow the domain");
        let huge_report = huge_prepared.weights();
        assert_eq!(huge_report.raw_weight_sum, None);
        assert!(huge_report.log_raw_weight_sum.expect("huge log sum") > f64::MAX.ln());
        assert!(huge_report.raw_to_prepared_multiplier.is_some());
        assert_eq!(huge_report.prepared_weight_sum, 2.0);

        let tiny = [f64::from_bits(1), f64::from_bits(1)];
        let mut tiny_input = unweighted_input(&candidate, &outcome);
        tiny_input.weights = WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::new(&tiny),
        };
        let tiny_prepared = prepare_ppml_numeric_data(&tiny_input)
            .expect("finite pweight multiplier overflow must not narrow the domain");
        let tiny_report = tiny_prepared.weights();
        assert!(tiny_report.raw_weight_sum.is_some());
        assert_eq!(tiny_report.raw_to_prepared_multiplier, None);
        assert!(tiny_report.log_raw_to_prepared_multiplier > f64::MAX.ln());
        assert_eq!(tiny_report.prepared_weight_sum, 2.0);
    }

    #[test]
    fn unrepresentable_mixed_pweight_scale_is_numerical_nonconvergence_not_invalid_data() {
        let candidate = [true, true];
        let outcome = [1.0, 2.0];
        let raw = [f64::MAX, f64::from_bits(1)];
        let original_bits = raw.map(f64::to_bits);
        let mut input = unweighted_input(&candidate, &outcome);
        input.weights = WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::new(&raw),
        };
        let error = prepare_ppml_numeric_data(&input)
            .expect_err("normalized q outside finite f64 range must fail atomically");
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert_eq!(error.return_code(), 430);
        assert_eq!(raw.map(f64::to_bits), original_bits);
        assert!(error.message.contains("computational scale"));
    }

    #[test]
    fn multiplication_precedes_division_when_physical_n_rescues_a_subnormal_q() {
        let candidate = [true, true];
        let outcome = [1.0, 2.0];
        let raw = [2.0, f64::from_bits(1)];
        let mut input = unweighted_input(&candidate, &outcome);
        input.weights = WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::new(&raw),
        };
        let prepared = prepare_ppml_numeric_data(&input)
            .expect("physical N makes the exact tiny normalized q representable");
        let PreparedWeights::Provided { values, .. } = &prepared.sample().weights else {
            panic!("materialized pweights");
        };
        assert_eq!(values[0], 2.0);
        assert_eq!(values[1], f64::from_bits(1));
        assert_eq!(prepared.weights().prepared_weight_sum, 2.0);
    }

    #[test]
    fn nonpositive_pweights_fail_and_masked_pweights_drop() {
        let candidate = [true, true];
        let outcome = [1.0, 2.0];
        for invalid in [0.0, -1.0] {
            let values = [1.0, invalid];
            let mut input = unweighted_input(&candidate, &outcome);
            input.weights = WeightInput::Provided {
                kind: WeightKind::Probability,
                column: NumericColumnView::new(&values),
            };
            assert_eq!(
                prepare_ppml_numeric_data(&input)
                    .expect_err("nonpositive pweight")
                    .code,
                ErrorCode::InvalidData
            );
        }

        let values = [1.0, 0.0];
        let missing = [false, true];
        let mut input = unweighted_input(&candidate, &outcome);
        input.weights = WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::with_missing(&values, &missing),
        };
        let prepared = prepare_ppml_numeric_data(&input).expect("missing pweight drops");
        assert_eq!(prepared.sample().retained_rows(), &[0]);
        assert_eq!(
            prepared.sample().drop_reasons,
            [DropReason::Retained, DropReason::InvalidOrMissingWeight]
        );

        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let values = [1.0, invalid];
            let mut input = unweighted_input(&candidate, &outcome);
            input.weights = WeightInput::Provided {
                kind: WeightKind::Probability,
                column: NumericColumnView::new(&values),
            };
            assert_eq!(
                prepare_ppml_numeric_data(&input)
                    .expect_err("unmasked nonfinite pweight")
                    .code,
                ErrorCode::InvalidData
            );
        }
    }

    #[test]
    fn analytic_and_importance_weights_are_rejected_at_the_input_contract() {
        let candidate = [false];
        let outcome = [-1.0];
        let weight = [1.0];
        for kind in [WeightKind::Analytic, WeightKind::Importance] {
            let mut input = unweighted_input(&candidate, &outcome);
            input.weights = WeightInput::Provided {
                kind,
                column: NumericColumnView::new(&weight),
            };
            assert_eq!(
                prepare_ppml_numeric_data(&input)
                    .expect_err("unsupported weight kind")
                    .code,
                ErrorCode::UnsupportedWeight
            );
        }
    }

    #[test]
    fn offset_is_preserved_and_missing_offset_drops_in_original_order() {
        let candidate = [true, true, true];
        let outcome = [1.0, 2.0, 3.0];
        let offset = [-2.0, 99.0, 0.5];
        let missing = [false, true, false];
        let mut input = unweighted_input(&candidate, &outcome);
        input.offset = Some(NumericColumnView::with_missing(&offset, &missing));
        let prepared = prepare_ppml_numeric_data(&input).expect("offset prepare");
        assert_eq!(prepared.offset(), Some(&[-2.0, 0.5][..]));
        assert_eq!(prepared.sample().retained_rows(), &[0, 2]);
        assert_eq!(
            prepared.sample().drop_reasons,
            [
                DropReason::Retained,
                DropReason::MissingOffsetOrExposure,
                DropReason::Retained,
            ]
        );
        assert_eq!(
            prepared.adjustment(),
            PreparedPpmlAdjustmentReport {
                input_kind: PreparedPpmlAdjustmentKind::Offset,
                prepared_as_offset: true,
                exposure_log_transformed: false,
            }
        );
    }

    #[test]
    fn exposure_is_positive_and_logged_exactly_once() {
        let candidate = [true, true, true];
        let outcome = [1.0, 2.0, 3.0];
        let exposure = [1.0, std::f64::consts::E, std::f64::consts::E.powi(2)];
        let mut input = unweighted_input(&candidate, &outcome);
        input.exposure = Some(NumericColumnView::new(&exposure));
        let prepared = prepare_ppml_numeric_data(&input).expect("exposure prepare");
        let logged = prepared.offset().expect("canonical logged exposure");
        for (actual, expected) in logged.iter().zip([0.0, 1.0, 2.0]) {
            assert_close(*actual, expected);
        }
        assert!(matches!(
            prepared.sample().adjustment,
            PreparedAdjustment::Offset(_)
        ));
        assert_eq!(
            prepared.adjustment(),
            PreparedPpmlAdjustmentReport {
                input_kind: PreparedPpmlAdjustmentKind::Exposure,
                prepared_as_offset: true,
                exposure_log_transformed: true,
            }
        );

        let invalid = [1.0, 0.0, 2.0];
        input.exposure = Some(NumericColumnView::new(&invalid));
        assert_eq!(
            prepare_ppml_numeric_data(&input)
                .expect_err("nonpositive exposure")
                .code,
            ErrorCode::InvalidData
        );
    }

    #[test]
    fn unmasked_nonfinite_adjustments_are_invalid_but_masked_values_still_drop() {
        let candidate = [true, true];
        let outcome = [1.0, 2.0];
        for use_exposure in [false, true] {
            for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let adjustment = [invalid, 1.0];
                let mut input = unweighted_input(&candidate, &outcome);
                if use_exposure {
                    input.exposure = Some(NumericColumnView::new(&adjustment));
                } else {
                    input.offset = Some(NumericColumnView::new(&adjustment));
                }
                let error =
                    prepare_ppml_numeric_data(&input).expect_err("unmasked nonfinite adjustment");
                assert_eq!(error.code, ErrorCode::InvalidData);
                assert_eq!(error.return_code(), 459);

                let missing = [true, false];
                if use_exposure {
                    input.exposure = Some(NumericColumnView::with_missing(&adjustment, &missing));
                } else {
                    input.offset = Some(NumericColumnView::with_missing(&adjustment, &missing));
                }
                let prepared =
                    prepare_ppml_numeric_data(&input).expect("masked adjustment is missing");
                assert_eq!(prepared.sample().retained_rows(), &[1]);
                assert_eq!(
                    prepared.sample().drop_reasons,
                    [DropReason::MissingOffsetOrExposure, DropReason::Retained]
                );
            }
        }
    }

    #[test]
    fn off_candidate_or_preceded_nonfinite_adjustment_is_not_inspected() {
        let candidate = [false, true];
        let outcome = [1.0, 2.0];
        let adjustment = [f64::INFINITY, 1.0];
        for use_exposure in [false, true] {
            let mut input = unweighted_input(&candidate, &outcome);
            if use_exposure {
                input.exposure = Some(NumericColumnView::new(&adjustment));
            } else {
                input.offset = Some(NumericColumnView::new(&adjustment));
            }
            let prepared = prepare_ppml_numeric_data(&input)
                .expect("off-candidate nonfinite adjustment is ignored");
            assert_eq!(prepared.sample().retained_rows(), &[1]);
        }
    }

    #[test]
    fn offset_and_exposure_are_mutually_exclusive_before_row_reads() {
        let candidate = [false];
        let outcome = [-1.0];
        let offset = [0.0];
        let exposure = [1.0];
        let mut input = unweighted_input(&candidate, &outcome);
        input.offset = Some(NumericColumnView::new(&offset));
        input.exposure = Some(NumericColumnView::new(&exposure));
        let error = prepare_ppml_numeric_data(&input).expect_err("mutually exclusive adjustment");
        assert_eq!(error.code, ErrorCode::Syntax);
        assert_eq!(error.return_code(), 198);
    }

    #[test]
    fn common_seam_preserves_complete_drop_precedence_and_row_ledger() {
        let n = 8;
        let candidate = [false, true, true, true, true, true, true, true];
        let outcome = [9.0, 9.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0];
        let outcome_missing = [false, true, false, false, false, false, false, false];
        let ordinary = [1.0; 8];
        let ordinary_missing = [false, false, true, false, false, false, false, false];
        let fixed_effect = [1.0; 8];
        let fixed_effect_missing = [false, false, false, false, true, false, false, false];
        let cluster = [1.0; 8];
        let cluster_missing = [false, false, false, false, false, true, false, false];
        let raw_weights = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let weight_missing = [false, false, false, true, false, false, false, false];
        let exposure = [1.0; 8];
        let exposure_missing = [false, false, false, false, false, false, true, false];
        let sources = [
            NumericColumnView::with_missing(&ordinary, &ordinary_missing),
            NumericColumnView::with_missing(&fixed_effect, &fixed_effect_missing),
            NumericColumnView::with_missing(&cluster, &cluster_missing),
        ];
        let uses = [
            NumericSourceUse {
                ordinary: true,
                fixed_effect: false,
                cluster: false,
            },
            NumericSourceUse {
                ordinary: false,
                fixed_effect: true,
                cluster: false,
            },
            NumericSourceUse {
                ordinary: false,
                fixed_effect: false,
                cluster: true,
            },
        ];
        let input = PpmlNumericPreparationInput {
            candidate: &candidate,
            outcome: NumericColumnView::with_missing(&outcome, &outcome_missing),
            numeric_sources: &sources,
            source_uses: &uses,
            weights: WeightInput::Provided {
                kind: WeightKind::Probability,
                column: NumericColumnView::with_missing(&raw_weights, &weight_missing),
            },
            offset: None,
            exposure: Some(NumericColumnView::with_missing(
                &exposure,
                &exposure_missing,
            )),
        };
        let prepared = prepare_ppml_numeric_data(&input).expect("precedence fixture");
        assert_eq!(prepared.sample().n_original, n);
        assert_eq!(prepared.sample().n_candidate, 7);
        assert_eq!(prepared.sample().retained_rows(), &[7]);
        assert_eq!(
            prepared.sample().drop_reasons,
            [
                DropReason::ExcludedByIfIn,
                DropReason::MissingOutcome,
                DropReason::MissingRegressor,
                DropReason::InvalidOrMissingWeight,
                DropReason::MissingFixedEffect,
                DropReason::MissingCluster,
                DropReason::MissingOffsetOrExposure,
                DropReason::Retained,
            ]
        );
        assert_eq!(prepared.weights().raw_weight_sum, Some(8.0));
        assert_eq!(prepared.sample().regressors.len(), 3);
        assert!(prepared
            .sample()
            .regressors
            .iter()
            .all(|column| column == &[1.0]));
    }

    #[test]
    fn earlier_missingness_prevents_later_domain_errors() {
        let candidate = [true, true];
        let outcome = [1.0, 2.0];
        let ordinary = [1.0, 2.0];
        let ordinary_missing = [true, false];
        let invalid_frequency = [0.0, 1.0];
        let sources = [NumericColumnView::with_missing(
            &ordinary,
            &ordinary_missing,
        )];
        let uses = [NumericSourceUse::ordinary()];
        let input = PpmlNumericPreparationInput {
            candidate: &candidate,
            outcome: NumericColumnView::new(&outcome),
            numeric_sources: &sources,
            source_uses: &uses,
            weights: WeightInput::Provided {
                kind: WeightKind::Frequency,
                column: NumericColumnView::new(&invalid_frequency),
            },
            offset: None,
            exposure: None,
        };
        let prepared = prepare_ppml_numeric_data(&input).expect("regressor precedes weight");
        assert_eq!(
            prepared.sample().drop_reasons,
            [DropReason::MissingRegressor, DropReason::Retained]
        );

        let cluster = [1.0, 2.0];
        let cluster_missing = [true, false];
        let exposure = [f64::INFINITY, 1.0];
        let sources = [NumericColumnView::with_missing(&cluster, &cluster_missing)];
        let uses = [NumericSourceUse {
            ordinary: false,
            fixed_effect: false,
            cluster: true,
        }];
        let input = PpmlNumericPreparationInput {
            candidate: &candidate,
            outcome: NumericColumnView::new(&outcome),
            numeric_sources: &sources,
            source_uses: &uses,
            weights: WeightInput::None,
            offset: None,
            exposure: Some(NumericColumnView::new(&exposure)),
        };
        let prepared = prepare_ppml_numeric_data(&input).expect("cluster precedes exposure");
        assert_eq!(
            prepared.sample().drop_reasons,
            [DropReason::MissingCluster, DropReason::Retained]
        );
    }

    #[test]
    fn malformed_source_use_table_fails_at_the_generic_boundary() {
        let candidate = [true];
        let outcome = [1.0];
        let source = [1.0];
        let sources = [NumericColumnView::new(&source)];
        let input = PpmlNumericPreparationInput {
            candidate: &candidate,
            outcome: NumericColumnView::new(&outcome),
            numeric_sources: &sources,
            source_uses: &[],
            weights: WeightInput::None,
            offset: None,
            exposure: None,
        };
        assert_eq!(
            prepare_ppml_numeric_data(&input)
                .expect_err("source-use mismatch")
                .code,
            ErrorCode::InvalidArgument
        );
    }
}
