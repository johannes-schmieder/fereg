//! Store only distinct boundary vectors if even the reduced panel cannot fit.
//! Exact equality of the projected binary64 rows is required. The index keeps
//! full-order Farkas replay and original-row raw certification independent.
use super::{
    fallible_collect, fallible_filled, project, reserve, BinaryDesign, BoundaryCone, MAX_WORK,
};
use crate::error::Result;
use std::hash::{DefaultHasher, Hash, Hasher};

pub(super) fn build(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &[Vec<f64>],
    tolerance: f64,
    available: usize,
    basis_cells: usize,
) -> Result<Option<BoundaryCone>> {
    let dimension = basis.len();
    // The original-row index and both full-size dual vectors, with a
    // dimension-square allowance for the transposed Phase I basis.
    let fixed = 3usize
        .saturating_mul(boundary.len())
        .saturating_add(dimension.saturating_mul(dimension + 8))
        .saturating_add(8);
    let Some(remaining) = available.saturating_sub(basis_cells).checked_sub(fixed) else {
        return Ok(None);
    };
    let capacity = boundary.len().min(remaining / (2 * dimension + 12));
    if capacity == 0 || u32::try_from(capacity).is_err() {
        return Ok(None);
    }
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
    let mut work = MAX_WORK;
    for (i, &origin) in boundary.iter().enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if !project(
            design.row(origin),
            basis,
            tolerance,
            &mut projected,
            &mut work,
        ) {
            return Ok(None);
        }
        let Some(key) = row_key(&projected, &mut work) else {
            return Ok(None);
        };
        let mut slot = (key % slots.len() as u64) as usize;
        for probe in 0..slots.len() {
            crate::interrupt::row_checkpoint(probe)?;
            let group = slots[slot];
            if group == usize::MAX {
                if rows.len() == capacity {
                    return Ok(None);
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
                return Ok(None);
            };
            work = remaining;
            if rows[group] == projected {
                index.push(group as u32);
                groups[group].1 += 1;
                break;
            }
            slot = (slot + 1) % slots.len();
        }
    }
    let live_cells = basis_cells
        + index.capacity()
        + 3 * rows.capacity()
        + 2 * groups.capacity()
        + dimension * rows.len();
    Ok(Some(BoundaryCone {
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
