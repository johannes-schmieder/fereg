//! Saved slope recovery: the retained-coefficient and MAP finishing peaks.
use super::{checked_mul, checked_sum, InterceptRecoveryMemoryEstimate};
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
) -> Result<InterceptRecoveryMemoryEstimate> {
    let invalid = || {
        FeregError::new(
            ErrorCode::InvalidArgument,
            "slope recovery memory dimensions are inconsistent",
        )
    };
    if observations == 0 || shapes.is_empty() {
        return Err(invalid());
    }
    let mut channels = 0_usize;
    let mut max_channels = 0;
    let mut directions = 0_u128;
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
    }
    if requested_channels > channels {
        return Err(invalid());
    }
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
    // MAP and retained-product normalization are sequential attempts.
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
    Ok(InterceptRecoveryMemoryEstimate {
        retained_peak_bytes: owned,
        map_peak_bytes: owned,
        // Intercept CG is inapplicable when any slope is present.
        cg_peak_bytes: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slope_recovery_includes_outputs_and_sequential_fallback() {
        let a =
            estimate_slope_recovery_memory(96, &[(4, true, 2), (3, false, 1)], 1, 4, 100).unwrap();
        let b =
            estimate_slope_recovery_memory(96, &[(4, true, 2), (3, false, 1)], 4, 4, 100).unwrap();
        assert_eq!(b.map_peak_bytes - a.map_peak_bytes, 96 * 3 * 8);
        assert_eq!(a.map_peak_bytes, a.retained_peak_bytes);
        assert_eq!(a.cg_peak_bytes, 0);
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
        assert!(estimate_slope_recovery_memory(96, &[(97, true, 1)], 1, 2, 0).is_err());
        assert!(estimate_slope_recovery_memory(96, &[(4, true, usize::MAX)], 1, 2, 0).is_err());
    }
}
