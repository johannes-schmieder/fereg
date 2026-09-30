//! Pinned within uses the shared same-factor, original-input refinement protocol.
use super::{refinement, LsmrCandidate, LsmrSession};
use crate::{data::DenseMatrix, error::Result, threads::ThreadTeam};

impl LsmrSession<'_, '_> {
    pub fn solve_refined(
        &self,
        rhs: &DenseMatrix,
        budget: usize,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<LsmrCandidate> {
        let mut candidate = self.solve(rhs, budget, tolerance, team)?;
        refinement::run(
            self.design,
            rhs,
            &mut candidate.coefficients,
            &mut candidate.progress,
            refinement::Limits { budget, tolerance },
            team,
            |residual, remaining, correction_tolerance, team| {
                let correction = self.solve(residual, remaining, correction_tolerance, team)?;
                Ok((correction.coefficients, correction.progress))
            },
        )?;
        Ok(candidate)
    }
}
