use crate::error::{ErrorCode, FeregError, Result};
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
    ])?;
    Ok(MemoryEstimate {
        fast_bytes: fast,
        low_bytes: low,
        recommended_poolsize: pool as usize,
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
