//! Bounded Householder tall-skinny compression, without rank decisions or Q.
//!
//! Each shape-dependent leaf retains only a triangular factor and a fresh row
//! panel. A fixed binary tree compresses stacked factors. All workspaces belong
//! to the caller; the final factor enters the common canonical pivoted QR.

use super::*;

const TILE_ROWS: usize = 256;
const TILE_ELEMENTS: usize = 32_768;
const TARGET_BYTES: usize = 4 * 1024 * 1024;
const MAX_PARTS: usize = 64;

#[derive(Clone, Copy)]
struct Shape {
    dimension: usize,
    stored: usize,
    tile: usize,
    height: usize,
    panel: usize,
    stride: usize,
    parts: usize,
}

impl Shape {
    fn new(rows: usize, dimension: usize, stored: usize) -> Result<Self> {
        if rows == 0 || dimension == 0 || stored < dimension {
            return Err(invalid_argument("TSQR requires nonempty, consistent axes"));
        }
        let width = dimension.checked_add(1).ok_or_else(size_error)?;
        let tile = (TILE_ELEMENTS / width).clamp(1, TILE_ROWS);
        let height = dimension
            .checked_add(tile)
            .ok_or_else(size_error)?
            .max(dimension.checked_mul(2).ok_or_else(size_error)?);
        let panel = height.checked_mul(width).ok_or_else(size_error)?;
        let stride = panel
            .checked_add(stored)
            .and_then(|n| n.checked_add(dimension))
            .ok_or_else(size_error)?;
        let parts = crate::parallel::reduction_blocks(
            rows,
            FIXED_BLOCK_MOMENT_ROWS,
            MAX_PARTS,
            stride,
            TARGET_BYTES,
        )?;
        Ok(Self {
            dimension,
            stored,
            tile,
            height,
            panel,
            stride,
            parts,
        })
    }

    fn elements(self) -> Result<usize> {
        self.parts.checked_mul(self.stride).ok_or_else(size_error)
    }
}

fn size_error() -> crate::error::FeregError {
    allocation("TSQR workspace dimensions overflow")
}

pub(super) fn workspace_elements(rows: usize, dimension: usize, stored: usize) -> Result<usize> {
    Shape::new(rows, dimension, stored)?.elements()
}

/// Admission envelope over every selected rank from one through `dimension`.
/// The target bounds all partials unless a single complete bank is larger.
pub(super) fn workspace_envelope_bytes(
    rows: usize,
    dimension: usize,
    stored: usize,
) -> Result<u128> {
    Shape::new(rows, dimension, stored)?;
    let width = dimension as u128 + 1;
    let height = 2 * dimension as u128 + TILE_ROWS as u128;
    let maximum_stride = height * width + stored as u128 + dimension as u128;
    let parts = rows.div_ceil(FIXED_BLOCK_MOMENT_ROWS).clamp(1, MAX_PARTS) as u128;
    Ok((parts * maximum_stride).min((TARGET_BYTES as u128 / 8).max(maximum_stride)) * 8)
}

/// Source validation precedes this call. Centered response values are scaled
/// before all reflectors and unscaled at the root to avoid intermediate range
/// failures. Other source families retain their original response units.
pub(super) fn compress(
    source: FixedBlockMomentSource<'_>,
    workspace: &mut [f64],
    qr: &mut [f64],
    rhs: &mut [f64],
) -> Result<()> {
    let stored = match source {
        FixedBlockMomentSource::Dense { stored_columns, .. } => stored_columns,
        FixedBlockMomentSource::Scratch { values, .. } => values.columns(),
        FixedBlockMomentSource::Implicit { values, .. } => values.columns(),
        FixedBlockMomentSource::Centered { values, .. } => values.columns(),
    };
    let shape = Shape::new(source.rows(), source.dimension(), stored)?;
    let elements = shape.elements()?;
    if workspace.len() < elements
        || qr.len()
            != shape
                .dimension
                .checked_mul(shape.dimension)
                .ok_or_else(size_error)?
        || rhs.len() != shape.dimension
    {
        return Err(invalid_argument(
            "TSQR output or workspace axes are inconsistent",
        ));
    }
    let response_scale = match source {
        FixedBlockMomentSource::Centered { values, .. } => {
            let norm = values.summary().raw_norms()[0];
            if norm == 0.0 {
                1.0
            } else {
                norm
            }
        }
        _ => 1.0,
    };
    let workspace = &mut workspace[..elements];
    let work = source.rows() as u128 * shape.dimension as u128 * shape.dimension as u128;
    crate::parallel::stage_try_for_each(
        workspace.par_chunks_mut(shape.stride).enumerate(),
        work,
        |(leaf, bank)| {
            let base = source.rows() / shape.parts;
            let tail = source.rows() % shape.parts;
            let start = leaf * base + leaf.min(tail);
            let end = start + base + usize::from(leaf < tail);
            compress_leaf(source, shape, start, end, response_scale, bank)
        },
    )?;
    let mut distance = 1;
    while distance < shape.parts {
        crate::interrupt::checkpoint()?;
        let offset = distance * shape.stride;
        crate::parallel::stage_try_for_each(
            workspace.par_chunks_mut(2 * offset),
            shape.parts as u128 * (shape.dimension as u128).pow(3),
            |group| {
                if group.len() <= offset {
                    return Ok(());
                }
                let (left, right) = group.split_at_mut(offset);
                merge(shape, &mut left[..shape.stride], &right[..shape.stride])
            },
        )?;
        distance *= 2;
    }
    qr.fill(0.0);
    for row in 0..shape.dimension {
        for column in row..shape.dimension {
            qr[row * shape.dimension + column] = workspace[column * shape.height + row];
        }
        rhs[row] = workspace[shape.dimension * shape.height + row] * response_scale;
        if !rhs[row].is_finite() {
            return Err(invalid_data("TSQR transformed response is not finite"));
        }
    }
    crate::interrupt::checkpoint()
}

