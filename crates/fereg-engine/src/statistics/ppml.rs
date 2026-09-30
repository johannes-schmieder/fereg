//! PPML likelihood/statistics retain the accepted state and offset-free reference null.
use crate::{
    data::ReferenceWeights,
    error::{ErrorCode, FeregError, Result},
    ppml::{family::Family, irls},
    spec::Estimator,
    threads::compensated_sum,
};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlStatistics {
    pub deviance: f64,
    pub log_likelihood: f64,
    pub null_mean: f64,
    pub null_log_likelihood: f64,
    pub pseudo_r_squared: f64,
    pub weighted_outcome_sum: f64,
    pub outer_iterations: usize,
    pub inner_iterations: usize,
    pub step_halvings: usize,
    pub converged: bool,
}
pub fn ppml(fit: &irls::Fit<'_>) -> Result<PpmlStatistics> {
    let sample = fit.problem().sample;
    let q = ReferenceWeights::new(&sample.weights, sample.n_active(), Estimator::Ppml)?;
    let family = Family::new(&sample.outcome, q, None)?;
    let null = family.null_objective()?;
    // Retain the frozen public guard: an undefined ratio cannot be published.
    if null.log_likelihood == 0. {
        return Err(numerical(
            "PPML null likelihood is zero, so pseudo R-squared is undefined",
        ));
    }
    let pseudo_r_squared = 1. - fit.evaluation.objective.log_likelihood / null.log_likelihood;
    let weighted_outcome_sum = compensated_sum(
        sample
            .outcome
            .iter()
            .enumerate()
            .map(|(i, y)| q.value(i) * y),
    );
    if !pseudo_r_squared.is_finite()
        || !weighted_outcome_sum.is_finite()
        || weighted_outcome_sum < 0.
    {
        return Err(numerical("PPML fit statistic is not representable"));
    }
    Ok(PpmlStatistics {
        deviance: fit.evaluation.objective.deviance,
        log_likelihood: fit.evaluation.objective.log_likelihood,
        null_mean: null.mean,
        null_log_likelihood: null.log_likelihood,
        pseudo_r_squared,
        weighted_outcome_sum,
        outer_iterations: fit.diagnostics.accepted_iterations,
        inner_iterations: fit.diagnostics.work.counts().summed_iterations,
        step_halvings: fit.diagnostics.step_halvings,
        converged: true,
    })
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
