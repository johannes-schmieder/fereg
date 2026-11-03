//! Eigensolver accuracy for reported covariance after exceptional rank repair.
use super::*;

/// The existing certificate checks every original row relative to that row's
/// magnitude. A global stopping scale can ignore a coupling in a small row
/// when an absorbed-intercept covariance is much larger. Use the smallest
/// positive original row maximum, without changing coordinates or clipping.
///
/// Zero rows impose no scale. If the threshold underflows to zero, the bounded
/// Jacobi loop must annihilate every off-diagonal entry or fail; the unchanged
/// reconstruction and orthogonality certificates remain the final authority.
pub(super) fn stopping_scale(matrix: &[f64], dimension: usize) -> Result<f64> {
    let square = checked_square(
        dimension,
        "quotient covariance stopping dimensions overflow usize",
    )?;
    if matrix.len() != square {
        return Err(invalid_argument(
            "quotient covariance stopping dimensions are inconsistent",
        ));
    }
    if dimension == 0 {
        return Ok(0.0);
    }
    let mut smallest = f64::INFINITY;
    for row in matrix.chunks_exact(dimension) {
        crate::interrupt::checkpoint()?;
        let mut largest = 0.0_f64;
        for value in row {
            if !value.is_finite() {
                return Err(invalid_data(
                    "quotient covariance stopping scale is not finite",
                ));
            }
            largest = largest.max(value.abs());
        }
        if largest > 0.0 {
            smallest = smallest.min(largest);
        }
    }
    Ok(if smallest.is_infinite() {
        0.0
    } else {
        smallest
    })
}

#[cfg(test)]
mod tests;
