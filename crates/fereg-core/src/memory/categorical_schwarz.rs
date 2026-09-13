//! Shared pinned-within categorical Schwarz construction and retained state.
use super::{checked_mul, checked_sum};
use crate::error::{ErrorCode, FeregError, Result};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CategoricalSchwarzMemoryEstimate {
    pub(crate) retained_bytes: u128,
    pub(crate) build_peak_bytes: u128,
    pub(crate) apply_peak_bytes: u128,
    pub(crate) solver_frame_bytes: u128,
}

pub(crate) fn categorical_dense_cross_tab_bytes(
    rows: u128,
    level_counts: &[usize],
) -> Result<u128> {
    // The pinned within cross_tab/accumulate.rs chooses dense whenever
    // L_i * L_j <= 5,000,000 OR its 8-byte table costs no more than 12N.
    // Pair construction is parallel; charge all simultaneously live tables.
    // The existing edge bound separately covers CSR output and sparse scratch.
    let sparse_bytes = checked_mul(rows, 12, "categorical LSMR sparse cross-tab scratch")?;
    let mut dense_bytes = 0;
    for (index, &left) in level_counts.iter().enumerate() {
        for &right in &level_counts[index + 1..] {
            let cells = checked_mul(
                left as u128,
                right as u128,
                "categorical LSMR cross-tab cells",
            )?;
            let bytes = bytes_for_memory::<f64>(cells, "categorical LSMR dense cross-tab scratch")?;
            if cells <= 5_000_000 || bytes <= sparse_bytes {
                dense_bytes =
                    checked_add_memory(dense_bytes, bytes, "categorical LSMR dense pair peak")?;
            }
        }
    }
    Ok(dense_bytes)
}

/// Estimate the owned `within::Solver` categorical/Schwarz state used by
/// both categorical LSMR routes. Every factor-pair can contain N distinct edges; the
/// accounting charges both CSR orientations, construction copies, per-level
/// maps/factors, locality-sort copies, and concurrent Schwarz apply buffers.
pub(crate) fn estimate_categorical_schwarz_memory(
    rows: u128,
    effects: u128,
    levels: u128,
    maximum_levels: u128,
    threads: u128,
    dense_cross_tab_bytes: u128,
) -> Result<CategoricalSchwarzMemoryEstimate> {
    estimate_schwarz_memory(
        rows,
        effects,
        levels,
        maximum_levels,
        threads,
        dense_cross_tab_bytes,
        false,
    )
}

