#[cfg(test)]
use super::prepared_inference_view::compute_canonical_prepared_ols_absorbed_dof;
use super::prepared_wald::{calculate_wald, WaldCalculation};
use crate::data::WeightKind;
use crate::error::{allocation, invalid_argument, invalid_data, unsupported, Result};
use crate::parallel::ThreadTeam;
use crate::sandwich::{
    bread_meat_bread, reduce_dense_score_meats, reduce_serial_influence_covariance,
    standard_errors, FixedBlockReductionPolicy, SandwichCorrectionPolicy, SandwichProductMode,
    SandwichScoreMultiplier, SandwichScorePair,
};
use crate::topology_dof::CategoricalTopologyDofReport;
use rayon::prelude::*;
use std::time::Instant;

#[cfg(test)]
use super::prepared_fit::fit_prepared_ols_coefficients_view_map;
#[cfg(test)]
use super::prepared_fit::fit_prepared_ols_coefficients_view_map_for_inference;
use super::prepared_inference_view::{PreparedOlsAbsorbedDofSummary, PreparedOlsInferenceView};
#[cfg(test)]
use super::prepared_scores::take_prepared_ols_score_product;
use super::prepared_scores::PreparedOlsScoreProduct;
use super::prepared_scores::{
    build_prepared_ols_score_design, reserved_filled, CertifiedPreparedOlsScoreDesign,
    PreparedInferenceReservation, PreparedOlsDenseScoreView, SystemPreparedInferenceReservation,
};
use super::prepared_unadjusted::{
    build_reference_weight_report_view, build_residual_dof_view, build_statistics, checked_square,
    compensated_add, compensated_add_scaled_row, exact_integer_as_f64, independent_outcome_sum,
    independent_residual_sum, relative_difference, validate_fit_and_absorbed_dof_view,
    validate_sum_of_squares, ReferenceWeightView,
};
use super::{
    CertifiedPreparedOlsFit, PreparedOlsAbsorbedDofPolicy, PreparedOlsColumnKind, PreparedOlsData,
    PreparedOlsFitConfig, PreparedOlsFitStatistics, PreparedOlsReferenceWeightReport,
    PreparedOlsResidualDofReport,
};

/// Numerical and structural policy for the prepared HC1 OLS/WLS checkpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsRobustConfig {
    pub fit: PreparedOlsFitConfig,
    pub absorbed_dof: PreparedOlsAbsorbedDofPolicy,
    /// Maximum normalized score-design, sandwich, PSD, and statistic error.
    pub certificate_tolerance: f64,
}

const ROBUST_REDUCTION_ROWS: usize = 16_384;
const ROBUST_MAX_BLOCKS: usize = 64;
const ROBUST_PARTIAL_BUDGET_BYTES: usize = 32 << 20;

impl Default for PreparedOlsRobustConfig {
    fn default() -> Self {
        Self {
            fit: PreparedOlsFitConfig::default(),
            absorbed_dof: PreparedOlsAbsorbedDofPolicy::default(),
            certificate_tolerance: 5e-9,
        }
    }
}

/// Independently recomputed certificate for HC1 covariance and robust Wald F.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsRobustCertificate {
    pub reference_weight_sum_relative_error: f64,
    pub score_design_left_inverse_relative_infinity_norm: f64,
    pub score_design_right_inverse_relative_infinity_norm: f64,
    pub intercept_coordinate_relative_infinity_norm: f64,
    pub hc1_correction_relative_error: f64,
    pub sandwich_formula_relative_infinity_norm: f64,
    pub covariance_symmetry_relative_infinity_norm: f64,
    pub standard_error_relative_infinity_norm: f64,
    pub minimum_normalized_psd_pivot: f64,
    pub psd_rank: usize,
    pub wald_solve_relative_infinity_norm: f64,
    pub statistic_formula_relative_infinity_norm: f64,
    pub passed: bool,
}

/// HC1 covariance on the fit's canonical reported bread axis.
#[derive(Debug, PartialEq)]
pub struct PreparedOlsRobustCovariance {
    hc1_correction: f64,
    values: Vec<f64>,
    standard_errors: Vec<f64>,
}

impl PreparedOlsRobustCovariance {
    #[must_use]
    pub const fn hc1_correction(&self) -> f64 {
        self.hc1_correction
    }

    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    #[must_use]
    pub fn standard_errors(&self) -> &[f64] {
        &self.standard_errors
    }

    #[must_use]
    pub fn dimension(&self) -> usize {
        self.standard_errors.len()
    }
}

/// Same-call certified fit with its robust covariance payload.
pub type CertifiedPreparedOlsRobust<A = CategoricalTopologyDofReport> =
    super::prepared_products::CertifiedPreparedOlsInference<
        A,
        PreparedOlsResidualDofReport,
        PreparedOlsRobustCovariance,
        PreparedOlsRobustCertificate,
    >;

impl super::prepared_products::PreparedOlsInferenceBuffers for PreparedOlsRobustCovariance {
    fn inference_owned_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        bytes(&self.values) + bytes(&self.standard_errors)
    }
}

type PreparedOlsRobustComponents = CertifiedPreparedOlsRobust<()>;

struct PreparedOlsRobustCovarianceBuild {
    covariance: PreparedOlsRobustCovariance,
    /// Independently accumulated raw score meat from the fused dense pass.
    /// The production covariance is accumulated from bread-transformed
    /// influences, so this remains an algebraically distinct certificate path.
    dense_certificate_meat: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Copy)]
struct OlsRobustScoreMultiplier<'a> {
    residuals: &'a [f64],
    weights: ReferenceWeightView<'a>,
    weight_kind: Option<WeightKind>,
    perturb_production: bool,
}

