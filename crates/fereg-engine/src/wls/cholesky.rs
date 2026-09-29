//! Guarded diagonal-pivoted Cholesky harvested from legacy finite_wls_kernel.
use crate::{
    data::DenseMatrix,
    error::{reserved, Result},
    threads::compensated_sum,
};
pub(super) struct Cholesky {
    factor: DenseMatrix,
    permutation: Vec<usize>,
}
impl Cholesky {
    pub(super) fn factor(gram: &DenseMatrix, threshold: f64) -> Result<Option<Self>> {
        let p = gram.nrows();
        let mut values = reserved(gram.as_slice().len())?;
        values.extend_from_slice(gram.as_slice());
        let mut work = DenseMatrix::from_row_major(p, p, values)?;
        let mut factor = DenseMatrix::zeros(p, p)?;
        let mut permutation = reserved(p)?;
        permutation.extend(0..p);
        let initial = (0..p).map(|i| gram[(i, i)]).fold(0., f64::max);
        let guard = initial * threshold.sqrt();
        for step in 0..p {
            let mut pivot = step;
            for candidate in step + 1..p {
                let value = work[(candidate, candidate)];
                let previous = work[(pivot, pivot)];
                let tied = (value - previous).abs()
                    <= 64. * f64::EPSILON * value.abs().max(previous.abs());
                if value > previous && !tied || tied && permutation[candidate] < permutation[pivot]
                {
                    pivot = candidate;
                }
            }
            let value = work[(pivot, pivot)];
            if !value.is_finite() || value <= guard {
                return Ok(None);
            }
            if step != pivot {
                for column in 0..p {
                    let value = work[(step, column)];
                    work[(step, column)] = work[(pivot, column)];
                    work[(pivot, column)] = value;
                }
                for row in 0..p {
                    let value = work[(row, step)];
                    work[(row, step)] = work[(row, pivot)];
                    work[(row, pivot)] = value;
                }
                for column in 0..step {
                    let value = factor[(step, column)];
                    factor[(step, column)] = factor[(pivot, column)];
                    factor[(pivot, column)] = value;
                }
                permutation.swap(step, pivot);
            }
            factor[(step, step)] = work[(step, step)].sqrt();
            for row in step + 1..p {
                factor[(row, step)] = work[(row, step)] / factor[(step, step)];
            }
            for row in step + 1..p {
                for column in row..p {
                    let value = work[(column, row)] - factor[(row, step)] * factor[(column, step)];
                    if !value.is_finite() {
                        return Ok(None);
                    }
                    work[(column, row)] = value;
                    work[(row, column)] = value;
                }
            }
        }
        Ok(Some(Self {
            factor,
            permutation,
        }))
    }
    pub(super) fn solve(&self, rhs: &DenseMatrix) -> Result<DenseMatrix> {
        let p = self.permutation.len();
        let mut work = DenseMatrix::zeros(p, rhs.ncols())?;
        for row in 0..p {
            for column in 0..rhs.ncols() {
                let tail =
                    compensated_sum((0..row).map(|i| self.factor[(row, i)] * work[(i, column)]));
                work[(row, column)] =
                    (rhs[(self.permutation[row], column)] - tail) / self.factor[(row, row)];
            }
        }
        for row in (0..p).rev() {
            for column in 0..rhs.ncols() {
                let tail = compensated_sum(
                    (row + 1..p).map(|i| self.factor[(i, row)] * work[(i, column)]),
                );
                work[(row, column)] = (work[(row, column)] - tail) / self.factor[(row, row)];
            }
        }
        let mut result = DenseMatrix::zeros(p, rhs.ncols())?;
        for row in 0..p {
            for column in 0..rhs.ncols() {
                result[(self.permutation[row], column)] = work[(row, column)];
            }
        }
        Ok(result)
    }
}
