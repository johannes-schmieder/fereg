//! Bounded dense Gram partitions with observation-ordered updates.

use super::{
    moment_lane_work, normalized_weight, record_retained_moment_worker,
    with_private_retained_moments, DenseRowsPass, FIXED_BLOCK_MOMENT_MAX_COLUMNS,
};
use crate::error::{invalid_data, Result};

pub(super) fn fill(pass: DenseRowsPass<'_>, rhs: &mut [f64], gram: &mut [f64]) -> Result<()> {
    let dimension = pass.dimension;
    // Column partitions own disjoint Gram/RHS entries. Split by the number
    // of updates, since early Gram rows are longer than late ones. This only
    // changes the worker that performs an entry's original observation fold.
    let work = moment_lane_work(pass.response.len(), dimension);
    let lanes = crate::parallel::StagePlan::current(work, dimension).lanes;
    let mut tasks = [(); FIXED_BLOCK_MOMENT_MAX_COLUMNS];
    let mut offsets = [0; FIXED_BLOCK_MOMENT_MAX_COLUMNS + 1];
    for column in 0..dimension {
        offsets[column + 1] = offsets[column] + (dimension - column) + 1;
    }
    crate::parallel::group_blocks(
        crate::parallel::GroupBuffers {
            first: rhs,
            first_stride: 1,
            second: gram,
            second_stride: dimension,
            scratch: &mut tasks[..lanes],
        },
        &offsets[..=dimension],
        0,
        &|left_start, rhs_rows, gram_rows, _| {
            record_retained_moment_worker();
            with_private_retained_moments(rhs_rows, gram_rows, |rhs, gram, standardized| {
                accumulate_dense_suffixes(pass, left_start, rhs, gram, standardized)
            })
        },
    )
}

/// Standardize each needed column exactly once per partition and row. A
/// contiguous selection permits a division loop over three contiguous slices.
fn accumulate_dense_suffixes(
    pass: DenseRowsPass<'_>,
    left_start: usize,
    rhs: &mut [f64],
    gram: &mut [f64],
    standardized: &mut [f64],
) -> Result<()> {
    let selected = pass.selected_columns.map(|ids| &ids[left_start..]);
    let first = selected.map_or(left_start, |ids| ids[0]);
    let end = selected.map_or(pass.dimension, |ids| ids[ids.len() - 1] + 1);
    let scattered = selected.filter(|ids| end - first != ids.len());
    let scales = pass.scales.map(|values| &values[left_start..]);
    let standardized = &mut standardized[..pass.dimension];
    for (row, &response_value) in pass.response.iter().enumerate() {
        crate::interrupt::row_checkpoint(row)?;
        let source = pass.values.row(row);
        let weight = normalized_weight(pass.weights, pass.divisor, row)?;
        let suffix = &mut standardized[left_start..];
        match (scattered, scales) {
            (None, Some(scales)) => {
                for ((out, &value), &scale) in
                    suffix.iter_mut().zip(&source[first..end]).zip(scales)
                {
                    *out = value / scale;
                }
            }
            (Some(ids), Some(scales)) => {
                for ((out, &column), &scale) in suffix.iter_mut().zip(ids).zip(scales) {
                    *out = source[column] / scale;
                }
            }
            (None, None) => suffix.copy_from_slice(&source[first..end]),
            (Some(ids), None) => {
                for (out, &column) in suffix.iter_mut().zip(ids) {
                    *out = source[column];
                }
            }
        }
        // Check the result of every division before it can enter the kernel.
        // A bitwise reduction keeps the validation free of per-column exits.
        if !suffix
            .iter()
            .fold(true, |valid, value| valid & value.is_finite())
        {
            return Err(invalid_data("finite-WLS standardized column is not finite"));
        }
        crate::kernels::upper_rank_one(
            pass.hardware,
            weight,
            standardized,
            response_value,
            left_start,
            rhs,
            gram,
        );
    }
    Ok(())
}
