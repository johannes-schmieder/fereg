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
    if !fill_gram(rows, channels, rank, value, scratch)? {
        return Ok(false);
    }
    let AuditScratch {
        norms,
        moments,
        absolute_moments,
        gram,
        factor,
        inverse,
        ..
    } = scratch;
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

/// Stream rows once while preserving each Gram entry's original summation
/// order. The unused inverse row holds normalized basis values; factor holds
/// compensation until it is overwritten by the solver's Gram copy.
fn fill_gram<I: Iterator<Item = usize>>(
    rows: I,
    channels: usize,
    rank: usize,
    value: &impl Fn(usize, usize) -> f64,
    scratch: &mut AuditScratch,
) -> Result<bool> {
    let AuditScratch {
        bank,
        scales,
        norms,
        gram,
        factor,
        inverse,
        expansion,
        ..
    } = scratch;
    let square = rank * rank;
    gram[..square].fill(0.);
    factor[..square].fill(0.);
    for (position, row) in rows.enumerate() {
        crate::interrupt::row_checkpoint(position)?;
        for basis in 0..rank {
            crate::interrupt::row_checkpoint(basis)?;
            let Some(v) = evaluate_values(
                &bank[basis * channels..(basis + 1) * channels],
                value,
                row,
                expansion,
            )
            .and_then(|v| normalized_value(v, scales[basis], norms[basis])) else {
                return Ok(false);
            };
            inverse[basis] = v;
        }
        for left in 0..rank {
            crate::interrupt::row_checkpoint(left)?;
            for right in 0..=left {
                let entry = left * rank + right;
                neumaier_add(
                    inverse[left] * inverse[right],
                    &mut gram[entry],
                    &mut factor[entry],
                );
            }
        }
    }
    for left in 0..rank {
        crate::interrupt::checkpoint()?;
        for right in 0..=left {
            let v = gram[left * rank + right] + factor[left * rank + right];
            if !v.is_finite() {
                return Ok(false);
            }
            gram[left * rank + right] = v;
            gram[right * rank + left] = v;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streamed_gram_preserves_pairwise_compensated_arithmetic_bits() {
        let (channels, rank, n) = (7, 5, 23);
        let mut scratch = AuditScratch::new(channels, 0, u128::MAX).unwrap();
        for i in 0..rank {
            for j in 0..channels {
                scratch.bank[i * channels + j] = ((i + j * 3) as f64 - 7.) / 8.;
            }
            scratch.scales[i] = (i + 2) as f64;
            scratch.norms[i] = (i + 1) as f64 / 3.;
        }
        let value = |c: usize, r: usize| ((c * 13 + r * 7) as f64 - 61.) / 11.;
        let mut reference = vec![0.; rank * rank];
        for left in 0..rank {
            for right in 0..=left {
                let (mut sum, mut correction) = (0., 0.);
                for row in 0..n {
                    let a = evaluate_values(
                        &scratch.bank[left * channels..(left + 1) * channels],
                        &value,
                        row,
                        &mut scratch.expansion,
                    )
                    .unwrap();
                    let b = evaluate_values(
                        &scratch.bank[right * channels..(right + 1) * channels],
                        &value,
                        row,
                        &mut scratch.expansion,
                    )
                    .unwrap();
                    let a = normalized_value(a, scratch.scales[left], scratch.norms[left]).unwrap();
                    let b =
                        normalized_value(b, scratch.scales[right], scratch.norms[right]).unwrap();
                    neumaier_add(a * b, &mut sum, &mut correction);
                }
                reference[left * rank + right] = sum + correction;
                reference[right * rank + left] = sum + correction;
            }
        }
        assert!(fill_gram(0..n, channels, rank, &value, &mut scratch).unwrap());
        assert_eq!(
            scratch.gram[..rank * rank]
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            reference.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
    }
}
