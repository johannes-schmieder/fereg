use crate::error::{ErrorCode, FeregError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlPsdState {
    /// Robust and one-way clustered covariance must already be PSD.
    Strict,
    /// Multiway CGM inclusion--exclusion was already PSD.
    MultiwayRaw,
    /// Negative eigenvalues of a multiway CGM covariance were clipped only
    /// after complete inclusion--exclusion.
    MultiwayClipped,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmlPsdReport {
    pub state: PpmlPsdState,
    pub adjusted: bool,
    pub clipped_eigenvalues: usize,
    pub raw_minimum_eigenvalue: f64,
    pub final_minimum_eigenvalue: f64,
    pub final_rank: usize,
    pub eigensystem_reconstruction_relative_error: f64,
    pub orthogonality_relative_error: f64,
    pub clipped_reconstruction_relative_error: f64,
    pub passed: bool,
}

pub(crate) fn apply_ppml_psd_policy(
    covariance: &[f64],
    dimension: usize,
    cluster_dimensions: usize,
    tolerance: f64,
) -> Result<(Vec<f64>, PpmlPsdReport)> {
    if !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
        return Err(invalid_argument(
            "PPML PSD tolerance must be finite and lie in (0, 1)",
        ));
    }
    let square = checked_square(dimension, "PPML PSD dimensions overflow usize")?;
    if covariance.len() != square || covariance.iter().any(|value| !value.is_finite()) {
        return Err(invalid_argument(
            "PPML covariance is nonfinite or dimensionally inconsistent",
        ));
    }
    if dimension == 0 {
        return Ok((
            Vec::new(),
            PpmlPsdReport {
                state: if cluster_dimensions > 1 {
                    PpmlPsdState::MultiwayRaw
                } else {
                    PpmlPsdState::Strict
                },
                adjusted: false,
                clipped_eigenvalues: 0,
                raw_minimum_eigenvalue: 0.0,
                final_minimum_eigenvalue: 0.0,
                final_rank: 0,
                eigensystem_reconstruction_relative_error: 0.0,
                orthogonality_relative_error: 0.0,
                clipped_reconstruction_relative_error: 0.0,
                passed: true,
            },
        ));
    }

    let mut symmetric = fallible_copy(covariance, "unable to reserve PPML symmetric covariance")?;
    symmetrize(&mut symmetric, dimension)?;
    let raw = symmetric_eigensystem(&symmetric, dimension)?;
    let raw_minimum = raw.values.iter().copied().fold(f64::INFINITY, f64::min);
    let raw_scale = matrix_scale(&symmetric, dimension)?;
    if cluster_dimensions <= 1 && raw_minimum < -tolerance * raw_scale {
        return Err(numerical_nonconvergence(format!(
            "PPML robust/one-way covariance is materially indefinite: min={raw_minimum:.17e}, scale={raw_scale:.17e}"
        )));
    }

    let raw_reconstruction_error = raw.reconstruction_relative_error;
    let raw_orthogonality_error = raw.orthogonality_relative_error;
    let (output, final_system, final_scale, clipped, clipped_error, state) =
        if cluster_dimensions > 1 && raw_minimum < 0.0 {
            let (mut repaired, clipped) = reconstruct_clipped(&raw, dimension)?;
            let error = certify_clipped_reconstruction(&raw, &repaired, dimension)?;
            drop(raw);
            drop(symmetric);
            symmetrize(&mut repaired, dimension)?;
            let final_system = symmetric_eigensystem(&repaired, dimension)?;
            let final_scale = matrix_scale(&repaired, dimension)?;
            (
                repaired,
                final_system,
                final_scale,
                clipped,
                error,
                PpmlPsdState::MultiwayClipped,
            )
        } else {
            // This exact symmetric matrix already has its certified eigensystem.
            // Only a repair changes it. Publication independently inspects the
            // actual returned covariance under the separate result boundary.
            (
                symmetric,
                raw,
                raw_scale,
                0,
                0.0,
                if cluster_dimensions > 1 {
                    PpmlPsdState::MultiwayRaw
                } else {
                    PpmlPsdState::Strict
                },
            )
        };
    let final_minimum = final_system
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let final_rank = final_system
        .values
        .iter()
        .filter(|value| **value > tolerance * final_scale)
        .count();
    let reconstruction_error =
        raw_reconstruction_error.max(final_system.reconstruction_relative_error);
    let orthogonality_error =
        raw_orthogonality_error.max(final_system.orthogonality_relative_error);
    let passed = final_minimum >= -tolerance * final_scale
        && reconstruction_error <= tolerance
        && orthogonality_error <= tolerance
        && clipped_error <= tolerance;
    if !passed {
        return Err(numerical_nonconvergence(format!(
            "PPML covariance PSD certificate failed: min={final_minimum:.17e}, scale={final_scale:.17e}, eig={reconstruction_error:.3e}, orth={orthogonality_error:.3e}, clip={clipped_error:.3e}"
        )));
    }
    Ok((
        output,
        PpmlPsdReport {
            state,
            adjusted: clipped > 0,
            clipped_eigenvalues: clipped,
            raw_minimum_eigenvalue: raw_minimum,
            final_minimum_eigenvalue: final_minimum,
            final_rank,
            eigensystem_reconstruction_relative_error: reconstruction_error,
            orthogonality_relative_error: orthogonality_error,
            clipped_reconstruction_relative_error: clipped_error,
            passed,
        },
    ))
}

#[derive(Debug)]
struct SymmetricEigensystem {
    values: Vec<f64>,
    vectors: Vec<f64>,
    reconstruction_relative_error: f64,
    orthogonality_relative_error: f64,
}

#[derive(Debug)]
pub(crate) struct PublishedPpmlPsdInspection {
    pub minimum_eigenvalue: f64,
    pub rank: usize,
    pub reconstruction_relative_error: f64,
    pub orthogonality_relative_error: f64,
}

/// Inspect the published matrix itself. The returned comparison matrix retains
/// the existing bound on any further PSD repair, but its eigensystem must not
/// replace diagnostics for the matrix whose report is being validated.
pub(crate) fn inspect_published_ppml_covariance(
    covariance: &[f64],
    dimension: usize,
    cluster_dimensions: usize,
    tolerance: f64,
) -> Result<(Vec<f64>, PublishedPpmlPsdInspection)> {
    if !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
        return Err(invalid_argument(
            "PPML PSD tolerance must be finite and lie in (0, 1)",
        ));
    }
    let square = checked_square(dimension, "PPML PSD dimensions overflow usize")?;
    if covariance.len() != square || covariance.iter().any(|value| !value.is_finite()) {
        return Err(invalid_argument(
            "PPML covariance is nonfinite or dimensionally inconsistent",
        ));
    }
    if dimension == 0 {
        return Ok((
            Vec::new(),
            PublishedPpmlPsdInspection {
                minimum_eigenvalue: 0.0,
                rank: 0,
                reconstruction_relative_error: 0.0,
                orthogonality_relative_error: 0.0,
            },
        ));
    }
    let mut symmetric = fallible_copy(covariance, "unable to reserve PPML symmetric covariance")?;
    symmetrize(&mut symmetric, dimension)?;
    let spectrum = symmetric_eigensystem(&symmetric, dimension)?;
    let minimum = spectrum
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let scale = matrix_scale(&symmetric, dimension)?;
    if minimum < -tolerance * scale
        || spectrum.reconstruction_relative_error > tolerance
        || spectrum.orthogonality_relative_error > tolerance
    {
        return Err(numerical_nonconvergence(
            "PPML published covariance fails its independent PSD inspection",
        ));
    }
    let inspection = PublishedPpmlPsdInspection {
        minimum_eigenvalue: minimum,
        rank: spectrum
            .values
            .iter()
            .filter(|value| **value > tolerance * scale)
            .count(),
        reconstruction_relative_error: spectrum.reconstruction_relative_error,
        orthogonality_relative_error: spectrum.orthogonality_relative_error,
    };
    let comparison = if cluster_dimensions > 1 && minimum < 0.0 {
        let (repaired, _) = reconstruct_clipped(&spectrum, dimension)?;
        if certify_clipped_reconstruction(&spectrum, &repaired, dimension)? > tolerance {
            return Err(numerical_nonconvergence(
                "PPML published covariance repair comparison is uncertified",
            ));
        }
        repaired
    } else {
        symmetric
    };
    Ok((comparison, inspection))
}

