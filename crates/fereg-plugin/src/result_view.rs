//! Borrowed prepared-result views for the hidden OLS transport.
//!
//! Legacy single-source and categorical-interaction preparation own different
//! topology artifacts but expose the same final model, sample, singleton, and
//! cluster metadata. These copyable views let one serializer borrow either path
//! without cloning observation-scale state or changing result-wire dimensions.

use fereg_core::error::Result;
use fereg_core::{
    PreparedNumericSample, PreparedOlsClusterDesign, PreparedOlsData, PreparedOlsModel,
    PreparedOlsSingletons,
};
use rayon::prelude::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PreparedOlsResultData<'a>(&'a PreparedOlsData);

impl<'a> PreparedOlsResultData<'a> {
    pub(crate) const fn new(value: &'a PreparedOlsData) -> Self {
        Self(value)
    }
    pub(crate) const fn model(self) -> &'a PreparedOlsModel {
        self.0.model()
    }
    pub(crate) const fn sample(self) -> &'a PreparedNumericSample {
        self.0.sample()
    }
    pub(crate) fn column_value(self, column: usize, row: usize) -> Result<f64> {
        self.0.column_value(column, row)
    }
    pub(crate) fn add_fitted_terms(self, terms: &[(usize, f64)], output: &mut [f64]) -> Result<()> {
        if !fereg_core::ols::add_numeric_ols_predictor(self.model(), self.sample(), terms, output)?
        {
            output
                .par_iter_mut()
                .enumerate()
                .try_for_each(|(row, value)| -> Result<()> {
                    for &(column, coefficient) in terms {
                        *value = coefficient.mul_add(self.column_value(column, row)?, *value);
                    }
                    Ok(())
                })?;
        }
        Ok(())
    }
    pub(crate) const fn singletons(self) -> PreparedOlsSingletons {
        self.0.singletons()
    }
    pub(crate) fn coefficient_capacity(self) -> usize {
        self.0.report_plan().original_coefficient_count()
    }
    pub(crate) const fn categorical_data(self) -> Option<&'a PreparedOlsData> {
        if self.0.uses_categorical_preparation() {
            Some(self.0)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PreparedOlsClusteredResultData<'a>(&'a PreparedOlsData);

impl<'a> PreparedOlsClusteredResultData<'a> {
    pub(crate) const fn new(value: &'a PreparedOlsData) -> Self {
        Self(value)
    }
    pub(crate) fn clusters(self) -> &'a PreparedOlsClusterDesign {
        self.0.clusters()
    }
    pub(crate) fn is_bound_to(self, prepared: PreparedOlsResultData<'a>) -> bool {
        std::ptr::eq(self.0, prepared.0)
    }
}
