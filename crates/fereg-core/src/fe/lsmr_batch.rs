//! Bounded transport around the pinned independent-RHS solver.
use std::time::Instant;

use rayon::prelude::*;

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError};
use crate::memory::lsmr_rhs_window;
use within::{BatchSolveResult, Effect, LsmrOptions, PreconditionerConfig, Solver, WithinError};

#[derive(Debug)]
pub(super) enum BatchError {
    Upstream(WithinError),
    /// Maximum attempted work across this joined batch, including failed steps
    /// and RHSs completed in earlier windows.
    Attempt {
        source: within::SolveError,
        iterations: usize,
    },
    Allocation(FeregError),
}

impl BatchError {
    pub(super) fn attempted_iterations(&self) -> usize {
        match self {
            Self::Attempt { iterations, .. } => *iterations,
            _ => 0,
        }
    }

    fn positivity_failure(&self) -> bool {
        matches!(self,
            Self::Upstream(WithinError::Solve(within::SolveError::InvalidInput { context, message }))
            | Self::Attempt { source: within::SolveError::InvalidInput { context, message }, .. }
            if *context == "mlsmr" && message.contains("preconditioner not positive definite"))
    }

    pub(super) fn preconditioner_breakdown(&self) -> bool {
        !crate::interrupt::cancellation_requested() && self.positivity_failure()
    }

    // Pure priority after joining all RHSs. A positivity refusal must not hide
    // cancellation, a fatal error, or an allocation failure in another RHS.
    fn failure_rank(&self) -> u8 {
        match self {
            Self::Upstream(WithinError::Build(within::BuildError::Cancelled))
            | Self::Upstream(WithinError::Solve(within::SolveError::Cancelled))
            | Self::Attempt {
                source: within::SolveError::Cancelled,
                ..
            } => 0,
            Self::Allocation(error) if error.code == ErrorCode::Interrupted => 0,
            Self::Upstream(WithinError::Solve(within::SolveError::Allocation { .. }))
            | Self::Attempt {
                source: within::SolveError::Allocation { .. },
                ..
            } => 2,
            Self::Allocation(error) if error.code == ErrorCode::Allocation => 2,
            _ if self.positivity_failure() => 3,
            _ => 1,
        }
    }

    pub(super) fn into_fereg(self, context: &str) -> FeregError {
        if let Err(error) = crate::interrupt::checkpoint() {
            return error;
        }
        match self {
            Self::Allocation(error) => error,
            Self::Attempt { source, .. } => Self::Upstream(source.into()).into_fereg(context),
            Self::Upstream(WithinError::Build(within::BuildError::Cancelled))
            | Self::Upstream(WithinError::Solve(within::SolveError::Cancelled)) => {
                FeregError::new(ErrorCode::Interrupted, format!("{context}: cancelled"))
            }
            Self::Upstream(WithinError::Solve(within::SolveError::Allocation { .. })) => {
                FeregError::new(
                    ErrorCode::Allocation,
                    format!("{context}: allocation failed"),
                )
            }
            Self::Upstream(error) => {
                FeregError::new(ErrorCode::Internal, format!("{context}: {error}"))
            }
        }
    }
}

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<(), BatchError> {
    values.try_reserve_exact(additional).map_err(|_| {
        BatchError::Allocation(FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve bounded LSMR transport",
        ))
    })
}

fn cells(rows: usize, columns: usize) -> Result<usize, BatchError> {
    rows.checked_mul(columns).ok_or_else(|| {
        BatchError::Allocation(FeregError::new(
            ErrorCode::Allocation,
            "bounded LSMR transport dimensions overflow",
        ))
    })
}

// Keep a small row/column tile in cache while crossing the row-major public
// matrix and the independent-RHS solver's column-major transport. Workers own
// disjoint destinations; these are bitwise copies, with no reduction order.
const TRANSPORT_ROWS: usize = 32;
const TRANSPORT_COLUMNS: usize = 8;

/// Column-major RHS for the many-FE routing windows. A row-major source is
/// transported on demand, bitwise, so a window's copies can be released once
/// all independent attempts have joined and transported again for an escalation
/// or retry; borrowed columns are returned as they are.
pub(super) trait RhsSource: Sync {
    fn count(&self) -> usize;
    fn columns(&self, positions: &[usize]) -> crate::error::Result<RhsColumns<'_>>;
    fn all_columns(&self) -> crate::error::Result<RhsColumns<'_>> {
        let positions: Vec<_> = (0..self.count()).collect();
        self.columns(&positions)
    }
}

