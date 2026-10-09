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
use crate::options::ThreadConfig;
use crate::parallel::ThreadTeam;
use crate::prepared_categorical_data::PreparedCategoricalOlsData;
use crate::prepared_fit_view::PreparedOlsFitView;
use rayon::prelude::*;
use std::time::Instant;

use super::finite_wls_kernel::{
    certify_normal_equations, finite_wls_moment_layout,
    inverse_relative_infinity_norm as kernel_inverse_relative_infinity_norm,
    FixedBlockMomentSource, NormalEquationScale,
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

/// Fit the currently supported prepared OLS/WLS finite coefficients through
/// the strict borrowed-intercept MAP path and a certified scaled finite solve.
pub fn fit_prepared_ols_coefficients_map(
    prepared: &PreparedOlsData,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    fit_prepared_ols_coefficients_view_map(
        &crate::prepared_fit_view::OrdinaryPrepared(prepared),
        config,
    )
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

/// Fit the interaction-aware categorical prepared artifact through the exact
/// same certified MAP, guarded Cholesky/QR, bread, residual, and score path used
/// by the legacy prepared boundary.
pub fn fit_prepared_categorical_ols_coefficients_map(
    prepared: &PreparedCategoricalOlsData,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    fit_prepared_ols_coefficients_view_map(prepared, config)
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
    } else if !crate::prepared_fit_view::add_generated_predictor(
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
    let absorbed = cmg.project_ols(weights.borrowed(), raw, config.absorb, false, false)?;
    if !absorbed.report().certified() {
        return Err(invalid_data(
            "prepared projection initial absorption did not certify",
        ));
    }
    let (within, absorption_report, _) = absorbed.into_full_parts();
    let mut within_norms = reserved_filled(
        reservation,
        generated_columns + 1,
        0.0_f64,
        "unable to reserve prepared projection within norms",
    )?;
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
#[allow(clippy::too_many_arguments)]
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
    let uncentered = outcome_norm.mul_add(outcome_norm, 0.0);
    if !prepared.fit_report_intercept() || prepared.fit_has_absorbed_intercept() {
        return Ok(uncentered);
    }
    let (cross, intercept_norm) = intercept_cross.ok_or_else(|| {
        invalid_argument("prepared OLS low-memory reported intercept was not generated")
    })?;
    if outcome_norm == 0.0 || intercept_norm == 0.0 {
        return Ok(uncentered);
    }
    let centered = (-cross / intercept_norm).mul_add(cross / intercept_norm, uncentered);
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
    if prepared.fit_sample().n_active() == 0 {
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

#[allow(clippy::too_many_arguments)]
fn finalize_reported_bread_and_intercept<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    response: &[f64],
    qr: &mut FiniteFit,
    raw_residuals: Option<&mut [f64]>,
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
            .is_some_and(|residuals| residuals.len() != prepared.fit_sample().n_active())
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
        // state. Derive the independent raw-residual mean directly instead of
        // allocating and retaining a second observation-scale residual vector.
        let raw_residual_mean = if let Some(residuals) = raw_residuals.as_deref() {
            weighted_mean_slice(residuals, weights)?
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
        if let Some(residuals) = raw_residuals {
            for residual in residuals {
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
        fill_raw_moment_cache_columnwise(
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

#[allow(clippy::too_many_arguments)]
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

fn fill_raw_moment_cache_columnwise(
    raw: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    weight_sum: f64,
    scales: &mut [f64],
    means: &mut [f64],
    norms: &mut [f64],
) -> Result<()> {
    if scales.len() != raw.ncols()
        || means.len() != raw.ncols()
        || norms.len() != raw.ncols()
        || !weight_sum.is_finite()
        || weight_sum <= 0.0
    {
        return Err(invalid_argument(
            "prepared OLS columnwise raw-moment dimensions are inconsistent",
        ));
    }
    crate::parallel::stage_try_for_each(
        scales
            .par_iter_mut()
            .zip(means.par_iter_mut())
            .zip(norms.par_iter_mut())
            .enumerate(),
        raw.nrows() as u128 * raw.ncols() as u128,
        |(column, ((scale, mean), norm))| -> Result<()> {
            let mut norm_scale = 0.0_f64;
            let mut norm_sum = 1.0_f64;
            for row in 0..raw.nrows() {
                let value = raw[(row, column)];
                if !value.is_finite() {
                    return Err(invalid_data("prepared OLS raw-moment value is not finite"));
                }
                *scale = scale.max(value.abs());
                update_scaled_sum_of_squares(
                    weights.root(row)? * value,
                    &mut norm_scale,
                    &mut norm_sum,
                )?;
            }
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            if *scale != 0.0 {
                for row in 0..raw.nrows() {
                    compensated_add(
                        weights.value(row)? * (raw[(row, column)] / *scale),
                        &mut sum,
                        &mut correction,
                    )?;
                }
            }
            *mean = if *scale == 0.0 {
                0.0
            } else {
                *scale * ((sum + correction) / weight_sum)
            };
            if !mean.is_finite() {
                return Err(invalid_data("prepared OLS raw-moment mean is not finite"));
            }
            *norm = checked_norm(norm_scale, norm_sum)?;
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
fn fill_weighted_matrix_norms_blocked<const BLOCK: usize>(
    matrix: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    output: &mut [f64],
) -> Result<()> {
    if BLOCK == 0 || output.len() != matrix.ncols() {
        return Err(invalid_argument(
            "prepared OLS blocked weighted norm output has the wrong width",
        ));
    }
    crate::parallel::stage_try_for_each(
        output.par_chunks_mut(BLOCK).enumerate(),
        matrix.nrows() as u128 * matrix.ncols() as u128,
        |(block, norms)| -> Result<()> {
            // Output chunks need not be cache-line aligned. Accumulate locally
            // so adjacent workers do not repeatedly write a shared cache line.
            let mut scales = [0.0_f64; BLOCK];
            let mut square_sums = [1.0_f64; BLOCK];
            let start = block * BLOCK;
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

fn initial_within_sum_of_squares<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    within: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    outcome_norm: f64,
) -> Result<f64> {
    let uncentered = outcome_norm.mul_add(outcome_norm, 0.0);
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
    let sum_of_squares = norm.mul_add(norm, 0.0);
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
        // A coordinate certified as lying in the absorbed span has no finite
        // score equation: its normal equation is already covered by the
        // independent FE certificate. Do not divide its MAP-noise within norm
        // into a manufactured order-one moment. A normalized absorbed intercept
        // still keeps its raw score moment, but that moment uses the raw intercept
        // norm instead of a numerically nonzero MAP-noise norm. Finite
        // rank-deficient coordinates remain checked on their within scale.
        if matches!(
            status,
            PreparedOlsFitStatus::Kept
                | PreparedOlsFitStatus::NumericallyRankDeficient
                | PreparedOlsFitStatus::CollinearWithFixedEffects
        ) {
            continue;
        }
        let denominator = if status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept {
            reference_norm
        } else if score_norm > 0.0 {
            score_norm
        } else {
            reference_norm
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
        relative_square_sum = relative.mul_add(relative, relative_square_sum);
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
mod tests {

    /// Fit a prepared OLS model around an explicitly borrowed frame through
    /// the production Fast/Low frame entries (test comparator for sessions).
    fn fit_prepared_weighted_linear_once<'a, P: PreparedOlsFitView + Sync + ?Sized>(
        prepared: &'a P,
        frame: LinearSolveFrame<'a>,
        config: PreparedOlsFitConfig,
    ) -> Result<CertifiedPreparedOlsFit> {
        ThreadTeam::new(config.execution.threads).install(|| {
            validate_config(prepared, config)?;
            frame.validate_for(prepared)?;
            let mut reservation = SystemFitReservation;
            match config.execution.mode {
                PreparedOlsExecutionMode::Fast => fit_with_controls_internal_mode_with_frame(
                    prepared,
                    frame,
                    config,
                    &mut reservation,
                    FitPerturbations::default(),
                    false,
                    MAX_FAST_SCORE_CACHE_BYTES,
                    None,
                    None,
                    None,
                ),
                PreparedOlsExecutionMode::Low => fit_with_controls_low_internal_with_frame(
                    prepared,
                    frame,
                    config,
                    &mut reservation,
                    FitPerturbations::default(),
                    None,
                    None,
                    None,
                ),
            }
        })?
    }

    use super::*;
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm,
        LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId, TermComponent, TermId,
        TermOrder, TimeSeriesState,
    };
    use crate::ols::finite_wls_kernel::{
        begin_retained_moment_worker_observation, finish_retained_moment_worker_observation,
    };
    use crate::ols::{
        begin_frozen_qr_fallback_observation, finish_frozen_qr_fallback_observation,
        prepare_ols_data, CertifiedLinearSession, CertifiedLinearSessionOutput,
        CertifiedLinearSessionProducts, CertifiedLinearWithinDesign, LinearSessionAllocationPhase,
        LinearSessionGenerations, LinearSessionPhaseAllocationEvent, LoadedNumericColumn,
        LoadedOlsInput, OlsInputBinding, OlsInputRole, OlsRequestV1,
    };
    use crate::sandwich::SandwichDesign;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn qr_classification_preserves_norm_errors_and_deferred_intercept_order() {
        let y = [0.0; 6];
        let groups = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let data = prepared(&y, &[y.to_vec()], &[false], None, false, TestWeights::Unit);
        let intercept = prepared(&y, &[], &[], Some(&groups), true, TestWeights::Unit);
        let run = |data: &PreparedOlsData, reference, score, low| {
            let frame = LinearSolveFrame::from_prepared_ols(data).unwrap();
            let config = PreparedOlsFitConfig::default();
            if low {
                finite::solve_one_shot(
                    data,
                    finite::ProjectedFiniteRows::Scratch {
                        scratch: &PreparedOlsScratch::create_row_major(6, 1).unwrap(),
                        outcome: &[0.0; 6],
                    },
                    frame.weights(),
                    frame.bread_weight_scale(),
                    &[reference],
                    &[score],
                    config,
                    &mut SystemFitReservation,
                    false,
                    false,
                    true,
                )
            } else {
                finite::solve_one_shot(
                    data,
                    finite::ProjectedFiniteRows::Dense(&DenseMatrix::zeros(6, 2).unwrap()),
                    frame.weights(),
                    frame.bread_weight_scale(),
                    &[reference],
                    &[score],
                    config,
                    &mut SystemFitReservation,
                    false,
                    false,
                    false,
                )
            }
        };
        let reference_error = "prepared OLS reference regressor norm is invalid";
        let ratio_error = "prepared OLS fixed-effect collinearity ratio is not finite";
        let square_error = "prepared OLS fixed-effect squared collinearity ratio is not finite";
        let low_norm_error = "prepared OLS low-memory regressor norm is invalid";
        let low_ratio_error = "prepared OLS low-memory collinearity ratio is invalid";
        // A zero reference short-circuits fast validation, but low validates
        // both norms first. These are caller boundaries, not memory-mode rules
        // to introduce into the shared classification implementation.
        for (reference, score, fast, low) in [
            (-1.0, f64::NAN, Some(reference_error), Some(low_norm_error)),
            (f64::NAN, 0.0, Some(reference_error), Some(low_norm_error)),
            (0.0, f64::NAN, None, Some(low_norm_error)),
            (1.0, f64::NAN, Some(ratio_error), Some(low_norm_error)),
            (1e-300, 1e300, Some(ratio_error), Some(low_ratio_error)),
            (1.0, 1e200, Some(square_error), Some(low_ratio_error)),
            (0.0, 0.0, None, None),
        ] {
            for (low_mode, expected) in [(false, fast), (true, low)] {
                let result = run(&data, reference, score, low_mode);
                if let Some(message) = expected {
                    let error = result.unwrap_err();
                    assert_eq!(error.code, ErrorCode::InvalidData);
                    assert_eq!(error.message, message);
                } else {
                    let qr = result.unwrap();
                    assert_eq!(qr.rank, 0);
                    assert_eq!(
                        qr.generated_statuses,
                        [PreparedOlsFitStatus::NumericallyRankDeficient]
                    );
                }
            }
        }
        for low in [false, true] {
            let qr = run(&intercept, f64::NAN, f64::NAN, low).unwrap();
            assert_eq!(qr.rank, 0);
            assert_eq!(
                qr.generated_statuses,
                [PreparedOlsFitStatus::AbsorbedInterceptDeferred]
            );
        }
    }

    #[test]
    fn current_frame_means_select_canonical_coordinates_and_reject_invalid_views() {
        let known = CurrentFrameColumnMeans {
            generated_columns: &[0, 2, 5],
            values: &[1.0, -3.0, 7.0],
        };
        let mut output = [0.0; 2];
        known.fill_selected(&[5, 0], &mut output).unwrap();
        assert_eq!(output, [7.0, 1.0]);
        assert!(known.fill_selected(&[1, 0], &mut output).is_err());
        assert!(known.fill_selected(&[0], &mut output).is_err());
        for bad in [
            CurrentFrameColumnMeans {
                generated_columns: &[1, 1],
                values: &[1.0, 2.0],
            },
            CurrentFrameColumnMeans {
                generated_columns: &[2, 1],
                values: &[1.0, 2.0],
            },
            CurrentFrameColumnMeans {
                generated_columns: &[0, 1],
                values: &[1.0],
            },
            CurrentFrameColumnMeans {
                generated_columns: &[0, 1],
                values: &[1.0, f64::NAN],
            },
        ] {
            assert!(bad.fill_selected(&[0, 1], &mut output).is_err());
        }
    }

    static ACTIVE_LINEAR_ALLOCATION_PHASE: AtomicUsize = AtomicUsize::new(0);
    static LINEAR_PROJECTION_ENDS: AtomicUsize = AtomicUsize::new(0);
    static LINEAR_FINITE_ENDS: AtomicUsize = AtomicUsize::new(0);

    fn assert_retained_moment_team(observed: (usize, u64), expected: usize) {
        assert_eq!(
            observed.0,
            expected,
            "retained kernel escaped configured pool; observed participants={}",
            observed.1.count_ones(),
        );
    }

    fn observed_linear_allocation_phase(
        phase: LinearSessionAllocationPhase,
        event: LinearSessionPhaseAllocationEvent,
    ) -> u64 {
        let phase_id = match phase {
            LinearSessionAllocationPhase::Projection => 1,
            LinearSessionAllocationPhase::FiniteSolve => 2,
        };
        match event {
            LinearSessionPhaseAllocationEvent::Begin => {
                assert_eq!(
                    ACTIVE_LINEAR_ALLOCATION_PHASE.swap(phase_id, Ordering::SeqCst),
                    0,
                    "linear allocation phases must never overlap"
                );
                0
            }
            LinearSessionPhaseAllocationEvent::End => {
                assert_eq!(
                    ACTIVE_LINEAR_ALLOCATION_PHASE.swap(0, Ordering::SeqCst),
                    phase_id,
                    "linear allocation phase end must match its begin"
                );
                match phase {
                    LinearSessionAllocationPhase::Projection => {
                        LINEAR_PROJECTION_ENDS.fetch_add(1, Ordering::SeqCst);
                        11
                    }
                    LinearSessionAllocationPhase::FiniteSolve => {
                        LINEAR_FINITE_ENDS.fetch_add(1, Ordering::SeqCst);
                        23
                    }
                }
            }
        }
    }

    #[derive(Clone, Copy)]
    enum TestWeights<'a> {
        Unit,
        Analytic(&'a [f64]),
        Probability(&'a [f64]),
        Frequency(&'a [f64]),
    }

    #[derive(Default)]
    struct InjectedReservation {
        calls: usize,
        fail_at: Option<usize>,
        undersize: bool,
        records: Vec<(usize, &'static str)>,
    }

    impl FitReservation for InjectedReservation {
        fn reserve<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            label: &'static str,
        ) -> Result<()> {
            self.calls += 1;
            self.records.push((elements, label));
            if self.fail_at == Some(self.calls) {
                return Err(allocation("injected prepared OLS fit reservation failure"));
            }
            if !self.undersize {
                target
                    .try_reserve_exact(elements)
                    .map_err(|_| allocation("test prepared OLS fit reservation failure"))?;
            }
            Ok(())
        }
    }

    fn source(id: u32, name: &str) -> SourceEntry {
        SourceEntry {
            id: SourceId::new(id).expect("source"),
            canonical_name: name.to_owned(),
        }
    }

    fn continuous_term(id: u32, source: SourceId, omitted: bool) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(id).expect("term"),
            order: TermOrder::new(id).expect("order"),
            coefficient_slot: CoefficientSlot::new(id).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: omitted,
            canonical_token: format!("c.x{id}"),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
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

    fn categorical_term(id: u32, source: SourceId, level: f64, omitted: bool) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(id).expect("term"),
            order: TermOrder::new(id).expect("order"),
            coefficient_slot: CoefficientSlot::new(id).expect("slot"),
            raw_kind: RawTermKind::Factor,
            whole_column_omitted: omitted,
            canonical_token: format!("{level}.g"),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source,
                kind: ComponentKind::Categorical,
                level_state: LevelState::Present,
                level_value: level,
                base: if omitted {
                    MetadataFlag::True
                } else {
                    MetadataFlag::False
                },
                omitted: if omitted {
                    MetadataFlag::True
                } else {
                    MetadataFlag::False
                },
                raw_operator: level.to_string(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        }
    }

    fn categorical_interaction_term(id: u32, components: &[(SourceId, f64)]) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(id).expect("term"),
            order: TermOrder::new(id).expect("order"),
            coefficient_slot: CoefficientSlot::new(id).expect("slot"),
            raw_kind: RawTermKind::Interaction,
            whole_column_omitted: false,
            canonical_token: "categorical-interaction".to_owned(),
            components: components
                .iter()
                .copied()
                .enumerate()
                .map(|(index, (source, level))| TermComponent {
                    order: ComponentOrder::new(u32::try_from(index + 1).expect("small component"))
                        .expect("component"),
                    source,
                    kind: ComponentKind::Categorical,
                    level_state: LevelState::Present,
                    level_value: level,
                    base: MetadataFlag::False,
                    omitted: MetadataFlag::False,
                    raw_operator: level.to_string(),
                    time_series_state: TimeSeriesState::None,
                    raw_time_series_operator: String::new(),
                })
                .collect(),
        }
    }

    fn loaded(values: &[f64]) -> LoadedNumericColumn {
        LoadedNumericColumn {
            values: values.to_vec(),
            missing: vec![false; values.len()],
        }
    }

    fn prepared(
        outcome: &[f64],
        regressors: &[Vec<f64>],
        omitted: &[bool],
        fixed_effect: Option<&[f64]>,
        include_intercept: bool,
        weights: TestWeights<'_>,
    ) -> PreparedOlsData {
        assert_eq!(regressors.len(), omitted.len());
        let mut sources = Vec::new();
        let mut terms = Vec::new();
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ];
        let mut numeric_sources = Vec::new();
        for (index, values) in regressors.iter().enumerate() {
            let one_based = u32::try_from(index + 1).expect("small source");
            let entry = source(one_based, &format!("x{one_based}"));
            terms.push(continuous_term(one_based, entry.id, omitted[index]));
            bindings.push(OlsInputBinding {
                stata_var_position: one_based + 2,
                role: OlsInputRole::NumericSource(entry.id),
            });
            numeric_sources.push((entry.id, loaded(values)));
            sources.push(entry);
        }
        let absorbed_terms = fixed_effect.map_or_else(Vec::new, |codes| {
            let one_based = u32::try_from(sources.len() + 1).expect("small FE source");
            let entry = source(one_based, "g");
            bindings.push(OlsInputBinding {
                stata_var_position: one_based + 2,
                role: OlsInputRole::NumericSource(entry.id),
            });
            numeric_sources.push((entry.id, loaded(codes)));
            sources.push(entry.clone());
            vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorbed term"),
                intercept: true,
                categorical_sources: vec![entry.id],
                slope_sources: Vec::new(),
                label: "g".to_owned(),
                saved_name: String::new(),
            }]
        });
        let weight_position = u32::try_from(bindings.len() + 1).expect("small binding");
        let (analytic_weight, probability_weight, frequency_weight) = match weights {
            TestWeights::Unit => (None, None, None),
            TestWeights::Analytic(values) => {
                bindings.push(OlsInputBinding {
                    stata_var_position: weight_position,
                    role: OlsInputRole::AnalyticWeight,
                });
                (Some(loaded(values)), None, None)
            }
            TestWeights::Probability(values) => {
                bindings.push(OlsInputBinding {
                    stata_var_position: weight_position,
                    role: OlsInputRole::ProbabilityWeight,
                });
                (None, Some(loaded(values)), None)
            }
            TestWeights::Frequency(values) => {
                bindings.push(OlsInputBinding {
                    stata_var_position: weight_position,
                    role: OlsInputRole::FrequencyWeight,
                });
                (None, None, Some(loaded(values)))
            }
        };
        prepare_ols_data(
            &OlsRequestV1 {
                model: CompiledTermGraph { sources, terms },
                absorbed_terms,
                bindings,
                include_intercept,
                keep_singletons: true,
            },
            &LoadedOlsInput {
                candidate: vec![true; outcome.len()],
                outcome: loaded(outcome),
                numeric_sources,
                analytic_weight,
                frequency_weight,
                probability_weight,
            },
        )
        .expect("prepared data")
    }

    #[test]
    fn discarded_workspace_is_merged_once_even_without_a_setup_timer_or_warning() {
        use crate::memory::SolverWorkspacePeak;
        let y = [1.0, -1.0, 2.0, -2.0];
        let g = [0.0, 0.0, 1.0, 1.0];
        let data = prepared(&y, &[], &[], Some(&g), false, TestWeights::Unit);
        let config = PreparedOlsFitConfig::default();
        for discarded in [
            SolverWorkspacePeak::from_bytes(1 << 28).unwrap(),
            SolverWorkspacePeak::UNMODELED,
        ] {
            let mut session =
                crate::fe::FeSession::for_ols(&data, BorrowedMapWeights::Unit, config).unwrap();
            session.peak = discarded;
            for _ in 0..2 {
                let result = session
                    .project_ols(
                        BorrowedMapWeights::Unit,
                        DenseMatrix::from_columns(&[y.to_vec()]).unwrap(),
                        config.absorb,
                        false,
                        false,
                    )
                    .unwrap();
                assert!(result.report().certified());
                assert_eq!(result.residualized().as_slice(), &y);
                assert_eq!(result.report().solver_workspace_peak, discarded);
                assert_eq!(session.peak, discarded);
            }
        }
    }

    #[test]
    fn retained_fe_capacity_handoff_counts_spare_value_and_layout_buffers() {
        // Logical axes match a two-effect/two-RHS result. Spare storage must
        // remain charged when the result is moved out of its solver.
        for (value_capacity, channel_capacity, level_capacity) in
            [(14, 2, 2), (8192, 2, 2), (14, 64, 2), (14, 2, 64)]
        {
            let mut values = Vec::with_capacity(value_capacity);
            values.resize(14, 0.0);
            let mut channels = Vec::with_capacity(channel_capacity);
            channels.extend_from_slice(&[1, 1]);
            let mut levels = Vec::with_capacity(level_capacity);
            levels.extend_from_slice(&[4, 3]);
            let initial = FeCoefficientBatch::from_values(channels, levels, 2, values).unwrap();
            let combined =
                FeCoefficientBatch::from_values(vec![1, 1], vec![4, 3], 1, vec![0.0; 7]).unwrap();
            let exact = initial.retained_bytes() + combined.retained_bytes();
            check_retained_fe_initial_capacity(Some(&initial), exact).unwrap();
            let error = check_retained_fe_initial_capacity(Some(&initial), exact - 1).unwrap_err();
            assert_eq!(error.code, ErrorCode::Allocation);
            assert!(error.message.contains("capacity exceeds"));
            // Returning an error cannot change either coefficient owner.
            assert_eq!(initial.rhs(0).unwrap(), &[0.0; 7]);
            assert_eq!(initial.rhs(1).unwrap(), &[0.0; 7]);
        }
        check_retained_fe_initial_capacity(None, 0).unwrap();
    }

    #[test]
    fn retained_fe_capacity_handoff_rejects_logical_size_only_reservation() {
        let mut values = Vec::with_capacity(16_384);
        values.resize(14, 0.0);
        let initial = FeCoefficientBatch::from_values(vec![1, 1], vec![4, 3], 2, values).unwrap();
        let logical = 7 * 3 * 8 + 2 * 7 * std::mem::size_of::<usize>() as u128;
        assert_eq!(
            check_retained_fe_initial_capacity(Some(&initial), logical)
                .unwrap_err()
                .code,
            ErrorCode::Allocation,
        );
    }

    #[test]
    fn retained_fe_combination_admits_before_allocation_and_preserves_arithmetic() {
        let initial = || {
            FeCoefficientBatch::from_values(vec![1], vec![2], 2, vec![5.0, 9.0, 2.0, 3.0]).unwrap()
        };
        let residual =
            || FeCoefficientBatch::from_values(vec![1], vec![2], 1, vec![0.25, -0.5]).unwrap();
        let parent = initial().retained_bytes() + residual().retained_bytes();
        let mut reservation = InjectedReservation::default();
        assert_eq!(
            combine_recovered_fe_coefficients(
                Some(initial()),
                Some(residual()),
                1,
                &[Some(2.0)],
                parent - 1,
                &mut reservation,
            )
            .unwrap_err()
            .code,
            ErrorCode::Allocation
        );
        assert_eq!(
            reservation.calls, 0,
            "reject before the new coefficient bank"
        );
        for fail_at in 1..=3 {
            let mut reservation = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            assert_eq!(
                combine_recovered_fe_coefficients(
                    Some(initial()),
                    Some(residual()),
                    1,
                    &[Some(2.0)],
                    parent,
                    &mut reservation,
                )
                .unwrap_err()
                .code,
                ErrorCode::Allocation
            );
            assert_eq!(reservation.calls, fail_at);
        }
        for _ in 0..3 {
            let combined = combine_recovered_fe_coefficients(
                Some(initial()),
                Some(residual()),
                1,
                &[Some(2.0)],
                parent,
                &mut SystemFitReservation,
            )
            .unwrap()
            .unwrap();
            assert_eq!(combined.rhs(0).unwrap(), &[1.25, 2.5]);
        }
        struct SpareReservation;
        impl FitReservation for SpareReservation {
            fn reserve<T>(
                &mut self,
                target: &mut Vec<T>,
                elements: usize,
                _: &'static str,
            ) -> Result<()> {
                target
                    .try_reserve_exact(elements + 64)
                    .map_err(|_| allocation("test spare reservation failed"))
            }
        }
        let error = combine_recovered_fe_coefficients(
            Some(initial()),
            Some(residual()),
            1,
            &[Some(2.0)],
            parent,
            &mut SpareReservation,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Allocation);
        assert!(error.message.contains("combined FE product capacity"));
    }

    #[test]
    fn retained_fe_parent_is_admitted_before_session_and_survives_peak_reporting() {
        let x = (0..96)
            .map(|r| ((r * 19 % 37) as f64).sin())
            .collect::<Vec<_>>();
        let g = (0..96).map(|r| (r % 4) as f64).collect::<Vec<_>>();
        let y = (0..96)
            .map(|r| 2.0 * x[r] + g[r] + ((r * 7 % 31) as f64).cos())
            .collect::<Vec<_>>();
        let h = (0..96).map(|r| ((r / 4) % 3) as f64).collect::<Vec<_>>();
        let view = prepared_multiway_categorical(&y, &[x], &[g, h]);
        let parent = view.fit_retained_fe_parent_bytes().unwrap();
        let mut config = PreparedOlsFitConfig::default();
        config.absorb.solver = FeSolverKind::Cg;
        config.absorb.retain_coefficients = true;
        config.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
            execution_bytes: 1234,
            usable_bytes: 1 << 30,
        });
        let mut reservation = SystemFitReservation;
        let reference = fit_with_controls_internal(
            &view,
            config,
            &mut reservation,
            FitPerturbations::default(),
        )
        .unwrap();
        assert!(reference.recovered_fe_coefficients.is_some());
        assert!(u128::from(reference.solver_workspace_peak().bytes().unwrap()) >= parent);
        config.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
            execution_bytes: 1234,
            usable_bytes: 1234 + parent - 1,
        });
        let error = fit_with_controls_internal(
            &view,
            config,
            &mut reservation,
            FitPerturbations::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Allocation);
        assert!(error.message.contains("retained FE products"));
        // The returned solver envelope includes the retained parent exactly
        // once relative to the caller's unchanged execution base.
        config
            .absorb
            .solver_memory_budget
            .as_mut()
            .unwrap()
            .usable_bytes = 1234 + u128::from(reference.solver_workspace_peak().bytes().unwrap());
        let exact = fit_with_controls_internal(
            &view,
            config,
            &mut reservation,
            FitPerturbations::default(),
        )
        .unwrap();
        assert_eq!(exact.coefficients(), reference.coefficients());
        assert_eq!(exact.residuals(), reference.residuals());
        assert_eq!(
            exact.recovered_fe_coefficients,
            reference.recovered_fe_coefficients
        );
        assert_eq!(
            exact.solver_workspace_peak(),
            reference.solver_workspace_peak()
        );
        config
            .absorb
            .solver_memory_budget
            .as_mut()
            .unwrap()
            .usable_bytes -= 1;
        assert_eq!(
            fit_with_controls_internal(
                &view,
                config,
                &mut reservation,
                FitPerturbations::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::Allocation
        );
    }

    #[test]
    fn direct_prepared_cg_checks_joint_budget_with_retained_coefficients() {
        let n = 96;
        let effects = [
            (0..n).map(|row| (row % 4) as f64).collect::<Vec<_>>(),
            (0..n).map(|row| ((row / 4) % 3) as f64).collect::<Vec<_>>(),
        ];
        let noise = (0..n)
            .map(|row| if (row / 12) % 2 == 0 { -1.0 } else { 1.0 })
            .collect::<Vec<_>>();
        let y = (0..n)
            .map(|row| 2.0 * effects[0][row] + 3.0 * effects[1][row] + noise[row])
            .collect::<Vec<_>>();
        let simple_y = (0..n)
            .map(|row| 2.0 * effects[0][row] + noise[row])
            .collect::<Vec<_>>();
        let simple = prepared(
            &simple_y,
            &[],
            &[],
            Some(&effects[0]),
            false,
            TestWeights::Unit,
        );
        let categorical = prepared_multiway_categorical(&y, &[], &effects);
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    for (view, outcome, levels, retain) in [
                        (
                            &simple as &dyn PreparedOlsFitView,
                            &simple_y,
                            vec![4],
                            false,
                        ),
                        (
                            &categorical as &dyn PreparedOlsFitView,
                            &y,
                            vec![4, 3],
                            false,
                        ),
                        (
                            &categorical as &dyn PreparedOlsFitView,
                            &y,
                            vec![4, 3],
                            true,
                        ),
                    ] {
                        let model =
                            crate::memory::estimate_fe_solver_memory(n, 1, &levels, threads, None)
                                .unwrap();
                        let incremental = model.cg_bytes
                            + if retain {
                                5 * model.coefficient_batch_bytes
                            } else {
                                0
                            };
                        let execution = 137;
                        let supplied = (0..n).map(|row| 1.0 + (row % 4) as f64).collect::<Vec<_>>();
                        for weights in [
                            BorrowedMapWeights::Unit,
                            BorrowedMapWeights::Provided(&supplied),
                        ] {
                            for dispatch in 0..3 {
                                let run = |usable| {
                                    let config = AbsorbConfig {
                                        solver: FeSolverKind::Cg,
                                        retain_coefficients: retain,
                                        solver_memory_budget: Some(
                                            crate::memory::FeSolverMemoryBudget {
                                                execution_bytes: execution,
                                                usable_bytes: usable,
                                            },
                                        ),
                                        ..AbsorbConfig::default()
                                    };
                                    let values =
                                        DenseMatrix::from_row_major(n, 1, outcome.to_vec())
                                            .unwrap();
                                    match dispatch {
                                        0 => view.fit_absorb_matrix(weights, values, config),
                                        1 => view.fit_absorb_matrix(weights, values, config),
                                        _ => {
                                            let mut session = crate::fe::FeSession::for_ols(
                                                view,
                                                weights,
                                                PreparedOlsFitConfig {
                                                    absorb: config,
                                                    ..PreparedOlsFitConfig::default()
                                                },
                                            )?;
                                            session
                                                .project_ols(weights, values, config, false, false)
                                        }
                                    }
                                };
                                let error = run(execution + incremental - 1)
                                    .expect_err("CG must reject before its workspace");
                                assert_eq!(error.code, ErrorCode::Allocation);
                                assert!(error.message.contains("joint execution/CG memory plan"));
                                let fit = run(execution + incremental).unwrap();
                                assert!(fit.report().certified());
                                assert_eq!(
                                    fit.report().solver_workspace_peak.bytes(),
                                    Some(incremental as u64)
                                );
                                let (residual, _, coefficients) = fit.into_full_parts();
                                assert_eq!(coefficients.is_some(), retain);
                                for (&actual, &expected) in residual.as_slice().iter().zip(&noise) {
                                    assert!((actual - expected).abs() < 1e-10);
                                }
                                assert_eq!(run(0).unwrap_err().code, ErrorCode::Allocation);
                            }
                        }
                    }
                });
        }
    }

    #[test]
    fn ppml_slope_availability_preserves_classification_and_final_design() {
        use crate::prepared_categorical_data::{
            prepare_categorical_ols_data_with_cluster_sources, prepare_ppml_categorical_ols_data,
        };
        for swapped in [false, true] {
            for kind in 0..3 {
                for case in 0..12 {
                    let n = 97;
                    let x = source(1, "x");
                    let g = source(if swapped { 3 } else { 2 }, "g");
                    let z = source(if swapped { 2 } else { 3 }, "z");
                    let h = source(4, "h");
                    let t = source(5, "t");
                    let mut sources = vec![x.clone(), g.clone(), z.clone(), h.clone(), t.clone()];
                    sources.sort_by_key(|source| source.id);
                    let mut request = OlsRequestV1 {
                        model: CompiledTermGraph {
                            sources,
                            terms: vec![continuous_term(1, x.id, false)],
                        },
                        absorbed_terms: vec![
                            AbsorbedTerm {
                                id: AbsorbTermId::new(1).unwrap(),
                                intercept: case != 5,
                                categorical_sources: vec![g.id],
                                slope_sources: vec![z.id, x.id],
                                label: "g##c.(z x)".into(),
                                saved_name: String::new(),
                            },
                            AbsorbedTerm {
                                id: AbsorbTermId::new(2).unwrap(),
                                intercept: true,
                                categorical_sources: if case == 8 {
                                    vec![g.id, h.id]
                                } else {
                                    vec![h.id]
                                },
                                slope_sources: vec![t.id],
                                label: "h##c.t".into(),
                                saved_name: String::new(),
                            },
                        ],
                        bindings: vec![
                            OlsInputBinding {
                                stata_var_position: 1,
                                role: OlsInputRole::Candidate,
                            },
                            OlsInputBinding {
                                stata_var_position: 2,
                                role: OlsInputRole::Outcome,
                            },
                        ],
                        include_intercept: true,
                        keep_singletons: case != 7,
                    };
                    for source in &request.model.sources {
                        request.bindings.push(OlsInputBinding {
                            stata_var_position: source.id.get() + 2,
                            role: OlsInputRole::NumericSource(source.id),
                        });
                    }
                    let mut input = LoadedOlsInput {
                        candidate: vec![true; n],
                        outcome: loaded(&vec![1.0; n]),
                        numeric_sources: request
                            .model
                            .sources
                            .iter()
                            .map(|source| {
                                let values: Vec<_> = (0..n)
                                    .map(|r| {
                                        if source.id == g.id {
                                            (r % 7) as f64
                                        } else if source.id == h.id {
                                            (r % 5) as f64
                                        } else {
                                            ((r * (source.id.get() as usize + 3)) as f64).sin()
                                        }
                                    })
                                    .collect();
                                (source.id, loaded(&values))
                            })
                            .collect(),
                        analytic_weight: None,
                        frequency_weight: None,
                        probability_weight: None,
                    };
                    let zi = (z.id.get() - 1) as usize;
                    let gi = (g.id.get() - 1) as usize;
                    if case == 1 {
                        input.numeric_sources[zi].1.missing[3] = true;
                    }
                    if case == 2 {
                        input.numeric_sources[0].1.missing[4] = true;
                        input.numeric_sources[zi].1.missing[4] = true;
                        input.outcome.missing[5] = true;
                    }
                    if case == 3 {
                        input.numeric_sources[zi].1.values[6] = f64::INFINITY;
                    }
                    if case == 4 {
                        input.candidate[3] = false;
                        input.numeric_sources[zi].1.values[3] = f64::NAN;
                    }
                    if case == 6 {
                        input.numeric_sources[zi].1.missing[8] = true;
                    }
                    if case == 7 {
                        input.numeric_sources[gi].1.values[0] = 1000.0;
                    }
                    if case == 9 {
                        // A slope source may also define another partition. Its codes
                        // must survive compaction even with deficient slope channels.
                        request.absorbed_terms[1].categorical_sources = vec![z.id];
                        input.numeric_sources[zi].1.values.fill(1.0);
                        input.numeric_sources[4].1.values.fill(0.0);
                    }
                    if case >= 10 {
                        // Factor recipes bind indices in the complete availability
                        // table, so their existing path is deliberately preserved.
                        input.numeric_sources[zi].1.values =
                            (0..n).map(|r| (r % 3) as f64).collect();
                        request
                            .model
                            .terms
                            .push(categorical_term(2, z.id, 1.0, false));
                        request
                            .model
                            .terms
                            .push(categorical_interaction_term(3, &[(g.id, 2.0), (h.id, 1.0)]));
                        if case == 11 {
                            request
                                .model
                                .terms
                                .push(categorical_term(4, z.id, 9.0, false));
                            request
                                .model
                                .terms
                                .push(categorical_term(5, z.id, 0.0, true));
                        }
                    }
                    if kind > 0 {
                        let role = if kind == 1 {
                            OlsInputRole::FrequencyWeight
                        } else {
                            OlsInputRole::ProbabilityWeight
                        };
                        request.bindings.push(OlsInputBinding {
                            stata_var_position: 8,
                            role,
                        });
                        let mut weight =
                            loaded(&(0..n).map(|r| 1.0 + (r % 3) as f64).collect::<Vec<_>>());
                        if case == 2 {
                            weight.missing[7] = true;
                        }
                        if kind == 1 {
                            input.frequency_weight = Some(weight);
                        } else {
                            input.probability_weight = Some(weight);
                        }
                    }
                    let clusters = if case == 6 { vec![z.id] } else { Vec::new() };
                    let expected = prepare_categorical_ols_data_with_cluster_sources(
                        &request, &input, &clusters,
                    );
                    let actual = prepare_ppml_categorical_ols_data(&request, &input, &clusters);
                    match (expected, actual) {
                        (Ok(a), Ok(b)) => {
                            assert_eq!(a.sample(), b.sample());
                            assert_eq!(a.model(), b.model());
                            assert_eq!(a.effect_specs(), b.effect_specs());
                            assert_eq!(a.report_plan(), b.report_plan());
                            assert_eq!(a.singletons(), b.singletons());
                            assert_eq!(
                                a.fit_owned_fe_design().unwrap(),
                                b.fit_owned_fe_design().unwrap()
                            );
                            if case == 1 {
                                assert!(!b.sample().original_row_by_active.contains(&3));
                                assert_eq!(b.sample().n_active(), n - 1);
                            }
                            if case == 4 {
                                assert!(!b.sample().original_row_by_active.contains(&3));
                            }
                        }
                        (Err(a), Err(b)) => assert_eq!(a, b),
                        _ => panic!(
                            "PPML slope preparation changed acceptance: {swapped}/{kind}/{case}"
                        ),
                    }
                }
            }
        }
    }

    #[test]
    fn compact_slope_availability_preserves_legacy_samples_models_and_topology() {
        use crate::prepared_categorical_data::{
            prepare_categorical_ols_data_owned_with_cluster_sources,
            prepare_categorical_ols_data_with_cluster_sources,
        };
        for swapped in [false, true] {
            for kind in 0..4 {
                for case in 0..7 {
                    let n = 16_387;
                    let x = source(1, "x");
                    let group = source(if swapped { 3 } else { 2 }, "group");
                    let slope = source(if swapped { 2 } else { 3 }, "slope");
                    let mut sources = vec![x.clone(), group.clone(), slope.clone()];
                    sources.sort_by_key(|s| s.id);
                    let mut request = OlsRequestV1 {
                        model: CompiledTermGraph {
                            sources,
                            terms: vec![continuous_term(1, x.id, false)],
                        },
                        absorbed_terms: vec![AbsorbedTerm {
                            id: AbsorbTermId::new(1).unwrap(),
                            intercept: case != 5,
                            categorical_sources: vec![group.id],
                            slope_sources: vec![slope.id],
                            label: "group##c.slope".into(),
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
                        ],
                        include_intercept: true,
                        keep_singletons: false,
                    };
                    for source in &request.model.sources {
                        request.bindings.push(OlsInputBinding {
                            stata_var_position: source.id.get() + 2,
                            role: OlsInputRole::NumericSource(source.id),
                        });
                    }
                    if case == 6 {
                        request
                            .model
                            .terms
                            .push(continuous_term(2, slope.id, false));
                    }
                    let mut numeric_sources: Vec<_> = request
                        .model
                        .sources
                        .iter()
                        .map(|source| {
                            let values: Vec<_> = (0..n)
                                .map(|r| {
                                    if source.id == group.id {
                                        (r / 5) as f64
                                    } else {
                                        ((r * (source.id.get() as usize + 3)) as f64).sin()
                                    }
                                })
                                .collect();
                            (source.id, loaded(&values))
                        })
                        .collect();
                    let si = (slope.id.get() - 1) as usize;
                    let gi = (group.id.get() - 1) as usize;
                    if case == 1 {
                        numeric_sources[si].1.missing[3] = true;
                    }
                    if case == 2 {
                        numeric_sources[gi].1.missing[4] = true;
                        numeric_sources[si].1.missing[4] = true;
                    }
                    if case == 3 {
                        numeric_sources[gi].1.values[0] = 100_000.0;
                    }
                    let mut input = LoadedOlsInput {
                        candidate: vec![true; n],
                        outcome: loaded(&vec![1.0; n]),
                        numeric_sources,
                        analytic_weight: None,
                        frequency_weight: None,
                        probability_weight: None,
                    };
                    if case == 4 {
                        input.candidate[3] = false;
                        input.numeric_sources[si].1.values[3] = f64::NAN;
                    }
                    if kind > 0 {
                        let role = match kind {
                            1 => OlsInputRole::AnalyticWeight,
                            2 => OlsInputRole::ProbabilityWeight,
                            _ => OlsInputRole::FrequencyWeight,
                        };
                        request.bindings.push(OlsInputBinding {
                            stata_var_position: 6,
                            role,
                        });
                        let mut weight =
                            loaded(&(0..n).map(|r| 1.0 + (r % 3) as f64).collect::<Vec<_>>());
                        if case == 2 {
                            weight.missing[7] = true;
                        }
                        match kind {
                            1 => input.analytic_weight = Some(weight),
                            2 => input.probability_weight = Some(weight),
                            _ => input.frequency_weight = Some(weight),
                        }
                    }
                    let clusters = if case == 6 { vec![x.id] } else { Vec::new() };
                    let expected = prepare_categorical_ols_data_with_cluster_sources(
                        &request, &input, &clusters,
                    );
                    let actual = prepare_categorical_ols_data_owned_with_cluster_sources(
                        &request, input, &clusters,
                    );
                    match (expected, actual) {
                        (Ok(a), Ok(b)) => {
                            assert_eq!(a.sample(), b.sample());
                            assert_eq!(a.model(), b.model());
                            assert_eq!(a.effect_specs(), b.effect_specs());
                            assert_eq!(a.report_plan(), b.report_plan());
                            assert_eq!(a.singletons(), b.singletons());
                            assert_eq!(
                                a.fit_owned_fe_design().unwrap(),
                                b.fit_owned_fe_design().unwrap()
                            );
                        }
                        (Err(a), Err(b)) => assert_eq!(a, b),
                        _ => panic!("slope preparation changed acceptance"),
                    }
                }
            }
        }
    }

    #[test]
    fn single_slope_cache_is_ols_only_and_preserves_full_fit() {
        for n in [16_383, 16_384] {
            for intercept in [false, true] {
                let x = source(1, "x");
                let group = source(2, "g");
                let slope = source(3, "z");
                let request = OlsRequestV1 {
                    model: CompiledTermGraph {
                        sources: vec![x.clone(), group.clone(), slope.clone()],
                        terms: vec![continuous_term(1, x.id, false)],
                    },
                    absorbed_terms: vec![AbsorbedTerm {
                        id: AbsorbTermId::new(1).unwrap(),
                        intercept,
                        categorical_sources: vec![group.id],
                        slope_sources: vec![slope.id],
                        label: "g#c.z".into(),
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
                            role: OlsInputRole::NumericSource(x.id),
                        },
                        OlsInputBinding {
                            stata_var_position: 4,
                            role: OlsInputRole::NumericSource(group.id),
                        },
                        OlsInputBinding {
                            stata_var_position: 5,
                            role: OlsInputRole::NumericSource(slope.id),
                        },
                    ],
                    include_intercept: true,
                    keep_singletons: false,
                };
                let columns: Vec<Vec<f64>> = (0..3)
                    .map(|c| {
                        (0..n)
                            .map(|r| {
                                if c == 1 {
                                    (r % 397) as f64
                                } else {
                                    ((r * (c + 3)) as f64).sin()
                                }
                            })
                            .collect()
                    })
                    .collect();
                let y: Vec<_> = (0..n)
                    .map(|r| {
                        0.7 * columns[0][r] + columns[2][r] * columns[1][r].cos() + (r as f64).cos()
                    })
                    .collect();
                let input = LoadedOlsInput {
                    candidate: vec![true; n],
                    outcome: loaded(&y),
                    numeric_sources: vec![
                        (x.id, loaded(&columns[0])),
                        (group.id, loaded(&columns[1])),
                        (slope.id, loaded(&columns[2])),
                    ],
                    analytic_weight: None,
                    frequency_weight: None,
                    probability_weight: None,
                };
                let data = crate::prepared_categorical_data::prepare_categorical_ols_data_owned(
                    &request, input,
                )
                .unwrap();
                let config = PreparedOlsFitConfig::default();

                let admitted =
                    crate::fe::FeSession::for_ols(&data, BorrowedMapWeights::Unit, config).unwrap();

                drop(admitted);

                let mut legacy = config;
                legacy.absorb.solver = FeSolverKind::Map;
                let expected = fit_with_controls_internal(
                    &data,
                    legacy,
                    &mut SystemFitReservation,
                    FitPerturbations::default(),
                )
                .unwrap();
                for saved in [false, true] {
                    let mut current = config;
                    current.absorb.retain_coefficients = saved;
                    current.allow_final_fe_recovery = saved;
                    let actual = fit_with_controls_internal(
                        &data,
                        current,
                        &mut SystemFitReservation,
                        FitPerturbations::default(),
                    )
                    .unwrap();
                    assert_eq!(actual.finite_rank(), expected.finite_rank());
                    for (a, b) in actual.coefficients().iter().zip(expected.coefficients()) {
                        assert_eq!(a.status, b.status);
                        match (a.value, b.value) {
                            (Some(a), Some(b)) => assert_close(a, b, 1e-10),
                            (None, None) => {}
                            _ => panic!("coefficient presence changed"),
                        }
                    }
                    for (&a, &b) in actual.residuals().iter().zip(expected.residuals()) {
                        assert_close(a, b, 1e-10);
                    }
                    assert!(actual.score_certificate().passed);
                }
            }
        }
    }

    #[test]
    fn multipartition_slope_owner_preserves_span_and_admission_boundaries() {
        let g = source(1, "g");
        let h = source(2, "h");
        let z = source(3, "z");
        let n = 280;
        let groups: Vec<_> = (0..n).map(|r| (r % 5) as f64).collect();
        let other: Vec<_> = (0..n).map(|r| (r % 7) as f64).collect();
        let slope: Vec<_> = (0..n).map(|r| (r as f64 / 11.).sin()).collect();
        let outcome: Vec<_> = (0..n)
            .map(|r| 0.3 * groups[r] + groups[r].cos() * slope[r] - 0.2 * other[r])
            .collect();
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![g.clone(), h.clone(), z.clone()],
                terms: Vec::new(),
            },
            absorbed_terms: vec![
                AbsorbedTerm {
                    id: AbsorbTermId::new(1).unwrap(),
                    intercept: true,
                    categorical_sources: vec![g.id],
                    slope_sources: vec![z.id],
                    label: "g##c.z".into(),
                    saved_name: String::new(),
                },
                AbsorbedTerm {
                    id: AbsorbTermId::new(2).unwrap(),
                    intercept: true,
                    categorical_sources: vec![h.id],
                    slope_sources: Vec::new(),
                    label: "h".into(),
                    saved_name: String::new(),
                },
            ],
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
                    role: OlsInputRole::NumericSource(g.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(h.id),
                },
                OlsInputBinding {
                    stata_var_position: 5,
                    role: OlsInputRole::NumericSource(z.id),
                },
            ],
            include_intercept: false,
            keep_singletons: true,
        };
        let input = LoadedOlsInput {
            candidate: vec![true; n],
            outcome: loaded(&outcome),
            numeric_sources: vec![
                (g.id, loaded(&groups)),
                (h.id, loaded(&other)),
                (z.id, loaded(&slope)),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let data =
            crate::prepared_categorical_data::prepare_categorical_ols_data_owned(&request, input)
                .unwrap();
        let supplied: Vec<_> = (0..n).map(|r| 0.1 + (r % 13) as f64).collect();
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for threads in [1, 4] {
                for weights in [
                    BorrowedMapWeights::Unit,
                    BorrowedMapWeights::Provided(&supplied),
                ] {
                    let mut config = PreparedOlsFitConfig::default();
                    config.execution.mode = mode;
                    config.execution.poolsize = 1;
                    config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
                    ThreadTeam::new(config.execution.threads)
                        .install(|| {
                            let mut session =
                                crate::fe::FeSession::for_ols(&data, weights, config).unwrap();

                            let raw = DenseMatrix::from_row_major(n, 1, outcome.clone()).unwrap();
                            let actual = session
                                .project_ols(weights, raw, config.absorb, false, false)
                                .unwrap();
                            assert!(actual.report().certified());
                            assert!(actual
                                .residualized()
                                .as_slice()
                                .iter()
                                .all(|v| v.abs() < 1e-9));
                            let peak =
                                actual.report().solver_workspace_peak.bytes().unwrap() as u128;
                            let mut refused = config;
                            refused.absorb.solver_memory_budget =
                                Some(crate::memory::FeSolverMemoryBudget {
                                    execution_bytes: 100,
                                    usable_bytes: 100,
                                });
                            assert_eq!(
                                crate::fe::FeSession::for_ols(&data, weights, refused)
                                    .err()
                                    .unwrap()
                                    .code,
                                ErrorCode::Allocation
                            );
                            let mut bounded = config;
                            bounded.absorb.solver_memory_budget =
                                Some(crate::memory::FeSolverMemoryBudget {
                                    execution_bytes: 100,
                                    usable_bytes: 100 + peak,
                                });

                            let residual = DenseMatrix::from_row_major(n, 1, vec![0.; n]).unwrap();
                            let actual = session
                                .project_ols(weights, residual, config.absorb, true, false)
                                .unwrap();
                            assert_eq!(actual.report().iterations, 0);
                        })
                        .unwrap();
                }
            }
        }
    }

    #[test]
    fn generic_prepared_slope_admits_owned_design_unit_weights_and_solver_together() {
        let group = source(1, "group");
        let slope = source(2, "slope");
        let n = 12;
        let groups = (0..n).map(|row| (row / 6) as f64).collect::<Vec<_>>();
        let slopes = (0..n).map(|row| (row % 6) as f64 - 2.0).collect::<Vec<_>>();
        let noise = (0..n)
            .map(|row| [1.0, -2.0, 1.0][row % 3])
            .collect::<Vec<_>>();
        let y = (0..n)
            .map(|row| 3.0 * groups[row] + 2.0 * slopes[row] + noise[row])
            .collect::<Vec<_>>();
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![group.clone(), slope.clone()],
                terms: Vec::new(),
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![group.id],
                slope_sources: vec![slope.id],
                label: "group##c.slope".into(),
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
                    role: OlsInputRole::NumericSource(group.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(slope.id),
                },
            ],
            include_intercept: false,
            keep_singletons: true,
        };
        let input = LoadedOlsInput {
            candidate: vec![true; n],
            outcome: loaded(&y),
            numeric_sources: vec![(group.id, loaded(&groups)), (slope.id, loaded(&slopes))],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared =
            crate::prepared_categorical_data::prepare_categorical_ols_data(&request, &input)
                .unwrap();
        let memory = prepared.fit_owned_fe_design_memory().unwrap();
        let design = prepared.fit_owned_fe_design().unwrap();
        assert_eq!(
            memory.resident_bytes,
            design.storage_capacities().resident_bytes
        );
        let supplied = (0..n).map(|row| 1.0 + (row / 6) as f64).collect::<Vec<_>>();
        for weights in [
            BorrowedMapWeights::Unit,
            BorrowedMapWeights::Provided(&supplied),
        ] {
            let map_config = AbsorbConfig {
                solver: FeSolverKind::Map,
                ..AbsorbConfig::default()
            };
            let map = prepared
                .fit_absorb_matrix(
                    weights,
                    DenseMatrix::from_columns(&[y.clone()]).unwrap(),
                    map_config,
                )
                .unwrap();
            let map_bytes = map.report().solver_workspace_peak.bytes().unwrap() as u128;
            for missing in [0, 1] {
                let mut bounded = map_config;
                bounded.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
                    execution_bytes: 137,
                    usable_bytes: 137 + map_bytes - missing,
                });
                let result = prepared.fit_absorb_matrix(
                    weights,
                    DenseMatrix::from_columns(&[y.clone()]).unwrap(),
                    bounded,
                );
                if missing == 0 {
                    assert_eq!(result.unwrap().residualized(), map.residualized());
                } else {
                    assert_eq!(result.unwrap_err().code, ErrorCode::Allocation);
                }
            }
            for solver in [
                FeSolverKind::Cg,
                FeSolverKind::Lsmr,
                FeSolverKind::IronsTuck,
            ] {
                let mut config = AbsorbConfig {
                    solver,
                    ..AbsorbConfig::default()
                };
                let solver_bytes =
                    crate::memory::generic_fe_solver_bytes(&design, 1, solver, config.lsmr)
                        .unwrap()
                        + if solver == FeSolverKind::Lsmr {
                            (8 * n) as u128
                        } else {
                            0
                        };
                let required = memory
                    .construction_bytes
                    .max(memory.resident_bytes + solver_bytes)
                    + (32 * n) as u128;
                for usable in [1, 137 + required - 1, 137 + required] {
                    config.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
                        execution_bytes: 137,
                        usable_bytes: usable,
                    });
                    let result = prepared.fit_absorb_matrix(
                        weights,
                        DenseMatrix::from_columns(&[y.clone()]).unwrap(),
                        config,
                    );
                    if usable < 137 + required {
                        assert_eq!(result.unwrap_err().code, ErrorCode::Allocation);
                    } else {
                        let output = result.unwrap();
                        assert!(output.report().certified());
                        assert_eq!(
                            output.report().solver_workspace_peak.bytes(),
                            Some(required as u64)
                        );
                        assert!(output
                            .residualized()
                            .as_slice()
                            .iter()
                            .zip(&noise)
                            .all(|(a, b)| (a - b).abs() < 1e-10));
                    }
                }
            }
        }
    }

    #[test]
    fn saved_final_residual_removes_small_fe_components_across_solver_families() {
        let n = 512 * 16 * 4;
        let effects = [
            (0..n).map(|row| (row % 512) as f64).collect::<Vec<_>>(),
            (0..n)
                .map(|row| ((row / 512) % 16) as f64)
                .collect::<Vec<_>>(),
        ];
        // Each crossed FE cell has two +1 and two -1 observations with
        // identical weights: its exact projected residual is known directly.
        let residuals = (0..n)
            .map(|row| {
                if (row / (512 * 16)) % 2 == 0 {
                    -1.0
                } else {
                    1.0
                }
            })
            .collect::<Vec<_>>();
        let raw_weights = (0..n)
            .map(|row| 1.0 + ((row % (512 * 16)) % 3) as f64)
            .collect::<Vec<_>>();
        let prepared = prepared_multiway_categorical(&residuals, &[], &effects);
        for weights in [
            BorrowedMapWeights::Unit,
            BorrowedMapWeights::Provided(&raw_weights),
        ] {
            for retain in [false, true] {
                for solver in [
                    FeSolverKind::Auto,
                    FeSolverKind::Cg,
                    FeSolverKind::Map,
                    FeSolverKind::Lsmr,
                    FeSolverKind::Cmg,
                ] {
                    let mut config = PreparedOlsFitConfig::default();
                    config.absorb.solver = solver;
                    config.absorb.retain_coefficients = retain;
                    config.final_residual_tolerance = Some(1e-12);
                    config.absorb.map.tolerance = 1e-8;
                    let mut session =
                        crate::fe::FeSession::for_ols(&prepared, weights, config).unwrap();
                    let pending = (0..n)
                        .map(|row| residuals[row] + if row % 512 == 0 { 2e-9 } else { 0.0 })
                        .collect();
                    let projected = session
                        .project_ols(
                            weights,
                            DenseMatrix::from_row_major(n, 1, pending).unwrap(),
                            config.final_absorb_config(),
                            true,
                            false,
                        )
                        .unwrap();
                    assert!(projected.report().certified());
                    let (actual, _, _) = projected.into_full_parts();
                    let error = actual
                        .as_slice()
                        .iter()
                        .zip(&residuals)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0_f64, f64::max);
                    assert!(
                        error <= 1e-11,
                        "{solver:?} final residual retains an FE component: {error}"
                    );
                }
            }
        }
    }

    #[test]
    fn saved_slope_recovery_admits_each_attempt_and_keeps_map_fallback() {
        let group = source(1, "group");
        let slope = source(2, "slope");
        for n in [12, 16_386] {
            let groups = (0..n).map(|row| (row / (n / 2)) as f64).collect::<Vec<_>>();
            let slopes = (0..n).map(|row| (row % 6) as f64 - 2.0).collect::<Vec<_>>();
            let noise = (0..n)
                .map(|row| [1.0, -2.0, 1.0][row % 3])
                .collect::<Vec<_>>();
            let y = (0..n)
                .map(|row| 3.0 * groups[row] + 2.0 * slopes[row] + noise[row])
                .collect::<Vec<_>>();
            let request = OlsRequestV1 {
                model: CompiledTermGraph {
                    sources: vec![group.clone(), slope.clone()],
                    terms: Vec::new(),
                },
                absorbed_terms: vec![AbsorbedTerm {
                    id: AbsorbTermId::new(1).unwrap(),
                    intercept: true,
                    categorical_sources: vec![group.id],
                    slope_sources: vec![slope.id],
                    label: "group##c.slope".into(),
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
                        role: OlsInputRole::NumericSource(group.id),
                    },
                    OlsInputBinding {
                        stata_var_position: 4,
                        role: OlsInputRole::NumericSource(slope.id),
                    },
                ],
                include_intercept: false,
                keep_singletons: true,
            };
            let input = LoadedOlsInput {
                candidate: vec![true; n],
                outcome: loaded(&y),
                numeric_sources: vec![(group.id, loaded(&groups)), (slope.id, loaded(&slopes))],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            };
            let prepared =
                crate::prepared_categorical_data::prepare_categorical_ols_data(&request, &input)
                    .unwrap();
            let target = y.iter().zip(&noise).map(|(y, e)| y - e).collect::<Vec<_>>();
            for budget in [
                None,
                Some(crate::memory::FeSolverMemoryBudget {
                    execution_bytes: 0,
                    usable_bytes: 1,
                }),
            ] {
                let error = prepared
                    .recover_model_effects(
                        &target,
                        &noise,
                        10.0,
                        &[true, true],
                        false,
                        true,
                        crate::fe::MapConfig::default(),
                        budget,
                    )
                    .unwrap_err();
                assert_eq!(error.code, ErrorCode::InvalidArgument);
                assert_eq!(
                    error.message,
                    "model FE recovery requires intercept-only effects"
                );
            }
            let weights = (0..n).map(|row| 1.0 + (row % 3) as f64).collect::<Vec<_>>();
            for workers in [1, 4] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .build()
                    .unwrap()
                    .install(|| {
                        for weights in [
                            BorrowedMapWeights::Unit,
                            BorrowedMapWeights::Provided(&weights),
                        ] {
                            let lsmr = crate::fe::LsmrConfig::default();
                            let map = crate::fe::MapConfig {
                                tolerance: 1e-12,
                                ..Default::default()
                            };
                            let memory = crate::memory::estimate_slope_recovery_memory(
                                n,
                                &[(2, true, 1)],
                                2,
                                prepared.fe_topology().compacted_sources().len(),
                                prepared.fe_topology().validation_workspace_bytes().unwrap(),
                                workers,
                                lsmr.local_size,
                            )
                            .unwrap();
                            let run = |workspace| {
                                prepared.recover_absorbed_effects_fast_with_weights(
                                    weights,
                                    &target,
                                    &[true, true],
                                    None,
                                    false,
                                    true,
                                    lsmr,
                                    map,
                                    Some(crate::memory::FeSolverMemoryBudget {
                                        execution_bytes: 137,
                                        usable_bytes: 137 + workspace,
                                    }),
                                )
                            };
                            assert_eq!(
                                run(memory.map_peak_bytes - 1).unwrap_err().code,
                                ErrorCode::Allocation
                            );
                            assert_eq!(run(0).unwrap_err().code, ErrorCode::Allocation);
                            let tight = run(memory.map_peak_bytes).unwrap();
                            assert_eq!(
                                tight.recovery_method(),
                                crate::fe::CategoricalRecoveryMethod::Map
                            );
                            assert_eq!(
                                tight.solver_workspace_peak().bytes(),
                                Some(memory.map_peak_bytes as u64)
                            );
                            let direct_bytes = memory.retained_peak_bytes
                                + crate::fe::single_slope_recovery_extra_bytes(n, 2, 2, workers)
                                    .unwrap();
                            let ample = run(memory.lsmr_peak_bytes.max(direct_bytes)).unwrap();
                            if true {
                                assert_eq!(
                                    ample.recovery_method(),
                                    crate::fe::CategoricalRecoveryMethod::Direct
                                );
                                let cached_bytes = direct_bytes
                                    + crate::fe::single_slope_recovery_geometry_bytes(2, 2)
                                        .unwrap();
                                let expected = direct_bytes;
                                assert_eq!(
                                    ample.solver_workspace_peak().bytes(),
                                    Some(expected as u64)
                                );
                                let uncached = run(direct_bytes).unwrap();
                                assert_eq!(
                                    uncached.recovery_method(),
                                    crate::fe::CategoricalRecoveryMethod::Direct
                                );
                                assert_eq!(
                                    uncached.solver_workspace_peak().bytes(),
                                    Some(direct_bytes as u64)
                                );
                                if workers == 1 {
                                    let cached = run(cached_bytes).unwrap();
                                    assert_eq!(
                                        cached.solver_workspace_peak().bytes(),
                                        Some(direct_bytes as u64)
                                    );
                                    assert_eq!(cached.channels(), uncached.channels());
                                    assert_eq!(
                                        run(cached_bytes - 1)
                                            .unwrap()
                                            .solver_workspace_peak()
                                            .bytes(),
                                        Some(direct_bytes as u64)
                                    );
                                }
                                assert_eq!(
                                    run(direct_bytes - 1).unwrap().recovery_method(),
                                    crate::fe::CategoricalRecoveryMethod::Map
                                );
                            } else {
                                assert_eq!(
                                    ample.solver_workspace_peak().bytes(),
                                    Some(memory.lsmr_peak_bytes as u64)
                                );
                            }
                            let coefficients = crate::fe::FeCoefficientBatch::from_values(
                                vec![2],
                                vec![2],
                                1,
                                vec![0.0, 3.0, 2.0, 2.0],
                            )
                            .unwrap();
                            let retained = prepared
                                .recover_absorbed_effects_fast_with_weights(
                                    weights,
                                    &target,
                                    &[true, true],
                                    Some(&coefficients),
                                    false,
                                    true,
                                    lsmr,
                                    map,
                                    Some(crate::memory::FeSolverMemoryBudget {
                                        execution_bytes: 137,
                                        usable_bytes: 137 + memory.retained_peak_bytes,
                                    }),
                                )
                                .unwrap();
                            assert_eq!(
                                retained.recovery_method(),
                                crate::fe::CategoricalRecoveryMethod::RetainedCoefficients
                            );
                            assert_eq!(
                                retained.solver_workspace_peak().bytes(),
                                Some(memory.retained_peak_bytes as u64)
                            );
                            for recovered in [tight, ample, retained] {
                                for (row, expected) in target.iter().enumerate() {
                                    assert!(
                                        (recovered.absorbed_sum()[row] - expected).abs() < 1e-10
                                    );
                                    let sum = recovered
                                        .channels()
                                        .iter()
                                        .flatten()
                                        .map(|channel| channel[row])
                                        .sum::<f64>();
                                    assert!((sum - expected).abs() < 1e-10);
                                }
                            }
                        }
                    });
            }
        }
    }

    #[test]
    fn saved_intercept_recovery_enforces_joint_budget_before_materialization() {
        let n = 96;
        let effects = [
            (0..n).map(|row| (row % 4) as f64).collect::<Vec<_>>(),
            (0..n).map(|row| ((row / 4) % 3) as f64).collect::<Vec<_>>(),
        ];
        let target = (0..n)
            .map(|row| 2.0 * effects[0][row] + 3.0 * effects[1][row])
            .collect::<Vec<_>>();
        let prepared = prepared_multiway_categorical(&target, &[], &effects);
        for workers in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    let lsmr = crate::fe::LsmrConfig::default();
                    let map = crate::fe::MapConfig {
                        tolerance: 1e-12,
                        ..crate::fe::MapConfig::default()
                    };
                    let memory = crate::memory::estimate_intercept_recovery_memory(
                        n,
                        &[4, 3],
                        2,
                        workers,
                        lsmr.local_size,
                    )
                    .unwrap();
                    let workspace = memory.cg_peak_bytes;
                    let execution = 137;
                    let run = |usable_bytes| {
                        prepared.recover_absorbed_effects_fast(
                            &target,
                            &[true, true],
                            None,
                            false,
                            true,
                            lsmr,
                            map,
                            Some(crate::memory::FeSolverMemoryBudget {
                                execution_bytes: execution,
                                usable_bytes,
                            }),
                        )
                    };
                    let error =
                        run(execution + memory.map_peak_bytes.min(memory.retained_peak_bytes) - 1)
                            .unwrap_err();
                    assert_eq!(error.code, ErrorCode::Allocation);
                    assert!(error
                        .message
                        .contains("joint execution/recovery memory plan"));
                    let recovered = run(execution + workspace).unwrap();
                    assert_eq!(
                        recovered.recovery_method(),
                        crate::fe::CategoricalRecoveryMethod::Direct
                    );
                    assert_eq!(
                        recovered.solver_workspace_peak().bytes(),
                        Some(memory.retained_peak_bytes as u64)
                    );
                    let tight = run(execution + memory.map_peak_bytes).unwrap();
                    assert_eq!(
                        tight.recovery_method(),
                        if memory.retained_peak_bytes <= memory.map_peak_bytes {
                            crate::fe::CategoricalRecoveryMethod::Direct
                        } else {
                            crate::fe::CategoricalRecoveryMethod::Map
                        }
                    );
                    assert_eq!(
                        tight.solver_workspace_peak().bytes(),
                        Some(memory.map_peak_bytes.min(memory.retained_peak_bytes) as u64)
                    );
                    assert!(tight
                        .absorbed_sum()
                        .iter()
                        .zip(&target)
                        .all(|(actual, expected)| (actual - expected).abs() <= 1e-10));
                    // Keep the legacy matrix indices and numerical traversal order explicit.
                    #[allow(clippy::needless_range_loop)]
                    for row in 0..n {
                        assert!((recovered.absorbed_sum()[row] - target[row]).abs() <= 1e-10);
                        assert!(
                            (recovered.channels()[0].as_ref().unwrap()[row]
                                - (2.0 * effects[0][row] + 3.0))
                                .abs()
                                <= 1e-10
                        );
                        assert!(
                            (recovered.channels()[1].as_ref().unwrap()[row]
                                - (3.0 * effects[1][row] - 3.0))
                                .abs()
                                <= 1e-10
                        );
                    }
                    let retained = crate::fe::FeCoefficientBatch::from_values(
                        vec![1, 1],
                        vec![4, 3],
                        1,
                        vec![0.0, 2.0, 4.0, 6.0, 0.0, 3.0, 6.0],
                    )
                    .unwrap();
                    let retained_run = |coefficients, bytes| {
                        prepared.recover_absorbed_effects_fast(
                            &target,
                            &[true, true],
                            Some(coefficients),
                            false,
                            true,
                            lsmr,
                            map,
                            Some(crate::memory::FeSolverMemoryBudget {
                                execution_bytes: execution,
                                usable_bytes: execution + bytes,
                            }),
                        )
                    };
                    assert!(memory.retained_peak_bytes < workspace);
                    let normalized = retained_run(&retained, memory.retained_peak_bytes).unwrap();
                    assert_eq!(
                        normalized.recovery_method(),
                        crate::fe::CategoricalRecoveryMethod::RetainedCoefficients
                    );
                    assert_eq!(
                        normalized.solver_workspace_peak().bytes(),
                        Some(memory.retained_peak_bytes as u64)
                    );
                    assert!(normalized
                        .absorbed_sum()
                        .iter()
                        .zip(&target)
                        .all(|(actual, expected)| (actual - expected).abs() <= 1e-10));
                    assert_eq!(
                        retained_run(&retained, memory.retained_peak_bytes - 1)
                            .unwrap_err()
                            .code,
                        ErrorCode::Allocation
                    );
                    let incorrect = crate::fe::FeCoefficientBatch::from_values(
                        vec![1, 1],
                        vec![4, 3],
                        1,
                        vec![0.0; 7],
                    )
                    .unwrap();
                    let fallback = retained_run(&incorrect, memory.retained_peak_bytes).unwrap();
                    assert_eq!(
                        fallback.recovery_method(),
                        crate::fe::CategoricalRecoveryMethod::Direct
                    );
                    assert_eq!(
                        fallback.solver_workspace_peak().bytes(),
                        Some(memory.map_peak_bytes.min(memory.retained_peak_bytes) as u64)
                    );
                    assert!(fallback
                        .absorbed_sum()
                        .iter()
                        .zip(&target)
                        .all(|(actual, expected)| (actual - expected).abs() <= 1e-10));
                    let corrected = retained_run(&incorrect, workspace).unwrap();
                    assert_eq!(
                        corrected.solver_workspace_peak().bytes(),
                        Some(memory.retained_peak_bytes as u64)
                    );
                    assert!(corrected
                        .absorbed_sum()
                        .iter()
                        .zip(&target)
                        .all(|(actual, expected)| (actual - expected).abs() <= 1e-10));
                });
        }
    }

    fn prepared_multiway_categorical(
        outcome: &[f64],
        regressors: &[Vec<f64>],
        effects: &[Vec<f64>],
    ) -> crate::PreparedCategoricalOlsData {
        prepared_multiway_categorical_weights(outcome, regressors, effects, TestWeights::Unit)
    }

    fn prepared_multiway_categorical_weights(
        outcome: &[f64],
        regressors: &[Vec<f64>],
        effects: &[Vec<f64>],
        weights: TestWeights<'_>,
    ) -> crate::PreparedCategoricalOlsData {
        assert!((1..=3).contains(&effects.len()));
        assert!(regressors
            .iter()
            .chain(effects)
            .all(|values| values.len() == outcome.len()));
        let mut sources = Vec::new();
        let mut terms = Vec::new();
        let mut absorbed_terms = Vec::new();
        let mut numeric_sources = Vec::new();
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ];
        for (index, values) in regressors.iter().enumerate() {
            let one_based = u32::try_from(index + 1).expect("small control identity");
            let entry = source(one_based, &format!("x{one_based}"));
            terms.push(continuous_term(one_based, entry.id, false));
            bindings.push(OlsInputBinding {
                stata_var_position: u32::try_from(bindings.len() + 1)
                    .expect("small control binding"),
                role: OlsInputRole::NumericSource(entry.id),
            });
            numeric_sources.push((entry.id, loaded(values)));
            sources.push(entry);
        }
        for (effect, values) in effects.iter().enumerate() {
            let source_id =
                u32::try_from(regressors.len() + effect + 1).expect("small fixed-effect identity");
            let entry = source(source_id, &format!("g{}", effect + 1));
            bindings.push(OlsInputBinding {
                stata_var_position: u32::try_from(bindings.len() + 1)
                    .expect("small fixed-effect binding"),
                role: OlsInputRole::NumericSource(entry.id),
            });
            numeric_sources.push((entry.id, loaded(values)));
            absorbed_terms.push(AbsorbedTerm {
                id: AbsorbTermId::new(u32::try_from(effect + 1).expect("small absorb identity"))
                    .expect("absorb identity"),
                intercept: true,
                categorical_sources: vec![entry.id],
                slope_sources: Vec::new(),
                label: format!("g{}", effect + 1),
                saved_name: String::new(),
            });
            sources.push(entry);
        }
        let (role, analytic_weight, probability_weight, frequency_weight) = match weights {
            TestWeights::Unit => (None, None, None, None),
            TestWeights::Analytic(values) => (
                Some(OlsInputRole::AnalyticWeight),
                Some(loaded(values)),
                None,
                None,
            ),
            TestWeights::Probability(values) => (
                Some(OlsInputRole::ProbabilityWeight),
                None,
                Some(loaded(values)),
                None,
            ),
            TestWeights::Frequency(values) => (
                Some(OlsInputRole::FrequencyWeight),
                None,
                None,
                Some(loaded(values)),
            ),
        };
        if let Some(role) = role {
            bindings.push(OlsInputBinding {
                stata_var_position: (bindings.len() + 1) as u32,
                role,
            });
        }
        crate::prepare_categorical_ols_data(
            &OlsRequestV1 {
                model: CompiledTermGraph { sources, terms },
                absorbed_terms,
                bindings,
                include_intercept: true,
                keep_singletons: true,
            },
            &LoadedOlsInput {
                candidate: vec![true; outcome.len()],
                outcome: loaded(outcome),
                numeric_sources,
                analytic_weight,
                frequency_weight,
                probability_weight,
            },
        )
        .expect("prepared multiway categorical data")
    }

    fn assert_initial_checkpoint_lsmr_matches_map(
        prepared: &crate::PreparedCategoricalOlsData,
        threads: usize,
    ) -> FeSolverKind {
        let mut config = PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode: PreparedOlsExecutionMode::Fast,
                poolsize: 8,
                threads: ThreadConfig::from_requested(threads, true).expect("test thread config"),
            },
            ..PreparedOlsFitConfig::default()
        };
        config.absorb.solver = FeSolverKind::Map;
        config.absorb.map.tolerance = 1e-6;
        config.absorb.lsmr.tolerance = 1e-6;
        let frame = LinearSolveFrame::from_prepared_ols(prepared).expect("initial frame");
        let generations = LinearSessionGenerations::new(1, 1, 1, 1, 1);
        let mut map = CertifiedLinearSession::new_with_fe_design(
            prepared,
            frame,
            config,
            generations,
            prepared.fit_owned_fe_design().expect("MAP FE design"),
        )
        .expect("MAP initial session");
        let mut lsmr = CertifiedLinearSession::new_with_fe_design(
            prepared,
            frame,
            config,
            generations,
            prepared.fit_owned_fe_design().expect("LSMR FE design"),
        )
        .expect("LSMR initial session");
        lsmr.enable_checkpoint_lsmr(true);
        let expected = map.solve_fresh().expect("MAP initial solve");
        let actual = lsmr.solve_fresh().expect("LSMR initial solve");
        assert!(actual.certified());
        assert_eq!(
            actual.frame().absorption_report().requested_solver,
            FeSolverKind::Lsmr,
            "initial projection report: {:?}",
            actual.frame().absorption_report(),
        );
        let used_solver = actual.frame().absorption_report().used_solver;
        assert!(matches!(
            used_solver,
            FeSolverKind::Lsmr | FeSolverKind::Map
        ));
        // Bounded two/three-FE Schur checkpoints report Map on success without
        // a failed LSMR attempt. Only a true fallback must report Map.
        if actual.frame().absorption_report().fallback_used {
            assert_eq!(used_solver, FeSolverKind::Map);
        } else if used_solver == FeSolverKind::Map {
            assert!(matches!(
                prepared.fit_borrowed_cmg_design().unwrap().effects().len(),
                2 | 3
            ));
        }
        assert_eq!(actual.frame().finite_rank(), expected.frame().finite_rank());
        assert_eq!(
            actual.basis().kept_generated_columns(),
            expected.basis().kept_generated_columns()
        );
        assert_eq!(
            actual.basis().dropped_generated_columns(),
            expected.basis().dropped_generated_columns()
        );
        assert_eq!(
            actual
                .coefficients()
                .map(|coefficient| coefficient.status)
                .collect::<Vec<_>>(),
            expected
                .coefficients()
                .map(|coefficient| coefficient.status)
                .collect::<Vec<_>>()
        );
        for (actual, expected) in actual.coefficients().zip(expected.coefficients()) {
            match (actual.value, expected.value) {
                (Some(actual), Some(expected)) => assert_close(actual, expected, 2e-6),
                (None, None) => {}
                _ => panic!("initial checkpoint coefficient presence differs"),
            }
        }
        for (&actual, &expected) in actual.frame().bread().iter().zip(expected.frame().bread()) {
            assert_close(actual, expected, 2e-6);
        }
        assert!(actual.frame().rank_certificate().passed);
        assert!(actual.frame().score_certificate().passed);
        assert!(actual.frame().canonical_proof().unwrap().bread.passed);
        used_solver
    }

    #[test]
    fn bound_score_means_preserve_cache_bits_and_reservations() {
        for (rows, regressors, intercept, absorbed) in [
            (4095, 6, false, false),
            (4096, 1, true, false),
            (32771, 10, true, true),
            (4103, 63, true, false),
            (4103, 64, true, true),
            (32771, 6, false, true),
            (4096, 0, true, true),
        ] {
            let x = (0..regressors)
                .map(|column| {
                    (0..rows)
                        .map(|row| match column % 7 {
                            0 => -0.0,
                            1 => ((row * 37 % 101) as f64 - 50.0) * 1e150,
                            2 => ((row * 19 % 97) as f64 - 48.0) * 1e-150,
                            3 => [1e16, 1.0, -1e16][row % 3],
                            4 => 1.0,
                            _ => (row as f64 * 0.173).sin(),
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let groups = (0..rows).map(|row| (row % 17) as f64).collect::<Vec<_>>();
            let raw_weights = (0..rows)
                .map(|row| (row % 13 + 1) as f64)
                .collect::<Vec<_>>();
            for weight in [
                TestWeights::Unit,
                TestWeights::Analytic(&raw_weights),
                TestWeights::Probability(&raw_weights),
                TestWeights::Frequency(&raw_weights),
            ] {
                let data = prepared(
                    &vec![1.0; rows],
                    &x,
                    &vec![false; regressors],
                    absorbed.then_some(groups.as_slice()),
                    intercept,
                    weight,
                );
                let columns = data.model.columns.len();
                let binding = super::super::prepared_rhs::NumericOlsColumns::bind_score_means(
                    &data.model,
                    &data.sample,
                )
                .unwrap();
                assert_eq!(
                    binding.is_some(),
                    rows >= 4096 && (1..=64).contains(&columns)
                );
                let mut within = DenseMatrix::zeros(rows, columns + 1).unwrap();
                let scales = (0..columns)
                    .map(|column| {
                        for row in 0..rows {
                            within[(row, column + 1)] = (row as f64 - 13.0) / (column + 1) as f64;
                        }
                        (0..rows)
                            .map(|row| data.column_value(column, row).unwrap().abs())
                            .fold(0.0_f64, f64::max)
                    })
                    .collect::<Vec<_>>();
                let weights = normalized_weights(&data).unwrap();
                for threads in [1, 4] {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap()
                        .install(|| {
                            let run = |bound| {
                                let mut reservation = InjectedReservation::default();
                                let cache = build_fast_score_cache(
                                    &data,
                                    &within,
                                    &scales,
                                    weights,
                                    &mut reservation,
                                    usize::MAX,
                                    bound,
                                )
                                .unwrap()
                                .unwrap();
                                let PreparedOlsFastScoreCache::Packed {
                                    generated_columns,
                                    within,
                                    means,
                                } = cache
                                else {
                                    panic!("expected packed score cache")
                                };
                                (
                                    generated_columns,
                                    within
                                        .as_slice()
                                        .iter()
                                        .map(|v| v.to_bits())
                                        .collect::<Vec<_>>(),
                                    means.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                                    reservation.records,
                                )
                            };
                            assert_eq!(run(true), run(false),
                            "rows={rows} columns={columns} absorbed={absorbed} threads={threads}");
                        });
                }
            }
        }
    }

    #[test]
    fn bound_score_means_preserve_budget_and_failure_checks() {
        let rows = 4096;
        let data = prepared(
            &vec![1.0; rows],
            &[vec![2.0; rows]],
            &[false],
            None,
            true,
            TestWeights::Unit,
        );
        let weights = normalized_weights(&data).unwrap();
        let mut within = DenseMatrix::zeros(rows, 3).unwrap();
        for bound in [false, true] {
            let mut reservation = InjectedReservation::default();
            assert!(build_fast_score_cache(
                &data,
                &within,
                &[2.0, 1.0],
                weights,
                &mut reservation,
                0,
                bound
            )
            .unwrap()
            .is_none());
            assert_eq!(reservation.calls, 1);
            assert!(build_fast_score_cache(
                &data,
                &within,
                &[1.0],
                weights,
                &mut SystemFitReservation,
                usize::MAX,
                bound
            )
            .is_err());
            for fail_at in 1..=4 {
                let mut reservation = InjectedReservation {
                    fail_at: Some(fail_at),
                    ..InjectedReservation::default()
                };
                assert!(build_fast_score_cache(
                    &data,
                    &within,
                    &[2.0, 1.0],
                    weights,
                    &mut reservation,
                    usize::MAX,
                    bound
                )
                .is_err());
            }
            within[(rows - 1, 1)] = f64::INFINITY;
            assert!(build_fast_score_cache(
                &data,
                &within,
                &[2.0, 1.0],
                weights,
                &mut SystemFitReservation,
                usize::MAX,
                bound
            )
            .is_err());
            within[(rows - 1, 1)] = 0.0;
        }
    }

    fn test_projected_moments<R: FitReservation>(
        within: &DenseMatrix,
        candidates: &[usize],
        reservation: &mut R,
    ) -> Result<(Vec<f64>, Vec<f64>)> {
        use crate::ols::finite_wls_kernel::{
            fill_finite_wls_moments, FiniteDenseRows, FiniteResponse,
        };
        let source = FixedBlockMomentSource::Dense {
            values: FiniteDenseRows::projected(within),
            rows: within.nrows(),
            stored_columns: within.ncols() - 1,
            selected_columns: Some(candidates),
            response: FiniteResponse::Column(within, 0),
            weights: BorrowedMapWeights::Unit,
            solver_weight_divisor: 1.0,
            column_scales: None,
        };
        let layout = finite_wls_moment_layout(source)?;
        let mut partials = reserved_filled(
            reservation,
            layout.partial_elements,
            0.0,
            "test finite partials",
        )?;
        let mut rhs = reserved_filled(reservation, candidates.len(), 0.0, "test finite RHS")?;
        let mut gram = reserved_filled(
            reservation,
            candidates.len() * candidates.len(),
            0.0,
            "test finite Gram",
        )?;
        fill_finite_wls_moments(source, layout, &mut partials, &mut rhs, &mut gram)?;
        Ok((rhs, gram))
    }

    fn assert_packed_and_strided_score_caches_match(prepared: &PreparedOlsData) {
        let mut packed_reservation = InjectedReservation::default();
        let packed_fit = fit_with_controls_internal_mode(
            prepared,
            PreparedOlsFitConfig::default(),
            &mut packed_reservation,
            FitPerturbations::default(),
            true,
            MAX_FAST_SCORE_CACHE_BYTES,
        )
        .expect("packed score cache fit");
        let mut strided_reservation = InjectedReservation::default();
        let strided_fit = fit_with_controls_internal_mode(
            prepared,
            PreparedOlsFitConfig::default(),
            &mut strided_reservation,
            FitPerturbations::default(),
            true,
            0,
        )
        .expect("strided score cache fit");
        assert_eq!(packed_fit.coefficients(), strided_fit.coefficients());
        assert_eq!(packed_fit.bread(), strided_fit.bread());
        assert_eq!(packed_fit.residuals(), strided_fit.residuals());
        let PreparedOlsFastScoreCache::Packed {
            generated_columns: packed_columns,
            within: packed,
            means: packed_means,
        } = packed_fit.fast_score_cache.expect("packed score cache")
        else {
            panic!("expected packed score cache");
        };
        let PreparedOlsFastScoreCache::Selected {
            generated_columns: strided_columns,
            within: strided,
            means: strided_means,
        } = strided_fit.fast_score_cache.expect("strided score cache")
        else {
            panic!("expected strided score cache");
        };
        assert_eq!(packed_columns, strided_columns);
        assert_eq!(packed_means.len(), strided_means.len());
        for (actual, expected) in strided_means.iter().zip(&packed_means) {
            assert_close(*actual, *expected, 2e-12);
        }
        for row in 0..packed.nrows() {
            for (finite, generated) in strided_columns.iter().copied().enumerate() {
                let actual = strided[(row, generated + 1)];
                assert_close(actual, packed[(row, finite)], 2e-12);
            }
        }
    }

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        let scale = actual.abs().max(expected.abs()).max(1.0);
        assert!(
            (actual - expected).abs() <= tolerance * scale,
            "actual={actual:.17e}, expected={expected:.17e}, tolerance={tolerance:.3e}"
        );
    }

    fn without_solve_timings(mut fit: CertifiedPreparedOlsFit) -> CertifiedPreparedOlsFit {
        fit.absorption_report.setup_seconds = 0.0;
        fit.absorption_report.solve_seconds = 0.0;
        fit.residual_absorption_report.setup_seconds = 0.0;
        fit.residual_absorption_report.solve_seconds = 0.0;

        fit
    }

    #[test]
    fn loaded_moments_preserve_fit_bits_fallback_and_admission() {
        for (n, width) in [
            (16_383, 9),
            (16_384, 9),
            (32_771, 9),
            (16_384, 1),
            (16_384, 3),
            (16_384, 5),
        ] {
            let columns: Vec<Vec<f64>> = (0..width)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect()
                })
                .collect();
            let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
            let response: Vec<_> = (0..n)
                .map(|r| {
                    groups[r]
                        + columns[0][r] * 1.3
                        + columns[width - 1][r] * 0.7
                        + (r as f64).cos() * 0.1
                })
                .collect();
            let data = prepared(
                &response,
                &columns,
                &vec![false; width],
                Some(&groups),
                true,
                TestWeights::Unit,
            );
            let raw = data
                .fit_build_rhs_with_response(&data.fit_sample().outcome)
                .unwrap();
            let required = LoadedOlsRows::moment_workspace_bytes(n, raw.ncols());
            for threads in [4, 8, 16] {
                let mut config = PreparedOlsFitConfig::default();
                config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
                let expected = without_solve_timings(
                    fit_prepared_ols_coefficients_map(&data, config).unwrap(),
                );
                for short in [false, true] {
                    for stale in [false, true] {
                        let mut rows = raw.as_slice().to_vec();
                        if stale {
                            rows[1] += 0.25;
                        }
                        let budget = required.map_or(0, |b| b - usize::from(short));
                        let loaded = ThreadTeam::new(config.execution.threads)
                            .install(|| {
                                LoadedOlsRows::from(rows).with_unit_moments(n, raw.ncols(), budget)
                            })
                            .unwrap();
                        assert_eq!(
                            loaded.moments.is_some(),
                            required.is_some()
                                && !short
                                && crate::parallel::StagePlan::new(
                                    n as u128 * raw.ncols() as u128,
                                    2,
                                    threads
                                )
                                .lanes
                                    > 1
                        );
                        let actual = without_solve_timings(
                            fit_prepared_ols_coefficients_with_loaded_rows(&data, config, loaded)
                                .unwrap(),
                        );
                        assert!(
                            expected == actual,
                            "N={n}, width={width}, threads={threads}, short={short}, stale={stale}"
                        );
                    }
                }
            }
        }
        let mut reservation = LoadedMomentReservation { remaining: 7 };
        let mut values: Vec<f64> = Vec::new();
        assert!(reservation.reserve(&mut values, 1, "test").is_err());
        assert_eq!(values.capacity(), 0);
    }

    #[test]
    fn loaded_moment_errors_remain_after_validation_and_stale_rows_are_discarded() {
        for (n, width, workers) in [(257, 1, 1), (4095, 3, 4), (16_384, 8, 8)] {
            let columns: Vec<Vec<f64>> = (0..width)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect()
                })
                .collect();
            let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
            let response: Vec<_> = (0..n)
                .map(|r| groups[r] + columns[0][r] + (r as f64).cos())
                .collect();
            let mut data = prepared(
                &response,
                &columns,
                &vec![false; width],
                Some(&groups),
                true,
                TestWeights::Unit,
            );
            let raw = data
                .fit_build_rhs_with_response(&data.fit_sample().outcome)
                .unwrap();
            let mut config = PreparedOlsFitConfig::default();
            config.execution.threads = ThreadConfig::from_requested(workers, true).unwrap();
            let error = invalid_data("deferred raw moment error");
            let loaded = |stale| {
                let mut rows = raw.as_slice().to_vec();
                if stale {
                    rows[1] += 0.5;
                }
                LoadedOlsRows {
                    rows,
                    moments: Some(Err(error.clone())),
                }
            };
            assert_eq!(
                fit_prepared_ols_coefficients_with_loaded_rows(&data, config, loaded(false))
                    .unwrap_err(),
                error
            );
            let expected =
                without_solve_timings(fit_prepared_ols_coefficients_map(&data, config).unwrap());
            let actual = without_solve_timings(
                fit_prepared_ols_coefficients_with_loaded_rows(&data, config, loaded(true))
                    .unwrap(),
            );
            assert!(actual == expected);
            data.sample.outcome[0] = f64::NAN;
            assert_eq!(
                fit_prepared_ols_coefficients_with_loaded_rows(&data, config, loaded(false))
                    .unwrap_err(),
                fit_prepared_ols_coefficients_map(&data, config).unwrap_err()
            );
        }
    }
    #[test]
    fn one_fe_borrow_reuse_preserves_complete_fits_and_error_order() {
        for n in [16_383, 16_384, 32_771] {
            let columns: Vec<Vec<f64>> = (0..9)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect()
                })
                .collect();
            let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
            let response: Vec<_> = (0..n)
                .map(|r| {
                    groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1
                })
                .collect();
            let mut data = prepared(
                &response,
                &columns,
                &[false; 9],
                Some(&groups),
                true,
                TestWeights::Unit,
            );
            for threads in [1, 4, 16] {
                let mut config = PreparedOlsFitConfig::default();
                config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
                let run = |data: &PreparedOlsData, _disabled| {
                    ThreadTeam::new(config.execution.threads)
                        .install(|| {
                            fit_with_controls_internal(
                                data,
                                config,
                                &mut SystemFitReservation,
                                FitPerturbations {
                                    ..FitPerturbations::default()
                                },
                            )
                        })
                        .unwrap()
                        .map(without_solve_timings)
                };
                let expected = run(&data, true);
                let actual = run(&data, false);
                assert!(expected == actual, "N={n}, threads={threads}");
            }
            let source = data.categorical_sources()[0].prepared_source_index();
            data.sample.regressors[source][0] += 100.0;
            data.sample.outcome[0] = f64::NAN;
            let run = |_disabled| {
                fit_with_controls_internal(
                    &data,
                    PreparedOlsFitConfig::default(),
                    &mut SystemFitReservation,
                    FitPerturbations {
                        ..FitPerturbations::default()
                    },
                )
            };
            assert_eq!(run(true).unwrap_err(), run(false).unwrap_err());
        }
    }

    #[test]
    fn one_fe_final_target_recovery_preserves_saved_results() {
        for n in [360, 16_385] {
            let mut columns: Vec<Vec<f64>> = (0..9)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect()
                })
                .collect();
            if n == 360 {
                columns.push(columns[0].clone());
            }
            let groups: Vec<_> = (0..n).map(|r| (r / 5) as f64).collect();
            let response: Vec<_> = (0..n)
                .map(|r| {
                    (groups[r] / 19.0).sin()
                        + columns[0][r] * 1.3
                        + columns[1][r] * 0.7
                        + (r as f64).cos() * 0.1
                })
                .collect();
            let data = prepared_multiway_categorical(&response, &columns, &[groups]);
            for threads in [1, 16] {
                let mut config = PreparedOlsFitConfig::default();
                config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
                config.absorb.retain_coefficients = true;
                config.allow_final_fe_recovery = true;
                config.final_residual_tolerance = Some(1e-14);
                let run = |_disabled| {
                    ThreadTeam::new(config.execution.threads)
                        .install(|| {
                            fit_with_controls_internal(
                                &data,
                                config,
                                &mut SystemFitReservation,
                                FitPerturbations {
                                    one_fe_final_recovery_disabled: _disabled,
                                    ..FitPerturbations::default()
                                },
                            )
                        })
                        .unwrap()
                        .unwrap()
                };
                let expected = run(true);
                let actual = run(false);
                assert!(expected.recovered_fe_coefficients().is_none());
                assert!(actual.recovered_fe_coefficients().is_none());
                assert_eq!(actual.finite_rank(), expected.finite_rank());
                for (a, b) in actual.coefficients().iter().zip(expected.coefficients()) {
                    assert_eq!(a.status, b.status);
                    match (a.value, b.value) {
                        (Some(a), Some(b)) => assert_close(a, b, 1e-10),
                        (None, None) => {}
                        _ => panic!("coefficient presence changed"),
                    }
                }
                for (&a, &b) in actual.residuals().iter().zip(expected.residuals()) {
                    assert_close(a, b, 1e-10);
                }
                let recover = |fit: &CertifiedPreparedOlsFit| {
                    let target: Vec<_> = (0..n)
                        .map(|r| {
                            let mut xb = 0.0_f64;
                            for (column, coefficient) in columns.iter().zip(fit.coefficients()) {
                                if let Some(value) = coefficient.value {
                                    xb = value.mul_add(column[r], xb);
                                }
                            }
                            xb += fit.coefficients().last().unwrap().value.unwrap();
                            response[r] - xb - fit.residuals()[r]
                        })
                        .collect();
                    ThreadTeam::new(config.execution.threads)
                        .install(|| {
                            data.recover_absorbed_effects_fast(
                                &target,
                                &[true],
                                fit.recovered_fe_coefficients(),
                                false,
                                false,
                                config.absorb.lsmr,
                                crate::fe::MapConfig {
                                    tolerance: 1e-12,
                                    ..config.absorb.map
                                },
                                None,
                            )
                        })
                        .unwrap()
                        .unwrap()
                        .into_optional_parts()
                        .1
                        .remove(0)
                        .unwrap()
                };
                for (a, b) in recover(&actual).into_iter().zip(recover(&expected)) {
                    assert_close(a, b, 1e-9);
                }
                config.allow_final_fe_recovery = false;
                let required = fit_prepared_ols_coefficients_view_map(&data, config).unwrap();
                assert_eq!(
                    required.recovered_fe_coefficients(),
                    expected.recovered_fe_coefficients()
                );
            }
        }
    }

    #[test]
    fn residual_sums_stages_repeat_complete_fits_and_error_order() {
        for n in [16_383, 16_384, 32_771] {
            let columns: Vec<Vec<f64>> = (0..9)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect()
                })
                .collect();
            let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
            let response: Vec<_> = (0..n)
                .map(|r| {
                    groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1
                })
                .collect();
            let mut data = prepared(
                &response,
                &columns,
                &[false; 9],
                Some(&groups),
                true,
                TestWeights::Unit,
            );
            for threads in [1, 4, 16] {
                let mut config = PreparedOlsFitConfig::default();
                config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
                let run = |data: &PreparedOlsData, _repeat: bool| {
                    ThreadTeam::new(config.execution.threads)
                        .install(|| {
                            fit_with_controls_internal(
                                data,
                                config,
                                &mut SystemFitReservation,
                                FitPerturbations::default(),
                            )
                        })
                        .unwrap()
                        .map(without_solve_timings)
                };
                let expected = run(&data, true);
                let actual = run(&data, false);
                assert!(expected == actual, "N={n}, threads={threads}");
            }
            let source = data.categorical_sources()[0].prepared_source_index();
            data.sample.regressors[source][0] += 100.0;
            data.sample.outcome[0] = f64::NAN;
            let run = |_repeat: bool| {
                fit_with_controls_internal(
                    &data,
                    PreparedOlsFitConfig::default(),
                    &mut SystemFitReservation,
                    FitPerturbations::default(),
                )
            };
            assert_eq!(run(true).unwrap_err(), run(false).unwrap_err());
        }
    }

    #[test]
    fn score_stages_repeat_complete_fits_and_error_order() {
        for n in [16_383, 16_384, 32_771] {
            let columns: Vec<Vec<f64>> = (0..9)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect()
                })
                .collect();
            let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
            let response: Vec<_> = (0..n)
                .map(|r| {
                    groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1
                })
                .collect();
            let mut data = prepared(
                &response,
                &columns,
                &[false; 9],
                Some(&groups),
                true,
                TestWeights::Unit,
            );
            for threads in [1, 4, 16] {
                let mut config = PreparedOlsFitConfig::default();
                config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
                let run = |data: &PreparedOlsData, _repeat: bool, _raw_repeat: bool| {
                    ThreadTeam::new(config.execution.threads)
                        .install(|| {
                            fit_with_controls_internal(
                                data,
                                config,
                                &mut SystemFitReservation,
                                FitPerturbations::default(),
                            )
                        })
                        .unwrap()
                        .map(without_solve_timings)
                };
                let expected = run(&data, true, true);
                let actual = run(&data, false, false);
                assert_eq!(
                    run(&data, false, true),
                    actual,
                    "raw residual repeat N={n}, threads={threads}"
                );
                assert!(expected == actual, "N={n}, threads={threads}");
            }
            let source = data.categorical_sources()[0].prepared_source_index();
            data.sample.regressors[source][0] += 100.0;
            data.sample.outcome[0] = f64::NAN;
            let run = |_repeat: bool| {
                fit_with_controls_internal(
                    &data,
                    PreparedOlsFitConfig::default(),
                    &mut SystemFitReservation,
                    FitPerturbations::default(),
                )
            };
            assert_eq!(run(true).unwrap_err(), run(false).unwrap_err());
        }
    }

    #[test]
    fn unit_intercept_score_matches_generic_weights_and_errors() {
        for n in [1, 127, 16_384, 16_387] {
            let y: Vec<_> = (0..n).map(|r| (r as f64).sin()).collect();
            let groups = vec![0.0; n];
            let data = prepared(&y, &[], &[], Some(&groups), true, TestWeights::Unit);
            let ones = vec![1.0; n];
            let unit = NormalizedWeights {
                values: None,
                raw_scale: 1.0,
                kind: None,
            };
            let generic = NormalizedWeights {
                values: Some(&ones),
                raw_scale: 1.0,
                kind: Some(WeightKind::Frequency),
            };
            for scale in [0.0, 1e-200, 1.0, 1e150] {
                let mut residuals: Vec<_> = y.iter().map(|v| v * scale).collect();
                let call = |residuals: &[f64], weights| {
                    certify_scores(
                        &data,
                        residuals,
                        weights,
                        &[(n as f64).sqrt()],
                        &[0.0],
                        FiniteScoreDraft::default(),
                        &[PreparedOlsFitStatus::NormalizedAbsorbedIntercept],
                        scale,
                        1e-10,
                    )
                };
                assert_eq!(call(&residuals, unit), call(&residuals, generic));
                residuals[n / 2] = f64::NAN;
                assert_eq!(call(&residuals, unit), call(&residuals, generic));
            }
        }
    }

    #[test]
    fn two_fe_schur_final_stages_repeat_results_caches_and_error_order() {
        let n = 16_385;
        for width in [5, 6, 10, 17] {
            let columns = (0..width)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let effects = [
                (0..n).map(|r| (r % 41) as f64).collect::<Vec<_>>(),
                (0..n).map(|r| ((r / 41) % 37) as f64).collect::<Vec<_>>(),
            ];
            let response = (0..n)
                .map(|r| {
                    columns[0][r] * 1.3 + effects[0][r] + effects[1][r] + 0.1 * (r as f64).cos()
                })
                .collect::<Vec<_>>();
            let data = prepared_multiway_categorical(&response, &columns, &effects);
            let mut config = PreparedOlsFitConfig {
                allow_final_fe_recovery: true,
                ..PreparedOlsFitConfig::default()
            };
            config.absorb.retain_coefficients = true;
            config.execution.threads = ThreadConfig::from_requested(8, true).unwrap();
            for (cache, limit) in [(false, 0), (true, 0), (true, MAX_FAST_SCORE_CACHE_BYTES)] {
                for perturbation in [
                    FitPerturbations::default(),
                    FitPerturbations {
                        reported_bread_extension: true,
                        reconstructed_within_residual_constant: true,
                        ..FitPerturbations::default()
                    },
                    FitPerturbations {
                        final_residual_constant: true,
                        ..FitPerturbations::default()
                    },
                ] {
                    let run = |_repeat: bool| {
                        ThreadTeam::new(config.execution.threads)
                            .install(|| {
                                fit_with_controls_internal_mode(
                                    &data,
                                    config,
                                    &mut SystemFitReservation,
                                    perturbation,
                                    cache,
                                    limit,
                                )
                            })
                            .unwrap()
                            .map(without_solve_timings)
                    };
                    assert_eq!(
                        run(false),
                        run(true),
                        "width={width} cache={cache} limit={limit} perturbation={perturbation:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn narrow_weighted_final_stages_repeat_fits_and_error_order() {
        let n = 16_385;
        let weights = (0..n).map(|r| (1 + r % 7) as f64).collect::<Vec<_>>();
        for width in [5, 6] {
            let columns = (0..width)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let effects = [
                (0..n).map(|r| (r % 41) as f64).collect::<Vec<_>>(),
                (0..n).map(|r| ((r / 41) % 37) as f64).collect::<Vec<_>>(),
            ];
            let response = (0..n)
                .map(|r| {
                    columns[0][r] * 1.3 + effects[0][r] + effects[1][r] + 0.1 * (r as f64).cos()
                })
                .collect::<Vec<_>>();
            for effects in [&effects[..1], &effects[..]] {
                for weights in [
                    TestWeights::Analytic(&weights),
                    TestWeights::Probability(&weights),
                    TestWeights::Frequency(&weights),
                ] {
                    let data = prepared_multiway_categorical_weights(
                        &response, &columns, effects, weights,
                    );
                    let mut config = PreparedOlsFitConfig {
                        allow_final_fe_recovery: true,
                        ..PreparedOlsFitConfig::default()
                    };
                    config.absorb.retain_coefficients = true;
                    config.execution.threads = ThreadConfig::from_requested(8, true).unwrap();
                    for (cache, limit) in
                        [(false, 0), (true, 0), (true, MAX_FAST_SCORE_CACHE_BYTES)]
                    {
                        for perturbation in [
                            FitPerturbations::default(),
                            FitPerturbations {
                                reported_bread_extension: true,
                                reconstructed_within_residual_constant: true,
                                ..FitPerturbations::default()
                            },
                            FitPerturbations {
                                final_residual_constant: true,
                                ..FitPerturbations::default()
                            },
                        ] {
                            let run = |_repeat: bool| {
                                ThreadTeam::new(config.execution.threads)
                                    .install(|| {
                                        fit_with_controls_internal_mode(
                                            &data,
                                            config,
                                            &mut SystemFitReservation,
                                            perturbation,
                                            cache,
                                            limit,
                                        )
                                    })
                                    .unwrap()
                                    .map(without_solve_timings)
                            };
                            assert_eq!(run(false), run(true), "width={width} effects={} cache={cache} limit={limit} perturbation={perturbation:?}", effects.len());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn final_residual_stages_repeat_bits_and_rejection_order() {
        for n in [16_383, 16_384, 32_771] {
            let columns: Vec<Vec<f64>> = (0..9)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect()
                })
                .collect();
            let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
            let response: Vec<_> = (0..n)
                .map(|r| {
                    groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1
                })
                .collect();
            let data = prepared(
                &response,
                &columns,
                &[false; 9],
                Some(&groups),
                true,
                TestWeights::Unit,
            );
            for threads in [4, 8, 16] {
                let mut config = PreparedOlsFitConfig::default();
                config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
                for perturbation in [
                    FitPerturbations::default(),
                    FitPerturbations {
                        reported_intercept: true,
                        ..FitPerturbations::default()
                    },
                    FitPerturbations {
                        reported_bread_extension: true,
                        ..FitPerturbations::default()
                    },
                    FitPerturbations {
                        reconstructed_within_residual_constant: true,
                        ..FitPerturbations::default()
                    },
                    FitPerturbations {
                        final_residual_constant: true,
                        ..FitPerturbations::default()
                    },
                ] {
                    let run = |_repeat: bool| {
                        ThreadTeam::new(config.execution.threads)
                            .install(|| {
                                fit_with_controls_internal(
                                    &data,
                                    config,
                                    &mut SystemFitReservation,
                                    perturbation,
                                )
                            })
                            .unwrap()
                            .map(without_solve_timings)
                    };
                    let expected = run(true);
                    let actual = run(false);
                    assert!(
                        expected == actual,
                        "N={n}, threads={threads}, perturbation={perturbation:?}, errors={:?}/{:?}",
                        expected.as_ref().err(),
                        actual.as_ref().err()
                    );
                }
            }
        }
    }

    #[test]
    fn wide_residual_stages_repeat_fits_caches_and_error_order() {
        let n = 16_385;
        for width in [17, 50] {
            let mut columns: Vec<Vec<f64>> = (0..width)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect()
                })
                .collect();
            columns[width - 1] = columns[0].clone();
            let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
            let response: Vec<_> = (0..n)
                .map(|r| {
                    groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1
                })
                .collect();
            let data = prepared(
                &response,
                &columns,
                &vec![false; width],
                Some(&groups),
                true,
                TestWeights::Unit,
            );
            let mut config = PreparedOlsFitConfig::default();
            config.execution.threads = ThreadConfig::from_requested(8, true).unwrap();
            for (cache, limit) in [(false, 0), (true, 0), (true, MAX_FAST_SCORE_CACHE_BYTES)] {
                for perturbation in [
                    FitPerturbations::default(),
                    FitPerturbations {
                        reported_bread_extension: true,
                        reconstructed_within_residual_constant: true,
                        ..FitPerturbations::default()
                    },
                    FitPerturbations {
                        final_residual_constant: true,
                        ..FitPerturbations::default()
                    },
                ] {
                    let run = |_repeat: bool| {
                        ThreadTeam::new(config.execution.threads)
                            .install(|| {
                                fit_with_controls_internal_mode(
                                    &data,
                                    config,
                                    &mut SystemFitReservation,
                                    perturbation,
                                    cache,
                                    limit,
                                )
                            })
                            .unwrap()
                            .map(without_solve_timings)
                    };
                    assert_eq!(
                        run(false),
                        run(true),
                        "width={width} cache={cache} limit={limit} perturbation={perturbation:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn retained_score_cache_residual_stages_repeat_fits_and_errors() {
        for n in [16_383, 16_384, 32_771] {
            let columns: Vec<Vec<f64>> = (0..9)
                .map(|c| {
                    (0..n)
                        .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                        .collect()
                })
                .collect();
            let groups: Vec<_> = (0..n).map(|r| (r % 79) as f64).collect();
            let response: Vec<_> = (0..n)
                .map(|r| {
                    groups[r] + columns[0][r] * 1.3 + columns[1][r] * 0.7 + (r as f64).cos() * 0.1
                })
                .collect();
            let data = prepared(
                &response,
                &columns,
                &[false; 9],
                Some(&groups),
                true,
                TestWeights::Unit,
            );
            for threads in [1, 8, 16] {
                let mut config = PreparedOlsFitConfig::default();
                config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
                for cache_limit in [0, MAX_FAST_SCORE_CACHE_BYTES] {
                    for perturbation in [
                        FitPerturbations::default(),
                        FitPerturbations {
                            reported_intercept: true,
                            ..FitPerturbations::default()
                        },
                        FitPerturbations {
                            reported_bread_extension: true,
                            ..FitPerturbations::default()
                        },
                        FitPerturbations {
                            reconstructed_within_residual_constant: true,
                            ..FitPerturbations::default()
                        },
                        FitPerturbations {
                            final_residual_constant: true,
                            ..FitPerturbations::default()
                        },
                    ] {
                        let run = |_repeat: bool| {
                            ThreadTeam::new(config.execution.threads)
                                .install(|| {
                                    fit_with_controls_internal_mode(
                                        &data,
                                        config,
                                        &mut SystemFitReservation,
                                        perturbation,
                                        true,
                                        cache_limit,
                                    )
                                })
                                .unwrap()
                                .map(|fit| {
                                    assert!(fit.fast_score_cache.is_some());
                                    without_solve_timings(fit)
                                })
                        };
                        assert_eq!(run(false), run(true),
                            "n={n} threads={threads} cache_limit={cache_limit} perturbation={perturbation:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn numeric_tiles_preserve_rhs_and_raw_residual_bits_across_threads() {
        let rows = 4_103;
        for width in [1, 5, 6, 7, 15, 50, 63, 64] {
            let columns = (0..width)
                .map(|c| {
                    (0..rows)
                        .map(|r| {
                            if r % 17 == 0 {
                                -0.0
                            } else {
                                ((r * (c + 3) % 997) as f64).sin()
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let response = (0..rows)
                .map(|r| ((r * 37 % 991) as f64).cos())
                .collect::<Vec<_>>();
            let prepared = prepared(
                &response,
                &columns,
                &vec![false; width],
                None,
                true,
                TestWeights::Unit,
            );
            let make_expected = || {
                let mut values = Vec::new();
                let mut residuals = response.clone();
                let coefficients = (0..prepared.model.columns.len())
                    .map(|c| {
                        if c % 5 == 0 {
                            None
                        } else {
                            Some((c as f64 - 13.0) / 31.0)
                        }
                    })
                    .collect::<Vec<_>>();
                for r in 0..rows {
                    values.push(response[r]);
                    // Keep the legacy matrix indices and numerical traversal order explicit.
                    #[allow(clippy::needless_range_loop)]
                    for c in 0..prepared.model.columns.len() {
                        let value = prepared.column_value(c, r).unwrap();
                        values.push(value);
                        if let Some(coefficient) = coefficients[c] {
                            residuals[r] = (-coefficient).mul_add(value, residuals[r]);
                        }
                    }
                }
                (values, residuals, coefficients)
            };
            let (expected_values, expected_residuals, coefficients) = make_expected();
            for threads in [1, 4, 16] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let actual = prepared
                            .fit_build_rhs_with_response(&prepared.fit_sample().outcome)
                            .unwrap();
                        assert_eq!(
                            actual
                                .as_slice()
                                .iter()
                                .map(|v| v.to_bits())
                                .collect::<Vec<_>>(),
                            expected_values
                                .iter()
                                .map(|v| v.to_bits())
                                .collect::<Vec<_>>()
                        );
                        if let Some(bound) = crate::ols::NumericOlsColumns::bind_residual_mean(
                            &prepared.model,
                            &prepared.sample,
                        )
                        .unwrap()
                        {
                            for row in 0..rows {
                                assert_eq!(
                                    bound
                                        .residual_at(&coefficients, row, response[row])
                                        .unwrap()
                                        .to_bits(),
                                    expected_residuals[row].to_bits()
                                );
                            }
                            assert!(bound
                                .residual_at(&coefficients[..1], 0, response[0])
                                .is_err());
                            assert!(bound.residual_at(&coefficients, rows, response[0]).is_err());
                            assert!(bound.residual_at(&coefficients, 0, f64::NAN).is_err());
                        } else {
                            assert!(width >= 64);
                        }
                        let bound =
                            crate::ols::NumericOlsColumns::bind(&prepared.model, &prepared.sample)
                                .unwrap();
                        if let Some(bound) = bound {
                            let mut residuals = response.clone();
                            bound
                                .subtract_fitted(&coefficients, &mut residuals)
                                .unwrap();
                            assert_eq!(
                                residuals.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                                expected_residuals
                                    .iter()
                                    .map(|v| v.to_bits())
                                    .collect::<Vec<_>>()
                            );
                            assert!(bound
                                .subtract_fitted(&coefficients[..1], &mut residuals)
                                .is_err());
                            assert!(bound
                                .fill_reserved_rhs(
                                    &response[..1],
                                    &mut vec![0.0; expected_values.len()]
                                )
                                .is_err());
                            let mut invalid = coefficients.clone();
                            invalid[0] = Some(f64::INFINITY);
                            assert!(bound.subtract_fitted(&invalid, &mut residuals).is_err());
                        } else {
                            assert!(!(7..64).contains(&width));
                        }
                    });
            }
        }
    }

    #[test]
    fn numeric_tiles_support_omissions_working_responses_and_categorical_topology() {
        let rows = 4_103;
        let x = (0..12)
            .map(|c| {
                (0..rows)
                    .map(|r| ((r + c * 3) % 11) as f64)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let outcome = vec![1.0; rows];
        let omitted = (0..12).map(|c| c % 4 == 0).collect::<Vec<_>>();
        let mut numeric = prepared(&outcome, &x, &omitted, None, false, TestWeights::Unit);
        let effects = vec![
            (0..rows).map(|r| (r % 17) as f64).collect(),
            (0..rows).map(|r| (r % 23) as f64).collect(),
        ];
        let categorical = prepared_multiway_categorical(&outcome, &x, &effects);
        let working = (0..rows).map(|r| (r % 7) as f64 - 2.5).collect::<Vec<_>>();
        for view in [&numeric as &dyn PreparedOlsFitView, &categorical] {
            let actual = view.fit_build_rhs_with_response(&working).unwrap();
            let generated = view.fit_model().columns.len();
            let coefficients = (0..generated)
                .map(|c| {
                    if c % 3 == 0 {
                        None
                    } else {
                        Some(c as f64 / 7.0)
                    }
                })
                .collect::<Vec<_>>();
            let mut expected_residual = working.clone();
            for (r, &response) in working.iter().enumerate() {
                assert_eq!(actual[(r, 0)].to_bits(), response.to_bits());
                for c in 0..generated {
                    assert_eq!(
                        actual[(r, c + 1)].to_bits(),
                        view.fit_column_value(c, r).unwrap().to_bits()
                    );
                }
            }
            for (c, coefficient) in coefficients.iter().enumerate() {
                if let Some(coefficient) = coefficient {
                    view.fit_subtract_raw_column(c, *coefficient, &mut expected_residual)
                        .unwrap();
                }
            }
            let bound = crate::ols::NumericOlsColumns::bind(view.fit_model(), view.fit_sample())
                .unwrap()
                .unwrap();
            let mut residual = working.clone();
            bound.subtract_fitted(&coefficients, &mut residual).unwrap();
            assert_eq!(residual, expected_residual);
            let mut matrix = Vec::new();
            matrix.try_reserve_exact(actual.as_slice().len()).unwrap();
            let address = matrix.as_ptr();
            let capacity = matrix.capacity();
            bound.fill_reserved_rhs(&working, &mut matrix).unwrap();
            assert_eq!(address, matrix.as_ptr());
            assert_eq!(capacity, matrix.capacity());
            assert_eq!(matrix, actual.as_slice());
            assert_eq!(
                bound
                    .fill_reserved_rhs(&working, &mut Vec::new())
                    .unwrap_err()
                    .code,
                ErrorCode::Allocation
            );
            // Finite operands can overflow on an intermediate FMA. Later
            // cancellation must not turn this into a successful residual.
            let coefficients = vec![Some(f64::MAX); generated];
            assert_eq!(
                bound
                    .subtract_fitted(&coefficients, &mut residual)
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidData
            );
        }
        numeric.sample.regressors[1][200] = f64::NAN;
        assert_eq!(
            numeric
                .fit_build_rhs_with_response(&numeric.fit_sample().outcome)
                .unwrap_err()
                .code,
            ErrorCode::InvalidData
        );
        numeric.sample.regressors[1][200] = 0.0;
        numeric.sample.regressors[1].pop();
        assert!(crate::ols::NumericOlsColumns::bind(&numeric.model, &numeric.sample).is_err());
    }

    #[test]
    fn two_effect_graph_recovery_admits_exact_finishing_budget() {
        let n = 16_512;
        let effects = [
            (0..n).map(|r| (r % 17) as f64).collect::<Vec<_>>(),
            (0..n).map(|r| ((r / 17) % 13) as f64).collect::<Vec<_>>(),
        ];
        let target = (0..n)
            .map(|r| effects[0][r] * 0.1 - effects[1][r] * 0.07)
            .collect::<Vec<_>>();
        let x = vec![(0..n).map(|r| (r as f64).sin()).collect::<Vec<_>>()];
        let prepared = prepared_multiway_categorical(&target, &x, &effects);
        let planned =
            crate::memory::estimate_intercept_recovery_memory(n, &[17, 13], 2, 8, Some(4))
                .unwrap()
                .retained_peak_bytes;
        let threads = ThreadConfig::from_requested(8, false).unwrap();
        let supplied = (0..n).map(|r| (1 + r % 7) as f64).collect::<Vec<_>>();
        crate::ThreadTeam::new(threads)
            .install(|| {
                for weights in [
                    BorrowedMapWeights::Unit,
                    BorrowedMapWeights::Provided(&supplied),
                ] {
                    for usable in [planned, planned - 1] {
                        let recovered = prepared.recover_absorbed_effects_fast_with_weights(
                            weights,
                            &target,
                            &[true, true],
                            None,
                            true,
                            false,
                            crate::fe::LsmrConfig::default(),
                            crate::fe::MapConfig {
                                tolerance: 1e-12,
                                ..crate::fe::MapConfig::default()
                            },
                            Some(crate::memory::FeSolverMemoryBudget {
                                execution_bytes: 1234,
                                usable_bytes: 1234 + usable,
                            }),
                        );
                        if usable == planned {
                            let recovered = recovered.unwrap();
                            assert_eq!(
                                recovered.recovery_method(),
                                crate::fe::CategoricalRecoveryMethod::Direct
                            );
                            assert!(recovered.reconstruction_relative_infinity() < 1e-12);
                        } else {
                            assert_eq!(recovered.unwrap_err().code, ErrorCode::Allocation);
                        }
                    }
                }
            })
            .unwrap();
    }

    #[test]
    fn fused_standardized_moments_match_compensated_scalar_reference() {
        let rows = 40_003_usize;
        let columns = 5_usize;
        let mut values = Vec::with_capacity(rows * columns);
        for row in 0..rows {
            let axis = row as f64;
            values.extend_from_slice(&[
                (axis * 0.013).sin() + 0.4 * (axis * 0.007).cos(),
                (axis * 0.011).sin(),
                (axis * 0.017).cos(),
                ((row % 29) as f64 - 14.0) / 17.0,
                ((row * 37 % 101) as f64 - 50.0) / 31.0,
            ]);
        }
        let matrix = DenseMatrix::from_row_major(rows, columns, values).expect("moment matrix");
        let candidates = [0_usize, 1, 2, 3];
        let mut reservation = SystemFitReservation;
        let (rhs, gram) = test_projected_moments(&matrix, &candidates, &mut reservation)
            .expect("fused standardized moments");
        for (left, generated) in candidates.iter().copied().enumerate() {
            let mut expected_rhs = 0.0_f64;
            let mut rhs_correction = 0.0_f64;
            for row in 0..rows {
                compensated_add(
                    matrix[(row, generated + 1)] * matrix[(row, 0)],
                    &mut expected_rhs,
                    &mut rhs_correction,
                )
                .expect("reference RHS");
            }
            assert_close(rhs[left], expected_rhs + rhs_correction, 2e-12);
            for (right, other_generated) in candidates.iter().copied().enumerate() {
                let mut expected_gram = 0.0_f64;
                let mut gram_correction = 0.0_f64;
                for row in 0..rows {
                    compensated_add(
                        matrix[(row, generated + 1)] * matrix[(row, other_generated + 1)],
                        &mut expected_gram,
                        &mut gram_correction,
                    )
                    .expect("reference Gram");
                }
                assert_close(
                    gram[left * candidates.len() + right],
                    expected_gram + gram_correction,
                    2e-12,
                );
            }
        }
    }

    #[test]
    fn fused_weighted_norms_match_columnwise_reference_exactly() {
        let rows = 40_007_usize;
        let columns = 7_usize;
        let mut values = Vec::with_capacity(rows * columns);
        let mut weights = Vec::with_capacity(rows);
        for row in 0..rows {
            let axis = row as f64;
            for column in 0..columns {
                let frequency = (column + 3) as f64 * 0.000_017;
                values.push((axis * frequency).sin() + (column as f64 - 2.0) * 0.125);
            }
            weights.push(0.25 + ((row * 37 % 97) as f64 + 1.0) / 101.0);
        }
        let matrix = DenseMatrix::from_row_major(rows, columns, values).expect("norm matrix");
        let raw_scale = weights.iter().copied().fold(0.0_f64, f64::max);
        let normalized = NormalizedWeights {
            values: Some(&weights),
            raw_scale,
            kind: Some(WeightKind::Analytic),
        };
        let expected = (0..columns)
            .map(|column| {
                weighted_matrix_column_norm(&matrix, column, normalized)
                    .expect("columnwise weighted norm")
            })
            .collect::<Vec<_>>();
        let mut actual = vec![0.0_f64; columns];
        let mut reservation = SystemFitReservation;
        fill_weighted_matrix_norms_fused(&matrix, normalized, &mut actual, &mut reservation)
            .expect("fused weighted norms");
        assert_eq!(actual, expected);
    }

    #[test]
    fn blocked_weighted_norms_preserve_scalar_bits_at_all_widths() {
        for threads in [1, 7] {
            let team = ThreadTeam::new(ThreadConfig::from_requested(threads, true).unwrap());
            team.install(|| {
                for columns in [1, 2, 3, 4, 5, 7, 8, 9, 13, 51, 65] {
                    let rows = 1031;
                    let mut matrix = DenseMatrix::zeros(rows, columns).unwrap();
                    let weights = (0..rows)
                        .map(|row| 0.25 + (row % 17) as f64 / 19.0)
                        .collect::<Vec<_>>();
                    for row in 0..rows {
                        for col in 0..columns {
                            matrix[(row, col)] = match col % 5 {
                                0 => 0.0,
                                1 => -0.0,
                                2 => ((row * 37 % 101) as f64 - 50.0) * 1e150,
                                3 => ((row * 19 % 97) as f64 - 48.0) * 1e-150,
                                _ => (row as f64 * 0.173).sin(),
                            };
                        }
                    }
                    for values in [None, Some(weights.as_slice())] {
                        let normalized = NormalizedWeights {
                            values,
                            raw_scale: 2.0,
                            kind: values.map(|_| WeightKind::Analytic),
                        };
                        let expected = (0..columns)
                            .map(|col| {
                                weighted_matrix_column_norm(&matrix, col, normalized)
                                    .unwrap()
                                    .to_bits()
                            })
                            .collect::<Vec<_>>();
                        let mut actual = vec![0.0; columns];
                        for block in [2, 4, 8] {
                            match block {
                                2 => fill_weighted_matrix_norms_blocked::<2>(
                                    &matrix,
                                    normalized,
                                    &mut actual,
                                ),
                                4 => fill_weighted_matrix_norms_blocked::<4>(
                                    &matrix,
                                    normalized,
                                    &mut actual,
                                ),
                                _ => fill_weighted_matrix_norms_blocked::<8>(
                                    &matrix,
                                    normalized,
                                    &mut actual,
                                ),
                            }
                            .unwrap();
                            assert_eq!(
                                actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                                expected
                            );
                        }
                    }
                }
            })
            .unwrap();
        }
    }

    #[test]
    fn low_norm_preserves_frozen_large_row_reduction_bits() {
        // Independent former Low recurrence and ordered 16,384-row merge.
        fn accumulate(values: &[f64], raw: Option<&[f64]>, divisor: f64) -> (f64, f64) {
            let (mut scale, mut sum) = (0.0_f64, 1.0_f64);
            for (row, &value) in values.iter().enumerate() {
                let root = raw.map_or(1.0, |w| (w[row] / divisor).sqrt());
                let absolute = (root * value).abs();
                if absolute == 0.0 {
                    continue;
                }
                if scale < absolute {
                    let ratio = scale / absolute;
                    sum = 1.0 + sum * ratio * ratio;
                    scale = absolute;
                } else {
                    let ratio = absolute / scale;
                    sum += ratio * ratio;
                }
            }
            (scale, sum)
        }
        for rows in [16_383, 16_384, 16_385, 32_771] {
            let values: Vec<_> = (0..rows)
                .map(|r| match r % 5 {
                    0 => -0.0,
                    1 => 1e-150,
                    2 => -1e120,
                    _ => ((r * 37 % 173) as f64 - 86.0) / 91.0,
                })
                .collect();
            let raw: Vec<_> = (0..rows).map(|r| [8.0, 0.25, 1e-120][r % 3]).collect();
            for weighted in [false, true] {
                let weights = NormalizedWeights {
                    values: weighted.then_some(raw.as_slice()),
                    raw_scale: if weighted { 8.0 } else { 1.0 },
                    kind: weighted.then_some(WeightKind::Analytic),
                };
                for threads in [1, 4, 16] {
                    let (scale, sum) = if threads == 1 || rows <= 16_384 {
                        accumulate(&values, weights.values, weights.raw_scale)
                    } else {
                        let (mut scale, mut sum) = (0.0_f64, 1.0_f64);
                        for start in (0..rows).step_by(16_384) {
                            let end = (start + 16_384).min(rows);
                            let (other_scale, other_sum) = accumulate(
                                &values[start..end],
                                weights.values.map(|w| &w[start..end]),
                                weights.raw_scale,
                            );
                            if other_scale == 0.0 {
                                continue;
                            }
                            if scale < other_scale {
                                let ratio = scale / other_scale;
                                sum = other_sum + sum * ratio * ratio;
                                scale = other_scale;
                            } else {
                                let ratio = other_scale / scale;
                                sum += other_sum * ratio * ratio;
                            }
                        }
                        (scale, sum)
                    };
                    let expected = scale * sum.sqrt();
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap();
                    let actual = pool
                        .install(|| weighted_slice_norm(&values, weights))
                        .unwrap();
                    assert_eq!(
                        actual.to_bits(),
                        expected.to_bits(),
                        "rows={rows}, weighted={weighted}, threads={threads}"
                    );
                }
            }
        }
    }

    #[test]
    fn blocked_weighted_norms_fail_closed() {
        let mut matrix = DenseMatrix::zeros(3, 2).unwrap();
        let weights = NormalizedWeights {
            values: None,
            raw_scale: 1.0,
            kind: None,
        };
        assert!(fill_weighted_matrix_norms_blocked::<4>(&matrix, weights, &mut [0.0]).is_err());
        assert!(fill_weighted_matrix_norms_blocked::<0>(&matrix, weights, &mut [0.0; 2]).is_err());
        matrix[(1, 1)] = f64::INFINITY;
        assert!(fill_weighted_matrix_norms_blocked::<4>(&matrix, weights, &mut [0.0; 2]).is_err());
        matrix[(1, 1)] = 1.0;
        let bad_weights = [1.0, 0.0, 1.0];
        let weights = NormalizedWeights {
            values: Some(&bad_weights),
            raw_scale: 1.0,
            kind: Some(WeightKind::Analytic),
        };
        assert!(fill_weighted_matrix_norms_blocked::<4>(&matrix, weights, &mut [0.0; 2]).is_err());
    }

    #[test]
    fn fused_standardized_moments_preserve_noncontiguous_candidate_fallback() {
        let rows = 40_019_usize;
        let columns = 6_usize;
        let mut values = Vec::with_capacity(rows * columns);
        for row in 0..rows {
            let axis = row as f64;
            values.extend_from_slice(&[
                (axis * 0.019).sin(),
                (axis * 0.023).cos(),
                ((row % 31) as f64 - 15.0) / 19.0,
                (axis * 0.029).sin(),
                ((row * 41 % 109) as f64 - 54.0) / 37.0,
                (axis * 0.031).cos(),
            ]);
        }
        let matrix = DenseMatrix::from_row_major(rows, columns, values).expect("moment matrix");
        let candidates = [0_usize, 2, 4];
        let mut fused_reservation = SystemFitReservation;
        let (rhs, gram) = test_projected_moments(&matrix, &candidates, &mut fused_reservation)
            .expect("noncontiguous fused standardized moments");
        let mut reference_reservation = SystemFitReservation;
        let (reference_rhs, reference_gram) =
            build_standardized_moments_columnwise(&matrix, &candidates, &mut reference_reservation)
                .expect("noncontiguous scalar standardized moments");
        for (actual, expected) in rhs.iter().zip(reference_rhs) {
            assert_close(*actual, expected, 2e-12);
        }
        for (actual, expected) in gram.iter().zip(reference_gram) {
            assert_close(*actual, expected, 2e-12);
        }
    }

    #[test]
    fn bound_weighted_column_means_preserve_current_weight_bits() {
        for (rows, width) in [
            (1, 1),
            (4096, 9),
            (16383, 33),
            (16384, 32),
            (32771, 17),
            (65537, 8),
        ] {
            let columns: Vec<Vec<f64>> = (0..width)
                .map(|column| {
                    (0..rows)
                        .map(|row| match column % 6 {
                            0 => [1e300, 1e-300, -1e300][row % 3],
                            1 => ((row % 17) as f64 - 8.0) * 1e-300,
                            2 => -0.0,
                            3 => [1e16, 1.0, -1e16][row % 3],
                            _ => (row as f64 / 17.0).sin(),
                        })
                        .collect()
                })
                .collect();
            let raw: Vec<_> = columns
                .iter()
                .map(|v| Some(v.as_slice()))
                .chain([None])
                .collect();
            let generated: Vec<_> = (0..=width).rev().filter(|c| *c != 2).collect();
            let first: Vec<_> = (0..rows).map(|row| (row % 5 + 1) as f64).collect();
            let second: Vec<_> = (0..rows)
                .map(|row| {
                    if row % 7 == 0 {
                        1e-250
                    } else {
                        (row % 13 + 1) as f64
                    }
                })
                .collect();
            for weights in [
                NormalizedWeights {
                    values: None,
                    kind: None,
                    raw_scale: 1.0,
                },
                NormalizedWeights {
                    values: Some(&first),
                    kind: Some(WeightKind::Frequency),
                    raw_scale: 1.0,
                },
                NormalizedWeights {
                    values: Some(&second),
                    kind: Some(WeightKind::Probability),
                    raw_scale: 32.0,
                },
            ] {
                let expected: Vec<_> = generated
                    .iter()
                    .map(|&column| {
                        weighted_mean_by(rows, weights, |row| {
                            Ok(raw[column].map_or(1.0, |v| v[row]))
                        })
                        .unwrap()
                        .to_bits()
                    })
                    .collect();
                for threads in [1, 4] {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap()
                        .install(|| {
                            let mut means = vec![0.0; generated.len()];
                            super::fill_bound_weighted_column_means(
                                rows, &raw, &generated, weights, &mut means,
                            )
                            .unwrap();
                            assert_eq!(
                                means.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                                expected,
                                "rows={rows}, width={width}, threads={threads}"
                            );
                        });
                }
            }
        }
    }

    #[test]
    fn bound_weighted_column_means_reject_bad_axes_and_nonfinite_inputs() {
        let values = [1.0, 2.0];
        let unit = NormalizedWeights {
            values: None,
            kind: None,
            raw_scale: 1.0,
        };
        let mut means = [19.0];
        for (rows, generated) in [(1, 0), (2, 1)] {
            assert!(super::fill_bound_weighted_column_means(
                rows,
                &[Some(&values)],
                &[generated],
                unit,
                &mut means
            )
            .is_err());
            assert_eq!(means, [19.0]);
        }
        assert!(super::fill_bound_weighted_column_means(
            2,
            &[Some(&values)],
            &[],
            unit,
            &mut means
        )
        .is_err());
        assert!(
            super::fill_bound_weighted_column_means(0, &[None], &[0], unit, &mut means).is_err()
        );
        super::fill_bound_weighted_column_means(0, &[], &[], unit, &mut []).unwrap();
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let bad = [1.0, invalid];
            assert!(super::fill_bound_weighted_column_means(
                2,
                &[Some(&bad)],
                &[0],
                unit,
                &mut means
            )
            .is_err());
        }
        let invalid_weights = [1.0, 0.0];
        let weights = NormalizedWeights {
            values: Some(&invalid_weights),
            kind: Some(WeightKind::Probability),
            raw_scale: 1.0,
        };
        assert!(super::fill_bound_weighted_column_means(
            2,
            &[Some(&values)],
            &[0],
            weights,
            &mut means
        )
        .is_err());
    }

    #[test]
    fn unit_mean_by_shortcut_matches_explicit_unit_weights_bits() {
        for n in [0, 1, 127, 16384, 16387] {
            let ones = vec![1.0; n];
            let unit = NormalizedWeights {
                values: None,
                kind: None,
                raw_scale: 1.0,
            };
            let supplied = NormalizedWeights {
                values: Some(&ones),
                kind: Some(WeightKind::Frequency),
                raw_scale: 1.0,
            };
            for scale in [1e-200, 1.0, 1e200] {
                let values: Vec<_> = (0..n).map(|r| ((r % 107) as f64 - 51.0) * scale).collect();
                assert_eq!(
                    weighted_mean_by(n, unit, |r| Ok(values[r])).map(f64::to_bits),
                    weighted_mean_by(n, supplied, |r| Ok(values[r])).map(f64::to_bits)
                );
                for fail in [0, n / 2, n] {
                    let get = |r| {
                        if r == fail {
                            Err(invalid_data("injected mean input error"))
                        } else {
                            Ok(values[r])
                        }
                    };
                    assert_eq!(
                        weighted_mean_by(n, unit, get),
                        weighted_mean_by(n, supplied, get)
                    );
                }
            }
        }
    }

    #[test]
    fn residual_mean_bound_retains_cancellation_and_extreme_scale_tails() {
        let supplied = [1.0, 0.25, 0.5, 0.125];
        let tiny = [1e-300, 1.0, 1.0, 1.0];
        for values in [
            [1.0, 0.0, 0.0, 0.0],
            [1.0, -1.0, 2.0, -2.0],
            [1e-300, -1e-300, 2e-300, 4e-300],
            [1e300, -1e300, 2e299, 4e299],
            [
                f64::MIN_POSITIVE,
                -f64::MIN_POSITIVE,
                2.0 * f64::MIN_POSITIVE,
                0.0,
            ],
        ] {
            for weights in [
                NormalizedWeights {
                    values: None,
                    raw_scale: 1.0,
                    kind: None,
                },
                NormalizedWeights {
                    values: Some(&supplied),
                    raw_scale: 1.0,
                    kind: Some(WeightKind::Analytic),
                },
                NormalizedWeights {
                    values: Some(&tiny),
                    raw_scale: 1.0,
                    kind: Some(WeightKind::Analytic),
                },
            ] {
                let expected = weighted_mean_by(4, weights, |r| Ok(values[r])).unwrap();
                let maximum = values.iter().copied().map(f64::abs).fold(0.0, f64::max);
                for bound in [maximum, maximum * 16.0, 1e308] {
                    let actual =
                        weighted_mean_with_bound(4, weights, |r| Ok(values[r]), bound).unwrap();
                    assert!(
                        (actual - expected).abs() <= 1e-14 * expected.abs(),
                        "values={values:?}, bound={bound}, actual={actual}, expected={expected}"
                    );
                    if maximum < bound * f64::MIN_POSITIVE {
                        assert_eq!(actual.to_bits(), expected.to_bits());
                    }
                }
            }
        }
    }

    #[test]
    fn residual_bound_requires_weighted_underflow_fallback() {
        let raw_weights = [1e-300, 1.0, 1.0, 1.0];
        let values = [1.0, 0.0, 0.0, 0.0];
        let weights = NormalizedWeights {
            values: Some(&raw_weights),
            raw_scale: 1.0,
            kind: Some(WeightKind::Analytic),
        };
        let expected = weighted_mean_by(4, weights, |r| Ok(values[r])).unwrap();
        assert_eq!(expected, 1e-300 / 3.0);
        assert_eq!(
            weighted_mean_with_scale(4, weights, |r| Ok(values[r]), 1e300, None).unwrap(),
            0.0
        );
        assert_eq!(
            weighted_mean_with_bound(4, weights, |r| Ok(values[r]), 1e300)
                .unwrap()
                .to_bits(),
            expected.to_bits()
        );
        let values = [1e-299, 0.0, 0.0, 0.0];
        let weights = NormalizedWeights {
            values: None,
            raw_scale: 1.0,
            kind: None,
        };
        let mut underflow = false;
        assert!(values[0] / 2e8 >= f64::MIN_POSITIVE);
        weighted_mean_with_scale(4, weights, |r| Ok(values[r]), 2e8, Some(&mut underflow)).unwrap();
        assert!(
            underflow,
            "division by total mass can also underflow the bounded mean"
        );
        let expected = weighted_mean_by(4, weights, |r| Ok(values[r])).unwrap();
        assert_eq!(
            weighted_mean_with_bound(4, weights, |r| Ok(values[r]), 2e8)
                .unwrap()
                .to_bits(),
            expected.to_bits()
        );
    }

    #[test]
    fn blocked_raw_moments_preserve_columnwise_bits_and_dispatch() {
        for threads in [1, 7, 16] {
            let team = ThreadTeam::new(ThreadConfig::from_requested(threads, true).unwrap());
            team.install(|| {
                for columns in [1, 7, 8, 9, 63, 64, 65, 66, 129] {
                    let rows = 1031;
                    let mut matrix = DenseMatrix::zeros(rows, columns).unwrap();
                    let weights = (0..rows)
                        .map(|row| 1.0 + (row % 17) as f64)
                        .collect::<Vec<_>>();
                    for row in 0..rows {
                        for column in 0..columns {
                            matrix[(row, column)] = match column % 7 {
                                0 => 0.0,
                                1 => -0.0,
                                2 => ((row * 37 % 101) as f64 - 50.0) * 1e150,
                                3 => ((row * 19 % 97) as f64 - 48.0) * 1e-150,
                                4 => 1.0,
                                5 => [1e16, 1.0, -1e16][row % 3],
                                _ => (row as f64 * 0.173).sin(),
                            };
                        }
                    }
                    for kind in [
                        None,
                        Some(WeightKind::Analytic),
                        Some(WeightKind::Frequency),
                        Some(WeightKind::Probability),
                    ] {
                        let normalized = NormalizedWeights {
                            values: kind.map(|_| weights.as_slice()),
                            raw_scale: 17.0,
                            kind,
                        };
                        let mut reservation = SystemFitReservation;
                        let cache = build_raw_moment_cache(&matrix, normalized, &mut reservation)
                            .expect("dispatched raw moments");
                        let mut expected_scales = vec![0.0; columns];
                        let mut expected_means = vec![0.0; columns];
                        let mut expected_norms = vec![0.0; columns];
                        fill_raw_moment_cache_columnwise(
                            &matrix,
                            normalized,
                            cache.weight_sum,
                            &mut expected_scales,
                            &mut expected_means,
                            &mut expected_norms,
                        )
                        .unwrap();
                        let mut scales = vec![0.0; columns];
                        let mut means = vec![0.0; columns];
                        let mut norms = vec![0.0; columns];
                        fill_raw_moment_cache_blocked::<8>(
                            &matrix,
                            normalized,
                            cache.weight_sum,
                            &mut scales,
                            &mut means,
                            &mut norms,
                        )
                        .unwrap();
                        for (actual, expected) in [
                            (&scales, &expected_scales),
                            (&means, &expected_means),
                            (&norms, &expected_norms),
                        ] {
                            assert_eq!(
                                actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                                expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
                            );
                        }
                        if columns > FUSED_MOMENT_MAX_COLUMNS {
                            for (actual, expected) in [
                                (&cache.scales, &expected_scales),
                                (&cache.means, &expected_means),
                                (&cache.norms, &expected_norms),
                            ] {
                                assert_eq!(
                                    actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                                    expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
                                );
                            }
                        }
                    }
                }
            })
            .unwrap();
        }
    }

    #[test]
    fn blocked_raw_moments_fail_closed() {
        let mut matrix = DenseMatrix::zeros(3, 2).unwrap();
        let weights = NormalizedWeights {
            values: None,
            raw_scale: 1.0,
            kind: None,
        };
        assert!(fill_raw_moment_cache_blocked::<8>(
            &matrix,
            weights,
            3.0,
            &mut [0.0],
            &mut [0.0; 2],
            &mut [0.0; 2],
        )
        .is_err());
        assert!(fill_raw_moment_cache_blocked::<0>(
            &matrix,
            weights,
            3.0,
            &mut [0.0; 2],
            &mut [0.0; 2],
            &mut [0.0; 2],
        )
        .is_err());
        for weight_sum in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            assert!(fill_raw_moment_cache_blocked::<8>(
                &matrix,
                weights,
                weight_sum,
                &mut [0.0; 2],
                &mut [0.0; 2],
                &mut [0.0; 2],
            )
            .is_err());
        }
        for value in [f64::NAN, f64::INFINITY, f64::MAX] {
            matrix[(0, 1)] = value;
            matrix[(1, 1)] = value;
            let blocked = fill_raw_moment_cache_blocked::<8>(
                &matrix,
                weights,
                3.0,
                &mut [0.0; 2],
                &mut [0.0; 2],
                &mut [0.0; 2],
            )
            .unwrap_err();
            let reference = fill_raw_moment_cache_columnwise(
                &matrix,
                weights,
                3.0,
                &mut [0.0; 2],
                &mut [0.0; 2],
                &mut [0.0; 2],
            )
            .unwrap_err();
            assert_eq!(blocked.to_string(), reference.to_string());
        }
        matrix.as_mut_slice().fill(1.0);
        for invalid in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            let values = [1.0, invalid, 1.0];
            let weights = NormalizedWeights {
                values: Some(&values),
                raw_scale: 1.0,
                kind: Some(WeightKind::Analytic),
            };
            assert!(fill_raw_moment_cache_blocked::<8>(
                &matrix,
                weights,
                3.0,
                &mut [0.0; 2],
                &mut [0.0; 2],
                &mut [0.0; 2],
            )
            .is_err());
        }
    }

    #[test]
    fn fused_raw_moment_cache_matches_columnwise_reference() {
        let rows = 40_021_usize;
        let columns = 7_usize;
        let mut values = Vec::with_capacity(rows * columns);
        let mut raw_weights = Vec::with_capacity(rows);
        for row in 0..rows {
            let axis = row as f64;
            raw_weights.push(0.25 + ((row * 17 % 43) + 1) as f64 / 11.0);
            values.extend_from_slice(&[
                (axis * 0.013).sin(),
                (axis * 0.017).cos(),
                ((row % 29) as f64 - 14.0) / 17.0,
                ((row * 37 % 101) as f64 - 50.0) / 31.0,
                0.0,
                1.0e120 * (axis * 0.007).sin(),
                1.0e-120 * (axis * 0.011).cos(),
            ]);
        }
        let matrix = DenseMatrix::from_row_major(rows, columns, values).expect("raw matrix");
        let raw_scale = raw_weights.iter().copied().fold(0.0_f64, f64::max);
        for weights in [
            NormalizedWeights {
                values: None,
                raw_scale: 1.0,
                kind: None,
            },
            NormalizedWeights {
                values: Some(&raw_weights),
                raw_scale,
                kind: Some(WeightKind::Analytic),
            },
        ] {
            let mut reservation = SystemFitReservation;
            let cache = build_raw_moment_cache(&matrix, weights, &mut reservation)
                .expect("fused raw moments");
            let mut scales = vec![0.0_f64; columns];
            let mut means = vec![0.0_f64; columns];
            let mut norms = vec![0.0_f64; columns];
            fill_raw_moment_cache_columnwise(
                &matrix,
                weights,
                cache.weight_sum,
                &mut scales,
                &mut means,
                &mut norms,
            )
            .expect("columnwise raw moments");
            assert_eq!(cache.scales, scales);
            for (actual, expected) in cache.means.iter().zip(means) {
                assert_close(*actual, expected, 3e-12);
            }
            for (actual, expected) in cache.norms.iter().zip(norms) {
                assert_close(*actual, expected, 3e-12);
            }
        }
    }

    #[test]
    fn fused_raw_moment_block_cap_covers_tail_rows() {
        let rows = FUSED_MOMENT_MAX_BLOCKS * PARALLEL_REDUCTION_CHUNK_ROWS + 17;
        let mut values = vec![0.0_f64; rows * 2];
        for row in rows - 17..rows {
            values[row * 2] = 2.0;
            values[row * 2 + 1] = 3.0;
        }
        let matrix = DenseMatrix::from_row_major(rows, 2, values).expect("raw matrix");
        let mut reservation = SystemFitReservation;
        let cache = build_raw_moment_cache(
            &matrix,
            NormalizedWeights {
                values: None,
                raw_scale: 1.0,
                kind: None,
            },
            &mut reservation,
        )
        .expect("capped fused raw moments");
        assert_eq!(cache.scales, vec![2.0, 3.0]);
        assert_close(cache.means[0], 34.0 / rows as f64, 1e-15);
        assert_close(cache.means[1], 51.0 / rows as f64, 1e-15);
        assert_close(cache.norms[0], 2.0 * 17.0_f64.sqrt(), 1e-15);
        assert_close(cache.norms[1], 3.0 * 17.0_f64.sqrt(), 1e-15);
    }

    #[test]
    fn fused_standardized_moment_block_cap_covers_tail_rows() {
        let rows = FUSED_MOMENT_MAX_BLOCKS * PARALLEL_REDUCTION_CHUNK_ROWS + 17;
        let mut values = vec![0.0_f64; rows * 2];
        for row in rows - 17..rows {
            values[row * 2] = 2.0;
            values[row * 2 + 1] = 3.0;
        }
        let matrix = DenseMatrix::from_row_major(rows, 2, values).expect("moment matrix");
        let mut reservation = SystemFitReservation;
        let (rhs, gram) = test_projected_moments(&matrix, &[0], &mut reservation)
            .expect("capped fused standardized moments");
        assert_eq!(rhs, vec![102.0]);
        assert_eq!(gram, vec![153.0]);
    }

    #[test]
    fn shared_finite_accepts_known_wide_design() {
        for (rows, columns) in [(32_768_usize, 12_usize), (1_024, 65), (1_024, 128)] {
            let mut regressors = vec![vec![0.0_f64; rows]; columns];
            let mut state = 0x9e37_79b9_7f4a_7c15_u64;
            for row in 0..rows {
                for (column, values) in regressors.iter_mut().enumerate() {
                    state = state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    let uniform = ((state >> 11) as f64) / ((1_u64 << 53) as f64) - 0.5;
                    values[row] = uniform + 0.03 * ((row + 3 * column) as f64 * 0.17).sin();
                }
            }
            let outcome = (0..rows)
                .map(|row| {
                    regressors
                        .iter()
                        .enumerate()
                        .fold(0.0_f64, |value, (column, regressor)| {
                            ((column + 1) as f64 / columns as f64).mul_add(regressor[row], value)
                        })
                })
                .collect::<Vec<_>>();
            let prepared = prepared(
                &outcome,
                &regressors,
                &vec![false; columns],
                None,
                false,
                TestWeights::Unit,
            );
            for mode in [
                PreparedOlsExecutionMode::Fast,
                PreparedOlsExecutionMode::Low,
            ] {
                let fit = fit_prepared_ols_coefficients_map(
                    &prepared,
                    PreparedOlsFitConfig {
                        execution: PreparedOlsExecutionConfig {
                            mode,
                            poolsize: 8,
                            ..PreparedOlsExecutionConfig::default()
                        },
                        ..PreparedOlsFitConfig::default()
                    },
                )
                .expect("shared finite known wide fit");
                assert_eq!(fit.finite_rank(), columns);
                assert!(fit.rank_certificate().passed);
                assert!(fit.score_certificate().passed);
                for (column, coefficient) in fit.coefficients().iter().enumerate() {
                    assert_close(
                        coefficient.value.expect("known wide coefficient"),
                        (column + 1) as f64 / columns as f64,
                        1e-10,
                    );
                }
                assert!(fit.residuals().iter().all(|value| value.abs() < 1e-10));
            }
        }
    }

    #[test]
    fn no_fe_intercept_and_syntax_omission_preserve_canonical_slots() {
        let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 4.0];
        let omitted = vec![9.0, 8.0, 7.0, 6.0, 5.0, 4.0];
        let y = x.iter().map(|value| 3.0 + 2.5 * value).collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            &[x, omitted],
            &[false, true],
            None,
            true,
            TestWeights::Unit,
        );
        let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("certified fit");
        assert_eq!(fit.finite_rank(), 1);
        assert_eq!(fit.coefficients().len(), 3);
        assert_eq!(fit.coefficients()[0].status, PreparedOlsFitStatus::Kept);
        assert_close(fit.coefficients()[0].value.expect("slope"), 2.5, 1e-11);
        assert_eq!(
            fit.coefficients()[1].status,
            PreparedOlsFitStatus::SyntaxOmitted
        );
        assert_eq!(fit.coefficients()[1].generated_column_index, None);
        assert_eq!(
            fit.coefficients()[2].status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert_close(fit.coefficients()[2].value.expect("intercept"), 3.0, 1e-11);
        assert!(fit.residuals().iter().all(|value| value.abs() < 1e-11));
        assert!(fit.absorption_report().certified());
        assert!(fit.residual_absorption_report().certified());
        assert!(fit.rank_certificate().passed);
        assert!(fit.score_certificate().passed);
    }

    #[test]
    fn prepared_lsmr_path_matches_certified_map_fit() {
        let group = vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0];
        let x = vec![-2.0, 1.0, -1.0, 2.0, 0.5, 3.0, -0.5, 4.0];
        let y = x
            .iter()
            .zip(&group)
            .map(|(x, group)| 1.75 * x + 2.0 * group - 0.5)
            .collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&group),
            true,
            TestWeights::Unit,
        );
        let map = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("certified MAP fit");
        let lsmr = fit_prepared_ols_coefficients_map(
            &prepared,
            PreparedOlsFitConfig {
                absorb: AbsorbConfig {
                    solver: crate::fe::FeSolverKind::Lsmr,
                    lsmr: crate::fe::LsmrConfig {
                        tolerance: 1e-10,
                        preconditioner: crate::fe::LsmrPreconditioner::Diagonal,
                        ..crate::fe::LsmrConfig::default()
                    },
                    ..PreparedOlsFitConfig::default().absorb
                },
                ..PreparedOlsFitConfig::default()
            },
        )
        .expect("certified LSMR fit");
        assert_eq!(
            lsmr.absorption_report().used_solver,
            crate::fe::FeSolverKind::Lsmr
        );
        assert_eq!(
            lsmr.residual_absorption_report().used_solver,
            crate::fe::FeSolverKind::Lsmr
        );
        for (map, lsmr) in map.coefficients().iter().zip(lsmr.coefficients()) {
            assert_eq!(map.status, lsmr.status);
            match (map.value, lsmr.value) {
                (Some(map), Some(lsmr)) => assert_close(lsmr, map, 1e-8),
                (None, None) => {}
                _ => panic!("MAP and LSMR coefficient availability differs"),
            }
        }
        for (map, lsmr) in map.residuals().iter().zip(lsmr.residuals()) {
            assert_close(*lsmr, *map, 1e-8);
        }
    }

    #[test]
    fn reconstructed_within_residual_is_reused_and_refined_only_when_needed() {
        let group = (0..48).map(|row| (row / 6) as f64).collect::<Vec<_>>();
        let x = (0..48)
            .map(|row| (row % 11) as f64 - 5.0)
            .collect::<Vec<_>>();
        let y = x
            .iter()
            .zip(&group)
            .enumerate()
            .map(|(row, (x, group))| 1.75 * x + 0.6 * group + 0.03 * (row as f64).sin())
            .collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&group),
            true,
            TestWeights::Unit,
        );

        let ordinary =
            fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
                .expect("ordinary reconstructed-residual fit");
        assert!(ordinary.residual_absorption_report().certified());
        assert_eq!(ordinary.residual_absorption_report().iterations, 0);

        let mut reservation = InjectedReservation::default();
        let refined = fit_with_controls_internal(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut reservation,
            FitPerturbations {
                reconstructed_within_residual_constant: true,
                ..FitPerturbations::default()
            },
        )
        .expect("perturbed reconstructed residual is refined");
        assert!(refined.residual_absorption_report().certified());
        assert!(refined.residual_absorption_report().iterations > 0);
        for (ordinary, refined) in ordinary.residuals().iter().zip(refined.residuals()) {
            assert_close(*refined, *ordinary, 1e-10);
        }
    }

    fn automatic_lsmr_report_for_merge() -> SolveReport {
        SolveReport {
            requested_solver: FeSolverKind::Auto,
            used_solver: FeSolverKind::Lsmr,
            iterations: 12,
            checkpoint_retries: 0,
            solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
            stop_reason: SolveStopReason::Certified,
            certificate: FeCertificate {
                absolute_infinity_norm: 2e-8,
                relative_infinity_norm: 3e-11,
                relative_two_norm: 4e-12,
                passed: true,
            },
            normal_equation_residual: 5e-11,
            setup_seconds: 1.0,
            solve_seconds: 2.0,
            certification_seconds: 0.25,
            fallback_used: false,
            requested_preconditioner: Some(crate::fe::LsmrPreconditioner::Auto),
            used_preconditioner: Some(crate::fe::LsmrPreconditioner::Schwarz),
            pilot_iterations: 0,
            pilot_contraction: 0.0,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn merged_solver_memory_preserves_largest_attempt_and_unmodeled_scope() {
        use crate::memory::SolverWorkspacePeak;
        let mut first = automatic_lsmr_report_for_merge();
        first.solver_workspace_peak = SolverWorkspacePeak::from_bytes(400).unwrap();
        let mut second = first.clone();
        second.solver_workspace_peak = SolverWorkspacePeak::from_bytes(200).unwrap();
        for (mut target, source) in [(first.clone(), second.clone()), (second, first)] {
            merge_solve_report(&mut target, source).unwrap();
            assert_eq!(target.solver_workspace_peak.bytes(), Some(400));
            merge_solve_report(&mut target, automatic_lsmr_report_for_merge()).unwrap();
            assert_eq!(target.solver_workspace_peak.bytes(), None);
        }
    }

    #[test]
    fn low_report_merge_preserves_certified_lsmr_cg_continuation_in_any_order() {
        let lsmr = automatic_lsmr_report_for_merge();
        let mut cg = lsmr.clone();
        cg.used_solver = FeSolverKind::Cg;
        cg.fallback_used = true;
        cg.iterations = 17;
        cg.checkpoint_retries = 1;
        cg.certificate.absolute_infinity_norm = 9e-8;
        cg.certificate.relative_infinity_norm = 2e-10;
        cg.normal_equation_residual = 0.0;
        cg.setup_seconds = 3.0;
        cg.solve_seconds = 4.0;
        cg.certification_seconds = 0.5;
        cg.warnings
            .push("continued CG from the LSMR candidate".to_owned());
        let mut last = lsmr.clone();
        last.pilot_iterations = 2;
        last.pilot_contraction = 0.1;
        last.warnings.push("later LSMR batch".to_owned());
        let reports = [lsmr.clone(), cg.clone(), last];
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let mut merged = reports[order[0]].clone();
            let expected_warnings = order
                .iter()
                .flat_map(|i| reports[*i].warnings.clone())
                .collect::<Vec<_>>();
            for i in &order[1..] {
                merge_solve_report(&mut merged, reports[*i].clone()).unwrap();
            }
            assert!(merged.certified());
            assert_eq!(merged.requested_solver, FeSolverKind::Auto);
            assert_eq!(merged.used_solver, FeSolverKind::Cg);
            assert!(merged.fallback_used);
            assert_eq!(
                merged.requested_preconditioner,
                lsmr.requested_preconditioner
            );
            assert_eq!(merged.used_preconditioner, lsmr.used_preconditioner);
            assert_eq!(merged.iterations, 17);
            assert_eq!(merged.checkpoint_retries, 1);
            assert_eq!(
                merged.certificate.absolute_infinity_norm,
                cg.certificate.absolute_infinity_norm
            );
            assert_eq!(
                merged.certificate.relative_infinity_norm,
                cg.certificate.relative_infinity_norm
            );
            assert_eq!(
                merged.certificate.relative_two_norm,
                lsmr.certificate.relative_two_norm
            );
            assert_eq!(
                merged.normal_equation_residual,
                lsmr.normal_equation_residual
            );
            assert_eq!(merged.setup_seconds, 5.0);
            assert_eq!(merged.solve_seconds, 8.0);
            assert_eq!(merged.certification_seconds, 1.0);
            assert_eq!(merged.pilot_iterations, 2);
            assert_eq!(merged.pilot_contraction, 0.1);
            assert_eq!(merged.warnings, expected_warnings);
        }
    }

    #[test]
    fn low_report_merge_accepts_independent_auto_pilots_in_any_order() {
        let lsmr = automatic_lsmr_report_for_merge();
        let mut map = lsmr.clone();
        map.used_solver = FeSolverKind::Map;
        map.requested_preconditioner = None;
        map.used_preconditioner = None;
        map.iterations = 4;
        let mut direct_cg = map.clone();
        direct_cg.used_solver = FeSolverKind::Cg;
        direct_cg.iterations = 9;
        for fallback in [false, true] {
            let mut last = lsmr.clone();
            if fallback {
                last.used_solver = FeSolverKind::Cg;
                last.fallback_used = true;
            }
            let reports = [map.clone(), direct_cg.clone(), lsmr.clone(), last];
            for a in 0..4 {
                for b in 0..4 {
                    for c in 0..4 {
                        for d in 0..4 {
                            let order = [a, b, c, d];
                            if (0..4).any(|i| (i + 1..4).any(|j| order[i] == order[j])) {
                                continue;
                            }
                            let mut merged = reports[a].clone();
                            for index in [b, c, d] {
                                merge_solve_report(&mut merged, reports[index].clone()).unwrap();
                            }
                            assert!(merged.certified());
                            assert_eq!(
                                merged.used_solver,
                                if fallback {
                                    FeSolverKind::Cg
                                } else {
                                    FeSolverKind::Lsmr
                                }
                            );
                            assert_eq!(merged.fallback_used, fallback);
                            assert_eq!(
                                merged.requested_preconditioner,
                                lsmr.requested_preconditioner
                            );
                            assert_eq!(merged.used_preconditioner, lsmr.used_preconditioner);
                            assert_eq!(merged.iterations, 12);
                            assert_eq!(merged.setup_seconds, 4.0);
                            assert_eq!(merged.solve_seconds, 8.0);
                        }
                    }
                }
            }
        }
        // A direct-CG/early-MAP mixture did not attempt or fall back from LSMR.
        for (mut merged, source) in [(map.clone(), direct_cg.clone()), (direct_cg, map)] {
            merge_solve_report(&mut merged, source).unwrap();
            assert_eq!(merged.used_solver, FeSolverKind::Cg);
            assert!(!merged.fallback_used);
            assert_eq!(merged.used_preconditioner, None);
        }
    }

    #[test]
    fn low_report_merge_preserves_general_map_fallback_in_any_order() {
        let lsmr = automatic_lsmr_report_for_merge();
        let mut fallback = lsmr.clone();
        fallback.used_solver = FeSolverKind::Map;
        fallback.fallback_used = true;
        fallback.iterations = 40;
        let mut direct = lsmr.clone();
        direct.used_solver = FeSolverKind::Map;
        direct.requested_preconditioner = None;
        direct.used_preconditioner = None;
        let reports = [lsmr, fallback, direct];
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let mut merged = reports[order[0]].clone();
            for i in &order[1..] {
                merge_solve_report(&mut merged, reports[*i].clone()).unwrap();
            }
            assert!(merged.certified());
            assert_eq!(merged.used_solver, FeSolverKind::Map);
            assert!(merged.fallback_used);
            assert_eq!(merged.iterations, 40);
            assert_eq!(
                merged.requested_preconditioner,
                reports[0].requested_preconditioner
            );
            assert_eq!(merged.used_preconditioner, reports[0].used_preconditioner);
        }
    }

    #[test]
    fn low_report_merge_rejects_uncertified_or_incompatible_continuations() {
        let lsmr = automatic_lsmr_report_for_merge();
        let mut cg = lsmr.clone();
        cg.used_solver = FeSolverKind::Cg;
        cg.fallback_used = true;
        for variant in 0..8 {
            let mut bad = cg.clone();
            match variant {
                0 => bad.certificate.passed = false,
                1 => bad.stop_reason = SolveStopReason::Uncertified,
                2 => bad.requested_solver = FeSolverKind::Lsmr,
                3 => bad.fallback_used = false,
                4 => bad.used_solver = FeSolverKind::Squarem,
                5 => bad.requested_preconditioner = Some(crate::fe::LsmrPreconditioner::Schwarz),
                6 => bad.used_preconditioner = Some(crate::fe::LsmrPreconditioner::Off),
                7 => bad.requested_preconditioner = None,
                _ => unreachable!(),
            }
            for (target, source) in [(lsmr.clone(), bad.clone()), (bad, lsmr.clone())] {
                let mut merged = target.clone();
                let error = merge_solve_report(&mut merged, source).unwrap_err();
                assert_eq!(error.code, ErrorCode::InvalidData);
                assert_eq!(merged, target, "invalid input must not partially merge");
            }
        }
        let mut missing = lsmr;
        missing.requested_preconditioner = None;
        missing.used_preconditioner = None;
        cg.requested_preconditioner = None;
        cg.used_preconditioner = None;
        assert!(merge_solve_report(&mut missing, cg).is_err());
    }

    #[test]
    fn low_lsmr_batches_match_fast_across_pool_sizes() {
        let rows = 2048;
        let effects = [97, 89, 9].map(|levels| {
            (0..rows)
                .map(|r| ((r * 37 + r / 11) % levels) as f64)
                .collect::<Vec<_>>()
        });
        let x = (0..4)
            .map(|j| {
                (0..rows)
                    .map(|r| (r as f64 * (j + 3) as f64 * 0.137).sin())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let y = (0..rows)
            .map(|r| {
                0.7 * x[0][r] - 0.2 * x[1][r] + effects[0][r] * 0.01 + (r as f64 * 0.131).cos()
            })
            .collect::<Vec<_>>();
        let prepared = prepared_multiway_categorical(&y, &x, &effects);
        let mut config = PreparedOlsFitConfig::default();
        config.execution.threads = ThreadConfig::from_requested(1, true).unwrap();
        let fast = fit_prepared_categorical_ols_coefficients_map(&prepared, config).unwrap();
        config.execution.mode = PreparedOlsExecutionMode::Low;
        for poolsize in [1, 2, 4, 5] {
            config.execution.poolsize = poolsize;
            let low = fit_prepared_categorical_ols_coefficients_map(&prepared, config).unwrap();
            assert_eq!(low.finite_rank(), fast.finite_rank());
            assert_eq!(low.absorption_report().used_solver, FeSolverKind::Lsmr);
            for (a, b) in low.residuals().iter().zip(fast.residuals()) {
                assert_close(*a, *b, 1e-10);
            }
            assert!(low.rank_certificate().passed);
        }
    }

    #[test]
    fn low_fresh_blocks_preserve_explicit_weight_divisors_and_rank() {
        let rows = 257;
        let groups = (0..rows).map(|r| (r % 7) as f64).collect::<Vec<_>>();
        let weights = (0..rows).map(|r| (1 + r % 5) as f64).collect::<Vec<_>>();
        let x1 = (0..rows)
            .map(|r| 1e3 * groups[r] + (r as f64 * 0.37).sin())
            .collect::<Vec<_>>();
        let x2 = (0..rows)
            .map(|r| (r as f64 * 0.17).cos())
            .collect::<Vec<_>>();
        let x3 = x1.iter().map(|value| 2.0 * value).collect::<Vec<_>>();
        let y = (0..rows)
            .map(|r| 0.7 * x1[r] - 0.3 * x2[r] + groups[r] + (r as f64 * 0.11).sin())
            .collect::<Vec<_>>();
        for kind in [
            TestWeights::Unit,
            TestWeights::Analytic(&weights),
            TestWeights::Frequency(&weights),
            TestWeights::Probability(&weights),
        ] {
            let data = prepared(
                &y,
                &[x1.clone(), x2.clone(), x3.clone(), groups.clone()],
                &[false; 4],
                Some(&groups),
                true,
                kind,
            );
            let canonical = LinearSolveFrame::from_prepared_ols(&data).unwrap();
            let fast =
                fit_prepared_ols_coefficients_map(&data, PreparedOlsFitConfig::default()).unwrap();
            assert_eq!(fast.finite_rank(), 2);
            for divisor_factor in [1.0, 8.0] {
                if canonical.weights().values.is_none() && divisor_factor != 1.0 {
                    continue;
                }
                // Rescale the numerical solve without changing reference bread.
                let mut scale = canonical.bread_weight_scale();
                scale.solver_weight_divisor *= divisor_factor;
                scale.reference_weight_multiplier *= divisor_factor;
                let frame = LinearSolveFrame::from_parts(
                    canonical.response(),
                    canonical.raw_weights(),
                    scale,
                )
                .unwrap();
                for (poolsize, threads) in [(1, 1), (3, 7), (5, 1)] {
                    let config = PreparedOlsFitConfig {
                        execution: PreparedOlsExecutionConfig {
                            mode: PreparedOlsExecutionMode::Low,
                            poolsize,
                            threads: ThreadConfig::from_requested(threads, true).unwrap(),
                        },
                        ..PreparedOlsFitConfig::default()
                    };
                    let low = fit_prepared_weighted_linear_once(&data, frame, config).unwrap();
                    assert_eq!(low.kept_generated_columns(), fast.kept_generated_columns());
                    assert_eq!(
                        low.dropped_generated_columns(),
                        fast.dropped_generated_columns()
                    );
                    for (a, b) in low.coefficients().iter().zip(fast.coefficients()) {
                        assert_eq!(a.status, b.status);
                        match (a.value, b.value) {
                            (Some(a), Some(b)) => assert_close(a, b, 1e-9),
                            (a, b) => assert_eq!(a, b),
                        }
                    }
                    for (a, b) in low.bread().values().iter().zip(fast.bread().values()) {
                        assert_close(*a, *b, 1e-9);
                    }
                    for (a, b) in low.residuals().iter().zip(fast.residuals()) {
                        assert_close(*a, *b, 1e-9);
                    }
                    assert!(low.rank_certificate().passed);
                    assert!(low.score_certificate().passed);
                }
            }
        }
    }

    #[test]
    fn low_memory_scratch_fit_matches_fast_fit_across_multiple_blocks() {
        let rows = 40_usize;
        let g = (0..rows).map(|row| (row % 5) as f64).collect::<Vec<_>>();
        let x1 = (0..rows)
            .map(|row| row as f64 / 7.0 - 2.0)
            .collect::<Vec<_>>();
        let x2 = (0..rows)
            .map(|row| ((row * 3) % 17) as f64 / 5.0 - 1.0)
            .collect::<Vec<_>>();
        let x3 = (0..rows)
            .map(|row| ((row * row + 3) % 23) as f64 / 6.0)
            .collect::<Vec<_>>();
        let x4 = (0..rows)
            .map(|row| ((row * 11 + 5) % 29) as f64 / 8.0 - 1.5)
            .collect::<Vec<_>>();
        let y = (0..rows)
            .map(|row| {
                0.75 * x1[row] - 1.25 * x2[row]
                    + 0.4 * x3[row]
                    + 2.0 * x4[row]
                    + [0.2, -0.4, 0.7, 1.1, -0.3][row % 5]
                    + (row % 7) as f64 * 0.013
            })
            .collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            &[x1, x2, x3, x4],
            &[false; 4],
            Some(&g),
            true,
            TestWeights::Unit,
        );
        let fast = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("fast fit");
        let low = fit_prepared_ols_coefficients_map(
            &prepared,
            PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode: PreparedOlsExecutionMode::Low,
                    poolsize: 1,
                    ..PreparedOlsExecutionConfig::default()
                },
                ..PreparedOlsFitConfig::default()
            },
        )
        .expect("low fit");

        assert_eq!(fast.finite_rank(), low.finite_rank());
        assert_eq!(fast.coefficients().len(), low.coefficients().len());
        for (fast, low) in fast.coefficients().iter().zip(low.coefficients()) {
            assert_eq!(fast.status, low.status);
            assert_eq!(fast.generated_column_index, low.generated_column_index);
            match (fast.value, low.value) {
                (Some(fast), Some(low)) => assert_close(low, fast, 5e-11),
                (None, None) => {}
                pair => panic!("coefficient presence differs: {pair:?}"),
            }
        }
        for (fast, low) in fast.residuals().iter().zip(low.residuals()) {
            assert_close(*low, *fast, 5e-11);
        }
        for (fast, low) in fast.bread().values().iter().zip(low.bread().values()) {
            assert_close(*low, *fast, 5e-10);
        }
        assert_eq!(fast.execution().mode, PreparedOlsExecutionMode::Fast);
        assert_eq!(fast.execution().scratch_bytes, 0);
        assert_eq!(low.execution().mode, PreparedOlsExecutionMode::Low);
        assert_eq!(low.execution().poolsize, 1);
        assert_eq!(low.execution().scratch_bytes, (rows * 5 * 8) as u64);
    }

    #[test]
    fn absorbed_intercept_is_normalized_and_fe_collinear_column_is_separate() {
        let g = vec![1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0];
        let x = vec![-1.0, 0.5, 2.0, -2.0, 1.0, 3.0, -0.5, 2.5, 4.0];
        let y = x
            .iter()
            .zip(&g)
            .map(|(x, group)| 1.75 * x + [0.0, 3.0, -2.0, 4.5][*group as usize])
            .collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            &[x, g.clone()],
            &[false, false],
            Some(&g),
            true,
            TestWeights::Unit,
        );
        let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("FE fit");
        assert_eq!(fit.finite_rank(), 1);
        assert_close(
            fit.coefficients()[0].value.expect("identified slope"),
            1.75,
            1e-9,
        );
        assert_eq!(
            fit.coefficients()[1].status,
            PreparedOlsFitStatus::CollinearWithFixedEffects
        );
        assert_eq!(
            fit.coefficients()[2].status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert_close(
            fit.coefficients()[2].value.expect("normalized intercept"),
            11.0 / 6.0,
            1e-9,
        );
        assert_eq!(fit.kept_generated_columns(), &[0]);
        assert_eq!(fit.dropped_generated_columns(), &[1]);
        assert_eq!(fit.bread().dimension(), 2);
        assert_eq!(fit.bread().coordinates()[0].generated_column_index, 0);
        assert_eq!(fit.bread().coordinates()[1].generated_column_index, 2);
        assert_eq!(
            fit.bread().coordinates()[1].kind,
            PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept
        );
        assert!(fit.bread().certificate().passed);
        assert!(fit.residuals().iter().all(|value| value.abs() < 2e-9));
    }

    #[test]
    fn exact_scaled_duplicate_keeps_earlier_generated_column() {
        let g = vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
        let x = vec![100.0, 101.0, 99.0, 100.5, -100.0, -99.0, -101.0, -100.5];
        let duplicate = x.iter().map(|value| 7.0 * value).collect::<Vec<_>>();
        let distractor = vec![-2.0, 1.0, 2.0, -1.0, 1.0, -2.0, -1.0, 2.0];
        let y = x
            .iter()
            .zip(&distractor)
            .zip(&g)
            .map(|((x, z), group)| 4.0 * x + 2.0 * z + 3.0 * group)
            .collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            &[x, duplicate, distractor],
            &[false, false, false],
            Some(&g),
            false,
            TestWeights::Unit,
        );
        let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("rank-deficient fit");
        assert_eq!(fit.finite_rank(), 2);
        assert_eq!(fit.kept_generated_columns(), &[0, 2]);
        assert_eq!(fit.dropped_generated_columns(), &[1]);
        assert_eq!(
            fit.bread()
                .coordinates()
                .iter()
                .map(|coordinate| coordinate.generated_column_index)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert_close(
            fit.coefficients()[0].value.expect("first column"),
            4.0,
            1e-10,
        );
        assert_eq!(
            fit.coefficients()[1].status,
            PreparedOlsFitStatus::NumericallyRankDeficient
        );
        assert!(fit.rank_certificate().passed);
    }

    #[test]
    fn pinned_full_ibn_span_keeps_cons_and_earlier_indicator() {
        // Pinned Stata 18/reghdfe contract for `ibn.foreign` with the default
        // `_cons`: retain the structural intercept and one indicator, rather
        // than allowing the preceding full indicator span to evict `_cons`.
        let foreign = source(1, "foreign");
        let foreign_values = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let outcome = vec![10.0, 10.0, 10.0, 13.0, 13.0, 13.0];
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![foreign.clone()],
                terms: vec![
                    categorical_term(1, foreign.id, 0.0, false),
                    categorical_term(2, foreign.id, 1.0, false),
                ],
            },
            absorbed_terms: Vec::new(),
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
                    role: OlsInputRole::NumericSource(foreign.id),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        };
        let prepared = prepare_ols_data(
            &request,
            &LoadedOlsInput {
                candidate: vec![true; outcome.len()],
                outcome: loaded(&outcome),
                numeric_sources: vec![(foreign.id, loaded(&foreign_values))],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("full ibn preparation");
        let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("full ibn fit");
        assert_eq!(fit.finite_rank(), 1);
        assert_eq!(fit.kept_generated_columns(), &[0]);
        assert_eq!(fit.dropped_generated_columns(), &[1]);
        assert_eq!(fit.coefficients()[0].status, PreparedOlsFitStatus::Kept);
        assert_eq!(
            fit.coefficients()[1].status,
            PreparedOlsFitStatus::NumericallyRankDeficient
        );
        assert_eq!(
            fit.coefficients()[2].status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert_close(fit.coefficients()[0].value.expect("foreign=0"), -3.0, 1e-11);
        assert_close(fit.coefficients()[2].value.expect("intercept"), 13.0, 1e-11);
        assert!(fit.rank_certificate().passed);
        assert!(fit.score_certificate().passed);
    }

    #[test]
    fn factor_interaction_omission_and_vanished_level_keep_exact_slot_semantics() {
        let g = source(1, "g");
        let h = source(2, "h");
        let g_values = vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
        let h_values = vec![1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 2.0, 2.0];
        let outcome = (0..g_values.len())
            .map(|row| {
                1.0 + 2.0 * f64::from(g_values[row] == 2.0)
                    + 3.0 * f64::from(g_values[row] == 2.0 && h_values[row] == 2.0)
            })
            .collect::<Vec<_>>();
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![g.clone(), h.clone()],
                terms: vec![
                    categorical_term(1, g.id, 1.0, true),
                    categorical_term(2, g.id, 2.0, false),
                    categorical_interaction_term(3, &[(g.id, 2.0), (h.id, 2.0)]),
                    categorical_term(4, g.id, 3.0, false),
                ],
            },
            absorbed_terms: Vec::new(),
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
                    role: OlsInputRole::NumericSource(g.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(h.id),
                },
            ],
            include_intercept: true,
            keep_singletons: false,
        };
        let prepared = prepare_ols_data(
            &request,
            &LoadedOlsInput {
                candidate: vec![true; outcome.len()],
                outcome: loaded(&outcome),
                numeric_sources: vec![(g.id, loaded(&g_values)), (h.id, loaded(&h_values))],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("factor preparation");
        let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("factor fit");
        assert_eq!(fit.coefficients().len(), 5);
        assert_eq!(
            fit.coefficients()[0].status,
            PreparedOlsFitStatus::SyntaxOmitted
        );
        assert_close(fit.coefficients()[1].value.expect("g=2"), 2.0, 1e-10);
        assert_close(fit.coefficients()[2].value.expect("g=2,h=2"), 3.0, 1e-10);
        assert_eq!(
            fit.coefficients()[3].status,
            PreparedOlsFitStatus::NumericallyRankDeficient
        );
        assert_close(fit.coefficients()[4].value.expect("intercept"), 1.0, 1e-10);
        assert_eq!(fit.finite_rank(), 2);
    }

    #[test]
    fn rank_threshold_separates_clear_near_collinearity_cases() {
        let x = vec![-3.0, -2.0, -1.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let z = vec![1.0, -1.0, 0.5, -0.5, 1.5, -1.5, 2.0, -2.0];
        let y = x
            .iter()
            .zip(&z)
            .map(|(x, z)| 2.0 * x + 0.3 * z)
            .collect::<Vec<_>>();
        let config = PreparedOlsFitConfig {
            rank_tolerance_multiplier: 1.0e8,
            ..PreparedOlsFitConfig::default()
        };
        let dropped_column = x
            .iter()
            .zip(&z)
            .map(|(x, z)| x + 1e-10 * z)
            .collect::<Vec<_>>();
        let dropped = prepared(
            &y,
            &[x.clone(), dropped_column],
            &[false, false],
            None,
            false,
            TestWeights::Unit,
        );
        let dropped = fit_prepared_ols_coefficients_map(&dropped, config)
            .expect("clearly below-threshold fit");
        assert_eq!(dropped.finite_rank(), 1);

        let kept_column = x
            .iter()
            .zip(&z)
            .map(|(x, z)| x + 1e-3 * z)
            .collect::<Vec<_>>();
        let kept = prepared(
            &y,
            &[x, kept_column],
            &[false, false],
            None,
            false,
            TestWeights::Unit,
        );
        let kept =
            fit_prepared_ols_coefficients_map(&kept, config).expect("clearly above-threshold fit");
        assert_eq!(kept.finite_rank(), 2);
    }

    #[test]
    fn all_weight_kinds_and_common_scaling_preserve_coefficients() {
        let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 5.0, 7.0];
        let y = vec![2.2, 0.9, 1.5, 4.4, 4.8, 8.2, 11.1, 15.8];
        let weights = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
        let frequencies = vec![1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 3.0, 2.0];
        let mut coefficients = Vec::new();
        for case in [
            TestWeights::Analytic(&weights),
            TestWeights::Probability(&weights),
            TestWeights::Frequency(&frequencies),
        ] {
            let prepared = prepared(&y, std::slice::from_ref(&x), &[false], None, true, case);
            let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
                .expect("weighted fit");
            coefficients.push((
                fit.coefficients()[0].value.expect("slope"),
                fit.coefficients()[1].value.expect("intercept"),
            ));
        }
        assert_close(coefficients[0].0, coefficients[1].0, 1e-12);
        assert_close(coefficients[0].1, coefficients[1].1, 1e-12);

        for scale in [1e-200, 1e200] {
            let scaled = weights
                .iter()
                .map(|value| scale * value)
                .collect::<Vec<_>>();
            let prepared = prepared(
                &y,
                std::slice::from_ref(&x),
                &[false],
                None,
                true,
                TestWeights::Analytic(&scaled),
            );
            let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
                .expect("scaled analytic fit");
            assert_close(
                fit.coefficients()[0].value.expect("scaled slope"),
                coefficients[0].0,
                2e-11,
            );
            assert_close(
                fit.coefficients()[1].value.expect("scaled intercept"),
                coefficients[0].1,
                2e-11,
            );
        }
    }

    #[test]
    fn ols_adapter_and_borrowed_frame_are_exact_for_all_weight_kinds_and_modes() {
        let g = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
        let x = vec![-2.0, 0.5, -1.0, 2.0, -0.25, 3.0, 1.0, 4.5];
        let y = vec![1.2, 4.0, -0.5, 6.1, 2.7, 8.3, 5.5, 11.8];
        let continuous_weights = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
        let frequency_weights = vec![1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 3.0, 2.0];
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for case in [
                TestWeights::Unit,
                TestWeights::Analytic(&continuous_weights),
                TestWeights::Probability(&continuous_weights),
                TestWeights::Frequency(&frequency_weights),
            ] {
                let prepared =
                    prepared(&y, std::slice::from_ref(&x), &[false], Some(&g), true, case);
                let config = PreparedOlsFitConfig {
                    execution: PreparedOlsExecutionConfig {
                        mode,
                        poolsize: 1,
                        ..PreparedOlsExecutionConfig::default()
                    },
                    ..PreparedOlsFitConfig::default()
                };
                let adapter =
                    fit_prepared_ols_coefficients_map(&prepared, config).expect("OLS adapter fit");
                let derived =
                    LinearSolveFrame::from_prepared_ols(&prepared).expect("derived OLS frame");
                let explicit = LinearSolveFrame::from_parts(
                    derived.response(),
                    derived.raw_weights(),
                    derived.bread_weight_scale(),
                )
                .expect("explicit borrowed frame");
                let framed = fit_prepared_weighted_linear_once(&prepared, explicit, config)
                    .expect("explicit-frame fit");
                assert_eq!(
                    without_solve_timings(framed),
                    without_solve_timings(adapter)
                );
            }
        }
    }

    #[test]
    fn categorical_one_fe_initial_projection_preserves_warm_and_retained_paths() {
        let rows = 16_384;
        let x = source(1, "x");
        let g = source(2, "g");
        let h = source(3, "h");
        let outcome = (0..rows).map(|r| (r % 53) as f64).collect::<Vec<_>>();
        let g_values = (0..rows).map(|r| (r % 97) as f64).collect::<Vec<_>>();
        let h_values = (0..rows).map(|r| ((r / 97) % 3) as f64).collect::<Vec<_>>();
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![x.clone(), g.clone(), h.clone()],
                terms: vec![continuous_term(1, x.id, false)],
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![g.id, h.id],
                slope_sources: Vec::new(),
                label: "g#h".to_owned(),
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
                    role: OlsInputRole::NumericSource(x.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(g.id),
                },
                OlsInputBinding {
                    stata_var_position: 5,
                    role: OlsInputRole::NumericSource(h.id),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        };
        let prepared = crate::prepare_categorical_ols_data(
            &request,
            &LoadedOlsInput {
                candidate: vec![true; rows],
                outcome: loaded(&outcome),
                numeric_sources: vec![
                    (x.id, loaded(&outcome)),
                    (g.id, loaded(&g_values)),
                    (h.id, loaded(&h_values)),
                ],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .unwrap();
        let config = AbsorbConfig::default();
        assert_eq!(config.solver, FeSolverKind::Auto);
        for width in [1, 17, 65] {
            let columns = (0..width)
                .map(|c| {
                    (0..rows)
                        .map(|r| {
                            let between = if c == width - 1 { 1e6 } else { 3.0 };
                            between * (g_values[r] + h_values[r])
                                + ((r * (c + 7) % 997) as f64).sin()
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let raw = DenseMatrix::from_columns(&columns).unwrap();
            for weight_case in 0..4 {
                let supplied = (0..rows)
                    .map(|r| match weight_case {
                        1 => 128.0,
                        2 => (1 + r % 7) as f64,
                        _ => {
                            if r % 5 == 0 {
                                1_048_576.0
                            } else {
                                1.0 / 1_048_576.0
                            }
                        }
                    })
                    .collect::<Vec<_>>();
                let weights = if weight_case == 0 {
                    BorrowedMapWeights::Unit
                } else {
                    BorrowedMapWeights::Provided(&supplied)
                };
                let mut across_threads = None;
                for threads in [1, 7] {
                    crate::parallel::ThreadTeam::new(
                        ThreadConfig::from_requested(threads, true).unwrap(),
                    )
                    .install(|| {
                        let direct = prepared
                            .fit_absorb_matrix(weights, raw.clone(), config)
                            .unwrap();
                        assert_eq!(direct.report().iterations, 1);
                        assert!(!direct.report().fallback_used);
                        let residuals = direct.residualized();
                        // Independent anchored group means on the original two-key frame.
                        for c in 0..width {
                            for group in 0..291 {
                                let group_rows = (group..rows).step_by(291);
                                let anchor = columns[c][group];
                                let mut sum = 0.0;
                                let mut mass = 0.0;
                                for r in group_rows.clone() {
                                    let w = if weight_case == 0 { 1.0 } else { supplied[r] };
                                    sum += (columns[c][r] - anchor) * w;
                                    mass += w;
                                }
                                for r in group_rows {
                                    let expected = (columns[c][r] - anchor) - sum / mass;
                                    assert!(
                                        (residuals[(r, c)] - expected).abs() < 4e-8,
                                        "width={width} weights={weight_case} row={r} column={c}"
                                    );
                                }
                            }
                        }
                        if let Some(previous) = &across_threads {
                            assert_eq!(residuals, previous);
                        } else {
                            across_threads = Some(residuals.clone());
                        }
                        let warm = prepared
                            .fit_absorb_matrix(weights, residuals.clone(), config)
                            .unwrap();
                        assert_eq!(warm.report().iterations, 0);
                        assert_eq!(warm.residualized(), residuals);
                        for (solver, retain) in
                            [(FeSolverKind::Auto, true), (FeSolverKind::Map, false)]
                        {
                            let mut unchanged = config;
                            unchanged.solver = solver;
                            unchanged.retain_coefficients = retain;
                            let initial = prepared
                                .fit_absorb_matrix(weights, raw.clone(), unchanged)
                                .unwrap();
                            let ordinary = prepared
                                .fit_absorb_matrix(weights, raw.clone(), unchanged)
                                .unwrap();
                            assert_eq!(initial.residualized(), ordinary.residualized());
                            assert_eq!(initial.report().iterations, ordinary.report().iterations);
                        }
                    })
                    .unwrap();
                }
            }
        }
    }

    #[test]
    fn categorical_ols_adapter_and_borrowed_frame_are_exact() {
        let x = source(1, "x");
        let g = source(2, "g");
        let outcome = vec![1.0, 3.5, 2.0, 7.0, 4.5, 10.0, 8.0, 13.0];
        let x_values = vec![-1.0, 0.5, -2.0, 2.0, -0.5, 3.0, 1.0, 4.0];
        let g_values = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![x.clone(), g.clone()],
                terms: vec![continuous_term(1, x.id, false)],
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorbed term"),
                intercept: true,
                categorical_sources: vec![g.id],
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
                    role: OlsInputRole::NumericSource(x.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(g.id),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        };
        let prepared = crate::prepare_categorical_ols_data(
            &request,
            &LoadedOlsInput {
                candidate: vec![true; outcome.len()],
                outcome: loaded(&outcome),
                numeric_sources: vec![(x.id, loaded(&x_values)), (g.id, loaded(&g_values))],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("categorical prepared data");
        let config = PreparedOlsFitConfig::default();
        let adapter = fit_prepared_categorical_ols_coefficients_map(&prepared, config)
            .expect("categorical OLS adapter fit");
        let derived =
            LinearSolveFrame::from_prepared_ols(&prepared).expect("categorical OLS frame");
        let explicit = LinearSolveFrame::from_parts(
            derived.response(),
            derived.raw_weights(),
            derived.bread_weight_scale(),
        )
        .expect("explicit categorical frame");
        let framed = fit_prepared_weighted_linear_once(&prepared, explicit, config)
            .expect("categorical explicit-frame fit");
        assert_eq!(
            without_solve_timings(framed),
            without_solve_timings(adapter)
        );
    }

    #[test]
    fn initial_full_width_lsmr_preserves_canonical_multiway_rank_policy() {
        let rows = 384_usize;
        let effects = [
            (0..rows)
                .map(|row| (row % 17 + 1) as f64)
                .collect::<Vec<_>>(),
            (0..rows)
                .map(|row| ((7 * row + row / 17) % 13 + 1) as f64)
                .collect::<Vec<_>>(),
            (0..rows)
                .map(|row| ((11 * row + row / 13) % 9 + 1) as f64)
                .collect::<Vec<_>>(),
        ];
        let axis = (0..rows)
            .map(|row| row as f64 - (rows - 1) as f64 / 2.0)
            .collect::<Vec<_>>();
        let independent = (0..8_usize)
            .map(|column| {
                axis.iter()
                    .enumerate()
                    .map(|(row, value)| {
                        (value * (column + 3) as f64 / 97.0).sin()
                            + ((row * (column + 5) % 23) as f64 - 11.0) / 29.0
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let perturbation = axis
            .iter()
            .enumerate()
            .map(|(row, value)| (value / 31.0).cos() + (row % 7) as f64 / 19.0)
            .collect::<Vec<_>>();
        let outcome = (0..rows)
            .map(|row| {
                0.4 * independent[0][row] - 0.25 * independent[1][row]
                    + 0.15 * independent[2][row]
                    + 0.03 * effects[0][row]
                    - 0.02 * effects[1][row]
                    + (row % 5) as f64 / 41.0
            })
            .collect::<Vec<_>>();

        let mut rank_edges = independent.clone();
        rank_edges.push(effects[0].clone());
        rank_edges.push(independent[0].clone());
        rank_edges.push(
            independent[1]
                .iter()
                .zip(&perturbation)
                .map(|(value, perturbation)| value + 1e-13 * perturbation)
                .collect(),
        );
        rank_edges.push(
            independent[2]
                .iter()
                .zip(&perturbation)
                .map(|(value, perturbation)| value + 1e-5 * perturbation)
                .collect(),
        );
        let mut reordered = rank_edges.clone();
        reordered.reverse();
        let wide = (1..=50_usize)
            .map(|mask| {
                (0..rows)
                    .map(|row| {
                        if (row & mask).count_ones() % 2 == 0 {
                            1.0
                        } else {
                            -1.0
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let mut certified_schur_cases = 0_usize;
        for effect_count in [2_usize, 3] {
            for threads in [1_usize, 7] {
                for regressors in [
                    Vec::new(),
                    rank_edges.clone(),
                    reordered.clone(),
                    wide.clone(),
                ] {
                    let prepared = prepared_multiway_categorical(
                        &outcome,
                        &regressors,
                        &effects[..effect_count],
                    );
                    certified_schur_cases += usize::from(
                        assert_initial_checkpoint_lsmr_matches_map(&prepared, threads)
                            == FeSolverKind::Map,
                    );
                }
            }
        }
        assert!(
            certified_schur_cases > 0,
            "the rank matrix must exercise a certified Schur checkpoint"
        );
    }

    #[test]
    fn initial_checkpoint_lsmr_falls_back_only_after_numerical_noncertification() {
        let rows = 192_usize;
        let effects = vec![
            (0..rows)
                .map(|row| (row % 17 + 1) as f64)
                .collect::<Vec<_>>(),
            (0..rows)
                .map(|row| ((5 * row + row / 17) % 13 + 1) as f64)
                .collect::<Vec<_>>(),
        ];
        let regressors = vec![
            (0..rows)
                .map(|row| (row as f64 / 17.0).sin())
                .collect::<Vec<_>>(),
            (0..rows)
                .map(|row| (row as f64 / 23.0).cos())
                .collect::<Vec<_>>(),
        ];
        let outcome = (0..rows)
            .map(|row| {
                0.5 * regressors[0][row] - 0.2 * regressors[1][row]
                    + 0.03 * effects[0][row]
                    + (row % 7) as f64 / 31.0
            })
            .collect::<Vec<_>>();
        let prepared = prepared_multiway_categorical(&outcome, &regressors, &effects);
        let mut config = PreparedOlsFitConfig::default();
        config.absorb.solver = FeSolverKind::Map;
        config.absorb.map.tolerance = 1e-6;
        config.absorb.lsmr.tolerance = 1e-6;
        let frame = LinearSolveFrame::from_prepared_ols(&prepared).expect("initial frame");
        let generations = LinearSessionGenerations::new(1, 1, 1, 1, 1);

        let mut expected_session = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            frame,
            config,
            generations,
            prepared.fit_owned_fe_design().expect("MAP FE design"),
        )
        .expect("MAP initial session");
        let expected = expected_session.solve_fresh().expect("MAP initial solve");

        let mut fallback_session = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            frame,
            config,
            generations,
            prepared.fit_owned_fe_design().expect("fallback FE design"),
        )
        .expect("fallback initial session");
        fallback_session.enable_checkpoint_lsmr(true);
        fallback_session.inject_checkpoint_lsmr_certificate_failure();
        let fallback = fallback_session
            .solve_fresh()
            .expect("independently certified MAP fallback");
        assert_eq!(
            fallback.frame().absorption_report().requested_solver,
            FeSolverKind::Lsmr
        );
        assert_eq!(
            fallback.frame().absorption_report().used_solver,
            FeSolverKind::Map,
            "design={:?}, residual={:?}",
            fallback.frame().absorption_report(),
            fallback.frame().residual_absorption_report(),
        );
        assert!(fallback.frame().absorption_report().fallback_used);
        assert_eq!(
            fallback.frame().finite_rank(),
            expected.frame().finite_rank()
        );
        assert_eq!(
            fallback.basis().kept_generated_columns(),
            expected.basis().kept_generated_columns()
        );
        assert_eq!(
            fallback.basis().dropped_generated_columns(),
            expected.basis().dropped_generated_columns()
        );
        for (fallback, expected) in fallback.coefficients().zip(expected.coefficients()) {
            assert_eq!(fallback.status, expected.status);
            match (fallback.value, expected.value) {
                (Some(fallback), Some(expected)) => assert_close(fallback, expected, 2e-12),
                (None, None) => {}
                _ => panic!("fallback coefficient presence differs"),
            }
        }

        for code in [ErrorCode::Allocation, ErrorCode::Interrupted] {
            let mut session = CertifiedLinearSession::new_with_fe_design(
                &prepared,
                frame,
                config,
                generations,
                prepared.fit_owned_fe_design().expect("fatal FE design"),
            )
            .expect("fatal initial session");
            session.enable_checkpoint_lsmr(true);
            session.inject_checkpoint_lsmr_fatal_failure(code);
            let error = match session.solve_fresh() {
                Ok(_) => panic!("fatal LSMR failure must not fall back"),
                Err(error) => error,
            };
            assert_eq!(error.code, code);
            assert!(session.output().is_err());
            let retry = session
                .solve_fresh()
                .expect("fatal injection must leave the session reusable");
            assert_eq!(
                retry.frame().absorption_report().requested_solver,
                FeSolverKind::Lsmr
            );
            // With the fault consumed, the admitted two-FE Schur accelerator
            // can again certify the requested checkpoint before LSMR runs.
            assert_eq!(
                retry.frame().absorption_report().used_solver,
                FeSolverKind::Map
            );
            assert!(!retry.frame().absorption_report().fallback_used);
            assert!(retry.certified());
        }

        for code in [ErrorCode::Allocation, ErrorCode::Interrupted] {
            let mut session = CertifiedLinearSession::new_with_fe_design(
                &prepared,
                frame,
                config,
                generations,
                prepared.fit_owned_fe_design().expect("rank FE design"),
            )
            .expect("rank initial session");
            session.enable_checkpoint_lsmr(true);
            session.inject_fresh_rank_failure(code);
            let error = match session.solve_fresh() {
                Ok(_) => panic!("canonical rank failure must remain fatal"),
                Err(error) => error,
            };
            assert_eq!(error.code, code);
            assert!(session.output().is_err());
            let retry = session
                .solve_fresh()
                .expect("rank failure must leave the private session reusable");
            assert_eq!(retry.frame().finite_rank(), expected.frame().finite_rank());
            assert!(retry.certified());
        }
    }

    #[test]
    fn categorical_recovery_uses_explicit_final_hessian_weights_for_normalization() {
        let worker = source(1, "worker");
        let firm = source(2, "firm");
        let worker_codes = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0];
        let firm_codes = vec![1.0, 2.0, 1.0, 2.0, 1.0, 2.0];
        let worker_effect = [-1.0, -1.0, 0.25, 0.25, 0.75, 0.75];
        let firm_effect = [-0.5, 0.5, -0.5, 0.5, -0.5, 0.5];
        let target = worker_effect
            .iter()
            .zip(firm_effect)
            .map(|(left, right)| left + right)
            .collect::<Vec<_>>();
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![worker.clone(), firm.clone()],
                terms: Vec::new(),
            },
            absorbed_terms: vec![
                AbsorbedTerm {
                    id: AbsorbTermId::new(1).expect("worker absorb"),
                    intercept: true,
                    categorical_sources: vec![worker.id],
                    slope_sources: Vec::new(),
                    label: "worker".to_owned(),
                    saved_name: String::new(),
                },
                AbsorbedTerm {
                    id: AbsorbTermId::new(2).expect("firm absorb"),
                    intercept: true,
                    categorical_sources: vec![firm.id],
                    slope_sources: Vec::new(),
                    label: "firm".to_owned(),
                    saved_name: String::new(),
                },
            ],
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
                    role: OlsInputRole::NumericSource(worker.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(firm.id),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        };
        let prepared = crate::prepare_categorical_ols_data(
            &request,
            &LoadedOlsInput {
                candidate: vec![true; target.len()],
                outcome: loaded(&target),
                numeric_sources: vec![
                    (worker.id, loaded(&worker_codes)),
                    (firm.id, loaded(&firm_codes)),
                ],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("categorical recovery preparation");
        let requested = [true, true];
        let map = PreparedOlsFitConfig::default().absorb.map;
        let unit = prepared
            .recover_absorbed_effects_map_with_weights(
                BorrowedMapWeights::Unit,
                &target,
                &requested,
                map,
            )
            .expect("unit-weight recovery");
        // A nonconstant mean changes the final PPML Hessian from q to q*mu.
        // The saved FE decomposition must therefore be normalized on this
        // explicit final curvature measure, even though its absorbed sum is
        // unchanged.
        let final_hessian = [1.0, 3.0, 2.0, 9.0, 4.0, 27.0];
        let hessian = prepared
            .recover_absorbed_effects_map_with_weights(
                BorrowedMapWeights::Provided(&final_hessian),
                &target,
                &requested,
                map,
            )
            .expect("final-Hessian recovery");
        for recovered in [&unit, &hessian] {
            for (actual, expected) in recovered.absorbed_sum().iter().zip(&target) {
                assert_close(*actual, *expected, 2e-10);
            }
        }
        let unit_firm = unit.channels()[1].as_ref().expect("unit firm channel");
        let hessian_firm = hessian.channels()[1]
            .as_ref()
            .expect("Hessian firm channel");
        let unit_mean = unit_firm.iter().sum::<f64>() / unit_firm.len() as f64;
        assert_close(unit_mean, 0.0, 2e-10);
        let hessian_sum = final_hessian.iter().sum::<f64>();
        let hessian_mean = hessian_firm
            .iter()
            .zip(final_hessian)
            .map(|(value, weight)| value * weight)
            .sum::<f64>()
            / hessian_sum;
        assert_close(hessian_mean, 0.0, 2e-10);
        assert!(
            unit_firm
                .iter()
                .zip(hessian_firm)
                .any(|(left, right)| (left - right).abs() > 1e-3),
            "nonconstant q*mu must change the normalized saved-FE channels"
        );
    }

    #[test]
    fn empty_categorical_topology_preserves_finite_intercept_across_session_routes() {
        let x = source(1, "x");
        let y = vec![1.0, 2.5, 2.0, 5.0, 4.5, 8.0, 7.0, 11.5];
        let refreshed_y = vec![1.4, 2.1, 2.7, 4.6, 5.2, 7.4, 7.8, 10.9];
        let x_values = vec![-2.0, -1.0, -0.5, 0.5, 1.0, 2.0, 3.0, 4.0];
        let weights = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
        let refreshed_weights = vec![2.0, 0.75, 1.5, 0.6, 3.5, 1.25, 2.2, 1.8];
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![x.clone()],
                terms: vec![continuous_term(1, x.id, false)],
            },
            absorbed_terms: Vec::new(),
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
                    role: OlsInputRole::NumericSource(x.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::AnalyticWeight,
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        };
        let loaded_with = |outcome: &[f64], analytic_weight: &[f64]| LoadedOlsInput {
            candidate: vec![true; outcome.len()],
            outcome: loaded(outcome),
            numeric_sources: vec![(x.id, loaded(&x_values))],
            analytic_weight: Some(loaded(analytic_weight)),
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = crate::prepared_categorical_data::prepare_ppml_categorical_ols_data(
            &request,
            &loaded_with(&y, &weights),
            &[],
        )
        .expect("empty categorical topology");
        let refreshed_prepared =
            crate::prepared_categorical_data::prepare_ppml_categorical_ols_data(
                &request,
                &loaded_with(&refreshed_y, &refreshed_weights),
                &[],
            )
            .expect("refreshed empty categorical topology");
        assert!(!prepared.has_absorbed_intercept());
        assert!(prepared.effect_specs().is_empty());

        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let config = PreparedOlsFitConfig {
                absorb: AbsorbConfig {
                    solver: FeSolverKind::Map,
                    ..AbsorbConfig::default()
                },
                execution: PreparedOlsExecutionConfig {
                    mode,
                    poolsize: 1,
                    ..PreparedOlsExecutionConfig::default()
                },
                ..PreparedOlsFitConfig::default()
            };
            let initial_frame =
                LinearSolveFrame::from_prepared_ols(&prepared).expect("initial frame");
            let mut session = CertifiedLinearSession::new(
                &prepared,
                initial_frame,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 0),
            )
            .expect("empty-topology session");
            ACTIVE_LINEAR_ALLOCATION_PHASE.store(0, Ordering::SeqCst);
            LINEAR_PROJECTION_ENDS.store(0, Ordering::SeqCst);
            LINEAR_FINITE_ENDS.store(0, Ordering::SeqCst);
            session.set_phase_allocator_probe(Some(observed_linear_allocation_phase));
            let fresh = session.solve_fresh().expect("empty-topology fresh solve");
            assert_eq!(
                fresh.reuse_report().phase_allocation.projection_peak_bytes,
                Some(11)
            );
            assert_eq!(
                fresh
                    .reuse_report()
                    .phase_allocation
                    .finite_solve_peak_bytes,
                Some(23)
            );
            assert_eq!(LINEAR_PROJECTION_ENDS.load(Ordering::SeqCst), 1);
            assert_eq!(LINEAR_FINITE_ENDS.load(Ordering::SeqCst), 1);
            assert_eq!(
                fresh.frame().finite_rank(),
                2,
                "retained coefficients: {:?}",
                fresh.coefficients().collect::<Vec<_>>()
            );
            assert!(fresh.coefficients().any(|coefficient| {
                coefficient.status == PreparedOlsFitStatus::Kept
                    && coefficient.generated_column_index.is_some_and(|generated| {
                        matches!(
                            prepared.model().columns[generated].kind,
                            PreparedOlsColumnKind::Intercept
                        )
                    })
            }));
            let refreshed_frame =
                LinearSolveFrame::from_prepared_ols(&refreshed_prepared).expect("refreshed frame");
            session
                .refresh_response(&refreshed_y, 1)
                .expect("response refresh");
            session
                .refresh_weights(
                    refreshed_frame.raw_weights(),
                    refreshed_frame.bread_weight_scale(),
                    1,
                )
                .expect("weight refresh");
            let expected = fit_prepared_weighted_linear_once(
                &prepared,
                LinearSolveFrame::from_parts(
                    &refreshed_y,
                    refreshed_frame.raw_weights(),
                    refreshed_frame.bread_weight_scale(),
                )
                .expect("canonical refreshed frame"),
                config,
            )
            .expect("canonical refreshed fit");
            let frozen = session.solve_frozen().expect("empty-topology frozen solve");
            assert_eq!(
                frozen.reuse_report().phase_allocation.projection_peak_bytes,
                Some(11)
            );
            assert_eq!(
                frozen
                    .reuse_report()
                    .phase_allocation
                    .finite_solve_peak_bytes,
                Some(23)
            );
            assert_eq!(LINEAR_PROJECTION_ENDS.load(Ordering::SeqCst), 2);
            assert_eq!(LINEAR_FINITE_ENDS.load(Ordering::SeqCst), 2);
            assert_frozen_matches_fit(&frozen, &expected);
            assert_eq!(frozen.basis().kept_generated_columns().len(), 2);
            let strict = session.strictify().expect("empty-topology strict solve");
            assert_eq!(
                strict.reuse_report().phase_allocation.projection_peak_bytes,
                Some(11)
            );
            assert_eq!(
                strict
                    .reuse_report()
                    .phase_allocation
                    .finite_solve_peak_bytes,
                Some(23)
            );
            assert_eq!(LINEAR_PROJECTION_ENDS.load(Ordering::SeqCst), 3);
            assert_eq!(LINEAR_FINITE_ENDS.load(Ordering::SeqCst), 3);
            assert_eq!(ACTIVE_LINEAR_ALLOCATION_PHASE.load(Ordering::SeqCst), 0);
            assert_fit_science_equal(&strict, &expected);
            assert_eq!(strict.frame().finite_rank(), 2);
        }
    }

    #[test]
    fn ppml_slope_only_topology_preserves_structural_constant() {
        let group = source(1, "group");
        let slope = source(2, "slope");
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![group.clone(), slope.clone()],
                terms: Vec::new(),
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorb term"),
                intercept: false,
                categorical_sources: vec![group.id],
                slope_sources: vec![slope.id],
                label: "group#c.slope".to_owned(),
                saved_name: "saved_slope".to_owned(),
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
                    role: OlsInputRole::NumericSource(group.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(slope.id),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        };
        let loaded_input = LoadedOlsInput {
            candidate: vec![true; 6],
            outcome: loaded(&[1.0, 2.0, 1.5, 3.0, 2.5, 4.0]),
            numeric_sources: vec![
                (group.id, loaded(&[1.0, 1.0, 1.0, 2.0, 2.0, 2.0])),
                (slope.id, loaded(&[-1.0, 0.0, 1.0, -1.0, 0.0, 1.0])),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };

        let prepared = crate::prepared_categorical_data::prepare_ppml_categorical_ols_data(
            &request,
            &loaded_input,
            &[],
        )
        .expect("PPML slope-only topology");
        assert!(prepared.has_absorbed_slope());
        assert!(!prepared.has_absorbed_intercept());
        assert!(prepared.structural_intercept());
        assert!(prepared.report_intercept());
        let frame = LinearSolveFrame::from_prepared_ols(&prepared).expect("slope-only frame");
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let config = PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode,
                    poolsize: 2,
                    threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
                },
                ..PreparedOlsFitConfig::default()
            };
            let expected = fit_prepared_weighted_linear_once(&prepared, frame, config)
                .expect("slope-only one-shot fit");
            let mut session = CertifiedLinearSession::new(
                &prepared,
                frame,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 0),
            )
            .expect("slope-only retained session");
            let fresh = session.solve_fresh().expect("slope-only fresh");
            assert_fit_science_equal(&fresh, &expected);
            assert_fit_science_equal(&session.strictify().expect("slope-only strict"), &expected);
        }
    }

    #[test]
    fn projection_only_matches_full_fit_without_constructing_reported_bread() {
        let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 4.0, 5.0, 7.0];
        let y = vec![1.2, 0.9, 1.5, 4.4, 4.8, 8.2, 11.1, 15.8];
        let weights = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
        let prepared_data = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            None,
            true,
            TestWeights::Analytic(&weights),
        );
        let config = PreparedOlsFitConfig::default();
        let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("frame");
        let full =
            fit_prepared_weighted_linear_once(&prepared_data, frame, config).expect("full fit");
        let projection =
            fit_prepared_weighted_linear_projection_once(&prepared_data, frame, config)
                .expect("projection-only fit");
        assert!(projection.certified());
        assert_eq!(projection.finite_rank(), full.finite_rank());
        assert_eq!(
            projection.kept_generated_columns(),
            full.kept_generated_columns()
        );
        assert_eq!(
            projection.dropped_generated_columns(),
            full.dropped_generated_columns()
        );
        assert_eq!(projection.coefficients().len(), full.coefficients().len());
        for (projected, reported) in projection.coefficients().iter().zip(full.coefficients()) {
            if reported.status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept {
                assert_eq!(
                    projected.status,
                    PreparedOlsFitStatus::AbsorbedInterceptDeferred
                );
                assert_eq!(projected.value, None);
                assert_eq!(projected.coefficient_slot, reported.coefficient_slot);
                assert_eq!(
                    projected.generated_column_index,
                    reported.generated_column_index
                );
            } else {
                assert_eq!(projected, reported);
            }
        }
        assert_eq!(projection.rank_certificate(), full.rank_certificate());
        assert_eq!(projection.score_certificate(), full.score_certificate());
        for (actual, expected) in projection.residuals().iter().zip(full.residuals()) {
            assert_close(*actual, *expected, 2e-12);
        }
        for ((fitted, residual), response) in projection
            .fitted()
            .iter()
            .zip(projection.residuals())
            .zip(&y)
        {
            assert_close(*fitted + *residual, *response, 2e-15);
        }
        assert!(projection.reconstruction_certificate().passed);
        assert!(projection.absorption_report().certified());
        assert!(projection.residual_absorption_report().certified());
    }

    fn assert_frozen_matches_fit(
        frozen: &crate::ols::CertifiedFrozenLinearOutput<'_>,
        expected: &CertifiedPreparedOlsFit,
    ) {
        assert!(frozen.certified());
        assert_eq!(
            frozen.basis().kept_generated_columns(),
            expected.kept_generated_columns()
        );
        for (position, generated) in frozen
            .basis()
            .kept_generated_columns()
            .iter()
            .copied()
            .enumerate()
        {
            let expected_coefficient = expected
                .coefficients()
                .iter()
                .find(|coefficient| {
                    coefficient.generated_column_index == Some(generated)
                        && coefficient.status == PreparedOlsFitStatus::Kept
                })
                .and_then(|coefficient| coefficient.value)
                .expect("expected kept coefficient");
            assert_close(frozen.coefficients()[position], expected_coefficient, 5e-10);
        }
        assert_eq!(
            frozen.reported_coefficients().len(),
            expected.coefficients().len()
        );
        for (actual, expected) in frozen
            .reported_coefficients()
            .iter()
            .zip(expected.coefficients())
        {
            match (*actual, expected.value) {
                (Some(actual), Some(expected)) => assert_close(actual, expected, 5e-10),
                (None, None) => {}
                _ => panic!("frozen reported coefficient presence differs"),
            }
        }
        let rank = frozen.coefficients().len();
        assert_eq!(frozen.bread().len(), rank * rank);
        for (left, left_generated) in frozen
            .basis()
            .kept_generated_columns()
            .iter()
            .copied()
            .enumerate()
        {
            let left_position = expected
                .bread()
                .coordinates()
                .iter()
                .position(|coordinate| {
                    coordinate.generated_column_index == left_generated
                        && coordinate.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
                })
                .expect("expected left bread coordinate");
            for (right, right_generated) in frozen
                .basis()
                .kept_generated_columns()
                .iter()
                .copied()
                .enumerate()
            {
                let right_position = expected
                    .bread()
                    .coordinates()
                    .iter()
                    .position(|coordinate| {
                        coordinate.generated_column_index == right_generated
                            && coordinate.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
                    })
                    .expect("expected right bread coordinate");
                let dimension = expected.bread().dimension();
                assert_close(
                    frozen.bread()[left * rank + right],
                    expected.bread().values()[left_position * dimension + right_position],
                    2e-9,
                );
            }
        }
        assert_eq!(frozen.finite_within_rows(), expected.residuals().len());
        assert_eq!(frozen.finite_within_columns(), rank);
        assert!(frozen.reconstruction_certificate().passed);
    }

    trait LinearScienceWitness {
        fn coefficients(&self) -> Vec<PreparedOlsFittedCoefficient>;
        fn kept(&self) -> &[usize];
        fn dropped(&self) -> &[usize];
        fn rank(&self) -> PreparedOlsRankCertificate;
        fn score(&self) -> PreparedOlsScoreCertificate;
        fn bread(&self) -> Vec<f64>;
        fn bread_passed(&self) -> bool;
        fn residuals(&self) -> &[f64];
        fn absorption(&self) -> &SolveReport;
        fn residual_absorption(&self) -> &SolveReport;
    }

    impl LinearScienceWitness for CertifiedPreparedOlsFit {
        fn coefficients(&self) -> Vec<PreparedOlsFittedCoefficient> {
            self.coefficients().to_vec()
        }
        fn kept(&self) -> &[usize] {
            self.kept_generated_columns()
        }
        fn dropped(&self) -> &[usize] {
            self.dropped_generated_columns()
        }
        fn rank(&self) -> PreparedOlsRankCertificate {
            self.rank_certificate()
        }
        fn score(&self) -> PreparedOlsScoreCertificate {
            self.score_certificate()
        }
        fn bread(&self) -> Vec<f64> {
            let dimension = self.bread().coordinates().len();
            self.kept_generated_columns()
                .iter()
                .flat_map(|left| {
                    let i = self
                        .bread()
                        .coordinates()
                        .iter()
                        .position(|c| {
                            c.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
                                && c.generated_column_index == *left
                        })
                        .unwrap();
                    self.kept_generated_columns().iter().map(move |right| {
                        let j = self
                            .bread()
                            .coordinates()
                            .iter()
                            .position(|c| {
                                c.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor
                                    && c.generated_column_index == *right
                            })
                            .unwrap();
                        self.bread().values()[i * dimension + j]
                    })
                })
                .collect()
        }
        fn bread_passed(&self) -> bool {
            self.bread().certificate().passed
        }
        fn residuals(&self) -> &[f64] {
            self.residuals()
        }
        fn absorption(&self) -> &SolveReport {
            self.absorption_report()
        }
        fn residual_absorption(&self) -> &SolveReport {
            self.residual_absorption_report()
        }
    }

    impl LinearScienceWitness for CertifiedLinearSessionOutput<'_> {
        fn coefficients(&self) -> Vec<PreparedOlsFittedCoefficient> {
            self.coefficients().collect()
        }
        fn kept(&self) -> &[usize] {
            self.basis().kept_generated_columns()
        }
        fn dropped(&self) -> &[usize] {
            self.basis().dropped_generated_columns()
        }
        fn rank(&self) -> PreparedOlsRankCertificate {
            *self.frame().rank_certificate()
        }
        fn score(&self) -> PreparedOlsScoreCertificate {
            *self.frame().score_certificate()
        }
        fn bread(&self) -> Vec<f64> {
            self.frame().bread().to_vec()
        }
        fn bread_passed(&self) -> bool {
            self.frame().canonical_proof().unwrap().bread.passed
        }
        fn residuals(&self) -> &[f64] {
            self.frame().residuals()
        }
        fn absorption(&self) -> &SolveReport {
            self.frame().absorption_report()
        }
        fn residual_absorption(&self) -> &SolveReport {
            self.frame().residual_absorption_report()
        }
    }

    impl LinearScienceWitness for CertifiedLinearSessionProducts {
        fn coefficients(&self) -> Vec<PreparedOlsFittedCoefficient> {
            self.basis()
                .slots()
                .iter()
                .zip(&self.frame().reported_coefficients)
                .map(|(slot, &value)| PreparedOlsFittedCoefficient {
                    coefficient_slot: slot.coefficient_slot,
                    generated_column_index: slot.generated_column_index,
                    status: slot.status,
                    value,
                })
                .collect()
        }
        fn kept(&self) -> &[usize] {
            self.basis().kept_generated_columns()
        }
        fn dropped(&self) -> &[usize] {
            self.basis().dropped_generated_columns()
        }
        fn rank(&self) -> PreparedOlsRankCertificate {
            *self.frame().rank_certificate()
        }
        fn score(&self) -> PreparedOlsScoreCertificate {
            *self.frame().score_certificate()
        }
        fn bread(&self) -> Vec<f64> {
            self.frame().bread().to_vec()
        }
        fn bread_passed(&self) -> bool {
            self.frame().canonical_proof().unwrap().bread.passed
        }
        fn residuals(&self) -> &[f64] {
            self.frame().residuals()
        }
        fn absorption(&self) -> &SolveReport {
            self.frame().absorption_report()
        }
        fn residual_absorption(&self) -> &SolveReport {
            self.frame().residual_absorption_report()
        }
    }

    fn assert_fit_science_equal(
        actual: &impl LinearScienceWitness,
        expected: &impl LinearScienceWitness,
    ) {
        assert_eq!(actual.coefficients().len(), expected.coefficients().len());
        for (actual, expected) in actual.coefficients().iter().zip(expected.coefficients()) {
            assert_eq!(actual.coefficient_slot, expected.coefficient_slot);
            assert_eq!(
                actual.generated_column_index,
                expected.generated_column_index
            );
            assert_eq!(actual.status, expected.status);
            match (actual.value, expected.value) {
                (Some(actual), Some(expected)) => assert_close(actual, expected, 2e-12),
                (None, None) => {}
                _ => panic!("prepared fit coefficient presence differs"),
            }
        }
        assert_eq!(actual.kept(), expected.kept());
        assert_eq!(actual.dropped(), expected.dropped());
        assert_eq!(actual.kept().len(), expected.kept().len());
        assert_eq!(
            actual.rank().relative_threshold,
            expected.rank().relative_threshold
        );
        // The retained observation-space reduction compresses rows through a
        // streaming Givens factor before CPQR; one-shot fallback applies CPQR
        // directly to N rows.  The selected basis/rank and certificate pass
        // remain exact, while this descriptive trailing-residual magnitude can
        // differ by final-rounding noise.
        assert_close(
            actual.rank().maximum_dropped_trailing_norm,
            expected.rank().maximum_dropped_trailing_norm,
            512.0 * f64::EPSILON,
        );
        assert_eq!(actual.rank().passed, expected.rank().passed);
        // Retained rank revelation may compress rows through Givens before
        // CPQR while the one-shot fallback factors N rows directly. Basis and
        // rank decisions remain exact above. The descriptive diagonal-ratio
        // condition proxy is perturbation-sensitive on deliberately weak
        // full-rank designs, so its machine-precision comparison scales by
        // sqrt(kappa); no certificate threshold or publication value does.
        let condition_scale = actual
            .rank()
            .condition_proxy
            .max(expected.rank().condition_proxy)
            .max(1.0)
            .sqrt();
        assert_close(
            actual.rank().condition_proxy,
            expected.rank().condition_proxy,
            512.0 * f64::EPSILON * condition_scale,
        );
        assert_eq!(actual.score().passed, expected.score().passed);
        assert_eq!(actual.score().moments, expected.score().moments);
        assert_close(
            actual.score().relative_infinity_norm,
            expected.score().relative_infinity_norm,
            2e-11,
        );
        assert_close(
            actual.score().relative_two_norm,
            expected.score().relative_two_norm,
            2e-11,
        );
        let actual_bread = actual.bread();
        let expected_bread = expected.bread();
        assert_eq!(actual_bread.len(), expected_bread.len());
        for (&actual, &expected) in actual_bread.iter().zip(&expected_bread) {
            assert_close(actual, expected, 2e-12);
        }
        assert!(actual.bread_passed() && expected.bread_passed());
        assert_solve_report_science_equal(actual.absorption(), expected.absorption());
        assert_solve_report_science_equal(
            actual.residual_absorption(),
            expected.residual_absorption(),
        );
        assert_eq!(actual.residuals().len(), expected.residuals().len());
        for (&actual, &expected) in actual.residuals().iter().zip(expected.residuals()) {
            assert_close(actual, expected, 2e-12);
        }
    }

    fn assert_solve_report_science_equal(actual: &SolveReport, expected: &SolveReport) {
        assert_eq!(actual.requested_solver, expected.requested_solver);
        assert_eq!(actual.used_solver, expected.used_solver);
        // Iterations and the stop label describe work actually performed on
        // different, independently certified projection inputs.  Fresh
        // one-shot absorption owns a combined matrix and its QR routes may
        // reverse destructive Householder storage; retained warm/strict work
        // projects the response and preserves the sole row-stream design.
        // Their scientific equivalence is established below by both passing
        // FE certificates and by the exact fit/residual checks at the caller.
        assert!(actual.certified());
        assert!(expected.certified());
        assert_eq!(actual.fallback_used, expected.fallback_used);
        assert_eq!(
            actual.requested_preconditioner,
            expected.requested_preconditioner
        );
        assert_eq!(actual.used_preconditioner, expected.used_preconditioner);
        assert_eq!(actual.pilot_iterations, expected.pilot_iterations);
        // Pilot iteration counts and routing are exact above.  Once both
        // independently certified FE residuals are at the roundoff floor,
        // the reported contraction is a ratio of two vanishing diagnostics:
        // a one-shot Householder route (always in Low mode and for wide or
        // rank-deficient Fast designs) and the retained row stream may report
        // any finite ratio in [0, 1] even when their published residuals are
        // bit-identical. Outside that explicitly degenerate regime the
        // contraction remains a machine-precision comparison.
        let pilot_floor = 64.0 * f64::EPSILON;
        if actual.certificate.relative_two_norm <= pilot_floor
            && expected.certificate.relative_two_norm <= pilot_floor
            && actual.normal_equation_residual <= pilot_floor
            && expected.normal_equation_residual <= pilot_floor
        {
            assert!(actual.pilot_contraction.is_finite());
            assert!((0.0..=1.0).contains(&actual.pilot_contraction));
            assert!(expected.pilot_contraction.is_finite());
            assert!((0.0..=1.0).contains(&expected.pilot_contraction));
        } else {
            assert_close(
                actual.pilot_contraction,
                expected.pilot_contraction,
                512.0 * f64::EPSILON,
            );
        }
        assert_eq!(actual.warnings, expected.warnings);
        assert_eq!(actual.certificate.passed, expected.certificate.passed);
        // The retained route uses its owned reusable MAP workspace while the
        // one-shot reference uses the borrowed MAP adapter. They execute the
        // same solver and stop policy, but their independent certificate
        // reductions can differ in the final rounding bits. Keep this at a
        // machine-precision bound; neither publication tolerance is relaxed.
        let diagnostic_tolerance = 512.0 * f64::EPSILON;
        assert_close(
            actual.certificate.absolute_infinity_norm,
            expected.certificate.absolute_infinity_norm,
            diagnostic_tolerance,
        );
        assert_close(
            actual.certificate.relative_infinity_norm,
            expected.certificate.relative_infinity_norm,
            diagnostic_tolerance,
        );
        assert_close(
            actual.certificate.relative_two_norm,
            expected.certificate.relative_two_norm,
            diagnostic_tolerance,
        );
        assert_close(
            actual.normal_equation_residual,
            expected.normal_equation_residual,
            diagnostic_tolerance,
        );
    }

    fn assert_retained_report_has_no_warning_storage(report: &SolveReport) {
        assert!(report.warnings.is_empty());
        assert_eq!(report.warnings.capacity(), 0);
    }

    fn assert_retained_fit_has_no_warning_storage(
        fit: &crate::ols::linear_session::CertifiedLinearFrame,
    ) {
        assert_retained_report_has_no_warning_storage(fit.absorption_report());
        assert_retained_report_has_no_warning_storage(fit.residual_absorption_report());
    }

    #[test]
    fn retained_fresh_frozen_and_strict_kernels_use_requested_team_against_global_pool() {
        const CHILD_ENV: &str = "FEREG_RETAINED_THREAD_TEAM_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let executable = std::env::current_exe().expect("current Rust test executable");
            let output = std::process::Command::new(executable)
                .arg("retained_fresh_frozen_and_strict_kernels_use_requested_team_against_global_pool")
                .arg("--nocapture")
                .env(CHILD_ENV, "1")
                .output()
                .expect("spawn isolated Rayon-global child");
            assert!(
                output.status.success(),
                "isolated retained-team child failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            return;
        }

        rayon::ThreadPoolBuilder::new()
            .num_threads(7)
            .build_global()
            .expect("install mismatched seven-thread global pool");
        assert_eq!(rayon::current_num_threads(), 7);

        let rows = 512_usize;
        let regressors = (0..4)
            .map(|column| {
                (0..rows)
                    .map(|row| (((row >> column) & 1) as f64) * 2.0 - 1.0)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let response = (0..rows)
            .map(|row| {
                regressors
                    .iter()
                    .enumerate()
                    .map(|(column, values)| (column + 1) as f64 * values[row])
                    .sum::<f64>()
                    + (row % 11) as f64 / 17.0
            })
            .collect::<Vec<_>>();
        let prepared_data = prepared(
            &response,
            &regressors,
            &[false; 4],
            None,
            false,
            TestWeights::Unit,
        );
        let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("session frame");

        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for requested in [1_usize, 4] {
                let config = PreparedOlsFitConfig {
                    execution: PreparedOlsExecutionConfig {
                        mode,
                        poolsize: 2,
                        threads: ThreadConfig::from_requested(requested, true)
                            .expect("thread config"),
                    },
                    ..PreparedOlsFitConfig::default()
                };
                let mut session = CertifiedLinearSession::new(
                    &prepared_data,
                    frame,
                    config,
                    LinearSessionGenerations::new(0, 0, 0, 0, 0),
                )
                .expect("retained session");

                begin_retained_moment_worker_observation();
                session.solve_fresh().expect("observed fresh solve");
                assert_retained_moment_team(finish_retained_moment_worker_observation(), requested);

                session
                    .refresh_response(&response, 1)
                    .expect("response generation refresh");
                begin_retained_moment_worker_observation();
                session.solve_frozen().expect("observed frozen solve");
                assert_retained_moment_team(finish_retained_moment_worker_observation(), requested);

                begin_retained_moment_worker_observation();
                let candidate = session
                    .strict_candidate()
                    .expect("observed strict candidate");
                assert!(candidate.candidate().frame().rank_certificate().passed);
                let observed = finish_retained_moment_worker_observation();
                assert_retained_moment_team(observed, requested);
                candidate.commit();
            }
        }
    }

    #[test]
    fn wide_fast_and_low_agree_for_weighted_full_weak_and_deficient_designs() {
        let rows = 512_usize;
        let weights = (0..rows)
            .map(|row| 1.0 + (row % 7) as f64)
            .collect::<Vec<_>>();
        for columns in [65_usize, 128] {
            for gap in [1.0_f64, 1e-4, 0.0] {
                let mut x = (1..=columns)
                    .map(|mask| {
                        (0..rows)
                            .map(|row| {
                                if (row & mask).count_ones() % 2 == 0 {
                                    1.0
                                } else {
                                    -1.0
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                if gap != 1.0 {
                    for row in 0..rows {
                        x[columns - 1][row] = x[0][row] + gap * x[columns - 1][row];
                    }
                }
                let y = (0..rows)
                    .map(|row| {
                        x.iter()
                            .enumerate()
                            .map(|(j, values)| values[row] * (j + 1) as f64 / columns as f64)
                            .sum::<f64>()
                            + if (row & 256).count_ones() % 2 == 0 {
                                0.01
                            } else {
                                -0.01
                            }
                    })
                    .collect::<Vec<_>>();
                let data = prepared(
                    &y,
                    &x,
                    &vec![false; columns],
                    None,
                    false,
                    TestWeights::Analytic(&weights),
                );
                let fast =
                    fit_prepared_ols_coefficients_map(&data, PreparedOlsFitConfig::default())
                        .expect("certified wide fast fit");
                let low_config = PreparedOlsFitConfig {
                    execution: PreparedOlsExecutionConfig {
                        mode: PreparedOlsExecutionMode::Low,
                        poolsize: 8,
                        ..PreparedOlsExecutionConfig::default()
                    },
                    ..PreparedOlsFitConfig::default()
                };
                let low = fit_prepared_ols_coefficients_map(&data, low_config)
                    .expect("certified wide low fit");
                assert_eq!(fast.finite_rank(), columns - usize::from(gap == 0.0));
                assert_eq!(fast.kept_generated_columns(), low.kept_generated_columns());
                assert_eq!(
                    fast.dropped_generated_columns(),
                    low.dropped_generated_columns()
                );
                for (a, b) in fast.coefficients().iter().zip(low.coefficients()) {
                    assert_eq!(a.status, b.status);
                    assert_eq!(a.coefficient_slot, b.coefficient_slot);
                    if let (Some(a), Some(b)) = (a.value, b.value) {
                        assert!((a - b).abs() <= 1e-10 + 1e-7 * a.abs().max(b.abs()));
                    } else {
                        assert_eq!(a.value, b.value);
                    }
                }
                for (a, b) in fast.bread().values().iter().zip(low.bread().values()) {
                    assert!((a - b).abs() <= 1e-10 + 1e-6 * a.abs().max(b.abs()));
                }
                for (a, b) in fast.residuals().iter().zip(low.residuals()) {
                    assert_close(*a, *b, 1e-9);
                }
                for fit in [&fast, &low] {
                    assert!(fit.rank_certificate().passed);
                    assert!(fit.score_certificate().passed);
                }
            }
        }
    }

    #[test]
    fn retained_scratch_parallel_residuals_match_weighted_orthogonal_oracle() {
        // Complete Walsh blocks remain orthogonal under either block-constant
        // weight frame. The unused channel is an independent residual oracle;
        // the duplicate checks rank selection and validation of omitted storage.
        let rows = 2048;
        let channel = |row: usize, mask: usize| {
            if (row & mask).count_ones() % 2 == 0 {
                1.0
            } else {
                -1.0
            }
        };
        for width in [6, 65] {
            let mut regressors = (1..=width)
                .map(|mask| (0..rows).map(|row| channel(row, mask)).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            regressors.push(regressors[0].clone());
            let response = (0..rows)
                .map(|row| {
                    (0..width)
                        .map(|column| (column + 1) as f64 / 67.0 * regressors[column][row])
                        .sum::<f64>()
                        + channel(row, 256) / 16.0
                })
                .collect::<Vec<_>>();
            let initial_weights = (0..rows)
                .map(|row| (1 + row / 512) as f64)
                .collect::<Vec<_>>();
            let next_weights = (0..rows)
                .map(|row| (7 - row / 512) as f64)
                .collect::<Vec<_>>();
            let data = prepared(
                &response,
                &regressors,
                &vec![false; width + 1],
                None,
                false,
                TestWeights::Frequency(&initial_weights),
            );
            let next = prepared(
                &response,
                &regressors,
                &vec![false; width + 1],
                None,
                false,
                TestWeights::Frequency(&next_weights),
            );
            let frame = LinearSolveFrame::from_prepared_ols(&data).unwrap();
            let next_frame = LinearSolveFrame::from_prepared_ols(&next).unwrap();
            for mode in [
                PreparedOlsExecutionMode::Fast,
                PreparedOlsExecutionMode::Low,
            ] {
                // Three workers leave a short last row partition.
                for threads in [1, 3, 8] {
                    let config = PreparedOlsFitConfig {
                        execution: PreparedOlsExecutionConfig {
                            mode,
                            poolsize: 8,
                            threads: ThreadConfig::from_requested(threads, true).unwrap(),
                        },
                        ..PreparedOlsFitConfig::default()
                    };
                    let mut session = CertifiedLinearSession::new(
                        &data,
                        frame,
                        config,
                        LinearSessionGenerations::new(0, 0, 0, 0, 0),
                    )
                    .unwrap();
                    let check = |output: CertifiedLinearSessionOutput<'_>| {
                        let fit = output.frame();
                        assert_eq!(fit.finite_rank(), width);
                        assert_eq!(output.basis().dropped_generated_columns(), &[width]);
                        for coefficient in output.coefficients().filter(|c| c.value.is_some()) {
                            assert_close(
                                coefficient.value.unwrap(),
                                (coefficient.generated_column_index.unwrap() + 1) as f64 / 67.0,
                                2e-12,
                            );
                        }
                        for (row, &residual) in fit.residuals().iter().enumerate() {
                            assert_close(residual, channel(row, 256) / 16.0, 2e-12);
                        }
                        assert!(fit.rank_certificate().passed);
                        assert!(fit.score_certificate().passed);
                    };
                    check(session.solve_fresh().unwrap());
                    session
                        .refresh_weights(
                            next_frame.raw_weights(),
                            next_frame.bread_weight_scale(),
                            1,
                        )
                        .unwrap();
                    let strict = session.strict_candidate().unwrap();
                    check(strict.candidate());
                    strict.commit();
                }
            }
        }
    }

    #[test]
    fn retained_session_matches_one_shot_above_sixty_four_columns_with_exact_deficiency() {
        let rows = 128_usize;
        let mut regressors = (1..=65_usize)
            .map(|mask| {
                (0..rows)
                    .map(|row| {
                        if (row & mask).count_ones() % 2 == 0 {
                            1.0
                        } else {
                            -1.0
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        regressors.push(regressors[0].clone());
        let response = (0..rows)
            .map(|row| {
                regressors[..65]
                    .iter()
                    .enumerate()
                    .map(|(column, values)| (column + 1) as f64 / 67.0 * values[row])
                    .sum::<f64>()
            })
            .collect::<Vec<_>>();
        let prepared_data = prepared(
            &response,
            &regressors,
            &vec![false; regressors.len()],
            None,
            false,
            TestWeights::Unit,
        );
        let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("wide frame");

        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let config = PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode,
                    poolsize: 8,
                    threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
                },
                ..PreparedOlsFitConfig::default()
            };
            let expected = fit_prepared_weighted_linear_once(&prepared_data, frame, config)
                .expect("wide one-shot reference");
            assert_eq!(expected.finite_rank(), 65);
            assert_eq!(expected.dropped_generated_columns(), &[65]);

            let mut session = CertifiedLinearSession::new(
                &prepared_data,
                frame,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 0),
            )
            .expect("wide retained session");
            let fresh = session.solve_fresh().expect("wide retained fresh solve");
            assert_fit_science_equal(&fresh, &expected);
            session
                .refresh_response(&response, 1)
                .expect("wide response refresh");
            let frozen = session.solve_frozen().expect("wide retained frozen solve");
            assert_frozen_matches_fit(&frozen, &expected);
            let strict = session
                .strict_candidate()
                .expect("wide retained strict solve");
            assert!(strict.basis_unchanged());
            assert_fit_science_equal(&strict.candidate(), &expected);
            strict.commit();
        }
    }

    #[test]
    fn retained_session_drops_general_and_scaled_exact_linear_combinations() {
        let first = vec![-5.0_f64, 7.0, 8.0, -3.0, 4.0, 9.0, -6.0, 2.0];
        let second = vec![-1.0_f64, 4.0, -10.0, 6.0, 2.0, -7.0, 5.0, 3.0];
        let combination = first
            .iter()
            .copied()
            .zip(second.iter().copied())
            .map(|(left, right)| 2.0 * left - 3.0 * right)
            .collect::<Vec<_>>();
        let scaled = first
            .iter()
            .copied()
            .map(|value| 6.285_119_033_872_249_f64 * value)
            .collect::<Vec<_>>();
        let response = first
            .iter()
            .copied()
            .zip(second.iter().copied())
            .map(|(left, right)| 2.0 * left - 3.0 * right)
            .collect::<Vec<_>>();
        let regressors = vec![first, second, combination, scaled];
        let prepared_data = prepared(
            &response,
            &regressors,
            &[false; 4],
            None,
            false,
            TestWeights::Unit,
        );
        let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("exact frame");
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let config = PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode,
                    poolsize: 3,
                    threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
                },
                ..PreparedOlsFitConfig::default()
            };
            let expected = fit_prepared_weighted_linear_once(&prepared_data, frame, config)
                .expect("exact-combination one-shot fit");
            assert_eq!(expected.finite_rank(), 2);
            let mut session = CertifiedLinearSession::new(
                &prepared_data,
                frame,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 0),
            )
            .expect("exact-combination retained session");
            assert_fit_science_equal(
                &session.solve_fresh().expect("exact-combination fresh"),
                &expected,
            );
            assert_fit_science_equal(
                &session.strictify().expect("exact-combination strict"),
                &expected,
            );
        }
    }

    #[test]
    fn retained_session_covers_zero_and_single_finite_rank_through_final_handoff() {
        let response = vec![1.0_f64, -2.0, 3.0, 0.5, 4.0, -1.5];
        let regressor = vec![-3.0_f64, -1.0, 0.5, 2.0, 4.0, 6.0];
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for finite_columns in 0..=1_usize {
                let regressors = if finite_columns == 0 {
                    Vec::new()
                } else {
                    vec![regressor.clone()]
                };
                let prepared_data = prepared(
                    &response,
                    &regressors,
                    &vec![false; finite_columns],
                    None,
                    false,
                    TestWeights::Unit,
                );
                let config = PreparedOlsFitConfig {
                    execution: PreparedOlsExecutionConfig {
                        mode,
                        poolsize: 2,
                        threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
                    },
                    ..PreparedOlsFitConfig::default()
                };
                let frame =
                    LinearSolveFrame::from_prepared_ols(&prepared_data).expect("rank edge frame");
                let expected = fit_prepared_weighted_linear_once(&prepared_data, frame, config)
                    .expect("rank edge one-shot");
                assert_eq!(expected.finite_rank(), finite_columns);
                let mut session = CertifiedLinearSession::new(
                    &prepared_data,
                    frame,
                    config,
                    LinearSessionGenerations::new(0, 0, 0, 0, 0),
                )
                .expect("rank edge retained session");
                assert_fit_science_equal(
                    &session.solve_fresh().expect("rank edge fresh"),
                    &expected,
                );
                session
                    .refresh_response(&response, 1)
                    .expect("rank edge response refresh");
                assert_frozen_matches_fit(
                    &session.solve_frozen().expect("rank edge frozen"),
                    &expected,
                );
                assert_fit_science_equal(
                    &session.strictify().expect("rank edge strict"),
                    &expected,
                );
                let products = session
                    .into_current_products(&mut || Ok(()))
                    .expect("rank edge final products");
                assert_eq!(products.frame().finite_rank(), finite_columns);
                assert_eq!(products.finite_within_design().dimension(), finite_columns);
            }
        }
    }

    #[test]
    fn frozen_session_uses_standardized_qr_fallback_for_clear_weak_full_rank() {
        let rows = 64_usize;
        let first = (0..rows)
            .map(|row| row as f64 - (rows - 1) as f64 / 2.0)
            .collect::<Vec<_>>();
        let second = first
            .iter()
            .copied()
            .enumerate()
            .map(|(row, value)| value + 5.0e-4 * if row % 2 == 0 { 1.0 } else { -1.0 })
            .collect::<Vec<_>>();
        let response = first
            .iter()
            .copied()
            .zip(second.iter().copied())
            .map(|(left, right)| 1.25 * left - 0.75 * right)
            .collect::<Vec<_>>();
        let prepared_data = prepared(
            &response,
            &[first, second],
            &[false, false],
            None,
            false,
            TestWeights::Unit,
        );
        let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("weak-rank frame");
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let config = PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode,
                    poolsize: 4,
                    threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
                },
                ..PreparedOlsFitConfig::default()
            };
            let expected = fit_prepared_weighted_linear_once(&prepared_data, frame, config)
                .expect("weak-rank one-shot");
            assert_eq!(expected.finite_rank(), 2);
            let mut session = CertifiedLinearSession::new(
                &prepared_data,
                frame,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 0),
            )
            .expect("weak-rank retained session");
            assert_fit_science_equal(&session.solve_fresh().expect("weak-rank fresh"), &expected);
            session
                .strict_candidate()
                .expect("weak-rank initial strict")
                .commit();
            session
                .refresh_response(&response, 1)
                .expect("weak-rank response refresh");
            begin_frozen_qr_fallback_observation();
            let frozen = session.solve_frozen().expect("weak-rank frozen fallback");
            assert!(
                finish_frozen_qr_fallback_observation(),
                "weak but clearly full-rank frozen basis must use QR fallback"
            );
            assert_frozen_matches_fit(&frozen, &expected);
            assert_fit_science_equal(
                &session.strictify().expect("weak-rank final strict"),
                &expected,
            );
            let products = session
                .into_current_products(&mut || Ok(()))
                .expect("weak-rank final products");
            assert_eq!(products.frame().finite_rank(), 2);
        }
    }

    #[test]
    fn ppml_fresh_norm_selection_preserves_rank_and_explicit_weight_divisors() {
        let rows = 257;
        let groups = (0..rows).map(|r| (r % 7) as f64).collect::<Vec<_>>();
        let weights = (0..rows).map(|r| (1 + r % 5) as f64).collect::<Vec<_>>();
        let x1 = (0..rows)
            .map(|r| 100.0 * groups[r] + (r as f64 * 0.37).sin())
            .collect::<Vec<_>>();
        let x2 = (0..rows)
            .map(|r| (r as f64 * 0.17).cos())
            .collect::<Vec<_>>();
        let x3 = x1.iter().map(|value| 2.0 * value).collect::<Vec<_>>();
        let y = (0..rows)
            .map(|r| 0.7 * x1[r] - 0.3 * x2[r] + groups[r] + (r as f64 * 0.11).sin())
            .collect::<Vec<_>>();
        for kind in [
            TestWeights::Unit,
            TestWeights::Frequency(&weights),
            TestWeights::Probability(&weights),
        ] {
            let data = prepared(
                &y,
                &[x1.clone(), x2.clone(), x3.clone(), groups.clone()],
                &[false; 4],
                Some(&groups),
                true,
                kind,
            );
            let canonical = LinearSolveFrame::from_prepared_ols(&data).unwrap();
            let config = PreparedOlsFitConfig {
                absorb: AbsorbConfig {
                    solver: FeSolverKind::Map,
                    ..AbsorbConfig::default()
                },
                ..PreparedOlsFitConfig::default()
            };
            let mut generic = CertifiedLinearSession::new(
                &data,
                canonical,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 0),
            )
            .unwrap();
            let expected_output = generic.solve_fresh().unwrap();
            let expected = &expected_output;
            assert_eq!(expected.frame().finite_rank(), 2);
            for factor in [1.0, 8.0] {
                if canonical.weights().values.is_none() && factor != 1.0 {
                    continue;
                }
                let mut scale = canonical.bread_weight_scale();
                scale.solver_weight_divisor *= factor;
                scale.reference_weight_multiplier *= factor;
                let frame = LinearSolveFrame::from_parts(
                    canonical.response(),
                    canonical.raw_weights(),
                    scale,
                )
                .unwrap();
                let mut session = CertifiedLinearSession::new_with_fe_design(
                    &data,
                    frame,
                    config,
                    LinearSessionGenerations::new(0, 0, 0, 0, 0),
                    data.fit_owned_fe_design().unwrap(),
                )
                .unwrap();
                assert_eq!(session.bread_weight_scale_for_test(), scale);
                let actual_output = session.solve_fresh().unwrap();
                let actual = &actual_output;
                // The generic and categorical projectors have different raw
                // FE-certificate rounding, also on unchanged production code.
                // Compare scientific products and each current-frame certificate;
                // an explicit divisor need not preserve diagnostic magnitudes.
                assert_eq!(actual.frame().finite_rank(), expected.frame().finite_rank());
                assert_eq!(
                    actual.basis().kept_generated_columns(),
                    expected.basis().kept_generated_columns()
                );
                assert_eq!(
                    actual.basis().dropped_generated_columns(),
                    expected.basis().dropped_generated_columns()
                );
                assert_eq!(actual.coefficients().len(), expected.coefficients().len());
                for (actual, expected) in actual.coefficients().zip(expected.coefficients()) {
                    assert_eq!(actual.coefficient_slot, expected.coefficient_slot);
                    assert_eq!(
                        actual.generated_column_index,
                        expected.generated_column_index
                    );
                    assert_eq!(actual.status, expected.status);
                    match (actual.value, expected.value) {
                        (Some(actual), Some(expected)) => assert_close(actual, expected, 2e-12),
                        (actual, expected) => assert_eq!(actual, expected),
                    }
                }
                assert_eq!(actual.frame().bread().len(), expected.frame().bread().len());
                for (&actual, &expected) in
                    actual.frame().bread().iter().zip(expected.frame().bread())
                {
                    assert_close(actual, expected, 2e-12);
                }
                assert_eq!(
                    actual.frame().residuals().len(),
                    expected.frame().residuals().len()
                );
                for (actual, expected) in actual
                    .frame()
                    .residuals()
                    .iter()
                    .zip(expected.frame().residuals())
                {
                    assert_close(*actual, *expected, 2e-12);
                }
                assert!(actual.frame().rank_certificate().passed);
                assert!(actual.frame().score_certificate().passed);
                assert!(actual.frame().canonical_proof().unwrap().bread.passed);
                assert!(actual.frame().absorption_report().certified());
                assert!(actual.frame().residual_absorption_report().certified());
            }
        }
    }

    #[test]
    fn ppml_fused_weight_refresh_rejects_stale_fe_collinearity_basis() {
        let groups = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
        let x = groups
            .iter()
            .enumerate()
            .map(|(r, g)| g + if r % 2 == 0 { -0.001 } else { 0.001 })
            .collect::<Vec<_>>();
        let y = groups
            .iter()
            .enumerate()
            .map(|(r, g)| g + if r % 2 == 0 { -0.1 } else { 0.1 })
            .collect::<Vec<_>>();
        let initial_weights = vec![1.0; groups.len()];
        let next_weights = (0..groups.len())
            .map(|r| if r % 2 == 0 { 1.0 } else { 1e-8 })
            .collect::<Vec<_>>();
        let initial = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&groups),
            true,
            TestWeights::Analytic(&initial_weights),
        );
        let next = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&groups),
            true,
            TestWeights::Analytic(&next_weights),
        );
        let config = PreparedOlsFitConfig {
            absorb: AbsorbConfig {
                solver: FeSolverKind::Map,
                ..AbsorbConfig::default()
            },
            ..PreparedOlsFitConfig::default()
        };
        let canonical = LinearSolveFrame::from_prepared_ols(&next).unwrap();
        for divisor_factor in [1.0, 8.0] {
            let mut scale = canonical.bread_weight_scale();
            scale.solver_weight_divisor *= divisor_factor;
            scale.reference_weight_multiplier *= divisor_factor;
            let fresh_frame =
                LinearSolveFrame::from_parts(canonical.response(), canonical.raw_weights(), scale)
                    .unwrap();
            let mut fresh = CertifiedLinearSession::new_with_fe_design(
                &initial,
                fresh_frame,
                config,
                LinearSessionGenerations::new(0, 0, 1, 1, 0),
                initial.fit_owned_fe_design().unwrap(),
            )
            .unwrap();
            let fresh_output = fresh.solve_fresh().unwrap();
            let expected = &fresh_output;
            assert_eq!(expected.frame().finite_rank(), 0);
            let mut reused = CertifiedLinearSession::new_with_fe_design(
                &initial,
                LinearSolveFrame::from_prepared_ols(&initial).unwrap(),
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 0),
                initial.fit_owned_fe_design().unwrap(),
            )
            .unwrap();
            assert_eq!(reused.solve_fresh().unwrap().frame().finite_rank(), 1);
            reused.refresh_response(&y, 1).unwrap();
            reused
                .refresh_weights(
                    fresh_frame.raw_weights(),
                    fresh_frame.bread_weight_scale(),
                    1,
                )
                .unwrap();
            // Each two-row group's weighted within sum of squares is
            // w0*w1/(w0+w1) * (x1-x0)^2. The current ratio is below the unchanged
            // FE-collinearity cutoff; using the old numerator instead is above it.
            let within_square = 4.0 * (1e-8 / (1.0 + 1e-8)) * 0.002_f64.powi(2);
            let raw_square = x
                .iter()
                .zip(&next_weights)
                .map(|(x, w)| w * x * x)
                .sum::<f64>();
            assert!(within_square / raw_square < config.fixed_effect_collinearity_tolerance);
            assert!(8e-6 / raw_square > config.fixed_effect_collinearity_tolerance);
            // Strict staging must recertify the prior basis before a transaction
            // can publish. Here that basis became FE-collinear under new weights.
            let error = reused
                .strictify()
                .err()
                .expect("stale prior basis must fail closed");
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert_eq!(
                error.message,
                "strict weighted-linear prior basis could not be recertified"
            );
            assert_eq!(reused.basis_generation_for_test(), 0);
            assert!(reused.output().is_err());
            reused.refresh_basis(1).unwrap();
            let actual = reused.solve_fresh().unwrap();
            assert_fit_science_equal(&actual, expected);
        }
    }

    #[test]
    fn certified_linear_session_fresh_frozen_and_strict_match_across_routes() {
        let g = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
        let x = vec![-2.0, 0.5, -1.0, 2.0, -0.25, 3.0, 1.0, 4.5];
        let z = vec![1.0, -1.0, 0.5, 2.5, -2.0, 1.5, 3.0, -0.5];
        let y = vec![1.2, 4.0, -0.5, 6.1, 2.7, 8.3, 5.5, 11.8];
        let refreshed_y = vec![1.7, 3.6, 0.1, 6.8, 2.1, 9.0, 6.2, 12.4];
        let analytic = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
        let analytic_refresh = vec![1.5, 0.75, 2.0, 1.25, 3.5, 0.6, 1.8, 2.2];
        let frequency = vec![1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 3.0, 2.0];
        let frequency_refresh = vec![2.0, 1.0, 1.0, 3.0, 2.0, 4.0, 1.0, 2.0];
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for has_fe in [false, true] {
                for weight_case in 0..3 {
                    let initial_weights = match weight_case {
                        0 => TestWeights::Unit,
                        1 => TestWeights::Analytic(&analytic),
                        _ => TestWeights::Frequency(&frequency),
                    };
                    let refreshed_weights = match weight_case {
                        0 | 1 => TestWeights::Analytic(&analytic_refresh),
                        _ => TestWeights::Frequency(&frequency_refresh),
                    };
                    let fixed_effect = has_fe.then_some(g.as_slice());
                    let prepared_data = prepared(
                        &y,
                        &[x.clone(), z.clone()],
                        &[false, false],
                        fixed_effect,
                        true,
                        initial_weights,
                    );
                    let config = PreparedOlsFitConfig {
                        absorb: AbsorbConfig {
                            solver: FeSolverKind::Map,
                            ..AbsorbConfig::default()
                        },
                        execution: PreparedOlsExecutionConfig {
                            mode,
                            poolsize: 1,
                            ..PreparedOlsExecutionConfig::default()
                        },
                        ..PreparedOlsFitConfig::default()
                    };
                    let initial_frame =
                        LinearSolveFrame::from_prepared_ols(&prepared_data).expect("initial frame");
                    let initial_expected =
                        fit_prepared_ols_coefficients_map(&prepared_data, config)
                            .expect("initial fit");
                    if !has_fe {
                        assert!(prepared_data.fit_has_absorbed_intercept());
                        assert!(initial_expected.coefficients().iter().any(|coefficient| {
                            coefficient.status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept
                        }));
                    }
                    let mut session = CertifiedLinearSession::new(
                        &prepared_data,
                        initial_frame,
                        config,
                        LinearSessionGenerations::new(0, 0, 0, 0, 0),
                    )
                    .expect("session");
                    let initial = session.solve_fresh().expect("fresh session fit");
                    assert_fit_science_equal(&initial, &initial_expected);
                    assert_retained_fit_has_no_warning_storage(initial.frame());

                    session
                        .refresh_response(&refreshed_y, 1)
                        .expect("response refresh");
                    let response_frame = LinearSolveFrame::from_parts(
                        &refreshed_y,
                        initial_frame.raw_weights(),
                        initial_frame.bread_weight_scale(),
                    )
                    .expect("response frame");
                    let response_expected =
                        fit_prepared_weighted_linear_once(&prepared_data, response_frame, config)
                            .expect("response reference");
                    let response_warm = session.solve_warm().expect("response warm solve");
                    assert_fit_science_equal(&response_warm, &response_expected);
                    let response_frozen = session.solve_frozen().expect("response frozen solve");
                    assert_frozen_matches_fit(&response_frozen, &response_expected);
                    assert_retained_report_has_no_warning_storage(
                        response_frozen.response_absorption_report(),
                    );

                    let refreshed_prepared = prepared(
                        &refreshed_y,
                        &[x.clone(), z.clone()],
                        &[false, false],
                        fixed_effect,
                        true,
                        refreshed_weights,
                    );
                    let refreshed_frame = LinearSolveFrame::from_prepared_ols(&refreshed_prepared)
                        .expect("refreshed weight frame");
                    session
                        .refresh_weights(
                            refreshed_frame.raw_weights(),
                            refreshed_frame.bread_weight_scale(),
                            1,
                        )
                        .expect("weight refresh");
                    let weighted_frame = LinearSolveFrame::from_parts(
                        &refreshed_y,
                        refreshed_frame.raw_weights(),
                        refreshed_frame.bread_weight_scale(),
                    )
                    .expect("weighted reference frame");
                    let weighted_expected =
                        fit_prepared_weighted_linear_once(&prepared_data, weighted_frame, config)
                            .expect("weighted reference");
                    let weighted_frozen = session.solve_frozen().expect("weighted frozen solve");
                    assert_frozen_matches_fit(&weighted_frozen, &weighted_expected);
                    let warmed_capacities = session.workspace_capacities();
                    assert!(
                        session.output().is_err(),
                        "frozen proof cannot publish canonical products"
                    );
                    assert!(warmed_capacities.frozen_residual_elements >= y.len());
                    assert!(warmed_capacities.within_metadata[2] > 0);
                    assert!(warmed_capacities.basis_metadata[0] > 0);

                    session
                        .refresh_response(&refreshed_y, 2)
                        .expect("same-value response generation refresh");
                    let reused_frozen = session.solve_frozen().expect("reused frozen solve");
                    assert_frozen_matches_fit(&reused_frozen, &weighted_expected);
                    assert!(reused_frozen.reuse_report().within_design_reused);
                    assert!(reused_frozen.reuse_report().response_buffer_reused);
                    assert!(reused_frozen.reuse_report().kernel_moment_workspace_reused);
                    assert!(reused_frozen.reuse_report().kernel_solver_workspace_reused);
                    assert!(reused_frozen.reuse_report().fe_projection_workspace_reused);
                    assert!(reused_frozen.reuse_report().steady_state_allocation_free);
                    assert_eq!(session.workspace_capacities(), warmed_capacities);
                    let scratch_identity = session
                        .retained_scratch_identity()
                        .map(std::path::Path::to_path_buf);

                    for iteration in 0..16_u64 {
                        let response = if iteration % 2 == 0 { &y } else { &refreshed_y };
                        let loop_weights = match (weight_case, iteration % 2) {
                            (2, 0) => TestWeights::Frequency(&frequency),
                            (2, _) => TestWeights::Frequency(&frequency_refresh),
                            (_, 0) => TestWeights::Analytic(&analytic),
                            (_, _) => TestWeights::Analytic(&analytic_refresh),
                        };
                        let loop_prepared = prepared(
                            response,
                            &[x.clone(), z.clone()],
                            &[false, false],
                            fixed_effect,
                            true,
                            loop_weights,
                        );
                        let loop_frame = LinearSolveFrame::from_prepared_ols(&loop_prepared)
                            .expect("alternating frame");
                        session
                            .refresh_response(response, 3 + iteration)
                            .expect("alternating response refresh");
                        session
                            .refresh_weights(
                                loop_frame.raw_weights(),
                                loop_frame.bread_weight_scale(),
                                2 + iteration,
                            )
                            .expect("alternating weight refresh");
                        let expected = fit_prepared_weighted_linear_once(
                            &prepared_data,
                            LinearSolveFrame::from_parts(
                                response,
                                loop_frame.raw_weights(),
                                loop_frame.bread_weight_scale(),
                            )
                            .expect("alternating reference frame"),
                            config,
                        )
                        .expect("alternating canonical reference");
                        let frozen = session.solve_frozen().expect("alternating frozen solve");
                        assert_frozen_matches_fit(&frozen, &expected);
                        assert!(frozen.reuse_report().fe_projection_workspace_reused);
                        assert!(frozen.reuse_report().kernel_moment_workspace_reused);
                        assert!(frozen.reuse_report().kernel_solver_workspace_reused);
                        assert!(frozen.reuse_report().steady_state_allocation_free);
                        assert_eq!(session.workspace_capacities(), warmed_capacities);
                        assert_eq!(
                            session.retained_scratch_identity(),
                            scratch_identity.as_deref()
                        );
                    }

                    let strict_transaction = session.strict_candidate().expect("strict candidate");
                    assert!(
                        strict_transaction
                            .workspace_capacities()
                            .frozen_residual_elements
                            >= y.len()
                    );
                    assert!(
                        strict_transaction.rollback_residual_capacity() >= y.len(),
                        "the candidate residual overlaps the rollback residual until commit"
                    );
                    strict_transaction.commit();
                    let strict = session.output().expect("strict session fit");
                    assert_retained_fit_has_no_warning_storage(strict.frame());
                    let final_weights = if weight_case == 2 {
                        TestWeights::Frequency(&frequency_refresh)
                    } else {
                        TestWeights::Analytic(&analytic_refresh)
                    };
                    let final_prepared = prepared(
                        &refreshed_y,
                        &[x.clone(), z.clone()],
                        &[false, false],
                        fixed_effect,
                        true,
                        final_weights,
                    );
                    let final_frame = LinearSolveFrame::from_prepared_ols(&final_prepared)
                        .expect("final strict frame");
                    let final_expected = fit_prepared_weighted_linear_once(
                        &prepared_data,
                        LinearSolveFrame::from_parts(
                            &refreshed_y,
                            final_frame.raw_weights(),
                            final_frame.bread_weight_scale(),
                        )
                        .expect("final strict reference frame"),
                        config,
                    )
                    .expect("final strict reference");
                    assert_fit_science_equal(&strict, &final_expected);
                    let memory = strict.retained_within_memory();
                    assert_eq!(memory.rows, y.len());
                    assert_eq!(memory.columns, strict.frame().finite_rank());
                    assert_eq!(memory.mode, mode);
                    let final_capacities = session.workspace_capacities();
                    assert!(final_capacities.frozen_residual_elements >= y.len());
                    assert_eq!(
                        final_capacities.frozen_residual_elements,
                        y.len(),
                        "canonical frame owns the one residual allocation"
                    );
                    let products = session
                        .into_current_products(&mut || Ok(()))
                        .expect("owned final products");
                    assert_fit_science_equal(&products, &final_expected);
                    let design = products.finite_within_design();
                    assert_eq!(design.nrows(), y.len());
                    assert_eq!(design.dimension(), products.frame().finite_rank());
                    assert_eq!(
                        products.identified_bread().len(),
                        products.frame().finite_rank() * products.frame().finite_rank()
                    );
                    let mut within_row = vec![0.0; design.dimension()];
                    design
                        .fill_row(0, &mut within_row)
                        .expect("owned within row");
                    assert!(within_row.iter().all(|value| value.is_finite()));
                    fn assert_copy_sync<T: Copy + Sync>() {}
                    assert_copy_sync::<CertifiedLinearWithinDesign<'_>>();
                    let mut adapter_row = vec![0.0; design.dimension()];
                    design
                        .fill_row(0, &mut adapter_row)
                        .expect("thread-safe within row");
                    assert_eq!(adapter_row, within_row);
                }
            }
        }
    }

    #[test]
    fn retained_low_residual_report_preserves_actual_work_without_changing_one_shot() {
        let group = [1.0_f64, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
        let x = [-2.0_f64, 0.5, -1.0, 2.0, -0.25, 3.0, 1.0, 4.5];
        let z = [1.0_f64, -1.0, 0.5, 2.5, -2.0, 1.5, 3.0, -0.5];
        let outcome = [1.2_f64, 4.0, -0.5, 6.1, 2.7, 8.3, 5.5, 11.8];
        let prepared_data = prepared(
            &outcome,
            &[x.to_vec(), z.to_vec()],
            &[false, false],
            Some(&group),
            true,
            TestWeights::Unit,
        );
        let config = PreparedOlsFitConfig {
            absorb: AbsorbConfig {
                solver: FeSolverKind::Map,
                ..AbsorbConfig::default()
            },
            execution: PreparedOlsExecutionConfig {
                mode: PreparedOlsExecutionMode::Low,
                poolsize: 1,
                threads: ThreadConfig::from_requested(1, true).expect("serial threads"),
            },
            ..PreparedOlsFitConfig::default()
        };
        let expected = fit_prepared_ols_coefficients_map(&prepared_data, config)
            .expect("one-shot Low reference");
        let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("session frame");
        let mut session = CertifiedLinearSession::new(
            &prepared_data,
            frame,
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 0),
        )
        .expect("retained Low session");
        let retained = session.solve_fresh().expect("retained Low fit");

        // The public fereg Low route destroys and reverses its one-shot
        // Householder scratch, leaving a real one-sweep residual cleanup on
        // this fixture.  The retained PPML row stream must preserve its sole
        // design; direct subtraction is already FE-certified and truthfully
        // reports zero cleanup iterations.  Both are independent, passing
        // certificates at the roundoff floor.
        assert_eq!(expected.residual_absorption_report().iterations, 1);
        assert_eq!(retained.frame().residual_absorption_report().iterations, 0);
        for report in [
            expected.residual_absorption_report(),
            retained.frame().residual_absorption_report(),
        ] {
            assert!(report.certified());
            assert!(report.certificate.relative_two_norm <= 64.0 * f64::EPSILON);
            assert!(report.normal_equation_residual <= 64.0 * f64::EPSILON);
        }
        assert_fit_science_equal(&retained, &expected);
    }

    #[test]
    fn certified_linear_session_generation_invalidation_is_fail_closed() {
        let x = vec![-2.0, -1.0, 0.5, 1.0, 2.0, 4.0];
        let y = vec![1.0, 2.0, 2.5, 4.0, 5.0, 9.0];
        let prepared_data = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            None,
            true,
            TestWeights::Unit,
        );
        let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("frame");
        let mut session = CertifiedLinearSession::new(
            &prepared_data,
            frame,
            PreparedOlsFitConfig::default(),
            LinearSessionGenerations::new(10, 20, 30, 40, 50),
        )
        .expect("session");
        session.solve_fresh().expect("fresh fit");
        assert_eq!(
            session
                .refresh_response(&y, 40)
                .expect_err("response generation must increase")
                .code,
            ErrorCode::InvalidArgument
        );
        session.refresh_basis(51).expect("basis generation");
        assert!(session.output().is_err());
        session.solve_fresh().expect("refreshed basis fit");
        session.invalidate_sample(11).expect("sample invalidation");
        session
            .invalidate_topology(21)
            .expect("topology invalidation");
        assert!(session.requires_rebind());
        let error = match session.strictify() {
            Ok(_) => panic!("invalidated session must fail"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn rejected_uncertified_strict_candidate_restores_exact_basis_transaction() {
        let x = vec![-2.0, -1.0, 0.5, 1.0, 2.0, 4.0];
        let duplicate_x = x.clone();
        let y = vec![1.0, 2.0, 2.5, 4.0, 5.0, 9.0];
        let prepared_data = prepared(
            &y,
            &[x, duplicate_x],
            &[false, false],
            None,
            true,
            TestWeights::Unit,
        );
        let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("frame");
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let config = PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode,
                    poolsize: 1,
                    ..PreparedOlsExecutionConfig::default()
                },
                ..PreparedOlsFitConfig::default()
            };
            let mut session = CertifiedLinearSession::new(
                &prepared_data,
                frame,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 7),
            )
            .expect("session");
            let initial = session.solve_fresh().expect("initial canonical fit");
            assert!(
                initial.frame().canonical_proof().is_some(),
                "session solves must not retain a duplicate N-by-K score cache"
            );
            let original_kept = initial.basis().kept_generated_columns().to_vec();
            let original_dropped = initial.basis().dropped_generated_columns().to_vec();
            assert!(!original_dropped.is_empty());

            let mut strict = session.strict_candidate().expect("speculative strict fit");
            // Model the caller's uncertified strict-output branch: candidate
            // values are inspected but are not accepted as estimator state.
            assert!(strict.candidate().frame().finite_rank() > 0);
            assert!(strict.candidate().frame().canonical_proof().is_some());
            assert!(strict.inject_same_rank_basis_pivot());
            assert_eq!(strict.basis_generation_for_test(), 8);
            strict.rollback();
            assert_eq!(session.basis_generation_for_test(), 7);

            let recovered = session
                .solve_frozen()
                .expect("rollback reconstruction must support the next frozen solve");
            assert!(recovered.certified());
            assert_eq!(recovered.basis().kept_generated_columns(), original_kept);
            let rebuilt = session
                .solve_fresh()
                .expect("canonical solve after rejected strict candidate");
            assert_eq!(rebuilt.basis().kept_generated_columns(), original_kept);
            assert_eq!(
                rebuilt.basis().dropped_generated_columns(),
                original_dropped
            );
        }
    }

    #[test]
    fn strict_rollback_is_allocation_free_and_preserves_retained_storage() {
        let x = vec![-2.0, -1.0, 0.5, 1.0, 2.0, 4.0];
        let y = vec![1.0, 2.0, 2.5, 4.0, 5.0, 9.0];
        let prepared_data = prepared(
            &y,
            &[x.clone(), x],
            &[false, false],
            None,
            true,
            TestWeights::Unit,
        );
        let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("frame");
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let config = PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode,
                    poolsize: 1,
                    ..PreparedOlsExecutionConfig::default()
                },
                ..PreparedOlsFitConfig::default()
            };
            let mut session = CertifiedLinearSession::new(
                &prepared_data,
                frame,
                config,
                LinearSessionGenerations::new(0, 0, 0, 0, 7),
            )
            .expect("session");
            session.solve_fresh().expect("initial canonical fit");
            let original_dense_identity = session.retained_dense_identity();
            let original_scratch_identity = session
                .retained_scratch_identity()
                .map(std::path::Path::to_path_buf);
            let mut strict = session.strict_candidate().expect("speculative strict fit");
            assert!(strict.candidate().frame().finite_rank() > 0);
            assert!(strict.inject_same_rank_basis_pivot());
            let before_rollback = strict.workspace_capacities();
            strict.rollback();
            assert_eq!(
                session.workspace_capacities(),
                before_rollback,
                "rollback must not grow any session workspace"
            );
            assert_eq!(session.retained_dense_identity(), original_dense_identity);
            assert_eq!(
                session.retained_scratch_identity(),
                original_scratch_identity.as_deref()
            );
            assert!(session.output().is_ok());
            assert!(session
                .solve_frozen()
                .expect("rolled-back state must remain usable")
                .certified());
        }
    }

    #[test]
    fn strided_session_cache_compacts_in_place_and_reports_retained_capacity() {
        let x1 = vec![-2.0, -1.0, 0.5, 1.0, 2.0, 4.0];
        let x2 = vec![3.0, -4.0, 1.5, 2.0, -0.5, 5.0];
        let y = vec![1.0, 2.0, 2.5, 4.0, 5.0, 9.0];
        let weights = vec![1.0, 4.0, 2.0, 9.0, 3.0, 16.0];
        let prepared_data = prepared(
            &y,
            &[x1, x2],
            &[false, false],
            None,
            false,
            TestWeights::Analytic(&weights),
        );
        let frame = LinearSolveFrame::from_prepared_ols(&prepared_data).expect("frame");
        let session = CertifiedLinearSession::new(
            &prepared_data,
            frame,
            PreparedOlsFitConfig::default(),
            LinearSessionGenerations::new(0, 0, 0, 0, 0),
        )
        .expect("session");
        assert!(!session.uses_ppml_categorical_map_for_test());
        let rows = prepared_data.fit_sample().n_active();
        let stride = prepared_data.fit_model().columns.len() + 1;
        let logical_elements = rows * stride;
        let expected = vec![1.25, -2.5, 3.75, -4.0, 5.5, 6.25];
        let mut values = vec![0.0; logical_elements];
        for (row, expected) in expected.iter().copied().enumerate() {
            values[row * stride] = row as f64 + 1.0;
            values[row * stride + 1] = 100.0 + row as f64;
            values[row * stride + 2] = expected;
        }
        let source_capacity = values.capacity();
        assert!(source_capacity >= logical_elements);
        let cache = PreparedOlsFastScoreCache::Selected {
            generated_columns: vec![0, 1],
            within: DenseMatrix::from_row_major(rows, stride, values).expect("strided cache"),
            means: vec![0.0, 0.0],
        };
        let (memory, pointer_preserved, compacted) = session
            .compact_fast_cache_memory_for_test(cache, &[1])
            .expect("in-place strided compaction");
        assert!(
            pointer_preserved,
            "strided compaction allocated a second design"
        );
        assert_eq!(memory.rows, rows);
        assert_eq!(memory.columns, 1);
        assert_eq!(compacted, expected);
        assert_eq!(memory.resident_bytes, source_capacity as u128 * 8);
        assert!(
            memory.resident_bytes > (rows * memory.columns * 8) as u128,
            "telemetry must report retained capacity, not truncated logical length"
        );
    }

    #[test]
    fn no_fe_bread_is_the_exact_canonical_inverse_crossproduct() {
        let x = vec![-1.0, 0.0, 1.0];
        let y = x.iter().map(|value| 2.0 + 3.0 * value).collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            None,
            true,
            TestWeights::Unit,
        );
        let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("exact no-FE bread");
        assert_eq!(fit.bread().dimension(), 2);
        assert_eq!(
            fit.bread().coordinates()[0].kind,
            PreparedOlsBreadCoordinateKind::FiniteRegressor
        );
        assert_eq!(
            fit.bread().coordinates()[1].kind,
            PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept
        );
        assert_close(fit.bread().values()[0], 0.5, 1e-12);
        assert_close(fit.bread().values()[1], 0.0, 1e-12);
        assert_close(fit.bread().values()[2], 0.0, 1e-12);
        assert_close(fit.bread().values()[3], 1.0 / 3.0, 1e-12);
        assert_eq!(fit.bread().weight_scale().weight_kind, None);
        assert_close(fit.bread().weight_scale().reference_weight_sum, 3.0, 0.0);
        assert!(fit.bread().certificate().passed);
    }

    #[test]
    fn absorbed_intercept_and_bread_follow_reference_weight_normalization() {
        let g = vec![1.0, 1.0, 2.0, 2.0];
        let x = vec![-1.0, 1.0, -2.0, 2.0];
        let y = x
            .iter()
            .zip(&g)
            .map(|(value, group)| 2.0 * value + if *group == 1.0 { 5.0 } else { -1.0 })
            .collect::<Vec<_>>();
        let raw_weights = vec![1.0, 3.0, 2.0, 4.0];
        let analytic = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&g),
            true,
            TestWeights::Analytic(&raw_weights),
        );
        let analytic =
            fit_prepared_ols_coefficients_map(&analytic, PreparedOlsFitConfig::default())
                .expect("analytic normalized intercept and bread");
        assert_eq!(analytic.finite_rank(), 1);
        assert_close(analytic.coefficients()[0].value.expect("slope"), 2.0, 1e-11);
        assert_eq!(
            analytic.coefficients()[1].status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert_close(
            analytic.coefficients()[1].value.expect("intercept"),
            1.4,
            1e-11,
        );
        assert_close(analytic.bread().values()[0], 15.0 / 146.0, 1e-11);
        assert_close(analytic.bread().values()[1], -9.0 / 146.0, 1e-11);
        assert_close(analytic.bread().values()[2], -9.0 / 146.0, 1e-11);
        assert_close(analytic.bread().values()[3], 419.0 / 1460.0, 1e-11);
        let analytic_scale = analytic.bread().weight_scale();
        assert_eq!(analytic_scale.weight_kind, Some(WeightKind::Analytic));
        assert_close(analytic_scale.solver_weight_divisor, 4.0, 0.0);
        assert_close(analytic_scale.reference_weight_multiplier, 1.6, 1e-12);
        assert_close(analytic_scale.reference_weight_sum, 4.0, 0.0);

        let scaled_weights = raw_weights
            .iter()
            .map(|weight| weight * 1e200)
            .collect::<Vec<_>>();
        let scaled = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&g),
            true,
            TestWeights::Analytic(&scaled_weights),
        );
        let scaled = fit_prepared_ols_coefficients_map(&scaled, PreparedOlsFitConfig::default())
            .expect("rescaled analytic normalized intercept and bread");
        for (left, right) in analytic
            .bread()
            .values()
            .iter()
            .zip(scaled.bread().values())
        {
            assert_close(*left, *right, 2e-11);
        }

        let probability = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&g),
            true,
            TestWeights::Probability(&raw_weights),
        );
        let probability =
            fit_prepared_ols_coefficients_map(&probability, PreparedOlsFitConfig::default())
                .expect("probability normalized intercept and bread");
        assert_eq!(
            probability.bread().weight_scale().weight_kind,
            Some(WeightKind::Probability)
        );
        for (analytic, probability) in analytic
            .bread()
            .values()
            .iter()
            .zip(probability.bread().values())
        {
            assert_close(*analytic, *probability, 2e-11);
        }

        let frequencies = vec![1.0, 3.0, 2.0, 4.0];
        let frequency = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&g),
            true,
            TestWeights::Frequency(&frequencies),
        );
        let frequency =
            fit_prepared_ols_coefficients_map(&frequency, PreparedOlsFitConfig::default())
                .expect("frequency normalized intercept and bread");
        assert_close(frequency.bread().values()[0], 3.0 / 73.0, 1e-11);
        assert_close(frequency.bread().values()[1], -9.0 / 365.0, 1e-11);
        assert_close(frequency.bread().values()[3], 419.0 / 3650.0, 1e-11);
        assert_close(
            frequency.bread().weight_scale().reference_weight_sum,
            10.0,
            0.0,
        );
        let doubled = frequencies
            .iter()
            .map(|value| 2.0 * value)
            .collect::<Vec<_>>();
        let doubled = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&g),
            true,
            TestWeights::Frequency(&doubled),
        );
        let doubled = fit_prepared_ols_coefficients_map(&doubled, PreparedOlsFitConfig::default())
            .expect("doubled frequency bread");
        for (baseline, doubled) in frequency
            .bread()
            .values()
            .iter()
            .zip(doubled.bread().values())
        {
            assert_close(*doubled, *baseline / 2.0, 2e-11);
        }
    }

    #[test]
    fn normalized_intercept_does_not_add_finite_or_absorbed_rank() {
        let g = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0];
        let y = vec![2.0, 4.0, -1.0, 1.0, 7.0, 9.0];
        let prepared = prepared(&y, &[], &[], Some(&g), true, TestWeights::Unit);
        let absorbed = super::super::compute_prepared_ols_absorbed_dof(
            &prepared,
            super::super::PreparedOlsAbsorbedDofPolicy::Pairwise,
        )
        .expect("absorbed rank");
        let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("FE-only fit");
        assert_eq!(fit.finite_rank(), 0);
        assert_eq!(absorbed.absorbed_rank, 3);
        assert_eq!(fit.bread().dimension(), 1);
        assert_close(fit.bread().values()[0], 1.0 / 6.0, 1e-12);
        assert_eq!(
            fit.coefficients()[0].status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert_close(
            fit.coefficients()[0].value.expect("normalized intercept"),
            3.0 + 2.0 / 3.0,
            1e-12,
        );
        assert!(fit.kept_generated_columns().is_empty());
        assert!(fit.dropped_generated_columns().is_empty());
        assert_eq!(
            fit.bread().coordinates()[0].generated_column_index,
            fit.coefficients()[0]
                .generated_column_index
                .expect("reported intercept generated column")
        );
    }

    #[test]
    fn rank_zero_without_finite_columns_is_certified() {
        let y = vec![1.0, -2.0, 3.0, 0.5];
        let prepared = prepared(&y, &[], &[], None, false, TestWeights::Unit);
        let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("rank-zero fit");
        assert_eq!(fit.finite_rank(), 0);
        assert!(fit.coefficients().is_empty());
        assert!(fit.kept_generated_columns().is_empty());
        assert!(fit.dropped_generated_columns().is_empty());
        let mean = y.iter().sum::<f64>() / y.len() as f64;
        assert_eq!(
            fit.residuals(),
            y.iter().map(|value| value - mean).collect::<Vec<_>>()
        );
        assert_eq!(fit.score_certificate().moments, 0);
    }

    #[test]
    fn rank_zero_fast_path_matches_low_memory_weighted_fe_fit() {
        let g = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 3.0];
        let y = vec![2.0, 4.0, -1.0, 1.0, 7.0, 9.0, 8.0];
        let weights = vec![0.5, 2.0, 1.5, 0.75, 3.0, 1.0, 2.5];
        let prepared = prepared(
            &y,
            &[],
            &[],
            Some(&g),
            true,
            TestWeights::Analytic(&weights),
        );
        let fast = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("fast weighted FE-only fit");
        let low = fit_prepared_ols_coefficients_map(
            &prepared,
            PreparedOlsFitConfig {
                execution: PreparedOlsExecutionConfig {
                    mode: PreparedOlsExecutionMode::Low,
                    poolsize: 1,
                    ..PreparedOlsExecutionConfig::default()
                },
                ..PreparedOlsFitConfig::default()
            },
        )
        .expect("low-memory weighted FE-only fit");
        assert_eq!(fast.finite_rank(), 0);
        assert_eq!(fast.coefficients(), low.coefficients());
        for (actual, expected) in fast.residuals().iter().zip(low.residuals()) {
            assert_close(*actual, *expected, 2e-12);
        }
        for (actual, expected) in fast.bread().values().iter().zip(low.bread().values()) {
            assert_close(*actual, *expected, 2e-12);
        }
        assert!(fast.rank_certificate().passed);
        assert!(fast.score_certificate().passed);
    }

    #[test]
    fn strided_score_cache_matches_packed_cache_across_weight_semantics() {
        let group = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
        let x1 = vec![-2.0, 0.5, 1.0, 3.0, -1.5, 2.0, 0.25, 4.0];
        let x2 = vec![1.0, -3.0, 0.75, 2.5, 4.0, -0.5, 3.0, 1.5];
        let weights = vec![1.0, 2.0, 3.0, 4.0, 2.0, 1.0, 3.0, 2.0];
        let y = x1
            .iter()
            .zip(&x2)
            .zip(&group)
            .map(|((left, right), group)| {
                1.75 * left - 0.4 * right + if *group <= 2.0 { 2.0 } else { -1.0 }
            })
            .collect::<Vec<_>>();
        for test_weights in [
            TestWeights::Unit,
            TestWeights::Analytic(&weights),
            TestWeights::Probability(&weights),
            TestWeights::Frequency(&weights),
        ] {
            let prepared = prepared(
                &y,
                &[x1.clone(), x2.clone()],
                &[false, false],
                Some(&group),
                true,
                test_weights,
            );
            assert_packed_and_strided_score_caches_match(&prepared);
        }
        let no_intercept = prepared(
            &y,
            &[x1, x2],
            &[false, false],
            None,
            false,
            TestWeights::Unit,
        );
        assert_packed_and_strided_score_caches_match(&no_intercept);
    }

    #[test]
    fn packed_score_handoff_selects_arbitrary_basis_without_reallocating_rows() {
        let make_cache = || {
            let mut values = Vec::with_capacity(40);
            values.extend([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
            PreparedOlsFastScoreCache::Packed {
                generated_columns: vec![0, 2, 5, 9],
                within: DenseMatrix::from_row_major(2, 4, values).expect("matrix"),
                means: vec![-0.0, 0.25, -2.0, 3.0],
            }
        };
        let original = make_cache();
        let PreparedOlsFastScoreCache::Packed { within, .. } = &original else {
            unreachable!();
        };
        let pointer = within.as_slice().as_ptr();
        let capacity = within.storage_capacity();
        let mut reservation = InjectedReservation::default();
        let selected = original
            .into_kept(&[9, 0, 5], &mut reservation)
            .expect("selected packed bank");
        let PreparedOlsFastScoreCache::Packed {
            generated_columns,
            within,
            means,
        } = selected
        else {
            unreachable!();
        };
        assert_eq!(generated_columns, [9, 0, 5]);
        assert_eq!(within.as_slice(), [4.0, 1.0, 3.0, 8.0, 5.0, 7.0]);
        assert_eq!(within.as_slice().as_ptr(), pointer);
        assert_eq!(within.storage_capacity(), capacity);
        assert_eq!(
            means.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            [
                3.0_f64.to_bits(),
                (-0.0_f64).to_bits(),
                (-2.0_f64).to_bits()
            ]
        );
        assert_eq!(
            reservation.records,
            [(3, "unable to reserve accepted packed score selection")]
        );
        let mut no_alloc = InjectedReservation {
            fail_at: Some(1),
            ..Default::default()
        };
        make_cache()
            .into_kept(&[0, 2, 5, 9], &mut no_alloc)
            .expect("unchanged bank");
        assert_eq!(no_alloc.calls, 0);
        for kept in [&[2, 2][..], &[2, 7][..]] {
            assert_eq!(
                make_cache()
                    .into_kept(kept, &mut SystemFitReservation)
                    .expect_err("invalid selection")
                    .code,
                ErrorCode::InvalidData
            );
        }
        for mut refusal in [
            InjectedReservation {
                fail_at: Some(1),
                ..Default::default()
            },
            InjectedReservation {
                undersize: true,
                ..Default::default()
            },
        ] {
            assert_eq!(
                make_cache()
                    .into_kept(&[9, 0, 5], &mut refusal)
                    .expect_err("selection allocation refusal")
                    .code,
                ErrorCode::Allocation
            );
        }
    }

    #[test]
    fn allocation_capacity_and_score_failures_are_fatal() {
        let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
        let y = vec![1.2, -0.5, 0.4, 3.1, 4.0, 7.2];
        let prepared = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            None,
            true,
            TestWeights::Unit,
        );
        let mut successful = InjectedReservation::default();
        fit_with_controls(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut successful,
            false,
            false,
        )
        .expect("control fit");
        assert!(successful.calls >= 9);
        for fail_at in 1..=successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = fit_with_controls(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut injected,
                false,
                false,
            )
            .expect_err("injected allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "failure {fail_at}");
        }
        let mut cached_successful = InjectedReservation::default();
        let cached = fit_with_controls_internal_mode(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut cached_successful,
            FitPerturbations::default(),
            true,
            MAX_FAST_SCORE_CACHE_BYTES,
        )
        .expect("fast inference-cache control fit");
        assert!(cached.fast_score_cache.is_some());
        assert!(cached_successful.calls >= 9);
        for fail_at in 1..=cached_successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = fit_with_controls_internal_mode(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut injected,
                FitPerturbations::default(),
                true,
                MAX_FAST_SCORE_CACHE_BYTES,
            )
            .expect_err("injected fast inference-cache allocation failure");
            assert_eq!(
                error.code,
                ErrorCode::Allocation,
                "cached failure {fail_at}"
            );
        }
        let mut strided_successful = InjectedReservation::default();
        let strided = fit_with_controls_internal_mode(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut strided_successful,
            FitPerturbations::default(),
            true,
            0,
        )
        .expect("strided inference-cache control fit");
        assert!(matches!(
            strided.fast_score_cache,
            Some(PreparedOlsFastScoreCache::Selected { .. })
        ));
        for fail_at in 1..=strided_successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = fit_with_controls_internal_mode(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut injected,
                FitPerturbations::default(),
                true,
                0,
            )
            .expect_err("injected strided inference-cache allocation failure");
            assert_eq!(
                error.code,
                ErrorCode::Allocation,
                "strided failure {fail_at}"
            );
        }
        let mut undersized = InjectedReservation {
            undersize: true,
            ..InjectedReservation::default()
        };
        assert_eq!(
            fit_with_controls(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut undersized,
                false,
                false,
            )
            .expect_err("undersized reservation")
            .code,
            ErrorCode::Allocation
        );
        let mut reservation = InjectedReservation::default();
        assert_eq!(
            fit_with_controls(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut reservation,
                true,
                false,
            )
            .expect_err("perturbed coefficient must fail score certificate")
            .code,
            ErrorCode::InvalidData
        );
        let mut reservation = InjectedReservation::default();
        assert_eq!(
            fit_with_controls_internal(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut reservation,
                FitPerturbations {
                    finite_bread: true,
                    ..FitPerturbations::default()
                },
            )
            .expect_err("perturbed bread must fail its inverse certificate")
            .code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            checked_square(
                usize::MAX,
                "injected prepared OLS square-dimension overflow"
            )
            .expect_err("square dimensions must use checked arithmetic")
            .code,
            ErrorCode::Allocation
        );
    }

    #[test]
    fn low_guarded_fit_allocation_and_certificate_failures_are_fatal() {
        let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
        let y = vec![1.2, -0.5, 0.4, 3.1, 4.0, 7.2];
        let data = prepared(&y, &[x], &[false], None, true, TestWeights::Unit);
        let config = PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode: PreparedOlsExecutionMode::Low,
                poolsize: 1,
                ..PreparedOlsExecutionConfig::default()
            },
            ..PreparedOlsFitConfig::default()
        };
        let mut successful = InjectedReservation::default();
        fit_with_controls(&data, config, &mut successful, false, false)
            .expect("low guarded control");
        for fail_at in 1..=successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = fit_with_controls(&data, config, &mut injected, false, false)
                .expect_err("low guarded allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "failure {fail_at}");
        }
        for perturbations in [
            FitPerturbations {
                first_coefficient: true,
                ..FitPerturbations::default()
            },
            FitPerturbations {
                finite_bread: true,
                ..FitPerturbations::default()
            },
        ] {
            let error =
                fit_with_controls_internal(&data, config, &mut SystemFitReservation, perturbations)
                    .expect_err("low guarded corruption must fail certification");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn reported_intercept_certificate_fails_closed_before_fit_publication() {
        let g = (0..64).map(|row| (row / 8) as f64).collect::<Vec<_>>();
        let x = (0..64)
            .map(|row| (row % 8) as f64 - 3.5)
            .collect::<Vec<_>>();
        let y = x
            .iter()
            .zip(&g)
            .map(|(value, group)| 2.0 * value + 0.75 * group - 1.0)
            .collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&g),
            true,
            TestWeights::Unit,
        );
        let mut successful = InjectedReservation::default();
        fit_with_controls_internal(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut successful,
            FitPerturbations::default(),
        )
        .expect("absorbed-intercept allocation control");
        let covariance_bound = checked_square(
            prepared.model.columns.len(),
            "test prepared OLS covariance bound overflow",
        )
        .expect("small covariance bound");
        assert!(prepared.sample.n_active() > covariance_bound);
        let covariance_records = successful
            .records
            .iter()
            .filter(|(_, label)| {
                label.contains("Gram")
                    || label.contains("bread")
                    || label.contains("inverse R")
                    || label.contains("reported-intercept")
            })
            .collect::<Vec<_>>();
        assert!(!covariance_records.is_empty());
        assert!(covariance_records
            .iter()
            .all(|(elements, _)| *elements <= covariance_bound));
        for (label, elements) in [
            ("unable to reserve prepared OLS reported bread", 4),
            ("unable to reserve prepared OLS reported-intercept means", 1),
            (
                "unable to reserve prepared OLS reported-intercept bread side",
                1,
            ),
        ] {
            assert!(successful.records.contains(&(elements, label)));
        }
        for fail_at in 1..=successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = fit_with_controls_internal(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut injected,
                FitPerturbations::default(),
            )
            .expect_err("absorbed-intercept injected allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "failure {fail_at}");
        }

        let mut reservation = InjectedReservation::default();
        assert_eq!(
            fit_with_controls_internal(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut reservation,
                FitPerturbations {
                    reported_intercept: true,
                    ..FitPerturbations::default()
                },
            )
            .expect_err("perturbed reported intercept must not publish a partial fit")
            .code,
            ErrorCode::InvalidData
        );

        let mut reservation = InjectedReservation::default();
        assert_eq!(
            fit_with_controls_internal(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut reservation,
                FitPerturbations {
                    reported_bread_extension: true,
                    ..FitPerturbations::default()
                },
            )
            .expect_err("perturbed reported-bread extension must fail its inverse certificate")
            .code,
            ErrorCode::InvalidData
        );
    }

    #[test]
    fn fixed_effect_only_outcome_scale_cannot_mask_score_failure() {
        let g = vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
        let x = vec![-2.0, -0.5, 1.0, 2.5, -1.5, 0.0, 0.5, 1.0];
        let y = x
            .iter()
            .zip(&g)
            .map(|(x, group)| {
                let fixed_effect = if *group == 1.0 { 1e10 } else { -1e10 };
                fixed_effect + 2.5 * x
            })
            .collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&g),
            false,
            TestWeights::Unit,
        );
        let mut control_reservation = InjectedReservation::default();
        let control = fit_with_controls(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut control_reservation,
            false,
            false,
        )
        .expect("unperturbed large-FE-outcome fit");
        assert_close(
            control.coefficients()[0].value.expect("control slope"),
            2.5,
            2e-6,
        );
        let mut reservation = InjectedReservation::default();
        assert_eq!(
            fit_with_controls(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut reservation,
                true,
                false,
            )
            .expect_err("FE-only outcome scale must not hide a perturbed coefficient")
            .code,
            ErrorCode::InvalidData
        );
    }

    #[test]
    fn fixed_effect_only_regressor_scale_cannot_mask_score_failure() {
        let g = vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
        let within_x = [-2.0, -0.5, 1.0, 2.5, -1.5, 0.0, 0.5, 1.0];
        let x = within_x
            .iter()
            .zip(&g)
            .map(|(within, group)| {
                // Large enough to exercise scale separation, but outside the
                // pinned squared 1e-9 FE-collinearity cutoff so this remains
                // an identified-coordinate score-certificate mutation test.
                let fixed_effect = if *group == 1.0 { 1e4 } else { -1e4 };
                fixed_effect + within
            })
            .collect::<Vec<_>>();
        let y = within_x
            .iter()
            .map(|within| 100.0 * within)
            .collect::<Vec<_>>();
        let prepared = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            Some(&g),
            false,
            TestWeights::Unit,
        );
        let mut control_reservation = InjectedReservation::default();
        let control = fit_with_controls(
            &prepared,
            PreparedOlsFitConfig::default(),
            &mut control_reservation,
            false,
            false,
        )
        .expect("unperturbed large-FE-regressor fit");
        assert_close(
            control.coefficients()[0].value.expect("control slope"),
            100.0,
            2e-8,
        );
        let mut reservation = InjectedReservation::default();
        assert_eq!(
            fit_with_controls(
                &prepared,
                PreparedOlsFitConfig::default(),
                &mut reservation,
                true,
                false,
            )
            .expect_err("FE-only regressor scale must not hide a perturbed coefficient")
            .code,
            ErrorCode::InvalidData
        );
    }

    #[test]
    fn zero_within_norm_column_still_checks_its_raw_score_moment() {
        let g = vec![1.0, 1.0, 1.0, 2.0, 2.0, 2.0];
        let within_y = [-1.0, 0.0, 1.0, -2.0, 0.5, 1.5];
        let y = within_y
            .iter()
            .zip(&g)
            .map(|(within, group)| 10.0 * group + within)
            .collect::<Vec<_>>();
        let fe_prepared = prepared(&y, &[], &[], Some(&g), true, TestWeights::Unit);
        let mut control_reservation = InjectedReservation::default();
        let control = fit_with_controls(
            &fe_prepared,
            PreparedOlsFitConfig::default(),
            &mut control_reservation,
            false,
            false,
        )
        .expect("unperturbed deferred-intercept fit");
        assert_eq!(control.score_certificate().moments, 1);
        assert_eq!(
            control.coefficients()[0].status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert_close(
            control.coefficients()[0]
                .value
                .expect("normalized intercept"),
            weighted_mean_slice(&y, normalized_weights(&fe_prepared).expect("weights"))
                .expect("mean"),
            1e-10,
        );

        // A structurally absorbed intercept can acquire a tiny nonzero within
        // norm from iterative MAP roundoff. Its score remains a raw intercept
        // moment, not a moment normalized by that numerical noise.
        let weights = normalized_weights(&fe_prepared).expect("weights");
        let rhs = fe_prepared
            .fit_build_rhs_with_response(&fe_prepared.fit_sample().outcome)
            .expect("rhs");
        let reference_norm =
            weighted_matrix_column_norm(&rhs, 1, weights).expect("reference intercept norm");
        let mut roundoff_residuals = control.residuals().to_vec();
        for residual in &mut roundoff_residuals {
            *residual += 1e-14;
        }
        let roundoff = certify_scores(
            &fe_prepared,
            &roundoff_residuals,
            weights,
            &[reference_norm],
            &[reference_norm * 1e-14],
            FiniteScoreDraft::default(),
            &[PreparedOlsFitStatus::NormalizedAbsorbedIntercept],
            1.0,
            PreparedOlsFitConfig::default().score_tolerance,
        )
        .expect("roundoff score certificate");
        assert!(roundoff.passed);
        assert!(roundoff.relative_infinity_norm < 1e-12);

        // Exact-zero outcomes and residuals have a mathematically exact zero
        // score even though no positive response normalization exists. A
        // nonzero finite-coordinate score under the same response must remain
        // fatal instead of being hidden by a numerical floor.
        let zero_outcome = vec![0.0; 6];
        let varying = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
        let zero_response = prepared(
            &zero_outcome,
            &[varying],
            &[false],
            None,
            false,
            TestWeights::Unit,
        );
        let zero_weights = normalized_weights(&zero_response).expect("zero-response weights");
        let exact_zero = certify_scores(
            &zero_response,
            &zero_outcome,
            zero_weights,
            &[1.0],
            &[1.0],
            FiniteScoreDraft {
                infinity_scale: 0.0,
                square_sum_scale: 0.0,
                moments: 1,
            },
            &[PreparedOlsFitStatus::Kept],
            0.0,
            PreparedOlsFitConfig::default().score_tolerance,
        )
        .expect("exact-zero response score certificate");
        assert!(exact_zero.passed);
        assert_eq!(exact_zero.relative_infinity_norm, 0.0);
        assert_eq!(exact_zero.relative_two_norm, 0.0);

        let nonzero_error = certify_scores(
            &zero_response,
            &zero_outcome,
            zero_weights,
            &[1.0],
            &[1.0],
            FiniteScoreDraft {
                infinity_scale: f64::MIN_POSITIVE,
                square_sum_scale: 0.0,
                moments: 1,
            },
            &[PreparedOlsFitStatus::Kept],
            0.0,
            PreparedOlsFitConfig::default().score_tolerance,
        )
        .expect_err("nonzero score under an exact-zero response must fail");
        assert_eq!(nonzero_error.code, ErrorCode::InvalidData);

        let mut reservation = InjectedReservation::default();
        assert_eq!(
            fit_with_controls(
                &fe_prepared,
                PreparedOlsFitConfig::default(),
                &mut reservation,
                false,
                true,
            )
            .expect_err("a deferred intercept must retain its raw score moment")
            .code,
            ErrorCode::InvalidData
        );
    }

    #[test]
    fn empty_sample_and_invalid_policy_fail_closed() {
        let empty = prepared(&[], &[], &[], None, false, TestWeights::Unit);
        assert_eq!(
            fit_prepared_ols_coefficients_map(&empty, PreparedOlsFitConfig::default())
                .expect_err("empty fit")
                .code,
            ErrorCode::InsufficientObservations
        );
        let x = vec![0.0, 1.0, 2.0];
        let y = vec![1.0, 2.0, 3.0];
        let prepared = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            None,
            true,
            TestWeights::Unit,
        );
        let default = PreparedOlsFitConfig::default();
        let invalid = PreparedOlsFitConfig {
            score_tolerance: 1.0,
            ..default
        };
        assert_eq!(
            fit_prepared_ols_coefficients_map(&prepared, invalid)
                .expect_err("invalid policy")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn corrupted_structural_intercept_metadata_fails_before_map_and_dof() {
        let x = vec![0.0, 1.0, 2.0];
        let y = vec![1.0, 2.0, 3.0];
        let mut fit_corrupted = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            None,
            true,
            TestWeights::Unit,
        );
        assert!(fit_corrupted.structural_intercept);
        assert!(fit_corrupted.report_intercept);
        fit_corrupted.structural_intercept = false;

        let mut reservation = InjectedReservation::default();
        assert_eq!(
            fit_with_controls(
                &fit_corrupted,
                PreparedOlsFitConfig::default(),
                &mut reservation,
                false,
                false,
            )
            .expect_err("corrupted structural-intercept metadata must fail")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            reservation.calls, 0,
            "structural-intercept corruption must fail before MAP or fit allocation"
        );

        // A hidden no-absorb constant is mandatory for the ordinary prepared
        // path.  Pure slope-only categorical models use their separate
        // topology-aware path and legitimately carry neither structural nor
        // reported intercept metadata.
        let mut dof_corrupted = prepared(
            &y,
            std::slice::from_ref(&x),
            &[false],
            None,
            false,
            TestWeights::Unit,
        );
        assert!(dof_corrupted.structural_intercept);
        assert!(!dof_corrupted.report_intercept);
        dof_corrupted.structural_intercept = false;
        assert_eq!(
            super::super::compute_prepared_ols_absorbed_dof(
                &dof_corrupted,
                super::super::PreparedOlsAbsorbedDofPolicy::Pairwise,
            )
            .expect_err("corrupted structural-intercept metadata must fail DoF")
            .code,
            ErrorCode::InvalidArgument
        );
    }
}
