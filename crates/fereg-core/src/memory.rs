use crate::error::{ErrorCode, FeregError, Result};
use crate::options::MemoryMode;

mod capacity;
pub(crate) use capacity::{
    fallible_copy, fallible_empty, fallible_filled, matrix_bytes, push_reserved,
    require_admitted_capacity, Capacity, SystemVecReservation, VecReservation,
};
mod owned_fe;
mod slope_recovery;
mod workspace_peak;
pub(crate) use owned_fe::{
    generic_fe_solver_bytes, owned_fe_matrix_bytes, OwnedFeDesignMemoryEstimate,
};
pub(crate) use slope_recovery::estimate_slope_recovery_memory;
pub use workspace_peak::SolverWorkspacePeak;

const MINIMUM_HEADROOM_BYTES: u128 = 512 * 1024 * 1024;

/// Apply the shared native headroom policy before any construction stage.
#[must_use]
pub fn usable_memory_bytes(available_bytes: Option<u128>) -> Option<u128> {
    available_bytes
        .map(|available| available.saturating_sub(MINIMUM_HEADROOM_BYTES.max(available / 5)))
}

/// Heap payload of an owned vector, including unused capacity. The descriptor
/// belongs to its containing owner. This reads no elements and allocates nothing.
pub(crate) fn vec_capacity_bytes<T>(values: &Vec<T>) -> u128 {
    values.capacity() as u128 * std::mem::size_of::<T>() as u128
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryEstimate {
    pub fast_bytes: u128,
    pub low_bytes: u128,
    pub recommended_poolsize: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryPlan {
    pub requested: MemoryMode,
    pub selected: MemoryMode,
    pub estimate: MemoryEstimate,
    pub available_bytes: Option<u128>,
    pub safety_headroom_bytes: u128,
    pub usable_bytes: Option<u128>,
}

/// Solver workspace available after the selected execution plan is charged.
///
/// This is an internal planning seam, not the deferred public `maxmemory()`
/// option.  A future user cap can populate the same structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeSolverMemoryBudget {
    pub execution_bytes: u128,
    pub usable_bytes: u128,
}

/// CG and coefficient-bank workspace model for one FE absorption call.
/// fedemean's routes admit their own workspace within the memory limit
/// fereg passes (see `block_schur::lsmr_workspace_bytes` for fereg's estimate
/// of its modified-LSMR workspace on categorical intercept designs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeSolverMemoryEstimate {
    pub cg_bytes: u128,
    /// One compact RHS-major intercept coefficient batch.
    pub coefficient_batch_bytes: u128,
    pub rhs_copy_bytes: u128,
    pub thread_reduction_bytes: u128,
    /// Observation-scale code and membership bytes a borrowed design avoids
    /// copying, retained as a diagnostic.
    pub borrowed_code_copy_avoided_bytes: u128,
}

impl FeSolverMemoryBudget {
    #[must_use]
    pub fn fits(self, solver_workspace_bytes: u128) -> bool {
        self.execution_bytes
            .checked_add(solver_workspace_bytes)
            .is_some_and(|total| total <= self.usable_bytes)
    }
}

impl MemoryPlan {
    #[must_use]
    pub const fn selected_bytes(self) -> u128 {
        match self.selected {
            MemoryMode::Fast => self.estimate.fast_bytes,
            MemoryMode::Low => self.estimate.low_bytes,
            MemoryMode::Auto => 0,
        }
    }
}