impl SandwichScoreMultiplier for OlsRobustScoreMultiplier<'_> {
    fn nrows(&self) -> usize {
        self.residuals.len()
    }

    fn score_pair(&self, row: usize) -> Result<SandwichScorePair> {
        let reference_weight = self.weights.value(row)?;
        let certificate_root = meat_root_multiplier(self.weight_kind, reference_weight)?;
        let production_root = if self.perturb_production {
            match self.weight_kind {
                Some(WeightKind::Analytic | WeightKind::Probability) => reference_weight.sqrt(),
                Some(WeightKind::Frequency) => reference_weight,
                _ => 1.0,
            }
        } else {
            certificate_root
        };
        let residual = *self
            .residuals
            .get(row)
            .ok_or_else(|| invalid_argument("prepared robust residual row is out of range"))?;
        let production = production_root * residual;
        let certificate = certificate_root * residual;
        if production.is_finite() && certificate.is_finite() {
            Ok(SandwichScorePair {
                production,
                certificate,
            })
        } else {
            Err(invalid_data(
                "prepared robust dense influence scale is not finite",
            ))
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct OlsRobustSerialScoreMultiplier<'a> {
    residuals: &'a [f64],
    weights: ReferenceWeightView<'a>,
    weight_kind: Option<WeightKind>,
    perturb_production: bool,
    correction_root: f64,
}

impl SandwichScoreMultiplier for OlsRobustSerialScoreMultiplier<'_> {
    fn nrows(&self) -> usize {
        self.residuals.len()
    }

    fn score_pair(&self, row: usize) -> Result<SandwichScorePair> {
        let reference_weight = self.weights.value(row)?;
        let certificate_root = meat_root_multiplier(self.weight_kind, reference_weight)?;
        let production_root = if self.perturb_production {
            match self.weight_kind {
                Some(WeightKind::Analytic | WeightKind::Probability) => reference_weight.sqrt(),
                Some(WeightKind::Frequency) => reference_weight,
                _ => 1.0,
            }
        } else {
            certificate_root
        };
        let residual = *self
            .residuals
            .get(row)
            .ok_or_else(|| invalid_argument("prepared robust residual row is out of range"))?;
        // This is the frozen low-memory OLS evaluation order.
        let production = self.correction_root * production_root * residual;
        let certificate = certificate_root * residual;
        if production.is_finite() && certificate.is_finite() {
            Ok(SandwichScorePair {
                production,
                certificate,
            })
        } else {
            Err(invalid_data(
                "prepared robust influence scale is not finite",
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct RobustPerturbations {
    score_design: bool,
    meat_multiplier: bool,
    residual_dof: bool,
    hc1_correction: bool,
    covariance: bool,
    covariance_symmetry: bool,
    covariance_indefinite: bool,
    standard_error: bool,
    rss: bool,
    tss: bool,
    within_tss: bool,
    f_statistic: bool,
    r_squared: bool,
    adjusted_r_squared: bool,
    within_r_squared: bool,
    adjusted_within_r_squared: bool,
    root_mean_squared_error: bool,
    gaussian_log_likelihood: bool,
    gaussian_null_log_likelihood: bool,
}

#[derive(Debug, Clone, Copy)]
struct PsdCertificate {
    minimum_normalized_pivot: f64,
    rank: usize,
}

/// Fit and certify HC1 robust covariance through the owner's declared
/// ordinary or categorical projection semantics. Probability weights are
/// accepted here; importance weights remain outside the pinned checkpoint.
pub fn fit_prepared_model_robust_with_loaded_moments(
    prepared: &PreparedOlsData,
    config: PreparedOlsRobustConfig,
    rows: Option<super::LoadedOlsRows>,
) -> Result<CertifiedPreparedOlsRobust> {
    if prepared.uses_categorical_preparation() {
        fit_inference_view(prepared, config, rows, true)
    } else {
        fit_inference_view(
            &crate::prepared_fit_view::OrdinaryPrepared(prepared),
            config,
            rows,
            false,
        )
    }
}

fn fit_inference_view<P: PreparedOlsInferenceView + Sync + ?Sized>(
    prepared: &P,
    config: PreparedOlsRobustConfig,
    rows: Option<super::LoadedOlsRows>,
    defer_dof: bool,
) -> Result<CertifiedPreparedOlsRobust> {
    validate_config(config)?;
    let (fit, scores) = super::prepared_fit::fit_prepared_ols_coefficients_view_map_for_inference_with_loaded_moments(
        prepared, config.fit, rows)?;
    let early_dof = if defer_dof {
        None
    } else {
        Some(prepared.inference_compute_absorbed_dof(config.absorbed_dof)?)
    };
    ThreadTeam::new(config.fit.execution.threads).install(|| {
        let absorbed_dof = match early_dof {
            Some(report) => report,
            None => prepared.inference_compute_absorbed_dof(config.absorbed_dof)?,
        };
        let summary =
            prepared.inference_absorbed_dof_summary(&absorbed_dof, config.absorbed_dof)?;
        let mut reservation = SystemPreparedInferenceReservation;
        let components = compose_view_internal(
            prepared,
            fit,
            summary,
            config,
            &mut reservation,
            RobustPerturbations::default(),
            scores,
        )?;
        Ok(components.with_absorbed_dof(absorbed_dof))
    })?
}

// Keep the legacy workspace and ownership boundaries explicit during migration.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn compose_internal<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsData,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: CategoricalTopologyDofReport,
    config: PreparedOlsRobustConfig,
    reservation: &mut R,
    perturbations: RobustPerturbations,
    score_scratch: PreparedOlsScoreProduct,
) -> Result<CertifiedPreparedOlsRobust> {
    let ordinary = crate::prepared_fit_view::OrdinaryPrepared(prepared);
    let prepared = &ordinary;
    let absorbed_summary =
        prepared.inference_absorbed_dof_summary(&absorbed_dof, config.absorbed_dof)?;
    let components = compose_view_internal(
        prepared,
        fit,
        absorbed_summary,
        config,
        reservation,
        perturbations,
        score_scratch,
    )?;
    Ok(components.with_absorbed_dof(absorbed_dof))
}

// Keep the legacy workspace and ownership boundaries explicit during migration.
#[allow(clippy::too_many_arguments)]
fn compose_view_internal<P: PreparedOlsInferenceView + ?Sized, R: PreparedInferenceReservation>(
    prepared: &P,
    mut fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofSummary,
    config: PreparedOlsRobustConfig,
    reservation: &mut R,
    perturbations: RobustPerturbations,
    score_scratch: PreparedOlsScoreProduct,
) -> Result<PreparedOlsRobustComponents> {
    let phase_timing = crate::diagnostics::phase_timing_enabled();
    let total_started = Instant::now();
    let phase_started = Instant::now();
    validate_fit_and_absorbed_dof_view(prepared, &fit, absorbed_dof)?;
    let (weights, weight_view, reference_weight_error) =
        build_reference_weight_report_view(prepared, &fit, config.certificate_tolerance, true)?;
    let mut dof = build_residual_dof_view(prepared, &fit, absorbed_dof, weights)?;
    if perturbations.residual_dof {
        dof.residual_dof = dof
            .residual_dof
            .checked_add(1)
            .ok_or_else(|| allocation("injected prepared robust residual DoF overflows"))?;
    }
    certify_dof_identity(prepared, &fit, weights, dof)?;

    let tss = independent_outcome_sum(
        &prepared.fit_sample().outcome,
        weight_view,
        dof.has_structural_intercept,
    )?;
    let within_tss = if !prepared.inference_has_explicit_absorbed_effects() {
        tss
    } else {
        fit.initial_absorbed_outcome_sum_of_squares()
    };
    let rss = independent_residual_sum(&fit, weight_view)?;
    validate_sum_of_squares(rss, tss, within_tss)?;
    let mut statistics = build_statistics(rss, tss, within_tss, weights, dof)?;
    if perturbations.rss {
        statistics.rss += 0.25;
    }
    if perturbations.tss {
        statistics.tss += 0.25;
    }
    if perturbations.within_tss {
        statistics.within_tss += 0.25;
    }
    let setup_statistics_seconds = phase_started.elapsed().as_secs_f64();

    let phase_started = Instant::now();
    let mut score_design = build_prepared_ols_score_design(
        prepared,
        &mut fit,
        weight_view,
        config.certificate_tolerance,
        reservation,
        score_scratch,
    )?;
    if perturbations.score_design && score_design.finite_rank() > 0 {
        score_design.certificate.passed = false;
    }
    let score_design_seconds = phase_started.elapsed().as_secs_f64();

    let phase_started = Instant::now();
    let PreparedOlsRobustCovarianceBuild {
        mut covariance,
        dense_certificate_meat,
    } = build_covariance(
        &fit,
        &score_design,
        weight_view,
        weights,
        dof,
        perturbations.meat_multiplier,
        reservation,
    )?;
    if perturbations.hc1_correction {
        covariance.hc1_correction += 0.25;
    }
    if perturbations.covariance && !covariance.values.is_empty() {
        covariance.values[0] += 0.25;
    }
    if perturbations.covariance_symmetry && covariance.dimension() >= 2 {
        covariance.values[1] += 0.25;
    }
    if perturbations.covariance_indefinite && !covariance.values.is_empty() {
        covariance.values[0] = -covariance.values[0].abs().max(1.0);
    }
    if perturbations.standard_error && !covariance.standard_errors.is_empty() {
        covariance.standard_errors[0] += 0.25;
    }
    let covariance_seconds = phase_started.elapsed().as_secs_f64();

    let phase_started = Instant::now();
    let covariance_formula = certify_covariance_formula(
        &fit,
        &score_design,
        weight_view,
        weights,
        dof,
        &covariance,
        dense_certificate_meat.as_deref(),
        reservation,
    )?;
    let covariance_certificate_seconds = phase_started.elapsed().as_secs_f64();
    let phase_started = Instant::now();
    let psd = certify_psd(
        covariance.values(),
        covariance.dimension(),
        config.certificate_tolerance,
        reservation,
    )?;
    let psd_seconds = phase_started.elapsed().as_secs_f64();
    let phase_started = Instant::now();
    let wald = build_robust_wald(
        prepared,
        &fit,
        &covariance,
        dof,
        config.certificate_tolerance,
        reservation,
    )?;
    let wald_seconds = phase_started.elapsed().as_secs_f64();
    let phase_started = Instant::now();
    statistics.f_statistic = wald.statistic;
    if perturbations.f_statistic {
        perturb_optional(&mut statistics.f_statistic)?;
    }
    if perturbations.r_squared {
        perturb_optional(&mut statistics.r_squared)?;
    }
    if perturbations.adjusted_r_squared {
        perturb_optional(&mut statistics.adjusted_r_squared)?;
    }
    if perturbations.within_r_squared {
        perturb_optional(&mut statistics.within_r_squared)?;
    }
    if perturbations.adjusted_within_r_squared {
        perturb_optional(&mut statistics.adjusted_within_r_squared)?;
    }
    if perturbations.root_mean_squared_error {
        statistics.root_mean_squared_error += 0.25;
    }
    if perturbations.gaussian_log_likelihood {
        perturb_optional(&mut statistics.gaussian_log_likelihood)?;
    }
    if perturbations.gaussian_null_log_likelihood {
        perturb_optional(&mut statistics.gaussian_null_log_likelihood)?;
    }
    let statistic_error =
        certify_statistic_identities(statistics, weights, dof, rss, tss, within_tss, wald)?;
    let score_certificate = score_design.certificate();
    let passed = reference_weight_error <= config.certificate_tolerance
        && score_certificate.passed
        && covariance_formula.0 <= config.certificate_tolerance
        && covariance_formula.1 <= config.certificate_tolerance
        && covariance_formula.2 <= config.certificate_tolerance
        && covariance_formula.3 <= config.certificate_tolerance
        && wald.solve_relative_infinity_norm <= config.certificate_tolerance
        && statistic_error <= config.certificate_tolerance;
    let certificate = PreparedOlsRobustCertificate {
        reference_weight_sum_relative_error: reference_weight_error,
        score_design_left_inverse_relative_infinity_norm: score_certificate
            .left_inverse_relative_infinity_norm,
        score_design_right_inverse_relative_infinity_norm: score_certificate
            .right_inverse_relative_infinity_norm,
        intercept_coordinate_relative_infinity_norm: score_certificate
            .intercept_coordinate_relative_infinity_norm,
        hc1_correction_relative_error: covariance_formula.0,
        sandwich_formula_relative_infinity_norm: covariance_formula.1,
        covariance_symmetry_relative_infinity_norm: covariance_formula.2,
        standard_error_relative_infinity_norm: covariance_formula.3,
        minimum_normalized_psd_pivot: psd.minimum_normalized_pivot,
        psd_rank: psd.rank,
        wald_solve_relative_infinity_norm: wald.solve_relative_infinity_norm,
        statistic_formula_relative_infinity_norm: statistic_error,
        passed,
    };
    if !certificate.passed {
        return Err(invalid_data(
            "prepared robust covariance or statistic certificate did not pass",
        ));
    }
    let finalize_seconds = phase_started.elapsed().as_secs_f64();
    if phase_timing {
        eprintln!(
            "FEREG_PHASE|robust|setup_statistics={setup_statistics_seconds:.9}|score_design={score_design_seconds:.9}|covariance={covariance_seconds:.9}|covariance_certificate={covariance_certificate_seconds:.9}|psd={psd_seconds:.9}|wald={wald_seconds:.9}|finalize={finalize_seconds:.9}|total={:.9}",
            total_started.elapsed().as_secs_f64(),
        );
    }
    // `score_design` owns the only N-by-kept-coordinate inference matrix. It
    // goes out of scope here and is intentionally absent from the result.
    Ok(PreparedOlsRobustComponents {
        absorbed_dof: (),
        metadata: (),
        fit,
        weights,
        dof,
        statistics,
        covariance,
        certificate,
    })
}

fn validate_config(config: PreparedOlsRobustConfig) -> Result<()> {
    if !config.certificate_tolerance.is_finite()
        || config.certificate_tolerance <= 0.0
        || config.certificate_tolerance >= 1.0
    {
        return Err(invalid_argument(
            "prepared robust certificate tolerance must be finite and in (0,1)",
        ));
    }
    Ok(())
}

fn certify_dof_identity<P: PreparedOlsInferenceView + ?Sized>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
) -> Result<()> {
    let expected_finite_intercept =
        usize::from(prepared.fit_report_intercept() && !prepared.fit_has_absorbed_intercept());
    let expected_finite_model = fit
        .finite_rank()
        .checked_sub(expected_finite_intercept)
        .ok_or_else(|| invalid_data("prepared robust finite model DoF underflowed"))?;
    let expected_model_rank = dof
        .finite_rank
        .checked_add(dof.absorbed_rank)
        .ok_or_else(|| allocation("prepared robust model rank overflows"))?;
    let expected_residual = weights
        .effective_observations
        .checked_sub(expected_model_rank)
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid_data("prepared robust residual DoF is not positive"))?;
    if dof.finite_rank != fit.finite_rank()
        || dof.finite_model_dof != expected_finite_model
        || dof.model_rank != expected_model_rank
        || dof.residual_dof != expected_residual
        || dof.has_structural_intercept != prepared.fit_structural_intercept()
    {
        return Err(invalid_data(
            "prepared robust rank and residual-DoF identity did not pass",
        ));
    }
    Ok(())
}

fn build_covariance<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: &CertifiedPreparedOlsScoreDesign<'_>,
    weight_view: ReferenceWeightView<'_>,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    perturb_meat_multiplier: bool,
    reservation: &mut R,
) -> Result<PreparedOlsRobustCovarianceBuild> {
    let dimension = fit.bread().dimension();
    let square = checked_square(
        dimension,
        "prepared robust covariance dimensions overflow usize",
    )?;
    if fit.residuals().len() != scores.nrows() || fit.bread().values().len() != square {
        return Err(invalid_argument(
            "prepared robust covariance dimensions are inconsistent",
        ));
    }
    let hc1_correction = exact_integer_as_f64(weights.effective_observations as u128)?
        / exact_integer_as_f64(dof.residual_dof as u128)?;
    if !hc1_correction.is_finite() || hc1_correction <= 0.0 {
        return Err(invalid_data("prepared robust HC1 correction is invalid"));
    }
    if let Some(dense) = scores.dense_view() {
        return build_dense_covariance(
            fit,
            dense,
            weight_view,
            weights,
            hc1_correction,
            perturb_meat_multiplier,
            reservation,
        );
    }
    let values = reduce_serial_influence_covariance(
        scores,
        OlsRobustSerialScoreMultiplier {
            residuals: fit.residuals(),
            weights: weight_view,
            weight_kind: weights.kind,
            perturb_production: perturb_meat_multiplier,
            // Preserve the accepted OLS evaluation order exactly:
            // `(sqrt(HC1) * weight_root) * residual`.
            correction_root: hc1_correction.sqrt(),
        },
        fit.bread().values(),
        reservation,
    )?;
    let standard_errors = standard_errors(&values, dimension, reservation)?;
    Ok(PreparedOlsRobustCovarianceBuild {
        covariance: PreparedOlsRobustCovariance {
            hc1_correction,
            values,
            standard_errors,
        },
        dense_certificate_meat: None,
    })
}

fn build_dense_covariance<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: PreparedOlsDenseScoreView<'_>,
    weight_view: ReferenceWeightView<'_>,
    weights: PreparedOlsReferenceWeightReport,
    hc1_correction: f64,
    perturb_meat_multiplier: bool,
    reservation: &mut R,
) -> Result<PreparedOlsRobustCovarianceBuild> {
    let dimension = fit.bread().dimension();
    let _square = checked_square(
        dimension,
        "prepared robust dense covariance dimensions overflow usize",
    )?;
    if scores.reported_dimension() != dimension || fit.residuals().len() != scores.nrows() {
        return Err(invalid_argument(
            "prepared robust dense covariance dimensions are inconsistent",
        ));
    }
    if dimension == 0 {
        return Ok(PreparedOlsRobustCovarianceBuild {
            covariance: PreparedOlsRobustCovariance {
                hc1_correction,
                values: Vec::new(),
                standard_errors: Vec::new(),
            },
            dense_certificate_meat: Some(Vec::new()),
        });
    }
    let meats = reduce_dense_score_meats(
        scores,
        OlsRobustScoreMultiplier {
            residuals: fit.residuals(),
            weights: weight_view,
            weight_kind: weights.kind,
            perturb_production: perturb_meat_multiplier,
        },
        FixedBlockReductionPolicy {
            rows_per_block: ROBUST_REDUCTION_ROWS,
            maximum_blocks: ROBUST_MAX_BLOCKS,
            partial_budget_bytes: ROBUST_PARTIAL_BUDGET_BYTES,
        },
        None,
        reservation,
    )?;
    let values = bread_meat_bread(
        fit.bread().values(),
        &meats.production,
        dimension,
        SandwichCorrectionPolicy::Scale(hc1_correction),
        SandwichProductMode::OrdinaryUpperTriangle,
        reservation,
    )?;
    let standard_errors = standard_errors(&values, dimension, reservation)?;
    Ok(PreparedOlsRobustCovarianceBuild {
        covariance: PreparedOlsRobustCovariance {
            hc1_correction,
            values,
            standard_errors,
        },
        dense_certificate_meat: Some(meats.certificate),
    })
}

