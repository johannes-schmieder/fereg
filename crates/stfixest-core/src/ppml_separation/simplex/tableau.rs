use crate::error::{ErrorCode, Result, StFixestError};

pub(super) fn phase_one_cone(
    cone: &[Vec<f64>],
    max_iterations: usize,
    tolerance: f64,
) -> Result<Option<(Vec<f64>, usize)>> {
    let boundary = cone.len();
    let dimension = cone.first().map_or(0, Vec::len);
    if boundary == 0 || dimension == 0 {
        return Ok(None);
    }
    let rows = boundary + 1;
    let original_variables = 2 * dimension + boundary;
    let total_variables = original_variables + rows;
    let width = total_variables + 1;
    let rhs_column = total_variables;
    let mut tableau = vec![0.0_f64; rows * width];
    let mut basis = Vec::with_capacity(rows);

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

    let mut costs = vec![0.0_f64; total_variables];
    for artificial in original_variables..total_variables {
        costs[artificial] = -1.0;
    }

    for iteration in 0..max_iterations {
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
            let mut solution = vec![0.0_f64; original_variables];
            for row in 0..rows {
                if basis[row] < original_variables {
                    solution[basis[row]] = tableau[row * width + rhs_column].max(0.0);
                }
            }
            let coordinates = (0..dimension)
                .map(|column| solution[column] - solution[dimension + column])
                .collect();
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
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                "simplex phase-I tableau became unbounded",
            ));
        };
        pivot(&mut tableau, rows, width, leaving, entering, tolerance)?;
        basis[leaving] = entering;
    }
    Err(StFixestError::new(
        ErrorCode::InvalidData,
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
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
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
            tableau[row * width + column] -=
                factor * tableau[pivot_row * width + column];
        }
    }
    if tableau.iter().any(|value| !value.is_finite()) {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "simplex tableau became nonfinite",
        ));
    }
    Ok(())
}

