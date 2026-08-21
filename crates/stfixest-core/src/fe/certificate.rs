use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::FeDesign;

/// Final weighted fixed-effect orthogonality certificate.
#[derive(Debug, Clone, PartialEq)]
pub struct FeCertificate {
    pub absolute_infinity_norm: f64,
    pub relative_infinity_norm: f64,
    pub relative_two_norm: f64,
    pub passed: bool,
}

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
    if design.effects().is_empty() || residuals.ncols() == 0 {
        return Ok(FeCertificate {
            absolute_infinity_norm: 0.0,
            relative_infinity_norm: 0.0,
            relative_two_norm: 0.0,
            passed: true,
        });
    }

    let mut absolute_infinity_norm = 0.0_f64;
    let mut moment_square_sum = 0.0_f64;
    let mut residual_square_sum = 0.0_f64;
    let mut scale = 0.0_f64;
    for row in 0..residuals.nrows() {
        for rhs in 0..residuals.ncols() {
            let weighted_residual = weights[row] * residuals[(row, rhs)];
            residual_square_sum = weighted_residual.mul_add(weighted_residual, residual_square_sum);
            scale = scale.max(weighted_residual.abs());
        }
    }

    for effect in design.effects() {
        let channels = effect.channels();
        let mut moments = vec![0.0; effect.n_levels() * channels * residuals.ncols()];
        for observation in 0..design.nobs() {
            let level = effect.codes()[observation] as usize;
            for channel in 0..channels {
                let basis = effect.basis_value(channel, observation);
                for rhs in 0..residuals.ncols() {
                    let index = (level * channels + channel) * residuals.ncols() + rhs;
                    moments[index] = (weights[observation] * basis)
                        .mul_add(residuals[(observation, rhs)], moments[index]);
                }
            }
        }
        for moment in moments {
            absolute_infinity_norm = absolute_infinity_norm.max(moment.abs());
            moment_square_sum = moment.mul_add(moment, moment_square_sum);
        }
    }

    let denominator = scale.max(f64::MIN_POSITIVE);
    let relative_infinity_norm = absolute_infinity_norm / denominator;
    let relative_two_norm =
        moment_square_sum.sqrt() / residual_square_sum.sqrt().max(f64::MIN_POSITIVE);
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance || relative_two_norm <= tolerance,
    })
}
