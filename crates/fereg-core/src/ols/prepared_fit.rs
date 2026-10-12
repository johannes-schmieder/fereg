mod finite;
mod one_shot;

use crate::data::{DenseMatrix, PreparedWeights, WeightKind};
use crate::error::{allocation, invalid_argument, invalid_data, ErrorCode, FeregError, Result};
use crate::fe::{
    AbsorbConfig, BorrowedMapWeights, FeCertificate, FeCoefficientBatch, FeSolverKind, SolveReport,
    SolveStopReason,
};
use crate::linalg::neumaier_add;
use crate::memory::push_reserved;
use crate::metadata::CoefficientSlot;
use crate::ols::prepared_fit_view::PreparedOlsFitView;
use crate::options::ThreadConfig;
use crate::parallel::ThreadTeam;
use rayon::prelude::*;
use std::time::Instant;

use super::finite_wls_kernel::{
    certify_normal_equations,
    inverse_relative_infinity_norm as kernel_inverse_relative_infinity_norm,
    validate_moment_source, FixedBlockMomentSource, NormalEquationScale,
};
use super::linear_session::{
    LinearSessionAllocationPhase, LinearSessionPhaseAllocationGuard,
    LinearSessionPhaseAllocationObservation, LinearSessionPhaseAllocatorProbe,
};
use super::prepared_scratch::PreparedOlsScratch;
use super::{PreparedOlsCoefficientStatus, PreparedOlsColumnKind, PreparedOlsData};

/// Numerical policy for the guarded prepared-data coefficient checkpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsFitConfig {
    pub absorb: AbsorbConfig,
    /// The one-shot OLS publisher can recover saved effects from the certified
    /// final target when a direct one-FE or accepted Schur solve omits its
    /// initial product.
    /// This owner also permits same-weight graph reuse for the terminal OLS
    /// residual; shared weighted-linear callers keep their existing policy.
    /// Other weighted-linear callers retain their coefficient requirement.
    pub allow_final_fe_recovery: bool,
    /// Optional tighter final projection for pointwise residual-derived outputs.
    /// Independent of whether the execution mode retains FE coefficients.
    pub final_residual_tolerance: Option<f64>,
    /// Multiplier on `eps * max(N, K, 1)` after finite columns are scaled by
    /// their pre-absorption weighted norms.
    pub rank_tolerance_multiplier: f64,
    /// Squared relative weighted-norm cutoff for columns annihilated by fixed
    /// effects.  This matches pinned reghdfe's `cross(data,data) / norm2`
    /// collinearity statistic rather than applying its tolerance to the norm.
    pub fixed_effect_collinearity_tolerance: f64,
    /// Maximum recomputed normalized finite-score moment.
    pub score_tolerance: f64,
    /// Optional independent retained-solve coefficient accuracy, using the
    /// PPML publication envelope. No correction is made when already accurate.
    pub coefficient_tolerance: Option<f64>,
    /// Maximum normalized inverse, symmetry, and reported-intercept error.
    pub bread_tolerance: f64,
    /// Native materialization policy for the prepared finite-regressor path.
    pub execution: PreparedOlsExecutionConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsExecutionMode {
    Fast,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsExecutionConfig {
    pub mode: PreparedOlsExecutionMode,
    pub poolsize: usize,
    pub threads: ThreadConfig,
}

impl Default for PreparedOlsExecutionConfig {
    fn default() -> Self {
        Self {
            mode: PreparedOlsExecutionMode::Fast,
            poolsize: 10,
            threads: ThreadConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsExecutionReport {
    pub mode: PreparedOlsExecutionMode,
    pub poolsize: usize,
    pub scratch_bytes: u64,
    pub requested_threads: usize,
    pub effective_threads: usize,
    pub deterministic: bool,
}

impl PreparedOlsExecutionReport {
    fn from_config(config: PreparedOlsFitConfig, scratch_bytes: u64) -> Self {
        Self {
            mode: config.execution.mode,
            poolsize: config.execution.poolsize,
            scratch_bytes,
            requested_threads: config.execution.threads.requested.map_or(0, |v| v.get()),
            effective_threads: config.execution.threads.resolved(),
            deterministic: config.execution.threads.deterministic,
        }
    }
}

impl Default for PreparedOlsFitConfig {
    fn default() -> Self {
        Self {
            absorb: AbsorbConfig {
                map: crate::fe::MapConfig {
                    tolerance: 1e-10,
                    max_iterations: 20_000,
                    // Recurrence checks are cheap, but the independent FE
                    // certificate is a full pass over every absorbed effect.
                    // Check it periodically and always at solver exit rather
                    // than paying that cost after every iteration.
                    check_every: 4,
                    symmetric: true,
                },
                ..AbsorbConfig::default()
            },
            rank_tolerance_multiplier: 128.0,
            final_residual_tolerance: None,
            allow_final_fe_recovery: false,
            fixed_effect_collinearity_tolerance: 1e-9,
            score_tolerance: 5e-10,
            coefficient_tolerance: None,
            bread_tolerance: 5e-10,
            execution: PreparedOlsExecutionConfig::default(),
        }
    }
}

impl PreparedOlsFitConfig {
    fn final_absorb_config(self) -> AbsorbConfig {
        let mut config = self.absorb;
        if let Some(tolerance) = self.final_residual_tolerance {
            config.map.tolerance = config.map.tolerance.min(tolerance);
            config.lsmr.tolerance = config.lsmr.tolerance.min(tolerance);
        }
        config
    }
}

/// Internal scientific status of one canonical prepared coefficient slot.
///
/// An absorbed-model intercept is transiently deferred by the within problem,
/// then recovered from the reference normalization before a fit is returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsFitStatus {
    Kept,
    SyntaxOmitted,
    AbsorbedInterceptDeferred,
    /// Reference-normalized reported constant. This is a reported coordinate,
    /// not an additional finite-regressor rank contribution.
    NormalizedAbsorbedIntercept,
    CollinearWithFixedEffects,
    NumericallyRankDeficient,
}

/// One canonical coefficient slot after the certified finite solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsFittedCoefficient {
    pub coefficient_slot: CoefficientSlot,
    pub generated_column_index: Option<usize>,
    pub status: PreparedOlsFitStatus,
    /// Present only for an identified finite coefficient.
    pub value: Option<f64>,
}

/// Independently recomputed first-order-condition certificate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsScoreCertificate {
    pub relative_infinity_norm: f64,
    pub relative_two_norm: f64,
    pub moments: usize,
    pub passed: bool,
}

/// Rank decision certificate from the guarded scaled finite solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsRankCertificate {
    pub relative_threshold: f64,
    pub maximum_dropped_trailing_norm: f64,
    pub condition_proxy: f64,
    pub passed: bool,
}

/// Scientific role of one covariance-coordinate bread row and column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsBreadCoordinateKind {
    FiniteRegressor,
    NormalizedAbsorbedIntercept,
}

/// Canonical identity of one covariance-coordinate bread row and column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsBreadCoordinate {
    pub coefficient_slot: CoefficientSlot,
    pub generated_column_index: usize,
    pub kind: PreparedOlsBreadCoordinateKind,
}

/// Exact relationship between the numerically safe solver weights and the
/// reference command's cross-product weights.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsBreadWeightScale {
    pub weight_kind: Option<WeightKind>,
    /// Raw positive weights are divided by this value before MAP and QR.
    pub solver_weight_divisor: f64,
    /// Reference cross-product weights equal solver weights times this value.
    pub reference_weight_multiplier: f64,
    /// Sum of the reference cross-product weights: physical N for no/a/p
    /// weights and the frequency sum for fweights.
    pub reference_weight_sum: f64,
}

/// Recomputed numerical certificate for the covariance-coordinate bread and
/// normalized reported intercept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsBreadCertificate {
    /// Inverse check for the finite within-regressor block.
    pub left_inverse_relative_infinity_norm: f64,
    pub right_inverse_relative_infinity_norm: f64,
    /// Independent inverse check for the complete reported coordinate system,
    /// including a normalized absorbed-model intercept when present.
    pub reported_left_inverse_relative_infinity_norm: f64,
    pub reported_right_inverse_relative_infinity_norm: f64,
    pub symmetry_relative_infinity_norm: f64,
    pub minimum_diagonal: f64,
    pub intercept_formula_relative_error: f64,
    pub intercept_normalization_relative_error: f64,
    pub passed: bool,
}

/// Inverse cross-product on reported covariance coordinates, in canonical
/// generated-column order and on the pinned reference weight scale.
#[derive(Debug, PartialEq)]
pub struct PreparedOlsBread {
    coordinates: Vec<PreparedOlsBreadCoordinate>,
    values: Vec<f64>,
    weight_scale: PreparedOlsBreadWeightScale,
    certificate: PreparedOlsBreadCertificate,
}

impl PreparedOlsBread {
    #[must_use]
    pub fn coordinates(&self) -> &[PreparedOlsBreadCoordinate] {
        &self.coordinates
    }

    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    #[must_use]
    pub fn dimension(&self) -> usize {
        self.coordinates.len()
    }

    #[must_use]
    pub const fn weight_scale(&self) -> PreparedOlsBreadWeightScale {
        self.weight_scale
    }

    #[must_use]
    pub const fn certificate(&self) -> PreparedOlsBreadCertificate {
        self.certificate
    }
}

/// Certified result of the core-only prepared OLS/WLS coefficient checkpoint.
///
/// This is not a Stata result: it has no VCE, DoF, fit statistics, or recovered
/// fixed-effect coefficients. Its covariance-ready bread and reported
/// absorbed-model constant are nevertheless fully normalized and certified.
#[derive(Debug, PartialEq)]
pub struct CertifiedPreparedOlsFit {
    coefficients: Vec<PreparedOlsFittedCoefficient>,
    kept_generated_columns: Vec<usize>,
    dropped_generated_columns: Vec<usize>,
    finite_rank: usize,
    rank_certificate: PreparedOlsRankCertificate,
    score_certificate: PreparedOlsScoreCertificate,
    bread: PreparedOlsBread,
    absorption_report: SolveReport,
    residual_absorption_report: SolveReport,
    /// Reference-weight sum of squares of the outcome after the initial
    /// certified FE absorption and, when the absorbed span does not own the
    /// constant, after projecting on the residualized ordinary intercept.
    ///
    /// With at least one prepared intercept FE this is the reference command's
    /// within TSS.  A no-FE model may still need ordinary-intercept centering,
    /// so the prepared inference layer deliberately handles that case itself.
    initial_absorbed_outcome_sum_of_squares: f64,
    residuals: DenseMatrix,
    recovered_fe_coefficients: Option<FeCoefficientBatch>,
    fast_score_cache: Option<PreparedOlsFastScoreCache>,
    execution: PreparedOlsExecutionReport,
}

/// Certified projection onto the complete prepared finite-plus-FE model span.
///
/// This crate-private result is intentionally narrower than an OLS fit: it
/// freezes the same canonical finite rank decision and independently checks
/// the projected residual, but never constructs covariance bread or publishes
/// a reference-normalized absorbed intercept. PPML separation uses it when
/// only the model-space projection is scientifically relevant.
#[derive(Debug)]
pub(crate) struct CertifiedPreparedLinearProjection {
    coefficients: Vec<PreparedOlsFittedCoefficient>,
    kept_generated_columns: Vec<usize>,
    dropped_generated_columns: Vec<usize>,
    finite_rank: usize,
    rank_certificate: PreparedOlsRankCertificate,
    score_certificate: PreparedOlsScoreCertificate,
    absorption_report: SolveReport,
    residual_absorption_report: SolveReport,
    fitted: Vec<f64>,
    residuals: Vec<f64>,
    reconstruction_certificate: PreparedLinearProjectionReconstructionCertificate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PreparedLinearProjectionReconstructionCertificate {
    pub(crate) relative_infinity_norm: f64,
    pub(crate) passed: bool,
}

impl CertifiedPreparedLinearProjection {
    pub(crate) fn coefficients(&self) -> &[PreparedOlsFittedCoefficient] {
        &self.coefficients
    }

    pub(crate) fn kept_generated_columns(&self) -> &[usize] {
        &self.kept_generated_columns
    }

    pub(crate) fn dropped_generated_columns(&self) -> &[usize] {
        &self.dropped_generated_columns
    }

    pub(crate) const fn finite_rank(&self) -> usize {
        self.finite_rank
    }

    pub(crate) const fn rank_certificate(&self) -> PreparedOlsRankCertificate {
        self.rank_certificate
    }

    pub(crate) const fn score_certificate(&self) -> PreparedOlsScoreCertificate {
        self.score_certificate
    }

    pub(crate) const fn absorption_report(&self) -> &SolveReport {
        &self.absorption_report
    }

    pub(crate) const fn residual_absorption_report(&self) -> &SolveReport {
        &self.residual_absorption_report
    }

    pub(crate) fn fitted(&self) -> &[f64] {
        &self.fitted
    }

    pub(crate) fn residuals(&self) -> &[f64] {
        &self.residuals
    }

    pub(crate) const fn reconstruction_certificate(
        &self,
    ) -> PreparedLinearProjectionReconstructionCertificate {
        self.reconstruction_certificate
    }

    pub(crate) fn certified(&self) -> bool {
        let basis_is_coherent = self.finite_rank() == self.kept_generated_columns().len()
            && self.kept_generated_columns().iter().all(|generated| {
                self.coefficients().iter().any(|coefficient| {
                    coefficient.generated_column_index == Some(*generated)
                        && coefficient.status == PreparedOlsFitStatus::Kept
                        && coefficient.value.is_some_and(f64::is_finite)
                })
            })
            && self.dropped_generated_columns().iter().all(|generated| {
                !self.kept_generated_columns().contains(generated)
                    && self.coefficients().iter().any(|coefficient| {
                        coefficient.generated_column_index == Some(*generated)
                            && coefficient.status != PreparedOlsFitStatus::Kept
                    })
            });
        basis_is_coherent
            && self.rank_certificate().passed
            && self.score_certificate().passed
            && self.absorption_report().certified()
            && self.residual_absorption_report().certified()
            && self.reconstruction_certificate().passed
    }
}

#[derive(Debug, PartialEq)]
pub(super) enum PreparedOlsFastScoreCache {
    Packed {
        generated_columns: Vec<usize>,
        within: DenseMatrix,
        means: Vec<f64>,
    },
    Selected {
        generated_columns: Vec<usize>,
        within: DenseMatrix,
        means: Vec<f64>,
    },
}

impl PreparedOlsFastScoreCache {
    fn into_kept<R: FitReservation>(self, kept: &[usize], reservation: &mut R) -> Result<Self> {
        if self.matches(kept) {
            return Ok(self);
        }
        let Self::Packed {
            mut generated_columns,
            within,
            mut means,
        } = self
        else {
            return Err(invalid_data(
                "selected score product differs from accepted finite basis",
            ));
        };
        if within.ncols() != generated_columns.len()
            || means.len() != generated_columns.len()
            || generated_columns.windows(2).any(|pair| pair[0] >= pair[1])
            || kept.len() > generated_columns.len()
        {
            return Err(invalid_data(
                "packed score product has inconsistent identities or dimensions",
            ));
        }
        // One bounded row holds selected positions, original means and row
        // values. It makes compaction safe for any certified basis ordering.
        let mut selection = reserved_empty(
            reservation,
            kept.len(),
            "unable to reserve accepted packed score selection",
        )?;
        for &generated in kept {
            let position = generated_columns
                .binary_search(&generated)
                .map_err(|_| invalid_data("accepted score column is absent from packed product"))?;
            if selection
                .iter()
                .any(|entry: &(usize, f64, f64)| entry.0 == position)
            {
                return Err(invalid_data("accepted packed score column is duplicated"));
            }
            push_reserved(
                &mut selection,
                (position, means[position], 0.0),
                "accepted packed score selection capacity exhausted",
            )?;
        }
        let rows = within.nrows();
        let stride = within.ncols();
        let mut values = within.into_row_major();
        for row in 0..rows {
            for entry in &mut selection {
                entry.2 = values[row * stride + entry.0];
            }
            for (column, entry) in selection.iter().enumerate() {
                values[row * kept.len() + column] = entry.2;
            }
        }
        values.truncate(rows * kept.len());
        for (column, entry) in selection.iter().enumerate() {
            generated_columns[column] = kept[column];
            means[column] = entry.1;
        }
        generated_columns.truncate(kept.len());
        means.truncate(kept.len());
        Ok(Self::Packed {
            generated_columns,
            within: DenseMatrix::from_row_major(rows, kept.len(), values)?,
            means,
        })
    }

    fn mean(&self, generated: usize) -> Option<f64> {
        let (generated_columns, means) = match self {
            Self::Packed {
                generated_columns,
                means,
                ..
            }
            | Self::Selected {
                generated_columns,
                means,
                ..
            } => (generated_columns, means),
        };
        generated_columns
            .iter()
            .position(|candidate| *candidate == generated)
            .and_then(|position| means.get(position).copied())
    }

    fn matches(&self, generated_columns: &[usize]) -> bool {
        match self {
            Self::Packed {
                generated_columns: cached,
                within,
                means,
            } => {
                cached == generated_columns
                    && within.ncols() == generated_columns.len()
                    && means.len() == generated_columns.len()
            }
            Self::Selected {
                generated_columns: cached,
                within,
                means,
            } => {
                cached == generated_columns
                    && within.ncols() > generated_columns.len()
                    && means.len() == generated_columns.len()
            }
        }
    }
}

impl CertifiedPreparedOlsFit {
    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        let warnings = |report: &SolveReport| {
            bytes(&report.warnings)
                + report
                    .warnings
                    .iter()
                    .map(|warning| warning.capacity() as u128)
                    .sum::<u128>()
        };
        let cache = self
            .fast_score_cache
            .as_ref()
            .map_or(0, |cache| match cache {
                PreparedOlsFastScoreCache::Packed {
                    generated_columns,
                    within,
                    means,
                } => {
                    bytes(generated_columns) + within.storage_capacity() as u128 * 8 + bytes(means)
                }
                PreparedOlsFastScoreCache::Selected {
                    generated_columns,
                    within,
                    means,
                } => {
                    bytes(generated_columns) + within.storage_capacity() as u128 * 8 + bytes(means)
                }
            });
        bytes(&self.coefficients)
            + bytes(&self.kept_generated_columns)
            + bytes(&self.dropped_generated_columns)
            + bytes(&self.bread.coordinates)
            + bytes(&self.bread.values)
            + self.residuals.storage_capacity() as u128 * 8
            + self
                .recovered_fe_coefficients
                .as_ref()
                .map_or(0, FeCoefficientBatch::retained_bytes)
            + cache
            + warnings(&self.absorption_report)
            + warnings(&self.residual_absorption_report)
    }

    #[must_use]
    pub fn coefficients(&self) -> &[PreparedOlsFittedCoefficient] {
        &self.coefficients
    }

    #[must_use]
    pub fn kept_generated_columns(&self) -> &[usize] {
        &self.kept_generated_columns
    }

