use std::cell::RefCell;

use crate::data::{DenseMatrix, PreparedWeights};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{AbsorbConfig, BorrowedMapWeights, CmgAbsorber, CmgConfig, FeSolverKind};
use crate::options::ThreadConfig;
use crate::prepared_fit_view::PreparedOlsFitView;
use crate::sandwich::SandwichDesign;
use rayon::prelude::*;
use std::time::Instant;

use super::prepared_fit::PreparedOlsFastScoreCache;
use super::prepared_scratch::PreparedOlsScratch;
use super::prepared_unadjusted::{compensated_add, relative_difference, ReferenceWeightView};
use super::{CertifiedPreparedOlsFit, PreparedOlsBreadCoordinateKind, PreparedOlsExecutionMode};
pub(super) use crate::sandwich::{
    SandwichReservation as PreparedInferenceReservation,
    SystemSandwichReservation as SystemPreparedInferenceReservation,
};

const TRANSPOSE_BUFFER_BYTES: usize = 1 << 20;
const SCORE_CERTIFICATE_REDUCTION_ROWS: usize = 16_384;
const SCORE_CERTIFICATE_MAX_BLOCKS: usize = 64;
const SCORE_CERTIFICATE_PARTIAL_BUDGET_BYTES: usize = 32 << 20;

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
pub(super) struct CertifiedPreparedOlsScoreDesign<'a> {
    storage: PreparedOlsScoreStorage<'a>,
    nrows: usize,
    finite_rank: usize,
    means: Vec<f64>,
    finite_row_buffer: RefCell<Vec<f64>>,
    reported_to_finite: Vec<usize>,
    normalized_intercept_position: Option<usize>,
    pub(super) certificate: PreparedOlsScoreDesignCertificate,
}

#[derive(Debug)]
enum PreparedOlsScoreStorage<'a> {
    Dense(DenseMatrix),
    Strided(PreparedOlsStridedScoreStorage<'a>),
    Scratch(PreparedOlsScratchScoreStorage),
}

#[derive(Debug)]
struct PreparedOlsStridedScoreStorage<'a> {
    within: DenseMatrix,
    generated_columns: Vec<usize>,
    reference_norms: Vec<f64>,
    weights: ReferenceWeightView<'a>,
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

impl CertifiedPreparedOlsScoreDesign<'_> {
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
            PreparedOlsScoreStorage::Strided(strided) => {
                strided.fill_row(row, output)?;
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

    pub(super) fn dense_view(&self) -> Option<PreparedOlsDenseScoreView<'_>> {
        match &self.storage {
            PreparedOlsScoreStorage::Dense(within) => Some(PreparedOlsDenseScoreView {
                storage: PreparedOlsDenseScoreStorageView::Packed {
                    values: within.as_slice(),
                    finite_rank: self.finite_rank,
                },
                nrows: self.nrows,
                means: &self.means,
                reported_to_finite: &self.reported_to_finite,
                normalized_intercept_position: self.normalized_intercept_position,
            }),
            PreparedOlsScoreStorage::Strided(strided) => Some(PreparedOlsDenseScoreView {
                storage: PreparedOlsDenseScoreStorageView::Strided {
                    values: strided.within.as_slice(),
                    matrix_columns: strided.within.ncols(),
                    generated_columns: &strided.generated_columns,
                    reference_norms: &strided.reference_norms,
                    weights: strided.weights,
                },
                nrows: self.nrows,
                means: &self.means,
                reported_to_finite: &self.reported_to_finite,
                normalized_intercept_position: self.normalized_intercept_position,
            }),
            PreparedOlsScoreStorage::Scratch(_) => None,
        }
    }
}

