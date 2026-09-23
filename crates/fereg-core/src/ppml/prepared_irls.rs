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
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    absorb_fixed_effects, BorrowedMapWeights, CmgConfig, CmgTopologyProfile, FeDesign,
    FeSolverKind, PreparedCmgTopology, SolveReport, CMG_MAX_NUISANCE_COLUMNS,
};
use crate::memory::FeSolverMemoryBudget;
use crate::ols::{
    classify_frozen_linear_fallback, CertifiedFrozenPpmlLinearOutput, CertifiedLinearSession,
    CertifiedLinearSessionOutput, CertifiedLinearSessionProducts, FrozenLinearFallbackKind,
    LinearSessionGenerations, LinearSessionPhaseAllocatorProbe, LinearSessionReuseReport,
    LinearSessionWorkspaceCapacities, LinearSolveFrame, PreparedOlsBreadWeightScale,
    PreparedOlsColumnKind, PreparedOlsExecutionMode, PreparedOlsFitConfig, PreparedOlsFitStatus,
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
use super::family::{means_from_eta, poisson_objective_from_eta, CompensatedSum};
#[cfg(test)]
use super::memory::DIAGNOSTIC_RECORD_BYTES;
use super::memory::{begin_phase_allocation, end_phase_allocation};
#[cfg(test)]
use super::plan_ppml_memory;
use super::prepared_estimator::PpmlInterruptPoint;
use super::{
    scheduled_inner_tolerance, strict_inner_tolerance, PoissonObjective, PpmlAllocatorSnapshot,
    PpmlAllocatorSnapshotProbe, PpmlCertificate, PpmlConfig, PpmlFallbackKind,
    PpmlInnerToleranceProgress, PpmlIterationWorkspace, PpmlLinearWorkspaceCapacityReport,
    PpmlMemoryConfig, PpmlMemoryDimensions, PpmlMemoryPlan, PpmlMemoryRoute, PpmlMemoryTelemetry,
    PpmlPhase, PpmlPhaseAllocatorProbe, PpmlPhaseClock, PpmlSolverEngine, PpmlSolverFrameKind,
    PpmlSolverObservation, PpmlSolverPlan, PpmlSolverReport, PpmlSteadyStateAllocationObservation,
    PpmlWorkspaceCapacityReport, PPML_STEADY_STATE_AUDIT_ITERATIONS,
};

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
        && config.linear.execution.mode == PreparedOlsExecutionMode::Fast
        && use_checkpoint_lsmr_accelerator(observations, effects)
}

#[cfg(test)]
thread_local! {
    static STRICT_RESUME_INJECTION: Cell<u8> = const { Cell::new(0) };
    static REPEATED_STRICT_RESUME_INJECTION: Cell<usize> = const { Cell::new(0) };
    static STRICT_BASIS_CHANGE_INJECTION: Cell<u8> = const { Cell::new(0) };
    static CANONICAL_FRAME_REFRESH_COUNT: Cell<usize> = const { Cell::new(0) };
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
}

#[cfg(test)]
pub(super) fn take_canonical_frame_refresh_count() -> usize {
    CANONICAL_FRAME_REFRESH_COUNT.replace(0)
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
    mean: Vec<f64>,
    certificate: PreparedPpmlMuExplorationCertificate,
}

impl PreparedPpmlMuExploration {
    pub(crate) fn mean(&self) -> &[f64] {
        &self.mean
    }

    pub(crate) const fn certificate(&self) -> PreparedPpmlMuExplorationCertificate {
        self.certificate
    }

    #[cfg(test)]
    pub(crate) fn from_test_eta(
        eta: Vec<f64>,
        outcome: &[f64],
        prior_weights: &[f64],
        bounds: super::PoissonBounds,
    ) -> Result<Self> {
        let mut mean = vec![0.0; eta.len()];
        means_from_eta(&eta, bounds, &mut mean)?;
        let _objective = poisson_objective_from_eta(outcome, &eta, &mean, prior_weights)?;
        Ok(Self {
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
enum PreparedPpmlIrlsPurpose {
    Publication,
    MuExploration { mu_tolerance: f64 },
}

enum PreparedPpmlIrlsRun {
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
    linear: CertifiedLinearSessionProducts,
    coefficients: Vec<PreparedOlsFittedCoefficient>,
    eta: Vec<f64>,
    mean: Vec<f64>,
    score_residuals: Vec<f64>,
    objective: PoissonObjective,
    diagnostics: PreparedPpmlIrlsDiagnostics,
}

impl PreparedPpmlIrlsFit {
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
    fe_moments: Vec<f64>,
    fe_basis_squares: Vec<f64>,
}

impl PpmlScoreWorkspace {
    fn new(design: &FeDesign) -> Result<Self> {
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
                == self.end.linear_workspace,
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
    ) -> Result<()> {
        let probe = self.probe.ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "PPML allocation audit start lacks its allocator probe",
            )
        })?;
        self.start = Some(allocation_audit_start_boundary(session, workspace, probe)?);
        self.measured_iterations = 0;
        self.linear_route = FeSolverKind::Map;
        self.linear_session_allocation_free = true;
        self.iteration_nanoseconds = [0; PPML_STEADY_STATE_AUDIT_ITERATIONS];
        Ok(())
    }

    fn finish_iteration<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        session: &CertifiedLinearSession<'_, P>,
        workspace: &PpmlIterationWorkspace,
        reuse: LinearSessionReuseReport,
        route: FeSolverKind,
        elapsed: Duration,
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
        let end = allocation_audit_end_boundary(session, workspace, probe)?;
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
    // Capture last: the window begins after all boundary evidence has been
    // read and immediately before the next complete IRLS iteration.
    let allocator = probe();
    Ok(PpmlAllocationAuditBoundary {
        allocator,
        ppml_workspace,
        linear_workspace,
        linear_workspace_report,
        scratch_bytes,
    })
}

fn allocation_audit_end_boundary<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &CertifiedLinearSession<'_, P>,
    workspace: &PpmlIterationWorkspace,
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
        scratch_bytes,
    })
}

