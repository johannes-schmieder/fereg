//! Current-call two-partition projection and finite products for canonical PPML.
//! All weighted state dies with this call; only certified rows and one-use
//! moments cross into the canonical rank owner.
use super::prepared_scratch::PreparedOlsScratch;
use super::PreparedOlsColumnKind;
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
#[cfg(any(target_arch = "aarch64", target_feature = "fma"))]
use crate::fe::slope_coefficients::joint::normalized_product;
use crate::fe::{
    BorrowedInterceptFeDesign, BorrowedMapWeights, FeCertificate, FeSolverKind, MapConfig,
    SolveReport, SolveStopReason,
};
use crate::memory::SolverWorkspacePeak;
use crate::ols::prepared_fit_view::PreparedOlsFitView;
use rayon::prelude::*;
use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

mod three;
pub(crate) fn canonical_three_workspace_bytes(
    n: usize,
    width: usize,
    levels: [usize; 3],
    threads: usize,
) -> Option<u128> {
    three::workspace_bytes(n, width, levels, threads)
}

fn declined() -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        "canonical Schur products declined current-weight geometry",
    )
}
fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "unable to reserve admitted canonical Schur products",
    )
}

struct Budget {
    remaining: u128,
    limit: u128,
    #[cfg(test)]
    peak: u128,
}
impl Budget {
    fn new(limit: u128) -> Self {
        Self {
            remaining: limit,
            limit,
            #[cfg(test)]
            peak: 0,
        }
    }

    // Credit capacity only after ownership has ended and the allocation has
    // actually been released. Clearing a vector would not establish this bound.
    fn release<T>(&mut self, buffer: Vec<T>) -> Result<()> {
        let bytes = buffer.capacity() as u128 * size_of::<T>() as u128;
        drop(buffer);
        self.remaining = self
            .remaining
            .checked_add(bytes)
            .filter(|remaining| *remaining <= self.limit)
            .ok_or_else(allocation)?;
        Ok(())
    }

    // A cheap topology phase is admitted first. Its complete packed geometry
    // then fixes the final allowance before any weighted numeric bank exists.
    fn set_limit(&mut self, limit: u128) -> Result<()> {
        let live = self.limit - self.remaining;
        self.remaining = limit.checked_sub(live).ok_or_else(allocation)?;
        self.limit = limit;
        Ok(())
    }

    fn reserve<T: Clone>(&mut self, n: usize, value: T) -> Result<Vec<T>> {
        let mut result = Vec::new();
        let bytes = (n as u128)
            .checked_mul(size_of::<T>() as u128)
            .ok_or_else(allocation)?;
        if bytes > self.remaining {
            return Err(allocation());
        }
        result.try_reserve_exact(n).map_err(|_| allocation())?;
        self.remaining = self
            .remaining
            .checked_sub(result.capacity() as u128 * size_of::<T>() as u128)
            .ok_or_else(allocation)?;
        #[cfg(test)]
        {
            self.peak = self.peak.max(self.limit - self.remaining);
        }
        result.resize(n, value);
        Ok(result)
    }
}

#[derive(Clone, Copy, Default)]
struct ComponentFactor {
    start: usize,
    len: usize,
    offset: usize,
    matrix_offset: usize,
    level_start: usize,
}

#[derive(Clone, Copy)]
struct Plan {
    n: usize,
    p: usize,
    q: usize,
    width: usize,
    workers: usize,
    stride: usize,
    bytes: u128,
    factor_work_budget: u128,
}
impl Plan {
    fn new(
        design: &BorrowedInterceptFeDesign<'_>,
        width: usize,
        left: usize,
        threads: usize,
        prior_operator_work: u128,
    ) -> Option<Self> {
        let n = design.nobs();
        let p = design.effects()[left].n_levels();
        let q = design.effects()[1 - left].n_levels();
        if n == 0 || p == 0 || q == 0 || width < 2 || threads == 0 {
            return None;
        }
        let workers = threads.min(p);
        let square = width.checked_mul(width)?;
        let cross = q.checked_mul(width)?;
        let stride = square
            .checked_add(cross)?
            .checked_add(width.checked_mul(6)?)?;
        // Cold setup is bounded by a finite Gram. A later canonical call may
        // also spend the Schur operator work actually observed on its prior
        // completed projection. This predicts cost, not current-weight accuracy.
        // Complete weighted geometry and independent certification stay fresh.
        let mut work = 0_u128;
        for a in 0..p {
            let degree = design.effects()[left].level_index().members(a).len().min(q) as u128;
            work = work.checked_add(degree.checked_mul(degree.saturating_sub(1))? / 2)?;
        }
        let gram_work = (n as u128)
            .checked_mul(width as u128)?
            .checked_mul(width as u128 + 1)?
            / 2;
        let work_allowance = gram_work.checked_add(prior_operator_work)?;
        if work > work_allowance {
            return None;
        }
        let mut plan = Self {
            n,
            p,
            q,
            width,
            workers,
            stride,
            bytes: 0,
            factor_work_budget: work_allowance - work,
        };
        plan.bytes = plan.minimum_storage_bytes(0, 0)?;
        Some(plan)
    }

    fn topology_storage_bytes(&self) -> Option<u128> {
        // Before the complete component plan is known, only these two Q-sized
        // tables and the column bindings coexist with the old projection panel.
        // Future numeric/reduction banks are admitted after that panel can end.
        (self.q as u128)
            .checked_mul((size_of::<usize>() + size_of::<ComponentFactor>()) as u128)?
            .checked_add((self.width as u128).checked_mul(32)?)?
            .checked_add(2048)
    }

    fn output_tile_rows(&self) -> usize {
        // A bounded group bank keeps reconstruction independent of P*W. The
        // existing scratch encoder can use smaller tiles without new storage.
        (8192 / self.width).max(1).min(self.n)
    }

    fn minimum_storage_bytes(&self, matrix: usize, factor: usize) -> Option<u128> {
        Some(
            self.storage_bytes(matrix, factor, false)?
                .min(self.storage_bytes_for(matrix, factor, false, true)?),
        )
    }

    fn storage_bytes(&self, matrix: usize, factor: usize, retain_centers: bool) -> Option<u128> {
        self.storage_bytes_for(matrix, factor, retain_centers, false)
    }

