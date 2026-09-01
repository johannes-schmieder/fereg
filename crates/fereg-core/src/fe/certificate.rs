use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;

/// Final weighted fixed-effect orthogonality certificate.
#[derive(Debug, Clone, PartialEq)]
pub struct FeCertificate {
    /// Largest raw element of `D' W r` over all effects and right-hand sides.
    /// Saturates to infinity when a nonzero diagnostic is not representable.
    pub absolute_infinity_norm: f64,
    /// Largest basis-invariant local projection ratio over all FE levels and RHSs.
    pub relative_infinity_norm: f64,
    /// Root-mean-square of the basis-invariant local projection ratios.
    pub relative_two_norm: f64,
    pub passed: bool,
}

fn fallible_filled<T: Clone>(elements: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    output.resize(elements, value);
    Ok(output)
}

fn fallible_empty<T>(elements: usize, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    Ok(output)
}

/// Weighted Euclidean norm of each matrix column.
pub(crate) fn weighted_column_norms(values: &DenseMatrix, weights: &[f64]) -> Result<Vec<f64>> {
    validate_common(values, weights)?;
    let mut scales = fallible_filled(
        values.ncols(),
        0.0_f64,
        "unable to reserve weighted certificate scales",
    )?;
    let mut scaled_square_sums = fallible_filled(
        values.ncols(),
        1.0_f64,
        "unable to reserve weighted certificate square sums",
    )?;
    for row in 0..values.nrows() {
        let square_root_weight = weights[row].sqrt();
        for column in 0..values.ncols() {
            let value = square_root_weight * values[(row, column)];
            if !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "weighted certificate value is not finite",
                ));
            }
            update_scaled_sum_of_squares(
                value,
                &mut scales[column],
                &mut scaled_square_sums[column],
            );
        }
    }
    for column in 0..scales.len() {
        let scale = scales[column];
        let norm = if scale == 0.0 {
            0.0
        } else {
            scale * scaled_square_sums[column].sqrt()
        };
        if !norm.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "weighted certificate norm is not finite",
            ));
        }
        scales[column] = norm;
    }
    Ok(scales)
}

/// Recompute weighted fixed-effect span projections from returned residuals.
///
/// This convenience entry point uses the residual norm as the reference scale.
/// Estimation solvers should call the internal reference-norm entry point with
/// norms of the original right-hand sides so that an almost-exact fit does not
/// turn roundoff into an order-one relative violation.
pub fn certify_residuals(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    tolerance: f64,
) -> Result<FeCertificate> {
    let reference_norms = weighted_column_norms(residuals, weights)?;
    certify_residuals_with_reference_norms(design, weights, residuals, &reference_norms, tolerance)
}

/// Recompute weighted FE-span projections using pre-absorption RHS norms.
pub(crate) fn certify_residuals_with_reference_norms(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    reference_norms: &[f64],
    tolerance: f64,
) -> Result<FeCertificate> {
    if residuals.nrows() != design.nobs() || weights.len() != design.nobs() {
        return Err(FeregError::new(
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
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "certificate reference norms are invalid",
        ));
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(FeregError::new(
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
    let mut rhs_scales = fallible_empty(
        reference_norms.len(),
        "unable to reserve fixed-effect certificate RHS scales",
    )?;
    for (&reference, &residual) in reference_norms.iter().zip(&residual_norms) {
        rhs_scales.push(reference.max(residual));
    }

    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0_usize;
    for effect in design.effects() {
        let channels = effect.channels();
        for level in 0..effect.n_levels() {
            let observations = effect.level_index().members(level);
            let mut normalized_columns = fallible_empty(
                channels,
                "unable to reserve fixed-effect certificate normalized columns",
            )?;
            for channel in 0..channels {
                let mut column = fallible_empty(
                    observations.len(),
                    "unable to reserve fixed-effect certificate level column",
                )?;
                let mut column_scale = 0.0_f64;
                for &observation in observations {
                    let value =
                        weights[observation].sqrt() * effect.basis_value(channel, observation);
                    if !value.is_finite() {
                        return Err(FeregError::new(
                            ErrorCode::InvalidData,
                            "weighted fixed-effect basis is not finite",
                        ));
                    }
                    column_scale = column_scale.max(value.abs());
                    column.push(value);
                }
                if column_scale == 0.0 {
                    continue;
                }
                for value in &mut column {
                    *value /= column_scale;
                }
                let norm = euclidean_norm(&column);
                if norm == 0.0 || !norm.is_finite() {
                    return Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "fixed-effect basis normalization failed",
                    ));
                }
                for value in &mut column {
                    *value /= norm;
                }
                normalized_columns.push(column);
            }

            let orthonormal_basis = pivoted_reorthogonalized_qr(normalized_columns)?;
            for rhs in 0..residuals.ncols() {
                let denominator = rhs_scales[rhs];
                let mut projected_square_norm = 0.0_f64;
                if denominator > 0.0 {
                    for basis in &orthonormal_basis {
                        let mut coefficient = 0.0_f64;
                        for (local_row, &observation) in observations.iter().enumerate() {
                            let scaled_residual = weights[observation].sqrt()
                                * residuals[(observation, rhs)]
                                / denominator;
                            coefficient = basis[local_row].mul_add(scaled_residual, coefficient);
                        }
                        projected_square_norm =
                            coefficient.mul_add(coefficient, projected_square_norm);
                    }
                }
                let relative = projected_square_norm.sqrt();
                if !relative.is_finite() {
                    return Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "fixed-effect projection certificate is not finite",
                    ));
                }
                relative_infinity_norm = relative_infinity_norm.max(relative);
                relative_square_sum = relative.mul_add(relative, relative_square_sum);
                relative_count += 1;
            }

            for channel in 0..channels {
                for rhs in 0..residuals.ncols() {
                    let moment = scaled_raw_moment(
                        observations,
                        |observation| {
                            weights[observation].sqrt() * effect.basis_value(channel, observation)
                        },
                        |observation| weights[observation].sqrt() * residuals[(observation, rhs)],
                    );
                    absolute_infinity_norm = absolute_infinity_norm.max(moment.abs());
                }
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
        passed: relative_infinity_norm <= tolerance,
    })
}

