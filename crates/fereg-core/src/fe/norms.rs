//! Scaled RHS norms with independent columns and fixed observation order.
use crate::{
    data::DenseMatrix,
    error::{ErrorCode, FeregError, Result},
};
use rayon::prelude::*;

pub(super) fn column_norms(
    values: &DenseMatrix,
    root_weight: impl Fn(usize) -> Result<f64> + Sync,
    output: &mut [f64],
    scales: &mut [f64],
    sums: &mut [f64],
    value_error: &'static str,
    norm_error: &'static str,
) -> Result<()> {
    let work = values.nrows() as u128 * values.ncols() as u128;
    scales.fill(0.0);
    sums.fill(1.0);
    if crate::parallel::StagePlan::current(work, values.ncols()).lanes == 1 {
        for row in 0..values.nrows() {
            let root = root_weight(row)?;
            for column in 0..values.ncols() {
                update(
                    root * values[(row, column)],
                    &mut scales[column],
                    &mut sums[column],
                    value_error,
                )?;
            }
        }
        for column in 0..values.ncols() {
            output[column] = finish(scales[column], sums[column], norm_error)?;
        }
        return Ok(());
    }
    // Local scalar state avoids sharing cache lines during the recurrence.
    // Columns retain precisely the serial kernel's observation order.
    crate::parallel::stage_try_for_each(
        output
            .par_iter_mut()
            .zip(scales.par_iter_mut())
            .zip(sums.par_iter_mut())
            .enumerate(),
        work,
        |(column, ((out, scale), sum))| {
            let mut local_scale = 0.0;
            let mut local_sum = 1.0;
            for row in 0..values.nrows() {
                update(
                    root_weight(row)? * values[(row, column)],
                    &mut local_scale,
                    &mut local_sum,
                    value_error,
                )?;
            }
            *out = finish(local_scale, local_sum, norm_error)?;
            *scale = local_scale;
            *sum = local_sum;
            Ok(())
        },
    )
}

fn update(value: f64, scale: &mut f64, sum: &mut f64, message: &'static str) -> Result<()> {
    if !value.is_finite() {
        return Err(FeregError::new(ErrorCode::InvalidData, message));
    }
    let absolute = value.abs();
    if absolute != 0.0 {
        if *scale < absolute {
            let ratio = *scale / absolute;
            *sum = 1.0 + *sum * ratio * ratio;
            *scale = absolute;
        } else {
            let ratio = absolute / *scale;
            *sum += ratio * ratio;
        }
    }
    Ok(())
}

fn finish(scale: f64, sum: f64, message: &'static str) -> Result<f64> {
    let norm = if scale == 0.0 {
        0.0
    } else {
        scale * sum.sqrt()
    };
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(FeregError::new(ErrorCode::InvalidData, message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_rhs_norms_preserve_frozen_scalar_bits_and_scratch() {
        for width in [1, 7, 8, 9, 17, 33] {
            let rows = 10_007;
            let data = (0..rows * width)
                .map(|i| match i % 7 {
                    0 => -0.0,
                    1 => 1e-300,
                    2 => -1e150,
                    3 => 1e-150,
                    _ => ((i % 173) as f64 - 86.0) / 91.0,
                })
                .collect();
            let values = DenseMatrix::from_row_major(rows, width, data).unwrap();
            for weighted in [false, true] {
                let weight = |r: usize| {
                    if weighted {
                        [1.0_f64, 1e-120, 0.125][r % 3]
                    } else {
                        1.0
                    }
                };
                let mut scales = vec![0.0_f64; width];
                let mut sums = vec![1.0_f64; width];
                // Frozen former row-major recurrence; no shared update helper.
                for row in 0..rows {
                    let root = weight(row).sqrt();
                    for c in 0..width {
                        let absolute = (root * values[(row, c)]).abs();
                        if absolute == 0.0 {
                            continue;
                        }
                        if scales[c] < absolute {
                            let ratio = scales[c] / absolute;
                            sums[c] = 1.0 + sums[c] * ratio * ratio;
                            scales[c] = absolute;
                        } else {
                            let ratio = absolute / scales[c];
                            sums[c] += ratio * ratio;
                        }
                    }
                }
                let expected: Vec<_> = scales
                    .iter()
                    .zip(&sums)
                    .map(|(s, q)| if *s == 0.0 { 0.0 } else { s * q.sqrt() })
                    .collect();
                for threads in [1, 4, 16] {
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap();
                    let mut out = vec![0.0; width];
                    let mut s = vec![0.0; width];
                    let mut q = vec![0.0; width];
                    pool.install(|| {
                        column_norms(
                            &values,
                            |r| Ok(weight(r).sqrt()),
                            &mut out,
                            &mut s,
                            &mut q,
                            "value",
                            "norm",
                        )
                    })
                    .unwrap();
                    for (actual, target) in [(&out, &expected), (&s, &scales), (&q, &sums)] {
                        assert_eq!(
                            actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                            target.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                            "width={width}, weighted={weighted}, threads={threads}"
                        );
                    }
                }
            }
        }
    }
}
