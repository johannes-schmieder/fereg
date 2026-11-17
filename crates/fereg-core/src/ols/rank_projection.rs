//! Bounded accuracy check before an iterative session freezes finite rank.
use super::{
    merge_solve_report, LinearSessionAllocationPhase, LinearSessionPhaseAllocationObservation,
    RetainedSolveStage, SolveReport,
};
use crate::error::{ErrorCode, FeregError, Result};

// The existing strict FE accuracy floor, not a relaxed finite-rank tolerance.
pub(super) const RANK_PROJECTION_TOLERANCE: f64 = 1e-14;

pub(super) fn unresolved(condition: f64, projection_tolerance: f64, columns: usize) -> bool {
    // QR diagonal ratios are a cheap warning, not a rank authority or an
    // error bound for the FE projector. A combination can accumulate errors
    // from all standardized columns. Leave a factor-64 margin before trusting
    // a small retained direction; only the reprojected canonical QR decides.
    condition * projection_tolerance * (columns.max(1) as f64).sqrt() >= 1.0 / 64.0
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
            | "weighted-linear solve changed the frozen canonical rank basis"
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
            let final_report = &mut stage.candidate.frame.response_absorption_report;
            let certificate = final_report.certificate.clone();
            let normal_equation_residual = final_report.normal_equation_residual;
            let iterations = final_report.iterations.saturating_add(report.iterations);
            merge_solve_report(final_report, report)?;
            // Prefix work counts, but its looser certificate does not describe
            // the corrected design that is about to acquire rank authority.
            final_report.certificate = certificate;
            final_report.normal_equation_residual = normal_equation_residual;
            final_report.iterations = iterations;
            final_report.checkpoint_retries = final_report.checkpoint_retries.saturating_add(1);
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
