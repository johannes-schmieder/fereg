use crate::data::{PreparedWeights, WeightKind};
use crate::error::{ErrorCode, Result, StFixestError};
use crate::prepared_categorical_data::PreparedCategoricalOlsData;
use crate::topology_dof::CategoricalTopologyDofReport;

use super::prepared_inference_view::{
    validate_cluster_aware_legacy_absorbed_dof_summary, PreparedOlsAbsorbedDofSummary,
    PreparedOlsInferenceView,
};

#[cfg(test)]
use super::compute_prepared_ols_absorbed_dof;
use super::{
    fit_prepared_categorical_ols_coefficients_map, fit_prepared_ols_coefficients_map,
    CertifiedPreparedOlsFit, PreparedOlsAbsorbedDofPolicy, PreparedOlsAbsorbedDofReport,
    PreparedOlsBreadCoordinate, PreparedOlsBreadCoordinateKind, PreparedOlsColumnKind,
    PreparedOlsData, PreparedOlsFitConfig, PreparedOlsFitStatus,
};

/// Numerical and structural policy for the prepared iid OLS/WLS checkpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsUnadjustedConfig {
    pub fit: PreparedOlsFitConfig,
    pub absorbed_dof: PreparedOlsAbsorbedDofPolicy,
    /// Maximum normalized composition, covariance, and statistic error.
    pub certificate_tolerance: f64,
}

impl Default for PreparedOlsUnadjustedConfig {
    fn default() -> Self {
        Self {
            fit: PreparedOlsFitConfig::default(),
            absorbed_dof: PreparedOlsAbsorbedDofPolicy::default(),
            certificate_tolerance: 5e-10,
        }
    }
}

/// Reference objective scale and effective observation count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsReferenceWeightReport {
    pub kind: Option<WeightKind>,
    pub physical_observations: usize,
    pub effective_observations: usize,
    /// Raw supplied sum for weighted models; absent when unweighted.
    pub raw_weight_sum: Option<f64>,
    pub solver_weight_divisor: f64,
    pub reference_weight_multiplier: f64,
}

/// Centralized rank and residual-degree-of-freedom accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsResidualDofReport {
    pub finite_rank: usize,
    /// Reference `df_m`: identified finite regressors excluding an ordinary
    /// finite intercept.  A normalized absorbed intercept was never in rank.
    pub finite_model_dof: usize,
    pub absorbed_rank: usize,
    pub model_rank: usize,
    pub residual_dof: usize,
    pub has_structural_intercept: bool,
    pub has_normalized_absorbed_intercept: bool,
    pub absorbed_rank_is_approximate: bool,
}

/// Reference-compatible OLS/WLS fit statistics.
///
/// Undefined ratios and degenerate Wald/log-likelihood quantities are kept as
/// `None`; NaN is never used as a scientific sentinel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsFitStatistics {
    pub rss: f64,
    pub tss: f64,
    pub within_tss: f64,
    pub r_squared: Option<f64>,
    pub adjusted_r_squared: Option<f64>,
    pub within_r_squared: Option<f64>,
    pub adjusted_within_r_squared: Option<f64>,
    pub root_mean_squared_error: f64,
    pub f_statistic: Option<f64>,
    pub gaussian_log_likelihood: Option<f64>,
    pub gaussian_null_log_likelihood: Option<f64>,
}

/// Independent composition certificate for iid covariance and fit statistics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsUnadjustedCertificate {
    pub reference_weight_sum_relative_error: f64,
    pub sum_of_squares_order_relative_error: f64,
    pub covariance_scale_relative_infinity_norm: f64,
    pub covariance_symmetry_relative_infinity_norm: f64,
    pub statistic_formula_relative_infinity_norm: f64,
    pub minimum_covariance_diagonal: f64,
    pub passed: bool,
}

/// Conventional homoskedastic covariance on the fit's canonical bread axis.
#[derive(Debug, PartialEq)]
pub struct PreparedOlsUnadjustedCovariance {
    variance_scale: f64,
    values: Vec<f64>,
    standard_errors: Vec<f64>,
}