pub(super) enum RhsColumns<'a> {
    Borrowed(Vec<&'a [f64]>),
    Owned(Vec<Vec<f64>>),
}

impl RhsColumns<'_> {
    pub(super) fn views(&self) -> Vec<&[f64]> {
        match self {
            Self::Borrowed(columns) => columns.clone(),
            Self::Owned(columns) => columns.iter().map(Vec::as_slice).collect(),
        }
    }
}

impl RhsSource for DenseMatrix {
    fn count(&self) -> usize {
        self.ncols()
    }

    fn columns(&self, positions: &[usize]) -> crate::error::Result<RhsColumns<'_>> {
        if positions.len() == self.ncols() && positions.iter().enumerate().all(|(i, &p)| i == p) {
            rhs_columns(self).map(RhsColumns::Owned)
        } else {
            selected_rhs_columns(self, positions).map(RhsColumns::Owned)
        }
    }
}

impl RhsSource for [&[f64]] {
    fn count(&self) -> usize {
        self.len()
    }

    fn columns(&self, positions: &[usize]) -> crate::error::Result<RhsColumns<'_>> {
        positions
            .iter()
            .map(|&column| {
                self.get(column).copied().ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::InvalidArgument,
                        "LSMR selected RHS is out of bounds",
                    )
                })
            })
            .collect::<crate::error::Result<_>>()
            .map(RhsColumns::Borrowed)
    }
}

pub(super) fn rhs_columns(values: &DenseMatrix) -> crate::error::Result<Vec<Vec<f64>>> {
    rhs_columns_mapped(values, values.ncols(), |column| column)
}

pub(super) fn selected_rhs_columns(
    values: &DenseMatrix,
    positions: &[usize],
) -> crate::error::Result<Vec<Vec<f64>>> {
    if positions.iter().any(|&column| column >= values.ncols()) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR selected RHS is out of bounds",
        ));
    }
    rhs_columns_mapped(values, positions.len(), |column| positions[column])
}

fn rhs_columns_mapped(
    values: &DenseMatrix,
    width: usize,
    position: impl Fn(usize) -> usize + Sync,
) -> crate::error::Result<Vec<Vec<f64>>> {
    let mut columns = Vec::new();
    reserve(&mut columns, width).map_err(|e| e.into_fereg("LSMR RHS transport"))?;
    for _ in 0..width {
        let mut column = Vec::new();
        reserve(&mut column, values.nrows()).map_err(|e| e.into_fereg("LSMR RHS transport"))?;
        column.resize(values.nrows(), 0.0);
        columns.push(column);
    }
    let rows = values.nrows();
    let plan = crate::parallel::StagePlan::current(rows as u128 * width as u128, rows);
    if width.div_ceil(TRANSPORT_COLUMNS) >= plan.lanes || rows == 0 {
        columns
            .par_chunks_mut(TRANSPORT_COLUMNS)
            .enumerate()
            .for_each(|(tile, columns)| {
                transport_rows(values, columns, 0, |offset| {
                    position(tile * TRANSPORT_COLUMNS + offset)
                });
            });
        return Ok(columns);
    }
    // Narrow banks cannot fill the team with column tiles. Row blocks own
    // disjoint destination rows of every column; copies are bitwise.
    let block = plan.chunk(rows).next_multiple_of(TRANSPORT_ROWS);
    let mut blocks: Vec<Vec<&mut [f64]>> = Vec::new();
    reserve(&mut blocks, rows.div_ceil(block)).map_err(|e| e.into_fereg("LSMR RHS transport"))?;
    for _ in 0..rows.div_ceil(block) {
        let mut parts = Vec::new();
        reserve(&mut parts, width).map_err(|e| e.into_fereg("LSMR RHS transport"))?;
        blocks.push(parts);
    }
    for column in &mut columns {
        for (index, part) in column.chunks_mut(block).enumerate() {
            blocks[index].push(part);
        }
    }
    blocks
        .into_par_iter()
        .enumerate()
        .for_each(|(index, mut parts)| {
            transport_rows(values, &mut parts, index * block, &position);
        });
    Ok(columns)
}

