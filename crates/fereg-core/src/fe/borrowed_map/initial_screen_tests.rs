//! The entry screen only skips a provably failing complete initial
//! certificate: every published bit equals the unscreened arm's.
use super::*;
use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|value| value.to_bits()).collect()
}

fn certificate_bits(certificate: &FeCertificate) -> [u64; 4] {
    [
        certificate.absolute_infinity_norm.to_bits(),
        certificate.relative_infinity_norm.to_bits(),
        certificate.relative_two_norm.to_bits(),
        u64::from(certificate.passed),
    ]
}

fn assert_same(screened: &CertifiedBorrowedMap, unscreened: &CertifiedBorrowedMap) {
    assert_eq!(
        bits(screened.residualized.as_slice()),
        bits(unscreened.residualized.as_slice())
    );
    assert_eq!(
        bits(&screened.residual_norms),
        bits(&unscreened.residual_norms)
    );
    assert_eq!(screened.report.iterations, unscreened.report.iterations);
    assert_eq!(screened.report.stop_reason, unscreened.report.stop_reason);
    assert_eq!(
        certificate_bits(&screened.report.certificate),
        certificate_bits(&unscreened.report.certificate)
    );
    assert_eq!(screened.coefficients, unscreened.coefficients);
}

/// The comparison is not vacuous: the screen rejects this initial matrix.
fn assert_screen_rejects(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: &DenseMatrix,
) {
    let weights = validate_weights(weights, values.nrows()).unwrap();
    let mut workspace = BorrowedMapWorkspace::new(
        design,
        values.nrows(),
        values.ncols(),
        &mut SystemWorkspaceReservation,
    )
    .unwrap();
    update_workspace_norms(values, weights, &mut workspace, true).unwrap();
    assert!(initial_screen::rejects(design, weights, values, 1e-8, &mut workspace).unwrap());
}

fn matrix(rows: usize, columns: usize, codes: &[u32], scale: f64) -> DenseMatrix {
    DenseMatrix::from_row_major(
        rows,
        columns,
        (0..rows * columns)
            .map(|i| match i % columns {
                0 => 1.0,
                1 if codes[i / columns] == 0 => 1e-200,
                _ => scale * ((i as f64 * 0.17).sin() + f64::from(codes[i / columns]) * 0.01),
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn binding_wrapper_screen_preserves_every_bit_across_workers() {
    for workers in [1, 2, 4, 8] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                for rows in [257, 16_385] {
                    let first: Vec<_> = (0..rows).map(|r| (r % 17) as u32).collect();
                    let second: Vec<_> = (0..rows).map(|r| ((r / 3) % 11) as u32).collect();
                    let first_index = LevelIndex::from_codes(&first, 17).unwrap();
                    let second_index = LevelIndex::from_codes(&second, 11).unwrap();
                    let first_source =
                        ValidatedBorrowedInterceptSource::new(&first, 17, &first_index).unwrap();
                    let second_source =
                        ValidatedBorrowedInterceptSource::new(&second, 11, &second_index).unwrap();
                    let weights: Vec<_> = (0..rows).map(|r| 0.5 + (r % 7) as f64 * 0.25).collect();
                    for effects in [1, 2] {
                        let mut sources = vec![first_source.effect()];
                        if effects == 2 {
                            sources.push(second_source.effect());
                        }
                        let design = BorrowedInterceptFeDesign::new(rows, sources).unwrap();
                        for columns in [1, 17, 65] {
                            for weighted in [false, true] {
                                assert_screen_rejects(
                                    &design,
                                    if weighted {
                                        BorrowedMapWeights::Provided(&weights)
                                    } else {
                                        BorrowedMapWeights::Unit
                                    },
                                    &matrix(rows, columns, &first, 3.0),
                                );
                                for retain in [false, true] {
                                    let map = |unscreened: bool| {
                                        let run = || {
                                            absorb_borrowed_intercept_map(
                                                &design,
                                                if weighted {
                                                    BorrowedMapWeights::Provided(&weights)
                                                } else {
                                                    BorrowedMapWeights::Unit
                                                },
                                                matrix(rows, columns, &first, 3.0),
                                                MapConfig::default(),
                                                false,
                                                retain,
                                            )
                                            .unwrap()
                                        };
                                        if unscreened {
                                            initial_screen::unscreened(run)
                                        } else {
                                            run()
                                        }
                                    };
                                    assert_same(&map(false), &map(true));
                                }
                            }
                        }
                    }
                }
            })
            .unwrap();
    }
}

#[test]
fn three_fe_pilot_screen_preserves_every_bit_across_workers() {
    for workers in [1, 2, 4, 8] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                let rows = 16_385;
                let codes: [Vec<u32>; 3] = [
                    (0..rows).map(|r| (r % 31) as u32).collect(),
                    (0..rows).map(|r| ((r / 31) % 29) as u32).collect(),
                    (0..rows).map(|r| ((r * 7 + r / 899) % 13) as u32).collect(),
                ];
                let levels = [31, 29, 13];
                let indexes: Vec<_> = codes
                    .iter()
                    .zip(levels)
                    .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).unwrap())
                    .collect();
                let sources: Vec<_> = codes
                    .iter()
                    .zip(levels)
                    .zip(&indexes)
                    .map(|((codes, levels), index)| {
                        ValidatedBorrowedInterceptSource::new(codes, levels, index).unwrap()
                    })
                    .collect();
                let design = BorrowedInterceptFeDesign::new(
                    rows,
                    sources.iter().map(|source| source.effect()).collect(),
                )
                .unwrap();
                for columns in [1, 18, 65] {
                    let original = matrix(rows, columns, &codes[0], 2.0);
                    assert_screen_rejects(&design, BorrowedMapWeights::Unit, &original);
                    for checkpoint in [1, 3] {
                        for retain in [false, true] {
                            let attempt = |unscreened: bool| {
                                let run = || {
                                    three_fe_map::attempt(
                                        &design,
                                        &original,
                                        MapConfig::default(),
                                        None,
                                        retain,
                                        0,
                                        checkpoint,
                                        true,
                                    )
                                    .unwrap()
                                };
                                if unscreened {
                                    initial_screen::unscreened(run)
                                } else {
                                    run()
                                }
                            };
                            let (screened, unscreened) = (attempt(false), attempt(true));
                            assert_eq!(screened.iterations, unscreened.iterations);
                            match (&screened.result, &unscreened.result) {
                                (Some(screened), Some(unscreened)) => {
                                    assert_same(screened, unscreened);
                                }
                                (None, None) => {}
                                _ => panic!("screen changed the pilot outcome"),
                            }
                        }
                    }
                }
            })
            .unwrap();
    }
}

#[test]
fn parallel_finite_scan_matches_the_serial_predicate_across_workers() {
    let rows = 40_000;
    for workers in [1, 2, 4, 8] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                for position in [None, Some(0), Some(17_321), Some(rows * 3 - 1)] {
                    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                        let mut values: Vec<_> =
                            (0..rows * 3).map(|i| (i as f64 * 0.37).cos()).collect();
                        if let Some(position) = position {
                            values[position] = bad;
                        }
                        let serial = values.iter().all(|value| value.is_finite());
                        let matrix = DenseMatrix::from_row_major(rows, 3, values).unwrap();
                        assert_eq!(matrix.all_finite(), serial);
                    }
                }
            })
            .unwrap();
    }
}
