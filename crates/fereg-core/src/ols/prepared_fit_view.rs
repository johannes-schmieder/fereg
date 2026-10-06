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
    borrow_categorical_topology_design, AbsorbConfig, BorrowedInterceptFeDesign,
    BorrowedMapWeights, CertifiedBorrowedMap, EffectTerm, FeDesign,
};
use crate::memory::OwnedFeDesignMemoryEstimate;
use crate::memory::SolverWorkspacePeak;
use crate::prepared_categorical_data::PreparedCategoricalOlsData;
use rayon::prelude::*;

use super::{PreparedOlsData, PreparedOlsModel};
use crate::ols::build_prepared_ols_rhs_with_response;

/// One-shot ordinary arithmetic borrows the canonical owner. This view has
/// no storage and preserves the mathematically distinct DoF/report family.
pub(crate) struct OrdinaryPrepared<'a>(pub(crate) &'a PreparedOlsData);

impl std::ops::Deref for OrdinaryPrepared<'_> {
    type Target = PreparedOlsData;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

/// PPML generations retain precisely one canonical prepared owner.
pub(crate) type SharedPreparedOlsData = std::sync::Arc<PreparedOlsData>;

/// Scan an immutable prepared column in observation order. Interaction recipes
/// resolve source slices once per bounded tile; no expanded observation bank or
/// current-weight statistic is retained. Callers own all numerical reductions.
pub(crate) fn visit_prepared_column_range<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    column: usize,
    range: std::ops::Range<usize>,
    mut visit: impl FnMut(usize, f64) -> Result<()>,
) -> Result<()> {
    use crate::ols::PreparedOlsColumnKind;
    let invalid = || {
        FeregError::new(
            ErrorCode::InvalidArgument,
            "prepared column scan is not aligned on the active row axis",
        )
    };
    if range.start > range.end || range.end > prepared.fit_sample().n_active() {
        return Err(invalid());
    }
    let kind = &prepared
        .fit_model()
        .columns
        .get(column)
        .ok_or_else(invalid)?
        .kind;
    match kind {
        PreparedOlsColumnKind::Intercept => {
            for row in range {
                visit(row, 1.)?;
            }
        }
        PreparedOlsColumnKind::Continuous {
            prepared_source_index,
            ..
        } => {
            let values = prepared
                .fit_sample()
                .regressors
                .get(*prepared_source_index)
                .and_then(|v| v.get(range.clone()))
                .ok_or_else(invalid)?;
            for (offset, &value) in values.iter().enumerate() {
                visit(range.start + offset, value)?;
            }
        }
        PreparedOlsColumnKind::Categorical {
            categorical_source_index,
            level_code,
            ..
        } => {
            if let Some(level) = level_code {
                let codes = prepared.fit_categorical_codes(*categorical_source_index)?;
                let codes = codes.get(range.clone()).ok_or_else(invalid)?;
                for (offset, code) in codes.iter().enumerate() {
                    visit(range.start + offset, f64::from(code == level))?;
                }
            } else {
                for row in range {
                    visit(row, 0.)?;
                }
            }
        }
        PreparedOlsColumnKind::CategoricalInteraction { components } => {
            const ROWS: usize = 128;
            let mut present = [true; ROWS];
            for start in (range.start..range.end).step_by(ROWS) {
                let end = (start + ROWS).min(range.end);
                let values = &mut present[..end - start];
                values.fill(true);
                for component in components {
                    let Some(level) = component.level_code else {
                        values.fill(false);
                        break;
                    };
                    let codes =
                        prepared.fit_categorical_codes(component.categorical_source_index)?;
                    let codes = codes.get(start..end).ok_or_else(invalid)?;
                    for (value, &code) in values.iter_mut().zip(codes) {
                        *value &= code == level;
                    }
                }
                for (offset, &value) in values.iter().enumerate() {
                    visit(start + offset, f64::from(value))?;
                }
            }
        }
    }
    Ok(())
}