pub fn estimate_phase2_memory(
    observations: usize,
    loaded_numeric_columns: usize,
    finite_columns: usize,
    fe_dimensions: usize,
    cluster_dimensions: usize,
    poolsize: usize,
) -> Result<MemoryEstimate> {
    if poolsize == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "poolsize must be positive",
        ));
    }
    let n = observations as u128;
    let loaded = loaded_numeric_columns as u128;
    let k = finite_columns as u128;
    let g = fe_dimensions as u128;
    let c = cluster_dimensions as u128;
    let pool = poolsize.min(finite_columns) as u128;
    // Wide fast fits can now enter guarded Cholesky. In addition to the
    // existing cross-product allowance, its peak owns four square arrays
    // (Gram copy, factor, inverse work and inverse/bread) and linear metadata.
    // This legacy fast reserve is independent of the worker count. Public low
    // admission separately includes the OLS transient QR/Cholesky frame.
    let wide_finite_solve = if finite_columns > 64 {
        checked_sum(&[
            checked_mul(
                checked_mul(k, k, "wide finite solve")?,
                32,
                "wide finite solve bytes",
            )?,
            checked_mul(k, 128, "wide finite solve metadata bytes")?,
            // Private Gram moments: one Gram, one RHS and at most one row per
            // partition, with at most one partition per Gram row.
            checked_mul(
                checked_sum(&[
                    checked_mul(
                        checked_mul(k, k, "wide private Gram")?,
                        2,
                        "wide private Gram rows",
                    )?,
                    k,
                ])?,
                8,
                "wide private Gram bytes",
            )?,
        ])?
    } else {
        0
    };
    let projection_bytes = |columns| {
        if fe_dimensions == 0 || finite_columns == 0 {
            Ok(0)
        } else {
            matrix_bytes::<f64>(
                n,
                crate::ols::retained_rhs_stride(columns)?,
                "parallel FE projection bytes",
            )
        }
    };
    // A fast design wider than the stack binding borrows its sources through
    // one heap descriptor per generated column while building the RHS batch.
    let wide_rhs_binding = if finite_columns > crate::ols::MAX_BOUND_NUMERIC_COLUMNS {
        checked_mul(
            k,
            std::mem::size_of::<Option<&[f64]>>() as u128,
            "wide RHS binding bytes",
        )?
    } else {
        0
    };
    let fast_parallel_projection = projection_bytes(k)?;
    let low_parallel_projection = projection_bytes(pool)?;

    let base = checked_sum(&[
        checked_mul(
            checked_mul(n, loaded, "loaded numeric staging")?,
            8,
            "loaded numeric staging bytes",
        )?,
        checked_mul(n, 8, "outcome")?,
        checked_mul(n, 8, "weights")?,
        checked_mul(n, 8, "working vector")?,
        checked_mul(n, 8, "observation map")?,
        checked_mul(checked_mul(n, g, "FE codes")?, 4, "FE code bytes")?,
        checked_mul(checked_mul(n, c, "cluster codes")?, 4, "cluster code bytes")?,
    ])?;
    let fast = checked_sum(&[
        base,
        if finite_columns == 0 {
            0
        } else {
            checked_mul(n, 8, "fast independent raw residual bytes")?
        },
        checked_mul(checked_mul(n, k, "resident X")?, 16, "X plus transformed X")?,
        checked_mul(
            checked_mul(k, k, "cross-product")?,
            8,
            "cross-product bytes",
        )?,
        fast_parallel_projection,
        wide_finite_solve,
        wide_rhs_binding,
    ])?;
    let low = checked_sum(&[
        base,
        checked_mul(
            checked_mul(n, pool, "pooled X")?,
            16,
            "X block plus transformed block",
        )?,
        checked_mul(
            checked_mul(k, k, "cross-product")?,
            8,
            "cross-product bytes",
        )?,
        crate::ols::ols_scratch_transpose_reserve(finite_columns),
        low_parallel_projection,
    ])?;
    Ok(MemoryEstimate {
        fast_bytes: fast,
        low_bytes: low,
        recommended_poolsize: pool as usize,
    })
}

/// Estimate the CG workspace and coefficient bank of one FE absorption call
/// after topology is known, assuming borrowed categorical codes. Arithmetic
/// is checked so a malformed or adversarial dimension fails before
/// allocation.
pub fn estimate_fe_solver_memory(
    observations: usize,
    rhs_columns: usize,
    effect_level_counts: &[usize],
    threads: usize,
) -> Result<FeSolverMemoryEstimate> {
    if observations == 0 || rhs_columns == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "FE solver memory estimate requires nonempty observation and RHS axes",
        ));
    }
    if effect_level_counts.is_empty() || effect_level_counts.contains(&0) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "FE solver memory estimate requires positive effect level counts",
        ));
    }
    if threads == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "FE solver memory estimate has an invalid thread or reorthogonalization count",
        ));
    }

    let n = observations as u128;
    let rhs = rhs_columns as u128;
    let effects = effect_level_counts.len() as u128;
    let levels = effect_level_counts
        .iter()
        .try_fold(0_u128, |total, value| {
            total.checked_add(*value as u128).ok_or_else(|| {
                FeregError::new(ErrorCode::Allocation, "FE solver level count overflows")
            })
        })?;
    let maximum_levels = effect_level_counts.iter().copied().max().unwrap_or(0) as u128;
    let mut estimate =
        estimate_cg_memory_dimensions(n, rhs, effects, levels, maximum_levels, threads)?;
    estimate.borrowed_code_copy_avoided_bytes = checked_sum(&[
        checked_mul(
            checked_mul(n, effects, "avoided FE codes")?,
            4,
            "avoided FE code bytes",
        )?,
        checked_mul(
            checked_mul(n, effects, "avoided FE memberships")?,
            std::mem::size_of::<usize>() as u128,
            "avoided FE membership bytes",
        )?,
        checked_mul(
            levels,
            std::mem::size_of::<usize>() as u128,
            "avoided FE offset bytes",
        )?,
    ])?;
    Ok(estimate)
}

