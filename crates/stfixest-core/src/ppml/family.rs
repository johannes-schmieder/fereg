use crate::error::{ErrorCode, Result, StFixestError};

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
            eta_min: -29.9,
            eta_max: 29.9,
            mu_min: 1e-13,
            mu_max: 1e13,
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
            && self.mu_min < self.mu_max
            && self.eta_min.exp() >= self.mu_min * (1.0 - 1e-12)
            && self.eta_max.exp() <= self.mu_max * (1.0 + 1e-12);
        if valid {
            Ok(())
        } else {
            Err(StFixestError::new(
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

pub fn means_from_eta(eta: &[f64], bounds: PoissonBounds, mean: &mut [f64]) -> Result<()> {
    bounds.validate()?;
    if eta.len() != mean.len() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "eta and mean buffers have different lengths",
        ));
    }
    for (row, (linear_predictor, output)) in eta.iter().zip(mean).enumerate() {
        if !linear_predictor.is_finite()
            || *linear_predictor < bounds.eta_min
            || *linear_predictor > bounds.eta_max
        {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                format!("Poisson linear predictor at observation {row} is outside bounds"),
            ));
        }
        let value = linear_predictor.exp();
        if !value.is_finite() || value < bounds.mu_min || value > bounds.mu_max {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                format!("Poisson mean at observation {row} is outside bounds"),
            ));
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
    if outcome.len() != mean.len() || outcome.len() != prior_weights.len() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "Poisson objective inputs have inconsistent lengths",
        ));
    }
    let mut deviance_terms = Vec::with_capacity(outcome.len());
    let mut log_likelihood_terms = Vec::with_capacity(outcome.len());
    for row in 0..outcome.len() {
        let y = outcome[row];
        let mu = mean[row];
        let weight = prior_weights[row];
        if !y.is_finite() || y < 0.0 || !mu.is_finite() || mu <= 0.0 {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                format!("invalid Poisson outcome or mean at observation {row}"),
            ));
        }
        if !weight.is_finite() || weight <= 0.0 {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                format!("invalid Poisson prior weight at observation {row}"),
            ));
        }
        let deviance = if y == 0.0 {
            2.0 * mu
        } else {
            2.0 * (y * (y / mu).ln() - (y - mu))
        };
        deviance_terms.push(weight * deviance.max(0.0));
        log_likelihood_terms.push(weight * (y * mu.ln() - mu - log_gamma(y + 1.0)));
    }
    let objective = PoissonObjective {
        deviance: pairwise_sum(&deviance_terms),
        log_likelihood: pairwise_sum(&log_likelihood_terms),
    };
    if objective.deviance.is_finite() && objective.log_likelihood.is_finite() {
        Ok(objective)
    } else {
        Err(StFixestError::new(
            ErrorCode::InvalidData,
            "Poisson objective overflowed",
        ))
    }
}

pub fn offset_from_exposure(exposure: &[f64]) -> Result<Vec<f64>> {
    exposure
        .iter()
        .enumerate()
        .map(|(row, value)| {
            if value.is_finite() && *value > 0.0 {
                Ok(value.ln())
            } else {
                Err(StFixestError::new(
                    ErrorCode::InvalidData,
                    format!("exposure at observation {row} must be finite and positive"),
                ))
            }
        })
        .collect()
}

pub(crate) fn pairwise_sum(values: &[f64]) -> f64 {
    const LEAF: usize = 128;
    if values.len() <= LEAF {
        let mut sum = 0.0_f64;
        let mut compensation = 0.0_f64;
        for value in values {
            let adjusted = *value - compensation;
            let next = sum + adjusted;
            compensation = (next - sum) - adjusted;
            sum = next;
        }
        return sum;
    }
    let middle = values.len() / 2;
    pairwise_sum(&values[..middle]) + pairwise_sum(&values[middle..])
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
        PoissonBounds::default().validate().expect("bounds");
    }

    #[test]
    fn deviance_is_zero_at_the_mean() {
        let y = [0.0_f64, 0.5, 2.0, 10.0];
        let mean = y.map(|value| value.max(1e-12));
        let objective = poisson_objective(&y, &mean, &[1.0; 4]).expect("objective");
        assert!(objective.deviance < 3e-12);
    }

    #[test]
    fn exposure_requires_strict_positivity() {
        assert_eq!(
            offset_from_exposure(&[1.0, 2.0]).expect("offset"),
            [0.0, 2.0_f64.ln()]
        );
        assert!(offset_from_exposure(&[1.0, 0.0]).is_err());
    }

    #[test]
    fn gamma_matches_factorials() {
        for integer in 0_u32..12 {
            let exact = (1..=integer).fold(1.0_f64, |product, value| product * f64::from(value));
            assert!((log_gamma(f64::from(integer) + 1.0) - exact.ln()).abs() < 1e-12);
        }
    }
}
