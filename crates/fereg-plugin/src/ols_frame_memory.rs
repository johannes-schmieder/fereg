//! OLS-owned transient numerical frames, excluding FE solver/dependency storage.
//!
//! The prepared owner and request are added by the caller. Sequential finite,
//! score-certificate and covariance stages share a maximum, while buffers that
//! survive into each stage are summed. Public base-memory fields are unchanged.

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::MemoryEstimate;

#[derive(Debug, Clone, Copy)]
pub(crate) struct OlsFrameMemory {
    pub fast_bytes: u128,
    pub low_bytes: u128,
    /// Prepaid full projected owner plus worst-case score identities/means.
    /// This is a capacity ceiling, never a credit against the execution frame.
    pub retained_score_capacity_limit: u128,
    #[cfg(test)]
    pub cluster_reduction_bytes: u128,
}

impl OlsFrameMemory {
    pub(crate) fn estimate(
        rows: usize,
        generated: usize,
        reported: usize,
        clusters: usize,
        poolsize: usize,
        robust: bool,
    ) -> Result<Self> {
        if poolsize == 0 || clusters > 10 {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "invalid OLS frame dimensions",
            ));
        }
        let n = rows as u128;
        let k = generated as u128;
        let p = reported as u128;
        let q = k.max(p);
        let square = mul(q, q)?;
        let blocks = rows.div_ceil(16_384).clamp(1, 64) as u128;
        // Immutable within/RHS and the optional packed score cache; raw and
        // independent residuals, normalized weights, and certificate vectors.
        let fast_rows = mul(mul(n, add(mul(k, 2)?, 8)?)?, 8)?;
        // The existing frame retains the full N*(K+1) projection alongside a
        // packed NK score bank and seven other row vectors. Keeping that same
        // full projection through inference cannot exceed these simultaneous
        // banks. K identity/mean pairs fit the existing q*1024 metadata reserve.
        let retained_score_capacity_limit = add(
            mul(mul(n, add(k, 1)?)?, 8)?,
            mul(k, (std::mem::size_of::<usize>() + 8) as u128)?,
        )?;
        let low_rows = add(
            mul(mul(n, add(poolsize.min(generated) as u128, 8)?)?, 8)?,
            // Conservative block I/O and score-row cache reserve; a single
            // projected file survives until selected-column compaction.
            // Private file payload is reported separately, never heap-charged.
            if generated == 0 { 0 } else { 4 * (1 << 20) },
        )?;
        // Shared Gram, factor, QR fallback, inverse/bread and independent
        // checks. The factor trial and QR reuse the same numerical banks.
        let finite = add(add(mul(square, 128)?, mul(q, 1024)?)?, 4096)?;
        // The historical moment allowance bounds shared physical moments
        // and independent norm/mean partials; public admission stays unchanged.
        let moment_k = k.min(64);
        let moments = mul(
            mul(blocks, add(mul(moment_k, moment_k)?, mul(add(k, 1)?, 8)?)?)?,
            8,
        )?;
        let fit = add(finite, moments)?;
        let partial_stride = add(mul(square, 2)?, add(mul(q, 4)?, 1)?)?;
        let partial_bytes = mul(partial_stride, 8)?;
        // The reducers permit one block even if one block exceeds 32 MiB.
        let reduction_blocks = blocks.min(((32 << 20) / partial_bytes.max(1)).max(1));
        let ordinary_partials = mul(reduction_blocks, partial_bytes)?;
        let cluster_reduction_bytes = if clusters == 0 {
            0
        } else {
            // Dense subset slots are bounded by the product of level counts,
            // each <= N, and by the actual reducer's admission ceiling. Sorted
            // fallback keeps two radix orders, not one N-vector per subset.
            let ceiling = mul(n, 32)?.max(64 << 20);
            let bytes_per_group = add(mul(p, 16)?, 1)?;
            let mut dense = bytes_per_group;
            for _ in 0..clusters {
                dense = dense.saturating_mul(n).min(ceiling);
            }
            let dense_blocks = ((dense / bytes_per_group.max(1)).div_ceil(1024))
                .clamp(1, 64)
                .min(((32 << 20) / partial_bytes.max(1)).max(1));
            add(
                dense.max(mul(n, 16)?),
                mul(dense_blocks.max(reduction_blocks), partial_bytes)?,
            )?
        };
        let inference = if robust || clusters != 0 {
            let reducers = cluster_reduction_bytes.max(ordinary_partials);
            // Covariance meat/corrections, independent influence matrices,
            // sandwich, eigensystem/clipping/certificates and Wald work.
            let covariance = add(mul(square, 192)?, mul(q, 1024)?)?;
            let subsets = if clusters == 0 {
                0
            } else {
                (1_u128 << clusters) - 1
            };
            add(add(covariance, reducers)?, add(mul(subsets, 128)?, 4096)?)?
        } else {
            finite
        };
        let sequential = fit.max(inference);
        Ok(Self {
            fast_bytes: add(fast_rows, sequential)?,
            low_bytes: add(low_rows, sequential)?,
            retained_score_capacity_limit,
            #[cfg(test)]
            cluster_reduction_bytes,
        })
    }

    pub(crate) fn with_parent(self, base: MemoryEstimate, parent: u128) -> Result<MemoryEstimate> {
        Ok(MemoryEstimate {
            fast_bytes: base.fast_bytes.max(
                crate::action_memory::Frame {
                    parents: parent,
                    workspace: self.fast_bytes,
                }
                .peak()?,
            ),
            low_bytes: base.low_bytes.max(
                crate::action_memory::Frame {
                    parents: parent,
                    workspace: self.low_bytes,
                }
                .peak()?,
            ),
            recommended_poolsize: base.recommended_poolsize,
        })
    }
}

