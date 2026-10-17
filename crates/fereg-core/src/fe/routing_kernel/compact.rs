//! Private compact-cell arm. Original-observation certification remains outside
//! this module. Arithmetic regrouping is confined to the two routing_cells_v2 arms.
use super::*;
use crate::linalg::neumaier_add;
use crate::parallel::StagePlan;

impl Work {
    // Charge before a bounded tile. Unlike per-element checkpoints, the tile
    // boundary amortizes cancellation/work overhead without hiding a long sort.
    fn tiles(
        &mut self,
        count: usize,
        cost: u16,
        mut action: impl FnMut(std::ops::Range<usize>) -> Attempt<()>,
    ) -> Attempt<()> {
        let width = usize::from(1024 / cost);
        for first in (0..count).step_by(width) {
            let end = (first + width).min(count);
            self.charge(((end - first) as u16) * cost)?;
            action(first..end)?;
        }
        Ok(())
    }

    fn bank<T: Copy>(&mut self, count: usize, value: T) -> Attempt<Vec<T>> {
        let mut bank = self.capacity(count)?;
        self.tiles(count, 4, |range| {
            bank.resize(range.end, value);
            Ok(())
        })?;
        Ok(bank)
    }
}

/// Parents have already been charged to budget.execution_bytes by the session.
/// All private banks (including retained capacities and peak sorting scratch)
/// coexist in this conservative admission. Denial publishes no partial kernel.
pub(crate) fn build_v2(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    max_rhs: usize,
    limits: KernelLimits,
) -> Result<OptionalSetup> {
    if design.effects().len() != 2
        || left >= 2
        || max_rhs == 0
        || !scale.is_finite()
        || scale <= 0.0
    {
        return Err(invalid_argument("invalid compact cell dimensions or scale"));
    }
    if let BorrowedMapWeights::Provided(values) = weights {
        if values.len() != design.nobs() {
            return Err(invalid_argument("compact cell weight length differs"));
        }
    }
    let p = design.effects()[left].n_levels();
    let q = design.effects()[1 - left].n_levels();
    let mut work = Work {
        limit: limits.work_budget,
        used: 0,
    };
    let mut peak = SolverWorkspacePeak::ZERO;
    let attempt = (|| -> Attempt<CellSchur> {
        work.charge(128)?;
        let offsets = design.effects()[left].level_index().membership_offsets();
        let mut degree = 0;
        work.tiles(p, 4, |range| {
            for worker in range {
                degree = degree.max(offsets[worker + 1] - offsets[worker]);
            }
            Ok(())
        })?;
        // Cell bank + transpose indices; two degree-sized sort banks and
        // suffix; worker offsets/totals, firm offsets/cursors/totals/diagonal
        // with compensated accumulation banks; active lanes and descriptors.
        let bytes = design.nobs() as u128 * (std::mem::size_of::<Cell>() as u128 + 8)
            + degree as u128 * (2 * std::mem::size_of::<Cell>() as u128 + 8)
            + (4 * p as u128 + 6 * q as u128 + max_rhs as u128 + 3) * 8
            + 4096;
        if !limits
            .retained_live_bytes
            .checked_add(bytes)
            .is_some_and(|total| limits.memory.is_none_or(|budget| budget.fits(total)))
        {
            return Err(Stop::Refused(KernelRefusal::MemoryBudget));
        }
        peak = SolverWorkspacePeak::from_bytes(bytes)?;
        construct(design, left, weights, scale, max_rhs, degree, &mut work)
    })();
    let (kernel, refusal) = match attempt {
        Ok(kernel) => (Some(kernel), None),
        Err(Stop::Refused(reason)) => (None, Some(reason)),
        Err(Stop::Fatal(error)) => return Err(error),
    };
    Ok(OptionalSetup {
        kernel,
        refusal,
        work_used: work.used,
        peak,
    })
}

// Bottom-up stable merge sort: actual O(d log d) work, interruptible even for
// a single high-degree worker. No opaque standard-library sort in the budget.
fn sort(rows: &mut [Cell], scratch: &mut [Cell], work: &mut Work) -> Attempt<()> {
    let mut width = 1;
    while width < rows.len() {
        for first in (0..rows.len()).step_by(width.saturating_mul(2)) {
            let mid = (first + width).min(rows.len());
            let end = mid.saturating_add(width).min(rows.len());
            let (mut i, mut j) = (first, mid);
            work.tiles(end - first, 32, |range| {
                for k in range {
                    let take_left = j == end
                        || (i < mid
                            && rows[i]
                                .firm
                                .cmp(&rows[j].firm)
                                .then_with(|| rows[i].weight.total_cmp(&rows[j].weight))
                                .is_le());
                    scratch[first + k] = if take_left {
                        let c = rows[i];
                        i += 1;
                        c
                    } else {
                        let c = rows[j];
                        j += 1;
                        c
                    };
                }
                Ok(())
            })?;
        }
        work.tiles(rows.len(), 4, |range| {
            rows[range.clone()].copy_from_slice(&scratch[range]);
            Ok(())
        })?;
        width = width.saturating_mul(2);
    }
    Ok(())
}

