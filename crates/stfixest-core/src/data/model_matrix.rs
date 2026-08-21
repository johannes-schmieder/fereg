use crate::error::{ErrorCode, Result, StFixestError};

/// Compact dense matrix in observation-major order.
#[derive(Debug, Clone, PartialEq)]
pub struct DenseMatrix {
    nrows: usize,
    ncols: usize,
    data: Vec<f64>,
}

impl DenseMatrix {
    pub fn zeros(nrows: usize, ncols: usize) -> Result<Self> {
        let length = nrows.checked_mul(ncols).ok_or_else(|| {
            StFixestError::new(ErrorCode::Allocation, "matrix dimensions overflow usize")
        })?;
        let mut data = Vec::new();
        data.try_reserve_exact(length).map_err(|_| {
            StFixestError::new(ErrorCode::Allocation, "unable to reserve dense matrix")
        })?;
        data.resize(length, 0.0);
        Ok(Self { nrows, ncols, data })
    }

    pub fn from_columns(columns: &[Vec<f64>]) -> Result<Self> {
        let nrows = columns.first().map_or(0, Vec::len);
        if columns.iter().any(|column| column.len() != nrows) {
            return Err(StFixestError::new(
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

    pub fn column(&self, column: usize) -> Result<Vec<f64>> {
        if column >= self.ncols {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "matrix column is out of bounds",
            ));
        }
        Ok((0..self.nrows).map(|row| self[(row, column)]).collect())
    }
}

impl std::ops::Index<(usize, usize)> for DenseMatrix {
    type Output = f64;

    fn index(&self, (row, column): (usize, usize)) -> &Self::Output {
        &self.data[row * self.ncols + column]
    }
}

impl std::ops::IndexMut<(usize, usize)> for DenseMatrix {
    fn index_mut(&mut self, (row, column): (usize, usize)) -> &mut Self::Output {
        &mut self.data[row * self.ncols + column]
    }
}

/// Recipe for one canonical finite-dimensional model column.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnRecipe {
    Intercept,
    Continuous { source: usize },
    Categorical { source: usize, level: u32 },
    CategoricalInteraction { sources: Vec<usize>, levels: Vec<u32> },
    CategoricalByContinuous {
        categorical_source: usize,
        level: u32,
        continuous_source: usize,
    },
    ContinuousInteraction { sources: Vec<usize> },
}

/// Native model-matrix generator over compact continuous and categorical sources.
#[derive(Debug)]
pub struct ModelMatrixBuilder<'a> {
    continuous: &'a [Vec<f64>],
    categorical: &'a [Vec<u32>],
    nrows: usize,
}

