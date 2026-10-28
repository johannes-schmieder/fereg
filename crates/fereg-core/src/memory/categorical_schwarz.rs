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
/// `yielding` is an optional exact count of yielding factor-pair components
/// (see [`estimate_schwarz_memory`]); `None` keeps the level-count bound.
pub(crate) fn estimate_categorical_schwarz_memory(
    rows: u128,
    effects: u128,
    levels: u128,
    maximum_levels: u128,
    threads: u128,
    dense_cross_tab_bytes: u128,
    yielding: Option<u128>,
) -> Result<CategoricalSchwarzMemoryEstimate> {
    estimate_schwarz_memory(
        rows,
        effects,
        levels,
        maximum_levels,
        threads,
        dense_cross_tab_bytes,
        false,
        yielding,
    )
}

/// Levels in one FE block of a factor-pair component at which its local solve
/// may yield into nested Rayon work. The pinned within `block_elim/solver.rs`
/// and `csr_block.rs` parallel paths require more than 10,000 rows in a block;
/// counting blocks of at least 10,000 levels keeps one level of margin.
pub(crate) const SCHWARZ_YIELDING_BLOCK_LEVELS: usize = 10_000;

/// `yielding` is an optional exact count of factor-pair components with a
/// block of at least [`SCHWARZ_YIELDING_BLOCK_LEVELS`] levels. Without it,
/// `pair_level_incidence / 10_002` bounds them from level counts alone; that
/// bound grows with the levels and every charged bank is level-sized, so the
/// charge is quadratic in the levels. Within's subdomains are the bipartite
/// components of each pair's positive-weight cross-tab, the structural pair
/// components, so the exact count bounds the same yielding subdomains. The
/// smaller of both is used.
#[allow(clippy::too_many_arguments)]
pub(super) fn estimate_schwarz_memory(
    rows: u128,
    effects: u128,
    levels: u128,
    maximum_levels: u128,
    threads: u128,
    dense_cross_tab_bytes: u128,
    signed_cover: bool,
    yielding: Option<u128>,
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
    // Each component executes once per apply on one worker. Across all workers,
    // every stack entry beyond the first therefore consumes a distinct yielding
    // component. Summed worker high-water depths are bounded by T + R, not
    // T * (1 + R), even when their maxima occur at different times. At the end
    // of each apply all stacks return to one shared pool; prior idle buffers are
    // consumed before allocating, so repeated applies cannot grow this bound.
    // Each concurrent outer RHS owns a separate apply and is charged by its
    // caller. The lifetime argument and pinned source paths are recorded in
    // the decision record kept in Git history (ADR 0228).
    let reentrant_components = (pair_level_incidence / 10_002).min(yielding.unwrap_or(u128::MAX));
    let worker_buffers = checked_sum(&[threads, reentrant_components])?;
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
    let stable_index_bytes = stable_adjoint_index_bytes(rows, effects, levels)?;
    let solver_frame_bytes = checked_sum(&[
        stable_index_bytes,
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
        checked_mul(maximum_levels, 8, "LSMR stable index construction cursor")?,
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

/// Reproducible indices are shared across RHS; sorted terms need less storage.
fn stable_adjoint_index_bytes(rows: u128, effects: u128, levels: u128) -> Result<u128> {
    // Reproducible adjoints share one stable level index across every live RHS.
    // Charge every term as unsorted and every channel as a term, conservatively.
    // Keeping this reserve in the shared frame covers retained Off/Diagonal too.
    checked_sum(&[
        checked_mul(
            checked_mul(rows, effects, "LSMR stable row indices")?,
            8,
            "LSMR stable index bytes",
        )?,
        checked_mul(
            checked_sum(&[levels, effects])?,
            8,
            "LSMR stable level offsets",
        )?,
        checked_mul(effects, 64, "LSMR stable term descriptors")?,
    ])
}

fn checked_add_memory(left: u128, right: u128, context: &'static str) -> Result<u128> {
    left.checked_add(right)
        .ok_or_else(|| FeregError::new(ErrorCode::Allocation, context))
}

fn bytes_for_memory<T>(elements: u128, context: &'static str) -> Result<u128> {
    checked_mul(elements, std::mem::size_of::<T>() as u128, context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_schwarz_memory_does_not_multiply_yielding_components_by_workers() {
        // Three independent sparse partitions, with many levels in the first.
        // Every apply still charges all possible yielding frames, all worker
        // accumulators and four retained pooled applies. A 32 GiB solver budget
        // must not reject the same complete checkpoint at a larger team size.
        for threads in [1, 4, 8, 16, 28] {
            let model = crate::memory::estimate_fe_solver_memory(
                983_040,
                11,
                &[196_608, 9_831, 492],
                threads,
                Some(4),
            )
            .unwrap();
            assert!(model.lsmr_schwarz_bytes < 32 * 1024_u128.pow(3));
            let frame = estimate_categorical_schwarz_memory(
                983_040,
                3,
                206_931,
                196_608,
                threads as u128,
                0,
                None,
            )
            .unwrap();
            let active_worker_payload = threads as u128 * (206_931 + 6 * 196_608 + 4) * 8;
            assert!(frame.apply_peak_bytes > active_worker_payload);
            assert!(frame.retained_bytes > 4 * frame.apply_peak_bytes);
        }
    }

    #[test]
    fn exact_yielding_count_removes_the_quadratic_campaign_bound() {
        // The beta.8 campaign's sparse three-FE OLS cells: 17 RHS, four
        // workers, 2 million workers, 100,000 firms and a 32-level effect at
        // ten million rows. The level-count bound R = 419 charged about a
        // terabyte, which refused LSMR; exactly three pair components there
        // have a block of at least 10,000 levels.
        let estimate = |rows, levels: [usize; 3], yielding| {
            crate::memory::estimate_fe_solver_memory_with_yielding(
                rows, 17, &levels, 4, None, yielding,
            )
            .unwrap()
            .lsmr_schwarz_bytes
        };
        let gib = 1024_u128.pow(3);
        let small = [600_000, 30_000, 32];
        let large = [2_000_000, 100_000, 32];
        assert!(estimate(10_000_000, large, None) > 900 * gib);
        assert!(estimate(10_000_000, large, Some(3)) < 32 * gib);
        // With an exact count the bound grows linearly between 3M and 10M rows.
        let ratio = estimate(10_000_000, large, Some(3)) as f64
            / estimate(3_000_000, small, Some(3)) as f64;
        assert!(ratio < 10.0 / 3.0 * 1.05, "{ratio}");
        // No count is the unchanged estimate; a count is never above the bound.
        let unchanged =
            crate::memory::estimate_fe_solver_memory(3_000_000, 17, &small, 4, None).unwrap();
        assert_eq!(
            crate::memory::estimate_fe_solver_memory_with_yielding(
                3_000_000, 17, &small, 4, None, None
            )
            .unwrap(),
            unchanged
        );
        assert_eq!(
            estimate(3_000_000, small, Some(usize::MAX)),
            unchanged.lsmr_schwarz_bytes
        );
        assert!(estimate(3_000_000, small, Some(0)) < estimate(3_000_000, small, Some(3)));
    }

    #[test]
    fn schwarz_stack_bound_keeps_nonreentrant_and_overflow_boundaries() {
        // Below the first yielding component the ordinary T-worker allowance
        // is unchanged. Crossing the threshold must add scratch even at T=1.
        let below =
            estimate_categorical_schwarz_memory(20_000, 2, 10_001, 6_000, 1, 0, None).unwrap();
        let above =
            estimate_categorical_schwarz_memory(20_000, 2, 10_002, 6_000, 1, 0, None).unwrap();
        assert!(above.apply_peak_bytes > 2 * below.apply_peak_bytes - 256);
        assert!(estimate_categorical_schwarz_memory(1, 2, 10_002, 1, u128::MAX, 0, None).is_err());
    }
}
