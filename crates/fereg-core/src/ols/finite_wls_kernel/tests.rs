use crate::error::ErrorCode;
#[test]
fn retained_arithmetic_leaves_survive_serial_stage_admission() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap()
        .install(|| {
            let layout = super::FiniteWlsMomentLayout::plan(2, 12, 4).unwrap();
            assert_eq!(layout.partitions, 2);
            assert_eq!(
                crate::parallel::StagePlan::current(
                    super::moment_lane_work(6, 2),
                    layout.partitions,
                )
                .lanes,
                1,
            );
            assert!(super::scratch_row_partition_plan(6, 2, 2, layout).is_some());
        });
}

#[test]
fn extreme_schedule_shapes_fail_capacity_without_panicking() {
    let error = super::FiniteWlsMomentLayout::plan(usize::MAX, usize::MAX, 16)
        .expect_err("unaddressable scratch must fail checked capacity");
    assert_eq!(error.code, crate::ErrorCode::Allocation);
}

use super::*;

#[test]
fn tiled_dropped_span_keeps_scalar_order_and_tail() {
    let row: Vec<_> = (0..25).map(|i| (i as f64 + 0.125).cos()).collect();
    let (kept, dropped) = (
        [24, 22, 23, 21, 20, 19, 18],
        (0..18).rev().collect::<Vec<_>>(),
    );
    let coefficients: Vec<_> = (0..kept.len() * dropped.len())
        .map(|i| (i as f64 + 0.25).sin())
        .collect();
    let mut norms = vec![0.125; dropped.len()];
    let expected: Vec<_> = dropped
        .iter()
        .enumerate()
        .map(|(position, &column)| {
            let mut residual = row[column];
            for (selected, &keep) in kept.iter().enumerate() {
                residual -= row[keep] * coefficients[selected * dropped.len() + position];
            }
            hypot(0.125, residual).to_bits()
        })
        .collect();
    accumulate_dropped_span_row(&row, (&kept, &dropped), &coefficients, &mut norms).unwrap();
    assert_eq!(
        norms.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected
    );
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual:.17e}, expected {expected:.17e}, tolerance {tolerance:.3e}"
    );
}

fn assert_reordered_moments(
    rhs: &[f64],
    gram: &[f64],
    expected_rhs: &[f64],
    expected_gram: &[f64],
    rows: usize,
    response_norm: f64,
) {
    let k = rhs.len();
    // Both scalar and partitioned FMA sums have forward error bounded by
    // gamma_N times the absolute products; Cauchy bounds those products.
    // Four N*epsilon covers both sums and the compensated partial merge.
    let bound = 4. * rows as f64 * f64::EPSILON;
    for j in 0..k {
        let norm = expected_gram[j * k + j].sqrt();
        assert_close(rhs[j], expected_rhs[j], bound * norm * response_norm);
        for q in 0..k {
            assert_close(
                gram[j * k + q],
                expected_gram[j * k + q],
                bound * norm * expected_gram[q * k + q].sqrt(),
            );
        }
    }
}

#[test]
fn scratch_row_partition_admission_bounds_storage_and_critical_work() {
    for dimension in [1_usize, 7, 16, 28, 32, 33, 63, 64, 65, 128, 257] {
        for threads in [1_usize, 4, 8, 16] {
            let width = dimension.div_ceil(threads.min(dimension));
            let parts = dimension.div_ceil(width);
            let layout = FiniteWlsMomentLayout {
                partitions: parts,
                columns_per_partition: width,
                partial_stride: 1024,
                partial_elements: parts * 1024,
            };
            if let Some((count, stride)) =
                scratch_row_partition_plan(16_385, dimension, dimension, layout)
            {
                assert!(count >= 2 && count <= parts);
                assert!(count * stride <= layout.partial_elements);
                assert_eq!(stride, dimension * dimension + 2 * dimension);
                assert!(
                    16_385_usize.div_ceil(count) * (dimension * (dimension + 1) / 2 + dimension)
                        < 16_385 * (width * (2 * dimension - width + 1) / 2 + width)
                );
            } else if dimension == 32 && threads == 8 {
                panic!("the representative wide scratch bank admits seven complete partials");
            }
        }
    }
}

#[test]
fn scratch_moment_tiles_match_scalar_oracle_and_bound_read_chunks() {
    for (stored, columns, rows) in [
        (1, 1, 2051),
        (5, 4, 419),
        (18, 8, 131),
        (129, 7, 23),
        (1025, 4, 3),
        (18, 8, 65_536),
        (18, 8, 131_072),
    ] {
        let selected = (0..columns)
            .map(|c| c * (stored - 1) / columns.max(1))
            .collect::<Vec<_>>();
        let scales = (0..columns)
            .map(|c| 2.0_f64.powi(c as i32 - 3))
            .collect::<Vec<_>>();
        let mut data = vec![0.0; rows * stored];
        for row in 0..rows {
            for c in 0..columns {
                data[row * stored + selected[c]] =
                    (((row * 17 + c * 13) % 31) as f64 - 15.0) / 16.0 * scales[c];
            }
        }
        let response = (0..rows)
            .map(|row| ((row % 17) as f64 - 8.0) / 8.0)
            .collect::<Vec<_>>();
        let weights = (0..rows)
            .map(|row| 2.0_f64.powi((row % 5) as i32 * 100 - 200))
            .collect::<Vec<_>>();
        let mut scratch = PreparedOlsScratch::create_row_major(rows, stored).unwrap();
        scratch.write_linear(0, &data).unwrap();
        for provided in [false, true] {
            let divisor = if provided { 2.0_f64.powi(202) } else { 1.0 };
            let mut expected_rhs = vec![0.0; columns];
            let mut expected_gram = vec![0.0; columns * columns];
            for left in 0..columns {
                for row in 0..rows {
                    let weight = if provided {
                        weights[row] / divisor
                    } else {
                        1.0
                    };
                    let left_value = data[row * stored + selected[left]] / scales[left];
                    let weighted_left = weight * left_value;
                    expected_rhs[left] = weighted_left.mul_add(response[row], expected_rhs[left]);
                    for right in left..columns {
                        let position = left * columns + right;
                        expected_gram[position] = weighted_left.mul_add(
                            data[row * stored + selected[right]] / scales[right],
                            expected_gram[position],
                        );
                    }
                }
                for right in left..columns {
                    expected_gram[right * columns + left] = expected_gram[left * columns + right];
                }
            }
            for threads in [1, 4] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let source = FixedBlockMomentSource::Scratch {
                            values: &scratch,
                            selected_columns: Some(&selected),
                            response: (&response).into(),
                            weights: if provided {
                                BorrowedMapWeights::Provided(&weights)
                            } else {
                                BorrowedMapWeights::Unit
                            },
                            solver_weight_divisor: divisor,
                            column_scales: Some(&scales),
                        };
                        let layout_source = validate_moment_source(source).unwrap();
                        let layout = layout_source.layout();
                        let mut partials = vec![0.0; layout.partial_elements];
                        let mut rhs = vec![0.0; columns];
                        let mut gram = vec![0.0; columns * columns];
                        let before = scratch.shared_read_chunks_for_test();
                        fill_finite_wls_moments(layout_source, &mut partials, &mut rhs, &mut gram)
                            .unwrap();
                        let row_plan = scratch_row_partition_plan(rows, stored, columns, layout);
                        if row_plan.is_some() {
                            let response_norm = response
                                .iter()
                                .enumerate()
                                .map(|(i, y)| {
                                    (if provided { weights[i] / divisor } else { 1. }) * y * y
                                })
                                .sum::<f64>()
                                .sqrt();
                            assert_reordered_moments(
                                &rhs,
                                &gram,
                                &expected_rhs,
                                &expected_gram,
                                rows,
                                response_norm,
                            );
                        } else {
                            assert_eq!(rhs, expected_rhs);
                            assert_eq!(gram, expected_gram);
                        }
                        let tile_rows = layout.partial_stride / stored;
                        let chunks_per_tile = layout.partial_stride.div_ceil(1024);
                        let full_tiles = rows / tile_rows;
                        let tail = rows % tile_rows;
                        let expected_chunks = if let Some((parts, _)) = row_plan {
                            (0..parts)
                                .map(|part| {
                                    let start = part * (rows / parts) + part.min(rows % parts);
                                    let end =
                                        start + rows / parts + usize::from(part < rows % parts);
                                    (end * stored).div_ceil(1024) - start * stored / 1024
                                })
                                .sum()
                        } else {
                            layout.partitions
                                * (full_tiles * chunks_per_tile + (tail * stored).div_ceil(1024))
                        };
                        assert_eq!(
                            scratch.shared_read_chunks_for_test() - before,
                            expected_chunks as u64
                        );
                        let bound = threads.min(stored)
                            * (rows * stored).min(stored.max(RETAINED_MOMENT_TILE_ELEMENTS));
                        assert!(layout.partial_elements <= bound);
                        if stored == 18 && rows >= 65_536 {
                            assert!(expected_chunks * 50 < layout.partitions * rows);
                        }
                    });
            }
        }
    }
}