fn add(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b).ok_or_else(overflow)
}
fn mul(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b).ok_or_else(overflow)
}
fn overflow() -> FeregError {
    FeregError::new(ErrorCode::Allocation, "OLS transient frame size overflows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_cluster_and_parallel_partials_coexist() {
        let cluster = OlsFrameMemory::estimate(16_384, 2, 3, 2, 10, true).unwrap();
        assert!(cluster.cluster_reduction_bytes > 64 << 20);
        let unclustered = OlsFrameMemory::estimate(16_384, 2, 3, 0, 10, true).unwrap();
        assert!(cluster.fast_bytes > unclustered.fast_bytes + (64 << 20));
        // A single tiny dimension cannot consume the generic 64 MiB ceiling.
        assert!(
            OlsFrameMemory::estimate(12, 2, 3, 1, 10, true)
                .unwrap()
                .cluster_reduction_bytes
                < 16_384
        );
    }

    #[test]
    fn finite_wide_rank_zero_and_low_frames_are_checked() {
        for (n, k) in [(0, 0), (12, 0), (7680, 2), (65537, 65), (1000, 1000)] {
            let m = OlsFrameMemory::estimate(n, k, k + 1, 0, 10, false).unwrap();
            assert_eq!(
                m.retained_score_capacity_limit,
                n as u128 * (k as u128 + 1) * 8 + k as u128 * 16
            );
            assert!(m.retained_score_capacity_limit <= m.fast_bytes);
            let base = MemoryEstimate {
                fast_bytes: 1,
                low_bytes: 2,
                recommended_poolsize: 10,
            };
            let execution = m.with_parent(base, 1234).unwrap();
            assert_eq!(execution.fast_bytes, m.fast_bytes + 1234);
            assert_eq!(execution.low_bytes, m.low_bytes + 1234);
            assert!(m.with_parent(base, u128::MAX).is_err());
        }
        assert!(
            OlsFrameMemory::estimate(usize::MAX, usize::MAX, usize::MAX, 10, 10, true).is_err()
        );
    }
}