impl PreparedOlsStridedScoreStorage<'_> {
    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        if row >= self.within.nrows()
            || output.len() != self.generated_columns.len()
            || self.reference_norms.len() != self.generated_columns.len()
        {
            return Err(invalid_argument(
                "prepared robust strided score row dimensions are inconsistent",
            ));
        }
        let root = self.weights.solver_root(row)?;
        let matrix_columns = self.within.ncols();
        let values = &self.within.as_slice()[row * matrix_columns..(row + 1) * matrix_columns];
        for ((value, generated), norm) in output
            .iter_mut()
            .zip(self.generated_columns.iter().copied())
            .zip(self.reference_norms.iter().copied())
        {
            *value = values[generated + 1] * norm / root;
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared robust strided score coordinate is not finite",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum PreparedOlsDenseScoreStorageView<'a> {
    Packed {
        values: &'a [f64],
        finite_rank: usize,
    },
    Strided {
        values: &'a [f64],
        matrix_columns: usize,
        generated_columns: &'a [usize],
        reference_norms: &'a [f64],
        weights: ReferenceWeightView<'a>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PreparedOlsDenseScoreView<'a> {
    storage: PreparedOlsDenseScoreStorageView<'a>,
    nrows: usize,
    means: &'a [f64],
    reported_to_finite: &'a [usize],
    normalized_intercept_position: Option<usize>,
}

impl PreparedOlsDenseScoreView<'_> {
    pub(super) const fn nrows(self) -> usize {
        self.nrows
    }

    pub(super) const fn reported_dimension(self) -> usize {
        self.reported_to_finite.len()
    }

    pub(super) fn fill_reported_row(self, row: usize, output: &mut [f64]) -> Result<()> {
        if row >= self.nrows || output.len() != self.reported_dimension() {
            return Err(invalid_argument(
                "prepared robust dense reported score row dimensions are inconsistent",
            ));
        }
        let strided_root = match self.storage {
            PreparedOlsDenseScoreStorageView::Packed { .. } => None,
            PreparedOlsDenseScoreStorageView::Strided { weights, .. } => {
                Some(weights.solver_root(row)?)
            }
        };
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
            let finite_value = match self.storage {
                PreparedOlsDenseScoreStorageView::Packed {
                    values,
                    finite_rank,
                } => {
                    let start = row.checked_mul(finite_rank).ok_or_else(|| {
                        allocation("prepared robust dense score offset overflows")
                    })?;
                    values[start + finite_position]
                }
                PreparedOlsDenseScoreStorageView::Strided {
                    values,
                    matrix_columns,
                    generated_columns,
                    reference_norms,
                    ..
                } => {
                    let generated = *generated_columns.get(finite_position).ok_or_else(|| {
                        invalid_argument("prepared robust strided score coordinate is absent")
                    })?;
                    let norm = *reference_norms.get(finite_position).ok_or_else(|| {
                        invalid_argument("prepared robust strided score norm is absent")
                    })?;
                    let root = strided_root.ok_or_else(|| {
                        invalid_argument("prepared robust strided score root is absent")
                    })?;
                    values[row * matrix_columns + generated + 1] * norm / root
                }
            };
            *value = finite_value
                + if self.normalized_intercept_position.is_some() {
                    self.means[finite_position]
                } else {
                    0.0
                };
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared robust dense reported score coordinate is not finite",
                ));
            }
        }
        Ok(())
    }
}

impl SandwichDesign for PreparedOlsDenseScoreView<'_> {
    fn nrows(&self) -> usize {
        (*self).nrows()
    }

    fn dimension(&self) -> usize {
        (*self).reported_dimension()
    }

    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        (*self).fill_reported_row(row, output)
    }
}

impl SandwichDesign for CertifiedPreparedOlsScoreDesign<'_> {
    fn nrows(&self) -> usize {
        self.nrows()
    }
    fn dimension(&self) -> usize {
        self.reported_to_finite.len()
    }
    fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
        self.fill_reported_row(row, output)
    }
}

