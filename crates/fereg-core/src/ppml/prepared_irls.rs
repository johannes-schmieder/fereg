//! PPML IRLS over the reusable certified prepared weighted-linear session.
//!
//! PPML owns initialization, likelihood, convergence, rank-freezing, and
//! publication policy. The shared session owns only a certified weighted
//! linear subproblem on an immutable prepared model.

#[cfg(test)]
use crate::fe::PARALLEL_PROJECTION_MIN_ROWS;
#[cfg(test)]
use std::cell::Cell;
use std::time::{Duration, Instant};

use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, invalid_data, ErrorCode, FeregError, Result};
use crate::fe::{
    absorb_fixed_effects, BorrowedMapWeights, CmgConfig, CmgTopologyProfile, FeDesign,
    FeSolverKind, PreparedCmgTopology, SolveReport, CMG_MAX_NUISANCE_COLUMNS,
};
use crate::memory::{fallible_copy, fallible_filled, FeSolverMemoryBudget};
use crate::ols::{
    classify_frozen_linear_fallback, CertifiedLinearSession, CertifiedLinearSessionOutput,
    CertifiedLinearSessionProducts, FrozenLinearFallbackKind, LinearSessionGenerations,
    LinearSessionPhaseAllocatorProbe, LinearSessionReuseReport, LinearSessionWorkspaceCapacities,
    LinearSolveFrame, PpmlOrdinaryLinearOutput, PreparedOlsBreadWeightScale, PreparedOlsColumnKind,
    PreparedOlsExecutionMode, PreparedOlsFitConfig, PreparedOlsFitStatus,
    PreparedOlsFittedCoefficient,
};
use crate::prepared_fit_view::PreparedOlsFitView;

use super::categorical_fwl_cmg::{
    estimate_fused_cmg_workspace_bytes, fused_cmg_batch_size, FusedFwlCmgConfig,
    FusedFwlCmgFailureReport, FusedFwlCmgReport, FusedFwlCmgSolveOutcome,
    ReusableCategoricalFwlCmg,
};
use super::categorical_lsmr::{
    estimate_checkpoint_lsmr_workspace_bytes, estimate_joint_lsmr_workspace_bytes,
    joint_lsmr_policy, JointLsmrFrameDiagnostic, JointLsmrReport, ReusableCategoricalJointLsmr,
};
#[cfg(test)]
use super::family::poisson_objective_from_eta;
use super::family::{means_from_eta, CompensatedSum, PreparedPoissonInputs};
#[cfg(test)]
use super::memory::DIAGNOSTIC_RECORD_BYTES;
use super::memory::{begin_phase_allocation, end_phase_allocation};
use super::one_fe_profile::{ProfiledColumns, ProfiledOneFe, ProfiledRawColumns};
#[cfg(test)]
use super::plan_ppml_memory;
use super::prepared_estimator::PpmlInterruptPoint;
use super::prepared_model::{PpmlGenerationBinding, PreparedPpmlModelGeneration};
use super::profiled_intercept_curve::ProfiledInterceptCurve;
use super::{
    scheduled_inner_tolerance, strict_inner_tolerance, PoissonObjective, PpmlAllocatorSnapshot,
    PpmlAllocatorSnapshotProbe, PpmlCertificate, PpmlConfig, PpmlFallbackKind,
    PpmlInnerToleranceProgress, PpmlIterationWorkspace, PpmlLinearWorkspaceCapacityReport,
    PpmlMemoryConfig, PpmlMemoryDimensions, PpmlMemoryPlan, PpmlMemoryRoute, PpmlMemoryTelemetry,
    PpmlPhase, PpmlPhaseAllocatorProbe, PpmlPhaseClock, PpmlSolverEngine, PpmlSolverFrameKind,
    PpmlSolverObservation, PpmlSolverPlan, PpmlSolverReport, PpmlSteadyStateAllocationObservation,
    PpmlWorkspaceCapacityReport, PPML_STEADY_STATE_AUDIT_ITERATIONS,
};

mod controller;

mod retry;

mod direction;
use direction::OrdinaryDirection;

mod admission;
use admission::LinearAdmission;

const MAX_RETAINED_ITERATION_REPORTS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileRecoveryAction {
    Continue,
    Rebuild,
    Exhausted,
}

#[derive(Default)]
struct ProfileRecovery {
    discrepancies: [f64; 3],
    count: usize,
    // The rebuilt checkpoint and at most two further strict corrections.
    attempts_left: Option<usize>,
}

impl ProfileRecovery {
    fn observe(
        &mut self,
        eligible: bool,
        same_basis: bool,
        difference: f64,
    ) -> ProfileRecoveryAction {
        if !same_basis {
            *self = Self::default();
        }
        if !eligible {
            self.count = 0;
            return ProfileRecoveryAction::Continue;
        }
        if let Some(left) = &mut self.attempts_left {
            *left = left.saturating_sub(1);
            return if *left == 0 {
                ProfileRecoveryAction::Exhausted
            } else {
                ProfileRecoveryAction::Continue
            };
        }
        self.discrepancies[self.count] = difference;
        self.count += 1;
        if self.count < self.discrepancies.len() {
            return ProfileRecoveryAction::Continue;
        }
        if difference < 0.5 * self.discrepancies[0] {
            self.discrepancies[0] = difference;
            self.count = 1;
            return ProfileRecoveryAction::Continue;
        }
        self.count = 0;
        self.attempts_left = Some(3);
        ProfileRecoveryAction::Rebuild
    }
}

fn refined_checkpoint_tolerance(current: f64) -> f64 {
    // A certified warm residual may retain an old profile whose movement is
    // too large for publication. Refine computation, never the acceptance
    // envelope; preserve any user target already below the recurrence floor.
    (current * 0.01).max(1e-14).min(current)
}

fn use_checkpoint_lsmr_accelerator(observations: usize, effects: usize) -> bool {
    matches!(effects, 2 | 3) && observations > 0
}

fn use_initial_checkpoint_lsmr_accelerator(
    config: &PreparedPpmlIrlsConfig,
    purpose: PreparedPpmlIrlsPurpose,
    observations: usize,
    effects: usize,
) -> bool {
    matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
        && matches!(
            config.solver_plan.selected,
            super::PpmlSolverFamily::Lsmr | super::PpmlSolverFamily::Cmg
        )
        && use_checkpoint_lsmr_accelerator(observations, effects)
}

#[cfg(test)]
thread_local! {
    static FORCE_GENERIC_OUTER_SCORE: Cell<bool> = const { Cell::new(false) };
    static CANONICAL_FINAL_TEST_DISABLED: Cell<bool> = const { Cell::new(false) };
    static PROFILE_CURVE_TEST_DISABLED: Cell<bool> = const { Cell::new(false) };
    static PROFILE_FINAL_TEST_MODE: Cell<u8> = const { Cell::new(0) };
    static PROFILE_FINAL_ACCEPTED: Cell<usize> = const { Cell::new(0) };
    static JOINT_FINAL_TEST_MODE: Cell<u8> = const { Cell::new(0) };
    static JOINT_FINAL_TEST_COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
    static STRICT_RESUME_INJECTION: Cell<u8> = const { Cell::new(0) };
    static REPEATED_STRICT_RESUME_INJECTION: Cell<usize> = const { Cell::new(0) };
    static STRICT_BASIS_CHANGE_INJECTION: Cell<u8> = const { Cell::new(0) };
    static CANONICAL_FRAME_REFRESH_COUNT: Cell<usize> = const { Cell::new(0) };
    static FUSED_STRICT_WITNESS_COUNT: Cell<usize> = const { Cell::new(0) };
    static CMG_EARLY_MEMORY_PEAKS: Cell<Option<(u128, u128)>> = const { Cell::new(None) };
}

#[cfg(test)]
pub(super) fn inject_repeated_strict_resume(count: usize) {
    REPEATED_STRICT_RESUME_INJECTION.set(count);
}

#[cfg(test)]
fn inject_strict_resume(first_strict: bool, final_hessian: bool) {
    STRICT_RESUME_INJECTION.set(u8::from(first_strict) | (u8::from(final_hessian) << 1));
}

#[cfg(test)]
fn take_strict_resume(bit: u8) -> bool {
    if bit == 1 && REPEATED_STRICT_RESUME_INJECTION.get() > 0 {
        REPEATED_STRICT_RESUME_INJECTION.set(REPEATED_STRICT_RESUME_INJECTION.get() - 1);
        return true;
    }
    let state = STRICT_RESUME_INJECTION.get();
    let selected = state & bit != 0;
    STRICT_RESUME_INJECTION.set(state & !bit);
    selected
}

#[cfg(test)]
pub(super) fn inject_strict_basis_change(first_strict: bool, final_hessian: bool) {
    STRICT_BASIS_CHANGE_INJECTION.set(u8::from(first_strict) | (u8::from(final_hessian) << 1));
}

#[cfg(test)]
fn take_strict_basis_change(bit: u8) -> bool {
    let state = STRICT_BASIS_CHANGE_INJECTION.get();
    let selected = state & bit != 0;
    STRICT_BASIS_CHANGE_INJECTION.set(state & !bit);
    selected
}

#[cfg(test)]
pub(super) fn reset_canonical_frame_refresh_count() {
    CANONICAL_FRAME_REFRESH_COUNT.set(0);
    FUSED_STRICT_WITNESS_COUNT.set(0);
}

#[cfg(test)]
pub(super) fn take_canonical_frame_refresh_count() -> usize {
    CANONICAL_FRAME_REFRESH_COUNT.replace(0)
}

#[cfg(test)]
pub(super) fn take_fused_strict_witness_count() -> usize {
    FUSED_STRICT_WITNESS_COUNT.replace(0)
}

#[cfg(test)]
pub(super) fn take_cmg_early_memory_peaks() -> Option<(u128, u128)> {
    CMG_EARLY_MEMORY_PEAKS.replace(None)
}

#[cfg(test)]
fn inject_candidate_profile_movement(workspace: &mut PpmlIterationWorkspace) {
    // Move a complete candidate profile, not merely a control-flow flag, so
    // the strict resume tests exercise eta, beta, and objective comparisons.
    const DELTA: f64 = 1e-4;
    for value in workspace.candidate_eta_mut() {
        *value += DELTA;
    }
    if let Some(value) = workspace.candidate_coefficients_mut().last_mut() {
        *value += DELTA;
    }
}

fn profile_curve_enabled() -> bool {
    #[cfg(test)]
    {
        !PROFILE_CURVE_TEST_DISABLED.get()
    }
    #[cfg(not(test))]
    {
        true
    }
}

fn canonical_final_enabled() -> bool {
    #[cfg(test)]
    if CANONICAL_FINAL_TEST_DISABLED.get() {
        return false;
    }
    true
}

#[cfg(test)]
pub(super) struct CanonicalFinalTestGuard(bool);
#[cfg(test)]
impl CanonicalFinalTestGuard {
    pub(super) fn disabled() -> Self {
        Self(CANONICAL_FINAL_TEST_DISABLED.replace(true))
    }
}
#[cfg(test)]
impl Drop for CanonicalFinalTestGuard {
    fn drop(&mut self) {
        CANONICAL_FINAL_TEST_DISABLED.set(self.0);
    }
}

#[cfg(test)]
pub(super) struct ProfileCurveTestGuard(bool);
#[cfg(test)]
impl ProfileCurveTestGuard {
    pub(super) fn disabled() -> Self {
        Self(PROFILE_CURVE_TEST_DISABLED.replace(true))
    }
}
#[cfg(test)]
impl Drop for ProfileCurveTestGuard {
    fn drop(&mut self) {
        PROFILE_CURVE_TEST_DISABLED.set(self.0);
    }
}

#[cfg(test)]
pub(super) fn profile_final_test_mode(mode: u8) -> usize {
    PROFILE_FINAL_TEST_MODE.set(mode);
    PROFILE_FINAL_ACCEPTED.replace(0)
}

/// PPML-owned policy for calls into the shared linear engine.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PreparedPpmlIrlsConfig {
    pub(crate) ppml: PpmlConfig,
    pub(crate) linear: PreparedOlsFitConfig,
    pub(crate) memory: PpmlMemoryConfig,
    /// Qualification-only allocator observer. `None` is the production-fast
    /// default and imposes no callback or counter-read cost on OLS.
    pub(crate) allocation_snapshot_probe: Option<PpmlAllocatorSnapshotProbe>,
    pub(crate) phase_allocation_probe: Option<PpmlPhaseAllocatorProbe>,
    /// Qualification-only neutral probe installed on the shared linear
    /// session. It observes projection and finite-solve segments without
    /// coupling the OLS engine to PPML phase types.
    pub(crate) linear_phase_allocation_probe: Option<LinearSessionPhaseAllocatorProbe>,
    /// Public request/selection identity. MAP remains the canonical linear
    /// configuration even when the selected ordinary-frame engine is LSMR
    /// or prepared CMG.
    pub(crate) solver_plan: PpmlSolverPlan,
}

impl Default for PreparedPpmlIrlsConfig {
    fn default() -> Self {
        let mut ppml = PpmlConfig::default();
        if ppml.absorb.solver == FeSolverKind::Auto {
            ppml.absorb.solver = FeSolverKind::Map;
        }
        let linear = PreparedOlsFitConfig {
            absorb: ppml.absorb,
            fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
            ..PreparedOlsFitConfig::default()
        };
        Self {
            ppml,
            linear,
            memory: PpmlMemoryConfig::default(),
            allocation_snapshot_probe: None,
            phase_allocation_probe: None,
            linear_phase_allocation_probe: None,
            solver_plan: PpmlSolverPlan::new(super::PpmlSolverFamily::Auto),
        }
    }
}

/// Warm start on the canonical prepared coefficient-slot axis.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedPpmlStart {
    pub(crate) coefficient_values: Vec<f64>,
    /// Full linear predictor, including offset and absorbed contribution.
    pub(crate) eta: Vec<f64>,
}

/// Why a certified nonpublication trajectory stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreparedPpmlMuTerminal {
    Converged,
    MaximumIterations,
    LineSearchStalled,
    CandidateThresholdReached,
}

/// Independent integrity certificate for an exploratory Mu trajectory.
///
/// This deliberately does not require the PPML score certificate to pass: an
/// exact separated sample has no finite score root. It instead certifies that
/// every accepted state came from a certified shared-linear subproblem and
/// the unchanged bounded line search, that deviance respected its registered
/// slack, and that the retained means independently reconstruct from eta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PreparedPpmlMuExplorationCertificate {
    pub(crate) accepted_iterations: usize,
    pub(crate) certified_linear_subproblems: usize,
    pub(crate) maximum_deviance_increase_relative: f64,
    pub(crate) mean_reconstruction_relative: f64,
    pub(crate) terminal: PreparedPpmlMuTerminal,
    pub(crate) passed: bool,
}

/// Certified exploratory state usable only by fitted-mean Mu separation.
///
/// It intentionally has no conversion to `PreparedPpmlIrlsFit`; consuming it
/// can only inspect the compact eta/mean trajectory. Publication must perform
/// a new full converged, strict, final-Hessian-synchronized fit.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedPpmlMuExploration {
    binding: Option<PpmlGenerationBinding>,
    mean: Vec<f64>,
    certificate: PreparedPpmlMuExplorationCertificate,
}

impl PreparedPpmlMuExploration {
    pub(super) fn validate_generation(
        &self,
        generation: &PreparedPpmlModelGeneration,
    ) -> Result<()> {
        self.binding
            .as_ref()
            .ok_or_else(|| invalid_argument("PPML artifact has no prepared-generation binding"))?
            .validate(generation, self.mean.len())
    }
    pub(crate) fn mean(&self) -> &[f64] {
        &self.mean
    }

    pub(crate) const fn certificate(&self) -> PreparedPpmlMuExplorationCertificate {
        self.certificate
    }

    #[cfg(test)]
    pub(crate) fn from_test_eta(
        eta: Vec<f64>,
        generation: &PreparedPpmlModelGeneration,
        bounds: super::PoissonBounds,
    ) -> Result<Self> {
        let mut mean = vec![0.0; eta.len()];
        means_from_eta(&eta, bounds, &mut mean)?;
        let _objective = poisson_objective_from_eta(
            generation.outcome(),
            &eta,
            &mean,
            generation.prior_weights(),
        )?;
        Ok(Self {
            binding: Some(PpmlGenerationBinding::capture(generation)?),
            mean,
            certificate: PreparedPpmlMuExplorationCertificate {
                accepted_iterations: 1,
                certified_linear_subproblems: 1,
                maximum_deviance_increase_relative: 0.0,
                mean_reconstruction_relative: 0.0,
                terminal: PreparedPpmlMuTerminal::MaximumIterations,
                passed: true,
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum PreparedPpmlIrlsPurpose {
    Publication,
    MuExploration { mu_tolerance: f64 },
}

pub(super) enum PreparedPpmlIrlsRun {
    Publication(Box<PreparedPpmlIrlsFit>),
    MuExploration(Box<PreparedPpmlMuExploration>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedPpmlIterationReport {
    pub(crate) iteration: usize,
    pub(crate) deviance: f64,
    pub(crate) log_likelihood: f64,
    pub(crate) relative_deviance_change: f64,
    pub(crate) step: f64,
    pub(crate) step_halvings: usize,
    pub(crate) inner_tolerance: f64,
    pub(crate) finite_rank: usize,
    pub(crate) score: PpmlCertificate,
    pub(crate) linear_reuse: LinearSessionReuseReport,
    pub(crate) joint_lsmr: Option<JointLsmrFrameDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedPpmlIrlsDiagnostics {
    pub(crate) converged: bool,
    pub(crate) two_consecutive_convergence: bool,
    pub(crate) rank_frozen: bool,
    pub(crate) iterations: usize,
    /// Checked sum over every executed certified linear subproblem. Each
    /// contributes at least one unit, or its FE solver iteration count when
    /// larger; line-search trials are not linear subproblems.
    pub(crate) total_inner_iterations: usize,
    pub(crate) retained_iterations: Vec<PreparedPpmlIterationReport>,
    pub(crate) total_step_halvings: usize,
    pub(crate) strict_profile_rejections: usize,
    pub(crate) final_score: PpmlCertificate,
    pub(crate) solver: PpmlSolverReport,
    pub(crate) memory: PpmlMemoryTelemetry,
}

/// Converged PPML state and its owned final Hessian products.
///
/// The owned handoff keeps either dense or scratch-backed compact within rows
/// alive for inference without reconstructing an observation-by-slot matrix.
pub(crate) struct PreparedPpmlIrlsFit {
    binding: Option<PpmlGenerationBinding>,
    bias: Option<super::PpmlBiasResult>,
    linear: CertifiedLinearSessionProducts,
    coefficients: Vec<PreparedOlsFittedCoefficient>,
    eta: Vec<f64>,
    mean: Vec<f64>,
    score_residuals: Vec<f64>,
    objective: PoissonObjective,
    diagnostics: PreparedPpmlIrlsDiagnostics,
}

impl PreparedPpmlIrlsFit {
    pub(super) fn validate_generation(
        &self,
        generation: &PreparedPpmlModelGeneration,
    ) -> Result<()> {
        self.binding
            .as_ref()
            .ok_or_else(|| invalid_argument("PPML artifact has no prepared-generation binding"))?
            .validate(generation, self.mean.len())
    }
    pub(crate) fn take_bias(&mut self) -> Option<super::PpmlBiasResult> {
        self.bias.take()
    }
    pub(crate) fn coefficients(&self) -> &[PreparedOlsFittedCoefficient] {
        &self.coefficients
    }

    pub(crate) fn eta(&self) -> &[f64] {
        &self.eta
    }

    pub(crate) fn mean(&self) -> &[f64] {
        &self.mean
    }

    pub(crate) fn score_residuals(&self) -> &[f64] {
        &self.score_residuals
    }

    pub(crate) const fn objective(&self) -> PoissonObjective {
        self.objective
    }

    pub(crate) const fn diagnostics(&self) -> &PreparedPpmlIrlsDiagnostics {
        &self.diagnostics
    }

    pub(crate) const fn linear(&self) -> &CertifiedLinearSessionProducts {
        &self.linear
    }
}

struct PpmlScoreWorkspace {
    team: crate::parallel::InstalledThreadTeam,
    fe_moments: Vec<f64>,
    fe_basis_squares: Vec<f64>,
}

impl PpmlScoreWorkspace {
    fn new(design: &FeDesign, threads: crate::options::ThreadConfig) -> Result<Self> {
        let directions = design.effects().iter().try_fold(0_usize, |total, effect| {
            let effect_directions = effect
                .n_levels()
                .checked_mul(effect.channels())
                .ok_or_else(|| allocation("prepared PPML FE-score effect dimensions overflow"))?;
            total
                .checked_add(effect_directions)
                .ok_or_else(|| allocation("prepared PPML FE-score dimensions overflow"))
        })?;
        Ok(Self {
            team: crate::parallel::ThreadTeam::new(threads).acquire()?,
            fe_moments: fallible_filled(
                directions,
                0.0,
                "unable to reserve prepared PPML FE-score moments",
            )?,
            fe_basis_squares: fallible_filled(
                directions,
                0.0,
                "unable to reserve prepared PPML FE-score norms",
            )?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PpmlAllocationAuditBoundary {
    allocator: PpmlAllocatorSnapshot,
    ppml_workspace: PpmlWorkspaceCapacityReport,
    linear_workspace: LinearSessionWorkspaceCapacities,
    linear_workspace_report: PpmlLinearWorkspaceCapacityReport,
    profile_workspace_valid: Option<bool>,
    scratch_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct PendingPpmlAllocationAudit {
    start: PpmlAllocationAuditBoundary,
    end: PpmlAllocationAuditBoundary,
    measured_iterations: usize,
    linear_route: FeSolverKind,
    linear_session_allocation_free: bool,
    iteration_nanoseconds: [u128; PPML_STEADY_STATE_AUDIT_ITERATIONS],
}

impl PendingPpmlAllocationAudit {
    fn observation(self, fresh_warm_equivalent: bool) -> PpmlSteadyStateAllocationObservation {
        PpmlSteadyStateAllocationObservation {
            measured_iterations: self.measured_iterations,
            allocator_events_start: self.start.allocator.allocation_events,
            allocator_events_end: self.end.allocator.allocation_events,
            allocator_current_start: self.start.allocator.current_bytes,
            allocator_current_end: self.end.allocator.current_bytes,
            allocator_peak_start: self.start.allocator.peak_bytes,
            allocator_peak_end: self.end.allocator.peak_bytes,
            workspace_start: self.start.ppml_workspace,
            workspace_end: self.end.ppml_workspace,
            linear_workspace_start: self.start.linear_workspace_report,
            linear_workspace_end: self.end.linear_workspace_report,
            linear_workspace_capacity_stable: self.start.linear_workspace
                == self.end.linear_workspace
                && self.start.profile_workspace_valid == self.end.profile_workspace_valid
                && self.start.profile_workspace_valid != Some(false),
            scratch_bytes_start: self.start.scratch_bytes,
            scratch_bytes_end: self.end.scratch_bytes,
            linear_route: self.linear_route,
            linear_session_allocation_free: self.linear_session_allocation_free,
            fresh_warm_equivalent,
            steady_iteration_median_nanoseconds: median_nanoseconds(self.iteration_nanoseconds),
        }
    }
}

#[derive(Debug)]
struct PpmlAllocationAuditWindow {
    probe: Option<PpmlAllocatorSnapshotProbe>,
    start: Option<PpmlAllocationAuditBoundary>,
    pending: Option<PendingPpmlAllocationAudit>,
    measured_iterations: usize,
    linear_route: FeSolverKind,
    linear_session_allocation_free: bool,
    iteration_nanoseconds: [u128; PPML_STEADY_STATE_AUDIT_ITERATIONS],
    finished: bool,
}

impl PpmlAllocationAuditWindow {
    const fn new(probe: Option<PpmlAllocatorSnapshotProbe>) -> Self {
        Self {
            probe,
            start: None,
            pending: None,
            measured_iterations: 0,
            linear_route: FeSolverKind::Map,
            linear_session_allocation_free: true,
            iteration_nanoseconds: [0; PPML_STEADY_STATE_AUDIT_ITERATIONS],
            finished: false,
        }
    }

    const fn is_measuring(&self) -> bool {
        self.start.is_some()
    }

    const fn needs_equivalence(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether an explicitly requested qualification still needs its warmed
    /// window and independent equivalence solve.  Strict convergence must not
    /// terminate the fit during capacity warm-up: the first frozen solve is
    /// allowed to populate the shared kernel workspace, and only a later
    /// complete iteration may open the measured window.
    const fn qualification_pending(&self) -> bool {
        self.probe.is_some() && !self.finished
    }

    fn can_start_after(
        &self,
        iteration: usize,
        max_iterations: usize,
        reuse: LinearSessionReuseReport,
        route: FeSolverKind,
    ) -> bool {
        self.probe.is_some()
            && self.start.is_none()
            && self.pending.is_none()
            && !self.finished
            && reuse.steady_state_allocation_free
            && route == FeSolverKind::Map
            // Sixteen complete iterations are measured after this boundary;
            // the following iteration performs the out-of-window canonical
            // equivalence solve.
            && max_iterations.saturating_sub(iteration)
                > PPML_STEADY_STATE_AUDIT_ITERATIONS
    }

    fn start_after<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        session: &CertifiedLinearSession<'_, P>,
        workspace: &PpmlIterationWorkspace,
        profile: Option<&ProfiledOneFe>,
        curve: Option<&ProfiledInterceptCurve>,
    ) -> Result<()> {
        let probe = self.probe.ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "PPML allocation audit start lacks its allocator probe",
            )
        })?;
        self.start = Some(allocation_audit_start_boundary(
            session, workspace, profile, curve, probe,
        )?);
        self.measured_iterations = 0;
        self.linear_route = FeSolverKind::Map;
        self.linear_session_allocation_free = true;
        self.iteration_nanoseconds = [0; PPML_STEADY_STATE_AUDIT_ITERATIONS];
        Ok(())
    }

    // Keep the legacy workspace and ownership boundaries explicit during migration.
    #[allow(clippy::too_many_arguments)]
    fn finish_iteration<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        session: &CertifiedLinearSession<'_, P>,
        workspace: &PpmlIterationWorkspace,
        reuse: LinearSessionReuseReport,
        route: FeSolverKind,
        elapsed: Duration,
        profile: Option<&ProfiledOneFe>,
        curve: Option<&ProfiledInterceptCurve>,
    ) -> Result<()> {
        let Some(start) = self.start else {
            return Ok(());
        };
        let timing_slot = self.measured_iterations;
        if timing_slot >= PPML_STEADY_STATE_AUDIT_ITERATIONS {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML allocation audit recorded too many iteration timings",
            ));
        }
        self.iteration_nanoseconds[timing_slot] = elapsed.as_nanos();
        self.measured_iterations = self.measured_iterations.checked_add(1).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "PPML allocation audit iteration count overflowed",
            )
        })?;
        self.linear_route = route;
        self.linear_session_allocation_free &=
            route == FeSolverKind::Map && reuse.steady_state_allocation_free;
        if self.measured_iterations < PPML_STEADY_STATE_AUDIT_ITERATIONS {
            return Ok(());
        }
        let probe = self.probe.ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "PPML allocation audit end lacks its allocator probe",
            )
        })?;
        // The end snapshot is the first operation after the complete measured
        // iteration. Capacity/scratch reads that follow are outside the
        // allocator window and are themselves allocation-free.
        let end = allocation_audit_end_boundary(session, workspace, profile, curve, probe)?;
        self.pending = Some(PendingPpmlAllocationAudit {
            start,
            end,
            measured_iterations: self.measured_iterations,
            linear_route: self.linear_route,
            linear_session_allocation_free: self.linear_session_allocation_free,
            iteration_nanoseconds: self.iteration_nanoseconds,
        });
        self.start = None;
        Ok(())
    }

    fn complete_equivalence(
        &mut self,
        equivalent: bool,
    ) -> Result<PpmlSteadyStateAllocationObservation> {
        let pending = self.pending.take().ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "PPML allocation audit equivalence has no completed window",
            )
        })?;
        self.finished = true;
        Ok(pending.observation(equivalent))
    }
}

fn allocation_audit_start_boundary<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &CertifiedLinearSession<'_, P>,
    workspace: &PpmlIterationWorkspace,
    profile: Option<&ProfiledOneFe>,
    curve: Option<&ProfiledInterceptCurve>,
    probe: PpmlAllocatorSnapshotProbe,
) -> Result<PpmlAllocationAuditBoundary> {
    let ppml_workspace = workspace.capacity_report();
    let linear_workspace = session.workspace_capacities();
    let linear_workspace_report = summarize_linear_workspace(linear_workspace)?;
    let scratch_bytes = session
        .retained_within_memory()
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "PPML allocation audit start lacks retained within storage",
            )
        })?
        .scratch_bytes;
    let profile_workspace_valid = profile
        .map(ProfiledOneFe::capacities_unchanged)
        .or_else(|| curve.map(ProfiledInterceptCurve::capacities_unchanged));
    // Capture last: the window begins after all boundary evidence has been
    // read and immediately before the next complete IRLS iteration.
    let allocator = probe();
    Ok(PpmlAllocationAuditBoundary {
        allocator,
        ppml_workspace,
        linear_workspace,
        linear_workspace_report,
        profile_workspace_valid,
        scratch_bytes,
    })
}

fn allocation_audit_end_boundary<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &CertifiedLinearSession<'_, P>,
    workspace: &PpmlIterationWorkspace,
    profile: Option<&ProfiledOneFe>,
    curve: Option<&ProfiledInterceptCurve>,
    probe: PpmlAllocatorSnapshotProbe,
) -> Result<PpmlAllocationAuditBoundary> {
    // Capture first: nothing after this read belongs to the measured window.
    let allocator = probe();
    let ppml_workspace = workspace.capacity_report();
    let linear_workspace = session.workspace_capacities();
    let linear_workspace_report = summarize_linear_workspace(linear_workspace)?;
    let scratch_bytes = session
        .retained_within_memory()
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "PPML allocation audit end lacks retained within storage",
            )
        })?
        .scratch_bytes;
    Ok(PpmlAllocationAuditBoundary {
        allocator,
        ppml_workspace,
        linear_workspace,
        linear_workspace_report,
        profile_workspace_valid: profile
            .map(ProfiledOneFe::capacities_unchanged)
            .or_else(|| curve.map(ProfiledInterceptCurve::capacities_unchanged)),
        scratch_bytes,
    })
}

fn summarize_linear_workspace(
    capacity: LinearSessionWorkspaceCapacities,
) -> Result<PpmlLinearWorkspaceCapacityReport> {
    let elements = [
        capacity.constant_recipe_elements,
        capacity.response_elements,
        capacity.weight_elements,
        capacity.finite_predictor_elements,
        capacity.absorbed_predictor_elements,
        capacity.moment_elements,
        capacity.row_elements,
        capacity.frozen_response_elements,
        capacity.frozen_gram_elements,
        capacity.frozen_factor_elements,
        capacity.frozen_inverse_lower_elements,
        capacity.frozen_rhs_elements,
        capacity.frozen_moment_partial_elements,
        capacity.frozen_row_elements,
        capacity.frozen_solve_elements,
        capacity.frozen_permutation_elements,
        capacity.frozen_score_elements,
        capacity.frozen_coefficient_elements,
        capacity.frozen_bread_elements,
        capacity.frozen_residual_elements,
        capacity.frozen_reported_coefficient_elements,
        capacity.frozen_kept_elements,
        capacity.frozen_candidate_position_elements,
        capacity.frozen_candidate_identity_elements,
        capacity.frozen_candidate_scale_elements,
        capacity.frozen_basis_slot_elements,
        capacity.map_weight_elements,
        capacity.projection_block_elements,
        capacity.within_resident_elements,
        capacity.within_io_buffer_elements,
    ];
    let element_capacity_sum = elements
        .iter()
        .chain(capacity.fe.element_capacities().iter())
        .chain(capacity.within_metadata.iter())
        .chain(capacity.basis_metadata.iter())
        .try_fold(0_u128, |sum, value| {
            sum.checked_add(*value as u128).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "PPML linear workspace capacity summary overflowed",
                )
            })
        })?;
    Ok(PpmlLinearWorkspaceCapacityReport {
        element_capacity_sum,
        retained_resident_elements: capacity.within_resident_elements,
        retained_scratch_bytes: capacity.within_scratch_bytes,
        fe_topology_resident_bytes: capacity.fe.topology_resident_bytes,
    })
}

fn median_nanoseconds(mut values: [u128; PPML_STEADY_STATE_AUDIT_ITERATIONS]) -> u128 {
    values.sort_unstable();
    let right = values[PPML_STEADY_STATE_AUDIT_ITERATIONS / 2];
    let left = values[PPML_STEADY_STATE_AUDIT_ITERATIONS / 2 - 1];
    // Overflow-free floor of (left + right) / 2.
    left / 2 + right / 2 + (left % 2 + right % 2) / 2
}

