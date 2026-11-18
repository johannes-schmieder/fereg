//! Bounded accuracy check before an iterative session freezes finite rank.
use super::FROZEN_RANK_BASIS_CHANGED;
use super::{
    LinearSessionAllocationPhase, LinearSessionPhaseAllocationObservation, RetainedSolveStage,
    SolveReport,
};
use crate::error::{invalid_data, ErrorCode, FeregError, Result};

// The existing strict FE accuracy floor, not a relaxed finite-rank tolerance.
pub(super) const RANK_PROJECTION_TOLERANCE: f64 = 1e-14;

pub(super) fn unresolved(
    condition: f64,
    projection_tolerance: f64,
    columns: usize,
    coefficient_tolerance: Option<f64>,
    coefficient_projection_accuracy: f64,
) -> bool {
    // QR diagonal ratios are a cheap warning, not a rank authority or an
    // error bound for the FE projector. A combination can accumulate errors
    // from all standardized columns. Leave a factor-64 margin before trusting
    // a small retained direction; only the reprojected canonical QR decides.
    // Finite refinement alone cannot detect FE leakage in the retained design:
    // it recomputes scores on those same approximate columns. An explicit
    // coefficient goal therefore also limits this projection-risk warning.
    let rank_warning = condition * projection_tolerance * (columns.max(1) as f64).sqrt();
    rank_warning >= 1.0 / 64.0
        || coefficient_tolerance.is_some_and(|goal| {
            condition * coefficient_projection_accuracy * (columns.max(1) as f64).sqrt() >= goal
        })
}

pub(super) fn retryable(error: &FeregError) -> bool {
    matches!(
        error.code,
        ErrorCode::InvalidData | ErrorCode::NonConvergence
    ) && matches!(
        error.message.as_str(),
        "retained coefficient refinement exhausted its accuracy budget"
            | "retained coefficient refinement is nonfinite"
            | "retained weighted-linear inverse certificate did not pass"
            | "retained weighted-linear rank decision is numerically ambiguous"
            | "retained weighted-linear Cholesky inverse is not finite"
            | "retained weighted-linear Cholesky solution is not finite"
            | "retained weighted-linear rank solution is not finite"
            | "retained weighted-linear rank inverse is not finite"
            | "retained weighted-linear direct dropped-span certificate did not pass"
            | "retained finite-WLS QR dropped-span certificate did not pass"
            | FROZEN_RANK_BASIS_CHANGED
    )
}

#[derive(Default)]
pub(super) struct RankProjectionWork {
    pub projection: Option<SolveReport>,
    pub projection_seconds: f64,
    pub finite_seconds: f64,
    pub allocation: LinearSessionPhaseAllocationObservation,
}

impl RankProjectionWork {
    pub(super) fn merge_into(self, stage: &mut RetainedSolveStage) -> Result<()> {
        if let Some(report) = self.projection {
            record_projection_retry(
                &mut stage.candidate.frame.response_absorption_report,
                report,
            )?;
        }
        for (phase, peak) in [
            (
                LinearSessionAllocationPhase::Projection,
                self.allocation.projection_peak_bytes,
            ),
            (
                LinearSessionAllocationPhase::FiniteSolve,
                self.allocation.finite_solve_peak_bytes,
            ),
        ] {
            if let Some(peak) = peak {
                stage.report.phase_allocation.observe(phase, peak);
            }
        }
        if let Some(seconds) = &mut stage.report.phase_timing.projection_seconds {
            *seconds += self.projection_seconds;
        }
        if let Some(seconds) = &mut stage.report.phase_timing.finite_solve_seconds {
            *seconds += self.finite_seconds;
        }
        Ok(())
    }
}