#[test]
fn retained_dense_moments_preserve_scalar_order_across_cache_boundaries() {
    for (columns, spaced, provided, scaled) in [
        (1, false, false, true),
        (6, false, true, true),
        (7, true, true, true),
        (16, false, false, true),
        (63, true, true, true),
        (64, false, true, true),
        (65, true, true, true),
        (128, false, false, true),
        (129, true, true, true),
        (257, true, true, true),
        (7, false, false, false),
        (64, true, true, false),
    ] {
        let rows = 37;
        let selected = (0..columns)
            .map(|column| if spaced { 2 * column } else { column })
            .collect::<Vec<_>>();
        let stored = selected[columns - 1] + 1;
        let scales = (0..columns)
            .map(|column| {
                if scaled {
                    2.0_f64.powi((column % 3) as i32 * 400 - 400)
                } else {
                    1.0
                }
            })
            .collect::<Vec<_>>();
        let weights = (0..rows)
            .map(|row| 2.0_f64.powi((row % 5) as i32 * 100 - 200))
            .collect::<Vec<_>>();
        let divisor = if provided { 2.0_f64.powi(202) } else { 1.0 };
        let response = (0..rows)
            .map(|row| (row as f64 * 0.37).cos())
            .collect::<Vec<_>>();
        let mut values = vec![f64::NAN; rows * stored];
        for row in 0..rows {
            for column in 0..columns {
                values[row * stored + selected[column]] =
                    (((row * 17 + column * 13) % 31) as f64 - 15.0) / 16.0 * scales[column];
            }
        }
        let source = FixedBlockMomentSource::Dense {
            values: FiniteDenseRows::packed(&values, stored),
            rows,
            stored_columns: stored,
            selected_columns: if spaced { Some(&selected) } else { None },
            response: (&response).into(),
            weights: if provided {
                BorrowedMapWeights::Provided(&weights)
            } else {
                BorrowedMapWeights::Unit
            },
            solver_weight_divisor: divisor,
            column_scales: if scaled { Some(&scales) } else { None },
        };
        let mut expected_rhs = vec![0.0_f64; columns];
        let mut expected_gram = vec![0.0_f64; columns * columns];
        for left in 0..columns {
            for row in 0..rows {
                let weight = if provided {
                    weights[row] / divisor
                } else {
                    1.0
                };
                let left_value = values[row * stored + selected[left]] / scales[left];
                let weighted_left = weight * left_value;
                expected_rhs[left] = weighted_left.mul_add(response[row], expected_rhs[left]);
                for right in left..columns {
                    let right_value = values[row * stored + selected[right]] / scales[right];
                    let position = left * columns + right;
                    expected_gram[position] =
                        weighted_left.mul_add(right_value, expected_gram[position]);
                }
            }
            for right in left..columns {
                expected_gram[right * columns + left] = expected_gram[left * columns + right];
            }
        }
        // Dense widths up to eight use the fixed-width kernel: reciprocal
        // scales and separately rounded multiply-adds in the same row order.
        let (dense_rhs, dense_gram) = if columns <= 8 {
            let mut rhs = vec![0.0_f64; columns];
            let mut gram = vec![0.0_f64; columns * columns];
            for row in 0..rows {
                let weight = if provided {
                    weights[row] / divisor
                } else {
                    1.0
                };
                let x: Vec<f64> = (0..columns)
                    .map(|c| values[row * stored + selected[c]] * (1.0 / scales[c]))
                    .collect();
                for left in 0..columns {
                    let weighted = weight * x[left];
                    rhs[left] += weighted * response[row];
                    for right in left..columns {
                        gram[left * columns + right] += weighted * x[right];
                    }
                }
            }
            for left in 0..columns {
                for right in left..columns {
                    gram[right * columns + left] = gram[left * columns + right];
                }
            }
            (rhs, gram)
        } else {
            (expected_rhs.clone(), expected_gram.clone())
        };
        for threads in [1, 2, 4, 16] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let layout_source = validate_moment_source(source).unwrap();
                    let layout = layout_source.layout();
                    assert_eq!(layout.partial_elements, 0);
                    let mut rhs = vec![0.0; columns];
                    let mut gram = vec![0.0; columns * columns];
                    fill_finite_wls_moments(layout_source, &mut [], &mut rhs, &mut gram).unwrap();
                    assert_eq!(rhs, dense_rhs);
                    assert_eq!(gram, dense_gram);
                });
        }
        {
            let mut scratch = PreparedOlsScratch::create_row_major(rows, stored).unwrap();
            // Scratch validates every stored cell; only the dense source
            // can use nonfinite unselected cells to detect accidental reads.
            let scratch_values = values
                .iter()
                .map(|&value| if value.is_nan() { 0.0 } else { value })
                .collect::<Vec<_>>();
            scratch.write_linear(0, &scratch_values).unwrap();
            let source = FixedBlockMomentSource::Scratch {
                values: &scratch,
                selected_columns: Some(&selected),
                response: (&response).into(),
                weights: if provided {
                    BorrowedMapWeights::Provided(&weights)
                } else {
                    BorrowedMapWeights::Unit
                },
                solver_weight_divisor: divisor,
                column_scales: if scaled { Some(&scales) } else { None },
            };
            for threads in [1, 2, 4, 16] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let layout_source = validate_moment_source(source).unwrap();
                        let layout = layout_source.layout();
                        let mut partials = vec![0.0; layout.partial_elements];
                        let mut rhs = vec![0.0; columns];
                        let mut gram = vec![0.0; columns * columns];
                        fill_finite_wls_moments(layout_source, &mut partials, &mut rhs, &mut gram)
                            .unwrap();
                        if scratch_row_partition_plan(rows, stored, columns, layout).is_some() {
                            let response_norm = response
                                .iter()
                                .enumerate()
                                .map(|(i, y)| {
                                    (if provided { weights[i] / divisor } else { 1. }) * y * y
                                })
                                .sum::<f64>()
                                .sqrt();
                            assert_reordered_moments(
                                &rhs,
                                &gram,
                                &expected_rhs,
                                &expected_gram,
                                rows,
                                response_norm,
                            );
                        } else {
                            assert_eq!(rhs, expected_rhs);
                            assert_eq!(gram, expected_gram);
                        }
                    });
            }
            let mut preserved = vec![0.; scratch_values.len()];
            scratch.read_linear_shared(0, &mut preserved).unwrap();
            assert_eq!(preserved, scratch_values);
        }
    }
}