/// Accumulate generated finite predictors without per-cell recipe lookup.
/// Each row keeps coefficient-slot FMA order, including multiplication by zero
/// for absent indicators. Ordinary OLS subtraction has different zero handling
/// and deliberately does not enter this predictor-only helper.
pub(crate) fn add_generated_predictor<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    coefficients: &[crate::ols::PreparedOlsFittedCoefficient],
    output: &mut [f64],
) -> Result<bool> {
    use crate::ols::{PreparedOlsColumnKind, PreparedOlsFitStatus};
    let model = prepared.fit_model();
    let rows = prepared.fit_sample().n_active();
    if rows < 4096
        || model.columns.len() < 8
        || !model.columns.iter().any(|c| {
            matches!(
                c.kind,
                PreparedOlsColumnKind::Categorical { .. }
                    | PreparedOlsColumnKind::CategoricalInteraction { .. }
            )
        })
    {
        return Ok(false);
    }
    let invalid = || {
        FeregError::new(
            ErrorCode::InvalidArgument,
            "generated predictor columns are not aligned and finite",
        )
    };
    if output.len() != rows {
        return Err(invalid());
    }
    for c in coefficients
        .iter()
        .filter(|c| c.status == PreparedOlsFitStatus::Kept)
    {
        if c.generated_column_index
            .is_none_or(|i| i >= model.columns.len())
            || c.value.is_none_or(|v| !v.is_finite())
        {
            return Err(invalid());
        }
    }
    const TILE_ROWS: usize = 128;
    let fill = |(tile, destination): (usize, &mut [f64])| -> Result<()> {
        let start = tile * TILE_ROWS;
        let end = start + destination.len();
        for c in coefficients
            .iter()
            .filter(|c| c.status == PreparedOlsFitStatus::Kept)
        {
            let column = c.generated_column_index.ok_or_else(invalid)?;
            let coefficient = c.value.ok_or_else(invalid)?;
            visit_prepared_column_range(prepared, column, start..end, |row, value| {
                destination[row - start] = coefficient.mul_add(value, destination[row - start]);
                Ok(())
            })?;
        }
        if destination.iter().any(|v| !v.is_finite()) {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "generated finite predictor is not finite",
            ));
        }
        Ok(())
    };
    if rayon::current_thread_index().is_some() && rayon::current_num_threads() > 1 {
        output
            .par_chunks_mut(TILE_ROWS)
            .enumerate()
            .try_for_each(fill)?;
    } else {
        output
            .chunks_mut(TILE_ROWS)
            .enumerate()
            .try_for_each(fill)?;
    }
    Ok(true)
}

/// Common scientific input boundary for the prepared coefficient engine.
///
/// Implementations must return an owned certified result and may not expose a
/// partially transformed matrix on failure.
pub(crate) trait PreparedOlsFitView: Sync {
    fn fit_model(&self) -> &PreparedOlsModel;

    fn fit_sample(&self) -> &PreparedNumericSample;

    fn fit_structural_intercept(&self) -> bool;

    fn fit_has_absorbed_intercept(&self) -> bool;

    fn fit_has_one_slope_effect(&self) -> bool {
        false
    }

    fn fit_has_one_ordinary_intercept_effect(&self) -> bool {
        false
    }

    fn fit_report_intercept(&self) -> bool;

    fn fit_column_value(&self, column: usize, row: usize) -> Result<f64>;

    /// Borrow ordinary factor codes on the current finalized sample.
    fn fit_categorical_codes(&self, source: usize) -> Result<&[u32]>;

    /// Subtract one fitted raw-scale column from an observation vector. The
    /// default is topology-generic; concrete prepared layouts may resolve the
    /// column recipe once and stream its backing storage directly.
    fn fit_subtract_raw_column(
        &self,
        column: usize,
        coefficient: f64,
        residuals: &mut [f64],
    ) -> Result<()> {
        fit_subtract_raw_column_generic(self, column, coefficient, residuals)
    }

    #[allow(dead_code)]
    fn fit_build_rhs(&self) -> Result<DenseMatrix> {
        self.fit_build_rhs_with_response(&self.fit_sample().outcome)
    }

    /// Build the complete generated design beside a borrowed working
    /// response. Implementations retain their layout-specific hot path.
    fn fit_build_rhs_with_response(&self, response: &[f64]) -> Result<DenseMatrix>;

    fn fit_build_rhs_with_loaded_rows(
        &self,
        response: &[f64],
        rows: Option<Vec<f64>>,
    ) -> Result<(DenseMatrix, bool)> {
        fit_build_rhs_with_loaded_rows_generic(self, response, rows)
    }

