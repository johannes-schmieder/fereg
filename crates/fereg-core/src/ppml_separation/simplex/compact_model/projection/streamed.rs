//! Store only distinct boundary vectors if even the reduced panel cannot fit.
//! Exact equality of the projected binary64 rows is required. The index keeps
//! full-order Farkas replay and original-row raw certification independent.
use super::super::trace_refusal;
use super::{fallible_collect, fallible_filled, project, reserve, BinaryDesign, BoundaryCone};
use crate::error::Result;
use std::hash::{DefaultHasher, Hash, Hasher};

/// Grow hook: request a total allowance in eight-byte cells and receive the
/// granted total, or `None` when the live plan refuses it.
pub(in crate::ppml_separation::simplex) type Grow<'g> = &'g mut dyn FnMut(usize) -> Option<usize>;

pub(super) fn build(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &[Vec<f64>],
    tolerance: f64,
    (available, stage_work): (usize, usize),
    basis_cells: usize,
    mut grow: Option<Grow<'_>>,
) -> Result<Option<BoundaryCone>> {
    let dimension = basis.len();
    // The original-row index and both full-size dual vectors, with a
    // dimension-square allowance for the transposed Phase I basis.
    let fixed = 3usize
        .saturating_mul(boundary.len())
        .saturating_add(dimension.saturating_mul(dimension + 8))
        .saturating_add(8);
    let per_row = 2 * dimension + 12;
    let mut available = available;
    // Every attempt spends the same stage allowance. Only a scaled search can
    // request a larger distinct-row bank, by doubling its measured capacity.
    let mut work = stage_work;
    loop {
        let Some(remaining) = available.saturating_sub(basis_cells).checked_sub(fixed) else {
            trace_refusal(
                "streamed_cone_fixed_cells",
                fixed.saturating_add(basis_cells),
                available,
            );
            return Ok(None);
        };
        let capacity = boundary.len().min(remaining / per_row);
        if capacity == 0 || u32::try_from(capacity).is_err() {
            trace_refusal("streamed_cone_capacity", per_row, remaining);
            return Ok(None);
        }
        match attempt(design, boundary, basis, tolerance, capacity, &mut work)? {
            Attempt::Built(mut cone) => {
                cone.live_cells += basis_cells;
                return Ok(Some(cone));
            }
            Attempt::Failed => return Ok(None),
            Attempt::Full => {
                let next = boundary.len().min(capacity.saturating_mul(2));
                let needed = basis_cells
                    .saturating_add(fixed)
                    .saturating_add(next.saturating_mul(per_row));
                let granted = match grow.as_deref_mut() {
                    Some(grow) if next > capacity => grow(needed).filter(|&g| g >= needed),
                    _ => None,
                };
                let Some(granted) = granted else {
                    trace_refusal(
                        "streamed_cone_distinct",
                        capacity.saturating_add(1),
                        capacity,
                    );
                    return Ok(None);
                };
                available = granted;
            }
        }
    }
}

enum Attempt {
    Built(BoundaryCone),
    Full,
    Failed,
}

fn attempt(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &[Vec<f64>],
    tolerance: f64,
    capacity: usize,
    work: &mut usize,
) -> Result<Attempt> {
    let dimension = basis.len();
    let mut slots = fallible_filled(
        2 * capacity,
        usize::MAX,
        "unable to reserve streamed cone hash slots",
    )?;
    let mut projected = fallible_filled(dimension, 0., "unable to reserve grouped cone scratch")?;
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut index = Vec::new();
    reserve(&mut rows, capacity, "unable to reserve distinct cone rows")?;
    reserve(
        &mut groups,
        capacity,
        "unable to reserve distinct cone multiplicities",
    )?;
    reserve(
        &mut index,
        boundary.len(),
        "unable to reserve original boundary index",
    )?;
    for (i, &origin) in boundary.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if !project(&design.row(origin), basis, tolerance, &mut projected, work) {
            trace_refusal("streamed_cone_work", i, *work);
            return Ok(Attempt::Failed);
        }
        let Some(key) = row_key(&projected, work) else {
            return Ok(Attempt::Failed);
        };
        let mut slot = (key % slots.len() as u64) as usize;
        for probe in 0..slots.len() {
            crate::interrupt::row_checkpoint(probe)?;
            let group = slots[slot];
            if group == usize::MAX {
                if rows.len() == capacity {
                    return Ok(Attempt::Full);
                }
                slots[slot] = rows.len();
                index.push(rows.len() as u32);
                rows.push(fallible_collect(
                    projected.iter().copied(),
                    dimension,
                    "unable to retain distinct cone row",
                )?);
                groups.push((i, 1));
                break;
            }
            let Some(remaining) = work.checked_sub(dimension + 1) else {
                return Ok(Attempt::Failed);
            };
            *work = remaining;
            if rows[group] == projected {
                index.push(group as u32);
                groups[group].1 += 1;
                break;
            }
            slot = (slot + 1) % slots.len();
        }
    }
    let live_cells =
        index.capacity() + 3 * rows.capacity() + 2 * groups.capacity() + dimension * rows.len();
    Ok(Attempt::Built(BoundaryCone {
        rows,
        live_cells,
        groups,
        index,
    }))
}

fn row_key(row: &[f64], work: &mut usize) -> Option<u64> {
    *work = work.checked_sub(row.len())?;
    let mut hash = DefaultHasher::new();
    for &value in row {
        if !value.is_finite() {
            return None;
        }
        let bits = if value == 0. { 0 } else { value.to_bits() };
        bits.hash(&mut hash);
    }
    Some(hash.finish())
}
