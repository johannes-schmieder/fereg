//! Cell construction on the installed team with the serial constructors' bits.
//!
//! Workers are split into contiguous ranges balanced by membership rows. Each
//! lane walks its workers' membership lists in row order, exactly as the
//! occurrence constructor does: the first occurrence fixes a cell's slot and
//! later rows add in row order, so every cell weight is the serial sum. The
//! radix constructor's cells differ only in their within-worker order (by
//! firm), which a per-worker sort of unique firms restores. Finishing keeps
//! each serial reduction order: worker totals and Schur contributions per
//! worker in cell order, firm totals and diagonals in worker-major order
//! through a stable two-level (firm range, then firm) counting sort.

use super::*;
use crate::parallel::{initialize_reserved, stage_try_for_each, StagePlan};
use rayon::prelude::*;

/// Which serial constructor's within-worker cell order to reproduce.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Order {
    /// Radix constructor: firms ascending within each worker.
    Firm,
    /// Occurrence constructor: first membership occurrence within each worker.
    Occurrence,
}

/// Bound on one lane's descriptors (`Lane`, `Block`, `Range`, a segment,
/// five transient span slices and four boundary words), checked at compile
/// time.
const LANE_DESCRIPTOR_BYTES: u128 = 512;
const _: () = assert!(
    std::mem::size_of::<Lane<'static>>()
        + std::mem::size_of::<Block<'static>>()
        + std::mem::size_of::<Range<'static>>()
        + std::mem::size_of::<(usize, &'static [usize])>()
        + 5 * std::mem::size_of::<&'static mut [f64]>()
        + 4 * std::mem::size_of::<usize>()
        <= LANE_DESCRIPTOR_BYTES as usize
);

/// Lanes and admitted bytes for parallel construction, or `None` when the
/// installed team offers one lane or the optional scratch does not fit. The
/// serial constructors remain the fallback; parallel scratch never refuses.
pub(super) fn plan(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    limits: KernelLimits,
) -> Option<(usize, u128)> {
    let n = design.nobs();
    let (p, q) = (
        design.effects()[left].n_levels(),
        design.effects()[1 - left].n_levels(),
    );
    // Lane-private occurrence markers stay within one observation bank.
    let lanes = StagePlan::current(n as u128, p).lanes.min(n / q.max(1));
    if lanes < 2 {
        return None;
    }
    let bytes = parallel_bytes(n as u128, p as u128, q as u128, lanes as u128);
    fits(limits, bytes).then_some((lanes, bytes))
}

/// Peak bytes: every final dimension bank, the largest of the three phases
/// (lane spans and markers; spans and compact cells; compact cells, staged
/// firm-range entries with their contributions or the firm index, plus
/// contribution scratch and cursors), lane descriptors and fixed stack.
fn parallel_bytes(n: u128, p: u128, q: u128, lanes: u128) -> u128 {
    let cell = std::mem::size_of::<Cell>() as u128;
    let staged = std::mem::size_of::<Staged>() as u128;
    let firm_cell = std::mem::size_of::<FirmCell>() as u128;
    let word = std::mem::size_of::<usize>() as u128;
    let banks = (p + 1) * word + (p + 2 * q) * 8 + (q + 1) * word;
    let spans = n * cell + lanes * 2 * q * word;
    let compaction = 2 * n * cell;
    let finish = n * cell
        + (n * 8 + n * staged).max(n * staged + n * firm_cell + (q + lanes) * word)
        + lanes * lanes * word;
    banks + spans.max(compaction).max(finish) + lanes * LANE_DESCRIPTOR_BYTES + 4096
}

/// One worker range of the aggregation phase. Allocation happens on the
/// caller thread so allocator faults keep their caller-thread plans.
struct Lane<'a> {
    index: usize,
    first: usize,
    /// `offsets[first + 1..]` for this range: lane-local cumulative counts,
    /// rebased after compaction.
    ends: &'a mut [usize],
    cells: Vec<Cell>,
    marks: Vec<usize>,
    slots: Vec<usize>,
    stop: Option<Stop>,
    overflow: bool,
    completed: usize,
}

struct Context<'a> {
    design: &'a BorrowedInterceptFeDesign<'a>,
    left: usize,
    weights: BorrowedMapWeights<'a>,
    scale: f64,
    order: Order,
    /// Test plan: the lane that requests cancellation after its first row.
    cancel_lane: Option<usize>,
}