/// Copy rows `first..first + len` of the selected source columns into each
/// destination, in cache-sized row tiles.
fn transport_rows<D: AsMut<[f64]>>(
    values: &DenseMatrix,
    destinations: &mut [D],
    first: usize,
    position: impl Fn(usize) -> usize,
) {
    let len = destinations
        .first_mut()
        .map_or(0, |destination| destination.as_mut().len());
    for start in (0..len).step_by(TRANSPORT_ROWS) {
        let end = (start + TRANSPORT_ROWS).min(len);
        for (offset, destination) in destinations.iter_mut().enumerate() {
            let c = position(offset);
            for (row, value) in destination.as_mut()[start..end].iter_mut().enumerate() {
                *value = values[(first + start + row, c)];
            }
        }
    }
}

pub(super) fn copy_demeaned(columns: &[f64], values: &mut DenseMatrix) {
    let rows = values.nrows();
    let width = values.ncols();
    assert_eq!(columns.len(), values.as_slice().len());
    if width == 0 {
        return;
    }
    values
        .as_mut_slice()
        .par_chunks_mut(TRANSPORT_ROWS * width)
        .enumerate()
        .for_each(|(tile, destination)| {
            let start = tile * TRANSPORT_ROWS;
            for column in 0..width {
                let source = &columns
                    [column * rows + start..column * rows + start + destination.len() / width];
                for (row, value) in source.iter().enumerate() {
                    destination[row * width + column] = *value;
                }
            }
        });
}

/// Patch independently solved columns while retaining every already-certified
/// RHS. Only bitwise transport happens here; the owner recertifies the full bank.
pub(super) fn copy_selected_demeaned(
    columns: &[f64],
    values: &mut DenseMatrix,
    positions: &[usize],
) {
    let rows = values.nrows();
    let width = values.ncols();
    assert_eq!(columns.len(), rows * positions.len());
    assert!(positions.iter().all(|&column| column < width));
    if width == 0 {
        return;
    }
    values
        .as_mut_slice()
        .par_chunks_mut(TRANSPORT_ROWS * width)
        .enumerate()
        .for_each(|(tile, destination)| {
            let start = tile * TRANSPORT_ROWS;
            for (source_column, &column) in positions.iter().enumerate() {
                let source = &columns[source_column * rows + start
                    ..source_column * rows + start + destination.len() / width];
                for (row, value) in source.iter().enumerate() {
                    destination[row * width + column] = *value;
                }
            }
        });
}

pub(super) fn solve_batch(
    effects: Vec<Effect<'_>>,
    rhs: &[&[f64]],
    weights: Option<&[f64]>,
    options: &LsmrOptions,
    preconditioner: PreconditionerConfig,
    _deterministic: bool,
) -> Result<BatchSolveResult, BatchError> {
    let started = Instant::now();
    // Upstream keeps construction weights borrowed and uses its reproducible
    // owner reduction in every mode. The solver still lives only for this
    // batch; explicit windows below bound the simultaneous RHS recurrences.
    let solver = Solver::new_cancellable(
        effects,
        weights,
        preconditioner,
        within::Cancellation::new(&crate::interrupt::cancellation_requested),
    )
    .map_err(|error| BatchError::Upstream(error.into()))?;
    let setup = started.elapsed().as_secs_f64();
    let mut result = solve_prepared_batch(&solver, rhs, options)?;
    result.time_setup += setup;
    result.time_total = started.elapsed().as_secs_f64();
    Ok(result)
}

pub(super) fn solve_prepared_batch(
    solver: &Solver<'_>,
    rhs: &[&[f64]],
    options: &LsmrOptions,
) -> Result<BatchSolveResult, BatchError> {
    solve_prepared_batch_seeded(solver, rhs, options, None)
}

