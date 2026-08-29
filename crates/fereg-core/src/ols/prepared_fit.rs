use crate::data::{DenseMatrix, PreparedWeights, WeightKind};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    AbsorbConfig, BorrowedMapWeights, CertifiedBorrowedMap, CmgAbsorber, CmgConfig,
    CmgTopologyProfile, FeCertificate, FeCoefficientBatch, FeSolverKind, SolveReport,
    SolveStopReason,
};
use crate::metadata::CoefficientSlot;
use crate::options::ThreadConfig;
use crate::parallel::ThreadTeam;
use crate::prepared_categorical_data::PreparedCategoricalOlsData;
use crate::prepared_fit_view::PreparedOlsFitView;
use rayon::prelude::*;

use super::prepared_scratch::PreparedOlsScratch;
use super::{PreparedOlsCoefficientStatus, PreparedOlsColumnKind, PreparedOlsData};

/// Numerical policy for the guarded prepared-data coefficient checkpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsFitConfig {
    pub absorb: AbsorbConfig,
    /// Multiplier on `eps * max(N, K, 1)` after finite columns are scaled by
    /// their pre-absorption weighted norms.
    pub rank_tolerance_multiplier: f64,
    /// Squared relative weighted-norm cutoff for columns annihilated by fixed
    /// effects.  This matches pinned reghdfe's `cross(data,data) / norm2`
    /// collinearity statistic rather than applying its tolerance to the norm.
    pub fixed_effect_collinearity_tolerance: f64,
    /// Maximum recomputed normalized finite-score moment.
    pub score_tolerance: f64,
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
            fixed_effect_collinearity_tolerance: 1e-9,
            score_tolerance: 5e-10,
            bread_tolerance: 5e-10,
            execution: PreparedOlsExecutionConfig::default(),
        }
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

#[derive(Debug, PartialEq)]
pub(super) struct PreparedOlsFastScoreCache {
    generated_columns: Vec<usize>,
    within: DenseMatrix,
    means: Vec<f64>,
}

impl PreparedOlsFastScoreCache {
    fn mean(&self, generated: usize) -> Option<f64> {
        self.generated_columns
            .iter()
            .position(|candidate| *candidate == generated)
            .and_then(|position| self.means.get(position).copied())
    }
}

impl CertifiedPreparedOlsFit {
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

    pub(super) fn take_fast_score_cache(
        &mut self,
        generated_columns: &[usize],
    ) -> Option<(DenseMatrix, Vec<f64>)> {
        let cache = self.fast_score_cache.take()?;
        if cache.generated_columns == generated_columns
            && cache.within.ncols() == generated_columns.len()
            && cache.means.len() == generated_columns.len()
        {
            Some((cache.within, cache.means))
        } else {
            None
        }
    }

    pub(super) fn record_additional_scratch_peak(&mut self, bytes: u64) {
        self.execution.scratch_bytes = self.execution.scratch_bytes.max(bytes);
    }
}

trait FitReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemFitReservation;

const PARALLEL_QR_MIN_ROWS: usize = 65_536;
const PARALLEL_REDUCTION_CHUNK_ROWS: usize = 16_384;
const FUSED_MOMENT_MAX_COLUMNS: usize = 64;
const FUSED_MOMENT_MAX_BLOCKS: usize = 64;

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

#[derive(Debug, Clone, Copy, Default)]
struct FiniteScoreDraft {
    /// Largest finite-coordinate score after dividing out the within-X norm.
    /// The final response scale is applied only after the returned residual is
    /// independently certified against the FE span.
    infinity_scale: f64,
    square_sum_scale: f64,
    moments: usize,
}

#[derive(Debug)]
struct FiniteBreadDraft {
    generated_columns: Vec<usize>,
    values: Vec<f64>,
    reference_crossproduct: Vec<f64>,
    weight_scale: PreparedOlsBreadWeightScale,
    left_inverse_relative_infinity_norm: f64,
    right_inverse_relative_infinity_norm: f64,
}

struct FiniteBreadInputs<'a> {
    qr: &'a DenseMatrix,
    pivot_columns: &'a [usize],
    canonical_candidates: &'a [usize],
    standardized_gram: &'a [f64],
    rank: usize,
    reference_norms: &'a [f64],
    weights: NormalizedWeights<'a>,
}

#[derive(Debug)]
struct QrFit {
    generated_coefficients: Vec<Option<f64>>,
    generated_statuses: Vec<PreparedOlsFitStatus>,
    kept: Vec<usize>,
    dropped: Vec<usize>,
    rank: usize,
    certificate: PreparedOlsRankCertificate,
    finite_score: FiniteScoreDraft,
    bread: Option<FiniteBreadDraft>,
    within_residuals: Option<Vec<f64>>,
}

#[derive(Debug)]
struct RawMomentCache {
    means: Vec<f64>,
    scales: Vec<f64>,
    norms: Vec<f64>,
    weight_sum: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct FitPerturbations {
    first_coefficient: bool,
    reconstructed_within_residual_constant: bool,
    final_residual_constant: bool,
    finite_bread: bool,
    reported_intercept: bool,
    reported_bread_extension: bool,
}

/// Fit the currently supported prepared OLS/WLS finite coefficients through
/// the strict borrowed-intercept MAP path and a certified scaled finite solve.
pub fn fit_prepared_ols_coefficients_map(
    prepared: &PreparedOlsData,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    fit_prepared_ols_coefficients_view_map(prepared, config)
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
            ),
        }
    })?
}

pub(crate) fn fit_prepared_ols_coefficients_view_map_for_inference<
    P: PreparedOlsFitView + Sync + ?Sized,
>(
    prepared: &P,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    let team = ThreadTeam::new(config.execution.threads);
    // A fast-mode inference call will consume these absorbed columns again for
    // robust or clustered scores. Retain them whenever the bounded 512 MiB
    // cache admits the design; rebuilding and reabsorbing them is always more
    // expensive than the observation-major copy the cache performs.
    let retain_fast_score_cache = true;
    team.install(|| {
        let mut reservation = SystemFitReservation;
        match config.execution.mode {
            PreparedOlsExecutionMode::Fast => fit_with_controls_internal_mode(
                prepared,
                config,
                &mut reservation,
                FitPerturbations::default(),
                retain_fast_score_cache,
            ),
            PreparedOlsExecutionMode::Low => fit_with_controls_low_internal(
                prepared,
                config,
                &mut reservation,
                FitPerturbations::default(),
            ),
        }
    })?
}

struct FitCmgAbsorber<'a> {
    session: Option<CmgAbsorber<'a>>,
    auto_setup_warning: Option<String>,
}

fn build_fit_cmg_absorber<'a, P: PreparedOlsFitView + ?Sized>(
    prepared: &'a P,
    weights: BorrowedMapWeights<'a>,
    config: PreparedOlsFitConfig,
) -> Result<FitCmgAbsorber<'a>> {
    if config.absorb.solver != FeSolverKind::Cmg
        && (config.absorb.solver != FeSolverKind::Auto || !prepared.fit_supports_cmg_auto())
    {
        return Ok(FitCmgAbsorber {
            session: None,
            auto_setup_warning: None,
        });
    }
    let design = prepared.fit_borrowed_cmg_design()?;
    let profile = CmgTopologyProfile::analyze(&design)?;
    if config.absorb.solver == FeSolverKind::Auto && !profile.auto_favorable() {
        return Ok(FitCmgAbsorber {
            session: None,
            auto_setup_warning: None,
        });
    }
    let cmg_config = CmgConfig {
        map: config.absorb.map,
        threads: config.execution.threads,
        batch_size: config.execution.poolsize,
        retain_coefficients: config.absorb.retain_coefficients,
        requested_solver: config.absorb.solver,
        solver_memory_budget: config.absorb.solver_memory_budget,
    };
    match CmgAbsorber::build_profiled(
        design,
        weights,
        cmg_config,
        prepared.fit_structural_intercept(),
        profile,
    ) {
        Ok(session) => Ok(FitCmgAbsorber {
            session: Some(session),
            auto_setup_warning: None,
        }),
        Err(error) if config.absorb.solver == FeSolverKind::Auto => Ok(FitCmgAbsorber {
            session: None,
            auto_setup_warning: Some(format!(
                "automatic CMG setup was unavailable ({error}); used the existing certified auto route"
            )),
        }),
        Err(error) => Err(error),
    }
}

fn absorb_fit_matrix<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: AbsorbConfig,
    cmg: &mut FitCmgAbsorber<'_>,
    initial: bool,
    final_residual: bool,
) -> Result<CertifiedBorrowedMap> {
    if let Some(session) = cmg.session.as_mut() {
        return if final_residual {
            session.absorb_final_residual(values)
        } else {
            session.absorb(values)
        };
    }
    let absorbed = if initial {
        prepared.fit_absorb_initial_matrix(weights, values, config)
    } else {
        prepared.fit_absorb_matrix(weights, values, config)
    }?;
    let Some(warning) = cmg.auto_setup_warning.take() else {
        return Ok(absorbed);
    };
    let (residualized, mut report, coefficients) = absorbed.into_full_parts();
    report.fallback_used = true;
    report.warnings.insert(0, warning);
    if let Some(coefficients) = coefficients {
        CertifiedBorrowedMap::from_certified_parts_with_coefficients(
            residualized,
            report,
            coefficients,
        )
    } else {
        CertifiedBorrowedMap::from_certified_parts(residualized, report)
    }
}

fn fit_with_controls_internal<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
) -> Result<CertifiedPreparedOlsFit> {
    fit_with_controls_internal_mode(prepared, config, reservation, perturbations, false)
}

fn combine_recovered_fe_coefficients<R: FitReservation>(
    initial: Option<FeCoefficientBatch>,
    residual: Option<FeCoefficientBatch>,
    residual_iterations: usize,
    generated_coefficients: &[Option<f64>],
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
    FeCoefficientBatch::from_values(effect_channels, level_counts, 1, combined).map(Some)
}

