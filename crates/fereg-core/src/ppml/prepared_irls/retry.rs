//! One permitted ordinary retry; audit equivalence has a different measurement boundary.
use super::*;

pub(super) enum CanonicalRetryReason {
    Cmg(FusedFwlCmgFailureReport),
    Frozen(FrozenLinearFallbackKind),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn canonical_retry<P: PreparedOlsFitView + Sync + ?Sized>(
    reason: CanonicalRetryReason,
    direction: &mut OrdinaryDirection,
    session: &mut CertifiedLinearSession<'_, P>,
    workspace: &mut PpmlIterationWorkspace,
    offset: &[f64],
    frame_generation: u64,
    memory: &mut PpmlMemoryTelemetry,
    solver_report: &mut Option<PpmlSolverReport>,
    certified_subproblems: &mut usize,
    total_inner_iterations: &mut usize,
    allocation_audit: &mut PpmlAllocationAuditWindow,
) -> Result<(usize, LinearSessionReuseReport, FeSolverKind)> {
    let is_frozen = matches!(reason, CanonicalRetryReason::Frozen(_));
    let failure_message = match reason {
        CanonicalRetryReason::Cmg(report) => {
            solver_report
                .as_mut()
                .ok_or_else(|| internal("PPML CMG fallback lacks solver diagnostics"))?
                .record_cmg_numerical_fallback(cmg_failure_observation(report))?;
            direction.retire();
            memory.record_fallback(PpmlFallbackKind::Route);
            // Specialized ordinary solves leave this session stale between checkpoints.
            refresh_linear_frame(session, workspace, frame_generation)?;
            "prepared PPML MAP fallback after fused CMG did not certify"
        }
        CanonicalRetryReason::Frozen(kind) => {
            session.discard_invalid_frozen_state();
            memory.record_fallback(ppml_fallback_kind(kind));
            "prepared PPML canonical fallback did not certify"
        }
    };
    let canonical = session.solve_fresh()?;
    if !canonical.certified() {
        return Err(numerical_nonconvergence(failure_message));
    }
    *certified_subproblems = certified_subproblems.saturating_add(1);
    accumulate_inner_iterations(
        total_inner_iterations,
        canonical.frame().absorption_report(),
    )?;
    solver_report
        .as_mut()
        .ok_or_else(|| internal("PPML MAP fallback lacks solver diagnostics"))?
        .observe(map_solver_observation(
            canonical.frame().absorption_report(),
            PpmlSolverFrameKind::Ordinary,
        ))?;
    fill_candidate_eta_from_linear_output(&canonical, offset, workspace.candidate_eta_mut())?;
    fill_candidate_coefficients_from_linear_output(
        &canonical,
        workspace.candidate_coefficients_mut(),
    )?;
    if is_frozen && allocation_audit.needs_equivalence() {
        let observation = allocation_audit.complete_equivalence(false)?;
        memory.observe_steady_state_allocation(observation)?;
    }
    Ok((
        canonical.frame().finite_rank(),
        canonical.reuse_report(),
        canonical.frame().absorption_report().used_solver,
    ))
}