impl PreparedOlsUnadjustedCovariance {
    #[must_use]
    pub const fn variance_scale(&self) -> f64 {
        self.variance_scale
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

/// Certified core-only prepared iid OLS/WLS result.
///
/// This type is intentionally constructed by one same-call pipeline so a fit
/// or absorbed-DoF report from another prepared sample cannot be composed with
/// it accidentally.  It is not a Stata result and performs no SPI writes.
#[derive(Debug, PartialEq)]
pub struct CertifiedPreparedOlsUnadjusted {
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofReport,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsUnadjustedCovariance,
    certificate: PreparedOlsUnadjustedCertificate,
}

impl CertifiedPreparedOlsUnadjusted {
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
    pub const fn covariance(&self) -> &PreparedOlsUnadjustedCovariance {
        &self.covariance
    }

    #[must_use]
    pub fn covariance_coordinates(&self) -> &[PreparedOlsBreadCoordinate] {
        self.fit.bread().coordinates()
    }

    #[must_use]
    pub const fn certificate(&self) -> PreparedOlsUnadjustedCertificate {
        self.certificate
    }
}

/// Certified categorical-interaction iid OLS/WLS result.
///
/// The numerical fit, covariance, statistics, and certificates are produced by
/// the same common pipeline as [`CertifiedPreparedOlsUnadjusted`].  This result
/// retains the interaction-aware absorbed-DoF identities rather than weakening
/// them to the legacy single-source representation.
#[derive(Debug, PartialEq)]
pub struct CertifiedPreparedCategoricalOlsUnadjusted {
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: CategoricalTopologyDofReport,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsUnadjustedCovariance,
    certificate: PreparedOlsUnadjustedCertificate,
}

impl CertifiedPreparedCategoricalOlsUnadjusted {
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
    pub const fn covariance(&self) -> &PreparedOlsUnadjustedCovariance {
        &self.covariance
    }

    #[must_use]
    pub fn covariance_coordinates(&self) -> &[PreparedOlsBreadCoordinate] {
        self.fit.bread().coordinates()
    }

    #[must_use]
    pub const fn certificate(&self) -> PreparedOlsUnadjustedCertificate {
        self.certificate
    }
}

#[derive(Debug, PartialEq)]
struct PreparedOlsUnadjustedComponents {
    fit: CertifiedPreparedOlsFit,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsUnadjustedCovariance,
    certificate: PreparedOlsUnadjustedCertificate,
}

trait InferenceReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemInferenceReservation;

impl InferenceReservation for SystemInferenceReservation {
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

#[derive(Debug, Clone, Copy, Default)]
struct InferencePerturbations {
    rss: bool,
    tss: bool,
    residual_dof: bool,
    variance_scale: bool,
    covariance: bool,
    standard_error: bool,
    within_tss: bool,
    r_squared: bool,
    adjusted_r_squared: bool,
    within_r_squared: bool,
    adjusted_within_r_squared: bool,
    root_mean_squared_error: bool,
    f_statistic: bool,
    gaussian_log_likelihood: bool,
    gaussian_null_log_likelihood: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ReferenceWeightView<'a> {
    values: Option<&'a [f64]>,
    kind: Option<WeightKind>,
    divisor: f64,
    multiplier: f64,
}

#[derive(Debug, Clone, Copy)]
struct InferenceCertificateInputs {
    rss: f64,
    tss: f64,
    within_tss: f64,
    wald_solve_relative_infinity_norm: f64,
    reference_weight_sum_relative_error: f64,
    tolerance: f64,
}

#[derive(Debug, Clone, Copy)]
struct WaldFCalculation {
    statistic: Option<f64>,
    solve_relative_infinity_norm: f64,
}

impl ReferenceWeightView<'_> {
    pub(super) fn value(self, row: usize) -> Result<f64> {
        let value = match (self.kind, self.values) {
            (None, None) => 1.0,
            (Some(WeightKind::Frequency), Some(values)) => *values
                .get(row)
                .ok_or_else(|| invalid_argument("prepared iid weight row is out of range"))?,
            (Some(WeightKind::Analytic | WeightKind::Probability), Some(values)) => {
                let raw = *values
                    .get(row)
                    .ok_or_else(|| invalid_argument("prepared iid weight row is out of range"))?;
                (raw / self.divisor) * self.multiplier
            }
            _ => {
                return Err(invalid_argument(
                    "prepared iid weight kind and storage are inconsistent",
                ));
            }
        };
        if value.is_finite() && value > 0.0 {
            Ok(value)
        } else {
            Err(invalid_data(
                "prepared iid reference weight is not finite and positive",
            ))
        }
    }
}

/// Fit and certify the current prepared model with conventional homoskedastic
/// covariance. Probability weights fail closed because the pinned reference
/// changes them to robust inference instead of producing iid covariance.
pub fn fit_prepared_ols_unadjusted_map(
    prepared: &PreparedOlsData,
    config: PreparedOlsUnadjustedConfig,
) -> Result<CertifiedPreparedOlsUnadjusted> {
    validate_config(config)?;
    reject_probability_iid(prepared)?;
    let fit = fit_prepared_ols_coefficients_map(prepared, config.fit)?;
    let absorbed_dof = prepared.inference_compute_absorbed_dof(config.absorbed_dof)?;
    let mut reservation = SystemInferenceReservation;
    compose_internal(
        prepared,
        fit,
        absorbed_dof,
        config.certificate_tolerance,
        &mut reservation,
        InferencePerturbations::default(),
    )
}

/// Fit and certify categorical-interaction OLS/WLS with conventional
/// homoskedastic covariance through the same common inference pipeline.
pub fn fit_prepared_categorical_ols_unadjusted_map(
    prepared: &PreparedCategoricalOlsData,
    config: PreparedOlsUnadjustedConfig,
) -> Result<CertifiedPreparedCategoricalOlsUnadjusted> {
    validate_config(config)?;
    reject_probability_iid(prepared)?;
    let fit = fit_prepared_categorical_ols_coefficients_map(prepared, config.fit)?;
    let absorbed_dof = prepared.inference_compute_absorbed_dof(config.absorbed_dof)?;
    let summary = prepared.inference_absorbed_dof_summary(&absorbed_dof, config.absorbed_dof)?;
    let mut reservation = SystemInferenceReservation;
    let components = compose_view_internal(
        prepared,
        fit,
        summary,
        config.certificate_tolerance,
        &mut reservation,
        InferencePerturbations::default(),
    )?;
    Ok(CertifiedPreparedCategoricalOlsUnadjusted {
        fit: components.fit,
        absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}

fn reject_probability_iid<P: PreparedOlsInferenceView + ?Sized>(prepared: &P) -> Result<()> {
    if matches!(
        &prepared.fit_sample().weights,
        PreparedWeights::Provided {
            kind: WeightKind::Probability,
            ..
        }
    ) {
        return Err(unsupported(
            "prepared iid covariance is unavailable for probability weights; the pinned reference requires robust inference",
        ));
    }
    Ok(())
}

fn compose_internal<R: InferenceReservation>(
    prepared: &PreparedOlsData,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofReport,
    tolerance: f64,
    reservation: &mut R,
    perturbations: InferencePerturbations,
) -> Result<CertifiedPreparedOlsUnadjusted> {
    let summary = prepared.inference_absorbed_dof_summary(&absorbed_dof, absorbed_dof.policy)?;
    let components = compose_view_internal(
        prepared,
        fit,
        summary,
        tolerance,
        reservation,
        perturbations,
    )?;
    Ok(CertifiedPreparedOlsUnadjusted {
        fit: components.fit,
        absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}

fn compose_view_internal<P: PreparedOlsInferenceView + ?Sized, R: InferenceReservation>(
    prepared: &P,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofSummary,
    tolerance: f64,
    reservation: &mut R,
    perturbations: InferencePerturbations,
) -> Result<PreparedOlsUnadjustedComponents> {
    validate_fit_and_absorbed_dof_view(prepared, &fit, absorbed_dof)?;
    let (weights, weight_view, weight_sum_relative_error) =
        build_reference_weight_report_view(prepared, &fit, tolerance, false)?;
    let mut dof = build_residual_dof_view(prepared, &fit, absorbed_dof, weights)?;
    if perturbations.residual_dof {
        dof.residual_dof = dof
            .residual_dof
            .checked_add(1)
            .ok_or_else(|| allocation("injected prepared iid residual DoF overflows"))?;
    }

    let outcome_mean = weighted_mean(&prepared.fit_sample().outcome, weight_view)?;
    let center = if dof.has_structural_intercept {
        outcome_mean
    } else {
        0.0
    };
    let certified_tss =
        weighted_sum_of_squares(&prepared.fit_sample().outcome, weight_view, center)?;
    let certified_within_tss = if prepared.inference_has_explicit_absorbed_effects() {
        fit.initial_absorbed_outcome_sum_of_squares()
    } else {
        certified_tss
    };
    let certified_rss = weighted_sum_of_squares(fit.residuals(), weight_view, 0.0)?;
    let mut tss = certified_tss;
    if perturbations.tss {
        tss *= 1.25;
    }
    let mut within_tss = certified_within_tss;
    if perturbations.within_tss {
        within_tss *= 1.25;
    }
    let mut rss = certified_rss;
    if perturbations.rss {
        rss *= 1.25;
    }
    validate_sum_of_squares(rss, tss, within_tss)?;

    let mut statistics = build_statistics(rss, tss, within_tss, weights, dof)?;
    if perturbations.r_squared {
        perturb_optional_statistic(&mut statistics.r_squared)?;
    }
    if perturbations.adjusted_r_squared {
        perturb_optional_statistic(&mut statistics.adjusted_r_squared)?;
    }
    if perturbations.within_r_squared {
        perturb_optional_statistic(&mut statistics.within_r_squared)?;
    }
    if perturbations.adjusted_within_r_squared {
        perturb_optional_statistic(&mut statistics.adjusted_within_r_squared)?;
    }
    if perturbations.root_mean_squared_error {
        statistics.root_mean_squared_error += 0.25;
    }
    if perturbations.gaussian_log_likelihood {
        perturb_optional_statistic(&mut statistics.gaussian_log_likelihood)?;
    }
    if perturbations.gaussian_null_log_likelihood {
        perturb_optional_statistic(&mut statistics.gaussian_null_log_likelihood)?;
    }
    let mut covariance = build_covariance(&fit, rss, dof.residual_dof, reservation)?;
    let wald = build_wald_f(prepared, &fit, &covariance, dof, reservation)?;
    statistics.f_statistic = wald.statistic;
    if perturbations.f_statistic {
        perturb_optional_statistic(&mut statistics.f_statistic)?;
    }
    if perturbations.variance_scale {
        covariance.variance_scale += 0.25;
    }
    if perturbations.covariance && !covariance.values.is_empty() {
        covariance.values[0] += 0.25;
    }
    if perturbations.standard_error && !covariance.standard_errors.is_empty() {
        covariance.standard_errors[0] += 0.25;
    }
    let certificate = certify_inference(
        prepared,
        &fit,
        weights,
        dof,
        statistics,
        &covariance,
        InferenceCertificateInputs {
            rss: certified_rss,
            tss: certified_tss,
            within_tss: certified_within_tss,
            wald_solve_relative_infinity_norm: wald.solve_relative_infinity_norm,
            reference_weight_sum_relative_error: weight_sum_relative_error,
            tolerance,
        },
    )?;
    if !certificate.passed {
        return Err(invalid_data(
            "prepared iid covariance or fit-statistic certificate did not pass",
        ));
    }
    Ok(PreparedOlsUnadjustedComponents {
        fit,
        weights,
        dof,
        statistics,
        covariance,
        certificate,
    })
}

fn validate_config(config: PreparedOlsUnadjustedConfig) -> Result<()> {
    if !config.certificate_tolerance.is_finite()
        || config.certificate_tolerance <= 0.0
        || config.certificate_tolerance >= 1.0
    {
        return Err(invalid_argument(
            "prepared iid certificate tolerance must be finite and in (0,1)",
        ));
    }
    Ok(())
}

fn perturb_optional_statistic(value: &mut Option<f64>) -> Result<()> {
    let value = value.as_mut().ok_or_else(|| {
        invalid_argument("injected prepared iid perturbation requires a defined statistic")
    })?;
    *value += 0.25;
    Ok(())
}

pub(super) fn validate_fit_and_absorbed_dof_view<P: PreparedOlsInferenceView + ?Sized>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    absorbed: PreparedOlsAbsorbedDofSummary,
) -> Result<()> {
    let nobs = prepared.fit_sample().n_active();
    let reported_intercepts = prepared
        .fit_model()
        .columns
        .iter()
        .filter(|column| matches!(column.kind, PreparedOlsColumnKind::Intercept))
        .count();
    if nobs == 0
        || reported_intercepts != usize::from(prepared.fit_report_intercept())
        || prepared.fit_report_intercept() && !prepared.fit_structural_intercept()
        || fit.residuals().len() != nobs
        || fit.coefficients().len() != prepared.fit_model().coefficients.len()
        || !fit.rank_certificate().passed
        || !fit.score_certificate().passed
        || !fit.bread().certificate().passed
        || !fit.absorption_report().certified()
        || !fit.residual_absorption_report().certified()
        || absorbed.nested_categories != 0
        || absorbed.redundant_categories > absorbed.initial_categories
        || absorbed.absorbed_rank != absorbed.initial_categories - absorbed.redundant_categories
    {
        return Err(invalid_argument(
            "prepared iid fit or absorbed rank is not complete for the final sample",
        ));
    }
    let bread_dimension = fit.bread().dimension();
    let bread_square = checked_square(
        bread_dimension,
        "prepared iid bread dimensions overflow usize",
    )?;
    if fit.bread().values().len() != bread_square
        || fit.bread().coordinates().len() != bread_dimension
    {
        return Err(invalid_argument(
            "prepared iid bread dimensions are inconsistent",
        ));
    }

    let mut kept = 0_usize;
    for (prepared_coefficient, fitted) in prepared
        .fit_model()
        .coefficients
        .iter()
        .zip(fit.coefficients())
    {
        if prepared_coefficient.coefficient_slot != fitted.coefficient_slot
            || prepared_coefficient.generated_column_index != fitted.generated_column_index
        {
            return Err(invalid_argument(
                "prepared iid coefficient identity changed after fitting",
            ));
        }
        if fitted.status == PreparedOlsFitStatus::Kept {
            kept = kept
                .checked_add(1)
                .ok_or_else(|| allocation("prepared iid kept-rank count overflows"))?;
        }
    }
    if kept != fit.finite_rank() {
        return Err(invalid_argument(
            "prepared iid kept coefficients disagree with finite rank",
        ));
    }

    let mut finite_coordinates = 0_usize;
    let mut normalized_coordinates = 0_usize;
    for coordinate in fit.bread().coordinates() {
        let prepared_column = prepared
            .fit_model()
            .columns
            .get(coordinate.generated_column_index)
            .ok_or_else(|| invalid_argument("prepared iid bread coordinate is out of range"))?;
        if prepared_column.coefficient_slot != coordinate.coefficient_slot {
            return Err(invalid_argument(
                "prepared iid bread coordinate slot is inconsistent",
            ));
        }
        let fitted = fit
            .coefficients()
            .iter()
            .find(|coefficient| {
                coefficient.generated_column_index == Some(coordinate.generated_column_index)
            })
            .ok_or_else(|| {
                invalid_argument("prepared iid bread coordinate has no fitted coefficient")
            })?;
        match coordinate.kind {
            PreparedOlsBreadCoordinateKind::FiniteRegressor => {
                if fitted.status != PreparedOlsFitStatus::Kept {
                    return Err(invalid_argument(
                        "prepared iid finite bread coordinate is not kept",
                    ));
                }
                finite_coordinates = finite_coordinates
                    .checked_add(1)
                    .ok_or_else(|| allocation("prepared iid finite-coordinate count overflows"))?;
            }
            PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept => {
                if fitted.status != PreparedOlsFitStatus::NormalizedAbsorbedIntercept
                    || !matches!(prepared_column.kind, PreparedOlsColumnKind::Intercept)
                {
                    return Err(invalid_argument(
                        "prepared iid normalized-intercept coordinate is inconsistent",
                    ));
                }
                normalized_coordinates =
                    normalized_coordinates.checked_add(1).ok_or_else(|| {
                        allocation("prepared iid normalized-intercept count overflows")
                    })?;
            }
        }
    }
    if finite_coordinates != fit.finite_rank()
        || normalized_coordinates > 1
        || (normalized_coordinates == 1
            && (!prepared.fit_structural_intercept() || !prepared.fit_report_intercept()))
    {
        return Err(invalid_argument(
            "prepared iid covariance-coordinate rank is inconsistent",
        ));
    }
    Ok(())
}

pub(super) fn build_reference_weight_report<'a>(
    prepared: &'a PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    tolerance: f64,
    allow_probability: bool,
) -> Result<(
    PreparedOlsReferenceWeightReport,
    ReferenceWeightView<'a>,
    f64,
)> {
    build_reference_weight_report_view(prepared, fit, tolerance, allow_probability)
}

pub(super) fn build_reference_weight_report_view<'a, P: PreparedOlsInferenceView + ?Sized>(
    prepared: &'a P,
    fit: &CertifiedPreparedOlsFit,
    tolerance: f64,
    allow_probability: bool,
) -> Result<(
    PreparedOlsReferenceWeightReport,
    ReferenceWeightView<'a>,
    f64,
)> {
    let nobs = prepared.fit_sample().n_active();
    let bread_scale = fit.bread().weight_scale();
    let nobs_f64 = exact_integer_as_f64(nobs as u128)?;
    let (kind, values, effective, raw_weight_sum, expected_divisor, expected_multiplier) =
        match &prepared.fit_sample().weights {
            PreparedWeights::None => (None, None, nobs, None, 1.0, 1.0),
            PreparedWeights::Provided { kind, values } => {
                if values.len() != nobs
                    || values
                        .iter()
                        .any(|weight| !weight.is_finite() || *weight <= 0.0)
                {
                    return Err(invalid_data(
                        "prepared iid weights are not finite, positive, and aligned",
                    ));
                }
                let divisor = values.iter().copied().fold(0.0_f64, f64::max);
                if !divisor.is_finite()
                    || divisor <= 0.0
                    || values.iter().any(|weight| *weight / divisor == 0.0)
                {
                    return Err(invalid_data("prepared iid weight normalization is invalid"));
                }
                let raw_sum = positive_sum_scaled(values, divisor)?;
                match kind {
                    WeightKind::Frequency => {
                        let mut sum = 0_u128;
                        for value in values {
                            if value.fract() != 0.0 || *value > f64::from(u32::MAX) {
                                return Err(invalid_data(
                                    "prepared iid frequency weight is not a positive u32 integer",
                                ));
                            }
                            sum = sum.checked_add(u128::from(*value as u32)).ok_or_else(|| {
                                allocation("prepared iid frequency effective N overflows")
                            })?;
                        }
                        let effective = usize::try_from(sum).map_err(|_| {
                            allocation("prepared iid frequency effective N exceeds usize")
                        })?;
                        let exact_sum = exact_integer_as_f64(sum)?;
                        (
                            Some(*kind),
                            Some(values.as_slice()),
                            effective,
                            Some(exact_sum),
                            divisor,
                            divisor,
                        )
                    }
                    WeightKind::Analytic => {
                        let solver_sum = raw_sum / divisor;
                        let multiplier = nobs_f64 / solver_sum;
                        (
                            Some(*kind),
                            Some(values.as_slice()),
                            nobs,
                            Some(raw_sum),
                            divisor,
                            multiplier,
                        )
                    }
                    WeightKind::Probability if allow_probability => {
                        let solver_sum = raw_sum / divisor;
                        let multiplier = nobs_f64 / solver_sum;
                        (
                            Some(*kind),
                            Some(values.as_slice()),
                            nobs,
                            Some(raw_sum),
                            divisor,
                            multiplier,
                        )
                    }
                    WeightKind::Probability => {
                        return Err(unsupported(
                            "prepared iid covariance is unavailable for probability weights",
                        ));
                    }
                    WeightKind::Importance => {
                        return Err(unsupported(
                            "prepared iid covariance does not support importance weights",
                        ));
                    }
                }
            }
        };
    if effective == 0 {
        return Err(invalid_data(
            "prepared iid effective observation count is zero",
        ));
    }
    let effective_f64 = exact_integer_as_f64(effective as u128)?;
    let divisor_error = relative_difference(
        bread_scale.solver_weight_divisor,
        expected_divisor,
        expected_divisor.abs().max(1.0),
    )?;
    let multiplier_error = relative_difference(
        bread_scale.reference_weight_multiplier,
        expected_multiplier,
        expected_multiplier.abs().max(1.0),
    )?;
    let reference_sum_error = relative_difference(
        bread_scale.reference_weight_sum,
        effective_f64,
        effective_f64.max(1.0),
    )?;
    if bread_scale.weight_kind != kind
        || divisor_error > tolerance
        || multiplier_error > tolerance
        || reference_sum_error > tolerance
    {
        return Err(invalid_argument(
            "prepared iid bread and sample weight scales disagree",
        ));
    }
    let view = ReferenceWeightView {
        values,
        kind,
        divisor: bread_scale.solver_weight_divisor,
        multiplier: bread_scale.reference_weight_multiplier,
    };
    let mut computed_sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for row in 0..nobs {
        compensated_add(view.value(row)?, &mut computed_sum, &mut correction)?;
    }
    let computed_sum = computed_sum + correction;
    let computed_error = relative_difference(computed_sum, effective_f64, effective_f64.max(1.0))?;
    if computed_error > tolerance {
        return Err(invalid_data(
            "prepared iid reference weights do not sum to effective N",
        ));
    }
    Ok((
        PreparedOlsReferenceWeightReport {
            kind,
            physical_observations: nobs,
            effective_observations: effective,
            raw_weight_sum,
            solver_weight_divisor: bread_scale.solver_weight_divisor,
            reference_weight_multiplier: bread_scale.reference_weight_multiplier,
        },
        view,
        computed_error.max(reference_sum_error),
    ))
}

pub(super) fn build_clustered_base_residual_dof(
    prepared: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    absorbed: &PreparedOlsAbsorbedDofReport,
    weights: PreparedOlsReferenceWeightReport,
) -> Result<PreparedOlsResidualDofReport> {
    let summary =
        validate_cluster_aware_legacy_absorbed_dof_summary(prepared, absorbed, absorbed.policy)?;
    build_residual_dof_view(prepared, fit, summary, weights)
}

pub(super) fn build_residual_dof_view<P: PreparedOlsInferenceView + ?Sized>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    absorbed: PreparedOlsAbsorbedDofSummary,
    weights: PreparedOlsReferenceWeightReport,
) -> Result<PreparedOlsResidualDofReport> {
    let mut finite_intercepts = 0_usize;
    let mut normalized_intercepts = 0_usize;
    for coordinate in fit.bread().coordinates() {
        match coordinate.kind {
            PreparedOlsBreadCoordinateKind::FiniteRegressor => {
                if matches!(
                    prepared
                        .fit_model()
                        .columns
                        .get(coordinate.generated_column_index)
                        .map(|column| &column.kind),
                    Some(PreparedOlsColumnKind::Intercept)
                ) {
                    finite_intercepts = finite_intercepts.checked_add(1).ok_or_else(|| {
                        allocation("prepared iid finite-intercept count overflows")
                    })?;
                }
            }
            PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept => {
                normalized_intercepts = normalized_intercepts.checked_add(1).ok_or_else(|| {
                    allocation("prepared iid normalized-intercept count overflows")
                })?;
            }
        }
    }
    if finite_intercepts > 1 || normalized_intercepts > 1 {
        return Err(invalid_argument(
            "prepared iid model contains multiple reported intercept coordinates",
        ));
    }
    let ordinary_intercept_requested = prepared.fit_report_intercept();
    let has_absorbed_intercept = prepared.fit_structural_intercept();
    // The structural constant direction is a property of the requested model
    // span, not of whichever duplicate coordinate won a numerical pivot tie.
    // Absorbed intercepts own that direction; otherwise a requested `_cons`
    // consumes one finite-rank direction even if another constant column spans
    // the same space.
    let finite_intercept_rank =
        usize::from(ordinary_intercept_requested && !has_absorbed_intercept);
    let finite_model_dof = fit
        .finite_rank()
        .checked_sub(finite_intercept_rank)
        .ok_or_else(|| invalid_argument("prepared iid finite model DoF underflowed"))?;
    let model_rank = fit
        .finite_rank()
        .checked_add(absorbed.absorbed_rank)
        .ok_or_else(|| allocation("prepared iid model rank overflows"))?;
    let residual_dof = weights
        .effective_observations
        .checked_sub(model_rank)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            insufficient_observations(
                "prepared iid model has no positive residual degrees of freedom",
            )
        })?;
    let has_structural_intercept = prepared.fit_structural_intercept();
    Ok(PreparedOlsResidualDofReport {
        finite_rank: fit.finite_rank(),
        finite_model_dof,
        absorbed_rank: absorbed.absorbed_rank,
        model_rank,
        residual_dof,
        has_structural_intercept,
        has_normalized_absorbed_intercept: normalized_intercepts == 1,
        absorbed_rank_is_approximate: absorbed.is_approximate,
    })
}