    #[must_use]
    pub fn dropped_generated_columns(&self) -> &[usize] {
        &self.dropped_generated_columns
    }

    #[must_use]
    pub const fn finite_rank(&self) -> usize {
        self.finite_rank
    }

    #[must_use]
    pub const fn rank_certificate(&self) -> PreparedOlsRankCertificate {
        self.rank_certificate
    }

    #[must_use]
    pub const fn score_certificate(&self) -> PreparedOlsScoreCertificate {
        self.score_certificate
    }

    #[must_use]
    pub const fn bread(&self) -> &PreparedOlsBread {
        &self.bread
    }

    #[must_use]
    pub const fn absorption_report(&self) -> &SolveReport {
        &self.absorption_report
    }

    #[must_use]
    pub const fn residual_absorption_report(&self) -> &SolveReport {
        &self.residual_absorption_report
    }

    #[must_use]
    pub const fn initial_absorbed_outcome_sum_of_squares(&self) -> f64 {
        self.initial_absorbed_outcome_sum_of_squares
    }

    /// Final FE-orthogonal regression residuals on the compact prepared axis.
    #[must_use]
    pub fn residuals(&self) -> &[f64] {
        self.residuals.as_slice()
    }

    /// Combined coefficient-space absorbed contribution, when the selected
    /// solver retained every required RHS and residual-polish correction.
    #[must_use]
    pub const fn recovered_fe_coefficients(&self) -> Option<&FeCoefficientBatch> {
        self.recovered_fe_coefficients.as_ref()
    }

    #[must_use]
    pub const fn execution(&self) -> PreparedOlsExecutionReport {
        self.execution
    }

    pub(super) fn take_inference_score_cache(
        &mut self,
    ) -> Result<Option<PreparedOlsFastScoreCache>> {
        self.fast_score_cache
            .take()
            .map(|cache| cache.into_kept(self.kept_generated_columns(), &mut SystemFitReservation))
            .transpose()
    }

    /// Maximum live workspace from the fit's certified FE operations.
    #[must_use]
    pub fn solver_workspace_peak(&self) -> crate::memory::SolverWorkspacePeak {
        self.absorption_report
            .solver_workspace_peak
            .max(self.residual_absorption_report.solver_workspace_peak)
    }

    pub(super) fn record_additional_scratch_peak(&mut self, bytes: u64) {
        self.execution.scratch_bytes = self.execution.scratch_bytes.max(bytes);
    }
}

pub(super) trait FitReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;
}

pub(super) struct SystemFitReservation;

// Above this length, a single naive reduction can consume a material share
// of the default bread-certificate error budget. Keep short checkpoint
// arithmetic consistent with the established retained-session path.
const PARALLEL_REDUCTION_CHUNK_ROWS: usize = 16_384;
const FUSED_MOMENT_MAX_COLUMNS: usize = 64;
const FUSED_MOMENT_MAX_BLOCKS: usize = 64;

struct PhaseTrace {
    enabled: bool,
    started: Instant,
    mark: Instant,
}

impl PhaseTrace {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            enabled: crate::diagnostics::phase_timing_enabled(),
            started: now,
            mark: now,
        }
    }

    fn lap(&mut self) -> f64 {
        if !self.enabled {
            return 0.0;
        }
        let elapsed = self.mark.elapsed().as_secs_f64();
        self.mark = Instant::now();
        elapsed
    }

    fn total(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }
}

impl FitReservation for SystemFitReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }
}

#[derive(Debug, Clone, Copy)]
struct NormalizedWeights<'a> {
    values: Option<&'a [f64]>,
    raw_scale: f64,
    kind: Option<WeightKind>,
}

impl<'a> NormalizedWeights<'a> {
    // FE norm handoffs are max-normalized, including explicit divisor frames.
    fn matches_fe_norm_scale(self) -> bool {
        self.values
            .is_none_or(|values| values.iter().copied().fold(0.0_f64, f64::max) == self.raw_scale)
    }

    fn root(self, row: usize) -> Result<f64> {
        let root = self.value(row)?.sqrt();
        if root.is_finite() && root > 0.0 {
            Ok(root)
        } else {
            Err(invalid_data(
                "prepared OLS normalized weight is not finite and positive",
            ))
        }
    }

    const fn borrowed(self) -> BorrowedMapWeights<'a> {
        match self.values {
            Some(values) => BorrowedMapWeights::Provided(values),
            None => BorrowedMapWeights::Unit,
        }
    }

    fn value(self, row: usize) -> Result<f64> {
        let value = self
            .values
            .map_or(1.0, |values| values[row] / self.raw_scale);
        if value.is_finite() && value > 0.0 {
            Ok(value)
        } else {
            Err(invalid_data(
                "prepared OLS normalized weight is not finite and positive",
            ))
        }
    }
}

/// Borrowed numerical inputs for one prepared weighted-linear solve.
///
/// The frame deliberately owns no observation-scale storage. Its response and
/// raw positive weights remain caller-owned, while the two scalar weight
/// scales make the solver and reference cross-product conventions explicit.
/// OLS constructs this frame from its prepared sample; an iterative estimator
/// can instead borrow a changing working response and weight vector without
/// rebuilding the prepared design.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LinearSolveFrame<'a> {
    response: &'a [f64],
    weights: NormalizedWeights<'a>,
    bread_weight_scale: PreparedOlsBreadWeightScale,
}

impl<'a> LinearSolveFrame<'a> {
    fn from_prepared_ols<P: PreparedOlsFitView + ?Sized>(prepared: &'a P) -> Result<Self> {
        let weights = normalized_weights(prepared)?;
        let bread_weight_scale = prepared_bread_weight_scale(prepared, weights)?;
        Ok(Self {
            response: &prepared.fit_sample().outcome,
            weights,
            bread_weight_scale,
        })
    }

    /// Construct a frame from caller-owned response and raw solver weights.
    /// The supplied scale states how raw weights are normalized for the solve
    /// and how normalized cross-products map back to the reference Hessian.
    pub(crate) fn from_parts(
        response: &'a [f64],
        weights: BorrowedMapWeights<'a>,
        bread_weight_scale: PreparedOlsBreadWeightScale,
    ) -> Result<Self> {
        if response.iter().any(|value| !value.is_finite()) {
            return Err(invalid_data(
                "prepared weighted-linear response must be finite",
            ));
        }
        if !bread_weight_scale.solver_weight_divisor.is_finite()
            || bread_weight_scale.solver_weight_divisor <= 0.0
            || !bread_weight_scale.reference_weight_multiplier.is_finite()
            || bread_weight_scale.reference_weight_multiplier <= 0.0
            || !bread_weight_scale.reference_weight_sum.is_finite()
            || bread_weight_scale.reference_weight_sum <= 0.0
        {
            return Err(invalid_data(
                "prepared weighted-linear weight scale must be finite and positive",
            ));
        }
        let values = match weights {
            BorrowedMapWeights::Unit => {
                if bread_weight_scale.solver_weight_divisor != 1.0 {
                    return Err(invalid_argument(
                        "unit weighted-linear solve requires a unit solver divisor",
                    ));
                }
                None
            }
            BorrowedMapWeights::Provided(values) => {
                if values.len() != response.len()
                    || values.iter().any(|weight| {
                        !weight.is_finite()
                            || *weight <= 0.0
                            || *weight / bread_weight_scale.solver_weight_divisor == 0.0
                    })
                {
                    return Err(invalid_data(
                        "prepared weighted-linear weights must align and normalize positively",
                    ));
                }
                Some(values)
            }
        };
        Ok(Self {
            response,
            weights: NormalizedWeights {
                values,
                raw_scale: bread_weight_scale.solver_weight_divisor,
                kind: bread_weight_scale.weight_kind,
            },
            bread_weight_scale,
        })
    }

    pub(crate) const fn response(self) -> &'a [f64] {
        self.response
    }

    const fn weights(self) -> NormalizedWeights<'a> {
        self.weights
    }

    pub(crate) const fn raw_weights(self) -> BorrowedMapWeights<'a> {
        self.weights.borrowed()
    }

    pub(crate) const fn bread_weight_scale(self) -> PreparedOlsBreadWeightScale {
        self.bread_weight_scale
    }

    pub(crate) fn validate_for<P: PreparedOlsFitView + ?Sized>(self, prepared: &P) -> Result<()> {
        if self.response.len() != prepared.fit_sample().n_active()
            || match self.raw_weights() {
                BorrowedMapWeights::Unit => false,
                BorrowedMapWeights::Provided(values) => values.len() != self.response.len(),
            }
        {
            return Err(invalid_argument(
                "prepared weighted-linear frame is not aligned on the active observation axis",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct FiniteScoreDraft {
    /// Largest finite-coordinate score after dividing out the within-X norm.
    /// The final response scale is applied only after the returned residual is
    /// independently certified against the FE span.
    pub(super) infinity_scale: f64,
    pub(super) square_sum_scale: f64,
    pub(super) moments: usize,
}

#[derive(Debug)]
pub(super) struct FiniteBreadDraft {
    pub(super) generated_columns: Vec<usize>,
    pub(super) values: Vec<f64>,
    pub(super) reference_crossproduct: Vec<f64>,
    pub(super) weight_scale: PreparedOlsBreadWeightScale,
    pub(super) left_inverse_relative_infinity_norm: f64,
    pub(super) right_inverse_relative_infinity_norm: f64,
}

#[derive(Debug)]
pub(super) struct FiniteFit {
    pub(super) generated_coefficients: Vec<Option<f64>>,
    pub(super) generated_statuses: Vec<PreparedOlsFitStatus>,
    pub(super) kept: Vec<usize>,
    pub(super) dropped: Vec<usize>,
    pub(super) rank: usize,
    pub(super) certificate: PreparedOlsRankCertificate,
    pub(super) finite_score: FiniteScoreDraft,
    pub(super) bread: Option<FiniteBreadDraft>,
    pub(super) within_residuals: Option<Vec<f64>>,
}

/// Published fit products belong to one owner after independent certificates.
struct FitPublication {
    score_certificate: PreparedOlsScoreCertificate,
    bread: PreparedOlsBread,
    absorption_report: SolveReport,
    residual_absorption_report: SolveReport,
    initial_absorbed_outcome_sum_of_squares: f64,
    residuals: DenseMatrix,
    recovered_fe_coefficients: Option<FeCoefficientBatch>,
    fast_score_cache: Option<PreparedOlsFastScoreCache>,
    execution: PreparedOlsExecutionReport,
}

impl FiniteFit {
    fn validate_axes(&self, columns: usize, rows: usize) -> Result<()> {
        if self.generated_coefficients.len() != columns
            || self.generated_statuses.len() != columns
            || self.rank != self.kept.len()
            || self
                .within_residuals
                .as_ref()
                .is_some_and(|values| values.len() != rows)
            || self.bread.as_ref().is_some_and(|bread| {
                bread.generated_columns != self.kept
                    || bread.values.len() != self.rank * self.rank
                    || bread.reference_crossproduct.len() != self.rank * self.rank
            })
        {
            return Err(invalid_argument("finite fit product axes are inconsistent"));
        }
        Ok(())
    }

    fn publish(
        mut self,
        coefficients: Vec<PreparedOlsFittedCoefficient>,
        products: FitPublication,
    ) -> CertifiedPreparedOlsFit {
        self.kept.sort_unstable();
        self.dropped.sort_unstable();
        CertifiedPreparedOlsFit {
            coefficients,
            kept_generated_columns: self.kept,
            dropped_generated_columns: self.dropped,
            finite_rank: self.rank,
            rank_certificate: self.certificate,
            score_certificate: products.score_certificate,
            bread: products.bread,
            absorption_report: products.absorption_report,
            residual_absorption_report: products.residual_absorption_report,
            initial_absorbed_outcome_sum_of_squares: products
                .initial_absorbed_outcome_sum_of_squares,
            residuals: products.residuals,
            recovered_fe_coefficients: products.recovered_fe_coefficients,
            fast_score_cache: products.fast_score_cache,
            execution: products.execution,
        }
    }
}

/// Borrowed only during one canonical solve, after its session has checked the
/// sample/topology/weight generation. These are raw, current-weight means.
#[derive(Clone, Copy)]
pub(super) struct CurrentFrameColumnMeans<'a> {
    pub(super) generated_columns: &'a [usize],
    pub(super) values: &'a [f64],
}

impl CurrentFrameColumnMeans<'_> {
    fn fill_selected(self, selected: &[usize], output: &mut [f64]) -> Result<()> {
        if self.generated_columns.len() != self.values.len()
            || self.generated_columns.windows(2).any(|p| p[0] >= p[1])
            || selected.len() != output.len()
        {
            return Err(invalid_argument(
                "current-frame mean coordinates are inconsistent",
            ));
        }
        for (&generated, value) in selected.iter().zip(output) {
            let position = self
                .generated_columns
                .binary_search(&generated)
                .map_err(|_| invalid_argument("current-frame mean coordinate is absent"))?;
            *value = self.values[position];
            if !value.is_finite() {
                return Err(invalid_data("current-frame mean is not finite"));
            }
        }
        Ok(())
    }
}

pub(super) struct RetainedFitContext<'a> {
    pub(super) raw_column_means: CurrentFrameColumnMeans<'a>,
    pub(super) reference_norms: Vec<f64>,
    pub(super) score_norms: Vec<f64>,
    pub(super) within_outcome_norm: f64,
    pub(super) residual_absorption_report: SolveReport,
    pub(super) absorption_report: SolveReport,
}

#[derive(Debug)]
struct RawMomentCache {
    means: Vec<f64>,
    scales: Vec<f64>,
    norms: Vec<f64>,
    weight_sum: f64,
}

/// Same-call speculative RHS storage. Cached statistics remain tied to its
/// private row owner and are consumed only after exact prepared-value matching.
#[derive(Debug)]
pub struct LoadedOlsRows {
    rows: Vec<f64>,
    moments: Option<Result<RawMomentCache>>,
}

impl From<Vec<f64>> for LoadedOlsRows {
    fn from(rows: Vec<f64>) -> Self {
        Self {
            rows,
            moments: None,
        }
    }
}

impl LoadedOlsRows {
    /// Bound the three result vectors and deterministic fused partial bank.
    /// Returning None keeps the ordinary fit-time computation.
    pub fn moment_workspace_bytes(nrows: usize, columns: usize) -> Option<usize> {
        if nrows == 0 || !(1..=FUSED_MOMENT_MAX_COLUMNS).contains(&columns) {
            return None;
        }
        let chunks = nrows
            .div_ceil(PARALLEL_REDUCTION_CHUNK_ROWS)
            .clamp(1, FUSED_MOMENT_MAX_BLOCKS);
        columns
            .checked_mul(3)?
            .checked_mul(chunks.checked_add(1)?)?
            .checked_mul(8)
    }

    /// Compute unit-weight moments on an installed team before preparation
    /// finishes. Allocation admission belongs to the whole-action caller.
    /// Errors stay deferred until the original fit-time moment boundary.
    pub fn with_unit_moments(
        mut self,
        nrows: usize,
        columns: usize,
        admitted_bytes: usize,
    ) -> Self {
        let Some(required) = Self::moment_workspace_bytes(nrows, columns) else {
            return self;
        };
        if self.moments.is_some()
            || required > admitted_bytes
            || nrows.checked_mul(columns) != Some(self.rows.len())
            || rayon::current_thread_index().is_none()
            || crate::parallel::StagePlan::current(nrows as u128 * columns as u128, 2).lanes <= 1
        {
            return self;
        }
        // Shape was checked above; failure here cannot leave a usable capsule.
        let raw = DenseMatrix::from_row_major(nrows, columns, std::mem::take(&mut self.rows))
            .expect("checked loaded OLS row dimensions");
        self.moments = Some(build_raw_moment_cache(
            &raw,
            NormalizedWeights {
                values: None,
                raw_scale: 1.0,
                kind: None,
            },
            &mut LoadedMomentReservation {
                remaining: admitted_bytes,
            },
        ));
        self.rows = raw.into_row_major();
        self
    }
}

struct LoadedMomentReservation {
    remaining: usize,
}

impl FitReservation for LoadedMomentReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        let bytes = elements
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| allocation("loaded OLS moment reservation overflows"))?;
        if !target.is_empty() || target.capacity() != 0 || bytes > self.remaining {
            return Err(allocation("loaded OLS moments exceed admitted workspace"));
        }
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))?;
        let actual = target
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| allocation("loaded OLS moment capacity overflows"))?;
        self.remaining = self
            .remaining
            .checked_sub(actual)
            .ok_or_else(|| allocation("loaded OLS moment capacity exceeds admission"))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct FitPerturbations {
    first_coefficient: bool,
    reconstructed_within_residual_constant: bool,
    final_residual_constant: bool,
    finite_bread: bool,
    reported_intercept: bool,
    reported_bread_extension: bool,

    one_fe_final_recovery_disabled: bool,
}

/// Fit the finite coefficients of a prepared model, without inference,
/// through the owner's declared ordinary or categorical projection semantics:
/// the strict borrowed-intercept MAP path and a certified scaled finite solve.
pub fn fit_prepared_model_coefficients(
    prepared: &PreparedOlsData,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    fit_prepared_ols_coefficients_view_map(prepared, config)
}

pub(super) fn fit_prepared_ols_coefficients_with_loaded_rows<
    P: PreparedOlsFitView + Sync + ?Sized,
>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    rows: LoadedOlsRows,
) -> Result<CertifiedPreparedOlsFit> {
    if config.execution.mode != PreparedOlsExecutionMode::Fast {
        drop(rows);
        return fit_prepared_ols_coefficients_view_map(prepared, config);
    }
    ThreadTeam::new(config.execution.threads).install(|| {
        validate_config(prepared, config)?;
        let frame = LinearSolveFrame::from_prepared_ols(prepared)?;
        fit_with_controls_internal_mode_with_frame(
            prepared,
            frame,
            config,
            &mut SystemFitReservation,
            FitPerturbations::default(),
            false,
            MAX_FAST_SCORE_CACHE_BYTES,
            None,
            None,
            Some(rows),
        )
    })?
}

pub(crate) fn fit_prepared_ols_coefficients_view_map<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    let team = ThreadTeam::new(config.execution.threads);
    team.install(|| {
        let mut reservation = SystemFitReservation;
        match config.execution.mode {
            PreparedOlsExecutionMode::Fast => fit_with_controls_internal(
                prepared,
                config,
                &mut reservation,
                FitPerturbations::default(),
            ),
            PreparedOlsExecutionMode::Low => fit_with_controls_low_internal(
                prepared,
                config,
                &mut reservation,
                FitPerturbations::default(),
                None,
            ),
        }
    })?
}