fn aggregate(lane: &mut Lane<'_>, context: &Context<'_>) -> Attempt<()> {
    let q = context.design.effects()[1 - context.left].n_levels();
    let index = context.design.effects()[context.left].level_index();
    let firms = context.design.effects()[1 - context.left].codes();
    lane.marks.resize(q, usize::MAX);
    lane.slots.resize(q, 0);
    for local in 0..lane.ends.len() {
        crate::interrupt::checkpoint()?;
        let worker = lane.first + local;
        let start = lane.cells.len();
        for (position, &row) in index.checked_members(worker)?.iter().enumerate() {
            if position != 0 && position % 256 == 0 {
                crate::interrupt::checkpoint()?;
            }
            let firm = firms[row] as usize;
            let weight = match context.weights {
                BorrowedMapWeights::Unit => 1.0,
                BorrowedMapWeights::Provided(values) => values[row] / context.scale,
            };
            if !weight.is_finite() || weight <= 0.0 {
                return Err(invalid_data(
                    "cell kernel normalized weight must be finite and positive",
                )
                .into());
            }
            if lane.marks[firm] != worker {
                lane.marks[firm] = worker;
                lane.slots[firm] = lane.cells.len();
                // Capacity is this range's row count, so this never reallocates.
                lane.cells.push(Cell {
                    worker: worker as u32,
                    firm: firm as u32,
                    weight,
                });
            } else {
                let total = &mut lane.cells[lane.slots[firm]].weight;
                *total += weight;
                if !total.is_finite() {
                    // The radix constructor validates every row before it
                    // refuses an overflowing cell; occurrence order stops here.
                    match context.order {
                        Order::Occurrence => return Err(Stop::Refused(KernelRefusal::Numerical)),
                        Order::Firm => lane.overflow = true,
                    }
                }
            }
            if context.cancel_lane == Some(lane.index) && lane.completed == 0 {
                crate::interrupt::request_cancellation();
            }
            lane.completed += 1;
        }
        if context.order == Order::Firm {
            // Firms are unique within one worker, so the order is total.
            lane.cells[start..].sort_unstable_by_key(|cell| cell.firm);
        }
        lane.ends[local] = lane.cells.len();
    }
    Ok(())
}