/// Independent sandwich reconstruction. Dense production uses its
/// upper-triangle fixed-block meat and an ordinary final sandwich; the dense
/// certificate uses the separately stored reverse-block-order meat and this
/// compensated sandwich. Scratch-backed production retains the original
/// observation-influence path and is certified by a fresh meat traversal.
#[allow(clippy::too_many_arguments)]
fn certify_covariance_formula<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: &CertifiedPreparedOlsScoreDesign<'_>,
    weight_view: ReferenceWeightView<'_>,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    covariance: &PreparedOlsRobustCovariance,
    dense_certificate_meat: Option<&[f64]>,
    reservation: &mut R,
) -> Result<(f64, f64, f64, f64)> {
    let dimension = covariance.dimension();
    let square = checked_square(
        dimension,
        "prepared robust certificate dimensions overflow usize",
    )?;
    if covariance.values.len() != square || fit.bread().values().len() != square {
        return Err(invalid_argument(
            "prepared robust certificate covariance dimensions are inconsistent",
        ));
    }
    let expected_correction = exact_integer_as_f64(weights.effective_observations as u128)?
        / exact_integer_as_f64(dof.residual_dof as u128)?;
    let correction_error = relative_difference(
        covariance.hc1_correction,
        expected_correction,
        expected_correction.max(1.0),
    )?;
    let recomputed_meat;
    let meat = if let Some(meat) = dense_certificate_meat {
        if meat.len() != square {
            return Err(invalid_argument(
                "prepared robust fused certificate meat dimensions are inconsistent",
            ));
        }
        meat
    } else if let Some(dense) = scores.dense_view() {
        recomputed_meat =
            build_dense_certificate_meat(fit, dense, weight_view, weights.kind, reservation)?;
        &recomputed_meat
    } else {
        recomputed_meat =
            build_scratch_certificate_meat(fit, scores, weight_view, weights.kind, reservation)?;
        &recomputed_meat
    };
    let mut left_product = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust certificate bread-meat product",
    )?;
    for row in 0..dimension {
        for column in 0..dimension {
            let mut value = 0.0_f64;
            let mut correction = 0.0_f64;
            for inner in 0..dimension {
                compensated_add(
                    fit.bread().values()[row * dimension + inner]
                        * meat[inner * dimension + column],
                    &mut value,
                    &mut correction,
                )?;
            }
            left_product[row * dimension + column] = value + correction;
        }
    }
    let mut sandwich_error = 0.0_f64;
    let mut symmetry_error = 0.0_f64;
    let mut standard_error = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = 1.0_f64;
        let mut symmetry_row_error = 0.0_f64;
        let mut symmetry_row_scale = 1.0_f64;
        for column in 0..dimension {
            let mut expected = 0.0_f64;
            let mut correction = 0.0_f64;
            for inner in 0..dimension {
                compensated_add(
                    left_product[row * dimension + inner]
                        * fit.bread().values()[column * dimension + inner],
                    &mut expected,
                    &mut correction,
                )?;
            }
            expected = expected_correction * (expected + correction);
            let actual = covariance.values[row * dimension + column];
            row_error += (actual - expected).abs();
            row_scale += actual.abs().max(expected.abs());
            let transpose = covariance.values[column * dimension + row];
            symmetry_row_error += (actual - transpose).abs();
            symmetry_row_scale += actual.abs().max(transpose.abs());
        }
        sandwich_error = sandwich_error.max(row_error / row_scale);
        symmetry_error = symmetry_error.max(symmetry_row_error / symmetry_row_scale);
        let diagonal = covariance.values[row * dimension + row];
        if !diagonal.is_finite() || diagonal < 0.0 {
            return Err(invalid_data(
                "prepared robust certificate covariance diagonal is invalid",
            ));
        }
        standard_error = standard_error.max(relative_difference(
            covariance.standard_errors[row],
            diagonal.sqrt(),
            covariance.standard_errors[row]
                .abs()
                .max(diagonal.sqrt().abs())
                .max(1.0),
        )?);
    }
    if [sandwich_error, symmetry_error, standard_error]
        .iter()
        .all(|value| value.is_finite())
    {
        Ok((
            correction_error,
            sandwich_error,
            symmetry_error,
            standard_error,
        ))
    } else {
        Err(invalid_data(
            "prepared robust covariance certificate norm is not finite",
        ))
    }
}

