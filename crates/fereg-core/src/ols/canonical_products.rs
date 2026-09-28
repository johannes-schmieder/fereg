//! Current-call two-partition projection and finite products for canonical PPML.
//! All weighted state dies with this call; only certified rows and one-use
//! moments cross into the canonical rank owner.
use super::prepared_scratch::PreparedOlsScratch;
use super::PreparedOlsColumnKind;
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    BorrowedInterceptFeDesign, BorrowedMapWeights, FeCertificate, FeSolverKind, MapConfig,
    SolveReport, SolveStopReason,
};
#[cfg(any(target_arch = "aarch64", target_feature = "fma"))]
use crate::fe::slope_coefficients::joint::normalized_product;
use crate::memory::SolverWorkspacePeak;
use crate::prepared_fit_view::PreparedOlsFitView;
use rayon::prelude::*;
use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

mod three;
pub(crate) fn canonical_three_workspace_bytes(n: usize, width: usize, levels: [usize; 3], threads: usize) -> Option<u128> {
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

enum Column<'a, P: ?Sized> {
    Numeric(&'a [f64]),
    Generated(&'a P, usize),
}
impl<P: ?Sized> Clone for Column<'_, P> {
    fn clone(&self) -> Self {
        match self { Self::Numeric(v) => Self::Numeric(v), Self::Generated(p, j) => Self::Generated(p, *j) }
    }
}
impl<P: PreparedOlsFitView + ?Sized> Column<'_, P> {
    #[inline]
    fn value(&self, row: usize) -> Result<f64> {
        match self {
            Self::Numeric(x) => Ok(x[row]),
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
                        &columns[j], &self.weights, nonzero(scales[j]), self.masses[g],
                        a.level_index().members(g), Some((&right[j * q..(j + 1) * q], b.codes())),
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
            |tile, first, count| tile.prepare(&self.columns, &self.design, self.left_effect,
                &self.scales, &self.right, first, count),
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
            return Err(FeregError::new(ErrorCode::InvalidArgument,
                "canonical dense product storage is misaligned"));
        }
        if columns == 0 { return Ok(()); }
        let Some(tile) = self.tiled.as_mut() else {
            return storage.as_mut_slice().par_chunks_mut(columns).enumerate()
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
        for (index, values) in storage.as_mut_slice().chunks_mut(rows * columns).enumerate() {
            let first = index * rows;
            tile.prepare(&self.columns, &self.design, self.left_effect, &self.scales,
                &self.right, first, values.len() / columns)?;
            values.par_chunks_mut(columns).enumerate().try_for_each(|(i, values)| -> Result<()> {
                let row = first + i;
                let slot = tile.slots[a.codes()[row] as usize];
                for (column, value) in values.iter_mut().enumerate() {
                    let j = column + 1;
                    let scale = nonzero(self.scales[j]);
                    *value = (self.columns[j].value(row)? / scale - tile.values[slot * width + j]
                        - self.right[j * q + b.codes()[row] as usize]) * scale;
                    if !value.is_finite() { return Err(declined()); }
                }
                Ok(())
            })?;
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
        return three::try_build(prepared, generated, response, weights, divisor, config,
            allowance, obsolete_projection, design);
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
        || plan.topology_storage_bytes().is_none_or(|bytes| bytes > available)
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
        if let PreparedOlsColumnKind::Continuous {
            prepared_source_index,
            ..
        } = recipe.kind
        {
            let values = prepared
                .fit_sample()
                .regressors
                .get(prepared_source_index)
                .ok_or_else(declined)?;
            if values.len() != n {
                return Err(declined());
            }
            columns.push(Column::Numeric(values));
        } else {
            columns.push(Column::Generated(prepared, j));
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
        d.checked_mul(width).and_then(|x| x.checked_mul(3)).ok_or_else(declined)?
    } else { 0 };
    let dense_charge = factor_cells.checked_add(dense_krylov).ok_or_else(declined)?;
    // Dense geometry already affordable under the existing complete phase plan
    // retains its arithmetic. Implicit geometry admits the full certified
    // product pipeline where dense storage would otherwise force repeated
    // observation-space construction, not merely a different dense matvec.
    let implicit = plan.minimum_storage_bytes(matrix_cells, dense_charge)
        .is_none_or(|bytes| bytes > allowance);
    let iterative = implicit || dense_iterative;
    let krylov_cells = if iterative {
        d.checked_mul(width).and_then(|x| x.checked_mul(3)).ok_or_else(declined)?
    } else { 0 };
    let matrix_cells = if implicit { q } else { matrix_cells };
    let factor_cells = if implicit { n.checked_add(q).ok_or_else(declined)? } else { factor_cells };
    let incidence_indices = if implicit {
        (n as u128).checked_mul(size_of::<usize>() as u128)
            .and_then(|x| x.checked_add((p as u128) * size_of::<IncidenceRow>() as u128))
            .ok_or_else(declined)?.div_ceil(8)
    } else { 0 };
    let incidence_words = usize::try_from(incidence_indices).map_err(|_| declined())?;
    let factor_charge = factor_cells.checked_add(krylov_cells)
        .and_then(|x| x.checked_add(incidence_words)).ok_or_else(declined)?;
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
        roots[h] = if implicit { matrix[h] } else {
            matrix[block.matrix_offset + local[h] * size + local[h]]
        }.sqrt();
        if anchors[r] != h {
            if !roots[h].is_finite() || roots[h] <= 0. {
                return Err(declined());
            }
            free[h] = block.start + local[h] - usize::from(local[h] > local[anchors[r]]);
        }
    }
    let mut factor = budget.reserve(factor_cells, 0.)?;
    let mut incidence_ids = budget.reserve(if implicit { n } else { 0 }, 0_usize)?;
    let mut incidence_rows = budget.reserve(if implicit { p } else { 0 }, IncidenceRow::default())?;
    if implicit {
        let mut edge = 0;
        for g in 0..p {
            let members = a.level_index().members(g);
            let component = parent[b.codes()[members[0]] as usize];
            let mut count = 0;
            for &i in members {
                let h = b.codes()[i] as usize;
                if totals[h] == 0. { touched[count] = h; count += 1; }
                totals[h] += w[i];
            }
            touched[..count].sort_unstable();
            let start = edge;
            let mass_root = mass_a[g].sqrt();
            for &h in &touched[..count] {
                if free[h] != usize::MAX {
                    incidence_ids[edge] = free[h] - components[component].start;
                    factor[edge] = (totals[h] / mass_root) / roots[h];
                    if !factor[edge].is_finite() { return Err(declined()); }
                    edge += 1;
                }
                totals[h] = 0.;
            }
            incidence_rows[g] = IncidenceRow { start, end: edge, component };
        }
        incidence_rows.sort_unstable_by_key(|row| (row.component, row.start));
        for h in 0..q {
            if free[h] != usize::MAX {
                factor[n + free[h]] = (mass_b[h] / roots[h]) / roots[h];
                if !factor[n + free[h]].is_finite() { return Err(declined()); }
            }
        }
        for (root, block) in components.iter_mut().enumerate() { block.matrix_offset = root; }
    } else {
    for h in 0..q {
        if free[h] != usize::MAX {
            let block = components[parent[h]];
            let size = block.len + 1;
            for &j in &touched[block.level_start..block.level_start + size] {
                if free[j] != usize::MAX {
                    factor[block.offset + (free[h] - block.start) * block.len + free[j]
                        - block.start] = (matrix[block.matrix_offset + local[h] * size + local[j]]
                        / roots[h])
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
        if iterative { break; }
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
                        let count = solve_reduced_cg_with_product(values, &mut scratch[..3 * size],
                            config.max_iterations.min(size), config.tolerance * 0.01,
                            |direction, product| apply_incidence_schur(
                                &factor[..n], diagonal, &incidence_ids, rows, direction, product))?;
                        iterations.fetch_max(count, Ordering::Relaxed);
                        continue;
                    }
                    let factor = &factor[block.offset..block.offset + size * size];
                    if iterative {
                        let count = solve_reduced_cg(factor, values, &mut scratch[..3*size],
                            config.max_iterations.min(size), config.tolerance * 0.01)?;
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
            solved.par_chunks_mut(d).zip(krylov.par_chunks_mut(3*d)).enumerate()
                .try_for_each(|(j, (values, scratch))| solve_column(j, values, scratch))?;
        } else {
            solved.par_chunks_mut(d).enumerate()
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
    if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
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
    if matrix.len() != n * n { return Err(declined()); }
    solve_reduced_cg_with_product(values, scratch, max_iterations, tolerance,
        |direction, product| {
            for (row, out) in matrix.chunks(n).zip(product.iter_mut()) {
                *out = reduced_matrix_product(row, direction);
            }
        })
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
    incidence: &[f64], diagonal: &[f64], ids: &[usize], rows: &[IncidenceRow],
    direction: &[f64], product: &mut [f64],
) {
    for ((out, d), x) in product.iter_mut().zip(diagonal).zip(direction) { *out = d * x; }
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
    values: &mut [f64], scratch: &mut [f64], max_iterations: usize,
    tolerance: f64, apply: impl Fn(&[f64], &mut [f64]),
) -> Result<usize> {
    let n = values.len();
    if scratch.len() < 3 * n { return Err(declined()); }
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
mod tests {
    use super::*;
    use crate::metadata::*;
    use crate::ols::{
        prepare_ols_data, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
        OlsRequestV1, PreparedOlsData,
    };

    fn prepared(y: &[f64], x: &[Vec<f64>], g: &[f64], h: &[f64]) -> PreparedOlsData { prepared_effects(y, x, &[g, h]) }

    fn prepared_effects(y: &[f64], x: &[Vec<f64>], effects: &[&[f64]]) -> PreparedOlsData {
        let mut sources = Vec::new();
        let mut terms = Vec::new();
        let mut numeric = Vec::new();
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ];
        for (j, values) in x.iter().map(Vec::as_slice).chain(effects.iter().copied()).enumerate() {
            let id = (j + 1) as u32;
            let source = SourceId::new(id).unwrap();
            sources.push(SourceEntry {
                id: source,
                canonical_name: format!("v{id}"),
            });
            bindings.push(OlsInputBinding {
                stata_var_position: id + 2,
                role: OlsInputRole::NumericSource(source),
            });
            numeric.push((
                source,
                LoadedNumericColumn {
                    values: values.to_vec(),
                    missing: vec![false; y.len()],
                },
            ));
            if j < x.len() {
                terms.push(ExpandedTerm {
                    id: TermId::new(id).unwrap(),
                    order: TermOrder::new(id).unwrap(),
                    coefficient_slot: CoefficientSlot::new(id).unwrap(),
                    raw_kind: RawTermKind::Variable,
                    whole_column_omitted: false,
                    canonical_token: format!("c.v{id}"),
                    components: vec![TermComponent {
                        order: ComponentOrder::new(1).unwrap(),
                        source,
                        kind: ComponentKind::Continuous,
                        level_state: LevelState::Absent,
                        level_value: 0.,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::Unavailable,
                        raw_operator: "c".into(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    }],
                });
            }
        }
        prepare_ols_data(
            &OlsRequestV1 {
                model: CompiledTermGraph { sources, terms },
                absorbed_terms: (0..effects.len() as u32)
                    .map(|j| AbsorbedTerm {
                        id: AbsorbTermId::new(j + 1).unwrap(),
                        intercept: true,
                        categorical_sources: vec![SourceId::new(x.len() as u32 + j + 1).unwrap()],
                        slope_sources: Vec::new(),
                        label: format!("fe{j}"),
                        saved_name: String::new(),
                    })
                    .collect(),
                bindings,
                include_intercept: true,
                keep_singletons: true,
            },
            &LoadedOlsInput {
                candidate: vec![true; y.len()],
                outcome: LoadedNumericColumn {
                    values: y.to_vec(),
                    missing: vec![false; y.len()],
                },
                numeric_sources: numeric,
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn implicit_canonical_frame_matches_weighted_contrasts_when_dense_storage_refuses() {
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                for disconnected in [false, true] {
                    for frame in [0, 1] {
                        for mass in [1e-100, 1., 1e100] {
                            let (mut g, mut h, mut w) = (Vec::new(), Vec::new(), Vec::new());
                            let mut values = vec![Vec::new(); 7];
                            let mut truth = vec![Vec::new(); 7];
                            for a in 0..1024 { for e in 0..8 { for repeat in 0..2 {
                                let b = if disconnected { (a*7+e*61)%256 + (a/512)*256 }
                                    else { (a*7+e*61)%512 };
                                g.push(a as f64); h.push(b as f64);
                                w.push(mass*(0.2+((a*13+e*7+frame*3)%17) as f64));
                                for j in 0..7 {
                                    let scale = if j==1 { 1e20 } else if j==2 { 1e-20 } else { 1. };
                                    let r = if j==3 { 0. } else {
                                        (2.*repeat as f64-1.)*((a*(j+1)+e*3)%11) as f64
                                    };
                                    truth[j].push(r*scale);
                                    values[j].push((r+0.01*a as f64-0.02*b as f64)*scale);
                                }
                            }}}
                            let p = prepared(&values[0], &values[1..], &g, &h);
                            let config = MapConfig { tolerance: 1e-12, ..MapConfig::default() };
                            let divisor = w.iter().copied().fold(0_f64, f64::max);
                            let allowance = 16*g.len() as u128*6;
                            let cols = [0,1,2,3,4,5];
                            let mut product = try_build(&p, &cols, &values[0], BorrowedMapWeights::Provided(&w),
                                divisor, config, allowance, 0, &mut Vec::new()).unwrap().expect("implicit frame fits and certifies");
                            assert!(product.implicit && product.report.certified());
                            let mut dense = DenseMatrix::zeros(g.len(), 6).unwrap();
                            product.write_dense(&mut dense).unwrap();
                            let mut file = PreparedOlsScratch::create_row_major(g.len(), 6).unwrap();
                            product.write_controls(&mut file).unwrap();
                            let mut row = [0.;6];
                            for i in 0..g.len() {
                                file.read_linear_shared(i*6, &mut row).unwrap();
                                assert!((product.response[i]-truth[0][i]).abs()<1e-8);
                                for j in 1..7 {
                                    assert_eq!(row[j-1],dense[(i,j-1)]);
                                    let scale = if j==1 { 1e20 } else if j==2 { 1e-20 } else { 1. };
                                    assert!((row[j-1]-truth[j][i]).abs()/scale<1e-8);
                                }
                            }
                            for j in 0..7 { for k in 0..7 {
                                let expected: f64 = (0..g.len()).map(|i| (w[i]/divisor)*
                                    (truth[j][i]/nonzero(product.scales[j]))*(truth[k][i]/nonzero(product.scales[k]))).sum();
                                assert!((product.moments.gram[j*7+k]-expected).abs()<1e-10);
                            }}
                            let bytes=product.minimum_workspace_bytes;
                            assert!(try_build(&p,&cols,&values[0],BorrowedMapWeights::Provided(&w),divisor,
                                config,bytes-1,0,&mut Vec::new()).unwrap().is_none());
                            assert!(try_build(&p,&cols,&values[0],BorrowedMapWeights::Provided(&w),divisor,
                                config,bytes,0,&mut Vec::new()).unwrap().is_some());
                            assert!(try_build(&p,&cols,&values[0],BorrowedMapWeights::Provided(&w),divisor,
                                MapConfig { max_iterations:1, ..config },allowance,0,&mut Vec::new()).unwrap().is_none());
                            // A genuinely affordable dense frame retains its old organization.
                            let dense=try_build(&p,&cols,&values[0],BorrowedMapWeights::Provided(&w),divisor,
                                config,u128::MAX,0,&mut Vec::new()).unwrap().unwrap();
                            assert!(!dense.implicit);
                        }
                    }
                }
            });
        }
    }

    #[test]
    fn incidence_schur_matches_independent_dense_weighted_elimination() {
        let a = [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
        let b = [0, 1, 1, 1, 2, 0, 3, 4, 4, 4, 5, 3];
        let groups = crate::fe::LevelIndex::from_codes(&a, 4).unwrap();
        let free = [usize::MAX, 0, 1, usize::MAX, 2, 3];
        let truth = [0.3, -0.7, 1.1, 0.2];
        for scale in [1e-200, 1., 1e200] {
            for frame in [0, 1] {
                let weights: Vec<f64> = (0..a.len()).map(|i| scale *
                    (0.2 + ((i * 7 + frame * 3) % 11) as f64)).collect();
                let mut cross = [[0.; 6]; 4];
                let mut left_mass = [0.; 4];
                let mut right_mass = [0.; 6];
                for i in 0..a.len() {
                    cross[a[i] as usize][b[i] as usize] += weights[i];
                    left_mass[a[i] as usize] += weights[i];
                    right_mass[b[i] as usize] += weights[i];
                }
                let mut schur = [[0_f64; 6]; 6];
                for h in 0..6 {
                    schur[h][h] = right_mass[h];
                    for j in 0..6 {
                        for g in 0..4 {
                            schur[h][j] -= (cross[g][h] / left_mass[g]) * cross[g][j];
                        }
                    }
                }
                let roots: Vec<_> = (0..6).map(|h| schur[h][h].sqrt()).collect();
                let mut dense = vec![0.; 16];
                for h in 0..6 {
                    if free[h] == usize::MAX { continue; }
                    for j in 0..6 {
                        if free[j] != usize::MAX {
                            dense[free[h] * 4 + free[j]] = (schur[h][j] / roots[h]) / roots[j];
                        }
                    }
                }
                let mut incidence = Vec::new();
                let mut ids = Vec::new();
                let mut rows = Vec::new();
                for g in 0..4 {
                    let start = ids.len();
                    for &i in groups.members(g) {
                        let h = b[i] as usize;
                        if free[h] == usize::MAX { continue; }
                        ids.push(free[h]);
                        incidence.push((weights[i] / left_mass[g].sqrt()) / roots[h]);
                    }
                    rows.push(IncidenceRow { start, end: ids.len(), component: g / 2 });
                }
                let mut diagonal = [0.; 4];
                for h in 0..6 {
                    if free[h] != usize::MAX { diagonal[free[h]] = (right_mass[h] / roots[h]) / roots[h]; }
                }
                let apply = |x: &[f64], y: &mut [f64]| apply_incidence_schur(
                    &incidence, &diagonal, &ids, &rows, x, y);
                let rhs: Vec<_> = dense.chunks(4).map(|row| row.iter().zip(truth).map(|(a,b)| a*b).sum::<f64>()).collect();
                let mut actual = [0.; 4];
                apply(&truth, &mut actual);
                for i in 0..4 { assert!((actual[i] - rhs[i]).abs() < 3e-15); }
                let mut solution = rhs.clone();
                let count = solve_reduced_cg_with_product(&mut solution, &mut [0.; 12], 4, 1e-13, apply).unwrap();
                assert!(count <= 4);
                for i in 0..4 { assert!((solution[i] - truth[i]).abs() < 1e-12); }
                assert!(solve_reduced_cg_with_product(&mut rhs.clone(), &mut [0.; 12], 0, 1e-13, apply).is_err());
            }
        }
    }

    #[test]
    fn reduced_cg_matches_dense_spd_products_and_refuses_unfinished_solves() {
        for n in [2_usize, 7, 31] {
            let mut matrix = vec![0.; n * n];
            for i in 0..n {
                matrix[i * n + i] = 2.;
                if i + 1 < n {
                    matrix[i * n + i + 1] = -0.7;
                    matrix[(i + 1) * n + i] = -0.7;
                }
            }
            for scale in [1e-200, 1., 1e200] {
                let truth: Vec<_> = (0..n)
                    .map(|i| ((i as f64 + 0.2) * 0.37).sin() * scale)
                    .collect();
                let rhs: Vec<_> = (0..n)
                    .map(|i| (0..n).map(|j| matrix[i * n + j] * truth[j]).sum())
                    .collect();
                let mut answer = rhs.clone();
                assert!(
                    solve_reduced_cg(&matrix, &mut answer, &mut vec![0.; 3 * n], n, 1e-13).unwrap()
                        <= n
                );
                for (actual, expected) in answer.iter().zip(&truth) {
                    assert!((actual / scale - expected / scale).abs() < 1e-12);
                }
                assert!(
                    solve_reduced_cg(&matrix, &mut rhs.clone(), &mut vec![0.; 3 * n], 0, 1e-13)
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn reduced_cg_refuses_null_negative_and_nonfinite_curvature() {
        for matrix in [
            [1., -1., -1., 1.],
            [-1., 0., 0., -1.],
            [f64::NAN, 0., 0., 1.],
            [f64::INFINITY, 0., 0., 1.],
        ] {
            let mut rhs = [1., 1.];
            assert!(solve_reduced_cg(&matrix, &mut rhs, &mut [0.; 6], 2, 1e-13).is_err());
        }
    }

    #[test]
    fn iterative_products_preserve_independent_weighted_nonzero_residuals() {
        for threads in [1, 4] {
            let team = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            team.install(|| {
                let mut g = Vec::new();
                let mut h = Vec::new();
                let mut x = Vec::new();
                let mut y = Vec::new();
                let mut q = Vec::new();
                let mut truth = Vec::new();
                for a in 0..640 {
                    for slot in 0..6 {
                        let b = (a + 13 * slot) % 80;
                        let value = ((a * 6 + slot) as f64 * 0.23).sin();
                        let error = 0.03 * ((a * 6 + slot) as f64 * 0.71).cos();
                        for repeat in 0..2 {
                            let weight = if repeat == 0 { 0.7 } else { 2.3 };
                            let residual = if repeat == 0 {
                                error / weight
                            } else {
                                -error / weight
                            };
                            g.push(a as f64);
                            h.push(b as f64);
                            x.push(value);
                            q.push(weight);
                            truth.push(residual);
                            y.push(0.37 * value + 0.002 * a as f64 - 0.01 * b as f64 + residual);
                        }
                    }
                }
                let prepared = prepared(&y, &[x.clone(), vec![0.; y.len()]], &g, &h);
                let design = prepared
                    .fit_borrowed_intercept_map_design()
                    .unwrap()
                    .unwrap();
                let plan = Plan::new(&design, 3, 0, threads, 0).unwrap();
                assert!((plan.q as u128).pow(3) / 3 > plan.factor_work_budget);
                for frame in [0, 1] {
                    let weights: Vec<_> = q
                        .iter()
                        .enumerate()
                        .map(|(i, w)| {
                            w * if frame == 0 {
                                1.
                            } else {
                                0.3 + (i / 2 % 13) as f64
                            }
                        })
                        .collect();
                    let config = MapConfig {
                        tolerance: 1e-12,
                        ..MapConfig::default()
                    };
                    let products = try_build(
                        &prepared,
                        &[0, 1],
                        &y,
                        BorrowedMapWeights::Provided(&weights),
                        1.,
                        config,
                        8_000_000,
                        0,
                        &mut Vec::new(),
                    )
                    .unwrap()
                    .expect("iterative product proposal certifies");
                    assert!(products.report.iterations > 1 && products.report.certified());
                    let minimum = products.minimum_workspace_bytes;
                    let bounded = try_build(
                        &prepared,
                        &[0, 1],
                        &y,
                        BorrowedMapWeights::Provided(&weights),
                        1.,
                        config,
                        minimum,
                        0,
                        &mut Vec::new(),
                    )
                    .unwrap()
                    .expect("complete Krylov phase fits its reported minimum");
                    assert_eq!(products.response, bounded.response);
                    assert_eq!(products.moments.gram, bounded.moments.gram);
                    let mut panel = vec![42.; 1024];
                    let capacity = panel.capacity();
                    assert!(try_build(
                        &prepared,
                        &[0, 1],
                        &y,
                        BorrowedMapWeights::Provided(&weights),
                        1.,
                        config,
                        minimum - 1,
                        0,
                        &mut panel
                    )
                    .unwrap()
                    .is_none());
                    assert_eq!(panel.capacity(), capacity);
                    assert!(panel.iter().all(|&x| x == 42.));
                    let beta = products.moments.gram[3] / products.moments.gram[4] * products.scales[0]
                        / products.scales[1];
                    assert!((beta - 0.37).abs() < 1e-10);
                    for i in 0..y.len() {
                        let residual =
                            products.residual(i, 0).unwrap() - beta * products.residual(i, 1).unwrap();
                        assert!((residual - truth[i]).abs() < 1e-10);
                    }
                    let stopped = MapConfig {
                        max_iterations: 1,
                        ..config
                    };
                    assert!(try_build(
                        &prepared,
                        &[0, 1],
                        &y,
                        BorrowedMapWeights::Provided(&weights),
                        1.,
                        stopped,
                        8_000_000,
                        0,
                        &mut Vec::new()
                    )
                    .unwrap()
                    .is_none());
                }
            });
        }
    }

    #[test]
    fn canonical_budget_credits_only_released_capacity() {
        let mut budget = Budget::new(64);
        let first = budget.reserve(6, 0_f64).unwrap();
        let second = budget.reserve(2, 0_u64).unwrap();
        assert_eq!(budget.remaining, 0);
        assert!(budget.set_limit(63).is_err());
        assert_eq!(budget.limit, 64);
        budget.set_limit(80).unwrap();
        assert_eq!(budget.remaining, 16);
        budget.set_limit(64).unwrap();
        assert!(budget.reserve(1, 0_u8).is_err());
        budget.release(first).unwrap();
        let third = budget.reserve(12, 0_u32).unwrap();
        assert_eq!(budget.peak, 64);
        assert_eq!(budget.remaining, 0);
        budget.release(second).unwrap();
        budget.release(third).unwrap();
        assert_eq!(budget.remaining, budget.limit);
    }

    #[test]
    fn canonical_phase_peak_admits_wide_weighted_oracle_within_the_existing_pool() {
        use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
        use crate::ols::prepared_fit::LinearSolveFrame;
        use crate::ols::{
            PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig,
        };
        let n = 6144;
        let k = 72;
        let workers = 4;
        let allowance = (n * 10 * 16) as u128;
        let g: Vec<_> = (0..n).map(|i| (i / 6) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| ((i / 6 + i % 6) % 24) as f64).collect();
        let x: Vec<Vec<_>> = (0..k)
            .map(|j| {
                (0..n)
                    .map(|i| {
                        (i as f64 * (j as f64 + 1.618)).sin()
                            + 0.3 * ((i + j) as f64 / (j + 1) as f64).cos()
                    })
                    .collect()
            })
            .collect();
        let truth: Vec<_> = (0..k).map(|j| 0.005 * (j + 1) as f64).collect();
        let y: Vec<_> = (0..n)
            .map(|i| {
                0.01 * g[i] - 0.02 * h[i] + x.iter().zip(&truth).map(|(x, b)| x[i] * b).sum::<f64>()
            })
            .collect();
        let prepared = prepared(&y, &x, &g, &h);
        let design = prepared
            .fit_borrowed_intercept_map_design()
            .unwrap()
            .unwrap();
        let plan = Plan::new(&design, k + 1, 0, workers, 0).unwrap();
        let (p, q, w) = (plan.p, plan.q, plan.width);
        let previous_sum = 8
            * (2 * n
                + p
                + 3 * q
                + 2 * q * q
                + p * w
                + 3 * q * w
                + 3 * w
                + w * w
                + workers * plan.stride)
            + 4 * q * size_of::<usize>()
            + 32 * w
            + 2048;
        assert!(previous_sum as u128 > allowance);
        assert!(plan.bytes <= allowance);
        let mut config = PreparedOlsFitConfig::default();
        config.absorb.solver = FeSolverKind::Map;
        config.execution.mode = PreparedOlsExecutionMode::Low;
        config.execution.poolsize = 10;
        config.execution.threads =
            crate::options::ThreadConfig::from_requested(workers, true).unwrap();
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for frame in [0, 1] {
            let weights: Vec<_> = (0..n)
                .map(|i| {
                    if frame == 0 {
                        1.
                    } else {
                        0.2 + (i % 17) as f64 / 7.
                    }
                })
                .collect();
            let divisor = weights.iter().copied().fold(0_f64, f64::max);
            let scale = PreparedOlsBreadWeightScale {
                weight_kind: None,
                solver_weight_divisor: divisor,
                reference_weight_multiplier: divisor,
                reference_weight_sum: weights.iter().sum(),
            };
            team.install(|| {
                let products = try_build(
                    &prepared,
                    &(0..k).collect::<Vec<_>>(),
                    &y,
                    BorrowedMapWeights::Provided(&weights),
                    divisor,
                    config.absorb.map,
                    allowance,
                    0,
                    &mut Vec::new(),
                )
                .unwrap()
                .unwrap();
                assert!(products.report.certified());
                assert!(try_build(
                    &prepared,
                    &(0..k).collect::<Vec<_>>(),
                    &y,
                    BorrowedMapWeights::Provided(&weights),
                    divisor,
                    config.absorb.map,
                    products.minimum_workspace_bytes - 1,
                    0,
                    &mut Vec::new()
                )
                .unwrap()
                .is_none());
            });
            let frame =
                LinearSolveFrame::from_parts(&y, BorrowedMapWeights::Provided(&weights), scale)
                    .unwrap();
            let mut session = CertifiedLinearSession::new_with_ppml_fe_design(
                &prepared,
                frame,
                config,
                LinearSessionGenerations::default(),
                prepared.fit_owned_fe_design().unwrap(),
            )
            .unwrap();
            let fit = session.solve_fresh().unwrap();
            assert_eq!(fit.fit().absorption_report().iterations, 1);
            for (coefficient, expected) in fit.fit().coefficients().iter().zip(&truth) {
                assert!((coefficient.value.unwrap() - expected).abs() < 1e-10);
            }
            assert!(fit.fit().residuals().iter().all(|r| r.abs() < 1e-10));
        }
    }

    #[test]
    fn streamed_canonical_groups_admit_wide_nonzero_weighted_contrasts() {
        weighted_streaming_case(8640, 100, 36, 1, &[1, 8], false);
    }

    #[test]
    fn tiled_canonical_output_matches_independent_wls_under_changing_weights() {
        weighted_streaming_case(14400, 144, 32, 1, &[1, 8], true);
    }

    #[test]
    fn column_owned_rhs_admits_disconnected_wls_below_the_old_reduction_peak() {
        weighted_streaming_case(16320, 100, 124, 4, &[4], true);
    }

    fn weighted_streaming_case(
        n: usize, k: usize, q_per_component: usize, components: usize,
        worker_counts: &[usize], tile_output: bool,
    ) {
        let q = q_per_component * components;
        use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
        use crate::ols::prepared_fit::LinearSolveFrame;
        use crate::ols::{
            PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig,
        };
        let g: Vec<_> = (0..n).map(|i| (i / 6) as f64).collect();
        let h: Vec<_> = (0..n)
            .map(|i| (((i / 6 / components + 11 * ((i % 6) / 2)) % q_per_component)
                + (i / 6 % components) * q_per_component) as f64)
            .collect();
        // Both rows in each pair share all FE and control coordinates. Their
        // reciprocal-weight errors are orthogonal to the complete explicit
        // design, giving a nonzero-residual WLS oracle without either solver.
        let x: Vec<Vec<_>> = (0..k)
            .map(|j| {
                (0..n)
                    .map(|i| {
                        let row = (i / 2) as f64;
                        (row * (j as f64 + 1.618)).sin()
                            + 0.3 * ((row + j as f64) / (j + 1) as f64).cos()
                    })
                    .collect()
            })
            .collect();
        let truth: Vec<_> = (0..k).map(|j| 0.002 * (j + 1) as f64).collect();
        let predictor: Vec<_> = (0..n)
            .map(|i| {
                0.02 * g[i] - 0.03 * h[i] + x.iter().zip(&truth).map(|(x, b)| x[i] * b).sum::<f64>()
            })
            .collect();
        let prepared = prepared(&predictor, &x, &g, &h);
        let allowance = (n * 10 * 16) as u128;
        for &threads in worker_counts {
            let team = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            team.install(|| {
                let design = prepared
                    .fit_borrowed_intercept_map_design()
                    .unwrap()
                    .unwrap();
                let plan = Plan::new(&design, k + 1, 0, threads, 0).unwrap();
                let w = k + 1;
                if components > 1 {
                    let matrix = components * q_per_component * q_per_component;
                    let factor = components * (q_per_component - 1) * (q_per_component - 1);
                    let old_solve = 8 * (n + plan.p + 2*q + factor + q*w + 2*w
                        + threads * (q*w + 3*w))
                        + 6*q*size_of::<usize>() + 32*w + 2048;
                    assert!(old_solve as u128 > allowance);
                    assert!(plan.minimum_storage_bytes(matrix, factor).unwrap() <= allowance);
                }
                let previous_certificate = 8
                    * (n + n / 6
                        + q
                        + n / 6 * w
                        + q * w
                        + 3 * w
                        + w * w
                        + threads * (w * w + q * w + 5 * w))
                    + 32 * w
                    + 2048;
                assert!(previous_certificate as u128 > allowance);
                assert!(plan.bytes <= allowance);
                let mut config = PreparedOlsFitConfig::default();
                config.absorb.solver = FeSolverKind::Map;
                config.absorb.map.tolerance = 1e-12;
                config.execution.mode = PreparedOlsExecutionMode::Low;
                config.execution.poolsize = 10;
                config.execution.threads =
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                for frame in [0, 1] {
                    let ratio = 2. + frame as f64;
                    let weights: Vec<_> = (0..n)
                        .map(|i| {
                            (0.3 + ((i / 2 + 7 * frame) % 31) as f64 * 0.02)
                                * if i % 2 == 0 { 1. } else { ratio }
                        })
                        .collect();
                    let errors: Vec<_> = (0..n)
                        .map(|i| {
                            0.01 * ((i / 2) % 7 + 1) as f64
                                * if i % 2 == 0 { 1. } else { -1. / ratio }
                        })
                        .collect();
                    let y: Vec<_> = predictor.iter().zip(&errors).map(|(a, b)| a + b).collect();
                    let divisor = weights.iter().copied().fold(0_f64, f64::max);
                    let scale = PreparedOlsBreadWeightScale {
                        weight_kind: None,
                        solver_weight_divisor: divisor,
                        reference_weight_multiplier: divisor,
                        reference_weight_sum: weights.iter().sum(),
                    };
                    let mut old_panel = vec![42.; n * 10];
                    let old_capacity = old_panel.capacity();
                    let topology = plan.topology_storage_bytes().unwrap();
                    // The earlier preflight refused this live-panel frame by
                    // charging reductions that do not exist during topology.
                    if threads == 8 {
                        assert!(plan.bytes > allowance - old_capacity as u128 * 8);
                    }
                    assert!(topology <= allowance - old_capacity as u128 * 8);
                    assert!(try_build(
                        &prepared,
                        &(0..k).collect::<Vec<_>>(),
                        &y,
                        BorrowedMapWeights::Provided(&weights),
                        divisor,
                        config.absorb.map,
                        old_capacity as u128 * 8 + topology - 1,
                        0,
                        &mut old_panel,
                    )
                    .unwrap()
                    .is_none());
                    assert_eq!(old_panel.capacity(), old_capacity);
                    assert!(old_panel.iter().all(|&x| x == 42.));
                    let mut products = try_build(
                        &prepared,
                        &(0..k).collect::<Vec<_>>(),
                        &y,
                        BorrowedMapWeights::Provided(&weights),
                        divisor,
                        config.absorb.map,
                        allowance,
                        0,
                        &mut old_panel,
                    )
                    .unwrap()
                    .unwrap();
                    assert!(products.report.certified());
                    let selected_bytes = products.report.solver_workspace_peak.bytes().unwrap() as u128;
                    if selected_bytes > allowance - old_capacity as u128 * 8 {
                        assert_eq!(old_panel.capacity(), 0);
                    } else {
                        assert_eq!(old_panel.capacity(), old_capacity);
                        assert!(old_panel.iter().all(|&x| x == 42.));
                    }
                    assert!(!products.retained_centers);
                    assert_eq!(products.tiled.is_some(), tile_output);
                    let bytes = products.minimum_workspace_bytes;
                    assert!(
                        products.report.solver_workspace_peak.bytes().unwrap() as u128 <= allowance
                    );
                    let cached = try_build(
                        &prepared,
                        &(0..k).collect::<Vec<_>>(),
                        &y,
                        BorrowedMapWeights::Provided(&weights),
                        divisor,
                        config.absorb.map,
                        u128::MAX,
                        0,
                        &mut Vec::new(),
                    )
                    .unwrap()
                    .unwrap();
                    assert!(cached.retained_centers);
                    let mut dense = DenseMatrix::zeros(n, k).unwrap();
                    products.write_dense(&mut dense).unwrap();
                    for i in 0..n {
                        for j in 0..k {
                            assert_eq!(dense[(i, j)], cached.residual(i, j + 1).unwrap());
                        }
                    }
                    let mut misaligned = DenseMatrix::zeros(n - 1, k).unwrap();
                    assert_eq!(products.write_dense(&mut misaligned).unwrap_err().code, ErrorCode::InvalidArgument);
                    assert!(misaligned.as_slice().iter().all(|&x| x == 0.));
                    if tile_output {
                        let mut scratch = PreparedOlsScratch::create_row_major(n, k).unwrap();
                        products.write_controls(&mut scratch).unwrap();
                        let mut row = vec![0.; k];
                        let mut reader = scratch.shared_reader();
                        for i in 0..n {
                            reader.read_linear(i * k, &mut row).unwrap();
                            for (j, &actual) in row.iter().enumerate() {
                                assert_eq!(actual, cached.residual(i, j + 1).unwrap());
                            }
                        }
                        assert!(products.left.is_empty());
                    } else {
                        assert_eq!(products.left, cached.left);
                    }
                    assert_eq!(products.right, cached.right);
                    assert_eq!(products.response, cached.response);
                    assert_eq!(products.moments.gram, cached.moments.gram);
                    assert_eq!(products.means, cached.means);
                    drop(cached);
                    let mut refused_panel = vec![42.; n * 10];
                    let refused_capacity = refused_panel.capacity();
                    assert!(try_build(
                        &prepared,
                        &(0..k).collect::<Vec<_>>(),
                        &y,
                        BorrowedMapWeights::Provided(&weights),
                        divisor,
                        config.absorb.map,
                        bytes - 1,
                        0,
                        &mut refused_panel
                    )
                    .unwrap()
                    .is_none());
                    assert_eq!(refused_panel.capacity(), refused_capacity);
                    assert!(refused_panel.iter().all(|&x| x == 42.));
                    // The bank rebuilt after certification must reproduce the
                    // very residuals whose direct products supplied the Gram.
                    for j in [0, 1, k] {
                        let norm = nonzero(products.scales[j]);
                        let direct: f64 = (0..n)
                            .map(|i| {
                                let value = products.residual(i, j).unwrap() / norm;
                                (weights[i] / divisor) * value * value
                            })
                            .sum();
                        assert!((direct - products.moments.gram[j * (k + 1) + j]).abs() < 1e-11);
                    }
                    drop(products);
                    let frame = LinearSolveFrame::from_parts(
                        &y,
                        BorrowedMapWeights::Provided(&weights),
                        scale,
                    )
                    .unwrap();
                    let mut session = CertifiedLinearSession::new_with_ppml_fe_design(
                        &prepared,
                        frame,
                        config,
                        LinearSessionGenerations::default(),
                        prepared.fit_owned_fe_design().unwrap(),
                    )
                    .unwrap();
                    let result = session.solve_fresh().unwrap();
                    assert_eq!(result.fit().absorption_report().iterations, 1);
                    assert!(result.fit().score_certificate().passed);
                    for (actual, expected) in result.fit().coefficients().iter().take(k).zip(&truth)
                    {
                        assert!((actual.value.unwrap() - expected).abs() < 1e-9);
                    }
                    for (actual, expected) in result.fit().residuals().iter().zip(&errors) {
                        assert!((actual - expected).abs() < 1e-9);
                    }
                }
            });
        }
    }

    #[test]
    fn canonical_work_history_admits_changed_weights_and_reclaims_only_after_preflight() {
        use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
        use crate::ols::prepared_fit::LinearSolveFrame;
        use crate::ols::{
            PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig,
        };
        let n = 19200;
        // Four-member groups keep the complete implicit owner above this
        // allowance; Q158 keeps dense direct storage affordable only after
        // observed work permits eliminating its extra iterative workspace.
        let g: Vec<_> = (0..n).map(|i| (i / 4) as f64).collect();
        let h: Vec<_> = (0..n)
            .map(|i| ((i / 4 + 13 * (i % 4)) % 158) as f64)
            .collect();
        let x: Vec<Vec<_>> = (0..2)
            .map(|j| {
                (0..n)
                    .map(|i| (i as f64 * (0.17 + j as f64 * 0.09)).sin())
                    .collect()
            })
            .collect();
        let y: Vec<_> = (0..n)
            .map(|i| 0.25 * x[0][i] - 0.4 * x[1][i] + 0.02 * g[i] - 0.03 * h[i])
            .collect();
        let prepared = prepared(&y, &x, &g, &h);
        let allowance = (n * 2 * 16) as u128;
        for threads in [1, 4] {
            let team = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            team.install(|| {
                let mut config = PreparedOlsFitConfig::default();
                config.absorb.solver = FeSolverKind::Map;
                config.absorb.map.tolerance = 1e-12;
                config.execution.mode = PreparedOlsExecutionMode::Low;
                config.execution.poolsize = 2;
                config.execution.threads =
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                let mut scale = PreparedOlsBreadWeightScale {
                    weight_kind: None,
                    solver_weight_divisor: 1.,
                    reference_weight_multiplier: 1.,
                    reference_weight_sum: n as f64,
                };
                let frame =
                    LinearSolveFrame::from_parts(&y, BorrowedMapWeights::Unit, scale).unwrap();
                let mut session = CertifiedLinearSession::new_with_ppml_fe_design(
                    &prepared,
                    frame,
                    config,
                    LinearSessionGenerations::default(),
                    prepared.fit_owned_fe_design().unwrap(),
                )
                .unwrap();
                let cold = session.solve_fresh().unwrap();
                assert!(cold.fit().absorption_report().iterations > 1);
                for (coefficient, expected) in
                    cold.fit().coefficients().iter().take(2).zip([0.25, -0.4])
                {
                    assert!((coefficient.value.unwrap() - expected).abs() < 1e-9);
                }
                let observed = session.canonical_projection_work_hint_for_test();
                assert!(observed > 0);
                let mut old_panel = vec![42.; n * 2];
                let old_capacity = old_panel.capacity();
                for (budget, work) in [(allowance, 0), (allowance, u128::MAX), (1, observed)] {
                    assert!(try_build(
                        &prepared,
                        &[0, 1],
                        &y,
                        BorrowedMapWeights::Unit,
                        1.,
                        config.absorb.map,
                        budget,
                        work,
                        &mut old_panel
                    )
                    .unwrap()
                    .is_none());
                    assert_eq!(old_panel.capacity(), old_capacity);
                    assert!(old_panel.iter().all(|&x| x == 42.));
                }
                let product = try_build(
                    &prepared,
                    &[0, 1],
                    &y,
                    BorrowedMapWeights::Unit,
                    1.,
                    config.absorb.map,
                    allowance,
                    observed,
                    &mut old_panel,
                )
                .unwrap()
                .unwrap();
                assert!(product.report.certified());
                assert_eq!(old_panel.capacity(), 0);
                assert!(
                    product.report.solver_workspace_peak.bytes().unwrap() as u128 > allowance / 2
                );
                drop(product);
                let changed: Vec<_> = (0..n).map(|i| 0.3 + (i % 31) as f64 * 0.02).collect();
                scale.solver_weight_divisor = changed.iter().copied().fold(0_f64, f64::max);
                scale.reference_weight_multiplier = scale.solver_weight_divisor;
                scale.reference_weight_sum = changed.iter().sum();
                for commit in [false, true] {
                    session
                        .refresh_weights(
                            BorrowedMapWeights::Provided(&changed),
                            scale,
                            1 + u64::from(commit),
                        )
                        .unwrap();
                    let candidate = session.strict_candidate().unwrap();
                    let fit = candidate.candidate();
                    assert_eq!(fit.fit().absorption_report().iterations, 1);
                    assert!(fit.fit().score_certificate().passed);
                    for (coefficient, expected) in
                        fit.fit().coefficients().iter().take(2).zip([0.25, -0.4])
                    {
                        assert!((coefficient.value.unwrap() - expected).abs() < 1e-9);
                    }
                    assert!(fit.fit().residuals().iter().all(|r| r.abs() < 1e-9));
                    if commit {
                        candidate.commit();
                    }
                }
                session.refresh_basis(2).unwrap();
                assert_eq!(session.canonical_projection_work_hint_for_test(), 0);
            });
        }
    }

    #[test]
    fn canonical_products_session_keeps_rank_and_strict_rollback_across_weights() {
        use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
        use crate::ols::prepared_fit::LinearSolveFrame;
        use crate::ols::{
            PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig,
        };
        let n = 4096;
        let g: Vec<_> = (0..n).map(|i| (i % 8) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| ((i / 9) % 6) as f64).collect();
        for duplicate in [false, true] {
            let mut x: Vec<Vec<_>> = (0..8)
                .map(|j| {
                    (0..n)
                        .map(|i| {
                            (i as f64 / (13 + 7 * j) as f64).sin()
                                + (i % (7 + j)) as f64 / (5 + j) as f64
                        })
                        .collect()
                })
                .collect();
            if duplicate {
                x[7] = x[2].clone();
            }
            let y: Vec<_> = (0..n)
                .map(|i| {
                    0.2 * g[i] - 0.1 * h[i]
                        + x.iter()
                            .enumerate()
                            .map(|(j, x)| 0.03 * (j + 1) as f64 * x[i])
                            .sum::<f64>()
                })
                .collect();
            let prepared = prepared(&y, &x, &g, &h);
            for (threads, mode, admitted) in [
                (1, PreparedOlsExecutionMode::Low, true),
                (4, PreparedOlsExecutionMode::Low, true),
                (1, PreparedOlsExecutionMode::Fast, true),
                (4, PreparedOlsExecutionMode::Fast, true),
                (4, PreparedOlsExecutionMode::Fast, false),
            ] {
                let mut config = PreparedOlsFitConfig::default();
                config.absorb.solver = FeSolverKind::Map;
                config.execution.mode = mode;
                config.execution.poolsize = 8;
                config.execution.threads =
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                let mut scale = PreparedOlsBreadWeightScale {
                    weight_kind: None,
                    solver_weight_divisor: 1.,
                    reference_weight_multiplier: 1.,
                    reference_weight_sum: n as f64,
                };
                let frame =
                    LinearSolveFrame::from_parts(&y, BorrowedMapWeights::Unit, scale).unwrap();
                let mut session = CertifiedLinearSession::new_with_ppml_fe_design(
                    &prepared,
                    frame,
                    config,
                    LinearSessionGenerations::default(),
                    prepared.fit_owned_fe_design().unwrap(),
                )
                .unwrap();
                if mode == PreparedOlsExecutionMode::Fast {
                    session.enable_fast_canonical_products(if admitted { 16 * n as u128 * 8 } else { 1 }).unwrap();
                }
                let original = session.solve_fresh().unwrap();
                // A direct certified solve reports one solve, proving that
                // this test reaches the fused handoff rather than its fallback.
                if admitted { assert_eq!(original.fit().absorption_report().iterations, 1); }
                assert!(original.fit().residuals().iter().all(|r| r.abs() < 1e-10));
                let statuses: Vec<_> = original
                    .fit()
                    .coefficients()
                    .iter()
                    .map(|c| c.status)
                    .collect();
                let changed: Vec<_> = (0..n).map(|i| 0.2 + (i % 17) as f64 / 7.).collect();
                scale.solver_weight_divisor = changed.iter().copied().fold(0_f64, f64::max);
                scale.reference_weight_multiplier = scale.solver_weight_divisor;
                scale.reference_weight_sum = changed.iter().sum();
                let next_y: Vec<_> = y.iter().zip(&x[0]).map(|(y, x)| y + 0.4*x).collect();
                session.refresh_response(&next_y, 1).unwrap();
                session
                    .refresh_weights(BorrowedMapWeights::Provided(&changed), scale, 1)
                    .unwrap();
                // A rank failure occurs after the direct writer changed the
                // physical dense bank. Retrying must use the matching response
                // anchor and never publish the failed candidate.
                session.inject_fresh_rank_failure(ErrorCode::Interrupted);
                assert!(session.strict_candidate().is_err());
                // A dropped speculative candidate cannot publish its basis.
                {
                    let candidate = session.strict_candidate().unwrap();
                    assert!(candidate
                        .candidate()
                        .fit()
                        .residuals()
                        .iter()
                        .all(|r| r.abs() < 1e-10));
                }
                let candidate = session.strict_candidate().unwrap();
                assert_eq!(
                    candidate
                        .candidate()
                        .fit()
                        .coefficients()
                        .iter()
                        .map(|c| c.status)
                        .collect::<Vec<_>>(),
                    statuses
                );
                candidate.commit();
                assert!(session.output().unwrap().fit().score_certificate().passed);
                // Same-weight response-only solving bypasses direct products;
                // this protects the handoff back to the existing projector.
                session.refresh_response(&y, 2).unwrap();
                let recovered = session.solve_fresh().unwrap();
                assert!(recovered.fit().score_certificate().passed);
                assert!(recovered.fit().residuals().iter().all(|r| r.abs() < 1e-9));
                assert_eq!(recovered.fit().coefficients().iter().map(|c| c.status).collect::<Vec<_>>(), statuses);
            }
        }
    }

    #[test]
    fn canonical_three_products_session_keeps_rank_and_strict_rollback_across_weights() {
        use crate::ols::linear_session::{CertifiedLinearSession, LinearSessionGenerations};
        use crate::ols::prepared_fit::LinearSolveFrame;
        use crate::ols::{
            PreparedOlsBreadWeightScale, PreparedOlsExecutionMode, PreparedOlsFitConfig,
        };
        let n = 4096;
        let g: Vec<_> = (0..n).map(|i| (i % 8) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| ((i / 9) % 6) as f64).collect();
        let z: Vec<_> = (0..n).map(|i| ((i / 31) % 5) as f64).collect();
        for duplicate in [false, true] {
            let mut x: Vec<Vec<_>> = (0..8)
                .map(|j| {
                    (0..n)
                        .map(|i| {
                            (i as f64 / (13 + 7 * j) as f64).sin()
                                + (i % (7 + j)) as f64 / (5 + j) as f64
                        })
                        .collect()
                })
                .collect();
            if duplicate {
                x[7] = x[2].clone();
            }
            let y: Vec<_> = (0..n)
                .map(|i| {
                    0.2 * g[i] - 0.1 * h[i] + 0.07 * z[i]
                        + x.iter()
                            .enumerate()
                            .map(|(j, x)| 0.03 * (j + 1) as f64 * x[i])
                            .sum::<f64>()
                })
                .collect();
            let prepared = prepared_effects(&y, &x, &[&g, &h, &z]);
            for (threads, mode, admitted) in [
                (1, PreparedOlsExecutionMode::Low, true),
                (4, PreparedOlsExecutionMode::Low, true),
                (1, PreparedOlsExecutionMode::Fast, true),
                (4, PreparedOlsExecutionMode::Fast, true),
                (4, PreparedOlsExecutionMode::Fast, false),
            ] {
                let mut config = PreparedOlsFitConfig::default();
                config.absorb.solver = FeSolverKind::Map;
                config.execution.mode = mode;
                config.execution.poolsize = 8;
                config.execution.threads =
                    crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                let mut scale = PreparedOlsBreadWeightScale {
                    weight_kind: None,
                    solver_weight_divisor: 1.,
                    reference_weight_multiplier: 1.,
                    reference_weight_sum: n as f64,
                };
                let frame =
                    LinearSolveFrame::from_parts(&y, BorrowedMapWeights::Unit, scale).unwrap();
                let mut session = CertifiedLinearSession::new_with_ppml_fe_design(
                    &prepared,
                    frame,
                    config,
                    LinearSessionGenerations::default(),
                    prepared.fit_owned_fe_design().unwrap(),
                )
                .unwrap();
                if mode == PreparedOlsExecutionMode::Fast {
                    session.enable_fast_canonical_products(if admitted { 16 * n as u128 * 8 } else { 1 }).unwrap();
                }
                let original = session.solve_fresh().unwrap();
                // Independent product tests establish the direct route. This
                // test checks publication authority across speculative writes.
                assert!(original.fit().absorption_report().certified());
                assert!(original.fit().residuals().iter().all(|r| r.abs() < 1e-10));
                let statuses: Vec<_> = original
                    .fit()
                    .coefficients()
                    .iter()
                    .map(|c| c.status)
                    .collect();
                let changed: Vec<_> = (0..n).map(|i| 0.2 + (i % 17) as f64 / 7.).collect();
                scale.solver_weight_divisor = changed.iter().copied().fold(0_f64, f64::max);
                scale.reference_weight_multiplier = scale.solver_weight_divisor;
                scale.reference_weight_sum = changed.iter().sum();
                let next_y: Vec<_> = y.iter().zip(&x[0]).map(|(y, x)| y + 0.4*x).collect();
                session.refresh_response(&next_y, 1).unwrap();
                session
                    .refresh_weights(BorrowedMapWeights::Provided(&changed), scale, 1)
                    .unwrap();
                // A rank failure occurs after the direct writer changed the
                // physical dense bank. Retrying must use the matching response
                // anchor and never publish the failed candidate.
                session.inject_fresh_rank_failure(ErrorCode::Interrupted);
                assert!(session.strict_candidate().is_err());
                // A dropped speculative candidate cannot publish its basis.
                {
                    let candidate = session.strict_candidate().unwrap();
                    assert!(candidate
                        .candidate()
                        .fit()
                        .residuals()
                        .iter()
                        .all(|r| r.abs() < 1e-10));
                }
                let candidate = session.strict_candidate().unwrap();
                assert_eq!(
                    candidate
                        .candidate()
                        .fit()
                        .coefficients()
                        .iter()
                        .map(|c| c.status)
                        .collect::<Vec<_>>(),
                    statuses
                );
                candidate.commit();
                assert!(session.output().unwrap().fit().score_certificate().passed);
                // Same-weight response-only solving bypasses direct products;
                // this protects the handoff back to the existing projector.
                session.refresh_response(&y, 2).unwrap();
                let recovered = session.solve_fresh().unwrap();
                assert!(recovered.fit().score_certificate().passed);
                assert!(recovered.fit().residuals().iter().all(|r| r.abs() < 1e-9));
                assert_eq!(recovered.fit().coefficients().iter().map(|c| c.status).collect::<Vec<_>>(), statuses);
            }
        }
    }

    #[test]
    fn canonical_products_match_independent_weighted_contrasts_and_direct_gram() {
        for threads in [1, 4, 8] {
            let team = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            team.install(|| {
                for mass in [1., 1e-80, 1e80] {
                    for shift in [0., 0.7] {
                        let wa: Vec<_> = (0..6).map(|a| 1. + a as f64 + shift).collect();
                        let wb: Vec<_> = (0..5).map(|b| 0.5 + b as f64 + shift).collect();
                        let ma = wa
                            .iter()
                            .enumerate()
                            .map(|(i, w)| i as f64 * w)
                            .sum::<f64>()
                            / wa.iter().sum::<f64>();
                        let mb = wb
                            .iter()
                            .enumerate()
                            .map(|(i, w)| i as f64 * w)
                            .sum::<f64>()
                            / wb.iter().sum::<f64>();
                        let mut g = Vec::new();
                        let mut h = Vec::new();
                        let mut weights = Vec::new();
                        let mut values = vec![Vec::new(); 4];
                        let mut truth = vec![Vec::new(); 4];
                        let scales = [1., 1e70, 1e-70, 1.];
                        for component in 0..2 {
                            for a in 0..6 {
                                for b in 0..5 {
                                    for t in 0..3 {
                                        g.push((component * 6 + a) as f64);
                                        h.push((component * 5 + b) as f64);
                                        weights.push(
                                            wa[a]
                                                * wb[b]
                                                * if component == 0 { mass } else { 1. / mass },
                                        );
                                        let ab = (a as f64 - ma) * (b as f64 - mb);
                                        let noise = t as f64 - 1.;
                                        for (j, r) in [ab, noise, ab + noise * (b as f64 - mb), 0.]
                                            .into_iter()
                                            .enumerate()
                                        {
                                            truth[j].push(r * scales[j]);
                                            values[j].push(
                                                (r + 0.3 * a as f64 - 0.2 * b as f64
                                                    + component as f64)
                                                    * scales[j],
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        let p = prepared(&values[0], &values[1..], &g, &h);
                        let max = weights.iter().copied().fold(0_f64, f64::max);
                        let config = MapConfig {
                            tolerance: 1e-12,
                            ..MapConfig::default()
                        };
                        let mut result = try_build(
                            &p,
                            &[0, 1, 2],
                            &values[0],
                            BorrowedMapWeights::Provided(&weights),
                            max,
                            config,
                            u128::MAX,
                            0,
                            &mut Vec::new(),
                        )
                        .unwrap()
                        .expect("admitted exact contrasts");
                        assert!(result.report.certified());
                        let mut scratch = PreparedOlsScratch::create_row_major(g.len(), 3).unwrap();
                        result.write_controls(&mut scratch).unwrap();
                        let mut row = [0.; 3];
                        for i in 0..g.len() {
                            scratch.read_linear_shared(i * 3, &mut row).unwrap();
                            assert!((result.response[i] - truth[0][i]).abs() < 2e-12);
                            for j in 1..4 {
                                assert!((row[j - 1] - truth[j][i]).abs() / scales[j] < 2e-12);
                            }
                        }
                        for j in 0..4 {
                            for k in 0..4 {
                                let expected = (0..g.len())
                                    .map(|i| {
                                        (weights[i] / max)
                                            * (truth[j][i] / nonzero(result.scales[j]))
                                            * (truth[k][i] / nonzero(result.scales[k]))
                                    })
                                    .sum::<f64>();
                                assert!((result.moments.gram[j * 4 + k] - expected).abs() < 2e-13);
                            }
                        }
                        assert!(result.norms[3] / result.scales[3] < 2e-14);
                        let bytes = result.minimum_workspace_bytes;
                        assert!(try_build(
                            &p,
                            &[0, 1, 2],
                            &values[0],
                            BorrowedMapWeights::Provided(&weights),
                            max,
                            config,
                            bytes - 1,
                            0,
                            &mut Vec::new()
                        )
                        .unwrap()
                        .is_none());
                        assert!(try_build(
                            &p,
                            &[0, 1, 2],
                            &values[0],
                            BorrowedMapWeights::Provided(&weights),
                            max,
                            config,
                            bytes,
                            0,
                            &mut Vec::new()
                        )
                        .unwrap()
                        .is_some());
                    }
                }
            });
        }
    }

    #[test]
    fn canonical_component_factors_admit_disconnected_weighted_contrasts() {
        for threads in [1, 4] {
            let team = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            team.install(|| {
                for transpose in [false, true] {
                    for shift in [0., 0.7] {
                        let (mut g, mut h, mut weights) = (Vec::new(), Vec::new(), Vec::new());
                        let mut values = vec![Vec::new(); 3];
                        let mut truth = vec![Vec::new(); 3];
                        for component in 0..24 {
                            // Unequal component dimensions include one retained level
                            // (no factor) and noncontiguous labels across components.
                            let na = 4 + component % 3;
                            let nb = 1 + component % 4;
                            let wa: Vec<_> = (0..na).map(|a| 1. + a as f64 + shift).collect();
                            let wb: Vec<_> = (0..nb).map(|b| 0.5 + b as f64 + shift).collect();
                            let ma = wa
                                .iter()
                                .enumerate()
                                .map(|(a, w)| a as f64 * w)
                                .sum::<f64>()
                                / wa.iter().sum::<f64>();
                            let mb = wb
                                .iter()
                                .enumerate()
                                .map(|(b, w)| b as f64 * w)
                                .sum::<f64>()
                                / wb.iter().sum::<f64>();
                            for a in 0..na {
                                for b in 0..nb {
                                    for t in 0..4 {
                                        g.push((a * 24 + component) as f64);
                                        h.push((b * 24 + component) as f64);
                                        weights.push(
                                            wa[a]
                                                * wb[b]
                                                * if component % 2 == 0 { 0.03 } else { 30. },
                                        );
                                        let ab = (a as f64 - ma) * (b as f64 - mb);
                                        for (j, r) in
                                            [ab, t as f64 - 1.5, ab + 0.3 * (t as f64 - 1.5)]
                                                .into_iter()
                                                .enumerate()
                                        {
                                            truth[j].push(r);
                                            values[j].push(
                                                r + 0.2 * a as f64 - 0.1 * b as f64
                                                    + component as f64,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        if transpose {
                            std::mem::swap(&mut g, &mut h);
                        }
                        let prepared = prepared(&values[0], &values[1..], &g, &h);
                        let design = prepared
                            .fit_borrowed_intercept_map_design()
                            .unwrap()
                            .unwrap();
                        let left = usize::from(
                            design.effects()[1].n_levels() > design.effects()[0].n_levels(),
                        );
                        let plan = Plan::new(&design, 3, left, threads, 0).unwrap();
                        assert!(
                            (plan.q as u128).pow(3) / 3 > plan.factor_work_budget,
                            "the global factor bound must reject this fixture"
                        );
                        let allowance = (g.len() * 2 * 16) as u128;
                        assert!(
                            plan.storage_bytes(plan.q * plan.q, plan.q * plan.q, false)
                                .unwrap()
                                > allowance
                        );
                        let divisor = weights.iter().copied().fold(0_f64, f64::max);
                        let result = try_build(
                            &prepared,
                            &[0, 1],
                            &values[0],
                            BorrowedMapWeights::Provided(&weights),
                            divisor,
                            MapConfig {
                                tolerance: 1e-12,
                                ..MapConfig::default()
                            },
                            allowance,
                            0,
                            &mut Vec::new(),
                        )
                        .unwrap()
                        .expect("component factors admitted");
                        assert!(result.report.certified());
                        for j in 0..3 {
                            for i in 0..g.len() {
                                assert!(
                                    (result.residual(i, j).unwrap() - truth[j][i]).abs() < 1e-11
                                );
                            }
                        }
                        for j in 0..3 {
                            for k in 0..3 {
                                let expected = (0..g.len())
                                    .map(|i| {
                                        weights[i] / divisor * truth[j][i] * truth[k][i]
                                            / result.scales[j]
                                            / result.scales[k]
                                    })
                                    .sum::<f64>();
                                assert!((result.moments.gram[j * 3 + k] - expected).abs() < 1e-12);
                            }
                        }
                        assert!(try_build(
                            &prepared,
                            &[0, 1],
                            &values[0],
                            BorrowedMapWeights::Provided(&weights),
                            divisor,
                            MapConfig::default(),
                            result.minimum_workspace_bytes - 1,
                            0,
                            &mut Vec::new()
                        )
                        .unwrap()
                        .is_none());
                    }
                }
            });
        }
    }

    #[test]
    fn canonical_products_refuse_work_and_invalid_frames_without_mutation() {
        let n = 240;
        let g: Vec<_> = (0..n).map(|i| (i % 12) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| ((i / 12) % 5) as f64).collect();
        let x: Vec<_> = (0..n).map(|i| (i as f64).sin()).collect();
        let y: Vec<_> = (0..n).map(|i| g[i] + h[i] + 1e-10 * x[i]).collect();
        let p = prepared(&y, &[x.clone(), vec![0.; n]], &g, &h);
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        team.install(|| {
            let result = try_build(
                &p,
                &[0, 1],
                &y,
                BorrowedMapWeights::Unit,
                1.,
                MapConfig::default(),
                u128::MAX,
                0,
                &mut Vec::new(),
            )
            .unwrap()
            .unwrap();
            assert!(result.moments.gram[0] >= 0. && result.moments.gram[0] < 1e-20);
            assert_eq!(result.scales[2], 0.);
            assert_eq!(result.norms[2], 0.);
            let mut bad = vec![1.; n];
            bad[4] = 0.;
            assert!(try_build(
                &p,
                &[0, 1],
                &y,
                BorrowedMapWeights::Provided(&bad),
                1.,
                MapConfig::default(),
                u128::MAX,
                0,
                &mut Vec::new()
            )
            .unwrap()
            .is_none());
            let mut invalid = y.clone();
            invalid[5] = f64::NAN;
            assert_eq!(
                try_build(
                    &p,
                    &[0, 1],
                    &invalid,
                    BorrowedMapWeights::Unit,
                    1.,
                    MapConfig::default(),
                    u128::MAX,
                    0,
                    &mut Vec::new()
                )
                .err()
                .unwrap()
                .code,
                ErrorCode::InvalidData
            );
            let config = MapConfig {
                tolerance: 1e-30,
                ..MapConfig::default()
            };
            assert!(try_build(
                &p,
                &[0, 1],
                &y,
                BorrowedMapWeights::Unit,
                1.,
                config,
                u128::MAX,
                0,
                &mut Vec::new()
            )
            .unwrap()
            .is_none());
            let large: Vec<_> = (0..n).map(|i| (i % 60) as f64).collect();
            let other: Vec<_> = (0..n).map(|i| ((i / 4 + i) % 60) as f64).collect();
            // The old global-work refusal has multiple small components, so
            // the component engine now legitimately admits its same design.
            let disconnected = prepared(&y, &[x.clone()], &large, &other);
            assert!(try_build(
                &disconnected,
                &[0],
                &y,
                BorrowedMapWeights::Unit,
                1.,
                MapConfig::default(),
                u128::MAX,
                0,
                &mut Vec::new()
            )
            .unwrap()
            .is_some());
            // A connected cycle exceeds dense factor work but can now use a
            // certified iterative proposal. An unfinished proposal still refuses.
            let connected: Vec<_> = (0..n).map(|i| ((i % 60 + i / 60) % 60) as f64).collect();
            let wide = prepared(&y, &[x], &large, &connected);
            assert!(try_build(
                &wide,
                &[0],
                &y,
                BorrowedMapWeights::Unit,
                1.,
                MapConfig::default(),
                u128::MAX,
                0,
                &mut Vec::new()
            )
            .unwrap()
            .is_some_and(|products| products.report.certified() && products.report.iterations > 1));
            assert!(try_build(&wide, &[0], &y, BorrowedMapWeights::Unit, 1.,
                MapConfig {max_iterations:1,..MapConfig::default()},u128::MAX,0,&mut Vec::new())
                .unwrap().is_none());
        });
    }
    #[test]
    fn canonical_three_products_match_independent_weighted_contrasts() {
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                for shift in [0., 0.7] {
                    for mass in [1., 1e-100, 1e100] {
                        for nested in [false, true] {
                            let wa: Vec<_> = (0..6).map(|i| 1. + i as f64 + shift).collect();
                            let wb: Vec<_> = (0..5).map(|i| 0.5 + i as f64 + shift).collect();
                            let wc: Vec<_> = (0..3).map(|i| 0.3 + i as f64 + shift).collect();
                            let mean = |v: &[f64]| v.iter().enumerate().map(|(i,w)| i as f64*w).sum::<f64>() / v.iter().sum::<f64>();
                            let (ma, mb) = (mean(&wa), mean(&wb));
                            let (mut g, mut h, mut z, mut weights) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                            let mut values = vec![Vec::new(); 5];
                            let mut truth = vec![Vec::new(); 5];
                            let scales = [1., 1e30, 1e-30, 1., 1.];
                            for a in 0..6 { for b in 0..5 { for c in 0..3 { for t in 0..2 {
                                g.push(a as f64); h.push(b as f64); z.push(if nested { (b % 3) as f64 } else { c as f64 });
                                weights.push(wa[a]*wb[b]*wc[c]*mass);
                                let ab = (a as f64-ma)*(b as f64-mb);
                                let error = 2.*t as f64-1.;
                                for (j, r) in [ab+error, ab, error, 0., ab].into_iter().enumerate() {
                                    truth[j].push(r*scales[j]);
                                    values[j].push((r+0.2*a as f64-0.1*b as f64+0.3*z[z.len()-1])*scales[j]);
                                }
                            }}}}
                            let p = prepared_effects(&values[0], &values[1..], &[&g, &h, &z]);
                            let divisor = weights.iter().copied().fold(0_f64, f64::max);
                            let cfg = MapConfig { tolerance: 1e-12, ..MapConfig::default() };
                            let mut result = try_build(&p, &[0,1,2,3], &values[0], BorrowedMapWeights::Provided(&weights), divisor, cfg, u128::MAX, 0, &mut Vec::new()).unwrap().expect("three-FE direct products admitted");
                            assert!(result.three.is_some() && result.report.certified());
                            let mut dense = DenseMatrix::zeros(g.len(), 4).unwrap();
                            result.write_dense(&mut dense).unwrap();
                            let mut scratch = PreparedOlsScratch::create_row_major(g.len(), 4).unwrap();
                            result.write_controls(&mut scratch).unwrap();
                            let mut row = [0.;4];
                            for i in 0..g.len() {
                                scratch.read_linear_shared(i*4, &mut row).unwrap();
                                assert!((result.response[i]-truth[0][i]).abs()<1e-10);
                                for j in 1..5 {
                                    assert_eq!(row[j-1], dense[(i,j-1)]);
                                    assert!((row[j-1]-truth[j][i]).abs()/scales[j]<1e-10);
                                }
                            }
                            for j in 0..5 { for k in 0..5 {
                                let expected: f64 = (0..g.len()).map(|i| weights[i]/divisor * (truth[j][i]/nonzero(result.scales[j])) * (truth[k][i]/nonzero(result.scales[k]))).sum();
                                assert!((result.moments.gram[j*5+k]-expected).abs()<1e-11);
                            }}
                            let bytes = result.minimum_workspace_bytes;
                            assert!(try_build(&p, &[0,1,2,3], &values[0], BorrowedMapWeights::Provided(&weights), divisor, cfg, bytes-1, 0, &mut Vec::new()).unwrap().is_none());
                            assert!(try_build(&p, &[0,1,2,3], &values[0], BorrowedMapWeights::Provided(&weights), divisor, cfg, bytes, 0, &mut Vec::new()).unwrap().is_some());
                        }
                    }
                }
            });
        }
    }

    #[test]
    fn canonical_three_products_preserve_disconnected_and_invalid_frame_fallback() {
        let n = 480;
        let g: Vec<_> = (0..n).map(|i| (i%6) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| (i%2*5+(i/6)%5) as f64).collect();
        let z: Vec<_> = (0..n).map(|i| (i%2*4+(i/30)%4) as f64).collect();
        let x = vec![(0..n).map(|i| (i as f64).sin()).collect::<Vec<_>>()];
        let y = x[0].clone();
        let p = prepared_effects(&y, &x, &[&g, &h, &z]);
        let mut panel = vec![1.;48];
        assert!(try_build(&p, &[0], &y, BorrowedMapWeights::Unit, 1., MapConfig::default(), u128::MAX, 0, &mut panel).unwrap().is_none());
        assert_eq!(panel, vec![1.;48]);
        let ch: Vec<_> = (0..n).map(|i| ((i/6)%5) as f64).collect();
        let cz: Vec<_> = (0..n).map(|i| ((i/30)%4) as f64).collect();
        let connected = prepared_effects(&y, &x, &[&g, &ch, &cz]);
        for weights in [vec![0.;n], vec![f64::NAN;n], vec![1.;n-1]] {
            assert!(try_build(&connected, &[0], &y, BorrowedMapWeights::Provided(&weights), 1., MapConfig::default(), u128::MAX, 0, &mut panel).unwrap().is_none());
        }
    }

}
