use crate::error::{ErrorCode, FeregError, Result};
use crate::memory::Capacity;

/// Compact dense matrix in observation-major order.
#[derive(Debug, Clone, PartialEq)]
pub struct DenseMatrix {
    nrows: usize,
    ncols: usize,
    data: Vec<f64>,
}

impl DenseMatrix {
    pub(crate) fn from_row_major(nrows: usize, ncols: usize, data: Vec<f64>) -> Result<Self> {
        let expected = Capacity::<f64>::matrix(
            nrows as u128,
            ncols as u128,
            "matrix dimensions overflow usize",
        )?
        .len("matrix dimensions overflow usize")?;
        if data.len() != expected {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "row-major dense matrix storage has the wrong length",
            ));
        }
        Ok(Self { nrows, ncols, data })
    }

    pub fn zeros(nrows: usize, ncols: usize) -> Result<Self> {
        let length = Capacity::<f64>::matrix(
            nrows as u128,
            ncols as u128,
            "matrix dimensions overflow usize",
        )?
        .len("matrix dimensions overflow usize")?;
        let data =
            Capacity::<f64>::new(length as u128).filled(0.0, "unable to reserve dense matrix")?;
        Ok(Self { nrows, ncols, data })
    }

    pub fn from_columns(columns: &[Vec<f64>]) -> Result<Self> {
        let nrows = columns.first().map_or(0, Vec::len);
        if columns.iter().any(|column| column.len() != nrows) {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "dense matrix columns have inconsistent lengths",
            ));
        }
        let mut matrix = Self::zeros(nrows, columns.len())?;
        for (column_index, column) in columns.iter().enumerate() {
            for (row_index, value) in column.iter().enumerate() {
                matrix[(row_index, column_index)] = *value;
            }
        }
        Ok(matrix)
    }

    #[must_use]
    pub const fn nrows(&self) -> usize {
        self.nrows
    }

    #[must_use]
    pub const fn ncols(&self) -> usize {
        self.ncols
    }

    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut self.data
    }

    pub(crate) fn storage_capacity(&self) -> usize {
        self.data.capacity()
    }

    pub(crate) fn resize_zeroed_within_capacity(
        &mut self,
        nrows: usize,
        ncols: usize,
    ) -> Result<()> {
        let length = Capacity::<f64>::matrix(
            nrows as u128,
            ncols as u128,
            "matrix dimensions overflow usize",
        )?
        .len("matrix dimensions overflow usize")?;
        if length > self.data.capacity() {
            return Err(FeregError::new(
                ErrorCode::Allocation,
                "reusable dense matrix dimensions exceed fixed capacity",
            ));
        }
        self.data.resize(length, 0.0);
        self.nrows = nrows;
        self.ncols = ncols;
        Ok(())
    }

    pub(crate) fn into_row_major(self) -> Vec<f64> {
        self.data
    }

    pub fn column(&self, column: usize) -> Result<Vec<f64>> {
        if column >= self.ncols {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "matrix column is out of bounds",
            ));
        }
        let mut values = Vec::new();
        values.try_reserve_exact(self.nrows).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve dense matrix column",
            )
        })?;
        for row in 0..self.nrows {
            values.push(self[(row, column)]);
        }
        Ok(values)
    }
}

impl std::ops::Index<(usize, usize)> for DenseMatrix {
    type Output = f64;

    fn index(&self, (row, column): (usize, usize)) -> &Self::Output {
        // Storage bounds alone would accept a column that wraps into the next row.
        debug_assert!(column < self.ncols, "dense matrix column is out of bounds");
        &self.data[row * self.ncols + column]
    }
}

impl std::ops::IndexMut<(usize, usize)> for DenseMatrix {
    fn index_mut(&mut self, (row, column): (usize, usize)) -> &mut Self::Output {
        debug_assert!(column < self.ncols, "dense matrix column is out of bounds");
        &mut self.data[row * self.ncols + column]
    }
}