fn fit_with_controls_internal_mode<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
    retain_fast_score_cache: bool,
) -> Result<CertifiedPreparedOlsFit> {
    validate_config(prepared, config)?;
    let weights = normalized_weights(prepared)?;
    let mut cmg = build_fit_cmg_absorber(prepared, weights.borrowed(), config)?;

    let raw = prepared.fit_build_rhs()?;
    let generated_columns = prepared.fit_model().columns.len();
    let mut reference_norms = reserved_filled(
        reservation,
        generated_columns,
        0.0_f64,
        "unable to reserve prepared OLS reference norms",
    )?;
    let mut reference_mean_scales = if retain_fast_score_cache {
        reserved_filled(
            reservation,
            generated_columns,
            0.0_f64,
            "unable to reserve prepared OLS fast score mean scales",
        )?
    } else {
        Vec::new()
    };
    let raw_moments = if prepared.fit_has_absorbed_intercept() && !retain_fast_score_cache {
        let cache = build_raw_moment_cache(&raw, weights, reservation)?;
        for (column, norm) in reference_norms.iter_mut().enumerate() {
            *norm = cache.norms[column + 1];
        }
        Some(cache)
    } else {
        None
    };
    if retain_fast_score_cache {
        reference_norms
            .par_iter_mut()
            .zip(reference_mean_scales.par_iter_mut())
            .enumerate()
            .try_for_each(|(column, (norm, mean_scale))| -> Result<()> {
                *norm = weighted_matrix_column_norm(&raw, column + 1, weights)?;
                for row in 0..raw.nrows() {
                    *mean_scale = mean_scale.max(raw[(row, column + 1)].abs());
                }
                Ok(())
            })?;
    } else if raw_moments.is_none() {
        reference_norms.par_iter_mut().enumerate().try_for_each(
            |(column, norm)| -> Result<()> {
                *norm = weighted_matrix_column_norm(&raw, column + 1, weights)?;
                Ok(())
            },
        )?;
    }
    let absorbed = absorb_fit_matrix(
        prepared,
        weights.borrowed(),
        raw,
        config.absorb,
        &mut cmg,
        true,
        false,
    )?;
    if !absorbed.report().certified() {
        return Err(invalid_data(
            "prepared OLS initial absorption returned an uncertified result",
        ));
    }
    let (mut within, absorption_report, initial_fe_coefficients) = absorbed.into_full_parts();
    let fast_score_cache = if retain_fast_score_cache {
        build_fast_score_cache(
            prepared,
            &within,
            &reference_mean_scales,
            weights,
            reservation,
        )?
    } else {
        None
    };
    let mut within_norms = reserved_filled(
        reservation,
        generated_columns + 1,
        0.0_f64,
        "unable to reserve prepared OLS fused within norms",
    )?;
    if rayon::current_num_threads() == 1 {
        fill_weighted_matrix_norms_fused(&within, weights, &mut within_norms, reservation)?;
    } else {
        within_norms
            .par_iter_mut()
            .enumerate()
            .try_for_each(|(column, norm)| -> Result<()> {
                *norm = weighted_matrix_column_norm(&within, column, weights)?;
                Ok(())
            })?;
    }
    let within_outcome_norm = within_norms[0];
    let normalized_initial_within_sum_of_squares =
        initial_within_sum_of_squares(prepared, &within, weights, within_outcome_norm)?;
    let score_norms = &within_norms[1..];
    let mut qr = fit_scaled_pivoted_qr(
        prepared,
        &mut within,
        weights,
        &reference_norms,
        score_norms,
        config,
        reservation,
        perturbations.first_coefficient,
        perturbations.finite_bread,
    )?;
    let mut independent_raw_residuals =
        if raw_moments.is_some() && qr.generated_coefficients.iter().any(Option::is_some) {
            Some(build_independent_raw_residuals(prepared, &qr, reservation)?)
        } else {
            None
        };
    let bread = finalize_reported_bread_and_intercept(
        prepared,
        &mut qr,
        independent_raw_residuals.as_deref_mut(),
        weights,
        config.bread_tolerance,
        reservation,
        perturbations,
        fast_score_cache.as_ref(),
        raw_moments.as_ref(),
    )?;
    let initial_absorbed_outcome_sum_of_squares =
        normalized_initial_within_sum_of_squares * bread.weight_scale().reference_weight_multiplier;
    if !initial_absorbed_outcome_sum_of_squares.is_finite()
        || initial_absorbed_outcome_sum_of_squares < 0.0
    {
        return Err(invalid_data(
            "prepared OLS initial absorbed outcome sum of squares is invalid",
        ));
    }
    let mut final_residuals = qr.within_residuals.take().ok_or_else(|| {
        invalid_argument("fast prepared OLS QR did not retain its within residual")
    })?;
    if perturbations.reconstructed_within_residual_constant {
        for residual in &mut final_residuals {
            *residual += 0.25;
        }
    }
    let residual_matrix =
        DenseMatrix::from_row_major(prepared.fit_sample().n_active(), 1, final_residuals)?;
    let residual_absorbed = absorb_fit_matrix(
        prepared,
        weights.borrowed(),
        residual_matrix,
        config.absorb,
        &mut cmg,
        false,
        true,
    )?;
    if !residual_absorbed.report().certified() {
        return Err(invalid_data(
            "prepared OLS residual absorption returned an uncertified result",
        ));
    }
    let (mut residuals, residual_absorption_report, residual_fe_coefficients) =
        residual_absorbed.into_full_parts();
    if perturbations.final_residual_constant {
        for row in 0..residuals.nrows() {
            residuals[(row, 0)] += 0.25;
        }
    }
    let score_certificate = certify_scores(
        prepared,
        residuals.as_slice(),
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
            "prepared OLS finite score certificate did not pass",
        ));
    }

    let recovered_fe_coefficients = combine_recovered_fe_coefficients(
        initial_fe_coefficients,
        residual_fe_coefficients,
        residual_absorption_report.iterations,
        &qr.generated_coefficients,
        reservation,
    )?;
    let coefficients = build_coefficient_slots(prepared, &qr, reservation)?;
    qr.kept.sort_unstable();
    qr.dropped.sort_unstable();
    Ok(CertifiedPreparedOlsFit {
        coefficients,
        kept_generated_columns: qr.kept,
        dropped_generated_columns: qr.dropped,
        finite_rank: qr.rank,
        rank_certificate: qr.certificate,
        score_certificate,
        bread,
        absorption_report,
        residual_absorption_report,
        initial_absorbed_outcome_sum_of_squares,
        residuals,
        recovered_fe_coefficients,
        fast_score_cache,
        execution: PreparedOlsExecutionReport {
            mode: PreparedOlsExecutionMode::Fast,
            poolsize: config.execution.poolsize,
            scratch_bytes: 0,
            requested_threads: config
                .execution
                .threads
                .requested
                .map_or(0, |value| value.get()),
            effective_threads: config.execution.threads.resolved(),
            deterministic: config.execution.threads.deterministic,
        },
    })
}

fn build_independent_raw_residuals<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    qr: &QrFit,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let rows = prepared.fit_sample().n_active();
    let mut residuals = reserved_empty(
        reservation,
        rows,
        "unable to reserve prepared OLS independent raw residual",
    )?;
    residuals.extend_from_slice(&prepared.fit_sample().outcome);
    for (column, coefficient) in qr.generated_coefficients.iter().copied().enumerate() {
        if let Some(coefficient) = coefficient {
            prepared.fit_subtract_raw_column(column, coefficient, &mut residuals)?;
        }
    }
    Ok(residuals)
}

const MAX_FAST_SCORE_CACHE_BYTES: usize = 512 * 1024 * 1024;

fn build_fast_score_cache<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    within: &DenseMatrix,
    reference_mean_scales: &[f64],
    weights: NormalizedWeights<'_>,
    reservation: &mut R,
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
    if bytes > MAX_FAST_SCORE_CACHE_BYTES {
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
        return Ok(Some(PreparedOlsFastScoreCache {
            generated_columns,
            within,
            means,
        }));
    }

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
                        partial[cached + 1] +=
                            weight * (prepared.fit_column_value(generated, row)? / scale);
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
    Ok(Some(PreparedOlsFastScoreCache {
        generated_columns,
        within,
        means,
    }))
}

fn fit_with_controls_low_internal<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
) -> Result<CertifiedPreparedOlsFit> {
    validate_config(prepared, config)?;
    let weights = normalized_weights(prepared)?;
    let mut cmg = build_fit_cmg_absorber(prepared, weights.borrowed(), config)?;
    let rows = prepared.fit_sample().n_active();
    let generated_columns = prepared.fit_model().columns.len();

    let raw_outcome = prepared.fit_build_rhs_block(0, 0, true)?;
    let absorbed_outcome = absorb_fit_matrix(
        prepared,
        weights.borrowed(),
        raw_outcome,
        config.absorb,
        &mut cmg,
        false,
        false,
    )?;
    if !absorbed_outcome.report().certified() {
        return Err(invalid_data(
            "prepared OLS low-memory outcome absorption was not certified",
        ));
    }
    let (within_outcome_matrix, mut absorption_report) = absorbed_outcome.into_parts();
    let mut within_outcome = reserved_filled(
        reservation,
        rows,
        0.0_f64,
        "unable to reserve low-memory prepared OLS outcome",
    )?;
    for (row, value) in within_outcome.iter_mut().enumerate() {
        *value = within_outcome_matrix[(row, 0)];
    }
    let within_outcome_norm = weighted_slice_norm(&within_outcome, weights)?;

    let mut reference_norms = reserved_filled(
        reservation,
        generated_columns,
        0.0_f64,
        "unable to reserve low-memory prepared OLS reference norms",
    )?;
    let mut score_norms = reserved_filled(
        reservation,
        generated_columns,
        0.0_f64,
        "unable to reserve low-memory prepared OLS within score norms",
    )?;
    let mut scratch = PreparedOlsScratch::create(rows, generated_columns)?;
    let mut column_buffer = reserved_filled(
        reservation,
        rows,
        0.0_f64,
        "unable to reserve low-memory prepared OLS column buffer",
    )?;
    let block_width = config.execution.poolsize.min(generated_columns.max(1));
    let mut intercept_cross = None;
    for start in (0..generated_columns).step_by(block_width) {
        let end = start.saturating_add(block_width).min(generated_columns);
        let raw_block = prepared.fit_build_rhs_block(start, end, false)?;
        for (generated, reference_norm) in
            reference_norms.iter_mut().enumerate().take(end).skip(start)
        {
            let block_column = generated - start;
            for row in 0..rows {
                column_buffer[row] = raw_block[(row, block_column)];
            }
            *reference_norm = weighted_slice_norm(&column_buffer, weights)?;
        }
        let absorbed_block = absorb_fit_matrix(
            prepared,
            weights.borrowed(),
            raw_block,
            config.absorb,
            &mut cmg,
            false,
            false,
        )?;
        if !absorbed_block.report().certified() {
            return Err(invalid_data(
                "prepared OLS low-memory regressor-block absorption was not certified",
            ));
        }
        let (within_block, block_report) = absorbed_block.into_parts();
        merge_solve_report(&mut absorption_report, block_report)?;
        for (generated, score_norm) in score_norms.iter_mut().enumerate().take(end).skip(start) {
            let block_column = generated - start;
            for row in 0..rows {
                column_buffer[row] = within_block[(row, block_column)];
            }
            *score_norm = weighted_slice_norm(&column_buffer, weights)?;
            if prepared.fit_report_intercept()
                && !prepared.fit_has_absorbed_intercept()
                && matches!(
                    prepared.fit_model().columns[generated].kind,
                    PreparedOlsColumnKind::Intercept
                )
            {
                intercept_cross = Some((
                    weighted_slice_cross(&within_outcome, &column_buffer, weights)?,
                    *score_norm,
                ));
            }
            scratch.write_column(generated, &column_buffer)?;
        }
    }

    let normalized_initial_within_sum_of_squares =
        initial_within_sum_of_squares_low(prepared, within_outcome_norm, intercept_cross)?;
    let scratch_bytes = scratch.bytes();
    let mut qr = fit_scaled_pivoted_qr_low(
        prepared,
        &mut scratch,
        &mut within_outcome,
        weights,
        &reference_norms,
        &score_norms,
        config,
        reservation,
        perturbations.first_coefficient,
        perturbations.finite_bread,
    )?;

    let mut raw_residuals = reserved_filled(
        reservation,
        rows,
        0.0_f64,
        "unable to reserve low-memory prepared OLS final residual",
    )?;
    raw_residuals
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(row, residual)| -> Result<()> {
            let mut value = prepared.fit_sample().outcome[row];
            for (column, coefficient) in qr.generated_coefficients.iter().enumerate() {
                if let Some(coefficient) = coefficient {
                    value = (-*coefficient).mul_add(prepared.fit_column_value(column, row)?, value);
                }
            }
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared OLS low-memory raw residual is not finite",
                ));
            }
            *residual = value;
            Ok(())
        })?;
    let bread = finalize_reported_bread_and_intercept(
        prepared,
        &mut qr,
        Some(&mut raw_residuals),
        weights,
        config.bread_tolerance,
        reservation,
        perturbations,
        None,
        None,
    )?;
    let initial_absorbed_outcome_sum_of_squares =
        normalized_initial_within_sum_of_squares * bread.weight_scale().reference_weight_multiplier;
    if !initial_absorbed_outcome_sum_of_squares.is_finite()
        || initial_absorbed_outcome_sum_of_squares < 0.0
    {
        return Err(invalid_data(
            "prepared OLS low-memory initial absorbed outcome sum of squares is invalid",
        ));
    }
    let residual_matrix = DenseMatrix::from_row_major(rows, 1, raw_residuals)?;
    let residual_absorbed = absorb_fit_matrix(
        prepared,
        weights.borrowed(),
        residual_matrix,
        config.absorb,
        &mut cmg,
        false,
        true,
    )?;
    if !residual_absorbed.report().certified() {
        return Err(invalid_data(
            "prepared OLS low-memory residual absorption was not certified",
        ));
    }
    let (mut residuals, residual_absorption_report) = residual_absorbed.into_parts();
    if perturbations.final_residual_constant {
        for row in 0..residuals.nrows() {
            residuals[(row, 0)] += 0.25;
        }
    }
    let score_certificate = certify_scores(
        prepared,
        residuals.as_slice(),
        weights,
        &reference_norms,
        &score_norms,
        qr.finite_score,
        &qr.generated_statuses,
        within_outcome_norm,
        config.score_tolerance,
    )?;
    if !score_certificate.passed {
        return Err(invalid_data(
            "prepared OLS low-memory finite score certificate did not pass",
        ));
    }

    let coefficients = build_coefficient_slots(prepared, &qr, reservation)?;
    qr.kept.sort_unstable();
    qr.dropped.sort_unstable();
    Ok(CertifiedPreparedOlsFit {
        coefficients,
        kept_generated_columns: qr.kept,
        dropped_generated_columns: qr.dropped,
        finite_rank: qr.rank,
        rank_certificate: qr.certificate,
        score_certificate,
        bread,
        absorption_report,
        residual_absorption_report,
        initial_absorbed_outcome_sum_of_squares,
        residuals,
        recovered_fe_coefficients: None,
        fast_score_cache: None,
        execution: PreparedOlsExecutionReport {
            mode: PreparedOlsExecutionMode::Low,
            poolsize: config.execution.poolsize,
            scratch_bytes,
            requested_threads: config
                .execution
                .threads
                .requested
                .map_or(0, |value| value.get()),
            effective_threads: config.execution.threads.resolved(),
            deterministic: config.execution.threads.deterministic,
        },
    })
}