    fn storage_bytes_for(
        &self,
        matrix: usize,
        factor: usize,
        retain_centers: bool,
        tile_output: bool,
    ) -> Option<u128> {
        let Self {
            n,
            p,
            q,
            width,
            workers,
            stride,
            ..
        } = *self;
        let square = width.checked_mul(width)?;
        let cross = q.checked_mul(width)?;
        let coefficients = p.checked_mul(width)?;
        let cached_coefficients = if retain_centers { coefficients } else { 0 };
        let reductions = workers.checked_mul(stride)?;
        // RHS columns own disjoint Q-vectors. After construction, a solved
        // coordinate bank of at most Q*W overlaps that single cross bank.
        let rhs_solve_overlap = cross;
        let words = |sizes: &[usize]| {
            sizes
                .iter()
                .try_fold(0_u128, |sum, &n| sum.checked_add(n as u128))?
                .checked_mul(size_of::<f64>() as u128)
        };
        let indices = |n: usize| (n as u128).checked_mul(size_of::<usize>() as u128);
        // Five Q-index arrays and Q five-index descriptors belong to setup.
        // Only free coordinates and descriptors survive through column solves.
        let setup =
            words(&[n, p, q, q, q, matrix, factor])?.checked_add(indices(q.checked_mul(10)?)?)?;
        let solve = words(&[
            n,
            p,
            q,
            q,
            factor,
            cached_coefficients,
            cross,
            width,
            width,
            rhs_solve_overlap,
        ])?
        .checked_add(indices(q.checked_mul(6)?)?)?;
        let certify = words(&[
            n,
            p,
            q,
            cached_coefficients,
            cross,
            width,
            width,
            width,
            square,
            reductions,
        ])?;
        // Full reconstruction overlaps P*W with no worker reductions. Tiled
        // reconstruction instead keeps immutable weights and a bounded group
        // bank until the retained row writer completes.
        let reconstruct = if tile_output {
            let groups = p.min(self.output_tile_rows());
            // Keep current weights/masses until the last row tile is written.
            // Response coefficients die before the tile map and bank exist.
            words(&[
                n,
                n,
                p,
                cross,
                width,
                width,
                width,
                square,
                groups.checked_mul(width)?,
            ])?
            .checked_add(indices(p.checked_add(groups)?)?)?
            .max(words(&[n, n, p, p, cross, width, width, width, square])?)
        } else {
            words(&[n, p, q, coefficients, cross, width, width, width, square])?
        };
        setup
            .max(solve)
            .max(certify)
            .max(reconstruct)
            .checked_add((width as u128).checked_mul(32)?)?
            .checked_add(2048)
    }
}

/// A generated column bound once, so the N x K loops avoid per-cell recipe
/// dispatch. Each variant returns exactly `fit_column_value`'s value.
enum Column<'a, P: ?Sized> {
    Numeric(&'a [f64]),
    /// Factor indicator `f64::from(codes[row] == level)`.
    Indicator(&'a [u32], u32),
    /// Intercept (one) or a factor column without a level (zero).
    Constant(f64),
    Generated(&'a P, usize),
}
impl<P: ?Sized> Clone for Column<'_, P> {
    fn clone(&self) -> Self {
        match self {
            Self::Numeric(v) => Self::Numeric(v),
            Self::Indicator(codes, level) => Self::Indicator(codes, *level),
            Self::Constant(value) => Self::Constant(*value),
            Self::Generated(p, j) => Self::Generated(p, *j),
        }
    }
}
impl<P: PreparedOlsFitView + ?Sized> Column<'_, P> {
    #[inline]
    fn value(&self, row: usize) -> Result<f64> {
        match self {
            Self::Numeric(x) => Ok(x[row]),
            Self::Indicator(codes, level) => Ok(f64::from(codes[row] == *level)),
            Self::Constant(value) => Ok(*value),
            Self::Generated(p, j) => p.fit_column_value(*j, row),
        }
    }
}

/// Gram of [response, all stored controls], each divided by its original norm.
/// This value is consumed by the same call's canonical rank solve, never cached.
pub(super) struct CanonicalMoments {
    pub width: usize,
    pub gram: Vec<f64>,
    pub response_scale: f64,
}

struct TiledCoefficients {
    weights: Vec<f64>,
    masses: Vec<f64>,
    slots: Vec<usize>,
    groups: Vec<usize>,
    values: Vec<f64>,
    rows: usize,
}

impl TiledCoefficients {
    #[allow(clippy::too_many_arguments)]
    fn prepare<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        columns: &[Column<'_, P>],
        design: &BorrowedInterceptFeDesign<'_>,
        left_effect: usize,
        scales: &[f64],
        right: &[f64],
        first: usize,
        count: usize,
    ) -> Result<()> {
        let a = &design.effects()[left_effect];
        let b = &design.effects()[1 - left_effect];
        let q = b.n_levels();
        let width = columns.len();
        self.slots.fill(usize::MAX);
        let mut groups = 0;
        for &g in &a.codes()[first..first + count] {
            let g = g as usize;
            if self.slots[g] == usize::MAX {
                self.slots[g] = groups;
                self.groups[groups] = g;
                groups += 1;
            }
        }
        self.values[..groups * width]
            .par_chunks_mut(width)
            .enumerate()
            .try_for_each(|(slot, values)| -> Result<()> {
                let g = self.groups[slot];
                for j in 1..width {
                    values[j] = group_coefficient(
                        &columns[j],
                        &self.weights,
                        nonzero(scales[j]),
                        self.masses[g],
                        a.level_index().members(g),
                        Some((&right[j * q..(j + 1) * q], b.codes())),
                    )?;
                }
                Ok(())
            })
    }
}

pub(super) struct Products<'a, P: PreparedOlsFitView + ?Sized> {
    columns: Vec<Column<'a, P>>,
    design: BorrowedInterceptFeDesign<'a>,
    left_effect: usize,
    left: Vec<f64>,
    tiled: Option<TiledCoefficients>,
    right: Vec<f64>,
    three: Option<three::Coefficients>,
    pub scales: Vec<f64>,
    pub means: Vec<f64>,
    pub norms: Vec<f64>,
    pub response: Vec<f64>,
    pub moments: CanonicalMoments,
    pub report: SolveReport,
    #[cfg(test)]
    minimum_workspace_bytes: u128,
    #[cfg(test)]
    retained_centers: bool,
    #[cfg(test)]
    implicit: bool,
}

