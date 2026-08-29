use crate::data::WeightKind;
use crate::error::{ErrorCode, FeregError, Result};
use crate::parallel::ThreadTeam;
use crate::prepared_categorical_data::PreparedCategoricalOlsData;
use crate::topology_dof::CategoricalTopologyDofReport;
use rayon::prelude::*;

#[cfg(test)]
use super::prepared_fit::fit_prepared_ols_coefficients_view_map;
use super::prepared_fit::fit_prepared_ols_coefficients_view_map_for_inference;
use super::prepared_inference_view::{PreparedOlsAbsorbedDofSummary, PreparedOlsInferenceView};
use super::prepared_scores::{
    build_prepared_ols_score_design, reserved_filled, CertifiedPreparedOlsScoreDesign,
    PreparedInferenceReservation, PreparedOlsDenseScoreView, SystemPreparedInferenceReservation,
};
use super::prepared_unadjusted::{
    build_reference_weight_report_view, build_residual_dof_view, build_statistics, checked_square,
    compensated_add, exact_integer_as_f64, relative_difference, validate_fit_and_absorbed_dof_view,
    validate_sum_of_squares, weighted_mean, weighted_sum_of_squares, ReferenceWeightView,
};
use super::{
    compute_prepared_ols_absorbed_dof, CertifiedPreparedOlsFit, PreparedOlsAbsorbedDofPolicy,
    PreparedOlsAbsorbedDofReport, PreparedOlsBreadCoordinate, PreparedOlsColumnKind,
    PreparedOlsData, PreparedOlsFitConfig, PreparedOlsFitStatistics, PreparedOlsFitStatus,
    PreparedOlsReferenceWeightReport, PreparedOlsResidualDofReport,
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

/// Same-call certified fit, absorbed rank, HC1 covariance, and fit statistics.
/// The observation-by-kept-coordinate score design is destroyed before this
/// value is returned.
#[derive(Debug, PartialEq)]
pub struct CertifiedPreparedOlsRobust {
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofReport,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsRobustCovariance,
    certificate: PreparedOlsRobustCertificate,
}

impl CertifiedPreparedOlsRobust {
    #[must_use]
    pub const fn fit(&self) -> &CertifiedPreparedOlsFit {
        &self.fit
    }

    #[must_use]
    pub const fn absorbed_dof(&self) -> &PreparedOlsAbsorbedDofReport {
        &self.absorbed_dof
    }

    #[must_use]
    pub const fn weights(&self) -> PreparedOlsReferenceWeightReport {
        self.weights
    }

    #[must_use]
    pub const fn dof(&self) -> PreparedOlsResidualDofReport {
        self.dof
    }

    #[must_use]
    pub const fn statistics(&self) -> PreparedOlsFitStatistics {
        self.statistics
    }

    #[must_use]
    pub const fn covariance(&self) -> &PreparedOlsRobustCovariance {
        &self.covariance
    }

    #[must_use]
    pub fn covariance_coordinates(&self) -> &[PreparedOlsBreadCoordinate] {
        self.fit.bread().coordinates()
    }

    #[must_use]
    pub const fn certificate(&self) -> PreparedOlsRobustCertificate {
        self.certificate
    }
}

/// Same-call categorical-interaction fit, absorbed rank, HC1 covariance, and
/// fit statistics. The categorical topology DoF report remains exact and is
/// retained without changing the legacy result or plugin wire identity.
#[derive(Debug, PartialEq)]
pub struct CertifiedPreparedCategoricalOlsRobust {
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: CategoricalTopologyDofReport,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsRobustCovariance,
    certificate: PreparedOlsRobustCertificate,
}

impl CertifiedPreparedCategoricalOlsRobust {
    #[must_use]
    pub const fn fit(&self) -> &CertifiedPreparedOlsFit {
        &self.fit
    }

    #[must_use]
    pub const fn absorbed_dof(&self) -> &CategoricalTopologyDofReport {
        &self.absorbed_dof
    }

    #[must_use]
    pub const fn weights(&self) -> PreparedOlsReferenceWeightReport {
        self.weights
    }

    #[must_use]
    pub const fn dof(&self) -> PreparedOlsResidualDofReport {
        self.dof
    }

    #[must_use]
    pub const fn statistics(&self) -> PreparedOlsFitStatistics {
        self.statistics
    }

    #[must_use]
    pub const fn covariance(&self) -> &PreparedOlsRobustCovariance {
        &self.covariance
    }

    #[must_use]
    pub fn covariance_coordinates(&self) -> &[PreparedOlsBreadCoordinate] {
        self.fit.bread().coordinates()
    }

    #[must_use]
    pub const fn certificate(&self) -> PreparedOlsRobustCertificate {
        self.certificate
    }
}

#[derive(Debug, PartialEq)]
struct PreparedOlsRobustComponents {
    fit: CertifiedPreparedOlsFit,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsRobustCovariance,
    certificate: PreparedOlsRobustCertificate,
}

struct PreparedOlsRobustCovarianceBuild {
    covariance: PreparedOlsRobustCovariance,
    /// Independently accumulated raw score meat from the fused dense pass.
    /// The production covariance is accumulated from bread-transformed
    /// influences, so this remains an algebraically distinct certificate path.
    dense_certificate_meat: Option<Vec<f64>>,
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
struct WaldCalculation {
    statistic: Option<f64>,
    solve_relative_infinity_norm: f64,
}

#[derive(Debug, Clone, Copy)]
struct PsdCertificate {
    minimum_normalized_pivot: f64,
    rank: usize,
}

/// Fit and certify HC1 robust covariance through the prepared, strict-MAP path.
/// Probability weights are accepted here; importance weights remain outside
/// the pinned checkpoint.
pub fn fit_prepared_ols_robust_map(
    prepared: &PreparedOlsData,
    config: PreparedOlsRobustConfig,
) -> Result<CertifiedPreparedOlsRobust> {
    validate_config(config)?;
    let fit = fit_prepared_ols_coefficients_view_map_for_inference(prepared, config.fit)?;
    let absorbed_dof = compute_prepared_ols_absorbed_dof(prepared, config.absorbed_dof)?;
    ThreadTeam::new(config.fit.execution.threads).install(|| {
        let mut reservation = SystemPreparedInferenceReservation;
        compose_internal(
            prepared,
            fit,
            absorbed_dof,
            config,
            &mut reservation,
            RobustPerturbations::default(),
        )
    })?
}

/// Fit and certify categorical-interaction HC1 covariance through the same
/// numerical composition used by the legacy prepared path.
pub fn fit_prepared_categorical_ols_robust_map(
    prepared: &PreparedCategoricalOlsData,
    config: PreparedOlsRobustConfig,
) -> Result<CertifiedPreparedCategoricalOlsRobust> {
    validate_config(config)?;
    let fit = fit_prepared_ols_coefficients_view_map_for_inference(prepared, config.fit)?;
    let absorbed_dof = prepared.inference_compute_absorbed_dof(config.absorbed_dof)?;
    let absorbed_summary =
        prepared.inference_absorbed_dof_summary(&absorbed_dof, config.absorbed_dof)?;
    let components = ThreadTeam::new(config.fit.execution.threads).install(|| {
        let mut reservation = SystemPreparedInferenceReservation;
        compose_view_internal(
            prepared,
            fit,
            absorbed_summary,
            config,
            &mut reservation,
            RobustPerturbations::default(),
        )
    })??;
    Ok(CertifiedPreparedCategoricalOlsRobust {
        fit: components.fit,
        absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}

fn compose_internal<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsData,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofReport,
    config: PreparedOlsRobustConfig,
    reservation: &mut R,
    perturbations: RobustPerturbations,
) -> Result<CertifiedPreparedOlsRobust> {
    let absorbed_summary =
        prepared.inference_absorbed_dof_summary(&absorbed_dof, config.absorbed_dof)?;
    let components = compose_view_internal(
        prepared,
        fit,
        absorbed_summary,
        config,
        reservation,
        perturbations,
    )?;
    Ok(CertifiedPreparedOlsRobust {
        fit: components.fit,
        absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}

fn compose_view_internal<P: PreparedOlsInferenceView + ?Sized, R: PreparedInferenceReservation>(
    prepared: &P,
    mut fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofSummary,
    config: PreparedOlsRobustConfig,
    reservation: &mut R,
    perturbations: RobustPerturbations,
) -> Result<PreparedOlsRobustComponents> {
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

    let outcome_mean = weighted_mean(&prepared.fit_sample().outcome, weight_view)?;
    let center = if dof.has_structural_intercept {
        outcome_mean
    } else {
        0.0
    };
    let tss = weighted_sum_of_squares(&prepared.fit_sample().outcome, weight_view, center)?;
    let within_tss = if !prepared.inference_has_explicit_absorbed_effects() {
        tss
    } else {
        fit.initial_absorbed_outcome_sum_of_squares()
    };
    let rss = weighted_sum_of_squares(fit.residuals(), weight_view, 0.0)?;
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

    let mut score_design = build_prepared_ols_score_design(
        prepared,
        &mut fit,
        weight_view,
        config.fit.absorb,
        config.certificate_tolerance,
        reservation,
    )?;
    if perturbations.score_design && score_design.finite_rank() > 0 {
        score_design.certificate.passed = false;
    }

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
    let psd = certify_psd(
        covariance.values(),
        covariance.dimension(),
        config.certificate_tolerance,
        reservation,
    )?;
    let wald = build_robust_wald(
        prepared,
        &fit,
        &covariance,
        dof,
        config.certificate_tolerance,
        reservation,
    )?;
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
    // `score_design` owns the only N-by-kept-coordinate inference matrix. It
    // goes out of scope here and is intentionally absent from the result.
    Ok(PreparedOlsRobustComponents {
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
    scores: &CertifiedPreparedOlsScoreDesign,
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
    let mut values = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust covariance",
    )?;
    let mut corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust covariance corrections",
    )?;
    let mut influence = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve prepared robust influence row",
    )?;
    let mut score = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve prepared robust reported score row",
    )?;
    for row in 0..scores.nrows() {
        scores.fill_reported_row(row, &mut score)?;
        let reference_weight = weight_view.value(row)?;
        let root_multiplier = if perturb_meat_multiplier {
            match weights.kind {
                Some(WeightKind::Analytic | WeightKind::Probability) => reference_weight.sqrt(),
                Some(WeightKind::Frequency) => reference_weight,
                _ => 1.0,
            }
        } else {
            meat_root_multiplier(weights.kind, reference_weight)?
        };
        let scale = hc1_correction.sqrt() * root_multiplier * fit.residuals()[row];
        if !scale.is_finite() {
            return Err(invalid_data(
                "prepared robust influence scale is not finite",
            ));
        }
        for (left, influence_value) in influence.iter_mut().enumerate() {
            let mut product = 0.0_f64;
            let mut correction = 0.0_f64;
            for (right, score_value) in score.iter().copied().enumerate() {
                compensated_add(
                    fit.bread().values()[left * dimension + right] * score_value,
                    &mut product,
                    &mut correction,
                )?;
            }
            *influence_value = scale * (product + correction);
            if !influence_value.is_finite() {
                return Err(invalid_data(
                    "prepared robust influence row contains a nonfinite value",
                ));
            }
        }
        for left in 0..dimension {
            for right in left..dimension {
                let position = left * dimension + right;
                compensated_add(
                    influence[left] * influence[right],
                    &mut values[position],
                    &mut corrections[position],
                )?;
            }
        }
    }
    for left in 0..dimension {
        for right in left..dimension {
            let position = left * dimension + right;
            values[position] += corrections[position];
            if !values[position].is_finite() {
                return Err(invalid_data(
                    "prepared robust covariance contains a nonfinite value",
                ));
            }
            values[right * dimension + left] = values[position];
        }
    }
    let mut standard_errors = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve prepared robust standard errors",
    )?;
    for position in 0..dimension {
        let diagonal = values[position * dimension + position];
        if !diagonal.is_finite() || diagonal < 0.0 {
            return Err(invalid_data(
                "prepared robust covariance diagonal is invalid",
            ));
        }
        standard_errors[position] = diagonal.sqrt();
    }
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
    let square = checked_square(
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
    let workspace = dimension
        .checked_mul(3)
        .ok_or_else(|| allocation("prepared robust dense covariance workspace overflows"))?;
    let partial_stride = square
        .checked_mul(2)
        .and_then(|meats| meats.checked_add(workspace))
        .ok_or_else(|| allocation("prepared robust dense covariance partial stride overflows"))?;
    let chunks = robust_reduction_chunks(scores.nrows(), partial_stride)?;
    let partial_elements = chunks
        .checked_mul(partial_stride)
        .ok_or_else(|| allocation("prepared robust dense covariance partials overflow usize"))?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve prepared robust dense covariance partials",
    )?;
    let correction_root = hc1_correction.sqrt();
    partials
        .par_chunks_mut(partial_stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let (production, tail) = partial.split_at_mut(square);
            let (certificate, workspace) = tail.split_at_mut(square);
            let (score, workspace) = workspace.split_at_mut(dimension);
            let (certificate_score, influence) = workspace.split_at_mut(dimension);
            let rows_per_chunk = scores.nrows() / chunks;
            let remainder = scores.nrows() % chunks;
            let start = chunk * rows_per_chunk + chunk.min(remainder);
            let end = start + rows_per_chunk + usize::from(chunk < remainder);
            for row in start..end {
                scores.fill_reported_row(row, score)?;
                let reference_weight = weight_view.value(row)?;
                let certificate_root = meat_root_multiplier(weights.kind, reference_weight)?;
                let production_root = if perturb_meat_multiplier {
                    match weights.kind {
                        Some(WeightKind::Analytic | WeightKind::Probability) => {
                            reference_weight.sqrt()
                        }
                        Some(WeightKind::Frequency) => reference_weight,
                        _ => 1.0,
                    }
                } else {
                    certificate_root
                };
                let residual = fit.residuals()[row];
                let production_scale = correction_root * production_root * residual;
                let certificate_scale = certificate_root * residual;
                if !production_scale.is_finite() || !certificate_scale.is_finite() {
                    return Err(invalid_data(
                        "prepared robust dense influence scale is not finite",
                    ));
                }
                for (destination, value) in certificate_score.iter_mut().zip(score.iter().copied())
                {
                    *destination = certificate_scale * value;
                    if !destination.is_finite() {
                        return Err(invalid_data(
                            "prepared robust dense certificate score is not finite",
                        ));
                    }
                }
                for (left, influence_value) in influence.iter_mut().enumerate() {
                    let bread_row = &fit.bread().values()[left * dimension..(left + 1) * dimension];
                    let mut product = 0.0_f64;
                    for right in 0..dimension {
                        product += bread_row[right] * score[right];
                    }
                    *influence_value = production_scale * product;
                    if !influence_value.is_finite() {
                        return Err(invalid_data(
                            "prepared robust dense influence row contains a nonfinite value",
                        ));
                    }
                }
                for left in 0..dimension {
                    let left_value = influence[left];
                    let certificate_left = certificate_score[left];
                    for right in left..dimension {
                        let position = left * dimension + right;
                        production[position] += left_value * influence[right];
                        certificate[position] += certificate_left * certificate_score[right];
                    }
                }
            }
            if production.iter().all(|value| value.is_finite())
                && certificate.iter().all(|value| value.is_finite())
            {
                Ok(())
            } else {
                Err(invalid_data(
                    "prepared robust dense covariance partial contains a nonfinite value",
                ))
            }
        })?;
    let mut values = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust dense covariance",
    )?;
    let mut corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust dense covariance corrections",
    )?;
    let mut certificate_meat = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust dense certificate meat",
    )?;
    let mut certificate_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust dense certificate meat corrections",
    )?;
    for partial in partials.chunks_exact(partial_stride) {
        let production = &partial[..square];
        let certificate = &partial[square..2 * square];
        for left in 0..dimension {
            for right in left..dimension {
                let position = left * dimension + right;
                compensated_add(
                    production[position],
                    &mut values[position],
                    &mut corrections[position],
                )?;
                compensated_add(
                    certificate[position],
                    &mut certificate_meat[position],
                    &mut certificate_corrections[position],
                )?;
            }
        }
    }
    for left in 0..dimension {
        for right in left..dimension {
            let position = left * dimension + right;
            values[position] += corrections[position];
            if !values[position].is_finite() {
                return Err(invalid_data(
                    "prepared robust dense covariance contains a nonfinite value",
                ));
            }
            values[right * dimension + left] = values[position];
            certificate_meat[position] += certificate_corrections[position];
            if !certificate_meat[position].is_finite() {
                return Err(invalid_data(
                    "prepared robust dense certificate meat is not finite",
                ));
            }
            certificate_meat[right * dimension + left] = certificate_meat[position];
        }
    }
    let mut standard_errors = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve prepared robust dense standard errors",
    )?;
    for position in 0..dimension {
        let diagonal = values[position * dimension + position];
        if !diagonal.is_finite() || diagonal < 0.0 {
            return Err(invalid_data(
                "prepared robust dense covariance diagonal is invalid",
            ));
        }
        standard_errors[position] = diagonal.sqrt();
    }
    Ok(PreparedOlsRobustCovarianceBuild {
        covariance: PreparedOlsRobustCovariance {
            hc1_correction,
            values,
            standard_errors,
        },
        dense_certificate_meat: Some(certificate_meat),
    })
}

