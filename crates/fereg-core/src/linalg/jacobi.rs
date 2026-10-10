//! Jacobi eigen kernels shared by the OLS clustered and PPML PSD policies.
//!
//! Each kernel performs one fixed operation sequence. Callers keep their own
//! pivot strategy, certificate floor, compensated accumulator and error class.
use crate::error::Result;

/// Annihilate `matrix[p][q]` with one symmetric Jacobi rotation and apply it
/// to the eigenvector columns. Returns false, leaving both matrices
/// untouched, when the rotation is not finite.
pub(crate) fn rotate(
    matrix: &mut [f64],
    vectors: &mut [f64],
    dimension: usize,
    p: usize,
    q: usize,
) -> bool {
    let app = matrix[p * dimension + p];
    let aqq = matrix[q * dimension + q];
    let apq = matrix[p * dimension + q];
    if apq == 0.0 {
        return true;
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
        return false;
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
    true
}

/// Row-relative reconstruction error of `V diag(values) V'` against `matrix`
/// and row orthogonality error of `V'V`. Each row's scale starts at
/// `row_scale_floor`; `add` is the caller's compensated accumulator.
pub(crate) fn eigensystem_errors(
    matrix: &[f64],
    values: &[f64],
    vectors: &[f64],
    dimension: usize,
    row_scale_floor: f64,
    add: impl Fn(f64, &mut f64, &mut f64) -> Result<()>,
) -> Result<(f64, f64)> {
    let mut reconstruction_error = 0.0_f64;
    let mut orthogonality_error = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = row_scale_floor;
        let mut orthogonal_row_error = 0.0_f64;
        for column in 0..dimension {
            let mut reconstructed = 0.0_f64;
            let mut reconstructed_correction = 0.0_f64;
            let mut inner_product = 0.0_f64;
            let mut inner_correction = 0.0_f64;
            for eigen in 0..dimension {
                add(
                    vectors[row * dimension + eigen]
                        * values[eigen]
                        * vectors[column * dimension + eigen],
                    &mut reconstructed,
                    &mut reconstructed_correction,
                )?;
                add(
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
    Ok((reconstruction_error, orthogonality_error))
}

/// Entry `(row, column)` of `V diag(max(values, 0)) V'`, accumulated with the
/// caller's compensated accumulator.
pub(crate) fn clipped_entry(
    vectors: &[f64],
    values: &[f64],
    dimension: usize,
    row: usize,
    column: usize,
    add: impl Fn(f64, &mut f64, &mut f64) -> Result<()>,
) -> Result<f64> {
    let mut value = 0.0_f64;
    let mut correction = 0.0_f64;
    for index in 0..dimension {
        add(
            vectors[row * dimension + index]
                * values[index].max(0.0)
                * vectors[column * dimension + index],
            &mut value,
            &mut correction,
        )?;
    }
    Ok(value + correction)
}