impl<P: PreparedOlsFitView + Sync + ?Sized> Products<'_, P> {
    fn residual(&self, row: usize, j: usize) -> Result<f64> {
        if let Some(three) = &self.three {
            return three.residual(&self.design, &self.columns, &self.scales, row, j);
        }
        let a = &self.design.effects()[self.left_effect];
        let b = &self.design.effects()[1 - self.left_effect];
        let scale = nonzero(self.scales[j]);
        let group = a.codes()[row] as usize;
        let left = if let Some(tile) = &self.tiled {
            group_coefficient(
                &self.columns[j],
                &tile.weights,
                scale,
                tile.masses[group],
                a.level_index().members(group),
                Some((
                    &self.right[j * b.n_levels()..(j + 1) * b.n_levels()],
                    b.codes(),
                )),
            )?
        } else {
            self.left[j * a.n_levels() + group]
        };
        let value = (self.columns[j].value(row)? / scale
            - left
            - self.right[j * b.n_levels() + b.codes()[row] as usize])
            * scale;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(declined())
        }
    }

    pub fn write_controls(&mut self, storage: &mut PreparedOlsScratch) -> Result<()> {
        if storage.rows() != self.design.nobs() || storage.columns() + 1 != self.columns.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "canonical product storage is misaligned",
            ));
        }
        let Some(tile) = self.tiled.as_mut() else {
            return storage.rewrite_row_major_with(|row, column| self.residual(row, column + 1));
        };
        let a = &self.design.effects()[self.left_effect];
        let b = &self.design.effects()[1 - self.left_effect];
        let q = b.n_levels();
        let width = self.columns.len();
        let rows = tile.rows;
        storage.rewrite_row_major_prepared_with(
            tile,
            rows,
            |tile, first, count| {
                tile.prepare(
                    &self.columns,
                    &self.design,
                    self.left_effect,
                    &self.scales,
                    &self.right,
                    first,
                    count,
                )
            },
            |tile, row, column| {
                let j = column + 1;
                let scale = nonzero(self.scales[j]);
                let slot = tile.slots[a.codes()[row] as usize];
                Ok((self.columns[j].value(row)? / scale
                    - tile.values[slot * width + j]
                    - self.right[j * q + b.codes()[row] as usize])
                    * scale)
            },
        )
    }

    pub fn write_dense(&mut self, storage: &mut DenseMatrix) -> Result<()> {
        let columns = self.columns.len() - 1;
        if storage.nrows() != self.design.nobs() || storage.ncols() != columns {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "canonical dense product storage is misaligned",
            ));
        }
        if columns == 0 {
            return Ok(());
        }
        let Some(tile) = self.tiled.as_mut() else {
            return storage
                .as_mut_slice()
                .par_chunks_mut(columns)
                .enumerate()
                .try_for_each(|(row, values)| {
                    for (column, value) in values.iter_mut().enumerate() {
                        *value = self.residual(row, column + 1)?;
                    }
                    Ok(())
                });
        };
        let a = &self.design.effects()[self.left_effect];
        let b = &self.design.effects()[1 - self.left_effect];
        let q = b.n_levels();
        let width = self.columns.len();
        let rows = tile.rows;
        // Streamed centers are rebuilt once per group in an output tile, not
        // once for every observation that belongs to that group.
        for (index, values) in storage
            .as_mut_slice()
            .chunks_mut(rows * columns)
            .enumerate()
        {
            let first = index * rows;
            tile.prepare(
                &self.columns,
                &self.design,
                self.left_effect,
                &self.scales,
                &self.right,
                first,
                values.len() / columns,
            )?;
            values.par_chunks_mut(columns).enumerate().try_for_each(
                |(i, values)| -> Result<()> {
                    let row = first + i;
                    let slot = tile.slots[a.codes()[row] as usize];
                    for (column, value) in values.iter_mut().enumerate() {
                        let j = column + 1;
                        let scale = nonzero(self.scales[j]);
                        *value = (self.columns[j].value(row)? / scale
                            - tile.values[slot * width + j]
                            - self.right[j * q + b.codes()[row] as usize])
                            * scale;
                        if !value.is_finite() {
                            return Err(declined());
                        }
                    }
                    Ok(())
                },
            )?;
        }
        Ok(())
    }
}

fn nonzero(x: f64) -> f64 {
    if x == 0. {
        1.
    } else {
        x
    }
}
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_build<'a, P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &'a P,
    generated: &[usize],
    response: &'a [f64],
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    config: MapConfig,
    allowance: u128,
    prior_operator_work: u128,
    obsolete_projection: &mut Vec<f64>,
) -> Result<Option<Products<'a, P>>> {
    if config.max_iterations == 0
        || config.check_every == 0
        || !config.tolerance.is_finite()
        || config.tolerance <= 0.
        || config.tolerance >= 1.
        || !divisor.is_finite()
        || divisor <= 0.
    {
        return Ok(None);
    }
    let Some(design) = prepared.fit_borrowed_intercept_map_design()? else {
        return Ok(None);
    };
    if design.effects().len() == 3 && design.nobs() == response.len() {
        return three::try_build(
            prepared,
            generated,
            response,
            weights,
            divisor,
            config,
            allowance,
            obsolete_projection,
            design,
        );
    }
    if design.effects().len() != 2 || design.nobs() != response.len() {
        return Ok(None);
    }
    let left = usize::from(design.effects()[1].n_levels() > design.effects()[0].n_levels());
    let Some(plan) = Plan::new(
        &design,
        generated.len() + 1,
        left,
        rayon::current_num_threads(),
        prior_operator_work,
    ) else {
        return Ok(None);
    };
    let available = allowance.saturating_sub(obsolete_projection.capacity() as u128 * 8);
    if plan.bytes > allowance
        || plan
            .topology_storage_bytes()
            .is_none_or(|bytes| bytes > available)
    {
        return Ok(None);
    }
    match build(
        prepared,
        generated,
        response,
        weights,
        divisor,
        config,
        design,
        left,
        plan,
        allowance,
        obsolete_projection,
    ) {
        Ok(result) => Ok(Some(result)),
        Err(error) if error.code == ErrorCode::NonConvergence => Ok(None),
        Err(error) => Err(error),
    }
}

