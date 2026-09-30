//! Shared weighted column/RHS scaling for FE blocks and finite WLS.
use super::qr::scale_ratio;
use crate::{
    data::{DenseMatrix, Weights},
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
};
#[derive(Debug)]
pub(crate) struct ColumnScaling {
    pub(crate) maxima: Vec<f64>,
    pub(crate) norms: Vec<f64>,
    pub(crate) roots: Vec<f64>,
    pub(crate) root_scale: f64,
}
#[derive(Debug)]
pub(crate) struct ScaledDesign {
    pub(crate) matrix: DenseMatrix,
    pub(crate) scaling: ColumnScaling,
}
impl ScaledDesign {
    pub(crate) fn new(
        mut matrix: DenseMatrix,
        weights: Weights<'_>,
        poll: Poll<'_>,
    ) -> Result<Self> {
        execution::check(poll)?;
        weights.validate(matrix.nrows())?;
        if matrix.as_slice().iter().any(|v| !v.is_finite()) {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "nonfinite weighted matrix",
            ));
        }
        let root_scale = execution::fold(
            (0..matrix.nrows()).map(|row| weights.at(row)),
            0.,
            poll,
            f64::max,
        )?
        .sqrt();
        let mut roots = reserved(matrix.nrows())?;
        for row in 0..matrix.nrows() {
            execution::check_chunk(poll, row)?;
            roots.push(weights.at(row).sqrt() / root_scale);
        }
        let mut maxima = reserved(matrix.ncols())?;
        let mut norms = reserved(matrix.ncols())?;
        for column in 0..matrix.ncols() {
            execution::check(poll)?;
            let maximum = execution::fold(
                (0..matrix.nrows()).map(|r| matrix[(r, column)].abs()),
                0.,
                poll,
                f64::max,
            )?;
            maxima.push(maximum);
            for row in 0..matrix.nrows() {
                execution::check_chunk(poll, row)?;
                let original = matrix[(row, column)];
                let value = if maximum == 0. {
                    0.
                } else {
                    original / maximum * roots[row]
                };
                if original != 0. && value == 0. {
                    return Err(FeregError::new(
                        ErrorCode::NonConvergence,
                        "weighted column scale underflow",
                    ));
                }
                matrix[(row, column)] = value;
            }
            let norm = execution::fold(0..matrix.nrows(), 0_f64, poll, |norm, row| {
                norm.hypot(matrix[(row, column)])
            })?;
            norms.push(norm);
            if norm != 0. {
                for row in 0..matrix.nrows() {
                    execution::check_chunk(poll, row)?;
                    matrix[(row, column)] /= norm;
                }
            }
        }
        Ok(Self {
            matrix,
            scaling: ColumnScaling {
                maxima,
                norms,
                roots,
                root_scale,
            },
        })
    }
    /// Finite FWL rank is measured against the unprojected column, so a weak
    /// within column must not be promoted to unit strength before pivoting.
    pub(crate) fn reference_scales(&mut self, references: &[f64], poll: Poll<'_>) -> Result<()> {
        if references.len() != self.matrix.ncols() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "reference scale count differs",
            ));
        }
        for (column, &reference) in references.iter().enumerate() {
            execution::check(poll)?;
            if !reference.is_finite() || reference <= 0. {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "invalid weighted column reference scale",
                ));
            }
            let old = self.scaling.norms[column];
            if old == 0. {
                continue;
            }
            let norm = scale_ratio(
                reference,
                1.,
                self.scaling.maxima[column],
                self.scaling.root_scale,
            );
            if !norm.is_finite() || norm <= 0. {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "column reference scaling is not representable",
                ));
            }
            for row in 0..self.matrix.nrows() {
                execution::check_chunk(poll, row)?;
                self.matrix[(row, column)] *= old / norm;
            }
            self.scaling.norms[column] = norm;
        }
        Ok(())
    }
    pub(crate) fn select_columns(self, columns: &[usize], poll: Poll<'_>) -> Result<Self> {
        execution::check(poll)?;
        let n = self.matrix.nrows();
        let p = self.matrix.ncols();
        if columns.iter().any(|&column| column >= p)
            || columns.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "selected columns must be unique and in canonical order",
            ));
        }
        let mut data = self.matrix.into_row_major();
        for row in 0..n {
            execution::check_chunk(poll, row)?;
            for (local, &column) in columns.iter().enumerate() {
                data[row * columns.len() + local] = data[row * p + column];
            }
        }
        data.truncate(n * columns.len());
        let mut maxima = reserved(columns.len())?;
        let mut norms = reserved(columns.len())?;
        for &column in columns {
            maxima.push(self.scaling.maxima[column]);
            norms.push(self.scaling.norms[column]);
        }
        Ok(Self {
            matrix: DenseMatrix::from_row_major(n, columns.len(), data)?,
            scaling: ColumnScaling {
                maxima,
                norms,
                roots: self.scaling.roots,
                root_scale: self.scaling.root_scale,
            },
        })
    }
}
impl ColumnScaling {
    pub(crate) fn rhs(
        &self,
        mut rhs: DenseMatrix,
        poll: Poll<'_>,
    ) -> Result<(DenseMatrix, Vec<f64>)> {
        execution::check(poll)?;
        if rhs.nrows() != self.roots.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "scaled RHS row count differs",
            ));
        }
        if rhs.as_slice().iter().any(|v| !v.is_finite()) {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "nonfinite scaled RHS",
            ));
        }
        let mut scales = reserved(rhs.ncols())?;
        for column in 0..rhs.ncols() {
            execution::check(poll)?;
            let scale = execution::fold(
                (0..rhs.nrows()).map(|r| rhs[(r, column)].abs()),
                0.,
                poll,
                f64::max,
            )?;
            scales.push(scale);
            for row in 0..rhs.nrows() {
                execution::check_chunk(poll, row)?;
                rhs[(row, column)] = if scale == 0. {
                    0.
                } else {
                    rhs[(row, column)] / scale * self.roots[row]
                };
            }
        }
        Ok((rhs, scales))
    }
    pub(crate) fn coefficient(&self, value: f64, column: usize, rhs_scale: f64) -> f64 {
        scale_ratio(value, rhs_scale, self.maxima[column], self.norms[column])
    }
    pub(crate) fn normal_coordinate(&self, value: f64, column: usize) -> f64 {
        let value = scale_ratio(value, 1., self.maxima[column], self.norms[column]);
        scale_ratio(value, 1., self.root_scale, 1.)
    }
}
