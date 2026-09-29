use std::mem::size_of;

use rayon::prelude::*;

use crate::data::weights::WeightKind;
use crate::error::{ErrorCode, FeregError, Result};

/// Authoritative observation-level exclusion reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DropReason {
    Retained = 0,
    ExcludedByIfIn = 1,
    MissingOutcome = 2,
    MissingRegressor = 3,
    InvalidOrMissingWeight = 4,
    MissingFixedEffect = 5,
    MissingCluster = 6,
    MissingOffsetOrExposure = 7,
    Singleton = 8,
    FixedEffectSeparation = 9,
    SimplexSeparation = 10,
    IteratedRectifierSeparation = 11,
    MuSeparation = 12,
    OtherModelExclusion = 13,
}

impl DropReason {
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// A borrowed numeric source plus an optional explicit Stata-missing mask.
///
/// A value is unavailable when its mask entry is true or its value is not
/// finite. A finite placeholder under a true mask is therefore still missing.
#[derive(Debug, Clone, Copy)]
pub struct NumericColumnView<'a> {
    pub values: &'a [f64],
    pub missing: Option<&'a [bool]>,
}

impl<'a> NumericColumnView<'a> {
    #[must_use]
    pub const fn new(values: &'a [f64]) -> Self {
        Self {
            values,
            missing: None,
        }
    }

    #[must_use]
    pub const fn with_missing(values: &'a [f64], missing: &'a [bool]) -> Self {
        Self {
            values,
            missing: Some(missing),
        }
    }

    fn is_unavailable(self, original_row: usize) -> bool {
        self.missing.is_some_and(|mask| mask[original_row])
            || !self.values[original_row].is_finite()
    }
}

/// Weight input without an invented unweighted pseudo-kind or pseudo-column.
#[derive(Debug, Clone, Copy)]
pub enum WeightInput<'a> {
    None,
    Provided {
        kind: WeightKind,
        column: NumericColumnView<'a>,
    },
}

/// Mutually exclusive offset/exposure input preserving its raw scale.
#[derive(Debug, Clone, Copy)]
pub enum AdjustmentInput<'a> {
    None,
    Offset(NumericColumnView<'a>),
    Exposure(NumericColumnView<'a>),
}

/// Borrowed numeric sources used to construct the authoritative numeric sample.
#[derive(Debug)]
pub struct NumericPreparationInput<'a> {
    pub candidate: &'a [bool],
    pub outcome: NumericColumnView<'a>,
    pub regressors: &'a [NumericColumnView<'a>],
    pub weights: WeightInput<'a>,
    pub adjustment: AdjustmentInput<'a>,
}

/// Availability roles for one unified numeric model source.
///
/// A source can serve both an ordinary regressor recipe and a fixed-effect
/// recipe while crossing the SPI and compaction boundaries only once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericSourceUse {
    pub ordinary: bool,
    pub fixed_effect: bool,
    /// Declared cluster source. Cluster availability is checked after fixed
    /// effects and before offset/exposure availability. The role does not
    /// participate in singleton pruning.
    pub cluster: bool,
}

impl NumericSourceUse {
    #[must_use]
    pub const fn ordinary() -> Self {
        Self {
            ordinary: true,
            fixed_effect: false,
            cluster: false,
        }
    }

    #[must_use]
    pub const fn fixed_effect() -> Self {
        Self {
            ordinary: false,
            fixed_effect: true,
            cluster: false,
        }
    }

    #[must_use]
    pub const fn ordinary_and_fixed_effect() -> Self {
        Self {
            ordinary: true,
            fixed_effect: true,
            cluster: false,
        }
    }
}

/// Result of a caller-supplied domain decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainDecision {
    Retain,
    Drop,
}

/// Scientific domain decisions required by numeric sample preparation.
///
/// There is intentionally no default implementation: callers must choose an
/// estimator/reference-specific policy explicitly. Methods borrow `self`
/// immutably so decisions cannot depend on mutable row history. Returning an
/// error is fatal.
pub trait NumericDomainPolicy {
    fn outcome(&self, original_row: usize, value: f64) -> Result<DomainDecision>;

    fn weight(&self, original_row: usize, kind: WeightKind, value: f64) -> Result<DomainDecision>;

    fn offset(&self, original_row: usize, value: f64) -> Result<DomainDecision>;

    fn exposure(&self, original_row: usize, value: f64) -> Result<DomainDecision>;
}

/// Compact weights, absent for an unweighted model.
#[derive(Debug, PartialEq)]
pub enum PreparedWeights {
    None,
    Provided { kind: WeightKind, values: Vec<f64> },
}

/// Compact raw offset or exposure values.
#[derive(Debug, PartialEq)]
pub enum PreparedAdjustment {
    None,
    Offset(Vec<f64>),
    Exposure(Vec<f64>),
}

