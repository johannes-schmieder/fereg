//! PPML IRLS over the reusable certified prepared weighted-linear session.
//!
//! PPML owns initialization, likelihood, convergence, rank-freezing, and
//! publication policy. The shared session owns only a certified weighted
//! linear subproblem on an immutable prepared model.

#[cfg(test)]
use std::cell::Cell;
use std::time::{Duration, Instant};

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{absorb_fixed_effects, BorrowedMapWeights, FeDesign, FeSolverKind, SolveReport};
use crate::ols::{
    classify_frozen_linear_fallback, CertifiedFrozenLinearOutput, CertifiedLinearSession,
    CertifiedLinearSessionOutput, CertifiedLinearSessionProducts, FrozenLinearFallbackKind,
    LinearSessionGenerations, LinearSessionPhaseAllocatorProbe, LinearSessionReuseReport,
    LinearSessionWorkspaceCapacities, LinearSolveFrame, PreparedOlsBreadWeightScale,
    PreparedOlsColumnKind, PreparedOlsExecutionMode, PreparedOlsFitConfig, PreparedOlsFitStatus,
    PreparedOlsFittedCoefficient,
};
use crate::prepared_fit_view::PreparedOlsFitView;

use super::family::{means_from_eta, poisson_objective_from_eta, CompensatedSum};
use super::memory::{begin_phase_allocation, end_phase_allocation};
#[cfg(test)]
use super::plan_ppml_memory;
use super::prepared_estimator::PpmlInterruptPoint;
use super::{
    scheduled_inner_tolerance, strict_inner_tolerance, PoissonObjective, PpmlAllocatorSnapshot,
    PpmlAllocatorSnapshotProbe, PpmlCertificate, PpmlConfig, PpmlFallbackKind,
    PpmlInnerToleranceProgress, PpmlIterationWorkspace, PpmlLinearWorkspaceCapacityReport,
    PpmlMemoryConfig, PpmlMemoryDimensions, PpmlMemoryPlan, PpmlMemoryRoute, PpmlMemoryTelemetry,
    PpmlPhase, PpmlPhaseAllocatorProbe, PpmlPhaseClock, PpmlSteadyStateAllocationObservation,
    PpmlWorkspaceCapacityReport, PPML_STEADY_STATE_AUDIT_ITERATIONS,
};

const MAX_RETAINED_ITERATION_REPORTS: usize = 256;

#[cfg(test)]
thread_local! {
    static STRICT_RESUME_INJECTION: Cell<u8> = const { Cell::new(0) };
    static STRICT_BASIS_CHANGE_INJECTION: Cell<u8> = const { Cell::new(0) };
}

#[cfg(test)]
fn inject_strict_resume(first_strict: bool, final_hessian: bool) {
    STRICT_RESUME_INJECTION.set(u8::from(first_strict) | (u8::from(final_hessian) << 1));
}

#[cfg(test)]
fn take_strict_resume(bit: u8) -> bool {
    let state = STRICT_RESUME_INJECTION.get();
    let selected = state & bit != 0;
    STRICT_RESUME_INJECTION.set(state & !bit);
    selected
}

#[cfg(test)]
fn inject_strict_basis_change(first_strict: bool, final_hessian: bool) {
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
    /// Developer-only opt-in. CMG cannot enter PPML auto routing before its
    /// changing-weight qualification is complete.
    pub(crate) allow_experimental_cmg: bool,
}