impl PreparedOlsScratchScoreStorage {
    fn fill_row(&self, nrows: usize, rank: usize, row: usize, output: &mut [f64]) -> Result<()> {
        let mut cache = self.cache.borrow_mut();
        let cache_end = cache.start_row.saturating_add(cache.rows);
        if row < cache.start_row || row >= cache_end {
            let capacity_rows = cache.values.len() / rank;
            // Cluster ordering can jump across the file on every row. Decode
            // only that row after a jump; resume bounded read-ahead when the
            // next request continues the previous block. The allocated cache
            // and the score arithmetic are unchanged.
            let read_ahead = if cache.rows == 0 || row == cache_end {
                capacity_rows
            } else {
                1
            };
            let rows = read_ahead.min(nrows - row);
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
    'a,
    P: PreparedOlsFitView + ?Sized,
    R: PreparedInferenceReservation,
>(
    prepared: &'a P,
    fit: &mut CertifiedPreparedOlsFit,
    weights: ReferenceWeightView<'a>,
    absorb: AbsorbConfig,
    tolerance: f64,
    reservation: &mut R,
    retained_scratch: Option<PreparedOlsScratch>,
) -> Result<CertifiedPreparedOlsScoreDesign<'a>> {
    let phase_timing = std::env::var_os("FEREG_PHASE_TIMING").is_some();
    let total_started = Instant::now();
    let phase_started = Instant::now();
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
    // A reconstructed score bank can invoke another solver while the final
    // fit's coefficient product remains live. Retained/cached score paths do
    // not construct that solver and need no additional workspace reservation.
    let retained_fe_parent = if cached.is_none() && retained_scratch.is_none() {
        fit.retained_fe_coefficient_bytes()
    } else {
        0
    };
    let mut absorb = absorb;
    if let Some(budget) = absorb.solver_memory_budget.as_mut() {
        budget.execution_bytes = budget
            .execution_bytes
            .checked_add(retained_fe_parent)
            .ok_or_else(|| allocation("inference FE parent budget overflows"))?;
        if !budget.fits(0) {
            return Err(allocation(
                "inference FE coefficient parent exceeds usable memory",
            ));
        }
    }
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
    let mut solver_peak = crate::memory::SolverWorkspacePeak::ZERO;
    let mut cmg = if let (Some(requested_solver), true) = (
        score_cmg_solver,
        cached.is_none() && retained_scratch.is_none(),
    ) {
        let built = CmgAbsorber::build_with_workspace(
            prepared.fit_borrowed_cmg_design()?,
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
                components: crate::fe::explicit_cmg_components(requested_solver),
                solver_memory_budget: absorb.solver_memory_budget,
            },
            prepared.fit_structural_intercept(),
            &mut solver_peak,
        );
        match built {
            Ok(session) => Some(session),
            Err(_) if requested_solver == FeSolverKind::Auto => None,
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    let setup_seconds = phase_started.elapsed().as_secs_f64();
    let phase_started = Instant::now();
    let used_strided_cache = matches!(&cached, Some(PreparedOlsFastScoreCache::Strided { .. }));
    let reabsorbed_fast_scores =
        fit.execution().mode == PreparedOlsExecutionMode::Fast && cached.is_none();
    let (storage, means, scratch_peak_bytes) = match (fit.execution().mode, cached) {
        (
            PreparedOlsExecutionMode::Fast,
            Some(PreparedOlsFastScoreCache::Packed { within, means, .. }),
        ) => (PreparedOlsScoreStorage::Dense(within), means, 0),
        (
            PreparedOlsExecutionMode::Fast,
            Some(PreparedOlsFastScoreCache::Strided {
                generated_columns,
                within,
                reference_norms,
                means,
            }),
        ) => (
            PreparedOlsScoreStorage::Strided(PreparedOlsStridedScoreStorage {
                within,
                generated_columns,
                reference_norms,
                weights,
            }),
            means,
            0,
        ),
        (PreparedOlsExecutionMode::Fast, None) => build_dense_score_storage(
            prepared,
            &generated_columns,
            weights,
            absorb,
            &mut cmg,
            &mut solver_peak,
            reservation,
        )?,
        (PreparedOlsExecutionMode::Low, _) => build_scratch_score_storage(
            prepared,
            &generated_columns,
            weights,
            absorb,
            fit.execution().poolsize,
            &mut cmg,
            &mut solver_peak,
            reservation,
            retained_scratch,
        )?,
    };
    fit.record_additional_scratch_peak(scratch_peak_bytes);
    fit.record_additional_solver_workspace_peak(solver_peak.with_retained(retained_fe_parent)?);
    let storage_seconds = phase_started.elapsed().as_secs_f64();

    let phase_started = Instant::now();
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
    let certificate_seconds = phase_started.elapsed().as_secs_f64();
    let phase_started = Instant::now();
    let finite_row_buffer = reserved_filled(
        reservation,
        rank,
        0.0_f64,
        "unable to reserve prepared robust finite score row buffer",
    )?;
    let finalize_seconds = phase_started.elapsed().as_secs_f64();
    if phase_timing {
        eprintln!(
            "FEREG_PHASE|score_design|setup={setup_seconds:.9}|storage={storage_seconds:.9}|certificate={certificate_seconds:.9}|finalize={finalize_seconds:.9}|strided={}|reabsorbed={}|total={:.9}",
            usize::from(used_strided_cache),
            usize::from(reabsorbed_fast_scores),
            total_started.elapsed().as_secs_f64(),
        );
    }
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

fn build_dense_score_storage<
    'a,
    P: PreparedOlsFitView + ?Sized,
    R: PreparedInferenceReservation,
>(
    prepared: &'a P,
    generated_columns: &[usize],
    weights: ReferenceWeightView<'a>,
    absorb: AbsorbConfig,
    cmg: &mut Option<CmgAbsorber<'_>>,
    solver_peak: &mut crate::memory::SolverWorkspacePeak,
    reservation: &mut R,
) -> Result<(PreparedOlsScoreStorage<'a>, Vec<f64>, u64)> {
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
    let within = absorb_score_block(prepared, raw, absorb, cmg, solver_peak)?;
    Ok((PreparedOlsScoreStorage::Dense(within), means, 0))
}

fn build_scratch_score_storage<
    'a,
    P: PreparedOlsFitView + ?Sized,
    R: PreparedInferenceReservation,
>(
    prepared: &'a P,
    generated_columns: &[usize],
    weights: ReferenceWeightView<'a>,
    absorb: AbsorbConfig,
    poolsize: usize,
    cmg: &mut Option<CmgAbsorber<'_>>,
    solver_peak: &mut crate::memory::SolverWorkspacePeak,
    reservation: &mut R,
    retained_scratch: Option<PreparedOlsScratch>,
) -> Result<(PreparedOlsScoreStorage<'a>, Vec<f64>, u64)> {
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
    let reused = retained_scratch.is_some();
    let mut columns = match retained_scratch {
        Some(columns) => {
            if columns.rows() != nrows || columns.columns() != prepared.fit_model().columns.len() {
                return Err(invalid_argument(
                    "retained low-memory score scratch has a different model frame",
                ));
            }
            columns
        }
        None => PreparedOlsScratch::create(nrows, rank)?,
    };
    let mut column = reserved_filled(
        reservation,
        if reused { 0 } else { nrows },
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
        if reused {
            continue;
        }
        let raw = DenseMatrix::from_row_major(nrows, block_rank, raw_values)?;
        let within = absorb_score_block(prepared, raw, absorb, cmg, solver_peak)?;
        for local in 0..block_rank {
            for row in 0..nrows {
                column[row] = within[(row, local)];
            }
            columns.write_column(start + local, &column)?;
        }
    }
    let (rows, cache_values) = transpose_score_scratch(
        &mut columns,
        nrows,
        rank,
        reservation,
        reused.then_some(generated_columns),
    )?;
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
    solver_peak: &mut crate::memory::SolverWorkspacePeak,
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
    *solver_peak = solver_peak.max(absorbed.report().solver_workspace_peak);
    Ok(absorbed.into_parts().0)
}

fn transpose_score_scratch<R: PreparedInferenceReservation>(
    columns: &mut PreparedOlsScratch,
    nrows: usize,
    rank: usize,
    reservation: &mut R,
    selected_columns: Option<&[usize]>,
) -> Result<(PreparedOlsScratch, Vec<f64>)> {
    if selected_columns.is_some_and(|selected| selected.len() != rank) {
        return Err(invalid_argument(
            "retained score selection differs from finite rank",
        ));
    }
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
            let stored = selected_columns.map_or(finite, |selected| selected[finite]);
            columns.read_column_range(stored, start_row, &mut column[..rows_in_tile])?;
            for row in 0..rows_in_tile {
                tile[row * rank + finite] = column[row];
            }
        }
        let elements = rows_in_tile * rank;
        rows.write_linear(start_row * rank, &tile[..elements])?;
    }
    Ok((rows, tile))
}

struct ScoreCertificateInputs<'storage, 'weights> {
    storage: &'storage PreparedOlsScoreStorage<'weights>,
    nrows: usize,
    rank: usize,
    means: &'storage [f64],
    finite_positions: &'storage [usize],
    normalized_intercept_position: Option<usize>,
    fit: &'storage CertifiedPreparedOlsFit,
    weights: ReferenceWeightView<'weights>,
}

fn certify_score_design<R: PreparedInferenceReservation>(
    inputs: ScoreCertificateInputs<'_, '_>,
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
    if means.len() != rank || finite_positions.len() != rank {
        return Err(invalid_argument(
            "prepared robust score certificate dimensions are inconsistent",
        ));
    }
    let gram = match storage {
        PreparedOlsScoreStorage::Dense(within) => {
            certify_dense_score_gram(within.as_slice(), nrows, rank, weights, reservation)?
        }
        PreparedOlsScoreStorage::Strided(strided) => {
            certify_strided_score_gram(strided, nrows, rank, reservation)?
        }
        PreparedOlsScoreStorage::Scratch(_) => {
            certify_scratch_score_gram(storage, nrows, rank, weights, reservation)?
        }
    };

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

fn certify_strided_score_gram<R: PreparedInferenceReservation>(
    storage: &PreparedOlsStridedScoreStorage<'_>,
    nrows: usize,
    rank: usize,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let square = rank.checked_mul(rank).ok_or_else(|| {
        allocation("prepared robust strided score Gram dimensions overflow usize")
    })?;
    let matrix_columns = storage.within.ncols();
    if storage.within.nrows() != nrows
        || storage.generated_columns.len() != rank
        || storage.reference_norms.len() != rank
        || storage
            .generated_columns
            .iter()
            .any(|generated| generated.saturating_add(1) >= matrix_columns)
    {
        return Err(invalid_argument(
            "prepared robust strided score Gram dimensions are inconsistent",
        ));
    }
    if rank == 0 {
        return Ok(Vec::new());
    }
    let partial_bytes = square
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or_else(|| allocation("prepared robust score Gram partial bytes overflow usize"))?;
    let memory_blocks = (SCORE_CERTIFICATE_PARTIAL_BUDGET_BYTES / partial_bytes.max(1)).max(1);
    let chunks = nrows
        .div_ceil(SCORE_CERTIFICATE_REDUCTION_ROWS)
        .clamp(1, SCORE_CERTIFICATE_MAX_BLOCKS)
        .min(memory_blocks);
    let partial_elements = chunks
        .checked_mul(square)
        .ok_or_else(|| allocation("prepared robust score Gram partials overflow usize"))?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve prepared robust strided score Gram partials",
    )?;
    let multiplier = storage.weights.multiplier();
    if !multiplier.is_finite() || multiplier <= 0.0 {
        return Err(invalid_data(
            "prepared robust strided score weight multiplier is invalid",
        ));
    }
    partials
        .par_chunks_mut(square)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let rows_per_chunk = nrows / chunks;
            let remainder = nrows % chunks;
            let start = chunk * rows_per_chunk + chunk.min(remainder);
            let end = start + rows_per_chunk + usize::from(chunk < remainder);
            accumulate_strided_score_gram_rows(
                &storage.within.as_slice()[start * matrix_columns..end * matrix_columns],
                matrix_columns, &storage.generated_columns, &storage.reference_norms,
                multiplier, partial,
            );
            if partial.iter().all(|value| value.is_finite()) {
                Ok(())
            } else {
                Err(invalid_data(
                    "prepared robust strided score Gram partial contains a nonfinite value",
                ))
            }
        })?;
    let mut gram = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust strided score Gram",
    )?;
    let mut corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust strided score Gram corrections",
    )?;
    for partial in partials.chunks_exact(square) {
        for left in 0..rank {
            for right in left..rank {
                let position = left * rank + right;
                compensated_add(
                    partial[position],
                    &mut gram[position],
                    &mut corrections[position],
                )?;
            }
        }
    }
    for left in 0..rank {
        for right in left..rank {
            let position = left * rank + right;
            gram[position] += corrections[position];
            if !gram[position].is_finite() {
                return Err(invalid_data(
                    "prepared robust strided score Gram contains a nonfinite value",
                ));
            }
            gram[right * rank + left] = gram[position];
        }
    }
    Ok(gram)
}