fn aggregate(rows: &mut Vec<Cell>, work: &mut Work) -> Attempt<f64> {
    if rows.is_empty() {
        return Ok(0.0);
    }
    let mut previous = rows[0];
    let (mut sum, mut correction, mut total, mut total_correction) = (0.0, 0.0, 0.0, 0.0);
    let mut write = 0;
    work.tiles(rows.len(), 64, |range| {
        for i in range {
            let cell = rows[i];
            if cell.firm != previous.firm {
                previous.weight = sum + correction;
                rows[write] = previous;
                write += 1;
                neumaier_add(previous.weight, &mut total, &mut total_correction);
                previous = cell;
                (sum, correction) = (0.0, 0.0);
            }
            neumaier_add(cell.weight, &mut sum, &mut correction);
        }
        Ok(())
    })?;
    work.charge(32)?;
    previous.weight = sum + correction;
    rows[write] = previous;
    rows.truncate(write + 1);
    neumaier_add(previous.weight, &mut total, &mut total_correction);
    let total = total + total_correction;
    if !total.is_finite() || total <= 0.0 {
        return Err(Stop::Refused(KernelRefusal::Numerical));
    }
    Ok(total)
}

fn construct(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    max_rhs: usize,
    degree: usize,
    work: &mut Work,
) -> Attempt<CellSchur> {
    let effect = &design.effects()[left];
    let firms = design.effects()[1 - left].codes();
    let (p, q) = (effect.n_levels(), design.effects()[1 - left].n_levels());
    let mut cells = work.capacity(design.nobs())?;
    // Keep the original CellSchur owner layout: the offset bank also owns
    // the firm view, anchors and lane map. Uninjected admission stays exact.
    let mut offsets = work.capacity(design.nobs() + 2 * p + q + max_rhs + 2)?;
    work.tiles(p + 1, 4, |range| {
        offsets.resize(range.end, 0usize);
        Ok(())
    })?;
    let mut worker_weights = work.bank(p, 0.0)?;
    let mut anchors = work.bank(p, 0usize)?;
    let mut firm_weights = work.bank(q, 0.0)?;
    let mut firm_corrections = work.bank(q, 0.0)?;
    let mut diagonal = work.bank(q, 0.0)?;
    let mut diagonal_corrections = work.bank(q, 0.0)?;
    let mut rows = work.capacity(degree)?;
    let mut scratch = work.bank(degree, Cell::default())?;
    let mut suffix = work.bank(degree + 1, 0.0)?;
    for worker in 0..p {
        work.charge(32)?;
        rows.clear();
        let members = effect.level_index().checked_members(worker)?;
        work.tiles(members.len(), 16, |range| {
            for &row in &members[range] {
                if row >= design.nobs() {
                    return Err(invalid_argument("cell member exceeds rows").into());
                }
                let weight = match weights {
                    BorrowedMapWeights::Unit => 1.0,
                    BorrowedMapWeights::Provided(values) => values[row] / scale,
                };
                if !weight.is_finite() || weight <= 0.0 || firms[row] as usize >= q {
                    return Err(invalid_data("invalid compact cell weight or firm").into());
                }
                rows.push(Cell {
                    worker: worker as u32,
                    firm: firms[row],
                    weight,
                });
            }
            Ok(())
        })?;
        sort(&mut rows, &mut scratch, work)?;
        let total = aggregate(&mut rows, work)?;
        worker_weights[worker] = total;
        let (mut sum, mut correction) = (0.0, 0.0);
        let mut largest = 0.0;
        suffix[rows.len()] = 0.0;
        work.tiles(rows.len(), 24, |range| {
            for k in range {
                let i = rows.len() - 1 - k;
                neumaier_add(rows[i].weight, &mut sum, &mut correction);
                suffix[i] = sum + correction;
            }
            Ok(())
        })?;
        (sum, correction) = (0.0, 0.0);
        work.tiles(rows.len(), 96, |range| {
            for i in range {
                let cell = rows[i];
                let firm = cell.firm as usize;
                // Largest weight minimizes cancellation at the dominant cell.
                // Canonical firm order breaks ties deterministically.
                if cell.weight > largest {
                    largest = cell.weight;
                    anchors[worker] = firm;
                }
                neumaier_add(
                    cell.weight,
                    &mut firm_weights[firm],
                    &mut firm_corrections[firm],
                );
                if rows.len() > 1 {
                    let other = (sum + correction) + suffix[i + 1];
                    let value = if cell.weight >= other {
                        (cell.weight / total) * other
                    } else {
                        (other / total) * cell.weight
                    };
                    neumaier_add(value, &mut diagonal[firm], &mut diagonal_corrections[firm]);
                    cells.push(cell);
                }
                neumaier_add(cell.weight, &mut sum, &mut correction);
            }
            Ok(())
        })?;
        offsets[worker + 1] = cells.len();
    }
    work.tiles(q, 16, |range| {
        for firm in range {
            firm_weights[firm] += firm_corrections[firm];
            diagonal[firm] += diagonal_corrections[firm];
            if !firm_weights[firm].is_finite()
                || !diagonal[firm].is_finite()
                || diagonal[firm] < 0.0
            {
                return Err(Stop::Refused(KernelRefusal::Numerical));
            }
            if diagonal[firm] == 0.0 {
                diagonal[firm] = firm_weights[firm];
            }
        }
        Ok(())
    })?;
    transpose(&cells, &mut offsets, &anchors, q, max_rhs, work)?;

    Ok(CellSchur {
        cells,
        offsets,
        worker_weights,
        firm_weights,
        diagonal,
    })
}