#[test]
fn projected_dense_moments_match_ordered_division_and_fma_bits() {
    let (rows, columns) = (137_usize, 49_usize);
    for step in [1, 2] {
        let selected = (0..columns).map(|j| 1 + step * j).collect::<Vec<_>>();
        let stored = selected[columns - 1] + 2;
        let mut within =
            DenseMatrix::from_row_major(rows, stored + 1, vec![f64::NAN; rows * (stored + 1)])
                .unwrap();
        for row in 0..rows {
            within[(row, 0)] = ((row * 11 % 59) as f64 - 29.0) / 7.1;
            for (j, &column) in selected.iter().enumerate() {
                within[(row, 1 + column)] = if j == 0 {
                    -0.0
                } else {
                    ((row * 17 + j * 13) as f64 * 0.731).sin()
                };
            }
        }
        let scales = (0..columns)
            .map(|j| 0.7 + j as f64 / 2.9)
            .collect::<Vec<_>>();
        let weights = (0..rows)
            .map(|row| 0.3 + (row % 7) as f64)
            .collect::<Vec<_>>();
        let divisor = 2.9;
        let mut expected_rhs = vec![0.0; columns];
        let mut expected_gram = vec![0.0; columns * columns];
        for left in 0..columns {
            for row in 0..rows {
                let weight = weights[row] / divisor;
                let weighted = weight * (within[(row, 1 + selected[left])] / scales[left]);
                expected_rhs[left] = weighted.mul_add(within[(row, 0)], expected_rhs[left]);
                for right in left..columns {
                    let x = within[(row, 1 + selected[right])] / scales[right];
                    let index = left * columns + right;
                    expected_gram[index] = weighted.mul_add(x, expected_gram[index]);
                }
            }
            for right in left + 1..columns {
                expected_gram[right * columns + left] = expected_gram[left * columns + right];
            }
        }
        let source = FixedBlockMomentSource::Dense {
            values: FiniteDenseRows::projected(&within),
            rows,
            stored_columns: stored,
            selected_columns: Some(&selected),
            response: FiniteResponse::Column(&within, 0),
            weights: BorrowedMapWeights::Provided(&weights),
            solver_weight_divisor: divisor,
            column_scales: Some(&scales),
        };
        for threads in [1, 8] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let source = validate_moment_source(source).unwrap();
                    let mut rhs = vec![0.0; columns];
                    let mut gram = vec![0.0; columns * columns];
                    fill_finite_wls_moments(source, &mut [], &mut rhs, &mut gram).unwrap();
                    for (actual, expected) in rhs
                        .iter()
                        .chain(&gram)
                        .zip(expected_rhs.iter().chain(&expected_gram))
                    {
                        assert_eq!(actual.to_bits(), expected.to_bits());
                    }
                });
        }
    }
}

#[test]
fn retained_dense_moment_cache_rejects_nonfinite_selected_values() {
    let response = [1.0; 4];
    for columns in [8, 9, 49] {
        for (bad, scale) in [
            (f64::NAN, None),
            (f64::INFINITY, Some(1.0)),
            (f64::MAX, Some(0.5)),
        ] {
            let mut values = vec![1.0; response.len() * columns];
            let mut scales = vec![1.0; columns];
            values[3 * columns - 1] = bad;
            scales[columns - 1] = scale.unwrap_or(1.0);
            let source = FixedBlockMomentSource::Dense {
                values: FiniteDenseRows::packed(&values, columns),
                rows: response.len(),
                stored_columns: columns,
                selected_columns: None,
                response: (&response).into(),
                weights: BorrowedMapWeights::Unit,
                solver_weight_divisor: 1.0,
                column_scales: scale.map(|_| scales.as_slice()),
            };
            rayon::ThreadPoolBuilder::new()
                .num_threads(2)
                .build()
                .unwrap()
                .install(|| {
                    let layout_source = validate_moment_source(source).unwrap();
                    let error = fill_finite_wls_moments(
                        layout_source,
                        &mut [],
                        &mut vec![0.0; columns],
                        &mut vec![0.0; columns * columns],
                    )
                    .unwrap_err();
                    assert_eq!(error.code, ErrorCode::InvalidData);
                    assert_eq!(
                        error.message,
                        "finite-WLS standardized column is not finite"
                    );
                });
        }
    }
}

#[test]
fn wide_projected_moments_are_bounded_deterministic_and_match_direct_sums() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    for columns in [65_usize, 128] {
        for spaced in [false, true] {
            let candidates = (0..columns)
                .map(|j| if spaced { 2 * j } else { j })
                .collect::<Vec<_>>();
            let stored = candidates[columns - 1] + 2;
            let rows = 137;
            let values = (0..rows * stored)
                .map(|i| ((i * 17 % 127) as f64 - 63.0) / 64.0)
                .collect();
            let within = DenseMatrix::from_row_major(rows, stored, values).expect("wide matrix");
            let source = FixedBlockMomentSource::Dense {
                values: FiniteDenseRows::projected(&within),
                rows: within.nrows(),
                stored_columns: within.ncols() - 1,
                selected_columns: Some(&candidates),
                response: FiniteResponse::Column(&within, 0),
                weights: BorrowedMapWeights::Unit,
                solver_weight_divisor: 1.0,
                column_scales: None,
            };
            let mut reference = None;
            for threads in [1, 7] {
                let actual = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .expect("test pool")
                    .install(|| {
                        let layout_source = validate_moment_source(source).expect("wide layout");
                        let layout = layout_source.layout();
                        assert_eq!(layout.partial_elements, 0);
                        let mut rhs = vec![0.0; columns];
                        let mut gram = vec![0.0; columns * columns];
                        fill_finite_wls_moments(layout_source, &mut [], &mut rhs, &mut gram)
                            .expect("wide moments");
                        for left in 0..columns {
                            assert_eq!(
                                rhs[left],
                                (0..rows)
                                    .map(|r| within[(r, candidates[left] + 1)] * within[(r, 0)])
                                    .sum::<f64>()
                            );
                            for right in 0..columns {
                                assert_eq!(
                                    gram[left * columns + right],
                                    (0..rows)
                                        .map(|r| within[(r, candidates[left] + 1)]
                                            * within[(r, candidates[right] + 1)])
                                        .sum::<f64>()
                                );
                            }
                        }
                        (rhs, gram)
                    });
                if let Some(expected) = &reference {
                    assert_eq!(&actual, expected);
                }
                reference = Some(actual);
            }
            let mut invalid = within.clone();
            invalid[(0, 1)] = f64::MAX;
            let invalid_source = FixedBlockMomentSource::Dense {
                values: FiniteDenseRows::projected(&invalid),
                rows: invalid.nrows(),
                stored_columns: invalid.ncols() - 1,
                selected_columns: Some(&candidates),
                response: FiniteResponse::Column(&invalid, 0),
                weights: BorrowedMapWeights::Unit,
                solver_weight_divisor: 1.0,
                column_scales: None,
            };
            let layout_source =
                validate_moment_source(invalid_source).expect("invalid values retain valid layout");
            assert_eq!(
                fill_finite_wls_moments(
                    layout_source,
                    &mut [],
                    &mut vec![0.0; columns],
                    &mut vec![0.0; columns * columns]
                )
                .expect_err("overflow must fail closed")
                .code,
                ErrorCode::InvalidData
            );
        }
    }
}