    /// Build a bounded contiguous block of generated columns. The outcome is
    /// included as column zero only when requested.
    #[allow(dead_code)]
    fn fit_build_rhs_block(
        &self,
        start: usize,
        end: usize,
        include_outcome: bool,
    ) -> Result<DenseMatrix> {
        self.fit_build_rhs_block_with_response(
            start,
            end,
            include_outcome,
            &self.fit_sample().outcome,
        )
    }

    /// Build a bounded generated-column block beside a borrowed response.
    /// The response is read only when `include_response` is true.
    fn fit_build_rhs_block_with_response(
        &self,
        start: usize,
        end: usize,
        include_response: bool,
        response: &[f64],
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
            .checked_add(usize::from(include_response))
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "prepared OLS RHS block width overflows",
                )
            })?;
        let rows = self.fit_sample().n_active();
        if include_response && response.len() != rows {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "prepared OLS response block is not aligned on the active observation axis",
            ));
        }
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
                let offset = usize::from(include_response);
                if include_response {
                    destination[0] = response[row];
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

    fn fit_owned_fe_design(&self) -> Result<FeDesign>;

    fn fit_owned_fe_design_memory(&self) -> Result<OwnedFeDesignMemoryEstimate>;

    /// Initial all-RHS and combined one-RHS products that outlive an FE call.
    /// The current solver's own result/workspace is admitted separately.
    fn fit_retained_fe_parent_bytes(&self) -> Result<u128>;

    /// Borrow the finalized intercept-only topology for PPML's reusable MAP
    /// session. One-shot OLS never calls this seam; slope-bearing prepared
    /// views return `None` and retain the generic owned-design projector.
    fn fit_borrowed_intercept_map_design(&self) -> Result<Option<BorrowedInterceptFeDesign<'_>>> {
        Ok(None)
    }

    fn fit_borrowed_cmg_design(&self) -> Result<BorrowedInterceptFeDesign<'_>>;

    fn fit_absorb_matrix(
        &self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: AbsorbConfig,
    ) -> Result<CertifiedBorrowedMap> {
        let mut session = crate::fe::FeSession::for_ols_rhs(
            self,
            weights,
            crate::ols::PreparedOlsFitConfig {
                absorb: config,
                execution: crate::ols::PreparedOlsExecutionConfig {
                    poolsize: values.ncols().max(1),
                    threads: config.lsmr.threads,
                    ..crate::ols::PreparedOlsExecutionConfig::default()
                },
                ..crate::ols::PreparedOlsFitConfig::default()
            },
            values.ncols().max(1),
        )?;
        session.project_ols(weights, values, config, false, false)
    }
}

// Direct prepared dispatch can bypass the automatic LSMR/CG planner. Apply
// that same joint bound before CG allocates, including its retained coefficient
// recurrence. Structural-constant and slope adapters keep their separate plans.
pub(crate) fn admit_prepared_intercept_cg(
    design: &BorrowedInterceptFeDesign<'_>,
    rhs_columns: usize,
    budget: Option<crate::memory::FeSolverMemoryBudget>,
    retain_coefficients: bool,
) -> Result<SolverWorkspacePeak> {
    let Some(budget) = budget else {
        return Ok(SolverWorkspacePeak::UNMODELED);
    };
    if design.effects().is_empty() {
        return Ok(SolverWorkspacePeak::UNMODELED);
    }
    let mut levels = Vec::new();
    levels
        .try_reserve_exact(design.effects().len())
        .map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve CG planning level counts",
            )
        })?;
    levels.extend(design.effects().iter().map(|effect| effect.n_levels()));
    let estimate = crate::memory::estimate_fe_solver_memory(
        design.nobs(),
        rhs_columns,
        &levels,
        rayon::current_num_threads(),
        None,
    )?;
    let coefficients = if retain_coefficients {
        estimate.coefficient_batch_bytes.checked_mul(5)
    } else {
        Some(0)
    };
    let required = coefficients
        .and_then(|bytes| estimate.cg_bytes.checked_add(bytes))
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "prepared CG workspace memory overflows",
            )
        })?;
    if !budget.fits(required) {
        return Err(FeregError::new(
            ErrorCode::Allocation,
            "prepared CG does not fit the joint execution/CG memory plan",
        ));
    }
    SolverWorkspacePeak::from_bytes(required)
}

