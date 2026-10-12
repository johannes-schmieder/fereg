//! OLS input-array admission and reconciliation of retained preparation owners.
//! Construction scratch, solver/inference frames and dependency storage remain
//! separate scopes; this is not a complete action-memory certificate.

use fereg_core::error::{allocation, Result};
use fereg_core::MemoryEstimate;

#[derive(Debug, Clone, Copy)]
pub(crate) struct OlsPreparationMemory {
    pub(crate) input_array_bytes: u128,
    pub(crate) numeric_preparation_array_bytes: u128,
    prepared_payload_allowance: u128,
    construction_bytes: u128,
    pub(crate) frame_metadata_bytes: u128,
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
        let classification_arrays =
            add(input_array_bytes, mul(n, add(mul(value_columns, 8)?, 9)?)?)?;
        let singleton_compaction_arrays = mul(n, add(mul(value_columns, 16)?, 18)?)?;
        let numeric_preparation_array_bytes =
            classification_arrays.max(singleton_compaction_arrays);
        // This is exactly the prepared-payload portion of estimate_phase2_memory:
        // loaded outcome/sources, optional weight allowance, row map and codes.
        // Its separate outcome/working vectors and all finite/parallel reserves
        // remain intact when actual prepared ownership exceeds this allowance.
        let prepared_payload_allowance = mul(
            n,
            add(
                add(mul(loaded, 8)?, 16)?,
                mul(add(fe_channels as u128, clusters as u128)?, 4)?,
            )?,
        )?;
        Ok(Self {
            input_array_bytes,
            numeric_preparation_array_bytes,
            prepared_payload_allowance,
            construction_bytes: 0,
            frame_metadata_bytes: 0,
        })
    }

    /// Supplement the input-array floor with the construction families in
    /// prepared.rs and prepared_categorical_data.rs. Source descriptors are
    /// inspected without allocating a categorical marker on the row axis.
    pub(crate) fn with_construction(
        mut self,
        rows: usize,
        request: &fereg_core::ols::OlsRequestV1,
        clusters: &[fereg_core::ols::OlsClusterTermV1],
    ) -> Result<Self> {
        use fereg_core::metadata::ComponentKind;
        let categories = request
            .model
            .sources
            .iter()
            .filter(|source| {
                request.model.terms.iter().any(|term| {
                    term.components.iter().any(|component| {
                        component.source == source.id
                            && component.kind == ComponentKind::Categorical
                    })
                }) || request
                    .absorbed_terms
                    .iter()
                    .any(|term| term.categorical_sources.contains(&source.id))
                    || clusters
                        .iter()
                        .any(|cluster| cluster.sources.contains(&source.id))
            })
            .count() as u128;
        let effects = request.absorbed_terms.len() as u128;
        let complex = request.absorbed_terms.iter().any(|term| {
            term.categorical_sources.len() > 1
                || !term.slope_sources.is_empty()
                || !term.saved_name.is_empty()
        });
        let n = rows as u128;
        let numeric = self.numeric_preparation_array_bytes;
        // Candidate and final codes/levels, availability and final membership
        // indexes, compacted/original level identities, remaps and cursors.
        // Count all declared effects (deduplication can only reduce this).
        let topology = mul(
            n,
            add(
                mul(categories, if complex { 48 } else { 12 })?,
                mul(effects, if complex { 48 } else { 24 })?,
            )?,
        )?;
        // Hash factorization's largest growth keeps old/new bucket tables,
        // growing provisional levels and N codes simultaneously. Sorting drops
        // the dictionary first. Dense integer and tuple paths use <= 12N.
        let factorization = if categories == 0 {
            0
        } else {
            add(mul(n, 96)?, 4096)?
        };
        // Counts, XOR active-row identities, queue and queue markers: 25 bytes
        // per level. Active/initial masks and dropped-row list coexist.
        let pruning = add(
            mul(n, add(mul(effects, 25)?, 10)?)?,
            mul(add(effects, 1)?, 8)?,
        )?;
        let cluster_codes = mul(mul(n, clusters.len() as u128)?, 4)?;
        // Compiler and availability-request copies, nested recipes and BTree
        // validation registries are metadata-scale, never N-by-K expansions.
        let descriptors = add(
            request.model.sources.len() as u128,
            add(
                request.model.terms.len() as u128,
                request.bindings.len() as u128,
            )?,
        )?;
        let cluster_metadata = clusters.iter().try_fold(0, |total, cluster| {
            add(
                total,
                add(
                    cluster.label.capacity() as u128,
                    mul(cluster.sources.capacity() as u128, 4)?,
                )?,
            )
        })?;
        // Every saved/reported slope channel repeats the categorical label
        // and source identity. This is a product of channel count and tuple
        // arity/name length, not bounded by a constant times request bytes.
        let mut channel_metadata = 0;
        for effect in &request.absorbed_terms {
            let mut label = effect.categorical_sources.len().saturating_sub(1) as u128;
            for id in &effect.categorical_sources {
                let source = request
                    .model
                    .sources
                    .iter()
                    .find(|source| source.id == *id)
                    .ok_or_else(|| allocation("OLS metadata source is absent"))?;
                label = add(label, source.canonical_name.len() as u128)?;
            }
            let identity = mul(effect.categorical_sources.len() as u128, 4)?;
            if effect.intercept {
                channel_metadata = add(channel_metadata, add(add(label, identity)?, 256)?)?;
            }
            for id in &effect.slope_sources {
                let source = request
                    .model
                    .sources
                    .iter()
                    .find(|source| source.id == *id)
                    .ok_or_else(|| allocation("OLS slope metadata source is absent"))?;
                let channel = add(add(label, source.canonical_name.len() as u128)?, 3)?;
                channel_metadata = add(channel_metadata, add(add(channel, identity)?, 256)?)?;
            }
        }
        self.frame_metadata_bytes = add(
            mul(channel_metadata, 2)?,
            add(
                mul(request.owned_buffer_bytes(), 2)?,
                mul(descriptors, 128)?,
            )?,
        )?;
        let metadata = add(
            mul(add(request.owned_buffer_bytes(), cluster_metadata)?, 8)?,
            add(
                add(mul(descriptors, 256)?, mul(channel_metadata, 8)?)?,
                4096,
            )?,
        )?;
        self.construction_bytes = add(
            add(numeric, topology)?,
            add(
                factorization.max(pruning).max(mul(n, 12)?),
                add(cluster_codes, metadata)?,
            )?,
        )?;
        Ok(self)
    }

    pub(crate) fn preparation_peak_bytes(self, parent: u128) -> Result<u128> {
        add(
            parent,
            self.input_array_bytes
                .max(self.numeric_preparation_array_bytes)
                .max(self.construction_bytes),
        )
    }

    /// Optional scratch coexists only with preparation. Admit it after the
    /// required route and existing caches, without changing later admission.
    pub(crate) fn admit_membership_scratch(
        &mut self,
        requested: u128,
        usable: Option<u128>,
        parent: u128,
    ) -> Result<u128> {
        let baseline = self.preparation_peak_bytes(parent)?;
        let admitted = usable.map_or(requested, |limit| {
            requested.min(limit.saturating_sub(baseline))
        });
        self.construction_bytes = add(baseline - parent, admitted)?;
        Ok(admitted)
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
        self.admit_numeric_arrays_with_parent(usable_bytes, 0)
    }

    pub(crate) fn admit_numeric_arrays_with_parent(
        self,
        usable_bytes: Option<u128>,
        parent_bytes: u128,
    ) -> Result<()> {
        let peak = self.preparation_peak_bytes(parent_bytes)?;
        if usable_bytes.is_some_and(|usable| peak > usable) {
            return Err(allocation(
                "OLS loading and numeric preparation arrays exceed usable memory",
            ));
        }
        Ok(())
    }
}

