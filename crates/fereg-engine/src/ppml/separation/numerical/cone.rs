use super::{Controls, DirectionCertificate};
use crate::{
    error::{reserved, ErrorCode, FeregError, Result},
    ppml::separation::View,
};

pub(super) fn claim(
    view: View<'_>,
    direction: &[f64],
    controls: Controls,
) -> Result<(Vec<usize>, DirectionCertificate)> {
    controls.validate()?;
    if direction.len() != view.sample.n_active() || direction.iter().any(|v| !v.is_finite()) {
        return Err(failed(
            "separation direction is not finite and sample aligned",
        ));
    }
    let scale = direction.iter().fold(0_f64, |max, v| max.max(v.abs()));
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
    Ok((rows, certificate))
}
pub(super) fn failed(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