impl PreparedOlsFitView for OrdinaryPrepared<'_> {
    fn fit_has_one_ordinary_intercept_effect(&self) -> bool {
        self.fe_effects().len() == 1
    }

    fn fit_categorical_codes(&self, source: usize) -> Result<&[u32]> {
        self.categorical_codes(source)
    }

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

    fn fit_subtract_raw_column(
        &self,
        column: usize,
        coefficient: f64,
        residuals: &mut [f64],
    ) -> Result<()> {
        if residuals.len() != self.sample.n_active() || !coefficient.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "prepared OLS raw-residual update has an invalid shape",
            ));
        }
        let recipe = self.model.columns.get(column).ok_or_else(|| {
            FeregError::new(
                ErrorCode::InvalidArgument,
                "prepared OLS raw-residual column is out of range",
            )
        })?;
        match &recipe.kind {
            super::PreparedOlsColumnKind::Intercept => {
                for residual in residuals.iter_mut() {
                    *residual = (-coefficient).mul_add(1.0, *residual);
                }
            }
            super::PreparedOlsColumnKind::Continuous {
                prepared_source_index,
                ..
            } => {
                let source = self
                    .sample
                    .regressors
                    .get(*prepared_source_index)
                    .ok_or_else(|| {
                        FeregError::new(
                            ErrorCode::InvalidArgument,
                            "prepared continuous source is out of range",
                        )
                    })?;
                for (residual, value) in residuals.iter_mut().zip(source) {
                    *residual = (-coefficient).mul_add(*value, *residual);
                }
            }
            super::PreparedOlsColumnKind::Categorical {
                categorical_source_index,
                level_code,
                ..
            } => {
                if let Some(level_code) = level_code {
                    let codes = self.categorical_codes(*categorical_source_index)?;
                    for (residual, code) in residuals.iter_mut().zip(codes) {
                        if code == level_code {
                            *residual = (-coefficient).mul_add(1.0, *residual);
                        }
                    }
                }
            }
            super::PreparedOlsColumnKind::CategoricalInteraction { .. } => {
                visit_prepared_column_range(self, column, 0..residuals.len(), |row, value| {
                    if value != 0.0 {
                        residuals[row] = (-coefficient).mul_add(1.0, residuals[row]);
                    }
                    Ok(())
                })?;
            }
        }
        if residuals.iter().any(|value| !value.is_finite()) {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "prepared OLS raw residual is not finite",
            ));
        }
        Ok(())
    }

    fn fit_build_rhs_with_response(&self, response: &[f64]) -> Result<DenseMatrix> {
        Ok(build_prepared_ols_rhs_with_response(self, response)?.into_values())
    }

    fn fit_build_rhs_with_loaded_rows(
        &self,
        response: &[f64],
        rows: Option<Vec<f64>>,
    ) -> Result<(DenseMatrix, bool)> {
        let (rhs, reused) =
            crate::ols::build_prepared_ols_rhs_with_loaded_rows_and_reuse(self, response, rows)?;
        Ok((rhs.into_values(), reused))
    }

    fn fit_retained_fe_parent_bytes(&self) -> Result<u128> {
        self.0.fit_retained_fe_parent_bytes()
    }

    fn fit_owned_fe_design_memory(&self) -> Result<OwnedFeDesignMemoryEstimate> {
        let borrowed = self.borrowed_intercept_fe_design()?;
        let mut memory = OwnedFeDesignMemoryEstimate::default();
        for effect in borrowed.effects() {
            memory.add_effect(self.fit_sample().n_active(), effect.n_levels(), 0)?;
        }
        if borrowed.effects().is_empty() {
            if self.fit_structural_intercept() {
                memory.add_effect(self.fit_sample().n_active(), 1, 0)?;
            } else {
                // The existing owned constructor reserves at least one slot.
                memory.resident_bytes = std::mem::size_of::<EffectTerm>() as u128;
                memory.construction_bytes = memory.resident_bytes;
            }
        }
        memory.add_view_metadata(0, borrowed.effects().len(), 0)?;
        Ok(memory)
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

    fn fit_borrowed_intercept_map_design(&self) -> Result<Option<BorrowedInterceptFeDesign<'_>>> {
        Ok(Some(self.borrowed_intercept_fe_design()?))
    }

    fn fit_borrowed_cmg_design(&self) -> Result<BorrowedInterceptFeDesign<'_>> {
        self.borrowed_intercept_fe_design()
    }
}

