use std::cell::RefCell;

use crate::data::{DenseMatrix, PreparedWeights};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{AbsorbConfig, BorrowedMapWeights, CmgAbsorber, CmgConfig, FeSolverKind};
use crate::options::ThreadConfig;
use crate::prepared_fit_view::PreparedOlsFitView;

use super::prepared_scratch::PreparedOlsScratch;
use super::prepared_unadjusted::{compensated_add, relative_difference, ReferenceWeightView};
use super::{CertifiedPreparedOlsFit, PreparedOlsBreadCoordinateKind, PreparedOlsExecutionMode};

const TRANSPOSE_BUFFER_BYTES: usize = 1 << 20;

pub(super) trait PreparedInferenceReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;
}

pub(super) struct SystemPreparedInferenceReservation;

impl PreparedInferenceReservation for SystemPreparedInferenceReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PreparedOlsScoreDesignCertificate {
    pub left_inverse_relative_infinity_norm: f64,
    pub right_inverse_relative_infinity_norm: f64,
    pub intercept_coordinate_relative_infinity_norm: f64,
    pub passed: bool,
}

/// Ephemeral, kept-coordinate score design. It is deliberately consumed by
/// robust inference and is never retained in the final certified result.
#[derive(Debug)]
pub(super) struct CertifiedPreparedOlsScoreDesign {
    storage: PreparedOlsScoreStorage,
    nrows: usize,
    finite_rank: usize,
    means: Vec<f64>,
    finite_row_buffer: RefCell<Vec<f64>>,
    reported_to_finite: Vec<usize>,
    normalized_intercept_position: Option<usize>,
    pub(super) certificate: PreparedOlsScoreDesignCertificate,
}

#[derive(Debug)]
enum PreparedOlsScoreStorage {
    Dense(DenseMatrix),
    Scratch(PreparedOlsScratchScoreStorage),
}

#[derive(Debug)]
struct PreparedOlsScratchScoreStorage {
    scratch: RefCell<PreparedOlsScratch>,
    cache: RefCell<PreparedOlsScoreRowCache>,
}

#[derive(Debug)]
struct PreparedOlsScoreRowCache {
    start_row: usize,
    rows: usize,
    values: Vec<f64>,
}

impl CertifiedPreparedOlsScoreDesign {
    pub(super) const fn nrows(&self) -> usize {
        self.nrows
    }

    pub(super) const fn finite_rank(&self) -> usize {
        self.finite_rank
    }

    pub(super) fn fill_finite_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        if row >= self.nrows || output.len() != self.finite_rank {
            return Err(invalid_argument(
                "prepared robust finite score row dimensions are inconsistent",
            ));
        }
        match &self.storage {
            PreparedOlsScoreStorage::Dense(within) => {
                for (column, value) in output.iter_mut().enumerate() {
                    *value = within[(row, column)];
                }
            }
            PreparedOlsScoreStorage::Scratch(scratch) => {
                scratch.fill_row(self.nrows, self.finite_rank, row, output)?
            }
        }
        Ok(())
    }

    pub(super) fn fill_reported_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        if output.len() != self.reported_to_finite.len() {
            return Err(invalid_argument(
                "prepared robust reported score row dimensions are inconsistent",
            ));
        }
        let mut finite = self.finite_row_buffer.borrow_mut();
        self.fill_finite_row(row, &mut finite)?;
        for (position, value) in output.iter_mut().enumerate() {
            if self.normalized_intercept_position == Some(position) {
                *value = 1.0;
                continue;
            }
            let finite_position = *self
                .reported_to_finite
                .get(position)
                .filter(|finite| **finite != usize::MAX)
                .ok_or_else(|| invalid_argument("prepared robust score coordinate is absent"))?;
            *value = finite[finite_position]
                + if self.normalized_intercept_position.is_some() {
                    self.means[finite_position]
                } else {
                    0.0
                };
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared robust reported score coordinate is not finite",
                ));
            }
        }
        Ok(())
    }

    pub(super) const fn certificate(&self) -> PreparedOlsScoreDesignCertificate {
        self.certificate
    }
}