/// Allocation-free CG and coefficient-bank admission for a borrowed topology.
/// This uses the same checked scalar formula as the complete solver estimator.
pub(crate) fn estimate_cg_solver_memory(
    observations: usize,
    rhs_columns: usize,
    effect_level_counts: impl IntoIterator<Item = usize>,
    threads: usize,
) -> Result<(u128, u128)> {
    if observations == 0 || rhs_columns == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "FE solver memory estimate requires nonempty observation and RHS axes",
        ));
    }
    let mut effects = 0_u128;
    let mut levels = 0_u128;
    let mut maximum_levels = 0;
    for count in effect_level_counts {
        if count == 0 {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "FE solver memory estimate requires positive effect level counts",
            ));
        }
        effects = checked_sum(&[effects, 1])?;
        levels = levels.checked_add(count as u128).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "FE solver level count overflows")
        })?;
        maximum_levels = maximum_levels.max(count);
    }
    if effects == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "FE solver memory estimate requires positive effect level counts",
        ));
    }
    if threads == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "FE solver memory estimate has an invalid thread or reorthogonalization count",
        ));
    }
    let estimate = estimate_cg_memory_dimensions(
        observations as u128,
        rhs_columns as u128,
        effects,
        levels,
        maximum_levels as u128,
        threads,
    )?;
    Ok((estimate.cg_bytes, estimate.coefficient_batch_bytes))
}

