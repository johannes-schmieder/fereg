//! Schur corrections reuse exact factors, never the original response's warm guess.
use super::{refinement, MonitorControl, SchurCandidate, SchurSession, StopReason};
use crate::{data::DenseMatrix, error::Result, threads::ThreadTeam};

impl SchurSession<'_, '_> {
    pub fn solve_refined(
        &mut self,
        rhs: &DenseMatrix,
        control: MonitorControl,
        team: &ThreadTeam,
    ) -> Result<SchurCandidate> {
        let (design, budget, tolerance) = self.refinement_controls();
        let mut candidate = self.solve_monitored(rhs, control, team)?;
        refinement::run(
            design,
            rhs,
            &mut candidate.coefficients,
            &mut candidate.progress,
            refinement::Limits { budget, tolerance },
            team,
            |residual, remaining, correction_tolerance, team| {
                let correction = self.solve_budget(
                    residual,
                    control,
                    remaining,
                    false,
                    correction_tolerance,
                    team,
                )?;
                Ok((correction.coefficients, correction.progress))
            },
        )?;
        candidate.residuals = match design.reconstruct(rhs, &candidate.coefficients, team) {
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
