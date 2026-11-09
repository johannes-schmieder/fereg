//! Incremental storage for generic owned intercept/slope absorption.
use super::{checked_mul, checked_sum};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{EffectTerm, FeDesign, FeSolverKind};

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

/// fereg's own workspace of a generic owned route. The input matrix and
/// caller-owned FeDesign are charged by the caller. Generic certificate QR
/// moves each level column into its basis without copying the numeric bank;
/// both descriptor banks can coexist. For modified LSMR this is fereg's
/// certificate and projection scratch: fedemean admits its own workspace
/// (structure, weight state, recurrence and any Schwarz preconditioner)
/// within the memory limit the caller passes, and releases it before
/// fereg's certificate runs.
pub(crate) fn generic_fe_solver_bytes(
    design: &FeDesign,
    rhs: usize,
    solver: FeSolverKind,
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
        FeSolverKind::Map | FeSolverKind::Auto | FeSolverKind::Lsmr => 0,
        FeSolverKind::Cg => 3,
        FeSolverKind::IronsTuck | FeSolverKind::Squarem => 4,
        FeSolverKind::Elimination => return Err(crate::fe::elimination_request()),
        FeSolverKind::Cmg | FeSolverKind::CmgSchur => {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "generic owned FE memory planning does not support CMG",
            ))
        }
        FeSolverKind::Pcg => return Err(crate::fe::pcg_intercepts_only()),
    };
    checked_sum(&[
        certificate,
        projection,
        checked_mul(matrix, matrices, "generic accelerator matrices")?,
        bytes::<f64>(checked_mul(r, 16, "generic accelerator scalar RHS state")?)?,
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
}