fn merge_solve_report(target: &mut SolveReport, source: SolveReport) -> Result<()> {
    let cmg_fallback_mix = target.requested_solver == FeSolverKind::Cmg
        && source.requested_solver == FeSolverKind::Cmg
        && matches!(target.used_solver, FeSolverKind::Cmg | FeSolverKind::Cg)
        && matches!(source.used_solver, FeSolverKind::Cmg | FeSolverKind::Cg);
    if !target.certified()
        || !source.certified()
        || target.requested_solver != source.requested_solver
        || (target.used_solver != source.used_solver && !cmg_fallback_mix)
        || target.requested_preconditioner != source.requested_preconditioner
        || target.used_preconditioner != source.used_preconditioner
    {
        return Err(invalid_data(
            "prepared OLS low-memory solver reports are inconsistent",
        ));
    }
    if cmg_fallback_mix
        && (target.used_solver == FeSolverKind::Cg || source.used_solver == FeSolverKind::Cg)
    {
        target.used_solver = FeSolverKind::Cg;
        target.fallback_used = true;
    }
    target.iterations = target.iterations.max(source.iterations);
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
    target.setup_seconds += source.setup_seconds;
    target.solve_seconds += source.solve_seconds;
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
        || !valid_unit_tolerance(config.score_tolerance)
        || !valid_unit_tolerance(config.bread_tolerance)
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

fn build_standardized_moments<R: FitReservation>(
    within: &DenseMatrix,
    canonical_candidates: &[usize],
    reservation: &mut R,
) -> Result<(Vec<f64>, Vec<f64>)> {
    // Keep the row-block accumulations as ordinary multiply/add operations.
    // `f64::mul_add` becomes a software libm call on the portable x86-64
    // release target when FMA is unavailable at compile time; at moderate K
    // that turns hundreds of millions of moments into function calls. The
    // fixed block order and compensated merge remain deterministic, while the
    // independent score and bread certificates continue to fail closed.
    let candidate_count = canonical_candidates.len();
    if candidate_count == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    if candidate_count > FUSED_MOMENT_MAX_COLUMNS {
        return build_standardized_moments_columnwise(within, canonical_candidates, reservation);
    }
    let gram_elements = checked_square(
        candidate_count,
        "prepared OLS standardized Gram dimensions overflow usize",
    )?;
    let partial_stride = candidate_count
        .checked_add(gram_elements)
        .ok_or_else(|| allocation("prepared OLS moment-partial dimensions overflow usize"))?;
    let chunks = within
        .nrows()
        .div_ceil(PARALLEL_REDUCTION_CHUNK_ROWS)
        .clamp(1, FUSED_MOMENT_MAX_BLOCKS);
    let partial_elements = chunks
        .checked_mul(partial_stride)
        .ok_or_else(|| allocation("prepared OLS moment-partial allocation overflows usize"))?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve prepared OLS moment partials",
    )?;
    let ncols = within.ncols();
    let contiguous_matrix_start = canonical_candidates
        .first()
        .copied()
        .filter(|start| {
            canonical_candidates
                .iter()
                .copied()
                .enumerate()
                .all(|(offset, generated)| start.checked_add(offset) == Some(generated))
        })
        .and_then(|start| start.checked_add(1))
        .filter(|start| {
            start
                .checked_add(candidate_count)
                .is_some_and(|end| end <= ncols)
        });
    partials
        .par_chunks_mut(partial_stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let rows_per_chunk = within.nrows() / chunks;
            let remainder = within.nrows() % chunks;
            let start = chunk * rows_per_chunk + chunk.min(remainder);
            let end = start + rows_per_chunk + usize::from(chunk < remainder);
            if let Some(matrix_start) = contiguous_matrix_start {
                for row in start..end {
                    let values = &within.as_slice()[row * ncols..(row + 1) * ncols];
                    let outcome = values[0];
                    let candidates = &values[matrix_start..matrix_start + candidate_count];
                    for left in 0..candidate_count {
                        let left_value = candidates[left];
                        partial[left] += left_value * outcome;
                        for (offset, right_value) in candidates[left..].iter().copied().enumerate()
                        {
                            let right = left + offset;
                            let position = candidate_count + left * candidate_count + right;
                            partial[position] += left_value * right_value;
                        }
                    }
                }
            } else {
                for row in start..end {
                    let values = &within.as_slice()[row * ncols..(row + 1) * ncols];
                    let outcome = values[0];
                    for (left, generated) in canonical_candidates.iter().copied().enumerate() {
                        let left_value = values[generated + 1];
                        partial[left] += left_value * outcome;
                        for (right, other_generated) in
                            canonical_candidates.iter().copied().enumerate().skip(left)
                        {
                            let position = candidate_count + left * candidate_count + right;
                            partial[position] += left_value * values[other_generated + 1];
                        }
                    }
                }
            }
            if partial.iter().all(|value| value.is_finite()) {
                Ok(())
            } else {
                Err(invalid_data(
                    "prepared OLS standardized moment partial is not finite",
                ))
            }
        })?;

    let mut standardized_rhs = reserved_filled(
        reservation,
        candidate_count,
        0.0_f64,
        "unable to reserve prepared OLS standardized RHS cross-products",
    )?;
    let mut standardized_gram = reserved_filled(
        reservation,
        gram_elements,
        0.0_f64,
        "unable to reserve prepared OLS standardized Gram",
    )?;
    for position in 0..candidate_count {
        let mut sum = 0.0_f64;
        let mut correction = 0.0_f64;
        for partial in partials.chunks_exact(partial_stride) {
            compensated_add(partial[position], &mut sum, &mut correction)?;
        }
        standardized_rhs[position] = sum + correction;
    }
    for left in 0..candidate_count {
        for right in left..candidate_count {
            let partial_position = candidate_count + left * candidate_count + right;
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for partial in partials.chunks_exact(partial_stride) {
                compensated_add(partial[partial_position], &mut sum, &mut correction)?;
            }
            let value = sum + correction;
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared OLS standardized Gram entry is not finite",
                ));
            }
            standardized_gram[left * candidate_count + right] = value;
            standardized_gram[right * candidate_count + left] = value;
        }
    }
    Ok((standardized_rhs, standardized_gram))
}

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

#[allow(clippy::too_many_arguments)]
fn try_fit_scaled_pivoted_cholesky<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    within: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    reference_norms: &[f64],
    score_norms: &[f64],
    canonical_candidates: &[usize],
    initial_statuses: &[PreparedOlsFitStatus],
    standardized_gram: &[f64],
    standardized_rhs: &[f64],
    relative_threshold: f64,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturb_first_coefficient: bool,
    perturb_bread: bool,
) -> Result<Option<QrFit>> {
    let candidate_count = canonical_candidates.len();
    if candidate_count == 0 || candidate_count > FUSED_MOMENT_MAX_COLUMNS {
        return Ok(None);
    }
    let square = checked_square(
        candidate_count,
        "prepared OLS Cholesky dimensions overflow usize",
    )?;
    if standardized_gram.len() != square
        || standardized_rhs.len() != candidate_count
        || initial_statuses.len() != reference_norms.len()
        || score_norms.len() != reference_norms.len()
    {
        return Err(invalid_argument(
            "prepared OLS Cholesky inputs have inconsistent dimensions",
        ));
    }

    let mut gram = reserved_empty(
        reservation,
        square,
        "unable to reserve prepared OLS Cholesky Gram",
    )?;
    gram.extend_from_slice(standardized_gram);
    let mut rhs = reserved_empty(
        reservation,
        candidate_count,
        "unable to reserve prepared OLS Cholesky RHS",
    )?;
    rhs.extend_from_slice(standardized_rhs);
    let mut permutation = reserved_empty(
        reservation,
        candidate_count,
        "unable to reserve prepared OLS Cholesky permutation",
    )?;
    permutation.extend(0..candidate_count);
    let mut factor = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared OLS Cholesky factor",
    )?;
    let initial_pivot = (0..candidate_count)
        .map(|index| gram[index * candidate_count + index])
        .fold(0.0_f64, f64::max);
    if !initial_pivot.is_finite() || initial_pivot <= 0.0 {
        return Ok(None);
    }
    // Normal equations square the finite-design condition number. Only use
    // this route when every Schur pivot clears a deliberately stronger guard
    // than the QR rank threshold; weak or rank-deficient designs retain the
    // existing column-pivoted QR decision and certificates.
    let pivot_guard = initial_pivot * relative_threshold.sqrt();
    if !pivot_guard.is_finite() || pivot_guard <= 0.0 {
        return Ok(None);
    }
    let mut largest_diagonal = 0.0_f64;
    let mut smallest_diagonal = f64::INFINITY;
    for step in 0..candidate_count {
        let mut pivot_position = step;
        let mut pivot_value = gram[step * candidate_count + step];
        for candidate in (step + 1)..candidate_count {
            let value = gram[candidate * candidate_count + candidate];
            if value > pivot_value {
                pivot_position = candidate;
                pivot_value = value;
            }
        }
        if !pivot_value.is_finite() || pivot_value <= pivot_guard {
            return Ok(None);
        }
        if pivot_position != step {
            swap_symmetric_block(&mut gram, candidate_count, step, pivot_position);
            rhs.swap(step, pivot_position);
            permutation.swap(step, pivot_position);
            for column in 0..step {
                factor.swap(
                    step * candidate_count + column,
                    pivot_position * candidate_count + column,
                );
            }
        }
        let diagonal = gram[step * candidate_count + step].sqrt();
        if !diagonal.is_finite() || diagonal <= 0.0 {
            return Ok(None);
        }
        factor[step * candidate_count + step] = diagonal;
        largest_diagonal = largest_diagonal.max(diagonal);
        smallest_diagonal = smallest_diagonal.min(diagonal);
        for row in (step + 1)..candidate_count {
            factor[row * candidate_count + step] = gram[row * candidate_count + step] / diagonal;
        }
        for row in (step + 1)..candidate_count {
            for column in row..candidate_count {
                let updated = gram[column * candidate_count + row]
                    - factor[row * candidate_count + step]
                        * factor[column * candidate_count + step];
                if !updated.is_finite() {
                    return Ok(None);
                }
                gram[column * candidate_count + row] = updated;
                gram[row * candidate_count + column] = updated;
            }
        }
    }

    let mut forward = reserved_filled(
        reservation,
        candidate_count,
        0.0_f64,
        "unable to reserve prepared OLS Cholesky forward solve",
    )?;
    for row in 0..candidate_count {
        let mut value = rhs[row];
        for column in 0..row {
            value = (-factor[row * candidate_count + column]).mul_add(forward[column], value);
        }
        forward[row] = value / factor[row * candidate_count + row];
        if !forward[row].is_finite() {
            return Ok(None);
        }
    }
    let mut pivot_coefficients = reserved_filled(
        reservation,
        candidate_count,
        0.0_f64,
        "unable to reserve prepared OLS Cholesky coefficients",
    )?;
    for row in (0..candidate_count).rev() {
        let mut value = forward[row];
        for column in (row + 1)..candidate_count {
            value = (-factor[column * candidate_count + row])
                .mul_add(pivot_coefficients[column], value);
        }
        pivot_coefficients[row] = value / factor[row * candidate_count + row];
        if !pivot_coefficients[row].is_finite() {
            return Ok(None);
        }
    }
    let mut standardized_coefficients = reserved_filled(
        reservation,
        candidate_count,
        0.0_f64,
        "unable to reserve prepared OLS canonical Cholesky coefficients",
    )?;
    for position in 0..candidate_count {
        standardized_coefficients[permutation[position]] = pivot_coefficients[position];
    }

    let mut inverse_lower = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared OLS inverse Cholesky factor",
    )?;
    for column in 0..candidate_count {
        for row in 0..candidate_count {
            let mut value = f64::from(row == column);
            for inner in 0..row {
                value = (-factor[row * candidate_count + inner])
                    .mul_add(inverse_lower[inner * candidate_count + column], value);
            }
            inverse_lower[row * candidate_count + column] =
                value / factor[row * candidate_count + row];
        }
    }
    let mut canonical_inverse = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared OLS canonical Cholesky inverse",
    )?;
    for left in 0..candidate_count {
        for right in left..candidate_count {
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for inner in 0..candidate_count {
                compensated_add(
                    inverse_lower[inner * candidate_count + left]
                        * inverse_lower[inner * candidate_count + right],
                    &mut sum,
                    &mut correction,
                )?;
            }
            let value = sum + correction;
            if !value.is_finite() {
                return Ok(None);
            }
            let canonical_left = permutation[left];
            let canonical_right = permutation[right];
            canonical_inverse[canonical_left * candidate_count + canonical_right] = value;
            canonical_inverse[canonical_right * candidate_count + canonical_left] = value;
        }
    }

    let mut generated_coefficients = reserved_filled(
        reservation,
        reference_norms.len(),
        None,
        "unable to reserve prepared OLS Cholesky generated coefficients",
    )?;
    let mut generated_statuses = reserved_empty(
        reservation,
        initial_statuses.len(),
        "unable to reserve prepared OLS Cholesky generated statuses",
    )?;
    generated_statuses.extend_from_slice(initial_statuses);
    for (position, generated) in canonical_candidates.iter().copied().enumerate() {
        let coefficient = standardized_coefficients[position] / reference_norms[generated];
        if !coefficient.is_finite() {
            return Ok(None);
        }
        generated_coefficients[generated] = Some(coefficient);
        generated_statuses[generated] = PreparedOlsFitStatus::Kept;
    }
    let mut residuals = reserved_filled(
        reservation,
        within.nrows(),
        0.0_f64,
        "unable to reserve prepared OLS Cholesky residual",
    )?;
    residuals
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(row, residual)| -> Result<()> {
            let values = &within.as_slice()[row * within.ncols()..(row + 1) * within.ncols()];
            let mut value = values[0];
            for (position, generated) in canonical_candidates.iter().copied().enumerate() {
                value =
                    (-standardized_coefficients[position]).mul_add(values[generated + 1], value);
            }
            *residual = value / weights.root(row)?;
            if residual.is_finite() {
                Ok(())
            } else {
                Err(invalid_data("prepared OLS Cholesky residual is not finite"))
            }
        })?;
    if perturb_first_coefficient {
        let coefficient = generated_coefficients
            .iter_mut()
            .flatten()
            .next()
            .ok_or_else(|| invalid_argument("injected perturbation requires a kept coefficient"))?;
        *coefficient += 0.25;
    }
    let finite_score = build_finite_score_draft(
        canonical_candidates,
        standardized_gram,
        standardized_rhs,
        &generated_coefficients,
        &generated_statuses,
        reference_norms,
        score_norms,
    )?;
    let mut generated_columns = reserved_empty(
        reservation,
        candidate_count,
        "unable to reserve prepared OLS Cholesky bread coordinates",
    )?;
    generated_columns.extend_from_slice(canonical_candidates);
    let mut canonical_candidate_positions = reserved_empty(
        reservation,
        candidate_count,
        "unable to reserve prepared OLS Cholesky candidate positions",
    )?;
    canonical_candidate_positions.extend(0..candidate_count);
    let bread = match finalize_finite_bread_draft(
        prepared,
        canonical_candidates,
        standardized_gram,
        generated_columns,
        canonical_candidate_positions,
        canonical_inverse,
        reference_norms,
        weights,
        config.bread_tolerance,
        reservation,
        perturb_bread,
    ) {
        Ok(bread) => bread,
        Err(error) if error.code == ErrorCode::InvalidData && !perturb_bread => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut kept = reserved_empty(
        reservation,
        candidate_count,
        "unable to reserve prepared OLS Cholesky kept columns",
    )?;
    kept.extend_from_slice(canonical_candidates);
    let mut dropped = reserved_empty(
        reservation,
        initial_statuses.len(),
        "unable to reserve prepared OLS Cholesky dropped columns",
    )?;
    for (generated, status) in generated_statuses.iter().copied().enumerate() {
        if status != PreparedOlsFitStatus::Kept {
            push_reserved(
                &mut dropped,
                generated,
                "prepared OLS Cholesky dropped-column capacity exhausted",
            )?;
        }
    }
    Ok(Some(QrFit {
        generated_coefficients,
        generated_statuses,
        kept,
        dropped,
        rank: candidate_count,
        certificate: PreparedOlsRankCertificate {
            relative_threshold,
            maximum_dropped_trailing_norm: 0.0,
            condition_proxy: largest_diagonal / smallest_diagonal,
            passed: true,
        },
        finite_score,
        bread: Some(bread),
        within_residuals: Some(residuals),
    }))
}

