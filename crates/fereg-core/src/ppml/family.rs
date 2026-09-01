use crate::error::{ErrorCode, FeregError, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoissonBounds {
    pub eta_min: f64,
    pub eta_max: f64,
    pub mu_min: f64,
    pub mu_max: f64,
}

impl Default for PoissonBounds {
    fn default() -> Self {
        Self {
            // The defaults impose no modeling-scale clipping.  The exponential
            // evaluation below remains the authority for whether a proposed
            // predictor has a finite, strictly positive representable mean.
            eta_min: f64::MIN,
            eta_max: f64::MAX,
            mu_min: f64::from_bits(1),
            mu_max: f64::MAX,
        }
    }
}

impl PoissonBounds {
    pub fn validate(self) -> Result<()> {
        let valid = self.eta_min.is_finite()
            && self.eta_max.is_finite()
            && self.eta_min < self.eta_max
            && self.mu_min.is_finite()
            && self.mu_max.is_finite()
            && self.mu_min > 0.0
            && self.mu_min < self.mu_max;
        if valid {
            Ok(())
        } else {
            Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "invalid Poisson linear-predictor or mean bounds",
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoissonObjective {
    pub deviance: f64,
    pub log_likelihood: f64,
}

/// Comparator-compatible constant-only null likelihood.
///
/// The null model intentionally ignores absorbed effects and the fitted
/// model's offset. Its mean is the prior-weighted outcome mean on the final
/// PPML sample, as fixed by ADR 0016.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoissonNullObjective {
    pub mean: f64,
    pub log_likelihood: f64,
}

fn numerical_nonconvergence(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

pub fn means_from_eta(eta: &[f64], bounds: PoissonBounds, mean: &mut [f64]) -> Result<()> {
    bounds.validate()?;
    if eta.len() != mean.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "eta and mean buffers have different lengths",
        ));
    }
    for (row, (linear_predictor, output)) in eta.iter().zip(mean).enumerate() {
        if !linear_predictor.is_finite() {
            return Err(numerical_nonconvergence(format!(
                "Poisson linear predictor at observation {row} is nonfinite"
            )));
        }
        if *linear_predictor < bounds.eta_min || *linear_predictor > bounds.eta_max {
            return Err(numerical_nonconvergence(format!(
                "Poisson linear predictor at observation {row} is outside configured bounds"
            )));
        }
        let value = linear_predictor.exp();
        if !value.is_finite() || value <= 0.0 {
            return Err(numerical_nonconvergence(format!(
                "Poisson mean at observation {row} is not representable as a finite positive value; reduce the proposed step"
            )));
        }
        if value < bounds.mu_min || value > bounds.mu_max {
            return Err(numerical_nonconvergence(format!(
                "Poisson mean at observation {row} is outside configured bounds"
            )));
        }
        *output = value;
    }
    Ok(())
}

pub fn poisson_objective(
    outcome: &[f64],
    mean: &[f64],
    prior_weights: &[f64],
) -> Result<PoissonObjective> {
    poisson_objective_impl(outcome, None, mean, prior_weights)
}

pub fn poisson_null_objective(
    outcome: &[f64],
    prior_weights: &[f64],
) -> Result<PoissonNullObjective> {
    if outcome.is_empty() || outcome.len() != prior_weights.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "Poisson null-likelihood inputs are empty or misaligned",
        ));
    }
    let mut maximum_weight_log = f64::NEG_INFINITY;
    let mut maximum_outcome_term_log = f64::NEG_INFINITY;
    for row in 0..outcome.len() {
        let y = outcome[row];
        let weight = prior_weights[row];
        if !y.is_finite() || y < 0.0 || !weight.is_finite() || weight <= 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("invalid Poisson null-likelihood input at observation {row}"),
            ));
        }
        let weight_log = weight.ln();
        maximum_weight_log = maximum_weight_log.max(weight_log);
        if y > 0.0 {
            maximum_outcome_term_log = maximum_outcome_term_log.max(weight_log + y.ln());
        }
    }
    if !maximum_weight_log.is_finite() || !maximum_outcome_term_log.is_finite() {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "Poisson null model has no positive weighted outcome",
        ));
    }
    let mut weight_sum = CompensatedSum::default();
    let mut outcome_sum = CompensatedSum::default();
    for row in 0..outcome.len() {
        let weight_log = prior_weights[row].ln();
        weight_sum.add((weight_log - maximum_weight_log).exp());
        if outcome[row] > 0.0 {
            outcome_sum.add((weight_log + outcome[row].ln() - maximum_outcome_term_log).exp());
        }
    }
    let log_weight_sum = maximum_weight_log + weight_sum.total().ln();
    let log_outcome_sum = maximum_outcome_term_log + outcome_sum.total().ln();
    let log_mean = log_outcome_sum - log_weight_sum;
    let mean = log_mean.exp();
    if !log_mean.is_finite() || !mean.is_finite() || mean <= 0.0 {
        return Err(numerical_nonconvergence(
            "Poisson null mean is not finitely representable",
        ));
    }
    let mut likelihood = CompensatedSum::default();
    for row in 0..outcome.len() {
        let term =
            prior_weights[row] * (outcome[row] * log_mean - mean - log_gamma(outcome[row] + 1.0));
        if !term.is_finite() {
            return Err(numerical_nonconvergence(format!(
                "Poisson null likelihood overflowed at observation {row}"
            )));
        }
        likelihood.add(term);
    }
    let log_likelihood = likelihood.total();
    if log_likelihood.is_finite() {
        Ok(PoissonNullObjective {
            mean,
            log_likelihood,
        })
    } else {
        Err(numerical_nonconvergence(
            "Poisson null likelihood is not finite",
        ))
    }
}