/// Fit PPML after separation and topology rebasing are complete.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn fit_prepared_ppml_irls<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
) -> Result<PreparedPpmlIrlsFit> {
    fit_prepared_ppml_irls_interruptible(
        prepared,
        outcome,
        prior_weights,
        offset,
        config,
        memory_plan,
        start,
        &mut |_| Ok(()),
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn fit_prepared_ppml_irls_interruptible<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedPpmlIrlsFit> {
    match run_prepared_ppml_irls(
        PreparedPpmlFitFrame {
            prepared,
            shared_prepared: None,
            outcome,
            prior_weights,
            offset,
            binding: None,
        },
        config,
        memory_plan,
        start,
        PreparedPpmlIrlsPurpose::Publication,
        None,
        interrupt,
    )? {
        PreparedPpmlIrlsRun::Publication(fit) => Ok(*fit),
        PreparedPpmlIrlsRun::MuExploration(_) => Err(FeregError::new(
            ErrorCode::Internal,
            "publication PPML run returned an exploratory state",
        )),
    }
}

/// Every numerical consumer borrows the same finalized original-row axis.
/// The model epoch is independent of changing response/weight/basis stamps.
struct PreparedPpmlFitFrame<'a, P: ?Sized> {
    prepared: &'a P,
    shared_prepared: Option<crate::prepared_fit_view::SharedPreparedOlsData>,
    outcome: &'a [f64],
    prior_weights: &'a [f64],
    offset: &'a [f64],
    binding: Option<PpmlGenerationBinding>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fit_generation_ppml_irls_interruptible(
    generation: &super::prepared_model::PreparedPpmlModelGeneration,
    config: PreparedPpmlIrlsConfig,
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
    purpose: PreparedPpmlIrlsPurpose,
    bias: Option<(super::PpmlBiasRequest, &[f64])>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedPpmlIrlsRun> {
    let frame = PreparedPpmlFitFrame {
        prepared: generation.fit_view(),
        // Preserve exploratory storage admission: only publication may retain
        // the shared implicit-column source beside its canonical checkpoint.
        shared_prepared: matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            .then(|| generation.prepared().clone()),
        outcome: generation.outcome(),
        prior_weights: generation.prior_weights(),
        offset: generation.offset(),
        binding: Some(PpmlGenerationBinding::capture(generation)?),
    };
    run_prepared_ppml_irls(frame, config, memory_plan, start, purpose, bias, interrupt)
}

#[allow(clippy::too_many_arguments)]
fn run_prepared_ppml_irls<P: PreparedOlsFitView + Sync + ?Sized>(
    frame: PreparedPpmlFitFrame<'_, P>,
    mut config: PreparedPpmlIrlsConfig,
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
    purpose: PreparedPpmlIrlsPurpose,
    bias: Option<(super::PpmlBiasRequest, &[f64])>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedPpmlIrlsRun> {
    let PreparedPpmlFitFrame {
        prepared,
        mut shared_prepared,
        outcome,
        prior_weights,
        offset,
        binding,
    } = frame;
    let model_generation = binding
        .as_ref()
        .map_or(0, PpmlGenerationBinding::generation);
    // Direct internal callers may start from the shared FE default. Resolve
    // that default to the canonical PPML checkpoint engine before enforcing
    // the invariant; public route selection remains fixed in `solver_plan`.
    if config.ppml.absorb.solver == FeSolverKind::Auto {
        config.ppml.absorb.solver = FeSolverKind::Map;
    }
    if config.linear.absorb.solver == FeSolverKind::Auto {
        config.linear.absorb.solver = FeSolverKind::Map;
    }
    begin_phase_allocation(config.phase_allocation_probe, PpmlPhase::Preparation);
    let preparation_clock = PpmlPhaseClock::start();
    validate_inputs(prepared, outcome, prior_weights, offset, config)?;
    validate_memory_dimensions(prepared, memory_plan.dimensions, config)?;
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=prepared_irls_start|original_rows={}|active_rows={}|finite_columns={}|memory={:?}",
            memory_plan.dimensions.original_observations,
            outcome.len(),
            memory_plan.dimensions.finite_columns,
            memory_plan.selected,
        );
    }
    // Route selection happened exactly once on generation zero, before any
    // separation detector workspace. The composite owner rebases only the
    // exact final dimensions into that conservative fit-lifetime plan.
    let admission = LinearAdmission::new_public(prepared, memory_plan, &mut config, purpose)?;
    // The route and its complete fit-lifetime budget are admitted before this
    // observation-scale owned copy of the already-finalized FE topology.
    let fe_design = prepared.fit_owned_fe_design()?;
    let requested_threads = config
        .linear
        .execution
        .threads
        .requested
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(0);
    let effective_threads = config.linear.execution.threads.resolved();
    let (memory_plan, solver_admission, solver_memory_budget, installation) = admission
        .admit_installation(
            prepared,
            &fe_design,
            &mut config,
            purpose,
            effective_threads,
            shared_prepared.is_some(),
        )?;
    let mut memory = PpmlMemoryTelemetry::new(memory_plan, requested_threads, effective_threads)?;
    if config.solver_plan.selected == super::PpmlSolverFamily::Cmg {
        memory.expected_solver_workspace_peak =
            Some(crate::memory::SolverWorkspacePeak::from_bytes(
                solver_admission.checkpoint_workspace_bytes
                    + solver_admission.owned_fe_design_shortfall_bytes,
            )?);
    }
    // Admission and telemetry bind this fit to one resolved team. Repeated
    // automatic resolution may allocate while querying OS CPU quotas (Linux),
    // and must not change the team behind the admitted workspace dimensions.
    // Keep the caller's requested value in telemetry, then use this explicit
    // count for every ordinary, fallback and strict linear frame in the fit.
    config.linear.execution.threads.requested = std::num::NonZeroUsize::new(effective_threads);

    let (coefficient_values, eta) = initialize(
        prepared,
        outcome,
        prior_weights,
        offset,
        &fe_design,
        config,
        start,
        purpose,
    )?;
    let objective_inputs = PreparedPoissonInputs::new_with_threads(
        outcome,
        prior_weights,
        offset,
        config.linear.execution.threads,
    )?;
    let mut workspace =
        PpmlIterationWorkspace::from_initial_state(memory_plan, eta, coefficient_values)?;
    workspace.refresh_prepared_working_state(&objective_inputs, config.ppml.bounds)?;
    let mut objective = workspace.evaluate_current_objective(&objective_inputs)?;
    let mut score_workspace = PpmlScoreWorkspace::new(&fe_design, config.linear.execution.threads)?;
    let initial_tolerance =
        scheduled_inner_tolerance(config.ppml, 1, config.ppml.inner_tolerance_initial, None);
    let initial_config = ppml_linear_config(config, initial_tolerance)?;
    let frame = LinearSolveFrame::from_parts(
        workspace.working_response(),
        BorrowedMapWeights::Provided(workspace.hessian_weights()),
        ppml_bread_weight_scale(workspace.hessian_weights())?,
    )?;
    let (mut session, profiles, initial_checkpoint_lsmr) = installation.install(
        prepared,
        frame,
        initial_config,
        LinearSessionGenerations::new(model_generation, model_generation, 1, 1, 1),
        fe_design,
        &mut shared_prepared,
        &config,
        purpose,
        effective_threads,
    )?;
    let mut direction = OrdinaryDirection::Canonical;
    let mut profiled_curve: Option<ProfiledInterceptCurve> = None;
    let mut strict_control = controller::StrictCheckpointControl::new(
        strict_inner_tolerance(config.ppml),
        canonical_final_enabled()
            && matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
            && session.fe_design().effects().len() >= 2,
    );
    let mut solver_report = None;
    let retained_limit = MAX_RETAINED_ITERATION_REPORTS
        .min(config.memory.diagnostic_capacity)
        .min(config.ppml.max_iterations);
    let mut retained_iterations = Vec::new();
    retained_iterations
        .try_reserve_exact(retained_limit)
        .map_err(|_| allocation("unable to reserve prepared PPML diagnostics"))?;
    let mut total_step_halvings = 0_usize;
    let mut final_score = prepared_score_certificate(
        prepared,
        session.fe_design(),
        outcome,
        workspace.mean(),
        prior_weights,
        config.ppml.score_tolerance,
        &mut score_workspace,
    )?;
    let mut previous_inner_tolerance = initial_tolerance;
    let mut previous_outer_progress = None;
    let mut rank_frozen = false;
    let mut completed_iterations = 0_usize;
    let mut total_inner_iterations = 0_usize;
    let mut frame_generation = 1_u64;
    let mut accepted_iterations = 0_usize;
    let mut certified_linear_subproblems = 0_usize;
    let mut maximum_deviance_increase_relative = 0.0_f64;
    let mut exploration_terminal = PreparedPpmlMuTerminal::MaximumIterations;
    let mut allocation_audit = PpmlAllocationAuditWindow::new(config.allocation_snapshot_probe);
    memory.record_phase(PpmlPhase::Preparation, preparation_clock.elapsed());
    record_phase_allocation(
        &mut memory,
        config.phase_allocation_probe,
        PpmlPhase::Preparation,
    );

    for iteration in 1..=config.ppml.max_iterations {
        crate::interrupt::checkpoint()?;
        interrupt(PpmlInterruptPoint::IrlsIteration)?;
        // The qualification-only median covers the same complete warmed
        // iterations as the allocator window.  `None` is the normal path and
        // avoids even an extra clock read.
        let audited_iteration_clock = allocation_audit.is_measuring().then(Instant::now);
        completed_iterations = iteration;
        let inner_tolerance = scheduled_inner_tolerance(
            config.ppml,
            iteration,
            previous_inner_tolerance,
            previous_outer_progress,
        );
        previous_inner_tolerance = inner_tolerance;
        begin_phase_allocation(
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
        let refresh_clock = PpmlPhaseClock::start();
        session.set_config(ppml_linear_config(config, inner_tolerance)?)?;
        if iteration > 1 {
            frame_generation = next_frame_generation(frame_generation)?;
            if direction.is_canonical() {
                refresh_linear_frame(&mut session, &workspace, frame_generation)?;
            }
        }
        memory.record_phase(PpmlPhase::WeightResponseRefresh, refresh_clock.elapsed());
        record_phase_allocation(
            &mut memory,
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
        let mut profiled_step = None;
        if iteration > 1 {
            if let Some(profile) = direction.profile_mut() {
                let started = Instant::now();
                let profile_direction = if config.linear.execution.mode
                    == PreparedOlsExecutionMode::Low
                    || session.has_implicit_one_fe()
                {
                    let columns = session.one_fe_original_columns(profile.active_columns())?;
                    let data = ProfiledRawColumns::new(prepared, columns)?;
                    profile.prepare_direction(
                        &data,
                        session.fe_design(),
                        outcome,
                        prior_weights,
                        offset,
                        workspace.coefficients(),
                        config.ppml,
                    )
                } else {
                    let (values, positions) =
                        session.one_fe_representatives(profile.active_columns())?;
                    profile.prepare_direction(
                        ProfiledColumns::selected(values, positions)?,
                        session.fe_design(),
                        outcome,
                        prior_weights,
                        offset,
                        workspace.coefficients(),
                        config.ppml,
                    )
                };
                match profile_direction {
                    Ok(error) => {
                        profiled_step = Some((
                            profile.active_rank(),
                            error,
                            started.elapsed().as_secs_f64(),
                        ));
                        if allocation_audit.needs_equivalence() {
                            let equivalent = profile.fresh_direction_equivalent(
                                prepared,
                                session.fe_design(),
                                outcome,
                                prior_weights,
                                offset,
                                workspace.coefficients(),
                                config.linear.execution.threads,
                                config.ppml,
                                fresh_warm_tolerance(config.ppml, inner_tolerance),
                            )?;
                            let observation = allocation_audit.complete_equivalence(equivalent)?;
                            memory.observe_steady_state_allocation(observation)?;
                        }
                    }
                    Err(error) if error.code == ErrorCode::NonConvergence => {
                        if allocation_audit.needs_equivalence() {
                            let observation = allocation_audit.complete_equivalence(false)?;
                            memory.observe_steady_state_allocation(observation)?;
                        }
                        direction.retire_profile();
                        memory.record_fallback(PpmlFallbackKind::Route);
                        refresh_linear_frame(&mut session, &workspace, frame_generation)?;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        let mut joint_lsmr_frame = None;
        let (finite_rank, reuse, linear_route) = if iteration == 1 {
            let output = session.solve_fresh()?;
            if !output.certified() {
                return Err(numerical_nonconvergence(
                    "prepared PPML initial weighted-linear solve did not certify",
                ));
            }
            certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
            accumulate_inner_iterations(
                &mut total_inner_iterations,
                output.frame().absorption_report(),
            )?;
            let selected_engine = if matches!(purpose, PreparedPpmlIrlsPurpose::Publication) {
                config
                    .solver_plan
                    .selected_engine(output.basis().kept_generated_columns().len())
            } else {
                PpmlSolverEngine::Map
            };
            let mut initial_solver_report =
                PpmlSolverReport::new(config.solver_plan, selected_engine);
            if initial_checkpoint_lsmr {
                initial_solver_report.enable_lsmr_checkpoint();
                record_checkpoint_solver_observation(
                    &mut initial_solver_report,
                    output.frame().absorption_report(),
                    false,
                )?;
            } else {
                initial_solver_report.observe(map_solver_observation(
                    output.frame().absorption_report(),
                    PpmlSolverFrameKind::Checkpoint,
                ))?;
            }
            fill_candidate_eta_from_linear_output(&output, offset, workspace.candidate_eta_mut())?;
            fill_candidate_coefficients_from_linear_output(
                &output,
                workspace.candidate_coefficients_mut(),
            )?;
            let specialized_columns = (selected_engine != PpmlSolverEngine::Map
                || profiles.one_fe.is_some()
                || profiles.curve.is_some())
            .then(|| {
                fallible_copy(
                    output.basis().kept_generated_columns(),
                    "unable to reserve PPML specialized active-column identity",
                )
            })
            .transpose()?;
            let result = (
                output.frame().finite_rank(),
                output.reuse_report(),
                output.frame().absorption_report().used_solver,
            );
            profiled_curve = direction.initialize(
                prepared,
                session.fe_design(),
                specialized_columns.as_deref(),
                &profiles,
                outcome,
                prior_weights,
                config,
                selected_engine,
                solver_admission,
                solver_memory_budget,
            )?;
            if selected_engine != PpmlSolverEngine::Map
                && !initial_checkpoint_lsmr
                && use_checkpoint_lsmr_accelerator(
                    session.fe_design().nobs(),
                    session.fe_design().effects().len(),
                )
            {
                session.enable_checkpoint_lsmr(config.linear.execution.threads.deterministic);
                initial_solver_report.enable_lsmr_checkpoint();
            }
            solver_report = Some(initial_solver_report);
            if let Some(cmg) = direction.cmg() {
                record_cmg_heap_forecast(&mut memory, cmg, solver_admission)?;
            }
            result
        } else if let Some((rank, error, seconds)) = profiled_step {
            total_inner_iterations = total_inner_iterations
                .checked_add(1)
                .ok_or_else(|| internal("profile step count overflow"))?;
            solver_report
                .as_mut()
                .ok_or_else(|| internal("profile step lacks diagnostics"))?
                .observe(PpmlSolverObservation {
                    engine: PpmlSolverEngine::Map,
                    frame: PpmlSolverFrameKind::Ordinary,
                    retries: 0,
                    iterations: 1,
                    setup_seconds: 0.,
                    solve_seconds: seconds,
                    certification_seconds: 0.,
                    absolute_infinity_norm: None,
                    relative_infinity_norm: Some(error),
                    relative_two_norm: None,
                    normal_equation: Some(error),
                    projection_absolute_infinity_norm: None,
                    backward_error: Some(error),
                })?;
            rank_frozen = true;
            let mut reuse = LinearSessionReuseReport::ppml_specialized(0., seconds);
            reuse.steady_state_allocation_free = direction
                .profile()
                .is_some_and(ProfiledOneFe::capacities_unchanged);
            (rank, reuse, FeSolverKind::Map)
        } else if let Some((joint, engine)) = direction.lsmr_mut() {
            let report = joint.solve(
                session.fe_design(),
                outcome,
                workspace.mean(),
                workspace.hessian_weights(),
                inner_tolerance,
                config.linear.absorb.lsmr.max_iterations,
            )?;
            joint_lsmr_frame = Some(report.frame);
            joint.fill_candidate(&mut workspace)?;
            certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
            total_inner_iterations = total_inner_iterations
                .checked_add(report.iterations.max(1))
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "prepared PPML total inner iteration count overflowed",
                    )
                })?;
            solver_report
                .as_mut()
                .ok_or_else(|| internal("PPML LSMR solve lacks solver diagnostics"))?
                .observe(joint_solver_observation(report, engine))?;
            rank_frozen = true;
            (
                joint.active_rank(),
                LinearSessionReuseReport::ppml_specialized(
                    report.setup_seconds + report.solve_seconds,
                    report.certification_seconds,
                ),
                FeSolverKind::Lsmr,
            )
        } else if direction.cmg().is_some() {
            let attempt = direction
                .cmg_mut()
                .expect("fused CMG presence checked")
                .solve(
                    prepared,
                    session.fe_design(),
                    workspace.working_response(),
                    workspace.hessian_weights(),
                    inner_tolerance,
                )?;
            record_cmg_heap_forecast(
                &mut memory,
                direction.cmg().expect("CMG attempt retained"),
                solver_admission,
            )?;
            match attempt {
                FusedFwlCmgSolveOutcome::Certified(report) => {
                    let cmg = direction.cmg().expect("successful fused CMG is retained");
                    cmg.fill_candidate(&mut workspace, offset)?;
                    certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
                    total_inner_iterations = total_inner_iterations
                        .checked_add(report.iterations.max(1))
                        .ok_or_else(|| {
                            FeregError::new(
                                ErrorCode::Internal,
                                "prepared PPML total inner iteration count overflowed",
                            )
                        })?;
                    solver_report
                        .as_mut()
                        .ok_or_else(|| internal("PPML CMG solve lacks solver diagnostics"))?
                        .observe(cmg_solver_observation(report))?;
                    rank_frozen = true;
                    (
                        cmg.active_rank(),
                        LinearSessionReuseReport::ppml_specialized(
                            report.setup_seconds + report.solve_seconds,
                            report.finish_seconds,
                        ),
                        FeSolverKind::Cmg,
                    )
                }
                FusedFwlCmgSolveOutcome::NumericalFailure(report) => {
                    let result = retry::canonical_retry(
                        retry::CanonicalRetryReason::Cmg(report),
                        &mut direction,
                        &mut session,
                        &mut workspace,
                        offset,
                        frame_generation,
                        &mut memory,
                        &mut solver_report,
                        &mut certified_linear_subproblems,
                        &mut total_inner_iterations,
                        &mut allocation_audit,
                    )?;
                    rank_frozen = true;
                    result
                }
            }
        } else {
            #[derive(Clone, Copy)]
            enum FrozenAttempt {
                Certified {
                    rank: usize,
                    reuse: LinearSessionReuseReport,
                    route: FeSolverKind,
                    fully_certified: bool,
                },
                Fallback(FrozenLinearFallbackKind),
            }
            let frozen_attempt = match session.solve_ordinary_ppml() {
                Ok(output) if output.certified() => {
                    certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
                    accumulate_inner_iterations(
                        &mut total_inner_iterations,
                        output.response_absorption_report(),
                    )?;
                    solver_report
                        .as_mut()
                        .ok_or_else(|| internal("PPML MAP solve lacks solver diagnostics"))?
                        .observe(map_solver_observation(
                            output.response_absorption_report(),
                            PpmlSolverFrameKind::Ordinary,
                        ))?;
                    fill_candidate_eta_from_frozen_output(
                        &output,
                        offset,
                        workspace.candidate_eta_mut(),
                    )?;
                    fill_candidate_coefficients_from_frozen_output(
                        &output,
                        workspace.candidate_coefficients_mut(),
                    )?;
                    FrozenAttempt::Certified {
                        rank: output.basis().kept_generated_columns().len(),
                        reuse: output.reuse_report(),
                        route: output.response_absorption_report().used_solver,
                        fully_certified: output.certified(),
                    }
                }
                Ok(_) => FrozenAttempt::Fallback(FrozenLinearFallbackKind::Bread),
                Err(error) => match classify_frozen_linear_fallback(&error) {
                    Some(kind) => FrozenAttempt::Fallback(kind),
                    None => return Err(error),
                },
            };
            rank_frozen = true;
            if let FrozenAttempt::Certified {
                rank: frozen_rank,
                reuse: frozen_reuse,
                route: frozen_route,
                fully_certified: frozen_certified,
            } = frozen_attempt
            {
                if !allocation_audit.needs_equivalence() {
                    (frozen_rank, frozen_reuse, frozen_route)
                } else {
                    let frozen_objective = workspace
                        .evaluate_candidate_objective(&objective_inputs, config.ppml.bounds)?;
                    record_linear_phase_timings(&mut memory, frozen_reuse)?;
                    // This canonical same-frame solve is deliberately after the
                    // end allocator snapshot. It may allocate and therefore can
                    // validate warm/fresh science without contaminating the
                    // warmed steady-state window.
                    let canonical = session.solve_fresh()?;
                    accumulate_inner_iterations(
                        &mut total_inner_iterations,
                        canonical.frame().absorption_report(),
                    )?;
                    let solver = solver_report
                        .as_mut()
                        .ok_or_else(|| internal("PPML MAP equivalence solve lacks diagnostics"))?;
                    if initial_checkpoint_lsmr {
                        record_checkpoint_solver_observation(
                            solver,
                            canonical.frame().absorption_report(),
                            false,
                        )?;
                    } else {
                        solver.observe(map_solver_observation(
                            canonical.frame().absorption_report(),
                            PpmlSolverFrameKind::Ordinary,
                        ))?;
                    }
                    let canonical_route = canonical.frame().absorption_report().used_solver;
                    let mut equivalent = frozen_certified
                        && frozen_route == FeSolverKind::Map
                        && (canonical_route == frozen_route
                            || (initial_checkpoint_lsmr && canonical_route == FeSolverKind::Lsmr))
                        && fresh_warm_linear_equivalent(
                            &canonical,
                            offset,
                            workspace.candidate_eta(),
                            workspace.candidate_coefficients(),
                            frozen_rank,
                            config.ppml,
                            inner_tolerance,
                        )?;
                    fill_candidate_eta_from_linear_output(
                        &canonical,
                        offset,
                        workspace.candidate_eta_mut(),
                    )?;
                    fill_candidate_coefficients_from_linear_output(
                        &canonical,
                        workspace.candidate_coefficients_mut(),
                    )?;
                    let canonical_objective = workspace
                        .evaluate_candidate_objective(&objective_inputs, config.ppml.bounds)?;
                    equivalent &= relative_scalar_difference(
                        frozen_objective.deviance,
                        canonical_objective.deviance,
                    ) <= fresh_warm_tolerance(config.ppml, inner_tolerance)
                        && relative_scalar_difference(
                            frozen_objective.log_likelihood,
                            canonical_objective.log_likelihood,
                        ) <= fresh_warm_tolerance(config.ppml, inner_tolerance);
                    let canonical_rank = canonical.frame().finite_rank();
                    let canonical_reuse = canonical.reuse_report();
                    let observation = allocation_audit.complete_equivalence(equivalent)?;
                    memory.observe_steady_state_allocation(observation)?;
                    (canonical_rank, canonical_reuse, canonical_route)
                }
            } else if let FrozenAttempt::Fallback(kind) = frozen_attempt {
                retry::canonical_retry(
                    retry::CanonicalRetryReason::Frozen(kind),
                    &mut direction,
                    &mut session,
                    &mut workspace,
                    offset,
                    frame_generation,
                    &mut memory,
                    &mut solver_report,
                    &mut certified_linear_subproblems,
                    &mut total_inner_iterations,
                    &mut allocation_audit,
                )?
            } else {
                unreachable!("frozen attempt is exhaustively classified")
            }
        };
        record_linear_phase_timings(&mut memory, reuse)?;
        begin_phase_allocation(
            config.phase_allocation_probe,
            PpmlPhase::ObjectiveLineSearch,
        );
        let line_search_clock = PpmlPhaseClock::start();
        let previous_objective = objective;
        let accepted = {
            let mut poll_line_search = || interrupt(PpmlInterruptPoint::IrlsLineSearchTrial);
            let attempt = if profiled_step.is_some() {
                let profile = direction.profile_mut().expect("prepared profile step");
                if config.linear.execution.mode == PreparedOlsExecutionMode::Low
                    || session.has_implicit_one_fe()
                {
                    let columns = session.one_fe_original_columns(profile.active_columns())?;
                    let data = ProfiledRawColumns::new(prepared, columns)?;
                    profile.accept(
                        &data,
                        session.fe_design(),
                        &objective_inputs,
                        offset,
                        &mut workspace,
                        objective,
                        config.ppml,
                        &mut poll_line_search,
                    )
                } else {
                    let (values, positions) =
                        session.one_fe_representatives(profile.active_columns())?;
                    profile.accept(
                        ProfiledColumns::selected(values, positions)?,
                        session.fe_design(),
                        &objective_inputs,
                        offset,
                        &mut workspace,
                        objective,
                        config.ppml,
                        &mut poll_line_search,
                    )
                }
            } else if let Some(curve) = profiled_curve.as_mut() {
                curve.accept(
                    session.fe_design(),
                    &objective_inputs,
                    &mut workspace,
                    objective,
                    config.ppml,
                    &mut poll_line_search,
                )
            } else {
                workspace.accept_candidate_interruptible(
                    &objective_inputs,
                    objective,
                    config.ppml,
                    &mut poll_line_search,
                )
            };
            match attempt {
                Ok(accepted) => accepted,
                Err(error)
                    if matches!(purpose, PreparedPpmlIrlsPurpose::MuExploration { .. })
                        && error.code == ErrorCode::NonConvergence
                        && accepted_iterations > 0 =>
                {
                    memory
                        .record_phase(PpmlPhase::ObjectiveLineSearch, line_search_clock.elapsed());
                    record_phase_allocation(
                        &mut memory,
                        config.phase_allocation_probe,
                        PpmlPhase::ObjectiveLineSearch,
                    );
                    exploration_terminal = PreparedPpmlMuTerminal::LineSearchStalled;
                    break;
                }
                Err(error) => return Err(error),
            }
        };
        if let Some((joint, _)) = direction.lsmr_mut() {
            // The analytic FE shift changes the accepted increment. Discard
            // the LSMR history witness before solving under the new weights.
            if profiled_curve.is_some() {
                joint.reset_warm_start();
            } else {
                joint.accept_step(accepted.step);
            }
        }
        accepted_iterations = accepted_iterations.saturating_add(1);
        let increase = (accepted.objective.deviance - previous_objective.deviance).max(0.0)
            / previous_objective.deviance.abs().max(1.0);
        maximum_deviance_increase_relative = maximum_deviance_increase_relative.max(increase);
        total_step_halvings = total_step_halvings
            .checked_add(accepted.step_halvings)
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "prepared PPML step-halving count overflowed",
                )
            })?;
        let relative_change = relative_deviance_change(objective, accepted.objective);
        objective = accepted.objective;
        final_score = prepared_score_certificate(
            prepared,
            session.fe_design(),
            outcome,
            workspace.mean(),
            prior_weights,
            config.ppml.score_tolerance,
            &mut score_workspace,
        )?;
        memory.record_phase(PpmlPhase::ObjectiveLineSearch, line_search_clock.elapsed());
        record_phase_allocation(
            &mut memory,
            config.phase_allocation_probe,
            PpmlPhase::ObjectiveLineSearch,
        );
        if let PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance } = purpose {
            let minimum_positive = outcome
                .iter()
                .zip(workspace.mean())
                .filter_map(|(y, mean)| (*y > 0.0).then_some(*mean))
                .fold(f64::INFINITY, f64::min);
            let reached = outcome
                .iter()
                .zip(workspace.mean())
                .any(|(y, mean)| *y == 0.0 && *mean <= mu_tolerance * minimum_positive);
            if reached {
                exploration_terminal = PreparedPpmlMuTerminal::CandidateThresholdReached;
                break;
            }
        }
        let iteration_converged = relative_change <= config.ppml.tolerance && final_score.passed;
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=irls_convergence|iteration={iteration}|deviance_change={relative_change:.17e}|score={:.17e}|passed={}|step={:.17e}",
                final_score.relative_infinity_norm, u8::from(iteration_converged), accepted.step,
            ));
        }
        strict_control.observe_accepted_iteration(iteration_converged);
        if retained_iterations.len() < retained_limit {
            retained_iterations.push(PreparedPpmlIterationReport {
                iteration,
                deviance: objective.deviance,
                log_likelihood: objective.log_likelihood,
                relative_deviance_change: relative_change,
                step: accepted.step,
                step_halvings: accepted.step_halvings,
                inner_tolerance,
                finite_rank,
                score: final_score.clone(),
                linear_reuse: reuse,
                joint_lsmr: joint_lsmr_frame,
            });
        }
        let audit_can_start = allocation_audit.can_start_after(
            iteration,
            config.ppml.max_iterations,
            reuse,
            linear_route,
        );
        let audit_defers_strict = allocation_audit.qualification_pending();
        if strict_control.may_enter(audit_defers_strict) {
            let outcome = controller::StrictCycle {
                candidate: controller::CandidatePhase {
                    prepared,
                    workspace: &mut workspace,
                    inputs: &objective_inputs,
                    outcome,
                    prior_weights,
                    offset,
                    config,
                    finite_rank,
                    objective: &mut objective,
                    total_inner_iterations: &mut total_inner_iterations,
                    total_step_halvings: &mut total_step_halvings,
                    score_workspace: &mut score_workspace,
                    final_score: &mut final_score,
                },
                session: &mut session,
                direction: &mut direction,
                curve: &mut profiled_curve,
                control: &mut strict_control,
                memory: &mut memory,
                solver: &mut solver_report,
                frame_generation: &mut frame_generation,
                previous_inner_tolerance: &mut previous_inner_tolerance,
                interrupt: &mut *interrupt,
                iteration,
                solver_admission,
                solver_memory_budget,
            }
            .run()?;
            match outcome {
                controller::StrictCycleOutcome::JointAccepted => break,
                controller::StrictCycleOutcome::CheckpointAccepted => {
                    exploration_terminal = PreparedPpmlMuTerminal::Converged;
                    break;
                }
                controller::StrictCycleOutcome::Resume => {}
            }
        }
        begin_phase_allocation(
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
        let working_refresh_clock = PpmlPhaseClock::start();
        workspace.refresh_prepared_working_state(&objective_inputs, config.ppml.bounds)?;
        memory.record_phase(
            PpmlPhase::WeightResponseRefresh,
            working_refresh_clock.elapsed(),
        );
        record_phase_allocation(
            &mut memory,
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
        if allocation_audit.is_measuring() {
            let elapsed = audited_iteration_clock.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "PPML allocation audit iteration lacks its timing boundary",
                )
            })?;
            allocation_audit.finish_iteration(
                &session,
                &workspace,
                reuse,
                linear_route,
                elapsed.elapsed(),
                direction.profile(),
                profiled_curve.as_ref(),
            )?;
        } else if audit_can_start {
            allocation_audit.start_after(
                &session,
                &workspace,
                direction.profile(),
                profiled_curve.as_ref(),
            )?;
        }
        previous_outer_progress = Some(PpmlInnerToleranceProgress::new(
            relative_deviance_change(previous_objective, objective),
            final_score.relative_infinity_norm,
        ));
    }

    if matches!(purpose, PreparedPpmlIrlsPurpose::MuExploration { .. }) {
        let (_coefficient_values, eta, mean, _score_residuals) = workspace.into_final_buffers();
        let mean_reconstruction_relative = mean
            .iter()
            .zip(&eta)
            .map(|(left, eta)| relative_scalar_difference(*left, eta.exp()))
            .fold(0.0_f64, f64::max);
        let certificate = PreparedPpmlMuExplorationCertificate {
            accepted_iterations,
            certified_linear_subproblems,
            maximum_deviance_increase_relative,
            mean_reconstruction_relative,
            terminal: exploration_terminal,
            passed: accepted_iterations > 0
                && certified_linear_subproblems >= accepted_iterations
                && maximum_deviance_increase_relative
                    <= config.ppml.objective_slack + 64.0 * f64::EPSILON
                && mean_reconstruction_relative <= 64.0 * f64::EPSILON
                && eta.iter().all(|value| value.is_finite())
                && mean.iter().all(|value| value.is_finite() && *value > 0.0)
                && objective.deviance.is_finite()
                && objective.log_likelihood.is_finite(),
        };
        if !certificate.passed {
            return Err(numerical_nonconvergence(
                "prepared Mu exploratory trajectory did not certify",
            ));
        }
        return Ok(PreparedPpmlIrlsRun::MuExploration(Box::new(
            PreparedPpmlMuExploration {
                binding,
                mean,
                certificate,
            },
        )));
    }

    let accepted_checkpoint = strict_control.accepted_checkpoint().ok_or_else(|| {
        let strict_profile_rejections = strict_control.rejected_cycles();
        FeregError::new(ErrorCode::NonConvergence, format!(
            "prepared PPML failed to converge after {} iterations; deviance={:.16e}, score={:.3e}, strict_profile_rejections={strict_profile_rejections}",
            config.ppml.max_iterations, objective.deviance, final_score.relative_infinity_norm,
        ))
    })?;
    let final_output = session.output()?;
    if accepted_checkpoint.needs_observation() {
        record_checkpoint_solver_observation(
            solver_report
                .as_mut()
                .ok_or_else(|| internal("PPML final Hessian lacks solver diagnostics"))?,
            final_output.frame().absorption_report(),
            false,
        )?;
    }
    let coefficients = final_coefficients(&final_output, workspace.coefficients())?;
    let retained_memory = final_output.retained_within_memory();
    memory.observe_scratch(retained_memory.scratch_bytes);
    memory.observe_linear_route(final_output.frame().absorption_report().used_solver);
    memory.observe_diagnostic_records(retained_iterations.len())?;
    memory.certify_workspace(&workspace)?;
    workspace.refresh_score_residuals(outcome)?;
    let (_coefficient_values, eta, mean, score_residuals) = workspace.into_final_buffers();
    if bias.is_some() {
        validate_final_hessian_frame(session.bias_weights(), prior_weights, &mean)?;
    }
    let bias = match bias {
        Some((request, units)) => Some(super::bias::compute(
            &mut session,
            request,
            super::bias::AcceptedBiasFrame {
                units,
                y: outcome,
                mu: &mean,
                eta: &eta,
                offset,
            },
            config.ppml.tolerance,
            interrupt,
        )?),
        None => None,
    };
    let linear = session
        .into_current_products(&mut || interrupt(super::PpmlInterruptPoint::InferenceSubset))?;
    validate_final_hessian_frame(linear.weights(), prior_weights, &mean)?;
    let final_generations = linear.generations();
    if final_generations.sample != model_generation
        || final_generations.topology != model_generation
        || final_generations.response != frame_generation
        || final_generations.weights != frame_generation
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML final linear products have stale frame generations",
        ));
    }
    Ok(PreparedPpmlIrlsRun::Publication(Box::new(
        PreparedPpmlIrlsFit {
            binding,
            bias,
            linear,
            coefficients,
            eta,
            mean,
            score_residuals,
            objective,
            diagnostics: PreparedPpmlIrlsDiagnostics {
                converged: true,
                two_consecutive_convergence: true,
                rank_frozen,
                iterations: completed_iterations,
                total_inner_iterations,
                retained_iterations,
                total_step_halvings,
                strict_profile_rejections: strict_control.rejected_cycles(),
                final_score,
                solver: solver_report.ok_or_else(|| {
                    internal("prepared PPML publication lacks solver diagnostics")
                })?,
                memory,
            },
        },
    )))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PublicSolverMemoryAdmission {
    incremental_bytes: u128,
    owned_fe_design_shortfall_bytes: u128,
    cmg_batch_capacity: usize,
    checkpoint_workspace_bytes: u128,
}

fn record_cmg_heap_forecast(
    memory: &mut PpmlMemoryTelemetry,
    cmg: &ReusableCategoricalFwlCmg,
    admission: PublicSolverMemoryAdmission,
) -> Result<()> {
    let peak = cmg
        .expected_heap_peak(admission.checkpoint_workspace_bytes)?
        .with_retained(admission.owned_fe_design_shortfall_bytes)?;
    memory.expected_solver_workspace_peak = Some(
        memory
            .expected_solver_workspace_peak
            .unwrap_or(crate::memory::SolverWorkspacePeak::ZERO)
            .max(peak),
    );
    Ok(())
}

fn public_solver_memory_admission<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    memory_plan: PpmlMemoryPlan,
    config: &PreparedPpmlIrlsConfig,
    rhs: usize,
    purpose: PreparedPpmlIrlsPurpose,
) -> Result<PublicSolverMemoryAdmission> {
    let solver = config.solver_plan.selected;
    if matches!(
        solver,
        super::PpmlSolverFamily::Auto | super::PpmlSolverFamily::Map
    ) || matches!(purpose, PreparedPpmlIrlsPurpose::MuExploration { .. })
    {
        return Ok(PublicSolverMemoryAdmission::default());
    }
    let design = prepared.fit_borrowed_cmg_design()?;
    let dimensions = memory_plan.dimensions;
    let admitted_rhs = dimensions
        .finite_columns
        .checked_add(1)
        .ok_or_else(|| allocation("admitted PPML FE solver RHS width overflows"))?;
    if design.nobs() != dimensions.observations
        || design.effects().len() != dimensions.fe_dimensions
        || rhs != admitted_rhs
        || config.linear.execution.threads.resolved() != dimensions.threads
    {
        return Err(internal(
            "public PPML solver preflight does not match its admitted immutable dimensions",
        ));
    }
    let mut levels = Vec::new();
    levels
        .try_reserve_exact(design.effects().len())
        .map_err(|_| allocation("unable to reserve PPML FE solver preflight levels"))?;
    levels.extend(design.effects().iter().map(|effect| effect.n_levels()));
    let total_levels = levels.iter().try_fold(0_usize, |total, levels| {
        total
            .checked_add(*levels)
            .ok_or_else(|| allocation("PPML FE solver level count overflows"))
    })?;
    if total_levels != dimensions.fe_total_levels
        || dimensions.fe_total_channels != dimensions.fe_dimensions
        || dimensions.fe_max_channels != usize::from(!levels.is_empty())
        || levels.iter().copied().max().unwrap_or(0) != dimensions.fe_max_levels
    {
        return Err(internal(
            "public PPML solver topology does not match its admitted FE dimensions",
        ));
    }
    let owned_fe_design_peak =
        conservative_owned_intercept_fe_design_peak_bytes(design.nobs(), &levels)?;
    let owned_fe_design_shortfall_bytes = owned_fe_design_peak
        .saturating_sub(memory_plan.selected_inventory().fe_solver_topology_bytes);
    let mut cmg_batch_capacity = 0;
    let route_workspace = match solver {
        super::PpmlSolverFamily::Lsmr => {
            let active_columns = rhs.saturating_sub(1);
            estimate_joint_lsmr_workspace_bytes(
                dimensions.observations,
                active_columns,
                &levels,
                config.linear.execution.threads.resolved(),
                joint_lsmr_policy(levels.len()),
            )?
        }
        super::PpmlSolverFamily::Cmg => {
            let fused_workspace = estimate_fused_cmg_workspace_bytes(
                dimensions.observations,
                rhs.saturating_sub(1),
                config.linear.execution.threads.resolved(),
            )?;
            let profile = CmgTopologyProfile::analyze(&design)?;
            let fused_workspace = fused_workspace
                .checked_add(crate::fe::CmgControlSeed::estimated_bytes(
                    profile.vertices(),
                    rhs.saturating_sub(1),
                )?)
                .ok_or_else(|| allocation("prepared PPML CMG seed workspace overflows"))?;
            if profile.nuisance_columns() > CMG_MAX_NUISANCE_COLUMNS {
                return Err(FeregError::new(
                    ErrorCode::Unsupported,
                    format!(
                        "solver(cmg) supports at most {CMG_MAX_NUISANCE_COLUMNS} non-base nuisance columns; found {}",
                        profile.nuisance_columns()
                    ),
                ));
            }
            cmg_batch_capacity =
                fused_cmg_batch_size(config.memory.poolsize, rhs, profile.nuisance_columns());
            let cmg_config = CmgConfig {
                map: config.ppml.absorb.map,
                threads: config.linear.execution.threads,
                batch_size: cmg_batch_capacity,
                retain_coefficients: false,
                requested_solver: FeSolverKind::Cmg,
                components: crate::fe::explicit_cmg_components(FeSolverKind::Cmg),
                solver_memory_budget: None,
            };
            let topology_prepare = PreparedCmgTopology::conservative_prepare_peak_bytes(&design)?;
            let retained_frame =
                PreparedCmgTopology::conservative_retained_frame_peak_bytes(&design, cmg_config)?;
            let steady_peak = fused_workspace
                .checked_add(retained_frame)
                .ok_or_else(|| allocation("prepared PPML CMG steady-state peak overflows"))?;
            let complete_peak = topology_prepare.max(steady_peak);
            #[cfg(test)]
            {
                let topology_only = fused_workspace
                    .checked_add(topology_prepare)
                    .and_then(|value| value.checked_add(owned_fe_design_shortfall_bytes))
                    .ok_or_else(|| allocation("prepared PPML CMG legacy early peak overflows"))?;
                let complete_incremental = complete_peak
                    .checked_add(owned_fe_design_shortfall_bytes)
                    .ok_or_else(|| allocation("prepared PPML CMG complete early peak overflows"))?;
                CMG_EARLY_MEMORY_PEAKS.set(Some((topology_only, complete_incremental)));
            }
            complete_peak
        }
        super::PpmlSolverFamily::Auto | super::PpmlSolverFamily::Map => 0,
    };
    // Initial basis freezing, strict convergence, and final-Hessian
    // synchronization execute serially with the same maximum full width.
    // The existing full-width checkpoint peak therefore admits the initial
    // accelerator exactly once; summing three mutually exclusive workspaces
    // would overstate the fit-lifetime peak.
    let checkpoint_workspace =
        if use_checkpoint_lsmr_accelerator(dimensions.observations, levels.len()) {
            estimate_checkpoint_lsmr_workspace_bytes(
                dimensions.observations,
                rhs,
                &levels,
                config.linear.execution.threads.resolved(),
            )?
        } else {
            0
        };
    Ok(PublicSolverMemoryAdmission {
        incremental_bytes: route_workspace
            .checked_add(checkpoint_workspace)
            .ok_or_else(|| allocation("public PPML checkpoint memory admission overflows"))?
            .checked_add(owned_fe_design_shortfall_bytes)
            .ok_or_else(|| allocation("public PPML solver admission memory overflows"))?,
        owned_fe_design_shortfall_bytes,
        cmg_batch_capacity,
        checkpoint_workspace_bytes: checkpoint_workspace,
    })
}