fn build_dense_certificate_meat<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: PreparedOlsDenseScoreView<'_>,
    weight_view: ReferenceWeightView<'_>,
    weight_kind: Option<WeightKind>,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let dimension = scores.reported_dimension();
    let square = checked_square(
        dimension,
        "prepared robust dense certificate meat dimensions overflow usize",
    )?;
    if fit.residuals().len() != scores.nrows() || fit.bread().dimension() != dimension {
        return Err(invalid_argument(
            "prepared robust dense certificate meat dimensions are inconsistent",
        ));
    }
    if dimension == 0 {
        return Ok(Vec::new());
    }
    let partial_stride = square
        .checked_add(dimension)
        .ok_or_else(|| allocation("prepared robust dense certificate partial stride overflows"))?;
    let chunks = robust_reduction_chunks(scores.nrows(), partial_stride)?;
    let partial_elements = chunks.checked_mul(partial_stride).ok_or_else(|| {
        allocation("prepared robust dense certificate meat partials overflow usize")
    })?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve prepared robust dense certificate meat partials",
    )?;
    crate::parallel::stage_try_for_each(
        partials.par_chunks_mut(partial_stride).enumerate(),
        scores.nrows() as u128 * square as u128,
        |(chunk, partial)| -> Result<()> {
            let (meat, score) = partial.split_at_mut(square);
            let rows_per_chunk = scores.nrows() / chunks;
            let remainder = scores.nrows() % chunks;
            let start = chunk * rows_per_chunk + chunk.min(remainder);
            let end = start + rows_per_chunk + usize::from(chunk < remainder);
            for row in start..end {
                let scale = meat_root_multiplier(weight_kind, weight_view.value(row)?)?
                    * fit.residuals()[row];
                scores.fill_reported_row(row, score)?;
                for score_value in score.iter_mut() {
                    *score_value *= scale;
                    if !score_value.is_finite() {
                        return Err(invalid_data(
                            "prepared robust dense certificate score is not finite",
                        ));
                    }
                }
                for left in 0..dimension {
                    let left_value = score[left];
                    for right in left..dimension {
                        meat[left * dimension + right] += left_value * score[right];
                    }
                }
            }
            if meat.iter().all(|value| value.is_finite()) {
                Ok(())
            } else {
                Err(invalid_data(
                    "prepared robust dense certificate meat partial is not finite",
                ))
            }
        },
    )?;
    let mut meat = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust dense certificate meat",
    )?;
    let mut corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust dense certificate meat corrections",
    )?;
    for partial in partials.chunks_exact(partial_stride) {
        for left in 0..dimension {
            for right in left..dimension {
                let position = left * dimension + right;
                compensated_add(
                    partial[position],
                    &mut meat[position],
                    &mut corrections[position],
                )?;
            }
        }
    }
    for left in 0..dimension {
        for right in left..dimension {
            let position = left * dimension + right;
            meat[position] += corrections[position];
            if !meat[position].is_finite() {
                return Err(invalid_data(
                    "prepared robust dense certificate meat is not finite",
                ));
            }
            meat[right * dimension + left] = meat[position];
        }
    }
    Ok(meat)
}

fn build_scratch_certificate_meat<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: &CertifiedPreparedOlsScoreDesign<'_>,
    weight_view: ReferenceWeightView<'_>,
    weight_kind: Option<WeightKind>,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let dimension = fit.bread().dimension();
    let square = checked_square(
        dimension,
        "prepared robust scratch certificate meat dimensions overflow usize",
    )?;
    let mut meat = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust certificate meat",
    )?;
    let mut corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust certificate meat corrections",
    )?;
    let mut score = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve prepared robust certificate score row",
    )?;
    for row in 0..scores.nrows() {
        let scale =
            meat_root_multiplier(weight_kind, weight_view.value(row)?)? * fit.residuals()[row];
        scores.fill_reported_row(row, &mut score)?;
        for score_value in &mut score {
            *score_value *= scale;
            if !score_value.is_finite() {
                return Err(invalid_data(
                    "prepared robust certificate score is not finite",
                ));
            }
        }
        for left in 0..dimension {
            let start = left * dimension;
            compensated_add_scaled_row(
                score[left],
                &score,
                &mut meat[start..start + dimension],
                &mut corrections[start..start + dimension],
            )?;
        }
    }
    for (value, correction) in meat.iter_mut().zip(corrections) {
        *value += correction;
    }
    Ok(meat)
}

fn certify_psd<R: PreparedInferenceReservation>(
    covariance: &[f64],
    dimension: usize,
    tolerance: f64,
    reservation: &mut R,
) -> Result<PsdCertificate> {
    let square = checked_square(dimension, "prepared robust PSD dimensions overflow usize")?;
    if covariance.len() != square {
        return Err(invalid_argument(
            "prepared robust PSD covariance dimensions are inconsistent",
        ));
    }
    if dimension == 0 {
        return Ok(PsdCertificate {
            minimum_normalized_pivot: 0.0,
            rank: 0,
        });
    }
    let mut work = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust PSD factor",
    )?;
    work.copy_from_slice(covariance);
    let scale = (0..dimension)
        .map(|index| covariance[index * dimension + index].abs())
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);
    let threshold = tolerance * scale;
    let mut rank = 0_usize;
    let mut minimum = f64::INFINITY;
    for pivot_index in 0..dimension {
        let mut pivot_position = pivot_index;
        let mut pivot = work[pivot_index * dimension + pivot_index];
        for candidate in (pivot_index + 1)..dimension {
            let diagonal = work[candidate * dimension + candidate];
            if diagonal > pivot {
                pivot = diagonal;
                pivot_position = candidate;
            }
        }
        if pivot_position != pivot_index {
            for column in 0..dimension {
                work.swap(
                    pivot_index * dimension + column,
                    pivot_position * dimension + column,
                );
            }
            for row in 0..dimension {
                work.swap(
                    row * dimension + pivot_index,
                    row * dimension + pivot_position,
                );
            }
            pivot = work[pivot_index * dimension + pivot_index];
        }
        minimum = minimum.min(pivot / scale);
        if pivot < -threshold {
            return Err(invalid_data(
                "prepared robust covariance is materially indefinite",
            ));
        }
        if pivot <= threshold {
            let mut remaining_max = 0.0_f64;
            for row in pivot_index..dimension {
                for column in pivot_index..dimension {
                    remaining_max = remaining_max.max(work[row * dimension + column].abs());
                }
            }
            if remaining_max > threshold {
                return Err(invalid_data(
                    "prepared robust covariance has a singular non-PSD remainder",
                ));
            }
            break;
        }
        rank = rank
            .checked_add(1)
            .ok_or_else(|| allocation("prepared robust PSD rank overflows"))?;
        for row in (pivot_index + 1)..dimension {
            for column in row..dimension {
                let updated = work[row * dimension + column]
                    - work[pivot_index * dimension + row] * work[pivot_index * dimension + column]
                        / pivot;
                if !updated.is_finite() {
                    return Err(invalid_data(
                        "prepared robust PSD Schur complement is not finite",
                    ));
                }
                work[row * dimension + column] = updated;
                work[column * dimension + row] = updated;
            }
        }
    }
    if minimum == f64::INFINITY {
        minimum = 0.0;
    }
    Ok(PsdCertificate {
        minimum_normalized_pivot: minimum,
        rank,
    })
}

fn build_robust_wald<P: PreparedOlsInferenceView + ?Sized, R: PreparedInferenceReservation>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    covariance: &PreparedOlsRobustCovariance,
    dof: PreparedOlsResidualDofReport,
    tolerance: f64,
    reservation: &mut R,
) -> Result<WaldCalculation> {
    let slope_dimension = dof.finite_model_dof;
    if slope_dimension == 0 {
        return Ok(WaldCalculation {
            statistic: None,
            solve_relative_infinity_norm: 0.0,
        });
    }
    let dimension = covariance.dimension();
    let mut positions = reserved_filled(
        reservation,
        slope_dimension,
        0_usize,
        "unable to reserve prepared robust Wald slope coordinates",
    )?;
    let mut count = 0_usize;
    for (position, coordinate) in fit.bread().coordinates().iter().enumerate() {
        let column = prepared
            .fit_model()
            .columns
            .get(coordinate.generated_column_index)
            .ok_or_else(|| invalid_argument("prepared robust Wald coordinate is out of range"))?;
        if matches!(column.kind, PreparedOlsColumnKind::Intercept) {
            continue;
        }
        if count >= slope_dimension {
            return Err(invalid_argument(
                "prepared robust Wald slope count exceeds model DoF",
            ));
        }
        positions[count] = position;
        count += 1;
    }
    if count != slope_dimension {
        return Err(invalid_argument(
            "prepared robust Wald slope count differs from model DoF",
        ));
    }
    calculate_wald(
        fit,
        &covariance.values,
        dimension,
        &positions,
        tolerance,
        reservation,
    )
}