fn symmetric_eigensystem(matrix: &[f64], dimension: usize) -> Result<SymmetricEigensystem> {
    let square = checked_square(dimension, "PPML eigensystem dimensions overflow usize")?;
    if matrix.len() != square {
        return Err(invalid_argument(
            "PPML eigensystem dimensions are inconsistent",
        ));
    }
    let mut work = fallible_copy(matrix, "unable to reserve PPML eigensystem work matrix")?;
    symmetrize(&mut work, dimension)?;
    let mut vectors = filled(square, 0.0_f64, "unable to reserve PPML eigenvectors")?;
    for index in 0..dimension {
        vectors[index * dimension + index] = 1.0;
    }
    // The modeled eigensystem peak already includes this vector beside work
    // and eigenvectors. Rotations reuse it for upper-row maxima before
    // filling the final diagonal; there is no second cache allocation.
    let mut values = filled(dimension, 0.0_f64, "unable to reserve PPML eigenvalues")?;
    for (row, maximum) in values.iter_mut().enumerate() {
        *maximum = upper_row_maximum(&work, dimension, row).1;
    }
    if dimension > 1 {
        let threshold = 128.0 * f64::EPSILON * matrix_scale(&work, dimension)?;
        let maximum_iterations = dimension
            .checked_mul(dimension)
            .and_then(|value| value.checked_mul(128))
            .ok_or_else(|| allocation("PPML Jacobi iteration limit overflows usize"))?;
        let mut converged = false;
        for _ in 0..maximum_iterations {
            let (pivot_row, pivot_column, maximum) = cached_jacobi_pivot(&work, dimension, &values);
            if maximum <= threshold {
                converged = true;
                break;
            }
            invalidate_rotated_maxima(&work, dimension, &mut values, pivot_row, pivot_column);
            jacobi_rotate(&mut work, &mut vectors, dimension, pivot_row, pivot_column)?;
            refresh_rotated_maxima(&work, dimension, &mut values, pivot_row, pivot_column);
        }
        if !converged {
            return Err(numerical_nonconvergence(
                "PPML symmetric eigensystem did not converge",
            ));
        }
    }
    for index in 0..dimension {
        values[index] = work[index * dimension + index];
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(numerical_nonconvergence("PPML eigenvalue is not finite"));
    }
    let (reconstruction_relative_error, orthogonality_relative_error) =
        certify_eigensystem(matrix, &values, &vectors, dimension)?;
    Ok(SymmetricEigensystem {
        values,
        vectors,
        reconstruction_relative_error,
        orthogonality_relative_error,
    })
}

