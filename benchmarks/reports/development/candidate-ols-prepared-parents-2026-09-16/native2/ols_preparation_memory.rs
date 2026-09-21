//! OLS input-array admission and reconciliation of retained preparation owners.
//! Construction scratch, solver/inference frames and dependency storage remain
//! separate scopes; this is not a complete action-memory certificate.

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::MemoryEstimate;

#[derive(Debug, Clone, Copy)]
pub(crate) struct OlsPreparationMemory {
    pub(crate) input_array_bytes: u128,
    pub(crate) numeric_preparation_array_bytes: u128,
    prepared_payload_allowance: u128,
}

impl OlsPreparationMemory {
    pub(crate) fn estimate(
        observations: usize,
        loaded_columns: usize,
        weighted: bool,
        fe_channels: usize,
        clusters: usize,
    ) -> Result<Self> {
        let n = observations as u128;
        let loaded = loaded_columns as u128;
        // Every SPI value column has a bool missing mask; candidate flags are
        // separate. These arrays are allocated before authoritative compaction.
        let input_array_bytes = mul(n, add(mul(add(loaded, u128::from(weighted))?, 9)?, 1)?)?;
        let value_columns = add(loaded, u128::from(weighted))?;
        // Classification retains the input masks plus row map/drop reasons.
        // Restricted owned compaction can replace columns in parallel, bounded
        // here by a full second value bank. Singleton compaction retains two
        // value/map banks, one original-axis reason mask and the keep mask.
        // These stages are sequential; their array peaks are not summed.
        let classification_arrays = add(input_array_bytes, mul(n, add(mul(value_columns, 8)?, 9)?)?)?;
        let singleton_compaction_arrays = mul(n, add(mul(value_columns, 16)?, 18)?)?;
        let numeric_preparation_array_bytes = classification_arrays.max(singleton_compaction_arrays);
        // This is exactly the prepared-payload portion of estimate_phase2_memory:
        // loaded outcome/sources, optional weight allowance, row map and codes.
        // Its separate outcome/working vectors and all finite/parallel reserves
        // remain intact when actual prepared ownership exceeds this allowance.
        let prepared_payload_allowance = mul(n, add(
            add(mul(loaded, 8)?, 16)?,
            mul(add(fe_channels as u128, clusters as u128)?, 4)?,
        )?)?;
        Ok(Self { input_array_bytes, numeric_preparation_array_bytes, prepared_payload_allowance })
    }

    pub(crate) fn with_prepared_owner(
        self,
        base: MemoryEstimate,
        owned_bytes: u128,
    ) -> Result<MemoryEstimate> {
        let extra = owned_bytes.saturating_sub(self.prepared_payload_allowance);
        Ok(MemoryEstimate {
            fast_bytes: add(base.fast_bytes, extra)?,
            low_bytes: add(base.low_bytes, extra)?,
            recommended_poolsize: base.recommended_poolsize,
        })
    }

    pub(crate) fn admit_numeric_arrays(self, usable_bytes: Option<u128>) -> Result<()> {
        let peak = self.input_array_bytes.max(self.numeric_preparation_array_bytes);
        if usable_bytes.is_some_and(|usable| peak > usable) {
            return Err(allocation("OLS loading and numeric preparation arrays exceed usable memory"));
        }
        Ok(())
    }
}

fn add(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b).ok_or_else(|| allocation("OLS preparation memory size overflows"))
}

fn mul(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b).ok_or_else(|| allocation("OLS preparation memory size overflows"))
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_masks_have_exact_and_one_byte_short_boundaries() {
        // Outcome plus 100 sources, one weight and a separate candidate mask.
        let memory = OlsPreparationMemory::estimate(10_000, 101, true, 0, 0).unwrap();
        assert_eq!(memory.input_array_bytes, 9_190_000);
        // Classification: input + second value bank + map/reasons = 17,440,000.
        assert_eq!(memory.numeric_preparation_array_bytes, 17_440_000);
        memory.admit_numeric_arrays(Some(17_440_000)).unwrap();
        assert_eq!(memory.admit_numeric_arrays(Some(17_439_999)).unwrap_err().code, ErrorCode::Allocation);
        memory.admit_numeric_arrays(None).unwrap();
        let narrow = OlsPreparationMemory::estimate(10_000, 2, false, 0, 0).unwrap();
        assert_eq!(narrow.numeric_preparation_array_bytes, 500_000);
        assert!(OlsPreparationMemory::estimate(usize::MAX, usize::MAX, true, 0, 0).is_err());
    }

    #[test]
    fn retained_parent_replacement_preserves_all_other_reserves() {
        let memory = OlsPreparationMemory::estimate(100, 3, false, 2, 1).unwrap();
        // Loaded 2400 + weights/map 1600 + codes 1200 = 5200.
        let base = fereg_core::estimate_phase2_memory(100, 3, 8, 2, 1, 3).unwrap();
        assert_eq!(memory.with_prepared_owner(base, 5_200).unwrap(), base);
        assert_eq!(memory.with_prepared_owner(base, 4_000).unwrap(), base);
        let larger = memory.with_prepared_owner(base, 6_201).unwrap();
        assert_eq!(larger.fast_bytes, base.fast_bytes + 1_001);
        assert_eq!(larger.low_bytes, base.low_bytes + 1_001);
        assert_eq!(larger.fast_bytes-base.fast_bytes, larger.low_bytes-base.low_bytes);
        assert_eq!(memory.with_prepared_owner(base, u128::MAX).unwrap_err().code, ErrorCode::Allocation);
    }
}
