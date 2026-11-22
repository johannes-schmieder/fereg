//! Store only distinct boundary vectors if even the reduced panel cannot fit.
//! Exact equality of the projected binary64 rows is required. The index keeps
//! full-order Farkas replay and original-row raw certification independent.
use super::super::trace_refusal;
use super::{fallible_collect, fallible_filled, project, reserve, BinaryDesign, BoundaryCone};
use crate::error::Result;

/// Grow hook: request a total allowance in eight-byte cells and receive the
/// granted total, or `None` when the live plan refuses it.
pub(in crate::ppml_separation::simplex) type Grow<'g> = &'g mut dyn FnMut(usize) -> Option<usize>;

/// Rows projected onto the cone axes, one boundary position at a time. A
/// `false` return means the stage work allowance is spent.
pub(in crate::ppml_separation::simplex) type Projector<'p> =
    &'p mut dyn FnMut(usize, &mut [f64], &mut usize) -> Result<bool>;

pub(super) fn build(
    design: &BinaryDesign,
    boundary: &[usize],
    basis: &[Vec<f64>],
    tolerance: f64,
    (available, stage_work): (usize, usize),
    basis_cells: usize,
    grow: Option<Grow<'_>>,
) -> Result<Option<BoundaryCone>> {
    let mut projector = |i: usize, out: &mut [f64], work: &mut usize| {
        Ok(project(
            &design.row(boundary[i]),
            basis,
            tolerance,
            out,
            work,
        ))
    };
    build_with(
        (boundary.len(), basis.len()),
        &mut projector,
        (available, stage_work),
        basis_cells,
        grow,
    )
}

/// Distinct initial capacity requested when the current allowance cannot hold
/// the fixed part and one distinct row. Later growth doubles on demand.
const INITIAL_DISTINCT: usize = 4096;

pub(in crate::ppml_separation::simplex) fn build_with(
    (boundary, dimension): (usize, usize),
    projector: Projector<'_>,
    (available, stage_work): (usize, usize),
    basis_cells: usize,
    mut grow: Option<Grow<'_>>,
) -> Result<Option<BoundaryCone>> {
    // The original-row index and both full-size dual vectors, with a
    // dimension-square allowance for the transposed Phase I basis.
    let fixed = 3usize
        .saturating_mul(boundary)
        .saturating_add(dimension.saturating_mul(dimension + 8))
        .saturating_add(8);
    let per_row = 2 * dimension + 12;
    let base = basis_cells.saturating_add(fixed);
    let mut available = available;
    // A scaled search requests the row-proportional fixed part and an initial
    // distinct bank by their measured sizes before the first attempt.
    if available < base.saturating_add(per_row) {
        let needed = base.saturating_add(per_row.saturating_mul(boundary.min(INITIAL_DISTINCT)));
        let granted = match grow.as_deref_mut() {
            Some(grow) => grow(needed).filter(|&g| g >= needed),
            None => None,
        };
        let Some(granted) = granted else {
            trace_refusal("streamed_cone_fixed_cells", needed, available);
            return Ok(None);
        };
        available = granted;
    }
    // Every attempt spends the same stage allowance. Only a scaled search can
    // request a larger distinct-row bank, by doubling its measured capacity.
    let mut work = stage_work;
    loop {
        let capacity = boundary.min((available - base) / per_row);
        if capacity == 0 || u32::try_from(capacity).is_err() {
            trace_refusal("streamed_cone_capacity", per_row, available - base);
            return Ok(None);
        }
        match attempt((boundary, dimension), projector, capacity, &mut work)? {
            Attempt::Built(mut cone) => {
                cone.live_cells += basis_cells;
                return Ok(Some(cone));
            }
            Attempt::Failed => return Ok(None),
            Attempt::Full => {
                let next = boundary.min(capacity.saturating_mul(2));
                let needed = base.saturating_add(next.saturating_mul(per_row));
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
    (boundary, dimension): (usize, usize),
    projector: Projector<'_>,
    capacity: usize,
    work: &mut usize,
) -> Result<Attempt> {
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
        boundary,
        "unable to reserve original boundary index",
    )?;
    for i in 0..boundary {
        crate::interrupt::row_checkpoint(i)?;
        if !projector(i, &mut projected, work)? {
            trace_refusal("streamed_cone_work", i, *work);
            return Ok(Attempt::Failed);
        }
        let Some(key) = row_key(&projected, work) else {
            trace_refusal("streamed_cone_work", i, *work);
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
                trace_refusal("streamed_cone_work", i, *work);
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

/// Bucket key of a projected row. Equal rows always share a key (both zero
/// signs map to zero); buckets are only a lookup aid, and every match is
/// confirmed by exact comparison, so the key never affects which rows group.
pub(in crate::ppml_separation::simplex) fn row_key(row: &[f64], work: &mut usize) -> Option<u64> {
    *work = work.checked_sub(row.len())?;
    let mut hash = 0x243f_6a88_85a3_08d3_u64;
    for &value in row {
        if !value.is_finite() {
            return None;
        }
        let bits = if value == 0. { 0 } else { value.to_bits() };
        hash = (hash ^ bits).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        hash ^= hash >> 29;
    }
    Some(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_proportional_fixed_part_is_requested_before_the_first_attempt() {
        let (boundary, dimension) = (10_000usize, 2usize);
        let basis_cells = dimension * 8;
        let fixed = 3 * boundary + dimension * (dimension + 8) + 8;
        let available = fixed + basis_cells - 1;
        let mut requests = Vec::new();
        let mut grant = |needed: usize| {
            requests.push(needed);
            Some(needed)
        };
        let mut projector = |i: usize, out: &mut [f64], _: &mut usize| {
            out[0] = (i % 3) as f64;
            out[1] = 0.;
            Ok(true)
        };
        let cone = build_with(
            (boundary, dimension),
            &mut projector,
            (available, usize::MAX),
            basis_cells,
            Some(&mut grant),
        )
        .unwrap()
        .expect("the measured fixed part is granted");
        assert_eq!(cone.rows.len(), 3);
        assert_eq!(cone.index.len(), boundary);
        assert!(requests[0] > available);
        // Without a grow hook the same shortage refuses, as before.
        let refused = build_with(
            (boundary, dimension),
            &mut projector,
            (available, usize::MAX),
            basis_cells,
            None,
        )
        .unwrap();
        assert!(refused.is_none());
    }
}
