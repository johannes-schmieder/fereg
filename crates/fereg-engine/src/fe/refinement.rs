//! Shared original-input refinement: FE scores and fitted corrections must settle.
use super::{Attempt, Design, StopReason};
use crate::{
    data::DenseMatrix,
    error::{ErrorCode, FeregError, Result},
    execution::{self, Poll},
    threads::ThreadTeam,
};

pub(super) struct Limits<'a> {
    pub budget: usize,
    pub tolerance: f64,
    pub poll: Poll<'a>,
}
pub(super) fn run<F>(
    design: &Design<'_>,
    rhs: &DenseMatrix,
    bank: &mut DenseMatrix,
    reports: &mut [Attempt],
    limits: Limits<'_>,
    team: &ThreadTeam,
    mut solve: F,
) -> Result<()>
where
    F: FnMut(&DenseMatrix, usize, f64, &ThreadTeam) -> Result<(DenseMatrix, Vec<Attempt>)>,
{
    let Limits {
        budget,
        tolerance,
        poll,
    } = limits;
    for column in 0..rhs.ncols() {
        if reports[column].stop != StopReason::Converged {
            continue;
        }
        let original = DenseMatrix::from_columns(&[rhs.column(column)?])?;
        let mut coefficients = DenseMatrix::from_columns(&[bank.column(column)?])?;
        let progress = &mut reports[column];
        let mut previous = f64::INFINITY;
        let mut update = f64::INFINITY;
        let mut previous_update = f64::INFINITY;
        loop {
            execution::check(poll)?;
            let measured =
                design
                    .reconstruct(&original, &coefficients, team)
                    .and_then(|residual| {
                        let evidence = design.measure_certificate(
                            &original,
                            &coefficients,
                            &residual,
                            tolerance,
                            team,
                        )?;
                        Ok((residual, evidence))
                    });
            let Some((residual, evidence)) = retain_numeric_stop(measured, progress)? else {
                break;
            };
            let score = evidence.relative_score[0];
            let reference = evidence.weighted_rhs_norm[0];
            let within_norm = evidence.weighted_residual_norm[0];
            // Subtracting the fitted FE part rounds at the original response
            // scale. A tiny within contrast cannot demand fitted corrections
            // smaller than that observation-coordinate representation error.
            // The original-input score certificate remains unchanged.
            let update_target = if within_norm == 0. {
                tolerance
            } else {
                tolerance.max(f64::EPSILON * reference / within_norm)
            };
            if !update_target.is_finite() {
                progress.stop = StopReason::NumericalBreakdown;
                break;
            }
            // A small weighted residual can still hide a material anchored
            // coefficient error across a weak bridge. Only an exactly zero
            // reconstruction can dispense with measuring a fitted correction.
            if score <= tolerance && (update <= update_target || within_norm == 0.) {
                break;
            }
            if (score > tolerance && score >= previous)
                || (update > update_target && update >= previous_update && update.is_finite())
            {
                progress.stop = StopReason::Stagnation;
                break;
            }
            previous = score;
            previous_update = update;
            let remaining = budget
                .checked_sub(progress.iterations)
                .ok_or_else(internal)?;
            if remaining == 0 {
                progress.stop = StopReason::BudgetExhausted;
                break;
            }
            // A small local score can hide a fitted error along a weak graph
            // direction. Correct the true residual in observation coordinates;
            // gauge-dependent coefficient distances have no stopping authority.
            // A large FE component cannot set the forward scale of the small
            // within contrast that identifies finite coefficients.
            // A mostly orthogonal residual can have a tiny FE score but a
            // material fitted component along a weak direction. A correction
            // must reduce its own initial score, rather than stop at zero work
            // against the much larger residual norm.
            let correction_tolerance = correction_target(tolerance, score, reference, within_norm);
            if correction_tolerance == 0. {
                progress.stop = StopReason::NumericalBreakdown;
                break;
            }
            let correction = solve(&residual, remaining, correction_tolerance, team);
            let Some((correction, work)) = retain_numeric_stop(correction, progress)? else {
                break;
            };
            if work.len() != 1 || work[0].iterations > remaining {
                return Err(internal());
            }
            let work = &work[0];
            progress.iterations += work.iterations;
            progress.refinements += 1;
            let Some(fitted) = retain_numeric_stop(design.multiply(&correction, team), progress)?
            else {
                break;
            };
            update = (0..fitted.nrows()).fold(0_f64, |norm, row| {
                let value = design.weights().at(row).sqrt() * fitted[(row, 0)];
                norm.hypot(if within_norm == 0. {
                    value
                } else {
                    value / within_norm
                })
            });
            if !update.is_finite() {
                progress.stop = StopReason::NumericalBreakdown;
                break;
            }
            for row in 0..coefficients.nrows() {
                let next = coefficients[(row, 0)] + correction[(row, 0)];
                if !next.is_finite() {
                    progress.stop = StopReason::NumericalBreakdown;
                    break;
                }
                coefficients[(row, 0)] = next;
            }
            if progress.stop != StopReason::Converged {
                break;
            }
            if work.stop != StopReason::Converged {
                progress.stop = work.stop;
                break;
            }
        }
        for row in 0..coefficients.nrows() {
            bank[(row, column)] = coefficients[(row, 0)];
        }
    }
    Ok(())
}
fn correction_target(tolerance: f64, score: f64, reference: f64, within_norm: f64) -> f64 {
    let relative_normal = if within_norm == 0. {
        0.
    } else {
        score / (within_norm / reference.max(within_norm))
    };
    if relative_normal == 0. {
        tolerance
    } else {
        // Residual observations carry the rounding of the preceding
        // reconstruction. Do not request a sub-roundoff normal solve;
        // this floor never exceeds the original acceptance target.
        (tolerance * relative_normal.min(1.))
            .max(16. * f64::EPSILON)
            .min(tolerance)
    }
}
fn retain_numeric_stop<T>(result: Result<T>, progress: &mut Attempt) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        // A late correction failure must retain prior work instead of looking
        // like initial preparation and silently renewing the fallback budget.
        Err(error) if error.code == ErrorCode::NonConvergence => {
            progress.stop = StopReason::NumericalBreakdown;
            Ok(None)
        }
        Err(error) => Err(error),
    }
}
fn internal() -> FeregError {
    FeregError::new(
        ErrorCode::Internal,
        "invalid residual-refinement iteration report",
    )
}
