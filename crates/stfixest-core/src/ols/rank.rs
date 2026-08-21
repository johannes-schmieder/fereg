#![allow(clippy::needless_range_loop)]

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};

/// Rank-revealing method used for the finite-dimensional least-squares solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiniteSolver {
    /// Try pivoted Cholesky on the standardized normal equations and use
    /// column-pivoted QR when the Cholesky path detects weak rank.
    Auto,
    /// Symmetric diagonal-pivoted Cholesky on standardized normal equations.
    PivotedCholesky,
    /// Householder QR with greedy column pivoting.
    ColumnPivotedQr,
}

/// Diagnostics and numerical products from a rank-revealing weighted solve.
#[derive(Debug, Clone, PartialEq)]
pub struct RankSolve {
    pub coefficients: Vec<f64>,
    /// Generalized inverse on the identified coefficient subspace, row-major.
    pub bread: Vec<f64>,
    pub kept: Vec<usize>,
    pub dropped: Vec<usize>,
    pub rank: usize,
    pub method: FiniteSolver,
    /// Ratio of the largest to smallest accepted factor pivot.
    pub condition_proxy: f64,
}

#[derive(Debug, Clone)]
struct ScaledSolve {
    coefficients: Vec<f64>,
    bread: Vec<f64>,
    kept: Vec<usize>,
    dropped: Vec<usize>,
    rank: usize,
    condition_proxy: f64,
    numerical_failure: bool,
}

/// Solve weighted least squares after scaling every finite regressor to unit
/// weighted norm. The returned coefficients and bread are on the caller's
/// original scale.
pub fn solve_rank_revealing(
    regressors: &DenseMatrix,
    outcome: &[f64],
    weights: &[f64],
    tolerance: f64,
    requested: FiniteSolver,
) -> Result<RankSolve> {
    validate_inputs(regressors, outcome, weights, tolerance)?;
    let ncols = regressors.ncols();
    if ncols == 0 {
        return Ok(RankSolve {
            coefficients: Vec::new(),
            bread: Vec::new(),
            kept: Vec::new(),
            dropped: Vec::new(),
            rank: 0,
            method: requested,
            condition_proxy: 1.0,
        });
    }

    let scales = weighted_column_scales(regressors, weights)?;
    if let Some(index) = scales.iter().position(|scale| *scale == 0.0) {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            format!("rank solver received zero-norm regressor column {index}"),
        ));
    }

    let (scaled, method) = match requested {
        FiniteSolver::PivotedCholesky => (
            solve_pivoted_cholesky(regressors, outcome, weights, &scales, tolerance)?,
            FiniteSolver::PivotedCholesky,
        ),
        FiniteSolver::ColumnPivotedQr => (
            solve_column_pivoted_qr(regressors, outcome, weights, &scales, tolerance)?,
            FiniteSolver::ColumnPivotedQr,
        ),
        FiniteSolver::Auto => {
            let cholesky =
                solve_pivoted_cholesky(regressors, outcome, weights, &scales, tolerance)?;
            let weak_pivot_limit = 1.0 / tolerance.sqrt().max(f64::EPSILON.sqrt());
            if cholesky.numerical_failure
                || cholesky.rank < ncols
                || cholesky.condition_proxy > weak_pivot_limit
            {
                (
                    solve_column_pivoted_qr(regressors, outcome, weights, &scales, tolerance)?,
                    FiniteSolver::ColumnPivotedQr,
                )
            } else {
                (cholesky, FiniteSolver::PivotedCholesky)
            }
        }
    };

    let mut coefficients = scaled.coefficients;
    for (coefficient, scale) in coefficients.iter_mut().zip(&scales) {
        *coefficient /= *scale;
    }
    let mut bread = scaled.bread;
    for row in 0..ncols {
        for column in 0..ncols {
            bread[row * ncols + column] /= scales[row] * scales[column];
        }
    }

    Ok(RankSolve {
        coefficients,
        bread,
        kept: scaled.kept,
        dropped: scaled.dropped,
        rank: scaled.rank,
        method,
        condition_proxy: scaled.condition_proxy,
    })
}