fn conservative_owned_intercept_fe_design_peak_bytes(
    rows: usize,
    levels: &[usize],
) -> Result<u128> {
    let n = rows as u128;
    let effects = levels.len() as u128;
    let total_levels = levels.iter().try_fold(0_u128, |total, levels| {
        total
            .checked_add(*levels as u128)
            .ok_or_else(|| allocation("owned PPML FE level count overflows"))
    })?;
    let maximum_levels = levels.iter().copied().max().unwrap_or(0) as u128;
    let word = std::mem::size_of::<usize>() as u128;
    let retained = effects
        .checked_mul(std::mem::size_of::<crate::fe::EffectTerm>() as u128)
        .and_then(|value| {
            value.checked_add(
                n.checked_mul(effects)?
                    .checked_mul(std::mem::size_of::<u32>() as u128)?,
            )
        })
        .and_then(|value| value.checked_add(n.checked_mul(effects)?.checked_mul(word)?))
        .and_then(|value| value.checked_add(total_levels.checked_add(effects)?.checked_mul(word)?))
        .ok_or_else(|| allocation("owned PPML FE topology estimate overflows"))?;
    // LevelIndex construction holds one count and one cursor for every level
    // beside the retained codes, offsets, and memberships of the current and
    // previously completed effects.
    retained
        .checked_add(
            maximum_levels
                .checked_mul(2)
                .and_then(|value| value.checked_mul(word))
                .ok_or_else(|| allocation("owned PPML FE construction scratch overflows"))?,
        )
        .ok_or_else(|| allocation("owned PPML FE construction peak overflows"))
}

fn map_solver_observation(
    report: &SolveReport,
    frame: PpmlSolverFrameKind,
) -> PpmlSolverObservation {
    PpmlSolverObservation {
        engine: PpmlSolverEngine::Map,
        frame,
        retries: usize::from(report.fallback_used),
        iterations: report.iterations,
        setup_seconds: report.setup_seconds,
        solve_seconds: report.solve_seconds,
        certification_seconds: report.certification_seconds,
        absolute_infinity_norm: Some(report.certificate.absolute_infinity_norm),
        relative_infinity_norm: Some(report.certificate.relative_infinity_norm),
        relative_two_norm: Some(report.certificate.relative_two_norm),
        normal_equation: Some(report.normal_equation_residual),
        projection_absolute_infinity_norm: None,
        backward_error: None,
    }
}

fn checkpoint_solver_observation(report: &SolveReport) -> Result<PpmlSolverObservation> {
    let engine = match report.used_solver {
        FeSolverKind::Map => PpmlSolverEngine::Map,
        FeSolverKind::Lsmr => PpmlSolverEngine::LsmrFused,
        _ => {
            return Err(internal(
                "PPML checkpoint projection used an unsupported solver engine",
            ))
        }
    };
    Ok(PpmlSolverObservation {
        engine,
        frame: PpmlSolverFrameKind::Checkpoint,
        retries: report.checkpoint_retries + usize::from(report.fallback_used),
        iterations: report.iterations,
        setup_seconds: report.setup_seconds,
        solve_seconds: report.solve_seconds,
        certification_seconds: report.certification_seconds,
        absolute_infinity_norm: Some(report.certificate.absolute_infinity_norm),
        relative_infinity_norm: Some(report.certificate.relative_infinity_norm),
        relative_two_norm: Some(report.certificate.relative_two_norm),
        normal_equation: Some(report.normal_equation_residual),
        projection_absolute_infinity_norm: None,
        backward_error: None,
    })
}

fn record_checkpoint_solver_observation(
    solver: &mut PpmlSolverReport,
    report: &SolveReport,
    profile_refinement: bool,
) -> Result<()> {
    let mut observation = checkpoint_solver_observation(report)?;
    observation.retries = observation
        .retries
        .checked_add(usize::from(profile_refinement))
        .ok_or_else(|| internal("PPML checkpoint refinement count overflowed"))?;
    if report.requested_solver == FeSolverKind::Lsmr
        && report.used_solver == FeSolverKind::Map
        && report.fallback_used
    {
        solver.record_checkpoint_lsmr_fallback(observation)
    } else {
        solver.observe(observation)
    }
}

fn joint_solver_observation(
    report: JointLsmrReport,
    engine: PpmlSolverEngine,
) -> PpmlSolverObservation {
    debug_assert!(matches!(
        engine,
        PpmlSolverEngine::LsmrFused | PpmlSolverEngine::LsmrJoint
    ));
    PpmlSolverObservation {
        engine,
        frame: PpmlSolverFrameKind::Ordinary,
        retries: report.retries,
        iterations: report.iterations,
        setup_seconds: report.setup_seconds,
        solve_seconds: report.solve_seconds,
        certification_seconds: report.certification_seconds,
        absolute_infinity_norm: Some(
            report
                .certificate
                .finite_absolute_infinity_norm
                .max(report.certificate.fixed_effect_absolute_infinity_norm),
        ),
        relative_infinity_norm: Some(report.certificate.relative_infinity_norm),
        relative_two_norm: Some(report.certificate.relative_two_norm),
        normal_equation: (engine == PpmlSolverEngine::LsmrJoint)
            .then_some(report.recurrence_residual),
        projection_absolute_infinity_norm: None,
        backward_error: None,
    }
}

fn cmg_solver_observation(report: FusedFwlCmgReport) -> PpmlSolverObservation {
    PpmlSolverObservation {
        engine: PpmlSolverEngine::CmgPrepared,
        frame: PpmlSolverFrameKind::Ordinary,
        retries: report.retried_right_hand_sides,
        iterations: report.iterations,
        setup_seconds: report.setup_seconds,
        solve_seconds: report.solve_seconds,
        certification_seconds: report.finish_seconds,
        absolute_infinity_norm: Some(
            report
                .certificate
                .finite_absolute_infinity_norm
                .max(report.certificate.fixed_effect_absolute_infinity_norm),
        ),
        relative_infinity_norm: Some(report.certificate.relative_infinity_norm),
        relative_two_norm: Some(report.certificate.relative_two_norm),
        normal_equation: None,
        projection_absolute_infinity_norm: Some(report.projection_absolute_infinity_norm),
        backward_error: Some(report.backward_error),
    }
}

fn cmg_failure_observation(report: FusedFwlCmgFailureReport) -> PpmlSolverObservation {
    PpmlSolverObservation {
        engine: PpmlSolverEngine::CmgPrepared,
        frame: PpmlSolverFrameKind::Ordinary,
        retries: report.retries,
        iterations: report.iterations,
        setup_seconds: report.setup_seconds,
        solve_seconds: report.solve_seconds,
        certification_seconds: report.certification_seconds,
        absolute_infinity_norm: report.absolute_infinity_norm,
        relative_infinity_norm: report.relative_infinity_norm,
        relative_two_norm: report.relative_two_norm,
        normal_equation: None,
        projection_absolute_infinity_norm: report.projection_absolute_infinity_norm,
        backward_error: report.backward_error,
    }
}

fn accumulate_inner_iterations(total: &mut usize, report: &SolveReport) -> Result<()> {
    *total = total.checked_add(report.iterations.max(1)).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Internal,
            "prepared PPML inner-iteration count overflowed",
        )
    })?;
    Ok(())
}

fn validate_inputs<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
) -> Result<()> {
    let rows = prepared.fit_sample().n_active();
    if rows == 0 || outcome.len() != rows || prior_weights.len() != rows || offset.len() != rows {
        return Err(invalid_argument(
            "prepared PPML inputs are empty or not aligned on the prepared sample",
        ));
    }
    if outcome
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(invalid_data(
            "prepared PPML outcomes must be finite and nonnegative",
        ));
    }
    if !outcome.iter().any(|value| *value > 0.0) {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "prepared PPML sample has no positive outcome",
        ));
    }
    if prior_weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
        || offset.iter().any(|value| !value.is_finite())
    {
        return Err(invalid_data(
            "prepared PPML weights must be finite and positive and offsets finite",
        ));
    }
    if !prepared.fit_structural_intercept() {
        return Err(invalid_argument(
            "prepared PPML requires its structural constant direction",
        ));
    }
    config.ppml.bounds.validate()?;
    let valid = finite_unit(config.ppml.fixed_effect_collinearity_tolerance)
        && finite_unit(config.ppml.tolerance)
        && finite_unit(config.ppml.score_tolerance)
        && finite_unit(config.ppml.inner_tolerance_initial)
        && finite_unit(config.ppml.inner_tolerance_final)
        && config.ppml.inner_tolerance_initial >= config.ppml.inner_tolerance_final
        && config.ppml.max_iterations > 0
        && config.ppml.minimum_step.is_finite()
        && config.ppml.minimum_step > 0.0
        && config.ppml.minimum_step <= 1.0
        && config.ppml.objective_slack.is_finite()
        && config.ppml.objective_slack >= 0.0;
    if !valid {
        return Err(invalid_argument(
            "prepared PPML numerical configuration is invalid",
        ));
    }
    validate_solver_admission(config)?;
    Ok(())
}

fn validate_memory_dimensions<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    dimensions: PpmlMemoryDimensions,
    config: PreparedPpmlIrlsConfig,
) -> Result<()> {
    let expected_threads = config.linear.execution.threads.resolved();
    if dimensions.observations != prepared.fit_sample().n_active()
        || dimensions.finite_columns != prepared.fit_model().columns.len()
        || dimensions.coefficient_slots != prepared.fit_model().coefficients.len()
        || dimensions.threads != expected_threads
    {
        return Err(invalid_argument(
            "prepared PPML memory dimensions do not match the final prepared generation",
        ));
    }
    Ok(())
}

fn next_frame_generation(current: u64) -> Result<u64> {
    current.checked_add(1).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Internal,
            "prepared PPML weighted-linear generation overflowed",
        )
    })
}

fn refresh_linear_frame<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &mut CertifiedLinearSession<'_, P>,
    workspace: &PpmlIterationWorkspace,
    generation: u64,
) -> Result<()> {
    session.refresh_response(workspace.working_response(), generation)?;
    session.refresh_weights(
        BorrowedMapWeights::Provided(workspace.hessian_weights()),
        ppml_bread_weight_scale(workspace.hessian_weights())?,
        generation,
    )?;
    #[cfg(test)]
    CANONICAL_FRAME_REFRESH_COUNT.set(CANONICAL_FRAME_REFRESH_COUNT.get().saturating_add(1));
    Ok(())
}

fn refresh_final_linear_frame<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &mut CertifiedLinearSession<'_, P>,
    workspace: &PpmlIterationWorkspace,
    generation: u64,
) -> Result<bool> {
    let reused_identical_weights = session.refresh_frame(
        workspace.working_response(),
        BorrowedMapWeights::Provided(workspace.hessian_weights()),
        ppml_bread_weight_scale(workspace.hessian_weights())?,
        generation,
    )?;
    #[cfg(test)]
    CANONICAL_FRAME_REFRESH_COUNT.set(CANONICAL_FRAME_REFRESH_COUNT.get().saturating_add(1));
    Ok(reused_identical_weights)
}

/// Rebuild the reusable linear products on the accepted PPML Hessian without
/// advancing the Newton map. Projecting the accepted linear predictor itself
/// leaves the accepted beta/eta state fixed while strictly recertifying the
/// within design and bread under the exact accepted `q * mu` weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinalHessianSynchronization {
    Certified,
    ResumeWithCandidate,
    ResumeAcceptedState,
}

/// Joint Newton witness followed by canonical predictor finalization. This
/// remains necessary when canonical Newton reconstruction moves the point.
#[allow(clippy::too_many_arguments)]
fn try_joint_strict_final_frame<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &mut CertifiedLinearSession<'_, P>,
    workspace: &mut PpmlIterationWorkspace,
    objective_inputs: &PreparedPoissonInputs<'_>,
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    checkpoint_tolerance: f64,
    frame_generation: &mut u64,
    total_inner_iterations: &mut usize,
    expected_rank: usize,
    accepted_objective: PoissonObjective,
    solver_report: &mut PpmlSolverReport,
    mut categorical: Option<(&mut ReusableCategoricalJointLsmr, PpmlSolverEngine)>,
) -> Result<bool> {
    #[cfg(test)]
    if JOINT_FINAL_TEST_MODE.get() == 1
        || categorical.is_some()
            && (STRICT_RESUME_INJECTION.get() != 0
                || STRICT_BASIS_CHANGE_INJECTION.get() != 0
                || REPEATED_STRICT_RESUME_INJECTION.get() != 0)
    {
        // Existing injections explicitly exercise canonical checkpoint/resume.
        // Shared witness refusal and rollback have their own modes below.
        return Ok(false);
    }
    #[cfg(test)]
    JOINT_FINAL_TEST_COUNTS.set((
        JOINT_FINAL_TEST_COUNTS.get().0 + 1,
        JOINT_FINAL_TEST_COUNTS.get().1,
    ));
    workspace.refresh_prepared_working_state(objective_inputs, config.ppml.bounds)?;
    if categorical.is_none() {
        *frame_generation = next_frame_generation(*frame_generation)?;
        refresh_linear_frame(session, workspace, *frame_generation)?;
    }
    session.set_config(ppml_linear_config(config, checkpoint_tolerance)?)?;
    let rank = if let Some((joint, engine)) = categorical.as_mut() {
        let result = joint.solve(
            session.fe_design(),
            objective_inputs.outcome(),
            workspace.mean(),
            workspace.hessian_weights(),
            checkpoint_tolerance,
            config.linear.absorb.lsmr.max_iterations,
        );
        #[cfg(test)]
        let result = if JOINT_FINAL_TEST_MODE.get() == 4 {
            JOINT_FINAL_TEST_MODE.set(0);
            Err(FeregError::new(
                ErrorCode::NonConvergence,
                "injected strict LSMR witness refusal",
            ))
        } else {
            result
        };
        let report = match result {
            Ok(report) => report,
            Err(error) if error.code == ErrorCode::NonConvergence => {
                joint.reset_warm_start();
                if crate::diagnostics::solver_trace_enabled() {
                    crate::diagnostics::emit_solver_trace(
                        "FEREG_SOLVER_TRACE|phase=joint_strict_direction|refusal=lsmr_nonconvergence",
                    );
                }
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        *total_inner_iterations = total_inner_iterations
            .checked_add(report.iterations.max(1))
            .ok_or_else(|| internal("strict LSMR iteration count overflowed"))?;
        let mut observation = joint_solver_observation(report, *engine);
        observation.frame = PpmlSolverFrameKind::Checkpoint;
        solver_report.observe(observation)?;
        #[cfg(test)]
        if *engine == PpmlSolverEngine::LsmrFused {
            FUSED_STRICT_WITNESS_COUNT.set(FUSED_STRICT_WITNESS_COUNT.get() + 1);
        }
        joint.fill_candidate(workspace)?;
        joint.active_rank()
    } else {
        let direction = match session.solve_ordinary_ppml() {
            Ok(output) => output,
            Err(error) if classify_frozen_linear_fallback(&error).is_some() => return Ok(false),
            Err(error) => return Err(error),
        };
        if !matches!(direction, PpmlOrdinaryLinearOutput::Joint(_)) || !direction.certified() {
            return Ok(false);
        }
        accumulate_inner_iterations(
            total_inner_iterations,
            direction.response_absorption_report(),
        )?;
        record_checkpoint_solver_observation(
            solver_report,
            direction.response_absorption_report(),
            false,
        )?;
        fill_candidate_eta_from_frozen_output(&direction, offset, workspace.candidate_eta_mut())?;
        fill_candidate_coefficients_from_frozen_output(
            &direction,
            workspace.candidate_coefficients_mut(),
        )?;
        direction.basis().kept_generated_columns().len()
    };
    // Categorical LSMR updates identified finite coordinates. Its inherited
    // absorbed intercept is a normalization coordinate, as on the canonical
    // specialized path; the final canonical fit independently normalizes it.
    let coefficient_slots = categorical.as_ref().map(|(joint, _)| joint.active_slots());
    #[cfg(test)]
    if JOINT_FINAL_TEST_MODE.get() == 2 {
        JOINT_FINAL_TEST_MODE.set(0);
        inject_candidate_profile_movement(workspace);
    }
    let objective =
        match workspace.evaluate_candidate_objective(objective_inputs, config.ppml.bounds) {
            Ok(value) => Some(value),
            Err(error) if error.code == ErrorCode::NonConvergence => None,
            Err(error) => return Err(error),
        };
    let consistent = strict_candidate_decision_with_coefficient_slots(
        true,
        true,
        expected_rank,
        rank,
        workspace.eta(),
        workspace.coefficients(),
        accepted_objective,
        workspace.candidate_eta(),
        workspace.candidate_coefficients(),
        objective,
        config.ppml,
        coefficient_slots,
    ) == FinalHessianSynchronization::Certified;
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=joint_strict_direction|consistent={}|eta_change={:.17e}|beta_change={:.17e}",
            u8::from(consistent), relative_infinity_difference(workspace.eta(), workspace.candidate_eta()),
            relative_infinity_difference(workspace.coefficients(), workspace.candidate_coefficients())));
    }
    if !consistent {
        return Ok(false);
    }
    // Keep the accepted point. The strict direction is only a witness; the
    // canonical final rank/inverse below independently validates its basis on
    // these exact weights before either result can authorize publication.
    workspace.prepare_final_hessian_response(offset)?;
    *frame_generation = next_frame_generation(*frame_generation)?;
    if categorical.is_some() {
        // The external LSMR witness read these same workspace weights. It did
        // not project the session response, so refresh that response only now:
        // two unsolved refreshes would break its retained response-delta seed.
        refresh_linear_frame(session, workspace, *frame_generation)?;
    } else if !refresh_final_linear_frame(session, workspace, *frame_generation)? {
        return Ok(false);
    }
    let transaction = session.strict_candidate()?;
    let canonical = transaction.candidate();
    let unchanged = transaction.basis_unchanged();
    #[cfg(test)]
    let unchanged = if JOINT_FINAL_TEST_MODE.get() == 3 {
        JOINT_FINAL_TEST_MODE.set(0);
        false
    } else {
        unchanged
    };
    accumulate_inner_iterations(
        total_inner_iterations,
        canonical.frame().absorption_report(),
    )?;
    fill_candidate_eta_from_linear_output(&canonical, offset, workspace.candidate_eta_mut())?;
    fill_candidate_coefficients_from_linear_output(
        &canonical,
        workspace.candidate_coefficients_mut(),
    )?;
    let objective =
        match workspace.evaluate_candidate_objective(objective_inputs, config.ppml.bounds) {
            Ok(value) => Some(value),
            Err(error) if error.code == ErrorCode::NonConvergence => None,
            Err(error) => return Err(error),
        };
    let accepted = strict_candidate_decision_with_coefficient_slots(
        canonical.certified(),
        unchanged,
        expected_rank,
        canonical.frame().finite_rank(),
        workspace.eta(),
        workspace.coefficients(),
        accepted_objective,
        workspace.candidate_eta(),
        workspace.candidate_coefficients(),
        objective,
        config.ppml,
        coefficient_slots,
    ) == FinalHessianSynchronization::Certified
        && relative_infinity_difference(workspace.eta(), workspace.candidate_eta())
            <= config.ppml.tolerance
        && coefficient_slots.map_or_else(
            || {
                relative_infinity_difference(
                    workspace.coefficients(),
                    workspace.candidate_coefficients(),
                )
            },
            |slots| {
                relative_infinity_difference_at_slots(
                    workspace.coefficients(),
                    workspace.candidate_coefficients(),
                    slots,
                )
            },
        ) <= config.ppml.tolerance;
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=joint_strict_final|accepted={}|basis_unchanged={}",
            u8::from(accepted),
            u8::from(unchanged)
        ));
    }
    if accepted {
        transaction.commit();
    } else {
        record_checkpoint_solver_observation(
            solver_report,
            canonical.frame().absorption_report(),
            false,
        )?;
        transaction.rollback();
    }
    #[cfg(test)]
    if accepted {
        JOINT_FINAL_TEST_COUNTS.set((
            JOINT_FINAL_TEST_COUNTS.get().0,
            JOINT_FINAL_TEST_COUNTS.get().1 + 1,
        ));
    }
    Ok(accepted)
}

/// Rank, within rows and inverse from the strict Newton WLS depend on its
/// exact weights, not on whether its RHS was the working response or predictor.
/// Recheck the accepted predictor against that canonical decomposition before
/// allowing those products to serve final inference without a second WLS.
fn reuse_canonical_final_products<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &CertifiedLinearSession<'_, P>,
    workspace: &PpmlIterationWorkspace,
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    generation: u64,
) -> Result<bool> {
    #[cfg(test)]
    if PROFILE_FINAL_TEST_MODE.get() == 1 {
        return Ok(false);
    }
    #[cfg(test)]
    let generation = if PROFILE_FINAL_TEST_MODE.get() == 2 {
        generation + 1
    } else {
        generation
    };
    let weights = workspace.hessian_weights();
    if !session.current_hessian_products_match(
        BorrowedMapWeights::Provided(weights),
        ppml_bread_weight_scale(weights)?,
        generation,
    )? {
        return Ok(false);
    }
    validate_final_hessian_frame(
        BorrowedMapWeights::Provided(weights),
        prior_weights,
        workspace.mean(),
    )?;
    let output = session.output()?;
    let tolerance = config
        .ppml
        .tolerance
        .min(config.ppml.score_tolerance)
        .min(1e-8);
    if offset.len() != workspace.eta().len()
        || output.finite_predictor().len() != offset.len()
        || output.absorbed_predictor().len() != offset.len()
    {
        return Err(internal("canonical final predictor is misaligned"));
    }
    let mut absorbed_scale = 0.0_f64;
    let mut working_scale = 0.0_f64;
    // Keep the legacy matrix indices and numerical traversal order explicit.
    #[allow(clippy::needless_range_loop)]
    for row in 0..offset.len() {
        absorbed_scale = absorbed_scale.max(output.absorbed_predictor()[row].abs());
        working_scale = working_scale
            .max(output.response()[row].abs())
            .max(output.frame().residuals()[row].abs())
            .max(output.finite_predictor()[row].abs());
        let candidate =
            offset[row] + (output.finite_predictor()[row] + output.absorbed_predictor()[row]);
        #[cfg(test)]
        let candidate = if PROFILE_FINAL_TEST_MODE.get() == 3 && row == 0 {
            candidate + 0.01
        } else {
            candidate
        };
        if !candidate.is_finite()
            || relative_scalar_difference(workspace.eta()[row], candidate) > tolerance
        {
            return Ok(false);
        }
    }
    // A strict Newton response can be enormous when a fitted mean is tiny.
    // Its response-minus-residual decomposition can pass the outer predictor
    // check while losing the precision needed to recover separate FE channels.
    // Decline this shortcut when component rounding exceeds the strict inner
    // budget. The existing final-Hessian replay uses the small accepted
    // predictor as its RHS and retains all rank, score and frame checks.
    if absorbed_scale > 0.0
        && 8.0 * f64::EPSILON * working_scale > strict_inner_tolerance(config.ppml) * absorbed_scale
    {
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(
                "FEREG_SOLVER_TRACE|phase=profile_final_products|accepted=0|reason=working_response_cancellation",
            );
        }
        return Ok(false);
    }
    #[cfg(test)]
    PROFILE_FINAL_ACCEPTED.set(PROFILE_FINAL_ACCEPTED.get() + 1);
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(
            "FEREG_SOLVER_TRACE|phase=profile_final_products|accepted=1",
        );
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn synchronize_final_hessian_frame<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &mut CertifiedLinearSession<'_, P>,
    workspace: &mut PpmlIterationWorkspace,
    objective_inputs: &PreparedPoissonInputs<'_>,
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    checkpoint_tolerance: f64,
    frame_generation: &mut u64,
    total_inner_iterations: &mut usize,
    expected_rank: usize,
    accepted_objective: PoissonObjective,
    coefficient_slots: Option<&[usize]>,
) -> Result<FinalHessianSynchronization> {
    workspace.refresh_prepared_working_state(objective_inputs, config.ppml.bounds)?;
    workspace.prepare_final_hessian_response(offset)?;
    *frame_generation = next_frame_generation(*frame_generation)?;
    // The accepted strict point usually changes the Hessian. Only a bitwise
    // identical weight frame retains its current weight generation and skips
    // the redundant retained-control projection; the final response is still
    // independently absorbed, solved, reconstructed, and certified below.
    let _reused_identical_weights =
        refresh_final_linear_frame(session, workspace, *frame_generation)?;
    let strict_tolerance = checkpoint_tolerance;
    session.set_config(ppml_linear_config(config, strict_tolerance)?)?;
    let synchronized_transaction = session.strict_candidate()?;
    let synchronized_basis_unchanged = synchronized_transaction.basis_unchanged();
    #[cfg(test)]
    let synchronized_basis_unchanged = synchronized_basis_unchanged && !take_strict_basis_change(2);
    let (synchronized_certified, synchronized_rank) = {
        let synchronized = synchronized_transaction.candidate();
        let synchronized_certified = synchronized.certified();
        let synchronized_rank = synchronized.frame().finite_rank();
        accumulate_inner_iterations(
            total_inner_iterations,
            synchronized.frame().absorption_report(),
        )?;
        fill_candidate_eta_from_linear_output(
            &synchronized,
            offset,
            workspace.candidate_eta_mut(),
        )?;
        fill_candidate_coefficients_from_linear_output(
            &synchronized,
            workspace.candidate_coefficients_mut(),
        )?;
        (synchronized_certified, synchronized_rank)
    };
    if !synchronized_basis_unchanged || expected_rank != synchronized_rank {
        return Err(numerical_nonconvergence(
            "prepared PPML final Hessian changed the canonical rank basis",
        ));
    }
    let predictor_difference =
        relative_infinity_difference(workspace.eta(), workspace.candidate_eta());
    let coefficient_difference = coefficient_slots.map_or_else(
        || {
            relative_infinity_difference(
                workspace.coefficients(),
                workspace.candidate_coefficients(),
            )
        },
        |slots| {
            relative_infinity_difference_at_slots(
                workspace.coefficients(),
                workspace.candidate_coefficients(),
                slots,
            )
        },
    );
    #[cfg(test)]
    let injected_movement = take_strict_resume(2);
    #[cfg(test)]
    if injected_movement {
        inject_candidate_profile_movement(workspace);
    }
    let candidate_objective =
        match workspace.evaluate_candidate_objective(objective_inputs, config.ppml.bounds) {
            Ok(objective) => objective,
            Err(error) if error.code == ErrorCode::NonConvergence => {
                synchronized_transaction.rollback();
                return Ok(FinalHessianSynchronization::ResumeAcceptedState);
            }
            Err(error) => return Err(error),
        };
    let decision = strict_candidate_decision_with_coefficient_slots(
        synchronized_certified,
        synchronized_basis_unchanged,
        expected_rank,
        synchronized_rank,
        workspace.eta(),
        workspace.coefficients(),
        accepted_objective,
        workspace.candidate_eta(),
        workspace.candidate_coefficients(),
        Some(candidate_objective),
        config.ppml,
        coefficient_slots,
    );
    let decision = if decision == FinalHessianSynchronization::Certified
        && (predictor_difference > config.ppml.tolerance
            || coefficient_difference > config.ppml.tolerance)
    {
        FinalHessianSynchronization::ResumeWithCandidate
    } else {
        decision
    };
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=final_hessian_decision|eta_change={predictor_difference:.17e}|beta_change={coefficient_difference:.17e}|decision={decision:?}",
        ));
    }
    if decision == FinalHessianSynchronization::Certified {
        synchronized_transaction.commit();
    } else {
        synchronized_transaction.rollback();
    }
    Ok(decision)
}

fn validate_final_hessian_frame(
    weights: BorrowedMapWeights<'_>,
    prior_weights: &[f64],
    mean: &[f64],
) -> Result<()> {
    if prior_weights.len() != mean.len() || prior_weights.is_empty() {
        return Err(invalid_argument(
            "prepared PPML final-Hessian evidence is not sample-aligned",
        ));
    }
    let supplied = match weights {
        BorrowedMapWeights::Provided(values) if values.len() == mean.len() => Some(values),
        BorrowedMapWeights::Provided(_) => {
            return Err(invalid_argument(
                "prepared PPML final-Hessian weights are not sample-aligned",
            ));
        }
        BorrowedMapWeights::Unit => None,
    };
    for row in 0..mean.len() {
        let expected = prior_weights[row] * mean[row];
        let actual = supplied.map_or(1.0, |values| values[row]);
        if !expected.is_finite() || expected <= 0.0 || !actual.is_finite() || actual <= 0.0 {
            return Err(numerical_nonconvergence(
                "prepared PPML final Hessian is not finitely representable",
            ));
        }
        if actual.to_bits() != expected.to_bits() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "prepared PPML linear products are not on the accepted final Hessian",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn initialize<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    fe_design: &FeDesign,
    config: PreparedPpmlIrlsConfig,
    start: Option<&PreparedPpmlStart>,
    purpose: PreparedPpmlIrlsPurpose,
) -> Result<(Vec<f64>, Vec<f64>)> {
    if let Some(start) = start {
        validate_start(prepared, start, prior_weights, offset, fe_design, config)?;
        return Ok((
            fallible_copy(
                &start.coefficient_values,
                "unable to copy prepared PPML warm-start coefficients",
            )?,
            fallible_copy(
                &start.eta,
                "unable to copy prepared PPML warm-start predictor",
            )?,
        ));
    }
    let mut coefficient_values = fallible_filled(
        prepared.fit_model().coefficients.len(),
        0.0,
        "unable to reserve prepared PPML initial coefficients",
    )?;
    let mut intercept_position = None;
    if prepared.fit_report_intercept() {
        for (position, coefficient) in prepared.fit_model().coefficients.iter().enumerate() {
            let Some(generated) = coefficient.generated_column_index else {
                continue;
            };
            let column = prepared.fit_model().columns.get(generated).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "prepared PPML intercept coefficient points outside the generated design",
                )
            })?;
            if matches!(column.kind, PreparedOlsColumnKind::Intercept) {
                if intercept_position.is_some() {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "prepared PPML model contains multiple reported structural constants",
                    ));
                }
                intercept_position = Some(position);
            }
        }
        if intercept_position.is_none() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "prepared PPML reports a structural constant without an intercept slot",
            ));
        }
    }
    let mut eta = fallible_filled(
        outcome.len(),
        0.0,
        "unable to reserve prepared PPML initial predictor",
    )?;
    if purpose == PreparedPpmlIrlsPurpose::Publication
        && seed_multi_effect_predictor(
            outcome,
            prior_weights,
            offset,
            fe_design,
            config.ppml,
            &mut eta,
        )
    {
        // Absorbed intercepts represent the entire initial predictor. The
        // reported constant may start at zero; canonical solving establishes
        // its final normalization independently.
        return Ok((coefficient_values, eta));
    }
    let shift = if intercept_position.is_some() {
        log_weighted_outcome_sum(outcome, prior_weights)?
            - log_weighted_offset_sum(offset, prior_weights)?
    } else {
        0.0
    };
    if let Some(position) = intercept_position {
        coefficient_values[position] = shift;
    }
    if purpose == PreparedPpmlIrlsPurpose::Publication
        && seed_one_effect_predictor(
            outcome,
            prior_weights,
            offset,
            fe_design,
            config.ppml,
            &mut eta,
        )
    {
        return Ok((coefficient_values, eta));
    }
    // The first conditional intercept eliminates the global starting shift.
    // Compute that shift only when the complete provisional seed was refused;
    // overwriting every row also discards any partially completed sweep.
    let shift = if intercept_position.is_some() {
        shift
    } else {
        log_weighted_outcome_sum(outcome, prior_weights)?
            - log_weighted_offset_sum(offset, prior_weights)?
    };
    for row in 0..outcome.len() {
        eta[row] = offset[row] + shift;
        if !eta[row].is_finite() {
            return Err(numerical_nonconvergence(
                "prepared PPML initialization is not finitely representable",
            ));
        }
    }
    Ok((coefficient_values, eta))
}

fn seed_multi_effect_predictor(
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
    design: &FeDesign,
    config: PpmlConfig,
    eta: &mut [f64],
) -> bool {
    if design.effects().len() < 2
        || design
            .effects()
            .iter()
            .any(|effect| !effect.has_intercept())
    {
        return false;
    }
    // Solve the first partition directly from original offsets. Its exact
    // conditional optimum does not depend on a preceding global intercept.
    // Reuse the guarded one-partition arithmetic, including log-space fallback.
    if !seed_group_intercept_predictor(outcome, weights, offset, &design.effects()[0], config, eta)
    {
        return false;
    }
    // Start with exact conditional intercepts, then one safeguarded Newton
    // update per slope with that intercept profiled out. These model-space
    // initial guesses have no rank, convergence or final-Hessian authority.
    for effect in &design.effects()[1..] {
        for level in 0..effect.n_levels() {
            let members = effect.level_index().members(level);
            let mut observed = CompensatedSum::default();
            let mut fitted = CompensatedSum::default();
            for &row in members {
                let y = weights[row] * outcome[row];
                let mu = weights[row] * eta[row].exp();
                if !y.is_finite()
                    || !mu.is_finite()
                    || mu <= 0.0
                    || (y == 0.0 && outcome[row] > 0.0)
                {
                    return false;
                }
                observed.add(y);
                fitted.add(mu);
            }
            let (observed, fitted) = (observed.total(), fitted.total());
            if !observed.is_finite() || observed <= 0.0 || !fitted.is_finite() || fitted <= 0.0 {
                return false;
            }
            let shift = observed.ln() - fitted.ln();
            for &row in members {
                let value = eta[row] + shift;
                if !usable_seed_value(value, config) {
                    return false;
                }
                eta[row] = value;
            }
        }
    }
    // Establish all intercepts before slopes can explain another partition's
    // intercept variation. The second stage profiles the current group mass
    // afresh: later partition updates may have moved its conditional intercept.
    for effect in design
        .effects()
        .iter()
        .filter(|effect| !effect.slopes().is_empty())
    {
        for level in 0..effect.n_levels() {
            let members = effect.level_index().members(level);
            let mut observed = CompensatedSum::default();
            for &row in members {
                observed.add(weights[row] * outcome[row]);
            }
            for slope in effect.slopes() {
                if !seed_profiled_slope(
                    members,
                    slope,
                    outcome,
                    weights,
                    observed.total(),
                    config,
                    eta,
                ) {
                    return false;
                }
            }
        }
    }
    true
}

