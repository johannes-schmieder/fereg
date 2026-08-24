//! Certified Poisson pseudo-maximum-likelihood with high-dimensional fixed effects.
//!
//! Every IRLS subproblem uses the same FE absorption and rank-revealing finite
//! solver as OLS.  Candidate steps must remain finite and weakly reduce deviance;
//! success additionally requires independently recomputed finite and FE scores.

mod certificate;
mod family;
mod irls;
mod vce;

pub use certificate::PpmlCertificate;
pub use family::{
    means_from_eta, offset_from_exposure, poisson_objective, PoissonBounds, PoissonObjective,
};

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{AbsorbConfig, FeDesign, SolveReport};
use crate::ols::{
    compute_dof, DofConfig, DofReport, FiniteSolver, RegressorStatus, VceKind, VceReport,
};
use certificate::score_certificate;
use irls::{
    accept_step, initialize, multiply, profile_irls_step, scheduled_inner_tolerance,
    validate_inputs, validate_start, weighted_outcome_sum,
};
use vce::{compute_ppml_vce, PpmlVceInput};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlInitialization {
    OffsetOnly,
    Intercept,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlStart {
    pub coefficients: Vec<f64>,
    /// Includes the offset.
    pub linear_predictor: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlConfig {
    pub absorb: AbsorbConfig,
    pub finite_solver: FiniteSolver,
    pub rank_tolerance: f64,
    pub fixed_effect_collinearity_tolerance: f64,
    pub tolerance: f64,
    pub score_tolerance: f64,
    pub inner_tolerance_initial: f64,
    pub inner_tolerance_final: f64,
    pub max_iterations: usize,
    pub max_step_halvings: usize,
    pub minimum_step: f64,
    pub objective_slack: f64,
    pub bounds: PoissonBounds,
    pub initialization: PpmlInitialization,
    pub vce: VceKind,
    pub small_sample_correction: bool,
    pub dof: DofConfig,
}

impl Default for PpmlConfig {
    fn default() -> Self {
        Self {
            absorb: AbsorbConfig::default(),
            finite_solver: FiniteSolver::Auto,
            rank_tolerance: 1e-10,
            fixed_effect_collinearity_tolerance: 1e-10,
            tolerance: 1e-8,
            score_tolerance: 1e-8,
            inner_tolerance_initial: 1e-6,
            inner_tolerance_final: 1e-10,
            max_iterations: 100,
            max_step_halvings: 25,
            minimum_step: 2.0_f64.powi(-25),
            objective_slack: 1e-12,
            bounds: PoissonBounds::default(),
            initialization: PpmlInitialization::Intercept,
            vce: VceKind::Robust,
            small_sample_correction: true,
            dof: DofConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlIterationReport {
    pub iteration: usize,
    pub deviance: f64,
    pub log_likelihood: f64,
    pub relative_deviance_change: f64,
    pub step: f64,
    pub step_halvings: usize,
    pub inner_tolerance: f64,
    pub finite_rank: usize,
    pub absorb_report: Option<SolveReport>,
    pub certificate: PpmlCertificate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlDiagnostics {
    pub converged: bool,
    pub iterations: Vec<PpmlIterationReport>,
    pub total_step_halvings: usize,
    pub final_certificate: PpmlCertificate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlFitStatistics {
    pub observations: usize,
    pub finite_rank: usize,
    pub absorbed_rank: usize,
    pub residual_dof: usize,
    pub deviance: f64,
    pub log_likelihood: f64,
    pub weighted_outcome_sum: f64,
    pub irls_iterations: usize,
    pub step_halvings: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlResult {
    pub coefficients: Vec<f64>,
    pub covariance: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub statuses: Vec<RegressorStatus>,
    pub kept: Vec<usize>,
    pub linear_predictor: Vec<f64>,
    pub mean: Vec<f64>,
    pub score_residuals: Vec<f64>,
    pub finite_predictor: Vec<f64>,
    /// Concentrated FE contribution, normalized only through the fitted predictor.
    pub absorbed_predictor: Vec<f64>,
    pub objective: PoissonObjective,
    pub dof: DofReport,
    pub fit: PpmlFitStatistics,
    pub vce: VceReport,
    pub final_absorb_report: Option<SolveReport>,
    pub diagnostics: PpmlDiagnostics,
}

pub fn fit_ppml(
    outcome: &[f64],
    regressors: &DenseMatrix,
    prior_weights: &[f64],
    offset: &[f64],
    fixed_effects: Option<&FeDesign>,
    clusters: &[Vec<u32>],
    config: PpmlConfig,
) -> Result<PpmlResult> {
    fit_ppml_with_start(
        outcome,
        regressors,
        prior_weights,
        offset,
        fixed_effects,
        clusters,
        config,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn fit_ppml_with_start(
    outcome: &[f64],
    regressors: &DenseMatrix,
    prior_weights: &[f64],
    offset: &[f64],
    fixed_effects: Option<&FeDesign>,
    clusters: &[Vec<u32>],
    config: PpmlConfig,
    start: Option<&PpmlStart>,
) -> Result<PpmlResult> {
    validate_inputs(
        outcome,
        regressors,
        prior_weights,
        offset,
        fixed_effects,
        clusters,
        config,
    )?;
    let nrows = outcome.len();
    let ncols = regressors.ncols();
    let (mut coefficients, mut eta) = if let Some(start) = start {
        validate_start(start, nrows, ncols, config)?;
        (start.coefficients.clone(), start.linear_predictor.clone())
    } else {
        initialize(
            outcome,
            regressors,
            prior_weights,
            offset,
            fixed_effects,
            config,
        )?
    };
    let mut mean = vec![0.0_f64; nrows];
    means_from_eta(&eta, config.bounds, &mut mean)?;
    let mut objective = poisson_objective(outcome, &mean, prior_weights)?;
    let mut reports = Vec::with_capacity(config.max_iterations.min(128));
    let mut total_step_halvings = 0_usize;
    let mut converged = false;
    let mut final_certificate = score_certificate(
        outcome,
        &mean,
        prior_weights,
        regressors,
        fixed_effects,
        config.score_tolerance,
    )?;

    for iteration in 1..=config.max_iterations {
        let inner_tolerance = scheduled_inner_tolerance(config, iteration);
        let profile = profile_irls_step(
            outcome,
            regressors,
            prior_weights,
            offset,
            fixed_effects,
            &eta,
            &mean,
            config,
            inner_tolerance,
        )?;
        let accepted = accept_step(
            &eta,
            &coefficients,
            &profile.candidate_eta,
            &profile.coefficients,
            outcome,
            prior_weights,
            objective,
            config,
        )?;
        total_step_halvings = total_step_halvings
            .checked_add(accepted.step_halvings)
            .ok_or_else(|| {
                FeregError::new(ErrorCode::Internal, "PPML step-halving count overflowed")
            })?;
        let relative_change = (objective.deviance - accepted.objective.deviance).abs()
            / objective.deviance.abs().max(1.0);
        coefficients = accepted.coefficients;
        eta = accepted.eta;
        mean = accepted.mean;
        objective = accepted.objective;
        final_certificate = score_certificate(
            outcome,
            &mean,
            prior_weights,
            regressors,
            fixed_effects,
            config.score_tolerance,
        )?;
        reports.push(PpmlIterationReport {
            iteration,
            deviance: objective.deviance,
            log_likelihood: objective.log_likelihood,
            relative_deviance_change: relative_change,
            step: accepted.step,
            step_halvings: accepted.step_halvings,
            inner_tolerance,
            finite_rank: profile.kept.len(),
            absorb_report: profile.absorb_report,
            certificate: final_certificate.clone(),
        });
        if relative_change <= config.tolerance && final_certificate.passed {
            converged = true;
            break;
        }
    }
    if !converged {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            format!(
                "PPML failed to converge after {} iterations; deviance={:.16e}, score={:.3e}",
                config.max_iterations, objective.deviance, final_certificate.relative_infinity_norm
            ),
        ));
    }

    // Recreate the Hessian and transformed X at the accepted final mean.  This
    // prevents covariance and rank diagnostics from referring to a stale IRLS state.
    let final_profile = profile_irls_step(
        outcome,
        regressors,
        prior_weights,
        offset,
        fixed_effects,
        &eta,
        &mean,
        config,
        config.inner_tolerance_final,
    )?;
    for (column, status) in final_profile.statuses.iter().enumerate() {
        if !matches!(status, RegressorStatus::Kept) {
            coefficients[column] = 0.0;
        }
    }
    let score_residuals = outcome
        .iter()
        .zip(&mean)
        .map(|(outcome, mean)| outcome - mean)
        .collect::<Vec<_>>();
    let dof = compute_dof(nrows, final_profile.kept.len(), fixed_effects, config.dof)?;
    let vce = compute_ppml_vce(PpmlVceInput {
        regressors: &final_profile.transformed_regressors,
        score_residuals: &score_residuals,
        prior_weights,
        bread: &final_profile.bread,
        kept: &final_profile.kept,
        residual_dof: dof.residual,
        clusters,
        kind: config.vce,
        small_sample: config.small_sample_correction,
    })?;
    let finite_predictor = multiply(regressors, &coefficients)?;
    let absorbed_predictor = eta
        .iter()
        .zip(offset)
        .zip(&finite_predictor)
        .map(|((eta, offset), xb)| eta - offset - xb)
        .collect::<Vec<_>>();
    let fit = PpmlFitStatistics {
        observations: nrows,
        finite_rank: final_profile.kept.len(),
        absorbed_rank: dof.absorbed_rank,
        residual_dof: dof.residual,
        deviance: objective.deviance,
        log_likelihood: objective.log_likelihood,
        weighted_outcome_sum: weighted_outcome_sum(outcome, prior_weights)?,
        irls_iterations: reports.len(),
        step_halvings: total_step_halvings,
    };
    Ok(PpmlResult {
        coefficients,
        covariance: vce.covariance.clone(),
        standard_errors: vce.standard_errors.clone(),
        statuses: final_profile.statuses,
        kept: final_profile.kept,
        linear_predictor: eta,
        mean,
        score_residuals,
        finite_predictor,
        absorbed_predictor,
        objective,
        dof,
        fit,
        vce,
        final_absorb_report: final_profile.absorb_report,
        diagnostics: PpmlDiagnostics {
            converged,
            iterations: reports,
            total_step_halvings,
            final_certificate,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    fn model_based() -> PpmlConfig {
        PpmlConfig {
            vce: VceKind::Unadjusted,
            small_sample_correction: false,
            ..PpmlConfig::default()
        }
    }

    #[test]
    fn intercept_only_matches_the_mean() {
        let y = [1.0, 2.0, 4.0, 8.0, 3.0, 5.0];
        let x = DenseMatrix::from_columns(&[vec![1.0; y.len()]]).expect("matrix");
        let fit = fit_ppml(&y, &x, &[1.0; 6], &[0.0; 6], None, &[], model_based()).expect("fit");
        let expected = (y.iter().sum::<f64>() / y.len() as f64).ln();
        assert!((fit.coefficients[0] - expected).abs() < 1e-8);
        assert!(fit.diagnostics.final_certificate.passed);
    }

    #[test]
    fn exposure_recovers_a_constant_rate() {
        let exposure = [1.0, 2.0, 3.0, 4.0, 5.0, 7.0];
        let y = exposure.map(|value| 2.5 * value);
        let offset = offset_from_exposure(&exposure).expect("offset");
        let x = DenseMatrix::from_columns(&[vec![1.0; y.len()]]).expect("matrix");
        let fit = fit_ppml(&y, &x, &[1.0; 6], &offset, None, &[], model_based()).expect("fit");
        assert!((fit.coefficients[0] - 2.5_f64.ln()).abs() < 1e-8);
        assert!(fit
            .mean
            .iter()
            .zip(y)
            .all(|(left, right)| (left - right).abs() < 1e-8));
    }

    #[test]
    fn two_way_fixed_effects_recover_the_slope() {
        let worker_codes = vec![0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
        let firm_codes = vec![0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0];
        let x_values = vec![
            0.0_f64, 1.0, 1.5, 0.5, 2.0, 3.0, 2.5, 4.0, 1.2, 2.2, 3.2, 4.2,
        ];
        let worker_effect = [0.2, -0.1, 0.3, -0.25];
        let firm_effect = [0.15, -0.2];
        let y = x_values
            .iter()
            .enumerate()
            .map(|(row, value)| {
                (0.7 * *value
                    + worker_effect[worker_codes[row] as usize]
                    + firm_effect[firm_codes[row] as usize])
                    .exp()
            })
            .collect::<Vec<_>>();
        let x = DenseMatrix::from_columns(&[x_values]).expect("matrix");
        let worker = EffectTerm::new(worker_codes, 4, true, vec![]).expect("worker");
        let firm = EffectTerm::new(firm_codes, 2, true, vec![]).expect("firm");
        let design = FeDesign::new(y.len(), vec![worker, firm]).expect("design");
        let fit = fit_ppml(
            &y,
            &x,
            &vec![1.0; y.len()],
            &vec![0.0; y.len()],
            Some(&design),
            &[],
            model_based(),
        )
        .expect("fit");
        assert!((fit.coefficients[0] - 0.7).abs() < 1e-6);
        assert!(fit.diagnostics.final_certificate.passed);
    }

    #[test]
    fn clustered_vce_is_finite() {
        let x_values = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
        let y = x_values
            .iter()
            .enumerate()
            .map(|(row, value)| (0.4_f64 + 0.3 * *value).exp() + (row % 2) as f64 * 0.1)
            .collect::<Vec<_>>();
        let x = DenseMatrix::from_columns(&[vec![1.0; y.len()], x_values]).expect("matrix");
        let fit = fit_ppml(
            &y,
            &x,
            &[1.0; 8],
            &[0.0; 8],
            None,
            &[vec![0, 0, 1, 1, 2, 2, 3, 3]],
            PpmlConfig {
                vce: VceKind::Clustered,
                ..PpmlConfig::default()
            },
        )
        .expect("fit");
        assert!(fit.standard_errors.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn invalid_outcomes_fail_closed() {
        let x = DenseMatrix::from_columns(&[vec![1.0; 3]]).expect("matrix");
        for y in [[0.0, -1.0, 2.0], [0.0; 3]] {
            assert!(fit_ppml(
                &y,
                &x,
                &[1.0; 3],
                &[0.0; 3],
                None,
                &[],
                PpmlConfig::default(),
            )
            .is_err());
        }
    }
}
