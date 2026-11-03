//! Bounded residual correction in the verified quotient coordinates.
use super::*;
use crate::linalg::neumaier_add;

/// Refine only an activated repair, before taking the finite inverse out of
/// its workspace. Existing row/score/solve banks hold all scratch. Three
/// corrections are a fixed roundoff-reduction budget, not a new convergence
/// tolerance: the independent rank, score and bread certificates still decide
/// admission afterward. Ordinary-path coefficients never enter this routine.
pub(super) fn refine<R: FitReservation>(
    storage: &ProjectedFiniteRows<'_>,
    weights: NormalizedWeights<'_>,
    workspace: &mut FiniteWorkspace,
    scales: &[f64],
    reservation: &mut R,
) -> Result<()> {
    let rank = workspace.kept.len();
    if rank == 0 {
        return Ok(());
    }
    let rows = match storage {
        ProjectedFiniteRows::Dense(values) => values.nrows(),
        ProjectedFiniteRows::Scratch { scratch, .. } => scratch.rows(),
        ProjectedFiniteRows::Centered(_) => {
            return Err(invalid_argument("rank refinement requires projected rows"));
        }
    };
    if workspace.coefficients.len() != rank
        || workspace.bread.len() != checked_square(rank, "rank refinement dimensions overflow")?
    {
        return Err(invalid_argument(
            "rank refinement finite axes are inconsistent",
        ));
    }
    resize(&mut workspace.row, scales.len(), 0., reservation)?;
    resize(&mut workspace.scores, rank, 0., reservation)?;
    resize(&mut workspace.solve, rank, 0., reservation)?;
    let mut reader = match storage {
        ProjectedFiniteRows::Scratch { scratch, .. } => Some(scratch.shared_reader()),
        _ => None,
    };
    for _ in 0..3 {
        workspace.scores.fill(0.);
        workspace.solve.fill(0.);
        for observation in 0..rows {
            if observation % 256 == 0 {
                crate::interrupt::checkpoint()?;
            }
            let response = match storage {
                ProjectedFiniteRows::Dense(values) => {
                    for &column in &workspace.kept {
                        workspace.row[column] = values[(observation, column + 1)];
                    }
                    values[(observation, 0)]
                }
                ProjectedFiniteRows::Scratch { scratch, outcome } => {
                    reader
                        .as_mut()
                        .expect("scratch reader")
                        .read_linear(observation * scratch.columns(), &mut workspace.row)?;
                    outcome[observation]
                }
                ProjectedFiniteRows::Centered(_) => unreachable!(),
            };
            let mut residual = response;
            for (&column, &coefficient) in workspace.kept.iter().zip(&workspace.coefficients) {
                residual = (-coefficient / scales[column]).mul_add(workspace.row[column], residual);
            }
            let weight = weights.value(observation)?;
            for (position, &column) in workspace.kept.iter().enumerate() {
                let value = weight * (workspace.row[column] / scales[column]) * residual;
                if !value.is_finite() {
                    return Err(invalid_data("rank refinement score is nonfinite"));
                }
                neumaier_add(
                    value,
                    &mut workspace.scores[position],
                    &mut workspace.solve[position],
                );
            }
        }
        for (score, &correction) in workspace.scores.iter_mut().zip(&workspace.solve) {
            *score += correction;
        }
        let mut changed = false;
        for position in 0..rank {
            let (mut sum, mut correction) = (0., 0.);
            for (&inverse, &score) in workspace.bread[position * rank..(position + 1) * rank]
                .iter()
                .zip(&workspace.scores)
            {
                neumaier_add(inverse * score, &mut sum, &mut correction);
            }
            let coefficient = workspace.coefficients[position] + (sum + correction);
            if !coefficient.is_finite() {
                return Err(invalid_data("rank refinement coefficient is nonfinite"));
            }
            changed |= coefficient != workspace.coefficients[position];
            workspace.coefficients[position] = coefficient;
        }
        if !changed {
            break;
        }
    }
    Ok(())
}