fn transpose(
    cells: &[Cell],
    offsets: &mut Vec<usize>,
    anchors: &[usize],
    q: usize,
    max_rhs: usize,
    work: &mut Work,
) -> Attempt<()> {
    let p = anchors.len();
    let end = p + 1;
    let count = q + 1 + cells.len() + p + max_rhs;
    work.tiles(count, 4, |range| {
        offsets.resize(end + range.end, 0);
        Ok(())
    })?;
    let (_, extra) = offsets.split_at_mut(end);
    let (firm_offsets, extra) = extra.split_at_mut(q + 1);
    let (indices, extra) = extra.split_at_mut(cells.len());
    let (stored_anchors, _) = extra.split_at_mut(p);
    work.tiles(p, 4, |range| {
        stored_anchors[range.clone()].copy_from_slice(&anchors[range]);
        Ok(())
    })?;
    work.tiles(cells.len(), 8, |range| {
        for cell in &cells[range] {
            firm_offsets[cell.firm as usize + 1] += 1;
        }
        Ok(())
    })?;
    work.tiles(q, 4, |range| {
        for i in range {
            firm_offsets[i + 1] += firm_offsets[i];
        }
        Ok(())
    })?;
    let mut cursor = work.bank(q, 0usize)?;
    work.tiles(q, 4, |range| {
        cursor[range.clone()].copy_from_slice(&firm_offsets[range]);
        Ok(())
    })?;
    work.tiles(cells.len(), 12, |range| {
        for i in range {
            let firm = cells[i].firm as usize;
            indices[cursor[firm]] = i;
            cursor[firm] += 1;
        }
        Ok(())
    })?;
    Ok(())
}

// The ordinary fast path keeps the difference small. Extremely small weights
// can still make a finite product of an overflowing difference; distribute
// only in that case, without treating a numeric error as optional refusal.
fn weighted_difference(weight: f64, a: f64, b: f64) -> f64 {
    let delta = a - b;
    if delta.is_finite() {
        weight * delta
    } else {
        weight * a - weight * b
    }
}