fn retained_qr_report(
    values: &[f64],
    rows: usize,
    columns: usize,
    scales: &[f64],
    relative_threshold: f64,
) -> Result<(RankRevealingQrReport, Vec<usize>)> {
    let response = vec![0.0_f64; rows];
    let ids = (0..columns).collect::<Vec<_>>();
    let source = FixedBlockMomentSource::Dense {
        values: FiniteDenseRows::packed(values, columns),
        rows,
        stored_columns: columns,
        selected_columns: None,
        response: (&response).into(),
        weights: BorrowedMapWeights::Unit,
        solver_weight_divisor: 1.0,
        column_scales: Some(scales),
    };
    let mut stored_row = vec![0.0_f64; columns];
    let mut qr = vec![0.0_f64; columns * columns];
    let mut rhs = vec![0.0_f64; columns];
    let mut trailing = vec![0.0_f64; columns];
    let mut permutation = vec![usize::MAX; columns];
    let report = factor_rank_revealing_retained_qr(
        source,
        &ids,
        relative_threshold,
        &mut stored_row,
        &mut qr,
        &mut rhs,
        &mut trailing,
        &mut permutation,
        None,
    )?;
    Ok((report, permutation))
}

fn retained_qr_solution(
    values: &[f64],
    response: &[f64],
    rows: usize,
    columns: usize,
    scales: &[f64],
    ids: &[usize],
    relative_threshold: f64,
) -> Result<(RankRevealingQrReport, Vec<usize>, Vec<f64>)> {
    let source = FixedBlockMomentSource::Dense {
        values: FiniteDenseRows::packed(values, columns),
        rows,
        stored_columns: columns,
        selected_columns: None,
        response: (response).into(),
        weights: BorrowedMapWeights::Unit,
        solver_weight_divisor: 1.0,
        column_scales: Some(scales),
    };
    let mut stored_row = vec![0.0_f64; columns];
    let mut qr = vec![0.0_f64; columns * columns];
    let mut rhs = vec![0.0_f64; columns];
    let mut trailing = vec![0.0_f64; columns];
    let mut permutation = vec![usize::MAX; columns];
    let report = factor_rank_revealing_retained_qr(
        source,
        ids,
        relative_threshold,
        &mut stored_row,
        &mut qr,
        &mut rhs,
        &mut trailing,
        &mut permutation,
        None,
    )?;
    let mut pivot_solution = vec![0.0_f64; report.rank];
    let mut selected_ids = vec![usize::MAX; report.rank];
    let mut solution = vec![0.0_f64; report.rank];
    if !solve_rank_revealing_qr_leading(
        &qr,
        &rhs,
        &permutation,
        ids,
        report.rank,
        &mut pivot_solution,
        &mut selected_ids,
        &mut solution,
    )? {
        return Err(invalid_data("test retained QR solution did not certify"));
    }
    Ok((report, selected_ids, solution))
}

fn retained_qr_direct_span_certificate(
    factor_values: &[f64],
    certificate_values: &[f64],
    rows: usize,
    columns: usize,
    scales: &[f64],
    relative_threshold: f64,
) -> Result<(RankRevealingQrReport, DroppedSpanCertificate)> {
    let response = vec![0.0_f64; rows];
    let ids = (0..columns).collect::<Vec<_>>();
    let factor_source = FixedBlockMomentSource::Dense {
        values: FiniteDenseRows::packed(factor_values, columns),
        rows,
        stored_columns: columns,
        selected_columns: None,
        response: (&response).into(),
        weights: BorrowedMapWeights::Unit,
        solver_weight_divisor: 1.0,
        column_scales: Some(scales),
    };
    let mut stored_row = vec![0.0_f64; columns];
    let mut standardized_row = vec![0.0_f64; columns];
    let mut qr = vec![0.0_f64; columns * columns];
    let mut rhs = vec![0.0_f64; columns];
    let mut permutation = vec![usize::MAX; columns];
    let report = factor_rank_revealing_retained_qr(
        factor_source,
        &ids,
        relative_threshold,
        &mut stored_row,
        &mut qr,
        &mut rhs,
        &mut standardized_row,
        &mut permutation,
        None,
    )?;
    let dropped = columns - report.rank;
    let mut projection = vec![0.0_f64; report.rank * dropped];
    let mut residual_norms = vec![0.0_f64; dropped];
    let certificate_source = FixedBlockMomentSource::Dense {
        values: FiniteDenseRows::packed(certificate_values, columns),
        rows,
        stored_columns: columns,
        selected_columns: None,
        response: (&response).into(),
        weights: BorrowedMapWeights::Unit,
        solver_weight_divisor: 1.0,
        column_scales: Some(scales),
    };
    let certificate = certify_retained_qr_dropped_span(
        certificate_source,
        &qr,
        &permutation,
        report.rank,
        report.initial_norm,
        relative_threshold,
        &mut stored_row,
        &mut standardized_row,
        &mut projection,
        &mut residual_norms,
    )?;
    Ok((report, certificate))
}

#[test]
fn scratch_qr_scan_matches_dense_weighted_rank_and_bounds_reads() {
    let rows = 519;
    let stored = 6;
    let selected = [0, 2, 5];
    let ids = [7, 2, 11];
    let weights: Vec<_> = (0..rows).map(|r| 0.5 + (r % 13) as f64).collect();
    let scales = [0.125, 3.0, 8.0];
    for deficient in [false, true] {
        let mut values = Vec::new();
        let mut response = Vec::new();
        for r in 0..rows {
            let a = (r % 17) as f64 - 8.0;
            let b = (r % 23) as f64 - 11.0;
            let c = if deficient {
                2.0 * a - b
            } else {
                (r % 31) as f64
            };
            values.extend([a, 99.0, b, -11.0, 12.0, c]);
            response.push(a - 0.5 * b + 0.25 * c);
        }
        let mut scratch = PreparedOlsScratch::create_row_major(rows, stored).unwrap();
        scratch.write_linear(0, &values).unwrap();
        let sources = [
            FixedBlockMomentSource::Dense {
                values: FiniteDenseRows::packed(&values, stored),
                rows,
                stored_columns: stored,
                selected_columns: Some(&selected),
                response: (&response).into(),
                weights: BorrowedMapWeights::Provided(&weights),
                solver_weight_divisor: 7.0,
                column_scales: Some(&scales),
            },
            FixedBlockMomentSource::Scratch {
                values: &scratch,
                selected_columns: Some(&selected),
                response: (&response).into(),
                weights: BorrowedMapWeights::Provided(&weights),
                solver_weight_divisor: 7.0,
                column_scales: Some(&scales),
            },
        ];
        let mut results = Vec::new();
        for source in sources {
            let mut row = [0.0; 6];
            let mut qr = [0.0; 9];
            let mut rhs = [0.0; 3];
            let mut trailing = [0.0; 3];
            let mut permutation = [0; 3];
            let report = factor_rank_revealing_retained_qr(
                source,
                &ids,
                1.0e-10,
                &mut row,
                &mut qr,
                &mut rhs,
                &mut trailing,
                &mut permutation,
                None,
            )
            .unwrap();
            assert_eq!(report.rank, if deficient { 2 } else { 3 });
            results.push((report, qr, rhs, permutation));
        }
        assert_eq!(results[0], results[1]);
        assert_eq!(
            scratch.shared_read_chunks_for_test(),
            (rows * stored).div_ceil(1024) as u64
        );
    }
}

