use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::LsmrPreconditioner;
use crate::options::MemoryMode;

const MINIMUM_HEADROOM_BYTES: u128 = 512 * 1024 * 1024;
const LOW_MODE_TRANSPOSE_BUFFER_BYTES: u128 = 1024 * 1024;

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

/// Route-specific peak workspace model for one FE absorption call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeSolverMemoryEstimate {
    pub cg_bytes: u128,
    /// One compact RHS-major intercept coefficient batch.
    pub coefficient_batch_bytes: u128,
    pub lsmr_off_bytes: u128,
    pub lsmr_diagonal_bytes: u128,
    pub lsmr_schwarz_bytes: u128,
    pub rhs_copy_bytes: u128,
    pub krylov_bytes: u128,
    pub topology_bytes: u128,
    pub thread_reduction_bytes: u128,
    /// Observation-scale code and membership bytes avoided by the borrowed
    /// direct-LSMR route, retained as a diagnostic for the buffer experiment.
    pub borrowed_code_copy_avoided_bytes: u128,
}

impl FeSolverMemoryEstimate {
    #[must_use]
    pub const fn lsmr_bytes(self, preconditioner: LsmrPreconditioner) -> u128 {
        match preconditioner {
            LsmrPreconditioner::Off => self.lsmr_off_bytes,
            LsmrPreconditioner::Diagonal => self.lsmr_diagonal_bytes,
            LsmrPreconditioner::Schwarz | LsmrPreconditioner::Auto => self.lsmr_schwarz_bytes,
        }
    }
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

impl MemoryEstimate {
    #[must_use]
    pub fn fast_gib(self) -> f64 {
        self.fast_bytes as f64 / 1_073_741_824.0
    }

    #[must_use]
    pub fn low_gib(self) -> f64 {
        self.low_bytes as f64 / 1_073_741_824.0
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
    let fast_parallel_projection = if fe_dimensions == 0 || finite_columns == 0 {
        0
    } else {
        checked_mul(
            checked_mul(n, checked_sum(&[k, 1])?, "fast parallel FE projection")?,
            8,
            "fast parallel FE projection bytes",
        )?
    };
    let low_parallel_projection = if fe_dimensions == 0 || finite_columns == 0 {
        0
    } else {
        checked_mul(
            checked_mul(n, checked_sum(&[pool, 1])?, "low parallel FE projection")?,
            8,
            "low parallel FE projection bytes",
        )?
    };

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
        checked_mul(checked_mul(n, k, "resident X")?, 16, "X plus transformed X")?,
        checked_mul(
            checked_mul(k, k, "cross-product")?,
            8,
            "cross-product bytes",
        )?,
        fast_parallel_projection,
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
        if finite_columns == 0 {
            0
        } else {
            LOW_MODE_TRANSPOSE_BUFFER_BYTES
        },
        low_parallel_projection,
    ])?;
    Ok(MemoryEstimate {
        fast_bytes: fast,
        low_bytes: low,
        recommended_poolsize: pool as usize,
    })
}