pub(super) fn assemble_retained_linear_frame<'a, P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    config: PreparedOlsFitConfig,
    mut qr: FiniteFit,
    draft: RetainedFitContext<'_>,
    generations: super::linear_session::LinearSessionGenerations,
) -> Result<(
    crate::ols::linear_session::CertifiedLinearFrame,
    super::linear_session::FrozenLinearBasis,
    Vec<f64>,
)> {
    frame.validate_for(prepared)?;
    validate_config(prepared, config)?;
    qr.validate_axes(
        prepared.fit_model().columns.len(),
        prepared.fit_sample().n_active(),
    )?;
    if draft.reference_norms.len() != qr.generated_coefficients.len()
        || draft.score_norms.len() != qr.generated_coefficients.len()
    {
        return Err(invalid_argument(
            "retained weighted-linear fit draft dimensions are inconsistent",
        ));
    }
    let mut coefficients = build_coefficient_slots(prepared, &qr, &mut SystemFitReservation)?;
    let weights = frame.weights();
    let finite_predictor = retained_finite_predictor(prepared, &qr, &coefficients)?;
    let bread = finalize_reported_bread_and_intercept(
        prepared,
        frame.response(),
        &mut qr,
        None,
        weights,
        config.bread_tolerance,
        &mut SystemFitReservation,
        FitPerturbations::default(),
        None,
        None,
        None,
        Some(draft.raw_column_means),
        Some(&finite_predictor),
    )?;
    let within_residuals = qr.within_residuals.take().ok_or_else(|| {
        invalid_argument("retained weighted-linear fit did not retain its within residual")
    })?;
    if !draft.residual_absorption_report.certified() {
        return Err(invalid_data(
            "retained weighted-linear residual absorption did not certify",
        ));
    }
    let mut residual_absorption_report = draft.residual_absorption_report;
    // A retained PPML session may own the accepted fit beside one speculative
    // strict candidate. Solver warnings are diagnostic strings with no
    // scientific authority and no finite heap bound, so they cannot become
    // persistent session state. Drop both warning banks here, without
    // changing the public one-shot fereg result path.
    residual_absorption_report.warnings = Vec::new();
    let mut absorption_report = draft.absorption_report;
    absorption_report.warnings = Vec::new();
    let score_certificate = certify_scores(
        prepared,
        &within_residuals,
        weights,
        &draft.reference_norms,
        &draft.score_norms,
        qr.finite_score,
        &qr.generated_statuses,
        draft.within_outcome_norm,
        config.score_tolerance,
    )?;
    if !score_certificate.passed {
        return Err(invalid_data(
            "retained weighted-linear score certificate did not pass",
        ));
    }
    // Bread installs only the deferred absorbed-intercept normalization.
    // Preserve the already-built slot owner and refresh its reported fields.
    for coefficient in &mut coefficients {
        if let Some(generated) = coefficient.generated_column_index {
            coefficient.status = qr.generated_statuses[generated];
            coefficient.value = qr.generated_coefficients[generated];
        }
    }
    qr.kept.sort_unstable();
    qr.dropped.sort_unstable();
    let (bread, bread_certificate) = compact_linear_bread(bread, &qr.kept)?;
    let mut reported = reserved_empty(
        &mut SystemFitReservation,
        coefficients.len(),
        "unable to reserve linear reported values",
    )?;
    for coefficient in &coefficients {
        push_reserved(
            &mut reported,
            coefficient.value,
            "linear reported capacity was underestimated",
        )?;
    }
    let mut compact = reserved_empty(
        &mut SystemFitReservation,
        qr.kept.len(),
        "unable to reserve linear finite values",
    )?;
    for &generated in &qr.kept {
        push_reserved(
            &mut compact,
            qr.generated_coefficients[generated]
                .ok_or_else(|| invalid_data("linear kept coefficient is absent"))?,
            "linear finite capacity was underestimated",
        )?;
    }
    let basis =
        super::linear_session::FrozenLinearBasis::from_slots(&coefficients, qr.kept, qr.dropped)?;
    Ok((
        crate::ols::linear_session::CertifiedLinearFrame {
            coefficients: compact,
            reported_coefficients: reported,
            bread,
            within_residuals,
            response_absorption_report: absorption_report,
            generations,
            decomposition: None,
            proof: super::linear_session::LinearFrameProof::Canonical(
                super::linear_session::CanonicalLinearProof {
                    rank: qr.certificate,
                    score: score_certificate,
                    bread: bread_certificate,
                    residual_absorption: residual_absorption_report,
                },
            ),
        },
        basis,
        finite_predictor,
    ))
}

fn compact_linear_bread(
    mut bread: PreparedOlsBread,
    kept: &[usize],
) -> Result<(Vec<f64>, PreparedOlsBreadCertificate)> {
    let rank = kept.len();
    let dimension = bread.dimension();
    if dimension < rank
        || dimension > rank + 1
        || !bread.certificate.passed
        || bread
            .coordinates
            .iter()
            .take(rank)
            .zip(kept)
            .any(|(coordinate, generated)| {
                coordinate.kind != PreparedOlsBreadCoordinateKind::FiniteRegressor
                    || coordinate.generated_column_index != *generated
            })
        || bread.coordinates.iter().skip(rank).any(|coordinate| {
            coordinate.kind != PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept
        })
        || bread.values.len()
            != dimension
                .checked_mul(dimension)
                .ok_or_else(|| allocation("linear bread dimensions overflow"))?
    {
        return Err(invalid_argument(
            "linear bread coordinates do not match the canonical basis",
        ));
    }
    for row in 0..rank {
        bread
            .values
            .copy_within(row * dimension..row * dimension + rank, row * rank);
    }
    // Truncation preserves the admitted expanded-bread capacity.
    bread.values.truncate(rank * rank);
    Ok((bread.values, bread.certificate))
}

fn retained_finite_predictor<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    qr: &FiniteFit,
    coefficients: &[PreparedOlsFittedCoefficient],
) -> Result<Vec<f64>> {
    let mut finite_predictor = reserved_filled(
        &mut SystemFitReservation,
        prepared.fit_sample().n_active(),
        0.0_f64,
        "unable to reserve retained weighted-linear finite predictor",
    )?;
    let numeric = super::prepared_rhs::NumericOlsColumns::bind_score_means(
        prepared.fit_model(),
        prepared.fit_sample(),
    )?;
    if let Some(numeric) = numeric {
        let mut selected = [(0, 0.); super::prepared_rhs::MAX_BOUND_NUMERIC_COLUMNS];
        let mut count = 0;
        for (generated, coefficient) in qr.generated_coefficients.iter().enumerate() {
            if let Some(coefficient) = coefficient {
                selected[count] = (generated, *coefficient);
                count += 1;
            }
        }
        numeric.add_selected_columns(&selected[..count], &mut finite_predictor)?;
    } else if !crate::ols::prepared_fit_view::add_generated_predictor(
        prepared,
        coefficients,
        &mut finite_predictor,
    )? {
        crate::parallel::stage_try_for_each(
            finite_predictor.par_iter_mut().enumerate(),
            prepared.fit_sample().n_active() as u128 * qr.generated_coefficients.len() as u128,
            |(row, target)| -> Result<()> {
                let mut value = 0.;
                for (generated, coefficient) in qr.generated_coefficients.iter().enumerate() {
                    if let Some(coefficient) = coefficient {
                        value =
                            coefficient.mul_add(prepared.fit_column_value(generated, row)?, value);
                    }
                }
                if !value.is_finite() {
                    return Err(invalid_data("retained finite predictor is not finite"));
                }
                *target = value;
                Ok(())
            },
        )?;
    }
    Ok(finite_predictor)
}

/// Canonical finite-plus-FE projection without covariance/report publication.
/// The public OLS path never calls this boundary.
pub(crate) fn fit_prepared_weighted_linear_projection_once<
    'a,
    P: PreparedOlsFitView + Sync + ?Sized,
>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedLinearProjection> {
    let team = ThreadTeam::new(config.execution.threads);
    team.install(|| {
        validate_config(prepared, config)?;
        frame.validate_for(prepared)?;
        let mut reservation = SystemFitReservation;
        fit_projection_only_internal(prepared, frame, config, &mut reservation)
    })?
}

fn fit_projection_only_internal<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    frame: LinearSolveFrame<'_>,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
) -> Result<CertifiedPreparedLinearProjection> {
    let weights = frame.weights();
    let mut cmg = crate::fe::FeSession::for_ols(prepared, weights.borrowed(), config)?;
    let raw = prepared.fit_build_rhs_with_response(frame.response())?;
    let generated_columns = prepared.fit_model().columns.len();
    let mut reference_norms = reserved_filled(
        reservation,
        generated_columns,
        0.0_f64,
        "unable to reserve prepared projection reference norms",
    )?;
    for (column, norm) in reference_norms.iter_mut().enumerate() {
        *norm = weighted_matrix_column_norm(&raw, column + 1, weights)?;
    }
    let mut absorbed = cmg.project_ols(weights.borrowed(), raw, config.absorb, false, false)?;
    if !absorbed.report().certified() {
        return Err(invalid_data(
            "prepared projection initial absorption did not certify",
        ));
    }
    // The optional FE handoff certifies norms of this exact returned matrix
    // under the same normalized weights. Explicit divisor frames can use a
    // different normalization and must retain the established recomputation.
    let cached_norms = if weights.matches_fe_norm_scale() {
        absorbed.take_residual_norms()
    } else {
        Vec::new()
    };
    let (within, absorption_report, _) = absorbed.into_full_parts();
    let has_cached_norms = !cached_norms.is_empty();
    let mut within_norms = if has_cached_norms {
        cached_norms
    } else {
        reserved_filled(
            reservation,
            generated_columns + 1,
            0.0_f64,
            "unable to reserve prepared projection within norms",
        )?
    };
    if !has_cached_norms {
        if rayon::current_num_threads() == 1 {
            fill_weighted_matrix_norms_fused(&within, weights, &mut within_norms, reservation)?;
        } else {
            crate::parallel::stage_try_for_each(
                within_norms.par_iter_mut().enumerate(),
                within.nrows() as u128 * within.ncols() as u128,
                |(column, norm)| -> Result<()> {
                    *norm = weighted_matrix_column_norm(&within, column, weights)?;
                    Ok(())
                },
            )?;
        }
    }
    let within_outcome_norm = within_norms[0];
    let score_norms = &within_norms[1..];
    let mut qr = finite::solve_one_shot(
        prepared,
        finite::ProjectedFiniteRows::Dense(&within),
        weights,
        frame.bread_weight_scale(),
        &reference_norms,
        score_norms,
        config,
        reservation,
        false,
        false,
        false,
    )?;
    if qr.bread.is_some() {
        return Err(invalid_argument(
            "prepared projection-only finite solve unexpectedly constructed bread",
        ));
    }
    let within_residuals = qr.within_residuals.take().ok_or_else(|| {
        invalid_argument("prepared projection finite solve did not retain its residual")
    })?;
    let residual_matrix =
        DenseMatrix::from_row_major(prepared.fit_sample().n_active(), 1, within_residuals)?;
    let residual_absorbed = cmg.project_ols(
        weights.borrowed(),
        residual_matrix,
        config.final_absorb_config(),
        true,
        false,
    )?;
    if !residual_absorbed.report().certified() {
        return Err(invalid_data(
            "prepared projection residual absorption did not certify",
        ));
    }
    let (residual_matrix, residual_absorption_report, _) = residual_absorbed.into_full_parts();
    let residuals = residual_matrix.into_row_major();
    let score_certificate = certify_scores(
        prepared,
        &residuals,
        weights,
        &reference_norms,
        score_norms,
        qr.finite_score,
        &qr.generated_statuses,
        within_outcome_norm,
        config.score_tolerance,
    )?;
    if !score_certificate.passed {
        return Err(invalid_data(
            "prepared projection finite score certificate did not pass",
        ));
    }
    let coefficients = build_coefficient_slots(prepared, &qr, reservation)?;
    // A deferred absorbed-model intercept is not part of the finite rank
    // problem and is not scientifically dropped.  The full OLS publication
    // step removes it from this ledger when it installs the normalized
    // reported intercept; projection-only deliberately skips that
    // publication step, so make the same basis distinction here explicitly.
    qr.dropped.retain(|generated| {
        qr.generated_statuses[*generated] != PreparedOlsFitStatus::AbsorbedInterceptDeferred
    });
    qr.kept.sort_unstable();
    qr.dropped.sort_unstable();
    let mut fitted = reserved_filled(
        reservation,
        frame.response().len(),
        0.0_f64,
        "unable to reserve prepared projection fitted values",
    )?;
    let mut reconstruction_relative_infinity_norm = 0.0_f64;
    for row in 0..fitted.len() {
        fitted[row] = frame.response()[row] - residuals[row];
        let reconstructed = fitted[row] + residuals[row];
        let scale = frame.response()[row]
            .abs()
            .max(reconstructed.abs())
            .max(1.0);
        reconstruction_relative_infinity_norm = reconstruction_relative_infinity_norm
            .max((reconstructed - frame.response()[row]).abs() / scale);
        if !fitted[row].is_finite() || !reconstruction_relative_infinity_norm.is_finite() {
            return Err(invalid_data(
                "prepared projection reconstruction is not finite",
            ));
        }
    }
    let reconstruction_certificate = PreparedLinearProjectionReconstructionCertificate {
        relative_infinity_norm: reconstruction_relative_infinity_norm,
        passed: reconstruction_relative_infinity_norm <= 64.0 * f64::EPSILON,
    };
    let projection = CertifiedPreparedLinearProjection {
        coefficients,
        kept_generated_columns: qr.kept,
        dropped_generated_columns: qr.dropped,
        finite_rank: qr.rank,
        rank_certificate: qr.certificate,
        score_certificate,
        absorption_report,
        residual_absorption_report,
        fitted,
        residuals,
        reconstruction_certificate,
    };
    if projection.certified() {
        Ok(projection)
    } else {
        Err(invalid_data(
            "prepared projection-only solve did not certify",
        ))
    }
}

#[cfg(test)]
pub(crate) fn fit_prepared_ols_coefficients_view_map_for_inference<
    P: PreparedOlsFitView + Sync + ?Sized,
>(
    prepared: &P,
    config: PreparedOlsFitConfig,
) -> Result<(
    CertifiedPreparedOlsFit,
    super::prepared_scores::PreparedOlsScoreProduct,
)> {
    fit_prepared_ols_coefficients_view_map_for_inference_with_loaded_moments(prepared, config, None)
}

pub(crate) fn fit_prepared_ols_coefficients_view_map_for_inference_with_loaded_moments<
    P: PreparedOlsFitView + Sync + ?Sized,
>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    rows: Option<LoadedOlsRows>,
) -> Result<(
    CertifiedPreparedOlsFit,
    super::prepared_scores::PreparedOlsScoreProduct,
)> {
    let mut scratch = None;
    let mut fit = fit_for_inference_with_scratch(
        prepared,
        config,
        MAX_FAST_SCORE_CACHE_BYTES.min(32 * 1024 * 1024),
        Some(&mut scratch),
        rows,
    )?;
    let scores = super::prepared_scores::take_prepared_ols_score_product(&mut fit, scratch)?;
    Ok((fit, scores))
}

#[cfg(test)]
pub(crate) fn fit_prepared_ols_coefficients_view_map_for_inference_with_cache_limit<
    P: PreparedOlsFitView + Sync + ?Sized,
>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    max_fast_score_cache_bytes: usize,
) -> Result<CertifiedPreparedOlsFit> {
    fit_for_inference_with_scratch(prepared, config, max_fast_score_cache_bytes, None, None)
}

fn fit_for_inference_with_scratch<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    max_fast_score_cache_bytes: usize,
    retained_scratch: Option<&mut Option<PreparedOlsScratch>>,
    rows: Option<LoadedOlsRows>,
) -> Result<CertifiedPreparedOlsFit> {
    let team = ThreadTeam::new(config.execution.threads);
    // Inference needs the projected columns again. Prefer a packed cache when
    // its caller's bound admits it; larger fits retain immutable projected
    // rows through a selected-column view. Rank-selected QR and Cholesky solves both retain
    // the immutable projected rows used to certify their accepted fit.
    let retain_fast_score_cache = true;
    team.install(|| {
        let mut reservation = SystemFitReservation;
        match config.execution.mode {
            PreparedOlsExecutionMode::Fast => {
                validate_config(prepared, config)?;
                let frame = LinearSolveFrame::from_prepared_ols(prepared)?;
                fit_with_controls_internal_mode_with_frame(
                    prepared,
                    frame,
                    config,
                    &mut reservation,
                    FitPerturbations::default(),
                    retain_fast_score_cache,
                    max_fast_score_cache_bytes,
                    None,
                    None,
                    rows,
                )
            }
            PreparedOlsExecutionMode::Low => fit_with_controls_low_internal(
                prepared,
                config,
                &mut reservation,
                FitPerturbations::default(),
                retained_scratch,
            ),
        }
    })?
}

// The immutable FE binding spans initial blocks and the terminal residual.
fn fit_with_controls_internal<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
) -> Result<CertifiedPreparedOlsFit> {
    validate_config(prepared, config)?;
    let frame = LinearSolveFrame::from_prepared_ols(prepared)?;
    fit_with_controls_internal_mode_with_frame(
        prepared,
        frame,
        config,
        reservation,
        perturbations,
        false,
        MAX_FAST_SCORE_CACHE_BYTES,
        None,
        None,
        None,
    )
}

/// The initial solver can transfer dependency-owned buffers with spare capacity.
/// Check that owner plus the later one-RHS product against the reservation
/// captured by the solver session, before allocating the finite-solve frame.
fn check_retained_fe_initial_capacity(
    initial: Option<&FeCoefficientBatch>,
    reserved_parent: u128,
) -> Result<()> {
    let Some(initial) = initial else {
        return Ok(());
    };
    let combined_values =
        (initial.rhs(0)?.len() as u128).checked_mul(std::mem::size_of::<f64>() as u128);
    let combined_layout = (initial.effect_channels().len() as u128)
        .checked_mul(3)
        .and_then(|cells| cells.checked_add(1))
        .and_then(|cells| cells.checked_mul(std::mem::size_of::<usize>() as u128));
    let required = combined_values
        .and_then(|bytes| bytes.checked_add(combined_layout?))
        .and_then(|bytes| bytes.checked_add(initial.retained_bytes()))
        .ok_or_else(|| allocation("retained FE product capacity overflows"))?;
    if required > reserved_parent {
        return Err(allocation(
            "retained FE product capacity exceeds its admitted parent reservation",
        ));
    }
    Ok(())
}

