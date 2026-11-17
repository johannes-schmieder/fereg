//! Verified inverse residual bounds the norm of the full-span projection.
use super::*;

pub(super) fn condition_certified<I: Iterator<Item = usize> + Clone>(
    rows: I,
    channels: usize,
    rank: usize,
    value: &impl Fn(usize, usize) -> f64,
    bound: f64,
    scratch: &mut AuditScratch,
) -> Result<bool> {
    if rank == 0 {
        return Ok(true);
    }
    let n = rows.clone().count();
    let eps = f64::EPSILON;
    // Exact expansion evaluation is followed by at most 2D additions and two
    // divisions. Compensated products/moments contribute first-order product
    // error and second-order accumulation error. Inflate the norm bounds for
    // the positive scaled-square reduction separately.
    let eta = (8. * channels as f64 + 32.) * eps + (8. * n as f64 + 32.) * eps * eps;
    let norm_error = (n as f64 + 16.) * eps;
    if eta >= 0.125 || norm_error >= 0.125 {
        return Ok(false);
    }
    let underflow = n as f64 * f64::MIN_POSITIVE;
    let gram_error = eta * (1. + eta).powi(2) / (1. - norm_error).powi(2) + underflow;
    let square = rank * rank;
    let AuditScratch {
        bank,
        scales,
        norms,
        moments,
        absolute_moments,
        gram,
        factor,
        inverse,
        expansion,
        ..
    } = scratch;
    for left in 0..rank {
        crate::interrupt::checkpoint()?;
        for right in 0..=left {
            let (mut sum, mut correction) = (0., 0.);
            for (position, row) in rows.clone().enumerate() {
                crate::interrupt::row_checkpoint(position)?;
                let Some(a) = evaluate_values(
                    &bank[left * channels..(left + 1) * channels],
                    value,
                    row,
                    expansion,
                ) else {
                    return Ok(false);
                };
                let Some(b) = evaluate_values(
                    &bank[right * channels..(right + 1) * channels],
                    value,
                    row,
                    expansion,
                ) else {
                    return Ok(false);
                };
                let Some(a) = normalized_value(a, scales[left], norms[left]) else {
                    return Ok(false);
                };
                let Some(b) = normalized_value(b, scales[right], norms[right]) else {
                    return Ok(false);
                };
                neumaier_add(a * b, &mut sum, &mut correction);
            }
            if !(sum + correction).is_finite() {
                return Ok(false);
            }
            gram[left * rank + right] = sum + correction;
            gram[right * rank + left] = sum + correction;
        }
    }
    factor[..square].copy_from_slice(&gram[..square]);
    inverse[..square].fill(0.);
    for i in 0..rank {
        inverse[i * rank + i] = 1.;
    }
    let solved = crate::linalg::solve_linear_system(
        &mut factor[..square],
        &mut inverse[..square],
        rank,
        rank,
        eps * 8. * rank as f64,
    )?;
    if solved != rank || inverse[..square].iter().any(|v| !v.is_finite()) {
        return Ok(false);
    }
    let arithmetic = (4. * rank as f64 + 16.) * eps;
    let (mut inverse_norm, mut residual_norm) = (0.0_f64, 0.0_f64);
    for i in 0..rank {
        crate::interrupt::checkpoint()?;
        let row_norm: f64 = inverse[i * rank..(i + 1) * rank]
            .iter()
            .map(|v| v.abs())
            .sum();
        inverse_norm = inverse_norm.max(row_norm * (1. + arithmetic));
        let mut residual_row = 0.;
        for j in 0..rank {
            let (mut product, mut correction, mut absolute, mut column_norm) = (0., 0., 0., 0.);
            for k in 0..rank {
                let entry = gram[i * rank + k] * inverse[k * rank + j];
                if !entry.is_finite() {
                    return Ok(false);
                }
                neumaier_add(entry, &mut product, &mut correction);
                absolute += entry.abs();
                column_norm += inverse[k * rank + j].abs();
            }
            let diagonal = f64::from(i == j);
            residual_row += (diagonal - (product + correction)).abs()
                + arithmetic * (absolute + diagonal)
                + gram_error * column_norm * (1. + arithmetic);
        }
        if !residual_row.is_finite() {
            return Ok(false);
        }
        residual_norm = residual_norm.max(residual_row * (1. + arithmetic));
    }
    if !residual_norm.is_finite() || residual_norm >= 0.5 || !inverse_norm.is_finite() {
        return Ok(false);
    }
    // ||G_true^-1||_infinity <= ||H||_infinity/(1-||I-G_true H||_infinity).
    // G_true is a Gram and hence symmetric positive definite once this bound
    // proves invertibility. Therefore its spectral norm obeys the same bound.
    let inverse_bound = inverse_norm / (1. - residual_norm);
    let mut moment_bound = 0.0_f64;
    for i in 0..rank {
        if !moments[i].is_finite()
            || !absolute_moments[i].is_finite()
            || absolute_moments[i] < 0.
            || !norms[i].is_finite()
            || norms[i] <= 0.
        {
            return Ok(false);
        }
        let upper = (moments[i].abs() + eta * absolute_moments[i] + underflow) / norms[i];
        if !upper.is_finite() {
            return Ok(false);
        }
        moment_bound = moment_bound.max(upper);
    }
    let projected_bound = moment_bound * (rank as f64 * inverse_bound).sqrt() * (1. + arithmetic);
    Ok(projected_bound.is_finite() && projected_bound <= bound)
}