#[allow(clippy::too_many_arguments)]
fn build<'a, P: PreparedOlsFitView + Sync + ?Sized>(
    prepared: &'a P,
    generated: &[usize],
    response: &'a [f64],
    weights: BorrowedMapWeights<'_>,
    divisor: f64,
    config: MapConfig,
    design: BorrowedInterceptFeDesign<'a>,
    left_effect: usize,
    plan: Plan,
    allowance: u128,
    obsolete_projection: &mut Vec<f64>,
) -> Result<Products<'a, P>> {
    let started = Instant::now();
    let Plan {
        n,
        p,
        q,
        width,
        workers,
        stride,
        factor_work_budget,
        ..
    } = plan;
    let a = &design.effects()[left_effect];
    let b = &design.effects()[1 - left_effect];
    let initial_bytes = plan.topology_storage_bytes().ok_or_else(allocation)?;
    let mut budget = Budget::new(initial_bytes - 2048 - (width as u128) * 32);
    let mut columns = Vec::new();
    columns.try_reserve_exact(width).map_err(|_| allocation())?;
    if columns.capacity() as u128 * size_of::<Column<'_, P>>() as u128 > width as u128 * 32 {
        return Err(allocation());
    }
    columns.push(Column::Numeric(response));
    for &j in generated {
        let recipe = prepared.fit_model().columns.get(j).ok_or_else(declined)?;
        match &recipe.kind {
            PreparedOlsColumnKind::Continuous {
                prepared_source_index,
                ..
            } => {
                let values = prepared
                    .fit_sample()
                    .regressors
                    .get(*prepared_source_index)
                    .ok_or_else(declined)?;
                if values.len() != n {
                    return Err(declined());
                }
                columns.push(Column::Numeric(values));
            }
            PreparedOlsColumnKind::Intercept => columns.push(Column::Constant(1.0)),
            PreparedOlsColumnKind::Categorical {
                level_code: None, ..
            } => columns.push(Column::Constant(0.0)),
            PreparedOlsColumnKind::Categorical {
                categorical_source_index,
                level_code: Some(level),
                ..
            } => {
                let codes = prepared.fit_categorical_codes(*categorical_source_index)?;
                if codes.len() != n {
                    return Err(declined());
                }
                columns.push(Column::Indicator(codes, *level));
            }
            PreparedOlsColumnKind::CategoricalInteraction { .. } => {
                columns.push(Column::Generated(prepared, j));
            }
        }
    }
    let mut parent = budget.reserve(q, 0_usize)?;
    for (h, value) in parent.iter_mut().enumerate() {
        *value = h;
    }
    for g in 0..p {
        let (&first, rest) = a
            .level_index()
            .members(g)
            .split_first()
            .ok_or_else(declined)?;
        for &i in rest {
            let first_root = root(&mut parent, b.codes()[first] as usize);
            let next_root = root(&mut parent, b.codes()[i] as usize);
            parent[next_root.max(first_root)] = next_root.min(first_root);
        }
    }
    let mut components = budget.reserve(q, ComponentFactor::default())?;
    for h in 0..q {
        let r = root(&mut parent, h);
        parent[h] = r;
        components[r].len += 1;
    }
    let (mut d, mut factor_cells, mut matrix_cells, mut level_start) =
        (0_usize, 0_usize, 0_usize, 0_usize);
    let mut factor_work = 0_u128;
    for (r, block) in components.iter_mut().enumerate() {
        if parent[r] != r {
            continue;
        }
        let levels = block.len;
        block.len -= 1;
        block.start = d;
        block.offset = factor_cells;
        block.matrix_offset = matrix_cells;
        block.level_start = level_start;
        d = d.checked_add(block.len).ok_or_else(declined)?;
        factor_cells = factor_cells
            .checked_add(block.len.checked_mul(block.len).ok_or_else(declined)?)
            .ok_or_else(declined)?;
        matrix_cells = matrix_cells
            .checked_add(levels.checked_mul(levels).ok_or_else(declined)?)
            .ok_or_else(declined)?;
        level_start = level_start.checked_add(levels).ok_or_else(declined)?;
        let levels = levels as u128;
        factor_work = factor_work
            .checked_add(
                levels
                    .checked_mul(levels)
                    .and_then(|x| x.checked_mul(levels))
                    .ok_or_else(declined)?
                    / 3,
            )
            .ok_or_else(declined)?;
    }
    // A reduced matrix can be inexpensive to apply even when its cubic
    // factorization exceeds the current-call work allowance. The same fused
    // original-input certificate decides acceptance of either proposal.
    let dense_iterative = factor_work > factor_work_budget;
    let dense_krylov = if dense_iterative {
        d.checked_mul(width)
            .and_then(|x| x.checked_mul(3))
            .ok_or_else(declined)?
    } else {
        0
    };
    let dense_charge = factor_cells
        .checked_add(dense_krylov)
        .ok_or_else(declined)?;
    // Dense geometry already affordable under the existing complete phase plan
    // retains its arithmetic. Implicit geometry admits the full certified
    // product pipeline where dense storage would otherwise force repeated
    // observation-space construction, not merely a different dense matvec.
    let implicit = plan
        .minimum_storage_bytes(matrix_cells, dense_charge)
        .is_none_or(|bytes| bytes > allowance);
    let iterative = implicit || dense_iterative;
    let krylov_cells = if iterative {
        d.checked_mul(width)
            .and_then(|x| x.checked_mul(3))
            .ok_or_else(declined)?
    } else {
        0
    };
    let matrix_cells = if implicit { q } else { matrix_cells };
    let factor_cells = if implicit {
        n.checked_add(q).ok_or_else(declined)?
    } else {
        factor_cells
    };
    let incidence_indices = if implicit {
        (n as u128)
            .checked_mul(size_of::<usize>() as u128)
            .and_then(|x| x.checked_add((p as u128) * size_of::<IncidenceRow>() as u128))
            .ok_or_else(declined)?
            .div_ceil(8)
    } else {
        0
    };
    let incidence_words = usize::try_from(incidence_indices).map_err(|_| declined())?;
    let factor_charge = factor_cells
        .checked_add(krylov_cells)
        .and_then(|x| x.checked_add(incidence_words))
        .ok_or_else(declined)?;
    let minimum_bytes = plan
        .minimum_storage_bytes(matrix_cells, factor_charge)
        .ok_or_else(declined)?;
    if minimum_bytes > allowance {
        return Err(declined());
    }
    // Retention avoids repeated original-column passes when the complete
    // coexisting phase banks fit. Streaming is the same solve under tighter
    // workspace, not a different numerical route or a threshold by width.
    let (retain_centers, tile_output, bytes) =
        match plan.storage_bytes(matrix_cells, factor_charge, true) {
            Some(bytes) if bytes <= allowance => (true, false, bytes),
            _ => match plan.storage_bytes(matrix_cells, factor_charge, false) {
                Some(bytes) if bytes <= allowance => (false, false, bytes),
                _ => (false, true, minimum_bytes),
            },
        };
    // The complete topology/work/memory plan is now known. Reclaim the old
    // temporary panel only if the new phase needs its reservation; numerical
    // refusal can still rebuild that panel through the ordinary fallible path.
    if bytes > allowance.saturating_sub(obsolete_projection.capacity() as u128 * 8) {
        drop(std::mem::take(obsolete_projection));
    }
    budget.set_limit(bytes - 2048 - width as u128 * 32)?;
    let mut local = budget.reserve(q, 0_usize)?;
    let mut anchors = budget.reserve(q, 0_usize)?;
    for h in 0..q {
        let r = parent[h];
        local[h] = anchors[r];
        anchors[r] += 1;
    }
    anchors.fill(usize::MAX);
    let mut w = budget.reserve(n, 0.)?;
    let mut mass_a = budget.reserve(p, 0.)?;
    let mut mass_b = budget.reserve(q, 0.)?;
    if let BorrowedMapWeights::Provided(values) = weights {
        if values.len() != n {
            return Err(declined());
        }
    }
    for i in 0..n {
        w[i] = match weights {
            BorrowedMapWeights::Unit => 1. / divisor,
            BorrowedMapWeights::Provided(v) => v[i] / divisor,
        };
        if !w[i].is_finite() || w[i] <= 0. {
            return Err(declined());
        }
        mass_a[a.codes()[i] as usize] += w[i];
        mass_b[b.codes()[i] as usize] += w[i];
    }
    if mass_a
        .iter()
        .chain(&mass_b)
        .any(|x| !x.is_finite() || *x <= 0.)
    {
        return Err(declined());
    }
    let mut matrix = budget.reserve(matrix_cells, 0.)?;
    let mut totals = budget.reserve(q, 0.)?;
    let mut touched = budget.reserve(q, 0_usize)?;
    // Keep the legacy matrix indices and numerical traversal order explicit.
    #[allow(clippy::needless_range_loop)]
    for g in 0..p {
        let mut count = 0;
        for &i in a.level_index().members(g) {
            let h = b.codes()[i] as usize;
            if totals[h] == 0. {
                touched[count] = h;
                count += 1;
            }
            totals[h] += w[i];
        }
        touched[..count].sort_unstable();
        if count == 0 {
            return Err(declined());
        }
        if implicit {
            // Prefix/suffix sums form each Schur diagonal without subtracting
            // nearly equal group masses. The diagonal is a preconditioner;
            // acceptance still uses independently reconstructed original rows.
            let mut prefix = 0.;
            for &h in &touched[..count] {
                matrix[h] += (totals[h] / mass_a[g]) * prefix;
                prefix += totals[h];
            }
            let mut suffix = 0.;
            for &h in touched[..count].iter().rev() {
                matrix[h] += (totals[h] / mass_a[g]) * suffix;
                suffix += totals[h];
            }
        } else {
            for (position, &h) in touched[..count].iter().enumerate() {
                let block = components[parent[h]];
                let size = block.len + 1;
                let row = local[h];
                for &j in &touched[position + 1..count] {
                    let value = (totals[h].max(totals[j]) / mass_a[g]) * totals[h].min(totals[j]);
                    if !value.is_finite() || value <= 0. {
                        return Err(declined());
                    }
                    let column = local[j];
                    matrix[block.matrix_offset + row * size + column] -= value;
                    matrix[block.matrix_offset + column * size + row] -= value;
                    matrix[block.matrix_offset + row * size + row] += value;
                    matrix[block.matrix_offset + column * size + column] += value;
                }
            }
        }
        for &h in &touched[..count] {
            totals[h] = 0.;
        }
    }
    for h in 0..q {
        let r = parent[h];
        let old = anchors[r];
        if old == usize::MAX || mass_b[h] > mass_b[old] {
            anchors[r] = h;
        }
    }
    let mut free = budget.reserve(q, usize::MAX)?;
    let mut roots = budget.reserve(q, 0.)?;
    // Assembly no longer needs its neighbour list. Reuse that allocation for
    // component membership so matrix packing never scans cross-component cells.
    for h in 0..q {
        let r = parent[h];
        let block = components[r];
        let size = block.len + 1;
        touched[block.level_start + local[h]] = h;
        roots[h] = if implicit {
            matrix[h]
        } else {
            matrix[block.matrix_offset + local[h] * size + local[h]]
        }
        .sqrt();
        if anchors[r] != h {
            if !roots[h].is_finite() || roots[h] <= 0. {
                return Err(declined());
            }
            free[h] = block.start + local[h] - usize::from(local[h] > local[anchors[r]]);
        }
    }
    let mut factor = budget.reserve(factor_cells, 0.)?;
    let mut incidence_ids = budget.reserve(if implicit { n } else { 0 }, 0_usize)?;
    let mut incidence_rows =
        budget.reserve(if implicit { p } else { 0 }, IncidenceRow::default())?;
    if implicit {
        let mut edge = 0;
        for g in 0..p {
            let members = a.level_index().members(g);
            let component = parent[b.codes()[members[0]] as usize];
            let mut count = 0;
            for &i in members {
                let h = b.codes()[i] as usize;
                if totals[h] == 0. {
                    touched[count] = h;
                    count += 1;
                }
                totals[h] += w[i];
            }
            touched[..count].sort_unstable();
            let start = edge;
            let mass_root = mass_a[g].sqrt();
            for &h in &touched[..count] {
                if free[h] != usize::MAX {
                    incidence_ids[edge] = free[h] - components[component].start;
                    factor[edge] = (totals[h] / mass_root) / roots[h];
                    if !factor[edge].is_finite() {
                        return Err(declined());
                    }
                    edge += 1;
                }
                totals[h] = 0.;
            }
            incidence_rows[g] = IncidenceRow {
                start,
                end: edge,
                component,
            };
        }
        incidence_rows.sort_unstable_by_key(|row| (row.component, row.start));
        for h in 0..q {
            if free[h] != usize::MAX {
                factor[n + free[h]] = (mass_b[h] / roots[h]) / roots[h];
                if !factor[n + free[h]].is_finite() {
                    return Err(declined());
                }
            }
        }
        for (root, block) in components.iter_mut().enumerate() {
            block.matrix_offset = root;
        }
    } else {
        for h in 0..q {
            if free[h] != usize::MAX {
                let block = components[parent[h]];
                let size = block.len + 1;
                for &j in &touched[block.level_start..block.level_start + size] {
                    if free[j] != usize::MAX {
                        factor[block.offset + (free[h] - block.start) * block.len + free[j]
                            - block.start] =
                            (matrix[block.matrix_offset + local[h] * size + local[j]] / roots[h])
                                / roots[j];
                    }
                }
            }
        }
    }
    components.retain(|block| block.len != 0);
    // Direct proposals retain the existing global pivot guard. Iterative
    // proposals require positive curvature and the same fresh FE certificate;
    // neither reduced solve establishes canonical finite-rank authority.
    let guard = 128. * f64::EPSILON * d.max(1) as f64;
    for block in &components {
        if iterative {
            break;
        }
        let size = block.len;
        let factor = &mut factor[block.offset..block.offset + size * size];
        for j in 0..size {
            let mut diagonal = factor[j * size + j];
            for k in 0..j {
                diagonal -= factor[j * size + k].powi(2);
            }
            if !diagonal.is_finite() || diagonal <= guard {
                return Err(declined());
            }
            factor[j * size + j] = diagonal.sqrt();
            for i in j + 1..size {
                let mut value = factor[i * size + j];
                for k in 0..j {
                    value -= factor[i * size + k] * factor[j * size + k];
                }
                factor[i * size + j] = value / factor[j * size + j];
            }
        }
    }
    budget.release(matrix)?;
    budget.release(totals)?;
    budget.release(parent)?;
    budget.release(local)?;
    budget.release(touched)?;
    budget.release(anchors)?;
    let setup_seconds = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let mut scales = budget.reserve(width, 0.)?;
    let mut means = budget.reserve(width, 0.)?;
    let total: f64 = mass_a.iter().sum();
    if !total.is_finite() || total <= 0. {
        return Err(declined());
    }
    scales
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(j, norm)| -> Result<()> {
            let mut scale = 0_f64;
            let mut square = 1.;
            // Keep the legacy matrix indices and numerical traversal order explicit.
            #[allow(clippy::needless_range_loop)]
            for i in 0..n {
                let value = columns[j].value(i)?;
                if !value.is_finite() {
                    return Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "canonical original column is nonfinite",
                    ));
                }
                let absolute = (value * w[i].sqrt()).abs();
                if absolute > scale {
                    square = 1. + square * (scale / absolute).powi(2);
                    scale = absolute;
                } else if absolute > 0. {
                    square += (absolute / scale).powi(2);
                }
            }
            *norm = scale * square.sqrt();
            if !norm.is_finite() {
                return Err(declined());
            }
            Ok(())
        })?;
    let mut retained_left = if retain_centers {
        let mut values = budget.reserve(p * width, 0.)?;
        values
            .par_chunks_mut(p)
            .enumerate()
            .try_for_each(|(j, values)| -> Result<()> {
                for (g, value) in values.iter_mut().enumerate() {
                    *value = group_coefficient(
                        &columns[j],
                        &w,
                        nonzero(scales[j]),
                        mass_a[g],
                        a.level_index().members(g),
                        None,
                    )?;
                }
                Ok(())
            })?;
        Some(values)
    } else {
        None
    };
    // Each RHS owns its complete retained-level vector and compensated mean.
    // This avoids T copies of Q*W and visits each original column in group order.
    let mut cross = budget.reserve(q * width, 0.)?;
    cross
        .par_chunks_mut(q)
        .zip(means.par_iter_mut())
        .enumerate()
        .try_for_each(|(j, (cross, mean))| -> Result<()> {
            let scale = nonzero(scales[j]);
            let mut sum = 0.;
            let mut correction = 0.;
            for g in 0..p {
                let members = a.level_index().members(g);
                let center = if let Some(left) = &retained_left {
                    left[j * p + g]
                } else {
                    group_coefficient(&columns[j], &w, scale, mass_a[g], members, None)?
                };
                let term = (mass_a[g] / total) * center - correction;
                let next = sum + term;
                correction = (next - sum) - term;
                sum = next;
                for &i in members {
                    let h = b.codes()[i] as usize;
                    let value = columns[j].value(i)? / scale - center;
                    cross[h] = w[i].mul_add(value, cross[h]);
                }
            }
            *mean = sum * scale;
            if !mean.is_finite() {
                return Err(declined());
            }
            Ok(())
        })?;
    let mut solved = budget.reserve(d * width, 0.)?;
    let mut krylov = budget.reserve(krylov_cells, 0.)?;
    let iterations = AtomicUsize::new(1);
    if d > 0 {
        let solve_column = |j: usize, values: &mut [f64], scratch: &mut [f64]| -> Result<()> {
            for h in 0..q {
                if free[h] != usize::MAX {
                    values[free[h]] = cross[j * q + h] / roots[h];
                }
            }
            for block in &components {
                let size = block.len;
                let values = &mut values[block.start..block.start + size];
                if implicit {
                    let root = block.matrix_offset;
                    let start = incidence_rows.partition_point(|row| row.component < root);
                    let end = incidence_rows.partition_point(|row| row.component <= root);
                    let rows = &incidence_rows[start..end];
                    let diagonal = &factor[n + block.start..n + block.start + size];
                    let count = solve_reduced_cg_with_product(
                        values,
                        &mut scratch[..3 * size],
                        config.max_iterations.min(size),
                        config.tolerance * 0.01,
                        |direction, product| {
                            apply_incidence_schur(
                                &factor[..n],
                                diagonal,
                                &incidence_ids,
                                rows,
                                direction,
                                product,
                            )
                        },
                    )?;
                    iterations.fetch_max(count, Ordering::Relaxed);
                    continue;
                }
                let factor = &factor[block.offset..block.offset + size * size];
                if iterative {
                    let count = solve_reduced_cg(
                        factor,
                        values,
                        &mut scratch[..3 * size],
                        config.max_iterations.min(size),
                        config.tolerance * 0.01,
                    )?;
                    iterations.fetch_max(count, Ordering::Relaxed);
                    continue;
                }
                for i in 0..size {
                    for k in 0..i {
                        values[i] -= factor[i * size + k] * values[k];
                    }
                    values[i] /= factor[i * size + i];
                }
                for i in (0..size).rev() {
                    for k in i + 1..size {
                        values[i] -= factor[k * size + i] * values[k];
                    }
                    values[i] /= factor[i * size + i];
                }
            }
            if values.iter().any(|x| !x.is_finite()) {
                return Err(declined());
            }
            Ok(())
        };
        if iterative {
            solved
                .par_chunks_mut(d)
                .zip(krylov.par_chunks_mut(3 * d))
                .enumerate()
                .try_for_each(|(j, (values, scratch))| solve_column(j, values, scratch))?;
        } else {
            solved
                .par_chunks_mut(d)
                .enumerate()
                .try_for_each(|(j, values)| solve_column(j, values, &mut []))?;
        }
    }
    budget.release(krylov)?;
    budget.release(incidence_ids)?;
    budget.release(incidence_rows)?;
    budget.release(factor)?;
    budget.release(components)?;
    budget.release(cross)?;
    let mut right = budget.reserve(q * width, 0.)?;
    right.par_chunks_mut(q).enumerate().for_each(|(j, values)| {
        for h in 0..q {
            if free[h] != usize::MAX {
                values[h] = solved[j * d + free[h]] / roots[h];
            }
        }
    });
    budget.release(solved)?;
    budget.release(roots)?;
    budget.release(free)?;
    if let Some(left) = retained_left.as_mut() {
        left.par_chunks_mut(p)
            .enumerate()
            .try_for_each(|(j, values)| -> Result<()> {
                for (g, value) in values.iter_mut().enumerate() {
                    *value -= group_effect_mean(
                        &w,
                        mass_a[g],
                        a.level_index().members(g),
                        &right[j * q..(j + 1) * q],
                        b.codes(),
                    );
                    if !value.is_finite() {
                        return Err(declined());
                    }
                }
                Ok(())
            })?;
    }
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!("FEREG_SOLVER_TRACE|phase=canonical_reduced_solve|rows={n}|iterative={}|implicit={}|iterations={}",
            u8::from(iterative), u8::from(implicit), iterations.load(Ordering::Relaxed));
    }
    let solve_seconds = started.elapsed().as_secs_f64();
    let started = Instant::now();
    // RHS reductions are already released. Certificate reductions also need
    // a Gram and independent left-score diagnostics, only in this later phase.
    let mut partials = budget.reserve(workers * stride, 0.)?;
    let groups_per_worker = p.div_ceil(workers);
    partials.par_chunks_mut(stride).enumerate().try_for_each(
        |(worker, partial)| -> Result<()> {
            let (gram, rest) = partial.split_at_mut(width * width);
            let (scores, rest) = rest.split_at_mut(q * width);
            let (centers, rest) = rest.split_at_mut(width);
            let (row, rest) = rest.split_at_mut(width);
            let (sums, rest) = rest.split_at_mut(width);
            let (maximum, rest) = rest.split_at_mut(width);
            let (squares, absolute) = rest.split_at_mut(width);
            for g in worker * groups_per_worker..((worker + 1) * groups_per_worker).min(p) {
                for j in 0..width {
                    centers[j] = if let Some(left) = &retained_left {
                        left[j * p + g]
                    } else {
                        group_coefficient(
                            &columns[j],
                            &w,
                            nonzero(scales[j]),
                            mass_a[g],
                            a.level_index().members(g),
                            Some((&right[j * q..(j + 1) * q], b.codes())),
                        )?
                    };
                }
                sums.fill(0.);
                for &i in a.level_index().members(g) {
                    let h = b.codes()[i] as usize;
                    for j in 0..width {
                        row[j] = columns[j].value(i)? / nonzero(scales[j])
                            - centers[j]
                            - right[j * q + h];
                        if !row[j].is_finite() {
                            return Err(declined());
                        }
                        let score = w[i] * row[j];
                        sums[j] += score;
                        scores[h * width + j] += score;
                    }
                    for j in 0..width {
                        let value = w[i] * row[j];
                        for k in j..width {
                            gram[j * width + k] = value.mul_add(row[k], gram[j * width + k]);
                        }
                    }
                }
                for j in 0..width {
                    let value = sums[j].abs() / mass_a[g].sqrt();
                    if !value.is_finite() {
                        return Err(declined());
                    }
                    maximum[j] = maximum[j].max(value);
                    squares[j] = value.mul_add(value, squares[j]);
                    absolute[j] = absolute[j].max(sums[j].abs());
                }
            }
            Ok(())
        },
    )?;
    let mut gram = budget.reserve(width * width, 0.)?;
    for j in 0..width {
        for k in j..width {
            let value = merge(&partials, stride, j * width + k);
            if !value.is_finite() {
                return Err(declined());
            }
            gram[j * width + k] = value;
            gram[k * width + j] = value;
        }
    }
    let mut norms = budget.reserve(width, 0.)?;
    let mut maximum = 0_f64;
    let mut absolute = 0_f64;
    let mut squares = 0.;
    for j in 0..width {
        let norm = gram[j * width + j].sqrt();
        let denominator = norm.max(1.);
        norms[j] = norm * nonzero(scales[j]);
        if !norms[j].is_finite() {
            return Err(declined());
        }
        let mut column_max = 0_f64;
        let mut column_abs = 0_f64;
        let mut column_square = 0.;
        for partial in partials.chunks(stride) {
            column_max = column_max.max(partial[stride - 3 * width + j]);
            column_square += partial[stride - 2 * width + j];
            column_abs = column_abs.max(partial[stride - width + j]);
        }
        // Keep the legacy matrix indices and numerical traversal order explicit.
        #[allow(clippy::needless_range_loop)]
        for h in 0..q {
            let score = merge(&partials, stride, width * width + h * width + j).abs();
            let value = score / mass_b[h].sqrt();
            if !value.is_finite() {
                return Err(declined());
            }
            column_max = column_max.max(value);
            column_square = value.mul_add(value, column_square);
            column_abs = column_abs.max(score);
        }
        maximum = maximum.max(column_max / denominator);
        squares += column_square / (denominator * denominator);
        absolute = absolute.max((column_abs * nonzero(scales[j])) * divisor);
    }
    if !maximum.is_finite()
        || maximum > config.tolerance
        || !squares.is_finite()
        || !absolute.is_finite()
    {
        return Err(declined());
    }
    budget.release(partials)?;
    // Replay the certified pure coefficient calculation with unchanged inputs.
    // Full reconstruction releases weights now; tiled reconstruction retains
    // them through row writing. Neither layout carries authority across calls.
    budget.release(mass_b)?;
    let (left, tiled, within_response) = if tile_output {
        let mut response_coefficients = budget.reserve(p, 0.)?;
        response_coefficients
            .par_iter_mut()
            .enumerate()
            .try_for_each(|(g, value)| -> Result<()> {
                *value = group_coefficient(
                    &columns[0],
                    &w,
                    nonzero(scales[0]),
                    mass_a[g],
                    a.level_index().members(g),
                    Some((&right[..q], b.codes())),
                )?;
                Ok(())
            })?;
        let mut within = budget.reserve(n, 0.)?;
        within
            .par_iter_mut()
            .enumerate()
            .try_for_each(|(i, value)| -> Result<()> {
                *value = (response[i] / nonzero(scales[0])
                    - response_coefficients[a.codes()[i] as usize]
                    - right[b.codes()[i] as usize])
                    * nonzero(scales[0]);
                if !value.is_finite() {
                    return Err(declined());
                }
                Ok(())
            })?;
        budget.release(response_coefficients)?;
        let rows = plan.output_tile_rows();
        let groups = p.min(rows);
        let tile = TiledCoefficients {
            weights: w,
            masses: mass_a,
            slots: budget.reserve(p, usize::MAX)?,
            groups: budget.reserve(groups, 0_usize)?,
            values: budget.reserve(groups * width, 0.)?,
            rows,
        };
        (Vec::new(), Some(tile), within)
    } else {
        let left = if let Some(left) = retained_left {
            left
        } else {
            let mut left = budget.reserve(p * width, 0.)?;
            left.par_chunks_mut(p)
                .enumerate()
                .try_for_each(|(j, values)| -> Result<()> {
                    for (g, value) in values.iter_mut().enumerate() {
                        *value = group_coefficient(
                            &columns[j],
                            &w,
                            nonzero(scales[j]),
                            mass_a[g],
                            a.level_index().members(g),
                            Some((&right[j * q..(j + 1) * q], b.codes())),
                        )?;
                    }
                    Ok(())
                })?;
            left
        };
        budget.release(mass_a)?;
        // Every weight-dependent solve and acceptance check has finished. Each
        // observation is overwritten from immutable originals, so this transfer
        // needs neither another N-vector nor a changed reconstruction recurrence.
        let mut within_response = w;
        within_response
            .par_iter_mut()
            .enumerate()
            .try_for_each(|(i, value)| -> Result<()> {
                *value = (response[i] / nonzero(scales[0])
                    - left[a.codes()[i] as usize]
                    - right[b.codes()[i] as usize])
                    * nonzero(scales[0]);
                if !value.is_finite() {
                    return Err(declined());
                }
                Ok(())
            })?;
        (left, None, within_response)
    };
    let report = SolveReport {
        requested_solver: FeSolverKind::Map,
        used_solver: FeSolverKind::Map,
        iterations: iterations.load(Ordering::Relaxed),
        checkpoint_retries: 0,
        solver_workspace_peak: SolverWorkspacePeak::from_bytes(bytes)?,
        stop_reason: SolveStopReason::Certified,
        certificate: FeCertificate {
            absolute_infinity_norm: absolute,
            relative_infinity_norm: maximum,
            relative_two_norm: (squares / ((p + q) * width) as f64).sqrt(),
            passed: true,
        },
        normal_equation_residual: maximum,
        setup_seconds,
        solve_seconds,
        certification_seconds: started.elapsed().as_secs_f64(),
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.,
        warnings: Vec::new(),
    };
    let moments = CanonicalMoments {
        width,
        gram,
        response_scale: nonzero(scales[0]),
    };
    Ok(Products {
        columns,
        design,
        left_effect,
        left,
        tiled,
        right,
        three: None,
        scales,
        means,
        norms,
        response: within_response,
        moments,
        report,
        #[cfg(test)]
        minimum_workspace_bytes: minimum_bytes,
        #[cfg(test)]
        retained_centers: retain_centers,
        #[cfg(test)]
        implicit,
    })
}