fn accumulate_strided_score_gram_rows(
    rows: &[f64],
    matrix_columns: usize,
    generated_columns: &[usize],
    reference_norms: &[f64],
    multiplier: f64,
    partial: &mut [f64],
) {
    let rank = generated_columns.len();
    let mut reconstructed = [0.0_f64; 64];
    for values in rows.chunks_exact(matrix_columns) {
        // Each selected value has one fixed reconstruction scale. Reuse
        // that product across the Gram row without changing any entry's
        // observation order, multiplication order, or reduction tree.
        if (8..=reconstructed.len()).contains(&rank) {
            let reconstructed = &mut reconstructed[..rank];
            for ((value, &generated), &norm) in reconstructed.iter_mut()
                .zip(generated_columns).zip(reference_norms)
            {
                *value = values[generated + 1] * norm;
            }
            for left in 0..rank {
                let scaled_left = multiplier * reconstructed[left];
                for (destination, &right_value) in partial[left * rank + left..(left + 1) * rank]
                    .iter_mut().zip(&reconstructed[left..])
                {
                    *destination += scaled_left * right_value;
                }
            }
            continue;
        }
        for left in 0..rank {
            let left_value =
                values[generated_columns[left] + 1] * reference_norms[left];
            let scaled_left = multiplier * left_value;
            for right in left..rank {
                let right_value = values[generated_columns[right] + 1]
                    * reference_norms[right];
                partial[left * rank + right] += scaled_left * right_value;
            }
        }
    }
}

