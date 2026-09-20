use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::LsmrPreconditioner;
use crate::options::MemoryMode;

mod categorical_schwarz;
mod owned_fe;
mod slope_recovery;
mod workspace_peak;
pub(crate) use slope_recovery::estimate_slope_recovery_memory;
pub(crate) use categorical_schwarz::{
    categorical_dense_cross_tab_bytes, estimate_categorical_schwarz_memory,
    CategoricalSchwarzMemoryEstimate,
};
pub(crate) use owned_fe::{
    generic_fe_solver_bytes, owned_fe_matrix_bytes, OwnedFeDesignMemoryEstimate,
};
pub use workspace_peak::SolverWorkspacePeak;

const MINIMUM_HEADROOM_BYTES: u128 = 512 * 1024 * 1024;
const LOW_MODE_TRANSPOSE_BUFFER_BYTES: u128 = 1024 * 1024;

/// Apply the shared native headroom policy before any construction stage.
#[must_use]
pub fn usable_memory_bytes(available_bytes: Option<u128>) -> Option<u128> {
    available_bytes.map(|available| {
        available.saturating_sub(MINIMUM_HEADROOM_BYTES.max(available / 5))
    })
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

/// Route-specific peak workspace model for one FE absorption call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeSolverMemoryEstimate {
    pub cg_bytes: u128,
    /// One compact RHS-major intercept coefficient batch.
    pub coefficient_batch_bytes: u128,
    pub lsmr_off_bytes: u128,
    pub lsmr_diagonal_bytes: u128,
    pub lsmr_schwarz_bytes: u128,
    /// Owned operator and bounded lazy pools retained between RHS batches.
    pub(crate) lsmr_off_retained_bytes: u128,
    pub(crate) lsmr_diagonal_retained_bytes: u128,
    pub(crate) lsmr_schwarz_retained_bytes: u128,
    pub(crate) lsmr_result_metadata_bytes: u128,
    pub rhs_copy_bytes: u128,
    pub krylov_bytes: u128,
    pub topology_bytes: u128,
    pub thread_reduction_bytes: u128,
    /// Observation-scale code and membership bytes avoided by the borrowed
    /// direct-LSMR route, retained as a diagnostic for the buffer experiment.
    pub borrowed_code_copy_avoided_bytes: u128,
}

