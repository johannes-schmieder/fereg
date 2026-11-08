//! Independent iid fit sums: outcome and residual sums of squares and the
//! compensated weighted mean over fixed row chunks.
use super::*;

pub(in crate::ols) fn independent_residual_sum(
    fit: &CertifiedPreparedOlsFit,
    weight_view: ReferenceWeightView<'_>,
) -> Result<f64> {
    weighted_sum_of_squares(fit.residuals(), weight_view, 0.0)
}

pub(in crate::ols) fn independent_fit_sums(
    outcome: &[f64],
    residuals: &[f64],
    weight_view: ReferenceWeightView<'_>,
    has_intercept: bool,
) -> Result<(f64, f64)> {
    let tss = || independent_outcome_sum(outcome, weight_view, has_intercept);
    let rss = || weighted_sum_of_squares(residuals, weight_view, 0.0);
    // These certificates use separate source/residual scans. Overlap their
    // work without changing either reduction or the established error order.
    let (tss_result, rss_result) =
        if crate::parallel::StagePlan::current(outcome.len() as u128 * 2, 2).lanes > 1
            && rayon::current_thread_index().is_some()
        {
            rayon::join(tss, rss)
        } else {
            (tss(), rss())
        };
    Ok((tss_result?, rss_result?))
}

pub(in crate::ols) fn independent_outcome_sum(
    outcome: &[f64],
    weights: ReferenceWeightView<'_>,
    has_intercept: bool,
) -> Result<f64> {
    let mean = weighted_mean(outcome, weights)?;
    weighted_sum_of_squares(outcome, weights, if has_intercept { mean } else { 0.0 })
}

pub(in crate::ols) fn weighted_mean(
    values: &[f64],
    weights: ReferenceWeightView<'_>,
) -> Result<f64> {
    // Fixed row chunks keep their own compensated sums, merged in chunk order.
    const ROWS: usize = 1 << 14;
    if values.is_empty() {
        return Err(invalid_argument(
            "prepared iid weighted mean requires a nonempty sample",
        ));
    }
    let mut value_scale = 0.0_f64;
    let mut finite = true;
    let chunk_values = |chunk: usize| &values[chunk * ROWS..((chunk + 1) * ROWS).min(values.len())];
    ordered_chunk_reduce(
        values.len().div_ceil(ROWS),
        |chunk| {
            chunk_values(chunk)
                .iter()
                .fold((0.0_f64, true), |(scale, ok), value| {
                    (scale.max(value.abs()), ok && value.is_finite())
                })
        },
        |(scale, ok)| {
            value_scale = value_scale.max(scale);
            finite &= ok;
            Ok(())
        },
    )?;
    if !finite {
        return Err(invalid_data(
            "prepared iid weighted-mean value is not finite",
        ));
    }
    if value_scale == 0.0 {
        return Ok(0.0);
    }
    // Unit mass sums exactly to N through 2^53.
    let unit = weights.kind.is_none()
        && weights.values.is_none()
        && (values.len() as u128) <= (1_u128 << 53);
    let (mut numerator, mut numerator_correction) = (0.0_f64, 0.0_f64);
    let (mut denominator, mut denominator_correction) = (0.0_f64, 0.0_f64);
    ordered_chunk_reduce(
        values.len().div_ceil(ROWS),
        |chunk| -> Result<(f64, f64, f64, f64)> {
            let values = chunk_values(chunk);
            let (mut numerator, mut numerator_correction) = (0.0_f64, 0.0_f64);
            let (mut denominator, mut denominator_correction) = (0.0_f64, 0.0_f64);
            if unit {
                // Every value is finite (checked above) and the scaled terms lie
                // in [-1, 1], so no term or partial can fail the checked add:
                // the identical Neumaier updates without per-term checks.
                for &value in values {
                    neumaier_add(
                        value / value_scale,
                        &mut numerator,
                        &mut numerator_correction,
                    );
                }
                return Ok((
                    numerator,
                    numerator_correction,
                    denominator,
                    denominator_correction,
                ));
            }
            for (offset, &value) in values.iter().enumerate() {
                let weight = weights.value(chunk * ROWS + offset)?;
                compensated_add(
                    weight * (value / value_scale),
                    &mut numerator,
                    &mut numerator_correction,
                )?;
                compensated_add(weight, &mut denominator, &mut denominator_correction)?;
            }
            Ok((
                numerator,
                numerator_correction,
                denominator,
                denominator_correction,
            ))
        },
        |part| {
            let (n, nc, d, dc) = part?;
            compensated_add(n, &mut numerator, &mut numerator_correction)?;
            compensated_add(nc, &mut numerator, &mut numerator_correction)?;
            compensated_add(d, &mut denominator, &mut denominator_correction)?;
            compensated_add(dc, &mut denominator, &mut denominator_correction)
        },
    )?;
    let denominator = if unit {
        values.len() as f64
    } else {
        denominator + denominator_correction
    };
    let mean = value_scale * ((numerator + numerator_correction) / denominator);
    if mean.is_finite() && denominator.is_finite() && denominator > 0.0 {
        Ok(mean)
    } else {
        Err(invalid_data("prepared iid weighted mean is not finite"))
    }
}