pub(super) fn build_statistics(
    rss: f64,
    tss: f64,
    within_tss: f64,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
) -> Result<PreparedOlsFitStatistics> {
    let n = exact_integer_as_f64(weights.effective_observations as u128)?;
    let df_r = exact_integer_as_f64(dof.residual_dof as u128)?;
    let centered_dof = weights
        .effective_observations
        .checked_sub(usize::from(dof.has_structural_intercept));
    let within_baseline_dof = dof
        .residual_dof
        .checked_add(dof.finite_model_dof)
        .ok_or_else(|| allocation("prepared iid within baseline DoF overflows"))?;
    let r_squared = one_minus_ratio(rss, tss)?;
    let adjusted_r_squared = match centered_dof.filter(|value| *value > 0) {
        Some(value) if tss > 0.0 => {
            Some(1.0 - (rss / df_r) / (tss / exact_integer_as_f64(value as u128)?))
        }
        _ => None,
    };
    let within_r_squared = one_minus_ratio(rss, within_tss)?;
    let adjusted_within_r_squared = if within_tss > 0.0 && within_baseline_dof > 0 {
        Some(1.0 - (rss / df_r) / (within_tss / exact_integer_as_f64(within_baseline_dof as u128)?))
    } else {
        None
    };
    let root_mean_squared_error = (rss / df_r).sqrt();
    let gaussian_log_likelihood = gaussian_log_likelihood_from_ss(rss, n)?;
    let gaussian_null_log_likelihood = gaussian_log_likelihood_from_ss(within_tss, n)?;
    for value in [
        r_squared,
        adjusted_r_squared,
        within_r_squared,
        adjusted_within_r_squared,
        gaussian_log_likelihood,
        gaussian_null_log_likelihood,
    ]
    .into_iter()
    .flatten()
    {
        if !value.is_finite() {
            return Err(invalid_data(
                "prepared iid defined fit statistic is not finite",
            ));
        }
    }
    if !root_mean_squared_error.is_finite() {
        return Err(invalid_data("prepared iid RMSE is not finite"));
    }
    Ok(PreparedOlsFitStatistics {
        rss,
        tss,
        within_tss,
        r_squared,
        adjusted_r_squared,
        within_r_squared,
        adjusted_within_r_squared,
        root_mean_squared_error,
        f_statistic: None,
        gaussian_log_likelihood,
        gaussian_null_log_likelihood,
    })
}

