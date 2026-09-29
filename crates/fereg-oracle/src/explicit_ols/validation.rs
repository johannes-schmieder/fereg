//! Harvested from fereg-testkit/src/explicit_ols.rs at legacy-v0.5.0-beta.
//! Independent dense oracle; no production numerical implementation is called.

use super::*;

pub(super) struct ValidatedProblem {
    /// Objective weights are internally normalized for WLS so a harmless
    /// common scale cannot underflow robust scores.
    pub(super) objective_weights: Vec<f64>,
    pub(super) sqrt_weights: Vec<f64>,
    pub(super) hc0_multipliers: Vec<f64>,
    /// Converts normalized WLS objective quantities back to the caller's
    /// weight scale. Unit and frequency weights use one.
    pub(super) raw_weight_scale: f64,
    pub(super) effective_observations: usize,
}

pub(super) fn validate_problem(
    problem: &ExplicitOlsProblem,
    config: OracleConfig,
) -> Result<ValidatedProblem, OracleError> {
    let nobs = problem.outcome.len();
    if nobs == 0 {
        return Err(OracleError::InvalidInput(
            "the outcome must contain at least one observation".to_owned(),
        ));
    }
    if !config.rank_tolerance_multiplier.is_finite()
        || config.rank_tolerance_multiplier <= 0.0
        || !config.certificate_tolerance.is_finite()
        || config.certificate_tolerance <= 0.0
    {
        return Err(OracleError::InvalidInput(
            "numerical tolerances must be finite and positive".to_owned(),
        ));
    }
    if problem.outcome.iter().any(|value| !value.is_finite()) {
        return Err(OracleError::InvalidInput(
            "outcomes must be finite".to_owned(),
        ));
    }
    if problem.regressors.iter().any(|column| column.len() != nobs) {
        return Err(OracleError::InvalidInput(
            "regressor and outcome lengths differ".to_owned(),
        ));
    }
    if problem
        .regressors
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err(OracleError::InvalidInput(
            "regressors must be finite".to_owned(),
        ));
    }
    for effect in &problem.effects {
        if effect.codes.len() != nobs || effect.slopes.iter().any(|slope| slope.len() != nobs) {
            return Err(OracleError::InvalidInput(
                "absorbed-effect and outcome lengths differ".to_owned(),
            ));
        }
        if effect
            .codes
            .iter()
            .any(|&code| code as usize >= effect.n_levels)
        {
            return Err(OracleError::InvalidInput(
                "an absorbed-effect code exceeds its declared level count".to_owned(),
            ));
        }
        if effect
            .slopes
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(OracleError::InvalidInput(
                "absorbed slopes must be finite".to_owned(),
            ));
        }
    }
    if problem.clusters.len() > 10 {
        return Err(OracleError::InvalidInput(
            "the small-model oracle supports at most ten cluster dimensions".to_owned(),
        ));
    }
    if problem.clusters.iter().any(|cluster| cluster.len() != nobs) {
        return Err(OracleError::InvalidInput(
            "cluster and outcome lengths differ".to_owned(),
        ));
    }

    let (objective_weights, hc0_multipliers, raw_weight_scale, effective_observations) =
        match &problem.weights {
            OracleWeights::Unit => (vec![1.0; nobs], vec![1.0; nobs], 1.0, nobs),
            OracleWeights::Wls(weights) => {
                validate_positive_weights(weights, nobs)?;
                let raw_weight_scale = weights.iter().copied().fold(0.0_f64, f64::max);
                let normalized = weights
                    .iter()
                    .map(|weight| weight / raw_weight_scale)
                    .collect::<Vec<_>>();
                if normalized
                    .iter()
                    .any(|weight| !weight.is_finite() || *weight <= 0.0)
                {
                    return Err(OracleError::NumericalFailure(
                        "relative WLS weight normalization underflowed".to_owned(),
                    ));
                }
                let squared = normalized
                    .iter()
                    .map(|weight| weight * weight)
                    .collect::<Vec<_>>();
                if squared.contains(&0.0) {
                    return Err(OracleError::NumericalFailure(
                        "relative WLS weights are too extreme for a robust-score calculation"
                            .to_owned(),
                    ));
                }
                (normalized, squared, raw_weight_scale, nobs)
            }
            OracleWeights::Frequency(frequencies) => {
                if frequencies.len() != nobs || frequencies.contains(&0) {
                    return Err(OracleError::InvalidInput(
                        "frequency weights must be positive and match the outcome length"
                            .to_owned(),
                    ));
                }
                let effective_observations =
                    frequencies.iter().try_fold(0_usize, |sum, &frequency| {
                        sum.checked_add(frequency as usize)
                    });
                let effective_observations = effective_observations.ok_or_else(|| {
                    OracleError::InvalidInput(
                        "frequency-weight observation count overflowed".to_owned(),
                    )
                })?;
                let weights = frequencies
                    .iter()
                    .map(|&frequency| f64::from(frequency))
                    .collect::<Vec<_>>();
                (weights.clone(), weights, 1.0, effective_observations)
            }
        };
    if objective_weights.iter().any(|weight| !weight.is_finite())
        || hc0_multipliers
            .iter()
            .any(|multiplier| !multiplier.is_finite())
    {
        return Err(OracleError::InvalidInput(
            "weight transformation overflowed".to_owned(),
        ));
    }
    let sqrt_weights = objective_weights
        .iter()
        .map(|weight| weight.sqrt())
        .collect();
    Ok(ValidatedProblem {
        objective_weights,
        sqrt_weights,
        hc0_multipliers,
        raw_weight_scale,
        effective_observations,
    })
}

pub(super) fn validate_positive_weights(weights: &[f64], nobs: usize) -> Result<(), OracleError> {
    if weights.len() != nobs
        || weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(OracleError::InvalidInput(
            "WLS weights must be finite, positive, and match the outcome length".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn materialize_dummy_columns(problem: &ExplicitOlsProblem) -> Vec<Vec<f64>> {
    let nobs = problem.outcome.len();
    let capacity = problem
        .effects
        .iter()
        .map(|effect| effect.n_levels * (usize::from(effect.intercept) + effect.slopes.len()))
        .sum();
    let mut columns = Vec::with_capacity(capacity);
    for effect in &problem.effects {
        if effect.intercept {
            for level in 0..effect.n_levels {
                columns.push(
                    effect
                        .codes
                        .iter()
                        .map(|&code| f64::from((code as usize == level) as u8))
                        .collect(),
                );
            }
        }
        for slope in &effect.slopes {
            for level in 0..effect.n_levels {
                columns.push(
                    (0..nobs)
                        .map(|row| {
                            if effect.codes[row] as usize == level {
                                slope[row]
                            } else {
                                0.0
                            }
                        })
                        .collect(),
                );
            }
        }
    }
    columns
}

pub(super) fn weight_columns(columns: &[Vec<f64>], sqrt_weights: &[f64]) -> Vec<Vec<f64>> {
    columns
        .iter()
        .map(|column| weight_column(column, sqrt_weights))
        .collect()
}

pub(super) fn weight_column(column: &[f64], sqrt_weights: &[f64]) -> Vec<f64> {
    column
        .iter()
        .zip(sqrt_weights)
        .map(|(value, sqrt_weight)| value * sqrt_weight)
        .collect()
}
