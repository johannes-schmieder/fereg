//! Schur corrections reuse exact factors, never the original response's warm guess.
use super::{refinement, MonitorControl, SchurCandidate, SchurSession, StopReason};
use crate::{data::DenseMatrix, error::Result, execution::Poll, threads::ThreadTeam};
#[cfg(test)]
mod correction_tests;
#[cfg(test)]
mod refinement_tests;

impl SchurSession<'_, '_> {
    pub fn solve_refined(
        &mut self,
        rhs: &DenseMatrix,
        control: MonitorControl,
        team: &ThreadTeam,
    ) -> Result<SchurCandidate> {
        self.solve_refined_polled(rhs, control, None, team)
    }
    pub(super) fn solve_refined_polled(
        &mut self,
        rhs: &DenseMatrix,
        control: MonitorControl,
        poll: Poll<'_>,
        team: &ThreadTeam,
    ) -> Result<SchurCandidate> {
        let (design, budget, tolerance) = self.refinement_controls();
        let mut candidate = self.solve_budget(
            rhs,
            control,
            true,
            refinement::Limits {
                budget,
                tolerance,
                poll,
            },
            team,
        )?;
        refinement::run(
            design,
            rhs,
            &mut candidate.coefficients,
            &mut candidate.progress,
            refinement::Limits {
                budget,
                tolerance,
                poll,
            },
            team,
            |residual, remaining, correction_tolerance, team| {
                let correction = self.solve_budget(
                    residual,
                    control,
                    false,
                    refinement::Limits {
                        budget: remaining,
                        tolerance: correction_tolerance,
                        poll,
                    },
                    team,
                )?;
                Ok((correction.coefficients, correction.progress))
            },
        )?;
        candidate.residuals =
            match design.reconstruct_polled(rhs, &candidate.coefficients, team, poll) {
                Ok(residuals) => residuals,
                Err(error)
                    if error.code == crate::error::ErrorCode::NonConvergence
                        && candidate
                            .progress
                            .iter()
                            .any(|work| work.stop != StopReason::Converged) =>
                {
                    DenseMatrix::zeros(rhs.nrows(), rhs.ncols())?
                }
                Err(error) => return Err(error),
            };
        self.retain_refinement(
            &candidate.coefficients,
            candidate
                .progress
                .iter()
                .all(|work| work.stop == StopReason::Converged),
        )?;
        Ok(candidate)
    }
}