/// Compact native numeric data after availability and policy validation.
#[derive(Debug, PartialEq)]
pub struct PreparedNumericSample {
    pub n_original: usize,
    pub n_candidate: usize,
    pub original_row_by_active: Vec<usize>,
    pub outcome: Vec<f64>,
    pub regressors: Vec<Vec<f64>>,
    pub weights: PreparedWeights,
    pub adjustment: PreparedAdjustment,
    pub drop_reasons: Vec<DropReason>,
}

impl PreparedNumericSample {
    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        bytes(&self.original_row_by_active)
            + bytes(&self.outcome)
            + bytes(&self.regressors)
            + self.regressors.iter().map(bytes).sum::<u128>()
            + match &self.weights {
                PreparedWeights::None => 0,
                PreparedWeights::Provided { values, .. } => bytes(values),
            }
            + match &self.adjustment {
                PreparedAdjustment::None => 0,
                PreparedAdjustment::Offset(values) | PreparedAdjustment::Exposure(values) => {
                    bytes(values)
                }
            }
            + bytes(&self.drop_reasons)
    }

    #[must_use]
    pub fn n_active(&self) -> usize {
        self.original_row_by_active.len()
    }

    #[must_use]
    pub fn is_retained(&self, original_row: usize) -> bool {
        self.drop_reason(original_row) == Some(DropReason::Retained)
    }

    #[must_use]
    pub fn drop_reason(&self, original_row: usize) -> Option<DropReason> {
        self.drop_reasons.get(original_row).copied()
    }

    /// Zero-based original row numbers, in strictly increasing order.
    #[must_use]
    pub fn retained_rows(&self) -> &[usize] {
        &self.original_row_by_active
    }

    /// Consume this sample and retain the selected rows on its compact axis.
    ///
    /// Every aligned observation-scale vector is rebuilt before a replacement
    /// sample is returned. Rows selected out of the compact axis receive
    /// `dropped_reason` on the original observation axis. The original and
    /// candidate counts are intentionally unchanged.
    pub(crate) fn compact_active_rows(
        self,
        keep: &[bool],
        dropped_reason: DropReason,
    ) -> Result<Self> {
        if dropped_reason == DropReason::Retained {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "active-row compaction requires a non-retained drop reason",
            ));
        }

        let n_active = self.original_row_by_active.len();
        if keep.len() != n_active {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                format!(
                    "active-row mask has length {}, expected {n_active}",
                    keep.len()
                ),
            ));
        }
        if self.outcome.len() != n_active
            || self
                .regressors
                .iter()
                .any(|column| column.len() != n_active)
            || matches!(
                &self.weights,
                PreparedWeights::Provided { values, .. } if values.len() != n_active
            )
            || matches!(
                &self.adjustment,
                PreparedAdjustment::Offset(values) | PreparedAdjustment::Exposure(values)
                    if values.len() != n_active
            )
            || self.drop_reasons.len() != self.n_original
            || self
                .original_row_by_active
                .iter()
                .enumerate()
                .any(|(index, original_row)| {
                    *original_row >= self.n_original
                        || self.drop_reasons[*original_row] != DropReason::Retained
                        || (index > 0 && self.original_row_by_active[index - 1] >= *original_row)
                })
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "prepared numeric sample is not aligned on one compact observation axis",
            ));
        }

        let n_retained = keep.iter().filter(|retain| **retain).count();
        if n_retained == n_active {
            return Ok(self);
        }

        let mut reserve = SystemReserve;
        let mut original_row_by_active = reserved_vec(
            &mut reserve,
            PreparationBuffer::OriginalRowByActive,
            n_retained,
            "compacted observation map",
        )?;
        let mut outcome = reserved_vec(
            &mut reserve,
            PreparationBuffer::Outcome,
            n_retained,
            "compacted outcome",
        )?;
        let mut regressors = reserved_vec(
            &mut reserve,
            PreparationBuffer::RegressorColumns,
            self.regressors.len(),
            "compacted regressor column table",
        )?;
        for index in 0..self.regressors.len() {
            regressors.push(reserved_vec(
                &mut reserve,
                PreparationBuffer::RegressorColumn(index),
                n_retained,
                "compacted regressor column",
            )?);
        }
        let mut weights = match &self.weights {
            PreparedWeights::None => PreparedWeights::None,
            PreparedWeights::Provided { kind, .. } => PreparedWeights::Provided {
                kind: *kind,
                values: reserved_vec(
                    &mut reserve,
                    PreparationBuffer::Weights,
                    n_retained,
                    "compacted weight column",
                )?,
            },
        };
        let mut adjustment = match &self.adjustment {
            PreparedAdjustment::None => PreparedAdjustment::None,
            PreparedAdjustment::Offset(_) => PreparedAdjustment::Offset(reserved_vec(
                &mut reserve,
                PreparationBuffer::Offset,
                n_retained,
                "compacted offset column",
            )?),
            PreparedAdjustment::Exposure(_) => PreparedAdjustment::Exposure(reserved_vec(
                &mut reserve,
                PreparationBuffer::Exposure,
                n_retained,
                "compacted exposure column",
            )?),
        };
        let mut drop_reasons = self.drop_reasons;

        for (active_row, retain) in keep.iter().copied().enumerate() {
            let original_row = self.original_row_by_active[active_row];
            if !retain {
                drop_reasons[original_row] = dropped_reason;
                continue;
            }
            original_row_by_active.push(original_row);
            outcome.push(self.outcome[active_row]);
            for (destination, source) in regressors.iter_mut().zip(&self.regressors) {
                destination.push(source[active_row]);
            }
            if let (
                PreparedWeights::Provided {
                    values: destination,
                    ..
                },
                PreparedWeights::Provided { values: source, .. },
            ) = (&mut weights, &self.weights)
            {
                destination.push(source[active_row]);
            }
            match (&mut adjustment, &self.adjustment) {
                (PreparedAdjustment::Offset(destination), PreparedAdjustment::Offset(source))
                | (
                    PreparedAdjustment::Exposure(destination),
                    PreparedAdjustment::Exposure(source),
                ) => destination.push(source[active_row]),
                _ => {}
            }
        }

        Ok(Self {
            n_original: self.n_original,
            n_candidate: self.n_candidate,
            original_row_by_active,
            outcome,
            regressors,
            weights,
            adjustment,
            drop_reasons,
        })
    }
}