// Profile the group intercept exactly for each proposed slope step. Other
// coefficients remain fixed; only a nondecreasing finite group Poisson
// objective permits a provisional update, including the intercept correction.
#[allow(clippy::too_many_arguments)]
fn seed_profiled_slope(
    members: &[usize],
    slope: &[f64],
    outcome: &[f64],
    weights: &[f64],
    observed: f64,
    config: PpmlConfig,
    eta: &mut [f64],
) -> bool {
    let scale = members
        .iter()
        .map(|&r| slope[r].abs())
        .fold(0.0_f64, f64::max);
    let constant = members.iter().all(|&r| slope[r] == slope[members[0]]);
    let scale = if scale == 0.0 { 1.0 } else { scale };
    let mut target = CompensatedSum::default();
    let mut first = CompensatedSum::default();
    let mut mass = CompensatedSum::default();
    for &r in members {
        let z = slope[r] / scale;
        let wmu = weights[r] * eta[r].exp();
        target.add(weights[r] * outcome[r] * z);
        first.add(wmu * z);
        mass.add(wmu);
    }
    let center = first.total() / mass.total();
    let mut curvature = CompensatedSum::default();
    for &r in members {
        let z = slope[r] / scale - center;
        curvature.add(weights[r] * eta[r].exp() * z * z);
    }
    let score = target.total() - observed * center;
    let curvature = curvature.total() * (observed / mass.total());
    if !score.is_finite() || !curvature.is_finite() {
        return false;
    }
    // A constant slope has no profiled direction, but the current intercept
    // can still need its conditional correction after another partition moved.
    let mut step = if constant || curvature == 0.0 {
        0.0
    } else {
        score / curvature
    };
    if !step.is_finite() {
        return false;
    }
    for _ in 0..=config.max_step_halvings {
        // A scaled sum avoids overflow of the unnormalized trial means.
        let maximum = members
            .iter()
            .map(|&r| weights[r].ln() + eta[r] + step * (slope[r] / scale))
            .fold(f64::NEG_INFINITY, f64::max);
        let mut trial_mass = CompensatedSum::default();
        for &r in members {
            trial_mass.add((weights[r].ln() + eta[r] + step * (slope[r] / scale) - maximum).exp());
        }
        let shift = observed.ln() - (maximum + trial_mass.total().ln());
        let change = step * target.total() + observed * shift - (observed - mass.total());
        if change.is_finite()
            && change >= 0.0
            && members
                .iter()
                .all(|&r| usable_seed_value(eta[r] + step * (slope[r] / scale) + shift, config))
        {
            for &r in members {
                eta[r] += step * (slope[r] / scale) + shift;
            }
            return true;
        }
        step *= 0.5;
    }
    // A refused coordinate leaves this group unchanged. The seed remains a
    // finite model-space guess; full IRLS still establishes its own acceptance.
    true
}

fn seed_one_effect_predictor(
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
    design: &FeDesign,
    config: PpmlConfig,
    eta: &mut [f64],
) -> bool {
    let [effect] = design.effects() else {
        return false;
    };
    if !effect.has_intercept() || !effect.slopes().is_empty() {
        return false;
    }
    seed_group_intercept_predictor(outcome, weights, offset, effect, config, eta)
}

fn seed_group_intercept_predictor(
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
    effect: &crate::fe::EffectTerm,
    config: PpmlConfig,
    eta: &mut [f64],
) -> bool {
    // At beta=0 each one-way intercept has an analytic Poisson optimum.
    // Evaluate both sums in log space; this is an initialization only, before
    // canonical rank freezing or any accepted IRLS state exists. A failed
    // numerical seed is overwritten by the original global initialization.
    for level in 0..effect.n_levels() {
        let members = effect.level_index().members(level);
        if let Some(value) = constant_offset_group_log_mean(members, outcome, weights, offset) {
            if !usable_seed_value(value, config) {
                return false;
            }
            for &row in members {
                eta[row] = value;
            }
            continue;
        }
        let mut maximum_y = f64::NEG_INFINITY;
        let mut maximum_offset = f64::NEG_INFINITY;
        for &row in members {
            if outcome[row] > 0.0 {
                maximum_y = maximum_y.max(outcome[row].ln() + weights[row].ln());
            }
            maximum_offset = maximum_offset.max(offset[row] + weights[row].ln());
        }
        if !maximum_y.is_finite() || !maximum_offset.is_finite() {
            return false;
        }
        let mut sum_y = CompensatedSum::default();
        let mut sum_offset = CompensatedSum::default();
        for &row in members {
            if outcome[row] > 0.0 {
                sum_y.add((outcome[row].ln() + weights[row].ln() - maximum_y).exp());
            }
            sum_offset.add((offset[row] + weights[row].ln() - maximum_offset).exp());
        }
        let shift = maximum_y + sum_y.total().ln() - sum_offset.total().ln();
        for &row in members {
            let value = (offset[row] - maximum_offset) + shift;
            if !usable_seed_value(value, config) {
                return false;
            }
            eta[row] = value;
        }
    }
    true
}

fn constant_offset_group_log_mean(
    members: &[usize],
    outcome: &[f64],
    weights: &[f64],
    offset: &[f64],
) -> Option<f64> {
    // A group-constant offset cancels analytically. Avoid per-row logarithms
    // and exponentials in this common case; nonrepresentable raw moments
    // return to the compensated log-sum-exp path before any predictor write.
    let &first = members.first()?;
    let mut numerator = CompensatedSum::default();
    let mut denominator = CompensatedSum::default();
    for &row in members {
        if offset[row] != offset[first] {
            return None;
        }
        let value = weights[row] * outcome[row];
        if !value.is_finite() || (value == 0.0 && outcome[row] > 0.0) {
            return None;
        }
        numerator.add(value);
        denominator.add(weights[row]);
    }
    let numerator = numerator.total();
    let denominator = denominator.total();
    if !numerator.is_finite() || numerator <= 0.0 || !denominator.is_finite() || denominator <= 0.0
    {
        return None;
    }
    let mean = numerator / denominator;
    (mean.is_finite() && mean > 0.0).then(|| mean.ln())
}

fn usable_seed_value(value: f64, config: PpmlConfig) -> bool {
    let mean = value.exp();
    value.is_finite()
        && value >= config.bounds.eta_min
        && value <= config.bounds.eta_max
        && mean.is_finite()
        && mean > 0.0
        && mean >= config.bounds.mu_min
        && mean <= config.bounds.mu_max
}

fn validate_start<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    start: &PreparedPpmlStart,
    prior_weights: &[f64],
    offset: &[f64],
    fe_design: &FeDesign,
    config: PreparedPpmlIrlsConfig,
) -> Result<()> {
    let rows = prepared.fit_sample().n_active();
    if start.coefficient_values.len() != prepared.fit_model().coefficients.len()
        || start.eta.len() != rows
    {
        return Err(invalid_argument(
            "prepared PPML warm start has inconsistent dimensions",
        ));
    }
    if start
        .coefficient_values
        .iter()
        .chain(&start.eta)
        .any(|value| !value.is_finite())
    {
        return Err(invalid_data("prepared PPML warm start must be finite"));
    }
    let mut means = fallible_filled(
        rows,
        0.0,
        "unable to reserve prepared PPML warm-start means",
    )?;
    means_from_eta(&start.eta, config.ppml.bounds, &mut means)?;
    let mut remainder = fallible_filled(
        rows,
        0.0,
        "unable to reserve prepared PPML warm-start remainder",
    )?;
    for row in 0..rows {
        let mut finite = 0.0_f64;
        for (slot, coefficient) in prepared
            .fit_model()
            .coefficients
            .iter()
            .zip(&start.coefficient_values)
        {
            if let Some(generated) = slot.generated_column_index {
                finite = coefficient.mul_add(prepared.fit_column_value(generated, row)?, finite);
            } else if *coefficient != 0.0 {
                return Err(invalid_argument(
                    "prepared PPML warm start assigns a syntax-omitted coefficient",
                ));
            }
        }
        remainder[row] = start.eta[row] - offset[row] - finite;
        if !remainder[row].is_finite() {
            return Err(invalid_data(
                "prepared PPML warm-start decomposition is nonfinite",
            ));
        }
    }
    let mut residualized = DenseMatrix::from_columns(&[remainder])?;
    let report = absorb_fixed_effects(
        fe_design,
        prior_weights,
        &mut residualized,
        strict_absorb(config.linear, config.ppml.inner_tolerance_final).absorb,
    )?;
    if !report.certified() {
        return Err(invalid_data(
            "prepared PPML warm-start FE decomposition did not certify",
        ));
    }
    let reference = start
        .eta
        .iter()
        .chain(offset)
        .fold(1.0_f64, |scale, value| scale.max(value.abs()));
    let residual = residualized
        .as_slice()
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    let tolerance = config
        .ppml
        .tolerance
        .max(config.ppml.inner_tolerance_final)
        .max(256.0 * f64::EPSILON);
    if residual <= tolerance * reference {
        Ok(())
    } else {
        Err(invalid_data(
            "prepared PPML warm start is outside the prepared model span",
        ))
    }
}

fn ppml_linear_config(
    config: PreparedPpmlIrlsConfig,
    inner_tolerance: f64,
) -> Result<PreparedOlsFitConfig> {
    let mut linear = config.linear;
    linear.absorb = config.ppml.absorb;
    linear.coefficient_tolerance = Some(
        config
            .ppml
            .tolerance
            .min(config.ppml.score_tolerance)
            .min(1e-8),
    );
    if linear.absorb.solver == FeSolverKind::Auto {
        linear.absorb.solver = FeSolverKind::Map;
    }
    validate_solver_admission(config)?;
    linear.fixed_effect_collinearity_tolerance = config.ppml.fixed_effect_collinearity_tolerance;
    Ok(strict_absorb(linear, inner_tolerance))
}

fn validate_solver_admission(config: PreparedPpmlIrlsConfig) -> Result<()> {
    if config.linear.absorb.solver != FeSolverKind::Map
        || config.ppml.absorb.solver != FeSolverKind::Map
        || config.solver_plan.selected == super::PpmlSolverFamily::Auto
    {
        return Err(invalid_argument(
            "prepared PPML canonical sessions require a MAP-configured fallback under a resolved public solver plan",
        ));
    }
    Ok(())
}

fn strict_absorb(mut config: PreparedOlsFitConfig, tolerance: f64) -> PreparedOlsFitConfig {
    config.absorb.map.tolerance = tolerance;
    config.absorb.lsmr.tolerance = tolerance;
    // The linear score is an inner-solve certificate. It may be no tighter
    // than the scheduled projection during optimization, while the strict
    // final solve resets this to publication tolerance.
    config.score_tolerance = config.score_tolerance.max(tolerance);
    config
}

fn ppml_bread_weight_scale(weights: &[f64]) -> Result<PreparedOlsBreadWeightScale> {
    let mut maximum = 0.0_f64;
    let mut sum = CompensatedSum::default();
    for weight in weights {
        if !weight.is_finite() || *weight <= 0.0 {
            return Err(numerical_nonconvergence(
                "prepared PPML Hessian weights must be finite and positive",
            ));
        }
        maximum = maximum.max(*weight);
        sum.add(*weight);
    }
    let reference_weight_sum = sum.total();
    if maximum <= 0.0 || !maximum.is_finite() || !reference_weight_sum.is_finite() {
        return Err(numerical_nonconvergence(
            "prepared PPML Hessian weight scale is not finite",
        ));
    }
    Ok(PreparedOlsBreadWeightScale {
        weight_kind: None,
        solver_weight_divisor: maximum,
        reference_weight_multiplier: maximum,
        reference_weight_sum,
    })
}

fn fill_candidate_eta_from_linear_output(
    output: &CertifiedLinearSessionOutput<'_>,
    offset: &[f64],
    eta: &mut [f64],
) -> Result<()> {
    if output.response().len() != offset.len()
        || output.frame().residuals().len() != offset.len()
        || output.finite_predictor().len() != offset.len()
        || output.absorbed_predictor().len() != offset.len()
        || eta.len() != offset.len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML linear output has inconsistent dimensions",
        ));
    }
    for row in 0..offset.len() {
        // The canonical response and residual already identify the total
        // predictor. Splitting and recombining its FE part adds cancellation.
        eta[row] = offset[row] + (output.response()[row] - output.frame().residuals()[row]);
        if !eta[row].is_finite() {
            return Err(numerical_nonconvergence(
                "prepared PPML candidate predictor is not finite",
            ));
        }
    }
    Ok(())
}

fn fill_candidate_coefficients_from_linear_output(
    output: &CertifiedLinearSessionOutput<'_>,
    coefficients: &mut [f64],
) -> Result<()> {
    if coefficients.len() != output.coefficients().len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML linear coefficient output has inconsistent dimensions",
        ));
    }
    for (target, source) in coefficients.iter_mut().zip(output.coefficients()) {
        *target = source.value.unwrap_or(0.0);
    }
    Ok(())
}

fn fill_candidate_eta_from_frozen_output(
    output: &PpmlOrdinaryLinearOutput<'_>,
    offset: &[f64],
    eta: &mut [f64],
) -> Result<()> {
    if output.response().len() != offset.len()
        || output.within_residuals().len() != offset.len()
        || eta.len() != offset.len()
        || output.coefficients().len() != output.basis().kept_generated_columns().len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML frozen output has inconsistent dimensions",
        ));
    }
    for row in 0..offset.len() {
        eta[row] = offset[row] + output.response()[row] - output.within_residuals()[row];
        if !eta[row].is_finite() {
            return Err(numerical_nonconvergence(
                "prepared PPML frozen candidate predictor is not finite",
            ));
        }
    }
    Ok(())
}

fn fill_candidate_coefficients_from_frozen_output(
    output: &PpmlOrdinaryLinearOutput<'_>,
    coefficients: &mut [f64],
) -> Result<()> {
    if coefficients.len() != output.basis().slots().len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML frozen coefficient output has inconsistent dimensions",
        ));
    }
    coefficients.fill(0.0);
    if output.reported_coefficients().len() != coefficients.len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML frozen reported coefficients are misaligned",
        ));
    }
    for (target, source) in coefficients.iter_mut().zip(output.reported_coefficients()) {
        *target = source.unwrap_or(0.0);
    }
    Ok(())
}

fn record_linear_phase_timings(
    memory: &mut PpmlMemoryTelemetry,
    reuse: LinearSessionReuseReport,
) -> Result<()> {
    for (phase, seconds) in [
        (
            PpmlPhase::FeProjection,
            reuse.phase_timing.projection_seconds,
        ),
        (
            PpmlPhase::FiniteSolve,
            reuse.phase_timing.finite_solve_seconds,
        ),
    ] {
        let Some(seconds) = seconds else {
            continue;
        };
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "prepared PPML linear phase timer is invalid",
            ));
        }
        memory.record_phase(phase, std::time::Duration::from_secs_f64(seconds));
    }
    for (phase, peak_bytes) in [
        (
            PpmlPhase::FeProjection,
            reuse.phase_allocation.projection_peak_bytes,
        ),
        (
            PpmlPhase::FiniteSolve,
            reuse.phase_allocation.finite_solve_peak_bytes,
        ),
    ] {
        if let Some(peak_bytes) = peak_bytes {
            memory.observe_phase_peak(phase, peak_bytes);
        }
    }
    Ok(())
}

fn record_phase_allocation(
    memory: &mut PpmlMemoryTelemetry,
    probe: Option<PpmlPhaseAllocatorProbe>,
    phase: PpmlPhase,
) {
    if let Some(bytes) = end_phase_allocation(probe, phase) {
        memory.observe_phase_peak(phase, bytes);
    }
}

fn fresh_warm_tolerance(config: PpmlConfig, inner_tolerance: f64) -> f64 {
    config
        .tolerance
        .max(64.0 * inner_tolerance)
        .max(512.0 * f64::EPSILON)
}

/// Same-frame equivalence evidence collected after the allocation window.
/// `solve_fresh` independently rebuilds the canonical current-frame solution
/// and the session rejects any disagreement with its frozen basis identity;
/// this comparison then covers every reported coefficient slot and the
/// complete finite-plus-absorbed predictor under the registered tolerance.
fn fresh_warm_linear_equivalent(
    fresh: &CertifiedLinearSessionOutput<'_>,
    offset: &[f64],
    frozen_eta: &[f64],
    frozen_coefficients: &[f64],
    frozen_rank: usize,
    config: PpmlConfig,
    inner_tolerance: f64,
) -> Result<bool> {
    if fresh.response().len() != offset.len()
        || fresh.frame().residuals().len() != offset.len()
        || fresh.finite_predictor().len() != offset.len()
        || fresh.absorbed_predictor().len() != offset.len()
        || frozen_eta.len() != offset.len()
        || fresh.coefficients().len() != frozen_coefficients.len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML fresh/warm audit dimensions are inconsistent",
        ));
    }
    let tolerance = fresh_warm_tolerance(config, inner_tolerance);
    let mut predictor_difference = 0.0_f64;
    for row in 0..offset.len() {
        let value = offset[row] + (fresh.response()[row] - fresh.frame().residuals()[row]);
        if !value.is_finite() || !frozen_eta[row].is_finite() {
            return Ok(false);
        }
        predictor_difference = predictor_difference
            .max((value - frozen_eta[row]).abs() / value.abs().max(frozen_eta[row].abs()).max(1.0));
    }
    let mut coefficient_difference = 0.0_f64;
    for (fresh, frozen) in fresh.coefficients().zip(frozen_coefficients) {
        let value = fresh.value.unwrap_or(0.0);
        if !value.is_finite() || !frozen.is_finite() {
            return Ok(false);
        }
        coefficient_difference = coefficient_difference
            .max((value - frozen).abs() / value.abs().max(frozen.abs()).max(1.0));
    }
    Ok(fresh.certified()
        && fresh.frame().finite_rank() == frozen_rank
        && predictor_difference <= tolerance
        && coefficient_difference <= tolerance)
}

#[allow(clippy::too_many_arguments)]
fn strict_profile_consistent(
    eta: &[f64],
    coefficients: &[f64],
    objective: PoissonObjective,
    strict_eta: &[f64],
    strict_coefficients: &[f64],
    strict_objective: PoissonObjective,
    config: PpmlConfig,
    coefficient_slots: Option<&[usize]>,
) -> bool {
    // Publication may be stricter than the attainable inner-solve or machine
    // floor. In that case fail closed instead of silently weakening the
    // caller's requested outer/score tolerance.
    let tolerance = config.tolerance.min(config.score_tolerance).min(1e-8);
    strict_profile_difference(
        eta,
        coefficients,
        objective,
        strict_eta,
        strict_coefficients,
        strict_objective,
        coefficient_slots,
    ) <= tolerance
}

#[allow(clippy::too_many_arguments)]
fn strict_profile_difference(
    eta: &[f64],
    coefficients: &[f64],
    objective: PoissonObjective,
    strict_eta: &[f64],
    strict_coefficients: &[f64],
    strict_objective: PoissonObjective,
    coefficient_slots: Option<&[usize]>,
) -> f64 {
    let differences = [
        relative_infinity_difference(eta, strict_eta),
        strict_coefficient_difference(coefficients, strict_coefficients, coefficient_slots),
        relative_scalar_difference(objective.deviance, strict_objective.deviance),
        relative_scalar_difference(objective.log_likelihood, strict_objective.log_likelihood),
    ];
    differences
        .into_iter()
        .fold(0.0_f64, |maximum, difference| {
            if difference.is_finite() {
                maximum.max(difference)
            } else {
                f64::INFINITY
            }
        })
}