#[cfg(test)]
fn exhaustive_jacobi_pivot(matrix: &[f64], dimension: usize) -> (usize, usize, f64) {
    let (mut pivot_row, mut pivot_column, mut maximum) = (0, 1, 0.0_f64);
    for row in 0..dimension {
        for column in (row + 1)..dimension {
            let candidate = matrix[row * dimension + column].abs();
            if candidate > maximum {
                maximum = candidate;
                pivot_row = row;
                pivot_column = column;
            }
        }
    }
    (pivot_row, pivot_column, maximum)
}

fn upper_row_maximum(matrix: &[f64], dimension: usize, row: usize) -> (usize, f64) {
    let mut column = row + 1;
    let mut maximum = 0.0_f64;
    for index in (row + 1)..dimension {
        let candidate = matrix[row * dimension + index].abs();
        if candidate > maximum {
            column = index;
            maximum = candidate;
        }
    }
    (column, maximum)
}

fn cached_jacobi_pivot(
    matrix: &[f64],
    dimension: usize,
    row_maxima: &[f64],
) -> (usize, usize, f64) {
    let (mut pivot_row, mut maximum) = (0, 0.0_f64);
    for (row, &candidate) in row_maxima.iter().enumerate() {
        if candidate > maximum {
            pivot_row = row;
            maximum = candidate;
        }
    }
    // Strict greater-than comparisons preserve the exhaustive search's first
    // row and first column on ties, including its all-zero stopping pivot.
    let (pivot_column, _) = upper_row_maximum(matrix, dimension, pivot_row);
    (pivot_row, pivot_column, maximum)
}