fn certify_statistic_identities(
    statistics: PreparedOlsFitStatistics,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    rss: f64,
    tss: f64,
    within_tss: f64,
    wald: WaldCalculation,
) -> Result<f64> {
    let n = exact_integer_as_f64(weights.effective_observations as u128)?;
    let df_r = exact_integer_as_f64(dof.residual_dof as u128)?;
    let centered_dof = weights
        .effective_observations
        .checked_sub(usize::from(dof.has_structural_intercept));
    let within_baseline_dof = dof
        .residual_dof
        .checked_add(dof.finite_model_dof)
        .ok_or_else(|| allocation("prepared robust within baseline DoF overflows"))?;
    let scale = rss.max(tss).max(within_tss).max(1.0);
    let mut norm = wald.solve_relative_infinity_norm;
    norm = norm.max((rss - tss).max(0.0) / scale);
    norm = norm.max((rss - within_tss).max(0.0) / scale);
    norm = norm.max((within_tss - tss).max(0.0) / scale);
    norm = norm.max(relative_difference(statistics.rss, rss, scale)?);
    norm = norm.max(relative_difference(statistics.tss, tss, scale)?);
    norm = norm.max(relative_difference(
        statistics.within_tss,
        within_tss,
        scale,
    )?);
    norm = norm.max(optional_identity_error(
        statistics.r_squared,
        tss > 0.0,
        |value| relative_difference((1.0 - value) * tss, rss, scale),
    )?);
    norm = norm.max(optional_identity_error(
        statistics.adjusted_r_squared,
        tss > 0.0 && centered_dof.is_some_and(|value| value > 0),
        |value| {
            let baseline = exact_integer_as_f64(centered_dof.unwrap_or_default() as u128)?;
            relative_difference((1.0 - value) * (df_r / baseline), rss / tss, 1.0)
        },
    )?);
    norm = norm.max(optional_identity_error(
        statistics.within_r_squared,
        within_tss > 0.0,
        |value| relative_difference((1.0 - value) * within_tss, rss, scale),
    )?);
    norm = norm.max(optional_identity_error(
        statistics.adjusted_within_r_squared,
        within_tss > 0.0 && within_baseline_dof > 0,
        |value| {
            let baseline = exact_integer_as_f64(within_baseline_dof as u128)?;
            relative_difference((1.0 - value) * (df_r / baseline), rss / within_tss, 1.0)
        },
    )?);
    norm = norm.max(relative_difference(
        statistics.root_mean_squared_error * statistics.root_mean_squared_error,
        rss / df_r,
        (rss / df_r).max(1.0),
    )?);
    norm = norm.max(optional_pair_error(statistics.f_statistic, wald.statistic)?);
    norm = norm.max(optional_identity_error(
        statistics.gaussian_log_likelihood,
        rss > 0.0,
        |value| gaussian_identity_error(value, rss, n),
    )?);
    norm = norm.max(optional_identity_error(
        statistics.gaussian_null_log_likelihood,
        within_tss > 0.0,
        |value| gaussian_identity_error(value, within_tss, n),
    )?);
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(invalid_data(
            "prepared robust statistic certificate norm is not finite",
        ))
    }
}

fn optional_identity_error<F>(actual: Option<f64>, defined: bool, identity: F) -> Result<f64>
where
    F: FnOnce(f64) -> Result<f64>,
{
    match (actual, defined) {
        (Some(value), true) if value.is_finite() => identity(value),
        (None, false) => Ok(0.0),
        _ => Ok(1.0),
    }
}

fn optional_pair_error(actual: Option<f64>, expected: Option<f64>) -> Result<f64> {
    match (actual, expected) {
        (Some(actual), Some(expected)) => {
            relative_difference(actual, expected, actual.abs().max(expected.abs()).max(1.0))
        }
        (None, None) => Ok(0.0),
        _ => Ok(1.0),
    }
}

fn gaussian_identity_error(likelihood: f64, sum_of_squares: f64, observations: f64) -> Result<f64> {
    let per_observation = likelihood / observations;
    let actual = -2.0 * per_observation - 1.0 - (2.0 * std::f64::consts::PI).ln();
    let expected = (sum_of_squares / observations).ln();
    relative_difference(actual, expected, actual.abs().max(expected.abs()).max(1.0))
}

fn meat_root_multiplier(kind: Option<WeightKind>, reference_weight: f64) -> Result<f64> {
    let value = match kind {
        None => 1.0,
        Some(WeightKind::Frequency) => reference_weight.sqrt(),
        Some(WeightKind::Analytic | WeightKind::Probability) => reference_weight,
        Some(WeightKind::Importance) => {
            return Err(unsupported(
                "prepared robust covariance does not support importance weights",
            ));
        }
    };
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(invalid_data(
            "prepared robust meat multiplier is not finite and positive",
        ))
    }
}

fn robust_reduction_chunks(nrows: usize, partial_stride: usize) -> Result<usize> {
    if nrows == 0 || partial_stride == 0 {
        return Err(invalid_argument(
            "prepared robust reduction dimensions are empty",
        ));
    }
    crate::parallel::reduction_blocks(
        nrows,
        ROBUST_REDUCTION_ROWS,
        ROBUST_MAX_BLOCKS,
        partial_stride,
        ROBUST_PARTIAL_BUDGET_BYTES,
    )
}