fn strict_coefficient_difference(left: &[f64], right: &[f64], slots: Option<&[usize]>) -> f64 {
    if left.len() != right.len() {
        return f64::INFINITY;
    }
    let difference = |slot: usize| {
        let (Some(&left), Some(&right)) = (left.get(slot), right.get(slot)) else {
            return f64::INFINITY;
        };
        if !left.is_finite() || !right.is_finite() {
            return f64::INFINITY;
        }
        // The registered coefficient envelope has atol/rtol = 1e-3. With
        // the strict tolerance capped at 1e-8, this requires Newton movement
        // within one tenth of that envelope even near zero. Retain the old
        // scale as an upper bound so no coefficient gets a weaker check.
        let scale = crate::ols::coefficient_accuracy_scale(left, right);
        (left - right).abs() / scale
    };
    match slots {
        Some(slots) => slots.iter().copied().map(difference).fold(0.0, f64::max),
        None => (0..left.len()).map(difference).fold(0.0, f64::max),
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn strict_candidate_decision(
    certified: bool,
    basis_unchanged: bool,
    accepted_rank: usize,
    candidate_rank: usize,
    eta: &[f64],
    coefficients: &[f64],
    objective: PoissonObjective,
    candidate_eta: &[f64],
    candidate_coefficients: &[f64],
    candidate_objective: Option<PoissonObjective>,
    config: PpmlConfig,
) -> FinalHessianSynchronization {
    strict_candidate_decision_with_coefficient_slots(
        certified,
        basis_unchanged,
        accepted_rank,
        candidate_rank,
        eta,
        coefficients,
        objective,
        candidate_eta,
        candidate_coefficients,
        candidate_objective,
        config,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn strict_candidate_decision_with_coefficient_slots(
    certified: bool,
    basis_unchanged: bool,
    accepted_rank: usize,
    candidate_rank: usize,
    eta: &[f64],
    coefficients: &[f64],
    objective: PoissonObjective,
    candidate_eta: &[f64],
    candidate_coefficients: &[f64],
    candidate_objective: Option<PoissonObjective>,
    config: PpmlConfig,
    coefficient_slots: Option<&[usize]>,
) -> FinalHessianSynchronization {
    let Some(candidate_objective) = candidate_objective else {
        return FinalHessianSynchronization::ResumeAcceptedState;
    };
    if !certified {
        return FinalHessianSynchronization::ResumeAcceptedState;
    }
    if !basis_unchanged
        || accepted_rank != candidate_rank
        || !strict_profile_consistent(
            eta,
            coefficients,
            objective,
            candidate_eta,
            candidate_coefficients,
            candidate_objective,
            config,
            coefficient_slots,
        )
    {
        FinalHessianSynchronization::ResumeWithCandidate
    } else {
        FinalHessianSynchronization::Certified
    }
}

fn relative_deviance_change(old: PoissonObjective, new: PoissonObjective) -> f64 {
    (old.deviance - new.deviance).abs() / old.deviance.min(new.deviance).max(0.1)
}

fn relative_infinity_difference(left: &[f64], right: &[f64]) -> f64 {
    if left.len() != right.len() {
        return f64::INFINITY;
    }
    left.iter()
        .zip(right)
        .map(|(left, right)| (left - right).abs() / left.abs().max(right.abs()).max(1.0))
        .fold(0.0_f64, f64::max)
}

fn relative_infinity_difference_at_slots(left: &[f64], right: &[f64], slots: &[usize]) -> f64 {
    if left.len() != right.len() || slots.iter().any(|slot| *slot >= left.len()) {
        return f64::INFINITY;
    }
    slots
        .iter()
        .map(|slot| {
            let left = left[*slot];
            let right = right[*slot];
            (left - right).abs() / left.abs().max(right.abs()).max(1.0)
        })
        .fold(0.0_f64, f64::max)
}

fn relative_scalar_difference(left: f64, right: f64) -> f64 {
    (left - right).abs() / left.abs().max(right.abs()).max(1.0)
}

fn final_coefficients(
    fit: &CertifiedLinearSessionOutput<'_>,
    values: &[f64],
) -> Result<Vec<PreparedOlsFittedCoefficient>> {
    if fit.coefficients().len() != values.len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML final coefficient dimensions differ",
        ));
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(values.len())
        .map_err(|_| allocation("unable to reserve prepared PPML final coefficients"))?;
    for (coefficient, value) in fit.coefficients().zip(values) {
        result.push(PreparedOlsFittedCoefficient {
            coefficient_slot: coefficient.coefficient_slot,
            generated_column_index: coefficient.generated_column_index,
            status: coefficient.status,
            value: match coefficient.status {
                PreparedOlsFitStatus::Kept | PreparedOlsFitStatus::NormalizedAbsorbedIntercept => {
                    Some(*value)
                }
                PreparedOlsFitStatus::SyntaxOmitted
                | PreparedOlsFitStatus::AbsorbedInterceptDeferred
                | PreparedOlsFitStatus::CollinearWithFixedEffects
                | PreparedOlsFitStatus::NumericallyRankDeficient => None,
            },
        });
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn prepared_score_certificate<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    fe_design: &FeDesign,
    outcome: &[f64],
    mean: &[f64],
    prior_weights: &[f64],
    tolerance: f64,
    workspace: &mut PpmlScoreWorkspace,
) -> Result<PpmlCertificate> {
    let rows = outcome.len();
    if rows != mean.len()
        || rows != prior_weights.len()
        || rows != prepared.fit_sample().n_active()
        || rows != fe_design.nobs()
        || !tolerance.is_finite()
        || tolerance <= 0.0
    {
        return Err(invalid_argument(
            "prepared PPML score-certificate inputs are inconsistent",
        ));
    }
    let mut residual_scale = CompensatedSum::default();
    let mut reference_scale = CompensatedSum::default();
    for row in 0..rows {
        let residual = outcome[row] - mean[row];
        let inverse_variance_weight = prior_weights[row] / mean[row];
        residual_scale.add(inverse_variance_weight * residual * residual);
        reference_scale.add(inverse_variance_weight * outcome[row] * outcome[row]);
    }
    let score_scale = residual_scale
        .total()
        .sqrt()
        .max(reference_scale.total().sqrt());
    if !score_scale.is_finite() {
        return Err(numerical_nonconvergence(
            "prepared PPML score scale is not finite",
        ));
    }
    let mut finite_absolute = 0.0_f64;
    let mut fe_absolute = 0.0_f64;
    let mut relative_infinity = 0.0_f64;
    let mut relative_square = CompensatedSum::default();
    let mut relative_count = 0_usize;
    let numeric = crate::ols::NumericOlsColumns::bind_score_means(
        prepared.fit_model(),
        prepared.fit_sample(),
    )?;
    #[cfg(test)]
    let numeric = if FORCE_GENERIC_OUTER_SCORE.get() {
        None
    } else {
        numeric
    };
    let generated_parallel = numeric.is_none()
        && rows >= 4096
        && prepared.fit_model().columns.iter().any(|c| {
            matches!(
                c.kind,
                crate::ols::PreparedOlsColumnKind::Categorical { .. }
                    | crate::ols::PreparedOlsColumnKind::CategoricalInteraction { .. }
            )
        });
    #[cfg(test)]
    let generated_parallel = generated_parallel && !FORCE_GENERIC_OUTER_SCORE.get();
    let mut numeric_moments = [(0.0_f64, 0.0_f64); crate::ols::MAX_BOUND_NUMERIC_COLUMNS];
    if let Some(numeric) = numeric.as_ref() {
        // Each column keeps the original observation order and arithmetic.
        // Only independent columns run concurrently; the certificate reduction
        // below remains in canonical column order. Inputs are original rows,
        // not previously projected controls or a cached Newton score.
        workspace.team.install(|| {
            use rayon::prelude::*;
            numeric_moments[..numeric.raw_columns().len()]
                .par_iter_mut()
                .zip(numeric.raw_columns().par_iter())
                .for_each(|(target, column)| {
                    let mut moment = CompensatedSum::default();
                    let mut square = CompensatedSum::default();
                    for row in 0..rows {
                        let basis = column.map_or(1.0, |values| values[row]);
                        moment.add(prior_weights[row] * (outcome[row] - mean[row]) * basis);
                        square.add(prior_weights[row] * mean[row] * basis * basis);
                    }
                    *target = (moment.total(), square.total());
                });
        });
    }
    for column in 0..prepared.fit_model().columns.len() {
        // Reuse the numeric path's bounded scalar bank for arbitrary generated
        // width. Each job reads originals in row order; canonical reduction
        // order below and every independently formed score remain unchanged.
        if generated_parallel && column % numeric_moments.len() == 0 {
            let width = numeric_moments
                .len()
                .min(prepared.fit_model().columns.len() - column);
            workspace.team.install(|| {
                use rayon::prelude::*;
                numeric_moments[..width]
                    .par_iter_mut()
                    .enumerate()
                    .try_for_each(|(local, target)| -> Result<()> {
                        let mut moment = CompensatedSum::default();
                        let mut square = CompensatedSum::default();
                        crate::prepared_fit_view::visit_prepared_column_range(
                            prepared,
                            column + local,
                            0..rows,
                            |row, basis| {
                                moment.add(prior_weights[row] * (outcome[row] - mean[row]) * basis);
                                square.add(prior_weights[row] * mean[row] * basis * basis);
                                Ok(())
                            },
                        )?;
                        *target = (moment.total(), square.total());
                        Ok(())
                    })
            })?;
        }
        let (moment, basis_square) = if generated_parallel {
            numeric_moments[column % numeric_moments.len()]
        } else if numeric.is_some() {
            numeric_moments[column]
        } else {
            let mut moment = CompensatedSum::default();
            let mut basis_square = CompensatedSum::default();
            for row in 0..rows {
                let basis = prepared.fit_column_value(column, row)?;
                moment.add(prior_weights[row] * (outcome[row] - mean[row]) * basis);
                basis_square.add(prior_weights[row] * mean[row] * basis * basis);
            }
            (moment.total(), basis_square.total())
        };
        finite_absolute = finite_absolute.max(moment.abs());
        add_relative_score(
            moment,
            basis_square.sqrt(),
            score_scale,
            &mut relative_infinity,
            &mut relative_square,
            &mut relative_count,
        );
    }
    workspace.fe_moments.fill(0.0);
    workspace.fe_basis_squares.fill(0.0);
    let moments = &mut workspace.fe_moments;
    let squares = &mut workspace.fe_basis_squares;
    let budget = workspace.team.threads();
    let parallel = fe_design.effects().iter().any(|effect| {
        crate::parallel::StagePlan::new(
            rows as u128 * effect.channels() as u128 * 2,
            effect.n_levels(),
            budget,
        )
        .lanes
            > 1
    });
    let mut accumulate_effects = || -> Result<()> {
        let mut effect_offset = 0_usize;
        for effect in fe_design.effects() {
            let directions = effect.n_levels().saturating_mul(effect.channels());
            let plan = crate::parallel::StagePlan::new(
                rows as u128 * effect.channels() as u128 * 2,
                effect.n_levels(),
                budget,
            );
            if plan.lanes > 1 {
                // A direction owns its two accumulators. Membership order is
                // original observation order, so independent groups/channels
                // can run together without reassociating either score sum.
                let mut contexts = vec![(); plan.lanes];
                crate::parallel::group_blocks(
                    crate::parallel::GroupBuffers {
                        first: &mut moments[effect_offset..effect_offset + directions],
                        first_stride: effect.channels(),
                        second: &mut squares[effect_offset..effect_offset + directions],
                        second_stride: effect.channels(),
                        scratch: &mut contexts,
                    },
                    effect.level_index().membership_offsets(),
                    0,
                    &|first, moments, squares, _| {
                        for (local, (moment, square)) in moments.iter_mut().zip(squares).enumerate()
                        {
                            let level = first + local / effect.channels();
                            let channel = local % effect.channels();
                            let mut total = 0.0;
                            let mut norm = 0.0;
                            for &row in effect.level_index().members(level) {
                                let basis = effect.basis_value(channel, row);
                                total += prior_weights[row] * (outcome[row] - mean[row]) * basis;
                                norm += prior_weights[row] * mean[row] * basis * basis;
                            }
                            *moment = total;
                            *square = norm;
                        }
                        Ok(())
                    },
                )?;
            } else {
                for row in 0..rows {
                    let level = effect.codes()[row] as usize;
                    for channel in 0..effect.channels() {
                        let direction = effect_offset + level * effect.channels() + channel;
                        let basis = effect.basis_value(channel, row);
                        moments[direction] +=
                            prior_weights[row] * (outcome[row] - mean[row]) * basis;
                        squares[direction] += prior_weights[row] * mean[row] * basis * basis;
                    }
                }
            }
            for direction in effect_offset..effect_offset + directions {
                let moment = moments[direction];
                fe_absolute = fe_absolute.max(moment.abs());
                add_relative_score(
                    moment,
                    squares[direction].sqrt(),
                    score_scale,
                    &mut relative_infinity,
                    &mut relative_square,
                    &mut relative_count,
                );
            }
            effect_offset += directions;
        }
        Ok(())
    };
    // Do not submit a serial task: external Rayon submissions can allocate
    // queue blocks inside the required allocation-free iteration window.
    if parallel {
        workspace.team.install(accumulate_effects)?;
    } else {
        accumulate_effects()?;
    }
    let relative_two = if relative_count == 0 {
        0.0
    } else {
        (relative_square.total() / relative_count as f64).sqrt()
    };
    if !finite_absolute.is_finite()
        || !fe_absolute.is_finite()
        || !relative_infinity.is_finite()
        || !relative_two.is_finite()
    {
        return Err(numerical_nonconvergence(
            "prepared PPML score certificate is not finite",
        ));
    }
    Ok(PpmlCertificate {
        finite_absolute_infinity_norm: finite_absolute,
        fixed_effect_absolute_infinity_norm: fe_absolute,
        relative_infinity_norm: relative_infinity,
        relative_two_norm: relative_two,
        passed: relative_infinity <= tolerance && relative_two <= tolerance,
    })
}

fn add_relative_score(
    moment: f64,
    basis_norm: f64,
    score_scale: f64,
    infinity: &mut f64,
    square: &mut CompensatedSum,
    count: &mut usize,
) {
    let denominator = basis_norm * score_scale;
    let relative = if denominator > 0.0 {
        moment.abs() / denominator
    } else if moment == 0.0 {
        0.0
    } else {
        f64::INFINITY
    };
    *infinity = (*infinity).max(relative);
    square.add(relative * relative);
    *count += 1;
}

fn log_weighted_outcome_sum(outcome: &[f64], weights: &[f64]) -> Result<f64> {
    let mut maximum = f64::NEG_INFINITY;
    for (outcome, weight) in outcome.iter().zip(weights) {
        if *outcome > 0.0 {
            maximum = maximum.max(outcome.ln() + weight.ln());
        }
    }
    if !maximum.is_finite() {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "prepared PPML sample has no positive weighted outcome",
        ));
    }
    let mut sum = CompensatedSum::default();
    for (outcome, weight) in outcome.iter().zip(weights) {
        if *outcome > 0.0 {
            sum.add((outcome.ln() + weight.ln() - maximum).exp());
        }
    }
    let value = maximum + sum.total().ln();
    if value.is_finite() {
        Ok(value)
    } else {
        Err(numerical_nonconvergence(
            "prepared PPML weighted outcome sum is not finite",
        ))
    }
}

fn log_weighted_offset_sum(offset: &[f64], weights: &[f64]) -> Result<f64> {
    let mut maximum = f64::NEG_INFINITY;
    for (offset, weight) in offset.iter().zip(weights) {
        maximum = maximum.max(*offset + weight.ln());
    }
    if !maximum.is_finite() {
        return Err(numerical_nonconvergence(
            "prepared PPML weighted offset scale is not finite",
        ));
    }
    let mut sum = CompensatedSum::default();
    for (offset, weight) in offset.iter().zip(weights) {
        sum.add((*offset + weight.ln() - maximum).exp());
    }
    let value = maximum + sum.total().ln();
    if value.is_finite() {
        Ok(value)
    } else {
        Err(numerical_nonconvergence(
            "prepared PPML weighted offset sum is not finite",
        ))
    }
}

fn finite_unit(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < 1.0
}

fn internal(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}

fn numerical_nonconvergence(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

const fn ppml_fallback_kind(kind: FrozenLinearFallbackKind) -> PpmlFallbackKind {
    match kind {
        FrozenLinearFallbackKind::WarmProjection => PpmlFallbackKind::WarmProjection,
        FrozenLinearFallbackKind::Moment => PpmlFallbackKind::Moment,
        FrozenLinearFallbackKind::Rank => PpmlFallbackKind::Rank,
        FrozenLinearFallbackKind::Bread => PpmlFallbackKind::Bread,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::retained_effect_order_by_levels;
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind,
        ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId,
        TermComponent, TermId, TermOrder, TimeSeriesState,
    };
    use crate::ols::{
        inject_frozen_linear_failure, prepare_ols_data, FrozenLinearFailureInjection,
        LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole, OlsRequestV1,
        PreparedOlsData,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn source(id: u32, name: &str) -> SourceEntry {
        SourceEntry {
            id: SourceId::new(id).expect("source"),
            canonical_name: name.to_owned(),
        }
    }

    fn continuous_term(id: u32, source: SourceId) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(id).expect("term"),
            order: TermOrder::new(id).expect("order"),
            coefficient_slot: CoefficientSlot::new(id).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: format!("c.x{id}"),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source,
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.0,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".to_owned(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        }
    }

    fn loaded(values: &[f64]) -> LoadedNumericColumn {
        LoadedNumericColumn {
            values: values.to_vec(),
            missing: vec![false; values.len()],
        }
    }

    fn prepared(outcome: &[f64], regressors: &[Vec<f64>]) -> PreparedOlsData {
        prepared_with_factor_sources(outcome, regressors, 0)
    }

    fn prepared_with_factor_sources(
        outcome: &[f64],
        regressors: &[Vec<f64>],
        factor_count: usize,
    ) -> PreparedOlsData {
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
        for (zero_based, values) in regressors.iter().enumerate() {
            let id = u32::try_from(zero_based + 1).expect("small fixture");
            let entry = source(id, &format!("x{id}"));
            terms.push(continuous_term(id, entry.id));
            bindings.push(OlsInputBinding {
                stata_var_position: id + 2,
                role: OlsInputRole::NumericSource(entry.id),
            });
            numeric_sources.push((entry.id, loaded(values)));
            sources.push(entry);
        }
        let mut request = OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            absorbed_terms: Vec::new(),
            bindings,
            include_intercept: true,
            keep_singletons: true,
        };
        let original_model = request
            .validate_and_compile()
            .expect("numeric fixture model");
        // Use the real factor grammar to create immutable source handles. The
        // score witness then restores its original numeric recipe/slot axis.
        for term in request.model.terms.iter_mut().take(factor_count) {
            term.raw_kind = RawTermKind::Factor;
            term.canonical_token = format!("1.x{}", term.id.get());
            let component = &mut term.components[0];
            component.kind = ComponentKind::Categorical;
            component.level_state = LevelState::Present;
            component.level_value = 1.;
            component.omitted = MetadataFlag::False;
            component.raw_operator = "1".into();
        }
        let mut prepared = prepare_ols_data(
            &request,
            &LoadedOlsInput {
                candidate: vec![true; outcome.len()],
                outcome: loaded(outcome),
                numeric_sources,
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("prepared fixture");
        if factor_count != 0 {
            prepared.model = original_model;
        }
        prepared
    }

    fn prepared_no_fe_categorical(
        outcome: &[f64],
        regressors: &[Vec<f64>],
    ) -> crate::PreparedCategoricalOlsData {
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
        for (zero_based, values) in regressors.iter().enumerate() {
            let id = u32::try_from(zero_based + 1).expect("small fixture");
            let entry = source(id, &format!("x{id}"));
            terms.push(continuous_term(id, entry.id));
            bindings.push(OlsInputBinding {
                stata_var_position: id + 2,
                role: OlsInputRole::NumericSource(entry.id),
            });
            numeric_sources.push((entry.id, loaded(values)));
            sources.push(entry);
        }
        crate::prepared_categorical_data::prepare_ppml_categorical_ols_data(
            &OlsRequestV1 {
                model: CompiledTermGraph { sources, terms },
                absorbed_terms: Vec::new(),
                bindings,
                include_intercept: true,
                keep_singletons: true,
            },
            &LoadedOlsInput {
                candidate: vec![true; outcome.len()],
                outcome: loaded(outcome),
                numeric_sources,
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
            &[],
        )
        .expect("prepared fixture")
    }

    #[test]
    fn original_profile_columns_match_cold_design_across_bindings_and_weight_frames() {
        use super::super::one_fe_profile::ProfiledDesign;
        for (rows, width) in [(80, 3), (4096, 3), (4096, 65)] {
            let y: Vec<_> = (0..rows).map(|r| 0.3 + (r % 13) as f64).collect();
            let x: Vec<Vec<_>> = (0..width)
                .map(|j| {
                    (0..rows)
                        .map(|r| {
                            (r as f64 / (j + 3) as f64).sin() + (r % 17) as f64 * (j + 1) as f64
                        })
                        .collect()
                })
                .collect();
            let prepared = prepared(&y, &x);
            let design = FeDesign::new(
                rows,
                vec![crate::fe::EffectTerm::new(
                    (0..rows).map(|r| (r % 17) as u32).collect(),
                    17,
                    true,
                    vec![],
                )
                .unwrap()],
            )
            .unwrap();
            let active = [2, 0];
            let columns = ProfiledRawColumns::new(&prepared, &active).unwrap();
            for row in 0..rows {
                for (position, &generated) in active.iter().enumerate() {
                    assert_eq!(
                        (&columns).value(row, position).unwrap(),
                        prepared.fit_column_value(generated, row).unwrap()
                    );
                }
            }
            assert!(ProfiledRawColumns::new(&prepared, &[usize::MAX]).is_err());
            let offset: Vec<_> = (0..rows).map(|r| (r as f64 / 19.).cos() * 0.1).collect();
            let coefficients = vec![0.03; prepared.fit_model().coefficients.len()];
            for threads in [1, 4] {
                let threads = crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                for frame in [1, 7] {
                    let weights: Vec<_> =
                        (0..rows).map(|r| 0.2 + ((r * frame) % 13) as f64).collect();
                    let mut profile =
                        ProfiledOneFe::new(&prepared, &active, &design, &y, &weights, threads)
                            .unwrap();
                    profile
                        .prepare_direction(
                            &columns,
                            &design,
                            &y,
                            &weights,
                            &offset,
                            &coefficients,
                            PpmlConfig::default(),
                        )
                        .unwrap();
                    assert!(profile
                        .fresh_direction_equivalent(
                            &prepared,
                            &design,
                            &y,
                            &weights,
                            &offset,
                            &coefficients,
                            threads,
                            PpmlConfig::default(),
                            1e-12
                        )
                        .unwrap());
                }
            }
        }
    }

    #[test]
    fn numeric_outer_scores_preserve_every_certificate_bit_across_teams_and_weights() {
        let rows = 4096;
        let y: Vec<_> = (0..rows).map(|r| (r % 13) as f64 / 3.0).collect();
        let mu: Vec<_> = (0..rows)
            .map(|r| (0.4 * (r as f64 / 91.0).sin()).exp())
            .collect();
        let outer = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        for width in [1, 6, 63, 64, 65] {
            let x: Vec<Vec<_>> = (0..width)
                .map(|j| {
                    (0..rows)
                        .map(|r| (r as f64 / (j + 3) as f64).sin() + 0.1 * (r % 17) as f64)
                        .collect()
                })
                .collect();
            let prepared = prepared(&y, &x);
            let design = FeDesign::new(
                rows,
                vec![crate::fe::EffectTerm::new(
                    (0..rows).map(|r| (r % 17) as u32).collect(),
                    17,
                    true,
                    vec![x[0].clone()],
                )
                .unwrap()],
            )
            .unwrap();
            for scale in [1e-30, 1.0, 1e30] {
                let q: Vec<_> = (0..rows)
                    .map(|r| scale * (0.25 + (r % 11) as f64))
                    .collect();
                let mut expected = PpmlScoreWorkspace::new(
                    &design,
                    crate::options::ThreadConfig::from_requested(1, true).unwrap(),
                )
                .unwrap();
                FORCE_GENERIC_OUTER_SCORE.set(true);
                let baseline = prepared_score_certificate(
                    &prepared,
                    &design,
                    &y,
                    &mu,
                    &q,
                    1e-8,
                    &mut expected,
                );
                FORCE_GENERIC_OUTER_SCORE.set(false);
                let baseline = baseline.unwrap();
                for threads in [1, 4, 8] {
                    let mut workspace = PpmlScoreWorkspace::new(
                        &design,
                        crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
                    )
                    .unwrap();
                    let actual = outer
                        .install(|| {
                            prepared_score_certificate(
                                &prepared,
                                &design,
                                &y,
                                &mu,
                                &q,
                                1e-8,
                                &mut workspace,
                            )
                        })
                        .unwrap();
                    assert_eq!(actual, baseline);
                    assert_eq!(workspace.fe_moments, expected.fe_moments);
                    assert_eq!(workspace.fe_basis_squares, expected.fe_basis_squares);
                }
            }
        }
    }

    #[test]
    fn generated_outer_scores_preserve_fresh_certificates_across_tiles_frames_and_teams() {
        use crate::ols::{PreparedOlsCategoricalComponent, PreparedOlsColumnKind};
        for rows in [4095, 4097] {
            for width in [8, 65, 71] {
                let y: Vec<_> = (0..rows).map(|r| (r % 13) as f64 / 3.).collect();
                let x: Vec<Vec<_>> = (0..width)
                    .map(|j| {
                        (0..rows)
                            .map(|r| {
                                if j < 2 {
                                    ((r / (j + 1)) % (j + 3)) as f64
                                } else {
                                    (r as f64 / (j + 3) as f64).sin()
                                }
                            })
                            .collect()
                    })
                    .collect();
                let mut prepared = prepared_with_factor_sources(&y, &x, 2);
                for j in 2..width {
                    let component = |source_index: usize, code| PreparedOlsCategoricalComponent {
                        source: SourceId::new((source_index + 1) as u32).unwrap(),
                        categorical_source_index: source_index,
                        level_bits: 1f64.to_bits(),
                        level_code: code,
                    };
                    prepared.model.columns[j].kind = match j % 3 {
                        0 => PreparedOlsColumnKind::Categorical {
                            source: SourceId::new(1).unwrap(),
                            prepared_source_index: 0,
                            categorical_source_index: 0,
                            level_bits: 1f64.to_bits(),
                            level_code: Some(1),
                        },
                        1 => PreparedOlsColumnKind::CategoricalInteraction {
                            components: vec![
                                component(0, Some(1)),
                                component(1, Some(2)),
                                component(0, if j % 7 == 0 { None } else { Some(1) }),
                            ],
                        },
                        _ => prepared.model.columns[j].kind.clone(),
                    };
                }
                let design = FeDesign::new(
                    rows,
                    vec![crate::fe::EffectTerm::new(
                        (0..rows).map(|r| (r % 17) as u32).collect(),
                        17,
                        true,
                        vec![x[2].clone()],
                    )
                    .unwrap()],
                )
                .unwrap();
                for frame in [1, 7] {
                    let mu: Vec<_> = (0..rows)
                        .map(|r| (0.4 * (r as f64 / (91 * frame) as f64).sin()).exp())
                        .collect();
                    let q: Vec<_> = (0..rows)
                        .map(|r| [1e-30, 1., 1e30][r % 3] * (0.25 + ((r * frame) % 11) as f64))
                        .collect();
                    let mut reference = PpmlScoreWorkspace::new(
                        &design,
                        crate::options::ThreadConfig::from_requested(1, true).unwrap(),
                    )
                    .unwrap();
                    FORCE_GENERIC_OUTER_SCORE.set(true);
                    let expected = prepared_score_certificate(
                        &prepared,
                        &design,
                        &y,
                        &mu,
                        &q,
                        1e-8,
                        &mut reference,
                    );
                    FORCE_GENERIC_OUTER_SCORE.set(false);
                    let expected = expected.unwrap();
                    for threads in [1, 4, 8] {
                        let mut workspace = PpmlScoreWorkspace::new(
                            &design,
                            crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
                        )
                        .unwrap();
                        let actual = prepared_score_certificate(
                            &prepared,
                            &design,
                            &y,
                            &mu,
                            &q,
                            1e-8,
                            &mut workspace,
                        )
                        .unwrap();
                        assert_eq!(actual, expected);
                        assert_eq!(workspace.fe_moments, reference.fe_moments);
                        assert_eq!(workspace.fe_basis_squares, reference.fe_basis_squares);
                    }
                }
            }
        }
    }

    #[test]
    fn independent_fe_scores_match_integer_design_across_frames_and_empty_directions() {
        use crate::fe::EffectTerm;
        for rows in [4097, 16385] {
            let y: Vec<_> = (0..rows)
                .map(|r| if r % 3 == 0 { 0. } else { 2. })
                .collect();
            let z: Vec<_> = (0..rows).map(|r| (r % 5) as f64 - 2.).collect();
            let w: Vec<_> = (0..rows).map(|r| (r % 7) as f64 - 3.).collect();
            let x: Vec<_> = (0..rows).map(|r| (r as f64 / 17.).sin()).collect();
            let prepared = prepared(&y, &[x]);
            let first: Vec<_> = (0..rows)
                .map(|r| (12 * (r % 2) + if r % 97 < 80 { 0 } else { 1 + (r / 2) % 7 }) as u32)
                .collect();
            let second: Vec<_> = (0..rows)
                .map(|r| (5 * (r % 2) + (r / 2) % 3) as u32)
                .collect();
            let third: Vec<_> = (0..rows).map(|r| (r % 2) as u32).collect();
            // Two disconnected components, a dominant group, unused labels,
            // duplicate slopes, a slope-only effect and an identically zero
            // direction exercise ownership without granting any rank authority.
            let specs = [
                (first, 24, true, vec![z.clone(), z]),
                (second, 10, false, vec![w, vec![0.; rows]]),
                (third, 3, true, vec![]),
            ];
            let mut explicit = Vec::new();
            let mut effects = Vec::new();
            for (codes, levels, intercept, slopes) in specs {
                let channels = usize::from(intercept) + slopes.len();
                for level in 0..levels {
                    for channel in 0..channels {
                        explicit.push(
                            (0..rows)
                                .map(|r| {
                                    if codes[r] as usize != level {
                                        0_i64
                                    } else if intercept && channel == 0 {
                                        1
                                    } else {
                                        slopes[channel - usize::from(intercept)][r] as i64
                                    }
                                })
                                .collect::<Vec<_>>(),
                        );
                    }
                }
                effects.push(EffectTerm::new(codes, levels, intercept, slopes).unwrap());
            }
            let design = FeDesign::new(rows, effects).unwrap();
            for threads in [1, 4, 8] {
                let mut workspace = PpmlScoreWorkspace::new(
                    &design,
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
                )
                .unwrap();
                let storage = (
                    workspace.fe_moments.as_ptr(),
                    workspace.fe_moments.capacity(),
                    workspace.fe_basis_squares.as_ptr(),
                    workspace.fe_basis_squares.capacity(),
                );
                for frame in [1, 7] {
                    let mu: Vec<_> = (0..rows).map(|r| (1 + (r + frame) % 2) as f64).collect();
                    let q: Vec<_> = (0..rows).map(|r| (1 + (r * frame) % 7) as i64).collect();
                    // Integer explicit-design dot products are an independent
                    // exact oracle; binary common scales remain exactly representable.
                    let expected: Vec<_> = explicit
                        .iter()
                        .map(|column| {
                            let mut score = 0_i64;
                            let mut square = 0_i64;
                            for r in 0..rows {
                                score += q[r] * (y[r] as i64 - mu[r] as i64) * column[r];
                                square += q[r] * mu[r] as i64 * column[r] * column[r];
                            }
                            (score, square)
                        })
                        .collect();
                    for exponent in [-400, 0, 400] {
                        let scale = 2_f64.powi(exponent);
                        let weights: Vec<_> = q.iter().map(|&value| value as f64 * scale).collect();
                        workspace.fe_moments.fill(f64::NAN);
                        workspace.fe_basis_squares.fill(f64::NAN);
                        let actual = prepared_score_certificate(
                            &prepared,
                            &design,
                            &y,
                            &mu,
                            &weights,
                            1e-8,
                            &mut workspace,
                        )
                        .unwrap();
                        assert!(!actual.passed);
                        for (direction, &(score, square)) in expected.iter().enumerate() {
                            assert_eq!(workspace.fe_moments[direction], score as f64 * scale);
                            assert_eq!(
                                workspace.fe_basis_squares[direction],
                                square as f64 * scale
                            );
                        }
                        assert_eq!(
                            (
                                workspace.fe_moments.as_ptr(),
                                workspace.fe_moments.capacity(),
                                workspace.fe_basis_squares.as_ptr(),
                                workspace.fe_basis_squares.capacity()
                            ),
                            storage
                        );
                    }
                }
            }
        }
    }

    fn prepared_two_fe(outcome: &[f64], regressor: &[f64]) -> PreparedOlsData {
        assert_eq!(outcome.len(), 8);
        prepared_two_fe_with_codes(
            outcome,
            regressor,
            &[1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0],
            &[1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0],
        )
    }

    fn prepared_two_fe_with_codes(
        outcome: &[f64],
        regressor: &[f64],
        first: &[f64],
        second: &[f64],
    ) -> PreparedOlsData {
        assert_eq!(regressor.len(), outcome.len());
        let x = source(1, "x");
        let g = source(2, "g");
        let h = source(3, "h");
        prepare_ols_data(
            &OlsRequestV1 {
                model: CompiledTermGraph {
                    sources: vec![x.clone(), g.clone(), h.clone()],
                    terms: vec![continuous_term(1, x.id)],
                },
                absorbed_terms: vec![
                    AbsorbedTerm {
                        id: AbsorbTermId::new(1).expect("g absorb id"),
                        intercept: true,
                        categorical_sources: vec![g.id],
                        slope_sources: Vec::new(),
                        label: "g".to_owned(),
                        saved_name: String::new(),
                    },
                    AbsorbedTerm {
                        id: AbsorbTermId::new(2).expect("h absorb id"),
                        intercept: true,
                        categorical_sources: vec![h.id],
                        slope_sources: Vec::new(),
                        label: "h".to_owned(),
                        saved_name: String::new(),
                    },
                ],
                bindings: vec![
                    OlsInputBinding {
                        stata_var_position: 1,
                        role: OlsInputRole::Candidate,
                    },
                    OlsInputBinding {
                        stata_var_position: 2,
                        role: OlsInputRole::Outcome,
                    },
                    OlsInputBinding {
                        stata_var_position: 3,
                        role: OlsInputRole::NumericSource(x.id),
                    },
                    OlsInputBinding {
                        stata_var_position: 4,
                        role: OlsInputRole::NumericSource(g.id),
                    },
                    OlsInputBinding {
                        stata_var_position: 5,
                        role: OlsInputRole::NumericSource(h.id),
                    },
                ],
                include_intercept: true,
                keep_singletons: true,
            },
            &LoadedOlsInput {
                candidate: vec![true; outcome.len()],
                outcome: loaded(outcome),
                numeric_sources: vec![
                    (x.id, loaded(regressor)),
                    (g.id, loaded(first)),
                    (h.id, loaded(second)),
                ],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("prepared two-FE fixture")
    }

    fn prepared_joint_slopes(
        outcome: &[f64],
        regressor: &[f64],
        first: &[f64],
        second: &[f64],
    ) -> crate::PreparedCategoricalOlsData {
        assert_eq!(regressor.len(), outcome.len());
        let z = source(4, "z");
        let w = source(5, "w");
        let slopes: Vec<f64> = (0..outcome.len()).map(|i| (i % 5) as f64 - 2.).collect();
        let x = source(1, "x");
        let g = source(2, "g");
        let h = source(3, "h");
        crate::prepare_categorical_ols_data(
            &OlsRequestV1 {
                model: CompiledTermGraph {
                    sources: vec![x.clone(), g.clone(), h.clone(), z.clone(), w.clone()],
                    terms: vec![continuous_term(1, x.id)],
                },
                absorbed_terms: vec![
                    AbsorbedTerm {
                        id: AbsorbTermId::new(1).expect("g absorb id"),
                        intercept: true,
                        categorical_sources: vec![g.id],
                        slope_sources: vec![z.id],
                        label: "g".to_owned(),
                        saved_name: String::new(),
                    },
                    AbsorbedTerm {
                        id: AbsorbTermId::new(2).expect("h absorb id"),
                        intercept: true,
                        categorical_sources: vec![h.id],
                        slope_sources: vec![w.id],
                        label: "h".to_owned(),
                        saved_name: String::new(),
                    },
                ],
                bindings: vec![
                    OlsInputBinding {
                        stata_var_position: 1,
                        role: OlsInputRole::Candidate,
                    },
                    OlsInputBinding {
                        stata_var_position: 2,
                        role: OlsInputRole::Outcome,
                    },
                    OlsInputBinding {
                        stata_var_position: 3,
                        role: OlsInputRole::NumericSource(x.id),
                    },
                    OlsInputBinding {
                        stata_var_position: 4,
                        role: OlsInputRole::NumericSource(g.id),
                    },
                    OlsInputBinding {
                        stata_var_position: 5,
                        role: OlsInputRole::NumericSource(h.id),
                    },
                    OlsInputBinding {
                        stata_var_position: 6,
                        role: OlsInputRole::NumericSource(z.id),
                    },
                    OlsInputBinding {
                        stata_var_position: 7,
                        role: OlsInputRole::NumericSource(w.id),
                    },
                ],
                include_intercept: true,
                keep_singletons: true,
            },
            &LoadedOlsInput {
                candidate: vec![true; outcome.len()],
                outcome: loaded(outcome),
                numeric_sources: vec![
                    (x.id, loaded(regressor)),
                    (g.id, loaded(first)),
                    (h.id, loaded(second)),
                    (z.id, loaded(&slopes)),
                    (w.id, loaded(&slopes)),
                ],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("prepared two-FE fixture")
    }

    #[test]
    fn refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model() {
        for (hot, mode) in [
            (30.0_f64, PreparedOlsExecutionMode::Low),
            (30., PreparedOlsExecutionMode::Fast),
        ] {
            let mut x = Vec::new();
            let mut g = Vec::new();
            let mut h = Vec::new();
            let mut truth = Vec::new();
            let mut initial = Vec::new();
            for worker in 0..40 {
                for repeat in 0..4 {
                    let firm = if worker < 8 {
                        (worker + repeat % 2) % 8
                    } else {
                        8 + (worker - 8 + repeat % 2) % 32
                    };
                    let value = ((worker * 2 + repeat % 2) as f64 * 1.31).sin();
                    x.push(value);
                    g.push(worker as f64);
                    h.push(firm as f64);
                    let eta = if worker < 8 { hot } else { 1.0 }
                        + 0.2 * value
                        + 0.3 * (firm as f64).cos();
                    truth.push(eta);
                    initial.push(
                        eta + if worker < 8 {
                            0.0
                        } else {
                            0.4 * (worker as f64).sin()
                        },
                    );
                }
            }
            let y: Vec<_> = truth
                .iter()
                .enumerate()
                .map(|(row, eta)| eta.exp() * if row % 4 < 2 { 0.9 } else { 1.1 })
                .collect();
            let prepared = prepared_two_fe_with_codes(&y, &x, &g, &h);
            let mut config = PreparedPpmlIrlsConfig::default();
            config.ppml.max_iterations = 6;
            config.memory.requested = if mode == PreparedOlsExecutionMode::Low {
                crate::options::MemoryMode::Low
            } else {
                crate::options::MemoryMode::Fast
            };
            config.memory.poolsize = 1;
            config.linear.execution.mode = mode;
            config.linear.execution.poolsize = 1;
            let start = PreparedPpmlStart {
                coefficient_values: vec![0.2, 0.0],
                eta: initial,
            };
            // Newer proposal paths can reach this known optimum without a
            // rejection. Exercise checkpoint refinement explicitly rather
            // than depending on an obsolete iteration trajectory.
            inject_strict_resume(true, false);
            let fitted = fit_prepared_ppml_irls(
                &prepared,
                &y,
                &vec![1.0; y.len()],
                &vec![0.0; y.len()],
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                Some(&start),
            );
            let fit = fitted.expect("refined ordinary solves must preserve strict progress");
            assert!(fit.diagnostics().converged);
            assert_eq!(fit.diagnostics().strict_profile_rejections, 1);
            assert!(
                relative_infinity_difference(fit.eta(), &truth) < 1e-10,
                "mode={mode:?} eta_error={:.17e}",
                relative_infinity_difference(fit.eta(), &truth)
            );
            assert!(fit.diagnostics().iterations <= 6);
        }
    }
    #[test]
    fn canonical_final_preserves_accuracy_on_a_small_dynamic_range_model() {
        for (hot, mode) in [
            (30.0_f64, PreparedOlsExecutionMode::Low),
            (30., PreparedOlsExecutionMode::Fast),
        ] {
            let mut x = Vec::new();
            let mut g = Vec::new();
            let mut h = Vec::new();
            let mut truth = Vec::new();
            let mut initial = Vec::new();
            for worker in 0..40 {
                for repeat in 0..4 {
                    let firm = if worker < 8 {
                        (worker + repeat % 2) % 8
                    } else {
                        8 + (worker - 8 + repeat % 2) % 32
                    };
                    let value = ((worker * 2 + repeat % 2) as f64 * 1.31).sin();
                    x.push(value);
                    g.push(worker as f64);
                    h.push(firm as f64);
                    let eta = if worker < 8 { hot } else { 1.0 }
                        + 0.2 * value
                        + 0.3 * (firm as f64).cos();
                    truth.push(eta);
                    initial.push(
                        eta + if worker < 8 {
                            0.0
                        } else {
                            0.4 * (worker as f64).sin()
                        },
                    );
                }
            }
            let y: Vec<_> = truth
                .iter()
                .enumerate()
                .map(|(row, eta)| eta.exp() * if row % 4 < 2 { 0.9 } else { 1.1 })
                .collect();
            let prepared = prepared_two_fe_with_codes(&y, &x, &g, &h);
            let mut config = PreparedPpmlIrlsConfig::default();
            config.ppml.max_iterations = 6;
            config.memory.requested = if mode == PreparedOlsExecutionMode::Low {
                crate::options::MemoryMode::Low
            } else {
                crate::options::MemoryMode::Fast
            };
            config.memory.poolsize = 1;
            config.linear.execution.mode = mode;
            config.linear.execution.poolsize = 1;
            let start = PreparedPpmlStart {
                coefficient_values: vec![0.2, 0.0],
                eta: initial,
            };
            let fitted = fit_prepared_ppml_irls(
                &prepared,
                &y,
                &vec![1.0; y.len()],
                &vec![0.0; y.len()],
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                Some(&start),
            );
            let fit = fitted.expect("refined ordinary solves must preserve strict progress");
            assert!(fit.diagnostics().converged);
            assert!(
                relative_infinity_difference(fit.eta(), &truth) < 1e-10,
                "mode={mode:?} eta_error={:.17e}",
                relative_infinity_difference(fit.eta(), &truth)
            );
            assert!(fit.diagnostics().iterations <= 6);
        }
    }

    fn prepared_three_fe(
        order: [usize; 3],
        zero_control: bool,
        nested_in: Option<usize>,
    ) -> (PreparedOlsData, Vec<f64>) {
        let rows = 240;
        let x_values = (0..rows)
            .map(|row| {
                if zero_control {
                    0.0
                } else {
                    (row as f64 * 0.173).sin() + (row as f64 * 0.071).cos()
                }
            })
            .collect::<Vec<_>>();
        let mut categorical = [
            (0..rows)
                .map(|row| (row % 24 + 1) as f64)
                .collect::<Vec<_>>(),
            (0..rows)
                .map(|row| ((row * 5 + row / 24) % 9 + 1) as f64)
                .collect::<Vec<_>>(),
            (0..rows)
                .map(|row| ((row * 7 + row / 11) % 4 + 1) as f64)
                .collect::<Vec<_>>(),
        ];
        if let Some(core) = nested_in {
            categorical[2] = categorical[core]
                .iter()
                .map(|value| 1.0 + ((*value - 1.0) / 3.0).floor())
                .collect();
        }
        let outcome = (0..rows)
            .map(|row| {
                1.0 + ((row * 17
                    + categorical[0][row] as usize * 3
                    + categorical[1][row] as usize * 5
                    + categorical[2][row] as usize * 7)
                    % 11) as f64
            })
            .collect::<Vec<_>>();
        let x = source(1, "x");
        let effects = [source(2, "large"), source(3, "medium"), source(4, "small")];
        let labels = ["large", "medium", "small"];
        let absorbed_terms = order
            .iter()
            .enumerate()
            .map(|(position, &effect)| AbsorbedTerm {
                id: AbsorbTermId::new(u32::try_from(position + 1).expect("small position"))
                    .expect("absorb id"),
                intercept: true,
                categorical_sources: vec![effects[effect].id],
                slope_sources: Vec::new(),
                label: labels[effect].to_owned(),
                saved_name: format!("__fe_{}", labels[effect]),
            })
            .collect();
        let prepared = prepare_ols_data(
            &OlsRequestV1 {
                model: CompiledTermGraph {
                    sources: vec![
                        x.clone(),
                        effects[0].clone(),
                        effects[1].clone(),
                        effects[2].clone(),
                    ],
                    terms: vec![continuous_term(1, x.id)],
                },
                absorbed_terms,
                bindings: vec![
                    OlsInputBinding {
                        stata_var_position: 1,
                        role: OlsInputRole::Candidate,
                    },
                    OlsInputBinding {
                        stata_var_position: 2,
                        role: OlsInputRole::Outcome,
                    },
                    OlsInputBinding {
                        stata_var_position: 3,
                        role: OlsInputRole::NumericSource(x.id),
                    },
                    OlsInputBinding {
                        stata_var_position: 4,
                        role: OlsInputRole::NumericSource(effects[0].id),
                    },
                    OlsInputBinding {
                        stata_var_position: 5,
                        role: OlsInputRole::NumericSource(effects[1].id),
                    },
                    OlsInputBinding {
                        stata_var_position: 6,
                        role: OlsInputRole::NumericSource(effects[2].id),
                    },
                ],
                include_intercept: true,
                keep_singletons: true,
            },
            &LoadedOlsInput {
                candidate: vec![true; rows],
                outcome: loaded(&outcome),
                numeric_sources: vec![
                    (x.id, loaded(&x_values)),
                    (effects[0].id, loaded(&categorical[0])),
                    (effects[1].id, loaded(&categorical[1])),
                    (effects[2].id, loaded(&categorical[2])),
                ],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("prepared three-FE fixture");
        (prepared, outcome)
    }

    #[test]
    fn explicit_three_fe_solver_roles_are_permutation_invariant_and_certified() {
        let orders = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let mut reference_eta: Option<Vec<f64>> = None;
        let mut reference_mean: Option<Vec<f64>> = None;
        let mut reference_objective: Option<PoissonObjective> = None;

        for family in [
            super::super::PpmlSolverFamily::Lsmr,
            super::super::PpmlSolverFamily::Cmg,
        ] {
            for order in orders {
                let (prepared, outcome) = prepared_three_fe(order, false, None);
                let design = prepared.fit_owned_fe_design().expect("three-FE design");
                let selected = retained_effect_order_by_levels(design.effects().len(), |effect| {
                    design.effects()[effect].n_levels()
                })
                .expect("effect role order");
                let selected_original = [order[selected[0]], order[selected[1]]];
                assert_eq!(selected_original, [0, 1]);

                let mut config = PreparedPpmlIrlsConfig {
                    solver_plan: PpmlSolverPlan::new(family),
                    ..PreparedPpmlIrlsConfig::default()
                };
                config.linear.execution.threads =
                    crate::options::ThreadConfig::from_requested(2, true).expect("threads");
                config.ppml.absorb.lsmr.threads = config.linear.execution.threads;
                config.linear.absorb.lsmr.threads = config.linear.execution.threads;
                let prior_weights = vec![1.0; outcome.len()];
                let offset = vec![0.0; outcome.len()];
                let fit = fit_prepared_ppml_irls(
                    &prepared,
                    &outcome,
                    &prior_weights,
                    &offset,
                    config,
                    plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                        .expect("three-FE memory plan"),
                    None,
                )
                .expect("permuted three-FE fit");
                assert!(fit.diagnostics().converged);
                assert!(fit.diagnostics().two_consecutive_convergence);
                assert!(fit.diagnostics().final_score.passed);
                assert_eq!(fit.diagnostics().solver.effective, family);
                assert_eq!(fit.diagnostics().solver.fallback_count, 0);

                if let (Some(eta), Some(mean), Some(objective)) =
                    (&reference_eta, &reference_mean, reference_objective)
                {
                    assert!(relative_infinity_difference(fit.eta(), eta) <= 2e-9);
                    assert!(relative_infinity_difference(fit.mean(), mean) <= 2e-9);
                    assert!(
                        (fit.objective().deviance - objective.deviance).abs()
                            <= 2e-9 * objective.deviance.abs().max(1.0)
                    );
                    assert!(
                        (fit.objective().log_likelihood - objective.log_likelihood).abs()
                            <= 2e-9 * objective.log_likelihood.abs().max(1.0)
                    );
                    assert!(fit.diagnostics().final_score.relative_infinity_norm <= 1e-8);
                    assert!(fit.diagnostics().final_score.relative_two_norm <= 1e-8);
                } else {
                    reference_eta = Some(fit.eta().to_vec());
                    reference_mean = Some(fit.mean().to_vec());
                    reference_objective = Some(fit.objective());
                }
            }
        }
    }

    #[test]
    fn cmg_warm_seed_requires_complete_certification_and_survives_no_failed_frame() {
        use super::super::categorical_fwl_cmg as cmg;
        let (prepared, response) = prepared_three_fe([2, 0, 1], false, None);
        let design = prepared.fit_owned_fe_design().unwrap();
        let config = FusedFwlCmgConfig {
            map: crate::fe::MapConfig::default(),
            threads: crate::options::ThreadConfig::from_requested(2, true).unwrap(),
            batch_size: 2,
            solver_memory_budget: FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: u128::MAX,
            },
        };
        let mut engine = ReusableCategoricalFwlCmg::new(&prepared, &[0], &design, config).unwrap();
        let weights = vec![1.0; response.len()];
        for failure in 0..7 {
            assert_eq!(engine.warm_seed_ready(), (false, false));
            assert!(matches!(
                engine
                    .solve(&prepared, &design, &response, &weights, 1e-8)
                    .unwrap(),
                FusedFwlCmgSolveOutcome::Certified(_)
            ));
            assert_eq!(engine.warm_seed_ready(), (true, true));
            let expected = engine.expected_heap_peak(1024).unwrap();
            assert!(expected.expected_heap_bytes().unwrap() >= 1024);
            assert!(expected.expected_heap_bytes().unwrap() < 16 * 1024 * 1024);
            match failure {
                0 => {
                    cmg::inject_projection_certificate_failure_once(0);
                    let FusedFwlCmgSolveOutcome::Certified(report) = engine
                        .solve(&prepared, &design, &response, &weights, 1e-8)
                        .unwrap()
                    else {
                        panic!("retry must certify")
                    };
                    assert_eq!(report.retried_right_hand_sides, 1);
                }
                1 => {
                    cmg::inject_post_projection_numerical_failure_once();
                    assert!(matches!(
                        engine
                            .solve(&prepared, &design, &response, &weights, 1e-8)
                            .unwrap(),
                        FusedFwlCmgSolveOutcome::NumericalFailure(_)
                    ));
                }
                2..=4 => {
                    let code = [
                        ErrorCode::Allocation,
                        ErrorCode::Interrupted,
                        ErrorCode::Internal,
                    ][failure - 2];
                    cmg::inject_fatal_failure_once(code);
                    assert_eq!(
                        engine
                            .solve(&prepared, &design, &response, &weights, 1e-8)
                            .unwrap_err()
                            .code,
                        code
                    );
                }
                5 => {
                    let changed = prepared.fit_owned_fe_design().unwrap();
                    assert_eq!(
                        engine
                            .solve(&prepared, &changed, &response, &weights, 1e-8)
                            .unwrap_err()
                            .code,
                        ErrorCode::InvalidArgument
                    );
                }
                _ => engine.reset_warm_start(),
            }
            assert_eq!(
                engine.warm_seed_ready(),
                (false, false),
                "failed/retried/checkpoint frame cannot promote a seed"
            );
        }
        let replacement = ReusableCategoricalFwlCmg::new(&prepared, &[], &design, config).unwrap();
        assert_eq!(
            replacement.warm_seed_ready(),
            (false, false),
            "a replacement active basis starts cold"
        );
    }

    #[test]
    fn cmg_nuisance_batches_preserve_weighted_fits_and_deterministic_certificates() {
        for zero_control in [false, true] {
            let (prepared, outcome) = prepared_three_fe([2, 0, 1], zero_control, None);
            let prior_weights = (0..outcome.len())
                .map(|row| 0.5 + (row % 7) as f64 / 3.0)
                .collect::<Vec<_>>();
            let offset = (0..outcome.len())
                .map(|row| (row as f64 * 0.03).sin() * 0.1)
                .collect::<Vec<_>>();
            let mut reference: Option<PreparedPpmlIrlsFit> = None;
            let mut deterministic: Option<PreparedPpmlIrlsFit> = None;
            for (poolsize, threads) in [(1, 1), (7, 1), (7, 2), (7, 7), (7, 7)] {
                let mut config = PreparedPpmlIrlsConfig {
                    solver_plan: PpmlSolverPlan::new(super::super::PpmlSolverFamily::Cmg),
                    ..PreparedPpmlIrlsConfig::default()
                };
                config.memory.poolsize = poolsize;
                config.linear.execution.threads =
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                config.ppml.absorb.lsmr.threads = config.linear.execution.threads;
                config.linear.absorb.lsmr.threads = config.linear.execution.threads;
                let fit = fit_prepared_ppml_irls(
                    &prepared,
                    &outcome,
                    &prior_weights,
                    &offset,
                    config,
                    plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                    None,
                )
                .expect("weighted CMG fit with a nuisance block wider than K+1");
                assert!(fit.diagnostics().converged);
                assert!(fit.diagnostics().two_consecutive_convergence);
                assert!(fit.diagnostics().final_score.passed);
                assert_eq!(fit.diagnostics().solver.fallback_count, 0);
                assert_eq!(
                    fit.diagnostics().solver.effective,
                    super::super::PpmlSolverFamily::Cmg
                );
                if let Some(reference) = &reference {
                    assert!(relative_infinity_difference(fit.eta(), reference.eta()) <= 2e-9);
                    assert!(relative_infinity_difference(fit.mean(), reference.mean()) <= 2e-9);
                    assert!(
                        (fit.objective().deviance - reference.objective().deviance).abs() <= 1e-8
                    );
                } else {
                    reference = Some(fit);
                    continue;
                }
                if threads == 7 {
                    if let Some(previous) = &deterministic {
                        assert_eq!(fit.eta(), previous.eta());
                        assert_eq!(fit.mean(), previous.mean());
                        assert_eq!(fit.objective(), previous.objective());
                        assert_eq!(
                            fit.diagnostics().final_score,
                            previous.diagnostics().final_score
                        );
                        let report = &fit.diagnostics().solver.engines()[3];
                        let previous = &previous.diagnostics().solver.engines()[3];
                        assert_eq!(report.iterations, previous.iterations);
                        assert_eq!(report.retries, previous.retries);
                        assert_eq!(report.ordinary_frames, previous.ordinary_frames);
                    }
                    deterministic = Some(fit);
                }
            }
        }
    }

    #[test]
    fn cmg_nested_nuisance_preserves_full_weighted_checkpoint_and_retry_certificates() {
        use super::super::PpmlSolverFamily;
        for core in [0, 1] {
            for zero_control in [false, true] {
                let mut reference: Option<PreparedPpmlIrlsFit> = None;
                for (order, family, retry) in [
                    ([0, 1, 2], PpmlSolverFamily::Map, false),
                    ([0, 1, 2], PpmlSolverFamily::Cmg, false),
                    ([2, 0, 1], PpmlSolverFamily::Cmg, false),
                    ([1, 2, 0], PpmlSolverFamily::Cmg, true),
                ] {
                    let (prepared, outcome) = prepared_three_fe(order, zero_control, Some(core));
                    let weights = (0..outcome.len())
                        .map(|row| 0.5 + (row % 7) as f64 / 3.0)
                        .collect::<Vec<_>>();
                    let offset = (0..outcome.len())
                        .map(|row| 0.1 * (row as f64 / 17.0).sin())
                        .collect::<Vec<_>>();
                    let mut config = PreparedPpmlIrlsConfig {
                        solver_plan: PpmlSolverPlan::new(family),
                        ..PreparedPpmlIrlsConfig::default()
                    };
                    config.linear.execution.threads =
                        crate::options::ThreadConfig::from_requested(2, true).unwrap();
                    config.ppml.absorb.lsmr.threads = config.linear.execution.threads;
                    config.linear.absorb.lsmr.threads = config.linear.execution.threads;
                    if retry {
                        super::super::categorical_fwl_cmg::inject_projection_certificate_failure_once(0);
                    }
                    let fit = fit_prepared_ppml_irls(
                        &prepared,
                        &outcome,
                        &weights,
                        &offset,
                        config,
                        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                            .unwrap(),
                        None,
                    )
                    .expect("nested CMG fit");
                    assert!(fit.diagnostics().converged);
                    assert!(fit.diagnostics().two_consecutive_convergence);
                    assert!(fit.diagnostics().final_score.passed);
                    assert_eq!(fit.diagnostics().solver.effective, family);
                    assert_eq!(fit.diagnostics().solver.fallback_count, 0);
                    if retry {
                        assert!(fit.diagnostics().solver.engines()[3].retries >= 1);
                    }
                    if let Some(reference) = &reference {
                        assert!(relative_infinity_difference(fit.eta(), reference.eta()) <= 2e-9);
                        assert!(relative_infinity_difference(fit.mean(), reference.mean()) <= 2e-9);
                        assert!(
                            (fit.objective().deviance - reference.objective().deviance).abs()
                                <= 1e-8
                        );
                        assert!(
                            (fit.objective().log_likelihood - reference.objective().log_likelihood)
                                .abs()
                                <= 1e-8
                        );
                    } else {
                        reference = Some(fit);
                    }
                }
            }
        }
    }

    #[test]
    fn cmg_nuisance_batch_memory_is_admitted_before_any_canonical_work() {
        let (prepared, outcome) = prepared_three_fe([2, 0, 1], true, None);
        let mut config = PreparedPpmlIrlsConfig {
            solver_plan: PpmlSolverPlan::new(super::super::PpmlSolverFamily::Cmg),
            ..PreparedPpmlIrlsConfig::default()
        };
        config.linear.execution.threads =
            crate::options::ThreadConfig::from_requested(7, true).unwrap();
        config.memory.poolsize = 1;
        let dimensions = memory_dimensions(&prepared, config);
        let rhs = dimensions.finite_columns + 1;
        let small_plan = plan_ppml_memory(dimensions, config.memory).unwrap();
        let small = public_solver_memory_admission(
            &prepared,
            small_plan,
            &config,
            rhs,
            PreparedPpmlIrlsPurpose::Publication,
        )
        .unwrap();
        config.memory.poolsize = 7;
        let mut plan = plan_ppml_memory(dimensions, config.memory).unwrap();
        let wide = public_solver_memory_admission(
            &prepared,
            plan,
            &config,
            rhs,
            PreparedPpmlIrlsPurpose::Publication,
        )
        .unwrap();
        assert!(wide.incremental_bytes > small.incremental_bytes);
        assert_eq!(small.cmg_batch_capacity, 1);
        assert_eq!(wide.cmg_batch_capacity, 3);
        let exact_peak = plan.selected_inventory().peak_bytes + wide.incremental_bytes;
        plan.usable_bytes = Some(exact_peak - 1);
        reset_canonical_frame_refresh_count();
        super::super::categorical_fwl_cmg::take_route_observation_preparations();
        let result = fit_prepared_ppml_irls(
            &prepared,
            &outcome,
            &vec![1.0; outcome.len()],
            &vec![0.0; outcome.len()],
            config,
            plan,
            None,
        );
        let error = match result {
            Ok(_) => panic!("enlarged nuisance pool escaped preflight"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::Allocation);
        assert!(error.message.contains("public PPML FE solver workspace"));
        assert_eq!(take_canonical_frame_refresh_count(), 0);
        assert_eq!(
            super::super::categorical_fwl_cmg::take_route_observation_preparations(),
            0
        );
        plan.usable_bytes = Some(exact_peak);
        assert!(plan.with_solver_workspace(wide.incremental_bytes).is_ok());
    }

    fn memory_dimensions<P: PreparedOlsFitView + ?Sized>(
        prepared: &P,
        config: PreparedPpmlIrlsConfig,
    ) -> PpmlMemoryDimensions {
        let design = prepared.fit_owned_fe_design().expect("fixture FE design");
        let mut total_levels = 0_usize;
        let mut max_levels = 0_usize;
        let mut total_channels = 0_usize;
        let mut max_channels = 0_usize;
        let mut max_level_members = 0_usize;
        let mut directions = 0_usize;
        let mut max_directions = 0_usize;
        for effect in design.effects() {
            total_levels = total_levels
                .checked_add(effect.n_levels())
                .expect("small fixture levels");
            max_levels = max_levels.max(effect.n_levels());
            total_channels = total_channels
                .checked_add(effect.channels())
                .expect("small fixture channels");
            max_channels = max_channels.max(effect.channels());
            for level in 0..effect.n_levels() {
                max_level_members =
                    max_level_members.max(effect.level_index().members(level).len());
            }
            let effect_directions = effect
                .n_levels()
                .checked_mul(effect.channels())
                .expect("small fixture directions");
            directions = directions
                .checked_add(effect_directions)
                .expect("small fixture directions");
            max_directions = max_directions.max(effect_directions);
        }
        PpmlMemoryDimensions {
            metadata: Default::default(),
            observations: prepared.fit_sample().n_active(),
            original_observations: prepared.fit_sample().n_original,
            candidate_observations: prepared.fit_sample().n_candidate,
            loaded_numeric_columns: prepared.fit_sample().regressors.len(),
            retained_base_adjustment_capacity_elements: 0,
            retained_raw_preseparation_probability_weight_elements: 0,
            finite_columns: prepared.fit_model().columns.len(),
            coefficient_slots: prepared.fit_model().coefficients.len(),
            reported_coefficient_slots: prepared.fit_model().coefficients.len(),
            fe_dimensions: design.effects().len(),
            fe_total_levels: total_levels,
            fe_max_levels: max_levels,
            fe_total_channels: total_channels,
            fe_max_channels: max_channels,
            fe_max_level_members: max_level_members,
            fe_coefficient_directions: directions,
            fe_max_effect_directions: max_directions,
            cluster_dimensions: 0,
            cluster_subset_groups: 0,
            separation_workspace_vectors: 0,
            separation_singleton_enabled: false,
            separation_method_count: 0,
            separation_generation_capacity: 1,
            separation_rebase_enabled: false,
            separation_exact_fe_enabled: false,
            separation_mu_enabled: false,
            simplex_tableau_cells: 0,
            requested_output_columns: 4,
            requested_mean_output: false,
            requested_saved_fe_channels: 0,
            publication_matrix_cells: prepared.fit_model().coefficients.len().saturating_pow(2),
            threads: config.linear.execution.threads.resolved(),
            allocation_audit_enabled: config.allocation_snapshot_probe.is_some(),
        }
    }

    #[test]
    fn initial_checkpoint_is_structural_and_preserves_explicit_solver_purpose() {
        let mut config = PreparedPpmlIrlsConfig::default();
        config.linear.execution.mode = PreparedOlsExecutionMode::Fast;
        for family in [
            super::super::PpmlSolverFamily::Lsmr,
            super::super::PpmlSolverFamily::Cmg,
        ] {
            config.solver_plan = PpmlSolverPlan::new(family);
            assert!(use_initial_checkpoint_lsmr_accelerator(
                &config,
                PreparedPpmlIrlsPurpose::Publication,
                PARALLEL_PROJECTION_MIN_ROWS,
                2,
            ));
            assert!(use_initial_checkpoint_lsmr_accelerator(
                &config,
                PreparedPpmlIrlsPurpose::Publication,
                PARALLEL_PROJECTION_MIN_ROWS,
                3,
            ));
            assert!(use_initial_checkpoint_lsmr_accelerator(
                &config,
                PreparedPpmlIrlsPurpose::Publication,
                PARALLEL_PROJECTION_MIN_ROWS - 1,
                3,
            ));
            for observations in [1, 400, 8_000, 10_000] {
                assert!(use_initial_checkpoint_lsmr_accelerator(
                    &config,
                    PreparedPpmlIrlsPurpose::Publication,
                    observations,
                    3,
                ));
            }
            assert!(!use_initial_checkpoint_lsmr_accelerator(
                &config,
                PreparedPpmlIrlsPurpose::Publication,
                0,
                3,
            ));
            assert!(!use_initial_checkpoint_lsmr_accelerator(
                &config,
                PreparedPpmlIrlsPurpose::Publication,
                PARALLEL_PROJECTION_MIN_ROWS,
                4,
            ));
            assert!(!use_initial_checkpoint_lsmr_accelerator(
                &config,
                PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance: 1e-8 },
                PARALLEL_PROJECTION_MIN_ROWS,
                3,
            ));
            config.linear.execution.mode = PreparedOlsExecutionMode::Low;
            assert!(use_initial_checkpoint_lsmr_accelerator(
                &config,
                PreparedPpmlIrlsPurpose::Publication,
                PARALLEL_PROJECTION_MIN_ROWS,
                3,
            ));
            config.linear.execution.mode = PreparedOlsExecutionMode::Fast;
        }
        for family in [
            super::super::PpmlSolverFamily::Auto,
            super::super::PpmlSolverFamily::Map,
        ] {
            config.solver_plan = PpmlSolverPlan::new(family);
            assert!(!use_initial_checkpoint_lsmr_accelerator(
                &config,
                PreparedPpmlIrlsPurpose::Publication,
                PARALLEL_PROJECTION_MIN_ROWS,
                3,
            ));
        }
    }

    static STABLE_AUDIT_PROBE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static CHURN_AUDIT_PROBE_CALLS: AtomicUsize = AtomicUsize::new(0);

    fn stable_audit_probe() -> PpmlAllocatorSnapshot {
        STABLE_AUDIT_PROBE_CALLS.fetch_add(1, Ordering::SeqCst);
        PpmlAllocatorSnapshot {
            current_bytes: 16_384,
            peak_bytes: 32_768,
            allocation_events: 41,
        }
    }

    fn churn_audit_probe() -> PpmlAllocatorSnapshot {
        let call = CHURN_AUDIT_PROBE_CALLS.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            PpmlAllocatorSnapshot {
                current_bytes: 16_384,
                peak_bytes: 32_768,
                allocation_events: 41,
            }
        } else {
            // Simulate one successful allocate/free pair inside the measured
            // solves: current bytes return to baseline, while the monotone
            // event counter exposes the otherwise invisible churn.
            PpmlAllocatorSnapshot {
                current_bytes: 16_384,
                peak_bytes: 32_768,
                allocation_events: 42,
            }
        }
    }

    fn allocation_audited_fit(probe: PpmlAllocatorSnapshotProbe) -> PreparedPpmlIrlsFit {
        let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
        let eta = x
            .iter()
            .map(|value| 0.35_f64 + 0.42_f64 * value)
            .collect::<Vec<_>>();
        let outcome = eta
            .iter()
            .enumerate()
            .map(|(row, eta)| eta.exp() + (row as f64 % 3.0) * 0.03)
            .collect::<Vec<_>>();
        let prepared = prepared(&outcome, &[x]);
        let config = PreparedPpmlIrlsConfig {
            ppml: PpmlConfig {
                max_iterations: 100,
                ..PpmlConfig::default()
            },
            allocation_snapshot_probe: Some(probe),
            ..PreparedPpmlIrlsConfig::default()
        };
        fit_prepared_ppml_irls(
            &prepared,
            &outcome,
            &[1.0; 8],
            &[0.0; 8],
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                .expect("allocation-audit memory plan"),
            None,
        )
        .expect("allocation-audited prepared PPML")
    }

    #[test]
    fn complete_warmed_map_window_certifies_stable_fake_allocator() {
        STABLE_AUDIT_PROBE_CALLS.store(0, Ordering::SeqCst);
        let fitted = allocation_audited_fit(stable_audit_probe);
        let certificate = fitted
            .diagnostics()
            .memory
            .steady_state_allocation_certificate
            .expect("steady-state allocation certificate");
        assert_eq!(STABLE_AUDIT_PROBE_CALLS.load(Ordering::SeqCst), 2);
        assert_eq!(
            certificate.observation.measured_iterations,
            PPML_STEADY_STATE_AUDIT_ITERATIONS
        );
        assert_eq!(certificate.allocation_events, 0);
        assert!(certificate.workspace_capacity_stable);
        assert!(certificate.linear_workspace_capacity_stable);
        assert!(certificate.scratch_stable);
        assert!(certificate.observation.linear_session_allocation_free);
        assert!(certificate.observation.fresh_warm_equivalent);
        assert!(certificate.passed);
        assert!(fitted.diagnostics().memory.whole_iteration_allocation_free);
    }

    #[test]
    fn allocator_event_inside_warmed_solve_fails_even_with_zero_net_growth() {
        CHURN_AUDIT_PROBE_CALLS.store(0, Ordering::SeqCst);
        let fitted = allocation_audited_fit(churn_audit_probe);
        let certificate = fitted
            .diagnostics()
            .memory
            .steady_state_allocation_certificate
            .expect("steady-state allocation certificate");
        assert_eq!(CHURN_AUDIT_PROBE_CALLS.load(Ordering::SeqCst), 2);
        assert_eq!(certificate.allocation_events, 1);
        assert_eq!(certificate.allocator_growth_bytes(), 0);
        assert_eq!(certificate.allocator_peak_growth_bytes, 0);
        assert!(!certificate.passed);
        assert!(!fitted.diagnostics().memory.whole_iteration_allocation_free);
    }

    #[test]
    fn direct_first_partition_start_matches_independent_offset_poisson_truth() {
        for dimensions in [2, 3, 4] {
            let n = 2 * 2 * 3 * 2 * 2;
            let codes: Vec<Vec<u32>> = (0..dimensions)
                .map(|d| {
                    (0..n)
                        .map(|r| ((r / [2, 4, 12, 24][d]) % [2, 3, 2, 2][d]) as u32)
                        .collect()
                })
                .collect();
            let offset: Vec<_> = (0..n)
                .map(
                    |r| if codes[0][r] == 0 { 1000.0 } else { -1000.0 } + 0.05 * codes[1][r] as f64,
                )
                .collect();
            let truth: Vec<_> = (0..n)
                .map(|r| {
                    (0.05 * codes[1][r] as f64
                        + (0..dimensions)
                            .map(|d| 0.2 * (d + 1) as f64 * codes[d][r] as f64)
                            .sum::<f64>())
                    .exp()
                })
                .collect();
            // Paired outcome deviations give zero score for every explicit
            // intercept and slope, independently specifying the Poisson optimum.
            let y: Vec<_> = truth
                .iter()
                .enumerate()
                .map(|(r, m)| m * if r % 2 == 0 { 0.8 } else { 1.2 })
                .collect();
            let prepared = prepared(&y, &[]);
            for slopes in [false, true] {
                let effects = codes
                    .iter()
                    .enumerate()
                    .map(|(d, c)| {
                        crate::fe::EffectTerm::new(
                            c.clone(),
                            [2, 3, 2, 2][d],
                            true,
                            if slopes {
                                vec![
                                    (0..n).map(|r| (0.37 * (r / 2) as f64).sin()).collect(),
                                    vec![1.; n],
                                ]
                            } else {
                                vec![]
                            },
                        )
                        .unwrap()
                    })
                    .collect();
                let design = FeDesign::new(n, effects).unwrap();
                for scale in [1e-150, 1., 1e150] {
                    let q: Vec<_> = (0..n)
                        .map(|r| {
                            scale
                                * (0..dimensions)
                                    .map(|d| 1.0 + 0.25 * codes[d][r] as f64)
                                    .product::<f64>()
                        })
                        .collect();
                    let (beta, eta) = initialize(
                        &prepared,
                        &y,
                        &q,
                        &offset,
                        &design,
                        PreparedPpmlIrlsConfig::default(),
                        None,
                        PreparedPpmlIrlsPurpose::Publication,
                    )
                    .unwrap();
                    assert!(
                        beta.iter().all(|b| *b == 0.),
                        "absorbed initial constant has a zero gauge"
                    );
                    for (value, expected) in eta.iter().zip(&truth) {
                        assert!((value.exp() - expected).abs() <= 1e-11 * (1.0 + expected));
                    }
                    for effect in design.effects() {
                        for level in 0..effect.n_levels() {
                            let score: f64 = effect
                                .level_index()
                                .members(level)
                                .iter()
                                .map(|&r| q[r] / scale * (y[r] - eta[r].exp()))
                                .sum();
                            assert!(score.abs() < 1e-9);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn direct_first_partition_refusal_restores_global_after_later_partition_failure() {
        let y = [1., 2., 0., 0., 3., 4., 0., 0.];
        let q = [1., 2., 3., 4., 5., 6., 7., 8.];
        let offset = [0.2, -0.3, 0.7, -0.1, 0.4, -0.5, 0.1, 0.9];
        let design = FeDesign::new(
            8,
            vec![
                crate::fe::EffectTerm::new(vec![0, 0, 0, 0, 1, 1, 1, 1], 2, true, vec![]).unwrap(),
                crate::fe::EffectTerm::new(vec![0, 0, 1, 1, 0, 0, 1, 1], 2, true, vec![]).unwrap(),
            ],
        )
        .unwrap();
        let mut eta = [0.; 8];
        assert!(seed_group_intercept_predictor(
            &y,
            &q,
            &offset,
            &design.effects()[0],
            PpmlConfig::default(),
            &mut eta
        ));
        assert!(!seed_multi_effect_predictor(
            &y,
            &q,
            &offset,
            &design,
            PpmlConfig::default(),
            &mut eta
        ));
        let prepared = prepared(&y, &[]);
        let run = |purpose| {
            initialize(
                &prepared,
                &y,
                &q,
                &offset,
                &design,
                PreparedPpmlIrlsConfig::default(),
                None,
                purpose,
            )
            .unwrap()
        };
        assert_eq!(
            run(PreparedPpmlIrlsPurpose::Publication),
            run(PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance: 1e-4 })
        );
    }

    #[test]
    fn stable_initialization_handles_large_offsets() {
        let outcome = [0.0, 2.0, 8.0];
        let weights = [1.0, 3.0, 0.5];
        let offset = [700.0, 699.0, 698.0];
        let shift = log_weighted_outcome_sum(&outcome, &weights).expect("outcome")
            - log_weighted_offset_sum(&offset, &weights).expect("offset");
        assert!(shift.is_finite());
        let eta = offset.map(|value| value + shift);
        let mut means = [0.0; 3];
        means_from_eta(&eta, super::super::PoissonBounds::default(), &mut means)
            .expect("representable initialized means");
        assert!(means.iter().all(|mean| mean.is_finite() && *mean > 0.0));
    }

    #[test]
    fn one_effect_seed_is_group_optimal_weighted_stable_and_deterministic() {
        let design = FeDesign::new(
            6,
            vec![
                crate::fe::EffectTerm::new(vec![0, 0, 0, 1, 1, 1], 2, true, Vec::new())
                    .expect("effect"),
            ],
        )
        .expect("design");
        let outcome = [0.0, 2.0, 5.0, 3.0, 0.5, 9.0];
        let weights = [0.25, 4.0, 2.0, 3.0, 1.0, 0.5];
        let offset = [1000.0, 1000.25, 999.8, -1000.0, -1000.1, -999.6];
        let mut eta = [0.0; 6];
        assert!(seed_one_effect_predictor(
            &outcome,
            &weights,
            &offset,
            &design,
            PpmlConfig::default(),
            &mut eta
        ));
        for group in [0..3, 3..6] {
            let score: f64 = group
                .clone()
                .map(|row| weights[row] * (outcome[row] - eta[row].exp()))
                .sum();
            assert!(score.abs() < 1e-11, "group score {score}");
            let alpha = eta[group.start] - offset[group.start];
            for row in group {
                assert!((eta[row] - offset[row] - alpha).abs() < 3e-13);
            }
        }
        let mut repeated = [0.0; 6];
        assert!(seed_one_effect_predictor(
            &outcome,
            &weights,
            &offset,
            &design,
            PpmlConfig::default(),
            &mut repeated
        ));
        assert_eq!(eta, repeated);
        let scaled = weights.map(|value| 7.5 * value);
        assert!(seed_one_effect_predictor(
            &outcome,
            &scaled,
            &offset,
            &design,
            PpmlConfig::default(),
            &mut repeated
        ));
        assert!(relative_infinity_difference(&eta, &repeated) < 1e-12);
    }

    #[test]
    fn multi_effect_seed_matches_separable_poisson_truth_and_weight_scaling() {
        for dimensions in [2, 3] {
            let n = 4 * 5 * 3 * 2;
            let codes: Vec<Vec<u32>> = (0..dimensions)
                .map(|d| {
                    (0..n)
                        .map(|r| match d {
                            0 => (r / 2) % 4,
                            1 => (r / 8) % 5,
                            _ => (r / 40) % 3,
                        } as u32)
                        .collect()
                })
                .collect();
            let effects = codes
                .iter()
                .enumerate()
                .map(|(d, c)| {
                    crate::fe::EffectTerm::new(c.clone(), [4, 5, 3][d], true, Vec::new()).unwrap()
                })
                .collect();
            let design = FeDesign::new(n, effects).unwrap();
            let truth: Vec<f64> = (0..n)
                .map(|r| {
                    (0..dimensions)
                        .map(|d| 0.2 * (d + 1) as f64 * codes[d][r] as f64)
                        .sum::<f64>()
                        .exp()
                })
                .collect();
            // Paired deviations have zero weighted score for every explicit
            // dummy. The independent optimum is the prescribed product mean.
            let outcome: Vec<_> = truth
                .iter()
                .enumerate()
                .map(|(r, m)| m * if r % 2 == 0 { 0.8 } else { 1.2 })
                .collect();
            for scale in [1e-50, 1.0, 1e50] {
                let weights: Vec<_> = (0..n)
                    .map(|r| {
                        scale
                            * (0..dimensions)
                                .map(|d| 1.0 + 0.25 * codes[d][r] as f64)
                                .product::<f64>()
                    })
                    .collect();
                let total: f64 = outcome.iter().zip(&weights).map(|(y, w)| y * w).sum();
                let initial = (total / weights.iter().sum::<f64>()).ln();
                let mut eta = vec![initial; n];
                assert!(seed_multi_effect_predictor(
                    &outcome,
                    &weights,
                    &vec![0.; n],
                    &design,
                    PpmlConfig::default(),
                    &mut eta
                ));
                for (value, expected) in eta.iter().zip(&truth) {
                    assert!((value.exp() - expected).abs() <= 1e-12 * (1.0 + expected));
                }
            }
        }
    }

    #[test]
    fn profiled_slope_seed_matches_analytic_newton_step_and_scaling() {
        let members = [0, 1, 2, 3];
        let outcome = [1., 9., 1., 9.];
        for slope_scale in [1e-50, 1., 1e50] {
            for weight_scale in [1e-50, 1., 1e50] {
                let slope = [-slope_scale, slope_scale, -slope_scale, slope_scale];
                let weights = [weight_scale; 4];
                let mut eta = [5.0_f64.ln(); 4];
                assert!(seed_profiled_slope(
                    &members,
                    &slope,
                    &outcome,
                    &weights,
                    20. * weight_scale,
                    PpmlConfig::default(),
                    &mut eta
                ));
                // At beta=0, the normalized slope score/curvature is 16/20.
                // Profiling the intercept sets the mean back to five.
                let b = 0.8_f64;
                let intercept = 5.0_f64.ln() - b.cosh().ln();
                // Keep the legacy matrix indices and numerical traversal order explicit.
                #[allow(clippy::needless_range_loop)]
                for r in 0..4 {
                    let truth = intercept + if r % 2 == 0 { -b } else { b };
                    assert!((eta[r] - truth).abs() < 1e-12);
                }
                let objective =
                    |e: &[f64]| (0..4).map(|r| outcome[r] * e[r] - e[r].exp()).sum::<f64>();
                assert!(objective(&eta) > objective(&[5.0_f64.ln(); 4]));
            }
        }
    }

    #[test]
    fn slope_intercept_seed_matches_independent_poisson_truth() {
        for dimensions in [2, 3] {
            let n = 4 * 5 * 3 * 2;
            let codes: Vec<Vec<u32>> = (0..dimensions)
                .map(|d| {
                    (0..n)
                        .map(|r| match d {
                            0 => (r / 2) % 4,
                            1 => (r / 8) % 5,
                            _ => (r / 40) % 3,
                        } as u32)
                        .collect()
                })
                .collect();
            // Every explicit intercept/slope is constant within a replica pair.
            // Paired outcome deviations cancel its weighted score exactly.
            let slopes: Vec<_> = (0..n).map(|r| (0.37 * (r / 2) as f64).sin()).collect();
            let effects = codes
                .iter()
                .enumerate()
                .map(|(d, c)| {
                    crate::fe::EffectTerm::new(
                        c.clone(),
                        [4, 5, 3][d],
                        true,
                        vec![
                            slopes.clone(),
                            slopes.iter().map(|z| z * 1e6).collect(),
                            vec![0.0; n],
                        ],
                    )
                    .unwrap()
                })
                .collect();
            let design = FeDesign::new(n, effects).unwrap();
            let truth: Vec<f64> = (0..n)
                .map(|r| {
                    (0..dimensions)
                        .map(|d| 0.2 * (d + 1) as f64 * codes[d][r] as f64)
                        .sum::<f64>()
                        .exp()
                })
                .collect();
            let outcome: Vec<_> = truth
                .iter()
                .enumerate()
                .map(|(r, m)| m * if r % 2 == 0 { 0.8 } else { 1.2 })
                .collect();
            for scale in [1e-50, 1.0, 1e50] {
                let weights: Vec<_> = (0..n)
                    .map(|r| {
                        scale
                            * (0..dimensions)
                                .map(|d| 1.0 + 0.25 * codes[d][r] as f64)
                                .product::<f64>()
                    })
                    .collect();
                let total: f64 = outcome.iter().zip(&weights).map(|(y, w)| y * w).sum();
                let initial = (total / weights.iter().sum::<f64>()).ln();
                let mut eta = vec![initial; n];
                assert!(seed_multi_effect_predictor(
                    &outcome,
                    &weights,
                    &vec![0.; n],
                    &design,
                    PpmlConfig::default(),
                    &mut eta
                ));
                for (value, expected) in eta.iter().zip(&truth) {
                    assert!((value.exp() - expected).abs() <= 1e-12 * (1.0 + expected));
                }
            }
        }
    }

    #[test]
    fn slope_intercept_seed_preserves_offsets_warm_starts_and_full_refusal() {
        let make_design = |intercept| {
            FeDesign::new(
                8,
                vec![
                    crate::fe::EffectTerm::new(
                        vec![0, 0, 0, 0, 1, 1, 1, 1],
                        2,
                        true,
                        vec![vec![0., 0., 2., 2., 0., 0., 2., 2.]],
                    )
                    .unwrap(),
                    crate::fe::EffectTerm::new(
                        vec![0, 0, 1, 1, 0, 0, 1, 1],
                        2,
                        intercept,
                        vec![vec![1., 1., 0., 0., 1., 1., 0., 0.], vec![0.; 8]],
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
        };
        let design = make_design(true);
        let weights = [1., 2., 3., 4., 5., 6., 7., 8.];
        let offset = [0.2, -0.3, 0.7, -0.1, 0.4, -0.5, 0.1, 0.9];
        let outcome = [1., 3., 4., 2., 7., 5., 8., 9.];
        let prepared = prepared(&outcome, &[]);
        let config = PreparedPpmlIrlsConfig::default();
        let initialize_as = |purpose, start| {
            initialize(
                &prepared, &outcome, &weights, &offset, &design, config, start, purpose,
            )
            .unwrap()
        };
        let mu = PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance: 1e-4 };
        let (global_coefficients, global) = initialize_as(mu, None);
        let (_, publication) = initialize_as(PreparedPpmlIrlsPurpose::Publication, None);
        let objective = |e: &[f64]| {
            (0..8)
                .map(|r| weights[r] * (outcome[r] * e[r] - e[r].exp()))
                .sum::<f64>()
        };
        assert!(objective(&publication) > objective(&global));
        for group in [vec![0, 1, 4, 5], vec![2, 3, 6, 7]] {
            let score: f64 = group
                .iter()
                .map(|&r| weights[r] * (outcome[r] - publication[r].exp()))
                .sum();
            assert!(score.abs() < 1e-11);
        }
        for r in (0..8).step_by(2) {
            assert!(
                ((publication[r] - offset[r]) - (publication[r + 1] - offset[r + 1])).abs() < 1e-14
            );
        }
        let start = PreparedPpmlStart {
            coefficient_values: global_coefficients,
            eta: global.clone(),
        };
        let (coefficients, warm) =
            initialize_as(PreparedPpmlIrlsPurpose::Publication, Some(&start));
        assert_eq!(coefficients, start.coefficient_values);
        assert_eq!(warm, start.eta);
        let mut refused = global.clone();
        assert!(!seed_multi_effect_predictor(
            &outcome,
            &weights,
            &offset,
            &make_design(false),
            config.ppml,
            &mut refused
        ));
        assert_eq!(
            refused, global,
            "a slope-only partition retains the prior route"
        );
        for y in [[1., 2., 3., 4., 0., 0., 0., 0.], [2.; 8]] {
            let mut limited = config;
            if y[4] > 0.0 {
                limited.ppml.bounds.eta_max = 0.01;
            }
            let run = |purpose| {
                initialize(
                    &prepared, &y, &weights, &offset, &design, limited, None, purpose,
                )
                .unwrap()
                .1
            };
            assert_eq!(run(PreparedPpmlIrlsPurpose::Publication), run(mu));
        }
        let mut eta = global;
        assert!(!seed_multi_effect_predictor(
            &[2.; 8],
            &[1e308; 8],
            &offset,
            &design,
            config.ppml,
            &mut eta
        ));
    }

    #[test]
    fn multi_effect_seed_improves_objective_and_refusal_restores_global_start() {
        let design = FeDesign::new(
            8,
            vec![
                crate::fe::EffectTerm::new(vec![0, 0, 0, 0, 1, 1, 1, 1], 2, true, vec![]).unwrap(),
                crate::fe::EffectTerm::new(vec![0, 0, 1, 1, 0, 0, 1, 1], 2, true, vec![]).unwrap(),
            ],
        )
        .unwrap();
        let weights = [1., 2., 3., 4., 5., 6., 7., 8.];
        let offset = [0.2, -0.3, 0.7, -0.1, 0.4, -0.5, 0.1, 0.9];
        let outcome = [1., 3., 4., 2., 7., 5., 8., 9.];
        let mut eta = offset;
        let objective = |e: &[f64]| {
            (0..8)
                .map(|r| weights[r] * (outcome[r] * e[r] - e[r].exp()))
                .sum::<f64>()
        };
        let before = objective(&eta);
        assert!(seed_multi_effect_predictor(
            &outcome,
            &weights,
            &offset,
            &design,
            PpmlConfig::default(),
            &mut eta
        ));
        assert!(objective(&eta) > before);
        for group in [vec![0, 1, 4, 5], vec![2, 3, 6, 7]] {
            let score: f64 = group
                .iter()
                .map(|&r| weights[r] * (outcome[r] - eta[r].exp()))
                .sum();
            assert!(score.abs() < 1e-11);
        }
        // Each replica within a cell differs only by its original offset.
        for r in (0..8).step_by(2) {
            assert!(((eta[r] - offset[r]) - (eta[r + 1] - offset[r + 1])).abs() < 1e-14);
        }
        let outcome = [1., 2., 3., 4., 0., 0., 0., 0.];
        let prepared = prepared(&outcome, &[]);
        let config = PreparedPpmlIrlsConfig::default();
        let (_, global) = initialize(
            &prepared,
            &outcome,
            &weights,
            &offset,
            &design,
            config,
            None,
            PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance: 1e-4 },
        )
        .unwrap();
        let (_, publication) = initialize(
            &prepared,
            &outcome,
            &weights,
            &offset,
            &design,
            config,
            None,
            PreparedPpmlIrlsPurpose::Publication,
        )
        .unwrap();
        assert_eq!(publication, global);
        let mut unusable = [0.; 8];
        assert!(!seed_multi_effect_predictor(
            &[2.; 8],
            &[1e308; 8],
            &[0.; 8],
            &design,
            config.ppml,
            &mut unusable
        ));
        let mut bounded = PpmlConfig::default();
        bounded.bounds.eta_max = 0.01;
        assert!(!seed_multi_effect_predictor(
            &[2.; 8],
            &[1.; 8],
            &[0.; 8],
            &design,
            bounded,
            &mut unusable
        ));
    }

    #[test]
    fn unusable_one_effect_seed_restores_global_start_and_mu_keeps_global_start() {
        let design = FeDesign::new(
            4,
            vec![
                crate::fe::EffectTerm::new(vec![0, 0, 1, 1], 2, true, Vec::new()).expect("effect"),
            ],
        )
        .expect("design");
        let offset = [0.0; 4];
        let weights = [1.0; 4];
        for outcome in [[2.0, 4.0, 0.0, 0.0], [2.0, 4.0, 1.0, 9.0]] {
            let prepared = prepared(&outcome, &[]);
            let config = PreparedPpmlIrlsConfig::default();
            let (_, global) = initialize(
                &prepared,
                &outcome,
                &weights,
                &offset,
                &design,
                config,
                None,
                PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance: 1e-4 },
            )
            .expect("Mu start");
            let expected = (outcome.iter().sum::<f64>() / 4.0).ln();
            assert!(global.iter().all(|value| (*value - expected).abs() < 1e-14));
            let (_, publication) = initialize(
                &prepared,
                &outcome,
                &weights,
                &offset,
                &design,
                config,
                None,
                PreparedPpmlIrlsPurpose::Publication,
            )
            .expect("publication start");
            if outcome[2] == 0.0 {
                assert_eq!(
                    publication, global,
                    "zero-outcome group cannot produce a finite seed"
                );
            } else {
                assert_ne!(publication, global);
            }
        }
    }

    #[test]
    fn constant_offset_seed_uses_weighted_means_and_falls_back_on_raw_overflow() {
        let members = [0, 1];
        assert_eq!(
            constant_offset_group_log_mean(&members, &[2.0, 4.0], &[3.0, 1.0], &[1000.0; 2]),
            Some(2.5_f64.ln()),
        );
        for (outcome, weights, offset) in [
            ([2.0, 4.0], [1e308; 2], [1000.0; 2]),
            ([1e-100; 2], [1e-300; 2], [0.0; 2]),
            ([2.0, 4.0], [1.0; 2], [0.0, 1.0]),
            ([0.0; 2], [1.0; 2], [0.0; 2]),
        ] {
            assert!(
                constant_offset_group_log_mean(&members, &outcome, &weights, &offset).is_none()
            );
        }
        let design = FeDesign::new(
            2,
            vec![crate::fe::EffectTerm::new(vec![0, 0], 1, true, Vec::new()).unwrap()],
        )
        .unwrap();
        let mut eta = [0.0; 2];
        assert!(seed_one_effect_predictor(
            &[2.0, 4.0],
            &[1e308; 2],
            &[1000.0; 2],
            &design,
            PpmlConfig::default(),
            &mut eta,
        ));
        assert!(eta
            .iter()
            .all(|value| (*value - 3.0_f64.ln()).abs() < 1e-12));
    }

    #[test]
    fn one_effect_seed_does_not_admit_slopes_or_multiple_effects() {
        let codes = vec![0, 0, 1, 1];
        let intercept =
            crate::fe::EffectTerm::new(codes.clone(), 2, true, Vec::new()).expect("effect");
        let slope = crate::fe::EffectTerm::new(codes, 2, true, vec![vec![0.0, 1.0, 2.0, 3.0]])
            .expect("slope");
        for effects in [Vec::new(), vec![slope], vec![intercept.clone(), intercept]] {
            let design = FeDesign::new(4, effects).expect("design");
            let mut eta = [9.0; 4];
            assert!(!seed_one_effect_predictor(
                &[1.0; 4],
                &[1.0; 4],
                &[0.0; 4],
                &design,
                PpmlConfig::default(),
                &mut eta
            ));
            assert_eq!(eta, [9.0; 4]);
        }
    }

    #[test]
    fn line_search_reuses_buffers_and_rejects_overflow() {
        let dimensions = PpmlMemoryDimensions {
            metadata: Default::default(),
            observations: 2,
            original_observations: 2,
            candidate_observations: 2,
            loaded_numeric_columns: 0,
            retained_base_adjustment_capacity_elements: 0,
            retained_raw_preseparation_probability_weight_elements: 0,
            finite_columns: 1,
            coefficient_slots: 1,
            reported_coefficient_slots: 1,
            fe_dimensions: 0,
            fe_total_levels: 0,
            fe_max_levels: 0,
            fe_total_channels: 0,
            fe_max_channels: 0,
            fe_max_level_members: 0,
            fe_coefficient_directions: 0,
            fe_max_effect_directions: 0,
            cluster_dimensions: 0,
            cluster_subset_groups: 0,
            separation_workspace_vectors: 0,
            separation_singleton_enabled: false,
            separation_method_count: 0,
            separation_generation_capacity: 1,
            separation_rebase_enabled: false,
            separation_exact_fe_enabled: false,
            separation_mu_enabled: false,
            simplex_tableau_cells: 0,
            requested_output_columns: 0,
            requested_mean_output: false,
            requested_saved_fe_channels: 0,
            publication_matrix_cells: 1,
            threads: 1,
            allocation_audit_enabled: false,
        };
        let plan = plan_ppml_memory(dimensions, PpmlMemoryConfig::default()).expect("plan");
        let mut workspace =
            PpmlIterationWorkspace::from_initial_state(plan, vec![0.0, 0.0], vec![0.0])
                .expect("workspace");
        workspace
            .refresh_working_state(
                &[1.0, 1.0],
                &[1.0, 1.0],
                &[0.0, 0.0],
                PpmlConfig::default().bounds,
            )
            .expect("working state");
        workspace
            .set_candidate_eta(&[1_000.0, -1_000.0])
            .expect("candidate eta");
        workspace
            .set_candidate_coefficients(&[1.0])
            .expect("candidate coefficients");
        let objective =
            poisson_objective_from_eta(&[1.0, 1.0], workspace.eta(), workspace.mean(), &[1.0, 1.0])
                .expect("objective");
        let accepted = workspace
            .accept_candidate(&[1.0, 1.0], &[1.0, 1.0], objective, PpmlConfig::default())
            .expect_err("no improving extreme direction");
        assert_eq!(accepted.code, ErrorCode::NonConvergence);
    }

    fn injected_warm_failure_fit(
        injection: Option<FrozenLinearFailureInjection>,
    ) -> Result<PreparedPpmlIrlsFit> {
        let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
        let eta = x
            .iter()
            .map(|value| 0.35_f64 + 0.42_f64 * value)
            .collect::<Vec<_>>();
        let outcome = eta
            .iter()
            .enumerate()
            .map(|(row, eta)| eta.exp() + (row as f64 % 3.0) * 0.03)
            .collect::<Vec<_>>();
        let prepared = prepared(&outcome, &[x]);
        let config = PreparedPpmlIrlsConfig::default();
        if let Some(injection) = injection {
            inject_frozen_linear_failure(injection);
        }
        fit_prepared_ppml_irls(
            &prepared,
            &outcome,
            &[1.0; 8],
            &[0.0; 8],
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)?,
            None,
        )
    }

    #[test]
    fn invalid_warm_projection_falls_back_to_independently_certified_canonical_solve() {
        let fitted = injected_warm_failure_fit(Some(
            FrozenLinearFailureInjection::RecoverableWarmProjection,
        ))
        .expect("canonical fallback fit");
        let baseline = injected_warm_failure_fit(None).expect("baseline fit");
        assert!(fitted.diagnostics().converged);
        assert!(fitted.diagnostics().final_score.passed);
        assert_eq!(fitted.diagnostics().memory.fallback_count, 1);
        assert_eq!(
            fitted.diagnostics().memory.fallback_mask,
            1_u32 << PpmlFallbackKind::WarmProjection as u8
        );
        assert!(relative_infinity_difference(fitted.eta(), baseline.eta()) <= 1e-12);
        let fitted_coefficients = fitted
            .coefficients()
            .iter()
            .map(|coefficient| coefficient.value.unwrap_or(0.0))
            .collect::<Vec<_>>();
        let baseline_coefficients = baseline
            .coefficients()
            .iter()
            .map(|coefficient| coefficient.value.unwrap_or(0.0))
            .collect::<Vec<_>>();
        assert!(
            relative_infinity_difference(&fitted_coefficients, &baseline_coefficients) <= 1e-12
        );
    }

    #[test]
    fn failed_canonical_retry_after_warm_invalidation_is_atomic() {
        let error = injected_warm_failure_fit(Some(
            FrozenLinearFailureInjection::RecoverableThenCanonicalAllocation,
        ))
        .err()
        .expect("canonical allocation failure must abort the fit");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(
            error.message,
            "injected canonical fallback allocation failure"
        );
    }

    #[test]
    fn domain_and_interrupt_failures_never_enter_the_canonical_fallback() {
        let domain = injected_warm_failure_fit(Some(FrozenLinearFailureInjection::Domain))
            .err()
            .expect("domain failure must remain fail-closed");
        assert_eq!(domain.code, ErrorCode::InvalidData);
        assert_eq!(
            domain.message,
            "frozen weighted-linear normalized weight is invalid"
        );

        let interrupted =
            injected_warm_failure_fit(Some(FrozenLinearFailureInjection::Interrupted))
                .err()
                .expect("interruption must remain fail-closed");
        assert_eq!(interrupted.code, ErrorCode::Interrupted);
        assert_eq!(interrupted.message, "injected frozen solve interruption");
    }

    #[test]
    fn strict_candidate_categorical_projection_preserves_scratch_and_fresh_parity() {
        check_strict_candidate_categorical_projection(false);
    }

    #[test]
    fn rebuilt_strict_candidate_preserves_storage_and_fresh_science() {
        check_strict_candidate_categorical_projection(true);
    }

    fn check_strict_candidate_categorical_projection(rebuild: bool) {
        for (mode, poolsize) in [
            (PreparedOlsExecutionMode::Fast, 10),
            (PreparedOlsExecutionMode::Low, 1),
            (PreparedOlsExecutionMode::Low, 10_000),
        ] {
            let outcome = vec![1.2, 4.0, 0.5, 6.1, 2.7, 8.3, 5.5, 11.8];
            let x = vec![-2.0, 0.5, -1.0, 2.0, -0.25, 3.0, 1.0, 4.5];
            let response = vec![1.7, 3.6, 0.1, 6.8, 2.1, 9.0, 6.2, 12.4];
            let initial_weights = vec![1.0; outcome.len()];
            let refreshed_weights = vec![1.5, 0.75, 2.0, 1.25, 3.5, 0.6, 1.8, 2.2];
            let prepared = prepared_two_fe(&outcome, &x);
            let mut config = ppml_linear_config(PreparedPpmlIrlsConfig::default(), 1e-10)
                .expect("two-FE linear config");
            config.execution.mode = mode;
            config.execution.poolsize = poolsize;
            let initial_scale =
                ppml_bread_weight_scale(&initial_weights).expect("initial weight scale");
            let initial_frame = LinearSolveFrame::from_parts(
                &outcome,
                BorrowedMapWeights::Provided(&initial_weights),
                initial_scale,
            )
            .expect("initial two-FE frame");
            let design = prepared.fit_owned_fe_design().expect("two-FE design");
            let mut session = CertifiedLinearSession::new_with_fe_design(
                &prepared,
                initial_frame,
                config,
                LinearSessionGenerations::new(1, 1, 1, 1, 1),
                design,
            )
            .expect("two-FE PPML session");
            assert!(session.uses_ppml_categorical_map_for_test());
            assert!(session.solve_fresh().expect("initial solve").certified());
            let scratch = session
                .retained_scratch_identity()
                .map(std::path::Path::to_owned);
            assert_eq!(scratch.is_some(), mode == PreparedOlsExecutionMode::Low);
            if mode == PreparedOlsExecutionMode::Low {
                assert_eq!(
                    session.workspace_capacities().fe.rhs,
                    poolsize.min(prepared.fit_model().columns.len().max(1))
                );
                assert_eq!(session.workspace_capacities().within_resident_elements, 0);
            }
            session
                .refresh_response(&response, 2)
                .expect("strict response refresh");
            let refreshed_scale =
                ppml_bread_weight_scale(&refreshed_weights).expect("refreshed weight scale");
            session
                .refresh_weights(
                    BorrowedMapWeights::Provided(&refreshed_weights),
                    refreshed_scale,
                    2,
                )
                .expect("strict weight refresh");

            let offset = vec![0.0; outcome.len()];
            if rebuild {
                let error = session
                    .rebuild_retained_inputs_interruptible(&mut || {
                        Err(FeregError::new(
                            ErrorCode::Interrupted,
                            "rebuild interrupt test",
                        ))
                    })
                    .expect_err("rebuild must propagate caller interruption");
                assert_eq!(error.code, ErrorCode::Interrupted);
                let original_thread = std::thread::current().id();
                let capacities = session.workspace_capacities();
                let mut polls = 0;
                session
                    .rebuild_retained_inputs_interruptible(&mut || {
                        assert_eq!(std::thread::current().id(), original_thread);
                        polls += 1;
                        Ok(())
                    })
                    .expect("canonical input rebuild");
                assert!(polls > 0);
                assert_eq!(session.retained_scratch_identity(), scratch.as_deref());
                let after_rebuild = session.workspace_capacities();
                let mut allowed = capacities;
                if mode == PreparedOlsExecutionMode::Low {
                    allowed.projection_block_elements = allowed
                        .projection_block_elements
                        .max(outcome.len() * poolsize.min(1));
                }
                assert_eq!(after_rebuild, allowed);
                session
                    .rebuild_retained_inputs_interruptible(&mut || Ok(()))
                    .expect("warm rebuild");
                assert_eq!(session.workspace_capacities(), after_rebuild);
            }
            let transaction = session.strict_candidate().expect("fused strict candidate");
            assert_eq!(
                transaction.used_fused_projection_for_test(),
                mode == PreparedOlsExecutionMode::Fast
            );
            let actual = transaction.candidate();
            assert!(actual.certified());
            assert!(actual.reuse_report().warm_projection_attempted);
            let mut actual_eta = vec![0.0; outcome.len()];
            fill_candidate_eta_from_linear_output(&actual, &offset, &mut actual_eta)
                .expect("fused strict eta");
            let mut actual_coefficients = vec![0.0; prepared.fit_model().coefficients.len()];
            fill_candidate_coefficients_from_linear_output(&actual, &mut actual_coefficients)
                .expect("fused strict coefficients");
            let actual_bread = actual.frame().bread().to_vec();
            transaction.commit();
            assert_eq!(session.retained_scratch_identity(), scratch.as_deref());

            let expected_frame = LinearSolveFrame::from_parts(
                &response,
                BorrowedMapWeights::Provided(&refreshed_weights),
                refreshed_scale,
            )
            .expect("fresh comparison frame");
            let expected_design = prepared
                .fit_owned_fe_design()
                .expect("comparison FE design");
            let mut expected_session = CertifiedLinearSession::new_with_fe_design(
                &prepared,
                expected_frame,
                config,
                LinearSessionGenerations::new(1, 1, 1, 1, 1),
                expected_design,
            )
            .expect("fresh comparison session");
            let expected = expected_session
                .solve_fresh()
                .expect("fresh comparison solve");
            let mut expected_eta = vec![0.0; outcome.len()];
            fill_candidate_eta_from_linear_output(&expected, &offset, &mut expected_eta)
                .expect("fresh comparison eta");
            let mut expected_coefficients = vec![0.0; prepared.fit_model().coefficients.len()];
            fill_candidate_coefficients_from_linear_output(&expected, &mut expected_coefficients)
                .expect("fresh comparison coefficients");
            assert!(relative_infinity_difference(&actual_eta, &expected_eta) <= 1e-11);
            assert!(
                relative_infinity_difference(&actual_coefficients, &expected_coefficients) <= 1e-11
            );
            assert!(relative_infinity_difference(&actual_bread, expected.frame().bread()) <= 1e-10);
        }
    }

    #[test]
    fn joint_strict_final_preserves_truth_current_hessian_and_canonical_retry() {
        let _curve = ProfileCurveTestGuard::disabled();
        let _canonical = CanonicalFinalTestGuard::disabled();
        let n = 240;
        let g: Vec<_> = (0..n).map(|i| (i / 5 % 4) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| (i / 20 % 3) as f64).collect();
        let x: Vec<_> = (0..n)
            .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
            .collect();
        let eta: Vec<_> = (0..n)
            .map(|i| {
                0.3 + 0.2 * x[i] + g[i] * 0.1 - h[i] * 0.05
                    + (g[i] + h[i]) * ((i % 5) as f64 - 2.) * 0.02
            })
            .collect();
        let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
        let prepared = prepared_joint_slopes(&y, &x, &g, &h);
        let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 13) as f64 / 7.).collect();
        for memory_mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for threads in [1, 4] {
                let mut config = PreparedPpmlIrlsConfig::default();
                config.linear.execution.threads =
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                config.linear.execution.mode = memory_mode;
                config.linear.execution.poolsize = 1;
                config.memory.requested = match memory_mode {
                    PreparedOlsExecutionMode::Fast => crate::options::MemoryMode::Fast,
                    PreparedOlsExecutionMode::Low => crate::options::MemoryMode::Low,
                };
                config.memory.poolsize = 1;
                for mode in [0, 1, 2, 3] {
                    JOINT_FINAL_TEST_MODE.set(mode);
                    JOINT_FINAL_TEST_COUNTS.set((0, 0));
                    let fit = fit_prepared_ppml_irls(
                        &prepared,
                        &y,
                        &weights,
                        &vec![0.; n],
                        config,
                        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                            .unwrap(),
                        None,
                    )
                    .unwrap();
                    assert!(
                        fit.diagnostics().converged
                            && fit.diagnostics().two_consecutive_convergence
                    );
                    assert!(fit.diagnostics().final_score.passed);
                    assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
                    assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
                    validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean())
                        .unwrap();
                    if mode == 0 {
                        assert_eq!(JOINT_FINAL_TEST_COUNTS.get().1, 1);
                    }
                    if mode == 1 {
                        assert_eq!(JOINT_FINAL_TEST_COUNTS.get(), (0, 0));
                    }
                    if mode >= 2 {
                        assert!(JOINT_FINAL_TEST_COUNTS.get().0 >= 1);
                        assert_eq!(JOINT_FINAL_TEST_MODE.get(), 0);
                    }
                }
            }
        }
        JOINT_FINAL_TEST_MODE.set(0);
    }

    #[test]
    fn canonical_final_preserves_strict_disconnected_slope_truth() {
        let n = 480;
        let g: Vec<_> = (0..n).map(|i| (i / 5 % 4 + 4 * (i / 240)) as f64).collect();
        let h: Vec<_> = (0..n)
            .map(|i| (i / 20 % 3 + 3 * (i / 240)) as f64)
            .collect();
        let x: Vec<_> = (0..n)
            .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
            .collect();
        let eta: Vec<_> = (0..n)
            .map(|i| {
                0.3 + 0.2 * x[i] + 0.1 * g[i] - 0.05 * h[i]
                    + (g[i] + h[i]) * ((i % 5) as f64 - 2.) * 0.02
            })
            .collect();
        let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
        let prepared = prepared_joint_slopes(&y, &x, &g, &h);
        for disparity in [1e13] {
            let weights: Vec<_> = (0..n)
                .map(|i| (0.2 + (i % 13) as f64 / 7.) * if i < 240 { 1. } else { disparity })
                .collect();
            for (execution, memory) in [(
                PreparedOlsExecutionMode::Fast,
                crate::options::MemoryMode::Fast,
            )] {
                for threads in [1, 4] {
                    let mut config = PreparedPpmlIrlsConfig::default();
                    config.ppml.tolerance = 1e-12;
                    config.ppml.score_tolerance = 1e-12;
                    config.linear.execution.mode = execution;
                    config.memory.requested = memory;
                    config.linear.execution.poolsize = 1;
                    config.memory.poolsize = 1;
                    config.linear.execution.threads =
                        crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                    {
                        let _canonical = CanonicalFinalTestGuard::disabled();
                        let _curve = ProfileCurveTestGuard::disabled();
                        let baseline = fit_prepared_ppml_irls(
                            &prepared,
                            &y,
                            &weights,
                            &vec![0.; n],
                            config,
                            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                                .unwrap(),
                            None,
                        )
                        .unwrap();
                        assert!(relative_infinity_difference(baseline.eta(), &eta) < 1e-10);
                    }
                    for mode in [0, 1, 2, 3] {
                        profile_final_test_mode(mode);
                        let fit = fit_prepared_ppml_irls(
                            &prepared,
                            &y,
                            &weights,
                            &vec![0.; n],
                            config,
                            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                                .unwrap(),
                            None,
                        )
                        .unwrap();
                        let accepted = profile_final_test_mode(0);
                        assert_eq!(accepted, 0);
                        assert!(fit.diagnostics().strict_profile_rejections > 0);
                        assert!(
                            fit.diagnostics().converged
                                && fit.diagnostics().two_consecutive_convergence
                        );
                        assert!(fit.diagnostics().final_score.passed);
                        assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10,
                            "disparity={disparity} mode={mode} storage={execution:?} threads={threads} eta_error={:.17e}", relative_infinity_difference(fit.eta(), &eta));
                        assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
                        validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean())
                            .unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn canonical_final_replays_cancellation_before_saved_effect_recovery() {
        let n = 4096;
        let g: Vec<_> = (0..n).map(|i| (i % 8) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| (i / 8 % 8) as f64).collect();
        let x: Vec<_> = (0..n).map(|i| (i as f64 * 0.37).sin()).collect();
        let y: Vec<_> = (0..n).map(|i| (2 + i % 5) as f64).collect();
        let mut offset: Vec<_> = (0..n).map(|i| (0.2 + (i % 17) as f64 / 8.0).ln()).collect();
        offset[37] = 5e-7_f64.ln();
        let prepared = prepared_two_fe_with_codes(&y, &x, &g, &h);
        let config = PreparedPpmlIrlsConfig::default();
        profile_final_test_mode(0);
        let fit = fit_prepared_ppml_irls(
            &prepared,
            &y,
            &vec![1.0; n],
            &offset,
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
            None,
        )
        .unwrap();
        assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
        // This balanced Cartesian design has a separate closed-form additive
        // projection, independent of the production FE solver and its weights.
        let d = fit.linear().absorbed_predictor();
        let mut first = [0.0; 8];
        let mut second = [0.0; 8];
        let mut overall = 0.0;
        for i in 0..n {
            first[g[i] as usize] += d[i] / (n / 8) as f64;
            second[h[i] as usize] += d[i] / (n / 8) as f64;
            overall += d[i] / n as f64;
        }
        let error = (0..n)
            .map(|i| (d[i] - first[g[i] as usize] - second[h[i] as usize] + overall).abs())
            .fold(0.0_f64, f64::max);
        assert!(error < 1e-12, "absorbed span error={error:.17e}");
        assert_eq!(profile_final_test_mode(0), 0);
    }

    #[test]
    fn canonical_final_preserves_slope_truth_current_hessian_and_reuse_refusal() {
        let n = 240;
        let g: Vec<_> = (0..n).map(|i| (i / 5 % 4) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| (i / 20 % 3) as f64).collect();
        let x: Vec<_> = (0..n)
            .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
            .collect();
        let eta: Vec<_> = (0..n)
            .map(|i| {
                0.3 + 0.2 * x[i] + g[i] * 0.1 - h[i] * 0.05
                    + (g[i] + h[i]) * ((i % 5) as f64 - 2.) * 0.02
            })
            .collect();
        let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
        let prepared = prepared_joint_slopes(&y, &x, &g, &h);
        let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 13) as f64 / 7.).collect();
        for memory_mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for threads in [1, 4] {
                let mut config = PreparedPpmlIrlsConfig::default();
                config.linear.execution.threads =
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                config.linear.execution.mode = memory_mode;
                config.linear.execution.poolsize = 1;
                config.memory.requested = match memory_mode {
                    PreparedOlsExecutionMode::Fast => crate::options::MemoryMode::Fast,
                    PreparedOlsExecutionMode::Low => crate::options::MemoryMode::Low,
                };
                config.memory.poolsize = 1;
                for mode in [0, 1, 2, 3] {
                    profile_final_test_mode(mode);
                    let fit = fit_prepared_ppml_irls(
                        &prepared,
                        &y,
                        &weights,
                        &vec![0.; n],
                        config,
                        plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                            .unwrap(),
                        None,
                    )
                    .unwrap();
                    assert!(
                        fit.diagnostics().converged
                            && fit.diagnostics().two_consecutive_convergence
                    );
                    assert!(fit.diagnostics().final_score.passed);
                    assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
                    assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
                    validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean())
                        .unwrap();
                    assert_eq!(profile_final_test_mode(0), usize::from(mode == 0));
                }
            }
        }
    }

    #[test]
    fn low_three_fe_audit_certifies_map_window_independently_of_checkpoint_engine() {
        fn stable_probe() -> PpmlAllocatorSnapshot {
            PpmlAllocatorSnapshot {
                current_bytes: 16384,
                peak_bytes: 32768,
                allocation_events: 41,
            }
        }
        let (prepared, y) = prepared_three_fe([0, 1, 2], false, None);
        let mut config = PreparedPpmlIrlsConfig::default();
        config.linear.execution.mode = PreparedOlsExecutionMode::Low;
        config.linear.execution.poolsize = 1;
        config.memory.requested = crate::options::MemoryMode::Low;
        config.memory.poolsize = 1;
        config.allocation_snapshot_probe = Some(stable_probe);
        let fit = fit_prepared_ppml_irls(
            &prepared,
            &y,
            &vec![1.; y.len()],
            &vec![0.; y.len()],
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
            None,
        )
        .unwrap();
        let memory = &fit.diagnostics().memory;
        memory.validate().unwrap();
        let audit = memory.steady_state_allocation_certificate.unwrap();
        assert!(audit.passed && audit.observation.fresh_warm_equivalent);
        assert_eq!(audit.observation.linear_route, FeSolverKind::Map);
        // The admitted centered Schur checkpoint is a coefficient-space MAP
        // accelerator. Its allocating canonical work is outside the audited
        // ordinary window; the separately certified LSMR alternative remains
        // an allowed final route if Schur declines.
        assert_eq!(memory.linear_route, Some(FeSolverKind::Lsmr));
        assert!(memory
            .plan
            .audit_route_matches_final(FeSolverKind::Map, FeSolverKind::Lsmr));
        for route in [FeSolverKind::Auto, FeSolverKind::Cg, FeSolverKind::Cmg] {
            assert!(!memory
                .plan
                .audit_route_matches_final(FeSolverKind::Map, route));
        }
        let mut invalid = memory.plan;
        invalid.solver_workspace_bytes = 0;
        assert!(!invalid.audit_route_matches_final(FeSolverKind::Map, FeSolverKind::Lsmr));
        invalid = memory.plan;
        invalid.dimensions.fe_total_channels += 1;
        assert!(!invalid.audit_route_matches_final(FeSolverKind::Map, FeSolverKind::Lsmr));
    }

    #[test]
    fn low_three_fe_checkpoint_budget_refusal_preserves_joint_owner_and_science() {
        let (prepared, y) = prepared_three_fe([0, 1, 2], false, None);
        let weights: Vec<_> = (0..y.len()).map(|i| 0.2 + (i % 13) as f64).collect();
        let mut config = PreparedPpmlIrlsConfig::default();
        config.linear.execution.mode = PreparedOlsExecutionMode::Low;
        config.linear.execution.poolsize = 1;
        config.memory.requested = crate::options::MemoryMode::Low;
        config.memory.poolsize = 1;
        let plan = plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap();
        let fit = |plan| {
            fit_prepared_ppml_irls(
                &prepared,
                &y,
                &weights,
                &vec![0.; y.len()],
                config,
                plan,
                None,
            )
            .unwrap()
        };
        let admitted = fit(plan);
        assert_eq!(
            admitted.diagnostics().solver.checkpoint,
            super::super::PpmlSolverFamily::Lsmr
        );
        let mut constrained = plan;
        constrained.usable_bytes = Some(admitted.diagnostics().memory.plan.selected_bytes() - 1);
        let declined = fit(constrained);
        assert_eq!(
            declined.diagnostics().solver.checkpoint,
            super::super::PpmlSolverFamily::Map
        );
        assert_eq!(
            admitted.diagnostics().memory.plan.reusable_slope_bytes,
            declined.diagnostics().memory.plan.reusable_slope_bytes
        );
        assert!(declined.diagnostics().memory.plan.reusable_slope_bytes > 0);
        assert!(
            declined.diagnostics().memory.plan.selected_bytes()
                <= constrained.usable_bytes.unwrap()
        );
        assert!(relative_infinity_difference(admitted.eta(), declined.eta()) < 1e-10);
        assert!(
            admitted.diagnostics().final_score.passed && declined.diagnostics().final_score.passed
        );
    }

    #[test]
    fn low_categorical_geometry_refusal_retains_canonical_projection() {
        let (prepared, y) = prepared_three_fe([0, 1, 2], false, None);
        let weights: Vec<_> = (0..y.len()).map(|i| 0.2 + (i % 13) as f64).collect();
        let frame = LinearSolveFrame::from_parts(
            &y,
            BorrowedMapWeights::Provided(&weights),
            ppml_bread_weight_scale(&weights).unwrap(),
        )
        .unwrap();
        let mut config = ppml_linear_config(PreparedPpmlIrlsConfig::default(), 1e-12).unwrap();
        config.execution.mode = PreparedOlsExecutionMode::Low;
        config.execution.poolsize = 1;
        let generations = LinearSessionGenerations::new(1, 1, 1, 1, 1);
        let mut session = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            frame,
            config,
            generations,
            prepared.fit_owned_fe_design().unwrap(),
        )
        .unwrap();
        // Optional certificate-bank refusal must leave the established
        // categorical projector available without a second topology copy.
        session
            .enable_ppml_coefficient_solver(1, u128::MAX, Some(u128::MAX), Some(u128::MAX))
            .unwrap();
        assert!(session.uses_ppml_categorical_map_for_test());
        let actual = session.solve_fresh().unwrap();
        assert!(actual.certified());
        let mut baseline = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            frame,
            config,
            generations,
            prepared.fit_owned_fe_design().unwrap(),
        )
        .unwrap();
        let expected = baseline.solve_fresh().unwrap();
        assert_eq!(actual.frame().residuals(), expected.frame().residuals());
        assert_eq!(actual.frame().bread(), expected.frame().bread());
    }

    #[test]
    fn fast_categorical_lsmr_strict_final_preserves_truth_and_canonical_retry() {
        // Preserve coverage of the fallback when the optional curve is absent.
        let _canonical = CanonicalFinalTestGuard::disabled();
        let _curve = ProfileCurveTestGuard::disabled();
        let n = 240;
        let g: Vec<_> = (0..n).map(|i| (i / 5 % 4) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| (i / 20 % 3) as f64).collect();
        let x: Vec<_> = (0..n)
            .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
            .collect();
        let eta: Vec<_> = (0..n)
            .map(|i| 0.3 + 0.2 * x[i] + g[i] * 0.1 - h[i] * 0.05)
            .collect();
        let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
        let prepared = prepared_two_fe_with_codes(&y, &x, &g, &h);
        let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 13) as f64 / 7.).collect();
        for threads in [1, 4] {
            let mut config = PreparedPpmlIrlsConfig::default();
            config.linear.execution.threads =
                crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            config.solver_plan = PpmlSolverPlan::new(super::super::PpmlSolverFamily::Lsmr);
            for mode in [0, 1, 2, 3, 4] {
                JOINT_FINAL_TEST_MODE.set(mode);
                JOINT_FINAL_TEST_COUNTS.set((0, 0));
                let fit = fit_prepared_ppml_irls(
                    &prepared,
                    &y,
                    &weights,
                    &vec![0.; n],
                    config,
                    plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                    None,
                )
                .unwrap();
                assert!(
                    fit.diagnostics().converged && fit.diagnostics().two_consecutive_convergence
                );
                assert!(fit.diagnostics().final_score.passed);
                assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
                assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
                validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
                match mode {
                    0 => {
                        assert_eq!(JOINT_FINAL_TEST_COUNTS.get().1, 1);
                        assert!(fit.diagnostics().solver.engines()[2].checkpoint_frames >= 1);
                    }
                    1 => assert_eq!(JOINT_FINAL_TEST_COUNTS.get(), (0, 0)),
                    _ => {
                        assert!(JOINT_FINAL_TEST_COUNTS.get().0 >= 1);
                        assert_eq!(JOINT_FINAL_TEST_MODE.get(), 0);
                    }
                }
            }
        }
        JOINT_FINAL_TEST_MODE.set(0);
    }

    #[test]
    fn low_categorical_joint_strict_final_preserves_truth_and_canonical_retry() {
        // Preserve coverage of the fallback when the optional curve is absent.
        let _canonical = CanonicalFinalTestGuard::disabled();
        let _curve = ProfileCurveTestGuard::disabled();
        let n = 240;
        let g: Vec<_> = (0..n).map(|i| (i / 5 % 4) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| (i / 20 % 3) as f64).collect();
        let x: Vec<_> = (0..n)
            .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
            .collect();
        let eta: Vec<_> = (0..n)
            .map(|i| 0.3 + 0.2 * x[i] + g[i] * 0.1 - h[i] * 0.05)
            .collect();
        let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
        let prepared = prepared_two_fe_with_codes(&y, &x, &g, &h);
        let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 13) as f64 / 7.).collect();
        for threads in [1, 4] {
            let mut config = PreparedPpmlIrlsConfig::default();
            config.linear.execution.threads =
                crate::options::ThreadConfig::from_requested(threads, true).unwrap();
            config.linear.execution.mode = PreparedOlsExecutionMode::Low;
            config.linear.execution.poolsize = 1;
            config.memory.requested = crate::options::MemoryMode::Low;
            config.memory.poolsize = 1;
            for mode in [0, 1, 2, 3] {
                JOINT_FINAL_TEST_MODE.set(mode);
                JOINT_FINAL_TEST_COUNTS.set((0, 0));
                let fit = fit_prepared_ppml_irls(
                    &prepared,
                    &y,
                    &weights,
                    &vec![0.; n],
                    config,
                    plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                    None,
                )
                .unwrap();
                assert!(
                    fit.diagnostics().converged && fit.diagnostics().two_consecutive_convergence
                );
                assert!(fit.diagnostics().final_score.passed);
                assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
                assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
                validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
                match mode {
                    0 => assert_eq!(JOINT_FINAL_TEST_COUNTS.get().1, 1),
                    1 => assert_eq!(JOINT_FINAL_TEST_COUNTS.get(), (0, 0)),
                    _ => {
                        assert!(JOINT_FINAL_TEST_COUNTS.get().0 >= 1);
                        assert_eq!(JOINT_FINAL_TEST_MODE.get(), 0);
                    }
                }
            }
        }
        JOINT_FINAL_TEST_MODE.set(0);
    }

    #[test]
    fn fast_categorical_lsmr_three_fe_preserves_canonical_science_nested_rank_and_zero_control() {
        let _canonical = CanonicalFinalTestGuard::disabled();
        for (order, nested, zero_control) in [
            ([0, 1, 2], None, false),
            ([2, 0, 1], Some(0), false),
            ([1, 2, 0], Some(1), true),
        ] {
            let (prepared, _) = prepared_three_fe(order, zero_control, nested);
            let design = prepared.fit_owned_fe_design().unwrap();
            let n = design.nobs();
            let offset: Vec<_> = (0..n).map(|i| (i as f64 / 17.).cos() * 0.1).collect();
            let eta: Vec<_> = (0..n)
                .map(|i| {
                    let x = if zero_control {
                        0.
                    } else {
                        (i as f64 * 0.173).sin() + (i as f64 * 0.071).cos()
                    };
                    offset[i]
                        + 0.3
                        + 0.2 * x
                        + design
                            .effects()
                            .iter()
                            .enumerate()
                            .map(|(j, e)| 0.02 * (j + 1) as f64 * e.codes()[i] as f64)
                            .sum::<f64>()
                })
                .collect();
            let y: Vec<_> = eta
                .iter()
                .enumerate()
                .map(|(i, v)| v.exp() + 0.1 * (i as f64 / 11.).sin())
                .collect();
            let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 17) as f64 / 7.).collect();
            let mut config = PreparedPpmlIrlsConfig::default();
            let reference = fit_prepared_ppml_irls(
                &prepared,
                &y,
                &weights,
                &offset,
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                None,
            )
            .unwrap();
            config.solver_plan = PpmlSolverPlan::new(super::super::PpmlSolverFamily::Lsmr);
            JOINT_FINAL_TEST_COUNTS.set((0, 0));
            let fit = fit_prepared_ppml_irls(
                &prepared,
                &y,
                &weights,
                &offset,
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                None,
            )
            .unwrap();
            assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
            assert_eq!(JOINT_FINAL_TEST_COUNTS.get().1, 1);
            let engine = if zero_control { 1 } else { 2 };
            assert!(fit.diagnostics().solver.engines()[engine].checkpoint_frames >= 1);
            assert!(relative_infinity_difference(fit.eta(), reference.eta()) < 1e-9);
            if !zero_control {
                assert!(
                    (fit.coefficients()[0].value.unwrap()
                        - reference.coefficients()[0].value.unwrap())
                    .abs()
                        < 1e-10
                );
            }
            validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
        }
    }
    #[test]
    fn higher_dimensional_categorical_frames_preserve_weighted_poisson_truth() {
        // Identical pairs have opposite response errors, so every weighted
        // explicit-design score vanishes at eta despite nonzero residuals.
        for (dimensions, nested, zero_control) in
            [(4, false, false), (5, false, false), (4, true, true)]
        {
            let n = 480;
            let x = source(1, "x");
            let effects: Vec<_> = (0..dimensions)
                .map(|j| source((j + 2) as u32, &format!("g{j}")))
                .collect();
            let codes: Vec<Vec<f64>> = (0..dimensions)
                .map(|j| {
                    (0..n)
                        .map(|i| {
                            let row = i / 2;
                            (match j {
                                0 => row % 24,
                                1 => (row * 5 + row / 24) % 9,
                                2 => (row * 7 + row / 11) % 4,
                                3 if nested => (row % 24) / 3,
                                3 => (row * 11 + row / 7) % 5,
                                _ => (row / 3) % 3,
                            }) as f64
                        })
                        .collect()
                })
                .collect();
            let values: Vec<_> = (0..n)
                .map(|i| {
                    if zero_control {
                        0.
                    } else {
                        ((i / 2) as f64 * 0.173).sin()
                    }
                })
                .collect();
            let offset: Vec<_> = (0..n).map(|i| ((i / 2) as f64 / 17.).cos() * 0.1).collect();
            let eta: Vec<_> = (0..n)
                .map(|i| {
                    offset[i]
                        + 0.3
                        + 0.2 * values[i]
                        + codes
                            .iter()
                            .enumerate()
                            .map(|(j, c)| 0.02 * (j + 1) as f64 * c[i])
                            .sum::<f64>()
                })
                .collect();
            let y: Vec<_> = eta
                .iter()
                .enumerate()
                .map(|(i, v)| v.exp() * if i % 2 == 0 { 0.9 } else { 1.1 })
                .collect();
            let weights: Vec<_> = (0..n).map(|i| 0.2 + ((i / 2) % 17) as f64 / 7.).collect();
            let mut sources = vec![x.clone()];
            sources.extend(effects.clone());
            let mut numeric_sources = vec![(x.id, loaded(&values))];
            numeric_sources.extend(effects.iter().zip(&codes).map(|(e, c)| (e.id, loaded(c))));
            let mut bindings = vec![
                OlsInputBinding {
                    stata_var_position: 1,
                    role: OlsInputRole::Candidate,
                },
                OlsInputBinding {
                    stata_var_position: 2,
                    role: OlsInputRole::Outcome,
                },
                OlsInputBinding {
                    stata_var_position: 3,
                    role: OlsInputRole::NumericSource(x.id),
                },
            ];
            bindings.extend(effects.iter().enumerate().map(|(j, e)| OlsInputBinding {
                stata_var_position: (j + 4) as u32,
                role: OlsInputRole::NumericSource(e.id),
            }));
            let prepared = prepare_ols_data(
                &OlsRequestV1 {
                    model: CompiledTermGraph {
                        sources,
                        terms: vec![continuous_term(1, x.id)],
                    },
                    absorbed_terms: effects
                        .iter()
                        .enumerate()
                        .rev()
                        .map(|(j, e)| AbsorbedTerm {
                            id: AbsorbTermId::new((dimensions - j) as u32).unwrap(),
                            intercept: true,
                            categorical_sources: vec![e.id],
                            slope_sources: Vec::new(),
                            label: format!("g{j}"),
                            saved_name: String::new(),
                        })
                        .collect(),
                    bindings,
                    include_intercept: true,
                    keep_singletons: true,
                },
                &LoadedOlsInput {
                    candidate: vec![true; n],
                    outcome: loaded(&y),
                    numeric_sources,
                    analytic_weight: None,
                    frequency_weight: None,
                    probability_weight: None,
                },
            )
            .unwrap();
            for (mode, threads) in [
                (PreparedOlsExecutionMode::Fast, 1),
                (PreparedOlsExecutionMode::Fast, 4),
                (PreparedOlsExecutionMode::Low, 4),
            ] {
                let mut config = PreparedPpmlIrlsConfig::default();
                config.linear.execution.mode = mode;
                config.linear.execution.threads.requested = std::num::NonZeroUsize::new(threads);
                config.memory.requested = if mode == PreparedOlsExecutionMode::Low {
                    crate::options::MemoryMode::Low
                } else {
                    crate::options::MemoryMode::Fast
                };
                let plan =
                    plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap();
                let fit =
                    fit_prepared_ppml_irls(&prepared, &y, &weights, &offset, config, plan, None)
                        .unwrap();
                assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
                assert!(fit.diagnostics().memory.plan.reusable_slope_bytes > 0);
                assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
                if !zero_control {
                    assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
                }
                validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
                if dimensions == 4
                    && !nested
                    && mode == PreparedOlsExecutionMode::Fast
                    && threads == 1
                {
                    let mut constrained = plan;
                    constrained.usable_bytes = Some(plan.selected_bytes());
                    let fallback = fit_prepared_ppml_irls(
                        &prepared,
                        &y,
                        &weights,
                        &offset,
                        config,
                        constrained,
                        None,
                    )
                    .unwrap();
                    assert_eq!(fallback.diagnostics().memory.plan.reusable_slope_bytes, 0);
                    assert!(
                        fallback.diagnostics().memory.plan.selected_bytes()
                            <= constrained.usable_bytes.unwrap()
                    );
                    // Refused optional workspace retains the generic engine;
                    // check its published means against the scientific envelope.
                    for (&actual, &truth) in fallback.mean().iter().zip(&eta) {
                        let expected = truth.exp();
                        assert!((actual - expected).abs() <= 1e-9 + 1e-7 * expected.abs());
                    }
                    validate_final_hessian_frame(
                        fallback.linear().weights(),
                        &weights,
                        fallback.mean(),
                    )
                    .unwrap();
                }
            }
        }
    }

    #[test]
    fn low_categorical_joint_three_fe_preserves_truth_nested_rank_and_zero_control() {
        for (order, nested, zero_control) in [
            ([0, 1, 2], None, false),
            ([2, 0, 1], Some(0), false),
            ([1, 2, 0], Some(1), true),
        ] {
            let (prepared, _) = prepared_three_fe(order, zero_control, nested);
            let design = prepared.fit_owned_fe_design().unwrap();
            let n = design.nobs();
            let offset: Vec<_> = (0..n).map(|i| (i as f64 / 17.).cos() * 0.1).collect();
            let eta: Vec<_> = (0..n)
                .map(|i| {
                    let x = if zero_control {
                        0.
                    } else {
                        (i as f64 * 0.173).sin() + (i as f64 * 0.071).cos()
                    };
                    offset[i]
                        + 0.3
                        + 0.2 * x
                        + design
                            .effects()
                            .iter()
                            .enumerate()
                            .map(|(j, e)| 0.02 * (j + 1) as f64 * e.codes()[i] as f64)
                            .sum::<f64>()
                })
                .collect();
            let y: Vec<_> = eta.iter().map(|v| v.exp()).collect();
            let weights: Vec<_> = (0..n).map(|i| 0.2 + (i % 17) as f64 / 7.).collect();
            let mut config = PreparedPpmlIrlsConfig::default();
            config.linear.execution.mode = PreparedOlsExecutionMode::Low;
            config.linear.execution.poolsize = 1;
            config.memory.requested = crate::options::MemoryMode::Low;
            config.memory.poolsize = 1;
            let fit = fit_prepared_ppml_irls(
                &prepared,
                &y,
                &weights,
                &offset,
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap(),
                None,
            )
            .unwrap();
            assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
            assert!(relative_infinity_difference(fit.eta(), &eta) < 1e-10);
            if !zero_control {
                assert!((fit.coefficients()[0].value.unwrap() - 0.2).abs() < 1e-10);
            }
            validate_final_hessian_frame(fit.linear().weights(), &weights, fit.mean()).unwrap();
        }
    }
    #[test]
    fn joint_slope_session_preserves_changing_frames_rollback_and_fresh_rank() {
        let n = 240;
        let g: Vec<_> = (0..n).map(|i| (i / 5 % 4) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| (i / 20 % 3) as f64).collect();
        let x: Vec<_> = (0..n)
            .map(|i| ((i % 5) as f64 - 2.).powi(2) + (i as f64 / 17.).sin())
            .collect();
        let y: Vec<_> = (0..n)
            .map(|i| 3. + 0.7 * x[i] + g[i] * 0.2 - h[i] * 0.1 + (i as f64 / 13.).cos())
            .collect();
        let prepared = prepared_joint_slopes(&y, &x, &g, &h);
        for (mode, poolsize) in [
            (PreparedOlsExecutionMode::Fast, 10),
            (PreparedOlsExecutionMode::Low, 1),
            (PreparedOlsExecutionMode::Low, 2),
        ] {
            for threads in [1, 4] {
                let mut config =
                    ppml_linear_config(PreparedPpmlIrlsConfig::default(), 1e-12).unwrap();
                config.execution.threads =
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                config.execution.mode = mode;
                config.execution.poolsize = poolsize;
                let initial = vec![1.; n];
                let frame = LinearSolveFrame::from_parts(
                    &y,
                    BorrowedMapWeights::Provided(&initial),
                    ppml_bread_weight_scale(&initial).unwrap(),
                )
                .unwrap();
                let mut session = CertifiedLinearSession::new_with_fe_design(
                    &prepared,
                    frame,
                    config,
                    LinearSessionGenerations::new(1, 1, 1, 1, 1),
                    prepared.fit_owned_fe_design().unwrap(),
                )
                .unwrap();
                session.enable_ppml_certificate_geometry(u128::MAX).unwrap();
                session
                    .enable_ppml_coefficient_solver(
                        threads,
                        u128::MAX,
                        Some(u128::MAX),
                        Some(u128::MAX),
                    )
                    .unwrap();
                session.enable_ppml_joint_slopes(u128::MAX).unwrap();
                assert!(session.solve_fresh().unwrap().certified());
                for generation in 2..7 {
                    let weights: Vec<_> = (0..n)
                        .map(|i| 0.2 + ((i * 13 + generation * 7) % 31) as f64 / 7.)
                        .collect();
                    let response: Vec<_> = (0..n)
                        .map(|i| y[i] + (generation as f64) * 0.2 * (i as f64 / 11.).sin())
                        .collect();
                    let scale = ppml_bread_weight_scale(&weights).unwrap();
                    session
                        .refresh_response(&response, generation as u64)
                        .unwrap();
                    session
                        .refresh_weights(
                            BorrowedMapWeights::Provided(&weights),
                            scale,
                            generation as u64,
                        )
                        .unwrap();
                    let ordinary = session.solve_ordinary_ppml().unwrap();
                    assert!(matches!(ordinary, PpmlOrdinaryLinearOutput::Joint(_)));
                    assert!(ordinary.certified());
                    let actual_beta = ordinary.coefficients().to_vec();
                    let actual_residuals = ordinary.within_residuals().to_vec();
                    let expected_frame = LinearSolveFrame::from_parts(
                        &response,
                        BorrowedMapWeights::Provided(&weights),
                        scale,
                    )
                    .unwrap();
                    let mut expected_session = CertifiedLinearSession::new_with_fe_design(
                        &prepared,
                        expected_frame,
                        config,
                        LinearSessionGenerations::new(1, 1, 1, 1, 1),
                        prepared.fit_owned_fe_design().unwrap(),
                    )
                    .unwrap();
                    let expected = expected_session.solve_fresh().unwrap();
                    assert!(
                        relative_infinity_difference(
                            &actual_beta,
                            &expected
                                .coefficients()
                                .filter(|c| c.generated_column_index.is_some_and(|g| expected
                                    .basis()
                                    .kept_generated_columns()
                                    .contains(&g)))
                                .map(|c| c.value.unwrap())
                                .collect::<Vec<_>>()
                        ) <= 1e-10
                    );
                    assert!(
                        relative_infinity_difference(
                            &actual_residuals,
                            expected.frame().residuals()
                        ) <= 1e-10
                    );
                    let expected_bread = expected.frame().bread().to_vec();
                    // A speculative strict frame may rebuild the canonical within
                    // design, but rollback must leave a valid representative for
                    // another ordinary direction under this frame and the next.
                    let transaction = session.strict_candidate().unwrap();
                    assert!(transaction.candidate().certified());
                    assert!(
                        relative_infinity_difference(
                            transaction.candidate().frame().bread(),
                            &expected_bread
                        ) <= 1e-10
                    );
                    if generation % 2 == 0 {
                        transaction.rollback();
                    } else {
                        transaction.commit();
                    }
                    let repeated = session.solve_ordinary_ppml().unwrap();
                    assert!(repeated.certified());
                    assert!(
                        relative_infinity_difference(repeated.coefficients(), &actual_beta)
                            <= 1e-10
                    );
                    assert!(
                        relative_infinity_difference(
                            repeated.within_residuals(),
                            &actual_residuals
                        ) <= 1e-10
                    );
                    session.discard_invalid_frozen_state();
                    assert!(session.solve_fresh().unwrap().certified());
                }
            }
        }
    }

    #[test]
    fn bitwise_identical_weights_skip_final_frame_control_reprojection() {
        let outcome = vec![1.2, 4.0, 0.5, 6.1, 2.7, 8.3, 5.5, 11.8];
        let x = vec![-2.0, 0.5, -1.0, 2.0, -0.25, 3.0, 1.0, 4.5];
        let response = vec![0.9, 3.8, 0.4, 5.9, 2.5, 8.1, 5.2, 11.4];
        let weights = vec![1.5, 0.75, 2.0, 1.25, 3.5, 0.6, 1.8, 2.2];
        let prepared = prepared_two_fe(&outcome, &x);
        let config = ppml_linear_config(PreparedPpmlIrlsConfig::default(), 1e-10)
            .expect("identical-weight linear config");
        let weight_scale = ppml_bread_weight_scale(&weights).expect("weight scale");
        let frame = LinearSolveFrame::from_parts(
            &outcome,
            BorrowedMapWeights::Provided(&weights),
            weight_scale,
        )
        .expect("identical-weight initial frame");
        let design = prepared.fit_owned_fe_design().expect("two-FE design");
        let mut session = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            frame,
            config,
            LinearSessionGenerations::new(1, 1, 1, 1, 1),
            design,
        )
        .expect("identical-weight session");
        assert!(session.solve_fresh().expect("initial solve").certified());
        assert!(session
            .current_hessian_products_match(BorrowedMapWeights::Provided(&weights), weight_scale, 1)
            .unwrap());
        assert!(!session
            .current_hessian_products_match(BorrowedMapWeights::Provided(&weights), weight_scale, 2)
            .unwrap());
        let mut changed = weights.clone();
        changed[0] = f64::from_bits(changed[0].to_bits() + 1);
        assert!(!session
            .current_hessian_products_match(BorrowedMapWeights::Provided(&changed), weight_scale, 1)
            .unwrap());
        let mut changed_scale = weight_scale;
        changed_scale.reference_weight_multiplier *= 2.0;
        assert!(!session
            .current_hessian_products_match(
                BorrowedMapWeights::Provided(&weights),
                changed_scale,
                1
            )
            .unwrap());
        assert!(session
            .refresh_frame(
                &response,
                BorrowedMapWeights::Provided(&weights),
                weight_scale,
                2,
            )
            .expect("identical weight refresh"));

        let transaction = session
            .strict_candidate()
            .expect("identical-weight strict candidate");
        assert!(!transaction.used_fused_projection_for_test());
        let candidate = transaction.candidate();
        assert!(candidate.certified());
        assert!(candidate.reuse_report().within_design_reused);
        assert!(!candidate.reuse_report().warm_projection_attempted);
        let offset = vec![0.0; outcome.len()];
        let mut actual_eta = vec![0.0; outcome.len()];
        fill_candidate_eta_from_linear_output(&candidate, &offset, &mut actual_eta)
            .expect("identical-weight candidate eta");
        transaction.commit();

        let expected_frame = LinearSolveFrame::from_parts(
            &response,
            BorrowedMapWeights::Provided(&weights),
            weight_scale,
        )
        .expect("identical-weight comparison frame");
        let expected_design = prepared
            .fit_owned_fe_design()
            .expect("comparison FE design");
        let mut expected_session = CertifiedLinearSession::new_with_fe_design(
            &prepared,
            expected_frame,
            config,
            LinearSessionGenerations::new(1, 1, 1, 1, 1),
            expected_design,
        )
        .expect("comparison session");
        let expected = expected_session.solve_fresh().expect("comparison solve");
        let mut expected_eta = vec![0.0; outcome.len()];
        fill_candidate_eta_from_linear_output(&expected, &offset, &mut expected_eta)
            .expect("comparison eta");
        assert!(relative_infinity_difference(&actual_eta, &expected_eta) <= 1e-11);
    }

    #[test]
    fn strict_no_fe_constant_outcome_matches_the_independent_optimum() {
        let rows = 16_384;
        let outcome = vec![2.0; rows];
        let mut columns: Vec<Vec<f64>> = (0..4)
            .map(|column| {
                (0..rows)
                    .map(|row| ((row + 1) as f64 * (column + 1) as f64 * 1.123).sin())
                    .collect()
            })
            .collect();
        for level in 1..7 {
            columns.push(
                (0..rows)
                    .map(|row| f64::from((row + 1) % 7 == level))
                    .collect(),
            );
        }
        let data = prepared_no_fe_categorical(&outcome, &columns);
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let mut config = PreparedPpmlIrlsConfig::default();
            config.ppml.tolerance = 1e-12;
            config.ppml.score_tolerance = 1e-12;
            config.linear.execution.mode = mode;
            config.linear.execution.poolsize = 1;
            config.memory.requested = if mode == PreparedOlsExecutionMode::Fast {
                crate::options::MemoryMode::Fast
            } else {
                crate::options::MemoryMode::Low
            };
            config.memory.poolsize = 1;
            let plan = plan_ppml_memory(memory_dimensions(&data, config), config.memory).unwrap();
            let fit = fit_prepared_ppml_irls(
                &data,
                &outcome,
                &vec![1.0; rows],
                &vec![0.0; rows],
                config,
                plan,
                None,
            )
            .expect("full-rank no-FE constant outcome has a unique finite optimum");
            assert!(fit.diagnostics().converged && fit.diagnostics().final_score.passed);
            assert!(fit.diagnostics().iterations < 40);
            assert_eq!(fit.linear().frame().finite_rank(), 11);
            for (slot, coefficient) in fit.coefficients().iter().enumerate() {
                let expected = if slot == 10 { 2.0_f64.ln() } else { 0.0 };
                assert!(
                    (coefficient.value.expect("full-rank coefficient") - expected).abs() < 1e-13,
                    "mode={mode:?} slot={slot} coefficient={coefficient:?}"
                );
            }
            assert!(fit
                .eta()
                .iter()
                .all(|eta| (eta - 2.0_f64.ln()).abs() < 1e-13));
            assert!(fit.mean().iter().all(|mean| (mean - 2.0).abs() < 2e-13));
        }
    }

    #[test]
    fn persistent_strict_profile_failure_rebuilds_then_returns_bounded_nonconvergence() {
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
            let y: Vec<_> = x
                .iter()
                .enumerate()
                .map(|(row, x)| (0.35_f64 + 0.42 * x).exp() + (row % 3) as f64 * 0.03)
                .collect();
            let prepared = prepared(&y, &[x]);
            let mut config = PreparedPpmlIrlsConfig::default();
            config.linear.execution.mode = mode;
            config.linear.execution.poolsize = 1;
            config.memory.requested = if mode == PreparedOlsExecutionMode::Fast {
                crate::options::MemoryMode::Fast
            } else {
                crate::options::MemoryMode::Low
            };
            config.memory.poolsize = 1;
            let plan =
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory).unwrap();
            REPEATED_STRICT_RESUME_INJECTION.set(100);
            let mut iterations = 0;
            let result = fit_prepared_ppml_irls_interruptible(
                &prepared,
                &y,
                &[1.0; 8],
                &[0.0; 8],
                config,
                plan,
                None,
                &mut |point| {
                    if point == PpmlInterruptPoint::IrlsIteration {
                        iterations += 1;
                    }
                    Ok(())
                },
            );
            REPEATED_STRICT_RESUME_INJECTION.set(0);
            let error = result
                .err()
                .expect("persistent profile corruption must publish nothing");
            assert_eq!(error.code, ErrorCode::NonConvergence);
            assert!(
                error
                    .message
                    .contains("canonical rebuild and two further strict attempts"),
                "{error:?}"
            );
            assert!(iterations < 40, "unbounded retry count: {iterations}");
        }
    }

    #[test]
    fn profile_recovery_is_bounded_but_does_not_penalize_progress_or_changed_bases() {
        let mut recovery = ProfileRecovery::default();
        for difference in [1e-3, 5e-4, 2e-4, 1e-4, 3e-5, 1e-5] {
            assert_eq!(
                recovery.observe(true, true, difference),
                ProfileRecoveryAction::Continue
            );
        }
        recovery = ProfileRecovery::default();
        assert_eq!(
            recovery.observe(true, true, 1e-4),
            ProfileRecoveryAction::Continue
        );
        assert_eq!(
            recovery.observe(true, true, 2e-4),
            ProfileRecoveryAction::Continue
        );
        assert_eq!(
            recovery.observe(true, true, 1e-4),
            ProfileRecoveryAction::Rebuild
        );
        assert_eq!(
            recovery.observe(true, true, 1e-4),
            ProfileRecoveryAction::Continue
        );
        assert_eq!(
            recovery.observe(true, true, 1e-4),
            ProfileRecoveryAction::Continue
        );
        assert_eq!(
            recovery.observe(true, true, 1e-4),
            ProfileRecoveryAction::Exhausted
        );
        assert_eq!(
            recovery.observe(false, false, 1e-4),
            ProfileRecoveryAction::Continue
        );
        assert_eq!(
            recovery.observe(true, true, 1e-4),
            ProfileRecoveryAction::Continue
        );
        assert_eq!(
            recovery.observe(false, true, 1e-4),
            ProfileRecoveryAction::Continue
        );
        assert_eq!(
            recovery.observe(true, true, 1e-4),
            ProfileRecoveryAction::Continue
        );
    }

    #[test]
    fn strict_projection_refinement_is_bounded_and_never_loosens_a_target() {
        for current in [1e-8, 1e-12, 1e-14, 1e-16] {
            let refined = refined_checkpoint_tolerance(current);
            assert!(refined > 0.0 && refined <= current);
        }
        assert_eq!(refined_checkpoint_tolerance(1e-12), 1e-14);
        assert_eq!(refined_checkpoint_tolerance(1e-14), 1e-14);
        assert_eq!(refined_checkpoint_tolerance(1e-16), 1e-16);
    }

    #[test]
    fn strict_checkpoint_movement_resumes_bounded_irls_at_both_checkpoints() {
        for (first_strict, final_hessian) in [(true, false), (false, true)] {
            let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
            let eta = x
                .iter()
                .map(|value| 0.35_f64 + 0.42_f64 * value)
                .collect::<Vec<_>>();
            let outcome = eta
                .iter()
                .enumerate()
                .map(|(row, eta)| eta.exp() + (row as f64 % 3.0) * 0.03)
                .collect::<Vec<_>>();
            let prepared = prepared(&outcome, &[x]);
            let config = PreparedPpmlIrlsConfig::default();
            inject_strict_resume(first_strict, final_hessian);
            let fitted = fit_prepared_ppml_irls(
                &prepared,
                &outcome,
                &[1.0; 8],
                &[0.0; 8],
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                    .expect("strict-resume memory plan"),
                None,
            )
            .expect("strict movement resumes and converges");
            assert!(fitted.diagnostics().converged);
            assert!(
                fitted.diagnostics().strict_profile_rejections >= 1,
                "strict resume injection was not exercised: first={first_strict} final={final_hessian}"
            );
            assert!(fitted.diagnostics().iterations < config.ppml.max_iterations);
            assert!(fitted
                .diagnostics()
                .solver
                .engines()
                .iter()
                .any(|row| row.retries > 0));
            assert!(fitted.diagnostics().two_consecutive_convergence);
            assert!(fitted.diagnostics().final_score.passed);
        }
    }

    #[test]
    fn strict_candidate_rank_predicate_is_shared_by_both_checkpoints() {
        let eta = [0.1, -0.2, 0.3];
        let coefficients = [0.4, -0.1];
        let objective = PoissonObjective {
            deviance: 2.5,
            log_likelihood: -1.25,
        };
        let config = PpmlConfig::default();
        assert_eq!(
            strict_candidate_decision(
                true,
                true,
                2,
                2,
                &eta,
                &coefficients,
                objective,
                &eta,
                &coefficients,
                Some(objective),
                config,
            ),
            FinalHessianSynchronization::Certified
        );
        assert_eq!(
            strict_candidate_decision(
                true,
                true,
                2,
                1,
                &eta,
                &coefficients,
                objective,
                &eta,
                &coefficients,
                Some(objective),
                config,
            ),
            FinalHessianSynchronization::ResumeWithCandidate
        );
        assert_eq!(
            strict_candidate_decision(
                false,
                false,
                2,
                1,
                &eta,
                &coefficients,
                objective,
                &eta,
                &coefficients,
                Some(objective),
                config,
            ),
            FinalHessianSynchronization::ResumeAcceptedState
        );
    }

    #[test]
    fn experimental_strict_guard_ignores_only_nonestimable_coefficient_slots() {
        let eta = [0.1, -0.2, 0.3];
        let coefficients = [0.4, 2.0];
        let moved_inactive = [0.4, 3.0];
        let moved_active = [0.5, 2.0];
        let objective = PoissonObjective {
            deviance: 2.5,
            log_likelihood: -1.25,
        };
        let config = PpmlConfig::default();

        assert_eq!(
            strict_candidate_decision_with_coefficient_slots(
                true,
                true,
                1,
                1,
                &eta,
                &coefficients,
                objective,
                &eta,
                &moved_inactive,
                Some(objective),
                config,
                Some(&[0]),
            ),
            FinalHessianSynchronization::Certified
        );
        assert_eq!(
            strict_candidate_decision_with_coefficient_slots(
                true,
                true,
                1,
                1,
                &eta,
                &coefficients,
                objective,
                &eta,
                &moved_active,
                Some(objective),
                config,
                Some(&[0]),
            ),
            FinalHessianSynchronization::ResumeWithCandidate
        );
        assert_eq!(
            strict_candidate_decision(
                true,
                true,
                1,
                1,
                &eta,
                &coefficients,
                objective,
                &eta,
                &moved_inactive,
                Some(objective),
                config,
            ),
            FinalHessianSynchronization::ResumeWithCandidate
        );
    }

    #[test]
    fn same_rank_basis_pivot_resumes_first_checkpoint_and_fails_final_hessian_closed() {
        let run = |first_strict: bool, final_hessian: bool| {
            let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
            let eta = x
                .iter()
                .map(|value| 0.35_f64 + 0.42_f64 * value)
                .collect::<Vec<_>>();
            let outcome = eta
                .iter()
                .enumerate()
                .map(|(row, eta)| eta.exp() + (row as f64 % 3.0) * 0.03)
                .collect::<Vec<_>>();
            let prepared = prepared(&outcome, &[x]);
            let config = PreparedPpmlIrlsConfig::default();
            inject_strict_basis_change(first_strict, final_hessian);
            let result = fit_prepared_ppml_irls(
                &prepared,
                &outcome,
                &[1.0; 8],
                &[0.0; 8],
                config,
                plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                    .expect("same-rank pivot memory plan"),
                None,
            );
            (result, config)
        };

        let (resumed, config) = run(true, false);
        let resumed = resumed.expect("same-rank first-checkpoint pivot must resume IRLS");
        assert!(resumed.diagnostics().converged);
        assert!(resumed.diagnostics().strict_profile_rejections >= 1);
        assert!(resumed.diagnostics().iterations < config.ppml.max_iterations);

        let (failed, _) = run(false, true);
        let error = failed
            .err()
            .expect("same-rank final-Hessian pivot must never publish");
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert_eq!(
            error.message,
            "prepared PPML final Hessian changed the canonical rank basis"
        );
    }

    #[test]
    fn strict_profile_requires_small_coefficient_newton_accuracy() {
        let eta = [0.1, -0.2, 0.3];
        let objective = PoissonObjective {
            deviance: 2.5,
            log_likelihood: -1.25,
        };
        for beta in [0.0_f64, -2e-4, 3e-3, 0.5, 4.0] {
            let allowance = (1e-11 + 1e-8 * beta.abs()).min(1e-8 * beta.abs().max(1.0));
            for (factor, expected) in [
                (0.5, FinalHessianSynchronization::Certified),
                (2.0, FinalHessianSynchronization::ResumeWithCandidate),
            ] {
                assert_eq!(
                    strict_candidate_decision(
                        true,
                        true,
                        1,
                        1,
                        &eta,
                        &[beta],
                        objective,
                        &eta,
                        &[beta + factor * allowance],
                        Some(objective),
                        PpmlConfig::default(),
                    ),
                    expected,
                    "beta={beta}, factor={factor}"
                );
            }
        }
        // Excluded normalization coordinates have no Newton authority. An
        // invalid active coordinate or slot must still fail closed.
        assert_eq!(
            strict_coefficient_difference(&[0.0, 2.0], &[0.0, 3.0], Some(&[0])),
            0.0
        );
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(strict_coefficient_difference(&[invalid], &[0.0], None).is_infinite());
            assert!(strict_coefficient_difference(&[0.0], &[invalid], Some(&[0])).is_infinite());
        }
        assert!(strict_coefficient_difference(&[0.0], &[], None).is_infinite());
        assert!(strict_coefficient_difference(&[0.0], &[0.0], Some(&[1])).is_infinite());
    }

    #[test]
    fn strict_profile_movement_never_weakens_a_tiny_user_tolerance() {
        let eta = [0.1, -0.2, 0.3];
        let moved_eta = [0.1 + 1e-12, -0.2, 0.3];
        let coefficients = [0.4, -0.1];
        let objective = PoissonObjective {
            deviance: 2.5,
            log_likelihood: -1.25,
        };
        let config = PpmlConfig {
            tolerance: 1e-14,
            score_tolerance: 1e-14,
            ..PpmlConfig::default()
        };
        assert_eq!(
            strict_candidate_decision(
                true,
                true,
                2,
                2,
                &eta,
                &coefficients,
                objective,
                &moved_eta,
                &coefficients,
                Some(objective),
                config,
            ),
            FinalHessianSynchronization::ResumeWithCandidate
        );
    }

    #[test]
    fn prepared_driver_reports_nonconvergence_atomically() {
        let outcome = vec![1.0, 2.0, 1.0, 3.0, 2.0, 4.0];
        let x = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
        let prepared = prepared(&outcome, &[x]);
        let config = PreparedPpmlIrlsConfig {
            ppml: PpmlConfig {
                max_iterations: 1,
                ..PpmlConfig::default()
            },
            ..PreparedPpmlIrlsConfig::default()
        };
        let result = fit_prepared_ppml_irls(
            &prepared,
            &outcome,
            &[1.0; 6],
            &[0.0; 6],
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                .expect("nonconvergence memory plan"),
            None,
        );
        let error = match result {
            Ok(_) => panic!("one iteration cannot satisfy two-pass convergence"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert!(error.message.contains("after 1 iterations"));
    }

    #[test]
    fn intercept_initialization_and_backtracking_move_beta_and_eta_together() {
        let x = vec![-2.0, -1.0, 1.0, 2.0];
        let outcome = vec![0.1, 0.2, 5.0, 10.0];
        let prepared = prepared(&outcome, std::slice::from_ref(&x));
        let config = PreparedPpmlIrlsConfig::default();
        let design = prepared.fit_owned_fe_design().expect("fixture FE design");
        let (coefficients, eta) = initialize(
            &prepared,
            &outcome,
            &[1.0; 4],
            &[0.0; 4],
            &design,
            config,
            None,
            PreparedPpmlIrlsPurpose::Publication,
        )
        .expect("initial state");
        let intercept = prepared
            .fit_model()
            .columns
            .iter()
            .position(|column| matches!(column.kind, PreparedOlsColumnKind::Intercept))
            .expect("intercept generated column");
        let intercept_slot = prepared
            .fit_model()
            .coefficients
            .iter()
            .position(|coefficient| coefficient.generated_column_index == Some(intercept))
            .expect("intercept slot");
        assert!(coefficients[intercept_slot].abs() > 0.1);
        for value in &eta {
            assert!((*value - coefficients[intercept_slot]).abs() <= 64.0 * f64::EPSILON);
        }

        let plan = plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
            .expect("memory plan");
        let mut workspace =
            PpmlIterationWorkspace::from_initial_state(plan, eta, coefficients).expect("workspace");
        workspace
            .refresh_working_state(&outcome, &[1.0; 4], &[0.0; 4], config.ppml.bounds)
            .expect("working state");
        let objective =
            poisson_objective_from_eta(&outcome, workspace.eta(), workspace.mean(), &[1.0; 4])
                .expect("objective");
        let initial_intercept = workspace.coefficients()[intercept_slot];
        let mut candidate = workspace.coefficients().to_vec();
        candidate[0] = 20.0;
        workspace
            .set_candidate_coefficients(&candidate)
            .expect("candidate coefficients");
        let candidate_eta = x
            .iter()
            .map(|value| initial_intercept + 20.0 * value)
            .collect::<Vec<_>>();
        workspace
            .set_candidate_eta(&candidate_eta)
            .expect("candidate eta");
        let accepted = workspace
            .accept_candidate(&outcome, &[1.0; 4], objective, config.ppml)
            .expect("backtracked improving step");
        assert!(accepted.step < 1.0);
        for (row, value) in x.iter().enumerate() {
            let reconstructed =
                workspace.coefficients()[intercept_slot] + workspace.coefficients()[0] * value;
            assert!((workspace.eta()[row] - reconstructed).abs() <= 1e-12);
        }
    }

    #[test]
    fn strict_caller_error_and_unwind_restore_a_usable_prior_session() {
        let x = vec![-2.0, -1.0, 0.5, 1.0, 2.0, 4.0];
        let outcome = vec![1.0, 2.0, 2.5, 4.0, 5.0, 9.0];
        let prepared = prepared(&outcome, &[x.clone(), x]);
        let frame = LinearSolveFrame::from_parts(
            &outcome,
            BorrowedMapWeights::Unit,
            PreparedOlsBreadWeightScale {
                weight_kind: None,
                solver_weight_divisor: 1.0,
                reference_weight_multiplier: 1.0,
                reference_weight_sum: outcome.len() as f64,
            },
        )
        .expect("strict caller frame");

        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let mut linear_config = PreparedOlsFitConfig::default();
            linear_config.execution.mode = mode;
            linear_config.execution.poolsize = 1;
            linear_config.coefficient_tolerance = Some(1e-12);
            let mut session = CertifiedLinearSession::new(
                &prepared,
                frame,
                linear_config,
                LinearSessionGenerations::new(0, 0, 0, 0, 7),
            )
            .expect("strict caller session");
            let initial = session.solve_fresh().expect("strict caller initial fit");
            let original_kept = initial.basis().kept_generated_columns().to_vec();
            let original_dropped = initial.basis().dropped_generated_columns().to_vec();
            let original_dense_identity = session.retained_dense_identity();
            let original_scratch_identity = session
                .retained_scratch_identity()
                .map(std::path::Path::to_path_buf);

            let before_error_capacities = session.workspace_capacities();
            let mut candidate_eta = vec![0.0; outcome.len()];
            let error = (|| -> Result<()> {
                let transaction = session.strict_candidate()?;
                let candidate = transaction.candidate();
                // This is a real fallible caller edge after candidate
                // construction. `?` must drop the still-live transaction.
                fill_candidate_eta_from_linear_output(&candidate, &[], &mut candidate_eta)?;
                transaction.commit();
                Ok(())
            })()
            .expect_err("misaligned candidate fill must fail");
            assert_eq!(error.code, ErrorCode::Internal);
            assert_eq!(session.basis_generation_for_test(), 7);
            assert_eq!(
                session.workspace_capacities(),
                before_error_capacities,
                "error rollback must not allocate or discard reusable capacity",
            );
            assert_eq!(session.retained_dense_identity(), original_dense_identity);
            assert_eq!(
                session.retained_scratch_identity(),
                original_scratch_identity.as_deref(),
            );
            let restored = session
                .output()
                .expect("error rollback restores prior output");
            assert_eq!(restored.basis().kept_generated_columns(), original_kept);
            assert_eq!(
                restored.basis().dropped_generated_columns(),
                original_dropped
            );
            assert!(session
                .solve_frozen()
                .expect("error rollback supports the next frozen solve")
                .certified());
            let fresh_prior = session
                .solve_fresh()
                .expect("establish a certified fresh state before unwind");
            assert_eq!(fresh_prior.basis().kept_generated_columns(), original_kept);
            assert_eq!(
                fresh_prior.basis().dropped_generated_columns(),
                original_dropped
            );

            let before_unwind_capacities = session.workspace_capacities();
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut transaction = session.strict_candidate().expect("strict unwind candidate");
                assert!(
                    transaction.inject_same_rank_basis_pivot(),
                    "duplicate columns provide a speculative same-rank pivot",
                );
                let candidate = transaction.candidate();
                assert!(candidate.certified());
                panic!("injected strict caller unwind");
            }));
            assert!(unwind.is_err());
            assert_eq!(session.basis_generation_for_test(), 7);
            assert_eq!(
                session.workspace_capacities(),
                before_unwind_capacities,
                "unwind rollback must not allocate or discard reusable capacity",
            );
            assert_eq!(session.retained_dense_identity(), original_dense_identity);
            assert_eq!(
                session.retained_scratch_identity(),
                original_scratch_identity.as_deref(),
            );
            let restored = session
                .output()
                .expect("unwind rollback restores prior output");
            assert_eq!(restored.basis().kept_generated_columns(), original_kept);
            assert_eq!(
                restored.basis().dropped_generated_columns(),
                original_dropped
            );
            assert!(session
                .solve_frozen()
                .expect("unwind rollback supports the next frozen solve")
                .certified());
        }
    }

    #[test]
    fn prepared_iteration_report_fits_registered_memory_slot() {
        assert!(
            std::mem::size_of::<PreparedPpmlIterationReport>() as u128 <= DIAGNOSTIC_RECORD_BYTES,
            "prepared iteration report uses {} bytes",
            std::mem::size_of::<PreparedPpmlIterationReport>(),
        );
    }

    #[test]
    fn owned_intercept_fe_design_peak_covers_the_allocated_topology() {
        for codes in [
            vec![vec![0, 0, 1, 1], vec![0, 1, 0, 1]],
            vec![
                vec![0, 0, 1, 1, 2, 2],
                vec![0, 1, 0, 1, 0, 1],
                vec![0, 1, 2, 0, 1, 2],
            ],
        ] {
            let levels: Vec<usize> = codes
                .iter()
                .map(|effect| {
                    effect.iter().copied().max().expect("nonempty test effect") as usize + 1
                })
                .collect();
            let rows = codes[0].len();
            let effects = codes
                .into_iter()
                .zip(&levels)
                .map(|(effect, levels)| {
                    crate::fe::EffectTerm::new(effect, *levels, true, Vec::new())
                        .expect("intercept-only test effect")
                })
                .collect();
            let design = FeDesign::new(rows, effects).expect("intercept-only test design");
            let bound = conservative_owned_intercept_fe_design_peak_bytes(rows, &levels)
                .expect("owned topology peak");
            assert!(bound >= design.storage_capacities().resident_bytes);
        }
    }
}