fn validate_inputs(
    regressors: &DenseMatrix,
    outcome: &[f64],
    weights: &[f64],
    tolerance: f64,
) -> Result<()> {
    if regressors.nrows() != outcome.len() || outcome.len() != weights.len() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "rank-solver inputs have inconsistent observation counts",
        ));
    }
    if !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "rank tolerance must be finite and lie strictly between zero and one",
        ));
    }
    if outcome.iter().any(|value| !value.is_finite())
        || regressors.as_slice().iter().any(|value| !value.is_finite())
        || weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "rank-solver values and weights must be finite and weights positive",
        ));
    }
    Ok(())
}

fn weighted_column_scales(regressors: &DenseMatrix, weights: &[f64]) -> Result<Vec<f64>> {
    let mut scales = vec![0.0_f64; regressors.ncols()];
    for row in 0..regressors.nrows() {
        for column in 0..regressors.ncols() {
            let value = regressors[(row, column)];
            scales[column] = (weights[row] * value).mul_add(value, scales[column]);
        }
    }
    for scale in &mut scales {
        *scale = scale.sqrt();
        if !scale.is_finite() {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                "weighted regressor norm overflowed",
            ));
        }
    }
    Ok(scales)
}

fn solve_pivoted_cholesky(
    regressors: &DenseMatrix,
    outcome: &[f64],
    weights: &[f64],
    scales: &[f64],
    tolerance: f64,
) -> Result<ScaledSolve> {
    let ncols = regressors.ncols();
    let mut gram = vec![0.0_f64; ncols * ncols];
    let mut rhs = vec![0.0_f64; ncols];
    for row in 0..regressors.nrows() {
        let weight = weights[row];
        for left in 0..ncols {
            let left_value = regressors[(row, left)] / scales[left];
            rhs[left] = (weight * left_value).mul_add(outcome[row], rhs[left]);
            for right in left..ncols {
                let right_value = regressors[(row, right)] / scales[right];
                gram[left * ncols + right] =
                    (weight * left_value).mul_add(right_value, gram[left * ncols + right]);
            }
        }
    }
    for left in 0..ncols {
        for right in 0..left {
            gram[left * ncols + right] = gram[right * ncols + left];
        }
    }

    let mut permutation: Vec<usize> = (0..ncols).collect();
    let mut factor = vec![0.0_f64; ncols * ncols];
    let initial_pivot = (0..ncols)
        .map(|index| gram[index * ncols + index])
        .fold(0.0_f64, f64::max);
    let threshold = initial_pivot * tolerance.max(f64::EPSILON);
    let mut rank = 0_usize;
    let mut minimum_pivot = f64::INFINITY;
    let mut numerical_failure = false;

    for step in 0..ncols {
        let mut pivot_position = step;
        let mut pivot_value = gram[step * ncols + step];
        for candidate in (step + 1)..ncols {
            let candidate_value = gram[candidate * ncols + candidate];
            if candidate_value > pivot_value {
                pivot_position = candidate;
                pivot_value = candidate_value;
            }
        }
        if pivot_value < -threshold {
            numerical_failure = true;
            break;
        }
        if pivot_value <= threshold {
            break;
        }
        if pivot_position != step {
            swap_symmetric(&mut gram, ncols, step, pivot_position);
            rhs.swap(step, pivot_position);
            permutation.swap(step, pivot_position);
            for column in 0..step {
                factor.swap(step * ncols + column, pivot_position * ncols + column);
            }
        }

        let diagonal = gram[step * ncols + step].sqrt();
        if !diagonal.is_finite() || diagonal <= 0.0 {
            numerical_failure = true;
            break;
        }
        factor[step * ncols + step] = diagonal;
        minimum_pivot = minimum_pivot.min(gram[step * ncols + step]);
        for row in (step + 1)..ncols {
            factor[row * ncols + step] = gram[row * ncols + step] / diagonal;
        }
        for row in (step + 1)..ncols {
            for column in row..ncols {
                let update = factor[row * ncols + step] * factor[column * ncols + step];
                let value = gram[column * ncols + row] - update;
                gram[column * ncols + row] = value;
                gram[row * ncols + column] = value;
            }
        }
        rank += 1;
    }

    let condition_proxy = if rank == 0 || minimum_pivot == 0.0 {
        f64::INFINITY
    } else {
        initial_pivot / minimum_pivot
    };
    let (coefficients_permuted, bread_permuted) =
        solve_from_lower_factor(&factor, &rhs, ncols, rank)?;
    let mut coefficients = vec![0.0_f64; ncols];
    let mut bread = vec![0.0_f64; ncols * ncols];
    for position in 0..rank {
        coefficients[permutation[position]] = coefficients_permuted[position];
        for other in 0..rank {
            bread[permutation[position] * ncols + permutation[other]] =
                bread_permuted[position * rank + other];
        }
    }

    Ok(ScaledSolve {
        coefficients,
        bread,
        kept: permutation[..rank].to_vec(),
        dropped: permutation[rank..].to_vec(),
        rank,
        condition_proxy,
        numerical_failure,
    })
}

