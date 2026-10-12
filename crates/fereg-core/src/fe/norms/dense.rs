//! Dual norm states for initial dense OLS, sharing each weighted matrix read.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::fe) fn column_norms(
    values: &DenseMatrix,
    root_weight: impl Fn(usize) -> Result<f64> + Sync,
    output: &mut [f64],
    scales: &mut [f64],
    sums: &mut [f64],
    dense: &mut [f64],
    value_error: &'static str,
    norm_error: &'static str,
) -> Result<bool> {
    let threads = rayon::current_num_threads();
    let result = if threads == 1 {
        blocks::<64>(values, &root_weight, scales, sums, dense, value_error)
    } else if values.ncols().div_ceil(8) >= threads {
        blocks::<8>(values, &root_weight, scales, sums, dense, value_error)
    } else {
        blocks::<1>(values, &root_weight, scales, sums, dense, value_error)
    };
    if let Err(error) = result {
        // Preserve the certificate's original cold row-major error replay.
        super::column_norms(
            values,
            root_weight,
            output,
            scales,
            sums,
            &mut [],
            value_error,
            norm_error,
        )?;
        return Err(error);
    }
    for column in 0..output.len() {
        output[column] = finish(scales[column], sums[column], norm_error)?;
    }
    Ok(dense.iter().all(|value| value.is_finite() && *value >= 0.0))
}

fn blocks<const BLOCK: usize>(
    values: &DenseMatrix,
    root_weight: &(impl Fn(usize) -> Result<f64> + Sync),
    scales: &mut [f64],
    sums: &mut [f64],
    dense: &mut [f64],
    value_error: &'static str,
) -> Result<()> {
    crate::parallel::stage_try_for_each(
        scales
            .par_chunks_mut(BLOCK)
            .zip(sums.par_chunks_mut(BLOCK))
            .zip(dense.par_chunks_mut(BLOCK))
            .enumerate(),
        values.as_slice().len() as u128,
        |(block, ((scales, sums), dense))| {
            let first = block * BLOCK;
            let width = dense.len();
            let mut full_scales = [0.0; BLOCK];
            let mut full_sums = [1.0; BLOCK];
            let mut full_valid = [true; BLOCK];
            scales.fill(0.0);
            sums.fill(1.0);
            for start in (0..values.nrows()).step_by(LEAF_ROWS) {
                let mut leaf_scales = [0.0; BLOCK];
                let mut leaf_sums = [1.0; BLOCK];
                for row in start..(start + LEAF_ROWS).min(values.nrows()) {
                    let root = root_weight(row)?;
                    let offset = row * values.ncols() + first;
                    for column in 0..width {
                        let value = root * values.as_slice()[offset + column];
                        update(
                            value,
                            &mut leaf_scales[column],
                            &mut leaf_sums[column],
                            value_error,
                        )?;
                        update(
                            value,
                            &mut full_scales[column],
                            &mut full_sums[column],
                            value_error,
                        )?;
                        full_valid[column] &=
                            full_scales[column].is_finite() && full_sums[column].is_finite();
                    }
                }
                for column in 0..width {
                    merge(
                        leaf_scales[column],
                        leaf_sums[column],
                        &mut scales[column],
                        &mut sums[column],
                    );
                }
            }
            for column in 0..width {
                // Dense-only overflow belongs to the later OLS norm call.
                dense[column] = if full_valid[column] {
                    if full_scales[column] == 0.0 {
                        0.0
                    } else {
                        full_scales[column] * full_sums[column].sqrt()
                    }
                } else {
                    f64::NAN
                };
            }
            Ok(())
        },
    )
}

#[cfg(test)]
mod tests;
