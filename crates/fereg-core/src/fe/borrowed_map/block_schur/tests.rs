use super::*;
use crate::fe::ValidatedBorrowedInterceptSource;

/// Seeded codes with every level present and repeated row combinations.
fn codes(rows: usize, levels: &[usize], seed: u64) -> Vec<Vec<u32>> {
    let mut state = seed;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as usize
    };
    levels
        .iter()
        .enumerate()
        .map(|(effect, &count)| {
            (0..rows)
                .map(|row| {
                    if row < count {
                        row as u32
                    } else if effect == 0 {
                        // A panel-like eliminated effect with repeated rows.
                        (row / 3 % count) as u32
                    } else {
                        (next() % count) as u32
                    }
                })
                .collect()
        })
        .collect()
}

fn values(rows: usize, columns: usize, seed: u64) -> DenseMatrix {
    let mut state = seed;
    let data = (0..rows * columns)
        .map(|_| {
            state = state
                .wrapping_mul(2_862_933_555_777_941_757)
                .wrapping_add(3_037_000_493);
            ((state >> 11) as f64 / (1_u64 << 53) as f64) * 4.0 - 2.0
        })
        .collect();
    DenseMatrix::from_row_major(rows, columns, data).unwrap()
}

fn with_design(
    rows: usize,
    levels: &[usize],
    seed: u64,
    action: impl FnOnce(&BorrowedInterceptFeDesign<'_>, &[Vec<u32>]),
) {
    let codes = codes(rows, levels, seed);
    let indices: Vec<_> = codes
        .iter()
        .zip(levels)
        .map(|(c, &l)| LevelIndex::from_codes(c, l).unwrap())
        .collect();
    let sources: Vec<_> = codes
        .iter()
        .zip(levels)
        .zip(&indices)
        .map(|((c, &l), i)| ValidatedBorrowedInterceptSource::new(c, l, i).unwrap())
        .collect();
    let design =
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap();
    action(&design, &codes);
}

#[test]
fn elimination_matches_converged_map_and_returns_reproducing_coefficients() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    let rows = 900;
    for levels in [
        vec![120, 17],
        vec![150, 23, 6],
        vec![150, 23, 6, 11],
        vec![150, 23, 6, 11, 4],
    ] {
        with_design(
            rows,
            &levels,
            20261107 + levels.len() as u64,
            |design, codes| {
                let weights: Vec<f64> = (0..rows).map(|r| 0.25 + (r * 7 % 11) as f64).collect();
                for weighted in [false, true] {
                    let borrowed = if weighted {
                        BorrowedMapWeights::Provided(&weights)
                    } else {
                        BorrowedMapWeights::Unit
                    };
                    for columns in [1, 3, 11] {
                        let input = values(rows, columns, 7 + columns as u64);
                        let map = MapConfig {
                            tolerance: 1e-10,
                            ..MapConfig::default()
                        };
                        let Outcome::Done(result) = Cache::default()
                            .project(
                                design,
                                borrowed,
                                input.clone(),
                                map,
                                None,
                                Request {
                                    retain_coefficients: true,
                                    fresh_weights: false,
                                    coefficient_accuracy: weighted,
                                },
                            )
                            .unwrap()
                        else {
                            panic!("elimination declined: levels={levels:?} weighted={weighted}");
                        };
                        assert!(result.report().certified());
                        let oracle = absorb_borrowed_intercept_map(
                            design,
                            borrowed,
                            input.clone(),
                            MapConfig {
                                tolerance: 1e-13,
                                max_iterations: 200_000,
                                ..MapConfig::default()
                            },
                            false,
                            false,
                        )
                        .unwrap();
                        let (residual, _, coefficients) = result.into_full_parts();
                        let coefficients = coefficients.expect("retained coefficients");
                        for row in 0..rows {
                            for column in 0..columns {
                                let r = residual[(row, column)];
                                assert!(
                                (r - oracle.residualized()[(row, column)]).abs() < 1e-7,
                                "levels={levels:?} weighted={weighted} row={row} column={column}"
                            );
                                // v - D a reproduces the returned residual.
                                let mut fit = 0.0;
                                for (effect, effect_codes) in codes.iter().enumerate() {
                                    let base = column * coefficients.coefficients_per_rhs
                                        + coefficients.effect_offsets[effect];
                                    fit += coefficients.values[base + effect_codes[row] as usize];
                                }
                                assert!((input[(row, column)] - fit - r).abs() < 1e-9);
                            }
                        }
                    }
                }
            },
        );
    }
}

#[test]
fn declines_return_the_untouched_input() {
    with_design(400, &[80, 9, 5], 3, |design, _| {
        let input = values(400, 4, 11);
        // A one-iteration ceiling cannot reach the tolerance.
        let short = MapConfig {
            tolerance: 1e-14,
            max_iterations: 1,
            ..MapConfig::default()
        };
        match Cache::default()
            .project(
                design,
                BorrowedMapWeights::Unit,
                input.clone(),
                short,
                None,
                Request::default(),
            )
            .unwrap()
        {
            Outcome::Declined(values) => assert_eq!(values, input),
            Outcome::Done(_) => panic!("one iteration unexpectedly certified"),
        }
        // A budget below the admission bound declines before any work.
        let tiny = FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 1,
        };
        match Cache::default()
            .project(
                design,
                BorrowedMapWeights::Unit,
                input.clone(),
                MapConfig::default(),
                Some(tiny),
                Request::default(),
            )
            .unwrap()
        {
            Outcome::Declined(values) => assert_eq!(values, input),
            Outcome::Done(_) => panic!("refused budget admitted"),
        }
    });
}

#[test]
fn structure_covers_two_to_five_effects_only() {
    for count in [1_usize, 6] {
        let levels: Vec<usize> = (0..count).map(|e| 3 + e).collect();
        with_design(60, &levels, 5, |design, _| {
            assert!(Structure::build(design).unwrap().is_none());
        });
    }
}
