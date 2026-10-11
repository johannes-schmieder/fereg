//! Fixed-row-leaf scaled norms; scheduling never changes the arithmetic tree.
use crate::{
    data::DenseMatrix,
    error::{ErrorCode, FeregError, Result},
};
use rayon::prelude::*;

const LEAF_ROWS: usize = super::borrowed_map::PARALLEL_DOT_CHUNK_ROWS;
const TILE_COLUMNS: usize = 64;
const MAX_WAVE_LEAVES: usize = 128;

#[allow(clippy::too_many_arguments)]
pub(super) fn column_norms(
    values: &DenseMatrix,
    root_weight: impl Fn(usize) -> Result<f64> + Sync,
    output: &mut [f64],
    scales: &mut [f64],
    sums: &mut [f64],
    partials: &mut [f64],
    value_error: &'static str,
    norm_error: &'static str,
) -> Result<()> {
    scales.fill(0.0);
    sums.fill(1.0);
    let leaves = values.nrows().div_ceil(LEAF_ROWS);
    let result = (|| {
        for first in (0..values.ncols()).step_by(TILE_COLUMNS) {
            let width = (values.ncols() - first).min(TILE_COLUMNS);
            let wave = (partials.len() / (2 * width)).min(MAX_WAVE_LEAVES);
            if wave > 1 && rayon::current_thread_index().is_some() {
                for start in (0..leaves).step_by(wave) {
                    let count = wave.min(leaves - start);
                    let bank = &mut partials[..count * 2 * width];
                    let work = ((count * LEAF_ROWS).min(values.nrows() - start * LEAF_ROWS)
                        as u128)
                        * width as u128;
                    crate::parallel::stage_try_for_each(
                        bank.par_chunks_mut(2 * width).enumerate(),
                        work,
                        |(local, pair)| {
                            let (scale, sum) = pair.split_at_mut(width);
                            leaf(
                                values,
                                &root_weight,
                                start + local,
                                first,
                                scale,
                                sum,
                                value_error,
                            )
                        },
                    )?;
                    for pair in bank.chunks_exact(2 * width) {
                        for column in 0..width {
                            merge(
                                pair[column],
                                pair[width + column],
                                &mut scales[first + column],
                                &mut sums[first + column],
                            );
                        }
                    }
                }
            } else {
                // No allocation or admission is needed for the identical tree.
                let mut scale = [0.0; TILE_COLUMNS];
                let mut sum = [1.0; TILE_COLUMNS];
                for index in 0..leaves {
                    leaf(
                        values,
                        &root_weight,
                        index,
                        first,
                        &mut scale[..width],
                        &mut sum[..width],
                        value_error,
                    )?;
                    for column in 0..width {
                        merge(
                            scale[column],
                            sum[column],
                            &mut scales[first + column],
                            &mut sums[first + column],
                        );
                    }
                }
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        // Tile scheduling must not choose which invalid input is reported.
        // This cold replay preserves row/column validation order on failure.
        for row in 0..values.nrows() {
            let root = root_weight(row)?;
            for &value in &values.as_slice()[row * values.ncols()..(row + 1) * values.ncols()] {
                if !(root * value).is_finite() {
                    return Err(FeregError::new(ErrorCode::InvalidData, value_error));
                }
            }
        }
        return Err(error);
    }
    for column in 0..values.ncols() {
        output[column] = finish(scales[column], sums[column], norm_error)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn leaf(
    values: &DenseMatrix,
    root_weight: &(impl Fn(usize) -> Result<f64> + Sync),
    index: usize,
    first: usize,
    scales: &mut [f64],
    sums: &mut [f64],
    message: &'static str,
) -> Result<()> {
    scales.fill(0.0);
    sums.fill(1.0);
    let start = index * LEAF_ROWS;
    let end = (start + LEAF_ROWS).min(values.nrows());
    for row in start..end {
        let root = root_weight(row)?;
        let offset = row * values.ncols() + first;
        for column in 0..scales.len() {
            update(
                root * values.as_slice()[offset + column],
                &mut scales[column],
                &mut sums[column],
                message,
            )?;
        }
    }
    Ok(())
}

fn merge(other_scale: f64, other_sum: f64, scale: &mut f64, sum: &mut f64) {
    if other_scale == 0.0 {
        return;
    }
    // Every nonzero leaf has sum >= 1 and its scale is the largest computed
    // weighted element. Retain the larger scale and its >= 1 contribution;
    // the other contribution is nonnegative, even if its ratio underflows.
    // Thus finish >= scale >= every weighted element, as required by D1.
    if *scale < other_scale {
        let ratio = *scale / other_scale;
        *sum = other_sum + *sum * ratio * ratio;
        *scale = other_scale;
    } else {
        let ratio = other_scale / *scale;
        *sum += other_sum * ratio * ratio;
    }
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
#[path = "../../../fereg-testkit/src/fixed_leaf_norm_reference.rs"]
mod reference;

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        values: &DenseMatrix,
        weights: &[f64],
        scratch: usize,
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let mut out = vec![f64::NAN; values.ncols()];
        let mut scales = out.clone();
        let mut sums = out.clone();
        let mut partials = vec![f64::NAN; scratch];
        let pointer = partials.as_ptr();
        let capacity = partials.capacity();
        column_norms(
            values,
            |r| Ok(weights[r].sqrt()),
            &mut out,
            &mut scales,
            &mut sums,
            &mut partials,
            "value",
            "norm",
        )
        .unwrap();
        assert_eq!(partials.as_ptr(), pointer);
        assert_eq!(partials.capacity(), capacity);
        for c in 0..values.ncols() {
            let maximum = (0..values.nrows())
                .map(|r| (weights[r].sqrt() * values[(r, c)]).abs())
                .fold(0.0_f64, f64::max);
            assert_eq!(scales[c].to_bits(), maximum.to_bits());
            assert!(sums[c] >= 1.0);
            assert!(out[c] >= maximum);
        }
        (out, scales, sums)
    }

    #[test]
    fn fixed_leaf_norms_retain_legacy_bits_for_one_leaf() {
        let teams: Vec<_> = [1, 4, 16]
            .into_iter()
            .map(|threads| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
            })
            .collect();
        for rows in [0, 1, 73, 511, 512] {
            for width in [1, 17, 65, 129] {
                for case in 0..4 {
                    let values = DenseMatrix::from_row_major(
                        rows,
                        width,
                        (0..rows * width)
                            .map(|i| match case {
                                0 => -0.0,
                                1 => {
                                    if i % 2 == 0 {
                                        f64::from_bits(1)
                                    } else {
                                        -f64::from_bits(3)
                                    }
                                }
                                2 => {
                                    if i / width == rows - 1 {
                                        f64::MAX
                                    } else {
                                        0.0
                                    }
                                }
                                _ => match i % 5 {
                                    0 => 1e-300,
                                    1 => -1e150,
                                    2 => -0.0,
                                    _ => ((i % 173) as f64 - 86.0) / 91.0,
                                },
                            })
                            .collect(),
                    )
                    .unwrap();
                    for weighted in [false, true] {
                        let weights: Vec<f64> = (0..rows)
                            .map(|r| {
                                if weighted {
                                    [1.0, 0.25, 0.125, 1e-120][r % 4]
                                } else {
                                    1.0
                                }
                            })
                            .collect();
                        let expected: Vec<_> = (0..width)
                            .map(|c| {
                                reference::legacy_scalar_norm_state(
                                    (0..rows).map(|r| weights[r].sqrt() * values[(r, c)]),
                                )
                            })
                            .collect();
                        for team in &teams {
                            for scratch in [0, 2 * width.min(TILE_COLUMNS) * MAX_WAVE_LEAVES] {
                                let (norms, scales, sums) =
                                    team.install(|| run(&values, &weights, scratch));
                                for c in 0..width {
                                    assert_eq!((norms[c].to_bits(), scales[c].to_bits(), sums[c].to_bits()),
                                        (expected[c].0.to_bits(), expected[c].1.to_bits(), expected[c].2.to_bits()),
                                        "rows={rows}, width={width}, case={case}, weighted={weighted}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_leaf_norms_match_independent_integer_power_witness() {
        // Integer square sums are exact; the reference has no scaled-sum
        // recurrence, row leaves, or shared production arithmetic.
        for rows in [0, 1, 511, 512, 513, 1_537, 65_539] {
            for width in [1, 17, 65, 129] {
                let coefficient =
                    |r: usize, c: usize| (r.wrapping_mul(13) + c * 7) as i64 % 25 - 12;
                let base = |c: usize| 2.0_f64.powi([-900, -500, 0, 500, 900][c % 5]);
                let values = DenseMatrix::from_row_major(
                    rows,
                    width,
                    (0..rows * width)
                        .map(|i| coefficient(i / width, i % width) as f64 * base(i % width))
                        .collect(),
                )
                .unwrap();
                for shift in [0, 1] {
                    let multiplier = |r: usize| [1_i64, 2, 4, 8][(r + shift) % 4];
                    let weights: Vec<_> = (0..rows)
                        .map(|r| {
                            let root = multiplier(r) as f64 / 8.0;
                            root * root
                        })
                        .collect();
                    let (actual, _, _) = run(&values, &weights, 0);
                    for (c, actual) in actual.iter().copied().enumerate() {
                        let expected = reference::integer_power_norm(
                            (0..rows).map(|r| coefficient(r, c) * multiplier(r)),
                            base(c) / 8.0,
                        );
                        if expected == 0.0 {
                            assert_eq!(actual.to_bits(), 0.0_f64.to_bits());
                        } else {
                            assert!((actual / expected - 1.0).abs() <= 64.0 * f64::EPSILON,
                                "rows={rows}, width={width}, column={c}, got={actual}, expected={expected}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_leaf_norms_are_exact_across_workers_scratch_and_waves() {
        let teams: Vec<_> = [1, 4, 16]
            .into_iter()
            .map(|threads| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
            })
            .collect();
        for rows in [511, 512, 513, 1_537, 65_539] {
            for width in [1, 17, 65, 129] {
                let values = DenseMatrix::from_row_major(
                    rows,
                    width,
                    (0..rows * width)
                        .map(|i| match i % 7 {
                            0 => -0.0,
                            1 => 1e-300,
                            2 => -1e150,
                            3 => 1e-150,
                            _ => ((i % 173) as f64 - 86.0) / 91.0,
                        })
                        .collect(),
                )
                .unwrap();
                for weighted in [false, true] {
                    let weights: Vec<_> = (0..rows)
                        .map(|r| {
                            if weighted {
                                [1.0, 1e-120, 0.125][r % 3]
                            } else {
                                1.0
                            }
                        })
                        .collect();
                    let expected = run(&values, &weights, 0);
                    let tile = width.min(TILE_COLUMNS);
                    for team in &teams {
                        for scratch in [
                            0,
                            2 * tile - 1,
                            2 * tile,
                            6 * tile + 1,
                            2 * tile * MAX_WAVE_LEAVES + 7,
                        ] {
                            let actual = team.install(|| run(&values, &weights, scratch));
                            for (got, want) in [
                                (&actual.0, &expected.0),
                                (&actual.1, &expected.1),
                                (&actual.2, &expected.2),
                            ] {
                                assert_eq!(
                                    got.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                                    want.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                                    "rows={rows}, width={width}, scratch={scratch}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_leaf_norms_preserve_tiny_values_and_error_checks() {
        let tiny = f64::from_bits(1);
        for value in [0.0, -0.0, tiny, -tiny, f64::MIN_POSITIVE, 1e-300, f64::MAX] {
            let mut values = vec![0.0; 513];
            values[512] = value;
            let matrix = DenseMatrix::from_row_major(513, 1, values).unwrap();
            assert_eq!(
                run(&matrix, &vec![1.0; 513], 0).0[0].to_bits(),
                value.abs().to_bits()
            );
        }
        let matrix = DenseMatrix::from_row_major(1, 1, vec![tiny]).unwrap();
        assert_eq!(run(&matrix, &[0.25], 0).0[0], 0.0); // D1 must still decline weighted zero norms.
        for threads in [1, 4, 16] {
            let team = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            for scratch in [0, 1, 256] {
                for (value, root, error) in [
                    (f64::NAN, 1.0, "value"),
                    (f64::INFINITY, 1.0, "value"),
                    (f64::MAX, 2.0, "value"),
                    (f64::MAX, 1.0, "norm"),
                ] {
                    let matrix = DenseMatrix::from_row_major(513, 1, vec![value; 513]).unwrap();
                    let actual = team.install(|| {
                        column_norms(
                            &matrix,
                            |_| Ok(root),
                            &mut [0.0],
                            &mut [0.0],
                            &mut [0.0],
                            &mut vec![0.0; scratch],
                            "value",
                            "norm",
                        )
                    });
                    assert_eq!(
                        actual.unwrap_err(),
                        FeregError::new(ErrorCode::InvalidData, error)
                    );
                }
                // An invalid high-column value precedes a later root error:
                // a tile/leaf schedule must preserve the row-major input error.
                let mut values = vec![0.0; 513 * 65];
                values[64] = f64::INFINITY;
                let matrix = DenseMatrix::from_row_major(513, 65, values).unwrap();
                let actual = team.install(|| {
                    column_norms(
                        &matrix,
                        |r| {
                            if r == 512 {
                                Err(FeregError::new(ErrorCode::InvalidData, "root"))
                            } else {
                                Ok(1.0)
                            }
                        },
                        &mut [0.0; 65],
                        &mut [0.0; 65],
                        &mut [0.0; 65],
                        &mut vec![0.0; scratch],
                        "value",
                        "norm",
                    )
                });
                assert_eq!(actual.unwrap_err().message, "value");
                let actual = team.install(|| {
                    column_norms(
                        &matrix,
                        |_| Err(FeregError::new(ErrorCode::InvalidData, "root")),
                        &mut [0.0; 65],
                        &mut [0.0; 65],
                        &mut [0.0; 65],
                        &mut vec![0.0; scratch],
                        "value",
                        "norm",
                    )
                });
                assert_eq!(actual.unwrap_err().message, "root");
            }
        }
    }
}
