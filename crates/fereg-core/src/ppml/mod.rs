//! Certified Poisson pseudo-maximum-likelihood with high-dimensional fixed effects.
//!
//! Every IRLS subproblem uses the same FE absorption and rank-revealing finite
//! solver as OLS.  Candidate steps must remain finite and weakly reduce deviance;
//! success additionally requires independently recomputed finite and FE scores.

mod bias;
pub use bias::{PpmlBiasMethod, PpmlBiasRequest, PpmlBiasResult};
mod categorical_fwl_cmg;
mod categorical_lsmr;
mod certificate;
mod family;
#[cfg(test)]
mod irls;
mod memory;
mod metadata_memory;
mod one_fe_profile;
mod profiled_intercept_curve;
mod prepared;
mod prepared_estimator;
mod prepared_irls;
mod prepared_model;
mod prepared_separation;
mod psd;
mod residuals;
mod result;
mod solver;
mod vce;

pub use certificate::PpmlCertificate;
#[cfg(test)]
pub(crate) use family::means_from_eta;
pub(crate) use family::PoissonBounds;
pub use family::{
    offset_from_exposure, poisson_null_objective, poisson_objective, PoissonNullObjective,
    PoissonObjective,
};
pub use memory::{
    certify_ppml_steady_state_allocation, estimate_ppml_memory, plan_ppml_memory,
    BoundedPpmlDiagnostics, PpmlAllocationTelemetry, PpmlAllocatorSnapshot,
    PpmlAllocatorSnapshotProbe, PpmlFallbackKind, PpmlIterationWorkspace, PpmlLineSearchAcceptance,
    PpmlLinearWorkspaceCapacityReport, PpmlMemoryConfig, PpmlMemoryDimensions, PpmlMemoryEstimate,
    PpmlMemoryInventory, PpmlMemoryPlan, PpmlMemoryPreflightEvent, PpmlMemoryPreflightObservation,
    PpmlMemoryPreflightProbe, PpmlMemoryRoute, PpmlMemoryTelemetry, PpmlPhase,
    PpmlPhaseAllocatorEvent, PpmlPhaseAllocatorProbe, PpmlPhaseClock, PpmlPhaseTimings,
    PpmlSteadyStateAllocationCertificate, PpmlSteadyStateAllocationObservation,
    PpmlWorkspaceCapacityReport, PPML_PHASES, PPML_PHASE_COUNT, PPML_STEADY_STATE_AUDIT_ITERATIONS,
};
pub use metadata_memory::PpmlMetadataMemory;
pub use prepared::{
    prepare_ppml_numeric_data, PpmlNumericPreparationInput, PreparedPpmlAdjustmentKind,
    PreparedPpmlAdjustmentReport, PreparedPpmlNumericData, PreparedPpmlWeightReport,
};
pub use prepared_estimator::{
    fit_prepared_ppml, fit_prepared_ppml_interruptible, LoadedPpmlInput, PpmlInterruptPoint,
    PreparedPpmlOutputRequest, PreparedPpmlRequestV1,
};
pub use psd::{PpmlPsdReport, PpmlPsdState};
pub use residuals::{fill_ppml_residuals, ppml_residuals, PpmlResidualKind};
pub use result::{
    certify_ppml_reconstruction, CertifiedPpmlResult, PpmlClusterDimensionResult,
    PpmlCoefficientOutput, PpmlCoefficientStatus, PpmlConvergenceReport, PpmlFinalSample,
    PpmlFixedEffectChannelKind, PpmlFixedEffectResult, PpmlLikelihoodStatistics, PpmlOutputState,
    PpmlPredictionKind, PpmlReconstructionCertificate, PpmlResultCertificate,
    PpmlSavedEffectOutput,
};
#[allow(unused_imports)]
pub(crate) use result::{
    prepare_ppml_fixed_effect_report, recover_ppml_fixed_effects, CertifiedPpmlResultInput,
    PreparedPpmlFixedEffectReport, PreparedPpmlRecoveredFixedEffects,
};
pub use solver::{
    PpmlSolverEngine, PpmlSolverEngineReport, PpmlSolverFallbackReason, PpmlSolverFamily,
    PpmlSolverReport,
};
pub(crate) use solver::{PpmlSolverFrameKind, PpmlSolverObservation, PpmlSolverPlan};
pub use vce::{
    compute_ppml_dof, compute_ppml_inference, PpmlClusterSubsetReport, PpmlDofReport,
    PpmlInferenceCertificate, PpmlInferenceDesign, PpmlInferenceInput, PpmlInferenceResult,
    PpmlTestDistribution, PpmlVceKind, PpmlVceRequest, PpmlVceResult, PpmlWaldReport,
};