/// Controlled batch with optional total coefficient seeds in RHS-major order,
/// using the original response and one fixed preconditioner for each attempt.
pub(super) fn solve_prepared_batch_seeded(
    solver: &Solver<'_>,
    rhs: &[&[f64]],
    options: &LsmrOptions,
    initial: Option<&[f64]>,
) -> Result<BatchSolveResult, BatchError> {
    if let Some(initial) = initial {
        if initial.len() != cells(solver.n_dofs(), rhs.len())? {
            return Err(BatchError::Allocation(FeregError::new(
                ErrorCode::InvalidArgument,
                "LSMR initial coefficient bank has inconsistent dimensions",
            )));
        }
    }
    if rhs.is_empty() {
        return solver
            .solve_batch_cancellable(
                rhs,
                options,
                within::Cancellation::new(&crate::interrupt::cancellation_requested),
            )
            .map_err(BatchError::Upstream);
    }
    let started = Instant::now();
    // Bound live recurrences even when nested Rayon suspends one RHS and starts
    // another. Cold controlled calls preserve the upstream cold arithmetic and
    // also report a failed iteration, which a later retry must not refund.
    let window = lsmr_rhs_window(rhs.len(), rayon::current_num_threads());
    let mut batch = None;
    let mut spent = 0;
    for (window_index, columns) in rhs.chunks(window).enumerate() {
        let mut results = Vec::new();
        reserve(&mut results, columns.len())?;
        columns
            .par_iter()
            .enumerate()
            .map(|(offset, column)| {
                let seed = initial.map(|initial| {
                    let first = (window_index * window + offset) * solver.n_dofs();
                    &initial[first..first + solver.n_dofs()]
                });
                solver
                    .solve_controlled(
                        column,
                        options,
                        within::SolveControl {
                            initial_coefficients: seed,
                            escalation: super::lsmr_break::policy(
                                super::DeveloperLsmrBackend::Within,
                            ),
                            cancellation: within::Cancellation::new(
                                &crate::interrupt::cancellation_requested,
                            ),
                            residual_output: within::ResidualOutput::PreparedModel,
                        },
                    )
                    .map(|result| result.solution)
                    .map_err(|failure| {
                        if matches!(failure.source, within::SolveError::Cancelled) {
                            super::lsmr_break::record_cancelled(
                                super::DeveloperLsmrBackend::Within,
                            );
                        }
                        BatchError::Attempt {
                            source: failure.source,
                            iterations: failure.iterations,
                        }
                    })
            })
            .collect_into_vec(&mut results);
        for solved in joined_solutions(results, &mut spent)? {
            append_solved(
                &mut batch,
                solved,
                rhs.len(),
                solver.n_obs(),
                solver.n_dofs(),
            )?;
        }
    }
    let mut batch = batch.expect("nonempty RHS batch");
    batch.time_total = started.elapsed().as_secs_f64();
    Ok(batch)
}

/// Admit a fully joined window only after accounting for all attempted work
/// and selecting the highest-priority failure, with RHS order breaking ties.
fn joined_solutions(
    results: Vec<Result<within::SolveResult, BatchError>>,
    spent: &mut usize,
) -> Result<impl Iterator<Item = within::SolveResult>, BatchError> {
    *spent = results.iter().fold(*spent, |spent, result| {
        spent.max(match result {
            Ok(solved) => solved.iterations,
            Err(error) => error.attempted_iterations(),
        })
    });
    if crate::interrupt::cancellation_requested() {
        return Err(BatchError::Attempt {
            source: within::SolveError::Cancelled,
            iterations: *spent,
        });
    }
    let failure_index = results
        .iter()
        .enumerate()
        .filter_map(|(index, result)| result.as_ref().err().map(|e| (e.failure_rank(), index)))
        .min()
        .map(|(_, index)| index);
    if let Some(index) = failure_index {
        let error = results.into_iter().nth(index).unwrap().unwrap_err();
        return Err(match error {
            BatchError::Attempt { source, .. } => BatchError::Attempt {
                source,
                iterations: *spent,
            },
            error => error,
        });
    }
    Ok(results
        .into_iter()
        .map(|solved| solved.expect("joined batch errors checked")))
}

