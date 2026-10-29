//! Incremental storage for generic owned intercept/slope absorption.
use super::{checked_mul, checked_sum, estimate_fe_solver_memory_for_channels};
use crate::error::{allocation, ErrorCode, FeregError, Result};
use crate::fe::{EffectTerm, FeDesign, FeSolverKind, LsmrConfig};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct OwnedFeDesignMemoryEstimate {
    pub(crate) resident_bytes: u128,
    pub(crate) construction_bytes: u128,
}

impl OwnedFeDesignMemoryEstimate {
    /// Pre-allocation envelope when only aggregate topology dimensions exist.
    /// All resident effects coexist; only one effect's count/cursor scratch
    /// exists at a time. Descriptor and offset sentinels are heap payloads too.
    pub(crate) fn from_summary(
        rows: usize,
        effects: usize,
        total_levels: usize,
        maximum_levels: usize,
        slope_channels: usize,
    ) -> Result<Self> {
        if maximum_levels > total_levels
            || (effects == 0 && (total_levels != 0 || slope_channels != 0))
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "owned FE summary dimensions are inconsistent",
            ));
        }
        let n = rows as u128;
        let d = effects as u128;
        let s = slope_channels as u128;
        let resident_bytes = effect_resident_bytes(n, d, total_levels as u128, s)?;
        let construction_bytes = checked_sum(&[
            resident_bytes,
            bytes::<usize>(checked_mul(
                maximum_levels as u128,
                2,
                "owned FE summary scratch",
            )?)?,
        ])?;
        Ok(Self {
            resident_bytes,
            construction_bytes,
        })
    }
    // EffectTerm::new builds counts and cursors beside the retained codes,
    // slopes, offsets and memberships. Effects are constructed sequentially.
    pub(crate) fn add_effect(&mut self, rows: usize, levels: usize, slopes: usize) -> Result<()> {
        let n = rows as u128;
        let l = levels as u128;
        let s = slopes as u128;
        let resident = effect_resident_bytes(n, 1, l, s)?;
        let scratch = bytes::<usize>(checked_mul(l, 2, "owned FE membership scratch")?)?;
        self.construction_bytes = checked_sum(&[self.construction_bytes, resident])?
            .max(checked_sum(&[self.resident_bytes, resident, scratch])?);
        self.resident_bytes = checked_sum(&[self.resident_bytes, resident])?;
        Ok(())
    }

    pub(crate) fn add_view_metadata(
        &mut self,
        sources: usize,
        effects: usize,
        slopes: usize,
    ) -> Result<()> {
        // Source views, validated source descriptors, wire-order borrowed
        // effects and absorbed-effect/slope views coexist during construction.
        self.construction_bytes = checked_sum(&[
            self.construction_bytes,
            checked_mul(sources as u128, 128, "owned FE source descriptors")?,
            checked_mul(effects as u128, 128, "owned FE effect descriptors")?,
            bytes::<&[f64]>(slopes as u128)?,
        ])?;
        Ok(())
    }
}

// The aggregate preflight and the sequential allocating adapter use the same
// effect payload; only their construction-overlap composition differs.
fn effect_resident_bytes(n: u128, d: u128, levels: u128, s: u128) -> Result<u128> {
    checked_sum(&[
        bytes::<EffectTerm>(d)?,
        bytes::<u32>(checked_mul(n, d, "owned FE summary codes")?)?,
        bytes::<usize>(checked_sum(&[
            checked_mul(n, d, "owned FE summary memberships")?,
            levels,
            d,
        ])?)?,
        bytes::<f64>(checked_mul(n, s, "owned FE summary slopes")?)?,
        bytes::<Vec<f64>>(s)?,
    ])
}

pub(crate) fn owned_fe_matrix_bytes(rows: usize, rhs: usize) -> Result<u128> {
    bytes::<f64>(checked_mul(
        rows as u128,
        rhs as u128,
        "generic FE matrix cells",
    )?)
}