fn estimate_cg_memory_dimensions(
    n: u128,
    rhs: u128,
    effects: u128,
    levels: u128,
    maximum_levels: u128,
    threads: usize,
) -> Result<FeSolverMemoryEstimate> {
    let matrix_bytes = checked_mul(
        checked_mul(n, rhs, "FE solver observation-RHS axis")?,
        8,
        "FE solver observation-RHS bytes",
    )?;
    let rhs_copy_bytes = matrix_bytes;
    let cg_matrix_bytes = checked_mul(matrix_bytes, 3, "CG residual/direction/projection bytes")?;
    let group_workspace_bytes = checked_mul(
        checked_mul(maximum_levels, rhs, "CG group workspace")?,
        16,
        "CG group sums and corrections",
    )?;
    let dot_chunks = n.div_ceil(32_768);
    let thread_reduction_bytes = checked_mul(
        checked_mul(
            dot_chunks.max(threads as u128),
            rhs,
            "solver thread reductions",
        )?,
        8,
        "solver thread-reduction bytes",
    )?;
    let scalar_rhs_bytes = checked_mul(rhs, 8 * 16, "CG scalar RHS workspace")?;
    let coefficient_batch_bytes = checked_sum(&[
        checked_mul(
            checked_mul(levels, rhs, "FE retained coefficient axis")?,
            8,
            "FE retained coefficient bytes",
        )?,
        checked_mul(
            checked_sum(&[checked_mul(effects, 3, "FE coefficient layouts")?, 1])?,
            std::mem::size_of::<usize>() as u128,
            "FE coefficient layout bytes",
        )?,
    ])?;
    let cg_bytes = checked_sum(&[
        cg_matrix_bytes,
        group_workspace_bytes,
        thread_reduction_bytes,
        scalar_rhs_bytes,
    ])?;

    Ok(FeSolverMemoryEstimate {
        cg_bytes,
        coefficient_batch_bytes,
        rhs_copy_bytes,
        thread_reduction_bytes,
        borrowed_code_copy_avoided_bytes: 0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InterceptRecoveryMemoryEstimate {
    pub(crate) retained_peak_bytes: u128,
    pub(crate) cg_peak_bytes: u128,
    pub(crate) map_peak_bytes: u128,
}

/// Peak added by the intercept-only saved-effect recovery chain, including
/// retained-coefficient normalization.
/// The caller's prepared data, final fit and output target remain charged in
/// `FeSolverMemoryBudget::execution_bytes`. Ordinary robust/clustered inference
/// has already consumed and released the fit's score cache before recovery.
pub(crate) fn estimate_intercept_recovery_memory(
    observations: usize,
    effect_level_counts: &[usize],
    requested_channels: usize,
    threads: usize,
) -> Result<InterceptRecoveryMemoryEstimate> {
    if requested_channels > effect_level_counts.len()
        || effect_level_counts
            .iter()
            .any(|&levels| levels > observations)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "intercept recovery memory dimensions are inconsistent",
        ));
    }
    let solver = estimate_fe_solver_memory(observations, 1, effect_level_counts, threads)?;
    let n = observations as u128;
    let effects = effect_level_counts.len() as u128;
    let levels = effect_level_counts
        .iter()
        .try_fold(0_u128, |total, &count| checked_sum(&[total, count as u128]))?;
    let retained_cg = checked_sum(&[
        solver.cg_bytes,
        checked_mul(
            solver.coefficient_batch_bytes,
            5,
            "recovery CG coefficient state",
        )?,
    ])?;
    // Include target copying, residual certification, combined output and all
    // requested channels. MAP and retained-product finishing fit this reserve.
    let finishing = checked_sum(&[
        checked_mul(
            checked_mul(n, requested_channels as u128 + 4, "recovery output axes")?,
            8,
            "recovery output and certificate values",
        )?,
        // Graph recovery needs 16 bytes/level for coefficients and its queue;
        // the queue dies before normalization. This reserve also covers it.
        // Parent/rank/observed/component labels, component means/weights and
        // normalized coefficients coexist with the retained coefficient input.
        checked_mul(levels, 128, "recovery normalization and coefficient state")?,
        checked_mul(effects, 256, "recovery effect views and output registry")?,
        4096,
    ])?;
    Ok(InterceptRecoveryMemoryEstimate {
        retained_peak_bytes: finishing,
        cg_peak_bytes: checked_sum(&[retained_cg, finishing])?,
        map_peak_bytes: finishing,
    })
}

pub fn plan_ols_memory(
    requested: MemoryMode,
    estimate: MemoryEstimate,
    available_bytes: Option<u128>,
) -> Result<MemoryPlan> {
    let safety_headroom_bytes = available_bytes
        .map(|available| MINIMUM_HEADROOM_BYTES.max(available / 5))
        .unwrap_or(0);
    let usable_bytes = usable_memory_bytes(available_bytes);
    let fits = |bytes: u128| usable_bytes.is_none_or(|usable| bytes <= usable);
    let selected = match requested {
        MemoryMode::Fast if fits(estimate.fast_bytes) => MemoryMode::Fast,
        MemoryMode::Low if fits(estimate.low_bytes) => MemoryMode::Low,
        MemoryMode::Auto if fits(estimate.fast_bytes) => MemoryMode::Fast,
        MemoryMode::Auto if fits(estimate.low_bytes) => MemoryMode::Low,
        MemoryMode::Fast | MemoryMode::Low | MemoryMode::Auto => {
            let available = available_bytes.map_or_else(
                || "unavailable".to_owned(),
                |value| format!("{value} bytes"),
            );
            return Err(FeregError::new(
                ErrorCode::Allocation,
                format!(
                    "fereg memory preflight failed: fast={} bytes, low={} bytes, available={}, safety_headroom={} bytes; use memory(low), reduce poolsize(), or request more memory",
                    estimate.fast_bytes,
                    estimate.low_bytes,
                    available,
                    safety_headroom_bytes,
                ),
            ));
        }
    };
    Ok(MemoryPlan {
        requested,
        selected,
        estimate,
        available_bytes,
        safety_headroom_bytes,
        usable_bytes,
    })
}

fn checked_mul(left: u128, right: u128, label: &str) -> Result<u128> {
    left.checked_mul(right).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Allocation,
            format!("memory estimate overflow while sizing {label}"),
        )
    })
}