impl Default for PreparedPpmlIrlsConfig {
    fn default() -> Self {
        let ppml = PpmlConfig::default();
        let mut absorb = ppml.absorb;
        if absorb.solver == FeSolverKind::Auto {
            absorb.solver = FeSolverKind::Map;
        }
        let linear = PreparedOlsFitConfig {
            absorb,
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
            allow_experimental_cmg: false,
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
        capacity.fe_certificate_column_elements,
        capacity.fe_certificate_basis_elements,
        capacity.fe_topology_effect_elements,
        capacity.fe_topology_code_elements,
        capacity.fe_topology_slope_vector_elements,
        capacity.fe_topology_slope_value_elements,
        capacity.fe_topology_level_offset_elements,
        capacity.fe_topology_level_observation_elements,
    ];
    let element_capacity_sum = elements.iter().try_fold(0_u128, |sum, value| {
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

/// Follow the certified bounded IRLS trajectory without constructing any
/// publication products. This is intentionally usable only by Mu separation.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn explore_prepared_ppml_mu<P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &P,
    outcome: &[f64],
    prior_weights: &[f64],
    offset: &[f64],
    config: PreparedPpmlIrlsConfig,
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
    mu_tolerance: f64,
) -> Result<PreparedPpmlMuExploration> {
    explore_prepared_ppml_mu_interruptible(
        prepared,
        outcome,
        prior_weights,
        offset,
        config,
        memory_plan,
        start,
        mu_tolerance,
        &mut |_| Ok(()),
    )
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
    memory_plan: PpmlMemoryPlan,
    start: Option<&PreparedPpmlStart>,
    purpose: PreparedPpmlIrlsPurpose,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedPpmlIrlsRun> {
    begin_phase_allocation(config.phase_allocation_probe, PpmlPhase::Preparation);
    let preparation_clock = PpmlPhaseClock::start();
    validate_inputs(prepared, outcome, prior_weights, offset, config)?;
    validate_memory_dimensions(prepared, memory_plan.dimensions, config)?;
    // Route selection happened exactly once on generation zero, before any
    // separation detector workspace. The composite owner rebases only the
    // exact final dimensions into that conservative fit-lifetime plan.
    config.linear.execution.mode = match memory_plan.selected {
        PpmlMemoryRoute::Fast => PreparedOlsExecutionMode::Fast,
        PpmlMemoryRoute::Low => PreparedOlsExecutionMode::Low,
    };
    config.linear.execution.poolsize = config.memory.poolsize;
    let requested_threads = config
        .linear
        .execution
        .threads
        .requested
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(0);
    let effective_threads = config.linear.execution.threads.resolved();
    let mut memory = PpmlMemoryTelemetry::new(memory_plan, requested_threads, effective_threads)?;

    let fe_design = prepared.fit_owned_fe_design()?;
    let (coefficient_values, eta) = initialize(
        prepared,
        outcome,
        prior_weights,
        offset,
        &fe_design,
        config,
        start,
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
    let mut session = CertifiedLinearSession::new_with_fe_design(
        prepared,
        frame,
        initial_config,
        LinearSessionGenerations::new(1, 1, 1, 1, 1),
        fe_design,
    )?;
    session.set_phase_allocator_probe(config.linear_phase_allocation_probe);
    let retained_limit = MAX_RETAINED_ITERATION_REPORTS
        .min(config.memory.diagnostic_capacity)
        .min(config.ppml.max_iterations);
    let mut retained_iterations = Vec::new();
    retained_iterations
        .try_reserve_exact(retained_limit)
        .map_err(|_| allocation("unable to reserve prepared PPML diagnostics"))?;
    let mut total_step_halvings = 0_usize;
    let mut strict_profile_rejections = 0_usize;
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
            refresh_linear_frame(&mut session, &workspace, frame_generation)?;
        }
        memory.record_phase(PpmlPhase::WeightResponseRefresh, refresh_clock.elapsed());
        record_phase_allocation(
            &mut memory,
            config.phase_allocation_probe,
            PpmlPhase::WeightResponseRefresh,
        );
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
            fill_candidate_eta_from_linear_output(&output, offset, workspace.candidate_eta_mut())?;
            fill_candidate_coefficients_from_linear_output(
                &output,
                workspace.candidate_coefficients_mut(),
            )?;
            (
                output.fit().finite_rank(),
                output.reuse_report(),
                output.fit().absorption_report().used_solver,
            )
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
            let frozen_attempt = match session.solve_frozen() {
                Ok(output) if output.certified() => {
                    certified_linear_subproblems = certified_linear_subproblems.saturating_add(1);
                    accumulate_inner_iterations(
                        &mut total_inner_iterations,
                        output.response_absorption_report(),
                    )?;
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
        workspace.refresh_score_residuals(outcome)?;
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
            let strict_tolerance = strict_inner_tolerance(config.ppml);
            previous_inner_tolerance = previous_inner_tolerance.min(strict_tolerance);
            session.set_config(ppml_linear_config(config, strict_tolerance)?)?;
            let strict_transaction = session.strict_candidate()?;
            let strict_basis_unchanged = strict_transaction.basis_unchanged();
            #[cfg(test)]
            let strict_basis_unchanged = strict_basis_unchanged && !take_strict_basis_change(1);
            let (strict_certified, strict_rank) = {
                let strict = strict_transaction.candidate();
                let strict_certified = strict.certified();
                let strict_rank = strict.fit().finite_rank();
                accumulate_inner_iterations(
                    &mut total_inner_iterations,
                    strict.fit().absorption_report(),
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
                (strict_certified, strict_rank)
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
            let strict_consistent = strict_candidate_decision(
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
            ) == FinalHessianSynchronization::Certified;

            if strict_consistent {
                // Publish the strict state itself. A unit trial copies the
                // already-certified strict eta/beta and its evaluated means
                // into the accepted buffers without allocating.
                workspace.fill_line_search_trial(1.0, config.ppml.bounds)?;
                workspace.accept_trial()?;
                objective = strict_objective.expect("consistent strict objective is present");
                strict_transaction.commit();
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
            workspace.refresh_score_residuals(outcome)?;
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
                    &mut frame_generation,
                    &mut total_inner_iterations,
                    finite_rank,
                    objective,
                )?;
                if synchronized == FinalHessianSynchronization::Certified {
                    two_consecutive_convergence = consecutive_converged >= 2;
                    converged = true;
                } else if synchronized == FinalHessianSynchronization::ResumeWithCandidate {
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
                    workspace.refresh_score_residuals(outcome)?;
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
        workspace.refresh_score_residuals(outcome)?;
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
                memory,
            },
        },
    )))
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
    if config.linear.absorb.solver == FeSolverKind::Cmg && !config.allow_experimental_cmg {
        return Err(invalid_argument(
            "CMG is not qualified for automatic prepared PPML routing",
        ));
    }
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
    )
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
    frame_generation: &mut u64,
    total_inner_iterations: &mut usize,
    expected_rank: usize,
    accepted_objective: PoissonObjective,
) -> Result<FinalHessianSynchronization> {
    workspace.refresh_working_state(outcome, prior_weights, offset, config.ppml.bounds)?;
    workspace.prepare_final_hessian_response(offset)?;
    *frame_generation = next_frame_generation(*frame_generation)?;
    refresh_linear_frame(session, workspace, *frame_generation)?;
    let strict_tolerance = strict_inner_tolerance(config.ppml);
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
    let shift = log_weighted_outcome_sum(outcome, prior_weights)?
        - log_weighted_offset_sum(offset, prior_weights)?;
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
    if linear.absorb.solver == FeSolverKind::Cmg && !config.allow_experimental_cmg {
        return Err(invalid_argument("CMG is not qualified for prepared PPML"));
    }
    linear.fixed_effect_collinearity_tolerance = config.ppml.fixed_effect_collinearity_tolerance;
    Ok(strict_absorb(linear, inner_tolerance))
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
    if output.finite_predictor().len() != offset.len()
        || output.absorbed_predictor().len() != offset.len()
        || eta.len() != offset.len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML linear output has inconsistent dimensions",
        ));
    }
    for row in 0..offset.len() {
        eta[row] = offset[row] + output.finite_predictor()[row] + output.absorbed_predictor()[row];
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
    output: &CertifiedFrozenLinearOutput<'_>,
    offset: &[f64],
    eta: &mut [f64],
) -> Result<()> {
    if output.finite_predictor().len() != offset.len()
        || output.absorbed_predictor().len() != offset.len()
        || output.response().len() != offset.len()
        || eta.len() != offset.len()
        || output.coefficients().len() != output.basis().kept_generated_columns().len()
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "prepared PPML frozen output has inconsistent dimensions",
        ));
    }
    for row in 0..offset.len() {
        eta[row] = offset[row] + output.finite_predictor()[row] + output.absorbed_predictor()[row];
        if !eta[row].is_finite() {
            return Err(numerical_nonconvergence(
                "prepared PPML frozen candidate predictor is not finite",
            ));
        }
    }
    Ok(())
}