fn combine_recovered_fe_coefficients<R: FitReservation>(
    initial: Option<FeCoefficientBatch>,
    residual: Option<FeCoefficientBatch>,
    residual_iterations: usize,
    generated_coefficients: &[Option<f64>],
    reserved_parent: u128,
    reservation: &mut R,
) -> Result<Option<FeCoefficientBatch>> {
    let Some(initial) = initial else {
        return Ok(None);
    };
    if initial.rhs_count() != generated_coefficients.len() + 1 {
        return Err(invalid_argument(
            "initial FE coefficient batch and generated OLS columns differ",
        ));
    }
    check_retained_fe_initial_capacity(Some(&initial), reserved_parent)?;
    let initial_capacity = initial.retained_bytes();
    let initial_outcome = initial.rhs(0)?;
    let mut combined = reserved_empty(
        reservation,
        initial_outcome.len(),
        "unable to reserve combined FE coefficient product",
    )?;
    combined.extend_from_slice(initial_outcome);
    for (generated, coefficient) in generated_coefficients.iter().copied().enumerate() {
        let Some(coefficient) = coefficient else {
            continue;
        };
        for (destination, source) in combined.iter_mut().zip(initial.rhs(generated + 1)?) {
            *destination = (-coefficient).mul_add(*source, *destination);
            if !destination.is_finite() {
                return Err(invalid_data("combined initial FE coefficient is nonfinite"));
            }
        }
    }
    if let Some(residual) = residual {
        if residual.rhs_count() != 1
            || residual.effect_channels() != initial.effect_channels()
            || residual.level_counts() != initial.level_counts()
        {
            return Err(invalid_argument(
                "residual FE coefficient correction layout differs from the initial solve",
            ));
        }
        for (destination, correction) in combined.iter_mut().zip(residual.rhs(0)?) {
            *destination += correction;
            if !destination.is_finite() {
                return Err(invalid_data(
                    "combined residual FE coefficient correction is nonfinite",
                ));
            }
        }
    } else if residual_iterations != 0 {
        return Ok(None);
    }
    let mut effect_channels = reserved_empty(
        reservation,
        initial.effect_channels().len(),
        "unable to reserve combined FE channel layout",
    )?;
    effect_channels.extend_from_slice(initial.effect_channels());
    let mut level_counts = reserved_empty(
        reservation,
        initial.level_counts().len(),
        "unable to reserve combined FE level layout",
    )?;
    level_counts.extend_from_slice(initial.level_counts());
    let combined = FeCoefficientBatch::from_values(effect_channels, level_counts, 1, combined)?;
    if initial_capacity
        .checked_add(combined.retained_bytes())
        .is_none_or(|bytes| bytes > reserved_parent)
    {
        return Err(allocation(
            "combined FE product capacity exceeds its admitted parent reservation",
        ));
    }
    Ok(Some(combined))
}

#[cfg(test)]
fn fit_with_controls_internal_mode<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
    retain_fast_score_cache: bool,
    max_fast_score_cache_bytes: usize,
) -> Result<CertifiedPreparedOlsFit> {
    validate_config(prepared, config)?;
    let frame = LinearSolveFrame::from_prepared_ols(prepared)?;
    fit_with_controls_internal_mode_with_frame(
        prepared,
        frame,
        config,
        reservation,
        perturbations,
        retain_fast_score_cache,
        max_fast_score_cache_bytes,
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn fit_with_controls_internal_mode_with_frame<
    'a,
    P: PreparedOlsFitView + ?Sized,
    R: FitReservation,
>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
    retain_fast_score_cache: bool,
    max_fast_score_cache_bytes: usize,
    phase_probe: Option<LinearSessionPhaseAllocatorProbe>,
    phase_observation: Option<&mut LinearSessionPhaseAllocationObservation>,
    loaded_rows: Option<LoadedOlsRows>,
) -> Result<CertifiedPreparedOlsFit> {
    one_shot::fit(
        prepared,
        frame,
        config,
        reservation,
        perturbations,
        retain_fast_score_cache,
        max_fast_score_cache_bytes,
        phase_probe,
        phase_observation,
        loaded_rows,
        None,
    )
}

fn build_independent_raw_residuals<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    response: &[f64],
    qr: &FiniteFit,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let rows = prepared.fit_sample().n_active();
    if response.len() != rows {
        return Err(invalid_argument(
            "prepared weighted-linear response and residual axis differ",
        ));
    }
    let mut residuals = reserved_empty(
        reservation,
        rows,
        "unable to reserve prepared OLS independent raw residual",
    )?;
    residuals.extend_from_slice(response);
    if let Some(columns) =
        super::prepared_rhs::NumericOlsColumns::bind(prepared.fit_model(), prepared.fit_sample())?
    {
        columns.subtract_fitted(&qr.generated_coefficients, &mut residuals)?;
    } else {
        for (column, coefficient) in qr.generated_coefficients.iter().copied().enumerate() {
            if let Some(coefficient) = coefficient {
                prepared.fit_subtract_raw_column(column, coefficient, &mut residuals)?;
            }
        }
    }
    Ok(residuals)
}

// Shared upper bound for one-shot weighted-linear callers. Ordinary OLS
// inference selects a lower packed-cache limit at its entry point; larger
// Larger fits retain their immutable projected rows through a selected view.
// Explicit cache-limit callers retain their own policy.
const MAX_FAST_SCORE_CACHE_BYTES: usize = 512 * 1024 * 1024;

fn fast_score_cache_fits<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    max_bytes: usize,
) -> Result<bool> {
    let cached_columns = prepared
        .fit_model()
        .columns
        .iter()
        .filter(|column| {
            !(prepared.fit_has_absorbed_intercept()
                && matches!(column.kind, PreparedOlsColumnKind::Intercept))
        })
        .count();
    let bytes = prepared
        .fit_sample()
        .n_active()
        .checked_mul(cached_columns)
        .and_then(|elements| elements.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| allocation("prepared OLS fast score cache bytes overflow"))?;
    Ok(bytes <= max_bytes)
}

fn build_fast_score_cache<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    within: &DenseMatrix,
    reference_mean_scales: &[f64],
    weights: NormalizedWeights<'_>,
    reservation: &mut R,
    max_bytes: usize,
    bind_numeric_means: bool,
) -> Result<Option<PreparedOlsFastScoreCache>> {
    let nrows = prepared.fit_sample().n_active();
    if within.nrows() != nrows
        || within.ncols() != prepared.fit_model().columns.len() + 1
        || reference_mean_scales.len() != prepared.fit_model().columns.len()
    {
        return Err(invalid_argument(
            "prepared OLS fast score cache dimensions are inconsistent",
        ));
    }
    let mut generated_columns = reserved_empty(
        reservation,
        prepared.fit_model().columns.len(),
        "unable to reserve prepared OLS fast score cache identities",
    )?;
    for (generated, column) in prepared.fit_model().columns.iter().enumerate() {
        if prepared.fit_has_absorbed_intercept()
            && matches!(column.kind, PreparedOlsColumnKind::Intercept)
        {
            continue;
        }
        push_reserved(
            &mut generated_columns,
            generated,
            "prepared OLS fast score cache identity capacity exhausted",
        )?;
    }
    let elements = nrows
        .checked_mul(generated_columns.len())
        .ok_or_else(|| allocation("prepared OLS fast score cache dimensions overflow"))?;
    let bytes = elements
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or_else(|| allocation("prepared OLS fast score cache bytes overflow"))?;
    if bytes > max_bytes {
        return Ok(None);
    }
    let mut values = reserved_filled(
        reservation,
        elements,
        0.0_f64,
        "unable to reserve prepared OLS fast score cache values",
    )?;
    let mut means = reserved_filled(
        reservation,
        generated_columns.len(),
        0.0_f64,
        "unable to reserve prepared OLS fast score cache means",
    )?;
    if generated_columns.is_empty() {
        let within = DenseMatrix::from_row_major(nrows, 0, values)?;
        return Ok(Some(PreparedOlsFastScoreCache::Packed {
            generated_columns,
            within,
            means,
        }));
    }

    let numeric = if bind_numeric_means {
        super::prepared_rhs::NumericOlsColumns::bind_score_means(
            prepared.fit_model(),
            prepared.fit_sample(),
        )?
    } else {
        None
    };
    let numeric_columns = numeric.as_ref().map(|columns| columns.raw_columns());

    // Cache construction is an O(NK) inference-path operation. Keep its hot
    // inner loop to ordinary additions and merge fixed row blocks with
    // compensation. This preserves deterministic, high-accuracy reference
    // means without paying a compensated update for every observation-term
    // pair. The independent score and covariance certificates still recompute
    // their publication checks from the cached design.
    let requested_chunks = nrows
        .div_ceil(PARALLEL_REDUCTION_CHUNK_ROWS)
        .clamp(1, FUSED_MOMENT_MAX_BLOCKS);
    let rows_per_chunk = nrows.div_ceil(requested_chunks);
    let chunks = nrows.div_ceil(rows_per_chunk);
    let partial_stride = generated_columns
        .len()
        .checked_add(1)
        .ok_or_else(|| allocation("prepared OLS fast score partial stride overflows usize"))?;
    let partial_elements = chunks
        .checked_mul(partial_stride)
        .ok_or_else(|| allocation("prepared OLS fast score partial allocation overflows usize"))?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve prepared OLS fast score cache partials",
    )?;
    let value_chunk_elements = rows_per_chunk
        .checked_mul(generated_columns.len())
        .ok_or_else(|| allocation("prepared OLS fast score value chunk overflows usize"))?;
    values
        .par_chunks_mut(value_chunk_elements)
        .zip(partials.par_chunks_mut(partial_stride))
        .enumerate()
        .try_for_each(|(chunk, (value_chunk, partial))| -> Result<()> {
            let start = chunk * rows_per_chunk;
            let chunk_rows = value_chunk.len() / generated_columns.len();
            for local_row in 0..chunk_rows {
                let row = start + local_row;
                let weight = weights.value(row)?;
                partial[0] += weight;
                for (cached, generated) in generated_columns.iter().copied().enumerate() {
                    let value = within[(row, generated + 1)];
                    if !value.is_finite() {
                        return Err(invalid_data(
                            "prepared OLS fast score cache contains a nonfinite value",
                        ));
                    }
                    value_chunk[local_row * generated_columns.len() + cached] = value;
                    let scale = reference_mean_scales[generated];
                    if scale > 0.0 {
                        let raw_value = if let Some(columns) = numeric_columns {
                            columns[generated].map_or(1.0, |source| source[row])
                        } else {
                            prepared.fit_column_value(generated, row)?
                        };
                        partial[cached + 1] += weight * (raw_value / scale);
                    }
                }
            }
            if partial.iter().all(|value| value.is_finite()) {
                Ok(())
            } else {
                Err(invalid_data(
                    "prepared OLS fast score cache partial is not finite",
                ))
            }
        })?;

    let mut denominator = 0.0_f64;
    let mut denominator_correction = 0.0_f64;
    for partial in partials.chunks_exact(partial_stride) {
        compensated_add(partial[0], &mut denominator, &mut denominator_correction)?;
    }
    let denominator = denominator + denominator_correction;
    for (cached, generated) in generated_columns.iter().copied().enumerate() {
        let scale = reference_mean_scales[generated];
        let mut normalized_mean = 0.0_f64;
        let mut normalized_mean_correction = 0.0_f64;
        for partial in partials.chunks_exact(partial_stride) {
            compensated_add(
                partial[cached + 1],
                &mut normalized_mean,
                &mut normalized_mean_correction,
            )?;
        }
        let mean = if scale == 0.0 {
            0.0
        } else {
            scale * ((normalized_mean + normalized_mean_correction) / denominator)
        };
        if !mean.is_finite() || !denominator.is_finite() || denominator <= 0.0 {
            return Err(invalid_data(
                "prepared OLS fast score cache mean is not finite",
            ));
        }
        means[cached] = mean;
    }
    let within = DenseMatrix::from_row_major(nrows, generated_columns.len(), values)?;
    Ok(Some(PreparedOlsFastScoreCache::Packed {
        generated_columns,
        within,
        means,
    }))
}

fn build_selected_fast_score_cache<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    within: DenseMatrix,
    qr: &FiniteFit,
    all_means: Option<&[f64]>,
    reservation: &mut R,
) -> Result<PreparedOlsFastScoreCache> {
    let bread = qr
        .bread
        .as_ref()
        .ok_or_else(|| invalid_argument("prepared OLS strided score cache has no bread"))?;
    let nrows = prepared.fit_sample().n_active();
    let expected_columns = prepared
        .fit_model()
        .columns
        .len()
        .checked_add(1)
        .ok_or_else(|| allocation("prepared OLS strided score column count overflows"))?;
    if within.nrows() != nrows
        || within.ncols() != expected_columns
        || all_means.is_none_or(|means| means.len() != prepared.fit_model().columns.len())
    {
        return Err(invalid_argument(
            "prepared OLS strided score cache dimensions are inconsistent",
        ));
    }
    let mut generated_columns = reserved_empty(
        reservation,
        bread.generated_columns.len(),
        "unable to reserve prepared OLS strided score identities",
    )?;
    generated_columns.extend_from_slice(&bread.generated_columns);
    let all_means =
        all_means.ok_or_else(|| invalid_argument("prepared OLS strided score means are absent"))?;
    let mut means = reserved_filled(
        reservation,
        generated_columns.len(),
        0.0_f64,
        "unable to reserve prepared OLS strided score means",
    )?;
    for (target, generated) in means.iter_mut().zip(generated_columns.iter().copied()) {
        *target = all_means[generated];
        if !target.is_finite() {
            return Err(invalid_data(
                "prepared OLS strided score mean is not finite",
            ));
        }
    }
    Ok(PreparedOlsFastScoreCache::Selected {
        generated_columns,
        within,
        means,
    })
}

fn fit_with_controls_low_internal<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
    retained_scratch: Option<&mut Option<PreparedOlsScratch>>,
) -> Result<CertifiedPreparedOlsFit> {
    validate_config(prepared, config)?;
    let frame = LinearSolveFrame::from_prepared_ols(prepared)?;
    fit_with_controls_low_internal_with_frame(
        prepared,
        frame,
        config,
        reservation,
        perturbations,
        None,
        None,
        retained_scratch,
    )
}

// Keep the legacy workspace and ownership boundaries explicit during migration.
#[allow(clippy::too_many_arguments)]
fn fit_with_controls_low_internal_with_frame<
    'a,
    P: PreparedOlsFitView + ?Sized,
    R: FitReservation,
>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
    phase_probe: Option<LinearSessionPhaseAllocatorProbe>,
    phase_observation: Option<&mut LinearSessionPhaseAllocationObservation>,
    retained_scratch: Option<&mut Option<PreparedOlsScratch>>,
) -> Result<CertifiedPreparedOlsFit> {
    one_shot::fit(
        prepared,
        frame,
        config,
        reservation,
        perturbations,
        false,
        MAX_FAST_SCORE_CACHE_BYTES,
        phase_probe,
        phase_observation,
        None,
        retained_scratch,
    )
}

pub(super) fn merge_solve_report(target: &mut SolveReport, source: SolveReport) -> Result<()> {
    // RHS blocks share a model and weight frame. A bounded numerical fallback
    // can finish different blocks with MAP, direct CG or LSMR.
    // Some preconditioner metadata describes an LSMR attempt; None describes
    // a block that never entered LSMR, rather than a conflicting request.
    let valid_auto = |report: &SolveReport| {
        report.requested_solver == FeSolverKind::Auto
            && match (
                report.used_solver,
                report.requested_preconditioner,
                report.used_preconditioner,
            ) {
                (FeSolverKind::Map | FeSolverKind::Cg, None, None) => true,
                (FeSolverKind::Lsmr, Some(_), Some(_)) => true,
                (FeSolverKind::Cg | FeSolverKind::Map, Some(_), Some(_)) => report.fallback_used,
                _ => false,
            }
    };
    let auto_mix = valid_auto(target)
        && valid_auto(&source)
        && match (
            target.requested_preconditioner,
            source.requested_preconditioner,
        ) {
            (Some(a), Some(b)) => {
                a == b && target.used_preconditioner == source.used_preconditioner
            }
            _ => true,
        };
    let cmg_fallback_mix = target.requested_solver == FeSolverKind::Cmg
        && source.requested_solver == FeSolverKind::Cmg
        && matches!(target.used_solver, FeSolverKind::Cmg | FeSolverKind::Cg)
        && matches!(source.used_solver, FeSolverKind::Cmg | FeSolverKind::Cg);
    if !target.certified()
        || !source.certified()
        || target.requested_solver != source.requested_solver
        || (!auto_mix
            && ((target.used_solver != source.used_solver && !cmg_fallback_mix)
                || target.requested_preconditioner != source.requested_preconditioner
                || target.used_preconditioner != source.used_preconditioner))
    {
        return Err(FeregError::new(ErrorCode::InvalidData, format!(
            "prepared OLS low-memory solver reports are inconsistent: target={:?}/{:?}/{:?}/{:?}, source={:?}/{:?}/{:?}/{:?}",
            target.requested_solver, target.used_solver, target.requested_preconditioner, target.used_preconditioner,
            source.requested_solver, source.used_solver, source.requested_preconditioner, source.used_preconditioner,
        )));
    }
    if auto_mix {
        // A certified continuation remains authoritative over its LSMR
        // attempt. Otherwise LSMR metadata takes precedence over direct CG.
        let priority = |report: &SolveReport| match report.used_solver {
            FeSolverKind::Map if report.fallback_used => 4,
            FeSolverKind::Cg if report.fallback_used => 3,
            FeSolverKind::Lsmr => 2,
            FeSolverKind::Cg => 1,
            _ => 0,
        };
        if priority(&source) > priority(target) {
            target.used_solver = source.used_solver;
        }
        if target.requested_preconditioner.is_none() {
            target.requested_preconditioner = source.requested_preconditioner;
            target.used_preconditioner = source.used_preconditioner;
        }
    } else if cmg_fallback_mix
        && (target.used_solver == FeSolverKind::Cg || source.used_solver == FeSolverKind::Cg)
    {
        target.used_solver = FeSolverKind::Cg;
        target.fallback_used = true;
    }
    target.iterations = target.iterations.max(source.iterations);
    target.checkpoint_retries = target
        .checkpoint_retries
        .saturating_add(source.checkpoint_retries);
    target.stop_reason = match (target.stop_reason, source.stop_reason) {
        (SolveStopReason::NoFixedEffects, SolveStopReason::NoFixedEffects) => {
            SolveStopReason::NoFixedEffects
        }
        _ => SolveStopReason::Certified,
    };
    target.certificate = FeCertificate {
        absolute_infinity_norm: target
            .certificate
            .absolute_infinity_norm
            .max(source.certificate.absolute_infinity_norm),
        relative_infinity_norm: target
            .certificate
            .relative_infinity_norm
            .max(source.certificate.relative_infinity_norm),
        relative_two_norm: target
            .certificate
            .relative_two_norm
            .max(source.certificate.relative_two_norm),
        passed: true,
    };
    target.normal_equation_residual = target
        .normal_equation_residual
        .max(source.normal_equation_residual);
    target.solver_workspace_peak = target
        .solver_workspace_peak
        .max(source.solver_workspace_peak);
    target.setup_seconds += source.setup_seconds;
    target.solve_seconds += source.solve_seconds;
    target.certification_seconds += source.certification_seconds;
    target.fallback_used |= source.fallback_used;
    target.pilot_iterations = target.pilot_iterations.max(source.pilot_iterations);
    target.pilot_contraction = target.pilot_contraction.max(source.pilot_contraction);
    target.warnings.extend(source.warnings);
    if target.setup_seconds.is_finite() && target.solve_seconds.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "prepared OLS low-memory solver timing is not finite",
        ))
    }
}