fn checked_sum(values: &[u128]) -> Result<u128> {
    values.iter().try_fold(0_u128, |total, value| {
        total.checked_add(*value).ok_or(FeregError::new(
            ErrorCode::Allocation,
            "memory estimate overflow",
        ))
    })
}

#[cfg(test)]
mod tests {
    const LOW_MODE_TRANSPOSE_BUFFER_BYTES: u128 = 1024 * 1024;
    use super::*;

    #[test]
    fn borrowed_cg_memory_summary_matches_full_estimate_and_checked_boundaries() {
        for counts in [vec![3, 5], vec![17], vec![1; 1_000]] {
            for (rows, rhs, threads) in [(7, 2, 1), (32_769, 5, 4), (64, 65, 2)] {
                let complete = estimate_fe_solver_memory(rows, rhs, &counts, threads).unwrap();
                assert_eq!(
                    estimate_cg_solver_memory(rows, rhs, counts.iter().copied(), threads).unwrap(),
                    (complete.cg_bytes, complete.coefficient_batch_bytes),
                );
            }
        }
        // Three 7x2 matrices, group sums/corrections, one reduction chunk,
        // sixteen scalar slots per RHS; one bank has eight levels and layouts.
        assert_eq!(
            estimate_cg_solver_memory(7, 2, [3, 5], 1).unwrap(),
            (768, 128 + 7 * std::mem::size_of::<usize>() as u128)
        );
        for (rows, rhs, counts, threads) in [
            (0, 1, vec![1], 1),
            (1, 0, vec![1], 1),
            (1, 1, vec![], 1),
            (1, 1, vec![1, 0], 1),
            (1, 1, vec![1], 0),
            (usize::MAX, usize::MAX, vec![usize::MAX], 1),
        ] {
            let expected = estimate_fe_solver_memory(rows, rhs, &counts, threads).unwrap_err();
            let actual = estimate_cg_solver_memory(rows, rhs, counts, threads).unwrap_err();
            assert_eq!(actual.code, expected.code);
            assert_eq!(actual.message, expected.message);
        }
    }

    #[test]
    fn low_mode_is_smaller_for_wide_models() {
        let estimate = estimate_phase2_memory(1_000_000, 5, 100, 2, 2, 8).expect("estimate");
        assert!(estimate.low_bytes < estimate.fast_bytes);
    }

    #[test]
    fn intercept_recovery_memory_covers_outputs_and_rejects_invalid_axes() {
        let partial = estimate_intercept_recovery_memory(96, &[4, 3], 1, 4).unwrap();
        let all = estimate_intercept_recovery_memory(96, &[4, 3], 2, 4).unwrap();
        assert_eq!(
            all.retained_peak_bytes - partial.retained_peak_bytes,
            96 * 8
        );
        assert_eq!(all.cg_peak_bytes - partial.cg_peak_bytes, 96 * 8);
        assert_eq!(all.map_peak_bytes - partial.map_peak_bytes, 96 * 8);
        let solver = estimate_fe_solver_memory(96, 1, &[4, 3], 4).unwrap();
        assert!(partial.cg_peak_bytes > solver.cg_bytes + 5 * solver.coefficient_batch_bytes);
        assert!(partial.map_peak_bytes < partial.cg_peak_bytes);
        assert!(!FeSolverMemoryBudget {
            execution_bytes: u128::MAX,
            usable_bytes: u128::MAX,
        }
        .fits(all.cg_peak_bytes));
        for (n, levels, requested, workers) in [
            (0, vec![1], 1, 1),
            (96, vec![], 0, 1),
            (96, vec![0], 1, 1),
            (96, vec![97], 1, 1),
            (96, vec![4, 3], 3, 1),
            (96, vec![4, 3], 2, 0),
        ] {
            assert_eq!(
                estimate_intercept_recovery_memory(n, &levels, requested, workers)
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidArgument,
            );
        }
    }

    #[test]
    fn wide_cholesky_workspace_is_preflighted_without_changing_low_mode() {
        for k in [65_usize, 128, 1024] {
            let estimate = estimate_phase2_memory(100, 0, k, 0, 0, 8).expect("wide estimate");
            let legacy_fast = 100 * 40 + 16 * 100 * k as u128 + 8 * (k * k) as u128;
            let incremental = 32 * (k * k) as u128
                + 128 * k as u128
                + 16 * k as u128
                + 8 * (2 * k * k + k) as u128;
            assert_eq!(estimate.fast_bytes, legacy_fast + incremental);
            assert_eq!(
                estimate.low_bytes,
                100 * 32 + 16 * 100 * 8 + 8 * (k * k) as u128 + LOW_MODE_TRANSPOSE_BUFFER_BYTES
            );
            let available = legacy_fast + MINIMUM_HEADROOM_BYTES;
            assert_eq!(
                plan_ols_memory(MemoryMode::Fast, estimate, Some(available))
                    .expect_err("wide workspace must fit before solving")
                    .code,
                ErrorCode::Allocation
            );
        }
    }