fn fill_candidate_coefficients_from_frozen_output(
    output: &CertifiedFrozenLinearOutput<'_>,
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
    if fresh.finite_predictor().len() != offset.len()
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
        let value = offset[row] + fresh.finite_predictor()[row] + fresh.absorbed_predictor()[row];
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

fn strict_profile_consistent(
    eta: &[f64],
    coefficients: &[f64],
    objective: PoissonObjective,
    strict_eta: &[f64],
    strict_coefficients: &[f64],
    strict_objective: PoissonObjective,
    config: PpmlConfig,
) -> bool {
    // Publication may be stricter than the attainable inner-solve or machine
    // floor. In that case fail closed instead of silently weakening the
    // caller's requested outer/score tolerance.
    let tolerance = config.tolerance.min(config.score_tolerance).min(1e-8);
    relative_infinity_difference(eta, strict_eta) <= tolerance
        && relative_infinity_difference(coefficients, strict_coefficients) <= tolerance
        && relative_scalar_difference(objective.deviance, strict_objective.deviance) <= tolerance
        && relative_scalar_difference(objective.log_likelihood, strict_objective.log_likelihood)
            <= tolerance
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
    use crate::metadata::{
        CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm,
        LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId, TermComponent, TermId,
        TermOrder, TimeSeriesState,
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
    fn line_search_reuses_buffers_and_rejects_overflow() {
        let dimensions = PpmlMemoryDimensions {
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
            &prepared, &outcome, &[1.0; 4], &[0.0; 4], &design, config, None,
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
        assert!(std::mem::size_of::<PreparedPpmlIterationReport>() <= 256);
    }
}