/// Identifies every output reservation made by numeric preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparationBuffer {
    DropReasons,
    OriginalRowByActive,
    Outcome,
    RegressorColumns,
    RegressorColumn(usize),
    Weights,
    Offset,
    Exposure,
}

/// Injection seam for deterministic reservation-failure tests.
trait Reserve {
    fn allow_reservation(
        &mut self,
        buffer: PreparationBuffer,
        elements: usize,
        element_size: usize,
    ) -> bool;
}

#[derive(Debug, Default)]
struct SystemReserve;

impl Reserve for SystemReserve {
    fn allow_reservation(
        &mut self,
        _buffer: PreparationBuffer,
        _elements: usize,
        _element_size: usize,
    ) -> bool {
        true
    }
}

/// Construct the authoritative compact numeric sample in original row order.
///
/// This function performs availability classification and applies an explicit
/// scientific domain policy. It does not transform, normalize, or reinterpret
/// retained numeric values.
pub fn prepare_numeric_sample<P: NumericDomainPolicy>(
    input: &NumericPreparationInput<'_>,
    policy: &P,
) -> Result<PreparedNumericSample> {
    prepare_numeric_sample_with_plan(input, None, policy, &mut SystemReserve)
}

/// Prepare a unified source registry with role-specific missingness precedence.
///
/// The source-use table must correspond one-for-one with `input.regressors`.
/// Ordinary-source availability is checked before weights; fixed-effect-source
/// availability is checked after weights. A shared unavailable source therefore
/// receives `MissingRegressor`, while an FE-only unavailable source receives
/// `MissingFixedEffect`.
pub fn prepare_numeric_sample_with_source_uses<P: NumericDomainPolicy>(
    input: &NumericPreparationInput<'_>,
    source_uses: &[NumericSourceUse],
    policy: &P,
) -> Result<PreparedNumericSample> {
    prepare_numeric_sample_with_plan(input, Some(source_uses), policy, &mut SystemReserve)
}

/// Allocation-free shape classification for an estimator that must decide
/// whether its complete fit can be admitted before compacting observation
/// data.
///
/// The visitor observes the same authoritative row decision and precedence as
/// [`prepare_numeric_sample_with_source_uses`].  It must remain
/// allocation-free: callers use this boundary inside an action-wide allocator
/// window whose purpose is to prove that a rejected memory route never
/// constructs the canonical numeric sample.
pub(crate) fn classify_numeric_sample_with_source_uses<
    P: NumericDomainPolicy,
    F: FnMut(usize, DropReason) -> Result<()>,
>(
    input: &NumericPreparationInput<'_>,
    source_uses: &[NumericSourceUse],
    policy: &P,
    mut visit: F,
) -> Result<NumericSampleClassification> {
    let n_original = input.candidate.len();
    validate_input(input, n_original)?;
    validate_source_uses(input, Some(source_uses))?;
    let n_candidate = input
        .candidate
        .iter()
        .filter(|candidate| **candidate)
        .count();
    // Retain the same checked output-shape boundary as materialization.  This
    // is scalar arithmetic only and therefore cannot create a large buffer.
    check_output_dimensions(input, n_original, n_candidate)?;

    let mut n_active = 0_usize;
    for original_row in 0..n_original {
        let reason = classify_row(input, Some(source_uses), policy, original_row)?;
        visit(original_row, reason)?;
        if reason == DropReason::Retained {
            n_active = n_active
                .checked_add(1)
                .ok_or_else(|| allocation_error("active observation count overflows"))?;
        }
    }
    Ok(NumericSampleClassification {
        n_original,
        n_candidate,
        n_active,
    })
}