/// Evaluate the objective against the accepted predictor rather than taking
/// `log(exp(eta))` again. This is the production IRLS path; the public
/// mean-only helper above remains as a small mathematical convenience.
pub(crate) fn poisson_objective_from_eta(
    outcome: &[f64],
    eta: &[f64],
    mean: &[f64],
    prior_weights: &[f64],
) -> Result<PoissonObjective> {
    poisson_objective_impl(outcome, Some(eta), mean, prior_weights)
}

fn poisson_objective_impl(
    outcome: &[f64],
    eta: Option<&[f64]>,
    mean: &[f64],
    prior_weights: &[f64],
) -> Result<PoissonObjective> {
    if outcome.len() != mean.len() || outcome.len() != prior_weights.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "Poisson objective inputs have inconsistent lengths",
        ));
    }
    if eta.is_some_and(|values| values.len() != outcome.len()) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "Poisson predictor and objective inputs have inconsistent lengths",
        ));
    }
    let mut deviance_sum = CompensatedSum::default();
    let mut log_likelihood_sum = CompensatedSum::default();
    for row in 0..outcome.len() {
        let y = outcome[row];
        let mu = mean[row];
        let weight = prior_weights[row];
        if !y.is_finite() || y < 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("invalid Poisson outcome at observation {row}"),
            ));
        }
        if !weight.is_finite() || weight <= 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("invalid Poisson prior weight at observation {row}"),
            ));
        }
        if !mu.is_finite() || mu <= 0.0 {
            return Err(numerical_nonconvergence(format!(
                "Poisson mean at observation {row} is outside the finite positive computational domain"
            )));
        }
        let log_mean = eta.map_or_else(|| mu.ln(), |values| values[row]);
        if !log_mean.is_finite() {
            return Err(numerical_nonconvergence(format!(
                "Poisson linear predictor at observation {row} is nonfinite"
            )));
        }
        let half_deviance = poisson_half_deviance_with_log_mean(y, mu, log_mean)?;
        let deviance_term = 2.0 * weight * half_deviance;
        let log_likelihood_term = weight * (y * log_mean - mu - log_gamma(y + 1.0));
        if !deviance_term.is_finite() || !log_likelihood_term.is_finite() {
            return Err(numerical_nonconvergence(format!(
                "Poisson objective overflowed at observation {row}"
            )));
        }
        deviance_sum.add(deviance_term);
        log_likelihood_sum.add(log_likelihood_term);
    }
    let objective = PoissonObjective {
        deviance: deviance_sum.total(),
        log_likelihood: log_likelihood_sum.total(),
    };
    if objective.deviance.is_finite() && objective.log_likelihood.is_finite() {
        Ok(objective)
    } else {
        Err(numerical_nonconvergence("Poisson objective overflowed"))
    }
}