// The retried frame replaces the earlier projection entirely. Report its
// solver and certificate, while accounting for work spent on both attempts.
fn record_projection_retry(final_report: &mut SolveReport, discarded: SolveReport) -> Result<()> {
    let setup_seconds = final_report.setup_seconds + discarded.setup_seconds;
    let solve_seconds = final_report.solve_seconds + discarded.solve_seconds;
    let certification_seconds =
        final_report.certification_seconds + discarded.certification_seconds;
    if !final_report.certified()
        || !discarded.certified()
        || ![setup_seconds, solve_seconds, certification_seconds]
            .into_iter()
            .all(f64::is_finite)
    {
        return Err(invalid_data(
            "invalid retained-rank projection retry report",
        ));
    }
    final_report.iterations = final_report.iterations.saturating_add(discarded.iterations);
    final_report.checkpoint_retries = final_report
        .checkpoint_retries
        .saturating_add(discarded.checkpoint_retries)
        .saturating_add(usize::from(discarded.fallback_used))
        .saturating_add(1);
    final_report.solver_workspace_peak = final_report
        .solver_workspace_peak
        .max(discarded.solver_workspace_peak);
    let escalations = discarded.route().escalations;
    if escalations != 0 {
        let mut route = final_report.route();
        route.escalations = route.escalations.saturating_add(escalations);
        final_report.set_route(route);
    }
    final_report.pilot_iterations = final_report
        .pilot_iterations
        .max(discarded.pilot_iterations);
    final_report.pilot_contraction = final_report
        .pilot_contraction
        .max(discarded.pilot_contraction);
    final_report.setup_seconds = setup_seconds;
    final_report.solve_seconds = solve_seconds;
    final_report.certification_seconds = certification_seconds;
    final_report.warnings.extend(discarded.warnings);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{record_projection_retry, unresolved};
    use crate::fe::{
        FeCertificate, FeFallbackReason, FeSolverKind, LsmrPreconditioner, SolveReport,
        SolveStopReason,
    };

    #[test]
    fn stricter_rank_projection_can_change_solver_and_retains_its_certificate() {
        let certificate = FeCertificate {
            absolute_infinity_norm: 1e-15,
            relative_infinity_norm: 1e-15,
            relative_two_norm: 1e-15,
            passed: true,
        };
        let mut retry = SolveReport::single_route(
            FeSolverKind::Lsmr,
            SolveStopReason::Certified,
            certificate.clone(),
        );
        retry.used_solver = FeSolverKind::Map;
        retry.requested_preconditioner = Some(LsmrPreconditioner::Auto);
        retry.used_preconditioner = Some(LsmrPreconditioner::Diagonal);
        retry.note_fallback(FeFallbackReason::Uncertified);
        retry.iterations = 3;
        retry.checkpoint_retries = 2;
        retry.solve_seconds = 0.25;
        retry.normal_equation_residual = 1e-15;
        let mut prefix =
            SolveReport::single_route(FeSolverKind::Map, SolveStopReason::Certified, certificate);
        prefix.certificate.relative_infinity_norm = 1e-9;
        prefix.normal_equation_residual = 1e-9;
        prefix.iterations = 5;
        prefix.checkpoint_retries = 1;
        prefix.solve_seconds = 0.5;
        prefix.note_fallback(FeFallbackReason::Nonconvergence);
        let mut prefix_route = prefix.route();
        prefix_route.escalations = 1;
        prefix.set_route(prefix_route);
        prefix.warnings.push("prefix diagnostic".into());
        let mut expected = retry.clone();
        expected.iterations = 8;
        expected.checkpoint_retries = 5;
        let mut expected_route = expected.route();
        expected_route.escalations = 1;
        expected.set_route(expected_route);
        expected.solve_seconds = 0.75;
        expected.warnings.push("prefix diagnostic".into());

        record_projection_retry(&mut retry, prefix.clone()).unwrap();
        assert_eq!(retry, expected);
        let mut no_fallback = SolveReport::single_route(
            FeSolverKind::Lsmr,
            SolveStopReason::Certified,
            retry.certificate.clone(),
        );
        record_projection_retry(&mut no_fallback, prefix.clone()).unwrap();
        assert_ne!(no_fallback.used_solver, prefix.used_solver);
        assert!(!no_fallback.fallback_used);
        assert_eq!(no_fallback.checkpoint_retries, 3);

        prefix.certificate.passed = false;
        assert!(record_projection_retry(&mut retry, prefix).is_err());
        assert_eq!(retry, expected);
    }

    #[test]
    fn coefficient_goal_adds_a_warning_without_changing_rank_authority() {
        // A weak retained direction can satisfy the rank margin but warrant
        // tighter projection for an explicitly requested coefficient goal.
        assert!(!unresolved(1e3, 1e-6, 3, None, 1e-6));
        assert!(unresolved(1e3, 1e-6, 3, Some(1e-8), 1e-6));
        // A measured full-design certificate can already be much more accurate
        // than requested. An ordinary well-conditioned solve needs no retry.
        assert!(!unresolved(1., 1e-6, 3, Some(1e-8), 1e-12));
        // Observed coefficient accuracy never bypasses the old rank warning.
        assert!(unresolved(1e6, 1e-6, 3, None, 0.));
        assert!(unresolved(1e6, 1e-6, 3, Some(0.1), 0.));
    }
}