impl PreparedOlsFitView for PreparedCategoricalOlsData {
    fn fit_build_rhs_with_loaded_rows(
        &self,
        response: &[f64],
        rows: Option<Vec<f64>>,
    ) -> Result<(DenseMatrix, bool)> {
        if !self.uses_categorical_preparation() {
            return OrdinaryPrepared(self).fit_build_rhs_with_loaded_rows(response, rows);
        }
        fit_build_rhs_with_loaded_rows_generic(self, response, rows)
    }

    fn fit_subtract_raw_column(
        &self,
        column: usize,
        coefficient: f64,
        residuals: &mut [f64],
    ) -> Result<()> {
        if !self.uses_categorical_preparation() {
            return OrdinaryPrepared(self).fit_subtract_raw_column(column, coefficient, residuals);
        }
        fit_subtract_raw_column_generic(self, column, coefficient, residuals)
    }

    fn fit_has_one_slope_effect(&self) -> bool {
        self.effect_specs().len() == 1 && !self.effect_specs()[0].slopes.is_empty()
    }

    fn fit_has_one_ordinary_intercept_effect(&self) -> bool {
        let effects = self.effect_specs();
        effects.len() == 1 && effects[0].intercept && effects[0].slopes.is_empty()
    }

    fn fit_categorical_codes(&self, source: usize) -> Result<&[u32]> {
        self.categorical_sources()
            .get(source)
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::InvalidArgument,
                    "prepared categorical source is out of range",
                )
            })?
            .codes(self.fe_topology())
    }

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
        if self.uses_categorical_preparation() {
            self.has_absorbed_intercept()
        } else {
            self.structural_intercept()
        }
    }

    fn fit_report_intercept(&self) -> bool {
        self.report_intercept()
    }

    fn fit_column_value(&self, column: usize, row: usize) -> Result<f64> {
        self.column_value(column, row)
    }

    fn fit_build_rhs_with_response(&self, response: &[f64]) -> Result<DenseMatrix> {
        if !self.uses_categorical_preparation() {
            return OrdinaryPrepared(self).fit_build_rhs_with_response(response);
        }
        Ok(self.build_rhs_with_response(response)?.into_values())
    }

    fn fit_retained_fe_parent_bytes(&self) -> Result<u128> {
        let topology = self.fe_topology();
        if topology.effect_to_topology().len() != self.effect_specs().len() {
            return Err(allocation("retained FE topology and effects differ"));
        }
        let mut directions = 0_u128;
        for (&index, spec) in topology
            .effect_to_topology()
            .iter()
            .zip(self.effect_specs())
        {
            let channels = spec.slopes.len() as u128 + u128::from(spec.intercept);
            let cells = (topology.registry().source(index)?.n_levels() as u128)
                .checked_mul(channels)
                .ok_or_else(|| allocation("retained FE directions overflow"))?;
            directions = directions
                .checked_add(cells)
                .ok_or_else(|| allocation("retained FE directions overflow"))?;
        }
        let effects = self.effect_specs().len();
        let virtual_constant = effects == 0 && self.fit_structural_intercept();
        retained_fe_parent_bytes(
            if virtual_constant { 1 } else { directions },
            effects + usize::from(virtual_constant),
            self.model().columns.len(),
        )
    }

    fn fit_owned_fe_design_memory(&self) -> Result<OwnedFeDesignMemoryEstimate> {
        if !self.uses_categorical_preparation() {
            return OrdinaryPrepared(self).fit_owned_fe_design_memory();
        }
        let topology = self.fe_topology();
        if topology.effect_to_topology().len() != self.effect_specs().len() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "categorical owned FE memory shape and effects differ",
            ));
        }
        let mut memory = OwnedFeDesignMemoryEstimate::default();
        let mut slopes = 0_usize;
        for (&index, spec) in topology
            .effect_to_topology()
            .iter()
            .zip(self.effect_specs())
        {
            memory.add_effect(
                self.fit_sample().n_active(),
                topology.registry().source(index)?.n_levels(),
                spec.slopes.len(),
            )?;
            slopes = slopes
                .checked_add(spec.slopes.len())
                .ok_or_else(|| allocation("owned FE slope count overflows"))?;
        }
        let sources = topology
            .compacted_sources()
            .len()
            .checked_add(topology.registry().sources().len())
            .ok_or_else(|| allocation("owned FE source count overflows"))?;
        memory.add_view_metadata(sources, self.effect_specs().len(), slopes)?;
        Ok(memory)
    }

    fn fit_owned_fe_design(&self) -> Result<FeDesign> {
        if !self.uses_categorical_preparation() {
            return OrdinaryPrepared(self).fit_owned_fe_design();
        }
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

    fn fit_borrowed_intercept_map_design(&self) -> Result<Option<BorrowedInterceptFeDesign<'_>>> {
        if !self.uses_categorical_preparation() {
            return self.borrowed_intercept_fe_design().map(Some);
        }
        if self.has_absorbed_slope() {
            Ok(None)
        } else {
            Ok(Some(self.fit_borrowed_cmg_design()?))
        }
    }

    fn fit_borrowed_cmg_design(&self) -> Result<BorrowedInterceptFeDesign<'_>> {
        if !self.uses_categorical_preparation() {
            return self.borrowed_intercept_fe_design();
        }
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
        if let Some(validated) = self.validated_single_topology()? {
            return validated.into_design(self.fit_sample().n_active());
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

/// Initial K+1 RHS and the combined one-RHS result can coexist. The residual
/// solver's own product is already in its workspace report. Repeated effects
/// are counted separately because coefficient batches preserve their ordering.
fn retained_fe_parent_bytes(directions: u128, effects: usize, generated: usize) -> Result<u128> {
    if effects == 0 {
        return Ok(0);
    }
    let values = directions
        .checked_mul(generated as u128 + 2)
        .and_then(|cells| cells.checked_mul(8));
    let metadata = (effects as u128)
        .checked_mul(3)
        .and_then(|cells| cells.checked_add(1))
        .and_then(|cells| cells.checked_mul(2 * std::mem::size_of::<usize>() as u128));
    values
        .and_then(|bytes| bytes.checked_add(metadata?))
        .ok_or_else(|| allocation("retained FE product parent overflows"))
}

fn fit_subtract_raw_column_generic<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    column: usize,
    coefficient: f64,
    residuals: &mut [f64],
) -> Result<()> {
    if residuals.len() != prepared.fit_sample().n_active() || !coefficient.is_finite() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "prepared OLS raw-residual update has an invalid shape",
        ));
    }
    residuals
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(row, residual)| -> Result<()> {
            *residual = (-coefficient).mul_add(prepared.fit_column_value(column, row)?, *residual);
            if residual.is_finite() {
                Ok(())
            } else {
                Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "prepared OLS raw residual is not finite",
                ))
            }
        })
}