// Use an identical coefficient recurrence in the independent residual replay
// and the later retained-row reconstruction; no weighted input changes between
// those phases. Omitting right coefficients gives the eliminated-group center.
fn group_coefficient<P: PreparedOlsFitView + ?Sized>(
    column: &Column<'_, P>,
    weights: &[f64],
    scale: f64,
    mass: f64,
    members: &[usize],
    right: Option<(&[f64], &[u32])>,
) -> Result<f64> {
    let mut sum = 0.;
    let mut correction = 0.;
    for &i in members {
        let value = (weights[i] / mass) * (column.value(i)? / scale);
        let term = value - correction;
        let next = sum + term;
        correction = (next - sum) - term;
        sum = next;
    }
    if let Some((values, codes)) = right {
        sum -= group_effect_mean(weights, mass, members, values, codes);
    }
    if sum.is_finite() {
        Ok(sum)
    } else {
        Err(declined())
    }
}

// The coefficient recurrence only proposes a projection. The independent
// original-row FE certificate and direct Gram construction remain mandatory.
fn reduced_matrix_product(left: &[f64], right: &[f64]) -> f64 {
    #[cfg(any(target_arch = "aarch64", target_feature = "fma"))]
    {
        normalized_product(left, right)
    }
    #[cfg(not(any(target_arch = "aarch64", target_feature = "fma")))]
    {
        // Portable mul_add calls libm for every entry. This speculative operator
        // can use inline arithmetic; geometry and fresh certificates retain FMA.
        debug_assert_eq!(left.len(), right.len());
        let mut sums = [0.0_f64; 8];
        let blocks = left.len() / sums.len();
        for block in 0..blocks {
            let start = block * sums.len();
            for lane in 0..sums.len() {
                sums[lane] += left[start + lane] * right[start + lane];
            }
        }
        for row in blocks * sums.len()..left.len() {
            sums[0] += left[row] * right[row];
        }
        sums.iter().sum()
    }
}