#[cfg(test)]
use crate::data::DenseMatrix;
#[cfg(test)]
use crate::error::{ErrorCode, FeregError, Result};
#[cfg(test)]
use crate::fe::FeDesign;
use crate::fe::{AbsorbConfig, SolveReport};
#[cfg(test)]
use crate::ols::{compute_dof, DofConfig, DofReport, RegressorStatus, VceKind, VceReport};
#[cfg(test)]
use certificate::score_certificate;
#[cfg(test)]
use family::poisson_objective_from_eta;
#[cfg(test)]
use irls::{
    accept_step, initialize, multiply, profile_irls_step, validate_inputs, validate_start,
    weighted_outcome_sum, ProfileStep,
};
#[cfg(test)]
use vce::{compute_ppml_vce, PpmlVceInput};

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PpmlStart {
    pub coefficients: Vec<f64>,
    /// Includes the offset.
    pub linear_predictor: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlConfig {
    pub(crate) absorb: AbsorbConfig,
    pub(crate) fixed_effect_collinearity_tolerance: f64,
    pub(crate) tolerance: f64,
    pub(crate) score_tolerance: f64,
    pub(crate) inner_tolerance_initial: f64,
    pub(crate) inner_tolerance_final: f64,
    pub(crate) max_iterations: usize,
    pub(crate) max_step_halvings: usize,
    pub(crate) minimum_step: f64,
    pub(crate) objective_slack: f64,
    pub(crate) bounds: PoissonBounds,
}

impl PpmlConfig {
    /// Construct the frozen PPML numerical policy with the two sanctioned
    /// command-level controls. All representability, initialization, line
    /// search, score, and inner-solve policies remain crate-owned.
    pub fn with_outer_controls(tolerance: f64, max_iterations: usize) -> crate::Result<Self> {
        if !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
            return Err(crate::FeregError::new(
                crate::ErrorCode::InvalidArgument,
                "PPML tolerance must be finite and in (0, 1)",
            ));
        }
        if 0.1 * tolerance == 0.0 {
            return Err(crate::FeregError::new(
                crate::ErrorCode::InvalidArgument,
                "PPML tolerance is too small to admit a positive strict inner tolerance",
            ));
        }
        if max_iterations == 0 {
            return Err(crate::FeregError::new(
                crate::ErrorCode::InvalidArgument,
                "PPML maximum iterations must be positive",
            ));
        }
        Ok(Self {
            tolerance,
            score_tolerance: tolerance,
            max_iterations,
            ..Self::default()
        })
    }

    /// Read-only shared absorption policy used to configure the certified
    /// weighted-linear session. This does not expose a mutation surface.
    pub fn shared_linear_policy(self) -> (AbsorbConfig, f64) {
        (self.absorb, self.fixed_effect_collinearity_tolerance)
    }
}

impl Default for PpmlConfig {
    fn default() -> Self {
        Self {
            absorb: AbsorbConfig::default(),
            fixed_effect_collinearity_tolerance: 1e-10,
            tolerance: 1e-8,
            score_tolerance: 1e-8,
            inner_tolerance_initial: 1e-6,
            inner_tolerance_final: 1e-12,
            max_iterations: 10_000,
            max_step_halvings: 25,
            minimum_step: 2.0_f64.powi(-25),
            objective_slack: 1e-12,
            bounds: PoissonBounds::default(),
        }
    }
}

/// Accepted outer-state diagnostics that govern the next inexact weighted-
/// linear solve. Both fields are independently recomputed after an accepted
/// step rather than inferred from the inner solver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PpmlInnerToleranceProgress {
    relative_deviance_change: f64,
    score_relative_infinity_norm: f64,
}

