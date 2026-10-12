//! Single-use LSMR certificate workspace lends its dead reference norm bank.
use super::*;

pub(super) fn certify(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
    dense: bool,
) -> Result<(FeCertificate, bool)> {
    if !dense {
        return super::certify(design, weights, values, tolerance, workspace).map(|c| (c, false));
    }
    let valid = super::super::norms::column_norms_with_dense(
        values,
        |row| {
            let root = weights.normalized(row).sqrt();
            if root.is_finite() && root > 0.0 {
                Ok(root)
            } else {
                Err(invalid_data(
                    "borrowed MAP normalized weight is not finite and positive",
                ))
            }
        },
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
        &mut workspace.rhs_scales,
        "borrowed MAP weighted value is not finite",
        "borrowed MAP weighted column norm is not finite",
    )?;
    workspace.rhs_relative_infinity_norms.fill(0.0);
    for rhs in 0..values.ncols() {
        let scale = workspace.reference_norms[rhs].max(workspace.residual_norms[rhs]);
        if !scale.is_finite() || scale < 0.0 {
            return Err(invalid_data("borrowed MAP RHS scale is invalid"));
        }
        workspace.reference_norms[rhs] = workspace.rhs_scales[rhs];
        workspace.rhs_scales[rhs] = scale;
    }
    certify_with_current_scales(design, weights, values, tolerance, workspace).map(|c| (c, valid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

    #[test]
    fn norm_handoff_reuses_reference_capacity_after_exact_certificate_scale_checks() {
        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).unwrap();
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).unwrap();
        let values =
            DenseMatrix::from_columns(&[vec![1., -1., 2., -2.], vec![3., -3., 4., -4.]]).unwrap();
        let weights = validate_weights(BorrowedMapWeights::Unit, 4).unwrap();
        let mut workspace =
            BorrowedMapWorkspace::new(&design, 4, 2, &mut SystemWorkspaceReservation).unwrap();
        workspace.reference_norms.reserve_exact(71);
        let identity = (
            workspace.reference_norms.as_ptr(),
            workspace.reference_norms.capacity(),
        );
        let peak = workspace.payload_peak().unwrap();
        update_workspace_norms(&values, weights, &mut workspace, true).unwrap();
        let expected =
            super::super::certify(&design, weights, &values, 1e-12, &mut workspace).unwrap();
        let (actual, valid) =
            certify(&design, weights, &values, 1e-12, &mut workspace, true).unwrap();
        assert!(valid && actual.passed);
        assert_eq!(actual, expected);
        assert_eq!(workspace.payload_peak().unwrap(), peak);
        let norms = std::mem::take(&mut workspace.reference_norms);
        assert_eq!((norms.as_ptr(), norms.capacity()), identity);
        assert_eq!(norms, vec![10.0_f64.sqrt(), 50.0_f64.sqrt()]);
    }
}