fn solve_reduced_cg(
    matrix: &[f64],
    values: &mut [f64],
    scratch: &mut [f64],
    max_iterations: usize,
    tolerance: f64,
) -> Result<usize> {
    let n = values.len();
    if matrix.len() != n * n {
        return Err(declined());
    }
    solve_reduced_cg_with_product(
        values,
        scratch,
        max_iterations,
        tolerance,
        |direction, product| {
            for (row, out) in matrix.chunks(n).zip(product.iter_mut()) {
                *out = reduced_matrix_product(row, direction);
            }
        },
    )
}

#[derive(Clone, Copy, Default)]
struct IncidenceRow {
    start: usize,
    end: usize,
    component: usize,
}

// diag(m_b / root^2) - C'C. Group-pair weights are combined once;
// grounding and component-local coordinates are resolved before any iteration.
fn apply_incidence_schur(
    incidence: &[f64],
    diagonal: &[f64],
    ids: &[usize],
    rows: &[IncidenceRow],
    direction: &[f64],
    product: &mut [f64],
) {
    for ((out, d), x) in product.iter_mut().zip(diagonal).zip(direction) {
        *out = d * x;
    }
    for row in rows {
        let mut center = 0.;
        let coefficients = &incidence[row.start..row.end];
        let columns = &ids[row.start..row.end];
        for (&c, &j) in coefficients.iter().zip(columns) {
            center = c.mul_add(direction[j], center);
        }
        for (&c, &j) in coefficients.iter().zip(columns) {
            product[j] = (-c).mul_add(center, product[j]);
        }
    }
}