fn build_covariance<R: InferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    rss: f64,
    residual_dof: usize,
    reservation: &mut R,
) -> Result<PreparedOlsUnadjustedCovariance> {
    let dimension = fit.bread().dimension();
    let square = checked_square(
        dimension,
        "prepared iid covariance dimensions overflow usize",
    )?;
    if fit.bread().values().len() != square || residual_dof == 0 {
        return Err(invalid_argument(
            "prepared iid covariance inputs have inconsistent dimensions or DoF",
        ));
    }
    let variance_scale = rss / exact_integer_as_f64(residual_dof as u128)?;
    if !variance_scale.is_finite() || variance_scale < 0.0 {
        return Err(invalid_data(
            "prepared iid covariance variance scale is invalid",
        ));
    }
    let mut values = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared iid covariance",
    )?;
    for (value, bread) in values.iter_mut().zip(fit.bread().values()) {
        *value = variance_scale * *bread;
        if !value.is_finite() {
            return Err(invalid_data(
                "prepared iid covariance contains a nonfinite value",
            ));
        }
    }
    let mut standard_errors = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve prepared iid standard errors",
    )?;
    for (index, standard_error) in standard_errors.iter_mut().enumerate() {
        let diagonal = values[index * dimension + index];
        if !diagonal.is_finite() || diagonal < 0.0 {
            return Err(invalid_data("prepared iid covariance diagonal is invalid"));
        }
        *standard_error = diagonal.sqrt();
    }
    Ok(PreparedOlsUnadjustedCovariance {
        variance_scale,
        values,
        standard_errors,
    })
}

fn build_wald_f<P: PreparedOlsInferenceView + ?Sized, R: InferenceReservation>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    covariance: &PreparedOlsUnadjustedCovariance,
    dof: PreparedOlsResidualDofReport,
    reservation: &mut R,
) -> Result<WaldFCalculation> {
    let dimension = covariance.dimension();
    if dimension != fit.bread().coordinates().len() {
        return Err(invalid_argument(
            "prepared iid Wald covariance coordinates are inconsistent",
        ));
    }
    if dof.finite_model_dof == 0 || covariance.variance_scale == 0.0 {
        return Ok(WaldFCalculation {
            statistic: None,
            solve_relative_infinity_norm: 0.0,
        });
    }

    let slope_dimension = dof.finite_model_dof;
    let mut slope_positions = reserved_filled(
        reservation,
        slope_dimension,
        0_usize,
        "unable to reserve prepared iid Wald slope coordinates",
    )?;
    let mut slope_count = 0_usize;
    for (position, coordinate) in fit.bread().coordinates().iter().enumerate() {
        let column = prepared
            .fit_model()
            .columns
            .get(coordinate.generated_column_index)
            .ok_or_else(|| invalid_argument("prepared iid Wald coordinate is out of range"))?;
        if matches!(column.kind, PreparedOlsColumnKind::Intercept) {
            continue;
        }
        let target = slope_positions.get_mut(slope_count).ok_or_else(|| {
            invalid_argument("prepared iid Wald slope count exceeds finite model DoF")
        })?;
        *target = position;
        slope_count = slope_count
            .checked_add(1)
            .ok_or_else(|| allocation("prepared iid Wald slope count overflows"))?;
    }
    if slope_count != slope_dimension {
        return Err(invalid_argument(
            "prepared iid Wald slope count differs from finite model DoF",
        ));
    }

    let square = checked_square(
        slope_dimension,
        "prepared iid Wald dimensions overflow usize",
    )?;
    let mut factor = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared iid Wald factor",
    )?;
    for row in 0..slope_dimension {
        for column in 0..=row {
            let covariance_row = slope_positions[row];
            let covariance_column = slope_positions[column];
            let mut value = covariance.values[covariance_row * dimension + covariance_column];
            for inner in 0..column {
                value -= factor[row * slope_dimension + inner]
                    * factor[column * slope_dimension + inner];
            }
            if row == column {
                if !value.is_finite() || value <= 0.0 {
                    return Err(invalid_data(
                        "prepared iid Wald covariance is not positive definite",
                    ));
                }
                factor[row * slope_dimension + column] = value.sqrt();
            } else {
                let diagonal = factor[column * slope_dimension + column];
                let entry = value / diagonal;
                if !entry.is_finite() {
                    return Err(invalid_data(
                        "prepared iid Wald Cholesky factor is not finite",
                    ));
                }
                factor[row * slope_dimension + column] = entry;
            }
        }
    }

    let mut solution = reserved_filled(
        reservation,
        slope_dimension,
        0.0_f64,
        "unable to reserve prepared iid Wald solution",
    )?;
    for (slope, position) in slope_positions.iter().copied().enumerate() {
        solution[slope] = fitted_coordinate_value(fit, position)?;
    }
    for row in 0..slope_dimension {
        let mut value = solution[row];
        for column in 0..row {
            value -= factor[row * slope_dimension + column] * solution[column];
        }
        solution[row] = value / factor[row * slope_dimension + row];
        if !solution[row].is_finite() {
            return Err(invalid_data(
                "prepared iid Wald forward solve is not finite",
            ));
        }
    }
    for row in (0..slope_dimension).rev() {
        let mut value = solution[row];
        for column in (row + 1)..slope_dimension {
            value -= factor[column * slope_dimension + row] * solution[column];
        }
        solution[row] = value / factor[row * slope_dimension + row];
        if !solution[row].is_finite() {
            return Err(invalid_data(
                "prepared iid Wald backward solve is not finite",
            ));
        }
    }

    let mut quadratic = 0.0_f64;
    let mut quadratic_correction = 0.0_f64;
    let mut solve_relative_infinity_norm = 0.0_f64;
    for row in 0..slope_dimension {
        let coefficient = fitted_coordinate_value(fit, slope_positions[row])?;
        compensated_add(
            coefficient * solution[row],
            &mut quadratic,
            &mut quadratic_correction,
        )?;
        let mut fitted = 0.0_f64;
        let mut fitted_correction = 0.0_f64;
        let mut row_scale = coefficient.abs();
        for column in 0..slope_dimension {
            let value = covariance.values
                [slope_positions[row] * dimension + slope_positions[column]]
                * solution[column];
            compensated_add(value, &mut fitted, &mut fitted_correction)?;
            row_scale += value.abs();
            if !row_scale.is_finite() {
                return Err(invalid_data("prepared iid Wald solve scale is not finite"));
            }
        }
        let residual = (fitted + fitted_correction - coefficient).abs();
        solve_relative_infinity_norm =
            solve_relative_infinity_norm.max(residual / row_scale.max(f64::MIN_POSITIVE));
    }
    quadratic += quadratic_correction;
    if !quadratic.is_finite() || quadratic < 0.0 || !solve_relative_infinity_norm.is_finite() {
        return Err(invalid_data(
            "prepared iid Wald quadratic or solve certificate is invalid",
        ));
    }
    let statistic = quadratic / exact_integer_as_f64(slope_dimension as u128)?;
    if !statistic.is_finite() {
        return Err(invalid_data("prepared iid Wald F statistic is not finite"));
    }
    Ok(WaldFCalculation {
        statistic: Some(statistic),
        solve_relative_infinity_norm,
    })
}

fn fitted_coordinate_value(fit: &CertifiedPreparedOlsFit, position: usize) -> Result<f64> {
    let coordinate = fit
        .bread()
        .coordinates()
        .get(position)
        .ok_or_else(|| invalid_argument("prepared iid fitted coordinate is out of range"))?;
    let fitted = fit
        .coefficients()
        .iter()
        .find(|coefficient| {
            coefficient.generated_column_index == Some(coordinate.generated_column_index)
        })
        .ok_or_else(|| invalid_argument("prepared iid fitted coordinate has no coefficient"))?;
    if fitted.status != PreparedOlsFitStatus::Kept {
        return Err(invalid_argument(
            "prepared iid Wald slope coordinate is not a kept finite coefficient",
        ));
    }
    fitted
        .value
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid_data("prepared iid Wald coefficient is not finite"))
}