#[test]
fn retained_qr_drops_general_exact_linear_combinations() {
    let values = [-5.0_f64, -1.0, -7.0, 7.0, 4.0, 2.0, 8.0, -10.0, 46.0];
    let mut scales = [0.0_f64; 3];
    for column in 0..3 {
        scales[column] = (0..3)
            .map(|row| values[row * 3 + column])
            .fold(0.0_f64, fedemean::numerics::hypot);
    }
    let threshold = 128.0 * f64::EPSILON * 3.0;
    let (report, permutation) =
        retained_qr_report(&values, 3, 3, &scales, threshold).expect("retained QR");
    assert_eq!(report.rank, 2);
    assert!(!report.ambiguous);
    assert!(report.maximum_dropped_relative_norm <= threshold);
    assert_eq!(&permutation[..2], &[0, 1]);
}

#[test]
fn retained_qr_is_canonical_under_row_and_column_permutations() {
    let first = [3.0_f64, -2.0, 4.0, 1.0, -5.0];
    let second = [1.0_f64, 2.0, -1.0, 3.0, 4.0];
    let dependent = std::array::from_fn::<_, 5, _>(|row| 2.0 * first[row] - 3.0 * second[row]);
    let response = std::array::from_fn::<_, 5, _>(|row| 4.0 * first[row] + 0.5 * second[row]);
    let columns = [&first, &second, &dependent];
    let values = (0..first.len())
        .flat_map(|row| columns.iter().map(move |column| column[row]))
        .collect::<Vec<_>>();
    let scales = columns
        .iter()
        .map(|column| {
            column
                .iter()
                .copied()
                .fold(0.0_f64, fedemean::numerics::hypot)
        })
        .collect::<Vec<_>>();
    let ids = [20_usize, 10, 5];
    let threshold = 128.0 * f64::EPSILON * columns.len() as f64;
    let reference = retained_qr_solution(
        &values,
        &response,
        first.len(),
        columns.len(),
        &scales,
        &ids,
        threshold,
    )
    .expect("canonical retained QR");
    assert_eq!(reference.0.rank, 2);
    assert!(!reference.0.ambiguous);

    let physical_order = [2_usize, 0, 1];
    let permuted_values = (0..first.len())
        .flat_map(|row| {
            physical_order
                .iter()
                .map(move |column| columns[*column][row])
        })
        .collect::<Vec<_>>();
    let permuted_scales = physical_order
        .iter()
        .map(|column| scales[*column])
        .collect::<Vec<_>>();
    let permuted_ids = physical_order
        .iter()
        .map(|column| ids[*column])
        .collect::<Vec<_>>();
    let column_permuted = retained_qr_solution(
        &permuted_values,
        &response,
        first.len(),
        columns.len(),
        &permuted_scales,
        &permuted_ids,
        threshold,
    )
    .expect("column-permuted retained QR");

    let reversed_values = (0..first.len())
        .rev()
        .flat_map(|row| columns.iter().map(move |column| column[row]))
        .collect::<Vec<_>>();
    let reversed_response = response.iter().copied().rev().collect::<Vec<_>>();
    let row_permuted = retained_qr_solution(
        &reversed_values,
        &reversed_response,
        first.len(),
        columns.len(),
        &scales,
        &ids,
        threshold,
    )
    .expect("row-permuted retained QR");

    for candidate in [&column_permuted, &row_permuted] {
        assert_eq!(candidate.0.rank, reference.0.rank);
        assert!(!candidate.0.ambiguous);
        assert_eq!(candidate.1, reference.1);
        for (actual, expected) in candidate.2.iter().zip(&reference.2) {
            assert_close(*actual, *expected, 2.0e-12);
        }
    }
}

#[test]
fn retained_qr_handles_extreme_finite_column_scales() {
    let first = [1.0_f64, -2.0, 3.0, -4.0];
    let second = [2.0_f64, 1.0, -1.0, 3.0];
    let huge = 1.0e150_f64;
    let tiny = 1.0e-150_f64;
    let values = (0..first.len())
        .flat_map(|row| [huge * first[row], tiny * second[row]])
        .collect::<Vec<_>>();
    let response = std::array::from_fn::<_, 4, _>(|row| first[row] - second[row]);
    let scales = [
        first
            .iter()
            .copied()
            .fold(0.0_f64, fedemean::numerics::hypot)
            * huge,
        second
            .iter()
            .copied()
            .fold(0.0_f64, fedemean::numerics::hypot)
            * tiny,
    ];
    let (report, selected, solution) = retained_qr_solution(
        &values,
        &response,
        first.len(),
        2,
        &scales,
        &[0, 1],
        128.0 * f64::EPSILON * first.len() as f64,
    )
    .expect("extreme-scale retained QR");
    assert_eq!(report.rank, 2);
    assert!(!report.ambiguous);
    assert_eq!(selected, [0, 1]);
    assert!(solution.iter().all(|value| value.is_finite()));
}

#[test]
fn retained_qr_covers_single_column_and_clear_threshold_sides() {
    let (single, _) = retained_qr_report(&[3.0_f64, -4.0], 2, 1, &[5.0], 1.0e-10)
        .expect("single-column retained QR");
    assert_eq!(single.rank, 1);
    assert!(!single.ambiguous);

    let relative_threshold = 0.25_f64;
    for (multiplier, expected_rank) in [
        (1.0 - 128.0 * f64::EPSILON, 1_usize),
        (1.0 + 128.0 * f64::EPSILON, 2_usize),
    ] {
        let values = [1.0_f64, 0.0, 0.0, relative_threshold * multiplier];
        let (report, _) = retained_qr_report(&values, 2, 2, &[1.0, 1.0], relative_threshold)
            .expect("clear threshold-side retained QR");
        assert_eq!(report.rank, expected_rank);
        assert!(!report.ambiguous);
    }
}

#[test]
fn retained_qr_rank_decision_is_invariant_to_exact_column_scaling() {
    let first = [3.439_211_223_327_039_f64, -7.017_100_583_832_24];
    let multiplier = 6.285_119_033_872_249_f64;
    let values = [
        first[0],
        multiplier * first[0],
        first[1],
        multiplier * first[1],
    ];
    let scales = [
        first.into_iter().fold(0.0_f64, fedemean::numerics::hypot),
        values
            .iter()
            .copied()
            .skip(1)
            .step_by(2)
            .fold(0.0_f64, fedemean::numerics::hypot),
    ];
    let threshold = 128.0 * f64::EPSILON * 2.0;
    let (report, _) =
        retained_qr_report(&values, 2, 2, &scales, threshold).expect("scaled retained QR");
    assert_eq!(report.rank, 1);
    assert!(!report.ambiguous);
    assert!(report.maximum_dropped_relative_norm <= threshold);
}

#[test]
fn retained_qr_marks_both_sides_of_rank_threshold_ambiguous() {
    let relative_threshold = 0.25_f64;
    for multiplier in [1.0 - 32.0 * f64::EPSILON, 1.0 + 32.0 * f64::EPSILON] {
        let values = [1.0_f64, 0.0, 0.0, relative_threshold * multiplier];
        let (report, _) = retained_qr_report(&values, 2, 2, &[1.0, 1.0], relative_threshold)
            .expect("guard-band retained QR");
        assert!(report.ambiguous);
    }
}

