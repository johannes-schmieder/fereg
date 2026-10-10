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
                    assert_eq!(rhs, expected_rhs);
                    assert_eq!(gram, expected_gram);
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
fn retained_dense_moment_cache_rejects_nonfinite_selected_values() {
    let response = [1.0; 4];
    let scales = [1.0; 8];
    for bad in [f64::NAN, f64::INFINITY] {
        let mut values = [1.0; 32];
        values[17] = bad;
        let source = FixedBlockMomentSource::Dense {
            values: FiniteDenseRows::packed(&values, 8),
            rows: 4,
            stored_columns: 8,
            selected_columns: None,
            response: (&response).into(),
            weights: BorrowedMapWeights::Unit,
            solver_weight_divisor: 1.0,
            column_scales: Some(&scales),
        };
        rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap()
            .install(|| {
                let layout_source = validate_moment_source(source).unwrap();
                let error =
                    fill_finite_wls_moments(layout_source, &mut [], &mut [0.0; 8], &mut [0.0; 64])
                        .unwrap_err();
                assert_eq!(error.code, ErrorCode::InvalidData);
            });
    }
}

#[test]
fn wide_projected_moments_are_bounded_deterministic_and_match_direct_sums() {
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
    )?;
    Ok((report, permutation))
}

#[allow(clippy::too_many_arguments)]
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
            .fold(0.0_f64, f64::hypot);
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
        .map(|column| column.iter().copied().fold(0.0_f64, f64::hypot))
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
        first.iter().copied().fold(0.0_f64, f64::hypot) * huge,
        second.iter().copied().fold(0.0_f64, f64::hypot) * tiny,
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
        first.into_iter().fold(0.0_f64, f64::hypot),
        values
            .iter()
            .copied()
            .skip(1)
            .step_by(2)
            .fold(0.0_f64, f64::hypot),
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
            inverse_relative_infinity_norm(&source_gram, 2, &positions, &inverse, 2, false,)
                .expect("left certificate")
                < 1e-14
        );
        assert!(
            inverse_relative_infinity_norm(&source_gram, 2, &positions, &inverse, 2, true,)
                .expect("right certificate")
                < 1e-14
        );
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