/// First worker whose membership starts at or after `row`.
fn first_worker_at(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    row: usize,
) -> Result<usize> {
    let index = design.effects()[left].level_index();
    let (mut low, mut high) = (0, design.effects()[left].n_levels());
    while low < high {
        let middle = low + (high - low) / 2;
        if index.checked_member_range(middle)?.start < row {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    Ok(low)
}

fn member_start(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    worker: usize,
) -> Result<usize> {
    if worker == design.effects()[left].n_levels() {
        Ok(design.nobs())
    } else {
        Ok(design.effects()[left]
            .level_index()
            .checked_member_range(worker)?
            .start)
    }
}

/// Reserve on the caller (allocation faults stay observable), then first-touch
/// on the installed team.
fn zeroed<T: Copy + Send + Sync>(work: &mut Work, count: usize, value: T) -> Attempt<Vec<T>> {
    let mut result = work.capacity(count)?;
    initialize_reserved(&mut result, count, value);
    Ok(result)
}

/// Split `values` into consecutive spans of the given lengths.
fn spans<T>(mut values: &mut [T], lengths: impl Iterator<Item = usize>) -> Vec<&mut [T]> {
    let mut result = Vec::new();
    for length in lengths {
        let (head, tail) = std::mem::take(&mut values).split_at_mut(length);
        result.push(head);
        values = tail;
    }
    result
}

pub(super) fn construct(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    order: Order,
    lanes: usize,
    work: &mut Work,
) -> Attempt<CellSchur> {
    work.charge(64)?;
    let (n, p, q) = (
        design.nobs(),
        design.effects()[left].n_levels(),
        design.effects()[1 - left].n_levels(),
    );
    let mut bounds = work.capacity(lanes + 1)?;
    bounds.push(0);
    for lane in 1..lanes {
        let worker = first_worker_at(design, left, n / lanes * lane + n % lanes * lane / lanes)?;
        bounds.push(worker.max(bounds[lane - 1]));
    }
    bounds.push(p);
    let mut offsets = zeroed(work, p + 1, 0usize)?;
    let mut lane_states = work.capacity(lanes)?;
    {
        let ends = spans(&mut offsets[1..], bounds.windows(2).map(|w| w[1] - w[0]));
        for (index, ends) in ends.into_iter().enumerate() {
            let rows = member_start(design, left, bounds[index + 1])?
                - member_start(design, left, bounds[index])?;
            lane_states.push(Lane {
                index,
                first: bounds[index],
                ends,
                cells: work.capacity(rows)?,
                marks: work.capacity(q)?,
                slots: work.capacity(q)?,
                stop: None,
                overflow: false,
                completed: 0,
            });
        }
    }
    let context = Context {
        design,
        left,
        weights,
        scale,
        order,
        cancel_lane: (CELL_CANCEL_PHASE.get() == 1).then_some(lanes - 1),
    };
    let aggregated = stage_try_for_each(lane_states.par_iter_mut(), n as u128, |lane| {
        if let Err(stop) = aggregate(lane, &context) {
            lane.stop = Some(stop);
        }
        Ok(())
    });
    if CELL_CANCEL_PHASE.get() == 1 {
        let rows = lane_states.iter().map(|lane| lane.completed).sum::<usize>();
        CELL_COMPLETED_WORK.set(CELL_COMPLETED_WORK.get() + 32 * rows);
    }
    aggregated?;
    // Occurrence order reports the first stop in worker order; the radix
    // constructor reports any invalid row before any overflowing cell.
    let mut refused = false;
    for lane in &mut lane_states {
        match lane.stop.take() {
            Some(Stop::Fatal(error)) => return Err(Stop::Fatal(error)),
            Some(Stop::Refused(reason)) if order == Order::Occurrence => {
                return Err(Stop::Refused(reason))
            }
            Some(Stop::Refused(_)) => refused = true,
            None => {}
        }
        refused |= lane.overflow;
        lane.marks = Vec::new();
        lane.slots = Vec::new();
    }
    if refused {
        return Err(Stop::Refused(KernelRefusal::Numerical));
    }
    crate::interrupt::checkpoint()?;
    let mut bases = work.capacity(lanes + 1)?;
    bases.push(0usize);
    for lane in &lane_states {
        bases.push(bases[bases.len() - 1] + lane.cells.len());
    }
    let total = bases[lanes];
    let mut cells = work.capacity(total)?;
    {
        let (lane_states, bases) = (&lane_states, &bases);
        (0..total)
            .into_par_iter()
            .with_min_len(StagePlan::current(total as u128, total).chunk(total))
            .map(|cell| {
                let lane = bases.partition_point(|&base| base <= cell) - 1;
                lane_states[lane].cells[cell - bases[lane]]
            })
            .collect_into_vec(&mut cells);
    }
    if cells.capacity() != total {
        return Err(allocation("cell kernel capacity exceeds admission").into());
    }
    stage_try_for_each(lane_states.par_iter_mut(), p as u128, |lane| {
        let base = bases[lane.index];
        for end in lane.ends.iter_mut() {
            *end += base;
        }
        Ok(())
    })?;
    drop(lane_states);
    drop(bases);
    crate::interrupt::checkpoint()?;
    finish(cells, offsets, &bounds, q, work)
}

/// A cell with its Schur-diagonal contribution, grouped by firm range.
#[derive(Clone, Copy, Default)]
struct Staged {
    cell: Cell,
    contribution: f64,
}

/// One worker range of the finishing phase.
struct Block<'a> {
    first: usize,
    worker_weights: &'a mut [f64],
    contributions: &'a mut [f64],
    staged: &'a mut [Staged],
    /// Cells per firm range; after staging, each range's end in the block.
    counts: &'a mut [usize],
    refused: bool,
}

/// One firm range of the finishing phase.
struct Range<'a> {
    index: usize,
    first: usize,
    base: usize,
    firm_cells: &'a mut [FirmCell],
    firm_ends: &'a mut [usize],
    firm_weights: &'a mut [f64],
    diagonal: &'a mut [f64],
    cursors: &'a mut [usize],
    refused: bool,
}

fn firm_range(firm_bounds: &[usize], firm: u32) -> usize {
    firm_bounds.partition_point(|&bound| bound <= firm as usize) - 1
}