impl<'a> ModelMatrixBuilder<'a> {
    pub fn new(continuous: &'a [Vec<f64>], categorical: &'a [Vec<u32>]) -> Result<Self> {
        let nrows = continuous
            .first()
            .map(Vec::len)
            .or_else(|| categorical.first().map(Vec::len))
            .unwrap_or(0);
        if continuous.iter().any(|column| column.len() != nrows)
            || categorical.iter().any(|column| column.len() != nrows)
        {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "model-matrix source columns have inconsistent lengths",
            ));
        }
        Ok(Self {
            continuous,
            categorical,
            nrows,
        })
    }

    #[must_use]
    pub const fn nrows(&self) -> usize {
        self.nrows
    }

    pub fn generate(&self, recipes: &[ColumnRecipe]) -> Result<DenseMatrix> {
        self.generate_range(recipes, 0, recipes.len())
    }

    pub fn generate_range(
        &self,
        recipes: &[ColumnRecipe],
        start: usize,
        end: usize,
    ) -> Result<DenseMatrix> {
        if start > end || end > recipes.len() {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "invalid model-matrix recipe range",
            ));
        }
        let selected = &recipes[start..end];
        let mut matrix = DenseMatrix::zeros(self.nrows, selected.len())?;
        for (column, recipe) in selected.iter().enumerate() {
            self.validate_recipe(recipe)?;
            for row in 0..self.nrows {
                matrix[(row, column)] = self.value(recipe, row);
            }
        }
        Ok(matrix)
    }

    pub fn for_each_block<F>(
        &self,
        recipes: &[ColumnRecipe],
        pool_size: usize,
        mut operation: F,
    ) -> Result<()>
    where
        F: FnMut(usize, &DenseMatrix) -> Result<()>,
    {
        if pool_size == 0 {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "model-matrix pool size must be positive",
            ));
        }
        for start in (0..recipes.len()).step_by(pool_size) {
            let end = recipes.len().min(start + pool_size);
            let block = self.generate_range(recipes, start, end)?;
            operation(start, &block)?;
        }
        Ok(())
    }

    fn validate_recipe(&self, recipe: &ColumnRecipe) -> Result<()> {
        match recipe {
            ColumnRecipe::Intercept => Ok(()),
            ColumnRecipe::Continuous { source } => self.check_continuous(*source),
            ColumnRecipe::Categorical { source, .. } => self.check_categorical(*source),
            ColumnRecipe::CategoricalInteraction { sources, levels } => {
                if sources.is_empty() || sources.len() != levels.len() {
                    return Err(StFixestError::new(
                        ErrorCode::InvalidArgument,
                        "categorical interaction must have equally many sources and levels",
                    ));
                }
                for source in sources {
                    self.check_categorical(*source)?;
                }
                Ok(())
            }
            ColumnRecipe::CategoricalByContinuous {
                categorical_source,
                continuous_source,
                ..
            } => {
                self.check_categorical(*categorical_source)?;
                self.check_continuous(*continuous_source)
            }
            ColumnRecipe::ContinuousInteraction { sources } => {
                if sources.is_empty() {
                    return Err(StFixestError::new(
                        ErrorCode::InvalidArgument,
                        "continuous interaction must contain at least one source",
                    ));
                }
                for source in sources {
                    self.check_continuous(*source)?;
                }
                Ok(())
            }
        }
    }

    fn check_continuous(&self, source: usize) -> Result<()> {
        if source < self.continuous.len() {
            Ok(())
        } else {
            Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                format!("continuous source index {source} is out of bounds"),
            ))
        }
    }

    fn check_categorical(&self, source: usize) -> Result<()> {
        if source < self.categorical.len() {
            Ok(())
        } else {
            Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                format!("categorical source index {source} is out of bounds"),
            ))
        }
    }

    fn value(&self, recipe: &ColumnRecipe, row: usize) -> f64 {
        match recipe {
            ColumnRecipe::Intercept => 1.0,
            ColumnRecipe::Continuous { source } => self.continuous[*source][row],
            ColumnRecipe::Categorical { source, level } => {
                f64::from(self.categorical[*source][row] == *level)
            }
            ColumnRecipe::CategoricalInteraction { sources, levels } => f64::from(
                sources
                    .iter()
                    .zip(levels)
                    .all(|(source, level)| self.categorical[*source][row] == *level),
            ),
            ColumnRecipe::CategoricalByContinuous {
                categorical_source,
                level,
                continuous_source,
            } => {
                if self.categorical[*categorical_source][row] == *level {
                    self.continuous[*continuous_source][row]
                } else {
                    0.0
                }
            }
            ColumnRecipe::ContinuousInteraction { sources } => sources
                .iter()
                .map(|source| self.continuous[*source][row])
                .product(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_factor_and_interaction_columns_in_blocks() {
        let continuous = vec![vec![1.0, 2.0, 3.0], vec![10.0, 20.0, 30.0]];
        let categorical = vec![vec![0, 1, 1], vec![2, 2, 3]];
        let builder = ModelMatrixBuilder::new(&continuous, &categorical).expect("builder");
        let recipes = vec![
            ColumnRecipe::Intercept,
            ColumnRecipe::Continuous { source: 0 },
            ColumnRecipe::Categorical { source: 0, level: 1 },
            ColumnRecipe::CategoricalInteraction {
                sources: vec![0, 1],
                levels: vec![1, 2],
            },
            ColumnRecipe::CategoricalByContinuous {
                categorical_source: 0,
                level: 1,
                continuous_source: 1,
            },
            ColumnRecipe::ContinuousInteraction {
                sources: vec![0, 1],
            },
        ];
        let matrix = builder.generate(&recipes).expect("matrix");
        assert_eq!(matrix.nrows(), 3);
        assert_eq!(matrix.ncols(), 6);
        assert_eq!(matrix.column(2).expect("column"), vec![0.0, 1.0, 1.0]);
        assert_eq!(matrix.column(3).expect("column"), vec![0.0, 1.0, 0.0]);
        assert_eq!(matrix.column(4).expect("column"), vec![0.0, 20.0, 30.0]);
        assert_eq!(matrix.column(5).expect("column"), vec![10.0, 40.0, 90.0]);

        let mut starts = Vec::new();
        builder
            .for_each_block(&recipes, 2, |start, block| {
                starts.push((start, block.ncols()));
                Ok(())
            })
            .expect("blocks");
        assert_eq!(starts, vec![(0, 2), (2, 2), (4, 2)]);
    }
}