fn summarize_linear_workspace(
    capacity: LinearSessionWorkspaceCapacities,
) -> Result<PpmlLinearWorkspaceCapacityReport> {
    let elements = [
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
        capacity.frozen_reference_mean_correction_elements,
        capacity.frozen_basis_slot_elements,
        capacity.map_weight_elements,
        capacity.projection_block_elements,
        capacity.within_resident_elements,
        capacity.within_io_buffer_elements,
        capacity.fe_rhs_elements,
        capacity.fe_max_channels,
        capacity.fe_max_level_members,
        capacity.fe_reference_norm_elements,
        capacity.fe_residual_norm_elements,
        capacity.fe_rhs_scale_elements,
        capacity.fe_norm_scale_elements,
        capacity.fe_norm_sum_elements,
        capacity.fe_group_sum_elements,
        capacity.fe_group_correction_elements,
        capacity.fe_projection_gram_elements,
        capacity.fe_projection_rhs_elements,
        capacity.fe_projection_workers,
        capacity.fe_certificate_column_elements,
        capacity.fe_certificate_basis_elements,
        capacity.fe_certificate_geometry_elements,
        capacity.fe_certificate_rank_elements,
        capacity.fe_accelerator_state_elements,
        capacity.fe_topology_effect_elements,
        capacity.fe_topology_code_elements,
        capacity.fe_topology_slope_vector_elements,
        capacity.fe_topology_slope_value_elements,
        capacity.fe_topology_level_offset_elements,
        capacity.fe_topology_level_observation_elements,
    ];
    if capacity.accepted_fit_optional_owners {
        return Err(internal(
            "PPML session retained unexpected inference/FE result storage",
        ));
    }
    let element_capacity_sum = elements
        .iter()
        .chain(capacity.accepted_fit.iter())
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
        fe_topology_resident_bytes: capacity.fe_topology_resident_bytes,
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
        prepared,
        outcome,
        prior_weights,
        offset,
        config,
        memory_plan,
        start,
        PreparedPpmlIrlsPurpose::Publication,
        interrupt,
    )? {
        PreparedPpmlIrlsRun::Publication(fit) => Ok(*fit),
        PreparedPpmlIrlsRun::MuExploration(_) => Err(FeregError::new(
            ErrorCode::Internal,
            "publication PPML run returned an exploratory state",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn explore_prepared_ppml_mu_interruptible<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
    mu_tolerance: f64,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedPpmlMuExploration> {
    match run_prepared_ppml_irls(
        prepared,
        outcome,
        prior_weights,
        offset,
        config,
        memory_plan,
        start,
        PreparedPpmlIrlsPurpose::MuExploration { mu_tolerance },
        interrupt,
    )? {
        PreparedPpmlIrlsRun::MuExploration(exploration) => Ok(*exploration),
        PreparedPpmlIrlsRun::Publication(_) => Err(FeregError::new(
            ErrorCode::Internal,
            "exploratory PPML run returned a publication fit",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_prepared_ppml_irls<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    mut config: PreparedPpmlIrlsConfig,
    mut memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
    purpose: PreparedPpmlIrlsPurpose,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedPpmlIrlsRun> {
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
    if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
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
    config.linear.execution.mode = match memory_plan.selected {
        PpmlMemoryRoute::Fast => PreparedOlsExecutionMode::Fast,
        PpmlMemoryRoute::Low => PreparedOlsExecutionMode::Low,
    };
    config.linear.execution.poolsize = config.memory.poolsize;
    let solver_rhs = memory_plan
        .dimensions
        .finite_columns
        .checked_add(1)
        .ok_or_else(|| allocation("PPML FE solver RHS width overflows"))?;
    let mut solver_admission =
        public_solver_memory_admission(prepared, memory_plan, &config, solver_rhs, purpose)?;
    // A modeled budget veto precedes any specialized solver construction.
    // It is selection, not recovery from allocation or numerical failure.
    if config.solver_plan.requested == super::PpmlSolverFamily::Auto
        && config.solver_plan.selected == super::PpmlSolverFamily::Lsmr
        && memory_plan.usable_bytes.is_some_and(|usable| {
            solver_admission.incremental_bytes
                > usable.saturating_sub(memory_plan.selected_inventory().peak_bytes)
        })
    {
        config.solver_plan.selected = super::PpmlSolverFamily::Map;
        solver_admission = PublicSolverMemoryAdmission::default();
    }
    memory_plan = memory_plan.with_solver_workspace(solver_admission.incremental_bytes)?;
    let solver_execution_bytes = memory_plan
        .selected_inventory()
        .peak_bytes
        .checked_add(solver_admission.owned_fe_design_shortfall_bytes)
        .ok_or_else(|| allocation("PPML FE solver execution memory overflows"))?;
    let solver_memory_budget = FeSolverMemoryBudget {
        execution_bytes: solver_execution_bytes,
        usable_bytes: memory_plan.usable_bytes.unwrap_or(u128::MAX),
    };
    config.ppml.absorb.solver_memory_budget = Some(solver_memory_budget);
    config.linear.absorb.solver_memory_budget = Some(solver_memory_budget);
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
    let projection_rhs = match config.linear.execution.mode {
        PreparedOlsExecutionMode::Fast => solver_rhs,
        PreparedOlsExecutionMode::Low => config
            .memory
            .poolsize
            .max(1)
            .min(memory_plan.dimensions.finite_columns.max(1)),
    };
    let slope_coefficients = if matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
        && config.solver_plan.selected == super::PpmlSolverFamily::Map
        && fe_design.effects().len() > 1
        && fe_design.effects().iter().any(|effect| !effect.slopes().is_empty())
    {
        let mut candidate = memory_plan;
        match crate::fe::ReusableMapAbsorber::certificate_geometry_bytes(&fe_design)
            .and_then(|geometry| crate::fe::ReusableMapAbsorber::coefficient_solver_bytes(
                &fe_design, projection_rhs, effective_threads).map(|workspace| (geometry, workspace + 8 * fe_design.nobs() as u128)))
        {
            Ok((geometry, workspace)) if candidate.admit_reusable_certificate(geometry)
                && candidate.admit_reusable_slope(workspace) => { memory_plan = candidate; true }
            Ok(_) => false,
            Err(error) if error.code == ErrorCode::Allocation => false,
            Err(error) => return Err(error),
        }
    } else { false };
    // Mu exploration has a separate parent separation-phase envelope.
    let projection_plan = if !slope_coefficients && matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
        && config.solver_plan.selected == super::PpmlSolverFamily::Map
    {
        match crate::fe::ParallelProjectionPlan::for_design_and_threads(
            &fe_design,
            projection_rhs,
            effective_threads,
        ) {
            Ok(Some(plan)) if memory_plan.admit_reusable_projection(plan.bytes()?) => Some(plan),
            Ok(_) => None,
            Err(error) if error.code == ErrorCode::Allocation => None,
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    let certificate_geometry = if slope_coefficients { true } else if matches!(purpose, PreparedPpmlIrlsPurpose::Publication)
        && config.solver_plan.selected == super::PpmlSolverFamily::Map
        && fe_design.effects().len() > 1
        && fe_design
            .effects()
            .iter()
            .any(|effect| !effect.slopes().is_empty())
    {
        match crate::fe::ReusableMapAbsorber::certificate_geometry_bytes(&fe_design) {
            Ok(bytes) => memory_plan.admit_reusable_certificate(bytes),
            Err(error) if error.code == ErrorCode::Allocation => false,
            Err(error) => return Err(error),
        }
    } else {
        false
    };
    // Certificate scheduling is optional even after the slope solver fits.
    // A tight budget must retain that solver with its serial fresh check.
    let slope_certificate = if slope_coefficients {
        match crate::fe::ReusableMapAbsorber::coefficient_certificate_bytes(&fe_design, projection_rhs) {
            Ok(bytes) => memory_plan.reusable_slope_bytes.checked_add(bytes)
                .is_some_and(|total| memory_plan.admit_reusable_slope(total)),
            Err(error) if error.code == ErrorCode::Allocation => false,
            Err(error) => return Err(error),
        }
    } else { false };
    // The retained bank cannot consume slack promised to another solver call.
    let solver_memory_budget = FeSolverMemoryBudget {
        execution_bytes: solver_execution_bytes
            .checked_add(memory_plan.reusable_projection_bytes)
            .and_then(|n| n.checked_add(memory_plan.reusable_certificate_bytes))
            .and_then(|n| n.checked_add(memory_plan.reusable_slope_bytes))
            .ok_or_else(|| allocation("PPML projection execution memory overflows"))?,
        ..solver_memory_budget
    };
    config.ppml.absorb.solver_memory_budget = Some(solver_memory_budget);
    config.linear.absorb.solver_memory_budget = Some(solver_memory_budget);
    let mut memory = PpmlMemoryTelemetry::new(memory_plan, requested_threads, effective_threads)?;
    if config.solver_plan.selected == super::PpmlSolverFamily::Cmg {
        memory.expected_solver_workspace_peak = Some(crate::memory::SolverWorkspacePeak::from_bytes(
            solver_admission.checkpoint_workspace_bytes + solver_admission.owned_fe_design_shortfall_bytes,
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
    let mut workspace =
        PpmlIterationWorkspace::from_initial_state(memory_plan, eta, coefficient_values)?;
    workspace.refresh_working_state(outcome, prior_weights, offset, config.ppml.bounds)?;
    let mut objective =
        poisson_objective_from_eta(outcome, workspace.eta(), workspace.mean(), prior_weights)?;
    let mut score_workspace = PpmlScoreWorkspace::new(&fe_design)?;
    let initial_tolerance =
        scheduled_inner_tolerance(config.ppml, 1, config.ppml.inner_tolerance_initial, None);
    let initial_config = ppml_linear_config(config, initial_tolerance)?;
    let frame = LinearSolveFrame::from_parts(
        workspace.working_response(),
        BorrowedMapWeights::Provided(workspace.hessian_weights()),
        ppml_bread_weight_scale(workspace.hessian_weights())?,
    )?;
    let mut session = CertifiedLinearSession::new_with_ppml_fe_design(
        prepared,
        frame,
        initial_config,
        LinearSessionGenerations::new(1, 1, 1, 1, 1),
        fe_design,
    )?;
    if let Some(plan) = projection_plan {
        session.enable_ppml_parallel_projection(plan)?;
    }
    if certificate_geometry {
        session.enable_ppml_certificate_geometry()?;
    }
    if slope_coefficients {
        session.enable_ppml_slope_coefficients(effective_threads, slope_certificate)?;
    }
    session.set_phase_allocator_probe(config.linear_phase_allocation_probe);
    let initial_checkpoint_lsmr = use_initial_checkpoint_lsmr_accelerator(
        &config,
        purpose,
        session.fe_design().nobs(),
        session.fe_design().effects().len(),
    );
    if initial_checkpoint_lsmr {
        // This changes only how the untouched full-width frame is projected.
        // The canonical session still makes the first rank/status decision
        // and independently certifies every downstream linear product.
        session.enable_checkpoint_lsmr();
    }
    let mut joint_lsmr = None;
    let mut joint_lsmr_engine = None;
    let mut fused_cmg = None;
    let mut solver_report = None;
    let retained_limit = MAX_RETAINED_ITERATION_REPORTS
        .min(config.memory.diagnostic_capacity)
        .min(config.ppml.max_iterations);
    let mut retained_iterations = Vec::new();
    retained_iterations
        .try_reserve_exact(retained_limit)
        .map_err(|_| allocation("unable to reserve prepared PPML diagnostics"))?;
    let mut total_step_halvings = 0_usize;
    let mut strict_profile_rejections = 0_usize;
    let mut profile_recovery = ProfileRecovery::default();
    let mut profile_rebuild_pending = false;
    let mut checkpoint_tolerance = strict_inner_tolerance(config.ppml);
    let mut checkpoint_refinement_pending = false;
    let mut consecutive_converged = 0_usize;
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
    let mut converged = false;
    let mut two_consecutive_convergence = false;
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
            if joint_lsmr.is_none() && fused_cmg.is_none() {
                refresh_linear_frame(&mut session, &workspace, frame_generation)?;
            }
        }
        memory.record_phase(PpmlPhase::WeightResponseRefresh, refresh_clock.elapsed());
        record_phase_allocation(
            &mut memory,
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
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
                output.fit().absorption_report(),
            )?;
            let selected_engine = if matches!(purpose, PreparedPpmlIrlsPurpose::Publication) {
                config
                    .solver_plan
                    .selected_engine(output.fit().kept_generated_columns().len())
            } else {
                PpmlSolverEngine::Map
            };
            let mut initial_solver_report =
                PpmlSolverReport::new(config.solver_plan, selected_engine);
            if initial_checkpoint_lsmr {
                initial_solver_report.enable_lsmr_checkpoint();
                record_checkpoint_solver_observation(
                    &mut initial_solver_report,
                    output.fit().absorption_report(),
                    false,
                )?;
            } else {
                initial_solver_report.observe(map_solver_observation(
                    output.fit().absorption_report(),
                    PpmlSolverFrameKind::Checkpoint,
                ))?;
            }
            fill_candidate_eta_from_linear_output(&output, offset, workspace.candidate_eta_mut())?;
            fill_candidate_coefficients_from_linear_output(
                &output,
                workspace.candidate_coefficients_mut(),
            )?;
            let specialized_columns = (selected_engine != PpmlSolverEngine::Map)
                .then(|| {
                    fallible_copy(
                        output.fit().kept_generated_columns(),
                        "unable to reserve PPML specialized active-column identity",
                    )
                })
                .transpose()?;
            let result = (
                output.fit().finite_rank(),
                output.reuse_report(),
                output.fit().absorption_report().used_solver,
            );
            match selected_engine {
                PpmlSolverEngine::Map => {}
                PpmlSolverEngine::LsmrJoint | PpmlSolverEngine::LsmrFused => {
                    let active_columns = specialized_columns.as_ref().ok_or_else(|| {
                        FeregError::new(
                            ErrorCode::Internal,
                            "joint PPML LSMR requires the frozen active basis",
                        )
                    })?;
                    joint_lsmr = Some(ReusableCategoricalJointLsmr::new_with_threads(
                        prepared,
                        active_columns,
                        session.fe_design(),
                        config.linear.execution.threads,
                        joint_lsmr_policy(session.fe_design().effects().len()),
                    )?);
                    joint_lsmr_engine = Some(selected_engine);
                }
                PpmlSolverEngine::CmgPrepared => {
                    let active_columns = specialized_columns.as_ref().ok_or_else(|| {
                        FeregError::new(
                            ErrorCode::Internal,
                            "fused PPML CMG requires the frozen active basis",
                        )
                    })?;
                    fused_cmg = Some(ReusableCategoricalFwlCmg::new(
                        prepared,
                        active_columns,
                        session.fe_design(),
                        FusedFwlCmgConfig {
                            map: config.ppml.absorb.map,
                            threads: config.linear.execution.threads,
                            batch_size: solver_admission.cmg_batch_capacity,
                            solver_memory_budget,
                        },
                    )?);
                }
            }
            if selected_engine != PpmlSolverEngine::Map
                && !initial_checkpoint_lsmr
                && use_checkpoint_lsmr_accelerator(
                    session.fe_design().nobs(),
                    session.fe_design().effects().len(),
                )
            {
                session.enable_checkpoint_lsmr();
                initial_solver_report.enable_lsmr_checkpoint();
            }
            solver_report = Some(initial_solver_report);
            if let Some(cmg) = fused_cmg.as_ref() {
                record_cmg_heap_forecast(&mut memory, cmg, solver_admission)?;
            }
            result
        } else if let Some(joint) = joint_lsmr.as_mut() {
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
                .observe(joint_solver_observation(
                    report,
                    joint_lsmr_engine.ok_or_else(|| {
                        internal("PPML joint LSMR state lacks its public engine identity")
                    })?,
                ))?;
            rank_frozen = true;
            (
                joint.active_rank(),
                LinearSessionReuseReport::ppml_specialized(
                    report.setup_seconds + report.solve_seconds,
                    report.certification_seconds,
                ),
                FeSolverKind::Lsmr,
            )
        } else if fused_cmg.is_some() {
            let attempt = fused_cmg
                .as_mut()
                .expect("fused CMG presence checked")
                .solve(
                    prepared,
                    session.fe_design(),
                    workspace.working_response(),
                    workspace.hessian_weights(),
                    inner_tolerance,
                )?;
            record_cmg_heap_forecast(&mut memory, fused_cmg.as_ref().expect("CMG attempt retained"), solver_admission)?;
            match attempt {
                FusedFwlCmgSolveOutcome::Certified(report) => {
                    let cmg = fused_cmg
                        .as_ref()
                        .expect("successful fused CMG is retained");
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
                    solver_report
                        .as_mut()
                        .ok_or_else(|| internal("PPML CMG fallback lacks solver diagnostics"))?
                        .record_cmg_numerical_fallback(cmg_failure_observation(report))?;
                    fused_cmg = None;
                    memory.record_fallback(PpmlFallbackKind::Route);
                    // The specialized route deliberately leaves the canonical
                    // MAP session stale between checkpoints. Rebind it to this
                    // exact frame before the one permitted numerical fallback.
                    refresh_linear_frame(&mut session, &workspace, frame_generation)?;
                    let canonical = session.solve_fresh()?;
                    if !canonical.certified() {
                        return Err(numerical_nonconvergence(
                            "prepared PPML MAP fallback after fused CMG did not certify",
                        ));
                    }
                    certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
                    accumulate_inner_iterations(
                        &mut total_inner_iterations,
                        canonical.fit().absorption_report(),
                    )?;
                    solver_report
                        .as_mut()
                        .ok_or_else(|| internal("PPML MAP fallback lacks solver diagnostics"))?
                        .observe(map_solver_observation(
                            canonical.fit().absorption_report(),
                            PpmlSolverFrameKind::Ordinary,
                        ))?;
                    fill_candidate_eta_from_linear_output(
                        &canonical,
                        offset,
                        workspace.candidate_eta_mut(),
                    )?;
                    fill_candidate_coefficients_from_linear_output(
                        &canonical,
                        workspace.candidate_coefficients_mut(),
                    )?;
                    rank_frozen = true;
                    (
                        canonical.fit().finite_rank(),
                        canonical.reuse_report(),
                        canonical.fit().absorption_report().used_solver,
                    )
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
            let frozen_attempt = match session.solve_frozen_ppml() {
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
                        fully_certified: output.certificate().passed
                            && output.within_certificate().passed
                            && output.reconstruction_certificate().passed,
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
                    let frozen_objective = workspace.evaluate_candidate_objective(
                        outcome,
                        prior_weights,
                        config.ppml.bounds,
                    )?;
                    record_linear_phase_timings(&mut memory, frozen_reuse)?;
                    // This canonical same-frame solve is deliberately after the
                    // end allocator snapshot. It may allocate and therefore can
                    // validate warm/fresh science without contaminating the
                    // warmed steady-state window.
                    let canonical = session.solve_fresh()?;
                    accumulate_inner_iterations(
                        &mut total_inner_iterations,
                        canonical.fit().absorption_report(),
                    )?;
                    solver_report
                        .as_mut()
                        .ok_or_else(|| internal("PPML MAP equivalence solve lacks diagnostics"))?
                        .observe(map_solver_observation(
                            canonical.fit().absorption_report(),
                            PpmlSolverFrameKind::Ordinary,
                        ))?;
                    let canonical_route = canonical.fit().absorption_report().used_solver;
                    let mut equivalent = frozen_certified
                        && frozen_route == FeSolverKind::Map
                        && canonical_route == frozen_route
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
                    let canonical_objective = workspace.evaluate_candidate_objective(
                        outcome,
                        prior_weights,
                        config.ppml.bounds,
                    )?;
                    equivalent &= relative_scalar_difference(
                        frozen_objective.deviance,
                        canonical_objective.deviance,
                    ) <= fresh_warm_tolerance(config.ppml, inner_tolerance)
                        && relative_scalar_difference(
                            frozen_objective.log_likelihood,
                            canonical_objective.log_likelihood,
                        ) <= fresh_warm_tolerance(config.ppml, inner_tolerance);
                    let canonical_rank = canonical.fit().finite_rank();
                    let canonical_reuse = canonical.reuse_report();
                    let observation = allocation_audit.complete_equivalence(equivalent)?;
                    memory.observe_steady_state_allocation(observation)?;
                    (canonical_rank, canonical_reuse, canonical_route)
                }
            } else if let FrozenAttempt::Fallback(kind) = frozen_attempt {
                session.discard_invalid_frozen_state();
                memory.record_fallback(ppml_fallback_kind(kind));
                let canonical = session.solve_fresh()?;
                if !canonical.certified() {
                    return Err(numerical_nonconvergence(
                        "prepared PPML canonical fallback did not certify",
                    ));
                }
                certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
                accumulate_inner_iterations(
                    &mut total_inner_iterations,
                    canonical.fit().absorption_report(),
                )?;
                solver_report
                    .as_mut()
                    .ok_or_else(|| internal("PPML MAP fallback lacks solver diagnostics"))?
                    .observe(map_solver_observation(
                        canonical.fit().absorption_report(),
                        PpmlSolverFrameKind::Ordinary,
                    ))?;
                fill_candidate_eta_from_linear_output(
                    &canonical,
                    offset,
                    workspace.candidate_eta_mut(),
                )?;
                fill_candidate_coefficients_from_linear_output(
                    &canonical,
                    workspace.candidate_coefficients_mut(),
                )?;
                if allocation_audit.needs_equivalence() {
                    let observation = allocation_audit.complete_equivalence(false)?;
                    memory.observe_steady_state_allocation(observation)?;
                }
                (
                    canonical.fit().finite_rank(),
                    canonical.reuse_report(),
                    canonical.fit().absorption_report().used_solver,
                )
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
            match workspace.accept_candidate_interruptible(
                outcome,
                prior_weights,
                objective,
                config.ppml,
                &mut poll_line_search,
            ) {
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
        if let Some(joint) = joint_lsmr.as_mut() {
            joint.accept_step(accepted.step);
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
        if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
            super::categorical_lsmr::emit_solver_trace_line(&format!(
                "FEREG_SOLVER_TRACE|phase=irls_convergence|iteration={iteration}|deviance_change={relative_change:.17e}|score={:.17e}|passed={}|step={:.17e}",
                final_score.relative_infinity_norm, u8::from(iteration_converged), accepted.step,
            ));
        }
        consecutive_converged = if iteration_converged {
            consecutive_converged.saturating_add(1)
        } else {
            0
        };
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
        if consecutive_converged >= 2 && !audit_defers_strict {
            interrupt(PpmlInterruptPoint::StrictFinalSolve)?;
            begin_phase_allocation(config.phase_allocation_probe, PpmlPhase::StrictFinalSolve);
            let strict_clock = PpmlPhaseClock::start();
            // The strict solve must be constructed from the newly accepted
            // eta, never from the pre-step working frame still in the session.
            workspace.refresh_working_state(outcome, prior_weights, offset, config.ppml.bounds)?;
            frame_generation = next_frame_generation(frame_generation)?;
            refresh_linear_frame(&mut session, &workspace, frame_generation)?;
            let strict_tolerance = checkpoint_tolerance;
            previous_inner_tolerance = previous_inner_tolerance.min(strict_tolerance);
            session.set_config(ppml_linear_config(config, strict_tolerance)?)?;
            let specialized_engine = if joint_lsmr.is_some() {
                joint_lsmr_engine
            } else if fused_cmg.is_some() {
                Some(PpmlSolverEngine::CmgPrepared)
            } else {
                None
            };
            if profile_rebuild_pending {
                let mut poll = || interrupt(PpmlInterruptPoint::StrictFinalSolve);
                session.rebuild_retained_inputs_interruptible(&mut poll)?;
                memory.record_fallback(PpmlFallbackKind::WarmProjection);
                checkpoint_refinement_pending = true;
                profile_rebuild_pending = false;
            }
            let strict_transaction = session.strict_candidate()?;
            let strict_basis_unchanged = strict_transaction.basis_unchanged();
            #[cfg(test)]
            let strict_basis_unchanged = strict_basis_unchanged && !take_strict_basis_change(1);
            let (strict_certified, strict_rank, strict_active_columns) = {
                let strict = strict_transaction.candidate();
                let strict_certified = strict.certified();
                let strict_rank = strict.fit().finite_rank();
                let strict_active_columns =
                    if !strict_basis_unchanged && specialized_engine.is_some() {
                        Some(fallible_copy(
                            strict.fit().kept_generated_columns(),
                            "unable to reserve PPML strict active-column identity",
                        )?)
                    } else {
                        None
                    };
                accumulate_inner_iterations(
                    &mut total_inner_iterations,
                    strict.fit().absorption_report(),
                )?;
                record_checkpoint_solver_observation(
                    solver_report
                        .as_mut()
                        .ok_or_else(|| internal("PPML strict solve lacks solver diagnostics"))?,
                    strict.fit().absorption_report(),
                    checkpoint_refinement_pending,
                )?;
                fill_candidate_eta_from_linear_output(
                    &strict,
                    offset,
                    workspace.candidate_eta_mut(),
                )?;
                fill_candidate_coefficients_from_linear_output(
                    &strict,
                    workspace.candidate_coefficients_mut(),
                )?;
                (strict_certified, strict_rank, strict_active_columns)
            };
            #[cfg(test)]
            let injected_strict_movement = take_strict_resume(1);
            #[cfg(test)]
            if injected_strict_movement {
                inject_candidate_profile_movement(&mut workspace);
            }
            let strict_objective = match workspace.evaluate_candidate_objective(
                outcome,
                prior_weights,
                config.ppml.bounds,
            ) {
                Ok(objective) => Some(objective),
                Err(error) if error.code == ErrorCode::NonConvergence => None,
                Err(error) => return Err(error),
            };
            let active_coefficient_slots = joint_lsmr
                .as_ref()
                .map(ReusableCategoricalJointLsmr::active_slots)
                .or_else(|| {
                    fused_cmg
                        .as_ref()
                        .map(ReusableCategoricalFwlCmg::active_slots)
                });
            let strict_consistent = strict_candidate_decision_with_coefficient_slots(
                strict_certified,
                strict_basis_unchanged,
                finite_rank,
                strict_rank,
                workspace.eta(),
                workspace.coefficients(),
                objective,
                workspace.candidate_eta(),
                workspace.candidate_coefficients(),
                strict_objective,
                config.ppml,
                active_coefficient_slots,
            ) == FinalHessianSynchronization::Certified;

            if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
                let mut worst_row = 0_usize;
                let mut worst_difference = 0.0_f64;
                let mut failing_rows = 0_usize;
                let tolerance = config
                    .ppml
                    .tolerance
                    .min(config.ppml.score_tolerance)
                    .min(1e-8);
                for (row, (&accepted, &candidate)) in workspace
                    .eta()
                    .iter()
                    .zip(workspace.candidate_eta())
                    .enumerate()
                {
                    let difference = relative_scalar_difference(accepted, candidate);
                    failing_rows += usize::from(difference > tolerance);
                    if difference > worst_difference {
                        worst_row = row;
                        worst_difference = difference;
                    }
                }
                let strict = strict_transaction.candidate();
                let row = worst_row;
                super::categorical_lsmr::emit_solver_trace_line(&format!(
                    "FEREG_SOLVER_TRACE|phase=strict_worst_row|iteration={iteration}|seconds={:.6}|original_row={}|failing_rows={failing_rows}|tolerance={tolerance:.17e}|accepted_eta={:.17e}|candidate_eta={:.17e}|working_response={:.17e}|residual={:.17e}|finite_predictor={:.17e}|absorbed_predictor={:.17e}|outcome={:.17e}|mean={:.17e}|hessian_weight={:.17e}|deviance_change={:.17e}|ll_change={:.17e}",
                    strict_clock.elapsed().as_secs_f64(), prepared.fit_sample().retained_rows()[row] + 1,
                    workspace.eta()[row], workspace.candidate_eta()[row], strict.response()[row],
                    strict.fit().residuals()[row], strict.finite_predictor()[row], strict.absorbed_predictor()[row],
                    outcome[row], workspace.mean()[row], workspace.hessian_weights()[row],
                    strict_objective.map_or(f64::INFINITY, |candidate| relative_scalar_difference(objective.deviance, candidate.deviance)),
                    strict_objective.map_or(f64::INFINITY, |candidate| relative_scalar_difference(objective.log_likelihood, candidate.log_likelihood)),
                ));
                super::categorical_lsmr::emit_solver_trace_line(&format!(
                    "FEREG_SOLVER_TRACE|phase=strict_decision|iteration={iteration}|certified={}|basis_unchanged={}|rank={strict_rank}|eta_change={:.17e}|beta_change={:.17e}|consistent={}",
                    u8::from(strict_certified), u8::from(strict_basis_unchanged),
                    relative_infinity_difference(workspace.eta(), workspace.candidate_eta()),
                    relative_infinity_difference(workspace.coefficients(), workspace.candidate_coefficients()),
                    u8::from(strict_consistent),
                ));
            }

            let mut profile_difference = if strict_consistent {
                0.0
            } else {
                strict_objective.map_or(f64::INFINITY, |candidate| {
                    strict_profile_difference(
                        workspace.eta(),
                        workspace.coefficients(),
                        objective,
                        workspace.candidate_eta(),
                        workspace.candidate_coefficients(),
                        candidate,
                        active_coefficient_slots,
                    )
                })
            };
            let mut accepted_strict_basis_change = false;
            let mut profile_rejected = strict_certified
                && strict_basis_unchanged
                && strict_rank == finite_rank
                && !strict_consistent;
            checkpoint_refinement_pending = false;
            let mut strict_committed = false;
            if strict_consistent {
                // Publish the strict state itself. A unit trial copies the
                // already-certified strict eta/beta and its evaluated means
                // into the accepted buffers without allocating.
                workspace.fill_line_search_trial(1.0, config.ppml.bounds)?;
                workspace.accept_trial()?;
                objective = strict_objective.expect("consistent strict objective is present");
                strict_transaction.commit();
                strict_committed = true;
            } else {
                // ADR 0016 requires a changed strict solution to become the
                // next IRLS state. Preserve the stable-objective invariant by
                // backtracking beta and eta together when necessary.
                let strict_step = if strict_certified && strict_objective.is_some() {
                    let mut poll_line_search =
                        || interrupt(PpmlInterruptPoint::IrlsLineSearchTrial);
                    workspace.accept_candidate_interruptible(
                        outcome,
                        prior_weights,
                        objective,
                        config.ppml,
                        &mut poll_line_search,
                    )
                } else {
                    Err(numerical_nonconvergence(
                        "prepared PPML strict final weighted-linear solve did not certify",
                    ))
                };
                match strict_step {
                    Ok(strict_step) => {
                        let updated_step_halvings = total_step_halvings
                            .checked_add(strict_step.step_halvings)
                            .ok_or_else(|| {
                                FeregError::new(
                                    ErrorCode::Internal,
                                    "prepared PPML strict step-halving count overflowed",
                                )
                            })?;
                        objective = strict_step.objective;
                        total_step_halvings = updated_step_halvings;
                        strict_transaction.commit();
                        strict_committed = true;
                        accepted_strict_basis_change = strict_active_columns.is_some();
                    }
                    Err(error) if error.code == ErrorCode::NonConvergence => {
                        strict_transaction.rollback();
                        // The last accepted PPML state remains authoritative.
                        // A strict profile that cannot improve it invalidates
                        // convergence, not the fit transaction; the bounded
                        // outer loop resumes from that accepted state.
                    }
                    Err(error) => return Err(error),
                }
            }
            if strict_committed {
                if let Some(joint) = joint_lsmr.as_mut() {
                    joint.reset_warm_start();
                }
                if let Some(cmg) = fused_cmg.as_mut() {
                    cmg.reset_warm_start();
                }
            }
            if accepted_strict_basis_change {
                let active_columns = strict_active_columns.as_deref().ok_or_else(|| {
                    internal("accepted PPML strict basis change lacks active-column identity")
                })?;
                // The committed MAP basis is now canonical. Drop the stale
                // specialized workspace before its fallible replacement so a
                // checkpoint pivot does not double the route's memory peak.
                match specialized_engine {
                    Some(PpmlSolverEngine::LsmrJoint | PpmlSolverEngine::LsmrFused) => {
                        drop(joint_lsmr.take());
                        joint_lsmr = Some(ReusableCategoricalJointLsmr::new_with_threads(
                            prepared,
                            active_columns,
                            session.fe_design(),
                            config.linear.execution.threads,
                            joint_lsmr_policy(session.fe_design().effects().len()),
                        )?);
                    }
                    Some(PpmlSolverEngine::CmgPrepared) => {
                        drop(fused_cmg.take());
                        fused_cmg = Some(ReusableCategoricalFwlCmg::new(
                            prepared,
                            active_columns,
                            session.fe_design(),
                            FusedFwlCmgConfig {
                                map: config.ppml.absorb.map,
                                threads: config.linear.execution.threads,
                                batch_size: solver_admission.cmg_batch_capacity,
                                solver_memory_budget,
                            },
                        )?);
                    }
                    Some(PpmlSolverEngine::Map) | None => {
                        return Err(internal(
                            "accepted PPML strict basis change lacks a specialized engine",
                        ));
                    }
                }
                if let Some(cmg) = fused_cmg.as_ref() {
                    record_cmg_heap_forecast(&mut memory, cmg, solver_admission)?;
                }
            }
            final_score = prepared_score_certificate(
                prepared,
                session.fe_design(),
                outcome,
                workspace.mean(),
                prior_weights,
                config.ppml.score_tolerance,
                &mut score_workspace,
            )?;
            if strict_consistent && final_score.passed {
                interrupt(PpmlInterruptPoint::StrictFinalSolve)?;
                let synchronized = synchronize_final_hessian_frame(
                    &mut session,
                    &mut workspace,
                    outcome,
                    prior_weights,
                    offset,
                    config,
                    checkpoint_tolerance,
                    &mut frame_generation,
                    &mut total_inner_iterations,
                    finite_rank,
                    objective,
                )?;
                if synchronized == FinalHessianSynchronization::Certified {
                    two_consecutive_convergence = consecutive_converged >= 2;
                    converged = true;
                } else if synchronized == FinalHessianSynchronization::ResumeWithCandidate {
                    profile_rejected = true;
                    let candidate_objective = workspace.evaluate_candidate_objective(
                        outcome,
                        prior_weights,
                        config.ppml.bounds,
                    )?;
                    profile_difference = strict_profile_difference(
                        workspace.eta(),
                        workspace.coefficients(),
                        objective,
                        workspace.candidate_eta(),
                        workspace.candidate_coefficients(),
                        candidate_objective,
                        None,
                    );
                    let mut poll_line_search =
                        || interrupt(PpmlInterruptPoint::IrlsLineSearchTrial);
                    match workspace.accept_candidate_interruptible(
                        outcome,
                        prior_weights,
                        objective,
                        config.ppml,
                        &mut poll_line_search,
                    ) {
                        Ok(strict_step) => {
                            objective = strict_step.objective;
                            total_step_halvings = total_step_halvings
                                .checked_add(strict_step.step_halvings)
                                .ok_or_else(|| {
                                    FeregError::new(
                                        ErrorCode::Internal,
                                        "prepared PPML Hessian-resume step-halving count overflowed",
                                    )
                                })?;
                        }
                        Err(error) if error.code == ErrorCode::NonConvergence => {}
                        Err(error) => return Err(error),
                    }
                    final_score = prepared_score_certificate(
                        prepared,
                        session.fe_design(),
                        outcome,
                        workspace.mean(),
                        prior_weights,
                        config.ppml.score_tolerance,
                        &mut score_workspace,
                    )?;
                }
            }
            if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
                super::categorical_lsmr::emit_solver_trace_line(&format!(
                    "FEREG_SOLVER_TRACE|phase=strict_result|iteration={iteration}|committed={}|score={:.17e}|score_passed={}|converged={}",
                    u8::from(strict_committed), final_score.relative_infinity_norm,
                    u8::from(final_score.passed), u8::from(converged),
                ));
            }
            memory.record_phase(PpmlPhase::StrictFinalSolve, strict_clock.elapsed());
            record_phase_allocation(
                &mut memory,
                config.phase_allocation_probe,
                PpmlPhase::StrictFinalSolve,
            );
            if converged {
                exploration_terminal = PreparedPpmlMuTerminal::Converged;
                break;
            }
            if profile_rejected {
                let refined = refined_checkpoint_tolerance(checkpoint_tolerance);
                checkpoint_refinement_pending = refined < checkpoint_tolerance;
                if checkpoint_refinement_pending && std::env::var_os("FEREG_SOLVER_TRACE").is_some()
                {
                    super::categorical_lsmr::emit_solver_trace_line(&format!(
                        "FEREG_SOLVER_TRACE|phase=checkpoint_profile_refinement|previous={checkpoint_tolerance:.17e}|next={refined:.17e}",
                    ));
                }
                checkpoint_tolerance = refined;
                previous_inner_tolerance = previous_inner_tolerance.min(refined);
            }
            // Entry already required two accepted objective/score passes.
            // A rejected candidate may move its own score away from the root;
            // that must not erase evidence of this repeated checkpoint cycle.
            let recovery_action = profile_recovery.observe(
                profile_rejected
                    && strict_basis_unchanged
                    && strict_rank == finite_rank
                    && refined_checkpoint_tolerance(strict_tolerance) == strict_tolerance,
                strict_basis_unchanged && strict_rank == finite_rank,
                profile_difference,
            );
            if std::env::var_os("FEREG_SOLVER_TRACE").is_some()
                && recovery_action != ProfileRecoveryAction::Continue
            {
                super::categorical_lsmr::emit_solver_trace_line(&format!(
                    "FEREG_SOLVER_TRACE|phase=profile_recovery|iteration={iteration}|action={recovery_action:?}|difference={profile_difference:.17e}|tolerance={checkpoint_tolerance:.17e}",
                ));
            }
            match recovery_action {
                ProfileRecoveryAction::Continue => {}
                ProfileRecoveryAction::Rebuild => {
                    profile_rebuild_pending = true;
                }
                ProfileRecoveryAction::Exhausted => {
                    return Err(FeregError::new(ErrorCode::NonConvergence, format!(
                        "prepared PPML strict profile stagnated after canonical rebuild and two further strict attempts; iteration={iteration}, difference={profile_difference:.16e}, inner_tolerance={checkpoint_tolerance:.3e}, score={:.3e}",
                        final_score.relative_infinity_norm,
                    )));
                }
            }
            strict_profile_rejections =
                strict_profile_rejections.checked_add(1).ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "prepared PPML strict-profile rejection count overflowed",
                    )
                })?;
            consecutive_converged = 0;
        }
        begin_phase_allocation(
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
        let working_refresh_clock = PpmlPhaseClock::start();
        workspace.refresh_working_state(outcome, prior_weights, offset, config.ppml.bounds)?;
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
            )?;
        } else if audit_can_start {
            allocation_audit.start_after(&session, &workspace)?;
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
            PreparedPpmlMuExploration { mean, certificate },
        )));
    }

    if !converged {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            format!(
                "prepared PPML failed to converge after {} iterations; deviance={:.16e}, score={:.3e}, strict_profile_rejections={strict_profile_rejections}",
                config.ppml.max_iterations,
                objective.deviance,
                final_score.relative_infinity_norm,
            ),
        ));
    }
    let final_output = session.output()?;
    record_checkpoint_solver_observation(
        solver_report
            .as_mut()
            .ok_or_else(|| internal("PPML final Hessian lacks solver diagnostics"))?,
        final_output.fit().absorption_report(),
        false,
    )?;
    let coefficients = final_coefficients(final_output.fit(), workspace.coefficients())?;
    let retained_memory = final_output.retained_within_memory();
    memory.observe_scratch(retained_memory.scratch_bytes);
    memory.observe_linear_route(final_output.fit().absorption_report().used_solver);
    memory.observe_diagnostic_records(retained_iterations.len())?;
    memory.certify_workspace(&workspace)?;
    workspace.refresh_score_residuals(outcome)?;
    let (_coefficient_values, eta, mean, score_residuals) = workspace.into_final_buffers();
    let linear = session.into_current_products()?;
    validate_final_hessian_frame(linear.weights(), prior_weights, &mean)?;
    let final_generations = linear.generations();
    if final_generations.response != frame_generation
        || final_generations.weights != frame_generation
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML final linear products have stale frame generations",
        ));
    }
    Ok(PreparedPpmlIrlsRun::Publication(Box::new(
        PreparedPpmlIrlsFit {
            linear,
            coefficients,
            eta,
            mean,
            score_residuals,
            objective,
            diagnostics: PreparedPpmlIrlsDiagnostics {
                converged,
                two_consecutive_convergence,
                rank_frozen,
                iterations: completed_iterations,
                total_inner_iterations,
                retained_iterations,
                total_step_halvings,
                strict_profile_rejections,
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
    let peak = cmg.expected_heap_peak(admission.checkpoint_workspace_bytes)?
        .with_retained(admission.owned_fe_design_shortfall_bytes)?;
    memory.expected_solver_workspace_peak = Some(
        memory.expected_solver_workspace_peak.unwrap_or(crate::memory::SolverWorkspacePeak::ZERO).max(peak),
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

#[allow(clippy::too_many_arguments)]
fn synchronize_final_hessian_frame<P: PreparedOlsFitView + Sync + ?Sized>(
    session: &mut CertifiedLinearSession<'_, P>,
    workspace: &mut PpmlIterationWorkspace,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    checkpoint_tolerance: f64,
    frame_generation: &mut u64,
    total_inner_iterations: &mut usize,
    expected_rank: usize,
    accepted_objective: PoissonObjective,
) -> Result<FinalHessianSynchronization> {
    workspace.refresh_working_state(outcome, prior_weights, offset, config.ppml.bounds)?;
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
        let synchronized_rank = synchronized.fit().finite_rank();
        accumulate_inner_iterations(
            total_inner_iterations,
            synchronized.fit().absorption_report(),
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
    let coefficient_difference =
        relative_infinity_difference(workspace.coefficients(), workspace.candidate_coefficients());
    #[cfg(test)]
    let injected_movement = take_strict_resume(2);
    #[cfg(test)]
    if injected_movement {
        inject_candidate_profile_movement(workspace);
    }
    let candidate_objective =
        match workspace.evaluate_candidate_objective(outcome, prior_weights, config.ppml.bounds) {
            Ok(objective) => objective,
            Err(error) if error.code == ErrorCode::NonConvergence => {
                synchronized_transaction.rollback();
                return Ok(FinalHessianSynchronization::ResumeAcceptedState);
            }
            Err(error) => return Err(error),
        };
    let decision = strict_candidate_decision(
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
    );
    let decision = if decision == FinalHessianSynchronization::Certified
        && (predictor_difference > config.ppml.tolerance
            || coefficient_difference > config.ppml.tolerance)
    {
        FinalHessianSynchronization::ResumeWithCandidate
    } else {
        decision
    };
    if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
        super::categorical_lsmr::emit_solver_trace_line(&format!(
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
    let shift = if prepared.fit_report_intercept() {
        log_weighted_outcome_sum(outcome, prior_weights)?
            - log_weighted_offset_sum(offset, prior_weights)?
    } else {
        0.0
    };
    if prepared.fit_report_intercept() {
        let mut found = false;
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
                if found {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "prepared PPML model contains multiple reported structural constants",
                    ));
                }
                coefficient_values[position] = shift;
                found = true;
            }
        }
        if !found {
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
    // An absorbed constant needs the global sum only when the group-optimal
    // seed was unavailable. Its final normalization is owned by certification.
    let shift = if prepared.fit_report_intercept() {
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
        || output.fit().residuals().len() != offset.len()
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
        eta[row] = offset[row] + (output.response()[row] - output.fit().residuals()[row]);
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
    if coefficients.len() != output.fit().coefficients().len() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML linear coefficient output has inconsistent dimensions",
        ));
    }
    for (target, source) in coefficients.iter_mut().zip(output.fit().coefficients()) {
        *target = source.value.unwrap_or(0.0);
    }
    Ok(())
}

fn fill_candidate_eta_from_frozen_output(
    output: &CertifiedFrozenPpmlLinearOutput<'_>,
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
    output: &CertifiedFrozenPpmlLinearOutput<'_>,
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
        || fresh.fit().residuals().len() != offset.len()
        || fresh.finite_predictor().len() != offset.len()
        || fresh.absorbed_predictor().len() != offset.len()
        || frozen_eta.len() != offset.len()
        || fresh.fit().coefficients().len() != frozen_coefficients.len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML fresh/warm audit dimensions are inconsistent",
        ));
    }
    let tolerance = fresh_warm_tolerance(config, inner_tolerance);
    let mut predictor_difference = 0.0_f64;
    for row in 0..offset.len() {
        let value = offset[row] + (fresh.response()[row] - fresh.fit().residuals()[row]);
        if !value.is_finite() || !frozen_eta[row].is_finite() {
            return Ok(false);
        }
        predictor_difference = predictor_difference
            .max((value - frozen_eta[row]).abs() / value.abs().max(frozen_eta[row].abs()).max(1.0));
    }
    let mut coefficient_difference = 0.0_f64;
    for (fresh, frozen) in fresh.fit().coefficients().iter().zip(frozen_coefficients) {
        let value = fresh.value.unwrap_or(0.0);
        if !value.is_finite() || !frozen.is_finite() {
            return Ok(false);
        }
        coefficient_difference = coefficient_difference
            .max((value - frozen).abs() / value.abs().max(frozen.abs()).max(1.0));
    }
    Ok(fresh.certified()
        && fresh.fit().finite_rank() == frozen_rank
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
        coefficient_slots.map_or_else(
            || relative_infinity_difference(coefficients, strict_coefficients),
            |slots| relative_infinity_difference_at_slots(coefficients, strict_coefficients, slots),
        ),
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
    fit: &crate::ols::CertifiedPreparedOlsFit,
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
    for (coefficient, value) in fit.coefficients().iter().zip(values) {
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
    for column in 0..prepared.fit_model().columns.len() {
        let mut moment = CompensatedSum::default();
        let mut basis_square = CompensatedSum::default();
        for row in 0..rows {
            let basis = prepared.fit_column_value(column, row)?;
            moment.add(prior_weights[row] * (outcome[row] - mean[row]) * basis);
            basis_square.add(prior_weights[row] * mean[row] * basis * basis);
        }
        let moment = moment.total();
        finite_absolute = finite_absolute.max(moment.abs());
        add_relative_score(
            moment,
            basis_square.total().sqrt(),
            score_scale,
            &mut relative_infinity,
            &mut relative_square,
            &mut relative_count,
        );
    }
    workspace.fe_moments.fill(0.0);
    workspace.fe_basis_squares.fill(0.0);
    let mut effect_offset = 0_usize;
    for effect in fe_design.effects() {
        let directions = effect.n_levels().saturating_mul(effect.channels());
        for row in 0..rows {
            let level = effect.codes()[row] as usize;
            for channel in 0..effect.channels() {
                let direction = effect_offset + level * effect.channels() + channel;
                let basis = effect.basis_value(channel, row);
                workspace.fe_moments[direction] +=
                    prior_weights[row] * (outcome[row] - mean[row]) * basis;
                workspace.fe_basis_squares[direction] +=
                    prior_weights[row] * mean[row] * basis * basis;
            }
        }
        for direction in effect_offset..effect_offset + directions {
            let moment = workspace.fe_moments[direction];
            fe_absolute = fe_absolute.max(moment.abs());
            add_relative_score(
                moment,
                workspace.fe_basis_squares[direction].sqrt(),
                score_scale,
                &mut relative_infinity,
                &mut relative_square,
                &mut relative_count,
            );
        }
        effect_offset += directions;
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

fn fallible_filled<T: Clone>(length: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| allocation(message))?;
    values.resize(length, value);
    Ok(values)
}

fn fallible_copy<T: Copy>(source: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(source.len())
        .map_err(|_| allocation(message))?;
    values.extend_from_slice(source);
    Ok(values)
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn internal(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}

fn invalid_data(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
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
        prepare_ols_data(
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
        )
        .expect("prepared fixture")
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

    #[test]
    fn refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model() {
        for hot in [30.0_f64] {
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
            config.memory.requested = crate::options::MemoryMode::Low;
            config.memory.poolsize = 1;
            config.linear.execution.mode = PreparedOlsExecutionMode::Low;
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
            assert_eq!(fit.diagnostics().strict_profile_rejections, 1);
            assert!(relative_infinity_difference(fit.eta(), &truth) < 1e-10);
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

    fn memory_dimensions(
        prepared: &PreparedOlsData,
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
    fn initial_checkpoint_lsmr_eligibility_is_narrow_and_route_explicit() {
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
            assert!(!use_initial_checkpoint_lsmr_accelerator(
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
            certificate.measured_iterations,
            PPML_STEADY_STATE_AUDIT_ITERATIONS
        );
        assert_eq!(certificate.allocation_events, 0);
        assert!(certificate.workspace_capacity_stable);
        assert!(certificate.linear_workspace_capacity_stable);
        assert!(certificate.scratch_stable);
        assert!(certificate.linear_session_allocation_free);
        assert!(certificate.fresh_warm_equivalent);
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
        assert_eq!(certificate.allocator_growth_bytes, 0);
        assert_eq!(certificate.allocator_peak_growth_bytes, 0);
        assert!(!certificate.passed);
        assert!(!fitted.diagnostics().memory.whole_iteration_allocation_free);
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

    #[test]
    fn prepared_driver_matches_explicit_dense_poisson_without_fixed_effects() {
        let x = vec![-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
        let eta: Vec<f64> = x
            .iter()
            .map(|value| 0.35_f64 + 0.42_f64 * value)
            .collect::<Vec<_>>();
        let outcome = eta
            .iter()
            .enumerate()
            .map(|(row, eta)| eta.exp() + (row as f64 % 3.0) * 0.03)
            .collect::<Vec<_>>();
        let offset = vec![0.0; outcome.len()];
        let weights = vec![1.0; outcome.len()];
        let prepared = prepared(&outcome, std::slice::from_ref(&x));
        let config = PreparedPpmlIrlsConfig::default();
        let fitted = fit_prepared_ppml_irls(
            &prepared,
            &outcome,
            &weights,
            &offset,
            config,
            plan_ppml_memory(memory_dimensions(&prepared, config), config.memory)
                .expect("prepared-driver memory plan"),
            None,
        )
        .expect("prepared PPML");
        let explicit_design =
            DenseMatrix::from_columns(&[x, vec![1.0; outcome.len()]]).expect("explicit design");
        let explicit = super::super::fit_ppml(
            &outcome,
            &explicit_design,
            &weights,
            &offset,
            None,
            &[],
            PpmlConfig::default(),
        )
        .expect("explicit dense PPML oracle");
        assert!(fitted.diagnostics().converged);
        assert!(fitted.diagnostics().iterations >= 2);
        assert!(fitted.diagnostics().final_score.passed);
        let mut previous_inner_tolerance = config.ppml.inner_tolerance_initial;
        let mut previous_outer_progress = None;
        for report in &fitted.diagnostics().retained_iterations {
            let expected = scheduled_inner_tolerance(
                config.ppml,
                report.iteration,
                previous_inner_tolerance,
                previous_outer_progress,
            );
            assert_eq!(report.inner_tolerance.to_bits(), expected.to_bits());
            previous_inner_tolerance = report.inner_tolerance;
            previous_outer_progress = Some(PpmlInnerToleranceProgress::new(
                report.relative_deviance_change,
                report.score.relative_infinity_norm,
            ));
        }
        assert!(fitted.linear().generations().response > fitted.diagnostics().iterations as u64);
        assert!(fitted
            .coefficients()
            .iter()
            .filter_map(|coefficient| coefficient.value)
            .all(f64::is_finite));
        assert!(fitted
            .mean()
            .iter()
            .all(|mean| mean.is_finite() && *mean > 0.0));
        assert_eq!(fitted.eta().len(), outcome.len());
        assert_eq!(fitted.score_residuals().len(), outcome.len());
        let reported = fitted
            .coefficients()
            .iter()
            .map(|coefficient| coefficient.value.unwrap_or(0.0))
            .collect::<Vec<_>>();
        assert_eq!(reported.len(), explicit.coefficients.len());
        for (actual, expected) in reported.iter().zip(&explicit.coefficients) {
            assert!((actual - expected).abs() <= 5e-9 * expected.abs().max(1.0));
        }
        for (actual, expected) in fitted.eta().iter().zip(&explicit.linear_predictor) {
            assert!((actual - expected).abs() <= 5e-9 * expected.abs().max(1.0));
        }
        for (actual, expected) in fitted.mean().iter().zip(&explicit.mean) {
            assert!((actual - expected).abs() <= 5e-9 * expected.abs().max(1.0));
        }
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
            let mut session = CertifiedLinearSession::new_with_ppml_fe_design(
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
                    session.workspace_capacities().fe_rhs_elements,
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
            let actual_bread = actual.fit().bread().values().to_vec();
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
            assert!(
                relative_infinity_difference(&actual_bread, expected.fit().bread().values())
                    <= 1e-10
            );
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
        let mut session = CertifiedLinearSession::new_with_ppml_fe_design(
            &prepared,
            frame,
            config,
            LinearSessionGenerations::new(1, 1, 1, 1, 1),
            design,
        )
        .expect("identical-weight session");
        assert!(session.solve_fresh().expect("initial solve").certified());
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
        let mut expected_session = CertifiedLinearSession::new_with_ppml_fe_design(
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
            let mut session = CertifiedLinearSession::new(
                &prepared,
                frame,
                linear_config,
                LinearSessionGenerations::new(0, 0, 0, 0, 7),
            )
            .expect("strict caller session");
            let initial = session.solve_fresh().expect("strict caller initial fit");
            let original_kept = initial.fit().kept_generated_columns().to_vec();
            let original_dropped = initial.fit().dropped_generated_columns().to_vec();
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
            assert_eq!(restored.fit().kept_generated_columns(), original_kept);
            assert_eq!(restored.fit().dropped_generated_columns(), original_dropped);
            assert!(session
                .solve_frozen()
                .expect("error rollback supports the next frozen solve")
                .certified());
            let fresh_prior = session
                .solve_fresh()
                .expect("establish a certified fresh state before unwind");
            assert_eq!(fresh_prior.fit().kept_generated_columns(), original_kept);
            assert_eq!(
                fresh_prior.fit().dropped_generated_columns(),
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
            assert_eq!(restored.fit().kept_generated_columns(), original_kept);
            assert_eq!(restored.fit().dropped_generated_columns(), original_dropped);
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