impl FeSolverMemoryEstimate {
    pub(crate) const fn lsmr_retained_bytes(self, preconditioner: LsmrPreconditioner) -> u128 {
        match preconditioner {
            LsmrPreconditioner::Off => self.lsmr_off_retained_bytes,
            LsmrPreconditioner::Diagonal => self.lsmr_diagonal_retained_bytes,
            LsmrPreconditioner::Schwarz | LsmrPreconditioner::Auto => {
                self.lsmr_schwarz_retained_bytes
            }
        }
    }

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
    // Wide fast fits can now enter guarded Cholesky. In addition to the
    // existing cross-product allowance, its peak owns four square arrays
    // (Gram copy, factor, inverse work and inverse/bread) and linear metadata.
    // This reserve is independent of the worker count; low mode still uses QR.
    let wide_finite_solve = if finite_columns > 64 {
        checked_sum(&[
            checked_mul(
                checked_mul(k, k, "wide finite solve")?,
                32,
                "wide finite solve bytes",
            )?,
            checked_mul(k, 128, "wide finite solve metadata bytes")?,
        ])?
    } else {
        0
    };
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

/// Cap live upstream RHS frames only when a full returned bank amortizes.
/// Small batches retain the existing transport lifetime: early full-bank
/// reservation can increase their peak even though it helps wide batches.
pub(crate) fn lsmr_rhs_window(rhs: usize, threads: usize) -> usize {
    let threads = threads.max(1);
    if rhs <= 64 || rhs <= threads.saturating_mul(4) {
        rhs.max(1)
    } else {
        threads
    }
}

/// Estimate the complete incremental solver workspace after topology is known.
///
/// The model deliberately separates CG, LSMR recurrence, Schwarz topology,
/// RHS copies, and thread reductions.  It assumes prepared categorical codes
/// remain borrowed at the adapter; upstream locality copies are charged.
/// Arithmetic is checked so a malformed or
/// adversarial dimension fails before allocation.
pub fn estimate_fe_solver_memory(
    observations: usize,
    rhs_columns: usize,
    effect_level_counts: &[usize],
    threads: usize,
    local_reorthogonalization_size: Option<usize>,
) -> Result<FeSolverMemoryEstimate> {
    estimate_fe_solver_memory_for_channels(
        observations,
        rhs_columns,
        effect_level_counts,
        threads,
        local_reorthogonalization_size,
        false,
    )
}

fn estimate_fe_solver_memory_for_channels(
    observations: usize,
    rhs_columns: usize,
    effect_level_counts: &[usize],
    threads: usize,
    local_reorthogonalization_size: Option<usize>,
    signed_cover: bool,
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

    // Wide adapters explicitly window calls to the installed team's width.
    // Small batches remain unwindowed and must charge their full RHS count:
    // nested Rayon can suspend outer solves beyond the number of workers.
    let active_rhs = lsmr_rhs_window(rhs_columns, threads) as u128;
    let active_matrix_bytes = checked_mul(n, active_rhs, "LSMR window observation values")?
        .checked_mul(8)
        .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "LSMR window bytes overflow"))?;
    let history = (local_reorthogonalization_size.unwrap_or(0) as u128)
        .min(n)
        .min(levels);
    let recurrence = |coefficient_vectors| -> Result<u128> {
        checked_mul(
            checked_mul(
                checked_sum(&[
                    checked_mul(n, 2, "LSMR observation recurrence")?,
                    checked_mul(levels, coefficient_vectors, "LSMR coefficient recurrence")?,
                ])?,
                active_rhs,
                "LSMR concurrent RHS recurrences",
            )?,
            8,
            "LSMR recurrence bytes",
        )
    };
    let krylov_bytes = recurrence(checked_sum(&[
        6,
        checked_mul(history, 2, "modified LSMR history lanes")?,
    ])?)?;
    let off_krylov_bytes = recurrence(checked_sum(&[5, history])?)?;

    // Each RHS operator owns its largest scatter block. Small blocks also
    // allocate fold/reduce accumulators; with_min_len bounds the number of
    // leaves independently of the nested task schedule. Charge both fold and
    // reduction identities for every leaf, plus the final result.
    let fold_levels = if observations > 10_000 {
        effect_level_counts
            .iter()
            .copied()
            .filter(|&count| count < 100_000)
            .max()
            .unwrap_or(0) as u128
    } else {
        0
    };
    let min_rows = (observations / threads).max(1024);
    let fold_leaves = observations.div_ceil(min_rows) as u128;
    let scatter_bytes = checked_mul(
        checked_mul(
            checked_sum(&[
                maximum_levels,
                checked_mul(
                    fold_levels,
                    checked_sum(&[checked_mul(fold_leaves, 2, "LSMR scatter leaves")?, 1])?,
                    "LSMR scatter fold and reduction storage",
                )?,
            ])?,
            active_rhs,
            "LSMR per-RHS scatter storage",
        )?,
        8,
        "LSMR scatter bytes",
    )?;
    // Both locality-permuted input and weighted RHS can be owned per solve.
    // During assembly the individual results coexist with combined banks;
    // these are a different peak from the recurrence, not additive to it.
    let rhs_transform_bytes =
        checked_mul(active_matrix_bytes, 2, "LSMR permuted and weighted RHS")?;
    let retained_output_bytes = if active_rhs < rhs {
        checked_sum(&[matrix_bytes, coefficient_batch_bytes])?
    } else {
        0
    };
    let active_coefficient_bytes = checked_mul(
        checked_mul(levels, active_rhs, "LSMR window coefficients")?,
        8,
        "LSMR window coefficient bytes",
    )?;
    let batch_output_bytes = checked_mul(
        checked_sum(&[active_matrix_bytes, active_coefficient_bytes])?,
        2,
        "LSMR window individual and combined output banks",
    )?;
    let metadata_bytes = checked_sum(&[
        checked_mul(rhs, 256, "LSMR RHS descriptors and result metadata")?,
        checked_mul(effects, 128, "LSMR effect and layout metadata")?,
    ])?;
    let common = |recurrence_bytes| -> Result<u128> {
        checked_sum(&[
            rhs_copy_bytes,
            retained_output_bytes,
            checked_sum(&[recurrence_bytes, rhs_transform_bytes, scatter_bytes])?
                .max(batch_output_bytes),
            thread_reduction_bytes,
            metadata_bytes,
        ])
    };
    let schwarz = categorical_schwarz::estimate_schwarz_memory(
        n,
        effects,
        levels,
        maximum_levels,
        threads as u128,
        checked_sum(&[
            categorical_dense_cross_tab_bytes(n, effect_level_counts)?,
            schwarz_weighted_correction_bytes(n, effect_level_counts)?,
        ])?,
        signed_cover,
    )?;
    let diagonal_bytes = checked_mul(levels, 16, "LSMR diagonal build and retained inverse")?;
    let lsmr_off_retained_bytes = checked_sum(&[
        schwarz.solver_frame_bytes,
        checked_mul(effects, 128, "retained LSMR effect and layout metadata")?,
    ])?;
    let lsmr_diagonal_retained_bytes = checked_sum(&[lsmr_off_retained_bytes, diagonal_bytes])?;
    let lsmr_schwarz_retained_bytes =
        checked_sum(&[lsmr_off_retained_bytes, schwarz.retained_bytes])?;
    // A retained operator survives conversion to row-major results and the
    // fresh MAP certificate. Krylov work and these stages are sequential.
    // Charge the coefficient bank even when the caller will discard it: the
    // upstream batch returns it before conversion can release it.
    let postprocess = |retained| -> Result<u128> {
        let conversion = checked_sum(&[
            checked_mul(matrix_bytes, 2, "LSMR result conversion banks")?,
            coefficient_batch_bytes,
        ])?;
        let certification = checked_sum(&[
            matrix_bytes,
            coefficient_batch_bytes,
            group_workspace_bytes,
            scalar_rhs_bytes,
            thread_reduction_bytes,
        ])?;
        checked_sum(&[retained, metadata_bytes, conversion.max(certification)])
    };
    let lsmr_off_bytes = checked_sum(&[common(off_krylov_bytes)?, schwarz.solver_frame_bytes])?
        .max(postprocess(lsmr_off_retained_bytes)?);
    let lsmr_diagonal_bytes = checked_sum(&[
        common(krylov_bytes)?,
        schwarz.solver_frame_bytes,
        diagonal_bytes,
    ])?
    .max(postprocess(lsmr_diagonal_retained_bytes)?);
    let topology_bytes = schwarz.build_peak_bytes;
    let schwarz_setup_peak = checked_sum(&[rhs_copy_bytes, topology_bytes, metadata_bytes])?;
    // The shared retained estimate charges up to four complete pooled objects.
    // Each live RHS can additionally own one complete apply stack.
    let schwarz_batch_apply_bytes = checked_mul(
        schwarz.apply_peak_bytes,
        active_rhs,
        "LSMR concurrent Schwarz applies",
    )?;
    let schwarz_solve_peak = checked_sum(&[
        common(krylov_bytes)?,
        schwarz.solver_frame_bytes,
        schwarz.retained_bytes,
        schwarz_batch_apply_bytes,
    ])?;
    let lsmr_schwarz_bytes = schwarz_setup_peak
        .max(schwarz_solve_peak)
        .max(postprocess(lsmr_schwarz_retained_bytes)?);
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
        lsmr_off_retained_bytes,
        lsmr_diagonal_retained_bytes,
        lsmr_schwarz_retained_bytes,
        lsmr_result_metadata_bytes: metadata_bytes,
        rhs_copy_bytes,
        krylov_bytes,
        topology_bytes,
        thread_reduction_bytes,
        borrowed_code_copy_avoided_bytes,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InterceptRecoveryMemoryEstimate {
    pub(crate) retained_peak_bytes: u128,
    pub(crate) cg_peak_bytes: u128,
    pub(crate) sd_lsmr_peak_bytes: u128,
    pub(crate) lsmr_peak_bytes: u128,
    pub(crate) map_peak_bytes: u128,
}

