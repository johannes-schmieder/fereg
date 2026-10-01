//! Complete-pivot positive constraints and their boundary images.
use super::{failed, filled, overflow, reserve};
use crate::{
    data::DenseMatrix,
    error::Result,
    execution::{self, Poll, WorkPoll},
    ppml::{Interrupt, InterruptPoint},
};

#[derive(Debug, PartialEq)]
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
    poll: Poll<'_>,
) -> Result<Nullspace> {
    let mut work = WorkPoll::new(poll)?;
    let width = model.ncols();
    let rows = model.nrows();
    let mut matrix = filled(
        rows.checked_mul(width).ok_or_else(overflow)?,
        0.,
        "simplex constraint matrix",
        poll,
    )?;
    for (out, &row) in positive.iter().chain(boundary).enumerate() {
        for column in 0..width {
            work.step()?;
            matrix[out * width + column] = model[(row, column)];
        }
    }
    let mut permutation = Vec::new();
    reserve(&mut permutation, width, "simplex constraint permutation")?;
    for column in 0..width {
        work.step()?;
        permutation.push(column);
    }
    let rank = eliminate(
        &mut matrix,
        rows,
        positive.len(),
        &mut permutation,
        tolerance,
        interrupt,
        poll,
    )?;
    let mut basis = Vec::new();
    reserve(&mut basis, width - rank, "simplex null-space basis")?;
    let mut cone = Vec::new();
    reserve(&mut cone, boundary.len(), "simplex boundary cone")?;
    for _ in boundary {
        work.step()?;
        cone.push(filled(width - rank, 0., "simplex boundary cone row", poll)?);
    }
    for free in rank..width {
        interrupt(InterruptPoint::SimplexReduction)?;
        let mut direction = filled(width, 0., "simplex null-space direction", poll)?;
        direction[permutation[free]] = 1.;
        for pivot in 0..rank {
            work.step()?;
            direction[permutation[pivot]] = -matrix[pivot * width + free];
        }
        let norm = execution::fold(&direction, 0_f64, poll, |sum, &v| sum.hypot(v))?;
        if !norm.is_finite() || norm == 0. {
            return Err(failed("simplex null-space direction is nonfinite or zero"));
        }
        for v in &mut direction {
            work.step()?;
            *v /= norm;
        }
        for (row, values) in cone.iter_mut().enumerate() {
            work.step()?;
            values[free - rank] = matrix[(positive.len() + row) * width + free] / norm;
        }
        basis.push(direction);
    }
    execution::check(poll)?;
    Ok(Nullspace { basis, cone })
}
fn eliminate(
    matrix: &mut [f64],
    rows: usize,
    positive: usize,
    permutation: &mut [usize],
    tolerance: f64,
    interrupt: &mut Interrupt<'_>,
    poll: Poll<'_>,
) -> Result<usize> {
    let mut work = WorkPoll::new(poll)?;
    let width = permutation.len();
    let maximum = execution::fold(&matrix[..positive * width], 0_f64, poll, |m, v| {
        m.max(v.abs())
    })?;
    let threshold = tolerance * maximum.max(1.);
    let mut rank = 0;
    while rank < positive.min(width) {
        interrupt(InterruptPoint::SimplexReduction)?;
        let mut pivot = None;
        let mut maximum = threshold;
        for row in rank..positive {
            for column in rank..width {
                work.step()?;
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
            work.step()?;
            matrix.swap(rank * width + column, pivot_row * width + column);
        }
        for row in 0..rows {
            work.step()?;
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
            work.step()?;
            if row == rank {
                continue;
            }
            let factor = matrix[row * width + rank] / diagonal;
            if factor == 0. {
                continue;
            }
            for column in rank..width {
                work.step()?;
                matrix[row * width + column] -= factor * matrix[rank * width + column];
            }
            matrix[row * width + rank] = 0.;
        }
        // Eliminate with the unscaled pivot row before normalizing it. An
        // identical row then uses factor 1 and cancels exactly, even when the
        // pivot's reciprocal is not binary representable.
        for column in rank..width {
            work.step()?;
            matrix[rank * width + column] /= diagonal;
        }
        rank += 1;
    }
    if execution::any(matrix.iter(), |v| !v.is_finite(), poll)? {
        return Err(failed("simplex constraint reduction became nonfinite"));
    }
    execution::check(poll)?;
    Ok(rank)
}
