//! Shared data/absorption view for certified prepared OLS fitting.
//!
//! The legacy prepared path and the categorical-interaction path differ only in
//! how they own and validate fixed-effect topology.  They must not fork the
//! rank-revealing QR, bread, residual, or score-certificate implementation.
//! This trait exposes the common immutable model/sample surface and one
//! topology-specific certified absorption operation.

use crate::data::{DenseMatrix, PreparedNumericSample};
use crate::error::Result;
use crate::fe::{
    absorb_borrowed_intercept_map_with_structural_constant_certified,
    absorb_finalized_categorical_topology_map_certified, BorrowedMapWeights,
    CertifiedBorrowedMap, MapConfig,
};
use crate::prepared_categorical_data::PreparedCategoricalOlsData;

use super::{
    build_prepared_ols_rhs, PreparedOlsData, PreparedOlsModel,
};

/// Common scientific input boundary for the prepared coefficient engine.
///
/// Implementations must return an owned certified result and may not expose a
/// partially transformed matrix on failure.
pub(crate) trait PreparedOlsFitView {
    fn fit_model(&self) -> &PreparedOlsModel;

    fn fit_sample(&self) -> &PreparedNumericSample;

    fn fit_structural_intercept(&self) -> bool;

    fn fit_report_intercept(&self) -> bool;

    fn fit_column_value(&self, column: usize, row: usize) -> Result<f64>;

    fn fit_build_rhs(&self) -> Result<DenseMatrix>;

    fn fit_absorb_matrix(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
    ) -> Result<CertifiedBorrowedMap>;
}

impl PreparedOlsFitView for PreparedOlsData {
    fn fit_model(&self) -> &PreparedOlsModel {
        &self.model
    }

    fn fit_sample(&self) -> &PreparedNumericSample {
        &self.sample
    }

    fn fit_structural_intercept(&self) -> bool {
        self.structural_intercept
    }

    fn fit_report_intercept(&self) -> bool {
        self.report_intercept
    }

    fn fit_column_value(&self, column: usize, row: usize) -> Result<f64> {
        self.column_value(column, row)
    }

    fn fit_build_rhs(&self) -> Result<DenseMatrix> {
        Ok(build_prepared_ols_rhs(self)?.into_values())
    }

    fn fit_absorb_matrix(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
    ) -> Result<CertifiedBorrowedMap> {
        let design = self.borrowed_intercept_fe_design()?;
        absorb_borrowed_intercept_map_with_structural_constant_certified(
            &design,
            weights,
            values,
            config,
        )
    }
}

impl PreparedOlsFitView for PreparedCategoricalOlsData {
    fn fit_model(&self) -> &PreparedOlsModel {
        self.model()
    }

    fn fit_sample(&self) -> &PreparedNumericSample {
        self.sample()
    }

    fn fit_structural_intercept(&self) -> bool {
        self.structural_intercept()
    }

    fn fit_report_intercept(&self) -> bool {
        self.report_intercept()
    }

    fn fit_column_value(&self, column: usize, row: usize) -> Result<f64> {
        self.column_value(column, row)
    }

    fn fit_build_rhs(&self) -> Result<DenseMatrix> {
        Ok(self.build_rhs()?.into_values())
    }

    fn fit_absorb_matrix(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
    ) -> Result<CertifiedBorrowedMap> {
        absorb_finalized_categorical_topology_map_certified(
            self.fe_topology(),
            weights,
            values,
            config,
        )
    }
}
