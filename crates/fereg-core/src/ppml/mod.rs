//! Certified Poisson pseudo-maximum-likelihood with high-dimensional fixed effects.
//!
//! Every IRLS subproblem uses the same FE absorption and rank-revealing finite
//! solver as OLS.  Candidate steps must remain finite and weakly reduce deviance;
//! success additionally requires independently recomputed finite and FE scores.

mod bias;
pub use bias::{PpmlBiasMethod, PpmlBiasRequest, PpmlBiasResult, PpmlBiasSupport};
mod categorical_fwl_cmg;
mod categorical_lsmr;
pub(crate) use categorical_lsmr::estimate_checkpoint_lsmr_workspace_bytes;
mod certificate;
mod family;
mod memory;
mod metadata_memory;
mod one_fe_profile;
mod prepared;
mod prepared_estimator;
mod prepared_irls;
mod prepared_model;
mod prepared_separation;
mod profiled_intercept_curve;
mod psd;
mod residuals;
mod result;
mod solver;
mod topology_identity;
mod vce;

pub use certificate::PpmlCertificate;
pub(crate) use family::PoissonBounds;
pub use family::{
    offset_from_exposure, poisson_null_objective, poisson_objective, PoissonNullObjective,
    PoissonObjective,
};
pub use memory::{
    certify_ppml_steady_state_allocation, estimate_ppml_memory, plan_ppml_memory,
    PpmlAllocationTelemetry, PpmlAllocatorSnapshot, PpmlAllocatorSnapshotProbe, PpmlFallbackKind,
    PpmlIterationWorkspace, PpmlLineSearchAcceptance, PpmlLinearWorkspaceCapacityReport,
    PpmlMemoryConfig, PpmlMemoryDimensions, PpmlMemoryEstimate, PpmlMemoryInventory,
    PpmlMemoryPlan, PpmlMemoryPreflightEvent, PpmlMemoryPreflightObservation,
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
pub(crate) use result::CertifiedPpmlResultInput;
pub use result::{
    certify_ppml_reconstruction, CertifiedPpmlResult, PpmlClusterDimensionResult,
    PpmlCoefficientOutput, PpmlCoefficientStatus, PpmlConvergenceReport, PpmlFinalSample,
    PpmlFixedEffectChannelKind, PpmlFixedEffectResult, PpmlLikelihoodStatistics, PpmlOutputState,
    PpmlPredictionKind, PpmlReconstructionCertificate, PpmlResultCertificate,
    PpmlSavedEffectOutput,
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

use crate::fe::AbsorbConfig;

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

/// The strict weighted-linear publication tolerance (SPEC §13).
pub(crate) fn strict_inner_tolerance(config: PpmlConfig) -> f64 {
    (0.1 * config.tolerance).min(1e-12)
}

/// Deterministic, monotone tolerance policy shared by the prepared production
/// session. The staged target preserves the
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

mod wald;
pub use wald::PpmlWaldReference;

mod constant_wald;

#[cfg(test)]
mod tests {
    use super::*;

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
            let config = PpmlConfig {
                tolerance: outer,
                score_tolerance: score,
                ..PpmlConfig::default()
            };
            let floor = config
                .inner_tolerance_final
                .min(strict_inner_tolerance(config));
            let next = |deviance, score, previous| {
                scheduled_inner_tolerance(
                    config,
                    2,
                    previous,
                    Some(PpmlInnerToleranceProgress::new(deviance, score)),
                )
            };
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
}