fn swap_symmetric_block(matrix: &mut [f64], dimension: usize, left: usize, right: usize) {
    if left == right {
        return;
    }
    for column in 0..dimension {
        matrix.swap(left * dimension + column, right * dimension + column);
    }
    for row in 0..dimension {
        matrix.swap(row * dimension + left, row * dimension + right);
    }
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

#[allow(clippy::too_many_arguments)]
fn fit_scaled_pivoted_qr<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    within: &mut DenseMatrix,
    weights: NormalizedWeights<'_>,
    reference_norms: &[f64],
    score_norms: &[f64],
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturb_first_coefficient: bool,
    perturb_bread: bool,
) -> Result<QrFit> {
    let nrows = within.nrows();
    let generated_columns = prepared.fit_model().columns.len();
    let expected_columns = generated_columns
        .checked_add(1)
        .ok_or_else(|| allocation("prepared OLS absorbed RHS column count overflows"))?;
    if within.ncols() != expected_columns
        || reference_norms.len() != generated_columns
        || score_norms.len() != generated_columns
    {
        return Err(invalid_argument(
            "prepared OLS absorbed RHS dimensions are inconsistent",
        ));
    }

    let mut generated_coefficients = reserved_filled(
        reservation,
        generated_columns,
        None,
        "unable to reserve prepared OLS generated coefficients",
    )?;
    let mut generated_statuses = reserved_filled(
        reservation,
        generated_columns,
        PreparedOlsFitStatus::NumericallyRankDeficient,
        "unable to reserve prepared OLS generated statuses",
    )?;
    let mut candidate_columns = reserved_empty(
        reservation,
        generated_columns,
        "unable to reserve prepared OLS QR candidate columns",
    )?;
    let has_absorbed_intercept = prepared.fit_has_absorbed_intercept();
    for column in 0..generated_columns {
        let is_intercept = matches!(
            prepared.fit_model().columns[column].kind,
            PreparedOlsColumnKind::Intercept
        );
        if has_absorbed_intercept && is_intercept {
            generated_statuses[column] = PreparedOlsFitStatus::AbsorbedInterceptDeferred;
            continue;
        }
        let reference = reference_norms[column];
        if !reference.is_finite() || reference < 0.0 {
            return Err(invalid_data(
                "prepared OLS reference regressor norm is invalid",
            ));
        }
        if reference == 0.0 {
            continue;
        }
        // The caller computed these norms from the untouched within matrix
        // immediately before entering QR. Reuse that certified scale instead
        // of rescanning every candidate column here.
        let within_norm = score_norms[column];
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
        if has_absorbed_intercept && squared_ratio <= config.fixed_effect_collinearity_tolerance {
            generated_statuses[column] = PreparedOlsFitStatus::CollinearWithFixedEffects;
        } else {
            push_reserved(
                &mut candidate_columns,
                column,
                "prepared OLS QR candidate capacity exhausted",
            )?;
        }
    }

    let candidate_count = candidate_columns.len();
    let mut canonical_candidates = reserved_empty(
        reservation,
        candidate_count,
        "unable to reserve prepared OLS canonical bread candidates",
    )?;
    for column in candidate_columns.iter().copied() {
        push_reserved(
            &mut canonical_candidates,
            column,
            "prepared OLS canonical bread-candidate capacity exhausted",
        )?;
    }
    let mut trailing_norms = reserved_filled(
        reservation,
        candidate_count,
        0.0_f64,
        "unable to reserve prepared OLS QR trailing norms",
    )?;
    let mut pivot_coefficients = reserved_filled(
        reservation,
        candidate_count,
        0.0_f64,
        "unable to reserve prepared OLS QR coefficients",
    )?;
    let mut kept = reserved_empty(
        reservation,
        candidate_count,
        "unable to reserve prepared OLS kept-column table",
    )?;
    let mut dropped = reserved_empty(
        reservation,
        generated_columns,
        "unable to reserve prepared OLS dropped-column table",
    )?;
    let relative_threshold = config.rank_tolerance_multiplier
        * f64::EPSILON
        * nrows.max(generated_columns).max(1) as f64;
    if candidate_count == 0 {
        if perturb_first_coefficient {
            return Err(invalid_argument(
                "injected perturbation requires a kept coefficient",
            ));
        }
        for (column, status) in generated_statuses.iter().copied().enumerate() {
            if status != PreparedOlsFitStatus::Kept {
                push_reserved(
                    &mut dropped,
                    column,
                    "prepared OLS dropped-column capacity exhausted",
                )?;
            }
        }
        let standardized_rhs = Vec::new();
        let standardized_gram = Vec::new();
        let finite_score = build_finite_score_draft(
            &canonical_candidates,
            &standardized_gram,
            &standardized_rhs,
            &generated_coefficients,
            &generated_statuses,
            reference_norms,
            score_norms,
        )?;
        let mut within_residuals = reserved_filled(
            reservation,
            nrows,
            0.0_f64,
            "unable to reserve prepared OLS rank-zero within residual",
        )?;
        within_residuals.par_iter_mut().enumerate().try_for_each(
            |(row, residual)| -> Result<()> {
                *residual = within[(row, 0)];
                if residual.is_finite() {
                    Ok(())
                } else {
                    Err(invalid_data(
                        "prepared OLS rank-zero within residual is not finite",
                    ))
                }
            },
        )?;
        let bread = build_finite_bread_draft(
            prepared,
            FiniteBreadInputs {
                qr: within,
                pivot_columns: &candidate_columns,
                canonical_candidates: &canonical_candidates,
                standardized_gram: &standardized_gram,
                rank: 0,
                reference_norms,
                weights,
            },
            config.bread_tolerance,
            reservation,
            perturb_bread,
        )?;
        return Ok(QrFit {
            generated_coefficients,
            generated_statuses,
            kept,
            dropped,
            rank: 0,
            certificate: PreparedOlsRankCertificate {
                relative_threshold,
                maximum_dropped_trailing_norm: 0.0,
                condition_proxy: 1.0,
                passed: true,
            },
            finite_score,
            bread: Some(bread),
            within_residuals: Some(within_residuals),
        });
    }
    within
        .as_mut_slice()
        .par_chunks_mut(expected_columns)
        .enumerate()
        .try_for_each(|(row, values)| -> Result<()> {
            let root_weight = weights.root(row)?;
            let outcome = root_weight * values[0];
            if !outcome.is_finite() {
                return Err(invalid_data(
                    "prepared OLS weighted within outcome is not finite",
                ));
            }
            values[0] = outcome;
            for &column in &candidate_columns {
                let scaled = root_weight * values[column + 1] / reference_norms[column];
                if !scaled.is_finite() {
                    return Err(invalid_data(
                        "prepared OLS standardized within regressor is not finite",
                    ));
                }
                values[column + 1] = scaled;
            }
            Ok(())
        })?;
    let (standardized_rhs, standardized_gram) =
        build_standardized_moments(within, &canonical_candidates, reservation)?;
    if let Some(fit) = try_fit_scaled_pivoted_cholesky(
        prepared,
        within,
        weights,
        reference_norms,
        score_norms,
        &canonical_candidates,
        &generated_statuses,
        &standardized_gram,
        &standardized_rhs,
        relative_threshold,
        config,
        reservation,
        perturb_first_coefficient,
        perturb_bread,
    )? {
        return Ok(fit);
    }
    fill_trailing_norms(within, &candidate_columns, 0, &mut trailing_norms)?;
    let initial_norm = trailing_norms.iter().copied().fold(0.0_f64, f64::max);
    let absolute_threshold = initial_norm * relative_threshold;
    if !absolute_threshold.is_finite() {
        return Err(invalid_data("prepared OLS QR rank threshold is not finite"));
    }

    let mut rank = 0_usize;
    let mut largest_diagonal = 0.0_f64;
    let mut smallest_diagonal = f64::INFINITY;
    while rank < nrows.min(candidate_count) {
        // Rank zero already has the initial trailing norms above. Later ranks
        // require a refresh after applying the previous reflector.
        if rank > 0 {
            fill_trailing_norms(within, &candidate_columns, rank, &mut trailing_norms)?;
        }
        let mut pivot_position = rank;
        let mut pivot_norm = trailing_norms[rank];
        for (position, candidate_norm) in trailing_norms.iter().copied().enumerate().skip(rank + 1)
        {
            let tie_scale = candidate_norm.abs().max(pivot_norm.abs());
            let tied = (candidate_norm - pivot_norm).abs() <= 64.0 * f64::EPSILON * tie_scale;
            if candidate_norm > pivot_norm && !tied
                || tied && candidate_columns[position] < candidate_columns[pivot_position]
            {
                pivot_position = position;
                pivot_norm = candidate_norm;
            }
        }
        if pivot_norm <= absolute_threshold {
            break;
        }
        candidate_columns.swap(rank, pivot_position);
        trailing_norms.swap(rank, pivot_position);
        let column = candidate_columns[rank] + 1;
        // `pivot_norm` is the stable segment norm just selected from the
        // current trailing-norm table; do not scan the pivot column again.
        let norm = pivot_norm;
        if norm <= absolute_threshold {
            break;
        }
        let first = within[(rank, column)];
        let beta = if first >= 0.0 { -norm } else { norm };
        let denominator = first - beta;
        if !beta.is_finite() || beta == 0.0 || !denominator.is_finite() || denominator == 0.0 {
            return Err(invalid_data(
                "prepared OLS QR Householder construction failed",
            ));
        }
        let tau = (beta - first) / beta;
        if !tau.is_finite() {
            return Err(invalid_data(
                "prepared OLS QR Householder coefficient is not finite",
            ));
        }
        within[(rank, column)] = beta;
        if nrows >= PARALLEL_QR_MIN_ROWS && rayon::current_num_threads() > 1 {
            within
                .as_mut_slice()
                .par_chunks_mut(expected_columns)
                .skip(rank + 1)
                .try_for_each(|row| -> Result<()> {
                    row[column] /= denominator;
                    if row[column].is_finite() {
                        Ok(())
                    } else {
                        Err(invalid_data(
                            "prepared OLS QR Householder vector is not finite",
                        ))
                    }
                })?;
        } else {
            for row in (rank + 1)..nrows {
                within[(row, column)] /= denominator;
                if !within[(row, column)].is_finite() {
                    return Err(invalid_data(
                        "prepared OLS QR Householder vector is not finite",
                    ));
                }
            }
        }
        for &other_generated in &candidate_columns[(rank + 1)..] {
            apply_householder(within, rank, column, other_generated + 1, tau)?;
        }
        apply_householder(within, rank, column, 0, tau)?;

        let diagonal = beta.abs();
        largest_diagonal = largest_diagonal.max(diagonal);
        smallest_diagonal = smallest_diagonal.min(diagonal);
        rank = rank
            .checked_add(1)
            .ok_or_else(|| allocation("prepared OLS QR rank overflows usize"))?;
    }

    if rank < candidate_count {
        fill_trailing_norms(within, &candidate_columns, rank, &mut trailing_norms)?;
    }
    let maximum_dropped_trailing_norm = if rank == candidate_count || initial_norm == 0.0 {
        0.0
    } else {
        trailing_norms[rank..]
            .iter()
            .copied()
            .fold(0.0_f64, f64::max)
            / initial_norm
    };
    let rank_passed =
        maximum_dropped_trailing_norm <= relative_threshold * (1.0 + 64.0 * f64::EPSILON);
    if !maximum_dropped_trailing_norm.is_finite() || !rank_passed {
        return Err(invalid_data(
            "prepared OLS QR dropped-column rank certificate did not pass",
        ));
    }

    for position in (0..rank).rev() {
        let column = candidate_columns[position] + 1;
        let diagonal = within[(position, column)];
        if !diagonal.is_finite() || diagonal.abs() <= absolute_threshold {
            return Err(invalid_data(
                "prepared OLS QR triangular diagonal is invalid",
            ));
        }
        let mut value = within[(position, 0)];
        for later in (position + 1)..rank {
            let later_column = candidate_columns[later] + 1;
            value = (-within[(position, later_column)]).mul_add(pivot_coefficients[later], value);
        }
        let scaled = value / diagonal;
        if !scaled.is_finite() {
            return Err(invalid_data("prepared OLS QR coefficient is not finite"));
        }
        pivot_coefficients[position] = scaled;
    }

    for position in 0..rank {
        let column = candidate_columns[position];
        let coefficient = pivot_coefficients[position] / reference_norms[column];
        if !coefficient.is_finite() {
            return Err(invalid_data(
                "prepared OLS coefficient rescaling is not finite",
            ));
        }
        generated_coefficients[column] = Some(coefficient);
        generated_statuses[column] = PreparedOlsFitStatus::Kept;
        push_reserved(
            &mut kept,
            column,
            "prepared OLS kept-column capacity exhausted",
        )?;
    }
    for &column in &candidate_columns[rank..] {
        generated_statuses[column] = PreparedOlsFitStatus::NumericallyRankDeficient;
    }
    if perturb_first_coefficient {
        let coefficient = generated_coefficients
            .iter_mut()
            .flatten()
            .next()
            .ok_or_else(|| invalid_argument("injected perturbation requires a kept coefficient"))?;
        *coefficient += 0.25;
    }
    for (column, status) in generated_statuses.iter().copied().enumerate() {
        if status != PreparedOlsFitStatus::Kept {
            push_reserved(
                &mut dropped,
                column,
                "prepared OLS dropped-column capacity exhausted",
            )?;
        }
    }

    let condition_proxy = if rank == 0 || smallest_diagonal == 0.0 {
        if rank == 0 {
            1.0
        } else {
            f64::INFINITY
        }
    } else {
        largest_diagonal / smallest_diagonal
    };
    if !condition_proxy.is_finite() {
        return Err(invalid_data(
            "prepared OLS QR condition proxy is not finite",
        ));
    }
    let finite_score = build_finite_score_draft(
        &canonical_candidates,
        &standardized_gram,
        &standardized_rhs,
        &generated_coefficients,
        &generated_statuses,
        reference_norms,
        score_norms,
    )?;
    let within_residuals = Some(reconstruct_within_residuals(
        within,
        &candidate_columns,
        rank,
        weights,
        reservation,
    )?);
    let bread = build_finite_bread_draft(
        prepared,
        FiniteBreadInputs {
            qr: within,
            pivot_columns: &candidate_columns,
            canonical_candidates: &canonical_candidates,
            standardized_gram: &standardized_gram,
            rank,
            reference_norms,
            weights,
        },
        config.bread_tolerance,
        reservation,
        perturb_bread,
    )?;
    Ok(QrFit {
        generated_coefficients,
        generated_statuses,
        kept,
        dropped,
        rank,
        certificate: PreparedOlsRankCertificate {
            relative_threshold,
            maximum_dropped_trailing_norm,
            condition_proxy,
            passed: true,
        },
        finite_score,
        bread: Some(bread),
        within_residuals,
    })
}