fn certify_dense_score_gram<R: PreparedInferenceReservation>(
    values: &[f64],
    nrows: usize,
    rank: usize,
    weights: ReferenceWeightView<'_>,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let square = rank
        .checked_mul(rank)
        .ok_or_else(|| allocation("prepared robust score Gram dimensions overflow usize"))?;
    if values.len() != nrows * rank {
        return Err(invalid_argument(
            "prepared robust dense score Gram dimensions are inconsistent",
        ));
    }
    if rank == 0 {
        return Ok(Vec::new());
    }
    let partial_bytes = square
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or_else(|| allocation("prepared robust score Gram partial bytes overflow usize"))?;
    let memory_blocks = (SCORE_CERTIFICATE_PARTIAL_BUDGET_BYTES / partial_bytes.max(1)).max(1);
    let chunks = nrows
        .div_ceil(SCORE_CERTIFICATE_REDUCTION_ROWS)
        .clamp(1, SCORE_CERTIFICATE_MAX_BLOCKS)
        .min(memory_blocks);
    let partial_elements = chunks
        .checked_mul(square)
        .ok_or_else(|| allocation("prepared robust score Gram partials overflow usize"))?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve prepared robust score Gram partials",
    )?;
    partials
        .par_chunks_mut(square)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let rows_per_chunk = nrows / chunks;
            let remainder = nrows % chunks;
            let start = chunk * rows_per_chunk + chunk.min(remainder);
            let end = start + rows_per_chunk + usize::from(chunk < remainder);
            for row in start..end {
                let weight = weights.value(row)?;
                let score = &values[row * rank..(row + 1) * rank];
                for left in 0..rank {
                    let scaled_left = weight * score[left];
                    for (destination, &right_value) in
                        partial[left * rank + left..(left + 1) * rank]
                            .iter_mut().zip(&score[left..])
                    {
                        *destination += scaled_left * right_value;
                    }
                }
            }
            if partial.iter().all(|value| value.is_finite()) {
                Ok(())
            } else {
                Err(invalid_data(
                    "prepared robust score Gram partial contains a nonfinite value",
                ))
            }
        })?;
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
    for partial in partials.chunks_exact(square) {
        for left in 0..rank {
            for right in left..rank {
                let position = left * rank + right;
                compensated_add(
                    partial[position],
                    &mut gram[position],
                    &mut corrections[position],
                )?;
            }
        }
    }
    for left in 0..rank {
        for right in left..rank {
            let position = left * rank + right;
            gram[position] += corrections[position];
            if !gram[position].is_finite() {
                return Err(invalid_data(
                    "prepared robust score Gram contains a nonfinite value",
                ));
            }
            gram[right * rank + left] = gram[position];
        }
    }
    Ok(gram)
}

