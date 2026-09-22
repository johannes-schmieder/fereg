use crate::data::{DenseMatrix, PreparedAdjustment, PreparedWeights, WeightKind};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    absorb_borrowed_intercept_map_with_structural_constant_certified, BorrowedMapWeights,
    CertifiedBorrowedMap, MapConfig,
};
use crate::metadata::SourceId;
use rayon::prelude::*;

use super::{PreparedOlsCoefficientStatus, PreparedOlsColumnKind, PreparedOlsData};

/// One directly materialized, observation-major OLS absorption batch.
///
/// Column zero is the prepared outcome. Column `j + 1` is exactly generated
/// model column `j`; syntax-omitted coefficient slots have no RHS column.
#[derive(Debug, PartialEq)]
pub struct PreparedOlsRhs {
    values: DenseMatrix,
}

impl PreparedOlsRhs {
    #[must_use]
    pub const fn values(&self) -> &DenseMatrix {
        &self.values
    }

    #[must_use]
    pub fn into_values(self) -> DenseMatrix {
        self.values
    }

    #[must_use]
    pub const fn outcome_column() -> usize {
        0
    }

    pub fn model_column(generated_column_index: usize) -> Result<usize> {
        generated_column_index
            .checked_add(1)
            .ok_or_else(|| allocation("prepared OLS RHS column index overflows"))
    }
}

trait RhsReservation {
    fn reserve_f64(&mut self, target: &mut Vec<f64>, elements: usize) -> Result<()>;
}

struct SystemRhsReservation;

impl RhsReservation for SystemRhsReservation {
    fn reserve_f64(&mut self, target: &mut Vec<f64>, elements: usize) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation("unable to reserve prepared OLS Y/X RHS"))
    }
}

/// Build the complete prepared Y/X batch without intermediate observation-
/// scale columns. The sole new observation-scale allocation is the returned
/// dense matrix.
pub fn build_prepared_ols_rhs(prepared: &PreparedOlsData) -> Result<PreparedOlsRhs> {
    build_prepared_ols_rhs_with_response(prepared, &prepared.sample.outcome)
}

/// Build the complete prepared X batch beside an explicitly borrowed response.
///
/// This is the crate-private weighted-linear seam used by estimators whose
/// working response changes while the prepared regressor recipes remain
/// fixed. It preserves the OLS builder's single observation-major allocation
/// and its established fill order.
pub(crate) fn build_prepared_ols_rhs_with_response(
    prepared: &PreparedOlsData,
    response: &[f64],
) -> Result<PreparedOlsRhs> {
    build_prepared_ols_rhs_with_reservation(prepared, response, &mut SystemRhsReservation)
}

/// Build and consume the prepared Y/X batch through the strict borrowed MAP
/// path. No residualized caller-owned state is observable on failure.
pub fn absorb_prepared_ols_rhs_map(
    prepared: &PreparedOlsData,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    // Validate and reserve the small borrowed design before allocating N x K.
    let design = prepared.borrowed_intercept_fe_design()?;
    let rhs = build_prepared_ols_rhs(prepared)?.into_values();
    let weights = match &prepared.sample.weights {
        PreparedWeights::None => BorrowedMapWeights::Unit,
        PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
    };
    absorb_borrowed_intercept_map_with_structural_constant_certified(&design, weights, rhs, config)
}

/// Reuse a speculative input matrix only after the authoritative sample and
/// every numeric value agree. Failed speculation frees the owner before the
/// ordinary builder runs; it cannot change samples, recipes or error ordering.
#[cfg(test)]
fn build_prepared_ols_rhs_with_loaded_rows(
    prepared: &PreparedOlsData,
    response: &[f64],
    rows: Option<Vec<f64>>,
) -> Result<PreparedOlsRhs> {
    build_prepared_ols_rhs_with_loaded_rows_and_reuse(prepared, response, rows).map(|result| result.0)
}

pub(crate) fn build_prepared_ols_rhs_with_loaded_rows_and_reuse(
    prepared: &PreparedOlsData, response: &[f64], rows: Option<Vec<f64>>,
) -> Result<(PreparedOlsRhs, bool)> {
    if let Some(rows) = rows {
        validate_prepared_rhs_inputs(prepared, response)?;
        if let Some(columns) = NumericOlsColumns::bind_loaded_rhs(&prepared.model, &prepared.sample)? {
            if columns.matches_rhs(response, &rows) {
                return Ok((PreparedOlsRhs { values: DenseMatrix::from_row_major(
                    response.len(), prepared.model.columns.len() + 1, rows,
                )? }, true));
            }
        }
    }
    build_prepared_ols_rhs_with_response(prepared, response).map(|rhs| (rhs, false))
}

fn build_prepared_ols_rhs_with_reservation<R: RhsReservation>(
    prepared: &PreparedOlsData,
    response: &[f64],
    reservation: &mut R,
) -> Result<PreparedOlsRhs> {
    validate_prepared_rhs_inputs(prepared, response)?;
    let (nrows, ncols, elements) =
        rhs_dimensions(prepared.sample.n_active(), prepared.model.columns.len())?;
    let mut data = Vec::new();
    reservation.reserve_f64(&mut data, elements)?;
    if data.capacity() < elements {
        return Err(allocation(
            "prepared OLS RHS reservation returned insufficient capacity",
        ));
    }
    if let Some(columns) = NumericOlsColumns::bind(&prepared.model, &prepared.sample)? {
        columns.fill_reserved_rhs(response, &mut data)?;
    } else {
        data.resize(elements, 0.0);
        if rayon::current_num_threads() == 1 {
            fill_rhs_single_thread(prepared, response, ncols, &mut data)?;
        } else {
            data.par_chunks_exact_mut(ncols).enumerate().try_for_each(
                |(row, destination)| -> Result<()> {
                    destination[0] = response[row];
                    for generated_column in 0..prepared.model.columns.len() {
                        let rhs_column = PreparedOlsRhs::model_column(generated_column)?;
                        destination[rhs_column] = prepared.column_value(generated_column, row)?;
                    }
                    Ok(())
                },
            )?;
        }
    }
    if data.par_iter().any(|value| !value.is_finite()) {
        return Err(invalid_data(
            "prepared OLS RHS construction produced a nonfinite value",
        ));
    }
    Ok(PreparedOlsRhs {
        values: DenseMatrix::from_row_major(nrows, ncols, data)?,
    })
}

