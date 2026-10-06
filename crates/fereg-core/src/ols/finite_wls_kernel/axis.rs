//! Borrowed finite axes; row/response views retain the original physical owner.
use crate::data::DenseMatrix;

#[derive(Debug, Clone, Copy)]
pub(crate) enum FiniteResponse<'a> {
    Slice(&'a [f64]),
    FirstColumn(&'a DenseMatrix),
}
impl FiniteResponse<'_> {
    pub(crate) fn len(self) -> usize {
        match self {
            Self::Slice(values) => values.len(),
            Self::FirstColumn(values) => values.nrows(),
        }
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = &f64> {
        (0..self.len()).map(|row| &self[row])
    }
}
impl std::ops::Index<usize> for FiniteResponse<'_> {
    type Output = f64;
    fn index(&self, row: usize) -> &f64 {
        match self {
            Self::Slice(values) => &values[row],
            Self::FirstColumn(values) => &values[(row, 0)],
        }
    }
}
impl<'a> From<&'a [f64]> for FiniteResponse<'a> {
    fn from(values: &'a [f64]) -> Self {
        Self::Slice(values)
    }
}
impl<'a> From<&'a Vec<f64>> for FiniteResponse<'a> {
    fn from(values: &'a Vec<f64>) -> Self {
        Self::Slice(values)
    }
}
impl<'a, const N: usize> From<&'a [f64; N]> for FiniteResponse<'a> {
    fn from(values: &'a [f64; N]) -> Self {
        Self::Slice(values)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FiniteDenseRows<'a> {
    values: &'a [f64],
    stride: usize,
    offset: usize,
    columns: usize,
}
impl<'a> FiniteDenseRows<'a> {
    pub(crate) fn packed(values: &'a [f64], columns: usize) -> Self {
        Self {
            values,
            stride: columns,
            offset: 0,
            columns,
        }
    }
    pub(crate) fn projected(values: &'a DenseMatrix) -> Self {
        Self {
            values: values.as_slice(),
            stride: values.ncols(),
            offset: 1,
            columns: values.ncols().saturating_sub(1),
        }
    }
    pub(crate) fn valid_for(self, rows: usize, columns: usize) -> bool {
        columns == self.columns
            && rows.checked_mul(self.stride) == Some(self.values.len())
            && self
                .offset
                .checked_add(columns)
                .is_some_and(|end| end <= self.stride)
    }
    pub(crate) fn len(self) -> usize {
        self.values.len().checked_div(self.stride).unwrap_or(0) * self.columns
    }
    pub(crate) fn row(self, row: usize) -> &'a [f64] {
        let start = row * self.stride + self.offset;
        &self.values[start..start + self.columns]
    }
}
