//! Shared finite-slope Wald solve; callers own estimator-specific coordinate selection.

use super::prepared_scores::{reserved_filled, PreparedInferenceReservation};
use super::prepared_unadjusted::{checked_square, compensated_add, exact_integer_as_f64};
use super::{CertifiedPreparedOlsFit, PreparedOlsFitStatus};
use crate::error::{invalid_argument, invalid_data, Result};

#[derive(Debug, Clone, Copy)]
pub(super) struct WaldCalculation {
    pub(super) statistic: Option<f64>,
    pub(super) solve_relative_infinity_norm: f64,
}

/// Pivot and certificate rules of the Wald solve. They stay distinct by
/// estimator on purpose (SPEC section 11).
#[derive(Debug, Clone, Copy)]
pub(super) enum WaldPolicy {
    /// IID: the slope covariance is s^2 B on rank-certified coordinates, so a
    /// nonpositive pivot contradicts the rank certificate and is fatal.
    IidRequiresPositive,
    /// Robust and clustered: a singular meat (for example G < K) leaves the
    /// statistic undefined. A pivot within this tolerance of the largest slope
    /// variance reports a missing F; a clearly negative pivot is indefinite.
    ScaledSingularTolerance(f64),
}

// Reconstruct V * solution independently after solving; a factorization alone
// does not certify the reported quadratic.
pub(super) fn calculate_wald<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    values: &[f64],
    dimension: usize,
    positions: &[usize],
    policy: WaldPolicy,
    reservation: &mut R,
) -> Result<WaldCalculation> {
    let iid = matches!(policy, WaldPolicy::IidRequiresPositive);
    let finite = |value: f64, message: &'static str| {
        if iid && !value.is_finite() {
            Err(invalid_data(message))
        } else {
            Ok(value)
        }
    };
    let slope_dimension = positions.len();
    let square = checked_square(slope_dimension, "prepared Wald dimensions overflow usize")?;
    let mut factor = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared Wald factor",
    )?;
    let threshold = match policy {
        WaldPolicy::IidRequiresPositive => 0.0,
        WaldPolicy::ScaledSingularTolerance(tolerance) => {
            let covariance_scale = positions
                .iter()
                .map(|position| values[*position * dimension + *position].abs())
                .fold(0.0_f64, f64::max)
                .max(f64::MIN_POSITIVE);
            tolerance * covariance_scale
        }
    };
    for row in 0..slope_dimension {
        for column in 0..=row {
            let mut value = values[positions[row] * dimension + positions[column]];
            for inner in 0..column {
                value -= factor[row * slope_dimension + inner]
                    * factor[column * slope_dimension + inner];
            }
            if row == column {
                if iid {
                    if !value.is_finite() || value <= 0.0 {
                        return Err(invalid_data(
                            "prepared iid Wald covariance is not positive definite",
                        ));
                    }
                } else if value < -threshold {
                    return Err(invalid_data("prepared Wald slope covariance is indefinite"));
                } else if value <= threshold {
                    return Ok(WaldCalculation {
                        statistic: None,
                        solve_relative_infinity_norm: 0.0,
                    });
                }
                factor[row * slope_dimension + column] = value.sqrt();
            } else {
                factor[row * slope_dimension + column] = finite(
                    value / factor[column * slope_dimension + column],
                    "prepared iid Wald Cholesky factor is not finite",
                )?;
            }
        }
    }
    let mut solution = reserved_filled(
        reservation,
        slope_dimension,
        0.0_f64,
        "unable to reserve prepared Wald solution",
    )?;
    for (slope, position) in positions.iter().copied().enumerate() {
        solution[slope] = fitted_coordinate_value(fit, position)?;
    }
    for row in 0..slope_dimension {
        let mut value = solution[row];
        for column in 0..row {
            value -= factor[row * slope_dimension + column] * solution[column];
        }
        solution[row] = finite(
            value / factor[row * slope_dimension + row],
            "prepared iid Wald forward solve is not finite",
        )?;
    }
    for row in (0..slope_dimension).rev() {
        let mut value = solution[row];
        for column in (row + 1)..slope_dimension {
            value -= factor[column * slope_dimension + row] * solution[column];
        }
        solution[row] = finite(
            value / factor[row * slope_dimension + row],
            "prepared iid Wald backward solve is not finite",
        )?;
    }
    let mut quadratic = 0.0_f64;
    let mut quadratic_correction = 0.0_f64;
    let mut solve_error = 0.0_f64;
    for row in 0..slope_dimension {
        let coefficient = fitted_coordinate_value(fit, positions[row])?;
        compensated_add(
            coefficient * solution[row],
            &mut quadratic,
            &mut quadratic_correction,
        )?;
        let mut fitted = 0.0_f64;
        let mut correction = 0.0_f64;
        // IID keeps a purely relative row scale; robust/clustered floor it at 1.
        let mut scale = if iid {
            coefficient.abs()
        } else {
            coefficient.abs().max(1.0)
        };
        for column in 0..slope_dimension {
            let value = values[positions[row] * dimension + positions[column]] * solution[column];
            compensated_add(value, &mut fitted, &mut correction)?;
            scale = finite(
                scale + value.abs(),
                "prepared iid Wald solve scale is not finite",
            )?;
        }
        let residual = (fitted + correction - coefficient).abs();
        solve_error = solve_error.max(residual / scale.max(f64::MIN_POSITIVE));
    }
    quadratic += quadratic_correction;
    if !quadratic.is_finite() || quadratic < 0.0 || !solve_error.is_finite() {
        return Err(invalid_data(
            "prepared Wald quadratic or solve certificate is invalid",
        ));
    }
    let statistic = quadratic / exact_integer_as_f64(slope_dimension as u128)?;
    if !statistic.is_finite() {
        return Err(invalid_data("prepared Wald statistic is not finite"));
    }
    Ok(WaldCalculation {
        statistic: Some(statistic),
        solve_relative_infinity_norm: solve_error,
    })
}

pub(super) fn fitted_coordinate_value(
    fit: &CertifiedPreparedOlsFit,
    position: usize,
) -> Result<f64> {
    let coordinate = fit
        .bread()
        .coordinates()
        .get(position)
        .ok_or_else(|| invalid_argument("prepared Wald fitted coordinate is out of range"))?;
    let fitted = fit
        .coefficients()
        .iter()
        .find(|coefficient| {
            coefficient.generated_column_index == Some(coordinate.generated_column_index)
        })
        .ok_or_else(|| invalid_argument("prepared Wald fitted coordinate has no coefficient"))?;
    if fitted.status != PreparedOlsFitStatus::Kept {
        return Err(invalid_argument(
            "prepared Wald coordinate is not a kept finite coefficient",
        ));
    }
    fitted
        .value
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid_data("prepared Wald coefficient is not finite"))
}