#[test]
fn retained_qr_handles_exact_deficiency_above_sixty_four_columns() {
    let independent = 65_usize;
    let columns = independent + 1;
    let rows = independent;
    let mut values = vec![0.0_f64; rows * columns];
    for row in 0..rows {
        values[row * columns + row] = 1.0;
        values[row * columns + independent] = f64::from(row == 0);
    }
    let scales = vec![1.0_f64; columns];
    let threshold = 128.0 * f64::EPSILON * columns as f64;
    let (report, permutation) =
        retained_qr_report(&values, rows, columns, &scales, threshold).expect("wide retained QR");
    assert_eq!(report.rank, independent);
    assert!(!report.ambiguous);
    assert_eq!(
        &permutation[..independent],
        &(0..independent).collect::<Vec<_>>()
    );
}

#[test]
fn retained_qr_rejects_nonfinite_observation_space_inputs() {
    let result = retained_qr_report(&[1.0_f64, f64::NAN], 1, 2, &[1.0, 1.0], 1.0e-10);
    assert!(result.is_err());
}

#[test]
fn pivoted_and_canonical_factorizations_share_the_same_primitives() {
    let source_gram = [4.0_f64, 1.0, 1.0, 3.0];
    let source_rhs = [2.0_f64, -1.0];
    for policy in [
        CholeskyPivotPolicy::Canonical,
        CholeskyPivotPolicy::MaximumDiagonal,
    ] {
        let mut gram = source_gram;
        let mut rhs = source_rhs;
        let mut factor = [0.0; 4];
        let mut permutation = [0_usize; 2];
        let report = factor_guarded_cholesky_in_place(
            &mut gram,
            &mut rhs,
            &mut factor,
            &mut permutation,
            policy,
            1e-14,
        )
        .expect("factor")
        .expect("positive definite");
        assert!(report.minimum_pivot > 0.0);
        let mut forward = [0.0; 2];
        let mut solution = [0.0; 2];
        assert!(solve_guarded_cholesky(&factor, &rhs, &mut forward, &mut solution).expect("solve"));
        let mut inverse_lower = [0.0; 4];
        let mut inverse = [0.0; 4];
        assert!(
            invert_guarded_cholesky(&factor, &permutation, &mut inverse_lower, &mut inverse,)
                .expect("inverse")
        );
        let positions = [0_usize, 1];
        assert!(
            inverse_relative_infinity_norm(&source_gram, 2, &positions, &inverse, false, None)
                .expect("left certificate")
                < 1e-14
        );
        assert!(
            inverse_relative_infinity_norm(&source_gram, 2, &positions, &inverse, true, None)
                .expect("right certificate")
                < 1e-14
        );
    }
}

#[test]
fn paired_inverse_certificate_preserves_both_norms_and_fallbacks() {
    // Only the selected Gram block is finite and symmetric. Deliberately
    // perturb the inverse so the two sides have distinct nonzero norms.
    #[rustfmt::skip]
    let gram = [
        5.25, f64::NAN, 0.125, -0.8,
        f64::INFINITY, 4.0, -2.0, 1.0,
        0.125, -3.0, 2.25, 0.675,
        -0.8, -2.5, 0.675, 3.75,
    ];
    let positions = [3, 0, 2];
    let inverse = [
        0.31, 0.065, -0.113, 0.065, 0.245, 0.018, -0.113, 0.018, 0.65,
    ];
    let mut asymmetric_gram = gram;
    asymmetric_gram[3] = -0.95;
    let mut asymmetric_inverse = inverse;
    asymmetric_inverse[1] = -0.075;
    for (gram, inverse, scratch_len, paired) in [
        (gram, inverse, 6, true),
        (gram, inverse, 5, false),
        (asymmetric_gram, inverse, 6, false),
        (gram, asymmetric_inverse, 6, false),
    ] {
        let mut scratch = [f64::NAN; 6];
        let result = certify_finite_inverse(
            &gram,
            4,
            &positions,
            &inverse,
            3,
            &mut scratch[..scratch_len],
        )
        .unwrap();
        let left =
            inverse_relative_infinity_norm(&gram, 4, &positions, &inverse, false, None).unwrap();
        let right =
            inverse_relative_infinity_norm(&gram, 4, &positions, &inverse, true, None).unwrap();
        assert_ne!(left, right);
        assert_eq!(result.left.to_bits(), left.to_bits());
        assert_eq!(result.right.to_bits(), right.to_bits());
        assert_eq!(scratch[0].is_finite(), paired);
    }
}

#[test]
fn paired_inverse_certificate_preserves_invalid_data_errors() {
    let positions = [1, 0];
    let inverse = [1.0, 0.5, 0.5, 1.0];
    for gram in [[f64::NAN, 1.0, 1.0, 1.0], [f64::MAX; 4]] {
        let original = inverse_relative_infinity_norm(&gram, 2, &positions, &inverse, false, None)
            .unwrap_err();
        let error = certify_finite_inverse(&gram, 2, &positions, &inverse, 2, &mut [0.0; 4])
            .err()
            .expect("invalid product should fail certification");
        assert_eq!(error, original);
    }
}

#[test]
fn retained_qr_direct_span_accepts_an_exact_dependency() {
    let values = [1.0_f64, 1.0, 0.0, 0.0];
    let threshold = 1.0e-8;
    let (report, certificate) =
        retained_qr_direct_span_certificate(&values, &values, 2, 2, &[1.0, 1.0], threshold)
            .expect("direct retained span certificate");
    assert_eq!(report.rank, 1);
    assert_eq!(certificate.maximum_dropped_relative_norm, 0.0);
    assert!(!certificate.ambiguous);
    assert!(certificate.passed);
}

#[test]
fn retained_qr_direct_span_rejects_a_perturbed_omitted_direction() {
    let factor_values = [1.0_f64, 1.0, 0.0, 0.0];
    let certificate_values = [1.0_f64, 1.0, 0.0, 0.5];
    let (report, certificate) = retained_qr_direct_span_certificate(
        &factor_values,
        &certificate_values,
        2,
        2,
        &[1.0, 1.0],
        1.0e-8,
    )
    .expect("direct retained span certificate");
    assert_eq!(report.rank, 1);
    assert_eq!(certificate.maximum_dropped_relative_norm, 0.5);
    assert!(!certificate.ambiguous);
    assert!(!certificate.passed);
}

#[test]
fn retained_qr_direct_span_fails_closed_in_the_threshold_guard() {
    let factor_values = [1.0_f64, 1.0, 0.0, 0.0];
    let relative_threshold = 0.25_f64;
    for multiplier in [1.0 - 32.0 * f64::EPSILON, 1.0 + 32.0 * f64::EPSILON] {
        let certificate_values = [1.0_f64, 1.0, 0.0, relative_threshold * multiplier];
        let (report, certificate) = retained_qr_direct_span_certificate(
            &factor_values,
            &certificate_values,
            2,
            2,
            &[1.0, 1.0],
            relative_threshold,
        )
        .expect("guard-band direct retained span certificate");
        assert_eq!(report.rank, 1);
        assert!(certificate.ambiguous);
        assert!(!certificate.passed);
    }
}