fn fit_build_rhs_with_loaded_rows_generic<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    response: &[f64],
    rows: Option<Vec<f64>>,
) -> Result<(DenseMatrix, bool)> {
    if let Some(rows) = rows {
        if let Some(columns) = crate::ols::NumericOlsColumns::bind_loaded_rhs(
            prepared.fit_model(),
            prepared.fit_sample(),
        )? {
            if columns.matches_rhs(response, &rows) {
                return Ok((
                    DenseMatrix::from_row_major(
                        response.len(),
                        prepared.fit_model().columns.len() + 1,
                        rows,
                    )?,
                    true,
                ));
            }
        }
    }
    prepared
        .fit_build_rhs_with_response(response)
        .map(|matrix| (matrix, false))
}

#[cfg(test)]
mod retained_parent_tests {
    use super::*;
    #[test]
    fn retained_product_parent_counts_both_lifetimes_and_checks_overflow() {
        // Two effects, seven coefficient directions, K=3: the initial four
        // RHS and combined one RHS coexist; each has three descriptor arrays.
        assert_eq!(
            retained_fe_parent_bytes(7, 2, 3).unwrap(),
            7 * 5 * 8 + 2 * 7 * std::mem::size_of::<usize>() as u128
        );
        assert_eq!(retained_fe_parent_bytes(0, 0, 3).unwrap(), 0);
        assert_eq!(
            retained_fe_parent_bytes(u128::MAX, 2, 3).unwrap_err().code,
            ErrorCode::Allocation
        );
    }
}
