use super::*;
use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|value| value.to_bits()).collect()
}

/// Deterministic pseudo-random stream (SplitMix64).
struct Stream(u64);

impl Stream {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }
}

/// Codes of one design: crossed dense, scattered random and contiguous
/// (sorted) effects with unequal level sizes.
fn design_codes(kind: usize, rows: usize) -> ([Vec<u32>; 3], [usize; 3]) {
    let mut stream = Stream(0x5eed + kind as u64);
    match kind {
        0 => (
            [
                (0..rows).map(|r| (r % 31) as u32).collect(),
                (0..rows).map(|r| ((r / 3) % 29) as u32).collect(),
                (0..rows).map(|r| ((r / 7) % 13) as u32).collect(),
            ],
            [31, 29, 13],
        ),
        1 => {
            // Every level of each effect is occupied by its first rows.
            let levels = [97, 7, 211];
            let codes = std::array::from_fn(|effect| {
                (0..rows)
                    .map(|r| {
                        if r < levels[effect] {
                            r as u32
                        } else {
                            (stream.next() % levels[effect] as u64) as u32
                        }
                    })
                    .collect()
            });
            (codes, levels)
        }
        _ => (
            [
                (0..rows).map(|r| (r * 5 / rows) as u32).collect(),
                (0..rows).map(|r| ((r * 11 + r / 17) % 17) as u32).collect(),
                (0..rows).map(|r| (r % 2) as u32).collect(),
            ],
            [5, 17, 2],
        ),
    }
}