/// Scalar-only result of the allocation-free numeric classification pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NumericSampleClassification {
    pub(crate) n_original: usize,
    pub(crate) n_candidate: usize,
    pub(crate) n_active: usize,
}

/// Fill a preallocated mask using the authoritative row classifier. Callers
/// must supply a stateless policy: independent chunks may run concurrently.
/// Errors are selected in observation order, including across chunk boundaries.
/// Unlike the allocation-free visitor, this boundary may schedule worker jobs;
/// it is not used by the PPML preflight allocation audit.
pub(crate) fn classify_numeric_sample_into_mask<P: NumericDomainPolicy + Sync>(
    input: &NumericPreparationInput<'_>,
    source_uses: &[NumericSourceUse],
    policy: &P,
    reasons: &mut [DropReason],
) -> Result<NumericSampleClassification> {
    let n_original = input.candidate.len();
    validate_input(input, n_original)?;
    validate_source_uses(input, Some(source_uses))?;
    let n_candidate = input
        .candidate
        .iter()
        .filter(|candidate| **candidate)
        .count();
    check_output_dimensions(input, n_original, n_candidate)?;
    if reasons.len() != n_original {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "classification mask must cover the original observation axis",
        ));
    }
    const CHUNK_ROWS: usize = 16_384;
    let classify = |(chunk, reasons): (usize, &mut [DropReason])| -> Result<()> {
        for (offset, reason) in reasons.iter_mut().enumerate() {
            *reason = classify_row(
                input,
                Some(source_uses),
                policy,
                chunk * CHUNK_ROWS + offset,
            )?;
        }
        Ok(())
    };
    if input.regressors.len() >= 8
        && n_original >= CHUNK_ROWS
        && rayon::current_thread_index().is_some()
        && rayon::current_num_threads() > 1
    {
        reasons
            .par_chunks_mut(CHUNK_ROWS)
            .enumerate()
            .map(classify)
            .find_first(Result::is_err)
            .unwrap_or(Ok(()))?;
    } else {
        reasons
            .chunks_mut(CHUNK_ROWS)
            .enumerate()
            .try_for_each(classify)?;
    }
    Ok(NumericSampleClassification {
        n_original,
        n_candidate,
        n_active: reasons
            .iter()
            .filter(|reason| **reason == DropReason::Retained)
            .count(),
    })
}

#[cfg(test)]
fn prepare_numeric_sample_with_reserve<P: NumericDomainPolicy, R: Reserve>(
    input: &NumericPreparationInput<'_>,
    policy: &P,
    reserve: &mut R,
) -> Result<PreparedNumericSample> {
    prepare_numeric_sample_with_plan(input, None, policy, reserve)
}

fn prepare_numeric_sample_with_plan<P: NumericDomainPolicy, R: Reserve>(
    input: &NumericPreparationInput<'_>,
    source_uses: Option<&[NumericSourceUse]>,
    policy: &P,
    reserve: &mut R,
) -> Result<PreparedNumericSample> {
    let n_original = input.candidate.len();
    validate_input(input, n_original)?;
    validate_source_uses(input, source_uses)?;
    let n_candidate = input
        .candidate
        .iter()
        .filter(|candidate| **candidate)
        .count();
    check_output_dimensions(input, n_original, n_candidate)?;

    let mut drop_reasons = reserved_vec(
        reserve,
        PreparationBuffer::DropReasons,
        n_original,
        "drop-reason mask",
    )?;
    let mut original_row_by_active = reserved_vec(
        reserve,
        PreparationBuffer::OriginalRowByActive,
        n_candidate,
        "observation map",
    )?;
    let mut outcome = reserved_vec(reserve, PreparationBuffer::Outcome, n_candidate, "outcome")?;

    let mut regressors = reserved_vec(
        reserve,
        PreparationBuffer::RegressorColumns,
        input.regressors.len(),
        "regressor column table",
    )?;
    for index in 0..input.regressors.len() {
        regressors.push(reserved_vec(
            reserve,
            PreparationBuffer::RegressorColumn(index),
            n_candidate,
            "regressor column",
        )?);
    }

    let mut weights = match input.weights {
        WeightInput::None => PreparedWeights::None,
        WeightInput::Provided { kind, .. } => PreparedWeights::Provided {
            kind,
            values: reserved_vec(
                reserve,
                PreparationBuffer::Weights,
                n_candidate,
                "weight column",
            )?,
        },
    };
    let mut adjustment = match input.adjustment {
        AdjustmentInput::None => PreparedAdjustment::None,
        AdjustmentInput::Offset(_) => PreparedAdjustment::Offset(reserved_vec(
            reserve,
            PreparationBuffer::Offset,
            n_candidate,
            "offset column",
        )?),
        AdjustmentInput::Exposure(_) => PreparedAdjustment::Exposure(reserved_vec(
            reserve,
            PreparationBuffer::Exposure,
            n_candidate,
            "exposure column",
        )?),
    };

    for original_row in 0..n_original {
        let reason = classify_row(input, source_uses, policy, original_row)?;
        drop_reasons.push(reason);
        if reason != DropReason::Retained {
            continue;
        }

        original_row_by_active.push(original_row);
        outcome.push(input.outcome.values[original_row]);
        for (destination, source) in regressors.iter_mut().zip(input.regressors) {
            destination.push(source.values[original_row]);
        }
        if let (PreparedWeights::Provided { values, .. }, WeightInput::Provided { column, .. }) =
            (&mut weights, input.weights)
        {
            values.push(column.values[original_row]);
        }
        match (&mut adjustment, input.adjustment) {
            (PreparedAdjustment::Offset(values), AdjustmentInput::Offset(column))
            | (PreparedAdjustment::Exposure(values), AdjustmentInput::Exposure(column)) => {
                values.push(column.values[original_row]);
            }
            _ => {}
        }
    }

    debug_assert_eq!(drop_reasons.len(), n_original);
    debug_assert!(original_row_by_active
        .windows(2)
        .all(|rows| rows[0] < rows[1]));
    debug_assert_eq!(outcome.len(), original_row_by_active.len());
    debug_assert!(regressors
        .iter()
        .all(|column| column.len() == original_row_by_active.len()));

    Ok(PreparedNumericSample {
        n_original,
        n_candidate,
        original_row_by_active,
        outcome,
        regressors,
        weights,
        adjustment,
        drop_reasons,
    })
}

