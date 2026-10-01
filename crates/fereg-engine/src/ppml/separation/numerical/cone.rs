use super::{Controls, DirectionCertificate};
use crate::{
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
    ppml::separation::View,
};

pub(super) fn claim(
    view: View<'_>,
    direction: &[f64],
    controls: Controls,
    poll: Poll<'_>,
) -> Result<(Vec<usize>, DirectionCertificate)> {
    controls.validate()?;
    execution::check(poll)?;
    if direction.len() != view.sample.n_active()
        || execution::any(direction, |v| !v.is_finite(), poll)?
    {
        return Err(failed(
            "separation direction is not finite and sample aligned",
        ));
    }
    let scale = execution::fold(direction, 0_f64, poll, |max, v| max.max(v.abs()))?;
    if scale <= f64::MIN_POSITIVE {
        return Err(failed(
            "separation direction has no representable nonzero scale",
        ));
    }
    let threshold = controls.tolerance * scale;
    let mut rows = reserved(direction.len())?;
    let mut interior = 0_f64;
    let mut positive = 0_f64;
    let mut margin = f64::INFINITY;
    for (row, (&y, &value)) in view.sample.outcome.iter().zip(direction).enumerate() {
        execution::check_chunk(poll, row)?;
        if y > 0. {
            interior = interior.max(value.abs() / scale);
        } else {
            positive = positive.max(value);
            if value < -threshold {
                rows.push(view.sample.retained_rows()[row]);
                margin = margin.min(-value / scale);
            }
        }
    }
    let certificate = DirectionCertificate {
        interior_zero_relative: interior,
        boundary_violation_relative: positive / scale,
        negative_margin_relative: margin,
        direction_scale: scale,
        positive_maximum: positive,
    };
    if rows.is_empty()
        || interior > controls.certificate_tolerance
        || positive > controls.certificate_tolerance * scale
        || margin < controls.certificate_tolerance
    {
        return Err(failed("separation direction failed its normalized equality, sign or strict-negative certificate"));
    }
    execution::check(poll)?;
    Ok((rows, certificate))
}
pub(super) fn failed(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