fn certify_inference<P: PreparedOlsInferenceView + ?Sized>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: &PreparedOlsUnadjustedCovariance,
    inputs: InferenceCertificateInputs,
) -> Result<PreparedOlsUnadjustedCertificate> {
    let ordinary_intercept_requested = prepared.fit_report_intercept();
    let has_absorbed_intercept = prepared.fit_structural_intercept();
    let expected_finite_intercept_rank =
        usize::from(ordinary_intercept_requested && !has_absorbed_intercept);
    let expected_finite_model_dof = fit
        .finite_rank()
        .checked_sub(expected_finite_intercept_rank)
        .ok_or_else(|| invalid_data("prepared iid certificate finite model DoF underflowed"))?;
    let expected_model_rank = dof
        .finite_rank
        .checked_add(dof.absorbed_rank)
        .ok_or_else(|| allocation("prepared iid certificate model rank overflows"))?;
    let expected_residual = weights
        .effective_observations
        .checked_sub(expected_model_rank)
        .ok_or_else(|| invalid_data("prepared iid certificate residual DoF underflowed"))?;
    if dof.finite_rank != fit.finite_rank()
        || dof.finite_model_dof != expected_finite_model_dof
        || dof.has_structural_intercept != prepared.fit_structural_intercept()
        || dof.model_rank != expected_model_rank
        || dof.residual_dof != expected_residual
        || dof.residual_dof == 0
    {
        return Err(invalid_data(
            "prepared iid residual-DoF identity did not pass",
        ));
    }

    let sum_scale = inputs.rss.max(inputs.tss).max(inputs.within_tss).max(1.0);
    let rss_primitive_error = (statistics.rss - inputs.rss).abs() / sum_scale;
    let tss_primitive_error = (statistics.tss - inputs.tss).abs() / sum_scale;
    let within_primitive_error = (statistics.within_tss - inputs.within_tss).abs() / sum_scale;
    let overall_excess = (inputs.rss - inputs.tss).max(0.0) / sum_scale;
    let within_excess = (inputs.rss - inputs.within_tss).max(0.0) / sum_scale;
    let within_overall_excess = (inputs.within_tss - inputs.tss).max(0.0) / sum_scale;
    let sum_of_squares_order_relative_error = overall_excess
        .max(within_excess)
        .max(within_overall_excess)
        .max(rss_primitive_error)
        .max(tss_primitive_error)
        .max(within_primitive_error);

    let dimension = covariance.dimension();
    let square = checked_square(
        dimension,
        "prepared iid certificate covariance dimensions overflow usize",
    )?;
    if dimension != fit.bread().dimension()
        || covariance.values.len() != square
        || fit.bread().values().len() != square
    {
        return Err(invalid_argument(
            "prepared iid certificate covariance dimensions are inconsistent",
        ));
    }
    let expected_scale = inputs.rss / exact_integer_as_f64(dof.residual_dof as u128)?;
    let mut covariance_scale_relative_infinity_norm = relative_difference(
        covariance.variance_scale,
        expected_scale,
        expected_scale.abs().max(1.0),
    )?;
    let mut covariance_symmetry_relative_infinity_norm = 0.0_f64;
    let mut minimum_covariance_diagonal = if dimension == 0 { 0.0 } else { f64::INFINITY };
    for row in 0..dimension {
        let mut scale_error = 0.0_f64;
        let mut scale = 1.0_f64;
        let mut symmetry_error = 0.0_f64;
        let mut symmetry_scale = 1.0_f64;
        for column in 0..dimension {
            let position = row * dimension + column;
            let value = covariance.values[position];
            let expected = expected_scale * fit.bread().values()[position];
            scale_error += (value - expected).abs();
            scale += value.abs().max(expected.abs());
            let transpose = covariance.values[column * dimension + row];
            symmetry_error += (value - transpose).abs();
            symmetry_scale += value.abs().max(transpose.abs());
            if !scale_error.is_finite()
                || !scale.is_finite()
                || !symmetry_error.is_finite()
                || !symmetry_scale.is_finite()
            {
                return Err(invalid_data(
                    "prepared iid covariance certificate accumulation is not finite",
                ));
            }
        }
        covariance_scale_relative_infinity_norm =
            covariance_scale_relative_infinity_norm.max(scale_error / scale);
        covariance_symmetry_relative_infinity_norm =
            covariance_symmetry_relative_infinity_norm.max(symmetry_error / symmetry_scale);
        minimum_covariance_diagonal =
            minimum_covariance_diagonal.min(covariance.values[row * dimension + row]);
        let expected_standard_error = covariance.values[row * dimension + row].sqrt();
        covariance_scale_relative_infinity_norm =
            covariance_scale_relative_infinity_norm.max(relative_difference(
                covariance.standard_errors[row],
                expected_standard_error,
                expected_standard_error.abs().max(1.0),
            )?);
    }

    let statistic_formula_relative_infinity_norm =
        certify_statistic_identities(statistics, weights, dof, inputs)?;
    let passed = inputs.reference_weight_sum_relative_error <= inputs.tolerance
        && sum_of_squares_order_relative_error <= inputs.tolerance
        && covariance_scale_relative_infinity_norm <= inputs.tolerance
        && covariance_symmetry_relative_infinity_norm <= inputs.tolerance
        && statistic_formula_relative_infinity_norm <= inputs.tolerance
        && (dimension == 0
            || minimum_covariance_diagonal.is_finite() && minimum_covariance_diagonal >= 0.0)
        && covariance.values.iter().all(|value| value.is_finite())
        && covariance
            .standard_errors
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0);
    Ok(PreparedOlsUnadjustedCertificate {
        reference_weight_sum_relative_error: inputs.reference_weight_sum_relative_error,
        sum_of_squares_order_relative_error,
        covariance_scale_relative_infinity_norm,
        covariance_symmetry_relative_infinity_norm,
        statistic_formula_relative_infinity_norm,
        minimum_covariance_diagonal,
        passed,
    })
}

fn certify_statistic_identities(
    statistics: PreparedOlsFitStatistics,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    inputs: InferenceCertificateInputs,
) -> Result<f64> {
    let n = exact_integer_as_f64(weights.effective_observations as u128)?;
    let df_r = exact_integer_as_f64(dof.residual_dof as u128)?;
    let df_m = exact_integer_as_f64(dof.finite_model_dof as u128)?;
    let centered_dof = weights
        .effective_observations
        .checked_sub(usize::from(dof.has_structural_intercept));
    let within_baseline_dof = dof
        .residual_dof
        .checked_add(dof.finite_model_dof)
        .ok_or_else(|| allocation("prepared iid certificate within baseline DoF overflows"))?;
    let sum_scale = inputs.rss.max(inputs.tss).max(inputs.within_tss).max(1.0);
    let mut norm = inputs.wald_solve_relative_infinity_norm;

    norm = norm.max(optional_identity_error(
        statistics.r_squared,
        inputs.tss > 0.0,
        |value| relative_difference((1.0 - value) * inputs.tss, inputs.rss, sum_scale),
    )?);
    let adjusted_defined = inputs.tss > 0.0 && centered_dof.is_some_and(|value| value > 0);
    norm = norm.max(optional_identity_error(
        statistics.adjusted_r_squared,
        adjusted_defined,
        |value| {
            let baseline_dof = exact_integer_as_f64(
                centered_dof.ok_or_else(|| invalid_data("prepared iid centered DoF is absent"))?
                    as u128,
            )?;
            relative_difference(
                (1.0 - value) * (df_r / baseline_dof),
                inputs.rss / inputs.tss,
                1.0,
            )
        },
    )?);
    norm = norm.max(optional_identity_error(
        statistics.within_r_squared,
        inputs.within_tss > 0.0,
        |value| relative_difference((1.0 - value) * inputs.within_tss, inputs.rss, sum_scale),
    )?);
    norm = norm.max(optional_identity_error(
        statistics.adjusted_within_r_squared,
        inputs.within_tss > 0.0 && within_baseline_dof > 0,
        |value| {
            let baseline_dof = exact_integer_as_f64(within_baseline_dof as u128)?;
            relative_difference(
                (1.0 - value) * (df_r / baseline_dof),
                inputs.rss / inputs.within_tss,
                1.0,
            )
        },
    )?);

    if !statistics.root_mean_squared_error.is_finite() || statistics.root_mean_squared_error < 0.0 {
        return Err(invalid_data(
            "prepared iid certificate RMSE is not finite and nonnegative",
        ));
    }
    norm = norm.max(relative_difference(
        statistics
            .root_mean_squared_error
            .mul_add(statistics.root_mean_squared_error, 0.0),
        inputs.rss / df_r,
        (inputs.rss / df_r).max(1.0),
    )?);

    let f_defined = dof.finite_model_dof > 0 && inputs.rss > 0.0;
    norm = norm.max(optional_identity_error(
        statistics.f_statistic,
        f_defined,
        |value| {
            if value < 0.0 || inputs.within_tss <= 0.0 {
                return Ok(1.0);
            }
            // This is a total-within-scale cross-check, not the construction
            // of F.  Its absolute normalization remains stable when the model
            // signal is tiny relative to orthogonal residual noise.
            let reported_explained_fraction =
                value * ((df_m / df_r) * (inputs.rss / inputs.within_tss));
            let certified_explained_fraction = (inputs.within_tss - inputs.rss) / inputs.within_tss;
            relative_difference(
                reported_explained_fraction,
                certified_explained_fraction,
                1.0,
            )
        },
    )?);

    norm = norm.max(optional_identity_error(
        statistics.gaussian_log_likelihood,
        inputs.rss > 0.0,
        |value| gaussian_log_likelihood_identity_error(value, inputs.rss, n),
    )?);
    norm = norm.max(optional_identity_error(
        statistics.gaussian_null_log_likelihood,
        inputs.within_tss > 0.0,
        |value| gaussian_log_likelihood_identity_error(value, inputs.within_tss, n),
    )?);
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(invalid_data(
            "prepared iid statistic certificate norm is not finite",
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

fn gaussian_log_likelihood_identity_error(
    likelihood: f64,
    sum_of_squares: f64,
    observations: f64,
) -> Result<f64> {
    let per_observation = likelihood / observations;
    let actual = -per_observation - per_observation - 1.0 - (2.0 * std::f64::consts::PI).ln();
    let expected = (sum_of_squares / observations).ln();
    relative_difference(actual, expected, actual.abs().max(expected.abs()).max(1.0))
}

pub(super) fn validate_sum_of_squares(rss: f64, tss: f64, within_tss: f64) -> Result<()> {
    if [rss, tss, within_tss]
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(invalid_data(
            "prepared iid sum of squares is not finite and nonnegative",
        ));
    }
    Ok(())
}

pub(super) fn weighted_mean(values: &[f64], weights: ReferenceWeightView<'_>) -> Result<f64> {
    if values.is_empty() {
        return Err(invalid_argument(
            "prepared iid weighted mean requires a nonempty sample",
        ));
    }
    let mut value_scale = 0.0_f64;
    for value in values {
        if !value.is_finite() {
            return Err(invalid_data(
                "prepared iid weighted-mean value is not finite",
            ));
        }
        value_scale = value_scale.max(value.abs());
    }
    if value_scale == 0.0 {
        return Ok(0.0);
    }
    let mut numerator = 0.0_f64;
    let mut numerator_correction = 0.0_f64;
    let mut denominator = 0.0_f64;
    let mut denominator_correction = 0.0_f64;
    for (row, value) in values.iter().copied().enumerate() {
        let weight = weights.value(row)?;
        compensated_add(
            weight * (value / value_scale),
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
        Err(invalid_data("prepared iid weighted mean is not finite"))
    }
}

pub(super) fn weighted_sum_of_squares(
    values: &[f64],
    weights: ReferenceWeightView<'_>,
    center: f64,
) -> Result<f64> {
    if !center.is_finite() {
        return Err(invalid_data(
            "prepared iid sum-of-squares center is invalid",
        ));
    }
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for (row, value) in values.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(invalid_data(
                "prepared iid sum-of-squares value is not finite",
            ));
        }
        let weighted = weights.value(row)?.sqrt() * (value - center);
        update_scaled_sum_of_squares(weighted, &mut scale, &mut scaled_square_sum)?;
    }
    let norm = if scale == 0.0 {
        0.0
    } else {
        scale * scaled_square_sum.sqrt()
    };
    let sum = norm.mul_add(norm, 0.0);
    if sum.is_finite() && sum >= 0.0 {
        Ok(sum)
    } else {
        Err(invalid_data(
            "prepared iid weighted sum of squares is not finite",
        ))
    }
}

fn update_scaled_sum_of_squares(value: f64, scale: &mut f64, sum: &mut f64) -> Result<()> {
    if !value.is_finite() {
        return Err(invalid_data(
            "prepared iid scaled sum-of-squares value is not finite",
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
            "prepared iid scaled sum-of-squares accumulation is not finite",
        ))
    }
}

fn positive_sum_scaled(values: &[f64], divisor: f64) -> Result<f64> {
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for value in values {
        compensated_add(*value / divisor, &mut sum, &mut correction)?;
    }
    let raw = divisor * (sum + correction);
    if raw.is_finite() && raw > 0.0 {
        Ok(raw)
    } else {
        Err(invalid_data(
            "prepared iid raw weight sum is not finite and positive",
        ))
    }
}

fn one_minus_ratio(numerator: f64, denominator: f64) -> Result<Option<f64>> {
    if denominator == 0.0 {
        return Ok(None);
    }
    let value = 1.0 - numerator / denominator;
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err(invalid_data("prepared iid ratio statistic is not finite"))
    }
}