/// Append an independently solved RHS in caller order. Only the first RHS
/// contributes shared layout, warnings and unidentified-coordinate metadata.
pub(super) fn append_solved(
    batch: &mut Option<BatchSolveResult>,
    mut solved: within::SolveResult,
    rhs: usize,
    rows: usize,
    dofs: usize,
) -> Result<(), BatchError> {
    if batch.is_none() {
        let mut x = Vec::new();
        let mut demeaned = Vec::new();
        reserve(&mut x, cells(dofs, rhs)?)?;
        reserve(&mut demeaned, cells(rows, rhs)?)?;
        let mut converged = Vec::new();
        let mut iterations = Vec::new();
        let mut residual = Vec::new();
        let mut time_solve = Vec::new();
        reserve(&mut converged, rhs)?;
        reserve(&mut iterations, rhs)?;
        reserve(&mut residual, rhs)?;
        reserve(&mut time_solve, rhs)?;
        *batch = Some(BatchSolveResult {
            x,
            demeaned,
            unidentified: std::mem::take(&mut solved.unidentified),
            warnings: std::mem::take(&mut solved.warnings),
            layout: solved.layout,
            converged,
            iterations,
            residual,
            time_solve,
            time_setup: 0.0,
            time_total: 0.0,
            n_dofs: dofs,
            n_obs: rows,
        });
    }
    let batch = batch.as_mut().expect("batch initialized");
    batch.x.append(&mut solved.x);
    batch.demeaned.append(&mut solved.demeaned);
    batch.converged.push(solved.converged);
    batch.iterations.push(solved.iterations);
    batch.residual.push(solved.residual);
    batch.time_solve.push(solved.time_solve);
    batch.time_setup += solved.time_setup;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_transport_preserves_other_columns_and_bits_at_tile_edges() {
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    for rows in [1, 31, 32, 33, 257] {
                        for width in [3, 8, 17, 67] {
                            let mut values = DenseMatrix::from_row_major(
                                rows,
                                width,
                                (0..rows * width)
                                    .map(|i| {
                                        f64::from_bits((i as u64).wrapping_mul(0x9e3779b97f4a7c15))
                                    })
                                    .collect(),
                            )
                            .unwrap();
                            let original = values.as_slice().to_vec();
                            let positions = [width - 1, 0, width / 2];
                            let mut rhs = selected_rhs_columns(&values, &positions).unwrap();
                            for (j, column) in rhs.iter_mut().enumerate() {
                                for (row, value) in column.iter_mut().enumerate() {
                                    assert_eq!(
                                        value.to_bits(),
                                        original[row * width + positions[j]].to_bits()
                                    );
                                    *value = f64::from_bits(value.to_bits() ^ 0x8000000000000000);
                                }
                            }
                            copy_selected_demeaned(&rhs.concat(), &mut values, &positions);
                            for row in 0..rows {
                                for column in 0..width {
                                    let expected = original[row * width + column].to_bits()
                                        ^ if positions.contains(&column) {
                                            0x8000000000000000
                                        } else {
                                            0
                                        };
                                    assert_eq!(values[(row, column)].to_bits(), expected);
                                }
                            }
                        }
                    }
                });
        }
    }

    #[test]
    fn narrow_transport_uses_row_blocks_with_identical_bits() {
        for (rows, width) in [(50_003, 1), (50_003, 3), (65_536, 6), (40_001, 9)] {
            let values = DenseMatrix::from_row_major(
                rows,
                width,
                (0..rows * width)
                    .map(|i| f64::from_bits((i as u64).wrapping_mul(0x9e3779b97f4a7c15)))
                    .collect(),
            )
            .unwrap();
            let expected: Vec<Vec<u64>> = (0..width)
                .map(|c| (0..rows).map(|r| values[(r, c)].to_bits()).collect())
                .collect();
            for threads in [1, 3, 4, 16] {
                let columns = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| rhs_columns(&values).unwrap());
                let bits: Vec<Vec<u64>> = columns
                    .iter()
                    .map(|c| c.iter().map(|v| v.to_bits()).collect())
                    .collect();
                assert_eq!(
                    bits, expected,
                    "rows {rows}, width {width}, threads {threads}"
                );
                let positions: Vec<_> = (0..width).rev().collect();
                let selected = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| selected_rhs_columns(&values, &positions).unwrap());
                for (j, column) in selected.iter().enumerate() {
                    assert!(column
                        .iter()
                        .map(|v| v.to_bits())
                        .eq(expected[positions[j]].iter().copied()));
                }
            }
        }
    }

    #[test]
    fn transport_preserves_bits_at_tile_edges_and_across_threads() {
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    for rows in [0, 1, 31, 32, 33, 257] {
                        for width in [0, 1, 7, 8, 9, 52, 67] {
                            let patterns = [
                                0,
                                1 << 63,
                                1,
                                u64::MAX,
                                f64::INFINITY.to_bits(),
                                1.25_f64.to_bits(),
                            ];
                            let bits: Vec<u64> = (0..rows * width)
                                .map(|i| {
                                    patterns[i % patterns.len()]
                                        ^ (i as u64 / patterns.len() as u64)
                                })
                                .collect();
                            let source = DenseMatrix::from_row_major(
                                rows,
                                width,
                                bits.iter().copied().map(f64::from_bits).collect(),
                            )
                            .unwrap();
                            let columns = rhs_columns(&source).unwrap();
                            assert_eq!(columns.len(), width);
                            for (column, values) in columns.iter().enumerate() {
                                assert_eq!(values.len(), rows);
                                for (row, value) in values.iter().enumerate() {
                                    assert_eq!(value.to_bits(), bits[row * width + column]);
                                }
                            }
                            let bank: Vec<f64> = columns.into_iter().flatten().collect();
                            let mut restored = DenseMatrix::zeros(rows, width).unwrap();
                            copy_demeaned(&bank, &mut restored);
                            assert_eq!(
                                restored
                                    .as_slice()
                                    .iter()
                                    .map(|x| x.to_bits())
                                    .collect::<Vec<_>>(),
                                bits
                            );
                        }
                    }
                });
        }
    }

    fn assert_categorical_model_output(
        result: &BatchSolveResult,
        rhs: &[&[f64]],
        left: &[u32],
        right: &[u32],
        left_levels: usize,
    ) {
        // The default upstream batch exposes weighted-audit output. Fereg
        // explicitly reconstructs the unweighted model instead. With these
        // two categorical terms, public coefficients give an independent
        // exact output witness while full-batch solve metadata stays exact.
        assert_eq!(result.n_obs, left.len());
        assert_eq!(result.converged.len(), rhs.len());
        assert_eq!(result.demeaned.len(), rhs.len() * result.n_obs);
        for (column, original) in rhs.iter().enumerate() {
            let coefficients = result.x(column);
            for (row, &y) in original.iter().enumerate() {
                let fitted = coefficients[left[row] as usize]
                    + coefficients[left_levels + right[row] as usize];
                assert_eq!(
                    result.demeaned(column)[row].to_bits(),
                    (y - fitted).to_bits(),
                    "column {column}, row {row}"
                );
            }
        }
    }

    #[test]
    fn windows_preserve_upstream_solves_and_prepared_outputs() {
        let rows = 1024;
        let left: Vec<u32> = (0..rows).map(|row| (row % 32) as u32).collect();
        let right: Vec<u32> = (0..rows).map(|row| ((row / 7) % 27) as u32).collect();
        let weights: Vec<f64> = (0..rows)
            .map(|row| 0.5 + (row % 19) as f64 / 19.0)
            .collect();
        let columns: Vec<Vec<f64>> = (0..67)
            .map(|column| {
                (0..rows)
                    .map(|row| ((row * 17 + column * 31) as f64 * 0.0031).sin())
                    .collect()
            })
            .collect();
        let rhs: Vec<&[f64]> = columns.iter().map(Vec::as_slice).collect();
        let options = LsmrOptions {
            tol: 1e-10,
            maxiter: 500,
            local_size: Some(4),
        };
        for threads in [1, 3, 4, 9] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    for preconditioner in [
                        PreconditionerConfig::Off,
                        PreconditionerConfig::Diagonal,
                        crate::fe::additive_schwarz(),
                    ] {
                        let solver = Solver::new(
                            vec![
                                Effect::new(&left, true, []).unwrap(),
                                Effect::new(&right, true, []).unwrap(),
                            ],
                            Some(weights.as_slice()),
                            preconditioner,
                        )
                        .unwrap();
                        let oracle = solver.solve_batch(&rhs, &options).unwrap();
                        assert!(oracle.converged.iter().all(|&converged| converged));
                        let result = solve_prepared_batch(&solver, &rhs, &options).unwrap();
                        assert_eq!(result.x, oracle.x);
                        assert_categorical_model_output(&result, &rhs, &left, &right, 32);
                        assert_eq!(result.unidentified, oracle.unidentified);
                        assert_eq!(result.converged, oracle.converged);
                        assert_eq!(result.iterations, oracle.iterations);
                        assert_eq!(result.residual, oracle.residual);
                        assert_eq!((result.n_dofs, result.n_obs), (oracle.n_dofs, oracle.n_obs));
                    }
                });
        }
    }

    #[test]
    fn narrow_batch_adapters_keep_prepared_outputs_and_fail_atomically() {
        let rows = 512;
        let left: Vec<u32> = (0..rows).map(|r| (r % 32) as u32).collect();
        let right: Vec<u32> = (0..rows).map(|r| ((r / 7) % 19) as u32).collect();
        let weights: Vec<f64> = (0..rows).map(|r| 0.5 + (r % 13) as f64 / 13.0).collect();
        let columns: Vec<Vec<f64>> = (0..64)
            .map(|c| {
                (0..rows)
                    .map(|r| ((r * 17 + c * 31) as f64 * 0.0031).sin())
                    .collect()
            })
            .collect();
        let views: Vec<&[f64]> = columns.iter().map(Vec::as_slice).collect();
        let options = LsmrOptions {
            tol: 1e-10,
            maxiter: 500,
            local_size: Some(4),
        };
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    for weighted in [false, true] {
                        for preconditioner in [
                            PreconditionerConfig::Off,
                            PreconditionerConfig::Diagonal,
                            crate::fe::additive_schwarz(),
                        ] {
                            let effects = || {
                                vec![
                                    Effect::new(&left, true, []).unwrap(),
                                    Effect::new(&right, true, []).unwrap(),
                                ]
                            };
                            let w = weighted.then_some(weights.as_slice());
                            let solver = Solver::new(effects(), w, preconditioner.clone()).unwrap();
                            for width in [16, 52, 64] {
                                let rhs = &views[..width];
                                let expected = solver.solve_batch(rhs, &options).unwrap();
                                assert!(expected.converged.iter().all(|&v| v));
                                for actual in [
                                    solve_prepared_batch(&solver, rhs, &options).unwrap(),
                                    solve_batch(
                                        effects(),
                                        rhs,
                                        w,
                                        &options,
                                        preconditioner.clone(),
                                        false,
                                    )
                                    .unwrap(),
                                ] {
                                    assert_eq!(actual.x, expected.x);
                                    assert_categorical_model_output(
                                        &actual, rhs, &left, &right, 32,
                                    );
                                    assert_eq!(actual.unidentified, expected.unidentified);
                                    assert_eq!(actual.converged, expected.converged);
                                    assert_eq!(actual.iterations, expected.iterations);
                                    assert_eq!(actual.residual, expected.residual);
                                    assert_eq!(
                                        (actual.n_dofs, actual.n_obs),
                                        (expected.n_dofs, expected.n_obs)
                                    );
                                }
                            }
                            if threads == 1 {
                                let mut malformed = views[..16].to_vec();
                                malformed[15] = &columns[15][..rows - 1];
                                for attempt in [
                                    solve_prepared_batch(&solver, &malformed, &options),
                                    solve_batch(
                                        effects(),
                                        &malformed,
                                        w,
                                        &options,
                                        preconditioner.clone(),
                                        false,
                                    ),
                                ] {
                                    assert!(matches!(attempt, Err(BatchError::Attempt { .. })));
                                }
                            }
                        }
                    }
                });
        }
    }

    #[test]
    fn a_later_window_failure_returns_no_partial_batch() {
        let levels = vec![0_u32, 0, 1, 1];
        let solver = Solver::new(
            vec![Effect::new(&levels, true, []).unwrap()],
            None,
            PreconditionerConfig::Off,
        )
        .unwrap();
        let rhs = [1.0, 2.0, 3.0, 4.0];
        let completed_iterations = solver
            .solve(&rhs, &LsmrOptions::default())
            .unwrap()
            .iterations;
        assert!(completed_iterations > 0);
        let mut batch: Vec<&[f64]> = vec![&rhs; 65];
        batch[64] = &rhs[..3];
        let error = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap()
            .install(|| solve_prepared_batch(&solver, &batch, &LsmrOptions::default()))
            .unwrap_err();
        assert!(matches!(&error, BatchError::Attempt { .. }));
        assert!(!error.preconditioner_breakdown());
        assert_eq!(error.attempted_iterations(), completed_iterations);
        assert_eq!(error.into_fereg("test failure").code, ErrorCode::Internal);
    }

    #[test]
    fn cancellation_keeps_interrupted_classification_and_cannot_fallback() {
        for error in [
            BatchError::Upstream(WithinError::Build(within::BuildError::Cancelled)),
            BatchError::Upstream(WithinError::Solve(within::SolveError::Cancelled)),
            BatchError::Attempt {
                source: within::SolveError::Cancelled,
                iterations: 3,
            },
        ] {
            assert!(!error.preconditioner_breakdown());
            assert_eq!(
                error.into_fereg("test cancellation").code,
                ErrorCode::Interrupted
            );
        }
    }

    #[test]
    fn late_cancellation_prevents_a_numerical_fallback() {
        const CHILD_ENV: &str = "FEREG_LSMR_BATCH_LATE_CANCELLATION_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD_ENV, "1")
                .args([
                    "fe::lsmr_batch::tests::late_cancellation_prevents_a_numerical_fallback",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let failure = BatchError::Attempt {
            source: within::SolveError::InvalidInput {
                context: "mlsmr",
                message: "preconditioner not positive definite".to_owned(),
            },
            iterations: 7,
        };
        assert!(failure.preconditioner_breakdown());
        crate::interrupt::request_cancellation();
        assert!(!failure.preconditioner_breakdown());
        assert_eq!(failure.attempted_iterations(), 7);
        assert_eq!(
            failure.into_fereg("late cancellation").code,
            ErrorCode::Interrupted
        );
        crate::interrupt::reset_cancellation();
    }

    #[test]
    fn joined_fatal_rhs_cannot_be_hidden_by_a_positivity_refusal() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Clone, Copy)]
        enum Outcome {
            Positivity,
            Malformed,
            Allocation,
            Cancelled,
            Success,
        }
        use Outcome::*;

        let levels = [0, 0, 1, 1];
        let rhs = [1.0, 2.0, 3.0, 4.0];
        let solver = Solver::new(
            vec![Effect::new(&levels, true, []).unwrap()],
            None,
            PreconditionerConfig::Off,
        )
        .unwrap();
        let options = LsmrOptions::default();
        let attempt = |outcome| match outcome {
            // Arbitration must not depend on a numerical fixture triggering a
            // roundoff-sensitive positivity refusal in this upstream revision.
            Positivity => Err(BatchError::Attempt {
                source: within::SolveError::InvalidInput {
                    context: "mlsmr",
                    message: "preconditioner not positive definite".to_owned(),
                },
                iterations: 7,
            }),
            Allocation => Err(BatchError::Attempt {
                source: within::SolveError::Allocation {
                    context: "joined test",
                },
                iterations: 3,
            }),
            Cancelled => Err(BatchError::Attempt {
                source: within::SolveError::Cancelled,
                iterations: 4,
            }),
            Malformed | Success => solver
                .solve_controlled(
                    if matches!(outcome, Malformed) {
                        &rhs[..3]
                    } else {
                        &rhs
                    },
                    &options,
                    within::SolveControl::default(),
                )
                .map(|result| result.solution)
                .map_err(|failure| BatchError::Attempt {
                    source: failure.source,
                    iterations: failure.iterations,
                }),
        };
        let source = |error: &BatchError| match error {
            BatchError::Attempt { source, .. } => source.to_string(),
            _ => panic!("expected counted solve error"),
        };
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    for (outcomes, expected) in [
                        (vec![Positivity, Success], Positivity),
                        (vec![Positivity, Allocation, Success], Allocation),
                        (vec![Positivity, Malformed, Allocation, Success], Malformed),
                        (
                            vec![Positivity, Malformed, Allocation, Cancelled, Success],
                            Cancelled,
                        ),
                    ] {
                        let expected_source = source(&attempt(expected).unwrap_err());
                        for reverse in [false, true] {
                            let mut outcomes = outcomes.clone();
                            if reverse {
                                outcomes.reverse();
                            }
                            for previous_spent in [0, 11] {
                                let completed = AtomicUsize::new(0);
                                let mut results = Vec::new();
                                outcomes
                                    .par_iter()
                                    .map(|&outcome| {
                                        let result = attempt(outcome);
                                        completed.fetch_add(1, Ordering::Relaxed);
                                        result
                                    })
                                    .collect_into_vec(&mut results);
                                assert_eq!(completed.load(Ordering::Relaxed), outcomes.len());
                                let mut spent = previous_spent;
                                let failure = joined_solutions(results, &mut spent)
                                    .err()
                                    .expect("joined window must fail atomically");
                                assert_eq!(source(&failure), expected_source);
                                assert_eq!(spent, previous_spent.max(7));
                                assert_eq!(failure.attempted_iterations(), spent);
                                assert_eq!(
                                    failure.preconditioner_breakdown(),
                                    matches!(expected, Positivity)
                                );
                            }
                        }
                    }
                });
        }
    }

    #[test]
    fn transport_capacity_errors_keep_allocation_classification() {
        let mut data = vec![1.0];
        let error = reserve(&mut data, usize::MAX).unwrap_err();
        assert_eq!(data, [1.0]);
        assert_eq!(
            error.into_fereg("test reservation").code,
            ErrorCode::Allocation
        );
        assert_eq!(
            cells(usize::MAX, 2)
                .unwrap_err()
                .into_fereg("test shape")
                .code,
            ErrorCode::Allocation
        );
    }
}
