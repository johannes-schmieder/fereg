//! Heap-backed private moments for Gram dimensions wider than the stack block.
//!
//! The partition of Gram rows across workers is the layout's, and each
//! partition runs the dense rank-1 pass of the stack path: rows in order, one
//! weighted `mul_add` per entry from a zeroed private accumulator. Every entry
//! therefore keeps the bits of the in-place wide kernels; only the private bank
//! (one Gram, one RHS and one row per partition, charged by the fast memory
//! estimate) is new. A failed reservation keeps the in-place kernels.
use super::{
    accumulate_standardized_rows, moment_lane_work, record_retained_moment_worker, DenseRowsPass,
    FiniteWlsMomentLayout, FIXED_BLOCK_MOMENT_MAX_COLUMNS,
};
use crate::error::Result;
use rayon::prelude::*;

/// Private elements of one partition: its Gram rows, RHS rows and one row.
fn stride(dimension: usize, layout: FiniteWlsMomentLayout) -> Option<usize> {
    let rows = layout.columns_per_partition;
    rows.checked_mul(dimension)?
        .checked_add(rows)?
        .checked_add(dimension)
}

/// Reserve the zeroed private bank for a wide dimension, or `None` for a
/// narrow dimension or a failed reservation.
pub(super) fn reserve(dimension: usize, layout: FiniteWlsMomentLayout) -> Option<Vec<f64>> {
    if dimension <= FIXED_BLOCK_MOMENT_MAX_COLUMNS || layout.columns_per_partition == 0 {
        return None;
    }
    let elements = layout.partitions.checked_mul(stride(dimension, layout)?)?;
    let mut bank = Vec::new();
    bank.try_reserve_exact(elements).ok()?;
    bank.resize(elements, 0.0);
    Some(bank)
}

/// Fill wide dense moments through the private bank from [`reserve`].
pub(super) fn fill_dense_rows(
    pass: DenseRowsPass<'_>,
    layout: FiniteWlsMomentLayout,
    bank: &mut [f64],
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<()> {
    let dimension = pass.dimension;
    let stride = stride(dimension, layout).unwrap_or(usize::MAX);
    crate::parallel::stage_try_for_each(
        rhs.par_chunks_mut(layout.columns_per_partition)
            .zip(gram.par_chunks_mut(layout.columns_per_partition * dimension))
            .zip(bank.par_chunks_mut(stride))
            .enumerate(),
        moment_lane_work(pass.response.len(), dimension),
        |(partition, ((rhs_rows, gram_rows), private))| -> Result<()> {
            record_retained_moment_worker();
            let left_start = partition * layout.columns_per_partition;
            let (local_gram, rest) = private.split_at_mut(gram_rows.len());
            let (local_rhs, rest) = rest.split_at_mut(rhs_rows.len());
            accumulate_standardized_rows(
                pass,
                left_start,
                local_rhs,
                local_gram,
                &mut rest[..dimension],
            )?;
            rhs_rows.copy_from_slice(local_rhs);
            gram_rows.copy_from_slice(local_gram);
            Ok(())
        },
    )
}