/// Fill the row-major batch one generated recipe at a time. On one thread this
/// resolves each recipe once and then streams its source/code column instead
/// of repeating the recipe dispatch for every observation.
fn fill_rhs_single_thread(
    prepared: &PreparedOlsData,
    response: &[f64],
    ncols: usize,
    data: &mut [f64],
) -> Result<()> {
    for (row, value) in response.iter().copied().enumerate() {
        data[row * ncols] = value;
    }
    for (generated, column) in prepared.model.columns.iter().enumerate() {
        let rhs_column = PreparedOlsRhs::model_column(generated)?;
        match &column.kind {
            PreparedOlsColumnKind::Intercept => {
                for row in 0..prepared.sample.n_active() {
                    data[row * ncols + rhs_column] = 1.0;
                }
            }
            PreparedOlsColumnKind::Continuous {
                prepared_source_index,
                ..
            } => {
                let source = prepared
                    .sample
                    .regressors
                    .get(*prepared_source_index)
                    .ok_or_else(|| {
                        invalid_argument("prepared continuous source is out of range")
                    })?;
                for (row, value) in source.iter().copied().enumerate() {
                    data[row * ncols + rhs_column] = value;
                }
            }
            PreparedOlsColumnKind::Categorical {
                categorical_source_index,
                level_code,
                ..
            } => {
                let codes = prepared
                    .categorical_sources
                    .get(*categorical_source_index)
                    .ok_or_else(|| invalid_argument("prepared categorical source is out of range"))?
                    .encoded
                    .codes
                    .as_slice();
                if let Some(level_code) = level_code {
                    for (row, code) in codes.iter().copied().enumerate() {
                        data[row * ncols + rhs_column] =
                            if code == *level_code { 1.0 } else { 0.0 };
                    }
                }
            }
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                for row in 0..prepared.sample.n_active() {
                    let mut present = true;
                    for component in components {
                        let Some(level_code) = component.level_code else {
                            present = false;
                            break;
                        };
                        let code = prepared
                            .categorical_sources
                            .get(component.categorical_source_index)
                            .and_then(|source| source.encoded.codes.get(row))
                            .copied()
                            .ok_or_else(|| {
                                invalid_argument(
                                    "prepared categorical interaction source is out of range",
                                )
                            })?;
                        if code != level_code {
                            present = false;
                            break;
                        }
                    }
                    data[row * ncols + rhs_column] = if present { 1.0 } else { 0.0 };
                }
            }
        }
    }
    Ok(())
}

/// Validation has no reduction or ordering-dependent diagnostic. Partition
/// wide source scans across an already installed pool. Preparation can call
/// validation before entering the configured team, so never create an ambient
/// global pool from this shared helper; keep those and small-model scans serial.
pub(crate) fn numeric_sources_are_finite(sample: &crate::data::PreparedNumericSample) -> bool {
    if sample.n_active() >= 4096
        && sample.regressors.len() >= 8
        && rayon::current_thread_index().is_some()
    {
        sample
            .regressors
            .par_iter()
            .all(|source| source.iter().all(|v| v.is_finite()))
    } else {
        sample.regressors.iter().flatten().all(|v| v.is_finite())
    }
}

const NUMERIC_TILE_ROWS: usize = 128;
pub(crate) const MAX_BOUND_NUMERIC_COLUMNS: usize = 64;

/// Add a numeric prepared predictor in coefficient-slot order using bounded
/// row tiles. Unsupported designs return false without changing the output.
/// This shares the solver's numeric binding with final output construction.
pub fn add_numeric_ols_predictor(
    model: &super::PreparedOlsModel,
    sample: &crate::data::PreparedNumericSample,
    selected: &[(usize, f64)],
    output: &mut [f64],
) -> Result<bool> {
    let Some(columns) = NumericOlsColumns::bind(model, sample)? else {
        return Ok(false);
    };
    columns.add_selected_columns(selected, output)?;
    Ok(true)
}

/// Stack-only bindings for the common numeric design. Keeping a row tile in
/// cache avoids per-cell recipe dispatch and repeated full residual scans.
/// None denotes an explicit intercept; unsupported recipes use the existing
/// generic evaluator without partially filling or updating the destination.
pub(crate) struct NumericOlsColumns<'a> {
    columns: [Option<&'a [f64]>; MAX_BOUND_NUMERIC_COLUMNS],
    width: usize,
    rows: usize,
}

impl<'a> NumericOlsColumns<'a> {
    pub(crate) fn bind_score_means(
        model: &super::PreparedOlsModel,
        sample: &'a crate::data::PreparedNumericSample,
    ) -> Result<Option<Self>> {
        Self::bind_with_minimum(model, sample, 1)
    }

    /// None is an explicit intercept; every borrowed source has the active row count.
    pub(crate) fn raw_columns(&self) -> &[Option<&'a [f64]>] {
        &self.columns[..self.width]
    }

    pub(crate) fn bind(
        model: &super::PreparedOlsModel,
        sample: &'a crate::data::PreparedNumericSample,
    ) -> Result<Option<Self>> {
        Self::bind_with_minimum(model, sample, 8)
    }

    pub(crate) fn bind_loaded_rhs(
        model: &super::PreparedOlsModel,
        sample: &'a crate::data::PreparedNumericSample,
    ) -> Result<Option<Self>> {
        // Narrow transport admission does not widen the separate predictor,
        // residual-update and ordinary RHS-construction fast paths.
        let minimum = if sample.n_active() >= 16_384 && rayon::current_num_threads() >= 8 { 2 } else { 8 };
        Self::bind_with_minimum(model, sample, minimum)
    }

    pub(crate) fn bind_residual_mean(
        model: &super::PreparedOlsModel,
        sample: &'a crate::data::PreparedNumericSample,
    ) -> Result<Option<Self>> {
        Self::bind_with_minimum(model, sample, 2)
    }

    fn bind_with_minimum(
        model: &super::PreparedOlsModel,
        sample: &'a crate::data::PreparedNumericSample,
        minimum: usize,
    ) -> Result<Option<Self>> {
        if sample.n_active() < 4096
            || !(minimum..=MAX_BOUND_NUMERIC_COLUMNS).contains(&model.columns.len())
            || model.columns.iter().any(|c| {
                !matches!(
                    c.kind,
                    PreparedOlsColumnKind::Continuous { .. } | PreparedOlsColumnKind::Intercept
                )
            })
        {
            return Ok(None);
        }
        let mut columns = [None; MAX_BOUND_NUMERIC_COLUMNS];
        for (destination, column) in columns.iter_mut().zip(&model.columns) {
            if let PreparedOlsColumnKind::Continuous {
                prepared_source_index,
                ..
            } = column.kind
            {
                let source = sample
                    .regressors
                    .get(prepared_source_index)
                    .filter(|v| v.len() == sample.n_active())
                    .ok_or_else(|| invalid_argument("numeric OLS column is not aligned"))?;
                *destination = Some(source.as_slice());
            }
        }
        Ok(Some(Self {
            columns,
            width: model.columns.len(),
            rows: sample.n_active(),
        }))
    }

    pub(crate) fn matches_rhs(&self, response: &[f64], data: &[f64]) -> bool {
        let width = self.width + 1;
        response.len() == self.rows && self.rows.checked_mul(width) == Some(data.len())
            && data.par_chunks(NUMERIC_TILE_ROWS * width).enumerate().all(|(tile, input)| {
                let start = tile * NUMERIC_TILE_ROWS;
                for (offset, row) in input.chunks_exact(width).enumerate() {
                    if row[0].to_bits() != response[start + offset].to_bits() { return false; }
                }
                for (column, source) in self.columns[..self.width].iter().enumerate() {
                    for (offset, row) in input.chunks_exact(width).enumerate() {
                        let expected = source.map_or(1.0, |values| values[start + offset]);
                        if row[column + 1].to_bits() != expected.to_bits() { return false; }
                    }
                }
                true
            })
    }