/// Peak added by the intercept-only saved-effect recovery chain, including
/// retained-coefficient normalization and the SD pilot's overlap with LSMR.
/// The caller's prepared data, final fit and output target remain charged in
/// `FeSolverMemoryBudget::execution_bytes`. Ordinary robust/clustered inference
/// has already consumed and released the fit's score cache before recovery.
pub(crate) fn estimate_intercept_recovery_memory(
    observations: usize,
    effect_level_counts: &[usize],
    requested_channels: usize,
    threads: usize,
    local_reorthogonalization_size: Option<usize>,
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
    let solver = estimate_fe_solver_memory(
        observations,
        1,
        effect_level_counts,
        threads,
        local_reorthogonalization_size,
    )?;
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
    // SD retains its original residual, direction, group workspace and two
    // coefficient products while the remainder-LSMR call runs. Charge these
    // even if an earlier SD certificate makes the LSMR attempt unnecessary.
    let sd_frame = checked_sum(&[
        checked_mul(n, 16, "recovery SD observation frame")?,
        checked_mul(levels, 32, "recovery SD groups and coefficient frame")?,
    ])?;
    let lsmr = solver
        .lsmr_off_bytes
        .max(solver.lsmr_diagonal_bytes)
        .max(solver.lsmr_schwarz_bytes);
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
        sd_lsmr_peak_bytes: checked_sum(&[
            sd_frame,
            if effects == 1 { 0 } else { lsmr },
            finishing,
        ])?,
        lsmr_peak_bytes: checked_sum(&[lsmr, finishing])?,
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

/// Transient compensated sums for all concurrently constructed intercept pairs.
/// Weights are not yet known here, so structural-unit and slope routes are
/// conservatively charged too. Sparse construction reuses column corrections.
pub(crate) fn schwarz_weighted_correction_bytes(
    observations: u128,
    level_counts: &[usize],
) -> Result<u128> {
    let sparse_cost = checked_mul(observations, 12, "Schwarz sparse scratch")?;
    let mut total = 0;
    for (i, &left) in level_counts.iter().enumerate() {
        for &right in &level_counts[i + 1..] {
            let cells = checked_mul(left as u128, right as u128, "Schwarz cross-tab cells")?;
            let dense_cost = checked_mul(cells, 8, "Schwarz cell corrections")?;
            let levels = checked_sum(&[left as u128, right as u128])?;
            let diagonals = checked_mul(levels, 8, "Schwarz diagonal corrections")?;
            total = checked_sum(&[
                total,
                diagonals,
                if cells <= 5_000_000 || dense_cost <= sparse_cost {
                    dense_cost
                } else {
                    0
                },
            ])?;
        }
    }
    Ok(total)
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
    fn schwarz_corrections_charge_dense_sparse_and_concurrent_pairs() {
        assert_eq!(
            schwarz_weighted_correction_bytes(8_000, &[2_500, 2_000]).unwrap(),
            40_036_000
        );
        assert_eq!(
            schwarz_weighted_correction_bytes(8_000, &[2_501, 2_000]).unwrap(),
            36_008
        );
        assert_eq!(
            schwarz_weighted_correction_bytes(4_000_000, &[3_000, 2_000]).unwrap(),
            48_040_000
        );
        assert_eq!(
            schwarz_weighted_correction_bytes(3_999_999, &[3_000, 2_000]).unwrap(),
            40_000
        );
        let levels = [2_666, 2_000, 1_600];
        let expected = (2_666_u128 * 1_600 + 2_000 * 1_600 + 2 * (2_666 + 2_000 + 1_600)) * 8;
        assert_eq!(
            schwarz_weighted_correction_bytes(8_000, &levels).unwrap(),
            expected
        );
        assert_eq!(
            schwarz_weighted_correction_bytes(8_000, &[1_600, 2_666, 2_000]).unwrap(),
            expected
        );
        assert_eq!(schwarz_weighted_correction_bytes(8_000, &[20]).unwrap(), 0);
        assert!(schwarz_weighted_correction_bytes(u128::MAX, &[2, 3]).is_err());
    }

    #[test]
    fn low_mode_is_smaller_for_wide_models() {
        let estimate = estimate_phase2_memory(1_000_000, 5, 100, 2, 2, 8).expect("estimate");
        assert!(estimate.low_bytes < estimate.fast_bytes);
    }

    #[test]
    fn intercept_recovery_memory_covers_outputs_and_rejects_invalid_axes() {
        let partial = estimate_intercept_recovery_memory(96, &[4, 3], 1, 4, Some(4)).unwrap();
        let all = estimate_intercept_recovery_memory(96, &[4, 3], 2, 4, Some(4)).unwrap();
        assert_eq!(
            all.retained_peak_bytes - partial.retained_peak_bytes,
            96 * 8
        );
        assert_eq!(all.cg_peak_bytes - partial.cg_peak_bytes, 96 * 8);
        assert_eq!(all.sd_lsmr_peak_bytes - partial.sd_lsmr_peak_bytes, 96 * 8);
        assert_eq!(all.lsmr_peak_bytes - partial.lsmr_peak_bytes, 96 * 8);
        assert_eq!(all.map_peak_bytes - partial.map_peak_bytes, 96 * 8);
        let solver = estimate_fe_solver_memory(96, 1, &[4, 3], 4, Some(4)).unwrap();
        assert!(partial.cg_peak_bytes > solver.cg_bytes + 5 * solver.coefficient_batch_bytes);
        assert!(partial.lsmr_peak_bytes > solver.lsmr_schwarz_bytes);
        assert!(partial.sd_lsmr_peak_bytes > partial.lsmr_peak_bytes);
        assert!(partial.map_peak_bytes < partial.cg_peak_bytes);
        let direct = estimate_intercept_recovery_memory(96, &[4], 1, 4, Some(4)).unwrap();
        assert!(direct.sd_lsmr_peak_bytes < direct.lsmr_peak_bytes);
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
                estimate_intercept_recovery_memory(n, &levels, requested, workers, Some(4))
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
            let incremental = 32 * (k * k) as u128 + 128 * k as u128;
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
    fn lsmr_recurrence_model_matches_independent_allocator_payloads() {
        // Pinned modified-LSMR payloads independently captured by the rectangular
        // operator/barrier probe in lsmr-transport-local-2026-09-12. They exclude
        // transport, scatter, preconditioners and result-container bookkeeping.
        for (rows, levels, rhs, history, payload) in [
            (8192, 4096, 1, None, 327_680),
            (8192, 4096, 1, Some(4), 589_824),
            (8192, 4096, 4, Some(4), 2_359_296),
            (16384, 4096, 4, Some(4), 2_883_584),
            (8192, 6144, 4, Some(4), 3_276_800),
            (32, 8, 4, Some(100), 7_680),
        ] {
            let estimate = estimate_fe_solver_memory(rows, rhs, &[levels], 4, history).unwrap();
            assert_eq!(estimate.krylov_bytes, payload);
            assert!(estimate.lsmr_diagonal_bytes > payload);
        }
    }

    #[test]
    fn ols_schwarz_admission_includes_dense_tables_before_solving() {
        let estimate = estimate_fe_solver_memory(320, 1, &[160, 161], 1, Some(4)).unwrap();
        // The old model omitted this primary table. The weighted builder
        // additionally owns an equally sized compensation table.
        let table_bytes = 160_u128 * 161 * 8;
        assert!(estimate.topology_bytes >= 2 * table_bytes);
        assert!(estimate.lsmr_schwarz_bytes >= estimate.topology_bytes);
        assert!(estimate.cg_bytes < 327_680);
        assert!(estimate.lsmr_schwarz_bytes > 327_680);
    }

    #[test]
    fn lsmr_batch_model_bounds_window_frames_and_retained_outputs() {
        let one = estimate_fe_solver_memory(8192, 1, &[2048, 2048], 1, Some(4)).unwrap();
        let four = estimate_fe_solver_memory(8192, 4, &[2048, 2048], 1, Some(4)).unwrap();
        let wide = estimate_fe_solver_memory(8192, 65, &[2048, 2048], 1, Some(4)).unwrap();
        assert_eq!(four.krylov_bytes, 4 * one.krylov_bytes);
        assert_eq!(wide.krylov_bytes, one.krylov_bytes);
        assert!(wide.lsmr_off_bytes > one.lsmr_off_bytes);
        assert!(wide.lsmr_schwarz_bytes > one.lsmr_schwarz_bytes);
        for threads in [1, 4, 16] {
            for rhs in [16, 17, 19, 32, 63, 64] {
                assert_eq!(lsmr_rhs_window(rhs, threads), rhs);
            }
            assert_eq!(lsmr_rhs_window(65, threads), threads);
        }
        assert_eq!(lsmr_rhs_window(63, 16), 63);
        assert_eq!(lsmr_rhs_window(64, 16), 64);
        assert_eq!(lsmr_rhs_window(65, 16), 16);
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