/// The input matrix and caller-owned FeDesign are charged by the caller.
/// Generic certificate QR moves each level column into its basis without
/// copying the numeric bank; both descriptor banks can coexist. `yielding` is
/// an optional exact count of yielding factor-pair components for an
/// intercept-only design (see `FeDesign::yielding_pair_components`); `None`
/// keeps the level-count bound.
pub(crate) fn generic_fe_solver_bytes(
    design: &FeDesign,
    rhs: usize,
    solver: FeSolverKind,
    config: LsmrConfig,
    yielding: Option<usize>,
) -> Result<u128> {
    let r = rhs as u128;
    let c = design.max_channels() as u128;
    let mut certificate_columns = 0;
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            certificate_columns = certificate_columns.max(checked_mul(
                effect.level_index().members(level).len() as u128,
                effect.channels() as u128,
                "generic certificate level columns",
            )?);
        }
    }
    let certificate = checked_sum(&[
        bytes::<f64>(certificate_columns)?,
        bytes::<Vec<f64>>(checked_mul(c, 2, "generic certificate descriptors")?)?,
        // Original/residual norms, scaled-square scans and RHS scales, with
        // room for the enclosing automatic route's reference norms.
        bytes::<f64>(checked_mul(r, 8, "generic certificate RHS state")?)?,
    ])?;
    let projection = bytes::<f64>(checked_sum(&[
        checked_mul(c, c, "generic projection Gram")?,
        checked_mul(c, r, "generic projection RHS")?,
    ])?)?;
    let matrix = owned_fe_matrix_bytes(design.nobs(), rhs)?;
    let matrices = match solver {
        FeSolverKind::Map | FeSolverKind::Auto => 0,
        FeSolverKind::Cg => 3,
        FeSolverKind::IronsTuck | FeSolverKind::Squarem => 4,
        FeSolverKind::Lsmr => {
            return generic_lsmr_bytes(design, rhs, config, certificate, yielding)
        }
        FeSolverKind::Cmg | FeSolverKind::CmgSchur => {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "generic owned FE memory planning does not support CMG",
            ))
        }
    };
    checked_sum(&[
        certificate,
        projection,
        checked_mul(matrix, matrices, "generic accelerator matrices")?,
        bytes::<f64>(checked_mul(r, 16, "generic accelerator scalar RHS state")?)?,
    ])
}

fn generic_lsmr_bytes(
    design: &FeDesign,
    rhs: usize,
    config: LsmrConfig,
    certificate: u128,
    yielding: Option<usize>,
) -> Result<u128> {
    if design.effects().is_empty() || rhs == 0 {
        return Ok(certificate);
    }
    let mut counts = Vec::new();
    let channels = design.effects().iter().try_fold(0_usize, |total, effect| {
        total
            .checked_add(effect.channels())
            .ok_or_else(|| allocation("generic FE channel count overflows"))
    })?;
    counts
        .try_reserve_exact(channels)
        .map_err(|_| allocation("unable to reserve generic FE channel counts"))?;
    let mut slope_bytes = 0;
    let mut has_slopes = false;
    for effect in design.effects() {
        counts.extend(std::iter::repeat(effect.n_levels()).take(effect.channels()));
        let s = effect.slopes().len() as u128;
        if s == 0 {
            continue;
        }
        has_slopes = true;
        slope_bytes = checked_sum(&[
            slope_bytes,
            lsmr_slope_effect_bytes(
                design.nobs(),
                rhs,
                effect.n_levels(),
                effect.slopes().len(),
                effect.channels(),
            )?,
        ])?;
    }
    // Each channel contributes its own coefficient block and cross-tab pairs.
    // Repeated term levels conservatively charge sorted codes once per channel.
    // The shared recurrence/window/scatter model and categorical routes remain
    // unchanged; signed channels additionally admit the doubled Schur cover.
    // An intercept-only design whose level-count bound was refused may pass
    // its exact yielding pair-component count (see `admit_generic_solver`).
    let estimate = match yielding {
        Some(yielding) if !has_slopes => super::estimate_fe_solver_memory_with_yielding(
            design.nobs(),
            rhs,
            &counts,
            rayon::current_num_threads(),
            config.local_size,
            Some(yielding),
        )?,
        _ => estimate_fe_solver_memory_for_channels(
            design.nobs(),
            rhs,
            &counts,
            rayon::current_num_threads(),
            config.local_size,
            has_slopes,
        )?,
    };
    let preconditioner = crate::fe::resolve_preconditioner_kind(design, config.preconditioner);
    let solver_bytes = estimate
        .lsmr_bytes(preconditioner)
        .max(estimate.lsmr_off_bytes);
    checked_sum(&[
        solver_bytes,
        slope_bytes,
        certificate,
        bytes::<usize>(channels as u128)?,
    ])
}