/// Fill one Poisson IRLS frame without allocating observation-scale storage.
///
/// `prior_weights` are the estimator's prior weights.  Consequently frequency
/// weights reproduce literal row replication and probability weights enter the
/// estimating equations multiplicatively; neither changes the working response.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fill_poisson_working_state(
    outcome: &[f64],
    mean: &[f64],
    prior_weights: &[f64],
    eta: &[f64],
    offset: &[f64],
    working_weights: &mut [f64],
    working_response: &mut [f64],
) -> Result<()> {
    let nrows = outcome.len();
    if mean.len() != nrows
        || prior_weights.len() != nrows
        || eta.len() != nrows
        || offset.len() != nrows
        || working_weights.len() != nrows
        || working_response.len() != nrows
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "Poisson working-state buffers have inconsistent lengths",
        ));
    }
    for row in 0..nrows {
        let y = outcome[row];
        let mu = mean[row];
        let prior_weight = prior_weights[row];
        let linear_predictor = eta[row];
        let row_offset = offset[row];
        if !y.is_finite() || y < 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("invalid Poisson outcome at observation {row}"),
            ));
        }
        if !prior_weight.is_finite() || prior_weight <= 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("invalid Poisson prior weight at observation {row}"),
            ));
        }
        if !row_offset.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("invalid Poisson offset at observation {row}"),
            ));
        }
        if !mu.is_finite() || mu <= 0.0 || !linear_predictor.is_finite() {
            return Err(numerical_nonconvergence(format!(
                "Poisson fitted state at observation {row} is outside the finite computational domain"
            )));
        }
        let weight = prior_weight * mu;
        let score_adjustment = if y == 0.0 {
            -1.0
        } else {
            // Form y/mu in log space so a representable ratio is not lost to
            // premature division under extreme but otherwise valid offsets.
            (y.ln() - linear_predictor).exp() - 1.0
        };
        let response = linear_predictor - row_offset + score_adjustment;
        if !weight.is_finite() || weight <= 0.0 || !response.is_finite() {
            return Err(numerical_nonconvergence(format!(
                "Poisson working state at observation {row} is not finitely representable"
            )));
        }
        working_weights[row] = weight;
        working_response[row] = response;
    }
    Ok(())
}

pub fn offset_from_exposure(exposure: &[f64]) -> Result<Vec<f64>> {
    let mut offset = fallible_output(
        exposure.len(),
        "unable to reserve PPML exposure-offset output",
    )?;
    for (row, value) in exposure.iter().enumerate() {
        if value.is_finite() && *value > 0.0 {
            offset.push(value.ln());
        } else {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("exposure at observation {row} must be finite and positive"),
            ));
        }
    }
    Ok(offset)
}

fn fallible_output<T>(length: usize, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    Ok(output)
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CompensatedSum {
    sum: f64,
    correction: f64,
}

impl CompensatedSum {
    pub(crate) fn add(&mut self, value: f64) {
        let next = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        self.sum = next;
    }

    pub(crate) fn total(self) -> f64 {
        self.sum + self.correction
    }
}

fn poisson_half_deviance_with_log_mean(y: f64, mu: f64, log_mean: f64) -> Result<f64> {
    if y == 0.0 {
        return Ok(mu);
    }
    if y == mu {
        return Ok(0.0);
    }
    let difference = y - mu;
    let relative_difference = difference / mu;
    let value = if relative_difference.is_finite() && relative_difference.abs() <= 0.5 {
        mu * one_plus_x_log1p_minus_x(relative_difference)
    } else {
        y * (y.ln() - log_mean) - difference
    };
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(numerical_nonconvergence(
            "Poisson deviance is not finitely representable",
        ))
    }
}

