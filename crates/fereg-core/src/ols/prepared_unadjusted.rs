use super::prepared_wald::fitted_coordinate_value;
use crate::data::{PreparedWeights, WeightKind};
use crate::error::{
    allocation, invalid_argument, invalid_data, unsupported, ErrorCode, FeregError, Result,
};
use crate::linalg::neumaier_add;
pub(super) use crate::sandwich::compensated_add_scaled_row;
use crate::topology_dof::CategoricalTopologyDofReport;
use std::time::Instant;

use super::prepared_inference_view::{PreparedOlsAbsorbedDofSummary, PreparedOlsInferenceView};

#[cfg(test)]
use super::fit_prepared_model_coefficients;
#[cfg(test)]
use super::prepared_inference_view::compute_canonical_prepared_ols_absorbed_dof;
use super::{
    CertifiedPreparedOlsFit, PreparedOlsAbsorbedDofPolicy, PreparedOlsBreadCoordinateKind,
    PreparedOlsColumnKind, PreparedOlsData, PreparedOlsFitConfig, PreparedOlsFitStatus,
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

/// Same-call certified fit with its unadjusted covariance payload.
pub type CertifiedPreparedOlsUnadjusted<A = CategoricalTopologyDofReport> =
    super::prepared_products::CertifiedPreparedOlsInference<
        A,
        PreparedOlsResidualDofReport,
        PreparedOlsUnadjustedCovariance,
        PreparedOlsUnadjustedCertificate,
    >;

impl super::prepared_products::PreparedOlsInferenceBuffers for PreparedOlsUnadjustedCovariance {
    fn inference_owned_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        bytes(&self.values) + bytes(&self.standard_errors)
    }
}