fn weighted_slice_cross(
    left: &[f64],
    right: &[f64],
    weights: NormalizedWeights<'_>,
) -> Result<f64> {
    if left.len() != right.len() {
        return Err(invalid_argument(
            "prepared OLS low-memory cross-product axes disagree",
        ));
    }
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for row in 0..left.len() {
        compensated_add(
            weights.value(row)? * left[row] * right[row],
            &mut sum,
            &mut correction,
        )?;
    }
    let value = sum + correction;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid_data(
            "prepared OLS low-memory cross-product is not finite",
        ))
    }
}

fn initial_within_sum_of_squares_low<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    outcome_norm: f64,
    intercept_cross: Option<(f64, f64)>,
) -> Result<f64> {
    let uncentered =
        crate::kernels::fused(crate::kernels::installed(), outcome_norm, outcome_norm, 0.0);
    if !prepared.fit_report_intercept() || prepared.fit_has_absorbed_intercept() {
        return Ok(uncentered);
    }
    let (cross, intercept_norm) = intercept_cross.ok_or_else(|| {
        invalid_argument("prepared OLS low-memory reported intercept was not generated")
    })?;
    if outcome_norm == 0.0 || intercept_norm == 0.0 {
        return Ok(uncentered);
    }
    let centered = crate::kernels::fused(
        crate::kernels::installed(),
        -cross / intercept_norm,
        cross / intercept_norm,
        uncentered,
    );
    if centered.is_finite() && centered >= -64.0 * f64::EPSILON * uncentered.max(1.0) {
        Ok(centered.max(0.0))
    } else {
        Err(invalid_data(
            "prepared OLS low-memory centered outcome sum of squares is invalid",
        ))
    }
}

#[cfg(test)]
fn fit_with_controls<R: FitReservation>(
    prepared: &PreparedOlsData,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturb_first_coefficient: bool,
    perturb_final_residual_constant: bool,
) -> Result<CertifiedPreparedOlsFit> {
    fit_with_controls_internal(
        prepared,
        config,
        reservation,
        FitPerturbations {
            first_coefficient: perturb_first_coefficient,
            final_residual_constant: perturb_final_residual_constant,
            ..FitPerturbations::default()
        },
    )
}

fn validate_config<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    config: PreparedOlsFitConfig,
) -> Result<()> {
    if prepared.fit_sample().retained_rows().is_empty() {
        return Err(insufficient_observations(
            "prepared OLS coefficient fitting requires a nonempty final sample",
        ));
    }
    let reported_intercepts = prepared
        .fit_model()
        .columns
        .iter()
        .filter(|column| matches!(column.kind, PreparedOlsColumnKind::Intercept))
        .count();
    if reported_intercepts != usize::from(prepared.fit_report_intercept())
        || (prepared.fit_report_intercept() && !prepared.fit_structural_intercept())
    {
        return Err(invalid_argument(
            "prepared OLS report and structural intercept metadata are inconsistent",
        ));
    }
    if !config.absorb.map.tolerance.is_finite()
        || config.absorb.map.tolerance <= 0.0
        || config.absorb.map.max_iterations == 0
        || config.absorb.map.check_every == 0
        || !config.rank_tolerance_multiplier.is_finite()
        || config.rank_tolerance_multiplier <= 0.0
        || !valid_unit_tolerance(config.fixed_effect_collinearity_tolerance)
        || config
            .coefficient_tolerance
            .is_some_and(|value| !valid_unit_tolerance(value))
        || !valid_unit_tolerance(config.score_tolerance)
        || !valid_unit_tolerance(config.bread_tolerance)
        || config
            .final_residual_tolerance
            .is_some_and(|value| !valid_unit_tolerance(value))
        || config.execution.poolsize == 0
    {
        return Err(invalid_argument(
            "prepared OLS coefficient-fit numerical policy is invalid",
        ));
    }
    let axis = prepared
        .fit_sample()
        .n_active()
        .max(prepared.fit_model().columns.len())
        .max(1);
    let relative = config.rank_tolerance_multiplier * f64::EPSILON * axis as f64;
    if !valid_unit_tolerance(relative) {
        return Err(invalid_argument(
            "prepared OLS dimension-adjusted rank tolerance is invalid",
        ));
    }
    Ok(())
}

/// Scalar reference retained only by the regression test for the shared
/// production moment dispatcher.
#[cfg(test)]
fn build_standardized_moments_columnwise<R: FitReservation>(
    within: &DenseMatrix,
    canonical_candidates: &[usize],
    reservation: &mut R,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let candidate_count = canonical_candidates.len();
    let mut standardized_rhs = reserved_filled(
        reservation,
        candidate_count,
        0.0_f64,
        "unable to reserve prepared OLS standardized RHS cross-products",
    )?;
    standardized_rhs.par_iter_mut().enumerate().try_for_each(
        |(position, destination)| -> Result<()> {
            let matrix_column = canonical_candidates[position]
                .checked_add(1)
                .ok_or_else(|| allocation("prepared OLS standardized RHS column overflows"))?;
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for row in 0..within.nrows() {
                compensated_add(
                    within[(row, matrix_column)] * within[(row, 0)],
                    &mut sum,
                    &mut correction,
                )?;
            }
            *destination = sum + correction;
            if destination.is_finite() {
                Ok(())
            } else {
                Err(invalid_data(
                    "prepared OLS standardized RHS cross-product is not finite",
                ))
            }
        },
    )?;
    let gram_elements = checked_square(
        candidate_count,
        "prepared OLS standardized Gram dimensions overflow usize",
    )?;
    let mut standardized_gram = reserved_filled(
        reservation,
        gram_elements,
        0.0_f64,
        "unable to reserve prepared OLS standardized Gram",
    )?;
    standardized_gram
        .par_chunks_mut(candidate_count)
        .enumerate()
        .try_for_each(|(left, gram_row)| -> Result<()> {
            for right in left..candidate_count {
                let left_column = canonical_candidates[left] + 1;
                let right_column = canonical_candidates[right] + 1;
                let mut sum = 0.0_f64;
                let mut correction = 0.0_f64;
                for row in 0..within.nrows() {
                    compensated_add(
                        within[(row, left_column)] * within[(row, right_column)],
                        &mut sum,
                        &mut correction,
                    )?;
                }
                let value = sum + correction;
                if !value.is_finite() {
                    return Err(invalid_data(
                        "prepared OLS standardized Gram entry is not finite",
                    ));
                }
                gram_row[right] = value;
            }
            Ok(())
        })?;
    for left in 0..candidate_count {
        for right in (left + 1)..candidate_count {
            standardized_gram[right * candidate_count + left] =
                standardized_gram[left * candidate_count + right];
        }
    }
    Ok((standardized_rhs, standardized_gram))
}

const fn valid_unit_tolerance(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < 1.0
}

fn normalized_weights<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
) -> Result<NormalizedWeights<'_>> {
    match &prepared.fit_sample().weights {
        PreparedWeights::None => Ok(NormalizedWeights {
            values: None,
            raw_scale: 1.0,
            kind: None,
        }),
        PreparedWeights::Provided { kind, values } => {
            if values.len() != prepared.fit_sample().n_active()
                || values
                    .iter()
                    .any(|weight| !weight.is_finite() || *weight <= 0.0)
            {
                return Err(invalid_data(
                    "prepared OLS coefficient-fit weights must be finite and positive",
                ));
            }
            let raw_scale = values.iter().copied().fold(0.0_f64, f64::max);
            if !raw_scale.is_finite()
                || raw_scale <= 0.0
                || values.iter().any(|weight| *weight / raw_scale == 0.0)
            {
                return Err(invalid_data(
                    "prepared OLS coefficient-fit weight normalization failed",
                ));
            }
            Ok(NormalizedWeights {
                values: Some(values),
                raw_scale,
                kind: Some(*kind),
            })
        }
    }
}

fn fast_qr_norms(reference: f64, within_norm: f64) -> Result<Option<(f64, f64)>> {
    if !reference.is_finite() || reference < 0.0 {
        return Err(invalid_data(
            "prepared OLS reference regressor norm is invalid",
        ));
    }
    if reference == 0.0 {
        return Ok(None);
    }
    // The caller computed these norms from the untouched within matrix
    // immediately before entering QR. Reuse that certified scale instead
    // of rescanning every candidate column here.
    let ratio = within_norm / reference;
    if !ratio.is_finite() {
        return Err(invalid_data(
            "prepared OLS fixed-effect collinearity ratio is not finite",
        ));
    }
    let squared_ratio = ratio * ratio;
    if !squared_ratio.is_finite() {
        return Err(invalid_data(
            "prepared OLS fixed-effect squared collinearity ratio is not finite",
        ));
    }
    Ok(Some((within_norm, squared_ratio)))
}

fn low_qr_norms(reference: f64, within_norm: f64) -> Result<Option<(f64, f64)>> {
    if !reference.is_finite() || reference < 0.0 || !within_norm.is_finite() || within_norm < 0.0 {
        return Err(invalid_data(
            "prepared OLS low-memory regressor norm is invalid",
        ));
    }
    if reference == 0.0 {
        return Ok(None);
    }
    let ratio = within_norm / reference;
    let squared_ratio = ratio * ratio;
    if !squared_ratio.is_finite() {
        return Err(invalid_data(
            "prepared OLS low-memory collinearity ratio is invalid",
        ));
    }
    Ok(Some((within_norm, squared_ratio)))
}

// The callers retain norm-validation precedence and allocation ownership.
// Classification itself depends only on the model and the validated scales.
fn classify_qr_candidates<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    statuses: &mut [PreparedOlsFitStatus],
    candidates: &mut Vec<usize>,
    tolerance: f64,
    capacity_error: &'static str,
    norms: impl Fn(usize) -> Result<Option<(f64, f64)>>,
) -> Result<()> {
    let has_absorbed_intercept = prepared.fit_has_absorbed_intercept();
    for (column, status) in statuses.iter_mut().enumerate() {
        let is_intercept = matches!(
            prepared.fit_model().columns[column].kind,
            PreparedOlsColumnKind::Intercept
        );
        if has_absorbed_intercept && is_intercept {
            *status = PreparedOlsFitStatus::AbsorbedInterceptDeferred;
            continue;
        }
        let Some((within_norm, squared_ratio)) = norms(column)? else {
            continue;
        };
        // Slope-only effects can annihilate a regressor exactly, including a
        // factor indicator supported on a retained physical singleton.
        if within_norm == 0.0 || (has_absorbed_intercept && squared_ratio <= tolerance) {
            *status = PreparedOlsFitStatus::CollinearWithFixedEffects;
        } else {
            push_reserved(candidates, column, capacity_error)?;
        }
    }
    Ok(())
}

pub(super) fn build_finite_score_draft(
    canonical_candidates: &[usize],
    standardized_gram: &[f64],
    standardized_rhs: &[f64],
    generated_coefficients: &[Option<f64>],
    generated_statuses: &[PreparedOlsFitStatus],
    reference_norms: &[f64],
    score_norms: &[f64],
) -> Result<FiniteScoreDraft> {
    let candidate_count = canonical_candidates.len();
    let candidate_square = checked_square(
        candidate_count,
        "prepared OLS finite-score Gram dimensions overflow usize",
    )?;
    if standardized_gram.len() != candidate_square
        || standardized_rhs.len() != candidate_count
        || generated_coefficients.len() != generated_statuses.len()
        || reference_norms.len() != generated_statuses.len()
        || score_norms.len() != generated_statuses.len()
    {
        return Err(invalid_argument(
            "prepared OLS finite-score dimensions are inconsistent",
        ));
    }

    for generated in canonical_candidates.iter().copied() {
        let status = generated_statuses[generated];
        if !matches!(
            status,
            PreparedOlsFitStatus::Kept | PreparedOlsFitStatus::NumericallyRankDeficient
        ) {
            return Err(invalid_argument(
                "prepared OLS finite-score candidate has a non-finite status",
            ));
        }
        let reference_norm = reference_norms[generated];
        let score_norm = score_norms[generated];
        if !reference_norm.is_finite()
            || reference_norm <= 0.0
            || !score_norm.is_finite()
            || score_norm <= 0.0
        {
            return Err(invalid_data(
                "prepared OLS finite-score regressor scale is not finite and positive",
            ));
        }
    }
    let certificate = certify_normal_equations(
        standardized_gram,
        standardized_rhs,
        NormalEquationScale::Prepared {
            generated_columns: canonical_candidates,
            generated_coefficients,
            reference_norms,
            score_norms,
        },
    )?;
    Ok(FiniteScoreDraft {
        infinity_scale: certificate.relative_infinity_norm,
        square_sum_scale: certificate.relative_square_sum,
        moments: certificate.moments,
    })
}

#[allow(clippy::too_many_arguments)]
fn finalize_finite_bread_draft<R: FitReservation>(
    canonical_candidates: &[usize],
    standardized_gram: &[f64],
    generated_columns: Vec<usize>,
    canonical_candidate_positions: Vec<usize>,
    mut canonical_inverse: Vec<f64>,
    reference_norms: &[f64],
    weight_scale: PreparedOlsBreadWeightScale,
    tolerance: f64,
    reservation: &mut R,
    perturb_bread: bool,
) -> Result<FiniteBreadDraft> {
    let candidate_count = canonical_candidates.len();
    let rank = generated_columns.len();
    let rank_square = checked_square(rank, "prepared OLS bread dimensions overflow usize")?;
    if standardized_gram.len()
        != checked_square(
            candidate_count,
            "prepared OLS standardized Gram dimensions overflow usize",
        )?
        || canonical_candidate_positions.len() != rank
        || canonical_inverse.len() != rank_square
        || generated_columns
            .iter()
            .any(|generated| *generated >= reference_norms.len())
        || canonical_candidate_positions
            .iter()
            .any(|position| *position >= candidate_count)
    {
        return Err(invalid_argument(
            "prepared OLS finalized bread inputs have inconsistent dimensions",
        ));
    }
    if perturb_bread && rank > 0 {
        canonical_inverse[0] += 0.25;
    }
    let left_inverse_relative_infinity_norm = inverse_relative_infinity_norm(
        standardized_gram,
        candidate_count,
        &canonical_candidate_positions,
        &canonical_inverse,
        rank,
        false,
    )?;
    let right_inverse_relative_infinity_norm = inverse_relative_infinity_norm(
        standardized_gram,
        candidate_count,
        &canonical_candidate_positions,
        &canonical_inverse,
        rank,
        true,
    )?;
    if left_inverse_relative_infinity_norm > tolerance
        || right_inverse_relative_infinity_norm > tolerance
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            format!(
                "prepared OLS bread inverse certificate did not pass (rank={rank}, left={left_inverse_relative_infinity_norm:e}, right={right_inverse_relative_infinity_norm:e}, tolerance={tolerance:e})"
            ),
        ));
    }

    let mut values = reserved_filled(
        reservation,
        rank_square,
        0.0_f64,
        "unable to reserve prepared OLS finite bread",
    )?;
    for left in 0..rank {
        let left_scale = reference_norms[generated_columns[left]];
        if !left_scale.is_finite() || left_scale <= 0.0 {
            return Err(invalid_data(
                "prepared OLS kept bread scale is not finite and positive",
            ));
        }
        for right in left..rank {
            let right_scale = reference_norms[generated_columns[right]];
            if !right_scale.is_finite() || right_scale <= 0.0 {
                return Err(invalid_data(
                    "prepared OLS kept bread scale is not finite and positive",
                ));
            }
            let value = canonical_inverse[left * rank + right]
                / left_scale
                / right_scale
                / weight_scale.reference_weight_multiplier;
            if !value.is_finite() || (left == right && value <= 0.0) {
                return Err(invalid_data(
                    "prepared OLS reference-scale finite bread is invalid",
                ));
            }
            values[left * rank + right] = value;
            values[right * rank + left] = value;
        }
    }

    // Reuse the now-dead canonical-inverse allocation for an independently
    // derived reference-scale finite cross-product. Keeping this pre-solve
    // Gram block permits the reported-coordinate certificate to check the
    // full intercept extension without allocating another O(K^2) buffer.
    let mut reference_crossproduct = canonical_inverse;
    for left in 0..rank {
        let left_scale = reference_norms[generated_columns[left]];
        for right in left..rank {
            let right_scale = reference_norms[generated_columns[right]];
            let value = standardized_gram[canonical_candidate_positions[left] * candidate_count
                + canonical_candidate_positions[right]]
                * left_scale
                * right_scale
                * weight_scale.reference_weight_multiplier;
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared OLS reference-scale finite cross-product is not finite",
                ));
            }
            reference_crossproduct[left * rank + right] = value;
            reference_crossproduct[right * rank + left] = value;
        }
    }

    Ok(FiniteBreadDraft {
        generated_columns,
        values,
        reference_crossproduct,
        weight_scale,
        left_inverse_relative_infinity_norm,
        right_inverse_relative_infinity_norm,
    })
}

fn inverse_relative_infinity_norm(
    gram: &[f64],
    gram_dimension: usize,
    gram_positions: &[usize],
    inverse: &[f64],
    rank: usize,
    reverse: bool,
) -> Result<f64> {
    kernel_inverse_relative_infinity_norm(
        gram,
        gram_dimension,
        gram_positions,
        inverse,
        rank,
        reverse,
    )
}

fn prepared_bread_weight_scale<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    weights: NormalizedWeights<'_>,
) -> Result<PreparedOlsBreadWeightScale> {
    let nrows = prepared.fit_sample().n_active();
    let nrows_f64 = exact_nonnegative_integer_as_f64(nrows as u128)?;
    let mut solver_sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for row in 0..nrows {
        compensated_add(weights.value(row)?, &mut solver_sum, &mut correction)?;
    }
    solver_sum += correction;
    if !solver_sum.is_finite() || solver_sum <= 0.0 {
        return Err(invalid_data(
            "prepared OLS solver weight sum is not finite and positive",
        ));
    }
    let (reference_weight_multiplier, reference_weight_sum) = match weights.kind {
        None => (1.0, nrows_f64),
        Some(WeightKind::Frequency) => {
            let PreparedWeights::Provided { values, .. } = &prepared.fit_sample().weights else {
                return Err(invalid_argument(
                    "prepared OLS frequency-weight scale has no weight column",
                ));
            };
            let mut exact_sum = 0_u128;
            for value in values {
                if value.fract() != 0.0 || *value <= 0.0 || *value > f64::from(u32::MAX) {
                    return Err(invalid_data(
                        "prepared OLS frequency weight is not a positive u32 integer",
                    ));
                }
                exact_sum = exact_sum
                    .checked_add(u128::from(*value as u32))
                    .ok_or_else(|| allocation("prepared OLS frequency-weight sum overflows"))?;
            }
            (
                weights.raw_scale,
                exact_nonnegative_integer_as_f64(exact_sum)?,
            )
        }
        Some(WeightKind::Analytic | WeightKind::Probability) => (nrows_f64 / solver_sum, nrows_f64),
        Some(WeightKind::Importance) => {
            return Err(invalid_argument(
                "prepared OLS importance weights are unsupported",
            ));
        }
    };
    if !weights.raw_scale.is_finite()
        || weights.raw_scale <= 0.0
        || !reference_weight_multiplier.is_finite()
        || reference_weight_multiplier <= 0.0
        || !reference_weight_sum.is_finite()
        || reference_weight_sum <= 0.0
    {
        return Err(invalid_data(
            "prepared OLS bread weight scale is not finite and positive",
        ));
    }
    Ok(PreparedOlsBreadWeightScale {
        weight_kind: weights.kind,
        solver_weight_divisor: weights.raw_scale,
        reference_weight_multiplier,
        reference_weight_sum,
    })
}

