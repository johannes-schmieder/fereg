//! Incremental OLS output storage beside the prepared fit and inference.
//! This does not certify the separately modeled execution/solver parents.

use std::mem::size_of;

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::{plan_ols_memory, MemoryEstimate, MemoryMode, MemoryPlan};

use crate::result::{
    CERTIFICATE_ROWS, CLUSTER_COLUMNS, COEFFICIENT_STATUS_COLUMNS, DOF_ROWS, FE_COLUMNS, FIT_ROWS,
    RECEIPT_COLUMNS, RUNTIME_ROWS, SOLVER_COLUMNS, SOLVER_ROWS, SUBSET_COLUMNS,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct OlsPublicationShape {
    pub observations: usize,
    pub reported_coefficients: usize,
    pub fe_channels: usize,
    pub cluster_dimensions: usize,
    pub xb: bool,
    pub xbd: bool,
    pub absorbed: bool,
    pub saved_channels: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct OlsPublicationMemory {
    pub shadow_bytes: u128,
    pub shadow_workspace_bytes: u128,
    pub derived_peak_bytes: u128,
    pub publication_bytes: u128,
    final_derived_bytes: u128,
}

impl OlsPublicationMemory {
    pub fn estimate(shape: OlsPublicationShape) -> Result<Self> {
        if shape.saved_channels > shape.fe_channels {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "OLS saved-channel count exceeds declared channels",
            ));
        }
        let m = shape.reported_coefficients as u128;
        let matrix = m.max(1);
        let square = mul(matrix, matrix)?;
        let subsets = (1_u128
            .checked_shl(u32::try_from(shape.cluster_dimensions).map_err(|_| overflow())?)
            .ok_or_else(overflow)?
            - 1)
        .max(1);
        let shadow_cells = sum(&[
            mul(2, square)?, // covariance and omission covariance marker
            mul(matrix, (COEFFICIENT_STATUS_COLUMNS + 2) as u128)?,
            ((FIT_ROWS + DOF_ROWS + CERTIFICATE_ROWS) * 2
                + SOLVER_ROWS * SOLVER_COLUMNS
                + RECEIPT_COLUMNS) as u128,
            mul(shape.fe_channels.max(1) as u128, FE_COLUMNS as u128)?,
            mul(
                shape.cluster_dimensions.max(1) as u128,
                CLUSTER_COLUMNS as u128,
            )?,
            mul(subsets, SUBSET_COLUMNS as u128)?,
        ])?;
        let shadow_bytes = mul(shadow_cells, size_of::<f64>() as u128)?;
        // Both coordinate validators remain in the builder's scope while the
        // determinant owns its matrix copy. Small determinants are stack-only.
        let shadow_workspace_bytes = sum(&[
            mul(m, (size_of::<bool>() + size_of::<Option<usize>>()) as u128)?,
            if m > 2 {
                mul(square, size_of::<f64>() as u128)?
            } else {
                0
            },
        ])?;
        let vector_bytes = mul(shape.observations as u128, size_of::<f64>() as u128)?;
        let registry_bytes = mul(
            shape.fe_channels as u128,
            size_of::<Option<Vec<f64>>>() as u128,
        )?;
        let saved = shape.saved_channels > 0;
        let derived = shape.xb || shape.xbd || shape.absorbed || saved;
        let final_vectors = sum(&[
            u128::from(shape.xb),
            u128::from(shape.xbd),
            u128::from(shape.absorbed),
            shape.saved_channels as u128,
        ])?;
        let final_derived_bytes = sum(&[mul(vector_bytes, final_vectors)?, registry_bytes])?;
        // Recovery returns before xbd allocation. Its result initially coexists
        // with the empty output registry and request mask; both are released
        // before xbd is constructed. Unrequested xb/target buffers live longer.
        let returned_vectors = sum(&[
            u128::from(shape.xb || shape.absorbed),
            u128::from(shape.absorbed || saved),
            u128::from(saved && shape.absorbed),
            shape.saved_channels as u128,
        ])?;
        let fitted_bytes = mul(m, size_of::<(usize, f64)>() as u128)?;
        let derived_peak_bytes = if derived {
            let completed = sum(&[
                mul(
                    vector_bytes,
                    sum(&[returned_vectors, u128::from(shape.xbd)])?,
                )?,
                fitted_bytes,
                registry_bytes,
            ])?;
            let returned = if saved {
                sum(&[
                    mul(vector_bytes, returned_vectors)?,
                    fitted_bytes,
                    mul(2, registry_bytes)?,
                    shape.fe_channels as u128,
                ])?
            } else {
                0
            };
            completed.max(returned)
        } else {
            registry_bytes
        };
        let publication_bytes = sum(&[
            final_derived_bytes,
            shadow_bytes,
            ((RUNTIME_ROWS * 2) * size_of::<f64>()) as u128,
        ])?;
        sum(&[final_derived_bytes, shadow_bytes, shadow_workspace_bytes])?;
        Ok(Self {
            shadow_bytes,
            shadow_workspace_bytes,
            derived_peak_bytes,
            publication_bytes,
            final_derived_bytes,
        })
    }

    pub fn peak_bytes(self) -> u128 {
        // The constructor checked this sum before returning the private model.
        self.derived_peak_bytes
            .max(self.final_derived_bytes + self.shadow_bytes + self.shadow_workspace_bytes)
            .max(self.publication_bytes)
    }

    /// Admit publication beside the base estimate, preserving public base fields.
    /// Absorption and publication are sequential; the FE solver retains its
    /// separate admission. Recovery adds only its caller's live output parents.
    pub fn plan(
        self,
        requested: MemoryMode,
        base: MemoryEstimate,
        available: Option<u128>,
    ) -> Result<MemoryPlan> {
        let admitted = MemoryEstimate {
            fast_bytes: sum(&[base.fast_bytes, self.peak_bytes()])?,
            low_bytes: sum(&[base.low_bytes, self.peak_bytes()])?,
            recommended_poolsize: base.recommended_poolsize,
        };
        let mut plan = plan_ols_memory(requested, admitted, available)?;
        plan.estimate = base;
        Ok(plan)
    }
}