fn invalidate_rotated_maxima(
    matrix: &[f64],
    dimension: usize,
    row_maxima: &mut [f64],
    p: usize,
    q: usize,
) {
    for (row, maximum) in row_maxima.iter_mut().enumerate() {
        if row == p
            || row == q
            || (row < p && matrix[row * dimension + p].abs() == *maximum)
            || (row < q && matrix[row * dimension + q].abs() == *maximum)
        {
            // Magnitudes are nonnegative. The sign bit marks an invalidated
            // row without an index/flag buffer; -0.0 must also be rescanned.
            *maximum = -*maximum;
        }
    }
}

fn refresh_rotated_maxima(
    matrix: &[f64],
    dimension: usize,
    row_maxima: &mut [f64],
    p: usize,
    q: usize,
) {
    for (row, maximum) in row_maxima.iter_mut().enumerate() {
        if maximum.is_sign_negative() {
            *maximum = upper_row_maximum(matrix, dimension, row).1;
        } else {
            // Other upper rows changed only in columns p/q. Their old
            // maximum remains present, but either new entry may exceed it.
            for column in [p, q] {
                if row < column {
                    let candidate = matrix[row * dimension + column].abs();
                    if candidate > *maximum {
                        *maximum = candidate;
                    }
                }
            }
        }
    }
}

fn jacobi_rotate(
    matrix: &mut [f64],
    vectors: &mut [f64],
    dimension: usize,
    p: usize,
    q: usize,
) -> Result<()> {
    let app = matrix[p * dimension + p];
    let aqq = matrix[q * dimension + q];
    let apq = matrix[p * dimension + q];
    if apq == 0.0 {
        return Ok(());
    }
    let tau = (aqq - app) / (2.0 * apq);
    let t = if tau >= 0.0 {
        1.0 / (tau + (1.0 + tau * tau).sqrt())
    } else {
        -1.0 / (-tau + (1.0 + tau * tau).sqrt())
    };
    let cosine = 1.0 / (1.0 + t * t).sqrt();
    let sine = t * cosine;
    if !cosine.is_finite() || !sine.is_finite() {
        return Err(numerical_nonconvergence(
            "PPML Jacobi rotation is not finite",
        ));
    }
    for index in 0..dimension {
        if index == p || index == q {
            continue;
        }
        let aip = matrix[index * dimension + p];
        let aiq = matrix[index * dimension + q];
        let new_p = cosine * aip - sine * aiq;
        let new_q = sine * aip + cosine * aiq;
        matrix[index * dimension + p] = new_p;
        matrix[p * dimension + index] = new_p;
        matrix[index * dimension + q] = new_q;
        matrix[q * dimension + index] = new_q;
    }
    matrix[p * dimension + p] = app - t * apq;
    matrix[q * dimension + q] = aqq + t * apq;
    matrix[p * dimension + q] = 0.0;
    matrix[q * dimension + p] = 0.0;
    for row in 0..dimension {
        let vip = vectors[row * dimension + p];
        let viq = vectors[row * dimension + q];
        vectors[row * dimension + p] = cosine * vip - sine * viq;
        vectors[row * dimension + q] = sine * vip + cosine * viq;
    }
    Ok(())
}