fn solve_reduced_cg_with_product(
    values: &mut [f64],
    scratch: &mut [f64],
    max_iterations: usize,
    tolerance: f64,
    apply: impl Fn(&[f64], &mut [f64]),
) -> Result<usize> {
    let n = values.len();
    if scratch.len() < 3 * n {
        return Err(declined());
    }
    let scale = values.iter().map(|x| x.abs()).fold(0_f64, f64::max);
    if !scale.is_finite() {
        return Err(declined());
    }
    if scale == 0. {
        return Ok(0);
    }
    let (residual, rest) = scratch.split_at_mut(n);
    let (direction, product) = rest.split_at_mut(n);
    let product = &mut product[..n];
    for i in 0..n {
        residual[i] = values[i] / scale;
        direction[i] = residual[i];
    }
    values.fill(0.);
    let mut square: f64 = residual.iter().map(|x| x * x).sum();
    let target = tolerance * square.sqrt();
    for iteration in 1..=max_iterations {
        apply(direction, product);
        let curvature: f64 = direction
            .iter()
            .zip(product.iter())
            .map(|(x, y)| x * y)
            .sum();
        if !curvature.is_finite() || curvature <= 0. || !square.is_finite() {
            return Err(declined());
        }
        let alpha = square / curvature;
        let mut next = 0.;
        for i in 0..n {
            values[i] = alpha.mul_add(direction[i], values[i]);
            residual[i] = (-alpha).mul_add(product[i], residual[i]);
            next = residual[i].mul_add(residual[i], next);
        }
        if !next.is_finite() {
            return Err(declined());
        }
        if next.sqrt() <= target {
            for value in values {
                *value *= scale;
                if !value.is_finite() {
                    return Err(declined());
                }
            }
            return Ok(iteration);
        }
        let beta = next / square;
        for i in 0..n {
            direction[i] = beta.mul_add(direction[i], residual[i]);
        }
        square = next;
    }
    Err(declined())
}

fn group_effect_mean(
    weights: &[f64],
    mass: f64,
    members: &[usize],
    values: &[f64],
    codes: &[u32],
) -> f64 {
    let mut sum = 0.;
    for &i in members {
        sum += (weights[i] / mass) * values[codes[i] as usize];
    }
    sum
}

fn merge(partials: &[f64], stride: usize, slot: usize) -> f64 {
    let mut sum = 0.;
    let mut correction = 0.;
    for partial in partials.chunks(stride) {
        let term = partial[slot] - correction;
        let next = sum + term;
        correction = (next - sum) - term;
        sum = next;
    }
    sum
}

#[cfg(test)]
mod tests;