fn sum(values: &[u128]) -> Result<u128> {
    values.iter().try_fold(0_u128, |total, value| {
        total.checked_add(*value).ok_or_else(overflow)
    })
}

fn mul(left: u128, right: u128) -> Result<u128> {
    left.checked_mul(right).ok_or_else(overflow)
}

fn overflow() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "OLS publication memory estimate overflows",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape() -> OlsPublicationShape {
        OlsPublicationShape {
            observations: 123,
            reported_coefficients: 5,
            fe_channels: 2,
            cluster_dimensions: 2,
            xb: false,
            xbd: false,
            absorbed: false,
            saved_channels: 0,
        }
    }

    #[test]
    fn reported_axis_squares_and_validator_lifetimes_are_explicit() {
        let estimate = OlsPublicationMemory::estimate(shape()).unwrap();
        let cells = 5 + 25 + 20 + 5 + 25 + 24 + 40 + 32 + 16 + 8 + 12 + 64 + 16;
        assert_eq!(estimate.shadow_bytes, cells * 8);
        assert_eq!(
            estimate.shadow_workspace_bytes,
            5 * (size_of::<bool>() + size_of::<Option<usize>>()) as u128 + 25 * 8
        );
        assert_eq!(
            estimate.derived_peak_bytes,
            2 * size_of::<Option<Vec<f64>>>() as u128
        );
        assert_eq!(
            estimate.peak_bytes(),
            estimate.publication_bytes.max(
                estimate.shadow_bytes
                    + estimate.shadow_workspace_bytes
                    + estimate.derived_peak_bytes
            )
        );
        for coefficients in [0, 1, 2] {
            let small = OlsPublicationMemory::estimate(OlsPublicationShape {
                reported_coefficients: coefficients,
                ..shape()
            })
            .unwrap();
            assert_eq!(
                small.shadow_workspace_bytes,
                coefficients as u128 * (size_of::<bool>() + size_of::<Option<usize>>()) as u128
            );
        }
    }

    #[test]
    fn partial_saves_charge_targets_registries_and_only_requested_vectors() {
        let estimate = OlsPublicationMemory::estimate(OlsPublicationShape {
            fe_channels: 7,
            saved_channels: 2,
            absorbed: true,
            xbd: true,
            ..shape()
        })
        .unwrap();
        let registry = 7 * size_of::<Option<Vec<f64>>>() as u128;
        // xb work + target + recovered total + xbd + two saved channels.
        assert_eq!(estimate.derived_peak_bytes, 6 * 123 * 8 + 5 * 16 + registry);
        assert_eq!(
            estimate.publication_bytes,
            4 * 123 * 8 + registry + estimate.shadow_bytes + 60 * 8
        );
        let empty = OlsPublicationMemory::estimate(OlsPublicationShape {
            observations: 0,
            fe_channels: 7,
            saved_channels: 2,
            absorbed: true,
            xbd: true,
            ..shape()
        })
        .unwrap();
        assert_eq!(empty.derived_peak_bytes, 5 * 16 + 2 * registry + 7);
    }

    #[test]
    fn exact_and_short_budgets_route_with_outputs_but_keep_base_fields() {
        let output = OlsPublicationMemory::estimate(shape()).unwrap();
        let base = MemoryEstimate {
            fast_bytes: 90_000,
            low_bytes: 40_000,
            recommended_poolsize: 1,
        };
        let headroom = 512 * 1024 * 1024;
        for (mode, bytes) in [
            (MemoryMode::Fast, base.fast_bytes),
            (MemoryMode::Low, base.low_bytes),
        ] {
            let limit = headroom + bytes + output.peak_bytes();
            let plan = output.plan(mode, base, Some(limit)).unwrap();
            assert_eq!(plan.estimate, base);
            assert_eq!(plan.selected_bytes(), bytes);
            assert_eq!(
                output.plan(mode, base, Some(limit - 1)).unwrap_err().code,
                ErrorCode::Allocation
            );
        }
        let plan = output
            .plan(
                MemoryMode::Auto,
                base,
                Some(headroom + base.low_bytes + output.peak_bytes()),
            )
            .unwrap();
        assert_eq!(plan.selected, MemoryMode::Low);
        assert_eq!(plan.estimate, base);
        assert_eq!(
            OlsPublicationMemory::estimate(OlsPublicationShape {
                reported_coefficients: usize::MAX,
                ..shape()
            })
            .unwrap_err()
            .code,
            ErrorCode::Allocation
        );
    }
}
