use crate::data::{PreparedWeights, WeightKind};
use crate::error::{ErrorCode, Result, StFixestError};

use super::{
    compute_prepared_ols_absorbed_dof, fit_prepared_ols_coefficients_map, CertifiedPreparedOlsFit,
    PreparedOlsAbsorbedDofPolicy, PreparedOlsAbsorbedDofReport, PreparedOlsBreadCoordinate,
    PreparedOlsBreadCoordinateKind, PreparedOlsColumnKind, PreparedOlsData, PreparedOlsFitConfig,
    PreparedOlsFitStatus,
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
    residual_dof: bool,
    covariance: bool,
    within_tss: bool,
    statistic: bool,
}

#[derive(Debug, Clone, Copy)]
struct ReferenceWeightView<'a> {
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
    reference_weight_sum_relative_error: f64,
    tolerance: f64,
}

impl ReferenceWeightView<'_> {
    fn value(self, row: usize) -> Result<f64> {
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
/// covariance.  Probability weights fail closed because the pinned reference
/// changes them to robust inference instead of producing iid covariance.
pub fn fit_prepared_ols_unadjusted_map(
    prepared: &PreparedOlsData,
    config: PreparedOlsUnadjustedConfig,
) -> Result<CertifiedPreparedOlsUnadjusted> {
    validate_config(config)?;
    if matches!(
        &prepared.sample.weights,
        PreparedWeights::Provided {
            kind: WeightKind::Probability,
            ..
        }
    ) {
        return Err(unsupported(
            "prepared iid covariance is unavailable for probability weights; the pinned reference requires robust inference",
        ));
    }
    let fit = fit_prepared_ols_coefficients_map(prepared, config.fit)?;
    let absorbed_dof = compute_prepared_ols_absorbed_dof(prepared, config.absorbed_dof)?;
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

fn compose_internal<R: InferenceReservation>(
    prepared: &PreparedOlsData,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofReport,
    tolerance: f64,
    reservation: &mut R,
    perturbations: InferencePerturbations,
) -> Result<CertifiedPreparedOlsUnadjusted> {
    validate_fit_and_absorbed_dof(prepared, &fit, &absorbed_dof)?;
    let (weights, weight_view, weight_sum_relative_error) =
        build_reference_weight_report(prepared, &fit, tolerance)?;
    let mut dof = build_residual_dof(prepared, &fit, &absorbed_dof, weights)?;
    if perturbations.residual_dof {
        dof.residual_dof = dof
            .residual_dof
            .checked_add(1)
            .ok_or_else(|| allocation("injected prepared iid residual DoF overflows"))?;
    }

    let outcome_mean = weighted_mean(&prepared.sample.outcome, weight_view)?;
    let center = if dof.has_structural_intercept {
        outcome_mean
    } else {
        0.0
    };
    let certified_tss = weighted_sum_of_squares(&prepared.sample.outcome, weight_view, center)?;
    let certified_within_tss = if prepared.fe_topology.effects().is_empty() {
        certified_tss
    } else {
        fit.initial_absorbed_outcome_sum_of_squares()
    };
    let certified_rss = weighted_sum_of_squares(fit.residuals(), weight_view, 0.0)?;
    let mut within_tss = certified_within_tss;
    if perturbations.within_tss {
        within_tss *= 1.25;
    }
    let mut rss = certified_rss;
    if perturbations.rss {
        rss *= 1.25;
    }
    validate_sum_of_squares(rss, certified_tss, within_tss)?;

    let mut statistics = build_statistics(rss, certified_tss, within_tss, weights, dof)?;
    if perturbations.statistic {
        statistics.root_mean_squared_error += 0.25;
    }
    let mut covariance = build_covariance(&fit, rss, dof.residual_dof, reservation)?;
    if perturbations.covariance && !covariance.values.is_empty() {
        covariance.values[0] += 0.25;
    }
    let certificate = certify_inference(
        &fit,
        weights,
        dof,
        statistics,
        &covariance,
        InferenceCertificateInputs {
            rss: certified_rss,
            tss: certified_tss,
            within_tss: certified_within_tss,
            reference_weight_sum_relative_error: weight_sum_relative_error,
            tolerance,
        },
    )?;
    if !certificate.passed {
        return Err(invalid_data(
            "prepared iid covariance or fit-statistic certificate did not pass",
        ));
    }
    Ok(CertifiedPreparedOlsUnadjusted {
        fit,
        absorbed_dof,
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

fn validate_fit_and_absorbed_dof(
    prepared: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    absorbed: &PreparedOlsAbsorbedDofReport,
) -> Result<()> {
    let nobs = prepared.sample.n_active();
    if nobs == 0
        || fit.residuals().len() != nobs
        || fit.coefficients().len() != prepared.model.coefficients.len()
        || !fit.rank_certificate().passed
        || !fit.score_certificate().passed
        || !fit.bread().certificate().passed
        || !fit.absorption_report().certified()
        || !fit.residual_absorption_report().certified()
    {
        return Err(invalid_argument(
            "prepared iid fit is not a complete certified result for the final sample",
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
    for (prepared_coefficient, fitted) in prepared.model.coefficients.iter().zip(fit.coefficients())
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
            .model
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
        || (normalized_coordinates == 1 && prepared.fe_topology.effects().is_empty())
    {
        return Err(invalid_argument(
            "prepared iid covariance-coordinate rank is inconsistent",
        ));
    }

    if absorbed.effects.len() != prepared.fe_topology.effects().len() {
        return Err(invalid_argument(
            "prepared iid absorbed-DoF report dimensions are inconsistent",
        ));
    }
    let mut initial = 0_usize;
    let mut redundant = 0_usize;
    let mut rank = 0_usize;
    let mut approximate = false;
    for (effect, row) in prepared.fe_topology.effects().iter().zip(&absorbed.effects) {
        let source = prepared
            .fe_topology
            .sources()
            .get(effect.topology_source_index)
            .ok_or_else(|| invalid_argument("prepared iid FE source is out of range"))?;
        if row.term_id != effect.term_id
            || row.label != effect.label
            || row.source != effect.source
            || row.categories != source.n_levels()
            || row.redundant > row.categories
            || row.coefficients != row.categories - row.redundant
            || row.nested
        {
            return Err(invalid_argument(
                "prepared iid absorbed-DoF row is inconsistent",
            ));
        }
        initial = initial
            .checked_add(row.categories)
            .ok_or_else(|| allocation("prepared iid absorbed category total overflows"))?;
        redundant = redundant
            .checked_add(row.redundant)
            .ok_or_else(|| allocation("prepared iid absorbed redundancy total overflows"))?;
        rank = rank
            .checked_add(row.coefficients)
            .ok_or_else(|| allocation("prepared iid absorbed rank total overflows"))?;
        approximate |= !row.redundancy_exact;
    }
    if initial != absorbed.initial_categories
        || redundant != absorbed.redundant_categories
        || rank != absorbed.absorbed_rank
        || approximate != absorbed.is_approximate
    {
        return Err(invalid_argument(
            "prepared iid absorbed-DoF totals are inconsistent",
        ));
    }
    Ok(())
}

fn build_reference_weight_report<'a>(
    prepared: &'a PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    tolerance: f64,
) -> Result<(
    PreparedOlsReferenceWeightReport,
    ReferenceWeightView<'a>,
    f64,
)> {
    let nobs = prepared.sample.n_active();
    let bread_scale = fit.bread().weight_scale();
    let nobs_f64 = exact_integer_as_f64(nobs as u128)?;
    let (kind, values, effective, raw_weight_sum, expected_divisor, expected_multiplier) =
        match &prepared.sample.weights {
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

fn build_residual_dof(
    prepared: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    absorbed: &PreparedOlsAbsorbedDofReport,
    weights: PreparedOlsReferenceWeightReport,
) -> Result<PreparedOlsResidualDofReport> {
    let mut finite_intercepts = 0_usize;
    let mut normalized_intercepts = 0_usize;
    for coordinate in fit.bread().coordinates() {
        match coordinate.kind {
            PreparedOlsBreadCoordinateKind::FiniteRegressor => {
                if matches!(
                    prepared
                        .model
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
    let finite_model_dof = fit
        .finite_rank()
        .checked_sub(finite_intercepts)
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
            invalid_data("prepared iid model has no positive residual degrees of freedom")
        })?;
    let has_structural_intercept = finite_intercepts == 1
        || normalized_intercepts == 1
        || !prepared.fe_topology.effects().is_empty();
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

fn build_statistics(
    rss: f64,
    tss: f64,
    within_tss: f64,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
) -> Result<PreparedOlsFitStatistics> {
    let n = exact_integer_as_f64(weights.effective_observations as u128)?;
    let df_r = exact_integer_as_f64(dof.residual_dof as u128)?;
    let df_m = exact_integer_as_f64(dof.finite_model_dof as u128)?;
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
    let explained_within = within_tss - rss;
    let f_statistic = if dof.finite_model_dof > 0 && rss > 0.0 && explained_within >= 0.0 {
        Some((explained_within / df_m) / (rss / df_r))
    } else {
        None
    };
    let gaussian_log_likelihood = gaussian_log_likelihood_from_ss(rss, n)?;
    let gaussian_null_log_likelihood = gaussian_log_likelihood_from_ss(within_tss, n)?;
    for value in [
        r_squared,
        adjusted_r_squared,
        within_r_squared,
        adjusted_within_r_squared,
        f_statistic,
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
        f_statistic,
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

fn certify_inference(
    fit: &CertifiedPreparedOlsFit,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: &PreparedOlsUnadjustedCovariance,
    inputs: InferenceCertificateInputs,
) -> Result<PreparedOlsUnadjustedCertificate> {
    let expected_model_rank = dof
        .finite_rank
        .checked_add(dof.absorbed_rank)
        .ok_or_else(|| allocation("prepared iid certificate model rank overflows"))?;
    let expected_residual = weights
        .effective_observations
        .checked_sub(expected_model_rank)
        .ok_or_else(|| invalid_data("prepared iid certificate residual DoF underflowed"))?;
    if dof.model_rank != expected_model_rank
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
    let expected_scale = statistics.rss / exact_integer_as_f64(dof.residual_dof as u128)?;
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

    let expected_statistics = build_statistics(
        statistics.rss,
        statistics.tss,
        statistics.within_tss,
        weights,
        dof,
    )?;
    let mut statistic_formula_relative_infinity_norm = relative_difference(
        statistics.root_mean_squared_error,
        expected_statistics.root_mean_squared_error,
        expected_statistics.root_mean_squared_error.abs().max(1.0),
    )?;
    for (actual, expected) in [
        (statistics.r_squared, expected_statistics.r_squared),
        (
            statistics.adjusted_r_squared,
            expected_statistics.adjusted_r_squared,
        ),
        (
            statistics.within_r_squared,
            expected_statistics.within_r_squared,
        ),
        (
            statistics.adjusted_within_r_squared,
            expected_statistics.adjusted_within_r_squared,
        ),
        (statistics.f_statistic, expected_statistics.f_statistic),
        (
            statistics.gaussian_log_likelihood,
            expected_statistics.gaussian_log_likelihood,
        ),
        (
            statistics.gaussian_null_log_likelihood,
            expected_statistics.gaussian_null_log_likelihood,
        ),
    ] {
        statistic_formula_relative_infinity_norm = statistic_formula_relative_infinity_norm
            .max(optional_relative_difference(actual, expected)?);
    }
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

fn validate_sum_of_squares(rss: f64, tss: f64, within_tss: f64) -> Result<()> {
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

fn weighted_mean(values: &[f64], weights: ReferenceWeightView<'_>) -> Result<f64> {
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

fn weighted_sum_of_squares(
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

fn optional_relative_difference(actual: Option<f64>, expected: Option<f64>) -> Result<f64> {
    match (actual, expected) {
        (None, None) => Ok(0.0),
        (Some(actual), Some(expected)) => {
            relative_difference(actual, expected, actual.abs().max(expected.abs()).max(1.0))
        }
        _ => Ok(1.0),
    }
}

fn relative_difference(actual: f64, expected: f64, scale: f64) -> Result<f64> {
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

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
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

fn exact_integer_as_f64(value: u128) -> Result<f64> {
    const MAX_EXACT_F64_INTEGER: u128 = 1_u128 << 53;
    if value > MAX_EXACT_F64_INTEGER {
        return Err(invalid_data(
            "prepared iid effective observation count exceeds exact f64 range",
        ));
    }
    Ok(value as f64)
}

fn checked_square(value: usize, message: &'static str) -> Result<usize> {
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
        assert_eq!(baseline.calls, 2);

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
                residual_dof: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                covariance: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                within_tss: true,
                ..InferencePerturbations::default()
            },
            InferencePerturbations {
                statistic: true,
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
    fn rank_zero_intercept_only_and_fe_only_edges_are_explicit() {
        let y = [2.0, 3.5, 1.0, 4.0, 5.0, 2.5];
        let no_intercept = fit_prepared_ols_unadjusted_map(
            &prepared(&y, &[], None, false, TestWeights::Unit),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("rank-zero no-intercept inference");
        assert_eq!(no_intercept.dof().model_rank, 0);
        assert_eq!(no_intercept.dof().residual_dof, y.len());
        assert!(!no_intercept.dof().has_structural_intercept);
        assert_eq!(no_intercept.covariance().dimension(), 0);
        assert_close(
            no_intercept.statistics().tss,
            y.iter().map(|value| value * value).sum(),
            1e-12,
        );
        assert!(no_intercept.statistics().f_statistic.is_none());

        let constant = [0.0; 6];
        let intercept_only = fit_prepared_ols_unadjusted_map(
            &prepared(&constant, &[], None, true, TestWeights::Unit),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("intercept-only exact inference");
        assert_eq!(intercept_only.dof().finite_rank, 1);
        assert_eq!(intercept_only.dof().finite_model_dof, 0);
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
        assert_eq!(error.code, ErrorCode::InvalidData);

        let empty = prepared(&[], &[], None, false, TestWeights::Unit);
        let error = fit_prepared_ols_unadjusted_map(&empty, PreparedOlsUnadjustedConfig::default())
            .expect_err("empty inference sample must fail");
        assert_eq!(error.code, ErrorCode::InvalidData);

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
        let prepared = prepared(
            &[1.0, 1.2, 2.5, 2.8, 4.1, 4.4],
            &[vec![-2.0, 0.5, -1.0, 2.0, 1.5, 4.0]],
            Some(&groups),
            true,
            TestWeights::Unit,
        );
        let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("certified fit");
        let mut absorbed =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("certified absorbed DoF");
        absorbed.effects[0].categories += 1;
        absorbed.initial_categories += 1;
        absorbed.absorbed_rank += 1;
        let mut reservation = CountingReservation::default();
        let error = compose_internal(
            &prepared,
            fit,
            absorbed,
            PreparedOlsUnadjustedConfig::default().certificate_tolerance,
            &mut reservation,
            InferencePerturbations::default(),
        )
        .expect_err("corrupted absorbed-DoF report must fail");
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
}