fn swap_symmetric(matrix: &mut [f64], dimension: usize, left: usize, right: usize) {
    if left == right {
        return;
    }
    for column in 0..dimension {
        matrix.swap(left * dimension + column, right * dimension + column);
    }
    for row in 0..dimension {
        matrix.swap(row * dimension + left, row * dimension + right);
    }
}

fn solve_from_lower_factor(
    factor: &[f64],
    rhs: &[f64],
    dimension: usize,
    rank: usize,
) -> Result<(Vec<f64>, Vec<f64>)> {
    if rank == 0 {
        return Ok((vec![0.0; dimension], Vec::new()));
    }
    let mut forward = vec![0.0_f64; rank];
    for row in 0..rank {
        let mut value = rhs[row];
        for column in 0..row {
            value = (-factor[row * dimension + column]).mul_add(forward[column], value);
        }
        let diagonal = factor[row * dimension + row];
        if diagonal == 0.0 || !diagonal.is_finite() {
            return Err(StFixestError::new(
                ErrorCode::Internal,
                "invalid Cholesky factor diagonal",
            ));
        }
        forward[row] = value / diagonal;
    }
    let mut coefficients = vec![0.0_f64; dimension];
    for row in (0..rank).rev() {
        let mut value = forward[row];
        for column in (row + 1)..rank {
            value = (-factor[column * dimension + row]).mul_add(coefficients[column], value);
        }
        coefficients[row] = value / factor[row * dimension + row];
    }

    let mut inverse_lower = vec![0.0_f64; rank * rank];
    for column in 0..rank {
        for row in 0..rank {
            let mut value = if row == column { 1.0 } else { 0.0 };
            for inner in 0..row {
                value = (-factor[row * dimension + inner])
                    .mul_add(inverse_lower[inner * rank + column], value);
            }
            inverse_lower[row * rank + column] = value / factor[row * dimension + row];
        }
    }
    let mut bread = vec![0.0_f64; rank * rank];
    for row in 0..rank {
        for column in 0..rank {
            let mut value = 0.0_f64;
            for inner in 0..rank {
                value = inverse_lower[inner * rank + row]
                    .mul_add(inverse_lower[inner * rank + column], value);
            }
            bread[row * rank + column] = value;
        }
    }
    Ok((coefficients, bread))
}