pub(in crate::ols) fn weighted_sum_of_squares(
    values: &[f64],
    weights: ReferenceWeightView<'_>,
    center: f64,
) -> Result<f64> {
    if !center.is_finite() {
        return Err(invalid_data(
            "prepared iid sum-of-squares center is invalid",
        ));
    }
    // Plain sums over fixed row chunks, combined in order, when every value is
    // finite and the magnitudes cannot over- or underflow; else the scaled sum.
    {
        const ROWS: usize = 1 << 14;
        let unit = weights.kind.is_none() && weights.values.is_none();
        let mut total = Some((0.0_f64, 0.0_f64));
        ordered_chunk_reduce(
            values.len().div_ceil(ROWS),
            |chunk| {
                let values = &values[chunk * ROWS..((chunk + 1) * ROWS).min(values.len())];
                let (mut sum, mut big) = (0.0_f64, 0.0_f64);
                for (offset, &value) in values.iter().enumerate() {
                    let d = value - center;
                    let w = if unit {
                        1.0
                    } else {
                        weights.value(chunk * ROWS + offset).ok()?
                    };
                    sum += w * d * d;
                    big = big.max(d.abs());
                }
                Some((sum, big))
            },
            |part| {
                total = total.zip(part).map(|(t, p)| (t.0 + p.0, t.1.max(p.1)));
                Ok(())
            },
        )?;
        if let Some((sum, big)) = total {
            if sum.is_finite() && (big == 0.0 || (1e-140..=1e140).contains(&big)) {
                return Ok(sum);
            }
        }
    }
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    if weights.kind.is_none() && weights.values.is_none() {
        for value in values.iter().copied() {
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared iid sum-of-squares value is not finite",
                ));
            }
            update_scaled_sum_of_squares(value - center, &mut scale, &mut scaled_square_sum)?;
        }
    } else {
        for (row, value) in values.iter().copied().enumerate() {
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared iid sum-of-squares value is not finite",
                ));
            }
            let weighted = weights.value(row)?.sqrt() * (value - center);
            update_scaled_sum_of_squares(weighted, &mut scale, &mut scaled_square_sum)?;
        }
    }
    let norm = if scale == 0.0 {
        0.0
    } else {
        scale * scaled_square_sum.sqrt()
    };
    let sum = crate::kernels::fused(crate::kernels::installed(), norm, norm, 0.0);
    if sum.is_finite() && sum >= 0.0 {
        Ok(sum)
    } else {
        Err(invalid_data(
            "prepared iid weighted sum of squares is not finite",
        ))
    }
}

fn update_scaled_sum_of_squares(value: f64, scale: &mut f64, sum: &mut f64) -> Result<()> {
    if !value.is_finite() {
        return Err(invalid_data(
            "prepared iid scaled sum-of-squares value is not finite",
        ));
    }
    let absolute = value.abs();
    if absolute == 0.0 {
        return Ok(());
    }
    if *scale < absolute {
        let ratio = *scale / absolute;
        *sum = 1.0 + *sum * ratio * ratio;
        *scale = absolute;
    } else {
        let ratio = absolute / *scale;
        *sum += ratio * ratio;
    }
    if scale.is_finite() && sum.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "prepared iid scaled sum-of-squares accumulation is not finite",
        ))
    }
}
