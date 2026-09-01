use crate::error::{ErrorCode, FeregError, Result};

use super::{fallible_filled, reserve};

#[cfg(test)]
pub(super) fn phase_one_cone(
    cone: &[Vec<f64>],
    max_iterations: usize,
    tolerance: f64,
) -> Result<Option<(Vec<f64>, usize)>> {
    phase_one_cone_interruptible(cone, max_iterations, tolerance, &mut || Ok(()))
}

pub(super) fn phase_one_cone_interruptible(
    cone: &[Vec<f64>],
    max_iterations: usize,
    tolerance: f64,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<(Vec<f64>, usize)>> {
    let boundary = cone.len();
    let dimension = cone.first().map_or(0, Vec::len);
    if boundary == 0 || dimension == 0 {
        return Ok(None);
    }
    if cone.iter().any(|row| row.len() != dimension) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "simplex boundary cone dimensions are inconsistent",
        ));
    }
    let rows = boundary.checked_add(1).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Allocation,
            "simplex tableau row count overflowed",
        )
    })?;
    let original_variables = dimension
        .checked_mul(2)
        .and_then(|value| value.checked_add(boundary))
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "simplex tableau variable count overflowed",
            )
        })?;
    let total_variables = original_variables.checked_add(rows).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Allocation,
            "simplex tableau total variable count overflowed",
        )
    })?;
    let width = total_variables.checked_add(1).ok_or_else(|| {
        FeregError::new(ErrorCode::Allocation, "simplex tableau width overflowed")
    })?;
    let rhs_column = total_variables;
    let cells = rows.checked_mul(width).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Allocation,
            "simplex tableau dimensions overflowed",
        )
    })?;
    let mut tableau = fallible_filled(
        cells,
        0.0_f64,
        "unable to reserve model-simplex phase-I tableau",
    )?;
    let mut basis = Vec::new();
    reserve(
        &mut basis,
        rows,
        "unable to reserve model-simplex phase-I basis",
    )?;

    for row in 0..boundary {
        for column in 0..dimension {
            tableau[row * width + column] = cone[row][column];
            tableau[row * width + dimension + column] = -cone[row][column];
        }
        tableau[row * width + 2 * dimension + row] = -1.0;
        let artificial = original_variables + row;
        tableau[row * width + artificial] = 1.0;
        basis.push(artificial);
    }
    let normalization_row = boundary;
    for slack in 0..boundary {
        tableau[normalization_row * width + 2 * dimension + slack] = 1.0;
    }
    tableau[normalization_row * width + original_variables + normalization_row] = 1.0;
    tableau[normalization_row * width + rhs_column] = 1.0;
    basis.push(original_variables + normalization_row);

    let mut costs = fallible_filled(
        total_variables,
        0.0_f64,
        "unable to reserve model-simplex phase-I costs",
    )?;
    for artificial in original_variables..total_variables {
        costs[artificial] = -1.0;
    }

    for iteration in 0..max_iterations {
        interrupt()?;
        let mut entering = None;
        for column in 0..total_variables {
            if basis.contains(&column) {
                continue;
            }
            let mut reduced = costs[column];
            for row in 0..rows {
                reduced -= costs[basis[row]] * tableau[row * width + column];
            }
            if reduced > tolerance {
                entering = Some(column);
                break; // Bland's rule
            }
        }
        let Some(entering) = entering else {
            let objective = (0..rows).fold(0.0_f64, |sum, row| {
                costs[basis[row]].mul_add(tableau[row * width + rhs_column], sum)
            });
            if objective < -100.0 * tolerance {
                return Ok(None);
            }
            let mut solution = fallible_filled(
                original_variables,
                0.0_f64,
                "unable to reserve model-simplex phase-I solution",
            )?;
            for row in 0..rows {
                if basis[row] < original_variables {
                    solution[basis[row]] = tableau[row * width + rhs_column].max(0.0);
                }
            }
            let mut coordinates = fallible_filled(
                dimension,
                0.0_f64,
                "unable to reserve model-simplex phase-I coordinates",
            )?;
            for column in 0..dimension {
                coordinates[column] = solution[column] - solution[dimension + column];
            }
            return Ok(Some((coordinates, iteration)));
        };

        let mut leaving = None;
        let mut best_ratio = f64::INFINITY;
        let mut best_basic = usize::MAX;
        for row in 0..rows {
            let coefficient = tableau[row * width + entering];
            if coefficient <= tolerance {
                continue;
            }
            let rhs = tableau[row * width + rhs_column].max(0.0);
            let ratio = rhs / coefficient;
            if ratio < best_ratio - tolerance
                || ((ratio - best_ratio).abs() <= tolerance && basis[row] < best_basic)
            {
                best_ratio = ratio;
                best_basic = basis[row];
                leaving = Some(row);
            }
        }
        let Some(leaving) = leaving else {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "simplex phase-I tableau became unbounded",
            ));
        };
        pivot(&mut tableau, rows, width, leaving, entering, tolerance)?;
        basis[leaving] = entering;
    }
    Err(FeregError::new(
        ErrorCode::NonConvergence,
        format!("simplex phase-I exceeded {max_iterations} pivots"),
    ))
}

fn pivot(
    tableau: &mut [f64],
    rows: usize,
    width: usize,
    pivot_row: usize,
    pivot_column: usize,
    tolerance: f64,
) -> Result<()> {
    let pivot = tableau[pivot_row * width + pivot_column];
    if !pivot.is_finite() || pivot.abs() <= tolerance {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "simplex selected a numerically zero pivot",
        ));
    }
    for column in 0..width {
        tableau[pivot_row * width + column] /= pivot;
    }
    for row in 0..rows {
        if row == pivot_row {
            continue;
        }
        let factor = tableau[row * width + pivot_column];
        if factor == 0.0 {
            continue;
        }
        for column in 0..width {
            tableau[row * width + column] -= factor * tableau[pivot_row * width + column];
        }
    }
    if tableau.iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "simplex tableau became nonfinite",
        ));
    }
    Ok(())
}
