//! Last-resort re-certification of a rejected two-FE detector candidate.
//!
//! The categorical FE certificate (shared by every solver) forms each level
//! mean as the first member's value plus a compensated weighted sum of
//! differences from it. When that member is a zero-outcome rectifier row,
//! whose weight is `tolerance^2 / zeros` (about 1e-18) but whose residual is
//! of order one, the positive rows' residuals (about 1e-18) vanish in their
//! differences, and the computed mean carries a rounding floor of order one
//! ulp of the shifted value. Against the rectifier's weighted residual norm
//! this floor reaches the 1e-8 tolerance, so an accurate projection can be
//! rejected (seeded census seed 3: exact moments 1.2e-18, computed 4.25e-8).
//!
//! Here each level's mean is recomputed about its largest-weight member
//! instead (the first such member on ties), with the same compensated sums,
//! the same per-RHS scales and the same tolerance. A rectifier level's
//! largest weight is a positive-outcome row whenever the level has one, so
//! the differences that carry its moment are no longer absorbed by an
//! order-one shift. This is only a re-check: the detector consults it only
//! after its Auto, retained-CG and LSMR attempts all failed a numerical
//! certificate ([`super::retry`]), on a re-executed Auto step whose two-FE
//! PCG miss would otherwise return `NonConvergence`. A fit that completes
//! without it is unchanged bit for bit.
use crate::data::DenseMatrix;
use crate::error::{invalid_argument, Result};
use crate::fe::{BorrowedInterceptFeDesign, FeCertificate};
use crate::linalg::neumaier_add;

/// The FE certificate of `values` with every level mean formed about the
/// level's largest-weight member. `weights` are raw weights (`None` is unit),
/// normalized by `raw_scale` as in the shared certificate; `scales` holds one
/// certificate scale per column. A nonfinite or unrepresentable moment fails
/// the re-check instead of raising an error.
pub(super) fn largest_weight_certificate(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: Option<&[f64]>,
    raw_scale: f64,
    values: &DenseMatrix,
    scales: &[f64],
    tolerance: f64,
) -> Result<FeCertificate> {
    let (rows, columns) = (values.nrows(), values.ncols());
    if design.nobs() != rows
        || scales.len() != columns
        || weights.is_some_and(|w| w.len() != rows)
        || !(raw_scale.is_finite() && raw_scale > 0.0)
    {
        return Err(invalid_argument(
            "separation re-certification inputs are inconsistent",
        ));
    }
    let data = values.as_slice();
    let weight = |row: usize| weights.map_or(1.0, |w| w[row] / raw_scale);
    let mut certificate = FeCertificate {
        absolute_infinity_norm: 0.0,
        relative_infinity_norm: 0.0,
        relative_two_norm: 0.0,
        passed: false,
    };
    let (mut square_sum, mut count) = (0.0_f64, 0_usize);
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let members = effect.level_index().checked_members(level)?;
            let Some(&reference) = members.iter().reduce(|best, row| {
                if weight(*row) > weight(*best) {
                    row
                } else {
                    best
                }
            }) else {
                return Err(invalid_argument(
                    "separation re-certification level is empty",
                ));
            };
            let (mut total, mut total_correction) = (0.0, 0.0);
            for &row in members {
                neumaier_add(weight(row), &mut total, &mut total_correction);
            }
            let total = total + total_correction;
            if !(total.is_finite() && total > 0.0) {
                return Ok(certificate);
            }
            for column in 0..columns {
                let shift = data[reference * columns + column];
                let (mut sum, mut correction) = (0.0, 0.0);
                for &row in members {
                    let difference = data[row * columns + column] - shift;
                    neumaier_add(weight(row) * difference, &mut sum, &mut correction);
                }
                let mean = shift + (sum + correction) / total;
                let relative = match scales[column] {
                    _ if mean == 0.0 => 0.0,
                    scale if scale > 0.0 => mean.abs() / scale * total.sqrt(),
                    _ => f64::INFINITY,
                };
                let absolute = mean.abs() * total * raw_scale;
                if !(relative.is_finite() && absolute.is_finite()) {
                    return Ok(certificate);
                }
                certificate.relative_infinity_norm =
                    certificate.relative_infinity_norm.max(relative);
                certificate.absolute_infinity_norm =
                    certificate.absolute_infinity_norm.max(absolute);
                square_sum += relative * relative;
                count += 1;
            }
        }
    }
    certificate.relative_two_norm = if count == 0 {
        0.0
    } else {
        (square_sum / count as f64).sqrt()
    };
    certificate.passed = certificate.relative_two_norm.is_finite()
        && certificate.relative_infinity_norm <= tolerance;
    Ok(certificate)
}

#[cfg(test)]
mod tests;