fn classify_row<P: NumericDomainPolicy>(
    input: &NumericPreparationInput<'_>,
    source_uses: Option<&[NumericSourceUse]>,
    policy: &P,
    original_row: usize,
) -> Result<DropReason> {
    if !input.candidate[original_row] {
        return Ok(DropReason::ExcludedByIfIn);
    }

    if input.outcome.is_unavailable(original_row) {
        return Ok(DropReason::MissingOutcome);
    }
    if policy.outcome(original_row, input.outcome.values[original_row])? == DomainDecision::Drop {
        return Ok(DropReason::OtherModelExclusion);
    }

    if input.regressors.iter().enumerate().any(|(index, column)| {
        source_is_ordinary(source_uses, index) && column.is_unavailable(original_row)
    }) {
        return Ok(DropReason::MissingRegressor);
    }

    if let WeightInput::Provided { kind, column } = input.weights {
        if column.is_unavailable(original_row) {
            return Ok(DropReason::InvalidOrMissingWeight);
        }
        if policy.weight(original_row, kind, column.values[original_row])? == DomainDecision::Drop {
            return Ok(DropReason::InvalidOrMissingWeight);
        }
    }

    if input.regressors.iter().enumerate().any(|(index, column)| {
        source_is_fixed_effect(source_uses, index) && column.is_unavailable(original_row)
    }) {
        return Ok(DropReason::MissingFixedEffect);
    }

    if input.regressors.iter().enumerate().any(|(index, column)| {
        source_is_cluster(source_uses, index) && column.is_unavailable(original_row)
    }) {
        return Ok(DropReason::MissingCluster);
    }

    match input.adjustment {
        AdjustmentInput::None => {}
        AdjustmentInput::Offset(column) => {
            if column.is_unavailable(original_row) {
                return Ok(DropReason::MissingOffsetOrExposure);
            }
            if policy.offset(original_row, column.values[original_row])? == DomainDecision::Drop {
                return Ok(DropReason::MissingOffsetOrExposure);
            }
        }
        AdjustmentInput::Exposure(column) => {
            if column.is_unavailable(original_row) {
                return Ok(DropReason::MissingOffsetOrExposure);
            }
            if policy.exposure(original_row, column.values[original_row])? == DomainDecision::Drop {
                return Ok(DropReason::MissingOffsetOrExposure);
            }
        }
    }

    Ok(DropReason::Retained)
}

fn source_is_ordinary(source_uses: Option<&[NumericSourceUse]>, index: usize) -> bool {
    source_uses.is_none_or(|uses| uses[index].ordinary)
}

fn source_is_fixed_effect(source_uses: Option<&[NumericSourceUse]>, index: usize) -> bool {
    source_uses.is_some_and(|uses| uses[index].fixed_effect)
}

fn source_is_cluster(source_uses: Option<&[NumericSourceUse]>, index: usize) -> bool {
    source_uses.is_some_and(|uses| uses[index].cluster)
}

fn validate_source_uses(
    input: &NumericPreparationInput<'_>,
    source_uses: Option<&[NumericSourceUse]>,
) -> Result<()> {
    let Some(source_uses) = source_uses else {
        return Ok(());
    };
    if source_uses.len() != input.regressors.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            format!(
                "numeric source-use table has {} entries, expected {}",
                source_uses.len(),
                input.regressors.len()
            ),
        ));
    }
    if source_uses
        .iter()
        .any(|usage| !usage.ordinary && !usage.fixed_effect && !usage.cluster)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "numeric source-use entries must select an ordinary, fixed-effect, or cluster role",
        ));
    }
    Ok(())
}