#[allow(clippy::too_many_arguments)]
fn fit_scaled_pivoted_qr_low<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    scratch: &mut PreparedOlsScratch,
    within_outcome: &mut [f64],
    weights: NormalizedWeights<'_>,
    reference_norms: &[f64],
    score_norms: &[f64],
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturb_first_coefficient: bool,
    perturb_bread: bool,
) -> Result<QrFit> {
    let nrows = prepared.fit_sample().n_active();
    let generated_columns = prepared.fit_model().columns.len();
    if scratch.rows() != nrows
        || scratch.columns() != generated_columns
        || within_outcome.len() != nrows
        || reference_norms.len() != generated_columns
        || score_norms.len() != generated_columns
    {
        return Err(invalid_argument(
            "prepared OLS low-memory QR dimensions are inconsistent",
        ));
    }

    let mut generated_coefficients = reserved_filled(
        reservation,
        generated_columns,
        None,
        "unable to reserve low-memory OLS generated coefficients",
    )?;
    let mut generated_statuses = reserved_filled(
        reservation,
        generated_columns,
        PreparedOlsFitStatus::NumericallyRankDeficient,
        "unable to reserve low-memory OLS generated statuses",
    )?;
    let mut candidate_columns = reserved_empty(
        reservation,
        generated_columns,
        "unable to reserve low-memory OLS QR candidates",
    )?;
    let has_absorbed_intercept = prepared.fit_has_absorbed_intercept();
    for column in 0..generated_columns {
        let is_intercept = matches!(
            prepared.fit_model().columns[column].kind,
            PreparedOlsColumnKind::Intercept
        );
        if has_absorbed_intercept && is_intercept {
            generated_statuses[column] = PreparedOlsFitStatus::AbsorbedInterceptDeferred;
            continue;
        }
        let reference = reference_norms[column];
        let within_norm = score_norms[column];
        if !reference.is_finite()
            || reference < 0.0
            || !within_norm.is_finite()
            || within_norm < 0.0
        {
            return Err(invalid_data(
                "prepared OLS low-memory regressor norm is invalid",
            ));
        }
        if reference == 0.0 {
            continue;
        }
        let ratio = within_norm / reference;
        let squared_ratio = ratio * ratio;
        if !squared_ratio.is_finite() {
            return Err(invalid_data(
                "prepared OLS low-memory collinearity ratio is invalid",
            ));
        }
        if has_absorbed_intercept && squared_ratio <= config.fixed_effect_collinearity_tolerance {
            generated_statuses[column] = PreparedOlsFitStatus::CollinearWithFixedEffects;
        } else {
            push_reserved(
                &mut candidate_columns,
                column,
                "prepared OLS low-memory QR candidate capacity exhausted",
            )?;
        }
    }

    let candidate_count = candidate_columns.len();
    let mut canonical_candidates = reserved_empty(
        reservation,
        candidate_count,
        "unable to reserve low-memory OLS canonical candidates",
    )?;
    for generated in candidate_columns.iter().copied() {
        push_reserved(
            &mut canonical_candidates,
            generated,
            "prepared OLS low-memory canonical-candidate capacity exhausted",
        )?;
    }
    let mut trailing_norms = reserved_filled(
        reservation,
        candidate_count,
        0.0_f64,
        "unable to reserve low-memory OLS trailing norms",
    )?;
    let mut pivot_coefficients = reserved_filled(
        reservation,
        candidate_count,
        0.0_f64,
        "unable to reserve low-memory OLS pivot coefficients",
    )?;
    let mut kept = reserved_empty(
        reservation,
        candidate_count,
        "unable to reserve low-memory OLS kept columns",
    )?;
    let mut dropped = reserved_empty(
        reservation,
        generated_columns,
        "unable to reserve low-memory OLS dropped columns",
    )?;
    let mut reflector = reserved_filled(
        reservation,
        nrows,
        0.0_f64,
        "unable to reserve low-memory OLS reflector buffer",
    )?;
    let mut target = reserved_filled(
        reservation,
        nrows,
        0.0_f64,
        "unable to reserve low-memory OLS target buffer",
    )?;

    for (row, value) in within_outcome.iter_mut().enumerate() {
        *value *= weights.root(row)?;
        if !value.is_finite() {
            return Err(invalid_data(
                "prepared OLS low-memory weighted outcome is not finite",
            ));
        }
    }
    for generated in candidate_columns.iter().copied() {
        scratch.read_column(generated, &mut reflector)?;
        for (row, value) in reflector.iter_mut().enumerate() {
            *value *= weights.root(row)? / reference_norms[generated];
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared OLS low-memory standardized regressor is not finite",
                ));
            }
        }
        scratch.write_column(generated, &reflector)?;
    }

    let mut standardized_rhs = reserved_filled(
        reservation,
        candidate_count,
        0.0_f64,
        "unable to reserve low-memory OLS standardized RHS",
    )?;
    let gram_elements = checked_square(
        candidate_count,
        "prepared OLS low-memory Gram dimensions overflow",
    )?;
    let mut standardized_gram = reserved_filled(
        reservation,
        gram_elements,
        0.0_f64,
        "unable to reserve low-memory OLS standardized Gram",
    )?;
    for (left, generated) in canonical_candidates.iter().copied().enumerate() {
        scratch.read_column(generated, &mut reflector)?;
        let mut rhs_sum = 0.0_f64;
        let mut rhs_correction = 0.0_f64;
        for row in 0..nrows {
            compensated_add(
                reflector[row] * within_outcome[row],
                &mut rhs_sum,
                &mut rhs_correction,
            )?;
        }
        standardized_rhs[left] = rhs_sum + rhs_correction;
        if !standardized_rhs[left].is_finite() {
            return Err(invalid_data(
                "prepared OLS low-memory standardized RHS is not finite",
            ));
        }
        for right in left..candidate_count {
            if right == left {
                target.copy_from_slice(&reflector);
            } else {
                scratch.read_column(canonical_candidates[right], &mut target)?;
            }
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for row in 0..nrows {
                compensated_add(reflector[row] * target[row], &mut sum, &mut correction)?;
            }
            let value = sum + correction;
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared OLS low-memory standardized Gram is not finite",
                ));
            }
            standardized_gram[left * candidate_count + right] = value;
            standardized_gram[right * candidate_count + left] = value;
        }
    }

    fill_scratch_trailing_norms(
        scratch,
        &candidate_columns,
        0,
        &mut trailing_norms,
        &mut target,
    )?;
    let initial_norm = trailing_norms.iter().copied().fold(0.0_f64, f64::max);
    let relative_threshold = config.rank_tolerance_multiplier
        * f64::EPSILON
        * nrows.max(generated_columns).max(1) as f64;
    let absolute_threshold = initial_norm * relative_threshold;
    if !absolute_threshold.is_finite() {
        return Err(invalid_data(
            "prepared OLS low-memory QR rank threshold is not finite",
        ));
    }

    let mut rank = 0_usize;
    let mut largest_diagonal = 0.0_f64;
    let mut smallest_diagonal = f64::INFINITY;
    while rank < nrows.min(candidate_count) {
        fill_scratch_trailing_norms(
            scratch,
            &candidate_columns,
            rank,
            &mut trailing_norms,
            &mut target,
        )?;
        let mut pivot_position = rank;
        let mut pivot_norm = trailing_norms[rank];
        for (position, candidate_norm) in trailing_norms.iter().copied().enumerate().skip(rank + 1)
        {
            let tie_scale = candidate_norm.abs().max(pivot_norm.abs());
            let tied = (candidate_norm - pivot_norm).abs() <= 64.0 * f64::EPSILON * tie_scale;
            if candidate_norm > pivot_norm && !tied
                || tied && candidate_columns[position] < candidate_columns[pivot_position]
            {
                pivot_position = position;
                pivot_norm = candidate_norm;
            }
        }
        if pivot_norm <= absolute_threshold {
            break;
        }
        candidate_columns.swap(rank, pivot_position);
        trailing_norms.swap(rank, pivot_position);
        let pivot_generated = candidate_columns[rank];
        scratch.read_column(pivot_generated, &mut reflector)?;
        let norm = stable_slice_segment_norm(&reflector, rank)?;
        if norm <= absolute_threshold {
            break;
        }
        let first = reflector[rank];
        let beta = if first >= 0.0 { -norm } else { norm };
        let denominator = first - beta;
        if !beta.is_finite() || beta == 0.0 || !denominator.is_finite() || denominator == 0.0 {
            return Err(invalid_data(
                "prepared OLS low-memory Householder construction failed",
            ));
        }
        let tau = (beta - first) / beta;
        if !tau.is_finite() {
            return Err(invalid_data(
                "prepared OLS low-memory Householder coefficient is not finite",
            ));
        }
        reflector[rank] = beta;
        for value in reflector.iter_mut().skip(rank + 1) {
            *value /= denominator;
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared OLS low-memory Householder vector is not finite",
                ));
            }
        }
        scratch.write_column(pivot_generated, &reflector)?;
        for other in candidate_columns.iter().copied().skip(rank + 1) {
            scratch.read_column(other, &mut target)?;
            apply_householder_slices(&reflector, &mut target, rank, tau)?;
            scratch.write_column(other, &target)?;
        }
        apply_householder_slices(&reflector, within_outcome, rank, tau)?;

        let diagonal = beta.abs();
        largest_diagonal = largest_diagonal.max(diagonal);
        smallest_diagonal = smallest_diagonal.min(diagonal);
        rank = rank
            .checked_add(1)
            .ok_or_else(|| allocation("prepared OLS low-memory rank overflows"))?;
    }

    fill_scratch_trailing_norms(
        scratch,
        &candidate_columns,
        rank,
        &mut trailing_norms,
        &mut target,
    )?;
    let maximum_dropped_trailing_norm = if rank == candidate_count || initial_norm == 0.0 {
        0.0
    } else {
        trailing_norms[rank..]
            .iter()
            .copied()
            .fold(0.0_f64, f64::max)
            / initial_norm
    };
    let rank_passed =
        maximum_dropped_trailing_norm <= relative_threshold * (1.0 + 64.0 * f64::EPSILON);
    if !maximum_dropped_trailing_norm.is_finite() || !rank_passed {
        return Err(invalid_data(
            "prepared OLS low-memory dropped-column rank certificate did not pass",
        ));
    }

    let compact_columns = generated_columns
        .checked_add(1)
        .ok_or_else(|| allocation("prepared OLS low-memory compact QR width overflows"))?;
    let compact_elements = rank
        .checked_mul(compact_columns)
        .ok_or_else(|| allocation("prepared OLS low-memory compact QR dimensions overflow"))?;
    let mut compact_values = reserved_filled(
        reservation,
        compact_elements,
        0.0_f64,
        "unable to reserve low-memory OLS compact R",
    )?;
    for row in 0..rank {
        compact_values[row * compact_columns] = within_outcome[row];
    }
    for generated in candidate_columns.iter().copied() {
        scratch.read_column(generated, &mut target)?;
        for row in 0..rank {
            compact_values[row * compact_columns + generated + 1] = target[row];
        }
    }
    let compact_qr = DenseMatrix::from_row_major(rank, compact_columns, compact_values)?;

    for position in (0..rank).rev() {
        let column = candidate_columns[position] + 1;
        let diagonal = compact_qr[(position, column)];
        if !diagonal.is_finite() || diagonal.abs() <= absolute_threshold {
            return Err(invalid_data(
                "prepared OLS low-memory triangular diagonal is invalid",
            ));
        }
        let mut value = within_outcome[position];
        for later in (position + 1)..rank {
            value = (-compact_qr[(position, candidate_columns[later] + 1)])
                .mul_add(pivot_coefficients[later], value);
        }
        pivot_coefficients[position] = value / diagonal;
        if !pivot_coefficients[position].is_finite() {
            return Err(invalid_data(
                "prepared OLS low-memory coefficient is not finite",
            ));
        }
    }
    for position in 0..rank {
        let generated = candidate_columns[position];
        let coefficient = pivot_coefficients[position] / reference_norms[generated];
        if !coefficient.is_finite() {
            return Err(invalid_data(
                "prepared OLS low-memory coefficient rescaling is not finite",
            ));
        }
        generated_coefficients[generated] = Some(coefficient);
        generated_statuses[generated] = PreparedOlsFitStatus::Kept;
        push_reserved(
            &mut kept,
            generated,
            "prepared OLS low-memory kept-column capacity exhausted",
        )?;
    }
    for generated in candidate_columns.iter().copied().skip(rank) {
        generated_statuses[generated] = PreparedOlsFitStatus::NumericallyRankDeficient;
    }
    if perturb_first_coefficient {
        let coefficient = generated_coefficients
            .iter_mut()
            .flatten()
            .next()
            .ok_or_else(|| invalid_argument("injected perturbation requires a kept coefficient"))?;
        *coefficient += 0.25;
    }
    for (generated, status) in generated_statuses.iter().copied().enumerate() {
        if status != PreparedOlsFitStatus::Kept {
            push_reserved(
                &mut dropped,
                generated,
                "prepared OLS low-memory dropped-column capacity exhausted",
            )?;
        }
    }

    let condition_proxy = if rank == 0 || smallest_diagonal == 0.0 {
        if rank == 0 {
            1.0
        } else {
            f64::INFINITY
        }
    } else {
        largest_diagonal / smallest_diagonal
    };
    if !condition_proxy.is_finite() {
        return Err(invalid_data(
            "prepared OLS low-memory condition proxy is not finite",
        ));
    }
    let finite_score = build_finite_score_draft(
        &canonical_candidates,
        &standardized_gram,
        &standardized_rhs,
        &generated_coefficients,
        &generated_statuses,
        reference_norms,
        score_norms,
    )?;
    let bread = build_finite_bread_draft(
        prepared,
        FiniteBreadInputs {
            qr: &compact_qr,
            pivot_columns: &candidate_columns,
            canonical_candidates: &canonical_candidates,
            standardized_gram: &standardized_gram,
            rank,
            reference_norms,
            weights,
        },
        config.bread_tolerance,
        reservation,
        perturb_bread,
    )?;
    Ok(QrFit {
        generated_coefficients,
        generated_statuses,
        kept,
        dropped,
        rank,
        certificate: PreparedOlsRankCertificate {
            relative_threshold,
            maximum_dropped_trailing_norm,
            condition_proxy,
            passed: true,
        },
        finite_score,
        bread: Some(bread),
        within_residuals: None,
    })
}