fn values(rows: usize, columns: usize, codes: &[Vec<u32>; 3], seed: u64) -> DenseMatrix {
    let mut stream = Stream(seed);
    let data = (0..rows)
        .flat_map(|r| {
            let base = codes[0][r] as f64 - 0.5 * codes[1][r] as f64 + 0.25 * codes[2][r] as f64;
            (0..columns)
                .map(|c| match c % 6 {
                    // An all-zero column keeps zero scales throughout.
                    5 => 0.0,
                    4 => 1.0e-200 * (stream.unit() - 0.5),
                    3 => 1.0e12 * (stream.unit() - 0.5) + base,
                    2 => base,
                    _ => (stream.unit() - 0.5) * (c + 1) as f64 + base,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    DenseMatrix::from_row_major(rows, columns, data).unwrap()
}

fn with_design(
    kind: usize,
    rows: usize,
    run: impl FnOnce(&BorrowedInterceptFeDesign<'_>, &[Vec<u32>; 3]),
) {
    let (codes, levels) = design_codes(kind, rows);
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
    let design =
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap();
    run(&design, &codes);
}

/// Two consecutive sweeps, recording coefficients when `retain`.
fn run_sweeps(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    input: &DenseMatrix,
    symmetric: bool,
    retain: bool,
    fused: bool,
) -> Result<(Vec<u64>, Option<Vec<u64>>)> {
    let weights = validate_weights(weights, input.nrows())?;
    let mut values = input.clone();
    let mut coefficients = retain
        .then(|| FeCoefficientBatch::intercept_zeros(design, input.ncols()))
        .transpose()?;
    if fused {
        let mut scratch = FusedSweepScratch::new(design, input.nrows(), input.ncols())?;
        for _ in 0..2 {
            sweep(
                design,
                weights,
                &mut values,
                &mut scratch,
                coefficients.as_mut(),
                symmetric,
            )?;
        }
    } else {
        let mut workspace = BorrowedMapWorkspace::new(
            design,
            input.nrows(),
            input.ncols(),
            &mut SystemWorkspaceReservation,
        )?;
        for _ in 0..2 {
            borrowed_residual_sweep_recording(
                design,
                weights,
                &mut values,
                &mut workspace,
                coefficients.as_mut(),
                symmetric,
            )?;
        }
    }
    Ok((
        bits(values.as_slice()),
        coefficients.map(|batch| bits(&batch.values)),
    ))
}

#[test]
fn fused_sweeps_are_bitwise_the_unfused_sweeps_at_every_worker_count() {
    let mut reference = std::collections::BTreeMap::new();
    for workers in [1, 2, 4, 8] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                for kind in 0..3 {
                    for rows in [211, 16_385] {
                        with_design(kind, rows, |design, codes| {
                            let weight_values: Vec<f64> = (0..rows)
                                .map(|r| 0.25 + ((r * 7919) % 101) as f64 / 37.0)
                                .collect();
                            for columns in [1, 5, 18, 64] {
                                let input = values(rows, columns, codes, 17 + kind as u64);
                                for weighted in [false, true] {
                                    let weights = if weighted {
                                        BorrowedMapWeights::Provided(&weight_values)
                                    } else {
                                        BorrowedMapWeights::Unit
                                    };
                                    for symmetric in [false, true] {
                                        for retain in [false, true] {
                                            let unfused = run_sweeps(
                                                design, weights, &input, symmetric, retain, false,
                                            )
                                            .unwrap();
                                            let fused = run_sweeps(
                                                design, weights, &input, symmetric, retain, true,
                                            )
                                            .unwrap();
                                            assert_eq!(fused, unfused);
                                            let key =
                                                (kind, rows, columns, weighted, symmetric, retain);
                                            let first = reference
                                                .entry(key)
                                                .or_insert_with(|| fused.clone());
                                            assert_eq!(&fused, first, "worker agreement {key:?}");
                                        }
                                    }
                                }
                            }
                        });
                    }
                }
            })
            .unwrap();
    }
}

#[test]
fn fused_sweep_rejects_nonfinite_results_as_the_unfused_sweep_does() {
    with_design(0, 16_385, |design, codes| {
        let mut input = values(16_385, 3, codes, 5);
        input[(7, 1)] = f64::MAX;
        input[(8, 1)] = -f64::MAX;
        let unfused = run_sweeps(design, BorrowedMapWeights::Unit, &input, true, true, false);
        let fused = run_sweeps(design, BorrowedMapWeights::Unit, &input, true, true, true);
        assert_eq!(
            unfused.map_err(|e| e.code),
            fused.map_err(|e| e.code),
            "the pilot treats either invalid-data outcome as a failed attempt"
        );
    });
}

/// Run `function` with the fused sweep refused on this thread.
fn unfused<T>(function: impl FnOnce() -> T) -> T {
    UNFUSED.set(true);
    let result = function();
    UNFUSED.set(false);
    result
}

fn certificate_bits(certificate: &FeCertificate) -> [u64; 4] {
    [
        certificate.absolute_infinity_norm.to_bits(),
        certificate.relative_infinity_norm.to_bits(),
        certificate.relative_two_norm.to_bits(),
        u64::from(certificate.passed),
    ]
}

#[test]
fn fused_pilot_publishes_the_unfused_pilot_bits_at_every_worker_count() {
    let mut reference = std::collections::BTreeMap::new();
    for workers in [1, 2, 4, 8] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                for kind in 0..3 {
                    with_design(kind, 16_385, |design, codes| {
                        for columns in [1, 18, 65] {
                            let input = values(16_385, columns, codes, 3);
                            assert_eq!(
                                planned_bytes(design, 16_385, columns).is_some(),
                                columns <= PARALLEL_GROUP_MAX_RHS
                            );
                            for (checkpoint, tolerance) in [(1, 1e-8), (3, 1e-8), (2, 1e-15)] {
                                for retain in [false, true] {
                                    let map = MapConfig {
                                        tolerance,
                                        ..MapConfig::default()
                                    };
                                    let run = || {
                                        three_fe_map::attempt(
                                            design, &input, map, None, retain, 0, checkpoint, true,
                                        )
                                        .unwrap()
                                    };
                                    let (fused, plain) = (run(), unfused(run));
                                    assert_eq!(fused.iterations, plain.iterations);
                                    let view = |attempt: &three_fe_map::Attempt| {
                                        attempt.result.as_ref().map(|result| {
                                            (
                                                bits(result.residualized.as_slice()),
                                                bits(&result.residual_norms),
                                                result.report.iterations,
                                                certificate_bits(&result.report.certificate),
                                                result
                                                    .coefficients
                                                    .as_ref()
                                                    .map(|c| bits(&c.values)),
                                            )
                                        })
                                    };
                                    let fused_view = view(&fused);
                                    assert_eq!(fused_view, view(&plain));
                                    // The optional tables are charged only when fused.
                                    if columns <= PARALLEL_GROUP_MAX_RHS {
                                        assert!(fused.peak.bytes() > plain.peak.bytes());
                                    } else {
                                        assert_eq!(fused.peak, plain.peak);
                                    }
                                    let key = (kind, columns, checkpoint, retain);
                                    let first = reference
                                        .entry(key)
                                        .or_insert_with(|| (fused.iterations, fused_view.clone()));
                                    assert_eq!(first, &(fused.iterations, fused_view), "{key:?}");
                                }
                            }
                        }
                    });
                }
            })
            .unwrap();
    }
}

#[test]
fn unaffordable_fused_tables_keep_the_unfused_pilot() {
    with_design(1, 16_385, |design, codes| {
        let input = values(16_385, 18, codes, 9);
        let plain = unfused(|| {
            three_fe_map::attempt(design, &input, MapConfig::default(), None, true, 0, 2, true)
                .unwrap()
        });
        let limit = plain.peak.bytes().unwrap() as u128;
        let budget = FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: limit,
        };
        let tight = three_fe_map::attempt(
            design,
            &input,
            MapConfig::default(),
            Some(budget),
            true,
            0,
            2,
            true,
        )
        .unwrap();
        assert_eq!(tight.peak, plain.peak);
        assert_eq!(tight.iterations, plain.iterations);
        assert_eq!(
            tight.result.map(|r| bits(r.residualized.as_slice())),
            plain.result.map(|r| bits(r.residualized.as_slice()))
        );
    });
}