/// Independent sandwich reconstruction. The production path accumulates
/// bread-transformed influences; this certificate instead builds the meat and
/// then applies bread on both sides.
#[allow(clippy::too_many_arguments)]
fn certify_covariance_formula<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: &CertifiedPreparedOlsScoreDesign,
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
    partials
        .par_chunks_mut(partial_stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
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
        })?;
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
    scores: &CertifiedPreparedOlsScoreDesign,
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
            for right in 0..dimension {
                let position = left * dimension + right;
                compensated_add(
                    score[left] * score[right],
                    &mut meat[position],
                    &mut corrections[position],
                )?;
            }
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
    let square = checked_square(
        slope_dimension,
        "prepared robust Wald dimensions overflow usize",
    )?;
    let mut factor = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust Wald factor",
    )?;
    let covariance_scale = positions
        .iter()
        .map(|position| covariance.values[*position * dimension + *position].abs())
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);
    let threshold = tolerance * covariance_scale;
    for row in 0..slope_dimension {
        for column in 0..=row {
            let mut value = covariance.values[positions[row] * dimension + positions[column]];
            for inner in 0..column {
                value -= factor[row * slope_dimension + inner]
                    * factor[column * slope_dimension + inner];
            }
            if row == column {
                if value < -threshold {
                    return Err(invalid_data(
                        "prepared robust Wald slope covariance is indefinite",
                    ));
                }
                if value <= threshold {
                    return Ok(WaldCalculation {
                        statistic: None,
                        solve_relative_infinity_norm: 0.0,
                    });
                }
                factor[row * slope_dimension + column] = value.sqrt();
            } else {
                factor[row * slope_dimension + column] =
                    value / factor[column * slope_dimension + column];
            }
        }
    }
    let mut solution = reserved_filled(
        reservation,
        slope_dimension,
        0.0_f64,
        "unable to reserve prepared robust Wald solution",
    )?;
    for (slope, position) in positions.iter().copied().enumerate() {
        solution[slope] = fitted_coordinate_value(fit, position)?;
    }
    for row in 0..slope_dimension {
        let mut value = solution[row];
        for column in 0..row {
            value -= factor[row * slope_dimension + column] * solution[column];
        }
        solution[row] = value / factor[row * slope_dimension + row];
    }
    for row in (0..slope_dimension).rev() {
        let mut value = solution[row];
        for column in (row + 1)..slope_dimension {
            value -= factor[column * slope_dimension + row] * solution[column];
        }
        solution[row] = value / factor[row * slope_dimension + row];
    }
    let mut quadratic = 0.0_f64;
    let mut quadratic_correction = 0.0_f64;
    let mut solve_error = 0.0_f64;
    for row in 0..slope_dimension {
        let coefficient = fitted_coordinate_value(fit, positions[row])?;
        compensated_add(
            coefficient * solution[row],
            &mut quadratic,
            &mut quadratic_correction,
        )?;
        let mut fitted = 0.0_f64;
        let mut correction = 0.0_f64;
        let mut scale = coefficient.abs().max(1.0);
        for column in 0..slope_dimension {
            let value = covariance.values[positions[row] * dimension + positions[column]]
                * solution[column];
            compensated_add(value, &mut fitted, &mut correction)?;
            scale += value.abs();
        }
        solve_error = solve_error.max((fitted + correction - coefficient).abs() / scale);
    }
    quadratic += quadratic_correction;
    if !quadratic.is_finite() || quadratic < 0.0 || !solve_error.is_finite() {
        return Err(invalid_data(
            "prepared robust Wald quadratic or solve certificate is invalid",
        ));
    }
    let statistic = quadratic / exact_integer_as_f64(slope_dimension as u128)?;
    if !statistic.is_finite() {
        return Err(invalid_data("prepared robust Wald statistic is not finite"));
    }
    Ok(WaldCalculation {
        statistic: Some(statistic),
        solve_relative_infinity_norm: solve_error,
    })
}

