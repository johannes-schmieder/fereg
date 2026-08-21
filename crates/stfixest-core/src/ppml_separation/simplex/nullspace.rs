use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};

pub(super) fn nullspace_basis(
    regressors: &DenseMatrix,
    rows: &[usize],
    tolerance: f64,
) -> Result<Vec<Vec<f64>>> {
    let ncols = regressors.ncols();
    let nrows = rows.len();
    let mut scales = vec![0.0_f64; ncols];
    for column in 0..ncols {
        let mut square_sum = 0.0_f64;
        for row in 0..regressors.nrows() {
            let value = regressors[(row, column)];
            square_sum = value.mul_add(value, square_sum);
        }
        scales[column] = square_sum.sqrt().max(1.0);
    }
    let mut matrix = vec![0.0_f64; nrows * ncols];
    for (out_row, &row) in rows.iter().enumerate() {
        for column in 0..ncols {
            matrix[out_row * ncols + column] = regressors[(row, column)] / scales[column];
        }
    }
    let initial_maximum = matrix
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    let threshold = tolerance * initial_maximum.max(1.0);
    let mut column_permutation = (0..ncols).collect::<Vec<_>>();
    let mut rank = 0_usize;
    while rank < nrows.min(ncols) {
        let mut pivot = None;
        let mut maximum = threshold;
        for row in rank..nrows {
            for column in rank..ncols {
                let value = matrix[row * ncols + column].abs();
                if value > maximum {
                    maximum = value;
                    pivot = Some((row, column));
                }
            }
        }
        let Some((pivot_row, pivot_column)) = pivot else {
            break;
        };
        if pivot_row != rank {
            for column in 0..ncols {
                matrix.swap(rank * ncols + column, pivot_row * ncols + column);
            }
        }
        if pivot_column != rank {
            for row in 0..nrows {
                matrix.swap(row * ncols + rank, row * ncols + pivot_column);
            }
            column_permutation.swap(rank, pivot_column);
        }
        let diagonal = matrix[rank * ncols + rank];
        if !diagonal.is_finite() || diagonal.abs() <= threshold {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                "simplex null-space pivot is invalid",
            ));
        }
        for column in rank..ncols {
            matrix[rank * ncols + column] /= diagonal;
        }
        for row in 0..nrows {
            if row == rank {
                continue;
            }
            let factor = matrix[row * ncols + rank];
            if factor == 0.0 {
                continue;
            }
            for column in rank..ncols {
                matrix[row * ncols + column] -=
                    factor * matrix[rank * ncols + column];
            }
        }
        rank += 1;
    }
    if rank == ncols {
        return Ok(Vec::new());
    }

    let mut basis = Vec::with_capacity(ncols - rank);
    for free in rank..ncols {
        let mut permuted = vec![0.0_f64; ncols];
        permuted[free] = 1.0;
        for pivot in 0..rank {
            permuted[pivot] = -matrix[pivot * ncols + free];
        }
        let mut original = vec![0.0_f64; ncols];
        for position in 0..ncols {
            original[column_permutation[position]] =
                permuted[position] / scales[column_permutation[position]];
        }
        let norm = original
            .iter()
            .fold(0.0_f64, |sum, value| value.mul_add(*value, sum))
            .sqrt();
        if !norm.is_finite() || norm <= f64::MIN_POSITIVE {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                "simplex null-space basis is nonfinite or zero",
            ));
        }
        original.iter_mut().for_each(|value| *value /= norm);
        basis.push(original);
    }
    Ok(basis)
}