fn reconstruct_within_residuals<R: FitReservation>(
    qr: &DenseMatrix,
    pivot_columns: &[usize],
    rank: usize,
    weights: NormalizedWeights<'_>,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    if rank > pivot_columns.len() || rank > qr.nrows() {
        return Err(invalid_argument(
            "prepared OLS residual reconstruction dimensions are inconsistent",
        ));
    }
    let mut residuals = reserved_filled(
        reservation,
        qr.nrows(),
        0.0_f64,
        "unable to reserve prepared OLS reconstructed within residual",
    )?;
    if qr.nrows() >= PARALLEL_QR_MIN_ROWS && rayon::current_num_threads() > 1 {
        residuals[rank..]
            .par_iter_mut()
            .enumerate()
            .for_each(|(offset, residual)| *residual = qr[(rank + offset, 0)]);
    } else {
        for (row, residual) in residuals.iter_mut().enumerate().skip(rank) {
            *residual = qr[(row, 0)];
        }
    }
    for position in (0..rank).rev() {
        let column = pivot_columns[position]
            .checked_add(1)
            .ok_or_else(|| allocation("prepared OLS residual reflector column overflows"))?;
        if column >= qr.ncols() {
            return Err(invalid_argument(
                "prepared OLS residual reflector column is out of range",
            ));
        }
        let mut norm_square = 1.0_f64;
        let mut norm_correction = 0.0_f64;
        let mut dot = residuals[position];
        let mut dot_correction = 0.0_f64;
        if qr.nrows() >= PARALLEL_QR_MIN_ROWS && rayon::current_num_threads() > 1 {
            let (tail_norm_square, tail_dot) =
                parallel_reflector_moments(qr, &residuals, column, position + 1)?;
            compensated_add(tail_norm_square, &mut norm_square, &mut norm_correction)?;
            compensated_add(tail_dot, &mut dot, &mut dot_correction)?;
        } else {
            for row in (position + 1)..qr.nrows() {
                let reflector = qr[(row, column)];
                compensated_add(
                    reflector * reflector,
                    &mut norm_square,
                    &mut norm_correction,
                )?;
                compensated_add(reflector * residuals[row], &mut dot, &mut dot_correction)?;
            }
        }
        let tau = 2.0 / (norm_square + norm_correction);
        let projection = tau * (dot + dot_correction);
        if !tau.is_finite() || !projection.is_finite() {
            return Err(invalid_data(
                "prepared OLS residual reflector reconstruction is not finite",
            ));
        }
        residuals[position] -= projection;
        if qr.nrows() >= PARALLEL_QR_MIN_ROWS && rayon::current_num_threads() > 1 {
            residuals
                .par_iter_mut()
                .enumerate()
                .skip(position + 1)
                .for_each(|(row, residual)| {
                    *residual = (-projection).mul_add(qr[(row, column)], *residual);
                });
        } else {
            for row in (position + 1)..qr.nrows() {
                residuals[row] = (-projection).mul_add(qr[(row, column)], residuals[row]);
            }
        }
    }
    residuals
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(row, residual)| -> Result<()> {
            *residual /= weights.root(row)?;
            if residual.is_finite() {
                Ok(())
            } else {
                Err(invalid_data(
                    "prepared OLS reconstructed within residual is not finite",
                ))
            }
        })?;
    Ok(residuals)
}

fn fill_scratch_trailing_norms(
    scratch: &mut PreparedOlsScratch,
    columns: &[usize],
    start_row: usize,
    output: &mut [f64],
    buffer: &mut [f64],
) -> Result<()> {
    if output.len() != columns.len() || start_row > scratch.rows() || buffer.len() != scratch.rows()
    {
        return Err(invalid_argument(
            "prepared OLS low-memory trailing-norm dimensions are inconsistent",
        ));
    }
    for (position, generated) in columns.iter().copied().enumerate() {
        output[position] = if position < start_row {
            0.0
        } else {
            scratch.read_column(generated, buffer)?;
            stable_slice_segment_norm(buffer, start_row)?
        };
    }
    Ok(())
}