fn fitted_coordinate_value(fit: &CertifiedPreparedOlsFit, position: usize) -> Result<f64> {
    let coordinate = fit
        .bread()
        .coordinates()
        .get(position)
        .ok_or_else(|| invalid_argument("prepared robust fitted coordinate is out of range"))?;
    let fitted = fit
        .coefficients()
        .iter()
        .find(|coefficient| {
            coefficient.generated_column_index == Some(coordinate.generated_column_index)
        })
        .ok_or_else(|| invalid_argument("prepared robust fitted coordinate has no coefficient"))?;
    if fitted.status != PreparedOlsFitStatus::Kept {
        return Err(invalid_argument(
            "prepared robust Wald coordinate is not a kept finite coefficient",
        ));
    }
    fitted
        .value
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid_data("prepared robust Wald coefficient is not finite"))
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
    let partial_bytes = partial_stride
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or_else(|| allocation("prepared robust reduction partial bytes overflow usize"))?;
    let memory_blocks = (ROBUST_PARTIAL_BUDGET_BYTES / partial_bytes.max(1)).max(1);
    Ok(nrows
        .div_ceil(ROBUST_REDUCTION_ROWS)
        .clamp(1, ROBUST_MAX_BLOCKS)
        .min(memory_blocks))
}

fn perturb_optional(value: &mut Option<f64>) -> Result<()> {
    let value = value.as_mut().ok_or_else(|| {
        invalid_argument("injected prepared robust perturbation requires a defined statistic")
    })?;
    *value += 0.25;
    Ok(())
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

fn unsupported(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Unsupported, message)
}