fn validate_input(input: &NumericPreparationInput<'_>, n: usize) -> Result<()> {
    validate_column("outcome", input.outcome, n)?;
    for column in input.regressors {
        validate_column("regressor", *column, n)?;
    }
    if let WeightInput::Provided { column, .. } = input.weights {
        validate_column("weight", column, n)?;
    }
    match input.adjustment {
        AdjustmentInput::None => {}
        AdjustmentInput::Offset(column) => validate_column("offset", column, n)?,
        AdjustmentInput::Exposure(column) => validate_column("exposure", column, n)?,
    }
    Ok(())
}

fn validate_column(name: &str, column: NumericColumnView<'_>, expected: usize) -> Result<()> {
    if column.values.len() != expected {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            format!(
                "{name} has {} values, expected {expected}",
                column.values.len()
            ),
        ));
    }
    if let Some(mask) = column.missing {
        if mask.len() != expected {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                format!(
                    "{name} missing mask has length {}, expected {expected}",
                    mask.len()
                ),
            ));
        }
    }
    Ok(())
}

fn check_output_dimensions(
    input: &NumericPreparationInput<'_>,
    n_original: usize,
    n_candidate: usize,
) -> Result<()> {
    let mut bytes = checked_bytes::<DropReason>(n_original, "drop-reason mask")?;
    bytes = checked_add(
        bytes,
        checked_bytes::<usize>(n_candidate, "observation map")?,
        "numeric sample outputs",
    )?;
    bytes = checked_add(
        bytes,
        checked_bytes::<f64>(n_candidate, "outcome")?,
        "numeric sample outputs",
    )?;
    bytes = checked_add(
        bytes,
        checked_bytes::<Vec<f64>>(input.regressors.len(), "regressor column table")?,
        "numeric sample outputs",
    )?;
    let regressor_elements = n_candidate
        .checked_mul(input.regressors.len())
        .ok_or_else(|| allocation_error("regressor dimensions overflow"))?;
    bytes = checked_add(
        bytes,
        checked_bytes::<f64>(regressor_elements, "regressor values")?,
        "numeric sample outputs",
    )?;
    if matches!(input.weights, WeightInput::Provided { .. }) {
        bytes = checked_add(
            bytes,
            checked_bytes::<f64>(n_candidate, "weight values")?,
            "numeric sample outputs",
        )?;
    }
    if !matches!(input.adjustment, AdjustmentInput::None) {
        let _total_bytes = checked_add(
            bytes,
            checked_bytes::<f64>(n_candidate, "adjustment values")?,
            "numeric sample outputs",
        )?;
    }
    Ok(())
}

fn checked_bytes<T>(elements: usize, label: &str) -> Result<usize> {
    elements
        .checked_mul(size_of::<T>())
        .ok_or_else(|| allocation_error(format!("{label} size overflows")))
}

fn checked_add(left: usize, right: usize, label: &str) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| allocation_error(format!("{label} size overflows")))
}

fn reserved_vec<T, R: Reserve>(
    reserve: &mut R,
    buffer: PreparationBuffer,
    elements: usize,
    label: &str,
) -> Result<Vec<T>> {
    let _bytes = checked_bytes::<T>(elements, label)?;
    let mut values = Vec::new();
    if !reserve.allow_reservation(buffer, elements, size_of::<T>()) {
        return Err(allocation_error(format!(
            "injected reservation failure for {label}"
        )));
    }
    values
        .try_reserve_exact(elements)
        .map_err(|_| allocation_error(format!("unable to reserve {label}")))?;
    Ok(values)
}