/// Worker totals and Schur contributions in the serial per-worker order.
fn worker_contributions(
    block: &mut Block<'_>,
    cells: &[Cell],
    offsets: &[usize],
    firm_bounds: &[usize],
) -> Result<()> {
    let start = offsets[block.first];
    for (local, total_slot) in block.worker_weights.iter_mut().enumerate() {
        crate::interrupt::checkpoint()?;
        let worker = block.first + local;
        let (first, end) = (offsets[worker], offsets[worker + 1]);
        let mut total = 0.0;
        for cell in &cells[first..end] {
            total += cell.weight;
        }
        *total_slot = total;
        if !total.is_finite() {
            block.refused = true;
            return Ok(());
        }
        let worker_cells = &cells[first..end];
        let slots = &mut block.contributions[first - start..end - start];
        // Exclusive suffix first, as the serial suffix[i + 1].
        let mut suffix = 0.0;
        for (slot, cell) in slots.iter_mut().zip(worker_cells).rev() {
            *slot = suffix;
            suffix += cell.weight;
        }
        let mut prefix = 0.0;
        for (slot, cell) in slots.iter_mut().zip(worker_cells) {
            let other = prefix + *slot;
            *slot = if cell.weight >= other {
                (cell.weight / total) * other
            } else {
                (other / total) * cell.weight
            };
            prefix += cell.weight;
            block.counts[firm_range(firm_bounds, cell.firm)] += 1;
        }
    }
    Ok(())
}

/// Stable grouping of one block's cells by firm range.
fn stage(block: &mut Block<'_>, cells: &[Cell], firm_bounds: &[usize], start: usize) -> Result<()> {
    let mut next = 0;
    for count in block.counts.iter_mut() {
        let length = *count;
        *count = next;
        next += length;
    }
    for (local, &contribution) in block.contributions.iter().enumerate() {
        crate::interrupt::row_checkpoint(local)?;
        let cell = cells[start + local];
        let cursor = &mut block.counts[firm_range(firm_bounds, cell.firm)];
        block.staged[*cursor] = Staged { cell, contribution };
        *cursor += 1;
    }
    Ok(())
}

/// Counting sort of one firm range's staged cells, in block (worker) order,
/// with the serial firm totals and diagonal sums in that same order.
fn index_range(
    range: &mut Range<'_>,
    blocks: &[(usize, &[usize])],
    staged: &[Staged],
) -> Result<()> {
    let index = range.index;
    let segments = || {
        blocks.iter().map(move |&(start, ends)| {
            let first = if index == 0 { 0 } else { ends[index - 1] };
            &staged[start + first..start + ends[index]]
        })
    };
    for segment in segments() {
        crate::interrupt::checkpoint()?;
        for (local, entry) in segment.iter().enumerate() {
            crate::interrupt::row_checkpoint(local)?;
            range.cursors[entry.cell.firm as usize - range.first + 1] += 1;
        }
    }
    for firm in 0..range.firm_ends.len() {
        crate::interrupt::row_checkpoint(firm)?;
        range.cursors[firm + 1] += range.cursors[firm];
        range.firm_ends[firm] = range.base + range.cursors[firm + 1];
    }
    for segment in segments() {
        crate::interrupt::checkpoint()?;
        for (local, entry) in segment.iter().enumerate() {
            crate::interrupt::row_checkpoint(local)?;
            let firm = entry.cell.firm as usize - range.first;
            let slot = &mut range.cursors[firm];
            range.firm_cells[*slot] = FirmCell {
                worker: entry.cell.worker,
                weight: entry.cell.weight,
            };
            *slot += 1;
            range.firm_weights[firm] += entry.cell.weight;
            range.diagonal[firm] += entry.contribution;
        }
    }
    for (firm, diagonal) in range.diagonal.iter_mut().enumerate() {
        crate::interrupt::row_checkpoint(firm)?;
        let total = range.firm_weights[firm];
        if !total.is_finite() || !diagonal.is_finite() || *diagonal < 0.0 {
            range.refused = true;
            return Ok(());
        }
        if *diagonal == 0.0 {
            *diagonal = total;
        }
    }
    Ok(())
}