fn certify_eigensystem(
    matrix: &[f64],
    values: &[f64],
    vectors: &[f64],
    dimension: usize,
) -> Result<(f64, f64)> {
    let mut reconstruction_error = 0.0_f64;
    let mut orthogonality_error = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = f64::MIN_POSITIVE;
        let mut orthogonal_row_error = 0.0_f64;
        for column in 0..dimension {
            let mut reconstructed = 0.0_f64;
            let mut reconstructed_correction = 0.0_f64;
            let mut inner_product = 0.0_f64;
            let mut inner_correction = 0.0_f64;
            for eigen in 0..dimension {
                compensated_add(
                    vectors[row * dimension + eigen]
                        * values[eigen]
                        * vectors[column * dimension + eigen],
                    &mut reconstructed,
                    &mut reconstructed_correction,
                )?;
                compensated_add(
                    vectors[row * dimension + eigen] * vectors[column * dimension + eigen],
                    &mut inner_product,
                    &mut inner_correction,
                )?;
            }
            reconstructed += reconstructed_correction;
            inner_product += inner_correction;
            row_error += (matrix[row * dimension + column] - reconstructed).abs();
            row_scale += matrix[row * dimension + column]
                .abs()
                .max(reconstructed.abs());
            orthogonal_row_error += (inner_product - f64::from(u8::from(row == column))).abs();
        }
        reconstruction_error = reconstruction_error.max(row_error / row_scale);
        orthogonality_error = orthogonality_error.max(orthogonal_row_error);
    }
    if reconstruction_error.is_finite() && orthogonality_error.is_finite() {
        Ok((reconstruction_error, orthogonality_error))
    } else {
        Err(numerical_nonconvergence(
            "PPML eigensystem certificate is not finite",
        ))
    }
}

fn reconstruct_clipped(
    eigen: &SymmetricEigensystem,
    dimension: usize,
) -> Result<(Vec<f64>, usize)> {
    let square = checked_square(
        dimension,
        "PPML clipped covariance dimensions overflow usize",
    )?;
    let mut output = filled(square, 0.0_f64, "unable to reserve PPML clipped covariance")?;
    let clipped = eigen.values.iter().filter(|value| **value < 0.0).count();
    for row in 0..dimension {
        for column in row..dimension {
            let mut value = 0.0_f64;
            let mut correction = 0.0_f64;
            for index in 0..dimension {
                compensated_add(
                    eigen.vectors[row * dimension + index]
                        * eigen.values[index].max(0.0)
                        * eigen.vectors[column * dimension + index],
                    &mut value,
                    &mut correction,
                )?;
            }
            let reconstructed = value + correction;
            if !reconstructed.is_finite() {
                return Err(numerical_nonconvergence(
                    "PPML clipped covariance is not finite",
                ));
            }
            output[row * dimension + column] = reconstructed;
            output[column * dimension + row] = reconstructed;
        }
    }
    Ok((output, clipped))
}

fn certify_clipped_reconstruction(
    eigen: &SymmetricEigensystem,
    actual: &[f64],
    dimension: usize,
) -> Result<f64> {
    let square = checked_square(
        dimension,
        "PPML clipped-certificate dimensions overflow usize",
    )?;
    if actual.len() != square || eigen.values.len() != dimension || eigen.vectors.len() != square {
        return Err(invalid_argument(
            "PPML clipped-certificate dimensions are inconsistent",
        ));
    }
    let mut error = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = f64::MIN_POSITIVE;
        for column in 0..dimension {
            let mut expected = 0.0_f64;
            let mut correction = 0.0_f64;
            for index in 0..dimension {
                compensated_add(
                    eigen.vectors[row * dimension + index]
                        * eigen.values[index].max(0.0)
                        * eigen.vectors[column * dimension + index],
                    &mut expected,
                    &mut correction,
                )?;
            }
            expected += correction;
            let observed = actual[row * dimension + column];
            row_error += (observed - expected).abs();
            row_scale += observed.abs().max(expected.abs());
        }
        error = error.max(row_error / row_scale);
    }
    if error.is_finite() {
        Ok(error)
    } else {
        Err(numerical_nonconvergence(
            "PPML clipped covariance certificate is not finite",
        ))
    }
}

