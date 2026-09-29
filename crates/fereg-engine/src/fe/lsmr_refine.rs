//! Reproject the true residual until both FE scores and fitted corrections settle.
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
            let mut update = f64::INFINITY;
            let mut previous_update = f64::INFINITY;
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
                let reference = evidence.weighted_rhs_norm[0];
                let residual_small = evidence.weighted_residual_norm[0] <= tolerance * reference;
                if score <= tolerance && (update <= tolerance || residual_small) {
                    break;
                }
                if (score > tolerance && score >= previous)
                    || (update > tolerance && update >= previous_update && update.is_finite())
                {
                    progress.stop = StopReason::Stagnation;
                    break;
                }
                previous = score;
                previous_update = update;
                let remaining = budget - progress.iterations;
                if remaining == 0 {
                    progress.stop = StopReason::BudgetExhausted;
                    break;
                }
                // A small local FE score can hide a larger fitted error along
                // a weak graph direction. Reprojecting the true residual tests
                // that remaining fitted component in observation coordinates,
                // without interpreting gauge-dependent coefficient distances.
                let correction = self.solve(&residual, remaining, tolerance, team)?;
                let work = &correction.progress[0];
                progress.iterations += work.iterations;
                progress.refinements += 1;
                let fitted = self.design.multiply(&correction.coefficients, team)?;
                update = (0..fitted.nrows()).fold(0_f64, |norm, row| {
                    let value = self.design.weights().at(row).sqrt() * fitted[(row, 0)];
                    norm.hypot(if reference == 0. {
                        value
                    } else {
                        value / reference
                    })
                });
                if !update.is_finite() {
                    progress.stop = StopReason::NumericalBreakdown;
                    break;
                }
                for row in 0..coefficients.nrows() {
                    coefficients[(row, 0)] += correction.coefficients[(row, 0)];
                }
                if work.stop != StopReason::Converged {
                    progress.stop = work.stop;
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
