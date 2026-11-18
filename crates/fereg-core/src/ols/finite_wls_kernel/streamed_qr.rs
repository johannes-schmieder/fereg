//! Observation-order Givens insertion into one retained factor.

use super::{
    fill_retained_qr_row, hypot, invalid_argument, invalid_data, rotate_streamed_qr_tail,
    FixedBlockMomentSource, Result, FIXED_BLOCK_MOMENT_MAX_COLUMNS,
};
use rayon::prelude::*;

const TILE_OBSERVATIONS: usize = 8;
const TILE_PIVOTS: usize = 8;
const MAX_PANEL_ROWS: usize = 128;

pub(in crate::ols) fn blocked_givens_eligible(source: FixedBlockMomentSource<'_>) -> bool {
    matches!(source, FixedBlockMomentSource::Dense { .. })
        && source.dimension() > FIXED_BLOCK_MOMENT_MAX_COLUMNS
        && source.rows() > TILE_OBSERVATIONS
}

/// The original observation-outer stream also owns ordered error diagnostics.
pub(super) fn scalar(
    source: FixedBlockMomentSource<'_>,
    stored_row: &mut [f64],
    row: &mut [f64],
    qr: &mut [f64],
    rhs: &mut [f64],
) -> Result<()> {
    let mut reader = match source {
        FixedBlockMomentSource::Scratch { values, .. } => Some(values.shared_reader()),
        _ => None,
    };
    for observation in 0..source.rows() {
        crate::interrupt::row_checkpoint(observation)?;
        let mut outcome =
            fill_retained_qr_row(source, observation, stored_row, row, reader.as_mut())?;
        insert_pivots(0, qr, rhs, row, &mut outcome)?;
    }
    Ok(())
}

/// A rotation touches only this factor row, this observation and their two
/// response entries. Both the scalar stream and the tiles use this operation.
fn insert_pivots(
    first_pivot: usize,
    factor_rows: &mut [f64],
    rhs: &mut [f64],
    row: &mut [f64],
    outcome: &mut f64,
) -> Result<()> {
    for (offset, (upper, upper_rhs)) in factor_rows.chunks_exact_mut(row.len()).zip(rhs).enumerate()
    {
        let pivot = first_pivot + offset;
        let diagonal = upper[pivot];
        let incoming = row[pivot];
        if incoming == 0.0 {
            continue;
        }
        let rotated = hypot(diagonal, incoming);
        if !rotated.is_finite() || rotated <= 0.0 {
            return Err(invalid_data(
                "retained finite-WLS streaming QR rotation is invalid",
            ));
        }
        let cosine = diagonal / rotated;
        let sine = incoming / rotated;
        upper[pivot] = rotated;
        row[pivot] = 0.0;
        let finite_tail =
            rotate_streamed_qr_tail(&mut upper[pivot + 1..], &mut row[pivot + 1..], cosine, sine);
        let before = *upper_rhs;
        *upper_rhs = cosine * before + sine * *outcome;
        *outcome = -sine * before + cosine * *outcome;
        if !finite_tail || !upper_rhs.is_finite() || !outcome.is_finite() {
            return Err(invalid_data(
                "retained finite-WLS streaming QR is not finite",
            ));
        }
    }
    Ok(())
}

pub(super) fn blocked(
    source: FixedBlockMomentSource<'_>,
    stored_row: &mut [f64],
    workspace: &mut [f64],
    qr: &mut [f64],
    rhs: &mut [f64],
) -> Result<()> {
    let k = rhs.len();
    let width = k + 1;
    let panel_rows = (workspace.len() / width).min(MAX_PANEL_ROWS);
    if !blocked_givens_eligible(source) || panel_rows <= TILE_OBSERVATIONS {
        return Err(invalid_argument(
            "blocked retained QR requires a wide dense source and staging space",
        ));
    }
    let parallel = crate::parallel::in_pool() && rayon::current_num_threads() > 1;
    for first in (0..source.rows()).step_by(panel_rows) {
        let count = (source.rows() - first).min(panel_rows);
        let panel = &mut workspace[..count * width];
        for (offset, packed) in panel.chunks_exact_mut(width).enumerate() {
            crate::interrupt::row_checkpoint(first + offset)?;
            let (row, outcome) = packed.split_at_mut(k);
            outcome[0] = fill_retained_qr_row(source, first + offset, stored_row, row, None)?;
        }
        if parallel {
            insert_waves(k, panel, qr, rhs)?;
        } else {
            for (p, (factor, rhs)) in qr
                .chunks_mut(TILE_PIVOTS * k)
                .zip(rhs.chunks_mut(TILE_PIVOTS))
                .enumerate()
            {
                crate::interrupt::checkpoint()?;
                insert_tile(k, p * TILE_PIVOTS, factor, rhs, panel)?;
            }
        }
    }
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=retained_qr_compression|method=blocked_givens|status=completed|rows={}|columns={k}|panel_rows={panel_rows}|parallel={parallel}",
            source.rows()
        ));
    }
    Ok(())
}

/// Tile (b,p) needs observation group b after pivot slab p-1, and pivot slab p
/// after observation group b-1. Equal b+p tiles therefore have disjoint writes.
/// Iterating factor slabs forward and observation groups backward pairs them.
fn insert_waves(k: usize, panel: &mut [f64], qr: &mut [f64], rhs: &mut [f64]) -> Result<()> {
    let width = k + 1;
    let observations = panel.len() / width;
    let groups = observations.div_ceil(TILE_OBSERVATIONS);
    let slabs = k.div_ceil(TILE_PIVOTS);
    for wave in 0..groups + slabs - 1 {
        crate::interrupt::checkpoint()?;
        let first_slab = wave.saturating_sub(groups - 1);
        let last_slab = wave.min(slabs - 1);
        let first_pivot = first_slab * TILE_PIVOTS;
        let end_pivot = ((last_slab + 1) * TILE_PIVOTS).min(k);
        let first_row = (wave - last_slab) * TILE_OBSERVATIONS;
        let end_row = ((wave - first_slab + 1) * TILE_OBSERVATIONS).min(observations);
        let factor = &mut qr[first_pivot * k..end_pivot * k];
        let rhs = &mut rhs[first_pivot..end_pivot];
        let rows = &mut panel[first_row * width..end_row * width];
        if first_slab == last_slab {
            insert_tile(k, first_pivot, factor, rhs, rows)?;
            continue;
        }
        // Six arithmetic operations update each pair of tail entries.
        let work = 3
            * TILE_OBSERVATIONS as u128
            * (end_pivot - first_pivot) as u128
            * (2 * k - first_pivot - end_pivot + 1) as u128;
        crate::parallel::stage_try_for_each(
            factor
                .par_chunks_mut(TILE_PIVOTS * k)
                .zip(rhs.par_chunks_mut(TILE_PIVOTS))
                .zip(rows.par_chunks_mut(TILE_OBSERVATIONS * width).rev())
                .enumerate(),
            work,
            |(offset, ((factor, rhs), rows))| {
                insert_tile(k, first_pivot + offset * TILE_PIVOTS, factor, rhs, rows)
            },
        )?;
    }
    Ok(())
}

fn insert_tile(
    k: usize,
    first_pivot: usize,
    qr: &mut [f64],
    rhs: &mut [f64],
    panel: &mut [f64],
) -> Result<()> {
    for packed in panel.chunks_exact_mut(k + 1) {
        let (row, outcome) = packed.split_at_mut(k);
        insert_pivots(first_pivot, qr, rhs, row, &mut outcome[0])?;
    }
    Ok(())
}
