use super::*;
use crate::fe::{LevelIndex, MapGeometryGeneration, ValidatedBorrowedInterceptSource};

fn fixture(effects: usize, visit: impl FnOnce(BorrowedInterceptFeDesign<'_>)) {
    let mut codes = [Vec::new(), Vec::new(), Vec::new()];
    for component in 0..2_u32 {
        for a in 0..2 {
            for b in 0..3 {
                for c in 0..4 {
                    for _ in 0..2 {
                        codes[0].push(a + 2 * component);
                        codes[1].push(b + 3 * component);
                        codes[2].push(c + 4 * component);
                    }
                }
            }
        }
    }
    let indices: Vec<_> = codes
        .iter()
        .enumerate()
        .map(|(i, codes)| LevelIndex::from_codes(codes, 4 + 2 * i).unwrap())
        .collect();
    let sources: Vec<_> = codes
        .iter()
        .enumerate()
        .map(|(i, codes)| {
            ValidatedBorrowedInterceptSource::new(codes, 4 + 2 * i, &indices[i]).unwrap()
        })
        .collect();
    visit(
        BorrowedInterceptFeDesign::new(
            codes[0].len(),
            sources[..effects]
                .iter()
                .map(|source| source.effect())
                .collect(),
        )
        .unwrap(),
    );
}

fn generation(weights: u64) -> MapGeometryGeneration {
    MapGeometryGeneration {
        sample: 7,
        topology: 9,
        weights,
    }
}

fn copy_design<'a>(design: &BorrowedInterceptFeDesign<'a>) -> BorrowedInterceptFeDesign<'a> {
    BorrowedInterceptFeDesign::new(design.nobs(), design.effects().to_vec()).unwrap()
}

fn report(mut report: SolveReport) -> SolveReport {
    report.setup_seconds = 0.;
    report.solve_seconds = 0.;
    report.certification_seconds = 0.;
    report
}

#[test]
fn ppml_topology_keeps_weighted_projections_and_signatures_exact() {
    for effects in [2, 3] {
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    fixture(effects, |design| {
                        let rows = design.nobs();
                        let mut baseline =
                            ReusableCategoricalMapAbsorber::new(copy_design(&design), 2, threads)
                                .unwrap();
                        let mut shared =
                            ReusableCategoricalMapAbsorber::new(copy_design(&design), 2, threads)
                                .unwrap();
                        for owner in [&mut baseline, &mut shared] {
                            owner.enable_checkpoint_lsmr(true);
                            owner.enable_component_checkpoint_scaling();
                        }
                        shared.enable_ppml_topology(generation(1), 4096).unwrap();
                        for frame in 0..3 {
                            let key = generation(frame + 1);
                            shared.begin_ppml_frame(key).unwrap();
                            let weights: Vec<_> = (0..rows)
                                .map(|row| {
                                    let mass = if (row < rows / 2) == (frame % 2 == 0) {
                                        1e13
                                    } else {
                                        1.
                                    };
                                    mass * (1. + (row / 2 % 7) as f64)
                                        * if row % 2 == 0 { 1. } else { 2. + frame as f64 }
                                })
                                .collect();
                            let expected: Vec<_> = (0..rows)
                                .map(|row| {
                                    if row % 2 == 0 {
                                        0.5
                                    } else {
                                        -0.5 / (2. + frame as f64)
                                    }
                                })
                                .collect();
                            let response: Vec<_> = (0..rows)
                                .map(|row| {
                                    expected[row]
                                        + design
                                            .effects()
                                            .iter()
                                            .enumerate()
                                            .map(|(i, effect)| {
                                                0.1 * (i + 1) as f64 * effect.codes()[row] as f64
                                            })
                                            .sum::<f64>()
                                })
                                .collect();
                            for width in [1, 2] {
                                let columns = [response.clone(), vec![0.; rows]];
                                let input = DenseMatrix::from_columns(&columns[..width]).unwrap();
                                let config = MapConfig {
                                    tolerance: 1e-12,
                                    ..MapConfig::default()
                                };
                                let old = baseline
                                    .absorb_ppml(
                                        BorrowedMapWeights::Provided(&weights),
                                        input.clone(),
                                        config,
                                    )
                                    .unwrap();
                                let new = shared
                                    .absorb_ppml(
                                        BorrowedMapWeights::Provided(&weights),
                                        input,
                                        config,
                                    )
                                    .unwrap();
                                assert_eq!(
                                    new.residualized()
                                        .as_slice()
                                        .iter()
                                        .map(|x| x.to_bits())
                                        .collect::<Vec<_>>(),
                                    old.residualized()
                                        .as_slice()
                                        .iter()
                                        .map(|x| x.to_bits())
                                        .collect::<Vec<_>>()
                                );
                                assert_eq!(
                                    report(new.report().clone()),
                                    report(old.report().clone())
                                );
                                for (row, expected) in expected.iter().enumerate() {
                                    assert!(
                                        (new.residualized()[(row, 0)] - expected).abs() < 1e-10
                                    );
                                }
                                let cached = shared.reuse.ppml().unwrap();
                                assert_eq!(
                                    cached.state(),
                                    if frame == 0 && width == 1 {
                                        (false, true)
                                    } else {
                                        (true, false)
                                    }
                                );
                                shared.accept_ppml_topology(key);
                                assert_eq!(shared.reuse.ppml().unwrap().state(), (true, false));
                            }
                        }
                        shared.begin_ppml_frame(generation(4)).unwrap();
                        let invalid = vec![0.; rows];
                        assert!(shared
                            .absorb_ppml(
                                BorrowedMapWeights::Provided(&invalid),
                                DenseMatrix::zeros(rows, 1).unwrap(),
                                MapConfig::default()
                            )
                            .is_err());
                        assert_eq!(shared.reuse.ppml().unwrap().state(), (true, false));
                        assert!(shared
                            .begin_ppml_frame(MapGeometryGeneration {
                                sample: 8,
                                ..generation(5)
                            })
                            .is_err());
                        assert_eq!(shared.reuse.ppml().unwrap().state(), (false, false));
                    });
                });
        }
    }
}

#[test]
fn ppml_topology_rejects_pending_generations_and_admits_retention_exactly() {
    fixture(3, |design| {
        let bytes = crate::fe::ppml_topology::retained_bytes(
            design.effects().iter().map(|effect| effect.n_levels()),
            3,
        )
        .unwrap();
        let mut owner = ReusableCategoricalMapAbsorber::new(copy_design(&design), 1, 1).unwrap();
        assert_eq!(
            owner
                .enable_ppml_topology(generation(1), bytes - 1)
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
        owner.enable_ppml_topology(generation(1), bytes).unwrap();
        let topology = owner.reuse.ppml_mut().unwrap();
        topology.prepare(&design).unwrap();
        assert_eq!(topology.bytes(), bytes);
        assert_eq!(topology.state(), (false, true));
        topology.begin(generation(2)).unwrap();
        assert_eq!(topology.state(), (false, false));
        topology.prepare(&design).unwrap();
        topology.accept(generation(1));
        assert_eq!(topology.state(), (false, false));
        topology.prepare(&design).unwrap();
        topology.accept(generation(2));
        let pointer = topology.prepare(&design).unwrap().component_by_effect_level[0].as_ptr();
        topology.begin(generation(3)).unwrap();
        topology.reject();
        assert_eq!(
            pointer,
            topology.prepare(&design).unwrap().component_by_effect_level[0].as_ptr()
        );
        assert!(topology
            .begin(MapGeometryGeneration {
                topology: 10,
                ..generation(4)
            })
            .is_err());
        assert!(topology.prepare(&design).is_err());
    });
}