#[test]
fn wide_private_moments_match_in_place_tiles_bit_for_bit() {
    for (columns, spaced, provided, scaled) in [
        (65_usize, false, false, false),
        (66, true, true, true),
        (97, false, true, true),
        (130, true, false, true),
    ] {
        let rows = 211;
        let selected = (0..columns)
            .map(|column| if spaced { 2 * column + 1 } else { column })
            .collect::<Vec<_>>();
        let stored = selected[columns - 1] + 1;
        let scales = (0..columns)
            .map(|column| 2.0_f64.powi((column % 5) as i32 * 7 - 14) * 1.3)
            .collect::<Vec<_>>();
        let weights = (0..rows)
            .map(|row| 0.25 + (row % 7) as f64 * 0.375)
            .collect::<Vec<_>>();
        let response = (0..rows)
            .map(|row| {
                if row == 3 {
                    -0.0
                } else {
                    (row as f64 * 0.37).cos()
                }
            })
            .collect::<Vec<_>>();
        let mut values = vec![f64::NAN; rows * stored];
        for row in 0..rows {
            for column in 0..columns {
                values[row * stored + selected[column]] = if (row + column) % 23 == 0 {
                    -0.0
                } else {
                    ((row * 17 + column * 13) as f64 * 0.731).sin() * 1e3
                };
            }
        }
        let weights_arg = if provided {
            BorrowedMapWeights::Provided(&weights)
        } else {
            BorrowedMapWeights::Unit
        };
        let divisor = if provided { 1.75 } else { 1.0 };
        let source = FixedBlockMomentSource::Dense {
            values: FiniteDenseRows::packed(&values, stored),
            rows,
            stored_columns: stored,
            selected_columns: if spaced { Some(&selected) } else { None },
            response: (&response).into(),
            weights: weights_arg,
            solver_weight_divisor: divisor,
            column_scales: if scaled { Some(&scales) } else { None },
        };
        let bits = |values: &[f64]| values.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        for threads in [1, 2, 4, 8] {
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(threads, true).unwrap())
                .install(|| {
                    let layout_source = validate_moment_source(source).unwrap();
                    let layout = layout_source.layout();
                    assert!(layout.columns_per_partition > 1);
                    assert!(wide_private::reserve(columns, layout).is_some());
                    let mut rhs = vec![0.0; columns];
                    let mut gram = vec![0.0; columns * columns];
                    fill_finite_wls_moments(layout_source, &mut [], &mut rhs, &mut gram).unwrap();
                    let mut old_rhs = vec![0.0; columns];
                    let mut old_gram = vec![0.0; columns * columns];
                    fill_retained_dense_standardized_tiles(
                        FiniteDenseRows::packed(&values, stored),
                        if spaced { Some(&selected) } else { None },
                        (&response).into(),
                        weights_arg,
                        divisor,
                        if scaled { Some(&scales) } else { None },
                        layout,
                        &mut old_rhs,
                        &mut old_gram,
                    )
                    .unwrap();
                    for left in 0..columns {
                        for right in (left + 1)..columns {
                            old_gram[right * columns + left] = old_gram[left * columns + right];
                        }
                    }
                    assert_eq!(bits(&rhs), bits(&old_rhs));
                    assert_eq!(bits(&gram), bits(&old_gram));
                })
                .unwrap();
        }
    }
}

/// Rank-revealing QR of weighted standardized columns with chosen canonical
/// identities; returns the report and the canonical identities of the kept
/// columns in pivot order.
fn weighted_tie_report(
    values: &[f64],
    rows: usize,
    columns: usize,
    weights: &[f64],
    ids: &[usize],
) -> (RankRevealingQrReport, Vec<usize>) {
    let divisor = weights.iter().sum::<f64>() / rows as f64;
    let scales = (0..columns)
        .map(|column| {
            (0..rows).fold(0.0_f64, |norm, row| {
                fedemean::numerics::hypot(
                    norm,
                    (weights[row] / divisor).sqrt() * values[row * columns + column],
                )
            })
        })
        .collect::<Vec<_>>();
    let response = vec![0.0_f64; rows];
    let source = FixedBlockMomentSource::Dense {
        values: FiniteDenseRows::packed(values, columns),
        rows,
        stored_columns: columns,
        selected_columns: None,
        response: (&response).into(),
        weights: BorrowedMapWeights::Provided(weights),
        solver_weight_divisor: divisor,
        column_scales: Some(&scales),
    };
    let mut stored_row = vec![0.0_f64; columns];
    let mut qr = vec![0.0_f64; columns * columns];
    let mut rhs = vec![0.0_f64; columns];
    let mut trailing = vec![0.0_f64; columns];
    let mut permutation = vec![usize::MAX; columns];
    let report = factor_rank_revealing_retained_qr(
        source,
        ids,
        128.0 * f64::EPSILON * rows.max(columns) as f64,
        &mut stored_row,
        &mut qr,
        &mut rhs,
        &mut trailing,
        &mut permutation,
        None,
    )
    .expect("weighted retained QR");
    let kept = permutation[..report.rank]
        .iter()
        .map(|&position| ids[position])
        .collect();
    (report, kept)
}

