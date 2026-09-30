//! Pinned within uses the shared same-factor, original-input refinement protocol.
use super::{refinement, LsmrCandidate, LsmrSession};
use crate::{data::DenseMatrix, error::Result, execution::Poll, threads::ThreadTeam};

impl LsmrSession<'_, '_> {
    pub fn solve_refined(
        &self,
        rhs: &DenseMatrix,
        budget: usize,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<LsmrCandidate> {
        self.solve_refined_polled(rhs, budget, tolerance, None, team)
    }
    pub(super) fn solve_refined_polled(
        &self,
        rhs: &DenseMatrix,
        budget: usize,
        tolerance: f64,
        poll: Poll<'_>,
        team: &ThreadTeam,
    ) -> Result<LsmrCandidate> {
        let mut candidate = self.solve_polled(rhs, budget, tolerance, poll, team)?;
        refinement::run(
            self.design,
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
                let correction =
                    self.solve_polled(residual, remaining, correction_tolerance, poll, team)?;
                Ok((correction.coefficients, correction.progress))
            },
        )?;
        Ok(candidate)
    }
}
