use super::{failed, SimplexDualCertificate};
use crate::{error::Result, threads::compensated_sum};

/// Farkas for A c - s = 0, 1's = 1, c free, s >= 0. The tableau is not an
/// acceptance source: check A'u=0, v-u_i>=0 and v<0 against the immutable cone.
pub(super) fn verify(
    cone: &[Vec<f64>],
    dual: &[f64],
    tolerance: f64,
) -> Result<SimplexDualCertificate> {
    let boundary = cone.len();
    let nullity = cone.first().map_or(0, Vec::len);
    if boundary == 0
        || nullity == 0
        || dual.len() != boundary + 1
        || cone.iter().any(|row| row.len() != nullity)
        || cone
            .iter()
            .flat_map(|row| row.iter())
            .chain(dual)
            .any(|v| !v.is_finite())
    {
        return Err(failed(
            "simplex dual is not finite and dimensionally aligned",
        ));
    }
    let scale = dual.iter().fold(0_f64, |s, v| s.max(v.abs()));
    if scale <= f64::MIN_POSITIVE {
        return Err(failed("simplex dual has no representable nonzero scale"));
    }
    let v = dual[boundary] / scale;
    let stationarity_relative = (0..nullity)
        .map(|column| {
            compensated_sum((0..boundary).map(|row| cone[row][column] * (dual[row] / scale))).abs()
        })
        .fold(0_f64, f64::max);
    let inequality_violation_relative = dual[..boundary]
        .iter()
        .map(|u| (u / scale - v).max(0.))
        .fold(0_f64, f64::max);
    let infeasibility_margin_relative = -v;
    if !stationarity_relative.is_finite()
        || stationarity_relative > tolerance
        || inequality_violation_relative > tolerance
        || infeasibility_margin_relative < tolerance
    {
        return Err(failed(
            "simplex infeasibility failed its independent Farkas certificate",
        ));
    }
    Ok(SimplexDualCertificate {
        boundary_rows: boundary,
        nullity,
        stationarity_relative,
        inequality_violation_relative,
        infeasibility_margin_relative,
        scale,
    })
}