fn exact_nonnegative_integer_as_f64(value: u128) -> Result<f64> {
    const MAX_EXACT_F64_INTEGER: u128 = 1_u128 << 53;
    if value > MAX_EXACT_F64_INTEGER {
        return Err(invalid_data(
            "prepared OLS reference weight sum exceeds exact f64 integer range",
        ));
    }
    Ok(value as f64)
}

/// The independent raw residual offered to the reported-intercept finalizer.
/// Only a seed that the caller publishes is shifted by the reported intercept;
/// a mean-only vector is read for its weighted mean and then dropped.
enum RawResiduals<'a> {
    MeanOnly(&'a [f64]),
    Seed(&'a mut [f64]),
}

impl RawResiduals<'_> {
    fn values(&self) -> &[f64] {
        match self {
            Self::MeanOnly(values) => values,
            Self::Seed(values) => values,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn finalize_reported_bread_and_intercept<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    response: &[f64],
    qr: &mut FiniteFit,
    raw_residuals: Option<RawResiduals<'_>>,
    weights: NormalizedWeights<'_>,
    tolerance: f64,
    reservation: &mut R,
    perturbations: FitPerturbations,
    fast_score_cache: Option<&PreparedOlsFastScoreCache>,
    raw_moments: Option<&RawMomentCache>,
    raw_column_scales: Option<&[f64]>,
    current_frame_means: Option<CurrentFrameColumnMeans<'_>>,
    current_frame_predictor: Option<&[f64]>,
) -> Result<PreparedOlsBread> {
    if response.len() != prepared.fit_sample().n_active()
        || current_frame_predictor.is_some_and(|values| values.len() != response.len())
        || raw_residuals
            .as_ref()
            .is_some_and(|residuals| residuals.values().len() != prepared.fit_sample().n_active())
    {
        return Err(invalid_argument(
            "prepared OLS reported-intercept residual axis is inconsistent",
        ));
    }
    let finite = qr
        .bread
        .take()
        .ok_or_else(|| invalid_argument("prepared OLS finite bread was already consumed"))?;
    if finite.generated_columns.len() != qr.rank {
        return Err(invalid_argument(
            "prepared OLS finite bread coordinate count disagrees with rank",
        ));
    }
    let finite_square = checked_square(
        finite.generated_columns.len(),
        "prepared OLS finite bread dimensions overflow usize",
    )?;
    if finite.values.len() != finite_square || finite.reference_crossproduct.len() != finite_square
    {
        return Err(invalid_argument(
            "prepared OLS finite bread dimensions are inconsistent",
        ));
    }

    let mut deferred_intercept = None;
    for (generated, status) in qr.generated_statuses.iter().copied().enumerate() {
        if status != PreparedOlsFitStatus::AbsorbedInterceptDeferred {
            continue;
        }
        if deferred_intercept.replace(generated).is_some()
            || !prepared.fit_has_absorbed_intercept()
            || !matches!(
                prepared
                    .fit_model()
                    .columns
                    .get(generated)
                    .map(|column| &column.kind),
                Some(PreparedOlsColumnKind::Intercept)
            )
            || finite.generated_columns.contains(&generated)
        {
            return Err(invalid_argument(
                "prepared OLS deferred absorbed intercept is structurally inconsistent",
            ));
        }
    }

    let reported_dimension = finite
        .generated_columns
        .len()
        .checked_add(usize::from(deferred_intercept.is_some()))
        .ok_or_else(|| allocation("prepared OLS reported bread dimension overflows"))?;
    let reported_square = checked_square(
        reported_dimension,
        "prepared OLS reported bread dimensions overflow usize",
    )?;
    let mut coordinates = reserved_empty(
        reservation,
        reported_dimension,
        "unable to reserve prepared OLS reported bread coordinates",
    )?;
    for generated in finite.generated_columns.iter().copied() {
        let column = prepared
            .fit_model()
            .columns
            .get(generated)
            .ok_or_else(|| invalid_argument("prepared OLS bread coordinate is out of range"))?;
        push_reserved(
            &mut coordinates,
            PreparedOlsBreadCoordinate {
                coefficient_slot: column.coefficient_slot,
                generated_column_index: generated,
                kind: PreparedOlsBreadCoordinateKind::FiniteRegressor,
            },
            "prepared OLS reported bread-coordinate capacity exhausted",
        )?;
    }

    let mut values = if deferred_intercept.is_some() {
        let mut values = reserved_filled(
            reservation,
            reported_square,
            0.0_f64,
            "unable to reserve prepared OLS reported bread",
        )?;
        let finite_dimension = finite.generated_columns.len();
        for row in 0..finite_dimension {
            for column in 0..finite_dimension {
                values[row * reported_dimension + column] =
                    finite.values[row * finite_dimension + column];
            }
        }
        values
    } else {
        finite.values
    };

    let mut intercept_formula_relative_error = 0.0_f64;
    let mut intercept_normalization_relative_error = 0.0_f64;
    let mut reported_left_inverse_relative_infinity_norm =
        finite.left_inverse_relative_infinity_norm;
    let mut reported_right_inverse_relative_infinity_norm =
        finite.right_inverse_relative_infinity_norm;
    if let Some(intercept_generated) = deferred_intercept {
        let finite_dimension = finite.generated_columns.len();
        let mut means = reserved_filled(
            reservation,
            finite_dimension,
            0.0_f64,
            "unable to reserve prepared OLS reported-intercept means",
        )?;
        let numeric =
            if current_frame_means.is_none() && raw_moments.is_none() && fast_score_cache.is_none()
            {
                super::prepared_rhs::NumericOlsColumns::bind_score_means(
                    prepared.fit_model(),
                    prepared.fit_sample(),
                )?
            } else {
                None
            };
        if let Some(current) = current_frame_means {
            current.fill_selected(&finite.generated_columns, &mut means)?;
        } else if let Some(numeric) = numeric.as_ref() {
            fill_bound_weighted_column_means(
                prepared.fit_sample().n_active(),
                numeric.raw_columns(),
                &finite.generated_columns,
                weights,
                &mut means,
            )?;
        } else {
            for (position, generated) in finite.generated_columns.iter().copied().enumerate() {
                means[position] = if let Some(mean) =
                    raw_moments.and_then(|cache| cache.means.get(generated + 1))
                {
                    *mean
                } else if let Some(mean) = fast_score_cache.and_then(|cache| cache.mean(generated))
                {
                    mean
                } else {
                    weighted_mean_by(prepared.fit_sample().n_active(), weights, |row| {
                        prepared.fit_column_value(generated, row)
                    })?
                };
            }
        }

        // Fast mode already owns the reconstructed within residual from the QR
        // state. A caller-built raw residual (the low-memory seed, or the
        // dense route's independent vector) supplies the mean directly;
        // otherwise the mean is derived rowwise without retaining a vector.
        let raw_residual_mean = if let Some(residuals) = raw_residuals.as_ref() {
            weighted_mean_slice(residuals.values(), weights)?
        } else if let Some(predictor) = current_frame_predictor {
            weighted_mean_by(response.len(), weights, |row| {
                Ok(response[row] - predictor[row])
            })?
        } else if let Some(cache) = raw_moments {
            cached_raw_residual_mean(prepared, qr, weights, cache)?
        } else {
            let numeric = super::prepared_rhs::NumericOlsColumns::bind_residual_mean(
                prepared.fit_model(),
                prepared.fit_sample(),
            )?;
            // The already scanned raw column maxima bound the residual. As in
            // the raw-moment cache route, this removes a residual maximum pass
            // while retaining fresh rowwise residuals and compensated sums.
            let bound = raw_column_scales
                .filter(|scales| {
                    numeric.is_some() && scales.len() == qr.generated_coefficients.len()
                })
                .map(|scales| {
                    let mut scale = response
                        .iter()
                        .copied()
                        .map(f64::abs)
                        .fold(0.0_f64, f64::max);
                    for (coefficient, &column_scale) in qr.generated_coefficients.iter().zip(scales)
                    {
                        if let Some(coefficient) = coefficient {
                            scale += coefficient.abs() * column_scale;
                        }
                    }
                    scale
                })
                .filter(|scale| scale.is_finite());
            let residual = |row| {
                if let Some(columns) = numeric.as_ref() {
                    return columns.residual_at(&qr.generated_coefficients, row, response[row]);
                }
                let mut value = response[row];
                for (column, coefficient) in qr.generated_coefficients.iter().enumerate() {
                    if let Some(coefficient) = coefficient {
                        value =
                            (-*coefficient).mul_add(prepared.fit_column_value(column, row)?, value);
                    }
                }
                if value.is_finite() {
                    Ok(value)
                } else {
                    Err(invalid_data(
                        "prepared OLS raw regression residual is not finite",
                    ))
                }
            };
            if let Some(scale) = bound {
                weighted_mean_with_bound(
                    prepared.fit_sample().n_active(),
                    weights,
                    residual,
                    scale,
                )?
            } else {
                weighted_mean_by(prepared.fit_sample().n_active(), weights, residual)?
            }
        };
        let mut reported_intercept = raw_residual_mean;
        if perturbations.reported_intercept {
            reported_intercept += 0.25;
        }
        if !reported_intercept.is_finite() {
            return Err(invalid_data(
                "prepared OLS normalized reported intercept is not finite",
            ));
        }
        let outcome_mean = if let Some(cache) = raw_moments {
            cache.means[0]
        } else {
            weighted_mean_slice(response, weights)?
        };
        let mut finite_mean_fit = 0.0_f64;
        let mut finite_mean_fit_correction = 0.0_f64;
        for (position, generated) in finite.generated_columns.iter().copied().enumerate() {
            let coefficient = qr
                .generated_coefficients
                .get(generated)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    invalid_argument("prepared OLS bread coordinate has no coefficient")
                })?;
            compensated_add(
                means[position] * coefficient,
                &mut finite_mean_fit,
                &mut finite_mean_fit_correction,
            )?;
        }
        let reference_formula = outcome_mean - (finite_mean_fit + finite_mean_fit_correction);
        if !reference_formula.is_finite() {
            return Err(invalid_data(
                "prepared OLS reported-intercept formula is not finite",
            ));
        }
        intercept_formula_relative_error = relative_difference(
            reported_intercept,
            reference_formula,
            outcome_mean.abs().max(reported_intercept.abs()).max(1.0),
        )?;
        intercept_normalization_relative_error = relative_difference(
            reported_intercept,
            raw_residual_mean,
            raw_residual_mean
                .abs()
                .max(reported_intercept.abs())
                .max(1.0),
        )?;

        qr.generated_coefficients[intercept_generated] = Some(reported_intercept);
        qr.generated_statuses[intercept_generated] =
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept;
        qr.dropped
            .retain(|generated| *generated != intercept_generated);
        if let Some(RawResiduals::Seed(residuals)) = raw_residuals {
            for residual in residuals.iter_mut() {
                *residual -= reported_intercept;
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "prepared OLS reported-intercept residual is not finite",
                    ));
                }
            }
        }

        let intercept_position = finite_dimension;
        let intercept_column = prepared
            .fit_model()
            .columns
            .get(intercept_generated)
            .ok_or_else(|| invalid_argument("prepared OLS intercept column is out of range"))?;
        push_reserved(
            &mut coordinates,
            PreparedOlsBreadCoordinate {
                coefficient_slot: intercept_column.coefficient_slot,
                generated_column_index: intercept_generated,
                kind: PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept,
            },
            "prepared OLS reported bread-coordinate capacity exhausted",
        )?;

        let mut side = reserved_filled(
            reservation,
            finite_dimension,
            0.0_f64,
            "unable to reserve prepared OLS reported-intercept bread side",
        )?;
        for row in 0..finite_dimension {
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for column in 0..finite_dimension {
                let term = values[row * reported_dimension + column] * means[column];
                compensated_add(term, &mut sum, &mut correction)?;
            }
            side[row] = -(sum + correction);
            if !side[row].is_finite() {
                return Err(invalid_data(
                    "prepared OLS reported-intercept bread side is not finite",
                ));
            }
            values[row * reported_dimension + intercept_position] = side[row];
            values[intercept_position * reported_dimension + row] = side[row];
        }
        let mut quadratic = 0.0_f64;
        let mut quadratic_correction = 0.0_f64;
        for row in 0..finite_dimension {
            compensated_add(
                -means[row] * side[row],
                &mut quadratic,
                &mut quadratic_correction,
            )?;
        }
        let corner =
            finite.weight_scale.reference_weight_sum.recip() + quadratic + quadratic_correction;
        if !corner.is_finite() || corner <= 0.0 {
            return Err(invalid_data(
                "prepared OLS reported-intercept bread corner is invalid",
            ));
        }
        values[intercept_position * reported_dimension + intercept_position] = corner;
        if perturbations.reported_bread_extension {
            if finite_dimension == 0 {
                values[intercept_position * reported_dimension + intercept_position] += 0.25;
            } else {
                values[intercept_position] += 0.25;
                values[intercept_position * reported_dimension] += 0.25;
            }
        }
        reported_left_inverse_relative_infinity_norm = reported_inverse_relative_infinity_norm(
            &finite.reference_crossproduct,
            &means,
            finite.weight_scale.reference_weight_sum,
            &values,
            false,
        )?;
        reported_right_inverse_relative_infinity_norm = reported_inverse_relative_infinity_norm(
            &finite.reference_crossproduct,
            &means,
            finite.weight_scale.reference_weight_sum,
            &values,
            true,
        )?;
    } else if perturbations.reported_bread_extension {
        return Err(invalid_argument(
            "injected prepared OLS reported-bread extension requires an absorbed intercept",
        ));
    }

    if coordinates.len() != reported_dimension || values.len() != reported_square {
        return Err(invalid_argument(
            "prepared OLS reported bread final dimensions are inconsistent",
        ));
    }
    let symmetry_relative_infinity_norm =
        symmetry_relative_infinity_norm(&values, reported_dimension)?;
    let minimum_diagonal = if reported_dimension == 0 {
        0.0
    } else {
        (0..reported_dimension)
            .map(|index| values[index * reported_dimension + index])
            .fold(f64::INFINITY, f64::min)
    };
    let passed = finite.left_inverse_relative_infinity_norm <= tolerance
        && finite.right_inverse_relative_infinity_norm <= tolerance
        && reported_left_inverse_relative_infinity_norm <= tolerance
        && reported_right_inverse_relative_infinity_norm <= tolerance
        && symmetry_relative_infinity_norm <= tolerance
        && (reported_dimension == 0 || minimum_diagonal.is_finite() && minimum_diagonal > 0.0)
        && intercept_formula_relative_error <= tolerance
        && intercept_normalization_relative_error <= tolerance
        && values.iter().all(|value| value.is_finite());
    if !passed {
        return Err(invalid_data(
            "prepared OLS reported bread or intercept certificate did not pass",
        ));
    }
    Ok(PreparedOlsBread {
        coordinates,
        values,
        weight_scale: finite.weight_scale,
        certificate: PreparedOlsBreadCertificate {
            left_inverse_relative_infinity_norm: finite.left_inverse_relative_infinity_norm,
            right_inverse_relative_infinity_norm: finite.right_inverse_relative_infinity_norm,
            reported_left_inverse_relative_infinity_norm,
            reported_right_inverse_relative_infinity_norm,
            symmetry_relative_infinity_norm,
            minimum_diagonal,
            intercept_formula_relative_error,
            intercept_normalization_relative_error,
            passed,
        },
    })
}

fn reported_inverse_relative_infinity_norm(
    finite_crossproduct: &[f64],
    means: &[f64],
    reference_weight_sum: f64,
    inverse: &[f64],
    reverse: bool,
) -> Result<f64> {
    let finite_dimension = means.len();
    let finite_square = checked_square(
        finite_dimension,
        "prepared OLS reported inverse finite dimensions overflow usize",
    )?;
    let dimension = finite_dimension
        .checked_add(1)
        .ok_or_else(|| allocation("prepared OLS reported inverse dimension overflows usize"))?;
    let square = checked_square(
        dimension,
        "prepared OLS reported inverse dimensions overflow usize",
    )?;
    if finite_crossproduct.len() != finite_square
        || inverse.len() != square
        || !reference_weight_sum.is_finite()
        || reference_weight_sum <= 0.0
    {
        return Err(invalid_argument(
            "prepared OLS reported inverse-certificate inputs are inconsistent",
        ));
    }

    let crossproduct = |row: usize, column: usize| -> Result<f64> {
        let value = match (row < finite_dimension, column < finite_dimension) {
            (true, true) => {
                finite_crossproduct[row * finite_dimension + column]
                    + reference_weight_sum * means[row] * means[column]
            }
            (true, false) => reference_weight_sum * means[row],
            (false, true) => reference_weight_sum * means[column],
            (false, false) => reference_weight_sum,
        };
        if value.is_finite() {
            Ok(value)
        } else {
            Err(invalid_data(
                "prepared OLS reported cross-product is not finite",
            ))
        }
    };

    let mut maximum = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = 1.0_f64;
        for column in 0..dimension {
            let mut product = 0.0_f64;
            let mut correction = 0.0_f64;
            let mut absolute_products = 0.0_f64;
            for inner in 0..dimension {
                let (left, right) = if reverse {
                    (
                        inverse[row * dimension + inner],
                        crossproduct(inner, column)?,
                    )
                } else {
                    (
                        crossproduct(row, inner)?,
                        inverse[inner * dimension + column],
                    )
                };
                let term = left * right;
                compensated_add(term, &mut product, &mut correction)?;
                absolute_products += term.abs();
                if !absolute_products.is_finite() {
                    return Err(invalid_data(
                        "prepared OLS reported inverse-certificate scale is not finite",
                    ));
                }
            }
            row_error += (product + correction - f64::from(row == column)).abs();
            row_scale += absolute_products;
            if !row_error.is_finite() || !row_scale.is_finite() {
                return Err(invalid_data(
                    "prepared OLS reported inverse-certificate accumulation is not finite",
                ));
            }
        }
        maximum = maximum.max(row_error / row_scale);
    }
    if maximum.is_finite() {
        Ok(maximum)
    } else {
        Err(invalid_data(
            "prepared OLS reported inverse certificate is not finite",
        ))
    }
}