fn gaussian_log_likelihood_from_ss(sum_of_squares: f64, observations: f64) -> Result<Option<f64>> {
    if sum_of_squares == 0.0 {
        return Ok(None);
    }
    let value = -0.5
        * observations
        * (1.0 + (2.0 * std::f64::consts::PI).ln() + (sum_of_squares / observations).ln());
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err(invalid_data(
            "prepared iid Gaussian log likelihood is not finite",
        ))
    }
}

pub(super) fn relative_difference(actual: f64, expected: f64, scale: f64) -> Result<f64> {
    if !actual.is_finite() || !expected.is_finite() || !scale.is_finite() || scale <= 0.0 {
        return Err(invalid_data(
            "prepared iid relative-certificate input is invalid",
        ));
    }
    let relative = (actual - expected).abs() / scale;
    if relative.is_finite() {
        Ok(relative)
    } else {
        Err(invalid_data(
            "prepared iid relative-certificate result is not finite",
        ))
    }
}

pub(super) fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    if !value.is_finite() {
        return Err(invalid_data("prepared iid compensated term is not finite"));
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
            "prepared iid compensated accumulation is not finite",
        ))
    }
}

pub(super) fn exact_integer_as_f64(value: u128) -> Result<f64> {
    const MAX_EXACT_F64_INTEGER: u128 = 1_u128 << 53;
    if value > MAX_EXACT_F64_INTEGER {
        return Err(invalid_data(
            "prepared iid effective observation count exceeds exact f64 range",
        ));
    }
    Ok(value as f64)
}

pub(super) fn checked_square(value: usize, message: &'static str) -> Result<usize> {
    value.checked_mul(value).ok_or_else(|| allocation(message))
}

fn reserved_filled<T: Clone, R: InferenceReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "prepared iid reservation returned insufficient capacity",
        ));
    }
    output.resize(elements, value);
    Ok(output)
}

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidData, message)
}

fn insufficient_observations(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InsufficientObservations, message)
}

