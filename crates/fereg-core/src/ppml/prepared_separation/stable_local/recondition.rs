//! Recondition an already complete triangular source-coordinate basis.
use super::*;

pub(super) fn recondition<I: Iterator<Item = usize> + Clone>(
    rows: I,
    channels: usize,
    rank: usize,
    value: &impl Fn(usize, usize) -> f64,
    dual: &[f64],
    scratch: &mut AuditScratch,
) -> Result<bool> {
    let AuditScratch {
        bank,
        scales,
        norms,
        moments,
        absolute_moments,
        expansion,
        ..
    } = scratch;
    for index in 0..rank {
        crate::interrupt::checkpoint()?;
        let (previous, rest) = bank.split_at_mut(index * channels);
        let current = &mut rest[..channels];
        // Earlier rows have zero coefficients at and beyond this row's
        // original leading column. FMA subtraction leaves its leading 1
        // unchanged, even when multipliers round. All earlier source columns
        // already lie in the preceding evaluated span, including exactly
        // dependent columns omitted during extraction. Hence these rounded
        // updates preserve the complete evaluated span by induction. Only
        // evaluated values, never coefficients, normalize.
        for _ in 0..2 {
            for (prior, basis) in previous.chunks_exact(channels).enumerate() {
                crate::interrupt::checkpoint()?;
                let (mut dot, mut correction) = (0., 0.);
                for (position, row) in rows.clone().enumerate() {
                    crate::interrupt::row_checkpoint(position)?;
                    let Some(a) = evaluate_values(current, value, row, expansion)
                        .and_then(|v| normalized_value(v, scales[index], 1.))
                    else {
                        return Ok(false);
                    };
                    let Some(b) = evaluate_values(basis, value, row, expansion)
                        .and_then(|v| normalized_value(v, scales[prior], norms[prior]))
                    else {
                        return Ok(false);
                    };
                    neumaier_add(a * b, &mut dot, &mut correction);
                }
                let multiplier =
                    ((dot + correction) * scales[index] / scales[prior]) / norms[prior];
                if !multiplier.is_finite() {
                    return Ok(false);
                }
                for (channel, (coefficient, &basis)) in current.iter_mut().zip(basis).enumerate() {
                    crate::interrupt::row_checkpoint(channel)?;
                    *coefficient = (-multiplier).mul_add(basis, *coefficient);
                    if !coefficient.is_finite() {
                        return Ok(false);
                    }
                }
            }
        }
        let mut scale = 0.0_f64;
        for (position, row) in rows.clone().enumerate() {
            crate::interrupt::row_checkpoint(position)?;
            let Some(v) = evaluate_values(current, value, row, expansion) else {
                return Ok(false);
            };
            scale = scale.max(v.abs());
        }
        if scale == 0. {
            return Ok(false);
        }
        let mut norm = ScaledSquares::ZERO;
        let (mut moment, mut correction, mut absolute, mut absolute_correction) = (0., 0., 0., 0.);
        for (position, row) in rows.clone().enumerate() {
            crate::interrupt::row_checkpoint(position)?;
            let Some(v) = evaluate_values(current, value, row, expansion)
                .and_then(|v| normalized_value(v, scale, 1.))
            else {
                return Ok(false);
            };
            norm.add(v.abs());
            neumaier_add(v * dual[row], &mut moment, &mut correction);
            neumaier_add(
                (v * dual[row]).abs(),
                &mut absolute,
                &mut absolute_correction,
            );
        }
        scales[index] = scale;
        norms[index] = norm.norm();
        moments[index] = moment + correction;
        absolute_moments[index] = absolute + absolute_correction;
    }
    Ok(true)
}