fn matrix_scale(matrix: &[f64], dimension: usize) -> Result<f64> {
    let mut scale = 0.0_f64;
    for row in 0..dimension {
        let mut row_sum = 0.0_f64;
        for column in 0..dimension {
            row_sum += matrix[row * dimension + column].abs();
        }
        scale = scale.max(row_sum);
    }
    if scale.is_finite() {
        Ok(scale.max(f64::MIN_POSITIVE))
    } else {
        Err(numerical_nonconvergence(
            "PPML covariance scale is not finite",
        ))
    }
}

fn symmetrize(matrix: &mut [f64], dimension: usize) -> Result<()> {
    if matrix.len() != dimension.saturating_mul(dimension) {
        return Err(invalid_argument(
            "PPML covariance symmetry dimensions are inconsistent",
        ));
    }
    for row in 0..dimension {
        for column in (row + 1)..dimension {
            let value = 0.5 * (matrix[row * dimension + column] + matrix[column * dimension + row]);
            if !value.is_finite() {
                return Err(numerical_nonconvergence(
                    "PPML symmetrized covariance is not finite",
                ));
            }
            matrix[row * dimension + column] = value;
            matrix[column * dimension + row] = value;
        }
    }
    Ok(())
}

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    let next = *sum + value;
    if sum.abs() >= value.abs() {
        *correction += (*sum - next) + value;
    } else {
        *correction += (value - next) + *sum;
    }
    *sum = next;
    if sum.is_finite() && correction.is_finite() {
        Ok(())
    } else {
        Err(numerical_nonconvergence(
            "PPML compensated covariance accumulator is not finite",
        ))
    }
}

fn filled<T: Clone>(elements: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(message))?;
    output.resize(elements, value);
    Ok(output)
}

fn fallible_copy<T: Clone>(source: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(source.len())
        .map_err(|_| allocation(message))?;
    output.extend_from_slice(source);
    Ok(output)
}

