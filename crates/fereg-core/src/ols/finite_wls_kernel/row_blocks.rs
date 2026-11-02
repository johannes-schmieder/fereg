//! Complete symmetric moments per observation block. Packing and eight FMA
//! chains expose vector work even with one regressor; a fixed tree owns order.
use super::*;
use crate::ols::centered::CenteredRows;

const PANEL_ELEMENTS: usize = 32_768;
const PANEL_ROWS: usize = 256;
const PARTIAL_TARGET_BYTES: usize = 4 * 1024 * 1024;
const MAX_BLOCKS: usize = 64;
const CHAINS: usize = 8;

#[derive(Clone, Copy)]
struct Shape {
    width: usize,
    pairs: usize,
    panel_rows: usize,
    panel: usize,
    lanes: usize,
    stride: usize,
}
impl Shape {
    fn new(columns: usize, stored: usize) -> Result<Self> {
        let width = columns
            .checked_add(1)
            .ok_or_else(|| allocation("centered panel width overflows"))?;
        let pairs = width
            .checked_mul(width + 1)
            .and_then(|v| v.checked_div(2))
            .ok_or_else(|| allocation("centered symmetric shape overflows"))?;
        let panel_rows = (PANEL_ELEMENTS / width).clamp(1, PANEL_ROWS);
        let panel = panel_rows
            .checked_mul(width)
            .ok_or_else(|| allocation("centered panel overflows"))?;
        let lanes = pairs
            .checked_mul(CHAINS)
            .ok_or_else(|| allocation("centered moment lanes overflow"))?;
        let stride = panel
            .checked_add(lanes)
            .and_then(|v| v.checked_add(stored)?.checked_add(1))
            .ok_or_else(|| allocation("centered moment workspace overflows"))?;
        Ok(Self {
            width,
            pairs,
            panel_rows,
            panel,
            lanes,
            stride,
        })
    }
}

pub(super) fn layout(rows: usize, columns: usize, stored: usize) -> Result<FiniteWlsMomentLayout> {
    let shape = Shape::new(columns, stored)?;
    let partitions = crate::parallel::reduction_blocks(
        rows,
        FIXED_BLOCK_MOMENT_ROWS,
        MAX_BLOCKS,
        shape.stride,
        PARTIAL_TARGET_BYTES,
    )?;
    Ok(FiniteWlsMomentLayout {
        partitions,
        columns_per_partition: columns,
        partial_stride: shape.stride,
        partial_elements: partitions
            .checked_mul(shape.stride)
            .ok_or_else(|| allocation("centered partial bank overflows"))?,
    })
}

pub(super) fn workspace_bytes(rows: usize, columns: usize, stored: usize) -> Result<u128> {
    let stride = Shape::new(columns, stored)?.stride as u128;
    let blocks = rows.div_ceil(FIXED_BLOCK_MOMENT_ROWS).clamp(1, MAX_BLOCKS) as u128;
    Ok((blocks * stride).min((PARTIAL_TARGET_BYTES as u128 / 8).max(stride)) * 8)
}

pub(super) fn fill(
    values: CenteredRows<'_>,
    selected: Option<&[usize]>,
    scales: Option<&[f64]>,
    layout: FiniteWlsMomentLayout,
    partials: &mut [f64],
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<()> {
    let count = rhs.len();
    let shape = Shape::new(count, values.columns())?;
    let chunk = values.rows().div_ceil(layout.partitions);
    let response_scale = values.summary().raw_norms()[0];
    let response_scale = if response_scale == 0.0 {
        1.0
    } else {
        response_scale
    };
    let hardware = crate::kernels::installed();
    let mut trace =
        crate::diagnostics::phase_ledger::PhaseLedger::new("centered_moments", ["leaves", "merge"]);
    trace.facts(&[
        ("rows", values.rows() as u64),
        ("columns", count as u64),
        ("blocks", layout.partitions as u64),
    ]);
    crate::parallel::stage_try_for_each(
        partials.par_chunks_mut(shape.stride).enumerate(),
        values.rows() as u128 * shape.pairs as u128,
        |(block, bank)| {
            let start = block * chunk;
            let end = (start + chunk).min(values.rows());
            let (panel, bank) = bank.split_at_mut(shape.panel);
            let (lanes, row) = bank.split_at_mut(shape.lanes);
            lanes.fill(0.0);
            for first in (start..end).step_by(shape.panel_rows) {
                crate::interrupt::checkpoint()?;
                let length = (end - first).min(shape.panel_rows);
                values.pack_panel(first, length, selected, scales, response_scale, panel, row)?;
                crate::kernels::dispatch!(
                    hardware,
                    gram_panel(panel, length, shape.width, 0, length, lanes)
                );
            }
            Ok(())
        },
    )?;
    trace.lap(0);
    // Reduce each leaf's independent chains first, then merge leaves in a
    // fixed binary tree. Tree shape depends on matrix shape, not worker count.
    for bank in partials.chunks_mut(shape.stride) {
        let lanes = &mut bank[shape.panel..shape.panel + shape.lanes];
        for pair in 0..shape.pairs {
            let mut sum = 0.0;
            let mut correction = 0.0;
            for chain in 0..CHAINS {
                neumaier_add(lanes[pair * CHAINS + chain], &mut sum, &mut correction);
            }
            lanes[pair] = sum + correction;
        }
    }
    for distance in (0..usize::BITS)
        .map(|bit| 1usize << bit)
        .take_while(|d| *d < layout.partitions)
    {
        crate::interrupt::checkpoint()?;
        for left in (0..layout.partitions).step_by(distance * 2) {
            let right = left + distance;
            if right >= layout.partitions {
                continue;
            }
            for pair in 0..shape.pairs {
                let other = partials[right * shape.stride + shape.panel + pair];
                partials[left * shape.stride + shape.panel + pair] += other;
            }
        }
    }
    let total = &partials[shape.panel..shape.panel + shape.pairs];
    let mut pair = 0;
    for left in 0..shape.width {
        for right in left..shape.width {
            let value = total[pair];
            if !value.is_finite() {
                return Err(invalid_data("centered symmetric moment is not finite"));
            }
            if left < count {
                if right < count {
                    gram[left * count + right] = value;
                    gram[right * count + left] = value;
                } else {
                    rhs[left] = value * response_scale;
                    if !rhs[left].is_finite() {
                        return Err(invalid_data("centered response moment is not finite"));
                    }
                }
            }
            pair += 1;
        }
    }
    trace.lap(1);
    Ok(())
}

#[cfg(test)]
#[path = "row_blocks/tests.rs"]
mod tests;