fn unsupported(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Unsupported, message)
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
        OlsRequestV1, PreparedOlsAbsorbedDofEffectIdentity,
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

    impl InferenceReservation for CountingReservation {
        fn reserve<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            _label: &'static str,
        ) -> Result<()> {
            self.calls += 1;
            if self.fail_at == Some(self.calls) {
                return Err(allocation("injected prepared iid reservation failure"));
            }
            if !self.undersize {
                target
                    .try_reserve_exact(elements)
                    .map_err(|_| allocation("test prepared iid reservation failed"))?;
            }
            Ok(())
        }
    }

    #[test]
    fn allocation_failures_and_undersized_reservations_fail_closed() {
        let prepared = prepared(
            &[1.2, 2.1, 2.9, 4.2, 5.1, 6.4],
            &[vec![-2.0, -1.0, 0.0, 1.0, 2.0, 4.0]],
            None,
            true,
            TestWeights::Unit,
        );
        let mut baseline = CountingReservation::default();
        let result = compose_for_test(&prepared, &mut baseline, InferencePerturbations::default())
            .expect("baseline prepared iid composition");
        assert!(result.certificate().passed);
        assert_eq!(baseline.calls, 5);

        for fail_at in 1..=baseline.calls {
            let mut reservation = CountingReservation {
                fail_at: Some(fail_at),
                ..CountingReservation::default()
            };
            let error = compose_for_test(
                &prepared,
                &mut reservation,
                InferencePerturbations::default(),
            )
            .expect_err("injected allocation must fail");
            assert_eq!(error.code, ErrorCode::Allocation);
        }

        let mut undersized = CountingReservation {
            undersize: true,
            ..CountingReservation::default()
        };
        let error = compose_for_test(
            &prepared,
            &mut undersized,
            InferencePerturbations::default(),
        )
        .expect_err("undersized reservation must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
    }

    #[test]
    fn every_inference_perturbation_is_rejected() {
        let groups = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0];
        let x = [-2.0, 0.0, 1.0, -1.0, 2.0, 4.0, -3.0, 1.5, 5.0];
        let y = (0..x.len())
            .map(|row| 1.7 * x[row] + groups[row] + 0.07 * (row as f64).sin())
            .collect::<Vec<_>>();
        let prepared = prepared(&y, &[x.to_vec()], Some(&groups), true, TestWeights::Unit);
        for perturbation in [
            InferencePerturbations {
                rss: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                tss: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                residual_dof: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                variance_scale: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                covariance: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                standard_error: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                within_tss: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                r_squared: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                adjusted_r_squared: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                within_r_squared: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                adjusted_within_r_squared: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                root_mean_squared_error: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                f_statistic: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                gaussian_log_likelihood: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                gaussian_null_log_likelihood: true,
                ..InferencePerturbations::default()
            },
        ] {
            let mut reservation = CountingReservation::default();
            let error = compose_for_test(&prepared, &mut reservation, perturbation)
                .expect_err("injected inference corruption must fail");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn hidden_constant_reported_constant_and_fe_only_edges_are_explicit() {
        let y = [2.0, 3.5, 1.0, 4.0, 5.0, 2.5];
        let no_intercept = fit_prepared_ols_unadjusted_map(
            &prepared(&y, &[], None, false, TestWeights::Unit),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("hidden structural-constant inference");
        assert_eq!(no_intercept.dof().absorbed_rank, 1);
        assert_eq!(no_intercept.dof().model_rank, 1);
        assert_eq!(no_intercept.dof().residual_dof, y.len() - 1);
        assert!(no_intercept.dof().has_structural_intercept);
        assert_eq!(
            no_intercept.absorbed_dof().effects[0].identity,
            PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb
        );
        assert_eq!(no_intercept.covariance().dimension(), 0);
        let mean = y.iter().sum::<f64>() / y.len() as f64;
        assert_close(
            no_intercept.statistics().tss,
            y.iter().map(|value| (value - mean) * (value - mean)).sum(),
            1e-12,
        );
        assert!(no_intercept.statistics().f_statistic.is_none());

        let constant = [0.0; 6];
        let intercept_only = fit_prepared_ols_unadjusted_map(
            &prepared(&constant, &[], None, true, TestWeights::Unit),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("intercept-only exact inference");
        assert_eq!(intercept_only.dof().finite_rank, 0);
        assert_eq!(intercept_only.dof().finite_model_dof, 0);
        assert_eq!(intercept_only.dof().absorbed_rank, 1);
        assert_eq!(intercept_only.dof().model_rank, 1);
        assert_eq!(intercept_only.statistics().tss, 0.0);
        assert_eq!(intercept_only.statistics().rss, 0.0);
        assert!(intercept_only.statistics().r_squared.is_none());
        assert!(intercept_only.statistics().within_r_squared.is_none());
        assert!(intercept_only.statistics().f_statistic.is_none());
        assert!(intercept_only
            .statistics()
            .gaussian_log_likelihood
            .is_none());
        assert_eq!(intercept_only.covariance().values(), &[0.0]);

        let groups = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0];
        let explained = [1.0, 1.0, 3.0, 3.0, -2.0, -2.0];
        let fe_only = fit_prepared_ols_unadjusted_map(
            &prepared(&explained, &[], Some(&groups), false, TestWeights::Unit),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("FE-only rank-zero exact inference");
        assert_eq!(fe_only.dof().finite_rank, 0);
        assert_eq!(fe_only.dof().absorbed_rank, 3);
        assert_eq!(fe_only.covariance().dimension(), 0);
        assert!(fe_only.dof().has_structural_intercept);
        assert_eq!(fe_only.statistics().rss, 0.0);
        assert_eq!(fe_only.statistics().within_tss, 0.0);
        assert!(fe_only.statistics().f_statistic.is_none());
    }

    #[test]
    fn pinned_constant_regressor_is_structurally_absorbed_and_cons_is_normalized() {
        // Pinned Stata 18/reghdfe contract: `c.one` followed by the default
        // `_cons` reports the normalized structural constant, while `one` is
        // classified as collinear with the virtual one-level absorbed effect.
        let y = [2.0, 5.0, 4.0, 8.0, 7.0, 11.0];
        let one = vec![1.0; y.len()];
        let result = fit_prepared_ols_unadjusted_map(
            &prepared(&y, &[one], None, true, TestWeights::Unit),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("constant plus requested intercept inference");
        assert_eq!(
            result.fit().coefficients()[0].status,
            PreparedOlsFitStatus::CollinearWithFixedEffects
        );
        assert_eq!(
            result.fit().coefficients()[1].status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert!(result.fit().kept_generated_columns().is_empty());
        assert_eq!(result.fit().dropped_generated_columns(), &[0]);
        assert_eq!(result.dof().finite_rank, 0);
        assert_eq!(result.dof().finite_model_dof, 0);
        assert_eq!(result.dof().absorbed_rank, 1);
        assert!(result.dof().has_structural_intercept);
        assert!(result.statistics().f_statistic.is_none());
        let mean = y.iter().sum::<f64>() / y.len() as f64;
        let centered_tss = y
            .iter()
            .map(|value| (value - mean) * (value - mean))
            .sum::<f64>();
        assert_close(result.statistics().rss, centered_tss, 1e-12);
        assert_close(result.statistics().tss, centered_tss, 1e-12);
        assert_close(
            result
                .statistics()
                .gaussian_log_likelihood
                .expect("model ll"),
            result
                .statistics()
                .gaussian_null_log_likelihood
                .expect("null ll"),
            1e-12,
        );

        let hidden = fit_prepared_ols_unadjusted_map(
            &prepared(&y, &[vec![1.0; y.len()]], None, false, TestWeights::Unit),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("hidden constant-regressor inference");
        assert_eq!(
            hidden.fit().coefficients()[0].status,
            PreparedOlsFitStatus::CollinearWithFixedEffects
        );
        assert_eq!(hidden.dof().finite_rank, 0);
        assert_eq!(hidden.dof().finite_model_dof, 0);
        assert_eq!(hidden.dof().absorbed_rank, 1);
        assert!(!hidden.dof().has_normalized_absorbed_intercept);
        assert_eq!(hidden.covariance().dimension(), 0);
        assert_slice_close(result.fit().residuals(), hidden.fit().residuals(), 1e-12);
        for (reported, unreported) in [
            (result.statistics().rss, hidden.statistics().rss),
            (result.statistics().tss, hidden.statistics().tss),
            (
                result.statistics().within_tss,
                hidden.statistics().within_tss,
            ),
            (
                result.statistics().root_mean_squared_error,
                hidden.statistics().root_mean_squared_error,
            ),
        ] {
            assert_close(reported, unreported, 1e-12);
        }
    }

    #[test]
    fn paired_no_x_default_and_noconstant_share_weighted_structural_residuals() {
        let y = [2.0, 7.0, 3.5, 11.0, 5.0, 13.0];
        let analytic = [0.5, 2.0, 1.5, 3.0, 0.75, 4.0];
        let groups = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0];
        for fixed_effect in [None, Some(groups.as_slice())] {
            let reported = fit_prepared_ols_unadjusted_map(
                &prepared(
                    &y,
                    &[],
                    fixed_effect,
                    true,
                    TestWeights::Analytic(&analytic),
                ),
                PreparedOlsUnadjustedConfig::default(),
            )
            .expect("reported no-X inference");
            let hidden = fit_prepared_ols_unadjusted_map(
                &prepared(
                    &y,
                    &[],
                    fixed_effect,
                    false,
                    TestWeights::Analytic(&analytic),
                ),
                PreparedOlsUnadjustedConfig::default(),
            )
            .expect("hidden no-X inference");
            let weighted_mean = y
                .iter()
                .zip(analytic)
                .map(|(value, weight)| value * weight)
                .sum::<f64>()
                / analytic.iter().sum::<f64>();
            assert_close(
                reported.fit().coefficients()[0]
                    .value
                    .expect("normalized reported mean"),
                weighted_mean,
                1e-12,
            );
            assert_eq!(
                reported.fit().coefficients()[0].status,
                PreparedOlsFitStatus::NormalizedAbsorbedIntercept
            );
            assert!(hidden.fit().coefficients().is_empty());
            assert_eq!(reported.dof().finite_rank, 0);
            assert_eq!(hidden.dof().finite_rank, 0);
            assert_eq!(reported.dof().finite_model_dof, 0);
            assert_eq!(hidden.dof().finite_model_dof, 0);
            assert_eq!(reported.dof().model_rank, hidden.dof().model_rank);
            assert_eq!(reported.dof().residual_dof, hidden.dof().residual_dof);
            assert_eq!(reported.covariance().dimension(), 1);
            assert_eq!(hidden.covariance().dimension(), 0);
            assert_slice_close(reported.fit().residuals(), hidden.fit().residuals(), 1e-12);
            for (left, right) in [
                (reported.statistics().rss, hidden.statistics().rss),
                (reported.statistics().tss, hidden.statistics().tss),
                (
                    reported.statistics().within_tss,
                    hidden.statistics().within_tss,
                ),
                (
                    reported.statistics().root_mean_squared_error,
                    hidden.statistics().root_mean_squared_error,
                ),
                (
                    reported
                        .statistics()
                        .gaussian_log_likelihood
                        .expect("reported likelihood"),
                    hidden
                        .statistics()
                        .gaussian_log_likelihood
                        .expect("hidden likelihood"),
                ),
            ] {
                assert_close(left, right, 1e-12);
            }
            assert!(reported.statistics().f_statistic.is_none());
            assert!(hidden.statistics().f_statistic.is_none());
        }
    }

    #[test]
    fn pinned_full_ibn_span_has_one_finite_contrast_and_report_only_cons() {
        let foreign = SourceEntry {
            id: SourceId::new(1).expect("foreign source"),
            canonical_name: "foreign".to_owned(),
        };
        let factor_term = |id: u32, level: f64| ExpandedTerm {
            id: TermId::new(id).expect("factor term"),
            order: TermOrder::new(id).expect("factor order"),
            coefficient_slot: CoefficientSlot::new(id).expect("factor slot"),
            raw_kind: RawTermKind::Factor,
            whole_column_omitted: false,
            canonical_token: format!("{level}.foreign"),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("factor component"),
                source: foreign.id,
                kind: ComponentKind::Categorical,
                level_state: LevelState::Present,
                level_value: level,
                base: MetadataFlag::False,
                omitted: MetadataFlag::False,
                raw_operator: level.to_string(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        };
        let foreign_values = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let y = [9.5, 10.0, 10.5, 12.5, 13.0, 13.5];
        let fit = |include_intercept| {
            let prepared = prepare_ols_data(
                &OlsRequestV1 {
                    model: CompiledTermGraph {
                        sources: vec![foreign.clone()],
                        terms: vec![factor_term(1, 0.0), factor_term(2, 1.0)],
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
                    include_intercept,
                    keep_singletons: true,
                },
                &LoadedOlsInput {
                    candidate: vec![true; y.len()],
                    outcome: loaded(&y),
                    numeric_sources: vec![(foreign.id, loaded(&foreign_values))],
                    analytic_weight: None,
                    frequency_weight: None,
                    probability_weight: None,
                },
            )
            .expect("full ibn preparation");
            fit_prepared_ols_unadjusted_map(&prepared, PreparedOlsUnadjustedConfig::default())
                .expect("full ibn inference")
        };
        let reported = fit(true);
        let hidden = fit(false);
        for result in [&reported, &hidden] {
            assert_eq!(result.dof().finite_rank, 1);
            assert_eq!(result.dof().finite_model_dof, 1);
            assert_eq!(result.dof().absorbed_rank, 1);
            assert_eq!(result.dof().model_rank, 2);
            assert_eq!(result.fit().kept_generated_columns(), &[0]);
            assert_eq!(result.fit().dropped_generated_columns(), &[1]);
            assert!(result.statistics().f_statistic.is_some());
        }
        assert_eq!(
            reported.fit().coefficients()[2].status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert_eq!(reported.covariance().dimension(), 2);
        assert_eq!(hidden.fit().coefficients().len(), 2);
        assert_eq!(hidden.covariance().dimension(), 1);
    }

    #[test]
    fn pinned_default_and_noconstant_pairs_share_the_structural_fit_for_unit_a_and_f_weights() {
        let groups = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let x = vec![-2.0, -0.5, 1.0, 3.0, -3.0, -1.0, 2.0, 4.0];
        let noise = [0.2, -0.1, 0.3, -0.2, -0.3, 0.1, -0.2, 0.2];
        let y = x
            .iter()
            .enumerate()
            .map(|(row, x)| 4.0 + 1.8 * x + 2.5 * groups[row] + noise[row])
            .collect::<Vec<_>>();
        let analytic = [0.5, 1.5, 2.0, 0.8, 1.2, 2.5, 0.7, 1.8];
        let frequency = [1_u32, 2, 3, 1, 2, 4, 1, 2];

        for weights in [
            TestWeights::Unit,
            TestWeights::Analytic(&analytic),
            TestWeights::Frequency(&frequency),
        ] {
            for fixed_effect in [None, Some(groups.as_slice())] {
                let reported = fit_prepared_ols_unadjusted_map(
                    &prepared(&y, std::slice::from_ref(&x), fixed_effect, true, weights),
                    PreparedOlsUnadjustedConfig::default(),
                )
                .expect("reported structural-constant inference");
                let hidden = fit_prepared_ols_unadjusted_map(
                    &prepared(&y, std::slice::from_ref(&x), fixed_effect, false, weights),
                    PreparedOlsUnadjustedConfig::default(),
                )
                .expect("hidden structural-constant inference");

                assert_eq!(reported.dof().finite_rank, hidden.dof().finite_rank);
                assert_eq!(
                    reported.dof().finite_model_dof,
                    hidden.dof().finite_model_dof
                );
                assert_eq!(reported.dof().absorbed_rank, hidden.dof().absorbed_rank);
                assert_eq!(reported.dof().model_rank, hidden.dof().model_rank);
                assert_eq!(reported.dof().residual_dof, hidden.dof().residual_dof);
                assert!(reported.dof().has_normalized_absorbed_intercept);
                assert!(!hidden.dof().has_normalized_absorbed_intercept);
                assert_eq!(reported.absorbed_dof(), hidden.absorbed_dof());
                assert!(reported.dof().has_structural_intercept);
                assert_eq!(reported.dof().finite_model_dof, 1);
                assert_eq!(reported.fit().coefficients().len(), 2);
                assert_eq!(hidden.fit().coefficients().len(), 1);
                assert_eq!(
                    reported.fit().coefficients()[1].status,
                    PreparedOlsFitStatus::NormalizedAbsorbedIntercept
                );
                assert_close(
                    reported.fit().coefficients()[0]
                        .value
                        .expect("reported slope"),
                    hidden.fit().coefficients()[0].value.expect("hidden slope"),
                    1e-11,
                );
                assert_eq!(
                    reported.fit().residuals().len(),
                    hidden.fit().residuals().len()
                );
                for (left, right) in reported
                    .fit()
                    .residuals()
                    .iter()
                    .zip(hidden.fit().residuals())
                {
                    assert_close(*left, *right, 1e-11);
                }
                for (left, right) in [
                    (reported.statistics().rss, hidden.statistics().rss),
                    (reported.statistics().tss, hidden.statistics().tss),
                    (
                        reported.statistics().within_tss,
                        hidden.statistics().within_tss,
                    ),
                    (
                        reported.statistics().root_mean_squared_error,
                        hidden.statistics().root_mean_squared_error,
                    ),
                    (
                        reported.statistics().f_statistic.expect("reported F"),
                        hidden.statistics().f_statistic.expect("hidden F"),
                    ),
                ] {
                    assert_close(left, right, 1e-10);
                }
                assert_eq!(reported.covariance().dimension(), 2);
                assert_eq!(hidden.covariance().dimension(), 1);
                assert_close(
                    reported.covariance().values()[0],
                    hidden.covariance().values()[0],
                    1e-10,
                );
                if fixed_effect.is_none() {
                    assert_eq!(
                        reported.absorbed_dof().effects[0].identity,
                        PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb
                    );
                } else {
                    assert!(matches!(
                        reported.absorbed_dof().effects[0].identity,
                        PreparedOlsAbsorbedDofEffectIdentity::Explicit { .. }
                    ));
                }
            }
        }
    }

    #[test]
    fn wald_f_survives_weak_signal_under_large_orthogonal_noise() {
        let x = vec![-3.5, -2.5, -1.5, -0.5, 0.5, 1.5, 2.5, 3.5];
        let orthogonal_noise = [1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0];
        let y = x
            .iter()
            .zip(orthogonal_noise)
            .map(|(x, noise)| 1.0e12 * noise + 10.0 * x)
            .collect::<Vec<_>>();
        let result = fit_prepared_ols_unadjusted_map(
            &prepared(&y, &[x], None, true, TestWeights::Unit),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("weak-signal Wald inference");
        let f_statistic = result
            .statistics()
            .f_statistic
            .expect("positive-variance slope test");
        assert!(f_statistic > 0.0 && f_statistic < 1.0e-18);
        assert!(result.certificate().passed);
        assert!(
            result
                .certificate()
                .statistic_formula_relative_infinity_norm
                < 5.0e-10
        );
    }

    #[test]
    fn invalid_configs_pweights_and_nonpositive_residual_dof_fail_closed() {
        let y = [1.0, 2.0, 3.0, 4.0];
        let x = vec![vec![0.0, 1.0, 2.0, 3.0]];
        for tolerance in [f64::NAN, 0.0, 1.0] {
            let error = fit_prepared_ols_unadjusted_map(
                &prepared(&y, &x, None, true, TestWeights::Unit),
                PreparedOlsUnadjustedConfig {
                    certificate_tolerance: tolerance,
                    ..PreparedOlsUnadjustedConfig::default()
                },
            )
            .expect_err("invalid inference tolerance must fail");
            assert_eq!(error.code, ErrorCode::InvalidArgument);
        }

        let pweights = [0.5, 1.5, 2.0, 3.0];
        let error = fit_prepared_ols_unadjusted_map(
            &prepared(&y, &x, None, true, TestWeights::Probability(&pweights)),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect_err("iid pweight inference must fail");
        assert_eq!(error.code, ErrorCode::Unsupported);

        let saturated = prepared(&[2.0], &[], None, true, TestWeights::Unit);
        let error =
            fit_prepared_ols_unadjusted_map(&saturated, PreparedOlsUnadjustedConfig::default())
                .expect_err("zero residual DoF must fail");
        assert_eq!(error.code, ErrorCode::InsufficientObservations);

        let empty = prepared(&[], &[], None, false, TestWeights::Unit);
        let error = fit_prepared_ols_unadjusted_map(&empty, PreparedOlsUnadjustedConfig::default())
            .expect_err("empty inference sample must fail");
        assert_eq!(error.code, ErrorCode::InsufficientObservations);

        let mut misaligned = prepared(&y, &x, None, true, TestWeights::Analytic(&pweights));
        let PreparedWeights::Provided { values, .. } = &mut misaligned.sample.weights else {
            panic!("analytic test weights must be present");
        };
        values.pop();
        let error =
            fit_prepared_ols_unadjusted_map(&misaligned, PreparedOlsUnadjustedConfig::default())
                .expect_err("misaligned prepared weights must fail");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn corrupted_absorbed_dof_artifact_cannot_be_composed() {
        let groups = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0];
        let with_fe = prepared(
            &[1.0, 1.2, 2.5, 2.8, 4.1, 4.4],
            &[vec![-2.0, 0.5, -1.0, 2.0, 1.5, 4.0]],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        let fit = fit_prepared_ols_coefficients_map(&with_fe, PreparedOlsFitConfig::default())
            .expect("certified fit");
        let mut absorbed =
            compute_prepared_ols_absorbed_dof(&with_fe, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("certified absorbed DoF");
        absorbed.effects[0].categories += 1;
        absorbed.initial_categories += 1;
        absorbed.absorbed_rank += 1;
        let mut reservation = CountingReservation::default();
        let error = compose_internal(
            &with_fe,
            fit,
            absorbed,
            PreparedOlsUnadjustedConfig::default().certificate_tolerance,
            &mut reservation,
            InferencePerturbations::default(),
        )
        .expect_err("corrupted absorbed-DoF report must fail");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(reservation.calls, 0);

        let no_absorb = prepared(
            &[1.0, 2.0, 4.0, 7.0],
            &[vec![-1.0, 0.0, 1.0, 3.0]],
            None,
            true,
            TestWeights::Unit,
        );
        let fit = fit_prepared_ols_coefficients_map(&no_absorb, PreparedOlsFitConfig::default())
            .expect("no-absorb fit");
        let mut absorbed =
            compute_prepared_ols_absorbed_dof(&no_absorb, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("implicit no-absorb DoF");
        absorbed.effects[0].identity = PreparedOlsAbsorbedDofEffectIdentity::Explicit {
            term_id: AbsorbTermId::new(1).expect("fake term"),
            source: SourceId::new(1).expect("fake source"),
        };
        let mut reservation = CountingReservation::default();
        let error = compose_internal(
            &no_absorb,
            fit,
            absorbed,
            PreparedOlsUnadjustedConfig::default().certificate_tolerance,
            &mut reservation,
            InferencePerturbations::default(),
        )
        .expect_err("implicit DoF identity corruption must fail");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(reservation.calls, 0);
    }

    #[test]
    fn analytic_and_frequency_reference_scales_are_reported_exactly() {
        let y = [1.1, 2.3, 3.0, 4.4, 5.2, 6.1];
        let x = vec![vec![-2.0, -1.0, 0.0, 1.0, 2.0, 4.0]];
        let analytic = [0.25, 2.0, 1.5, 3.0, 0.75, 4.0];
        let aresult = fit_prepared_ols_unadjusted_map(
            &prepared(&y, &x, None, true, TestWeights::Analytic(&analytic)),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("analytic iid inference");
        assert_eq!(aresult.weights().effective_observations, 6);
        assert_close(
            aresult.weights().raw_weight_sum.expect("raw analytic sum"),
            analytic.iter().sum(),
            1e-12,
        );

        let frequencies = [1_u32, 2, 3, 1, 4, 2];
        let fresult = fit_prepared_ols_unadjusted_map(
            &prepared(&y, &x, None, true, TestWeights::Frequency(&frequencies)),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("frequency iid inference");
        assert_eq!(fresult.weights().physical_observations, 6);
        assert_eq!(fresult.weights().effective_observations, 13);
        assert_eq!(fresult.weights().raw_weight_sum, Some(13.0));
    }

    fn compose_for_test<R: InferenceReservation>(
        prepared: &PreparedOlsData,
        reservation: &mut R,
        perturbations: InferencePerturbations,
    ) -> Result<CertifiedPreparedOlsUnadjusted> {
        let fit = fit_prepared_ols_coefficients_map(prepared, PreparedOlsFitConfig::default())?;
        let absorbed =
            compute_prepared_ols_absorbed_dof(prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)?;
        compose_internal(
            prepared,
            fit,
            absorbed,
            PreparedOlsUnadjustedConfig::default().certificate_tolerance,
            reservation,
            perturbations,
        )
    }

    fn prepared(
        outcome: &[f64],
        regressors: &[Vec<f64>],
        fixed_effect: Option<&[f64]>,
        include_intercept: bool,
        weights: TestWeights<'_>,
    ) -> PreparedOlsData {
        assert!(regressors
            .iter()
            .all(|column| column.len() == outcome.len()));
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
        let weight_position = u32::try_from(bindings.len() + 1).expect("small weight binding");
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
        .expect("prepared iid test case")
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
