use super::{failed, SimplexDualCertificate};
use crate::{
    error::Result,
    execution::{self, Poll, WorkPoll},
};

/// Farkas for A c - s = 0, 1's = 1, c free, s >= 0. The tableau is not an
/// acceptance source: check A'u=0, v-u_i>=0 and v<0 against the immutable cone.
pub(super) fn verify(
    cone: &[Vec<f64>],
    dual: &[f64],
    tolerance: f64,
    poll: Poll<'_>,
) -> Result<SimplexDualCertificate> {
    let mut work = WorkPoll::new(poll)?;
    let boundary = cone.len();
    let nullity = cone.first().map_or(0, Vec::len);
    if boundary == 0
        || nullity == 0
        || dual.len() != boundary + 1
        || execution::any(cone, |row| row.len() != nullity, poll)?
        || execution::any(
            cone.iter().flat_map(|row| row.iter()).chain(dual),
            |v| !v.is_finite(),
            poll,
        )?
    {
        return Err(failed(
            "simplex dual is not finite and dimensionally aligned",
        ));
    }
    let scale = execution::fold(dual, 0_f64, poll, |s, v| s.max(v.abs()))?;
    if scale <= f64::MIN_POSITIVE {
        return Err(failed("simplex dual has no representable nonzero scale"));
    }
    let v = dual[boundary] / scale;
    let mut stationarity_relative = 0_f64;
    for column in 0..nullity {
        let value = execution::sum(
            (0..boundary).map(|row| cone[row][column] * (dual[row] / scale)),
            poll,
        )?
        .abs();
        work.advance(boundary)?;
        stationarity_relative = stationarity_relative.max(value);
    }
    let inequality_violation_relative = execution::fold(
        dual[..boundary].iter().map(|u| (u / scale - v).max(0.)),
        0_f64,
        poll,
        f64::max,
    )?;
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
    execution::check(poll)?;
    Ok(SimplexDualCertificate {
        boundary_rows: boundary,
        nullity,
        stationarity_relative,
        inequality_violation_relative,
        infeasibility_margin_relative,
        scale,
    })
}