fn weighted_mean_slice(values: &[f64], weights: NormalizedWeights<'_>) -> Result<f64> {
    weighted_mean_by(values.len(), weights, |row| {
        values
            .get(row)
            .copied()
            .ok_or_else(|| invalid_argument("prepared OLS weighted-mean row is out of range"))
    })
}

fn fill_bound_weighted_column_means(
    nrows: usize,
    raw_columns: &[Option<&[f64]>],
    generated_columns: &[usize],
    weights: NormalizedWeights<'_>,
    means: &mut [f64],
) -> Result<()> {
    if means.len() != generated_columns.len()
        || generated_columns
            .iter()
            .any(|&column| column >= raw_columns.len())
        || raw_columns
            .iter()
            .flatten()
            .any(|source| source.len() != nrows)
    {
        return Err(invalid_argument(
            "bound weighted-mean columns are not aligned",
        ));
    }
    // Only immutable source binding is reused. Each column freshly runs the
    // original maximum-scaled, compensated mean under this weight frame, in
    // row order; disjoint destinations need no reduction or partial bank.
    let mean = |generated: usize| {
        let source = raw_columns[generated];
        weighted_mean_by(nrows, weights, |row| {
            Ok(source.map_or(1.0, |values| values[row]))
        })
    };
    if rayon::current_thread_index().is_some()
        && crate::parallel::StagePlan::current(nrows as u128 * means.len() as u128, means.len())
            .lanes
            > 1
    {
        crate::parallel::stage_try_for_each(
            means.par_iter_mut().zip(generated_columns.par_iter()),
            nrows as u128 * generated_columns.len() as u128,
            |(target, &generated)| {
                *target = mean(generated)?;
                Ok(())
            },
        )
    } else {
        for (target, &generated) in means.iter_mut().zip(generated_columns) {
            *target = mean(generated)?;
        }
        Ok(())
    }
}

fn build_raw_moment_cache<R: FitReservation>(
    raw: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    reservation: &mut R,
) -> Result<RawMomentCache> {
    if raw.nrows() == 0 || raw.ncols() == 0 {
        return Err(invalid_argument(
            "prepared OLS raw-moment cache requires a nonempty matrix",
        ));
    }
    let mut scales = reserved_filled(
        reservation,
        raw.ncols(),
        0.0_f64,
        "unable to reserve prepared OLS raw-moment scales",
    )?;
    let mut means = reserved_filled(
        reservation,
        raw.ncols(),
        0.0_f64,
        "unable to reserve prepared OLS raw-moment means",
    )?;
    let mut norms = reserved_filled(
        reservation,
        raw.ncols(),
        0.0_f64,
        "unable to reserve prepared OLS raw-moment norms",
    )?;
    let mut denominator = 0.0_f64;
    let mut denominator_correction = 0.0_f64;
    if weights.values.is_none() {
        denominator = raw.nrows() as f64;
    } else {
        for row in 0..raw.nrows() {
            compensated_add(
                weights.value(row)?,
                &mut denominator,
                &mut denominator_correction,
            )?;
        }
    }
    let weight_sum = denominator + denominator_correction;
    if !weight_sum.is_finite() || weight_sum <= 0.0 {
        return Err(invalid_data(
            "prepared OLS raw-moment weight sum is invalid",
        ));
    }
    if raw.ncols() <= FUSED_MOMENT_MAX_COLUMNS {
        fill_raw_moment_cache_fused(
            raw,
            weights,
            weight_sum,
            &mut scales,
            &mut means,
            &mut norms,
            reservation,
        )?;
    } else if raw.ncols().div_ceil(8) >= rayon::current_num_threads() {
        fill_raw_moment_cache_blocked::<8>(
            raw,
            weights,
            weight_sum,
            &mut scales,
            &mut means,
            &mut norms,
        )?;
    } else {
        // One column per task when eight-column blocks would idle threads.
        fill_raw_moment_cache_blocked::<1>(
            raw,
            weights,
            weight_sum,
            &mut scales,
            &mut means,
            &mut norms,
        )?;
    }
    Ok(RawMomentCache {
        means,
        scales,
        norms,
        weight_sum,
    })
}

fn fill_raw_moment_cache_fused<R: FitReservation>(
    raw: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    weight_sum: f64,
    scales: &mut [f64],
    means: &mut [f64],
    norms: &mut [f64],
    reservation: &mut R,
) -> Result<()> {
    let columns = raw.ncols();
    if columns == 0
        || columns > FUSED_MOMENT_MAX_COLUMNS
        || scales.len() != columns
        || means.len() != columns
        || norms.len() != columns
        || !weight_sum.is_finite()
        || weight_sum <= 0.0
    {
        return Err(invalid_argument(
            "prepared OLS fused raw-moment dimensions are inconsistent",
        ));
    }
    let partial_stride = columns
        .checked_mul(3)
        .ok_or_else(|| allocation("prepared OLS raw-moment partial stride overflows usize"))?;
    let chunks = raw
        .nrows()
        .div_ceil(PARALLEL_REDUCTION_CHUNK_ROWS)
        .clamp(1, FUSED_MOMENT_MAX_BLOCKS);
    let partial_elements = chunks
        .checked_mul(partial_stride)
        .ok_or_else(|| allocation("prepared OLS raw-moment partial allocation overflows usize"))?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve prepared OLS raw-moment partials",
    )?;
    crate::parallel::stage_try_for_each(
        partials.par_chunks_mut(partial_stride).enumerate(),
        raw.nrows() as u128 * columns as u128,
        |(chunk, partial)| -> Result<()> {
            let (partial_raw_scales, remainder) = partial.split_at_mut(columns);
            let (partial_norm_scales, partial_norm_sums) = remainder.split_at_mut(columns);
            partial_norm_sums.fill(1.0);
            let rows_per_chunk = raw.nrows() / chunks;
            let remainder = raw.nrows() % chunks;
            let start = chunk * rows_per_chunk + chunk.min(remainder);
            let end = start + rows_per_chunk + usize::from(chunk < remainder);
            for row in start..end {
                let root_weight = weights.root(row)?;
                let values = &raw.as_slice()[row * columns..(row + 1) * columns];
                for column in 0..columns {
                    let value = values[column];
                    if !value.is_finite() {
                        return Err(invalid_data("prepared OLS raw-moment value is not finite"));
                    }
                    partial_raw_scales[column] = partial_raw_scales[column].max(value.abs());
                    update_scaled_sum_of_squares(
                        root_weight * value,
                        &mut partial_norm_scales[column],
                        &mut partial_norm_sums[column],
                    )?;
                }
            }
            Ok(())
        },
    )?;
    for column in 0..columns {
        let mut norm_scale = 0.0_f64;
        let mut norm_sum = 1.0_f64;
        let mut scale = 0.0_f64;
        for partial in partials.chunks_exact(partial_stride) {
            scale = scale.max(partial[column]);
            merge_scaled_sum_of_squares(
                partial[columns + column],
                partial[2 * columns + column],
                &mut norm_scale,
                &mut norm_sum,
            )?;
        }
        scales[column] = scale;
        norms[column] = checked_norm(norm_scale, norm_sum)?;
    }

    partials.fill(0.0);
    crate::parallel::stage_try_for_each(
        partials.par_chunks_mut(partial_stride).enumerate(),
        raw.nrows() as u128 * columns as u128,
        |(chunk, partial)| -> Result<()> {
            let (partial_means, partial_corrections) = partial.split_at_mut(columns);
            let rows_per_chunk = raw.nrows() / chunks;
            let remainder = raw.nrows() % chunks;
            let start = chunk * rows_per_chunk + chunk.min(remainder);
            let end = start + rows_per_chunk + usize::from(chunk < remainder);
            for row in start..end {
                let weight = weights.value(row)?;
                let values = &raw.as_slice()[row * columns..(row + 1) * columns];
                for column in 0..columns {
                    if scales[column] != 0.0 {
                        compensated_add(
                            weight * (values[column] / scales[column]),
                            &mut partial_means[column],
                            &mut partial_corrections[column],
                        )?;
                    }
                }
            }
            Ok(())
        },
    )?;
    for column in 0..columns {
        let mut sum = 0.0_f64;
        let mut correction = 0.0_f64;
        for partial in partials.chunks_exact(partial_stride) {
            compensated_add(
                partial[column] + partial[columns + column],
                &mut sum,
                &mut correction,
            )?;
        }
        means[column] = if scales[column] == 0.0 {
            0.0
        } else {
            scales[column] * ((sum + correction) / weight_sum)
        };
        if !means[column].is_finite() {
            return Err(invalid_data("prepared OLS raw-moment mean is not finite"));
        }
    }
    Ok(())
}

/// Keep the wide columnwise cache's arithmetic and row order, but share adjacent
/// loads and weight reads. Stack accumulators avoid both a partial bank and
/// repeated writes to cache lines shared by neighboring output chunks.
fn fill_raw_moment_cache_blocked<const BLOCK: usize>(
    raw: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    weight_sum: f64,
    scales: &mut [f64],
    means: &mut [f64],
    norms: &mut [f64],
) -> Result<()> {
    if BLOCK == 0
        || raw.ncols() == 0
        || scales.len() != raw.ncols()
        || means.len() != raw.ncols()
        || norms.len() != raw.ncols()
        || !weight_sum.is_finite()
        || weight_sum <= 0.0
    {
        return Err(invalid_argument(
            "prepared OLS blocked raw-moment dimensions are inconsistent",
        ));
    }
    crate::parallel::stage_try_for_each(
        scales
            .par_chunks_mut(BLOCK)
            .zip(means.par_chunks_mut(BLOCK))
            .zip(norms.par_chunks_mut(BLOCK))
            .enumerate(),
        raw.nrows() as u128 * raw.ncols() as u128,
        |(block, ((scales, means), norms))| -> Result<()> {
            let width = scales.len();
            let start = block * BLOCK;
            let mut raw_scales = [0.0_f64; BLOCK];
            raw_scales[..width].copy_from_slice(scales);
            let mut norm_scales = [0.0_f64; BLOCK];
            let mut norm_sums = [1.0_f64; BLOCK];
            for (row, values) in raw.as_slice().chunks_exact(raw.ncols()).enumerate() {
                let root = weights.root(row)?;
                for column in 0..width {
                    let value = values[start + column];
                    if !value.is_finite() {
                        return Err(invalid_data("prepared OLS raw-moment value is not finite"));
                    }
                    raw_scales[column] = raw_scales[column].max(value.abs());
                    update_scaled_sum_of_squares(
                        root * value,
                        &mut norm_scales[column],
                        &mut norm_sums[column],
                    )?;
                }
            }
            let mut sums = [0.0_f64; BLOCK];
            let mut corrections = [0.0_f64; BLOCK];
            for (row, values) in raw.as_slice().chunks_exact(raw.ncols()).enumerate() {
                let weight = weights.value(row)?;
                for column in 0..width {
                    if raw_scales[column] != 0.0 {
                        compensated_add(
                            weight * (values[start + column] / raw_scales[column]),
                            &mut sums[column],
                            &mut corrections[column],
                        )?;
                    }
                }
            }
            for column in 0..width {
                scales[column] = raw_scales[column];
                means[column] = if raw_scales[column] == 0.0 {
                    0.0
                } else {
                    raw_scales[column] * ((sums[column] + corrections[column]) / weight_sum)
                };
                if !means[column].is_finite() {
                    return Err(invalid_data("prepared OLS raw-moment mean is not finite"));
                }
                norms[column] = checked_norm(norm_scales[column], norm_sums[column])?;
            }
            Ok(())
        },
    )
}

fn cached_raw_residual_mean<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    qr: &FiniteFit,
    weights: NormalizedWeights<'_>,
    cache: &RawMomentCache,
) -> Result<f64> {
    let generated = prepared.fit_model().columns.len();
    if cache.means.len() != generated + 1
        || cache.scales.len() != generated + 1
        || cache.norms.len() != generated + 1
    {
        return Err(invalid_argument(
            "prepared OLS raw-moment cache dimensions are inconsistent",
        ));
    }
    let mut scale = cache.scales[0];
    for (column, coefficient) in qr.generated_coefficients.iter().enumerate() {
        if let Some(coefficient) = coefficient {
            scale += coefficient.abs() * cache.scales[column + 1];
        }
    }
    if !scale.is_finite() {
        return weighted_mean_by(prepared.fit_sample().n_active(), weights, |row| {
            raw_residual_at(prepared, qr, row)
        });
    }
    if scale == 0.0 {
        return Ok(0.0);
    }
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for row in 0..prepared.fit_sample().n_active() {
        compensated_add(
            weights.value(row)? * (raw_residual_at(prepared, qr, row)? / scale),
            &mut sum,
            &mut correction,
        )?;
    }
    let mean = scale * ((sum + correction) / cache.weight_sum);
    if mean.is_finite() {
        Ok(mean)
    } else {
        Err(invalid_data(
            "prepared OLS cached raw-residual mean is not finite",
        ))
    }
}

fn raw_residual_at<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    qr: &FiniteFit,
    row: usize,
) -> Result<f64> {
    let mut value = *prepared
        .fit_sample()
        .outcome
        .get(row)
        .ok_or_else(|| invalid_argument("prepared OLS raw-residual row is out of range"))?;
    for (column, coefficient) in qr.generated_coefficients.iter().enumerate() {
        if let Some(coefficient) = coefficient {
            value = (-*coefficient).mul_add(prepared.fit_column_value(column, row)?, value);
        }
    }
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid_data(
            "prepared OLS raw regression residual is not finite",
        ))
    }
}

