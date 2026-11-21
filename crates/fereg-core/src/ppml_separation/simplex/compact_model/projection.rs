//! Numerically reduce the boundary cone before allocating a wide panel.
//! The projected columns propose a result. The original binary design
//! independently verifies every accepted ray or terminal dual.
use super::super::compact_tableau;
use super::{fallible_collect, fallible_filled, reserve, trace_refusal, BinaryDesign};
use crate::error::Result;
use std::hash::{DefaultHasher, Hash, Hasher};

mod streamed;
pub(super) use streamed::{build_with as streamed_with, row_key, Grow};

pub(super) struct BoundaryCone {
    pub(super) rows: Vec<Vec<f64>>,
    pub(super) live_cells: usize,
    pub(super) groups: Vec<(usize, usize)>,
    pub(super) index: Vec<u32>,
}

impl BoundaryCone {
    pub(super) fn propose(
        &self,
        boundary: usize,
        (tolerance, max_iterations, cell_visits, per_dimension): (f64, usize, usize, usize),
        pivots: &mut usize,
        available: usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Option<compact_tableau::Proposal>> {
        if self.groups.is_empty() {
            compact_tableau::propose_counting(
                &self.rows,
                (tolerance, max_iterations, cell_visits, per_dimension),
                pivots,
                available,
                interrupt,
            )
        } else {
            compact_tableau::propose_streamed(
                &self.rows,
                &self.groups,
                boundary,
                (tolerance, max_iterations, cell_visits, per_dimension),
                pivots,
                available,
                interrupt,
            )
        }
    }
}

pub(super) fn minimum_basis_cells(dimension: usize, width: usize) -> usize {
    dimension
        .saturating_mul(width.saturating_add(3))
        .saturating_add(dimension.saturating_mul(dimension.saturating_add(8)))
}

/// Column-major nonzeros of a cone basis: a raw column touches only the basis
/// vectors whose support contains it, so a row costs its terms' entries.
struct ColumnMajor {
    starts: Vec<usize>,
    entries: Vec<(u32, f64)>,
    dimension: usize,
}

impl ColumnMajor {
    fn new(basis: &[Vec<f64>], width: usize) -> Result<Self> {
        let mut starts =
            fallible_filled(width + 1, 0usize, "unable to reserve compact basis columns")?;
        for vector in basis {
            for (j, &value) in vector.iter().enumerate() {
                if value != 0. {
                    starts[j + 1] += 1;
                }
            }
        }
        for j in 0..width {
            starts[j + 1] += starts[j];
        }
        let mut entries = fallible_filled(
            starts[width],
            (0u32, 0.),
            "unable to reserve compact basis entries",
        )?;
        let mut next = fallible_collect(
            starts[..width].iter().copied(),
            width,
            "unable to reserve compact basis cursor",
        )?;
        for (k, vector) in basis.iter().enumerate() {
            for (j, &value) in vector.iter().enumerate() {
                if value != 0. {
                    entries[next[j]] = (k as u32, value);
                    next[j] += 1;
                }
            }
        }
        Ok(Self {
            starts,
            entries,
            dimension: basis.len(),
        })
    }

    fn cells(&self) -> usize {
        self.entries
            .len()
            .saturating_mul(2)
            .saturating_add(self.starts.len())
    }