impl PreparedOlsScratchScoreStorage {
    fn fill_row(&self, nrows: usize, rank: usize, row: usize, output: &mut [f64]) -> Result<()> {
        let mut cache = self.cache.borrow_mut();
        let cache_end = cache.start_row.saturating_add(cache.rows);
        if row < cache.start_row || row >= cache_end {
            let capacity_rows = cache.values.len() / rank;
            let rows = capacity_rows.min(nrows - row);
            let elements = rows
                .checked_mul(rank)
                .ok_or_else(|| allocation("prepared robust score cache dimensions overflow"))?;
            let offset = row
                .checked_mul(rank)
                .ok_or_else(|| allocation("prepared robust score cache offset overflows"))?;
            self.scratch
                .borrow_mut()
                .read_linear(offset, &mut cache.values[..elements])?;
            cache.start_row = row;
            cache.rows = rows;
        }
        let cache_row = row - cache.start_row;
        let start = cache_row * rank;
        output.copy_from_slice(&cache.values[start..start + rank]);
        Ok(())
    }
}

pub(super) fn build_prepared_ols_score_design<
    P: PreparedOlsFitView + ?Sized,
    R: PreparedInferenceReservation,
>(
    prepared: &P,
    fit: &mut CertifiedPreparedOlsFit,
    weights: ReferenceWeightView<'_>,
    absorb: AbsorbConfig,
    tolerance: f64,
    reservation: &mut R,
) -> Result<CertifiedPreparedOlsScoreDesign> {
    let nrows = prepared.fit_sample().n_active();
    let rank = fit.finite_rank();
    let dimension = fit.bread().dimension();
    if nrows == 0 || !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
        return Err(invalid_argument(
            "prepared robust score design has invalid sample or tolerance",
        ));
    }

    let mut generated_columns = reserved_filled(
        reservation,
        rank,
        0_usize,
        "unable to reserve prepared robust kept score identities",
    )?;
    let mut finite_positions = reserved_filled(
        reservation,
        rank,
        0_usize,
        "unable to reserve prepared robust finite score positions",
    )?;
    let mut reported_to_finite = reserved_filled(
        reservation,
        dimension,
        usize::MAX,
        "unable to reserve prepared robust reported-score coordinate map",
    )?;
    let mut normalized_intercept_position = None;
    let mut finite = 0_usize;
    for (position, coordinate) in fit.bread().coordinates().iter().enumerate() {
        match coordinate.kind {
            PreparedOlsBreadCoordinateKind::FiniteRegressor => {
                if finite >= rank
                    || generated_columns[..finite].contains(&coordinate.generated_column_index)
                {
                    return Err(invalid_argument(
                        "prepared robust finite score coordinate is duplicated or excessive",
                    ));
                }
                generated_columns[finite] = coordinate.generated_column_index;
                finite_positions[finite] = position;
                reported_to_finite[position] = finite;
                finite = finite
                    .checked_add(1)
                    .ok_or_else(|| allocation("prepared robust finite score rank overflows"))?;
            }
            PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept => {
                if normalized_intercept_position.replace(position).is_some() {
                    return Err(invalid_argument(
                        "prepared robust score design has multiple normalized intercepts",
                    ));
                }
            }
        }
    }
    if finite != rank || dimension != rank + usize::from(normalized_intercept_position.is_some()) {
        return Err(invalid_argument(
            "prepared robust score design rank and bread coordinates disagree",
        ));
    }

    let cached = if fit.execution().mode == PreparedOlsExecutionMode::Fast {
        fit.take_fast_score_cache(&generated_columns)
    } else {
        None
    };
    let map_weights = match &prepared.fit_sample().weights {
        PreparedWeights::None => BorrowedMapWeights::Unit,
        PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
    };
    let score_cmg_solver = if absorb.solver == FeSolverKind::Cmg {
        Some(FeSolverKind::Cmg)
    } else if absorb.solver == FeSolverKind::Auto
        && fit.absorption_report().used_solver == FeSolverKind::Cmg
    {
        Some(FeSolverKind::Auto)
    } else {
        None
    };
    let mut cmg = if let (Some(requested_solver), true) = (score_cmg_solver, cached.is_none()) {
        let built = prepared.fit_build_cmg_absorber(
            map_weights,
            CmgConfig {
                map: absorb.map,
                threads: ThreadConfig::from_requested(
                    fit.execution().effective_threads,
                    fit.execution().deterministic,
                )?,
                batch_size: fit.execution().poolsize.max(1),
                retain_coefficients: false,
                requested_solver,
                solver_memory_budget: absorb.solver_memory_budget,
            },
        );
        match built {
            Ok(session) => Some(session),
            Err(_) if requested_solver == FeSolverKind::Auto => None,
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    let (storage, means, scratch_peak_bytes) = match (fit.execution().mode, cached) {
        (PreparedOlsExecutionMode::Fast, Some((within, means))) => {
            (PreparedOlsScoreStorage::Dense(within), means, 0)
        }
        (PreparedOlsExecutionMode::Fast, None) => build_dense_score_storage(
            prepared,
            &generated_columns,
            weights,
            absorb,
            &mut cmg,
            reservation,
        )?,
        (PreparedOlsExecutionMode::Low, _) => build_scratch_score_storage(
            prepared,
            &generated_columns,
            weights,
            absorb,
            fit.execution().poolsize,
            &mut cmg,
            reservation,
        )?,
    };
    fit.record_additional_scratch_peak(scratch_peak_bytes);

    let mut certificate = certify_score_design(
        ScoreCertificateInputs {
            storage: &storage,
            nrows,
            rank,
            means: &means,
            finite_positions: &finite_positions,
            normalized_intercept_position,
            fit,
            weights,
        },
        reservation,
    )?;
    certificate.passed = certificate.left_inverse_relative_infinity_norm <= tolerance
        && certificate.right_inverse_relative_infinity_norm <= tolerance
        && certificate.intercept_coordinate_relative_infinity_norm <= tolerance;
    if !certificate.passed {
        return Err(invalid_data(
            "prepared robust score-design certificate did not pass",
        ));
    }
    let finite_row_buffer = reserved_filled(
        reservation,
        rank,
        0.0_f64,
        "unable to reserve prepared robust finite score row buffer",
    )?;
    Ok(CertifiedPreparedOlsScoreDesign {
        storage,
        nrows,
        finite_rank: rank,
        means,
        finite_row_buffer: RefCell::new(finite_row_buffer),
        reported_to_finite,
        normalized_intercept_position,
        certificate,
    })
}

fn build_dense_score_storage<P: PreparedOlsFitView + ?Sized, R: PreparedInferenceReservation>(
    prepared: &P,
    generated_columns: &[usize],
    weights: ReferenceWeightView<'_>,
    absorb: AbsorbConfig,
    cmg: &mut Option<CmgAbsorber<'_>>,
    reservation: &mut R,
) -> Result<(PreparedOlsScoreStorage, Vec<f64>, u64)> {
    let nrows = prepared.fit_sample().n_active();
    let rank = generated_columns.len();
    let elements = nrows
        .checked_mul(rank)
        .ok_or_else(|| allocation("prepared robust score design dimensions overflow usize"))?;
    let mut raw_values = reserved_filled(
        reservation,
        elements,
        0.0_f64,
        "unable to reserve prepared robust kept score design",
    )?;
    fill_raw_score_block(prepared, generated_columns, &mut raw_values)?;
    let means = score_block_means(&raw_values, nrows, rank, weights, reservation)?;
    let raw = DenseMatrix::from_row_major(nrows, rank, raw_values)?;
    let within = absorb_score_block(prepared, raw, absorb, cmg)?;
    Ok((PreparedOlsScoreStorage::Dense(within), means, 0))
}

fn build_scratch_score_storage<P: PreparedOlsFitView + ?Sized, R: PreparedInferenceReservation>(
    prepared: &P,
    generated_columns: &[usize],
    weights: ReferenceWeightView<'_>,
    absorb: AbsorbConfig,
    poolsize: usize,
    cmg: &mut Option<CmgAbsorber<'_>>,
    reservation: &mut R,
) -> Result<(PreparedOlsScoreStorage, Vec<f64>, u64)> {
    let nrows = prepared.fit_sample().n_active();
    let rank = generated_columns.len();
    let mut means = reserved_filled(
        reservation,
        rank,
        0.0_f64,
        "unable to reserve prepared robust score means",
    )?;
    if rank == 0 {
        let empty = DenseMatrix::from_row_major(nrows, 0, Vec::new())?;
        return Ok((PreparedOlsScoreStorage::Dense(empty), means, 0));
    }
    let block_width = poolsize.max(1).min(rank);
    let mut columns = PreparedOlsScratch::create(nrows, rank)?;
    let mut column = reserved_filled(
        reservation,
        nrows,
        0.0_f64,
        "unable to reserve prepared robust scratch score column",
    )?;
    for start in (0..rank).step_by(block_width) {
        let end = start.saturating_add(block_width).min(rank);
        let block_rank = end - start;
        let elements = nrows
            .checked_mul(block_rank)
            .ok_or_else(|| allocation("prepared robust scratch score block dimensions overflow"))?;
        let mut raw_values = reserved_filled(
            reservation,
            elements,
            0.0_f64,
            "unable to reserve prepared robust scratch score block",
        )?;
        fill_raw_score_block(prepared, &generated_columns[start..end], &mut raw_values)?;
        for local in 0..block_rank {
            means[start + local] =
                weighted_matrix_column_mean(&raw_values, nrows, block_rank, local, weights)?;
        }
        let raw = DenseMatrix::from_row_major(nrows, block_rank, raw_values)?;
        let within = absorb_score_block(prepared, raw, absorb, cmg)?;
        for local in 0..block_rank {
            for row in 0..nrows {
                column[row] = within[(row, local)];
            }
            columns.write_column(start + local, &column)?;
        }
    }
    let (rows, cache_values) = transpose_score_scratch(&mut columns, nrows, rank, reservation)?;
    let scratch_peak_bytes = columns
        .bytes()
        .checked_add(rows.bytes())
        .ok_or_else(|| allocation("prepared robust scratch peak bytes overflow"))?;
    Ok((
        PreparedOlsScoreStorage::Scratch(PreparedOlsScratchScoreStorage {
            scratch: RefCell::new(rows),
            cache: RefCell::new(PreparedOlsScoreRowCache {
                start_row: usize::MAX,
                rows: 0,
                values: cache_values,
            }),
        }),
        means,
        scratch_peak_bytes,
    ))
}

fn fill_raw_score_block<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    generated_columns: &[usize],
    output: &mut [f64],
) -> Result<()> {
    let nrows = prepared.fit_sample().n_active();
    let rank = generated_columns.len();
    if output.len() != nrows * rank {
        return Err(invalid_argument(
            "prepared robust raw score block dimensions are inconsistent",
        ));
    }
    for row in 0..nrows {
        for (column, generated) in generated_columns.iter().copied().enumerate() {
            let value = prepared.fit_column_value(generated, row)?;
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared robust raw score design contains a nonfinite value",
                ));
            }
            output[row * rank + column] = value;
        }
    }
    Ok(())
}

