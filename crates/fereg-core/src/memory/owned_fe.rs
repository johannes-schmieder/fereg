//! Incremental storage for generic owned intercept/slope absorption.
use super::{checked_mul, checked_sum, estimate_fe_solver_memory_for_channels};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{EffectTerm, FeDesign, FeSolverKind, LsmrConfig};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct OwnedFeDesignMemoryEstimate {
    pub(crate) resident_bytes: u128,
    pub(crate) construction_bytes: u128,
}

impl OwnedFeDesignMemoryEstimate {
    // EffectTerm::new builds counts and cursors beside the retained codes,
    // slopes, offsets and memberships. Effects are constructed sequentially.
    pub(crate) fn add_effect(&mut self, rows: usize, levels: usize, slopes: usize) -> Result<()> {
        let n = rows as u128;
        let l = levels as u128;
        let s = slopes as u128;
        let resident = checked_sum(&[
            std::mem::size_of::<EffectTerm>() as u128,
            bytes::<u32>(n)?,
            bytes::<usize>(checked_sum(&[n, l, 1])?)?,
            bytes::<f64>(checked_mul(n, s, "owned FE slope cells")?)?,
            bytes::<Vec<f64>>(s)?,
        ])?;
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

pub(crate) fn owned_fe_matrix_bytes(rows: usize, rhs: usize) -> Result<u128> {
    bytes::<f64>(checked_mul(
        rows as u128,
        rhs as u128,
        "generic FE matrix cells",
    )?)
}

/// The input matrix and caller-owned FeDesign are charged by the caller.
/// Generic certificate QR moves each level column into its basis without
/// copying the numeric bank; both descriptor banks can coexist.
pub(crate) fn generic_fe_solver_bytes(
    design: &FeDesign,
    rhs: usize,
    solver: FeSolverKind,
    config: LsmrConfig,
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
        FeSolverKind::Lsmr => return generic_lsmr_bytes(design, rhs, config, certificate),
        FeSolverKind::Cmg => {
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
        let l = effect.n_levels() as u128;
        let square = checked_mul(s, s, "LSMR slope transform square")?;
        // Pinned within's SlopeReparam keeps sorted old and whitened loadings
        // together during construction. Per-level means/comoments coexist
        // with retained centers/transforms. Full squares bound the triangle.
        // Vec growth for unidentified directions and transform descriptors is
        // charged separately; the state remains through coefficient recovery.
        slope_bytes = checked_sum(&[
            slope_bytes,
            bytes::<f64>(checked_mul(
                checked_mul(design.nobs() as u128, s, "LSMR slope loading cells")?,
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
                checked_mul(l, effect.channels() as u128, "LSMR unidentified directions")?,
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
        ])?;
    }
    // Each channel contributes its own coefficient block and cross-tab pairs.
    // Repeated term levels conservatively charge sorted codes once per channel.
    // The shared recurrence/window/scatter model and categorical routes remain
    // unchanged; signed channels additionally admit the doubled Schur cover.
    let estimate = estimate_fe_solver_memory_for_channels(
        design.nobs(),
        rhs,
        &counts,
        rayon::current_num_threads(),
        config.local_size,
        has_slopes,
    )?;
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

fn bytes<T>(elements: u128) -> Result<u128> {
    checked_mul(
        elements,
        std::mem::size_of::<T>() as u128,
        "generic FE byte count overflows",
    )
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
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
        let plain = estimate_categorical_schwarz_memory(48, 3, 15, 6, 4, 100).unwrap();
        let unsigned = estimate_schwarz_memory(48, 3, 15, 6, 4, 100, false).unwrap();
        let signed = estimate_schwarz_memory(48, 3, 15, 6, 4, 100, true).unwrap();
        assert_eq!(plain, unsigned);
        assert_eq!(signed.solver_frame_bytes, plain.solver_frame_bytes);
        assert!(signed.build_peak_bytes > plain.build_peak_bytes);
        assert!(signed.retained_bytes > plain.retained_bytes);
        assert!(signed.apply_peak_bytes > plain.apply_peak_bytes);
    }
}
