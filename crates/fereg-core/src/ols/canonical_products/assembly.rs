//! Ordered dense Schur assembly with disjoint component-major row owners.
use super::{allocation, declined, Budget, ComponentFactor};
use crate::error::Result;
use crate::fe::LevelIndex;
use rayon::prelude::*;
use std::mem::size_of;

#[derive(Clone, Copy)]
pub(super) struct Geometry<'a> {
    pub members: &'a LevelIndex,
    pub codes: &'a [u32],
    pub w: &'a [f64],
    pub mass_a: &'a [f64],
    pub parent: &'a [usize],
    pub local: &'a [usize],
    pub components: &'a [ComponentFactor],
}

pub(super) fn serial(
    geometry: Geometry<'_>,
    matrix: &mut [f64],
    totals: &mut [f64],
    touched: &mut [usize],
    implicit: bool,
) -> Result<()> {
    let Geometry {
        members,
        codes,
        w,
        mass_a,
        parent,
        local,
        components,
    } = geometry;
    // Keep the legacy matrix indices and numerical traversal order explicit.
    #[allow(clippy::needless_range_loop)]
    for g in 0..mass_a.len() {
        let mut count = 0;
        for &i in members.members(g) {
            let h = codes[i] as usize;
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
    Ok(())
}

struct Lane<'a> {
    matrix: &'a mut [f64],
    totals: &'a mut [f64],
    touched: &'a mut [usize],
    first_row: usize,
    end_row: usize,
    first_cell: usize,
}

// Extra scratch borrows spare space from the existing phase peak. Failing to
// obtain this optional storage leaves the original serial route available.
pub(super) fn try_parallel(
    geometry: Geometry<'_>,
    matrix: &mut [f64],
    totals: &mut [f64],
    touched: &mut [usize],
    workers: usize,
    budget: &mut Budget,
) -> Result<bool> {
    let q = totals.len();
    let scratch = q as u128 * (size_of::<f64>() + size_of::<usize>()) as u128;
    let per_lane = scratch + size_of::<Lane<'_>>() as u128;
    let count = workers.min(q).min(
        usize::try_from(budget.remaining.saturating_add(scratch) / per_lane).unwrap_or(usize::MAX),
    );
    if count < 2 {
        return Ok(false);
    }
    let Some(extra) = (count - 1).checked_mul(q) else {
        return Ok(false);
    };
    let Ok(mut extra_totals) = budget.reserve(extra, 0_f64) else {
        return Ok(false);
    };
    let mut extra_touched = match budget.reserve(extra, 0_usize) {
        Ok(values) => values,
        Err(_) => {
            budget.release(extra_totals)?;
            return Ok(false);
        }
    };
    let mut lanes = match budget.reserve_empty::<Lane<'_>>(count) {
        Ok(values) => values,
        Err(_) => {
            budget.release(extra_totals)?;
            budget.release(extra_touched)?;
            return Ok(false);
        }
    };
    let cells = matrix.len();
    let mut remaining = &mut *matrix;
    let (mut first_row, mut first_cell) = (0, 0);
    for (lane, (totals, touched)) in std::iter::once(totals)
        .chain(extra_totals.chunks_mut(q))
        .zip(std::iter::once(touched).chain(extra_touched.chunks_mut(q)))
        .enumerate()
    {
        let end_row = first_row + q / count + usize::from(lane < q % count);
        let end_cell = if end_row == q {
            cells
        } else {
            geometry
                .components
                .iter()
                .enumerate()
                .find_map(|(root, block)| {
                    (geometry.parent[root] == root
                        && (block.level_start..block.level_start + block.len + 1)
                            .contains(&end_row))
                    .then(|| block.matrix_offset + (end_row - block.level_start) * (block.len + 1))
                })
                .ok_or_else(allocation)?
        };
        let (owned, rest) = remaining.split_at_mut(end_cell - first_cell);
        remaining = rest;
        lanes.push(Lane {
            matrix: owned,
            totals,
            touched,
            first_row,
            end_row,
            first_cell,
        });
        first_row = end_row;
        first_cell = end_cell;
    }
    let result = lanes
        .par_iter_mut()
        .try_for_each(|lane| fill_lane(geometry, lane));
    budget.release(lanes)?;
    budget.release(extra_totals)?;
    budget.release(extra_touched)?;
    result?;
    // The serial loop subtracts each pair from both triangles in the same g
    // order. Copying the completed upper cell therefore preserves every bit.
    for (root, block) in geometry.components.iter().enumerate() {
        if geometry.parent[root] != root {
            continue;
        }
        let size = block.len + 1;
        for row in 0..size {
            for column in 0..row {
                matrix[block.matrix_offset + row * size + column] =
                    matrix[block.matrix_offset + column * size + row];
            }
        }
    }
    Ok(true)
}

fn fill_lane(geometry: Geometry<'_>, lane: &mut Lane<'_>) -> Result<()> {
    let Geometry {
        members,
        codes,
        w,
        mass_a,
        parent,
        local,
        components,
    } = geometry;
    for (g, &mass) in mass_a.iter().enumerate() {
        let mut count = 0;
        for &i in members.members(g) {
            let h = codes[i] as usize;
            if lane.totals[h] == 0. {
                lane.touched[count] = h;
                count += 1;
            }
            lane.totals[h] += w[i];
        }
        lane.touched[..count].sort_unstable();
        if count == 0 {
            return Err(declined());
        }
        for (position, &h) in lane.touched[..count].iter().enumerate() {
            let block = components[parent[h]];
            let owner_row = block.level_start + local[h];
            if !(lane.first_row..lane.end_row).contains(&owner_row) {
                continue;
            }
            let size = block.len + 1;
            let row_start = block.matrix_offset + local[h] * size - lane.first_cell;
            let diagonal = row_start + local[h];
            // In the serial pair loop (i,h) precedes (h,j), each in neighbour
            // order. Keep those additions separate from other owned rows.
            for &i in &lane.touched[..position] {
                let value = (lane.totals[i].max(lane.totals[h]) / mass)
                    * lane.totals[i].min(lane.totals[h]);
                if !value.is_finite() || value <= 0. {
                    return Err(declined());
                }
                lane.matrix[diagonal] += value;
            }
            for &j in &lane.touched[position + 1..count] {
                let value = (lane.totals[h].max(lane.totals[j]) / mass)
                    * lane.totals[h].min(lane.totals[j]);
                if !value.is_finite() || value <= 0. {
                    return Err(declined());
                }
                lane.matrix[row_start + local[j]] -= value;
                lane.matrix[diagonal] += value;
            }
        }
        for &h in &lane.touched[..count] {
            lane.totals[h] = 0.;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