fn score_block_means<R: PreparedInferenceReservation>(
    values: &[f64],
    nrows: usize,
    ncols: usize,
    weights: ReferenceWeightView<'_>,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let mut means = reserved_filled(
        reservation,
        ncols,
        0.0_f64,
        "unable to reserve prepared robust score means",
    )?;
    for (column, mean) in means.iter_mut().enumerate() {
        *mean = weighted_matrix_column_mean(values, nrows, ncols, column, weights)?;
    }
    Ok(means)
}

fn absorb_score_block<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    raw: DenseMatrix,
    absorb: AbsorbConfig,
    cmg: &mut Option<CmgAbsorber<'_>>,
) -> Result<DenseMatrix> {
    if raw.ncols() == 0 {
        return Ok(raw);
    }
    let map_weights = match &prepared.fit_sample().weights {
        PreparedWeights::None => BorrowedMapWeights::Unit,
        PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
    };
    let absorbed = if let Some(session) = cmg.as_mut() {
        session.absorb(raw)?
    } else {
        prepared.fit_absorb_matrix(map_weights, raw, absorb)?
    };
    if !absorbed.report().certified() {
        return Err(invalid_data(
            "prepared robust score-design absorption did not certify",
        ));
    }
    Ok(absorbed.into_parts().0)
}