    pub(crate) fn fill_reserved_rhs(&self, response: &[f64], data: &mut Vec<f64>) -> Result<()> {
        let width = self.width + 1;
        if response.len() != self.rows {
            return Err(invalid_argument("numeric OLS RHS response is not aligned"));
        }
        let elements = self
            .rows
            .checked_mul(width)
            .ok_or_else(|| allocation("numeric OLS RHS dimensions overflow"))?;
        if data.capacity() < elements {
            return Err(allocation("numeric OLS RHS reservation is insufficient"));
        }
        // The caller already reserved the complete matrix fallibly. Indexed
        // collection reuses that capacity and initializes disjoint slices in
        // parallel, without uninitialized memory in the safe core or a second
        // observation-scale allocation.
        (0..elements)
            .into_par_iter()
            .map(|_| 0.0)
            .collect_into_vec(data);
        data.par_chunks_mut(NUMERIC_TILE_ROWS * width)
            .enumerate()
            .for_each(|(tile, output)| {
                let start = tile * NUMERIC_TILE_ROWS;
                let rows = output.len() / width;
                for (destination, &value) in output
                    .chunks_exact_mut(width)
                    .zip(&response[start..start + rows])
                {
                    destination[0] = value;
                }
                for (column, source) in self.columns[..self.width].iter().enumerate() {
                    if let Some(source) = source {
                        for (destination, &value) in output
                            .chunks_exact_mut(width)
                            .zip(&source[start..start + rows])
                        {
                            destination[column + 1] = value;
                        }
                    } else {
                        for destination in output.chunks_exact_mut(width) {
                            destination[column + 1] = 1.0;
                        }
                    }
                }
            });
        Ok(())
    }

    /// Add selected raw columns in the caller's coefficient order. Keeping
    /// the original order preserves each row's FMA result after rank selection.
    pub(crate) fn add_selected_columns(
        &self,
        selected: &[(usize, f64)],
        fitted: &mut [f64],
    ) -> Result<()> {
        if fitted.len() != self.rows
            || selected
                .iter()
                .any(|&(column, value)| column >= self.width || !value.is_finite())
        {
            return Err(invalid_argument(
                "numeric predictor columns are not aligned and finite",
            ));
        }
        fitted
            .par_chunks_mut(NUMERIC_TILE_ROWS)
            .enumerate()
            .try_for_each(|(tile, output)| -> Result<()> {
                let start = tile * NUMERIC_TILE_ROWS;
                let rows = output.len();
                for &(column, coefficient) in selected {
                    if let Some(source) = self.columns[column] {
                        for (target, &value) in output.iter_mut().zip(&source[start..start + rows])
                        {
                            *target = coefficient.mul_add(value, *target);
                        }
                    } else {
                        for target in output.iter_mut() {
                            *target = coefficient.mul_add(1.0, *target);
                        }
                    }
                }
                if output.iter().any(|value| !value.is_finite()) {
                    return Err(invalid_data("numeric finite predictor is not finite"));
                }
                Ok(())
            })
    }

    pub(crate) fn subtract_fitted(
        &self,
        coefficients: &[Option<f64>],
        residuals: &mut [f64],
    ) -> Result<()> {
        if coefficients.len() != self.width
            || residuals.len() != self.rows
            || coefficients.iter().flatten().any(|v| !v.is_finite())
        {
            return Err(invalid_argument(
                "numeric OLS residual update is not aligned and finite",
            ));
        }
        residuals
            .par_chunks_mut(NUMERIC_TILE_ROWS)
            .enumerate()
            .try_for_each(|(tile, output)| -> Result<()> {
                let start = tile * NUMERIC_TILE_ROWS;
                let rows = output.len();
                for (source, coefficient) in self.columns[..self.width].iter().zip(coefficients) {
                    let Some(coefficient) = coefficient else {
                        continue;
                    };
                    if let Some(source) = source {
                        for (residual, &value) in
                            output.iter_mut().zip(&source[start..start + rows])
                        {
                            *residual = (-coefficient).mul_add(value, *residual);
                        }
                    } else {
                        for residual in output.iter_mut() {
                            *residual = (-coefficient).mul_add(1.0, *residual);
                        }
                    }
                }
                if output.iter().any(|v| !v.is_finite()) {
                    return Err(invalid_data("numeric OLS raw residual is not finite"));
                }
                Ok(())
            })
    }

    /// Preserve the scalar residual's coefficient/FMA order without repeating
    /// model-recipe dispatch in both passes of the stable weighted mean.
    pub(crate) fn residual_at(
        &self,
        coefficients: &[Option<f64>],
        row: usize,
        response: f64,
    ) -> Result<f64> {
        if coefficients.len() != self.width || row >= self.rows {
            return Err(invalid_argument("numeric OLS residual mean is not aligned"));
        }
        let mut value = response;
        for (source, coefficient) in self.columns[..self.width].iter().zip(coefficients) {
            if let Some(coefficient) = coefficient {
                value = (-coefficient).mul_add(source.map_or(1.0, |values| values[row]), value);
            }
        }
        if value.is_finite() {
            Ok(value)
        } else {
            Err(invalid_data("prepared OLS raw regression residual is not finite"))
        }
    }
}

fn rhs_dimensions(nrows: usize, model_columns: usize) -> Result<(usize, usize, usize)> {
    let ncols = model_columns
        .checked_add(1)
        .ok_or_else(|| allocation("prepared OLS RHS column count overflows"))?;
    let elements = nrows
        .checked_mul(ncols)
        .ok_or_else(|| allocation("prepared OLS RHS dimensions overflow usize"))?;
    Ok((nrows, ncols, elements))
}