fn stable_slice_segment_norm(values: &[f64], start: usize) -> Result<f64> {
    if start > values.len() {
        return Err(invalid_argument(
            "prepared OLS low-memory norm segment is out of range",
        ));
    }
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for value in values.iter().copied().skip(start) {
        update_scaled_sum_of_squares(value, &mut scale, &mut scaled_square_sum)?;
    }
    checked_norm(scale, scaled_square_sum)
}

fn apply_householder_slices(
    reflector: &[f64],
    target: &mut [f64],
    start: usize,
    tau: f64,
) -> Result<()> {
    if reflector.len() != target.len() || start >= target.len() {
        return Err(invalid_argument(
            "prepared OLS low-memory Householder axes are inconsistent",
        ));
    }
    let mut projection = target[start];
    for row in (start + 1)..target.len() {
        projection = reflector[row].mul_add(target[row], projection);
    }
    projection *= tau;
    if !projection.is_finite() {
        return Err(invalid_data(
            "prepared OLS low-memory Householder projection is not finite",
        ));
    }
    target[start] -= projection;
    for row in (start + 1)..target.len() {
        target[row] = (-reflector[row]).mul_add(projection, target[row]);
        if !target[row].is_finite() {
            return Err(invalid_data(
                "prepared OLS low-memory transformed value is not finite",
            ));
        }
    }
    Ok(())
}

fn build_finite_score_draft(
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

    let mut draft = FiniteScoreDraft::default();
    for (left, generated) in canonical_candidates.iter().copied().enumerate() {
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

        let mut fitted = 0.0_f64;
        let mut correction = 0.0_f64;
        for (right, other_generated) in canonical_candidates.iter().copied().enumerate() {
            if let Some(coefficient) = generated_coefficients[other_generated] {
                let standardized_coefficient = coefficient * reference_norms[other_generated];
                if !standardized_coefficient.is_finite() {
                    return Err(invalid_data(
                        "prepared OLS standardized coefficient is not finite",
                    ));
                }
                compensated_add(
                    standardized_gram[left * candidate_count + right] * standardized_coefficient,
                    &mut fitted,
                    &mut correction,
                )?;
            }
        }
        let standardized_score = (standardized_rhs[left] - (fitted + correction)).abs();
        let scale = standardized_score * reference_norm / score_norm;
        if !scale.is_finite() {
            return Err(invalid_data(
                "prepared OLS finite-score normal-equation scale is not finite",
            ));
        }
        draft.infinity_scale = draft.infinity_scale.max(scale);
        draft.square_sum_scale = scale.mul_add(scale, draft.square_sum_scale);
        if !draft.square_sum_scale.is_finite() {
            return Err(invalid_data(
                "prepared OLS finite-score two-norm scale is not finite",
            ));
        }
        draft.moments = draft
            .moments
            .checked_add(1)
            .ok_or_else(|| allocation("prepared OLS finite-score moment count overflows"))?;
    }
    Ok(draft)
}

fn build_finite_bread_draft<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    inputs: FiniteBreadInputs<'_>,
    tolerance: f64,
    reservation: &mut R,
    perturb_bread: bool,
) -> Result<FiniteBreadDraft> {
    let FiniteBreadInputs {
        qr,
        pivot_columns,
        canonical_candidates,
        standardized_gram,
        rank,
        reference_norms,
        weights,
    } = inputs;
    let candidate_count = canonical_candidates.len();
    let candidate_square = checked_square(
        candidate_count,
        "prepared OLS standardized Gram dimensions overflow usize",
    )?;
    if rank > candidate_count
        || pivot_columns.len() != candidate_count
        || standardized_gram.len() != candidate_square
        || reference_norms.len() != prepared.fit_model().columns.len()
    {
        return Err(invalid_argument(
            "prepared OLS bread inputs have inconsistent dimensions",
        ));
    }
    let rank_square = checked_square(rank, "prepared OLS bread dimensions overflow usize")?;
    let mut inverse_r = reserved_filled(
        reservation,
        rank_square,
        0.0_f64,
        "unable to reserve prepared OLS inverse R",
    )?;
    for right_hand_side in 0..rank {
        for row in (0..rank).rev() {
            let generated = pivot_columns[row];
            let matrix_column = generated
                .checked_add(1)
                .ok_or_else(|| allocation("prepared OLS bread column index overflows"))?;
            let diagonal = qr[(row, matrix_column)];
            if !diagonal.is_finite() || diagonal == 0.0 {
                return Err(invalid_data(
                    "prepared OLS bread triangular diagonal is invalid",
                ));
            }
            let mut value = f64::from(row == right_hand_side);
            let mut correction = 0.0_f64;
            for inner in (row + 1)..rank {
                let inner_column = pivot_columns[inner]
                    .checked_add(1)
                    .ok_or_else(|| allocation("prepared OLS bread column index overflows"))?;
                let term = -qr[(row, inner_column)] * inverse_r[inner * rank + right_hand_side];
                compensated_add(term, &mut value, &mut correction)?;
            }
            let solved = (value + correction) / diagonal;
            if !solved.is_finite() {
                return Err(invalid_data("prepared OLS inverse R is not finite"));
            }
            inverse_r[row * rank + right_hand_side] = solved;
        }
    }

    let mut pivot_inverse = reserved_filled(
        reservation,
        rank_square,
        0.0_f64,
        "unable to reserve prepared OLS standardized inverse",
    )?;
    for left in 0..rank {
        for right in left..rank {
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for inner in 0..rank {
                let term = inverse_r[left * rank + inner] * inverse_r[right * rank + inner];
                compensated_add(term, &mut sum, &mut correction)?;
            }
            let value = sum + correction;
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared OLS standardized inverse is not finite",
                ));
            }
            pivot_inverse[left * rank + right] = value;
            pivot_inverse[right * rank + left] = value;
        }
    }

    let mut generated_columns = reserved_empty(
        reservation,
        rank,
        "unable to reserve prepared OLS bread coordinates",
    )?;
    for generated in canonical_candidates.iter().copied() {
        if pivot_columns[..rank].contains(&generated) {
            push_reserved(
                &mut generated_columns,
                generated,
                "prepared OLS bread-coordinate capacity exhausted",
            )?;
        }
    }
    if generated_columns.len() != rank {
        return Err(invalid_argument(
            "prepared OLS bread rank and coordinate count disagree",
        ));
    }

    let mut canonical_candidate_positions = reserved_empty(
        reservation,
        rank,
        "unable to reserve prepared OLS bread candidate positions",
    )?;
    let mut canonical_pivot_positions = reserved_empty(
        reservation,
        rank,
        "unable to reserve prepared OLS bread pivot positions",
    )?;
    for generated in generated_columns.iter().copied() {
        let candidate = canonical_candidates
            .iter()
            .position(|column| *column == generated)
            .ok_or_else(|| invalid_argument("prepared OLS bread candidate is missing"))?;
        let pivot = pivot_columns[..rank]
            .iter()
            .position(|column| *column == generated)
            .ok_or_else(|| invalid_argument("prepared OLS bread pivot is missing"))?;
        push_reserved(
            &mut canonical_candidate_positions,
            candidate,
            "prepared OLS bread candidate-position capacity exhausted",
        )?;
        push_reserved(
            &mut canonical_pivot_positions,
            pivot,
            "prepared OLS bread pivot-position capacity exhausted",
        )?;
    }

    let mut canonical_inverse = reserved_filled(
        reservation,
        rank_square,
        0.0_f64,
        "unable to reserve prepared OLS canonical standardized inverse",
    )?;
    for left in 0..rank {
        for right in 0..rank {
            canonical_inverse[left * rank + right] = pivot_inverse
                [canonical_pivot_positions[left] * rank + canonical_pivot_positions[right]];
        }
    }
    finalize_finite_bread_draft(
        prepared,
        canonical_candidates,
        standardized_gram,
        generated_columns,
        canonical_candidate_positions,
        canonical_inverse,
        reference_norms,
        weights,
        tolerance,
        reservation,
        perturb_bread,
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize_finite_bread_draft<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    canonical_candidates: &[usize],
    standardized_gram: &[f64],
    generated_columns: Vec<usize>,
    canonical_candidate_positions: Vec<usize>,
    mut canonical_inverse: Vec<f64>,
    reference_norms: &[f64],
    weights: NormalizedWeights<'_>,
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
        return Err(invalid_data(
            "prepared OLS bread inverse certificate did not pass",
        ));
    }

    let weight_scale = prepared_bread_weight_scale(prepared, weights)?;
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
    let gram_square = checked_square(
        gram_dimension,
        "prepared OLS bread Gram dimensions overflow usize",
    )?;
    let rank_square = checked_square(rank, "prepared OLS bread dimensions overflow usize")?;
    if gram.len() != gram_square
        || gram_positions.len() != rank
        || inverse.len() != rank_square
        || gram_positions
            .iter()
            .any(|position| *position >= gram_dimension)
    {
        return Err(invalid_argument(
            "prepared OLS inverse-certificate dimensions are inconsistent",
        ));
    }
    let mut maximum = 0.0_f64;
    for row in 0..rank {
        let mut row_error = 0.0_f64;
        let mut row_scale = 1.0_f64;
        for column in 0..rank {
            let mut product = 0.0_f64;
            let mut correction = 0.0_f64;
            let mut absolute_products = 0.0_f64;
            for inner in 0..rank {
                let gram_value = gram[gram_positions[row] * gram_dimension + gram_positions[inner]];
                let inverse_value = inverse[inner * rank + column];
                let (left, right) = if reverse {
                    (
                        inverse[row * rank + inner],
                        gram[gram_positions[inner] * gram_dimension + gram_positions[column]],
                    )
                } else {
                    (gram_value, inverse_value)
                };
                let term = left * right;
                compensated_add(term, &mut product, &mut correction)?;
                absolute_products += term.abs();
                if !absolute_products.is_finite() {
                    return Err(invalid_data(
                        "prepared OLS inverse-certificate scale is not finite",
                    ));
                }
            }
            let delta = f64::from(row == column);
            row_error += (product + correction - delta).abs();
            row_scale += absolute_products;
            if !row_error.is_finite() || !row_scale.is_finite() {
                return Err(invalid_data(
                    "prepared OLS inverse-certificate accumulation is not finite",
                ));
            }
        }
        maximum = maximum.max(row_error / row_scale);
    }
    if maximum.is_finite() {
        Ok(maximum)
    } else {
        Err(invalid_data(
            "prepared OLS inverse certificate is not finite",
        ))
    }
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
    qr: &mut QrFit,
    raw_residuals: Option<&mut [f64]>,
    weights: NormalizedWeights<'_>,
    tolerance: f64,
    reservation: &mut R,
    perturbations: FitPerturbations,
    fast_score_cache: Option<&PreparedOlsFastScoreCache>,
    raw_moments: Option<&RawMomentCache>,
) -> Result<PreparedOlsBread> {
    if raw_residuals
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
        for (position, generated) in finite.generated_columns.iter().copied().enumerate() {
            means[position] = if let Some(mean) =
                raw_moments.and_then(|cache| cache.means.get(generated + 1))
            {
                *mean
            } else if let Some(mean) = fast_score_cache.and_then(|cache| cache.mean(generated)) {
                mean
            } else {
                weighted_mean_by(prepared.fit_sample().n_active(), weights, |row| {
                    prepared.fit_column_value(generated, row)
                })?
            };
        }

        // Fast mode already owns the reconstructed within residual from the QR
        // state. Derive the independent raw-residual mean directly instead of
        // allocating and retaining a second observation-scale residual vector.
        let raw_residual_mean = if let Some(residuals) = raw_residuals.as_deref() {
            weighted_mean_slice(residuals, weights)?
        } else if let Some(cache) = raw_moments {
            cached_raw_residual_mean(prepared, qr, weights, cache)?
        } else {
            weighted_mean_by(prepared.fit_sample().n_active(), weights, |row| {
                let mut value = prepared.fit_sample().outcome[row];
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
            })?
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
            weighted_mean_slice(&prepared.fit_sample().outcome, weights)?
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
    partials
        .par_chunks_mut(partial_stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
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
        })?;
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
    partials
        .par_chunks_mut(partial_stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
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
        })?;
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
    scales
        .par_iter_mut()
        .zip(means.par_iter_mut())
        .zip(norms.par_iter_mut())
        .enumerate()
        .try_for_each(|(column, ((scale, mean), norm))| -> Result<()> {
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
        })
}