fn weighted_mean_by<F>(nrows: usize, weights: NormalizedWeights<'_>, mut value: F) -> Result<f64>
where
    F: FnMut(usize) -> Result<f64>,
{
    if nrows == 0 {
        return Err(invalid_argument(
            "prepared OLS weighted mean requires a nonempty sample",
        ));
    }
    let mut value_scale = 0.0_f64;
    for row in 0..nrows {
        let current = value(row)?;
        if !current.is_finite() {
            return Err(invalid_data(
                "prepared OLS weighted-mean value is not finite",
            ));
        }
        value_scale = value_scale.max(current.abs());
    }
    weighted_mean_with_scale(nrows, weights, value, value_scale, None)
}

fn weighted_mean_with_bound<F>(
    nrows: usize,
    weights: NormalizedWeights<'_>,
    mut value: F,
    bound: f64,
) -> Result<f64>
where
    F: FnMut(usize) -> Result<f64>,
{
    let mut needs_rescale = false;
    let mean =
        weighted_mean_with_scale(nrows, weights, &mut value, bound, Some(&mut needs_rescale))?;
    // Cancellation can make a valid upper bound much larger than the actual
    // residual. Recover the original maximum scan if scaling loses precision
    // to subnormal arithmetic; the optimization never just discards that tail.
    if needs_rescale {
        weighted_mean_by(nrows, weights, value)
    } else {
        Ok(mean)
    }
}

fn weighted_mean_with_scale<F>(
    nrows: usize,
    weights: NormalizedWeights<'_>,
    mut value: F,
    value_scale: f64,
    mut underflow: Option<&mut bool>,
) -> Result<f64>
where
    F: FnMut(usize) -> Result<f64>,
{
    if value_scale == 0.0 {
        return Ok(0.0);
    }
    let mut numerator = 0.0_f64;
    let mut numerator_correction = 0.0_f64;
    // Unit mass sums exactly to N through 2^53; keep the value reduction's
    // scale, compensation and observation order unchanged.
    let denominator = if weights.values.is_none() && (nrows as u128) <= (1_u128 << 53) {
        for row in 0..nrows {
            let current = value(row)?;
            let scaled = current / value_scale;
            if let Some(flag) = underflow.as_deref_mut() {
                *flag |= current != 0.0 && scaled.abs() < f64::MIN_POSITIVE;
            }
            compensated_add(scaled, &mut numerator, &mut numerator_correction)?;
        }
        nrows as f64
    } else {
        let mut denominator = 0.0_f64;
        let mut denominator_correction = 0.0_f64;
        for row in 0..nrows {
            let weight = weights.value(row)?;
            let current = value(row)?;
            let scaled = weight * (current / value_scale);
            if let Some(flag) = underflow.as_deref_mut() {
                *flag |= current != 0.0 && scaled.abs() < f64::MIN_POSITIVE;
            }
            compensated_add(scaled, &mut numerator, &mut numerator_correction)?;
            compensated_add(weight, &mut denominator, &mut denominator_correction)?;
        }
        denominator + denominator_correction
    };
    let numerator = numerator + numerator_correction;
    let scaled_mean = numerator / denominator;
    if let Some(flag) = underflow {
        *flag |= numerator != 0.0 && scaled_mean.abs() < f64::MIN_POSITIVE;
    }
    let mean = value_scale * scaled_mean;
    if mean.is_finite() && denominator.is_finite() && denominator > 0.0 {
        Ok(mean)
    } else {
        Err(invalid_data("prepared OLS weighted mean is not finite"))
    }
}

fn relative_difference(actual: f64, expected: f64, scale: f64) -> Result<f64> {
    if !actual.is_finite() || !expected.is_finite() || !scale.is_finite() || scale <= 0.0 {
        return Err(invalid_data(
            "prepared OLS relative certificate input is invalid",
        ));
    }
    let relative = (actual - expected).abs() / scale;
    if relative.is_finite() {
        Ok(relative)
    } else {
        Err(invalid_data(
            "prepared OLS relative certificate is not finite",
        ))
    }
}

fn symmetry_relative_infinity_norm(values: &[f64], dimension: usize) -> Result<f64> {
    let square = checked_square(
        dimension,
        "prepared OLS symmetry-certificate dimensions overflow usize",
    )?;
    if values.len() != square {
        return Err(invalid_argument(
            "prepared OLS symmetry-certificate dimensions are inconsistent",
        ));
    }
    let mut maximum = 0.0_f64;
    for row in 0..dimension {
        let mut error = 0.0_f64;
        let mut scale = 1.0_f64;
        for column in 0..dimension {
            let value = values[row * dimension + column];
            let transpose = values[column * dimension + row];
            if !value.is_finite() || !transpose.is_finite() {
                return Err(invalid_data(
                    "prepared OLS bread symmetry input is not finite",
                ));
            }
            error += (value - transpose).abs();
            scale += value.abs().max(transpose.abs());
        }
        maximum = maximum.max(error / scale);
    }
    Ok(maximum)
}

/// QR reflector norms need accurate tail energy even when each squared term is
/// below one ulp of a large leading term. A fixed scale and compensated sum
/// avoid N-dependent loss; the pre-absorption reference-norm policy is separate.
fn weighted_matrix_column_norm(
    matrix: &DenseMatrix,
    column: usize,
    weights: NormalizedWeights<'_>,
) -> Result<f64> {
    if column >= matrix.ncols() {
        return Err(invalid_argument(
            "prepared OLS weighted matrix column is out of range",
        ));
    }
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for row in 0..matrix.nrows() {
        let value = weights.root(row)? * matrix[(row, column)];
        update_scaled_sum_of_squares(value, &mut scale, &mut scaled_square_sum)?;
    }
    checked_norm(scale, scaled_square_sum)
}

/// Compute every column norm in one row-major pass.  The update order within
/// each column is identical to `weighted_matrix_column_norm`; only the loop
/// nesting changes.  This matters for the single-threaded wide-control path,
/// where separate column scans repeatedly traverse the row-major matrix with a
/// large stride and recompute the same normalized weight root.
fn fill_weighted_matrix_norms_fused<R: FitReservation>(
    matrix: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    output: &mut [f64],
    reservation: &mut R,
) -> Result<()> {
    if output.len() != matrix.ncols() {
        return Err(invalid_argument(
            "prepared OLS fused weighted norm output has the wrong width",
        ));
    }
    output.fill(0.0);
    let mut scaled_square_sums = reserved_filled(
        reservation,
        matrix.ncols(),
        1.0_f64,
        "unable to reserve prepared OLS fused norm workspace",
    )?;
    for (row, values) in matrix.as_slice().chunks_exact(matrix.ncols()).enumerate() {
        let root = weights.root(row)?;
        for ((value, scale), scaled_square_sum) in values
            .iter()
            .copied()
            .zip(output.iter_mut())
            .zip(scaled_square_sums.iter_mut())
        {
            update_scaled_sum_of_squares(root * value, scale, scaled_square_sum)?;
        }
    }
    for (scale, scaled_square_sum) in output.iter_mut().zip(scaled_square_sums) {
        *scale = checked_norm(*scale, scaled_square_sum)?;
    }
    Ok(())
}

/// Each column keeps the scalar norm's row order and arithmetic. Small column
/// blocks share weight roots and adjacent matrix loads without a row reduction
/// or an observation-sized transpose; all scratch is fixed-size worker stack.
/// `output[j]` is the norm of matrix column `first_column + j`.
fn fill_weighted_matrix_norms_blocked<const BLOCK: usize>(
    matrix: &DenseMatrix,
    first_column: usize,
    weights: NormalizedWeights<'_>,
    output: &mut [f64],
) -> Result<()> {
    if BLOCK == 0 || first_column > matrix.ncols() || output.len() != matrix.ncols() - first_column
    {
        return Err(invalid_argument(
            "prepared OLS blocked weighted norm output has the wrong width",
        ));
    }
    let work = matrix.nrows() as u128 * output.len() as u128;
    crate::parallel::stage_try_for_each(
        output.par_chunks_mut(BLOCK).enumerate(),
        work,
        |(block, norms)| -> Result<()> {
            // Output chunks need not be cache-line aligned. Accumulate locally
            // so adjacent workers do not repeatedly write a shared cache line.
            let mut scales = [0.0_f64; BLOCK];
            let mut square_sums = [1.0_f64; BLOCK];
            let start = first_column + block * BLOCK;
            for (row, values) in matrix.as_slice().chunks_exact(matrix.ncols()).enumerate() {
                let root = weights.root(row)?;
                for (column, scale) in scales.iter_mut().enumerate().take(norms.len()) {
                    update_scaled_sum_of_squares(
                        root * values[start + column],
                        scale,
                        &mut square_sums[column],
                    )?;
                }
            }
            for ((norm, scale), square_sum) in norms.iter_mut().zip(scales).zip(square_sums) {
                *norm = checked_norm(scale, square_sum)?;
            }
            Ok(())
        },
    )
}

/// Raise `output[j]` to the largest absolute value of matrix column
/// `first_column + j`, streaming rows once per block of columns. A maximum is
/// exact, so the result equals any per-column scan from the same start.
fn fill_column_maxima_blocked<const BLOCK: usize>(
    matrix: &DenseMatrix,
    first_column: usize,
    output: &mut [f64],
) -> Result<()> {
    if BLOCK == 0 || first_column > matrix.ncols() || output.len() != matrix.ncols() - first_column
    {
        return Err(invalid_argument(
            "prepared OLS blocked column maxima have the wrong width",
        ));
    }
    let work = matrix.nrows() as u128 * output.len() as u128;
    crate::parallel::stage_try_for_each(
        output.par_chunks_mut(BLOCK).enumerate(),
        work,
        |(block, maxima)| -> Result<()> {
            let mut local = [0.0_f64; BLOCK];
            local[..maxima.len()].copy_from_slice(maxima);
            let start = first_column + block * BLOCK;
            for values in matrix.as_slice().chunks_exact(matrix.ncols()) {
                for (maximum, value) in local[..maxima.len()]
                    .iter_mut()
                    .zip(&values[start..start + maxima.len()])
                {
                    *maximum = maximum.max(value.abs());
                }
            }
            maxima.copy_from_slice(&local[..maxima.len()]);
            Ok(())
        },
    )
}

fn initial_within_sum_of_squares<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    within: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    outcome_norm: f64,
) -> Result<f64> {
    let uncentered =
        crate::kernels::fused(crate::kernels::installed(), outcome_norm, outcome_norm, 0.0);
    if !prepared.fit_report_intercept() || prepared.fit_has_absorbed_intercept() {
        return Ok(uncentered);
    }
    let intercept = prepared
        .fit_model()
        .columns
        .iter()
        .position(|column| matches!(column.kind, PreparedOlsColumnKind::Intercept))
        .ok_or_else(|| invalid_argument("prepared OLS ordinary intercept column is absent"))?;
    let intercept_column = intercept
        .checked_add(1)
        .ok_or_else(|| allocation("prepared OLS intercept RHS column overflows"))?;
    let intercept_norm = weighted_matrix_column_norm(within, intercept_column, weights)?;
    if outcome_norm == 0.0 || intercept_norm == 0.0 {
        return Ok(uncentered);
    }

    let mut normalized_cross = 0.0_f64;
    let mut cross_correction = 0.0_f64;
    for row in 0..within.nrows() {
        let weight = weights.value(row)?;
        let term = weight
            * (within[(row, 0)] / outcome_norm)
            * (within[(row, intercept_column)] / intercept_norm);
        compensated_add(term, &mut normalized_cross, &mut cross_correction)?;
    }
    let coefficient = (outcome_norm / intercept_norm) * (normalized_cross + cross_correction);
    if !coefficient.is_finite() {
        return Err(invalid_data(
            "prepared OLS ordinary-intercept baseline coefficient is not finite",
        ));
    }
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for row in 0..within.nrows() {
        let residual = (-coefficient).mul_add(within[(row, intercept_column)], within[(row, 0)]);
        update_scaled_sum_of_squares(
            weights.root(row)? * residual,
            &mut scale,
            &mut scaled_square_sum,
        )?;
    }
    let norm = checked_norm(scale, scaled_square_sum)?;
    let sum_of_squares = crate::kernels::fused(crate::kernels::installed(), norm, norm, 0.0);
    if sum_of_squares.is_finite() && sum_of_squares >= 0.0 {
        Ok(sum_of_squares)
    } else {
        Err(invalid_data(
            "prepared OLS ordinary-intercept baseline sum of squares is invalid",
        ))
    }
}

fn weighted_slice_norm(values: &[f64], weights: NormalizedWeights<'_>) -> Result<f64> {
    if crate::parallel::StagePlan::current(values.len() as u128, values.len()).lanes > 1 {
        let chunks = values.len().div_ceil(PARALLEL_REDUCTION_CHUNK_ROWS);
        let mut scales = Vec::new();
        scales
            .try_reserve_exact(chunks)
            .map_err(|_| allocation("unable to reserve parallel norm scales"))?;
        scales.resize(chunks, 0.0_f64);
        let mut sums = Vec::new();
        sums.try_reserve_exact(chunks)
            .map_err(|_| allocation("unable to reserve parallel norm sums"))?;
        sums.resize(chunks, 1.0_f64);
        crate::parallel::stage_try_for_each(
            scales.par_iter_mut().zip(sums.par_iter_mut()).enumerate(),
            values.len() as u128,
            |(chunk, (scale, sum))| -> Result<()> {
                let start = chunk * PARALLEL_REDUCTION_CHUNK_ROWS;
                let end = (start + PARALLEL_REDUCTION_CHUNK_ROWS).min(values.len());
                for (offset, value) in values[start..end].iter().copied().enumerate() {
                    update_scaled_sum_of_squares(
                        weights.root(start + offset)? * value,
                        scale,
                        sum,
                    )?;
                }
                Ok(())
            },
        )?;
        let mut scale = 0.0_f64;
        let mut scaled_square_sum = 1.0_f64;
        for (chunk_scale, chunk_sum) in scales.into_iter().zip(sums) {
            merge_scaled_sum_of_squares(
                chunk_scale,
                chunk_sum,
                &mut scale,
                &mut scaled_square_sum,
            )?;
        }
        return checked_norm(scale, scaled_square_sum);
    }
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for (row, value) in values.iter().copied().enumerate() {
        update_scaled_sum_of_squares(
            weights.root(row)? * value,
            &mut scale,
            &mut scaled_square_sum,
        )?;
    }
    checked_norm(scale, scaled_square_sum)
}

fn merge_scaled_sum_of_squares(
    source_scale: f64,
    source_sum: f64,
    target_scale: &mut f64,
    target_sum: &mut f64,
) -> Result<()> {
    if !source_scale.is_finite() || !source_sum.is_finite() || source_sum < 0.0 {
        return Err(invalid_data(
            "prepared OLS parallel norm partial is invalid",
        ));
    }
    if source_scale == 0.0 {
        return Ok(());
    }
    if *target_scale < source_scale {
        let ratio = *target_scale / source_scale;
        *target_sum = source_sum + *target_sum * ratio * ratio;
        *target_scale = source_scale;
    } else {
        let ratio = source_scale / *target_scale;
        *target_sum += source_sum * ratio * ratio;
    }
    if target_scale.is_finite() && target_sum.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "prepared OLS parallel norm accumulation is invalid",
        ))
    }
}

fn update_scaled_sum_of_squares(value: f64, scale: &mut f64, sum: &mut f64) -> Result<()> {
    if !value.is_finite() {
        return Err(invalid_data(
            "prepared OLS weighted norm value is not finite",
        ));
    }
    let absolute = value.abs();
    if absolute == 0.0 {
        return Ok(());
    }
    if *scale < absolute {
        let ratio = *scale / absolute;
        *sum = 1.0 + *sum * ratio * ratio;
        *scale = absolute;
    } else {
        let ratio = absolute / *scale;
        *sum += ratio * ratio;
    }
    if scale.is_finite() && sum.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "prepared OLS weighted norm accumulation is not finite",
        ))
    }
}

fn checked_norm(scale: f64, scaled_square_sum: f64) -> Result<f64> {
    let norm = if scale == 0.0 {
        0.0
    } else {
        scale * scaled_square_sum.sqrt()
    };
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(invalid_data("prepared OLS weighted norm is not finite"))
    }
}

#[allow(clippy::too_many_arguments)]
fn certify_scores<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    residuals: &[f64],
    weights: NormalizedWeights<'_>,
    reference_norms: &[f64],
    score_norms: &[f64],
    finite_score: FiniteScoreDraft,
    statuses: &[PreparedOlsFitStatus],
    within_outcome_norm: f64,
    tolerance: f64,
) -> Result<PreparedOlsScoreCertificate> {
    if residuals.len() != prepared.fit_sample().n_active()
        || reference_norms.len() != prepared.fit_model().columns.len()
        || score_norms.len() != prepared.fit_model().columns.len()
        || statuses.len() != prepared.fit_model().columns.len()
        || !within_outcome_norm.is_finite()
        || within_outcome_norm < 0.0
    {
        return Err(invalid_argument(
            "prepared OLS score-certificate dimensions are inconsistent",
        ));
    }
    let residual_norm = weighted_slice_norm(residuals, weights)?;
    let response_scale = within_outcome_norm.max(residual_norm);
    let zero_response = response_scale == 0.0;
    let (mut relative_infinity_norm, finite_relative_two_norm) = if zero_response {
        if finite_score.infinity_scale == 0.0 && finite_score.square_sum_scale == 0.0 {
            (0.0, 0.0)
        } else {
            return Err(invalid_data(
                "prepared OLS finite score is nonzero for an exact-zero response",
            ));
        }
    } else {
        (
            finite_score.infinity_scale / response_scale,
            finite_score.square_sum_scale.sqrt() / response_scale,
        )
    };
    let hardware = crate::kernels::installed();
    let mut relative_square_sum = finite_relative_two_norm * finite_relative_two_norm;
    let mut moments = finite_score.moments;
    if !relative_infinity_norm.is_finite() || !relative_square_sum.is_finite() {
        return Err(invalid_data(
            "prepared OLS finite-score response normalization is not finite",
        ));
    }
    for (column, ((reference_norm, score_norm), status)) in reference_norms
        .iter()
        .copied()
        .zip(score_norms.iter().copied())
        .zip(statuses.iter().copied())
        .enumerate()
    {
        if !reference_norm.is_finite()
            || reference_norm < 0.0
            || !score_norm.is_finite()
            || score_norm < 0.0
        {
            return Err(invalid_data(
                "prepared OLS within regressor score norm is invalid",
            ));
        }
        let Some(denominator) = score_denominator(status, score_norm, reference_norm) else {
            continue;
        };
        let relative = if denominator == 0.0 || zero_response {
            0.0
        } else {
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            if weights.values.is_none()
                && matches!(
                    prepared.fit_model().columns[column].kind,
                    PreparedOlsColumnKind::Intercept
                )
            {
                // Preserve the independently scaled compensated score, while
                // hoisting the unit intercept and weight out of the row loop.
                let left = 1.0 / denominator;
                for residual in residuals.iter().copied() {
                    compensated_add(
                        left * (residual / response_scale),
                        &mut sum,
                        &mut correction,
                    )?;
                }
            } else {
                for (row, residual) in residuals.iter().copied().enumerate() {
                    let root_weight = weights.root(row)?;
                    let regressor = prepared.fit_column_value(column, row)?;
                    let left = (root_weight * regressor) / denominator;
                    let right = (root_weight * residual) / response_scale;
                    let term = left * right;
                    compensated_add(term, &mut sum, &mut correction)?;
                }
            }
            (sum + correction).abs()
        };
        if !relative.is_finite() {
            return Err(invalid_data("prepared OLS normalized score is not finite"));
        }
        relative_infinity_norm = relative_infinity_norm.max(relative);
        relative_square_sum =
            crate::kernels::fused(hardware, relative, relative, relative_square_sum);
        if !relative_square_sum.is_finite() {
            return Err(invalid_data(
                "prepared OLS score two-norm accumulation is not finite",
            ));
        }
        moments = moments
            .checked_add(1)
            .ok_or_else(|| allocation("prepared OLS score moment count overflows"))?;
    }
    let relative_two_norm = if moments == 0 {
        0.0
    } else {
        (relative_square_sum / moments as f64).sqrt()
    };
    if !relative_two_norm.is_finite() {
        return Err(invalid_data("prepared OLS score two norm is not finite"));
    }
    Ok(PreparedOlsScoreCertificate {
        relative_infinity_norm,
        relative_two_norm,
        moments,
        passed: relative_infinity_norm <= tolerance,
    })
}

/// Scale for one column's raw score moment, or `None` when the column has no
/// finite score equation to check.
///
/// A coordinate certified as lying in the absorbed span has no finite score
/// equation: its normal equation is already covered by the independent FE
/// certificate. Do not divide its MAP-noise within norm into a manufactured
/// order-one moment. An absorbed intercept, whether normalized for publication
/// or still deferred (projection-only fits skip normalization), keeps its raw
/// score moment, but that moment uses the raw intercept norm instead of a
/// numerically nonzero MAP-noise within norm. Finite rank-deficient
/// coordinates remain checked on their within scale.
fn score_denominator(
    status: PreparedOlsFitStatus,
    score_norm: f64,
    reference_norm: f64,
) -> Option<f64> {
    match status {
        PreparedOlsFitStatus::Kept
        | PreparedOlsFitStatus::NumericallyRankDeficient
        | PreparedOlsFitStatus::CollinearWithFixedEffects => None,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        | PreparedOlsFitStatus::AbsorbedInterceptDeferred => Some(reference_norm),
        PreparedOlsFitStatus::SyntaxOmitted if score_norm > 0.0 => Some(score_norm),
        PreparedOlsFitStatus::SyntaxOmitted => Some(reference_norm),
    }
}

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    if !value.is_finite() {
        return Err(invalid_data(
            "prepared OLS compensated score term is not finite",
        ));
    }
    neumaier_add(value, sum, correction);
    if sum.is_finite() && correction.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "prepared OLS compensated score accumulation is not finite",
        ))
    }
}

fn build_coefficient_slots<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    qr: &FiniteFit,
    reservation: &mut R,
) -> Result<Vec<PreparedOlsFittedCoefficient>> {
    let mut output = reserved_empty(
        reservation,
        prepared.fit_model().coefficients.len(),
        "unable to reserve prepared OLS coefficient-slot results",
    )?;
    for coefficient in &prepared.fit_model().coefficients {
        let fitted = match coefficient.status {
            PreparedOlsCoefficientStatus::SyntaxOmitted => PreparedOlsFittedCoefficient {
                coefficient_slot: coefficient.coefficient_slot,
                generated_column_index: None,
                status: PreparedOlsFitStatus::SyntaxOmitted,
                value: None,
            },
            PreparedOlsCoefficientStatus::Active => {
                let generated = coefficient.generated_column_index.ok_or_else(|| {
                    invalid_argument("active prepared OLS coefficient has no generated column")
                })?;
                let status = *qr.generated_statuses.get(generated).ok_or_else(|| {
                    invalid_argument("prepared OLS fitted status is out of range")
                })?;
                let value = *qr.generated_coefficients.get(generated).ok_or_else(|| {
                    invalid_argument("prepared OLS fitted coefficient is out of range")
                })?;
                let has_reported_value = matches!(
                    status,
                    PreparedOlsFitStatus::Kept | PreparedOlsFitStatus::NormalizedAbsorbedIntercept
                );
                if has_reported_value != value.is_some() {
                    return Err(invalid_argument(
                        "prepared OLS fitted coefficient status and value disagree",
                    ));
                }
                PreparedOlsFittedCoefficient {
                    coefficient_slot: coefficient.coefficient_slot,
                    generated_column_index: Some(generated),
                    status,
                    value,
                }
            }
        };
        push_reserved(
            &mut output,
            fitted,
            "prepared OLS coefficient-slot result capacity exhausted",
        )?;
    }
    Ok(output)
}

fn reserved_empty<T, R: FitReservation>(
    reservation: &mut R,
    elements: usize,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "prepared OLS coefficient-fit reservation returned insufficient capacity",
        ));
    }
    Ok(output)
}

fn reserved_filled<T: Clone, R: FitReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = reserved_empty(reservation, elements, label)?;
    output.resize(elements, value);
    Ok(output)
}

fn checked_square(value: usize, message: &'static str) -> Result<usize> {
    value.checked_mul(value).ok_or_else(|| allocation(message))
}

fn insufficient_observations(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InsufficientObservations, message)
}

#[cfg(test)]
mod tests;
