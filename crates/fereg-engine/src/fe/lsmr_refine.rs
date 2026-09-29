//! Continue the same LSMR factors when its preconditioned stop precedes the FE target.
use super::{LsmrCandidate, LsmrSession, StopReason};
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
        for column in 0..rhs.ncols() {
            if candidate.progress[column].stop != StopReason::Converged {
                continue;
            }
            let original = DenseMatrix::from_columns(&[rhs.column(column)?])?;
            let mut coefficients =
                DenseMatrix::from_columns(&[candidate.coefficients.column(column)?])?;
            let progress = &mut candidate.progress[column];
            let mut previous = f64::INFINITY;
            loop {
                let residual = self.design.reconstruct(&original, &coefficients, team)?;
                let evidence = self.design.measure_certificate(
                    &original,
                    &coefficients,
                    &residual,
                    tolerance,
                    team,
                )?;
                let score = evidence.relative_score[0];
                if score <= tolerance {
                    break;
                }
                if score >= previous {
                    progress.stop = StopReason::Stagnation;
                    break;
                }
                previous = score;
                let remaining = budget - progress.iterations;
                if remaining == 0 {
                    progress.stop = StopReason::BudgetExhausted;
                    break;
                }
                // within stops in a preconditioned normal-equation metric. A
                // correction scales that recurrence to the actual remaining
                // residual; acceptance still uses the original RHS and FE span.
                let correction = self.solve(&residual, remaining, tolerance, team)?;
                let work = &correction.progress[0];
                progress.iterations += work.iterations;
                progress.refinements += 1;
                for row in 0..coefficients.nrows() {
                    coefficients[(row, 0)] += correction.coefficients[(row, 0)];
                }
                if work.stop != StopReason::Converged || work.iterations == 0 {
                    progress.stop = if work.stop == StopReason::Converged {
                        StopReason::NumericalBreakdown
                    } else {
                        work.stop
                    };
                    break;
                }
            }
            for row in 0..coefficients.nrows() {
                candidate.coefficients[(row, column)] = coefficients[(row, 0)];
            }
        }
        Ok(candidate)
    }
}