/// Stable unweighted per-row deviance contribution used by public PPML
/// residual output. The public convention is the nonnegative contribution,
/// not a signed square-root residual.
pub(crate) fn poisson_deviance_contribution(y: f64, mu: f64) -> Result<f64> {
    if !y.is_finite() || y < 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "Poisson deviance outcome is outside its domain",
        ));
    }
    if !mu.is_finite() || mu <= 0.0 {
        return Err(numerical_nonconvergence(
            "Poisson deviance mean is outside the finite positive computational domain",
        ));
    }
    let value = 2.0 * poisson_half_deviance_with_log_mean(y, mu, mu.ln())?;
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(numerical_nonconvergence(
            "Poisson deviance contribution is not finitely representable",
        ))
    }
}

fn one_plus_x_log1p_minus_x(value: f64) -> f64 {
    if value.abs() > 1e-4 {
        return (1.0 + value) * value.ln_1p() - value;
    }
    // (1+x)log(1+x)-x = sum_{k=2}^inf (-1)^k x^k/[k(k-1)].
    // Twelve terms leave the truncation far below one ulp at this cutoff.
    let mut power = value * value;
    let mut result = 0.5 * power;
    for order in 3_i32..=12 {
        power *= value;
        let sign = if order % 2 == 0 { 1.0 } else { -1.0 };
        result += sign * power / f64::from(order * (order - 1));
    }
    result
}

