//! Complete-pivot positive constraints and their boundary images.
use super::{failed, filled, overflow, reserve};
use crate::{
    data::DenseMatrix,
    error::Result,
    ppml::{Interrupt, InterruptPoint},
};

pub(super) struct Nullspace {
    pub basis: Vec<Vec<f64>>,
    pub cone: Vec<Vec<f64>>,
}
pub(super) fn reduce(
    model: &DenseMatrix,
    positive: &[usize],
    boundary: &[usize],
    tolerance: f64,
    interrupt: &mut Interrupt<'_>,
) -> Result<Nullspace> {
    let width = model.ncols();
    let rows = model.nrows();
    let mut matrix = filled(
        rows.checked_mul(width).ok_or_else(overflow)?,
        0.,
        "simplex constraint matrix",
    )?;
    for (out, &row) in positive.iter().chain(boundary).enumerate() {
        for column in 0..width {
            matrix[out * width + column] = model[(row, column)];
        }
    }
    let mut permutation = Vec::new();
    reserve(&mut permutation, width, "simplex constraint permutation")?;
    permutation.extend(0..width);
    let rank = eliminate(
        &mut matrix,
        rows,
        positive.len(),
        &mut permutation,
        tolerance,
        interrupt,
    )?;
    let mut basis = Vec::new();
    reserve(&mut basis, width - rank, "simplex null-space basis")?;
    let mut cone = Vec::new();
    reserve(&mut cone, boundary.len(), "simplex boundary cone")?;
    for _ in boundary {
        cone.push(filled(width - rank, 0., "simplex boundary cone row")?);
    }
    for free in rank..width {
        interrupt(InterruptPoint::SimplexReduction)?;
        let mut direction = filled(width, 0., "simplex null-space direction")?;
        direction[permutation[free]] = 1.;
        for pivot in 0..rank {
            direction[permutation[pivot]] = -matrix[pivot * width + free];
        }
        let norm = direction.iter().fold(0_f64, |sum, &v| sum.hypot(v));
        if !norm.is_finite() || norm == 0. {
            return Err(failed("simplex null-space direction is nonfinite or zero"));
        }
        direction.iter_mut().for_each(|v| *v /= norm);
        for (row, values) in cone.iter_mut().enumerate() {
            values[free - rank] = matrix[(positive.len() + row) * width + free] / norm;
        }
        basis.push(direction);
    }
    Ok(Nullspace { basis, cone })
}
fn eliminate(
    matrix: &mut [f64],
    rows: usize,
    positive: usize,
    permutation: &mut [usize],
    tolerance: f64,
    interrupt: &mut Interrupt<'_>,
) -> Result<usize> {
    let width = permutation.len();
    let maximum = matrix[..positive * width]
        .iter()
        .fold(0_f64, |m, v| m.max(v.abs()));
    let threshold = tolerance * maximum.max(1.);
    let mut rank = 0;
    while rank < positive.min(width) {
        interrupt(InterruptPoint::SimplexReduction)?;
        let mut pivot = None;
        let mut maximum = threshold;
        for row in rank..positive {
            for column in rank..width {
                let value = matrix[row * width + column].abs();
                if value > maximum {
                    maximum = value;
                    pivot = Some((row, column));
                }
            }
        }
        let Some((pivot_row, pivot_column)) = pivot else {
            break;
        };
        for column in 0..width {
            matrix.swap(rank * width + column, pivot_row * width + column);
        }
        for row in 0..rows {
            matrix.swap(row * width + rank, row * width + pivot_column);
        }
        permutation.swap(rank, pivot_column);
        let diagonal = matrix[rank * width + rank];
        if !diagonal.is_finite() || diagonal.abs() <= threshold {
            return Err(failed("simplex null-space pivot is invalid"));
        }
        // Apply exactly the same elimination to boundary rows. Substituting
        // rounded null-space coefficients into the original design can turn
        // a pure FE gauge into a tiny false cone; identical complete rows
        // instead annihilate here with the identical positive constraint.
        for row in 0..rows {
            if row == rank {
                continue;
            }
            let factor = matrix[row * width + rank] / diagonal;
            if factor == 0. {
                continue;
            }
            for column in rank..width {
                matrix[row * width + column] -= factor * matrix[rank * width + column];
            }
            matrix[row * width + rank] = 0.;
        }
        // Eliminate with the unscaled pivot row before normalizing it. An
        // identical row then uses factor 1 and cancels exactly, even when the
        // pivot's reciprocal is not binary representable.
        for column in rank..width {
            matrix[rank * width + column] /= diagonal;
        }
        rank += 1;
    }
    if matrix.iter().any(|v| !v.is_finite()) {
        return Err(failed("simplex constraint reduction became nonfinite"));
    }
    Ok(rank)
}