/// `x17 = c · x1` with fifteen other regressors, as in `ols_weighted_schur`:
/// the two columns tie in exact arithmetic, and over 16,512 weighted rows the
/// rounding of the streamed Givens QR moves their computed norms apart by up
/// to about 80 ulps (more than 64 for these seeds). Under every scaling and
/// storage order the earliest
/// canonical column is kept and the later one dropped, as `regress` does;
/// the rank is unchanged.
#[test]
fn retained_qr_keeps_the_earliest_of_tied_columns_under_scalings_and_permutations() {
    const ROWS: usize = 16_512;
    const COLUMNS: usize = 17;
    let weights = (0..ROWS)
        .map(|row| 1.0 + ((row + 1) % 7) as f64)
        .collect::<Vec<_>>();
    // Seeds whose rounding separates the tied norms by more than 64 ulps.
    for seed in [28_u64, 31, 36] {
        let mut state = 0x1234_5678_u64 ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut uniform = || {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            ((z ^ (z >> 31)) >> 11) as f64 / (1_u64 << 53) as f64
        };
        let mut base = vec![0.0_f64; ROWS * 16];
        for value in &mut base {
            let (u, v) = (uniform() + f64::EPSILON, uniform());
            *value = (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos();
        }
        for multiplier in [2.0_f64, 3.0, -0.5, 1.0e-6, 6.25e7] {
            // Canonical identity 0 is x1 and 16 its multiple; storage orders:
            // canonical, reversed, and the multiple stored first.
            let orders: [Vec<usize>; 3] = [
                (0..COLUMNS).collect(),
                (0..COLUMNS).rev().collect(),
                std::iter::once(16).chain(0..16).collect(),
            ];
            for order in orders {
                let mut values = vec![0.0_f64; ROWS * COLUMNS];
                for row in 0..ROWS {
                    for (position, &id) in order.iter().enumerate() {
                        values[row * COLUMNS + position] = if id == 16 {
                            multiplier * base[row * 16]
                        } else {
                            base[row * 16 + id]
                        };
                    }
                }
                let (report, kept) = weighted_tie_report(&values, ROWS, COLUMNS, &weights, &order);
                assert_eq!(report.rank, 16, "seed {seed}, multiplier {multiplier}");
                assert!(
                    kept.contains(&0) && !kept.contains(&16),
                    "seed {seed}, multiplier {multiplier}, order {order:?}: kept {kept:?}"
                );
            }
        }
    }
}

/// Exactly representable norms `1` and `1 + δ` of one direction: within the
/// tie band (`max(64, rows + columns)` ulps of the column scale) the earliest
/// canonical column is kept; just outside it the larger norm is kept, as
/// before the tie rule.
#[test]
fn retained_qr_tie_band_follows_the_rounding_of_the_column_norms() {
    for rows in [2_usize, 4_094] {
        let band = (rows + 2).max(64) as f64 * f64::EPSILON;
        for (delta, earliest_kept) in [(0.5 * band, true), (2.0 * band, false)] {
            for ids in [[0_usize, 1], [1, 0]] {
                let mut values = vec![0.0_f64; rows * 2];
                values[0] = 1.0;
                values[1] = 1.0 + delta;
                let response = vec![0.0_f64; rows];
                let source = FixedBlockMomentSource::Dense {
                    values: FiniteDenseRows::packed(&values, 2),
                    rows,
                    stored_columns: 2,
                    selected_columns: None,
                    response: (&response).into(),
                    weights: BorrowedMapWeights::Unit,
                    solver_weight_divisor: 1.0,
                    column_scales: Some(&[1.0, 1.0]),
                };
                let mut permutation = vec![usize::MAX; 2];
                let report = factor_rank_revealing_retained_qr(
                    source,
                    &ids,
                    128.0 * f64::EPSILON * rows as f64,
                    &mut [0.0; 2],
                    &mut [0.0; 4],
                    &mut [0.0; 2],
                    &mut [0.0; 2],
                    &mut permutation,
                    None,
                )
                .expect("near-tie retained QR");
                assert_eq!(report.rank, 1);
                let kept = ids[permutation[0]];
                let expected = if earliest_kept { 0 } else { ids[1] };
                assert_eq!(kept, expected, "rows {rows}, delta {delta:e}, ids {ids:?}");
            }
        }
    }
}

#[test]
fn blocked_givens_matches_streaming_bits_across_panels_and_workers() {
    let (rows, columns) = (157usize, 71usize);
    let selected: Vec<_> = (0..columns).map(|j| j + j / 8).collect();
    let stored = selected[columns - 1] + 2;
    let ids: Vec<_> = (0..columns).collect();
    let scales: Vec<_> = (0..columns)
        .map(|j| [2.0f64.powi(-70), 1.0, 2.0f64.powi(63)][j % 3])
        .collect();
    let weights: Vec<_> = (0..rows)
        .map(|i| [0.25, 0.5, 1.0, 2.0, 4.0][i % 5])
        .collect();
    let response: Vec<_> = (0..rows)
        .map(|i| ((i * 257 + 61) % 821) as f64 - 410.25)
        .collect();
    let mut values = vec![f64::NAN; rows * stored];
    for i in 0..rows {
        for j in 0..columns {
            // The final standardized column duplicates the first, and the
            // leading rows contain an identity on the other 70 columns.
            let base = if j + 1 == columns { 0 } else { j };
            let x = if i < columns - 1 {
                if i == base {
                    1.0
                } else if i == 0 && base == 1 {
                    f64::from_bits(16)
                } else {
                    -0.0
                }
            } else if (i + base) % 5 == 0 {
                -0.0
            } else {
                (((i + 71) * (base + 43) % 257) as f64 - 113.25) / 128.0
            };
            values[i * stored + selected[j]] = x * scales[j];
        }
    }
    let source = FixedBlockMomentSource::Dense {
        values: FiniteDenseRows::packed(&values, stored),
        rows,
        stored_columns: stored,
        selected_columns: Some(&selected),
        response: (&response).into(),
        weights: BorrowedMapWeights::Provided(&weights),
        solver_weight_divisor: 2.0,
        column_scales: Some(&scales),
    };
    assert!(blocked_givens_eligible(source));
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let run_factor = |compression: Option<RetainedQrCompression<'_>>| {
        let mut qr = vec![0.; columns * columns];
        let mut rhs = vec![0.; columns];
        let mut permutation = vec![0; columns];
        let mut norms = vec![0.; columns];
        let report = factor_rank_revealing_retained_qr(
            source,
            &ids,
            1e-10,
            &mut vec![0.; stored],
            &mut qr,
            &mut rhs,
            &mut norms,
            &mut permutation,
            compression,
        )
        .unwrap();
        (report, permutation, bits(&qr), bits(&rhs), bits(&norms))
    };
    let mut scalar_qr = vec![0.; columns * columns];
    let mut scalar_rhs = vec![0.; columns];
    streamed_qr::scalar(
        source,
        &mut vec![0.; stored],
        &mut vec![0.; columns],
        &mut scalar_qr,
        &mut scalar_rhs,
    )
    .unwrap();
    let expected_qr = bits(&scalar_qr);
    let expected_rhs = bits(&scalar_rhs);
    let expected_factor = run_factor(None);
    assert_eq!(expected_factor.0.rank, columns - 1);
    assert_eq!(expected_factor.1[columns - 1], columns - 1);
    for workers in [1, 8] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                let mut workspace = vec![f64::NAN; columns * columns];
                let mut qr = vec![0.; columns * columns];
                let mut rhs = vec![0.; columns];
                streamed_qr::blocked(
                    source,
                    &mut vec![0.; stored],
                    &mut workspace,
                    &mut qr,
                    &mut rhs,
                )
                .unwrap();
                assert_eq!(bits(&qr), expected_qr);
                assert_eq!(bits(&rhs), expected_rhs);
                assert_eq!(
                    run_factor(Some(RetainedQrCompression::BlockedGivens(&mut workspace))),
                    expected_factor
                );
            });
    }
}

#[test]
fn blocked_givens_replays_the_first_streaming_failure_in_source_order() {
    let (rows, columns) = (17, 65);
    let mut values = vec![0.; rows * columns];
    values[0] = f64::MAX;
    values[columns] = f64::MAX;
    values[9 * columns] = f64::NAN;
    let response = vec![0.; rows];
    let source = FixedBlockMomentSource::Dense {
        values: FiniteDenseRows::packed(&values, columns),
        rows,
        stored_columns: columns,
        selected_columns: None,
        response: (&response).into(),
        weights: BorrowedMapWeights::Unit,
        solver_weight_divisor: 1.0,
        column_scales: None,
    };
    let ids: Vec<_> = (0..columns).collect();
    let mut workspace = vec![0.; columns * columns];
    for compression in [
        None,
        Some(RetainedQrCompression::BlockedGivens(&mut workspace)),
    ] {
        let error = factor_rank_revealing_retained_qr(
            source,
            &ids,
            1e-10,
            &mut vec![0.; columns],
            &mut vec![0.; columns * columns],
            &mut vec![0.; columns],
            &mut vec![0.; columns],
            &mut vec![0; columns],
            compression,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert_eq!(
            error.message,
            "retained finite-WLS streaming QR rotation is invalid"
        );
    }
}

#[test]
fn blocked_givens_wave_cancellation_isolated() {
    const CHILD: &str = "FEREG_BLOCKED_GIVENS_CANCEL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ols::finite_wls_kernel::tests::blocked_givens_wave_cancellation_isolated",
            ])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let (rows, columns) = (51, 65);
    let values = vec![0.; rows * columns];
    let response = vec![0.; rows];
    let source = FixedBlockMomentSource::Dense {
        values: FiniteDenseRows::packed(&values, columns),
        rows,
        stored_columns: columns,
        selected_columns: None,
        response: (&response).into(),
        weights: BorrowedMapWeights::Unit,
        solver_weight_divisor: 1.0,
        column_scales: None,
    };
    let ids: Vec<_> = (0..columns).collect();
    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap()
        .install(|| {
            let factor = || {
                factor_rank_revealing_retained_qr(
                    source,
                    &ids,
                    1e-10,
                    &mut vec![0.; columns],
                    &mut vec![0.; columns * columns],
                    &mut vec![0.; columns],
                    &mut vec![0.; columns],
                    &mut vec![0; columns],
                    Some(RetainedQrCompression::BlockedGivens(&mut vec![
                        0.;
                        columns
                            * columns
                    ])),
                )
            };
            crate::interrupt::test_polls::cancel_at(Some(3));
            assert_eq!(factor().unwrap_err().code, ErrorCode::Interrupted);
            assert_eq!(
                crate::interrupt::test_polls::polls(),
                4,
                "cancellation must not replay the serial stream"
            );
            crate::interrupt::test_polls::cancel_at(None);
            assert_eq!(factor().unwrap().rank, 0);
        });
}