fn compress_leaf(
    source: FixedBlockMomentSource<'_>,
    shape: Shape,
    start: usize,
    end: usize,
    response_scale: f64,
    bank: &mut [f64],
) -> Result<()> {
    let (panel, row) = bank.split_at_mut(shape.panel);
    let (stored, selected) = row.split_at_mut(shape.stored);
    panel.fill(0.0);
    let mut reader = match source {
        FixedBlockMomentSource::Scratch { values, .. } => Some(values.shared_reader()),
        _ => None,
    };
    for first in (start..end).step_by(shape.tile) {
        crate::interrupt::checkpoint()?;
        clear_top_tails(shape, panel);
        let length = (end - first).min(shape.tile);
        for offset in 0..length {
            let response =
                fill_retained_qr_row(source, first + offset, stored, selected, reader.as_mut())?;
            for column in 0..shape.dimension {
                panel[column * shape.height + shape.dimension + offset] = selected[column];
            }
            panel[shape.dimension * shape.height + shape.dimension + offset] =
                response / response_scale;
        }
        factor_panel(shape, shape.dimension + length, panel)?;
    }
    Ok(())
}

fn clear_top_tails(shape: Shape, panel: &mut [f64]) {
    for column in 0..shape.dimension {
        panel[column * shape.height + column + 1..column * shape.height + shape.dimension]
            .fill(0.0);
    }
}

fn merge(shape: Shape, left: &mut [f64], right: &[f64]) -> Result<()> {
    let panel = &mut left[..shape.panel];
    clear_top_tails(shape, panel);
    for column in 0..=shape.dimension {
        for row in 0..shape.dimension {
            panel[column * shape.height + shape.dimension + row] = if row <= column {
                right[column * shape.height + row]
            } else {
                0.0
            };
        }
    }
    factor_panel(shape, 2 * shape.dimension, panel)
}

fn factor_panel(shape: Shape, rows: usize, panel: &mut [f64]) -> Result<()> {
    let hardware = crate::kernels::installed();
    for column in 0..shape.dimension {
        crate::interrupt::checkpoint()?;
        let (prefix, targets) = panel.split_at_mut((column + 1) * shape.height);
        let reflector = &mut prefix[column * shape.height + column..column * shape.height + rows];
        let Some((diagonal, tau)) = reflector_parameters(reflector)? else {
            continue;
        };
        for target in targets.chunks_exact_mut(shape.height) {
            let target = &mut target[column..rows];
            let projection =
                tau * crate::kernels::dispatch!(hardware, normalized_product(reflector, target));
            if !projection.is_finite() {
                return Err(invalid_data("TSQR reflector projection is not finite"));
            }
            crate::kernels::dispatch!(hardware, scaled_add(reflector, -projection, target));
            if target.iter().any(|value| !value.is_finite()) {
                return Err(invalid_data("TSQR transformed panel is not finite"));
            }
        }
        reflector[0] = diagonal;
    }
    Ok(())
}

/// Scale before squaring and form the reflector using a normalized leading
/// value, so neither x'x nor x[0]-beta needs to be representable in raw units.
fn reflector_parameters(values: &mut [f64]) -> Result<Option<(f64, f64)>> {
    let mut maximum = 0.0_f64;
    for &value in values.iter() {
        if !value.is_finite() {
            return Err(invalid_data("TSQR input panel is not finite"));
        }
        maximum = maximum.max(value.abs());
    }
    if maximum == 0.0 {
        return Ok(None);
    }
    for value in values.iter_mut() {
        *value /= maximum;
    }
    let hardware = crate::kernels::installed();
    let norm = crate::kernels::dispatch!(hardware, normalized_product(values, values)).sqrt();
    let leading = values[0] / norm;
    let sign = if leading.is_sign_negative() {
        -1.0
    } else {
        1.0
    };
    let denominator = leading + sign;
    let diagonal = -sign * (maximum * norm);
    if !diagonal.is_finite() {
        return Err(invalid_data("TSQR triangular diagonal is not finite"));
    }
    for value in &mut values[1..] {
        *value = (*value / norm) / denominator;
    }
    values[0] = 1.0;
    Ok(Some((diagonal, 1.0 + leading.abs())))
}

#[cfg(test)]
#[path = "tsqr/tests.rs"]
mod tests;