    /// The same sums in the same order as `project`: columns in row order,
    /// each adding its entry to every axis it touches.
    fn project(&self, support: &[u32], out: &mut [f64], tolerance: f64, work: &mut usize) -> bool {
        let touched: usize = support
            .iter()
            .map(|&j| self.starts[j as usize + 1] - self.starts[j as usize])
            .sum();
        let Some(left) = work.checked_sub(touched.saturating_add(self.dimension)) else {
            return false;
        };
        *work = left;
        out.fill(0.);
        for &j in support {
            let j = j as usize;
            for &(k, b) in &self.entries[self.starts[j]..self.starts[j + 1]] {
                out[k as usize] += b;
            }
        }
        for value in out.iter_mut() {
            if value.abs() <= tolerance {
                *value = 0.;
            }
        }
        true
    }
}

/// A scaled search projects each boundary row once through the column-major
/// basis and stores only distinct projected rows. When the plan refuses that
/// bank at the full dimension, the axes are first ranked by two streaming
/// passes, as the incumbent panel does, and the bank is rebuilt at the
/// retained dimension. Duplicates change neither rank nor pivots.
fn scaled_cone(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &mut Vec<Vec<f64>>,
    tolerance: f64,
    (available, work): (usize, usize),
    mut grow: Option<Grow<'_>>,
) -> Result<Option<BoundaryCone>> {
    let width = design.ncols();
    let columns = ColumnMajor::new(basis, width)?;
    let reborrowed: Option<Grow<'_>> = match grow.as_mut() {
        Some(hook) => Some(&mut **hook),
        None => None,
    };
    let first = scaled_bank(
        design,
        boundary,
        &columns,
        tolerance,
        (available, work),
        reborrowed,
    )?;
    if first.is_some() || basis.len() <= 1 {
        return Ok(first);
    }
    let Some(keep) = streamed_axes(design, boundary, &columns, tolerance, work)? else {
        trace_refusal("projection_rank_work", boundary.len(), work);
        return Ok(None);
    };
    drop(columns);
    if keep.iter().all(|&kept| kept) {
        return Ok(None);
    }
    retain_columns(basis, &keep);
    let columns = ColumnMajor::new(basis, width)?;
    scaled_bank(
        design,
        boundary,
        &columns,
        tolerance,
        (available, work),
        grow,
    )
}

fn scaled_bank(
    design: &BinaryDesign,
    boundary: &[usize],
    columns: &ColumnMajor,
    tolerance: f64,
    (available, work): (usize, usize),
    grow: Option<Grow<'_>>,
) -> Result<Option<BoundaryCone>> {
    let (dimension, width) = (columns.dimension, design.ncols());
    let mut projector = |i: usize, out: &mut [f64], work: &mut usize| -> Result<bool> {
        Ok(columns.project(&design.row(boundary[i]), out, tolerance, work))
    };
    let basis_cells = dimension
        .saturating_mul(width.saturating_add(3))
        .saturating_add(columns.cells());
    streamed::build_with(
        (boundary.len(), dimension),
        &mut projector,
        (available, work),
        basis_cells,
        grow,
    )
}

/// Axes on which some boundary row is independent, without storing rows: one
/// pass for the largest projected entry and one rank-revealing pass, which
/// stops once every axis has a pivot.
fn streamed_axes(
    design: &BinaryDesign,
    boundary: &[usize],
    columns: &ColumnMajor,
    tolerance: f64,
    stage_work: usize,
) -> Result<Option<Vec<bool>>> {
    let dimension = columns.dimension;
    let mut out = fallible_filled(dimension, 0., "unable to reserve streamed cone row")?;
    let mut work = stage_work;
    let mut maximum = 0_f64;
    for (i, &origin) in boundary.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if !columns.project(&design.row(origin), &mut out, tolerance, &mut work) {
            return Ok(None);
        }
        maximum = out.iter().fold(maximum, |m, v| m.max(v.abs()));
    }
    let mut rank = RowRank::new(dimension, maximum * tolerance, work)?;
    for (i, &origin) in boundary.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        let mut left = usize::MAX;
        columns.project(&design.row(origin), &mut out, tolerance, &mut left);
        if !rank.insert(&out)? {
            return Ok(None);
        }
        if rank.full() {
            break;
        }
    }
    rank.columns().map(Some)
}