fn transpose_score_scratch<R: PreparedInferenceReservation>(
    columns: &mut PreparedOlsScratch,
    nrows: usize,
    rank: usize,
    reservation: &mut R,
) -> Result<(PreparedOlsScratch, Vec<f64>)> {
    let mut rows = PreparedOlsScratch::create(nrows, rank)?;
    let target_elements = (TRANSPOSE_BUFFER_BYTES / 8).max(rank);
    let tile_rows = (target_elements / rank).max(1).min(nrows);
    let tile_elements = tile_rows
        .checked_mul(rank)
        .ok_or_else(|| allocation("prepared robust scratch transpose tile overflows"))?;
    let mut tile = reserved_filled(
        reservation,
        tile_elements,
        0.0_f64,
        "unable to reserve prepared robust scratch transpose tile",
    )?;
    let mut column = reserved_filled(
        reservation,
        tile_rows,
        0.0_f64,
        "unable to reserve prepared robust scratch transpose column",
    )?;
    for start_row in (0..nrows).step_by(tile_rows) {
        let rows_in_tile = (nrows - start_row).min(tile_rows);
        for finite in 0..rank {
            columns.read_column_range(finite, start_row, &mut column[..rows_in_tile])?;
            for row in 0..rows_in_tile {
                tile[row * rank + finite] = column[row];
            }
        }
        let elements = rows_in_tile * rank;
        rows.write_linear(start_row * rank, &tile[..elements])?;
    }
    Ok((rows, tile))
}