fn checked_square(dimension: usize, message: &'static str) -> Result<usize> {
    dimension
        .checked_mul(dimension)
        .ok_or_else(|| allocation(message))
}

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn numerical_nonconvergence(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_covariance_certificate_matches_known_householder_spectrum() {
        for dimension in [1, 3, 8, 32, 48, 65] {
            let n = dimension as f64;
            let eigenvalues = (0..dimension)
                .map(|i| 1.0 + (i % 7) as f64)
                .collect::<Vec<_>>();
            let sum = eigenvalues.iter().sum::<f64>();
            // Q = I - 2 uu' / n, u = (1, ..., 1). Expanding Q D Q'
            // gives a closed-form oracle without a second eigensolver.
            let covariance = (0..dimension * dimension)
                .map(|i| {
                    let (row, column) = (i / dimension, i % dimension);
                    f64::from(u8::from(row == column)) * eigenvalues[row]
                        - 2.0 * (eigenvalues[row] + eigenvalues[column]) / n
                        + 4.0 * sum / (n * n)
                })
                .collect::<Vec<_>>();
            for clusters in [0, 1, 2] {
                let (output, report) =
                    apply_ppml_psd_policy(&covariance, dimension, clusters, 1e-10).unwrap();
                assert_eq!(output, covariance);
                assert!(!report.adjusted);
                assert_eq!(report.clipped_eigenvalues, 0);
                assert_eq!(report.final_rank, dimension);
                assert!((report.raw_minimum_eigenvalue - 1.0).abs() < 1e-11);
                assert_eq!(
                    report.final_minimum_eigenvalue,
                    report.raw_minimum_eigenvalue
                );
                let (_, inspection) =
                    inspect_published_ppml_covariance(&output, dimension, clusters, 1e-10).unwrap();
                assert_eq!(report.final_rank, inspection.rank);
                assert_eq!(
                    report.final_minimum_eigenvalue,
                    inspection.minimum_eigenvalue
                );
                assert_eq!(
                    report.eigensystem_reconstruction_relative_error,
                    inspection.reconstruction_relative_error
                );
                assert_eq!(
                    report.orthogonality_relative_error,
                    inspection.orthogonality_relative_error
                );
            }
        }
    }

    #[test]
    fn covariance_owner_preserves_tiny_negative_and_repaired_rank_policies() {
        let covariance = [-1e-12, 0.0, 0.0, 2.0];
        for clusters in [0, 1] {
            let (output, report) = apply_ppml_psd_policy(&covariance, 2, clusters, 1e-10).unwrap();
            assert_eq!(output, covariance);
            assert_eq!(report.final_minimum_eigenvalue, -1e-12);
            assert_eq!(report.final_rank, 1);
            assert!(!report.adjusted);
        }
        let (output, report) = apply_ppml_psd_policy(&covariance, 2, 2, 1e-10).unwrap();
        assert_eq!(output, [0.0, 0.0, 0.0, 2.0]);
        assert_eq!(report.raw_minimum_eigenvalue, -1e-12);
        assert_eq!(report.final_minimum_eigenvalue, 0.0);
        assert_eq!(report.final_rank, 1);
        assert_eq!(report.clipped_eigenvalues, 1);
        assert!(report.adjusted);
        let (_, inspection) = inspect_published_ppml_covariance(&output, 2, 2, 1e-10).unwrap();
        assert_eq!(
            inspection.minimum_eigenvalue,
            report.final_minimum_eigenvalue
        );
        assert_eq!(inspection.rank, report.final_rank);
    }

    #[test]
    fn cached_jacobi_pivots_match_exhaustive_search_after_rotations() {
        for dimension in [1, 2, 7, 10, 16, 32, 48, 63, 64, 65, 129, 257] {
            for pattern in 0..4 {
                let scale = match pattern {
                    0 => 2.0_f64.powi(-400),
                    1 => 2.0_f64.powi(400),
                    _ => 1.0,
                };
                let mut matrix = vec![0.0; dimension * dimension];
                let mut vectors = vec![0.0; dimension * dimension];
                for row in 0..dimension {
                    vectors[row * dimension + row] = 1.0;
                    matrix[row * dimension + row] = (row + 1) as f64 * scale;
                    for column in (row + 1)..dimension {
                        let value = match pattern {
                            0 => ((row * 31 + column * 17) % 29) as f64 - 14.0,
                            1 => {
                                if (row + column) % 3 == 0 {
                                    0.25
                                } else {
                                    -0.25
                                }
                            }
                            2 => {
                                if row == 0 && column == dimension - 1 {
                                    0.5
                                } else {
                                    -0.0
                                }
                            }
                            _ => -0.0,
                        } * scale;
                        matrix[row * dimension + column] = value;
                        matrix[column * dimension + row] = value;
                    }
                }
                let mut maxima = (0..dimension)
                    .map(|row| upper_row_maximum(&matrix, dimension, row).1)
                    .collect::<Vec<_>>();
                let capacity = maxima.capacity();
                for _ in 0..512 {
                    let expected = exhaustive_jacobi_pivot(&matrix, dimension);
                    let cached = cached_jacobi_pivot(&matrix, dimension, &maxima);
                    assert_eq!(cached, expected, "dimension={dimension}, pattern={pattern}");
                    if expected.2 == 0.0 {
                        break;
                    }
                    invalidate_rotated_maxima(
                        &matrix,
                        dimension,
                        &mut maxima,
                        expected.0,
                        expected.1,
                    );
                    if pattern == 2 && dimension > 2 {
                        assert!(maxima
                            .iter()
                            .any(|value| *value == 0.0 && value.is_sign_negative()));
                    }
                    jacobi_rotate(&mut matrix, &mut vectors, dimension, expected.0, expected.1)
                        .unwrap();
                    refresh_rotated_maxima(&matrix, dimension, &mut maxima, expected.0, expected.1);
                    for (row, &maximum) in maxima.iter().enumerate() {
                        assert_eq!(maximum, upper_row_maximum(&matrix, dimension, row).1);
                        assert!(!maximum.is_sign_negative());
                    }
                    assert_eq!(maxima.capacity(), capacity);
                }
            }
        }
    }

    #[test]
    fn published_psd_inspection_rejects_invalid_inputs_without_mutation() {
        let positive = [2.0_f64, -0.5, -0.5, 1.0];
        for clusters in [0, 1, 2, 5] {
            let (comparison, inspection) =
                inspect_published_ppml_covariance(&positive, 2, clusters, 1e-10).unwrap();
            assert_eq!(comparison, positive);
            assert_eq!(inspection.rank, 2);
            assert!(inspection.minimum_eigenvalue > 0.0);
            assert!(
                inspect_published_ppml_covariance(&[1.0, 2.0, 2.0, 1.0], 2, clusters, 1e-10)
                    .is_err()
            );
        }
        for (values, dimension, tolerance) in [
            (vec![1.0, f64::NAN, 0.0, 1.0], 2, 1e-10),
            (vec![1.0], 2, 1e-10),
            (vec![1.0], 1, 0.0),
            (vec![1.0], 1, f64::INFINITY),
            (vec![], usize::MAX, 1e-10),
        ] {
            assert!(inspect_published_ppml_covariance(&values, dimension, 2, tolerance).is_err());
        }
        let (empty, inspection) = inspect_published_ppml_covariance(&[], 0, 2, 1e-10).unwrap();
        assert!(empty.is_empty());
        assert_eq!(inspection.rank, 0);
        assert_eq!(inspection.minimum_eigenvalue, 0.0);
    }

    #[test]
    fn robust_and_one_way_paths_reject_material_indefiniteness() {
        for cluster_dimensions in [0, 1] {
            let error =
                apply_ppml_psd_policy(&[1.0_f64, 2.0, 2.0, 1.0], 2, cluster_dimensions, 1e-10)
                    .expect_err("strict covariance must be PSD");
            assert_eq!(error.code, ErrorCode::NonConvergence);
        }
    }

    #[test]
    fn multiway_policy_clips_only_after_the_complete_covariance() {
        let (repaired, report) =
            apply_ppml_psd_policy(&[1.0_f64, 2.0, 2.0, 1.0], 2, 2, 1e-10).expect("multiway repair");
        assert_eq!(report.state, PpmlPsdState::MultiwayClipped);
        assert!(report.adjusted);
        assert_eq!(report.clipped_eigenvalues, 1);
        assert!(report.raw_minimum_eigenvalue < 0.0);
        assert!(report.final_minimum_eigenvalue >= -1e-10);
        assert!(report.passed);
        assert!((repaired[0] - 1.5).abs() < 1e-12);
        assert!((repaired[1] - 1.5).abs() < 1e-12);
        assert!((repaired[2] - 1.5).abs() < 1e-12);
        assert!((repaired[3] - 1.5).abs() < 1e-12);
    }

    #[test]
    fn positive_semidefinite_multiway_covariance_is_not_changed() {
        let covariance = [2.0_f64, -0.5, -0.5, 1.0];
        let (actual, report) =
            apply_ppml_psd_policy(&covariance, 2, 3, 1e-10).expect("PSD covariance");
        assert_eq!(actual, covariance);
        assert_eq!(report.state, PpmlPsdState::MultiwayRaw);
        assert!(!report.adjusted);
        assert!(report.passed);
    }
}