fn add(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b)
        .ok_or_else(|| allocation("OLS preparation memory size overflows"))
}

fn mul(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b)
        .ok_or_else(|| allocation("OLS preparation memory size overflows"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fereg_core::error::ErrorCode;

    #[test]
    fn loading_masks_have_exact_and_one_byte_short_boundaries() {
        // Outcome plus 100 sources, one weight and a separate candidate mask.
        let memory = OlsPreparationMemory::estimate(10_000, 101, true, 0, 0).unwrap();
        assert_eq!(memory.input_array_bytes, 9_190_000);
        // Classification: input + second value bank + map/reasons = 17,440,000.
        assert_eq!(memory.numeric_preparation_array_bytes, 17_440_000);
        memory.admit_numeric_arrays(Some(17_440_000)).unwrap();
        assert_eq!(
            memory
                .admit_numeric_arrays(Some(17_439_999))
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
        memory.admit_numeric_arrays(None).unwrap();
        memory
            .admit_numeric_arrays_with_parent(Some(17_440_137), 137)
            .unwrap();
        assert_eq!(
            memory
                .admit_numeric_arrays_with_parent(Some(17_440_136), 137)
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
        assert_eq!(
            memory
                .admit_numeric_arrays_with_parent(None, u128::MAX)
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
        let narrow = OlsPreparationMemory::estimate(10_000, 2, false, 0, 0).unwrap();
        assert_eq!(narrow.numeric_preparation_array_bytes, 500_000);
        assert!(OlsPreparationMemory::estimate(usize::MAX, usize::MAX, true, 0, 0).is_err());
    }

    #[test]
    fn optional_membership_scratch_uses_only_remaining_preparation_headroom() {
        let original = OlsPreparationMemory::estimate(100, 3, false, 2, 1).unwrap();
        let peak = original.preparation_peak_bytes(137).unwrap();
        let base = fereg_core::estimate_phase2_memory(100, 3, 8, 2, 1, 3).unwrap();
        for (usable, expected) in [(Some(peak), 0), (Some(peak + 47), 47), (None, 800)] {
            let mut memory = original;
            assert_eq!(
                memory.admit_membership_scratch(800, usable, 137).unwrap(),
                expected
            );
            memory
                .admit_numeric_arrays_with_parent(usable, 137)
                .unwrap();
            assert_eq!(
                memory.with_prepared_owner(base, 6_201).unwrap(),
                original.with_prepared_owner(base, 6_201).unwrap()
            );
        }
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
        assert_eq!(
            larger.fast_bytes - base.fast_bytes,
            larger.low_bytes - base.low_bytes
        );
        assert_eq!(
            memory
                .with_prepared_owner(base, u128::MAX)
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
    }
}