fn solve_column_pivoted_qr(
    regressors: &DenseMatrix,
    outcome: &[f64],
    weights: &[f64],
    scales: &[f64],
    tolerance: f64,
) -> Result<ScaledSolve> {
    let nrows = regressors.nrows();
    let ncols = regressors.ncols();
    let mut matrix = vec![0.0_f64; nrows * ncols];
    let mut transformed_rhs = vec![0.0_f64; nrows];
    for row in 0..nrows {
        let square_root_weight = weights[row].sqrt();
        transformed_rhs[row] = square_root_weight * outcome[row];
        for column in 0..ncols {
            matrix[row * ncols + column] =
                square_root_weight * regressors[(row, column)] / scales[column];
        }
    }

    let mut permutation: Vec<usize> = (0..ncols).collect();
    let mut trailing_norms = column_trailing_norms(&matrix, nrows, ncols, 0);
    let initial_norm = trailing_norms.iter().copied().fold(0.0_f64, f64::max);
    let threshold = initial_norm * tolerance.max(f64::EPSILON);
    let mut rank = 0_usize;
    let mut largest_diagonal = 0.0_f64;
    let mut smallest_diagonal = f64::INFINITY;

    for step in 0..nrows.min(ncols) {
        let mut pivot_position = step;
        let mut pivot_norm = trailing_norms[step];
        for candidate in (step + 1)..ncols {
            if trailing_norms[candidate] > pivot_norm {
                pivot_position = candidate;
                pivot_norm = trailing_norms[candidate];
            }
        }
        if pivot_norm <= threshold {
            break;
        }
        if pivot_position != step {
            swap_columns(&mut matrix, nrows, ncols, step, pivot_position);
            trailing_norms.swap(step, pivot_position);
            permutation.swap(step, pivot_position);
        }

        let mut square_sum = 0.0_f64;
        for row in step..nrows {
            let value = matrix[row * ncols + step];
            square_sum = value.mul_add(value, square_sum);
        }
        let norm = square_sum.sqrt();
        if norm <= threshold {
            break;
        }
        let first = matrix[step * ncols + step];
        let beta = if first >= 0.0 { -norm } else { norm };
        let denominator = first - beta;
        if denominator == 0.0 || !denominator.is_finite() || beta == 0.0 {
            break;
        }
        let tau = (beta - first) / beta;
        matrix[step * ncols + step] = beta;
        for row in (step + 1)..nrows {
            matrix[row * ncols + step] /= denominator;
        }
        for column in (step + 1)..ncols {
            let mut projection = matrix[step * ncols + column];
            for row in (step + 1)..nrows {
                projection =
                    matrix[row * ncols + step].mul_add(matrix[row * ncols + column], projection);
            }
            projection *= tau;
            matrix[step * ncols + column] -= projection;
            for row in (step + 1)..nrows {
                matrix[row * ncols + column] -= matrix[row * ncols + step] * projection;
            }
        }
        let mut rhs_projection = transformed_rhs[step];
        for row in (step + 1)..nrows {
            rhs_projection =
                matrix[row * ncols + step].mul_add(transformed_rhs[row], rhs_projection);
        }
        rhs_projection *= tau;
        transformed_rhs[step] -= rhs_projection;
        for row in (step + 1)..nrows {
            transformed_rhs[row] -= matrix[row * ncols + step] * rhs_projection;
        }

        let diagonal = beta.abs();
        largest_diagonal = largest_diagonal.max(diagonal);
        smallest_diagonal = smallest_diagonal.min(diagonal);
        rank += 1;
        trailing_norms = column_trailing_norms(&matrix, nrows, ncols, step + 1);
    }

    let mut coefficients_permuted = vec![0.0_f64; ncols];
    for row in (0..rank).rev() {
        let mut value = transformed_rhs[row];
        for column in (row + 1)..rank {
            value = (-matrix[row * ncols + column]).mul_add(coefficients_permuted[column], value);
        }
        coefficients_permuted[row] = value / matrix[row * ncols + row];
    }

    let mut inverse_upper = vec![0.0_f64; rank * rank];
    for column in 0..rank {
        for row in (0..rank).rev() {
            let mut value = if row == column { 1.0 } else { 0.0 };
            for inner in (row + 1)..rank {
                value = (-matrix[row * ncols + inner])
                    .mul_add(inverse_upper[inner * rank + column], value);
            }
            inverse_upper[row * rank + column] = value / matrix[row * ncols + row];
        }
    }
    let mut bread_permuted = vec![0.0_f64; rank * rank];
    for row in 0..rank {
        for column in 0..rank {
            let mut value = 0.0_f64;
            for inner in 0..rank {
                value = inverse_upper[row * rank + inner]
                    .mul_add(inverse_upper[column * rank + inner], value);
            }
            bread_permuted[row * rank + column] = value;
        }
    }

    let mut coefficients = vec![0.0_f64; ncols];
    let mut bread = vec![0.0_f64; ncols * ncols];
    for position in 0..rank {
        coefficients[permutation[position]] = coefficients_permuted[position];
        for other in 0..rank {
            bread[permutation[position] * ncols + permutation[other]] =
                bread_permuted[position * rank + other];
        }
    }
    let condition_proxy = if rank == 0 || smallest_diagonal == 0.0 {
        f64::INFINITY
    } else {
        largest_diagonal / smallest_diagonal
    };
    Ok(ScaledSolve {
        coefficients,
        bread,
        kept: permutation[..rank].to_vec(),
        dropped: permutation[rank..].to_vec(),
        rank,
        condition_proxy,
        numerical_failure: false,
    })
}