#[cfg(test)]
mod tests {
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind,
        ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId,
        TermComponent, TermId, TermOrder, TimeSeriesState,
    };

    use super::super::{
        prepare_ols_data, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
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

    #[derive(Default)]
    struct CountingReservation {
        calls: usize,
        fail_at: Option<usize>,
        undersize: bool,
    }

    impl PreparedInferenceReservation for CountingReservation {
        fn reserve<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            _label: &'static str,
        ) -> Result<()> {
            self.calls += 1;
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
    fn unit_aweight_pweight_and_scaled_pweight_follow_pinned_hc1_contract() {
        let y = [1.2, 2.4, 2.8, 4.9, 5.1, 7.4, 7.8, 9.7];
        let x = vec![vec![-3.0, -1.5, -0.5, 0.5, 1.5, 2.5, 4.0, 6.0]];
        let weights = [0.25, 1.5, 0.75, 3.0, 2.0, 0.5, 4.0, 1.25];
        let analytic = fit_prepared_ols_robust_map(
            &prepared(&y, &x, None, true, TestWeights::Analytic(&weights)),
            PreparedOlsRobustConfig::default(),
        )
        .expect("analytic robust inference");
        let probability = fit_prepared_ols_robust_map(
            &prepared(&y, &x, None, true, TestWeights::Probability(&weights)),
            PreparedOlsRobustConfig::default(),
        )
        .expect("probability robust inference");
        let scaled = weights.map(|weight| weight * 1e120);
        let probability_scaled = fit_prepared_ols_robust_map(
            &prepared(&y, &x, None, true, TestWeights::Probability(&scaled)),
            PreparedOlsRobustConfig::default(),
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
        let compact = fit_prepared_ols_robust_map(
            &prepared(&y, &x, None, true, TestWeights::Frequency(&frequency)),
            PreparedOlsRobustConfig::default(),
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
        let expanded = fit_prepared_ols_robust_map(
            &prepared(&expanded_y, &[expanded_x], None, true, TestWeights::Unit),
            PreparedOlsRobustConfig::default(),
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
        let reported = fit_prepared_ols_robust_map(
            &prepared(&y, &x, Some(&groups), true, TestWeights::Unit),
            PreparedOlsRobustConfig::default(),
        )
        .expect("reported exact robust fit");
        let hidden = fit_prepared_ols_robust_map(
            &prepared(&y, &x, Some(&groups), false, TestWeights::Unit),
            PreparedOlsRobustConfig::default(),
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
        let mut result = fit_prepared_ols_robust_map(&prepared, PreparedOlsRobustConfig::default())
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
        let hidden = fit_prepared_ols_robust_map(
            &prepared(&y, &[], None, false, TestWeights::Unit),
            PreparedOlsRobustConfig::default(),
        )
        .expect("hidden no-X robust inference");
        assert_eq!(hidden.fit().finite_rank(), 0);
        assert_eq!(hidden.covariance().dimension(), 0);
        assert_eq!(hidden.certificate().psd_rank, 0);
        assert!(hidden.statistics().f_statistic.is_none());

        let reported = fit_prepared_ols_robust_map(
            &prepared(&y, &[], None, true, TestWeights::Unit),
            PreparedOlsRobustConfig::default(),
        )
        .expect("reported intercept-only robust inference");
        assert_eq!(reported.fit().finite_rank(), 0);
        assert_eq!(reported.covariance().dimension(), 1);
        assert!(reported.certificate().passed);
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
        let fit = fit_prepared_ols_coefficients_view_map(prepared, config.fit)?;
        let absorbed = compute_prepared_ols_absorbed_dof(prepared, config.absorbed_dof)?;
        compose_internal(prepared, fit, absorbed, config, reservation, perturbations)
    }

    fn prepared(
        outcome: &[f64],
        regressors: &[Vec<f64>],
        fixed_effect: Option<&[f64]>,
        include_intercept: bool,
        weights: TestWeights<'_>,
    ) -> PreparedOlsData {
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
        .expect("prepared robust test case")
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