fn validate_prepared_rhs_inputs(prepared: &PreparedOlsData, response: &[f64]) -> Result<()> {
    let sample = &prepared.sample;
    let nrows = sample.n_active();
    if sample.n_candidate > sample.n_original
        || sample.drop_reasons.len() != sample.n_original
        || sample.outcome.len() != nrows
        || response.len() != nrows
        || sample.regressors.iter().any(|source| source.len() != nrows)
        || !matches!(sample.adjustment, PreparedAdjustment::None)
    {
        return Err(invalid_argument(
            "prepared OLS numeric sample is not aligned on one observation axis",
        ));
    }
    if response.iter().any(|value| !value.is_finite()) || !numeric_sources_are_finite(sample) {
        return Err(invalid_data(
            "prepared OLS outcome and numeric sources must be finite",
        ));
    }
    validate_row_map(prepared)?;
    validate_weights(prepared)?;
    validate_categorical_sources(prepared)?;
    validate_coefficient_axis(prepared)?;
    for column in &prepared.model.columns {
        match &column.kind {
            PreparedOlsColumnKind::Intercept => {}
            PreparedOlsColumnKind::Continuous {
                source,
                prepared_source_index,
            } => validate_continuous_recipe(prepared, *source, *prepared_source_index)?,
            PreparedOlsColumnKind::Categorical {
                source,
                prepared_source_index,
                categorical_source_index,
                level_bits,
                level_code,
            } => validate_categorical_recipe(
                prepared,
                *source,
                *prepared_source_index,
                *categorical_source_index,
                *level_bits,
                *level_code,
            )?,
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                if components.len() < 2 {
                    return Err(invalid_argument(
                        "prepared categorical interaction has fewer than two components",
                    ));
                }
                for component in components {
                    let categorical = prepared
                        .categorical_sources
                        .get(component.categorical_source_index)
                        .ok_or_else(|| {
                            invalid_argument(
                                "prepared categorical interaction source is out of range",
                            )
                        })?;
                    validate_categorical_recipe(
                        prepared,
                        component.source,
                        categorical.prepared_source_index,
                        component.categorical_source_index,
                        component.level_bits,
                        component.level_code,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_row_map(prepared: &PreparedOlsData) -> Result<()> {
    let sample = &prepared.sample;
    let mut previous = None;
    for original_row in sample.retained_rows().iter().copied() {
        if original_row >= sample.n_original
            || sample.drop_reasons[original_row] != crate::data::DropReason::Retained
            || previous.is_some_and(|prior| prior >= original_row)
        {
            return Err(invalid_argument(
                "prepared OLS retained-row map is inconsistent",
            ));
        }
        previous = Some(original_row);
    }
    if sample
        .drop_reasons
        .iter()
        .filter(|reason| **reason == crate::data::DropReason::Retained)
        .count()
        != sample.n_active()
    {
        return Err(invalid_argument(
            "prepared OLS retained-row count is inconsistent",
        ));
    }
    Ok(())
}

fn validate_weights(prepared: &PreparedOlsData) -> Result<()> {
    let nrows = prepared.sample.n_active();
    let PreparedWeights::Provided { kind, values } = &prepared.sample.weights else {
        return Ok(());
    };
    if *kind == WeightKind::Importance {
        return Err(invalid_argument(
            "prepared OLS importance weights are unsupported",
        ));
    }
    if values.len() != nrows {
        return Err(invalid_argument(
            "prepared OLS weights are not aligned with the observation axis",
        ));
    }
    for value in values {
        if !value.is_finite() || *value <= 0.0 {
            return Err(invalid_data(
                "prepared OLS weights must be finite and positive",
            ));
        }
        if *kind == WeightKind::Frequency && (value.fract() != 0.0 || *value > f64::from(u32::MAX))
        {
            return Err(invalid_data(
                "prepared OLS frequency weights must be positive u32 integers",
            ));
        }
    }
    Ok(())
}

fn validate_categorical_sources(prepared: &PreparedOlsData) -> Result<()> {
    let nrows = prepared.sample.n_active();
    for categorical in &prepared.categorical_sources {
        if categorical.prepared_source_index != source_index(categorical.source)? {
            return Err(invalid_argument(
                "prepared categorical source identity is inconsistent",
            ));
        }
        let raw = prepared
            .sample
            .regressors
            .get(categorical.prepared_source_index)
            .ok_or_else(|| invalid_argument("prepared categorical source is out of range"))?;
        if raw.len() != nrows || categorical.encoded.codes.len() != nrows {
            return Err(invalid_argument(
                "prepared categorical source is not aligned with the observation axis",
            ));
        }
        if categorical.encoded.levels.len() > u32::MAX as usize {
            return Err(invalid_argument(
                "prepared categorical level count exceeds u32",
            ));
        }
        if categorical.encoded.levels.iter().any(|level| {
            !level.is_finite() || (*level == 0.0 && level.to_bits() != 0.0_f64.to_bits())
        }) || categorical
            .encoded
            .levels
            .windows(2)
            .any(|levels| levels[0].total_cmp(&levels[1]).is_ge())
        {
            return Err(invalid_argument(
                "prepared categorical levels are not finite, canonical, and strictly ordered",
            ));
        }
        for (row, code) in categorical.encoded.codes.iter().copied().enumerate() {
            let level = categorical
                .encoded
                .levels
                .get(code as usize)
                .ok_or_else(|| invalid_argument("prepared categorical code is out of range"))?;
            if level.to_bits() != canonical_factor_value(raw[row]).to_bits() {
                return Err(invalid_argument(
                    "prepared categorical codes do not represent their numeric source",
                ));
            }
        }
    }
    Ok(())
}

fn validate_coefficient_axis(prepared: &PreparedOlsData) -> Result<()> {
    let mut next_generated = 0_usize;
    let mut previous_slot = None;
    for coefficient in &prepared.model.coefficients {
        if previous_slot.is_some_and(|slot| slot >= coefficient.coefficient_slot.get()) {
            return Err(invalid_argument(
                "prepared OLS coefficient slots are not strictly ordered",
            ));
        }
        previous_slot = Some(coefficient.coefficient_slot.get());
        match coefficient.status {
            PreparedOlsCoefficientStatus::Active => {
                if coefficient.generated_column_index != Some(next_generated)
                    || prepared
                        .model
                        .columns
                        .get(next_generated)
                        .is_none_or(|column| {
                            column.coefficient_slot != coefficient.coefficient_slot
                        })
                {
                    return Err(invalid_argument(
                        "prepared OLS active coefficient mapping is inconsistent",
                    ));
                }
                next_generated = next_generated
                    .checked_add(1)
                    .ok_or_else(|| allocation("prepared OLS generated-column count overflows"))?;
            }
            PreparedOlsCoefficientStatus::SyntaxOmitted => {
                if coefficient.generated_column_index.is_some() {
                    return Err(invalid_argument(
                        "syntax-omitted coefficient unexpectedly has a generated column",
                    ));
                }
            }
        }
    }
    if next_generated != prepared.model.columns.len() {
        return Err(invalid_argument(
            "prepared OLS generated columns are not covered exactly once",
        ));
    }
    Ok(())
}

fn validate_continuous_recipe(
    prepared: &PreparedOlsData,
    source: SourceId,
    prepared_source_index: usize,
) -> Result<()> {
    if prepared_source_index != source_index(source)?
        || prepared
            .sample
            .regressors
            .get(prepared_source_index)
            .is_none()
    {
        return Err(invalid_argument(
            "prepared continuous source identity is inconsistent",
        ));
    }
    Ok(())
}

fn validate_categorical_recipe(
    prepared: &PreparedOlsData,
    source: SourceId,
    prepared_source_index: usize,
    categorical_source_index: usize,
    level_bits: u64,
    level_code: Option<u32>,
) -> Result<()> {
    validate_continuous_recipe(prepared, source, prepared_source_index)?;
    let categorical = prepared
        .categorical_sources
        .get(categorical_source_index)
        .ok_or_else(|| invalid_argument("prepared categorical recipe source is out of range"))?;
    if categorical.source != source || categorical.prepared_source_index != prepared_source_index {
        return Err(invalid_argument(
            "prepared categorical recipe source identity is inconsistent",
        ));
    }
    let requested = canonical_factor_value(f64::from_bits(level_bits));
    if !requested.is_finite() || requested < 0.0 || requested.fract() != 0.0 {
        return Err(invalid_argument(
            "prepared categorical recipe level is invalid",
        ));
    }
    let resolved = categorical
        .encoded
        .levels
        .binary_search_by(|candidate| candidate.total_cmp(&requested))
        .ok()
        .and_then(|index| u32::try_from(index).ok());
    if resolved != level_code {
        return Err(invalid_argument(
            "prepared categorical recipe level code is inconsistent",
        ));
    }
    Ok(())
}

fn source_index(source: SourceId) -> Result<usize> {
    usize::try_from(source.get() - 1)
        .map_err(|_| invalid_argument("prepared source id does not fit usize"))
}

fn canonical_factor_value(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

#[cfg(test)]
mod tests {

    #[test]
    fn selected_predictor_tiles_preserve_scalar_fma_order_and_reject_invalid_inputs() {
        use crate::options::ThreadConfig;
        use crate::parallel::ThreadTeam;
        for width in [8_usize, 64] {
            let n = 4099;
            let sources = (0..width - 1)
                .map(|column| {
                    (0..n)
                        .map(|row| {
                            ((row * 17 + column * 3) as f64).sin()
                                * if column % 2 == 0 { 1e100 } else { 1e-100 }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut bindings = [None; MAX_BOUND_NUMERIC_COLUMNS];
            for (target, source) in bindings.iter_mut().zip(&sources) {
                *target = Some(source.as_slice());
            }
            let columns = NumericOlsColumns {
                columns: bindings,
                width,
                rows: n,
            };
            // Deliberately noncanonical order and a repeated identity catch
            // regrouping or sorting that would change per-row accumulation.
            let selected: [(usize, f64); 5] = [
                (width - 1, -0.0),
                (3, 1e100),
                (0, 1e-100),
                (3, -1e100),
                (1, 0.7),
            ];
            let mut expected = vec![0.0; n];
            for &(column, coefficient) in &selected {
                for row in 0..n {
                    let value = columns.columns[column].map_or(1.0, |source| source[row]);
                    expected[row] = coefficient.mul_add(value, expected[row]);
                }
            }
            for workers in [1, 4, 16] {
                let mut actual = vec![0.0; n];
                ThreadTeam::new(ThreadConfig::from_requested(workers, true).unwrap())
                    .install(|| columns.add_selected_columns(&selected, &mut actual))
                    .unwrap()
                    .unwrap();
                assert!(actual
                    .iter()
                    .zip(&expected)
                    .all(|(a, b)| a.to_bits() == b.to_bits()));
            }
            let mut unchanged = vec![7.0; n];
            for invalid in [[(width, 1.0)], [(0, f64::NAN)], [(0, f64::INFINITY)]] {
                assert!(columns
                    .add_selected_columns(&invalid, &mut unchanged)
                    .is_err());
                assert!(unchanged.iter().all(|x| *x == 7.0));
            }
            assert!(columns
                .add_selected_columns(&[], &mut unchanged[..n - 1])
                .is_err());
            columns.add_selected_columns(&[], &mut unchanged).unwrap();
            assert!(unchanged.iter().all(|x| *x == 7.0));
            assert!(columns
                .add_selected_columns(&[(0, f64::MAX)], &mut unchanged)
                .is_err());
        }
    }
    use super::*;
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind,
        ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry,
        TermComponent, TermId, TermOrder, TimeSeriesState,
    };
    use crate::ols::{
        prepare_ols_data, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
        OlsRequestV1,
    };

    #[derive(Default)]
    struct InjectedReservation {
        fail: bool,
        skip: bool,
        calls: usize,
    }

    impl RhsReservation for InjectedReservation {
        fn reserve_f64(&mut self, target: &mut Vec<f64>, elements: usize) -> Result<()> {
            self.calls += 1;
            if self.fail {
                return Err(allocation("injected prepared RHS reservation failure"));
            }
            if !self.skip {
                target
                    .try_reserve_exact(elements)
                    .map_err(|_| allocation("test prepared RHS reservation failure"))?;
            }
            Ok(())
        }
    }

    fn loaded(values: Vec<f64>) -> LoadedNumericColumn {
        LoadedNumericColumn {
            missing: vec![false; values.len()],
            values,
        }
    }

    fn source(id: u32, name: &str) -> SourceEntry {
        SourceEntry {
            id: SourceId::new(id).expect("source"),
            canonical_name: name.to_owned(),
        }
    }

    fn continuous_term(id: u32, source: SourceId) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(id).expect("term"),
            order: TermOrder::new(id).expect("order"),
            coefficient_slot: CoefficientSlot::new(id).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: format!("c.x{id}"),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source,
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.0,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".to_owned(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        }
    }

    fn categorical_term(id: u32, source: SourceId, level: f64, omitted: bool) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(id).expect("term"),
            order: TermOrder::new(id).expect("order"),
            coefficient_slot: CoefficientSlot::new(id).expect("slot"),
            raw_kind: RawTermKind::Factor,
            whole_column_omitted: omitted,
            canonical_token: format!("{level}.g"),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source,
                kind: ComponentKind::Categorical,
                level_state: LevelState::Present,
                level_value: level,
                base: if omitted {
                    MetadataFlag::True
                } else {
                    MetadataFlag::False
                },
                omitted: if omitted {
                    MetadataFlag::True
                } else {
                    MetadataFlag::False
                },
                raw_operator: level.to_string(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        }
    }

    fn categorical_interaction_term(
        id: u32,
        first: (SourceId, f64),
        second: (SourceId, f64),
    ) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(id).expect("term"),
            order: TermOrder::new(id).expect("order"),
            coefficient_slot: CoefficientSlot::new(id).expect("slot"),
            raw_kind: RawTermKind::Interaction,
            whole_column_omitted: false,
            canonical_token: format!("{}.g#{}.h", first.1, second.1),
            components: [first, second]
                .into_iter()
                .enumerate()
                .map(|(index, (source, level))| TermComponent {
                    order: ComponentOrder::new(u32::try_from(index + 1).expect("small order"))
                        .expect("component"),
                    source,
                    kind: ComponentKind::Categorical,
                    level_state: LevelState::Present,
                    level_value: level,
                    base: MetadataFlag::False,
                    omitted: MetadataFlag::False,
                    raw_operator: level.to_string(),
                    time_series_state: TimeSeriesState::None,
                    raw_time_series_operator: String::new(),
                })
                .collect(),
        }
    }

    fn request(with_fe: bool, include_intercept: bool) -> OlsRequestV1 {
        let x = source(1, "x");
        let g = source(2, "g");
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::NumericSource(x.id),
            },
        ];
        let mut sources = vec![x.clone()];
        if with_fe {
            sources.push(g.clone());
            bindings.push(OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::NumericSource(g.id),
            });
        }
        OlsRequestV1 {
            model: CompiledTermGraph {
                sources,
                terms: vec![continuous_term(1, x.id)],
            },
            absorbed_terms: if with_fe {
                vec![AbsorbedTerm {
                    id: AbsorbTermId::new(1).expect("effect"),
                    intercept: true,
                    categorical_sources: vec![g.id],
                    slope_sources: Vec::new(),
                    label: "g".to_owned(),
                    saved_name: String::new(),
                }]
            } else {
                Vec::new()
            },
            bindings,
            include_intercept,
            keep_singletons: true,
        }
    }

    fn prepared(with_fe: bool, include_intercept: bool) -> PreparedOlsData {
        let request = request(with_fe, include_intercept);
        prepare_ols_data(
            &request,
            &LoadedOlsInput {
                candidate: vec![true; 4],
                outcome: loaded(vec![1.0, 3.0, 4.0, 8.0]),
                numeric_sources: if with_fe {
                    vec![
                        (
                            SourceId::new(1).expect("x"),
                            loaded(vec![-0.0, 2.0, 4.0, 6.0]),
                        ),
                        (
                            SourceId::new(2).expect("g"),
                            loaded(vec![1.0, 1.0, 2.0, 2.0]),
                        ),
                    ]
                } else {
                    vec![(
                        SourceId::new(1).expect("x"),
                        loaded(vec![-0.0, 2.0, 4.0, 6.0]),
                    )]
                },
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("prepared")
    }

    #[test]
    fn loaded_rhs_reuse_preserves_bits_and_rejects_stale_rows() {
        use crate::{ThreadConfig, ThreadTeam};
        for (width, n) in [(3_usize,16383), (3,16384), (5,16384), (7,16384), (8,16384),
            (9,4099), (12,4099), (52,4099), (64,4099)] {
            let mut request = request(false, true);
            request.model.sources.clear();
            request.model.terms.clear();
            request.bindings.truncate(2);
            let mut input = LoadedOlsInput { candidate: vec![true; n],
                outcome: loaded((0..n).map(|r| (r as f64).cos()).collect()),
                numeric_sources: Vec::new(), analytic_weight: None,
                frequency_weight: None, probability_weight: None };
            for j in 0..width - 2 {
                let source = source(j as u32 + 1, &format!("x{j}"));
                request.model.terms.push(continuous_term(j as u32 + 1, source.id));
                request.bindings.push(OlsInputBinding { stata_var_position: j as u32 + 3,
                    role: OlsInputRole::NumericSource(source.id) });
                input.numeric_sources.push((source.id, loaded((0..n).map(|r|
                    if r == 0 { -0.0 } else { ((r * 17 + j) as f64).sin() * if j % 2 == 0 { 1e90 } else { 1e-90 } }).collect())));
                request.model.sources.push(source);
            }
            let mut prepared = prepare_ols_data(&request, &input).unwrap();
            for threads in [1, 4, 8, 16] {
                ThreadTeam::new(ThreadConfig::from_requested(threads, false).unwrap()).install(|| {
                    let reference = build_prepared_ols_rhs(&prepared).unwrap().into_values();
                    let rows = reference.as_slice().to_vec();
                    let pointer = rows.as_ptr();
                    let (rhs, transferred) = build_prepared_ols_rhs_with_loaded_rows_and_reuse(&prepared, &prepared.sample.outcome, Some(rows)).unwrap();
                    let reused = rhs.into_values();
                    assert_eq!(transferred, width >= 9 || n >= 16384 && threads >= 8);
                    if transferred { assert_eq!(pointer, reused.as_slice().as_ptr()); }
                    assert!(reference.as_slice().iter().zip(reused.as_slice()).all(|(a,b)| a.to_bits() == b.to_bits()));
                    for cell in [0, 1, width - 1, n * width / 2, n * width - 1] {
                        let mut stale = reference.as_slice().to_vec();
                        stale[cell] = f64::NAN;
                        let result = build_prepared_ols_rhs_with_loaded_rows(&prepared, &prepared.sample.outcome, Some(stale)).unwrap().into_values();
                        assert!(reference.as_slice().iter().zip(result.as_slice()).all(|(a,b)| a.to_bits() == b.to_bits()));
                    }
                    for length in [0, n * width - 1, n * width + 1] {
                        let result = build_prepared_ols_rhs_with_loaded_rows(&prepared, &prepared.sample.outcome, Some(vec![0.0; length])).unwrap().into_values();
                        assert_eq!(reference, result);
                    }
                }).unwrap();
            }
            prepared.sample.regressors[0][0] = f64::NAN;
            assert_eq!(build_prepared_ols_rhs(&prepared).unwrap_err(),
                build_prepared_ols_rhs_with_loaded_rows(&prepared, &prepared.sample.outcome, Some(Vec::new())).unwrap_err());
        }
    }

    #[test]
    fn direct_rhs_preserves_y_generated_column_order_and_bits() {
        let prepared = prepared(false, true);
        let rhs = build_prepared_ols_rhs(&prepared).expect("RHS");
        assert_eq!(rhs.values().nrows(), 4);
        assert_eq!(rhs.values().ncols(), 3);
        assert_eq!(rhs.values().as_slice()[0..3], [1.0, -0.0, 1.0]);
        assert_eq!(rhs.values().as_slice()[1].to_bits(), (-0.0_f64).to_bits());
        assert_eq!(rhs.values().as_slice()[9..12], [8.0, 6.0, 1.0]);
    }

    #[test]
    fn explicit_response_replaces_only_column_zero_with_one_allocation() {
        let prepared = prepared(false, true);
        let response = [8.0, 4.0, -0.0, 2.0];
        let ordinary = build_prepared_ols_rhs(&prepared).expect("ordinary RHS");
        let mut reservation = InjectedReservation::default();
        let explicit =
            build_prepared_ols_rhs_with_reservation(&prepared, &response, &mut reservation)
                .expect("explicit-response RHS");
        assert_eq!(reservation.calls, 1);
        assert_eq!(ordinary.values().nrows(), explicit.values().nrows());
        assert_eq!(ordinary.values().ncols(), explicit.values().ncols());
        for (row, expected_response) in response
            .iter()
            .copied()
            .enumerate()
            .take(explicit.values().nrows())
        {
            assert_eq!(
                explicit.values()[(row, 0)].to_bits(),
                expected_response.to_bits()
            );
            for column in 1..explicit.values().ncols() {
                assert_eq!(
                    explicit.values()[(row, column)].to_bits(),
                    ordinary.values()[(row, column)].to_bits()
                );
            }
        }
    }

    #[test]
    fn factor_omission_interaction_and_vanished_level_have_exact_rhs_semantics() {
        let g = source(1, "g");
        let h = source(2, "h");
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![g.clone(), h.clone()],
                terms: vec![
                    categorical_term(1, g.id, 1.0, true),
                    categorical_term(2, g.id, 2.0, false),
                    categorical_interaction_term(3, (g.id, 2.0), (h.id, 1.0)),
                    categorical_term(4, g.id, 3.0, false),
                ],
            },
            absorbed_terms: Vec::new(),
            bindings: vec![
                OlsInputBinding {
                    stata_var_position: 1,
                    role: OlsInputRole::Candidate,
                },
                OlsInputBinding {
                    stata_var_position: 2,
                    role: OlsInputRole::Outcome,
                },
                OlsInputBinding {
                    stata_var_position: 3,
                    role: OlsInputRole::NumericSource(g.id),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(h.id),
                },
            ],
            include_intercept: true,
            keep_singletons: false,
        };
        let mut prepared = prepare_ols_data(
            &request,
            &LoadedOlsInput {
                candidate: vec![true; 4],
                outcome: loaded(vec![10.0, 11.0, 12.0, 13.0]),
                numeric_sources: vec![
                    (g.id, loaded(vec![1.0, 2.0, 2.0, 1.0])),
                    (h.id, loaded(vec![1.0, 1.0, 2.0, 2.0])),
                ],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("factor preparation");
        assert_eq!(prepared.model.coefficients.len(), 5);
        assert_eq!(prepared.model.columns.len(), 4);
        let rhs = build_prepared_ols_rhs(&prepared).expect("factor RHS");
        assert_eq!((rhs.values().nrows(), rhs.values().ncols()), (4, 5));
        assert_eq!(
            rhs.values().as_slice(),
            &[
                10.0, 0.0, 0.0, 0.0, 1.0, 11.0, 1.0, 1.0, 0.0, 1.0, 12.0, 1.0, 0.0, 0.0, 1.0, 13.0,
                0.0, 0.0, 0.0, 1.0,
            ]
        );

        let PreparedOlsColumnKind::Categorical { level_code, .. } =
            &mut prepared.model.columns[0].kind
        else {
            panic!("categorical recipe")
        };
        let valid_level_code = *level_code;
        *level_code = Some(u32::MAX);
        assert_eq!(
            build_prepared_ols_rhs(&prepared)
                .expect_err("corrupt recipe code")
                .code,
            ErrorCode::InvalidArgument
        );
        let PreparedOlsColumnKind::Categorical { level_code, .. } =
            &mut prepared.model.columns[0].kind
        else {
            panic!("categorical recipe")
        };
        *level_code = valid_level_code;
        prepared.categorical_sources[0].encoded.codes[0] = u32::MAX;
        assert_eq!(
            build_prepared_ols_rhs(&prepared)
                .expect_err("corrupt categorical source code")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn generated_predictor_tiles_preserve_raw_level_oracle_and_fma_order() {
        use crate::options::ThreadConfig;
        use crate::parallel::ThreadTeam;
        use crate::prepared_fit_view::PreparedOlsFitView;
        for n in [4095, 4096, 4099] {
            let x = source(1, "x");
            let g = source(2, "g");
            let h = source(3, "h");
            let mut terms = vec![
                continuous_term(1, x.id),
                categorical_term(2, g.id, 1.0, true),
                categorical_interaction_term(3, (g.id, 2.0), (h.id, 2.0)),
                categorical_interaction_term(4, (g.id, 99.0), (h.id, 2.0)),
            ];
            for level in 2..=67 {
                terms.push(categorical_term(level + 3, g.id, f64::from(level), false));
            }
            terms.push(categorical_term(71, h.id, 1.0, true));
            let mut request = request(false, true);
            request.model = CompiledTermGraph {
                sources: vec![x.clone(), g.clone(), h.clone()],
                terms,
            };
            request.bindings.extend([
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(g.id),
                },
                OlsInputBinding {
                    stata_var_position: 5,
                    role: OlsInputRole::NumericSource(h.id),
                },
            ]);
            request.absorbed_terms = vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![g.id],
                slope_sources: Vec::new(),
                label: "g".into(),
                saved_name: String::new(),
            }];
            let xv: Vec<_> = (0..n)
                .map(|r| if r % 2 == 0 { -0.0 } else { r as f64 / 7.0 })
                .collect();
            let gv: Vec<_> = (0..n).map(|r| (1 + r % 7) as f64).collect();
            let hv: Vec<_> = (0..n).map(|r| (1 + (r / 7) % 3) as f64).collect();
            let input = LoadedOlsInput {
                candidate: vec![true; n],
                outcome: loaded((0..n).map(|r| r as f64).collect()),
                numeric_sources: vec![
                    (x.id, loaded(xv.clone())),
                    (g.id, loaded(gv.clone())),
                    (h.id, loaded(hv.clone())),
                ],
                analytic_weight: Some(loaded((0..n).map(|r| 1.0 + (r % 5) as f64).collect())),
                frequency_weight: None,
                probability_weight: None,
            };
            request.bindings.push(OlsInputBinding {
                stata_var_position: 6,
                role: OlsInputRole::AnalyticWeight,
            });
            let simple = prepare_ols_data(&request, &input).unwrap();
            let complex =
                crate::prepared_categorical_data::prepare_categorical_ols_data(&request, &input)
                    .unwrap();
            assert!(complex
                .categorical_sources()
                .iter()
                .any(|s| s.is_topology_backed()));
            let views: [&dyn PreparedOlsFitView; 2] = [&simple, &complex];
            for prepared in views {
                let width = prepared.fit_model().columns.len();
                let raw = |source: SourceId, row: usize| match source.get() {
                    1 => xv[row],
                    2 => gv[row],
                    3 => hv[row],
                    _ => panic!("unknown source"),
                };
                // Raw level values form the oracle independently of either
                // layout's compact codes or final-sample recipe rebasing.
                let expected: Vec<_> = (0..n)
                    .flat_map(|row| {
                        let mut values = vec![row as f64];
                        values.extend(prepared.fit_model().columns.iter().map(|column| {
                            match &column.kind {
                                PreparedOlsColumnKind::Intercept => 1.0,
                                PreparedOlsColumnKind::Continuous { source, .. } => raw(*source, row),
                                PreparedOlsColumnKind::Categorical {
                                    source, level_bits, ..
                                } => f64::from(raw(*source, row) == f64::from_bits(*level_bits)),
                                PreparedOlsColumnKind::CategoricalInteraction { components } => {
                                    f64::from(
                                        components.iter().all(|c| {
                                            raw(c.source, row) == f64::from_bits(c.level_bits)
                                        }),
                                    )
                                }
                            }
                        }));
                        values
                    })
                    .collect();
                assert_eq!(expected.len(), n * (width + 1));
                let selection = [
                    (width - 1, -0.0),
                    (1, 1e100),
                    (0, 1e-100),
                    (1, -1e100),
                    (2, 0.7),
                ];
                let coefficients: Vec<_> = selection
                    .iter()
                    .enumerate()
                    .map(
                        |(slot, (column, value))| crate::ols::PreparedOlsFittedCoefficient {
                            coefficient_slot: CoefficientSlot::new((slot + 1) as u32).unwrap(),
                            generated_column_index: Some(*column),
                            status: crate::ols::PreparedOlsFitStatus::Kept,
                            value: Some(*value),
                        },
                    )
                    .collect();
                let mut wanted = vec![-0.0_f64; n];
                for &(column, coefficient) in &selection {
                    for row in 0..n {
                        wanted[row] =
                            coefficient.mul_add(expected[row * (width + 1) + column + 1], wanted[row]);
                    }
                }
                for workers in [1, 4, 16] {
                    let team = ThreadTeam::new(ThreadConfig::from_requested(workers, true).unwrap());
                    let mut actual = vec![-0.0_f64; n];
                    let used = team
                        .install(|| {
                            crate::prepared_fit_view::add_generated_predictor(
                                prepared,
                                &coefficients,
                                &mut actual,
                            )
                        })
                        .unwrap()
                        .unwrap();
                    assert_eq!(used, n >= 4096);
                    if used {
                        assert!(actual
                            .iter()
                            .zip(&wanted)
                            .all(|(a, b)| a.to_bits() == b.to_bits()));
                    } else {
                        assert!(actual.iter().all(|v| v.to_bits() == (-0.0_f64).to_bits()));
                    }
                }
                if n >= 4096 {
                    let mut invalid = coefficients.clone();
                    for (column, value) in [
                        (None, Some(1.0)),
                        (Some(width), Some(1.0)),
                        (Some(0), None),
                        (Some(0), Some(f64::NAN)),
                        (Some(0), Some(f64::INFINITY)),
                    ] {
                        invalid[0].generated_column_index = column;
                        invalid[0].value = value;
                        let mut untouched = vec![7.0; n];
                        assert!(crate::prepared_fit_view::add_generated_predictor(
                            prepared,
                            &invalid,
                            &mut untouched
                        )
                        .is_err());
                        assert_eq!(untouched, vec![7.0; n]);
                    }
                    let mut short = vec![7.0; n - 1];
                    assert!(crate::prepared_fit_view::add_generated_predictor(
                        prepared,
                        &coefficients,
                        &mut short
                    )
                    .is_err());
                    let mut nonfinite = vec![f64::MAX; n];
                    let overflow = vec![crate::ols::PreparedOlsFittedCoefficient {
                        coefficient_slot: CoefficientSlot::new(1).unwrap(),
                        generated_column_index: Some(width - 1),
                        status: crate::ols::PreparedOlsFitStatus::Kept,
                        value: Some(f64::MAX),
                    }];
                    assert_eq!(
                        crate::prepared_fit_view::add_generated_predictor(
                            prepared,
                            &overflow,
                            &mut nonfinite
                        )
                        .unwrap_err()
                        .code,
                        ErrorCode::InvalidData
                    );
                    let omitted = vec![crate::ols::PreparedOlsFittedCoefficient {
                        coefficient_slot: CoefficientSlot::new(1).unwrap(),
                        generated_column_index: None,
                        status: crate::ols::PreparedOlsFitStatus::SyntaxOmitted,
                        value: None,
                    }];
                    let mut unchanged = vec![-0.0_f64; n];
                    assert!(crate::prepared_fit_view::add_generated_predictor(
                        prepared,
                        &omitted,
                        &mut unchanged
                    )
                    .unwrap());
                    assert!(unchanged
                        .iter()
                        .all(|v| v.to_bits() == (-0.0_f64).to_bits()));
                }
            }
        }
    }

    #[test]
    fn empty_rhs_is_valid_but_map_requires_nonempty_rows() {
        let request = request(false, false);
        let prepared = prepare_ols_data(
            &request,
            &LoadedOlsInput {
                candidate: Vec::new(),
                outcome: loaded(Vec::new()),
                numeric_sources: vec![(SourceId::new(1).expect("x"), loaded(Vec::new()))],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .expect("empty prepared");
        let rhs = build_prepared_ols_rhs(&prepared).expect("empty RHS");
        assert_eq!((rhs.values().nrows(), rhs.values().ncols()), (0, 2));
        assert_eq!(
            absorb_prepared_ols_rhs_map(&prepared, MapConfig::default())
                .expect_err("empty MAP")
                .code,
            ErrorCode::InvalidData
        );
    }

    #[test]
    fn no_absorb_projects_virtual_constant_and_fe_path_returns_only_certified_values() {
        let no_fe = prepared(false, true);
        let result =
            absorb_prepared_ols_rhs_map(&no_fe, MapConfig::default()).expect("virtual constant");
        for column in 0..result.residualized().ncols() {
            let sum = (0..result.residualized().nrows())
                .map(|row| result.residualized()[(row, column)])
                .sum::<f64>();
            assert!(sum.abs() < 1e-12);
        }
        assert_eq!(
            result.report().stop_reason,
            crate::fe::SolveStopReason::Certified
        );

        let with_fe = prepared(true, true);
        let result = absorb_prepared_ols_rhs_map(&with_fe, MapConfig::default()).expect("FE MAP");
        assert!(result.report().certified());
        assert!(result
            .residualized()
            .as_slice()
            .chunks_exact(3)
            .all(|row| row[2].abs() < 1e-12));
    }

    #[test]
    fn rhs_allocation_and_dimension_failures_are_fatal() {
        let prepared = prepared(false, true);
        for (fail, skip) in [(true, false), (false, true)] {
            let mut reservation = InjectedReservation {
                fail,
                skip,
                ..InjectedReservation::default()
            };
            let error = build_prepared_ols_rhs_with_reservation(
                &prepared,
                &prepared.sample.outcome,
                &mut reservation,
            )
            .expect_err("reservation failure");
            assert_eq!(error.code, ErrorCode::Allocation);
            assert_eq!(reservation.calls, 1);
        }
        assert_eq!(
            rhs_dimensions(1, usize::MAX)
                .expect_err("column overflow")
                .code,
            ErrorCode::Allocation
        );
        assert_eq!(
            rhs_dimensions(usize::MAX, 1)
                .expect_err("product overflow")
                .code,
            ErrorCode::Allocation
        );
    }

    #[test]
    fn corrupted_recipe_axis_and_weights_fail_before_a_rhs_is_returned() {
        let mut bad_mapping = prepared(false, true);
        bad_mapping.model.coefficients[0].generated_column_index = Some(1);
        assert_eq!(
            build_prepared_ols_rhs(&bad_mapping)
                .expect_err("bad mapping")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut bad_source = prepared(false, true);
        let PreparedOlsColumnKind::Continuous {
            prepared_source_index,
            ..
        } = &mut bad_source.model.columns[0].kind
        else {
            panic!("continuous recipe")
        };
        *prepared_source_index = 99;
        assert_eq!(
            build_prepared_ols_rhs(&bad_source)
                .expect_err("bad source")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut bad_weight = prepared(false, true);
        bad_weight.sample.weights = PreparedWeights::Provided {
            kind: WeightKind::Importance,
            values: vec![1.0; 4],
        };
        assert_eq!(
            build_prepared_ols_rhs(&bad_weight)
                .expect_err("importance weight")
                .code,
            ErrorCode::InvalidArgument
        );
    }
}