struct ScoreCertificateInputs<'a> {
    storage: &'a PreparedOlsScoreStorage,
    nrows: usize,
    rank: usize,
    means: &'a [f64],
    finite_positions: &'a [usize],
    normalized_intercept_position: Option<usize>,
    fit: &'a CertifiedPreparedOlsFit,
    weights: ReferenceWeightView<'a>,
}

fn certify_score_design<R: PreparedInferenceReservation>(
    inputs: ScoreCertificateInputs<'_>,
    reservation: &mut R,
) -> Result<PreparedOlsScoreDesignCertificate> {
    let ScoreCertificateInputs {
        storage,
        nrows,
        rank,
        means,
        finite_positions,
        normalized_intercept_position,
        fit,
        weights,
    } = inputs;
    let dimension = fit.bread().dimension();
    let square = rank
        .checked_mul(rank)
        .ok_or_else(|| allocation("prepared robust score Gram dimensions overflow usize"))?;
    if means.len() != rank || finite_positions.len() != rank {
        return Err(invalid_argument(
            "prepared robust score certificate dimensions are inconsistent",
        ));
    }
    let mut gram = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust score Gram",
    )?;
    let mut corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust score Gram corrections",
    )?;
    let mut score_row = reserved_filled(
        reservation,
        rank,
        0.0_f64,
        "unable to reserve prepared robust score certificate row",
    )?;
    for row in 0..nrows {
        fill_score_storage_row(storage, nrows, rank, row, &mut score_row)?;
        let weight = weights.value(row)?;
        for left in 0..rank {
            for right in 0..rank {
                let term = weight * score_row[left] * score_row[right];
                let position = left * rank + right;
                compensated_add(term, &mut gram[position], &mut corrections[position])?;
            }
        }
    }
    for (value, correction) in gram.iter_mut().zip(corrections) {
        *value += correction;
        if !value.is_finite() {
            return Err(invalid_data(
                "prepared robust score Gram contains a nonfinite value",
            ));
        }
    }

    let mut left_error = 0.0_f64;
    let mut right_error = 0.0_f64;
    for row in 0..rank {
        let mut left_row_error = 0.0_f64;
        let mut right_row_error = 0.0_f64;
        let mut left_scale = 1.0_f64;
        let mut right_scale = 1.0_f64;
        for column in 0..rank {
            let mut left_product = 0.0_f64;
            let mut left_correction = 0.0_f64;
            let mut right_product = 0.0_f64;
            let mut right_correction = 0.0_f64;
            for inner in 0..rank {
                let bread_left = fit.bread().values()
                    [finite_positions[row] * dimension + finite_positions[inner]];
                let bread_right = fit.bread().values()
                    [finite_positions[inner] * dimension + finite_positions[column]];
                compensated_add(
                    bread_left * gram[inner * rank + column],
                    &mut left_product,
                    &mut left_correction,
                )?;
                compensated_add(
                    gram[row * rank + inner] * bread_right,
                    &mut right_product,
                    &mut right_correction,
                )?;
            }
            left_product += left_correction;
            right_product += right_correction;
            let identity = if row == column { 1.0 } else { 0.0 };
            left_row_error += (left_product - identity).abs();
            right_row_error += (right_product - identity).abs();
            left_scale += left_product.abs();
            right_scale += right_product.abs();
        }
        left_error = left_error.max(left_row_error / left_scale);
        right_error = right_error.max(right_row_error / right_scale);
    }

    let mut intercept_error = 0.0_f64;
    if let Some(intercept) = normalized_intercept_position {
        let effective_n = fit.bread().weight_scale().reference_weight_sum;
        if !effective_n.is_finite() || effective_n <= 0.0 {
            return Err(invalid_data(
                "prepared robust score intercept has invalid effective N",
            ));
        }
        let mut quadratic = 0.0_f64;
        let mut quadratic_correction = 0.0_f64;
        for row in 0..rank {
            let mut expected_cross = 0.0_f64;
            let mut expected_correction = 0.0_f64;
            for column in 0..rank {
                let bread = fit.bread().values()
                    [finite_positions[row] * dimension + finite_positions[column]];
                compensated_add(
                    -bread * means[column],
                    &mut expected_cross,
                    &mut expected_correction,
                )?;
            }
            expected_cross += expected_correction;
            let actual_left = fit.bread().values()[finite_positions[row] * dimension + intercept];
            let actual_right = fit.bread().values()[intercept * dimension + finite_positions[row]];
            let scale = expected_cross
                .abs()
                .max(actual_left.abs())
                .max(actual_right.abs())
                .max(1.0);
            intercept_error =
                intercept_error.max(relative_difference(actual_left, expected_cross, scale)?);
            intercept_error =
                intercept_error.max(relative_difference(actual_right, expected_cross, scale)?);
            compensated_add(
                -means[row] * expected_cross,
                &mut quadratic,
                &mut quadratic_correction,
            )?;
        }
        let expected_corner = 1.0 / effective_n + quadratic + quadratic_correction;
        let actual_corner = fit.bread().values()[intercept * dimension + intercept];
        intercept_error = intercept_error.max(relative_difference(
            actual_corner,
            expected_corner,
            actual_corner.abs().max(expected_corner.abs()).max(1.0),
        )?);
    } else if dimension != rank {
        return Err(invalid_argument(
            "prepared robust score certificate has an unexplained bread coordinate",
        ));
    }
    Ok(PreparedOlsScoreDesignCertificate {
        left_inverse_relative_infinity_norm: left_error,
        right_inverse_relative_infinity_norm: right_error,
        intercept_coordinate_relative_infinity_norm: intercept_error,
        passed: false,
    })
}

