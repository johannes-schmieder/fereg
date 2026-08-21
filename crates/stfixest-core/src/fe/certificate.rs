use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::FeDesign;

/// Final weighted fixed-effect orthogonality certificate.
#[derive(Debug, Clone, PartialEq)]
pub struct FeCertificate {
    /// Largest raw element of `D' W r` over all effects and right-hand sides.
    pub absolute_infinity_norm: f64,
    /// Largest scale-free moment `|d_j'Wr| / sqrt((d_j'Wd_j)(r'Wr))`.
    pub relative_infinity_norm: f64,
    /// Root-sum-square of scale-free moments over all FE coefficient directions.
    pub relative_two_norm: f64,
    pub passed: bool,
}

/// Recompute true weighted fixed-effect moments from returned residuals.
///
/// The certificate never trusts a recurrence estimate. Each FE direction is
/// normalized by its weighted basis norm and the weighted residual norm, so the
/// tolerance does not deteriorate mechanically with group size or weight scale.
pub fn certify_residuals(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    tolerance: f64,
) -> Result<FeCertificate> {
    if residuals.nrows() != design.nobs() || weights.len() != design.nobs() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "certificate inputs have inconsistent observation counts",
        ));
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "certificate tolerance must be finite and positive",
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
    if residuals.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "certificate residuals must be finite",
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

    let mut residual_square_sums = vec![0.0_f64; residuals.ncols()];
    for row in 0..residuals.nrows() {
        for rhs in 0..residuals.ncols() {
            let residual = residuals[(row, rhs)];
            residual_square_sums[rhs] =
                (weights[row] * residual).mul_add(residual, residual_square_sums[rhs]);
        }
    }

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
                let denominator = basis_norm * residual_square_sums[rhs].sqrt();
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