fn swap_columns(matrix: &mut [f64], nrows: usize, ncols: usize, left: usize, right: usize) {
    for row in 0..nrows {
        matrix.swap(row * ncols + left, row * ncols + right);
    }
}

fn column_trailing_norms(matrix: &[f64], nrows: usize, ncols: usize, start_row: usize) -> Vec<f64> {
    let mut norms = vec![0.0_f64; ncols];
    for column in 0..ncols {
        let mut square_sum = 0.0_f64;
        for row in start_row..nrows {
            let value = matrix[row * ncols + column];
            square_sum = value.mul_add(value, square_sum);
        }
        norms[column] = square_sum.sqrt();
    }
    norms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pivoted_cholesky_solves_weighted_line() {
        let x = DenseMatrix::from_columns(&[vec![1.0; 5], vec![0.0, 1.0, 2.0, 3.0, 4.0]])
            .expect("matrix");
        let y = vec![1.0, 3.0, 5.0, 7.0, 9.0];
        let solve = solve_rank_revealing(
            &x,
            &y,
            &[1.0, 2.0, 1.0, 3.0, 2.0],
            1e-12,
            FiniteSolver::PivotedCholesky,
        )
        .expect("solve");
        assert_eq!(solve.rank, 2);
        assert!((solve.coefficients[0] - 1.0).abs() < 1e-10);
        assert!((solve.coefficients[1] - 2.0).abs() < 1e-10);
    }

    #[test]
    fn qr_drops_exact_duplicate_column() {
        let x = DenseMatrix::from_columns(&[
            vec![1.0; 5],
            vec![0.0, 1.0, 2.0, 3.0, 4.0],
            vec![0.0, 2.0, 4.0, 6.0, 8.0],
        ])
        .expect("matrix");
        let y = vec![1.0, 3.0, 5.0, 7.0, 9.0];
        let solve = solve_rank_revealing(&x, &y, &[1.0; 5], 1e-10, FiniteSolver::ColumnPivotedQr)
            .expect("solve");
        assert_eq!(solve.rank, 2);
        assert_eq!(solve.dropped.len(), 1);
        let fitted: Vec<f64> = (0..5)
            .map(|row| {
                (0..3)
                    .map(|column| x[(row, column)] * solve.coefficients[column])
                    .sum()
            })
            .collect();
        for (actual, expected) in fitted.iter().zip(y) {
            assert!((*actual - expected).abs() < 1e-10);
        }
    }
}
