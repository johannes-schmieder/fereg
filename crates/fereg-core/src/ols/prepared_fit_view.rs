//! Shared data/absorption view for certified prepared OLS fitting.
//!
//! The legacy prepared path and the categorical-interaction path differ only in
//! how they own and validate fixed-effect topology. They must not fork the
//! rank-revealing QR, bread, residual, or score-certificate implementation.
//! This trait exposes the common immutable model/sample surface and one
//! topology-specific certified absorption operation.

use crate::data::{DenseMatrix, PreparedNumericSample};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    absorb_borrowed_intercept_map_with_structural_constant_certified,
    absorb_finalized_categorical_slope_map_certified,
    absorb_finalized_categorical_topology_map_certified, BorrowedMapWeights, CertifiedBorrowedMap,
    MapConfig,
};
use crate::prepared_categorical_data::PreparedCategoricalOlsData;
use rayon::prelude::*;

use super::{build_prepared_ols_rhs, PreparedOlsData, PreparedOlsModel};

/// Common scientific input boundary for the prepared coefficient engine.
///
/// Implementations must return an owned certified result and may not expose a
/// partially transformed matrix on failure.
pub(crate) trait PreparedOlsFitView: Sync {
    fn fit_model(&self) -> &PreparedOlsModel;

    fn fit_sample(&self) -> &PreparedNumericSample;

    fn fit_structural_intercept(&self) -> bool;

    fn fit_has_absorbed_intercept(&self) -> bool;

    fn fit_report_intercept(&self) -> bool;

    fn fit_column_value(&self, column: usize, row: usize) -> Result<f64>;

    fn fit_build_rhs(&self) -> Result<DenseMatrix>;

    /// Build a bounded contiguous block of generated columns. The outcome is
    /// included as column zero only when requested.
    fn fit_build_rhs_block(
        &self,
        start: usize,
        end: usize,
        include_outcome: bool,
    ) -> Result<DenseMatrix> {
        let generated = self.fit_model().columns.len();
        if start > end || end > generated {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "prepared OLS RHS block is out of range",
            ));
        }
        let block = end - start;
        let columns = block
            .checked_add(usize::from(include_outcome))
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "prepared OLS RHS block width overflows",
                )
            })?;
        let rows = self.fit_sample().n_active();
        let elements = rows.checked_mul(columns).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "prepared OLS RHS block dimensions overflow",
            )
        })?;
        let mut values = Vec::new();
        values.try_reserve_exact(elements).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve prepared OLS RHS block",
            )
        })?;
        if values.capacity() < elements {
            return Err(FeregError::new(
                ErrorCode::Allocation,
                "prepared OLS RHS block reservation was undersized",
            ));
        }
        values.resize(elements, 0.0);
        values.par_chunks_mut(columns).enumerate().try_for_each(
            |(row, destination)| -> Result<()> {
                let offset = usize::from(include_outcome);
                if include_outcome {
                    destination[0] = self.fit_sample().outcome[row];
                }
                for (position, generated_column) in (start..end).enumerate() {
                    destination[offset + position] =
                        self.fit_column_value(generated_column, row)?;
                }
                Ok(())
            },
        )?;
        if values.iter().any(|value| !value.is_finite()) {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "prepared OLS RHS block contains a nonfinite value",
            ));
        }
        DenseMatrix::from_row_major(rows, columns, values)
    }

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

    fn fit_has_absorbed_intercept(&self) -> bool {
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
            &design, weights, values, config,
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

    fn fit_has_absorbed_intercept(&self) -> bool {
        self.has_absorbed_intercept()
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
        if self.has_absorbed_slope() {
            let effects = self.effect_views()?;
            absorb_finalized_categorical_slope_map_certified(
                self.fe_topology(),
                &effects,
                weights,
                values,
                self.slope_map_config(config),
            )
        } else {
            absorb_finalized_categorical_topology_map_certified(
                self.fe_topology(),
                weights,
                values,
                config,
            )
        }
    }
}
