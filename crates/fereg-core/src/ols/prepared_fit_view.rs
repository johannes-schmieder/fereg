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
    absorb_borrowed_intercept_auto_initial_with_structural_constant_certified,
    absorb_borrowed_intercept_auto_with_structural_constant_certified,
    absorb_borrowed_intercept_cg_with_structural_constant_certified,
    absorb_borrowed_intercept_irons_tuck_certified,
    absorb_borrowed_intercept_map_with_structural_constant_certified,
    absorb_finalized_categorical_slope_map_certified,
    absorb_finalized_categorical_topology_auto_routed_certified,
    absorb_finalized_categorical_topology_cg_certified,
    absorb_finalized_categorical_topology_cg_retaining_certified,
    absorb_finalized_categorical_topology_irons_tuck_certified,
    absorb_finalized_categorical_topology_lsmr_certified,
    absorb_finalized_categorical_topology_map_certified,
    absorb_finalized_categorical_topology_map_retaining_certified, absorb_fixed_effects,
    borrow_categorical_topology_design, AbsorbConfig, BorrowedInterceptFeDesign,
    BorrowedMapWeights, CertifiedBorrowedMap, CmgAbsorber, CmgConfig, EffectTerm, FeDesign,
    FeSolverKind, MapConfig,
};
use crate::options::ThreadConfig;
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

    fn fit_absorb_map(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
        retain_coefficients: bool,
    ) -> Result<CertifiedBorrowedMap>;

    fn fit_absorb_lsmr(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        self.fit_absorb_owned_matrix(weights, values, config)
    }

    fn fit_owned_fe_design(&self) -> Result<FeDesign>;

    fn fit_supports_cmg_auto(&self) -> bool {
        true
    }

    fn fit_borrowed_cmg_design(&self) -> Result<BorrowedInterceptFeDesign<'_>>;

    fn fit_build_cmg_absorber<'a>(
        &'a self,
        weights: BorrowedMapWeights<'a>,
        config: CmgConfig,
    ) -> Result<CmgAbsorber<'a>> {
        CmgAbsorber::build(
            self.fit_borrowed_cmg_design()?,
            weights,
            config,
            self.fit_structural_intercept(),
        )
    }

    fn fit_absorb_cg(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        self.fit_absorb_owned_matrix(weights, values, config)
    }

    fn fit_absorb_irons_tuck(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        self.fit_absorb_owned_matrix(weights, values, config)
    }

    fn fit_absorb_auto(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        self.fit_absorb_owned_matrix(weights, values, config)
    }

    fn fit_absorb_owned_matrix(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        let design = self.fit_owned_fe_design()?;
        let mut residualized = values;
        let unit_weights;
        let weights = match weights {
            BorrowedMapWeights::Unit => {
                unit_weights = fallible_filled(
                    residualized.nrows(),
                    1.0,
                    "unable to reserve unit weights for prepared FE solver",
                )?;
                unit_weights.as_slice()
            }
            BorrowedMapWeights::Provided(values) => values,
        };
        let report = absorb_fixed_effects(&design, weights, &mut residualized, config)?;
        CertifiedBorrowedMap::from_certified_parts(residualized, report)
    }

    fn fit_absorb_matrix(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        if config.solver == FeSolverKind::Auto {
            return self.fit_absorb_auto(weights, values, config);
        }
        if config.solver == FeSolverKind::Map {
            return self.fit_absorb_map(weights, values, config.map, config.retain_coefficients);
        }
        if config.solver == FeSolverKind::Lsmr {
            return self.fit_absorb_lsmr(weights, values, config);
        }
        if config.solver == FeSolverKind::Cg {
            return self.fit_absorb_cg(weights, values, config);
        }
        if config.solver == FeSolverKind::IronsTuck {
            return self.fit_absorb_irons_tuck(weights, values, config);
        }
        if config.solver == FeSolverKind::Cmg {
            let batch_size = values.ncols().max(1);
            let threads = ThreadConfig::from_requested(rayon::current_num_threads(), false)?;
            let mut session = self.fit_build_cmg_absorber(
                weights,
                CmgConfig {
                    map: config.map,
                    threads,
                    batch_size,
                    retain_coefficients: config.retain_coefficients,
                    requested_solver: FeSolverKind::Cmg,
                    solver_memory_budget: config.solver_memory_budget,
                },
            )?;
            return session.absorb(values);
        }
        self.fit_absorb_owned_matrix(weights, values, config)
    }

    fn fit_absorb_initial_matrix(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        self.fit_absorb_matrix(weights, values, config)
    }
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

    fn fit_absorb_map(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
        _retain_coefficients: bool,
    ) -> Result<CertifiedBorrowedMap> {
        let design = self.borrowed_intercept_fe_design()?;
        absorb_borrowed_intercept_map_with_structural_constant_certified(
            &design, weights, values, config,
        )
    }

    fn fit_absorb_cg(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        let design = self.borrowed_intercept_fe_design()?;
        absorb_borrowed_intercept_cg_with_structural_constant_certified(
            &design, weights, values, config.map,
        )
    }

    fn fit_absorb_irons_tuck(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        let design = self.borrowed_intercept_fe_design()?;
        if design.effects().is_empty() {
            self.fit_absorb_owned_matrix(weights, values, config)
        } else {
            absorb_borrowed_intercept_irons_tuck_certified(
                &design,
                weights,
                values,
                config.map,
                config.retain_coefficients,
            )
        }
    }

    fn fit_absorb_auto(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        let design = self.borrowed_intercept_fe_design()?;
        absorb_borrowed_intercept_auto_with_structural_constant_certified(
            &design,
            weights,
            values,
            config.map,
            config.lsmr,
            config.solver_memory_budget,
        )
    }

    fn fit_absorb_initial_matrix(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        if config.solver == FeSolverKind::Auto {
            let design = self.borrowed_intercept_fe_design()?;
            absorb_borrowed_intercept_auto_initial_with_structural_constant_certified(
                &design,
                weights,
                values,
                config.map,
                config.lsmr,
                config.solver_memory_budget,
            )
        } else {
            self.fit_absorb_matrix(weights, values, config)
        }
    }

    fn fit_owned_fe_design(&self) -> Result<FeDesign> {
        let borrowed = self.borrowed_intercept_fe_design()?;
        let mut effects = Vec::new();
        effects
            .try_reserve_exact(borrowed.effects().len().max(1))
            .map_err(|_| allocation("unable to reserve prepared owned FE effects"))?;
        for effect in borrowed.effects() {
            effects.push(EffectTerm::new(
                fallible_copy(effect.codes(), "unable to reserve prepared owned FE codes")?,
                effect.n_levels(),
                true,
                Vec::new(),
            )?);
        }
        if effects.is_empty() && self.fit_structural_intercept() {
            effects.push(EffectTerm::new(
                fallible_filled(
                    self.fit_sample().n_active(),
                    0_u32,
                    "unable to reserve structural-constant FE codes",
                )?,
                1,
                true,
                Vec::new(),
            )?);
        }
        FeDesign::new(self.fit_sample().n_active(), effects)
    }

    fn fit_borrowed_cmg_design(&self) -> Result<BorrowedInterceptFeDesign<'_>> {
        self.borrowed_intercept_fe_design()
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

    fn fit_supports_cmg_auto(&self) -> bool {
        self.effect_specs()
            .iter()
            .all(|effect| effect.intercept && effect.slopes.is_empty())
    }

    fn fit_column_value(&self, column: usize, row: usize) -> Result<f64> {
        self.column_value(column, row)
    }

    fn fit_build_rhs(&self) -> Result<DenseMatrix> {
        Ok(self.build_rhs()?.into_values())
    }

    fn fit_absorb_map(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
        retain_coefficients: bool,
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
        } else if retain_coefficients {
            absorb_finalized_categorical_topology_map_retaining_certified(
                self.fe_topology(),
                weights,
                values,
                config,
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

    fn fit_absorb_lsmr(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        if self.has_absorbed_slope() {
            self.fit_absorb_owned_matrix(weights, values, config)
        } else {
            absorb_finalized_categorical_topology_lsmr_certified(
                self.fe_topology(),
                weights,
                values,
                config.lsmr,
                config.retain_coefficients,
            )
        }
    }

    fn fit_absorb_cg(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        if self.has_absorbed_slope() {
            self.fit_absorb_owned_matrix(weights, values, config)
        } else if config.retain_coefficients {
            absorb_finalized_categorical_topology_cg_retaining_certified(
                self.fe_topology(),
                weights,
                values,
                config.map,
            )
        } else {
            absorb_finalized_categorical_topology_cg_certified(
                self.fe_topology(),
                weights,
                values,
                config.map,
            )
        }
    }

    fn fit_absorb_irons_tuck(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        if self.has_absorbed_slope() {
            self.fit_absorb_owned_matrix(weights, values, config)
        } else {
            absorb_finalized_categorical_topology_irons_tuck_certified(
                self.fe_topology(),
                weights,
                values,
                config.map,
                config.retain_coefficients,
            )
        }
    }

    fn fit_absorb_auto(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        if self.has_absorbed_slope() {
            self.fit_absorb_owned_matrix(weights, values, config)
        } else {
            absorb_finalized_categorical_topology_auto_routed_certified(
                self.fe_topology(),
                weights,
                values,
                config.map,
                config.lsmr,
                config.solver_memory_budget,
                config.retain_coefficients,
            )
        }
    }

    fn fit_owned_fe_design(&self) -> Result<FeDesign> {
        let topology = self.fe_topology();
        let mut source_table = Vec::new();
        source_table
            .try_reserve_exact(topology.compacted_sources().len())
            .map_err(|_| allocation("unable to reserve categorical FE source views"))?;
        for source in topology.compacted_sources() {
            source_table.push(source.view());
        }
        let borrowed = borrow_categorical_topology_design(
            self.fit_sample().n_active(),
            topology.registry(),
            &source_table,
            topology.effect_to_topology(),
        )?;
        let views = self.effect_views()?;
        if borrowed.effects().len() != views.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "categorical owned FE design and effect metadata differ",
            ));
        }
        let mut effects = Vec::new();
        effects
            .try_reserve_exact(views.len())
            .map_err(|_| allocation("unable to reserve categorical owned FE effects"))?;
        for (effect, view) in borrowed.effects().iter().zip(&views) {
            let mut slopes = Vec::new();
            slopes
                .try_reserve_exact(view.slopes().len())
                .map_err(|_| allocation("unable to reserve categorical owned FE slopes"))?;
            for slope in view.slopes() {
                slopes.push(fallible_copy(
                    slope,
                    "unable to reserve categorical owned FE slope values",
                )?);
            }
            effects.push(EffectTerm::new(
                fallible_copy(
                    effect.codes(),
                    "unable to reserve categorical owned FE codes",
                )?,
                effect.n_levels(),
                view.has_intercept(),
                slopes,
            )?);
        }
        FeDesign::new(self.fit_sample().n_active(), effects)
    }

    fn fit_borrowed_cmg_design(&self) -> Result<BorrowedInterceptFeDesign<'_>> {
        if self
            .effect_specs()
            .iter()
            .any(|effect| !effect.intercept || !effect.slopes.is_empty())
        {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "solver(cmg) supports only categorical intercept-only absorbed effects; absorbed slopes are unsupported",
            ));
        }
        let topology = self.fe_topology();
        let mut source_table = Vec::new();
        source_table
            .try_reserve_exact(topology.compacted_sources().len())
            .map_err(|_| allocation("unable to reserve CMG categorical source views"))?;
        for source in topology.compacted_sources() {
            source_table.push(source.view());
        }
        borrow_categorical_topology_design(
            self.fit_sample().n_active(),
            topology.registry(),
            &source_table,
            topology.effect_to_topology(),
        )
    }
}

fn fallible_copy<T: Copy>(values: &[T], label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| allocation(label))?;
    output.extend_from_slice(values);
    Ok(output)
}

fn fallible_filled<T: Clone>(elements: usize, value: T, label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(label))?;
    output.resize(elements, value);
    Ok(output)
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}