fn perturb_optional(value: &mut Option<f64>) -> Result<()> {
    let value = value.as_mut().ok_or_else(|| {
        invalid_argument("injected prepared robust perturbation requires a defined statistic")
    })?;
    *value += 0.25;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::error::ErrorCode;
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind,
        ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId,
        TermComponent, TermId, TermOrder, TimeSeriesState,
    };

    use super::super::prepared_fit::fit_prepared_ols_coefficients_view_map_for_inference_with_cache_limit;
    use super::super::{
        prepare_model, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
        OlsRequestV1,
    };
    use super::*;

    #[derive(Clone, Copy)]
    enum TestWeights<'a> {
        Unit,
        Analytic(&'a [f64]),
        Probability(&'a [f64]),
        Frequency(&'a [u32]),
    }

    #[test]
    fn outcome_overlap_preserves_robust_statistics_and_covariance() {
        let n = 16_385;
        let group: Vec<_> = (0..n).map(|r| (r % 71) as f64).collect();
        let x: Vec<_> = (0..n).map(|r| (r as f64 * 0.7).sin()).collect();
        let y: Vec<_> = (0..n)
            .map(|r| 1.7 * x[r] + group[r] + 0.1 * (r as f64).cos())
            .collect();
        let raw = (0..n)
            .map(|r| 0.125 + (r % 17) as f64 / 11.0)
            .collect::<Vec<_>>();
        let frequency = (0..n).map(|r| 1 + (r % 7) as u32).collect::<Vec<_>>();
        for kind in [
            TestWeights::Unit,
            TestWeights::Analytic(&raw),
            TestWeights::Probability(&raw),
            TestWeights::Frequency(&frequency),
        ] {
            let prepared = prepared(&y, &[x.clone()], Some(&group), true, kind);
            let mut config = PreparedOlsRobustConfig::default();
            config.fit.execution.threads = crate::ThreadConfig::from_requested(16, true).unwrap();
            let actual =
                fit_prepared_model_robust_with_loaded_moments(&prepared, config, None).unwrap();
            let (fit, scratch) =
                fit_prepared_ols_coefficients_view_map_for_inference(&prepared, config.fit)
                    .unwrap();
            let absorbed =
                compute_canonical_prepared_ols_absorbed_dof(&prepared, config.absorbed_dof)
                    .unwrap();
            let expected = ThreadTeam::new(config.fit.execution.threads)
                .install(|| {
                    compose_internal(
                        &prepared,
                        fit,
                        absorbed,
                        config,
                        &mut SystemPreparedInferenceReservation,
                        RobustPerturbations::default(),
                        scratch,
                    )
                })
                .unwrap()
                .unwrap();
            assert_eq!(actual.statistics, expected.statistics);
            assert_eq!(actual.covariance, expected.covariance);
            assert_eq!(actual.certificate, expected.certificate);
            assert_eq!(actual.dof, expected.dof);
            assert_eq!(actual.fit.coefficients(), expected.fit.coefficients());
        }
    }

    #[test]
    fn retained_low_scores_preserve_rank_selection_and_reject_corrupt_columns() {
        let n = 96;
        let group = (0..n).map(|row| (row / 12) as f64).collect::<Vec<_>>();
        let x = (0..n)
            .map(|row| (row as f64 * 0.7).sin())
            .collect::<Vec<_>>();
        let z = (0..n)
            .map(|row| (row as f64 * 0.3).cos())
            .collect::<Vec<_>>();
        let columns = [
            x.clone(),
            x.iter().map(|value| 2.0 * value).collect(),
            z.clone(),
        ];
        let y = (0..n)
            .map(|row| 1.7 * x[row] - 0.4 * z[row] + group[row] + 0.1 * (row as f64).cos())
            .collect::<Vec<_>>();
        let weights = (0..n).map(|row| 0.5 + (row % 5) as f64).collect::<Vec<_>>();
        let frequencies = (0..n).map(|row| 1 + (row % 3) as u32).collect::<Vec<_>>();
        for kind in [
            TestWeights::Unit,
            TestWeights::Analytic(&weights),
            TestWeights::Frequency(&frequencies),
            TestWeights::Probability(&weights),
        ] {
            let prepared = prepared(&y, &columns, Some(&group), true, kind);
            let mut config = PreparedOlsRobustConfig::default();
            config.fit.execution.mode = super::super::PreparedOlsExecutionMode::Low;
            config.fit.execution.poolsize = 1;
            let (fit, scratch) =
                fit_prepared_ols_coefficients_view_map_for_inference(&prepared, config.fit)
                    .unwrap();
            assert_eq!(fit.finite_rank(), 2);
            let PreparedOlsScoreProduct::Low(scratch) = scratch else {
                panic!("original low-memory columns retained")
            };
            assert!(scratch.columns() > fit.finite_rank());
            assert_eq!(fit.execution().scratch_bytes, scratch.bytes());
            let absorbed =
                compute_canonical_prepared_ols_absorbed_dof(&prepared, config.absorbed_dof)
                    .unwrap();
            let cached = compose_internal(
                &prepared,
                fit,
                absorbed,
                config,
                &mut CountingReservation::default(),
                RobustPerturbations::default(),
                PreparedOlsScoreProduct::Low(scratch),
            )
            .unwrap();
            let mut reference_config = config;
            reference_config.fit.execution.mode = super::super::PreparedOlsExecutionMode::Fast;
            let mut fit = fit_prepared_ols_coefficients_view_map_for_inference_with_cache_limit(
                &prepared,
                reference_config.fit,
                0,
            )
            .unwrap();
            let scores = take_prepared_ols_score_product(&mut fit, None).unwrap();
            let absorbed =
                compute_canonical_prepared_ols_absorbed_dof(&prepared, config.absorbed_dof)
                    .unwrap();
            let reference = compose_internal(
                &prepared,
                fit,
                absorbed,
                reference_config,
                &mut CountingReservation::default(),
                RobustPerturbations::default(),
                scores,
            )
            .unwrap();
            assert_eq!(cached.fit().coefficients(), reference.fit().coefficients());
            for (actual, expected) in cached
                .covariance()
                .values()
                .iter()
                .zip(reference.covariance().values())
            {
                assert!((actual - expected).abs() <= 1e-12 * (1.0 + expected.abs()));
            }
            let (fit, scratch) =
                fit_prepared_ols_coefficients_view_map_for_inference(&prepared, config.fit)
                    .unwrap();
            let selected = fit.kept_generated_columns()[0];
            let PreparedOlsScoreProduct::Low(mut scratch) = scratch else {
                panic!("retained low score rows")
            };
            scratch
                .patch_row_major_block(selected, 1, &vec![0.0; n])
                .unwrap();
            let absorbed =
                compute_canonical_prepared_ols_absorbed_dof(&prepared, config.absorbed_dof)
                    .unwrap();
            let error = compose_internal(
                &prepared,
                fit,
                absorbed,
                config,
                &mut CountingReservation::default(),
                RobustPerturbations::default(),
                PreparedOlsScoreProduct::Low(scratch),
            )
            .unwrap_err();
            assert!(
                error.message.contains("score-design certificate"),
                "{error:?}"
            );
        }
    }

    #[derive(Default)]
    struct CountingReservation {
        calls: usize,
        fail_at: Option<usize>,
        undersize: bool,
        labels: Vec<&'static str>,
    }

    impl PreparedInferenceReservation for CountingReservation {
        fn reserve<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            label: &'static str,
        ) -> Result<()> {
            self.calls += 1;
            self.labels.push(label);
            if self.fail_at == Some(self.calls) {
                return Err(allocation("injected prepared robust reservation failure"));
            }
            if !self.undersize {
                target
                    .try_reserve_exact(elements)
                    .map_err(|_| allocation("test prepared robust reservation failed"))?;
            }
            Ok(())
        }
    }

    #[test]
    fn accepted_score_handoff_rejects_missing_products() {
        let y = [2.0, 4.1, 5.9, 8.0, 10.1, 11.8];
        let prepared = prepared(
            &y,
            &[vec![1., 2., 3., 4., 5., 6.]],
            None,
            true,
            TestWeights::Unit,
        );
        for mode in [
            super::super::PreparedOlsExecutionMode::Fast,
            super::super::PreparedOlsExecutionMode::Low,
        ] {
            let mut config = PreparedOlsFitConfig::default();
            config.execution.mode = mode;
            let mut fit = fit_prepared_ols_coefficients_view_map(&prepared, config).unwrap();
            let error = take_prepared_ols_score_product(&mut fit, None).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert!(error.message.contains("score product"));
        }
    }

    #[test]
    fn inference_reuses_accepted_rows_without_another_solver_workspace() {
        let rows = 48;
        let groups = (0..rows).map(|r| (r / 6) as f64).collect::<Vec<_>>();
        let x = (0..rows)
            .map(|r| (r as f64 * 0.31).sin())
            .collect::<Vec<_>>();
        let y = (0..rows)
            .map(|r| 1.7 * x[r] + groups[r] + 0.07 * (r as f64 * 1.13).sin())
            .collect::<Vec<_>>();
        let weights = (0..rows).map(|r| 0.5 + (r % 5) as f64).collect::<Vec<_>>();
        for kind in [TestWeights::Unit, TestWeights::Analytic(&weights)] {
            let prepared = prepared(&y, &[x.clone()], Some(&groups), true, kind);
            for mode in [
                super::super::PreparedOlsExecutionMode::Fast,
                super::super::PreparedOlsExecutionMode::Low,
            ] {
                let mut config = PreparedOlsRobustConfig::default();
                config.fit.execution.mode = mode;
                config.fit.execution.poolsize = 1;
                config.fit.absorb.retain_coefficients = true;
                config.fit.absorb.solver = crate::fe::FeSolverKind::Map;
                let expected =
                    fit_prepared_model_robust_with_loaded_moments(&prepared, config, None).unwrap();
                let (fit, scores) =
                    fit_prepared_ols_coefficients_view_map_for_inference(&prepared, config.fit)
                        .unwrap();
                let initial_peak = fit.solver_workspace_peak();
                let absorbed =
                    compute_canonical_prepared_ols_absorbed_dof(&prepared, config.absorbed_dof)
                        .unwrap();
                // A different projection request cannot create another solver:
                // these scores belong to the fit's already certified rows.
                config.fit.absorb.solver = crate::fe::FeSolverKind::Lsmr;
                let result = compose_internal(
                    &prepared,
                    fit,
                    absorbed,
                    config,
                    &mut CountingReservation::default(),
                    RobustPerturbations::default(),
                    scores,
                )
                .unwrap();
                assert_eq!(result.fit().solver_workspace_peak(), initial_peak);
                assert_eq!(result.fit().coefficients(), expected.fit().coefficients());
                assert_eq!(result.fit().residuals(), expected.fit().residuals());
                assert_eq!(result.covariance(), expected.covariance());
                assert!(result.certificate().passed);
                assert_eq!(
                    result.fit().recovered_fe_coefficients(),
                    expected.fit().recovered_fe_coefficients()
                );
            }
        }
    }

    #[test]
    fn strided_score_storage_matches_packed_robust_inference() {
        let group = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
        let x1 = vec![-2.0, 0.5, 1.0, 3.0, -1.5, 2.0, 0.25, 4.0];
        let x2 = vec![1.0, -3.0, 0.75, 2.5, 4.0, -0.5, 3.0, 1.5];
        let weights = vec![0.5, 2.0, 1.5, 0.75, 3.0, 1.0, 2.5, 1.25];
        let frequencies = vec![1_u32, 2, 3, 1, 2, 1, 3, 2];
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
            TestWeights::Frequency(&frequencies),
        ] {
            let prepared = prepared(
                &y,
                &[x1.clone(), x2.clone()],
                Some(&group),
                true,
                test_weights,
            );
            let config = PreparedOlsRobustConfig::default();
            let packed = fit_prepared_model_robust_with_loaded_moments(&prepared, config, None)
                .expect("packed robust fit");
            let mut fit = fit_prepared_ols_coefficients_view_map_for_inference_with_cache_limit(
                &prepared, config.fit, 0,
            )
            .expect("strided coefficient fit");
            let scores = take_prepared_ols_score_product(&mut fit, None).unwrap();
            let absorbed =
                compute_canonical_prepared_ols_absorbed_dof(&prepared, config.absorbed_dof)
                    .expect("strided absorbed DoF");
            let mut reservation = CountingReservation::default();
            let strided = compose_internal(
                &prepared,
                fit,
                absorbed,
                config,
                &mut reservation,
                RobustPerturbations::default(),
                scores,
            )
            .expect("strided robust fit");
            assert!(strided.certificate().passed);
            assert_eq!(packed.fit().coefficients(), strided.fit().coefficients());
            for (actual, expected) in strided
                .covariance()
                .values()
                .iter()
                .zip(packed.covariance().values())
            {
                assert_close(*actual, *expected, 2e-11);
            }
        }
    }

    fn legacy_scratch_covariance(
        fit: &CertifiedPreparedOlsFit,
        scores: &CertifiedPreparedOlsScoreDesign<'_>,
        weight_view: ReferenceWeightView<'_>,
        weights: PreparedOlsReferenceWeightReport,
        dof: PreparedOlsResidualDofReport,
    ) -> Vec<f64> {
        let dimension = fit.bread().dimension();
        let square = dimension * dimension;
        let hc1_correction = weights.effective_observations as f64 / dof.residual_dof as f64;
        let mut values = vec![0.0_f64; square];
        let mut corrections = vec![0.0_f64; square];
        let mut influence = vec![0.0_f64; dimension];
        let mut score = vec![0.0_f64; dimension];
        for row in 0..scores.nrows() {
            scores
                .fill_reported_row(row, &mut score)
                .expect("legacy scratch score row");
            let reference_weight = weight_view.value(row).expect("legacy reference weight");
            let root_multiplier = meat_root_multiplier(weights.kind, reference_weight)
                .expect("legacy meat multiplier");
            let scale = hc1_correction.sqrt() * root_multiplier * fit.residuals()[row];
            for (left, influence_value) in influence.iter_mut().enumerate() {
                let mut product = 0.0_f64;
                let mut correction = 0.0_f64;
                for (right, score_value) in score.iter().copied().enumerate() {
                    compensated_add(
                        fit.bread().values()[left * dimension + right] * score_value,
                        &mut product,
                        &mut correction,
                    )
                    .expect("legacy scratch bread product");
                }
                *influence_value = scale * (product + correction);
            }
            for left in 0..dimension {
                for right in left..dimension {
                    let position = left * dimension + right;
                    compensated_add(
                        influence[left] * influence[right],
                        &mut values[position],
                        &mut corrections[position],
                    )
                    .expect("legacy scratch influence accumulation");
                }
            }
        }
        for left in 0..dimension {
            for right in left..dimension {
                let position = left * dimension + right;
                values[position] += corrections[position];
                values[right * dimension + left] = values[position];
            }
        }
        values
    }

    #[test]
    fn low_memory_robust_covariance_routes_through_bit_exact_shared_serial_reducer() {
        let groups = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0];
        let regressors = vec![
            vec![-2.0, 0.0, 1.0, -1.0, 2.0, 4.0, -3.0, 1.5, 5.0],
            vec![0.25, -1.0, 2.5, 3.0, -0.5, 1.0, 4.0, -2.0, 0.75],
        ];
        let analytic = [0.5, 2.0, 1.5, 3.0, 0.75, 4.0, 2.5, 1.25, 3.5];
        let outcome = (0..groups.len())
            .map(|row| {
                1.7 * regressors[0][row] - 0.35 * regressors[1][row]
                    + groups[row]
                    + 0.07 * (row as f64).sin()
            })
            .collect::<Vec<_>>();
        let prepared = prepared(
            &outcome,
            &regressors,
            Some(&groups),
            true,
            TestWeights::Analytic(&analytic),
        );
        let mut config = PreparedOlsRobustConfig::default();
        config.fit.execution.mode = super::super::PreparedOlsExecutionMode::Low;
        config.fit.execution.poolsize = 1;
        let (mut fit, scores) =
            fit_prepared_ols_coefficients_view_map_for_inference(&prepared, config.fit)
                .expect("low-memory coefficient fit");
        assert_eq!(
            fit.execution().mode,
            super::super::PreparedOlsExecutionMode::Low
        );
        let absorbed = compute_canonical_prepared_ols_absorbed_dof(&prepared, config.absorbed_dof)
            .expect("low-memory absorbed DoF");
        let absorbed_summary = crate::prepared_fit_view::OrdinaryPrepared(&prepared)
            .inference_absorbed_dof_summary(&absorbed, config.absorbed_dof)
            .expect("low-memory absorbed DoF summary");
        let (weights, weight_view, _) =
            build_reference_weight_report_view(&prepared, &fit, config.certificate_tolerance, true)
                .expect("low-memory reference weights");
        let dof = build_residual_dof_view(&prepared, &fit, absorbed_summary, weights)
            .expect("low-memory residual DoF");
        let mut reservation = CountingReservation::default();
        let scores = build_prepared_ols_score_design(
            &prepared,
            &mut fit,
            weight_view,
            config.certificate_tolerance,
            &mut reservation,
            scores,
        )
        .expect("low-memory score design");
        assert!(scores.dense_view().is_none());
        let expected = legacy_scratch_covariance(&fit, &scores, weight_view, weights, dof);
        let label_start = reservation.labels.len();
        let actual = build_covariance(
            &fit,
            &scores,
            weight_view,
            weights,
            dof,
            false,
            &mut reservation,
        )
        .expect("shared low-memory covariance");
        assert_eq!(actual.covariance.values(), expected);
        let covariance_labels = &reservation.labels[label_start..];
        assert!(covariance_labels.contains(&"unable to reserve serial sandwich covariance"));
        assert!(
            covariance_labels.contains(&"unable to reserve serial sandwich covariance corrections")
        );
        assert!(covariance_labels.contains(&"unable to reserve serial sandwich influence row"));
        assert!(covariance_labels.contains(&"unable to reserve serial sandwich score row"));
        assert!(!covariance_labels.contains(&"unable to reserve dense sandwich partials"));
    }

    #[test]
    fn unit_aweight_pweight_and_scaled_pweight_follow_pinned_hc1_contract() {
        let y = [1.2, 2.4, 2.8, 4.9, 5.1, 7.4, 7.8, 9.7];
        let x = vec![vec![-3.0, -1.5, -0.5, 0.5, 1.5, 2.5, 4.0, 6.0]];
        let weights = [0.25, 1.5, 0.75, 3.0, 2.0, 0.5, 4.0, 1.25];
        let analytic = fit_prepared_model_robust_with_loaded_moments(
            &prepared(&y, &x, None, true, TestWeights::Analytic(&weights)),
            PreparedOlsRobustConfig::default(),
            None,
        )
        .expect("analytic robust inference");
        let probability = fit_prepared_model_robust_with_loaded_moments(
            &prepared(&y, &x, None, true, TestWeights::Probability(&weights)),
            PreparedOlsRobustConfig::default(),
            None,
        )
        .expect("probability robust inference");
        let scaled = weights.map(|weight| weight * 1e120);
        let probability_scaled = fit_prepared_model_robust_with_loaded_moments(
            &prepared(&y, &x, None, true, TestWeights::Probability(&scaled)),
            PreparedOlsRobustConfig::default(),
            None,
        )
        .expect("scaled probability robust inference");
        assert_slice_close(
            analytic.covariance().values(),
            probability.covariance().values(),
            2e-10,
        );
        assert_slice_close(
            probability.covariance().values(),
            probability_scaled.covariance().values(),
            2e-10,
        );
        assert_eq!(probability.weights().effective_observations, y.len());
        assert_close(
            probability.covariance().hc1_correction(),
            y.len() as f64 / probability.dof().residual_dof as f64,
            1e-12,
        );
        assert!(analytic.certificate().passed);
        assert!(probability.certificate().passed);
    }

    #[test]
    fn compact_fweights_equal_literal_row_expansion_including_full_covariance() {
        let y = [1.0, 2.2, 4.1, 5.3, 8.0];
        let x = vec![vec![-2.0, -0.5, 1.0, 2.5, 5.0]];
        let frequency = [1_u32, 3, 2, 4, 1];
        let compact = fit_prepared_model_robust_with_loaded_moments(
            &prepared(&y, &x, None, true, TestWeights::Frequency(&frequency)),
            PreparedOlsRobustConfig::default(),
            None,
        )
        .expect("compact frequency robust inference");
        let mut expanded_y = Vec::new();
        let mut expanded_x = Vec::new();
        for row in 0..y.len() {
            for _ in 0..frequency[row] {
                expanded_y.push(y[row]);
                expanded_x.push(x[0][row]);
            }
        }
        let expanded = fit_prepared_model_robust_with_loaded_moments(
            &prepared(&expanded_y, &[expanded_x], None, true, TestWeights::Unit),
            PreparedOlsRobustConfig::default(),
            None,
        )
        .expect("expanded unit robust inference");
        assert_eq!(
            compact.weights().effective_observations,
            expanded.weights().effective_observations
        );
        assert_eq!(compact.dof().residual_dof, expanded.dof().residual_dof);
        assert_slice_close(
            compact.covariance().values(),
            expanded.covariance().values(),
            2e-10,
        );
        assert_slice_close(
            compact.covariance().standard_errors(),
            expanded.covariance().standard_errors(),
            2e-10,
        );
    }

    #[test]
    fn reported_and_hidden_intercepts_share_slope_block_and_exact_fit_is_psd_singular() {
        let groups = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0];
        let x = vec![vec![-2.0, 0.0, 1.0, -1.0, 2.0, 4.0, -3.0, 1.5, 5.0]];
        let y = (0..x[0].len())
            .map(|row| 2.5 * x[0][row] + 3.0 * groups[row])
            .collect::<Vec<_>>();
        let reported = fit_prepared_model_robust_with_loaded_moments(
            &prepared(&y, &x, Some(&groups), true, TestWeights::Unit),
            PreparedOlsRobustConfig::default(),
            None,
        )
        .expect("reported exact robust fit");
        let hidden = fit_prepared_model_robust_with_loaded_moments(
            &prepared(&y, &x, Some(&groups), false, TestWeights::Unit),
            PreparedOlsRobustConfig::default(),
            None,
        )
        .expect("hidden exact robust fit");
        assert_eq!(reported.covariance().dimension(), 2);
        assert_eq!(hidden.covariance().dimension(), 1);
        assert_close(
            reported.covariance().values()[0],
            hidden.covariance().values()[0],
            1e-12,
        );
        assert!(reported
            .statistics()
            .f_statistic
            .is_none_or(|value| value.is_finite() && value >= 0.0));
        assert!(hidden
            .statistics()
            .f_statistic
            .is_none_or(|value| value.is_finite() && value >= 0.0));
        assert!(reported
            .covariance()
            .values()
            .iter()
            .all(|value| value.is_finite()));
    }

    #[test]
    fn singular_psd_is_certified_while_material_indefiniteness_is_fatal() {
        let mut reservation = CountingReservation::default();
        let singular = certify_psd(&[1.0, 1.0, 1.0, 1.0], 2, 1e-10, &mut reservation)
            .expect("rank-one PSD certificate");
        assert_eq!(singular.rank, 1);
        assert!(singular.minimum_normalized_pivot.abs() <= 1e-10);

        let mut reservation = CountingReservation::default();
        let error = certify_psd(&[1.0, 2.0, 2.0, 1.0], 2, 1e-10, &mut reservation)
            .expect_err("indefinite covariance must fail");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn singular_psd_slope_covariance_makes_robust_wald_undefined() {
        let y = [1.0, 2.4, 2.8, 5.1, 4.9, 7.5, 8.1, 9.6];
        let regressors = vec![
            vec![-3.0, -1.0, 0.5, 1.0, 2.0, 3.0, 4.5, 6.0],
            vec![2.0, -1.0, 1.5, 0.0, 3.0, -2.0, 2.5, 4.0],
        ];
        let prepared = prepared(&y, &regressors, None, true, TestWeights::Unit);
        let mut result = fit_prepared_model_robust_with_loaded_moments(
            &prepared,
            PreparedOlsRobustConfig::default(),
            None,
        )
        .expect("two-slope robust fit");
        assert_eq!(result.dof().finite_model_dof, 2);
        let dimension = result.covariance.dimension();
        result.covariance.values[0] = 1.0;
        result.covariance.values[1] = 1.0;
        result.covariance.values[dimension] = 1.0;
        result.covariance.values[dimension + 1] = 1.0;
        let mut reservation = CountingReservation::default();
        let wald = build_robust_wald(
            &prepared,
            result.fit(),
            result.covariance(),
            result.dof(),
            1e-10,
            &mut reservation,
        )
        .expect("singular PSD Wald decision");
        assert!(wald.statistic.is_none());
        assert_eq!(wald.solve_relative_infinity_norm, 0.0);
    }

    #[test]
    fn rank_zero_no_x_path_has_no_score_matrix_and_certifies() {
        let y = [2.0, 3.5, 1.0, 4.0, 5.0, 2.5];
        let hidden = fit_prepared_model_robust_with_loaded_moments(
            &prepared(&y, &[], None, false, TestWeights::Unit),
            PreparedOlsRobustConfig::default(),
            None,
        )
        .expect("hidden no-X robust inference");
        assert_eq!(hidden.fit().finite_rank(), 0);
        assert_eq!(hidden.covariance().dimension(), 0);
        assert_eq!(hidden.certificate().psd_rank, 0);
        assert!(hidden.statistics().f_statistic.is_none());

        let reported = fit_prepared_model_robust_with_loaded_moments(
            &prepared(&y, &[], None, true, TestWeights::Unit),
            PreparedOlsRobustConfig::default(),
            None,
        )
        .expect("reported intercept-only robust inference");
        assert_eq!(reported.fit().finite_rank(), 0);
        assert_eq!(reported.covariance().dimension(), 1);
        assert!(reported.certificate().passed);

        let mut low_config = PreparedOlsRobustConfig::default();
        low_config.fit.execution.mode = super::super::PreparedOlsExecutionMode::Low;
        let hidden_low = fit_prepared_model_robust_with_loaded_moments(
            &prepared(&y, &[], None, false, TestWeights::Unit),
            low_config,
            None,
        )
        .expect("low-memory hidden no-X robust inference");
        assert_eq!(
            hidden_low.fit().execution().mode,
            super::super::PreparedOlsExecutionMode::Low
        );
        assert_eq!(hidden_low.covariance().dimension(), 0);
        assert!(hidden_low.certificate().passed);
    }

    #[test]
    fn every_robust_reservation_and_mutation_fails_closed() {
        let groups = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0];
        let x = vec![vec![-2.0, 0.0, 1.0, -1.0, 2.0, 4.0, -3.0, 1.5, 5.0]];
        let y = (0..x[0].len())
            .map(|row| 1.7 * x[0][row] + groups[row] + 0.07 * (row as f64).sin())
            .collect::<Vec<_>>();
        let analytic = [0.5, 2.0, 1.5, 3.0, 0.75, 4.0, 2.5, 1.25, 3.5];
        let prepared = prepared(
            &y,
            &x,
            Some(&groups),
            true,
            TestWeights::Analytic(&analytic),
        );
        let mut baseline = CountingReservation::default();
        compose_for_test(&prepared, &mut baseline, RobustPerturbations::default())
            .expect("baseline robust composition");
        assert!(baseline.calls > 10);
        for fail_at in 1..=baseline.calls {
            let mut reservation = CountingReservation {
                fail_at: Some(fail_at),
                ..CountingReservation::default()
            };
            let error =
                compose_for_test(&prepared, &mut reservation, RobustPerturbations::default())
                    .expect_err("injected robust allocation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "fail_at={fail_at}");
        }
        let mut undersized = CountingReservation {
            undersize: true,
            ..CountingReservation::default()
        };
        let error = compose_for_test(&prepared, &mut undersized, RobustPerturbations::default())
            .expect_err("undersized robust reservation must fail");
        assert_eq!(error.code, ErrorCode::Allocation);

        for perturbation in [
            RobustPerturbations {
                score_design: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                meat_multiplier: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                residual_dof: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                hc1_correction: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                covariance: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                covariance_symmetry: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                covariance_indefinite: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                standard_error: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                rss: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                tss: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                within_tss: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                f_statistic: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                r_squared: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                adjusted_r_squared: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                within_r_squared: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                adjusted_within_r_squared: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                root_mean_squared_error: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                gaussian_log_likelihood: true,
                ..RobustPerturbations::default()
            },
            RobustPerturbations {
                gaussian_null_log_likelihood: true,
                ..RobustPerturbations::default()
            },
        ] {
            let mut reservation = CountingReservation::default();
            let error = compose_for_test(&prepared, &mut reservation, perturbation)
                .expect_err("injected robust corruption must fail");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    fn compose_for_test<R: PreparedInferenceReservation>(
        prepared: &PreparedOlsData,
        reservation: &mut R,
        perturbations: RobustPerturbations,
    ) -> Result<CertifiedPreparedOlsRobust> {
        let config = PreparedOlsRobustConfig::default();
        let (fit, scores) =
            fit_prepared_ols_coefficients_view_map_for_inference(prepared, config.fit)?;
        let absorbed = compute_canonical_prepared_ols_absorbed_dof(prepared, config.absorbed_dof)?;
        compose_internal(
            prepared,
            fit,
            absorbed,
            config,
            reservation,
            perturbations,
            scores,
        )
    }

    fn prepared(
        outcome: &[f64],
        regressors: &[Vec<f64>],
        fixed_effect: Option<&[f64]>,
        include_intercept: bool,
        weights: TestWeights<'_>,
    ) -> PreparedOlsData {
        let (request, input) = prepared_inputs(
            outcome,
            regressors,
            fixed_effect,
            include_intercept,
            weights,
        );
        prepare_model(&request, &[], &input, false).expect("prepared robust test case")
    }

    fn prepared_inputs(
        outcome: &[f64],
        regressors: &[Vec<f64>],
        fixed_effect: Option<&[f64]>,
        include_intercept: bool,
        weights: TestWeights<'_>,
    ) -> (OlsRequestV1, LoadedOlsInput) {
        let mut sources = Vec::new();
        let mut terms = Vec::new();
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
            let one_based = u32::try_from(index + 1).expect("small source");
            let source = SourceEntry {
                id: SourceId::new(one_based).expect("source"),
                canonical_name: format!("x{one_based}"),
            };
            terms.push(ExpandedTerm {
                id: TermId::new(one_based).expect("term"),
                order: TermOrder::new(one_based).expect("term order"),
                coefficient_slot: CoefficientSlot::new(one_based).expect("coefficient slot"),
                raw_kind: RawTermKind::Variable,
                whole_column_omitted: false,
                canonical_token: format!("c.x{one_based}"),
                components: vec![TermComponent {
                    order: ComponentOrder::new(1).expect("component"),
                    source: source.id,
                    kind: ComponentKind::Continuous,
                    level_state: LevelState::Absent,
                    level_value: 0.0,
                    base: MetadataFlag::False,
                    omitted: MetadataFlag::Unavailable,
                    raw_operator: "c".to_owned(),
                    time_series_state: TimeSeriesState::None,
                    raw_time_series_operator: String::new(),
                }],
            });
            bindings.push(OlsInputBinding {
                stata_var_position: one_based + 2,
                role: OlsInputRole::NumericSource(source.id),
            });
            numeric_sources.push((source.id, loaded(values)));
            sources.push(source);
        }
        let absorbed_terms = fixed_effect.map_or_else(Vec::new, |codes| {
            let one_based = u32::try_from(sources.len() + 1).expect("small FE source");
            let source = SourceEntry {
                id: SourceId::new(one_based).expect("FE source"),
                canonical_name: "g".to_owned(),
            };
            bindings.push(OlsInputBinding {
                stata_var_position: one_based + 2,
                role: OlsInputRole::NumericSource(source.id),
            });
            numeric_sources.push((source.id, loaded(codes)));
            sources.push(source.clone());
            vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorbed term"),
                intercept: true,
                categorical_sources: vec![source.id],
                slope_sources: Vec::new(),
                label: "g".to_owned(),
                saved_name: String::new(),
            }]
        });
        let weight_position = u32::try_from(bindings.len() + 1).expect("weight binding");
        let (analytic_weight, frequency_weight, probability_weight) = match weights {
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
                (None, None, Some(loaded(values)))
            }
            TestWeights::Frequency(values) => {
                bindings.push(OlsInputBinding {
                    stata_var_position: weight_position,
                    role: OlsInputRole::FrequencyWeight,
                });
                let values = values
                    .iter()
                    .map(|value| f64::from(*value))
                    .collect::<Vec<_>>();
                (None, Some(loaded(&values)), None)
            }
        };
        (
            OlsRequestV1 {
                model: CompiledTermGraph { sources, terms },
                absorbed_terms,
                bindings,
                include_intercept,
                keep_singletons: true,
            },
            LoadedOlsInput {
                candidate: vec![true; outcome.len()],
                outcome: loaded(outcome),
                numeric_sources,
                analytic_weight,
                frequency_weight,
                probability_weight,
            },
        )
    }

    fn loaded(values: &[f64]) -> LoadedNumericColumn {
        LoadedNumericColumn {
            values: values.to_vec(),
            missing: vec![false; values.len()],
        }
    }

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        let scale = actual.abs().max(expected.abs()).max(1.0);
        assert!(
            (actual - expected).abs() <= tolerance * scale,
            "actual={actual:.17e}, expected={expected:.17e}, tolerance={tolerance:.3e}"
        );
    }

    fn assert_slice_close(actual: &[f64], expected: &[f64], tolerance: f64) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_close(*actual, *expected, tolerance);
        }
    }
}
