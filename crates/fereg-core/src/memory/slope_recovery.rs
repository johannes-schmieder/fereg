//! Saved slope recovery uses the same pinned LSMR storage model as absorption.
use super::{
    checked_mul, checked_sum, estimate_fe_solver_memory_for_channels,
    owned_fe::lsmr_slope_effect_bytes, InterceptRecoveryMemoryEstimate,
};
use crate::error::{ErrorCode, FeregError, Result};

/// Each shape is (levels, intercept present, slope channels). The caller owns
/// the prepared topology, input target and retained fit throughout recovery.
/// Only small effect/channel descriptors are allocated by this planner.
pub(crate) fn estimate_slope_recovery_memory(
    observations: usize,
    shapes: &[(usize, bool, usize)],
    requested_channels: usize,
    source_count: usize,
    validation_bytes: u128,
    threads: usize,
    local_size: Option<usize>,
) -> Result<InterceptRecoveryMemoryEstimate> {
    let invalid = || {
        FeregError::new(
            ErrorCode::InvalidArgument,
            "slope recovery memory dimensions are inconsistent",
        )
    };
    if observations == 0 || shapes.is_empty() || threads == 0 {
        return Err(invalid());
    }
    let mut channels = 0_usize;
    let mut max_channels = 0;
    let mut directions = 0_u128;
    let mut slope_bytes = 0;
    for &(levels, intercept, slopes) in shapes {
        let width = slopes
            .checked_add(usize::from(intercept))
            .ok_or_else(invalid)?;
        if levels == 0 || levels > observations || width == 0 {
            return Err(invalid());
        }
        channels = channels.checked_add(width).ok_or_else(invalid)?;
        max_channels = max_channels.max(width);
        directions = checked_sum(&[
            directions,
            checked_mul(
                levels as u128,
                width as u128,
                "recovery coefficient directions",
            )?,
        ])?;
        if slopes > 0 {
            slope_bytes = checked_sum(&[
                slope_bytes,
                lsmr_slope_effect_bytes(observations, 1, levels, slopes, width)?,
            ])?;
        }
    }
    if requested_channels > channels {
        return Err(invalid());
    }
    let mut counts = Vec::new();
    counts.try_reserve_exact(channels).map_err(|_| {
        FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve slope recovery channel counts",
        )
    })?;
    for &(levels, intercept, slopes) in shapes {
        counts.extend(std::iter::repeat_n(levels, slopes + usize::from(intercept)));
    }
    let solver = estimate_fe_solver_memory_for_channels(
        observations,
        1,
        &counts,
        threads,
        local_size,
        true,
    )?;
    let metadata = checked_sum(&[
        checked_mul(
            channels as u128,
            256,
            "slope recovery effect and channel descriptors",
        )?,
        checked_mul(source_count as u128, 128, "slope recovery source views")?,
        4096,
    ])?;
    // Finishing retains coefficients, normalized coefficients, component
    // labels/means, residual, combined output and requested channel vectors.
    // MAP and retained-product normalization are sequential attempts; neither
    // needs to reserve an unused LSMR hierarchy.
    let finishing = checked_sum(&[
        checked_mul(
            observations as u128,
            checked_mul(
                requested_channels as u128 + 4,
                8,
                "slope recovery output width",
            )?,
            "slope recovery outputs",
        )?,
        checked_mul(
            directions,
            128,
            "slope recovery normalization and coefficient state",
        )?,
        crate::fe::slope_map_workspace_bytes(max_channels, 1)?,
        metadata,
    ])?;
    let owned = finishing.max(checked_sum(&[validation_bytes, metadata])?);
    let lsmr = checked_sum(&[
        solver
            .lsmr_schwarz_bytes
            .max(solver.lsmr_diagonal_bytes)
            .max(solver.lsmr_off_bytes),
        slope_bytes,
        owned,
        // The LSMR residual certificate workspace remains in its caller frame
        // while finishing creates its own slope certificate workspace.
        crate::fe::slope_map_workspace_bytes(max_channels, 1)?,
    ])?;
    Ok(InterceptRecoveryMemoryEstimate {
        retained_peak_bytes: owned,
        map_peak_bytes: owned,
        lsmr_peak_bytes: lsmr,
        // Intercept CG/SD are inapplicable when any slope is present.
        cg_peak_bytes: 0,
        sd_lsmr_peak_bytes: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slope_recovery_includes_outputs_whitening_and_sequential_fallback() {
        let a = estimate_slope_recovery_memory(
            96,
            &[(4, true, 2), (3, false, 1)],
            1,
            4,
            100,
            4,
            Some(4),
        )
        .unwrap();
        let b = estimate_slope_recovery_memory(
            96,
            &[(4, true, 2), (3, false, 1)],
            4,
            4,
            100,
            4,
            Some(4),
        )
        .unwrap();
        assert_eq!(b.map_peak_bytes - a.map_peak_bytes, 96 * 3 * 8);
        assert_eq!(b.lsmr_peak_bytes - a.lsmr_peak_bytes, 96 * 3 * 8);
        assert_eq!(a.map_peak_bytes, a.retained_peak_bytes);
        assert!(a.lsmr_peak_bytes > a.map_peak_bytes);
        assert_eq!((a.cg_peak_bytes, a.sd_lsmr_peak_bytes), (0, 0));
        let exact = super::super::FeSolverMemoryBudget {
            execution_bytes: 37,
            usable_bytes: 37 + a.map_peak_bytes,
        };
        assert!(exact.fits(a.map_peak_bytes));
        assert!(!super::super::FeSolverMemoryBudget {
            usable_bytes: exact.usable_bytes - 1,
            ..exact
        }
        .fits(a.map_peak_bytes));
        assert!(!exact.fits(a.lsmr_peak_bytes));
        assert!(estimate_slope_recovery_memory(96, &[(97, true, 1)], 1, 2, 0, 4, None).is_err());
        assert!(
            estimate_slope_recovery_memory(96, &[(4, true, usize::MAX)], 1, 2, 0, 4, None).is_err()
        );
    }
}
