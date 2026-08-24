use crate::error::{ErrorCode, FeregError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryEstimate {
    pub fast_bytes: u128,
    pub low_bytes: u128,
    pub recommended_poolsize: usize,
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
    let k = finite_columns as u128;
    let g = fe_dimensions as u128;
    let c = cluster_dimensions as u128;
    let pool = poolsize.min(finite_columns) as u128;

    let base = checked_sum(&[
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
    ])?;
    Ok(MemoryEstimate {
        fast_bytes: fast,
        low_bytes: low,
        recommended_poolsize: pool as usize,
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
        let estimate = estimate_phase2_memory(1_000_000, 100, 2, 2, 8).expect("estimate");
        assert!(estimate.low_bytes < estimate.fast_bytes);
    }

    #[test]
    fn zero_finite_columns_need_no_pool() {
        let estimate = estimate_phase2_memory(1_000_000, 0, 2, 2, 8).expect("estimate");
        assert_eq!(estimate.recommended_poolsize, 0);
        assert_eq!(estimate.low_bytes, estimate.fast_bytes);
    }
}
