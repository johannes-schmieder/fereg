use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::FeDesign;

/// Final weighted fixed-effect orthogonality certificate.
#[derive(Debug, Clone, PartialEq)]
pub struct FeCertificate {
    /// Largest raw element of `D' W r` over all effects and right-hand sides.
    pub absolute_infinity_norm: f64,
    /// Largest scale-free moment `|d_j'Wr| / (||d_j||_W ||y_j||_W)`.
    pub relative_infinity_norm: f64,
    /// Root-mean-square of scale-free moments over all FE coefficient directions.
    pub relative_two_norm: f64,
    pub passed: bool,
}

/// Weighted Euclidean norm of each matrix column.
pub(crate) fn weighted_column_norms(values: &DenseMatrix, weights: &[f64]) -> Result<Vec<f64>> {
    validate_common(values, weights)?;
    let mut square_sums = vec![0.0_f64; values.ncols()];
    for row in 0..values.nrows() {
        for column in 0..values.ncols() {
            let value = values[(row, column)];
            square_sums[column] = (weights[row] * value).mul_add(value, square_sums[column]);
        }
    }
    Ok(square_sums.into_iter().map(f64::sqrt).collect())
}

/// Recompute true weighted fixed-effect moments from returned residuals.
///
/// This convenience entry point uses the residual norm as the reference scale.
/// Estimation solvers should call the internal reference-norm entry point with
/// norms of the original right-hand sides so that an almost-exact fit does not
/// turn roundoff into an order-one relative moment.
pub fn certify_residuals(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    tolerance: f64,
) -> Result<FeCertificate> {
    let reference_norms = weighted_column_norms(residuals, weights)?;
    certify_residuals_with_reference_norms(design, weights, residuals, &reference_norms, tolerance)
}

/// Recompute true weighted FE moments using pre-absorption RHS norms.
pub(crate) fn certify_residuals_with_reference_norms(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    reference_norms: &[f64],
    tolerance: f64,
) -> Result<FeCertificate> {
    if residuals.nrows() != design.nobs() || weights.len() != design.nobs() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "certificate inputs have inconsistent observation counts",
        ));
    }
    validate_common(residuals, weights)?;
    if reference_norms.len() != residuals.ncols()
        || reference_norms
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "certificate reference norms are invalid",
        ));
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "certificate tolerance must be finite and positive",
        ));
    }
    if design.effects().is_empty() || residuals.ncols() == 0 {
        return Ok(FeCertificate {
            absolute_infinity_norm: 0.0,
            relative_infinity_norm: 0.0,
            relative_two_norm: 0.0,
            passed: true,
        });
    }

    let residual_norms = weighted_column_norms(residuals, weights)?;
    let rhs_scales = reference_norms
        .iter()
        .zip(&residual_norms)
        .map(|(&reference, &residual)| reference.max(residual))
        .collect::<Vec<_>>();

    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0_usize;
    for effect in design.effects() {
        let channels = effect.channels();
        let directions = effect.n_levels() * channels;
        let mut moments = vec![0.0; directions * residuals.ncols()];
        let mut basis_square_sums = vec![0.0; directions];
        for observation in 0..design.nobs() {
            let level = effect.codes()[observation] as usize;
            let weight = weights[observation];
            for channel in 0..channels {
                let basis = effect.basis_value(channel, observation);
                let direction = level * channels + channel;
                basis_square_sums[direction] =
                    (weight * basis).mul_add(basis, basis_square_sums[direction]);
                for rhs in 0..residuals.ncols() {
                    let index = direction * residuals.ncols() + rhs;
                    moments[index] =
                        (weight * basis).mul_add(residuals[(observation, rhs)], moments[index]);
                }
            }
        }
        for direction in 0..directions {
            let basis_norm = basis_square_sums[direction].sqrt();
            for rhs in 0..residuals.ncols() {
                let moment = moments[direction * residuals.ncols() + rhs];
                absolute_infinity_norm = absolute_infinity_norm.max(moment.abs());
                let denominator = basis_norm * rhs_scales[rhs];
                let relative = if denominator > 0.0 {
                    moment.abs() / denominator
                } else if moment == 0.0 {
                    0.0
                } else {
                    f64::INFINITY
                };
                relative_infinity_norm = relative_infinity_norm.max(relative);
                relative_square_sum = relative.mul_add(relative, relative_square_sum);
                relative_count += 1;
            }
        }
    }

    let relative_two_norm = if relative_count == 0 {
        0.0
    } else {
        (relative_square_sum / relative_count as f64).sqrt()
    };
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance && relative_two_norm <= tolerance,
    })
}

fn validate_common(values: &DenseMatrix, weights: &[f64]) -> Result<()> {
    if values.nrows() != weights.len() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "weighted matrix and weight lengths differ",
        ));
    }
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "certificate weights must be finite and positive",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "certificate values must be finite",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn certificate_is_invariant_to_common_weight_scaling() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![]).expect("effect");
        let design = FeDesign::new(4, vec![effect]).expect("design");
        let residuals =
            DenseMatrix::from_columns(&[vec![-1.0, 1.0, -2.0, 2.0]]).expect("residuals");
        let first = certify_residuals(&design, &[1.0; 4], &residuals, 1e-12).expect("certificate");
        let second =
            certify_residuals(&design, &[100.0; 4], &residuals, 1e-12).expect("certificate");
        assert!((first.relative_infinity_norm - second.relative_infinity_norm).abs() < 1e-14);
        assert!(first.passed);
        assert!(second.passed);
    }

    #[test]
    fn reference_scale_certifies_near_exact_fit_roundoff() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![]).expect("effect");
        let design = FeDesign::new(4, vec![effect]).expect("design");
        let residuals =
            DenseMatrix::from_columns(&[vec![1e-15, -1e-15, 2e-15, -2e-15]]).expect("residuals");
        let certificate =
            certify_residuals_with_reference_norms(&design, &[1.0; 4], &residuals, &[10.0], 1e-12)
                .expect("certificate");
        assert!(certificate.passed);
    }

    #[test]
    fn certificate_rejects_nonorthogonal_residuals() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![]).expect("effect");
        let design = FeDesign::new(4, vec![effect]).expect("design");
        let residuals =
            DenseMatrix::from_columns(&[vec![1.0, 1.0, -1.0, -1.0]]).expect("residuals");
        let certificate =
            certify_residuals(&design, &[1.0; 4], &residuals, 1e-8).expect("certificate");
        assert!(!certificate.passed);
    }
}