    #[test]
    fn dense_two_fe_model_includes_loaded_numeric_staging() {
        let estimate = estimate_phase2_memory(10_000_000, 5, 3, 2, 0, 10).expect("estimate");
        assert_eq!(estimate.fast_bytes, 1_680_000_072);
        assert_eq!(estimate.low_bytes, 1_601_048_648);
    }

    #[test]
    fn no_fe_model_does_not_reserve_group_projection_workspace() {
        let estimate = estimate_phase2_memory(10_000_000, 5, 3, 0, 0, 10).expect("estimate");
        assert_eq!(estimate.fast_bytes, 1_280_000_072);
    }

    #[test]
    fn zero_finite_columns_need_no_pool() {
        let estimate = estimate_phase2_memory(1_000_000, 3, 0, 2, 2, 8).expect("estimate");
        assert_eq!(estimate.recommended_poolsize, 0);
        assert_eq!(estimate.low_bytes, estimate.fast_bytes);
    }

    #[test]
    fn auto_selects_low_when_fast_exceeds_the_safe_budget() {
        let estimate = estimate_phase2_memory(1_000_000, 4, 100, 2, 0, 4).expect("estimate");
        let available = estimate
            .low_bytes
            .checked_add(MINIMUM_HEADROOM_BYTES)
            .expect("budget");
        let plan = plan_ols_memory(MemoryMode::Auto, estimate, Some(available)).expect("plan");
        assert_eq!(plan.selected, MemoryMode::Low);
        assert_eq!(plan.selected_bytes(), estimate.low_bytes);
    }

    #[test]
    fn solver_memory_model_is_positive_and_borrowed() {
        let estimate = estimate_fe_solver_memory(1_000_000, 4, &[250_000, 80_000], 8)
            .expect("solver estimate");
        assert!(estimate.cg_bytes > estimate.rhs_copy_bytes);
        assert!(estimate.rhs_copy_bytes > 0);
        assert!(estimate.thread_reduction_bytes > 0);
        assert!(estimate.borrowed_code_copy_avoided_bytes > 0);
    }

    #[test]
    fn joint_budget_selects_only_complete_solver_plans() {
        let estimate =
            estimate_fe_solver_memory(100_000, 2, &[10_000, 4_000], 4).expect("solver estimate");
        let execution_bytes = 400_000_000_u128;
        let budget = FeSolverMemoryBudget {
            execution_bytes,
            usable_bytes: execution_bytes + estimate.cg_bytes,
        };
        assert!(budget.fits(estimate.cg_bytes));
        assert!(!budget.fits(estimate.cg_bytes + 1));
        assert!(!FeSolverMemoryBudget {
            execution_bytes: u128::MAX,
            usable_bytes: u128::MAX,
        }
        .fits(1));
    }

    #[test]
    fn explicit_fast_fails_before_allocation_when_only_low_fits() {
        let estimate = estimate_phase2_memory(1_000_000, 4, 100, 2, 0, 4).expect("estimate");
        let available = estimate
            .low_bytes
            .checked_add(MINIMUM_HEADROOM_BYTES)
            .expect("budget");
        let error = plan_ols_memory(MemoryMode::Fast, estimate, Some(available))
            .expect_err("fast preflight");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert!(error.message.contains("use memory(low)"));
    }

    #[test]
    fn unavailable_system_budget_keeps_requested_or_fast_auto_mode() {
        let estimate = estimate_phase2_memory(10, 3, 3, 1, 0, 2).expect("estimate");
        assert_eq!(
            plan_ols_memory(MemoryMode::Auto, estimate, None)
                .expect("auto")
                .selected,
            MemoryMode::Fast
        );
        assert_eq!(
            plan_ols_memory(MemoryMode::Low, estimate, None)
                .expect("low")
                .selected,
            MemoryMode::Low
        );
    }
}