/// Pinned within slope reparameterization: retained transforms/loadings and
/// construction scratch. Shared by owned absorption and saved-effect recovery.
pub(super) fn lsmr_slope_effect_bytes(
    observations: usize,
    rhs: usize,
    levels: usize,
    slopes: usize,
    channels: usize,
) -> Result<u128> {
    let s = slopes as u128;
    let l = levels as u128;
    let square = checked_mul(s, s, "LSMR slope transform square")?;
    // Pinned within's SlopeReparam keeps sorted old and whitened loadings
    // together during construction. Per-level means/comoments coexist
    // with retained centers/transforms. Full squares bound the triangle.
    // Vec growth for unidentified directions and transform descriptors is
    // charged separately; the state remains through coefficient recovery.
    checked_sum(&[
        bytes::<f64>(checked_mul(
            checked_mul(observations as u128, s, "LSMR slope loading cells")?,
            2,
            "LSMR loading copies",
        )?)?,
        bytes::<f64>(checked_mul(
            l,
            checked_sum(&[
                checked_mul(square, 2, "LSMR slope moments and transforms")?,
                checked_mul(s, 2, "LSMR slope means and centers")?,
                1,
            ])?,
            "LSMR slope level state",
        )?)?,
        checked_mul(l, 32, "LSMR level transform descriptors")?,
        checked_mul(
            checked_mul(l, channels as u128, "LSMR unidentified directions")?,
            96,
            "LSMR unidentified direction banks",
        )?,
        // Pivoted Gram-Schmidt's growing basis, Gram, working vectors,
        // slope views and back-transform scratch (one per active RHS).
        bytes::<f64>(checked_sum(&[
            checked_mul(square, 4, "LSMR slope Gram scratch")?,
            checked_mul(
                s,
                checked_sum(&[16, rhs as u128])?,
                "LSMR slope linear scratch",
            )?,
        ])?)?,
        256,
    ])
}

fn bytes<T>(elements: u128) -> Result<u128> {
    checked_mul(
        elements,
        std::mem::size_of::<T>() as u128,
        "generic FE byte count overflows",
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn summary_covers_sequential_construction_without_summing_scratch() {
        for levels in [&[97][..], &[97, 31, 2][..], &[2, 31, 97][..]] {
            let mut exact = super::OwnedFeDesignMemoryEstimate::default();
            for (index, &level) in levels.iter().enumerate() {
                exact.add_effect(97, level, index).unwrap();
            }
            let summary = super::OwnedFeDesignMemoryEstimate::from_summary(
                97,
                levels.len(),
                levels.iter().sum(),
                *levels.iter().max().unwrap(),
                (0..levels.len()).sum(),
            )
            .unwrap();
            assert_eq!(summary.resident_bytes, exact.resident_bytes);
            assert!(summary.construction_bytes >= exact.construction_bytes);
            assert_eq!(
                summary.construction_bytes - summary.resident_bytes,
                2 * *levels.iter().max().unwrap() as u128 * std::mem::size_of::<usize>() as u128
            );
        }
        assert!(super::OwnedFeDesignMemoryEstimate::from_summary(1, 0, 1, 1, 0).is_err());
        assert!(super::OwnedFeDesignMemoryEstimate::from_summary(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .is_err());
    }

    use super::*;

    #[test]
    fn owned_shape_charges_actual_retained_capacities_and_membership_scratch() {
        let n = 48;
        let mut estimate = OwnedFeDesignMemoryEstimate::default();
        let mut effects = Vec::with_capacity(2);
        for (levels, slopes) in [(4, 0), (3, 2)] {
            estimate.add_effect(n, levels, slopes).unwrap();
            effects.push(
                EffectTerm::new(
                    (0..n).map(|i| (i % levels) as u32).collect(),
                    levels,
                    true,
                    vec![vec![1.0; n]; slopes],
                )
                .unwrap(),
            );
        }
        let design = FeDesign::new(n, effects).unwrap();
        assert_eq!(
            estimate.resident_bytes,
            design.storage_capacities().resident_bytes
        );
        assert!(
            estimate.construction_bytes
                >= estimate.resident_bytes + 2 * 4 * std::mem::size_of::<usize>() as u128
        );
        let before = estimate.construction_bytes;
        estimate.add_view_metadata(2, 2, 2).unwrap();
        assert!(estimate.construction_bytes > before);
        assert_eq!(
            estimate.resident_bytes,
            design.storage_capacities().resident_bytes
        );
    }

    #[test]
    fn owned_shape_rejects_overflow_before_allocation() {
        let mut estimate = OwnedFeDesignMemoryEstimate::default();
        assert_eq!(
            estimate
                .add_effect(usize::MAX, usize::MAX, usize::MAX)
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
        assert_eq!(
            owned_fe_matrix_bytes(usize::MAX, usize::MAX)
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
    }

    #[test]
    fn signed_schwarz_reserves_cover_without_changing_categorical_estimates() {
        use crate::memory::categorical_schwarz::{
            estimate_categorical_schwarz_memory, estimate_schwarz_memory,
        };
        let plain = estimate_categorical_schwarz_memory(48, 3, 15, 6, 4, 100, None).unwrap();
        let unsigned = estimate_schwarz_memory(48, 3, 15, 6, 4, 1, 100, false, None).unwrap();
        let signed = estimate_schwarz_memory(48, 3, 15, 6, 4, 1, 100, true, None).unwrap();
        assert_eq!(plain, unsigned);
        assert_eq!(signed.solver_frame_bytes, plain.solver_frame_bytes);
        assert!(signed.build_peak_bytes > plain.build_peak_bytes);
        assert!(signed.retained_bytes > plain.retained_bytes);
        assert!(signed.apply_peak_bytes > plain.apply_peak_bytes);
    }
}