/// Keep the cone axes on which some distinct row is independent, in the rows
/// and in the basis. `false` means the rank pass exceeded its work.
pub(super) fn retain_independent(
    cone: &mut BoundaryCone,
    basis: &mut Vec<Vec<f64>>,
    tolerance: f64,
    work: usize,
) -> Result<bool> {
    if basis.is_empty() {
        return Ok(true);
    }
    let Some(keep) = independent_axes(&cone.rows, tolerance, work)? else {
        trace_refusal("projection_rank_work", cone.rows.len(), work);
        return Ok(false);
    };
    for row in &mut cone.rows {
        retain_columns(row, &keep);
    }
    retain_columns(basis, &keep);
    Ok(true)
}

/// The basis workspace; a scaled search requests it by its measured size.
fn admit_basis(minimum: usize, available: usize, grow: &mut Option<Grow<'_>>) -> Option<usize> {
    if minimum <= available {
        return Some(available);
    }
    let granted = grow
        .as_deref_mut()
        .and_then(|g| g(minimum))
        .filter(|&g| g >= minimum);
    if granted.is_none() {
        trace_refusal("projection_basis_cells", minimum, available);
    }
    granted
}

pub(super) fn build(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &mut Vec<Vec<f64>>,
    tolerance: f64,
    (available, work): (usize, usize),
    mut grow: Option<Grow<'_>>,
) -> Result<Option<BoundaryCone>> {
    let dimension = basis.len();
    let width = design.ncols();
    if grow.is_some() {
        // A scaled search projects each row once and ranks distinct rows.
        let needed = dimension.saturating_mul(width.saturating_mul(3).saturating_add(3));
        let Some(available) = admit_basis(needed, available, &mut grow) else {
            return Ok(None);
        };
        return scaled_cone(design, boundary, basis, tolerance, (available, work), grow);
    }
    let basis_cells = dimension.saturating_mul(width + 3);
    let pivot_cells = dimension.saturating_mul(dimension + 8);
    let row_cells = boundary.len().saturating_mul(dimension + 3);
    let minimum = minimum_basis_cells(dimension, width);
    let Some(available) = admit_basis(minimum, available, &mut grow) else {
        return Ok(None);
    };
    if basis_cells
        .saturating_add(pivot_cells)
        .saturating_add(row_cells)
        <= available
    {
        let Some(mut rows) = panel(design, boundary, basis, tolerance, work)? else {
            trace_refusal("projection_panel_work", boundary.len(), work);
            return Ok(None);
        };
        let maximum = rows.iter().flatten().fold(0_f64, |m, v| m.max(v.abs()));
        let mut reduction = RowRank::new(dimension, maximum * tolerance, work)?;
        for (i, row) in rows.iter().enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            if !reduction.insert(row)? {
                trace_refusal("projection_rank_work", dimension, work);
                return Ok(None);
            }
            if reduction.full() {
                break;
            }
        }
        let keep = reduction.columns()?;
        let dropped = dimension - keep.iter().filter(|&&v| v).count();
        let retained = dimension - dropped;
        let basis_cells = basis_cells - dropped * width;
        let dual_cells = boundary.len().saturating_mul(2).saturating_add(1);
        if boundary
            .len()
            .saturating_mul(retained + 3)
            .saturating_add(basis_cells)
            .saturating_add(dual_cells)
            > available
        {
            drop(rows);
            retain_columns(basis, &keep);
            return streamed::build(
                design,
                boundary,
                basis,
                tolerance,
                (available, work),
                basis_cells,
                grow,
            );
        }
        let tableau =
            compact_tableau::workspace_cells(boundary.len(), retained).unwrap_or(usize::MAX);
        let full_peak = row_cells
            .saturating_add(basis_cells)
            .saturating_add(tableau.max(dual_cells));
        let reclaim = dropped > 0 && full_peak > available;
        if reclaim {
            // Free the removed coordinates before phase I allocates its own
            // tableau. Each replacement overlaps just one old row, within
            // the scratch budget released by the rank reduction.
            for (i, row) in rows.iter_mut().enumerate() {
                crate::interrupt::row_checkpoint(i)?;
                *row = fallible_collect(
                    row.iter()
                        .zip(&keep)
                        .filter_map(|(&value, &kept)| kept.then_some(value)),
                    retained,
                    "unable to reserve reduced compact cone row",
                )?;
            }
        } else {
            for row in &mut rows {
                retain_columns(row, &keep);
            }
        }
        retain_columns(basis, &keep);
        let row_allocation = if reclaim {
            boundary.len() * (retained + 3)
        } else {
            row_cells
        };
        return Ok(Some(BoundaryCone {
            rows,
            live_cells: row_allocation + basis_cells,
            groups: Vec::new(),
            index: Vec::new(),
        }));
    }

    let memo_cells = boundary.len().min(available - basis_cells - pivot_cells);
    let Some(keep) = streaming_columns(design, boundary, basis, tolerance, (memo_cells, work))?
    else {
        trace_refusal(
            "projection_stream_work",
            boundary.len().saturating_mul(dimension),
            work,
        );
        return Ok(None);
    };
    retain_columns(basis, &keep);
    let basis_cells = dimension
        .saturating_mul(3)
        .saturating_add(basis.len().saturating_mul(width));
    let live_cells = boundary
        .len()
        .saturating_mul(basis.len() + 3)
        .saturating_add(basis_cells);
    let dual_cells = boundary.len().saturating_mul(2).saturating_add(1);
    if live_cells.saturating_add(dual_cells) > available {
        return streamed::build(
            design,
            boundary,
            basis,
            tolerance,
            (available, work),
            basis_cells,
            grow,
        );
    }
    Ok(
        panel(design, boundary, basis, tolerance, work)?.map(|rows| BoundaryCone {
            rows,
            live_cells,
            groups: Vec::new(),
            index: Vec::new(),
        }),
    )
}

