//! Harvested from legacy/fereg-core at b1700e6d; no memory-mode admission.
use super::weights::WeightKind;
use crate::error::{ErrorCode, FeregError, Result};
use std::mem::size_of;

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
pub struct Prepared {
    pub topology: Option<crate::topology::FeTopology>,
    pub n_original: usize,
    pub n_candidate: usize,
    pub original_row_by_active: Vec<usize>,
    pub outcome: Vec<f64>,
    pub regressors: Vec<Vec<f64>>,
    pub weights: PreparedWeights,
    pub adjustment: PreparedAdjustment,
    pub drop_reasons: Vec<DropReason>,
}

pub(super) fn classify_row<P: NumericDomainPolicy>(
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

pub(super) fn validate_source_uses(
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

pub(super) fn validate_input(input: &NumericPreparationInput<'_>, n: usize) -> Result<()> {
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

pub(super) fn check_output_dimensions(
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

fn allocation_error(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}
