use super::{normalized_solver_weight, RetainedWithinStorage};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::BorrowedMapWeights;
use crate::ols::PreparedOlsBreadWeightScale;

pub(super) struct RefinementFrame<'a> {
    pub storage: &'a RetainedWithinStorage,
    pub active_positions: &'a [usize],
    pub reference_norms: &'a [f64],
    pub response: &'a [f64],
    pub weights: BorrowedMapWeights<'a>,
    pub weight_scale: PreparedOlsBreadWeightScale,
}

pub(super) struct RefinementWorkspace<'a> {
    pub coefficients: &'a mut [f64],
    pub bread: &'a [f64],
    pub row: &'a mut [f64],
    pub scores: &'a mut [f64],
    pub corrections: &'a mut [f64],
}

pub(crate) fn coefficient_accuracy_scale(left: f64, right: f64) -> f64 {
    let magnitude = left.abs().max(right.abs());
    (1e-3 + magnitude).min(magnitude.max(1.0))
}

pub(super) fn refine_retained_coefficients(
    frame: RefinementFrame<'_>,
    workspace: RefinementWorkspace<'_>,
    tolerance: f64,
) -> Result<()> {
    let rank = frame.active_positions.len();
    if !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "retained coefficient accuracy tolerance is invalid",
        ));
    }
    // With no finite coefficients there is no correction to estimate, and
    // the session need not materialize a finite-design row bank.
    if rank == 0 {
        return Ok(());
    }
    let stored = frame.storage.columns();
    if frame.response.len() != frame.storage.rows()
        || frame.reference_norms.len() != stored
        || workspace.coefficients.len() != rank
        || rank.checked_mul(rank) != Some(workspace.bread.len())
        || workspace.row.len() != stored
        || workspace.scores.len() != rank
        || workspace.corrections.len() != rank
        || frame.active_positions.iter().any(|&position| {
            position >= stored
                || !frame.reference_norms[position].is_finite()
                || frame.reference_norms[position] <= 0.0
        })
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "retained coefficient refinement inputs are invalid",
        ));
    }
    let mut reader = frame.storage.reader();
    // Recomputed row residuals expose errors hidden by solving and checking
    // the same rounded Gram/RHS. Existing admitted banks hold all scratch.
    for pass in 0..=3 {
        workspace.scores.fill(0.0);
        workspace.corrections.fill(0.0);
        for (row, &response) in frame.response.iter().enumerate() {
            reader.fill_row(row, workspace.row)?;
            let mut residual = response;
            for (column, &position) in frame.active_positions.iter().enumerate() {
                let value = workspace.row[position] / frame.reference_norms[position];
                residual = (-workspace.coefficients[column]).mul_add(value, residual);
            }
            let weight = normalized_solver_weight(frame.weights, frame.weight_scale, row)?;
            for (column, &position) in frame.active_positions.iter().enumerate() {
                let value =
                    weight * (workspace.row[position] / frame.reference_norms[position]) * residual;
                let sum = workspace.scores[column];
                let next = sum + value;
                workspace.corrections[column] += if sum.abs() >= value.abs() {
                    (sum - next) + value
                } else {
                    (value - next) + sum
                };
                workspace.scores[column] = next;
            }
        }
        for (score, &correction) in workspace
            .scores
            .iter_mut()
            .zip(workspace.corrections.iter())
        {
            *score += correction;
        }
        let mut maximum = 0.0_f64;
        for (column, &position) in frame.active_positions.iter().enumerate() {
            let (mut sum, mut correction) = (0.0_f64, 0.0_f64);
            for (entry, &score) in workspace.bread[column * rank..(column + 1) * rank]
                .iter()
                .zip(workspace.scores.iter())
            {
                let value = entry * score;
                let next = sum + value;
                correction += if sum.abs() >= value.abs() {
                    (sum - next) + value
                } else {
                    (value - next) + sum
                };
                sum = next;
            }
            let delta = sum + correction;
            let before = workspace.coefficients[column] / frame.reference_norms[position];
            let after = (workspace.coefficients[column] + delta) / frame.reference_norms[position];
            let relative = (delta / frame.reference_norms[position]).abs()
                / coefficient_accuracy_scale(before, after);
            if !relative.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "retained coefficient refinement is nonfinite",
                ));
            }
            workspace.row[column] = delta;
            maximum = maximum.max(relative);
        }
        if maximum <= tolerance {
            return Ok(());
        }
        if pass == 3 {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "retained coefficient refinement exhausted its accuracy budget",
            ));
        }
        for (coefficient, &delta) in workspace.coefficients.iter_mut().zip(workspace.row.iter()) {
            *coefficient += delta;
        }
    }
    unreachable!("bounded refinement returns at its final pass")
}

#[cfg(test)]
#[path = "retained_refinement_tests.rs"]
mod tests;