/// Column-pivoted, twice-reorthogonalized modified Gram-Schmidt.
///
/// Input columns are individually normalized before this routine. That makes
/// rank selection insensitive to FE channel units. The cutoff is tied only to
/// machine precision and block dimensions, never to the user's convergence
/// tolerance.
fn pivoted_reorthogonalized_qr(mut columns: Vec<Vec<f64>>) -> Result<Vec<Vec<f64>>> {
    let nrows = columns.first().map_or(0, Vec::len);
    let ncols = columns.len();
    let rank_threshold = f64::EPSILON * nrows.max(ncols).max(1) as f64 * 8.0;
    let mut basis = fallible_empty(
        nrows.min(ncols),
        "unable to reserve fixed-effect certificate orthonormal basis",
    )?;

    while !columns.is_empty() && basis.len() < nrows {
        let (pivot, pivot_norm) = columns
            .iter()
            .enumerate()
            .map(|(index, column)| (index, euclidean_norm(column)))
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .expect("nonempty QR column set");
        if pivot_norm <= rank_threshold {
            break;
        }

        let mut candidate = columns.swap_remove(pivot);
        reorthogonalize(&mut candidate, &basis);
        let norm = euclidean_norm(&candidate);
        if norm <= rank_threshold {
            continue;
        }
        for value in &mut candidate {
            *value /= norm;
        }

        for column in &mut columns {
            subtract_projection(column, &candidate);
            subtract_projection(column, &candidate);
        }
        basis.push(candidate);
    }
    Ok(basis)
}

fn reorthogonalize(candidate: &mut [f64], basis: &[Vec<f64>]) {
    for _ in 0..2 {
        for direction in basis {
            subtract_projection(candidate, direction);
        }
    }
}

fn subtract_projection(values: &mut [f64], direction: &[f64]) {
    let coefficient = values
        .iter()
        .zip(direction)
        .fold(0.0_f64, |sum, (&value, &basis)| value.mul_add(basis, sum));
    for (value, &basis) in values.iter_mut().zip(direction) {
        *value = (-coefficient).mul_add(basis, *value);
    }
}

fn euclidean_norm(values: &[f64]) -> f64 {
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for &value in values {
        update_scaled_sum_of_squares(value, &mut scale, &mut scaled_square_sum);
    }
    if scale == 0.0 {
        0.0
    } else {
        scale * scaled_square_sum.sqrt()
    }
}

fn update_scaled_sum_of_squares(value: f64, scale: &mut f64, scaled_square_sum: &mut f64) {
    let absolute = value.abs();
    if absolute == 0.0 {
        return;
    }
    if *scale < absolute {
        let ratio = *scale / absolute;
        *scaled_square_sum = 1.0 + *scaled_square_sum * ratio * ratio;
        *scale = absolute;
    } else {
        let ratio = absolute / *scale;
        *scaled_square_sum += ratio * ratio;
    }
}

/// Accumulate a diagnostic dot product after separately scaling both vectors.
///
/// The certificate gate never consumes this value. A true nonzero result that
/// exceeds the `f64` range saturates to infinity instead of turning a valid
/// basis-invariant projection certificate into an error.
fn scaled_raw_moment(
    observations: &[usize],
    left: impl Fn(usize) -> f64,
    right: impl Fn(usize) -> f64,
) -> f64 {
    let mut left_scale = 0.0_f64;
    let mut right_scale = 0.0_f64;
    for &observation in observations {
        left_scale = left_scale.max(left(observation).abs());
        right_scale = right_scale.max(right(observation).abs());
    }
    if left_scale == 0.0 || right_scale == 0.0 {
        return 0.0;
    }

    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for &observation in observations {
        let product = (left(observation) / left_scale) * (right(observation) / right_scale);
        let updated = sum + product;
        correction += if sum.abs() >= product.abs() {
            (sum - updated) + product
        } else {
            (product - updated) + sum
        };
        sum = updated;
    }
    let normalized = sum + correction;
    if normalized == 0.0 {
        return 0.0;
    }

    let smaller_scale = left_scale.min(right_scale);
    let larger_scale = left_scale.max(right_scale);
    let moment = (normalized * smaller_scale) * larger_scale;
    if moment.is_finite() {
        moment
    } else {
        normalized.signum() * f64::INFINITY
    }
}

fn validate_common(values: &DenseMatrix, weights: &[f64]) -> Result<()> {
    if values.nrows() != weights.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "weighted matrix and weight lengths differ",
        ));
    }
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "certificate weights must be finite and positive",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
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
    fn certificate_capacity_overflow_is_an_allocation_error() {
        let error = fallible_empty::<f64>(usize::MAX, "injected certificate allocation")
            .expect_err("capacity overflow");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(error.return_code(), 909);
    }

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
