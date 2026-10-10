//! Steady-state allocation audit: the warmed measurement window, its
//! boundaries and the independent equivalence observation.
use super::*;

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
pub(super) struct PpmlAllocationAuditWindow {
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
    pub(super) const fn new(probe: Option<PpmlAllocatorSnapshotProbe>) -> Self {
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

    pub(super) const fn is_measuring(&self) -> bool {
        self.start.is_some()
    }

    pub(super) const fn needs_equivalence(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether an explicitly requested qualification still needs its warmed
    /// window and independent equivalence solve.  Strict convergence must not
    /// terminate the fit during capacity warm-up: the first frozen solve is
    /// allowed to populate the shared kernel workspace, and only a later
    /// complete iteration may open the measured window.
    pub(super) const fn qualification_pending(&self) -> bool {
        self.probe.is_some() && !self.finished
    }

    pub(super) fn can_start_after(
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

    pub(super) fn start_after<P: PreparedOlsFitView + Sync + ?Sized>(
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
    pub(super) fn finish_iteration<P: PreparedOlsFitView + Sync + ?Sized>(
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

    pub(super) fn complete_equivalence(
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