impl PpmlInnerToleranceProgress {
    pub(crate) fn new(relative_deviance_change: f64, score_relative_infinity_norm: f64) -> Self {
        Self {
            relative_deviance_change,
            score_relative_infinity_norm,
        }
    }

    fn scale(self) -> Option<f64> {
        (self.relative_deviance_change.is_finite()
            && self.relative_deviance_change >= 0.0
            && self.score_relative_infinity_norm.is_finite()
            && self.score_relative_infinity_norm >= 0.0)
            .then_some(
                self.relative_deviance_change
                    .max(self.score_relative_infinity_norm),
            )
    }
}

/// ADR 0016's strict weighted-linear publication tolerance.
pub(crate) fn strict_inner_tolerance(config: PpmlConfig) -> f64 {
    (0.1 * config.tolerance).min(1e-12)
}

/// Deterministic, monotone tolerance policy shared by the prepared production
/// session and the cfg(test)-only dense oracle. The staged target preserves the
/// bounded early-iteration policy, while accepted deviance and score progress
/// can tighten the next solve sooner. Once both outer diagnostics meet their
/// convergence bounds, the next ordinary solve uses the publication target;
/// a loose direction would otherwise spend a final checkpoint discovering its
/// avoidable linear error. Both outer passes and the strict checkpoint remain
/// mandatory. It is deliberately route-independent so
/// fast and low memory modes follow the same scientific path. Invalid progress
/// fails closed to the strict floor instead of silently permitting a loose
/// solve.
pub(crate) fn scheduled_inner_tolerance(
    config: PpmlConfig,
    iteration: usize,
    previous_tolerance: f64,
    progress: Option<PpmlInnerToleranceProgress>,
) -> f64 {
    const STAGES: usize = 8;
    let floor = config
        .inner_tolerance_final
        .min(strict_inner_tolerance(config));
    let staged_target = if iteration >= STAGES {
        floor
    } else {
        let fraction = iteration.saturating_sub(1) as f64 / (STAGES - 1) as f64;
        ((1.0 - fraction) * config.inner_tolerance_initial.ln() + fraction * floor.ln()).exp()
    };
    let previous_target = if previous_tolerance.is_finite() && previous_tolerance > 0.0 {
        previous_tolerance.min(config.inner_tolerance_initial)
    } else {
        floor
    };
    let progress_target = match progress {
        Some(progress) => progress
            .scale()
            .map(|scale| {
                if progress.relative_deviance_change <= config.tolerance
                    && progress.score_relative_infinity_norm <= config.score_tolerance
                {
                    floor
                } else {
                    (0.1 * scale).clamp(floor, config.inner_tolerance_initial)
                }
            })
            .unwrap_or(floor),
        None => config.inner_tolerance_initial,
    };
    // The nominal scheduling floor bounds newly chosen targets, not an
    // accuracy already required by a rejected strict checkpoint. Loosening
    // that accepted requirement recreates the profile the checkpoint rejected.
    staged_target
        .min(progress_target)
        .max(floor)
        .min(previous_target)
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
#[cfg(test)]
pub struct PpmlDiagnostics {
    pub converged: bool,
    pub iterations: Vec<PpmlIterationReport>,
    pub total_step_halvings: usize,
    pub final_certificate: PpmlCertificate,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg(test)]
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
#[cfg(test)]
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

#[cfg(test)]
pub(crate) fn fit_ppml(
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
#[cfg(test)]
pub(crate) fn fit_ppml_with_start(
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
    let (mut coefficients, mut eta) = if let Some(start) = start {
        validate_start(
            start,
            regressors,
            prior_weights,
            offset,
            fixed_effects,
            config,
        )?;
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
    let mut objective = poisson_objective_from_eta(outcome, &eta, &mean, prior_weights)?;
    let mut reports = Vec::with_capacity(config.max_iterations.min(128));
    let mut total_step_halvings = 0_usize;
    let mut converged = false;
    let mut certified_final_profile = None;
    let mut strict_profile_rejections = 0_usize;
    let mut consecutive_converged_iterations = 0_usize;
    let mut final_certificate = score_certificate(
        outcome,
        &mean,
        prior_weights,
        regressors,
        fixed_effects,
        config.score_tolerance,
    )?;
    let mut previous_inner_tolerance = config.inner_tolerance_initial;
    let mut previous_outer_progress = None;

    for iteration in 1..=config.max_iterations {
        let inner_tolerance = scheduled_inner_tolerance(
            config,
            iteration,
            previous_inner_tolerance,
            previous_outer_progress,
        );
        previous_inner_tolerance = inner_tolerance;
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
            / objective.deviance.min(accepted.objective.deviance).max(0.1);
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
        let iteration_certified = relative_change <= config.tolerance && final_certificate.passed;
        consecutive_converged_iterations = if iteration_certified {
            consecutive_converged_iterations.saturating_add(1)
        } else {
            0
        };
        let wants_convergence = consecutive_converged_iterations >= 2;
        let strict_profile = if wants_convergence {
            let strict_tolerance = strict_inner_tolerance(config);
            previous_inner_tolerance = previous_inner_tolerance.min(strict_tolerance);
            let candidate = profile_irls_step(
                outcome,
                regressors,
                prior_weights,
                offset,
                fixed_effects,
                &eta,
                &mean,
                config,
                strict_tolerance,
            )?;
            if final_profile_is_consistent(&profile, &candidate, &eta, &coefficients, config) {
                Some(candidate)
            } else {
                strict_profile_rejections =
                    strict_profile_rejections.checked_add(1).ok_or_else(|| {
                        FeregError::new(
                            ErrorCode::Internal,
                            "PPML strict-profile rejection count overflowed",
                        )
                    })?;
                consecutive_converged_iterations = 0;
                None
            }
        } else {
            None
        };
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
        previous_outer_progress = Some(PpmlInnerToleranceProgress::new(
            relative_change,
            final_certificate.relative_infinity_norm,
        ));
        if let Some(profile) = strict_profile {
            certified_final_profile = Some(profile);
            converged = true;
            break;
        }
    }
    if !converged {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            format!(
                "PPML failed to converge after {} iterations; deviance={:.16e}, score={:.3e}, strict_profile_rejections={strict_profile_rejections}",
                config.max_iterations, objective.deviance, final_certificate.relative_infinity_norm,
            ),
        ));
    }

    // The strict profile was computed at this exact accepted mean and was also
    // required to agree with the accepted coefficients, predictor, and rank.
    // Thus covariance and statuses cannot silently refer to a different WLS
    // solution than the published point estimate.
    let final_profile = certified_final_profile.ok_or_else(|| {
        FeregError::new(
            ErrorCode::Internal,
            "PPML convergence lacked a certified final profile",
        )
    })?;
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
    // This direct fitter is retained only as a legacy test/oracle bridge.  It
    // must not expose OLS inference policy through `PpmlConfig`: production
    // PPML owns VCE and DoF in `PreparedPpmlRequestV1`/`compute_ppml_inference`.
    let dof = compute_dof(
        nrows,
        final_profile.kept.len(),
        fixed_effects,
        DofConfig::default(),
    )?;
    let legacy_vce = if clusters.is_empty() {
        VceKind::Robust
    } else {
        VceKind::Clustered
    };
    let vce = compute_ppml_vce(PpmlVceInput {
        regressors: &final_profile.transformed_regressors,
        score_residuals: &score_residuals,
        prior_weights,
        bread: &final_profile.bread,
        kept: &final_profile.kept,
        residual_dof: dof.residual,
        clusters,
        kind: legacy_vce,
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
fn final_profile_is_consistent(
    iteration_profile: &ProfileStep,
    strict_profile: &ProfileStep,
    accepted_eta: &[f64],
    accepted_coefficients: &[f64],
    config: PpmlConfig,
) -> bool {
    if iteration_profile.statuses != strict_profile.statuses
        || iteration_profile.kept != strict_profile.kept
    {
        return false;
    }
    let tolerance = config
        .tolerance
        .min(config.score_tolerance)
        .min(1e-8)
        .max(64.0 * config.inner_tolerance_final)
        .max(256.0 * f64::EPSILON);
    relative_infinity_difference(accepted_eta, &strict_profile.candidate_eta) <= tolerance
        && relative_infinity_difference(accepted_coefficients, &strict_profile.coefficients)
            <= tolerance
}

#[cfg(test)]
fn relative_infinity_difference(left: &[f64], right: &[f64]) -> f64 {
    if left.len() != right.len() {
        return f64::INFINITY;
    }
    left.iter()
        .zip(right)
        .map(|(left, right)| (left - right).abs() / left.abs().max(right.abs()).max(1.0))
        .fold(0.0_f64, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    fn legacy_config() -> PpmlConfig {
        PpmlConfig::default()
    }

    #[test]
    fn public_outer_controls_preserve_frozen_internal_policy() {
        let defaults = PpmlConfig::default();
        let configured = PpmlConfig::with_outer_controls(1e-10, 37).expect("outer controls");
        assert_eq!(configured.tolerance, 1e-10);
        assert_eq!(configured.score_tolerance, 1e-10);
        assert_eq!(configured.max_iterations, 37);
        assert_eq!(configured.absorb, defaults.absorb);
        assert_eq!(
            configured.fixed_effect_collinearity_tolerance,
            defaults.fixed_effect_collinearity_tolerance
        );
        assert_eq!(
            configured.inner_tolerance_initial,
            defaults.inner_tolerance_initial
        );
        assert_eq!(
            configured.inner_tolerance_final,
            defaults.inner_tolerance_final
        );
        assert_eq!(configured.max_step_halvings, defaults.max_step_halvings);
        assert_eq!(configured.minimum_step, defaults.minimum_step);
        assert_eq!(configured.objective_slack, defaults.objective_slack);
        assert_eq!(configured.bounds, defaults.bounds);
        assert!(PpmlConfig::with_outer_controls(0.0, 1).is_err());
        assert!(PpmlConfig::with_outer_controls(f64::NAN, 1).is_err());
        assert!(PpmlConfig::with_outer_controls(1.0, 1).is_err());
        assert!(PpmlConfig::with_outer_controls(f64::from_bits(1), 1).is_err());
        assert!(PpmlConfig::with_outer_controls(f64::MIN_POSITIVE, 1).is_ok());
        let below_one = f64::from_bits(1.0_f64.to_bits() - 1);
        assert!(PpmlConfig::with_outer_controls(below_one, 1).is_ok());
        assert!(PpmlConfig::with_outer_controls(1e-8, 0).is_err());
    }

    #[test]
    fn inner_tolerance_responds_to_outer_progress_at_a_fixed_iteration() {
        let config = PpmlConfig::default();
        let deviance_limited = scheduled_inner_tolerance(
            config,
            2,
            config.inner_tolerance_initial,
            Some(PpmlInnerToleranceProgress::new(1e-5, 1e-12)),
        );
        let score_limited = scheduled_inner_tolerance(
            config,
            2,
            config.inner_tolerance_initial,
            Some(PpmlInnerToleranceProgress::new(1e-12, 1e-5)),
        );
        let both_improved = scheduled_inner_tolerance(
            config,
            2,
            config.inner_tolerance_initial,
            Some(PpmlInnerToleranceProgress::new(1e-10, 1e-10)),
        );
        assert!(both_improved < deviance_limited);
        assert!(both_improved < score_limited);
        assert!(both_improved >= strict_inner_tolerance(config));
    }

    #[test]
    fn inner_tolerance_prepares_a_converged_outer_point_for_publication() {
        for (outer, score) in [(1e-8, 1e-8), (1e-8, 1e-10), (1e-14, 1e-12)] {
            let mut config = PpmlConfig::default();
            config.tolerance = outer;
            config.score_tolerance = score;
            let floor = config.inner_tolerance_final.min(strict_inner_tolerance(config));
            let next = |deviance, score, previous| scheduled_inner_tolerance(
                config, 2, previous, Some(PpmlInnerToleranceProgress::new(deviance, score)),
            );
            assert_eq!(next(outer, score, config.inner_tolerance_initial), floor);
            assert_eq!(next(0.0, score, config.inner_tolerance_initial), floor);
            assert_eq!(next(outer, 0.0, config.inner_tolerance_initial), floor);
            assert_eq!(next(outer, score, floor * 0.01), floor * 0.01);
            // Both independent diagnostics must pass. A small score alone
            // cannot authorize the publication target on a moving objective.
            assert!(next(1e-3, 0.0, config.inner_tolerance_initial) > floor);
            assert!(next(0.0, 1e-3, config.inner_tolerance_initial) > floor);
            assert_eq!(next(f64::NAN, score, config.inner_tolerance_initial), floor);
            assert_eq!(next(outer, -1.0, config.inner_tolerance_initial), floor);
        }
    }

    #[test]
    fn inner_tolerance_is_monotone_and_reaches_the_outer_certificate_floor() {
        let mut config = PpmlConfig::default();
        config.tolerance = 1e-14;
        config.score_tolerance = config.tolerance;
        let floor = strict_inner_tolerance(config);
        let first = scheduled_inner_tolerance(config, 1, config.inner_tolerance_initial, None);
        let improved = scheduled_inner_tolerance(
            config,
            2,
            first,
            Some(PpmlInnerToleranceProgress::new(1e-9, 1e-9)),
        );
        let worsened = scheduled_inner_tolerance(
            config,
            3,
            improved,
            Some(PpmlInnerToleranceProgress::new(1e-2, 1e-2)),
        );
        let final_stage = scheduled_inner_tolerance(
            config,
            8,
            worsened,
            Some(PpmlInnerToleranceProgress::new(1e-2, 1e-2)),
        );
        assert!(improved <= first);
        assert!(worsened <= improved);
        assert_eq!(final_stage, floor);
        assert_eq!(floor, 1e-15);
        let resumed_after_strict = scheduled_inner_tolerance(
            config,
            3,
            strict_inner_tolerance(config),
            Some(PpmlInnerToleranceProgress::new(1e-2, 1e-2)),
        );
        assert_eq!(resumed_after_strict, floor);
    }

    #[test]
    fn checkpoint_refinement_survives_ordinary_scheduling() {
        let config = PpmlConfig::default();
        for previous in [1e-14, 1e-15] {
            for iteration in [1, 3, 8, 20, 100] {
                for progress in [
                    None,
                    Some(PpmlInnerToleranceProgress::new(1e-2, 1e-2)),
                    Some(PpmlInnerToleranceProgress::new(f64::NAN, 1e-2)),
                ] {
                    let next = scheduled_inner_tolerance(config, iteration, previous, progress);
                    assert!(
                        next <= previous,
                        "checkpoint {previous:e} loosened to {next:e}"
                    );
                }
            }
        }
    }

    #[test]
    fn nonfinite_inner_tolerance_progress_fails_closed() {
        let config = PpmlConfig::default();
        let tolerance = scheduled_inner_tolerance(
            config,
            2,
            config.inner_tolerance_initial,
            Some(PpmlInnerToleranceProgress::new(f64::NAN, 1e-4)),
        );
        assert_eq!(tolerance, strict_inner_tolerance(config));
    }

    #[test]
    fn intercept_only_matches_the_mean() {
        let y = [1.0, 2.0, 4.0, 8.0, 3.0, 5.0];
        let x = DenseMatrix::from_columns(&[vec![1.0; y.len()]]).expect("matrix");
        let fit = fit_ppml(&y, &x, &[1.0; 6], &[0.0; 6], None, &[], legacy_config()).expect("fit");
        let expected = (y.iter().sum::<f64>() / y.len() as f64).ln();
        assert!((fit.coefficients[0] - expected).abs() < 1e-8);
        assert!(fit.diagnostics.final_certificate.passed);
    }

    #[test]
    fn fractional_outcomes_are_valid_estimation_data() {
        let y = [0.25_f64, 0.5, 1.75, 3.25, 2.5, 4.75];
        let x = DenseMatrix::from_columns(&[vec![1.0; y.len()]]).expect("matrix");
        let fit = fit_ppml(&y, &x, &[1.0; 6], &[0.0; 6], None, &[], legacy_config()).expect("fit");
        let expected = (y.iter().sum::<f64>() / y.len() as f64).ln();
        assert!((fit.coefficients[0] - expected).abs() < 1e-9);
        assert!(fit.objective.deviance.is_finite());
    }

    #[test]
    fn exposure_recovers_a_constant_rate() {
        let exposure = [1.0, 2.0, 3.0, 4.0, 5.0, 7.0];
        let y = exposure.map(|value| 2.5 * value);
        let offset = offset_from_exposure(&exposure).expect("offset");
        let x = DenseMatrix::from_columns(&[vec![1.0; y.len()]]).expect("matrix");
        let fit = fit_ppml(&y, &x, &[1.0; 6], &offset, None, &[], legacy_config()).expect("fit");
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
            legacy_config(),
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
            legacy_config(),
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

    #[test]
    fn all_zero_outcome_reports_the_nonexistent_finite_intercept() {
        let x = DenseMatrix::from_columns(&[vec![1.0; 4]]).expect("matrix");
        let error = fit_ppml(
            &[0.0; 4],
            &x,
            &[1.0; 4],
            &[0.0; 4],
            None,
            &[],
            legacy_config(),
        )
        .expect_err("all-zero outcome must fail");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert!(error.message.contains("no finite PPML intercept solution"));
    }

    #[test]
    fn stale_final_profile_reproduction_is_rejected_instead_of_published() {
        let x_values = [-2.0_f64, -1.0, 0.0, 1.0, 2.0, 3.0];
        let outcome = x_values.map(|value| (0.2 + 0.6 * value).exp());
        let regressors = DenseMatrix::from_columns(&[vec![1.0; outcome.len()], x_values.to_vec()])
            .expect("regressors");
        let weights = [1.0_f64; 6];
        let offset = [0.0_f64; 6];
        let mut config = legacy_config();
        config.tolerance = 0.99;
        config.score_tolerance = 0.99;
        config.max_iterations = 1;

        let (coefficients, eta) =
            initialize(&outcome, &regressors, &weights, &offset, None, config)
                .expect("initialization");
        let mut mean = [0.0_f64; 6];
        means_from_eta(&eta, config.bounds, &mut mean).expect("initial mean");
        let objective = poisson_objective(&outcome, &mean, &weights).expect("objective");
        let first_profile = profile_irls_step(
            &outcome,
            &regressors,
            &weights,
            &offset,
            None,
            &eta,
            &mean,
            config,
            scheduled_inner_tolerance(config, 1, config.inner_tolerance_initial, None),
        )
        .expect("first profile");
        let accepted = accept_step(
            &eta,
            &coefficients,
            &first_profile.candidate_eta,
            &first_profile.coefficients,
            &outcome,
            &weights,
            objective,
            config,
        )
        .expect("accepted first step");
        let strict_profile = profile_irls_step(
            &outcome,
            &regressors,
            &weights,
            &offset,
            None,
            &accepted.eta,
            &accepted.mean,
            config,
            config.inner_tolerance_final,
        )
        .expect("strict profile");
        assert!(
            !final_profile_is_consistent(
                &first_profile,
                &strict_profile,
                &accepted.eta,
                &accepted.coefficients,
                config,
            ),
            "the second WLS profile must expose the stale-final-state mismatch"
        );

        let error = fit_ppml(&outcome, &regressors, &weights, &offset, None, &[], config)
            .expect_err("one loose iteration must not publish the stale final profile");
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert!(error.message.contains("strict_profile_rejections="));
    }

    #[test]
    fn public_warm_start_rejects_inconsistent_eta_and_beta() {
        let outcome = [1.0_f64, 2.0, 3.0, 4.0];
        let regressors = DenseMatrix::from_columns(&[vec![1.0; 4]]).expect("regressors");
        let start = PpmlStart {
            coefficients: vec![0.0],
            linear_predictor: vec![1.0; 4],
        };
        let error = fit_ppml_with_start(
            &outcome,
            &regressors,
            &[1.0; 4],
            &[0.0; 4],
            None,
            &[],
            legacy_config(),
            Some(&start),
        )
        .expect_err("inconsistent warm start");
        assert!(error.message.contains("warm start is inconsistent"));
    }
}