/// Cone axes carrying an independent pivot of the distinct constraints, by
/// the same thresholded elimination as the materialized panel.
pub(super) fn independent_axes(
    rows: &[Vec<f64>],
    tolerance: f64,
    work: usize,
) -> Result<Option<Vec<bool>>> {
    let dimension = rows.first().map_or(0, Vec::len);
    if dimension == 0 {
        return Ok(None);
    }
    let maximum = rows.iter().flatten().fold(0_f64, |m, v| m.max(v.abs()));
    let mut rank = RowRank::new(dimension, maximum * tolerance, work)?;
    for (i, row) in rows.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if !rank.insert(row)? {
            return Ok(None);
        }
        if rank.full() {
            break;
        }
    }
    rank.columns().map(Some)
}

fn retain_columns<T>(slice: &mut Vec<T>, keep: &[bool]) {
    let mut selected = keep.iter();
    slice.retain(|_| *selected.next().expect("cone axis matches its pivot mask"));
}

/// Compute the same column mask as a materialized factorization. The first
/// pass obtains the global scale; the second keeps only independent pivots.
fn streaming_columns(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &[Vec<f64>],
    tolerance: f64,
    (memo_cells, stage_work): (usize, usize),
) -> Result<Option<Vec<bool>>> {
    let mut work = stage_work;
    let mut row = fallible_filled(basis.len(), 0., "unable to reserve streamed boundary row")?;
    let mut maximum = 0_f64;
    for (i, &origin) in boundary.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if !project(&design.row(origin), basis, tolerance, &mut row, &mut work) {
            return Ok(None);
        }
        maximum = row.iter().fold(maximum, |m, v| m.max(v.abs()));
    }
    let mut rank = RowRank::new(basis.len(), maximum * tolerance, stage_work)?;
    let mut memo = fallible_filled(
        memo_cells,
        usize::MAX,
        "unable to reserve cone rank row memo",
    )?;
    for (i, &origin) in boundary.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        let support = design.row(origin);
        if !memo.is_empty() {
            let Some(remaining) = work.checked_sub(2 * support.len() + 1) else {
                return Ok(None);
            };
            work = remaining;
            if seen_support(design, &support, origin, &mut memo) {
                continue;
            }
        }
        if !project(&support, basis, tolerance, &mut row, &mut work) || !rank.insert(&row)? {
            return Ok(None);
        }
        if rank.full() {
            break;
        }
    }
    Ok(Some(rank.columns()?))
}