fn allocation_error(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod allocation_tests {
    use super::*;

    struct Retain;

    impl NumericDomainPolicy for Retain {
        fn outcome(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
            Ok(DomainDecision::Retain)
        }

        fn weight(&self, _row: usize, _kind: WeightKind, _value: f64) -> Result<DomainDecision> {
            Ok(DomainDecision::Retain)
        }

        fn offset(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
            Ok(DomainDecision::Retain)
        }

        fn exposure(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
            Ok(DomainDecision::Retain)
        }
    }

    #[test]
    fn parallel_mask_matches_visitor_and_reports_earliest_row_error() {
        struct Reject;
        impl NumericDomainPolicy for Reject {
            fn outcome(&self, row: usize, value: f64) -> Result<DomainDecision> {
                if value < 0.0 {
                    Err(FeregError::new(
                        ErrorCode::InvalidData,
                        format!("outcome row {row}"),
                    ))
                } else {
                    Ok(DomainDecision::Retain)
                }
            }
            fn weight(&self, row: usize, kind: WeightKind, value: f64) -> Result<DomainDecision> {
                Retain.weight(row, kind, value)
            }
            fn offset(&self, row: usize, value: f64) -> Result<DomainDecision> {
                Retain.offset(row, value)
            }
            fn exposure(&self, row: usize, value: f64) -> Result<DomainDecision> {
                Retain.exposure(row, value)
            }
        }
        let n = 65_539;
        let candidate: Vec<_> = (0..n).map(|row| row % 997 != 0).collect();
        let mut outcome = vec![1.0; n];
        let mut x = vec![2.0; n];
        x[16_384] = f64::NAN;
        x[32_767] = f64::INFINITY;
        let columns = vec![NumericColumnView::new(&x); 16];
        let uses = vec![
            NumericSourceUse {
                ordinary: true,
                fixed_effect: true,
                cluster: false
            };
            16
        ];
        for errors in [false, true] {
            if errors {
                for row in [0, 16_383, 16_384, 32_768, 65_538] {
                    outcome[row] = -1.0;
                }
            }
            let input = NumericPreparationInput {
                candidate: &candidate,
                outcome: NumericColumnView::new(&outcome),
                regressors: &columns,
                weights: WeightInput::None,
                adjustment: AdjustmentInput::None,
            };
            let mut serial = vec![DropReason::Retained; n];
            let expected =
                classify_numeric_sample_with_source_uses(&input, &uses, &Reject, |row, reason| {
                    serial[row] = reason;
                    Ok(())
                });
            for threads in [1, 4, 16] {
                let team = crate::ThreadTeam::new(
                    crate::ThreadConfig::from_requested(threads, true).unwrap(),
                );
                let mut actual_mask = vec![DropReason::Retained; n];
                let actual = team
                    .install(|| {
                        classify_numeric_sample_into_mask(&input, &uses, &Reject, &mut actual_mask)
                    })
                    .unwrap();
                match (&expected, actual) {
                    (Ok(expected), Ok(actual)) => {
                        assert_eq!(*expected, actual);
                        assert_eq!(serial, actual_mask);
                    }
                    (Err(expected), Err(actual)) => {
                        assert_eq!(expected.code, actual.code);
                        assert_eq!(expected.message, actual.message);
                        assert_eq!(actual.message, "outcome row 16383");
                    }
                    other => panic!("classification changed: {other:?}"),
                }
            }
        }
    }

    struct CountingReserve {
        fail_at: Option<usize>,
        calls: Vec<(PreparationBuffer, usize, usize)>,
    }

    impl CountingReserve {
        fn succeeds() -> Self {
            Self {
                fail_at: None,
                calls: Vec::new(),
            }
        }

        fn fails_at(call: usize) -> Self {
            Self {
                fail_at: Some(call),
                calls: Vec::new(),
            }
        }
    }

    impl Reserve for CountingReserve {
        fn allow_reservation(
            &mut self,
            buffer: PreparationBuffer,
            elements: usize,
            element_size: usize,
        ) -> bool {
            let call = self.calls.len();
            self.calls.push((buffer, elements, element_size));
            self.fail_at != Some(call)
        }
    }

    #[test]
    fn every_output_reservation_fails_closed_as_stata_909() {
        let candidate = [true, false, true];
        let y = [1.0, 2.0, 3.0];
        let x1 = [4.0, 5.0, 6.0];
        let x2 = [7.0, 8.0, 9.0];
        let weights = [1.0, 1.0, 1.0];
        let offset = [0.0, 0.0, 0.0];
        let regressors = [NumericColumnView::new(&x1), NumericColumnView::new(&x2)];
        let input = NumericPreparationInput {
            candidate: &candidate,
            outcome: NumericColumnView::new(&y),
            regressors: &regressors,
            weights: WeightInput::Provided {
                kind: WeightKind::Analytic,
                column: NumericColumnView::new(&weights),
            },
            adjustment: AdjustmentInput::Offset(NumericColumnView::new(&offset)),
        };
        let expected = [
            (PreparationBuffer::DropReasons, 3, size_of::<DropReason>()),
            (
                PreparationBuffer::OriginalRowByActive,
                2,
                size_of::<usize>(),
            ),
            (PreparationBuffer::Outcome, 2, size_of::<f64>()),
            (
                PreparationBuffer::RegressorColumns,
                2,
                size_of::<Vec<f64>>(),
            ),
            (PreparationBuffer::RegressorColumn(0), 2, size_of::<f64>()),
            (PreparationBuffer::RegressorColumn(1), 2, size_of::<f64>()),
            (PreparationBuffer::Weights, 2, size_of::<f64>()),
            (PreparationBuffer::Offset, 2, size_of::<f64>()),
        ];

        for failure in 0..expected.len() {
            let mut reserve = CountingReserve::fails_at(failure);
            let error =
                prepare_numeric_sample_with_reserve(&input, &Retain, &mut reserve).unwrap_err();
            assert_eq!(error.code, ErrorCode::Allocation);
            assert_eq!(error.return_code(), 909);
            assert_eq!(reserve.calls, expected[..=failure]);
        }

        let mut reserve = CountingReserve::succeeds();
        let prepared = prepare_numeric_sample_with_reserve(&input, &Retain, &mut reserve).unwrap();
        assert_eq!(reserve.calls, expected);
        assert_eq!(prepared.n_active(), 2);
        assert!(prepared.original_row_by_active.capacity() >= prepared.n_candidate);
        assert!(prepared.outcome.capacity() >= prepared.n_candidate);
        assert!(prepared.regressors.capacity() >= regressors.len());
        assert!(prepared
            .regressors
            .iter()
            .all(|column| column.capacity() >= prepared.n_candidate));
    }

    #[test]
    fn absent_optional_columns_have_no_buffers_or_reservations() {
        let candidate = [true];
        let y = [1.0];
        let input = NumericPreparationInput {
            candidate: &candidate,
            outcome: NumericColumnView::new(&y),
            regressors: &[],
            weights: WeightInput::None,
            adjustment: AdjustmentInput::None,
        };
        let mut reserve = CountingReserve::succeeds();
        let prepared = prepare_numeric_sample_with_reserve(&input, &Retain, &mut reserve).unwrap();
        assert_eq!(
            reserve.calls,
            [
                (PreparationBuffer::DropReasons, 1, size_of::<DropReason>()),
                (
                    PreparationBuffer::OriginalRowByActive,
                    1,
                    size_of::<usize>()
                ),
                (PreparationBuffer::Outcome, 1, size_of::<f64>()),
                (
                    PreparationBuffer::RegressorColumns,
                    0,
                    size_of::<Vec<f64>>()
                ),
            ]
        );
        assert!(matches!(prepared.weights, PreparedWeights::None));
        assert!(matches!(prepared.adjustment, PreparedAdjustment::None));
    }

    #[test]
    fn zero_sized_supplied_columns_use_the_same_fallible_path() {
        let regressors = [NumericColumnView::new(&[])];
        let input = NumericPreparationInput {
            candidate: &[],
            outcome: NumericColumnView::new(&[]),
            regressors: &regressors,
            weights: WeightInput::Provided {
                kind: WeightKind::Frequency,
                column: NumericColumnView::new(&[]),
            },
            adjustment: AdjustmentInput::Exposure(NumericColumnView::new(&[])),
        };
        let expected_buffers = [
            PreparationBuffer::DropReasons,
            PreparationBuffer::OriginalRowByActive,
            PreparationBuffer::Outcome,
            PreparationBuffer::RegressorColumns,
            PreparationBuffer::RegressorColumn(0),
            PreparationBuffer::Weights,
            PreparationBuffer::Exposure,
        ];
        for (failure, expected_buffer) in expected_buffers.iter().enumerate() {
            let mut reserve = CountingReserve::fails_at(failure);
            let error =
                prepare_numeric_sample_with_reserve(&input, &Retain, &mut reserve).unwrap_err();
            assert_eq!(error.code, ErrorCode::Allocation);
            assert_eq!(reserve.calls.last().unwrap().0, *expected_buffer);
            assert!(reserve.calls.iter().all(|(_, elements, _)| *elements <= 1));
        }
    }

    #[test]
    fn active_row_compaction_preserves_every_aligned_column_and_reason_axis() {
        let candidate = [true, true, true];
        let y = [1.0, 2.0, 3.0];
        let x = [10.0, 20.0, 30.0];
        let weights = [1.5, 2.5, 3.5];
        let exposure = [4.0, 5.0, 6.0];
        let regressors = [NumericColumnView::new(&x)];
        let input = NumericPreparationInput {
            candidate: &candidate,
            outcome: NumericColumnView::new(&y),
            regressors: &regressors,
            weights: WeightInput::Provided {
                kind: WeightKind::Probability,
                column: NumericColumnView::new(&weights),
            },
            adjustment: AdjustmentInput::Exposure(NumericColumnView::new(&exposure)),
        };
        let prepared = prepare_numeric_sample(&input, &Retain).expect("prepare");
        let compacted = prepared
            .compact_active_rows(&[true, false, true], DropReason::Singleton)
            .expect("compact");
        assert_eq!(compacted.n_original, 3);
        assert_eq!(compacted.n_candidate, 3);
        assert_eq!(compacted.original_row_by_active, vec![0, 2]);
        assert_eq!(compacted.outcome, vec![1.0, 3.0]);
        assert_eq!(compacted.regressors, vec![vec![10.0, 30.0]]);
        assert_eq!(
            compacted.weights,
            PreparedWeights::Provided {
                kind: WeightKind::Probability,
                values: vec![1.5, 3.5],
            }
        );
        assert_eq!(
            compacted.adjustment,
            PreparedAdjustment::Exposure(vec![4.0, 6.0])
        );
        assert_eq!(
            compacted.drop_reasons,
            vec![
                DropReason::Retained,
                DropReason::Singleton,
                DropReason::Retained,
            ]
        );
    }
}