fn certify_scratch_score_gram<R: PreparedInferenceReservation>(
    storage: &PreparedOlsScoreStorage<'_>,
    nrows: usize,
    rank: usize,
    weights: ReferenceWeightView<'_>,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let square = rank
        .checked_mul(rank)
        .ok_or_else(|| allocation("prepared robust score Gram dimensions overflow usize"))?;
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
                let position = left * rank + right;
                compensated_add(
                    weight * score_row[left] * score_row[right],
                    &mut gram[position],
                    &mut corrections[position],
                )?;
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
    Ok(gram)
}

fn fill_score_storage_row(
    storage: &PreparedOlsScoreStorage<'_>,
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
        PreparedOlsScoreStorage::Strided(strided) => strided.fill_row(row, output)?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_score_reads_preserve_bits_without_repeated_full_tile_decoding() {
        let nrows = 65_537;
        for rank in [1, 6, 50] {
            let values: Vec<_> = (0..nrows * rank).map(|i| match i % 7 {
                0 => -0.0,
                1 => 1e-300,
                2 => -1e300,
                _ => (i as f64 * 0.17).sin(),
            }).collect();
            let mut scratch = PreparedOlsScratch::create(nrows, rank).unwrap();
            scratch.write_linear(0, &values).unwrap();
            let cache_rows = (TRANSPOSE_BUFFER_BYTES / 8 / rank).min(nrows);
            let storage = PreparedOlsScratchScoreStorage {
                scratch: RefCell::new(scratch),
                cache: RefCell::new(PreparedOlsScoreRowCache {
                    start_row: usize::MAX,
                    rows: 0,
                    values: vec![0.0; cache_rows * rank],
                }),
            };
            let mut row_values = vec![0.0; rank];
            for stride in [1, 32_771, 1] {
                let mut decoded_rows = 0;
                let mut reads = 0;
                for index in 0..nrows {
                    let row = index * stride % nrows;
                    let misses = {
                        let cache = storage.cache.borrow();
                        row < cache.start_row || row >= cache.start_row.saturating_add(cache.rows)
                    };
                    storage.fill_row(nrows, rank, row, &mut row_values).unwrap();
                    assert!(row_values.iter().zip(&values[row * rank..(row + 1) * rank])
                        .all(|(actual, expected)| actual.to_bits() == expected.to_bits()));
                    if misses {
                        decoded_rows += storage.cache.borrow().rows;
                        reads += 1;
                    }
                }
                assert!(decoded_rows <= 2 * nrows, "rank={rank}, stride={stride}, decoded={decoded_rows}");
                if stride == 1 {
                    assert!(reads <= nrows.div_ceil(cache_rows) + 1);
                }
                assert_eq!(storage.cache.borrow().values.len(), cache_rows * rank);
            }
        }
    }

    #[test]
    fn strided_gram_row_cache_preserves_products_and_nonfinite_rejection() {
        for rank in [1, 7, 8, 10, 50, 64, 65] {
            let columns = 2 * rank + 1;
            let generated: Vec<_> = (0..rank).rev().map(|column| 2 * column).collect();
            let norms: Vec<_> = (0..rank).map(|column| 10.0_f64.powi(column as i32 % 9 - 4)).collect();
            let mut rows: Vec<_> = (0..257 * columns)
                .map(|index| ((index * 37 % 101) as f64 - 50.0) / 17.0).collect();
            for multiplier in [1.0, 0.17, 3.7, 1e-120, 1e120] {
                let mut expected = vec![0.0; rank * rank];
                for values in rows.chunks_exact(columns) {
                    for left in 0..rank {
                        let scaled = multiplier * (values[generated[left] + 1] * norms[left]);
                        for right in left..rank {
                            expected[left * rank + right] += scaled * (values[generated[right] + 1] * norms[right]);
                        }
                    }
                }
                let mut actual = vec![0.0; rank * rank];
                accumulate_strided_score_gram_rows(&rows, columns, &generated, &norms, multiplier, &mut actual);
                assert_eq!(actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                    expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), "rank={rank}, multiplier={multiplier}");
            }
            for nonfinite in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
                rows[generated[0] + 1] = nonfinite;
                let mut partial = vec![0.0; rank * rank];
                accumulate_strided_score_gram_rows(&rows, columns, &generated, &norms, 1.0, &mut partial);
                assert!(partial.iter().any(|value| !value.is_finite()));
            }
        }
    }
}