type PreparedOlsUnadjustedComponents = CertifiedPreparedOlsUnadjusted<()>;

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
struct ReferenceWeightFacts {
    kind: Option<WeightKind>,
    effective: usize,
    raw_weight_sum: Option<f64>,
    divisor: f64,
    expected_multiplier: f64,
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

#[cfg(test)]
impl<'a> ReferenceWeightView<'a> {
    pub(super) fn from_test_parts(
        values: Option<&'a [f64]>,
        kind: Option<WeightKind>,
        divisor: f64,
        multiplier: f64,
    ) -> Self {
        Self {
            values,
            kind,
            divisor,
            multiplier,
        }
    }
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

/// Fit and certify conventional homoskedastic covariance through the owner's
/// declared ordinary or categorical projection semantics. Probability weights
/// fail closed because the pinned reference changes them to robust inference
/// instead of producing iid covariance.
pub fn fit_prepared_model_unadjusted_with_loaded_moments(
    prepared: &PreparedOlsData,
    config: PreparedOlsUnadjustedConfig,
    rows: Option<super::LoadedOlsRows>,
) -> Result<CertifiedPreparedOlsUnadjusted> {
    fit_inference_view(
        prepared,
        config,
        rows,
        prepared.uses_categorical_preparation(),
    )
}

fn fit_inference_view<P: PreparedOlsInferenceView + Sync + ?Sized>(
    prepared: &P,
    config: PreparedOlsUnadjustedConfig,
    rows: Option<super::LoadedOlsRows>,
    defer_dof: bool,
) -> Result<CertifiedPreparedOlsUnadjusted> {
    validate_config(config)?;
    reject_probability_iid(prepared)?;
    let fit = match rows {
        Some(rows) => super::prepared_fit::fit_prepared_ols_coefficients_with_loaded_rows(
            prepared, config.fit, rows,
        ),
        None => super::prepared_fit::fit_prepared_ols_coefficients_view_map(prepared, config.fit),
    }?;
    // Preserve ordinary metadata errors before team entry and categorical
    // prefix errors inside it. Both paths consume the same accepted fit.
    let early_dof = if defer_dof {
        None
    } else {
        Some(prepared.inference_compute_absorbed_dof(config.absorbed_dof)?)
    };
    crate::ThreadTeam::new(config.fit.execution.threads).install(|| {
        let absorbed_dof = match early_dof {
            Some(report) => report,
            None => prepared.inference_compute_absorbed_dof(config.absorbed_dof)?,
        };
        let summary =
            prepared.inference_absorbed_dof_summary(&absorbed_dof, config.absorbed_dof)?;
        let mut reservation = SystemInferenceReservation;
        let components = compose_view_internal(
            prepared,
            fit,
            summary,
            config.certificate_tolerance,
            &mut reservation,
            InferencePerturbations::default(),
        )?;
        Ok(components.with_absorbed_dof(absorbed_dof))
    })?
}

pub(super) fn independent_residual_sum(
    fit: &CertifiedPreparedOlsFit,
    weight_view: ReferenceWeightView<'_>,
) -> Result<f64> {
    weighted_sum_of_squares(fit.residuals(), weight_view, 0.0)
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

#[cfg(test)]
fn compose_internal<R: InferenceReservation>(
    prepared: &PreparedOlsData,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: CategoricalTopologyDofReport,
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
    Ok(components.with_absorbed_dof(absorbed_dof))
}

fn compose_view_internal<P: PreparedOlsInferenceView + ?Sized, R: InferenceReservation>(
    prepared: &P,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofSummary,
    tolerance: f64,
    reservation: &mut R,
    perturbations: InferencePerturbations,
) -> Result<PreparedOlsUnadjustedComponents> {
    let phase_timing = crate::diagnostics::phase_timing_enabled();
    let total_started = Instant::now();
    let phase_started = Instant::now();
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

    let (certified_tss, certified_rss) = independent_fit_sums(
        &prepared.fit_sample().outcome,
        fit.residuals(),
        weight_view,
        dof.has_structural_intercept,
    )?;
    let certified_within_tss = if prepared.inference_has_explicit_absorbed_effects() {
        fit.initial_absorbed_outcome_sum_of_squares()
    } else {
        certified_tss
    };
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
    let setup_statistics_seconds = phase_started.elapsed().as_secs_f64();
    let phase_started = Instant::now();
    let mut covariance = build_covariance(&fit, rss, dof.residual_dof, reservation)?;
    let covariance_seconds = phase_started.elapsed().as_secs_f64();
    let phase_started = Instant::now();
    let wald = build_wald_f(prepared, &fit, &covariance, dof, reservation)?;
    let wald_seconds = phase_started.elapsed().as_secs_f64();
    let phase_started = Instant::now();
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
    let certificate_seconds = phase_started.elapsed().as_secs_f64();
    if phase_timing {
        eprintln!(
            "FEREG_PHASE|iid|setup_statistics={setup_statistics_seconds:.9}|covariance={covariance_seconds:.9}|wald={wald_seconds:.9}|certificate={certificate_seconds:.9}|total={:.9}",
            total_started.elapsed().as_secs_f64(),
        );
    }
    Ok(PreparedOlsUnadjustedComponents {
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
            && (!prepared.fit_has_absorbed_intercept() || !prepared.fit_report_intercept()))
    {
        return Err(invalid_argument(
            "prepared iid covariance-coordinate rank is inconsistent",
        ));
    }
    Ok(())
}

fn scan_reference_weight_facts<P: PreparedOlsInferenceView + ?Sized>(
    prepared: &P,
    allow_probability: bool,
) -> Result<ReferenceWeightFacts> {
    let nobs = prepared.fit_sample().n_active();
    let nobs_f64 = exact_integer_as_f64(nobs as u128)?;
    let (kind, effective, raw_weight_sum, expected_divisor, expected_multiplier) =
        match &prepared.fit_sample().weights {
            PreparedWeights::None => (None, nobs, None, 1.0, 1.0),
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
                let (raw_sum, _) = positive_weight_sums(values, divisor)?;
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
                        (Some(*kind), effective, Some(exact_sum), divisor, divisor)
                    }
                    WeightKind::Analytic => {
                        let solver_sum = raw_sum / divisor;
                        let multiplier = nobs_f64 / solver_sum;
                        (Some(*kind), nobs, Some(raw_sum), divisor, multiplier)
                    }
                    WeightKind::Probability if allow_probability => {
                        let solver_sum = raw_sum / divisor;
                        let multiplier = nobs_f64 / solver_sum;
                        (Some(*kind), nobs, Some(raw_sum), divisor, multiplier)
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
    Ok(ReferenceWeightFacts {
        kind,
        effective,
        raw_weight_sum,
        divisor: expected_divisor,
        expected_multiplier,
    })
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
    let facts = scan_reference_weight_facts(prepared, allow_probability)?;
    let ReferenceWeightFacts {
        kind,
        effective,
        raw_weight_sum,
        divisor: expected_divisor,
        expected_multiplier,
        ..
    } = facts;
    let values = match &prepared.fit_sample().weights {
        PreparedWeights::None => None,
        PreparedWeights::Provided { values, .. } => Some(values.as_slice()),
    };
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
    let computed_sum = reference_weight_sum(view, nobs)?;
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

fn reference_weight_sum(view: ReferenceWeightView<'_>, nobs: usize) -> Result<f64> {
    if view.kind.is_none() && view.values.is_none() {
        // Unit mass is fixed by the independently validated physical row
        // count, not by the fitted bread's declared weight sum.
        return exact_integer_as_f64(nobs as u128);
    }
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for row in 0..nobs {
        compensated_add(view.value(row)?, &mut sum, &mut correction)?;
    }
    Ok(sum + correction)
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
    let has_absorbed_intercept = prepared.fit_has_absorbed_intercept();
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
    let has_absorbed_intercept = prepared.fit_has_absorbed_intercept();
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

fn independent_fit_sums(
    outcome: &[f64],
    residuals: &[f64],
    weight_view: ReferenceWeightView<'_>,
    has_intercept: bool,
) -> Result<(f64, f64)> {
    let tss = || independent_outcome_sum(outcome, weight_view, has_intercept);
    let rss = || weighted_sum_of_squares(residuals, weight_view, 0.0);
    // These certificates use separate source/residual scans. Overlap their
    // work without changing either reduction or the established error order.
    let (tss_result, rss_result) =
        if crate::parallel::StagePlan::current(outcome.len() as u128 * 2, 2).lanes > 1
            && rayon::current_thread_index().is_some()
        {
            rayon::join(tss, rss)
        } else {
            (tss(), rss())
        };
    Ok((tss_result?, rss_result?))
}

pub(super) fn independent_outcome_sum(
    outcome: &[f64],
    weights: ReferenceWeightView<'_>,
    has_intercept: bool,
) -> Result<f64> {
    let mean = weighted_mean(outcome, weights)?;
    weighted_sum_of_squares(outcome, weights, if has_intercept { mean } else { 0.0 })
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
    // Unit mass sums exactly to N through 2^53; keep the value reduction's
    // scale, compensation and observation order unchanged.
    let denominator = if weights.kind.is_none()
        && weights.values.is_none()
        && (values.len() as u128) <= (1_u128 << 53)
    {
        for value in values {
            compensated_add(
                *value / value_scale,
                &mut numerator,
                &mut numerator_correction,
            )?;
        }
        values.len() as f64
    } else {
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
        denominator + denominator_correction
    };
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
    if weights.kind.is_none() && weights.values.is_none() {
        for value in values.iter().copied() {
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared iid sum-of-squares value is not finite",
                ));
            }
            update_scaled_sum_of_squares(value - center, &mut scale, &mut scaled_square_sum)?;
        }
    } else {
        for (row, value) in values.iter().copied().enumerate() {
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared iid sum-of-squares value is not finite",
                ));
            }
            let weighted = weights.value(row)?.sqrt() * (value - center);
            update_scaled_sum_of_squares(weighted, &mut scale, &mut scaled_square_sum)?;
        }
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

fn positive_weight_sums(values: &[f64], divisor: f64) -> Result<(f64, f64)> {
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for value in values {
        compensated_add(*value / divisor, &mut sum, &mut correction)?;
    }
    let raw = divisor * (sum + correction);
    if raw.is_finite() && raw > 0.0 {
        Ok((raw, sum + correction))
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
    neumaier_add(value, sum, correction);
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

fn insufficient_observations(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InsufficientObservations, message)
}

#[cfg(test)]
mod tests;