pub(super) fn estimate_schwarz_memory(
    rows: u128,
    effects: u128,
    levels: u128,
    maximum_levels: u128,
    threads: u128,
    dense_cross_tab_bytes: u128,
    signed_cover: bool,
) -> Result<CategoricalSchwarzMemoryEstimate> {
    let pairs = checked_mul(
        effects,
        effects.saturating_sub(1),
        "categorical LSMR factor-pair count",
    )? / 2;
    let edges = checked_mul(rows, pairs, "categorical LSMR factor-pair edges")?;
    let pair_level_incidence = checked_mul(
        levels,
        effects.saturating_sub(1),
        "categorical LSMR pair-level incidence",
    )?;

    // Constants include payload slack for opaque upstream vector and factor
    // records, not an empirically selected routing threshold.
    // A signed reduced Schur factor may use the pinned Gremban cover: two
    // vertices/edges per original. Charge that expansion in construction and
    // retained factors, even when a particular channel pair needs no cover.
    let factor_copies = if signed_cover { 2 } else { 1 };
    let factor_edges = checked_mul(edges, factor_copies, "Schwarz cover edges")?;
    let factor_levels = checked_mul(pair_level_incidence, factor_copies, "Schwarz cover levels")?;
    let retained_edge_bytes =
        checked_mul(factor_edges, 80, "categorical LSMR retained edge bytes")?;
    let retained_level_bytes = checked_mul(
        factor_levels,
        80,
        "categorical LSMR retained pair-level bytes",
    )?;
    let diagonal_bytes = bytes_for_memory::<f64>(levels, "categorical LSMR retained diagonal")?;

    // A categorical component owns both FE blocks and the reduced factor's
    // augmented dimension in each of its two local scratch vectors. A signed
    // cover additionally needs up to 2*maximum_levels entries in each vector.
    let local_scratch_values = checked_sum(&[
        checked_mul(
            maximum_levels,
            if signed_cover { 10 } else { 6 },
            "categorical Schwarz local scratch",
        )?,
        4,
    ])?;
    let global_apply_values = checked_sum(&[levels, local_scratch_values])?;

    // WorkerBufferStack retains each worker's maximum re-entry depth. Only
    // components with an FE block above 10,000 levels can yield inside the
    // pinned local solver; each consumes at least 10,002 pair-level incidences.
    // Charge every such suspended frame on every worker, plus its active frame.
    // The outer RHS loop is bounded separately by its full batch width.
    let reentrant_components = pair_level_incidence / 10_002;
    let worker_buffers = checked_mul(
        threads,
        checked_sum(&[1, reentrant_components])?,
        "categorical Schwarz worker stack depth",
    )?;
    let apply_peak_bytes = checked_sum(&[
        bytes_for_memory::<f64>(
            checked_mul(
                global_apply_values,
                worker_buffers,
                "categorical Schwarz apply values",
            )?,
            "categorical Schwarz apply bytes",
        )?,
        // Vec capacity growth, thread-local stacks and descriptors carry no
        // additional numeric payload, but remain live until the round ends.
        checked_mul(
            worker_buffers,
            512,
            "categorical Schwarz buffer descriptors",
        )?,
        checked_mul(threads, 256, "categorical Schwarz worker descriptors")?,
    ])?;
    // MAX_POOL_SIZE bounds top-level objects; each can retain its entire stack.
    let retained_pool_bytes =
        checked_mul(apply_peak_bytes, 4, "categorical Schwarz retained pools")?;
    let retained_bytes = checked_sum(&[
        retained_edge_bytes,
        retained_level_bytes,
        diagonal_bytes,
        retained_pool_bytes,
    ])?;

    let categorical_copy_bytes = checked_sum(&[
        bytes_for_memory::<u32>(
            checked_mul(rows, effects, "categorical LSMR sorted categorical cells")?,
            "categorical LSMR sorted categorical bytes",
        )?,
        bytes_for_memory::<u32>(rows, "categorical LSMR observation permutation")?,
    ])?;
    let solver_frame_bytes = checked_sum(&[
        categorical_copy_bytes,
        bytes_for_memory::<f64>(rows, "categorical LSMR square-root weights")?,
        bytes_for_memory::<usize>(
            checked_add_memory(
                pair_level_incidence,
                effects,
                "categorical LSMR categorical metadata count",
            )?,
            "categorical LSMR categorical metadata bytes",
        )?,
    ])?;

    // Construction may simultaneously hold caller-order and sorted weights,
    // stable-sort scratch, active maps, both transient cross tabs, the newly
    // built retained factor, and the solver frame that takes ownership.
    let sort_scratch_bytes = checked_sum(&[
        checked_mul(rows, 16, "categorical LSMR stable-sort scratch")?,
        bytes_for_memory::<usize>(
            checked_add_memory(maximum_levels, 1, "categorical LSMR sort cursor count")?,
            "categorical LSMR sort cursor bytes",
        )?,
    ])?;
    let build_edge_bytes = checked_mul(factor_edges, 64, "categorical LSMR transient edge bytes")?;
    let build_level_bytes = checked_mul(
        factor_levels,
        64,
        "categorical LSMR transient pair-level bytes",
    )?;
    let build_peak_bytes = checked_sum(&[
        solver_frame_bytes,
        bytes_for_memory::<f64>(rows, "categorical LSMR source weight copy")?,
        sort_scratch_bytes,
        build_edge_bytes,
        build_level_bytes,
        dense_cross_tab_bytes,
        retained_bytes,
    ])?;
    Ok(CategoricalSchwarzMemoryEstimate {
        retained_bytes,
        build_peak_bytes,
        apply_peak_bytes,
        solver_frame_bytes,
    })
}

fn checked_add_memory(left: u128, right: u128, context: &'static str) -> Result<u128> {
    left.checked_add(right)
        .ok_or_else(|| FeregError::new(ErrorCode::Allocation, context))
}

fn bytes_for_memory<T>(elements: u128, context: &'static str) -> Result<u128> {
    checked_mul(elements, std::mem::size_of::<T>() as u128, context)
}