/// Fixed within-group order at every worker count. Anchor differences avoid
/// subtracting large D_f*x and C' mean terms. A non-mover has no stored cells.
impl CellSchur {
    pub(crate) fn apply_compact(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        scratch: &mut [f64],
        activity: &[f64],
        stride: usize,
    ) -> Result<bool> {
        use std::sync::atomic::{AtomicBool, Ordering};
        let finite = AtomicBool::new(true);
        let p = self.worker_weights.len();
        let q = self.firm_weights.len();
        let (worker_offsets, extra) = self.offsets.split_at_mut(p + 1);
        let (firm_offsets, extra) = extra.split_at_mut(q + 1);
        let (indices, extra) = extra.split_at_mut(self.cells.len());
        let (anchors, active) = extra.split_at_mut(p);
        let rhs = activity.len();
        if stride == 0
            || rhs > stride
            || rhs > active.len()
            || self.firm_weights.len().checked_mul(stride) != Some(input.len())
            || input.len() != output.len()
            || self.worker_weights.len().checked_mul(stride) != Some(scratch.len())
        {
            return Err(invalid_argument("compact cell workspace dimensions differ"));
        }
        let mut active_count = 0;
        for (first, chunk) in activity.chunks(128).enumerate() {
            crate::interrupt::checkpoint()?;
            for (i, &value) in chunk.iter().enumerate() {
                if value != 0.0 {
                    active[active_count] = first * 128 + i;
                    active_count += 1;
                }
            }
        }
        let active = &active[..active_count];
        let work = self.cells.len() as u128 * active.len() as u128 * 4;
        StagePlan::current(work, self.worker_weights.len()).groups_mut(
            scratch,
            stride,
            worker_offsets,
            |first, bank| {
                for (local, mean) in bank.chunks_exact_mut(stride).enumerate() {
                    let worker = first + local;
                    let cells = &self.cells[worker_offsets[worker]..worker_offsets[worker + 1]];
                    for (block, lanes) in active.chunks(64).enumerate() {
                        crate::interrupt::checkpoint()?;
                        let packed = &mut mean[block * 64..block * 64 + lanes.len()];
                        packed.fill(0.0);
                        if cells.is_empty() {
                            continue;
                        }
                        let anchor = anchors[worker] * stride;
                        for tile in cells.chunks((1024 / (4 * lanes.len())).max(1)) {
                            crate::interrupt::checkpoint()?;
                            for cell in tile {
                                let firm = cell.firm as usize * stride;
                                for (sum, &rhs) in packed.iter_mut().zip(lanes) {
                                    *sum += weighted_difference(
                                        cell.weight,
                                        input[firm + rhs],
                                        input[anchor + rhs],
                                    );
                                }
                            }
                        }
                        for sum in packed {
                            *sum /= self.worker_weights[worker];
                            if !sum.is_finite() {
                                finite.store(false, Ordering::Relaxed);
                            }
                        }
                    }
                }
                Ok(())
            },
        )?;
        if !finite.load(Ordering::Relaxed) {
            return finite_breakdown(input, active, stride);
        }
        StagePlan::current(work, self.firm_weights.len()).groups_mut(
            output,
            stride,
            firm_offsets,
            |first, bank| {
                for (local, target) in bank.chunks_exact_mut(stride).enumerate() {
                    let firm = first + local;
                    for chunk in target[..rhs].chunks_mut(128) {
                        crate::interrupt::checkpoint()?;
                        chunk.fill(0.0);
                    }
                    for (block, lanes) in active.chunks(64).enumerate() {
                        let indices = &indices[firm_offsets[firm]..firm_offsets[firm + 1]];
                        for tile in indices.chunks((1024 / (8 * lanes.len())).max(1)) {
                            crate::interrupt::checkpoint()?;
                            for &i in tile {
                                let cell = self.cells[i];
                                let worker = cell.worker as usize;
                                let anchor = anchors[worker] * stride;
                                for (k, &rhs) in lanes.iter().enumerate() {
                                    let delta = input[firm * stride + rhs] - input[anchor + rhs];
                                    let mean = scratch[worker * stride + block * 64 + k];
                                    target[rhs] += if delta.is_finite() {
                                        weighted_difference(cell.weight, delta, mean)
                                    } else {
                                        weighted_difference(
                                            cell.weight,
                                            input[firm * stride + rhs],
                                            input[anchor + rhs],
                                        ) - cell.weight * mean
                                    };
                                }
                            }
                        }
                        crate::interrupt::checkpoint()?;
                        if lanes.iter().any(|&rhs| !target[rhs].is_finite()) {
                            finite.store(false, Ordering::Relaxed);
                        }
                    }
                }
                Ok(())
            },
        )?;
        if finite.load(Ordering::Relaxed) {
            Ok(true)
        } else {
            finite_breakdown(input, active, stride)
        }
    }
}

// Numerical loss of the optional recurrence uses its existing breakdown/MAP
// continuation. Encountered nonfinite inputs and all cancellation/allocation
// errors remain fatal; the caller supplies finite recurrence state.
fn finite_breakdown(input: &[f64], active: &[usize], stride: usize) -> Result<bool> {
    for row in input.chunks_exact(stride) {
        for lanes in active.chunks(128) {
            crate::interrupt::checkpoint()?;
            if lanes.iter().any(|&rhs| !row[rhs].is_finite()) {
                return Err(invalid_data("compact cell input is nonfinite"));
            }
        }
    }
    Ok(false)
}