/// Repeated original supports have identical projected rows. A collision
/// retires the old entry and only exact support equality skips elimination.
fn seen_support(design: &BinaryDesign, support: &[u32], origin: usize, memo: &mut [usize]) -> bool {
    let mut hash = DefaultHasher::new();
    support.hash(&mut hash);
    let slot = (hash.finish() % memo.len() as u64) as usize;
    let prior = std::mem::replace(&mut memo[slot], origin);
    prior != usize::MAX && *design.row(prior) == *support
}

fn panel(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &[Vec<f64>],
    tolerance: f64,
    stage_work: usize,
) -> Result<Option<Vec<Vec<f64>>>> {
    let mut work = stage_work;
    let mut rows = Vec::new();
    reserve(
        &mut rows,
        boundary.len(),
        "unable to reserve compact cone rows",
    )?;
    for (i, &origin) in boundary.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        let mut row = fallible_filled(basis.len(), 0., "unable to reserve compact cone row")?;
        if !project(&design.row(origin), basis, tolerance, &mut row, &mut work) {
            return Ok(None);
        }
        rows.push(row);
    }
    Ok(Some(rows))
}

fn project(
    support: &[u32],
    basis: &[Vec<f64>],
    tolerance: f64,
    row: &mut [f64],
    work: &mut usize,
) -> bool {
    let Some(remaining) = work.checked_sub((support.len() + 1).saturating_mul(basis.len())) else {
        return false;
    };
    *work = remaining;
    row.fill(0.);
    for &j in support {
        for (score, direction) in row.iter_mut().zip(basis) {
            *score += direction[j as usize];
        }
    }
    for value in row {
        if value.abs() <= tolerance {
            *value = 0.;
        }
    }
    true
}

struct RowRank {
    pivots: Vec<(usize, Vec<f64>)>,
    scratch: Vec<f64>,
    threshold: f64,
    work: usize,
}

impl RowRank {
    fn new(dimension: usize, threshold: f64, work: usize) -> Result<Self> {
        let mut pivots = Vec::new();
        reserve(
            &mut pivots,
            dimension,
            "unable to reserve compact cone pivots",
        )?;
        Ok(Self {
            pivots,
            scratch: fallible_filled(dimension, 0., "unable to reserve compact cone contrast")?,
            threshold,
            work,
        })
    }

    fn insert(&mut self, row: &[f64]) -> Result<bool> {
        let Some(remaining) = self
            .work
            .checked_sub((1 + self.pivots.len()).saturating_mul(row.len()))
        else {
            return Ok(false);
        };
        self.work = remaining;
        self.scratch.copy_from_slice(row);
        for (column, direction) in &self.pivots {
            let multiplier = self.scratch[*column];
            for (value, &prior) in self.scratch.iter_mut().zip(direction) {
                *value -= multiplier * prior;
            }
            self.scratch[*column] = 0.;
        }
        let Some((column, &pivot)) = self
            .scratch
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .filter(|(_, value)| value.abs() > self.threshold)
        else {
            return Ok(true);
        };
        let direction = fallible_collect(
            self.scratch.iter().map(|x| *x / pivot),
            row.len(),
            "unable to reserve compact cone direction",
        )?;
        self.pivots.push((column, direction));
        Ok(true)
    }

    fn full(&self) -> bool {
        self.pivots.len() == self.scratch.len()
    }

    fn columns(self) -> Result<Vec<bool>> {
        let mut keep = fallible_filled(
            self.scratch.len(),
            false,
            "unable to reserve compact cone mask",
        )?;
        // Keep one zero axis in a vacuous cone for the Farkas check.
        if self.pivots.is_empty() {
            keep[0] = true;
        }
        for (column, _) in self.pivots {
            keep[column] = true;
        }
        Ok(keep)
    }
}