fn fill_score_storage_row(
    storage: &PreparedOlsScoreStorage,
    nrows: usize,
    rank: usize,
    row: usize,
    output: &mut [f64],
) -> Result<()> {
    if row >= nrows || output.len() != rank {
        return Err(invalid_argument(
            "prepared robust score storage row dimensions are inconsistent",
        ));
    }
    match storage {
        PreparedOlsScoreStorage::Dense(within) => {
            for (column, value) in output.iter_mut().enumerate() {
                *value = within[(row, column)];
            }
        }
        PreparedOlsScoreStorage::Scratch(scratch) => {
            scratch.fill_row(nrows, rank, row, output)?;
        }
    }
    Ok(())
}

fn weighted_matrix_column_mean(
    values: &[f64],
    nrows: usize,
    ncols: usize,
    column: usize,
    weights: ReferenceWeightView<'_>,
) -> Result<f64> {
    if nrows == 0 || column >= ncols || values.len() != nrows * ncols {
        return Err(invalid_argument(
            "prepared robust score mean dimensions are inconsistent",
        ));
    }
    let mut scale = 0.0_f64;
    for row in 0..nrows {
        scale = scale.max(values[row * ncols + column].abs());
    }
    if scale == 0.0 {
        return Ok(0.0);
    }
    let mut numerator = 0.0_f64;
    let mut numerator_correction = 0.0_f64;
    let mut denominator = 0.0_f64;
    let mut denominator_correction = 0.0_f64;
    for row in 0..nrows {
        let weight = weights.value(row)?;
        compensated_add(
            weight * (values[row * ncols + column] / scale),
            &mut numerator,
            &mut numerator_correction,
        )?;
        compensated_add(weight, &mut denominator, &mut denominator_correction)?;
    }
    let denominator = denominator + denominator_correction;
    let mean = scale * ((numerator + numerator_correction) / denominator);
    if mean.is_finite() && denominator.is_finite() && denominator > 0.0 {
        Ok(mean)
    } else {
        Err(invalid_data("prepared robust score mean is not finite"))
    }
}

pub(super) fn reserved_filled<T: Clone, R: PreparedInferenceReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "prepared robust reservation returned insufficient capacity",
        ));
    }
    output.resize(elements, value);
    Ok(output)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}
