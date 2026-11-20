//! Numerically reduce the boundary cone before allocating a wide panel.
//! The projected columns propose a result. The original binary design
//! independently verifies every accepted ray or terminal dual.
use super::super::compact_tableau;
use super::{fallible_collect, fallible_filled, reserve, BinaryDesign, MAX_WORK};
use crate::error::Result;

mod streamed;

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
        (tolerance, max_iterations): (f64, usize),
        pivots: &mut usize,
        available: usize,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Option<compact_tableau::Proposal>> {
        if self.groups.is_empty() {
            compact_tableau::propose_counting(
                &self.rows,
                tolerance,
                max_iterations,
                pivots,
                available,
                interrupt,
            )
        } else {
            compact_tableau::propose_streamed(
                &self.rows,
                &self.groups,
                boundary,
                (tolerance, max_iterations),
                pivots,
                available,
                interrupt,
            )
        }
    }
}

pub(super) fn build(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &mut Vec<Vec<f64>>,
    tolerance: f64,
    available: usize,
) -> Result<Option<BoundaryCone>> {
    let dimension = basis.len();
    let width = design.ncols();
    let basis_cells = dimension.saturating_mul(width + 3);
    let pivot_cells = dimension.saturating_mul(dimension + 8);
    let row_cells = boundary.len().saturating_mul(dimension + 3);
    if basis_cells.saturating_add(pivot_cells) > available {
        return Ok(None);
    }
    if basis_cells
        .saturating_add(pivot_cells)
        .saturating_add(row_cells)
        <= available
    {
        let Some(mut rows) = panel(design, boundary, basis, tolerance)? else {
            return Ok(None);
        };
        let maximum = rows.iter().flatten().fold(0_f64, |m, v| m.max(v.abs()));
        let mut reduction = RowRank::new(dimension, maximum * tolerance)?;
        for (i, row) in rows.iter().enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            if !reduction.insert(row)? {
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
            return streamed::build(design, boundary, basis, tolerance, available, basis_cells);
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

    let Some(keep) = streaming_columns(design, boundary, basis, tolerance)? else {
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
        return streamed::build(design, boundary, basis, tolerance, available, basis_cells);
    }
    Ok(
        panel(design, boundary, basis, tolerance)?.map(|rows| BoundaryCone {
            rows,
            live_cells,
            groups: Vec::new(),
            index: Vec::new(),
        }),
    )
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
) -> Result<Option<Vec<bool>>> {
    let mut work = MAX_WORK;
    let mut row = fallible_filled(basis.len(), 0., "unable to reserve streamed boundary row")?;
    let mut maximum = 0_f64;
    for (i, &origin) in boundary.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if !project(design.row(origin), basis, tolerance, &mut row, &mut work) {
            return Ok(None);
        }
        maximum = row.iter().fold(maximum, |m, v| m.max(v.abs()));
    }
    let mut rank = RowRank::new(basis.len(), maximum * tolerance)?;
    for (i, &origin) in boundary.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if !project(design.row(origin), basis, tolerance, &mut row, &mut work)
            || !rank.insert(&row)?
        {
            return Ok(None);
        }
        if rank.full() {
            break;
        }
    }
    Ok(Some(rank.columns()?))
}

fn panel(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &[Vec<f64>],
    tolerance: f64,
) -> Result<Option<Vec<Vec<f64>>>> {
    let mut work = MAX_WORK;
    let mut rows = Vec::new();
    reserve(
        &mut rows,
        boundary.len(),
        "unable to reserve compact cone rows",
    )?;
    for (i, &origin) in boundary.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        let mut row = fallible_filled(basis.len(), 0., "unable to reserve compact cone row")?;
        if !project(design.row(origin), basis, tolerance, &mut row, &mut work) {
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
    fn new(dimension: usize, threshold: f64) -> Result<Self> {
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
            work: MAX_WORK,
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
