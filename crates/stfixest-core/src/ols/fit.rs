/// Weighted OLS goodness-of-fit statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct FitStatistics {
    pub observations: usize,
    pub weighted_rss: f64,
    pub weighted_tss: f64,
    pub weighted_within_tss: f64,
    pub r_squared: f64,
    pub adjusted_r_squared: f64,
    pub within_r_squared: f64,
    pub root_mean_squared_error: f64,
    pub gaussian_log_likelihood: f64,
}

pub(crate) fn compute_fit_statistics(
    outcome: &[f64],
    within_outcome: &[f64],
    residuals: &[f64],
    weights: &[f64],
    residual_dof: usize,
) -> FitStatistics {
    let weighted_rss = weighted_sum_of_squares(residuals, weights, 0.0);
    let outcome_mean = weighted_mean(outcome, weights);
    let within_mean = weighted_mean(within_outcome, weights);
    let weighted_tss = weighted_sum_of_squares(outcome, weights, outcome_mean);
    let weighted_within_tss = weighted_sum_of_squares(within_outcome, weights, within_mean);
    let r_squared = ratio_r_squared(weighted_rss, weighted_tss);
    let within_r_squared = ratio_r_squared(weighted_rss, weighted_within_tss);
    let observations = outcome.len();
    let adjusted_r_squared = if observations > 1 && weighted_tss > 0.0 {
        1.0 - (weighted_rss / residual_dof as f64)
            / (weighted_tss / observations.saturating_sub(1) as f64)
    } else {
        f64::NAN
    };
    let root_mean_squared_error = (weighted_rss / residual_dof as f64).sqrt();
    let gaussian_log_likelihood = if weighted_rss > 0.0 && observations > 0 {
        -0.5 * observations as f64
            * ((2.0 * std::f64::consts::PI).ln()
                + 1.0
                + (weighted_rss / observations as f64).ln())
    } else if weighted_rss == 0.0 {
        f64::INFINITY
    } else {
        f64::NAN
    };
    FitStatistics {
        observations,
        weighted_rss,
        weighted_tss,
        weighted_within_tss,
        r_squared,
        adjusted_r_squared,
        within_r_squared,
        root_mean_squared_error,
        gaussian_log_likelihood,
    }
}

fn weighted_mean(values: &[f64], weights: &[f64]) -> f64 {
    let (weighted_sum, weight_sum) = values.iter().zip(weights).fold(
        (0.0_f64, 0.0_f64),
        |(value_sum, weight_sum), (value, weight)| {
            ((*weight).mul_add(*value, value_sum), weight_sum + *weight)
        },
    );
    weighted_sum / weight_sum
}

fn weighted_sum_of_squares(values: &[f64], weights: &[f64], center: f64) -> f64 {
    values
        .iter()
        .zip(weights)
        .fold(0.0_f64, |sum, (value, weight)| {
            let centered = *value - center;
            (*weight * centered).mul_add(centered, sum)
        })
}

fn ratio_r_squared(rss: f64, tss: f64) -> f64 {
    if tss > 0.0 {
        1.0 - rss / tss
    } else if rss == 0.0 {
        1.0
    } else {
        f64::NAN
    }
}