fn finish(
    cells: Vec<Cell>,
    offsets: Vec<usize>,
    bounds: &[usize],
    q: usize,
    work: &mut Work,
) -> Attempt<CellSchur> {
    let lanes = bounds.len() - 1;
    let (p, total) = (bounds[lanes], cells.len());
    let work_units = total as u128;
    let mut firm_bounds = work.capacity(lanes + 1)?;
    for range in 0..=lanes {
        firm_bounds.push(q / lanes * range + q % lanes * range / lanes);
    }
    let mut worker_weights = zeroed(work, p, 0.0)?;
    let mut contributions = zeroed(work, total, 0.0)?;
    let mut staged = zeroed(work, total, Staged::default())?;
    let mut counts = zeroed(work, lanes * lanes, 0usize)?;
    let block_cells = || bounds.windows(2).map(|w| offsets[w[1]] - offsets[w[0]]);
    let mut blocks = work.capacity(lanes)?;
    for ((((first, worker_weights), contributions), staged), counts) in bounds
        .iter()
        .copied()
        .zip(spans(
            &mut worker_weights,
            bounds.windows(2).map(|w| w[1] - w[0]),
        ))
        .zip(spans(&mut contributions, block_cells()))
        .zip(spans(&mut staged, block_cells()))
        .zip(counts.chunks_mut(lanes))
    {
        blocks.push(Block {
            first,
            worker_weights,
            contributions,
            staged,
            counts,
            refused: false,
        });
    }
    stage_try_for_each(blocks.par_iter_mut(), work_units, |block| {
        worker_contributions(block, &cells, &offsets, &firm_bounds)
    })?;
    if blocks.iter().any(|block| block.refused) {
        return Err(Stop::Refused(KernelRefusal::Numerical));
    }
    stage_try_for_each(blocks.par_iter_mut(), work_units, |block| {
        stage(block, &cells, &firm_bounds, offsets[block.first])
    })?;
    drop(blocks);
    drop(contributions);
    // Firm range r starts after every block's cells in earlier ranges.
    let mut bases = work.capacity(lanes + 1)?;
    bases.push(0usize);
    for range in 0..lanes {
        let cells_in_range = counts
            .chunks(lanes)
            .map(|ends| ends[range] - if range == 0 { 0 } else { ends[range - 1] })
            .sum::<usize>();
        bases.push(bases[range] + cells_in_range);
    }
    let mut firm_cells = zeroed(work, total, FirmCell::default())?;
    let mut firm_offsets = zeroed(work, q + 1, 0usize)?;
    let mut firm_weights = zeroed(work, q, 0.0)?;
    let mut diagonal = zeroed(work, q, 0.0)?;
    let mut cursors = zeroed(work, q + lanes, 0usize)?;
    let widths = || firm_bounds.windows(2).map(|w| w[1] - w[0]);
    let mut ranges = work.capacity(lanes)?;
    for (index, ((((firm_cells, firm_ends), firm_weights), diagonal), cursors)) in
        spans(&mut firm_cells, bases.windows(2).map(|w| w[1] - w[0]))
            .into_iter()
            .zip(spans(&mut firm_offsets[1..], widths()))
            .zip(spans(&mut firm_weights, widths()))
            .zip(spans(&mut diagonal, widths()))
            .zip(spans(&mut cursors, widths().map(|width| width + 1)))
            .enumerate()
    {
        ranges.push(Range {
            index,
            first: firm_bounds[index],
            base: bases[index],
            firm_cells,
            firm_ends,
            firm_weights,
            diagonal,
            cursors,
            refused: false,
        });
    }
    let mut segments = work.capacity(lanes)?;
    for (block, ends) in counts.chunks(lanes).enumerate() {
        segments.push((offsets[bounds[block]], ends));
    }
    stage_try_for_each(ranges.par_iter_mut(), work_units, |range| {
        index_range(range, &segments, &staged)
    })?;
    if ranges.iter().any(|range| range.refused) {
        return Err(Stop::Refused(KernelRefusal::Numerical));
    }
    drop(ranges);
    drop(segments);
    drop(staged);
    work.charge(32)?;
    Ok(CellSchur {
        cells,
        offsets,
        firm_offsets,
        firm_cells,
        worker_weights,
        firm_weights,
        diagonal,
    })
}

#[cfg(test)]
#[path = "parallel_build_tests.rs"]
mod tests;