fn cached_raw_residual_mean<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    qr: &QrFit,
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
    qr: &QrFit,
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
    if value_scale == 0.0 {
        return Ok(0.0);
    }
    let mut numerator = 0.0_f64;
    let mut numerator_correction = 0.0_f64;
    let mut denominator = 0.0_f64;
    let mut denominator_correction = 0.0_f64;
    for row in 0..nrows {
        let weight = weights.value(row)?;
        compensated_add(
            weight * (value(row)? / value_scale),
            &mut numerator,
            &mut numerator_correction,
        )?;
        compensated_add(weight, &mut denominator, &mut denominator_correction)?;
    }
    let denominator = denominator + denominator_correction;
    let mean = value_scale * ((numerator + numerator_correction) / denominator);
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

fn fill_trailing_norms(
    matrix: &DenseMatrix,
    columns: &[usize],
    start_row: usize,
    output: &mut [f64],
) -> Result<()> {
    if output.len() != columns.len() || start_row > matrix.nrows() {
        return Err(invalid_argument(
            "prepared OLS QR trailing-norm dimensions are inconsistent",
        ));
    }
    if matrix.nrows() >= PARALLEL_QR_MIN_ROWS && rayon::current_num_threads() > 1 {
        output.par_iter_mut().enumerate().try_for_each(
            |(position, destination)| -> Result<()> {
                *destination = if position < start_row {
                    0.0
                } else {
                    stable_matrix_segment_norm(matrix, columns[position] + 1, start_row)?
                };
                Ok(())
            },
        )?;
    } else {
        for (position, generated_column) in columns.iter().copied().enumerate() {
            output[position] = if position < start_row {
                0.0
            } else {
                stable_matrix_segment_norm(matrix, generated_column + 1, start_row)?
            };
        }
    }
    Ok(())
}

fn stable_matrix_segment_norm(
    matrix: &DenseMatrix,
    column: usize,
    start_row: usize,
) -> Result<f64> {
    if column >= matrix.ncols() || start_row > matrix.nrows() {
        return Err(invalid_argument(
            "prepared OLS matrix norm segment is out of range",
        ));
    }
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for row in start_row..matrix.nrows() {
        update_scaled_sum_of_squares(matrix[(row, column)], &mut scale, &mut scaled_square_sum)?;
    }
    checked_norm(scale, scaled_square_sum)
}

fn apply_householder(
    matrix: &mut DenseMatrix,
    start_row: usize,
    reflector_column: usize,
    target_column: usize,
    tau: f64,
) -> Result<()> {
    let mut projection = matrix[(start_row, target_column)];
    if matrix.nrows() >= PARALLEL_QR_MIN_ROWS && rayon::current_num_threads() > 1 {
        let tail =
            parallel_matrix_column_dot(matrix, reflector_column, target_column, start_row + 1)?;
        projection += tail;
    } else {
        for row in (start_row + 1)..matrix.nrows() {
            projection =
                matrix[(row, reflector_column)].mul_add(matrix[(row, target_column)], projection);
        }
    }
    projection *= tau;
    if !projection.is_finite() {
        return Err(invalid_data("prepared OLS QR projection is not finite"));
    }
    matrix[(start_row, target_column)] -= projection;
    if !matrix[(start_row, target_column)].is_finite() {
        return Err(invalid_data(
            "prepared OLS QR transformed value is not finite",
        ));
    }
    if matrix.nrows() >= PARALLEL_QR_MIN_ROWS && rayon::current_num_threads() > 1 {
        let ncols = matrix.ncols();
        matrix
            .as_mut_slice()
            .par_chunks_mut(ncols)
            .skip(start_row + 1)
            .try_for_each(|row| -> Result<()> {
                let updated = (-row[reflector_column]).mul_add(projection, row[target_column]);
                if !updated.is_finite() {
                    return Err(invalid_data(
                        "prepared OLS QR transformed value is not finite",
                    ));
                }
                row[target_column] = updated;
                Ok(())
            })?;
    } else {
        for row in (start_row + 1)..matrix.nrows() {
            let updated = (-matrix[(row, reflector_column)])
                .mul_add(projection, matrix[(row, target_column)]);
            if !updated.is_finite() {
                return Err(invalid_data(
                    "prepared OLS QR transformed value is not finite",
                ));
            }
            matrix[(row, target_column)] = updated;
        }
    }
    Ok(())
}

fn parallel_matrix_column_dot(
    matrix: &DenseMatrix,
    left_column: usize,
    right_column: usize,
    start_row: usize,
) -> Result<f64> {
    if left_column >= matrix.ncols() || right_column >= matrix.ncols() || start_row > matrix.nrows()
    {
        return Err(invalid_argument(
            "prepared OLS parallel dot dimensions are inconsistent",
        ));
    }
    let rows = matrix.nrows() - start_row;
    let chunks = rows.div_ceil(PARALLEL_REDUCTION_CHUNK_ROWS);
    let mut partials = Vec::new();
    partials
        .try_reserve_exact(chunks)
        .map_err(|_| allocation("unable to reserve parallel QR dot partials"))?;
    partials.resize(chunks, 0.0_f64);
    partials
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let start = start_row + chunk * PARALLEL_REDUCTION_CHUNK_ROWS;
            let end = (start + PARALLEL_REDUCTION_CHUNK_ROWS).min(matrix.nrows());
            let mut sum = 0.0_f64;
            let mut correction = 0.0_f64;
            for row in start..end {
                compensated_add(
                    matrix[(row, left_column)] * matrix[(row, right_column)],
                    &mut sum,
                    &mut correction,
                )?;
            }
            *partial = sum + correction;
            Ok(())
        })?;
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for partial in partials {
        compensated_add(partial, &mut sum, &mut correction)?;
    }
    Ok(sum + correction)
}

fn parallel_reflector_moments(
    qr: &DenseMatrix,
    residuals: &[f64],
    reflector_column: usize,
    start_row: usize,
) -> Result<(f64, f64)> {
    if reflector_column >= qr.ncols() || residuals.len() != qr.nrows() || start_row > qr.nrows() {
        return Err(invalid_argument(
            "prepared OLS parallel reflector dimensions are inconsistent",
        ));
    }
    let rows = qr.nrows() - start_row;
    let chunks = rows.div_ceil(PARALLEL_REDUCTION_CHUNK_ROWS);
    let mut partials = Vec::new();
    partials
        .try_reserve_exact(chunks)
        .map_err(|_| allocation("unable to reserve parallel reflector partials"))?;
    partials.resize(chunks, (0.0_f64, 0.0_f64));
    partials
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let start = start_row + chunk * PARALLEL_REDUCTION_CHUNK_ROWS;
            let end = (start + PARALLEL_REDUCTION_CHUNK_ROWS).min(qr.nrows());
            let mut norm = 0.0_f64;
            let mut norm_correction = 0.0_f64;
            let mut dot = 0.0_f64;
            let mut dot_correction = 0.0_f64;
            for row in start..end {
                let reflector = qr[(row, reflector_column)];
                compensated_add(reflector * reflector, &mut norm, &mut norm_correction)?;
                compensated_add(reflector * residuals[row], &mut dot, &mut dot_correction)?;
            }
            *partial = (norm + norm_correction, dot + dot_correction);
            Ok(())
        })?;
    let mut norm = 0.0_f64;
    let mut norm_correction = 0.0_f64;
    let mut dot = 0.0_f64;
    let mut dot_correction = 0.0_f64;
    for (partial_norm, partial_dot) in partials {
        compensated_add(partial_norm, &mut norm, &mut norm_correction)?;
        compensated_add(partial_dot, &mut dot, &mut dot_correction)?;
    }
    Ok((norm + norm_correction, dot + dot_correction))
}

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
    if values.len() >= PARALLEL_QR_MIN_ROWS && rayon::current_num_threads() > 1 {
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
        scales
            .par_iter_mut()
            .zip(sums.par_iter_mut())
            .enumerate()
            .try_for_each(|(chunk, (scale, sum))| -> Result<()> {
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
            })?;
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
            for (row, residual) in residuals.iter().copied().enumerate() {
                let root_weight = weights.root(row)?;
                let regressor = prepared.fit_column_value(column, row)?;
                let left = (root_weight * regressor) / denominator;
                let right = (root_weight * residual) / response_scale;
                let term = left * right;
                compensated_add(term, &mut sum, &mut correction)?;
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
    let updated = *sum + value;
    let adjustment = if sum.abs() >= value.abs() {
        (*sum - updated) + value
    } else {
        (value - updated) + *sum
    };
    *sum = updated;
    *correction += adjustment;
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
    qr: &QrFit,
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

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn checked_square(value: usize, message: &'static str) -> Result<usize> {
    value.checked_mul(value).ok_or_else(|| allocation(message))
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

fn insufficient_observations(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InsufficientObservations, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm,
        LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId, TermComponent, TermId,
        TermOrder, TimeSeriesState,
    };
    use crate::ols::{
        prepare_ols_data, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
        OlsRequestV1,
    };

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

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        let scale = actual.abs().max(expected.abs()).max(1.0);
        assert!(
            (actual - expected).abs() <= tolerance * scale,
            "actual={actual:.17e}, expected={expected:.17e}, tolerance={tolerance:.3e}"
        );
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
        let (rhs, gram) = build_standardized_moments(&matrix, &candidates, &mut reservation)
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
        let (rhs, gram) = build_standardized_moments(&matrix, &candidates, &mut fused_reservation)
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
        let (rhs, gram) = build_standardized_moments(&matrix, &[0], &mut reservation)
            .expect("capped fused standardized moments");
        assert_eq!(rhs, vec![102.0]);
        assert_eq!(gram, vec![153.0]);
    }

    #[test]
    fn guarded_cholesky_accepts_well_conditioned_wide_finite_design() {
        let rows = 512_usize;
        let columns = 12_usize;
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
                regressors.iter().enumerate().fold(
                    0.02 * (row as f64 * 0.13).cos(),
                    |value, (column, regressor)| {
                        ((column + 1) as f64 / columns as f64).mul_add(regressor[row], value)
                    },
                )
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
        let mut within_values = Vec::with_capacity(rows * (columns + 1));
        for row in 0..rows {
            within_values.push(outcome[row]);
            for regressor in &regressors {
                within_values.push(regressor[row]);
            }
        }
        let mut within = DenseMatrix::from_row_major(rows, columns + 1, within_values)
            .expect("wide within matrix");
        let weights = normalized_weights(&prepared).expect("unit weights");
        let mut reference_norms = Vec::with_capacity(columns);
        for column in 0..columns {
            reference_norms.push(
                weighted_matrix_column_norm(&within, column + 1, weights).expect("reference norm"),
            );
        }
        let score_norms = reference_norms.clone();
        for row in 0..rows {
            for column in 0..columns {
                within[(row, column + 1)] /= reference_norms[column];
            }
        }
        let candidates = (0..columns).collect::<Vec<_>>();
        let statuses = vec![PreparedOlsFitStatus::NumericallyRankDeficient; columns];
        let mut reservation = SystemFitReservation;
        let (rhs, gram) = build_standardized_moments(&within, &candidates, &mut reservation)
            .expect("wide standardized moments");
        let relative_threshold =
            PreparedOlsFitConfig::default().rank_tolerance_multiplier * f64::EPSILON * rows as f64;
        let fit = try_fit_scaled_pivoted_cholesky(
            &prepared,
            &within,
            weights,
            &reference_norms,
            &score_norms,
            &candidates,
            &statuses,
            &gram,
            &rhs,
            relative_threshold,
            PreparedOlsFitConfig::default(),
            &mut reservation,
            false,
            false,
        )
        .expect("guarded Cholesky attempt")
        .expect("well-conditioned design selects guarded Cholesky");
        assert_eq!(fit.rank, columns);
        assert!(fit.certificate.passed);
        assert_eq!(fit.certificate.maximum_dropped_trailing_norm, 0.0);
        assert!(fit.finite_score.infinity_scale < 1e-10);
        let bread = fit.bread.expect("certified Cholesky bread");
        assert!(bread.left_inverse_relative_infinity_norm < 5e-10);
        assert!(bread.right_inverse_relative_infinity_norm < 5e-10);
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
            )
            .expect_err("injected fast inference-cache allocation failure");
            assert_eq!(
                error.code,
                ErrorCode::Allocation,
                "cached failure {fail_at}"
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
        let rhs = fe_prepared.fit_build_rhs().expect("rhs");
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