fn log_gamma(x: f64) -> f64 {
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let shifted = x - 1.0;
    let mut series = C[0];
    for (index, coefficient) in C.iter().enumerate().skip(1) {
        series += coefficient / (shifted + index as f64);
    }
    let t = shifted + 7.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (shifted + 0.5) * t.ln() - t + series.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_self_consistent() {
        let bounds = PoissonBounds::default();
        bounds.validate().expect("bounds");
        assert_eq!(bounds.eta_min, f64::MIN);
        assert_eq!(bounds.eta_max, f64::MAX);

        let mut mean = [0.0_f64; 2];
        means_from_eta(&[-700.0, 700.0], bounds, &mut mean).expect("representable means");
        assert!(mean[0].is_finite() && mean[0] > 0.0);
        assert!(mean[1].is_finite() && mean[1] > 1e300);

        let error = means_from_eta(&[1_000.0], bounds, &mut [0.0])
            .expect_err("overflowing exponential must request backtracking");
        assert!(error.message.contains("reduce the proposed step"));
    }

    #[test]
    fn exposure_output_capacity_failure_is_allocation_r909() {
        let error =
            fallible_output::<f64>(usize::MAX, "unable to reserve PPML exposure-offset output")
                .expect_err("impossible exposure-output capacity must fail before allocation");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(error.return_code(), 909);
    }

    #[test]
    fn invalid_raw_domains_remain_invalid_data_r459() {
        let errors = [
            poisson_objective(&[-1.0], &[1.0], &[1.0])
                .expect_err("negative outcome must remain a domain error"),
            poisson_objective(&[1.0], &[1.0], &[0.0])
                .expect_err("zero prior weight must remain a domain error"),
            poisson_null_objective(&[1.0], &[f64::NAN])
                .expect_err("nonfinite null-model weight must remain a domain error"),
            offset_from_exposure(&[0.0]).expect_err("zero exposure must remain a domain error"),
            poisson_deviance_contribution(-1.0, 1.0)
                .expect_err("negative deviance outcome must remain a domain error"),
        ];
        for error in errors {
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert_eq!(error.return_code(), 459);
        }
    }

    #[test]
    fn finite_domain_representability_failures_are_nonconvergence_r430() {
        let mean_error = means_from_eta(&[1_000.0], PoissonBounds::default(), &mut [0.0])
            .expect_err("finite eta with an overflowing exponential must be numerical failure");
        let objective_error = poisson_objective(&[1e307], &[1.0], &[1.0])
            .expect_err("finite-valid objective arithmetic may overflow");
        let null_error = poisson_null_objective(&[1e307], &[1.0])
            .expect_err("finite-valid null likelihood arithmetic may overflow");

        let mut working_weight = [0.0_f64];
        let mut working_response = [0.0_f64];
        let working_error = fill_poisson_working_state(
            &[1.0],
            &[f64::MAX],
            &[2.0],
            &[f64::MAX.ln()],
            &[0.0],
            &mut working_weight,
            &mut working_response,
        )
        .expect_err("finite-valid Hessian weight multiplication may overflow");
        let deviance_error = poisson_deviance_contribution(f64::MAX, 1.0)
            .expect_err("finite-valid deviance arithmetic may overflow");

        for error in [
            mean_error,
            objective_error,
            null_error,
            working_error,
            deviance_error,
        ] {
            assert_eq!(error.code, ErrorCode::NonConvergence);
            assert_eq!(error.return_code(), 430);
        }
    }

    #[test]
    fn deviance_is_zero_at_the_mean() {
        let y = [0.0_f64, 0.5, 2.0, 10.0];
        let mean = y.map(|value| value.max(1e-12));
        let objective = poisson_objective(&y, &mean, &[1.0; 4]).expect("objective");
        assert!(objective.deviance < 3e-12);
    }

    #[test]
    fn fractional_outcomes_follow_the_continuous_poisson_objective() {
        let y = 0.5_f64;
        let mu = 1.25_f64;
        let weight = 2.5_f64;
        let objective = poisson_objective(&[y], &[mu], &[weight]).expect("objective");
        let expected_deviance = 2.0 * weight * (y * (y.ln() - mu.ln()) - (y - mu));
        let expected_log_likelihood = weight * (y * mu.ln() - mu - log_gamma(y + 1.0));
        assert!((objective.deviance - expected_deviance).abs() < 1e-14);
        assert!((objective.log_likelihood - expected_log_likelihood).abs() < 1e-14);
    }

    #[test]
    fn deviance_remains_positive_for_nearly_equal_fractional_values() {
        let y = 1.0_f64;
        let mu = 1.0 + 1e-12;
        let objective = poisson_objective(&[y], &[mu], &[1.0]).expect("objective");
        assert!(objective.deviance > 0.0);
        let second_order = (mu - y).powi(2) / mu;
        assert!((objective.deviance - second_order).abs() <= 2e-12 * second_order);
    }

    #[test]
    fn frequency_weights_equal_literal_replication() {
        let outcome = [0.25_f64, 2.5, 4.0];
        let mean = [0.5_f64, 2.0, 3.5];
        let frequency = [2_usize, 1, 3];
        let compact_weights = frequency.map(|value| value as f64);
        let compact = poisson_objective(&outcome, &mean, &compact_weights).expect("compact");

        let mut expanded_outcome = Vec::new();
        let mut expanded_mean = Vec::new();
        for row in 0..outcome.len() {
            for _ in 0..frequency[row] {
                expanded_outcome.push(outcome[row]);
                expanded_mean.push(mean[row]);
            }
        }
        let expanded = poisson_objective(
            &expanded_outcome,
            &expanded_mean,
            &vec![1.0; expanded_outcome.len()],
        )
        .expect("expanded");
        assert!((compact.deviance - expanded.deviance).abs() < 1e-14);
        assert!((compact.log_likelihood - expanded.log_likelihood).abs() < 1e-14);
    }

    #[test]
    fn prior_weight_scaling_changes_hessian_and_objective_but_not_working_response() {
        let outcome = [2.5_f64, 0.5];
        let mean = [2.0_f64, 1.0];
        let eta = [0.9_f64, -0.1];
        let offset = [0.2_f64, -0.3];
        let probability_weights = [0.75_f64, 2.25];
        let scaled_weights = probability_weights.map(|value| 4.0 * value);
        let mut first_hessian = [0.0_f64; 2];
        let mut first_response = [0.0_f64; 2];
        let mut scaled_hessian = [0.0_f64; 2];
        let mut scaled_response = [0.0_f64; 2];
        fill_poisson_working_state(
            &outcome,
            &mean,
            &probability_weights,
            &eta,
            &offset,
            &mut first_hessian,
            &mut first_response,
        )
        .expect("pweight frame");
        fill_poisson_working_state(
            &outcome,
            &mean,
            &scaled_weights,
            &eta,
            &offset,
            &mut scaled_hessian,
            &mut scaled_response,
        )
        .expect("scaled pweight frame");
        assert_eq!(first_response, scaled_response);
        for row in 0..outcome.len() {
            assert_eq!(first_hessian[row], probability_weights[row] * mean[row]);
            assert_eq!(scaled_hessian[row], 4.0 * first_hessian[row]);
        }
        let first_objective =
            poisson_objective(&outcome, &mean, &probability_weights).expect("objective");
        let scaled_objective =
            poisson_objective(&outcome, &mean, &scaled_weights).expect("scaled objective");
        assert!((scaled_objective.deviance - 4.0 * first_objective.deviance).abs() < 1e-14);
        assert!(
            (scaled_objective.log_likelihood - 4.0 * first_objective.log_likelihood).abs() < 1e-14
        );
    }

    #[test]
    fn zero_outcome_working_response_uses_the_exact_minus_one_score_adjustment() {
        let mut working_weight = [0.0_f64];
        let mut working_response = [0.0_f64];
        fill_poisson_working_state(
            &[0.0],
            &[f64::from_bits(1)],
            &[2.0],
            &[-744.0],
            &[3.0],
            &mut working_weight,
            &mut working_response,
        )
        .expect("zero-outcome frame");
        assert_eq!(working_response[0], -748.0);
        assert!(working_weight[0] > 0.0);
    }

    #[test]
    fn buffered_family_helpers_reuse_caller_storage() {
        let eta = [0.1_f64, -0.4, 0.7, 1.2];
        let offset = [0.0_f64, -0.2, 0.3, 0.5];
        let outcome = [1.5_f64, 0.25, 3.0, 4.5];
        let weights = [1.0_f64, 2.0, 0.75, 3.0];
        let mut mean = vec![0.0_f64; eta.len()];
        let mut working_weights = vec![0.0_f64; eta.len()];
        let mut working_response = vec![0.0_f64; eta.len()];
        let mean_pointer = mean.as_ptr();
        let weight_pointer = working_weights.as_ptr();
        let response_pointer = working_response.as_ptr();
        let capacities = (
            mean.capacity(),
            working_weights.capacity(),
            working_response.capacity(),
        );
        for _ in 0..128 {
            means_from_eta(&eta, PoissonBounds::default(), &mut mean).expect("means");
            fill_poisson_working_state(
                &outcome,
                &mean,
                &weights,
                &eta,
                &offset,
                &mut working_weights,
                &mut working_response,
            )
            .expect("working frame");
            assert!(poisson_objective(&outcome, &mean, &weights)
                .expect("objective")
                .deviance
                .is_finite());
        }
        assert_eq!(mean.as_ptr(), mean_pointer);
        assert_eq!(working_weights.as_ptr(), weight_pointer);
        assert_eq!(working_response.as_ptr(), response_pointer);
        assert_eq!(
            (
                mean.capacity(),
                working_weights.capacity(),
                working_response.capacity(),
            ),
            capacities
        );
    }

    #[test]
    fn exposure_requires_strict_positivity() {
        assert_eq!(
            offset_from_exposure(&[1.0, 2.0]).expect("offset"),
            [0.0, 2.0_f64.ln()]
        );
        assert!(offset_from_exposure(&[1.0, 0.0]).is_err());

        let extreme =
            offset_from_exposure(&[f64::MIN_POSITIVE, f64::MAX]).expect("finite extreme exposures");
        assert!(extreme.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn gamma_matches_factorials() {
        for integer in 0_u32..12 {
            let exact = (1..=integer).fold(1.0_f64, |product, value| product * f64::from(value));
            assert!((log_gamma(f64::from(integer) + 1.0) - exact.ln()).abs() < 1e-12);
        }
    }
}