/// Estimate the complete incremental solver workspace after topology is known.
///
/// The model deliberately separates CG, LSMR recurrence, Schwarz topology,
/// RHS copies, and thread reductions.  It assumes prepared categorical codes
/// and memberships remain borrowed.  Arithmetic is checked so a malformed or
/// adversarial dimension fails before allocation.
pub fn estimate_fe_solver_memory(
    observations: usize,
    rhs_columns: usize,
    effect_level_counts: &[usize],
    threads: usize,
    local_reorthogonalization_size: Option<usize>,
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
    if threads == 0 || local_reorthogonalization_size == Some(0) {
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
    let dot_chunks = observations.div_ceil(32_768) as u128;
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
    let coefficient_batch_bytes = checked_mul(
        checked_mul(levels, rhs, "FE retained coefficient axis")?,
        8,
        "FE retained coefficient bytes",
    )?;
    let cg_bytes = checked_sum(&[
        cg_matrix_bytes,
        group_workspace_bytes,
        thread_reduction_bytes,
        scalar_rhs_bytes,
    ])?;

    // Modified LSMR processes batch columns with shared observation-space
    // recurrence storage. The execution plan already owns the returned dense
    // candidate, so the incremental peak charges one RHS materialization and
    // one shared recurrence/history block rather than K copies of both.
    let recurrence_vectors = local_reorthogonalization_size
        .unwrap_or(0)
        .checked_add(4)
        .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "LSMR history count overflows"))?
        as u128;
    let krylov_bytes = checked_mul(
        checked_mul(n, recurrence_vectors, "LSMR shared Krylov axis")?,
        8,
        "LSMR shared Krylov bytes",
    )?;
    let lsmr_common = checked_sum(&[
        rhs_copy_bytes,
        krylov_bytes,
        thread_reduction_bytes,
        checked_mul(levels, 16, "LSMR coefficient workspace")?,
    ])?;
    let diagonal_bytes = checked_mul(levels, 16, "LSMR diagonal preconditioner")?;
    let topology_bytes = checked_sum(&[
        checked_mul(n, effects, "Schwarz observation-effect topology")?,
        checked_mul(levels, 48, "Schwarz level topology")?,
        checked_mul(n, 24, "Schwarz cross-tab edge workspace")?,
    ])?;
    let lsmr_off_bytes = lsmr_common;
    let lsmr_diagonal_bytes = checked_sum(&[lsmr_common, diagonal_bytes])?;
    // Schwarz cross-tabs are constructed during setup and released or reused
    // before the Krylov solve reaches its peak. Model the larger stage plus
    // the small persistent level factor, not the impossible sum of both peaks.
    let schwarz_setup_peak = checked_sum(&[rhs_copy_bytes, topology_bytes, diagonal_bytes])?;
    let schwarz_solve_peak = checked_sum(&[lsmr_common, diagonal_bytes])?;
    let lsmr_schwarz_bytes = schwarz_setup_peak
        .max(schwarz_solve_peak)
        .checked_add(checked_mul(levels, 8, "persistent Schwarz level factor")?)
        .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "Schwarz peak estimate overflows"))?;
    let borrowed_code_copy_avoided_bytes = checked_sum(&[
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
    Ok(FeSolverMemoryEstimate {
        cg_bytes,
        coefficient_batch_bytes,
        lsmr_off_bytes,
        lsmr_diagonal_bytes,
        lsmr_schwarz_bytes,
        rhs_copy_bytes,
        krylov_bytes,
        topology_bytes,
        thread_reduction_bytes,
        borrowed_code_copy_avoided_bytes,
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
    let usable_bytes =
        available_bytes.map(|available| available.saturating_sub(safety_headroom_bytes));
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
    use super::*;

    #[test]
    fn low_mode_is_smaller_for_wide_models() {
        let estimate = estimate_phase2_memory(1_000_000, 5, 100, 2, 2, 8).expect("estimate");
        assert!(estimate.low_bytes < estimate.fast_bytes);
    }

    #[test]
    fn dense_two_fe_model_includes_loaded_numeric_staging() {
        let estimate = estimate_phase2_memory(10_000_000, 5, 3, 2, 0, 10).expect("estimate");
        assert_eq!(estimate.fast_bytes, 1_600_000_072);
        assert_eq!(estimate.low_bytes, 1_601_048_648);
    }

    #[test]
    fn no_fe_model_does_not_reserve_group_projection_workspace() {
        let estimate = estimate_phase2_memory(10_000_000, 5, 3, 0, 0, 10).expect("estimate");
        assert_eq!(estimate.fast_bytes, 1_200_000_072);
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
    fn solver_memory_model_is_route_specific_monotone_and_borrowed() {
        let estimate = estimate_fe_solver_memory(1_000_000, 4, &[250_000, 80_000], 8, Some(4))
            .expect("solver estimate");
        assert!(estimate.cg_bytes > 0);
        assert!(estimate.lsmr_off_bytes < estimate.lsmr_diagonal_bytes);
        assert!(estimate.lsmr_diagonal_bytes < estimate.lsmr_schwarz_bytes);
        assert!(estimate.rhs_copy_bytes > 0);
        assert!(estimate.krylov_bytes > estimate.rhs_copy_bytes);
        assert!(estimate.topology_bytes > 0);
        assert!(estimate.thread_reduction_bytes > 0);
        assert!(estimate.borrowed_code_copy_avoided_bytes > 0);
    }

    #[test]
    fn joint_budget_selects_only_complete_solver_plans() {
        let estimate = estimate_fe_solver_memory(100_000, 2, &[10_000, 4_000], 4, Some(4))
            .expect("solver estimate");
        let execution_bytes = 400_000_000_u128;
        let budget = FeSolverMemoryBudget {
            execution_bytes,
            usable_bytes: execution_bytes + estimate.cg_bytes,
        };
        assert!(budget.fits(estimate.cg_bytes));
        assert!(!budget.fits(estimate.lsmr_schwarz_bytes));
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
