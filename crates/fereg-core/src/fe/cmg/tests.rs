use super::super::certify_borrowed_intercept_candidate;
use super::*;
use crate::fe::{
    absorb_borrowed_intercept_cg, BorrowedCgOptions, BorrowedInterceptEffect, LevelIndex,
};
use crate::memory::estimate_fe_solver_memory;

#[test]
fn hierarchy_work_counts_recursive_level_visits() {
    let work = CmgHierarchyWork::from_levels(&[100, 20, 5], &[300, 40, 0], &[4, 3, 0]);
    assert_eq!(work.finest_work, 400);
    assert_eq!(work.estimated_recursive_work, 700);
    assert_eq!(work.deepest_visits, 12);
    assert_eq!(work.maximum_repeat, 4);
    assert!((work.work_over_finest - 1.75).abs() <= f64::EPSILON);
}

#[test]
fn prepared_nuisance_grouping_preserves_memory_caps_and_zero_nuisance_policy() {
    let mut profile = CmgTopologyProfile {
        effect_order: vec![0, 1, 2],
        core_effects: Some([0, 1]),
        vertices: 1_200,
        nuisance_columns: 0,
        observations: 25_000,
        highest_cardinality_levels: 1_000,
        second_cardinality_levels: 200,
    };
    let edges = (0..9)
        .flat_map(|left| (9..18).map(move |right| (left, right, 1.0)))
        .collect::<Vec<_>>();
    let graph = Laplacian::from_edges(18, edges).unwrap();
    assert_eq!(
        prepared_cmg_options(&profile, &graph),
        CmgOptions::default()
    );
    for width in [1, 32, 64] {
        profile.nuisance_columns = width;
        let options = prepared_cmg_options(&profile, &graph);
        assert_eq!(options.low_effective_degree_threshold, 0.0);
        assert_eq!(
            CmgOptions {
                low_effective_degree_threshold: CmgOptions::default()
                    .low_effective_degree_threshold,
                ..options
            },
            CmgOptions::default(),
            "direct factor, fill and depth bounds cannot grow"
        );
        let problem = CmgProblemSize {
            vertices: profile.vertices,
            input_edges: profile.observations,
            canonical_edges: profile.observations,
            right_hand_sides: 10,
        };
        for threads in [1, 7] {
            let parallel = ParallelOptions {
                threads,
                ..ParallelOptions::default()
            };
            assert_eq!(
                RepeatedPcgMemoryEstimate::conservative(
                    problem,
                    options,
                    parallel,
                    threads > 1,
                    false
                )
                .unwrap(),
                RepeatedPcgMemoryEstimate::conservative(
                    problem,
                    CmgOptions::default(),
                    parallel,
                    threads > 1,
                    false
                )
                .unwrap()
            );
        }
    }
    let boundary = Laplacian::from_edges(
        16,
        (0..8).flat_map(|left| (8..16).map(move |right| (left, right, 1.0))),
    )
    .unwrap();
    assert_eq!(
        prepared_cmg_options(&profile, &boundary),
        CmgOptions::default(),
        "equality is not weak"
    );
    let mixed = Laplacian::from_edges(
        18,
        (0..9).flat_map(|left| {
            (9..18).map(move |right| {
                (
                    left,
                    right,
                    if left == 0 && right == 9 { 100.0 } else { 1.0 },
                )
            })
        }),
    )
    .unwrap();
    assert_eq!(
        prepared_cmg_options(&profile, &mixed),
        CmgOptions::default(),
        "one strong edge is sufficient; do not use an average"
    );
    assert_eq!(
        prepared_cmg_options(&profile, &graph).low_effective_degree_threshold,
        0.0,
        "recompute eligibility from the current graph, without retained policy state"
    );
}

#[test]
fn prepared_coarsening_certifies_large_permuted_weighted_three_fe_graphs() {
    let rows = 24_000;
    let levels = [800, 160, 5];
    let codes = [
        (0..rows)
            .map(|row| (row % levels[0]) as u32)
            .collect::<Vec<_>>(),
        (0..rows)
            .map(|row| ((row * 17 + (row / levels[0]) * 7) % levels[1]) as u32)
            .collect(),
        (0..rows)
            .map(|row| ((row / 37) % levels[2]) as u32)
            .collect(),
    ];
    let indexes = codes
        .iter()
        .zip(levels)
        .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).unwrap())
        .collect::<Vec<_>>();
    let input = DenseMatrix::from_columns(&[
        (0..rows).map(|row| (row as f64 * 0.31).sin()).collect(),
        (0..rows)
            .map(|row| (row as f64 * 0.17).cos() + codes[2][row] as f64)
            .collect(),
    ])
    .unwrap();
    let mut references: [Option<DenseMatrix>; 3] = Default::default();
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let design = || {
            BorrowedInterceptFeDesign::new(
                rows,
                order
                    .iter()
                    .map(|&i| effect(&codes[i], levels[i], &indexes[i]))
                    .collect(),
            )
            .unwrap()
        };
        let mut topology = PreparedCmgTopology::prepare(&design()).unwrap();
        let mut control_seed = CmgControlSeed::new(960, 1).unwrap();
        for (frame, reference) in references.iter_mut().enumerate() {
            let weights = (0..rows)
                .map(|row| {
                    let value = 0.7 + (row % 11) as f64 * 0.13;
                    if frame == 0 {
                        value
                    } else if frame == 1 {
                        value * (1.0 + (row % 7) as f64 * 0.01)
                    } else if row % 3 == 0 {
                        value * 8.0
                    } else {
                        value
                    }
                })
                .collect::<Vec<_>>();
            let mut options = config(4);
            options.map.tolerance = 1e-10;
            options.threads =
                ThreadConfig::from_requested(if frame == 1 { 7 } else { 1 }, true).unwrap();
            let mut absorber = CmgAbsorber::build_unpolished_prepared(
                design(),
                BorrowedMapWeights::Provided(&weights),
                options,
                &mut topology,
            )
            .unwrap();
            let CmgRoute::Graph(graph) = &absorber.route else {
                panic!("graph")
            };
            let hierarchy = graph.solver.preconditioner().hierarchy();
            assert_eq!(hierarchy.report().vertex_counts()[0], 960);
            assert!(
                hierarchy.report().vertex_counts().len() >= 2,
                "exercise coarsening, not direct-only setup"
            );
            if order == [0, 1, 2] && frame == 0 {
                let owned = CmgAbsorber::build(
                    design(),
                    BorrowedMapWeights::Provided(&weights),
                    options,
                    false,
                )
                .unwrap();
                let CmgRoute::Graph(owned) = &owned.route else {
                    panic!("owned graph")
                };
                assert!(
                    owned
                        .solver
                        .preconditioner()
                        .hierarchy()
                        .report()
                        .vertex_counts()[1]
                        > hierarchy.report().vertex_counts()[1],
                    "the OLS owned route must retain default pruning"
                );
            }
            let mut candidate = input.clone();
            let report = absorber
                .project_unpolished_with_seed(&mut candidate, 1e-12, Some(&mut control_seed))
                .unwrap();
            assert!(!control_seed.ready());
            assert_eq!(
                report.projection_work.warm_right_hand_sides,
                usize::from(frame == 1)
            );
            assert_eq!(
                report.nuisance_setup.warm_right_hand_sides,
                if frame == 1 { 4 } else { 0 }
            );
            let oracle = absorb_borrowed_intercept_cg(
                &design(),
                BorrowedMapWeights::Provided(&weights),
                input.clone(),
                options.map,
                BorrowedCgOptions::default(),
            )
            .unwrap();
            assert_matrix_close(&candidate, oracle.residualized(), 1e-8);
            assert!(
                certify_borrowed_intercept_candidate(
                    &design(),
                    BorrowedMapWeights::Provided(&weights),
                    &candidate,
                    options.map
                )
                .unwrap()
                .passed
            );
            if let Some(reference) = reference {
                assert_eq!(
                    &candidate, reference,
                    "warm projections must preserve permutation determinism"
                );
            } else {
                *reference = Some(candidate);
            }
            control_seed.retain_certified(report.projection_work);
            topology.retain_certified_seed(absorber);
        }
    }
}

#[test]
fn component_quantiles_use_stable_nearest_lower_order_statistic() {
    let sizes = [1, 2, 3, 4, 100];
    assert_eq!(component_quantile(&sizes, 50), 3);
    assert_eq!(component_quantile(&sizes, 90), 4);
    assert_eq!(component_quantile(&[], 50), 0);
}

#[test]
fn component_default_is_explicit_only_and_has_an_ordinary_comparison_seam() {
    for solver in [
        FeSolverKind::Auto,
        FeSolverKind::Map,
        FeSolverKind::Lsmr,
        FeSolverKind::Cg,
    ] {
        assert!(!component_policy(solver, false));
        assert!(!component_policy(solver, true));
    }
    assert!(component_policy(FeSolverKind::Cmg, false));
    assert!(!component_policy(FeSolverKind::Cmg, true));
}

fn config(batch_size: usize) -> CmgConfig {
    CmgConfig {
        map: MapConfig {
            tolerance: 1e-10,
            max_iterations: 2_000,
            check_every: 2,
            symmetric: true,
        },
        threads: ThreadConfig::from_requested(1, true).expect("threads"),
        batch_size,
        retain_coefficients: false,
        requested_solver: FeSolverKind::Cmg,
        components: false,
        solver_memory_budget: None,
    }
}

fn effect<'a>(
    codes: &'a [u32],
    levels: usize,
    index: &'a LevelIndex,
) -> BorrowedInterceptEffect<'a> {
    BorrowedInterceptEffect::new(codes, levels, index).expect("borrowed effect")
}

fn assert_matrix_close(left: &DenseMatrix, right: &DenseMatrix, tolerance: f64) {
    assert_eq!((left.nrows(), left.ncols()), (right.nrows(), right.ncols()));
    let maximum = left
        .as_slice()
        .iter()
        .zip(right.as_slice())
        .map(|(left, right)| (left - right).abs())
        .fold(0.0_f64, f64::max);
    assert!(maximum <= tolerance, "maximum difference {maximum:e}");
}

fn assert_coefficients_reconstruct(
    input: &DenseMatrix,
    residualized: &DenseMatrix,
    effects: &[&[u32]],
    coefficients: &FeCoefficientBatch,
    tolerance: f64,
) {
    for rhs in 0..input.ncols() {
        let coefficient_rhs = coefficients.rhs(rhs).expect("coefficient RHS");
        for row in 0..input.nrows() {
            let fitted = effects
                .iter()
                .enumerate()
                .map(|(effect, codes)| {
                    coefficient_rhs[coefficients.effect_offsets()[effect] + codes[row] as usize]
                })
                .sum::<f64>();
            let expected = input[(row, rhs)] - residualized[(row, rhs)];
            assert!(
                (fitted - expected).abs() <= tolerance,
                "retained CMG coefficients differ by {:e}",
                (fitted - expected).abs()
            );
        }
    }
}

#[test]
fn prepared_topology_rejects_a_different_design_with_the_same_profile() {
    let worker_a = [0, 0, 1, 1, 2, 2, 3, 3];
    let firm_a = [0, 1, 0, 1, 0, 1, 0, 1];
    let worker_b = [0, 1, 1, 2, 2, 3, 3, 0];
    let firm_b = [0, 0, 1, 1, 0, 0, 1, 1];
    let worker_a_index = LevelIndex::from_codes(&worker_a, 4).expect("worker A index");
    let firm_a_index = LevelIndex::from_codes(&firm_a, 2).expect("firm A index");
    let worker_b_index = LevelIndex::from_codes(&worker_b, 4).expect("worker B index");
    let firm_b_index = LevelIndex::from_codes(&firm_b, 2).expect("firm B index");
    let design_a = BorrowedInterceptFeDesign::new(
        worker_a.len(),
        vec![
            effect(&worker_a, 4, &worker_a_index),
            effect(&firm_a, 2, &firm_a_index),
        ],
    )
    .expect("design A");
    let design_b = BorrowedInterceptFeDesign::new(
        worker_b.len(),
        vec![
            effect(&worker_b, 4, &worker_b_index),
            effect(&firm_b, 2, &firm_b_index),
        ],
    )
    .expect("design B");
    assert_eq!(
        CmgTopologyProfile::analyze(&design_a).expect("profile A"),
        CmgTopologyProfile::analyze(&design_b).expect("profile B")
    );

    let prepare_peak =
        PreparedCmgTopology::conservative_prepare_peak_bytes(&design_a).expect("peak");
    let mut prepared = PreparedCmgTopology::prepare(&design_a).expect("prepared topology");
    assert!(prepare_peak >= prepared.retained_bytes().unwrap() as u128);
    let error = match CmgAbsorber::build_unpolished_prepared(
        design_b,
        BorrowedMapWeights::Unit,
        config(1),
        &mut prepared,
    ) {
        Ok(_) => panic!("prepared topology accepted a different design"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert!(error.message.contains("does not match"));
}

#[test]
fn early_prepared_memory_bound_covers_exact_topology_and_frame() {
    let worker = [0, 0, 1, 1, 2, 2, 3, 3];
    let firm = [0, 1, 0, 1, 0, 1, 0, 1];
    let worker_index = LevelIndex::from_codes(&worker, 4).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm, 2).expect("firm index");
    let design = BorrowedInterceptFeDesign::new(
        worker.len(),
        vec![
            effect(&worker, 4, &worker_index),
            effect(&firm, 2, &firm_index),
        ],
    )
    .expect("design");
    let cmg_config = config(2);
    let early = PreparedCmgTopology::conservative_retained_frame_peak_bytes(&design, cmg_config)
        .expect("early retained-frame peak");
    let prepared = PreparedCmgTopology::prepare(&design).expect("prepared topology");
    let profile = CmgTopologyProfile::analyze(&design).expect("profile");
    let exact_problem = CmgProblemSize {
        vertices: prepared.topology.vertex_count(),
        input_edges: prepared.topology.input_edge_count(),
        canonical_edges: prepared.topology.canonical_edge_count(),
        right_hand_sides: cmg_config.batch_size,
    };
    let exact_frame =
        conservative_prepared_cmg_frame_bytes(&design, &profile, cmg_config, exact_problem)
            .expect("exact prepared frame peak");
    let exact_combined = (prepared.retained_bytes().unwrap() as u128)
        .checked_add(exact_frame)
        .expect("exact retained-frame total");
    assert!(early >= exact_combined);
}

#[test]
fn prepared_cmg_seed_weight_boundary_and_memory_are_explicit() {
    let codes = [
        [0, 0, 1, 1, 2, 2, 3, 3],
        [0, 1, 2, 0, 1, 2, 0, 1],
        [0, 1, 1, 0, 0, 1, 1, 0],
    ];
    let levels = [4, 3, 2];
    let indexes = codes
        .iter()
        .zip(levels)
        .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).unwrap())
        .collect::<Vec<_>>();
    let design = BorrowedInterceptFeDesign::new(
        8,
        (0..3)
            .map(|i| effect(&codes[i], levels[i], &indexes[i]))
            .collect(),
    )
    .unwrap();
    let mut topology = PreparedCmgTopology::prepare(&design).unwrap();
    let profile = CmgTopologyProfile::analyze(&design).unwrap();
    assert_eq!(topology.nuisance_seed.len(), 7);
    assert_eq!(nuisance_seed_bytes(&profile).unwrap(), 7 * 8 + 40);
    let early =
        PreparedCmgTopology::conservative_retained_frame_peak_bytes(&design, config(2)).unwrap();
    let frame = conservative_prepared_cmg_frame_bytes(
        &design,
        &profile,
        config(2),
        conservative_prepared_problem(&profile, 2),
    )
    .unwrap();
    assert!(early >= topology.retained_bytes().unwrap() as u128 + frame);
    for (ratio, eligible) in [
        (2.0, true),
        (0.5, true),
        (2.0 + 1e-12, false),
        (0.5 - 1e-12, false),
    ] {
        topology.numeric_weights.fill(1.0);
        topology.seed_valid = true;
        let mut weights = [1.0; 8];
        weights[0] = ratio;
        topology
            .assemble(
                &design,
                &profile,
                BorrowedMapWeights::Provided(&weights),
                1.0,
            )
            .unwrap();
        assert_eq!(topology.seed_for_frame, eligible);
        assert!(!topology.seed_valid);
    }
}

#[test]
fn control_seed_admission_consumes_only_finite_certified_evidence() {
    let mut seed = CmgControlSeed::new(6, 3).unwrap();
    assert!(!seed.ready());
    assert!(!seed.consume(1.0));
    let capacity = seed.potentials.capacity();
    assert_eq!(
        CmgControlSeed::estimated_bytes(6, 3).unwrap(),
        (18 * 8 + std::mem::size_of::<CmgControlSeed>()) as u128
    );
    assert!(CmgControlSeed::estimated_bytes(usize::MAX, usize::MAX).is_err());
    for (ratio, admitted) in [
        (0.5, false),
        (1.0, true),
        (2.0, true),
        (2.0000001, false),
        (f64::INFINITY, false),
        (f64::NAN, false),
    ] {
        seed.retain_certified(CmgProjectionWork::default());
        assert_eq!(seed.consume(ratio), admitted);
        assert!(!seed.ready());
        assert!(
            !seed.consume(1.0),
            "a partial/failed solve cannot reuse the buffer"
        );
    }
    for (residual, extra_restarts, retained) in [
        (0.499, 0, true),
        (0.5, 0, false),
        (0.01, 1, false),
        (f64::NAN, 0, false),
    ] {
        seed.retain_certified(CmgProjectionWork {
            warm_right_hand_sides: 3,
            initial_relative_residual: residual,
            extra_restarts,
            ..CmgProjectionWork::default()
        });
        assert_eq!(seed.ready(), retained);
    }
    seed.retain_certified(CmgProjectionWork::default());
    seed.potentials[2] = f64::NAN;
    assert!(!seed.consume(1.0));
    seed.retain_certified(CmgProjectionWork::default());
    assert!(!seed.ready());
    seed.potentials.fill(0.0);
    seed.retain_certified(CmgProjectionWork::default());
    seed.reset();
    assert!(!seed.ready());
    assert_eq!(
        capacity,
        seed.potentials.capacity(),
        "all state transitions must be allocation-free"
    );
}

#[test]
fn prepared_cmg_warm_seed_is_consumed_recertified_and_invalidated() {
    let rows = 1_200;
    let codes = [
        (0..rows).map(|row| (row % 40) as u32).collect::<Vec<_>>(),
        (0..rows)
            .map(|row| ((row / 40 + 7 * row) % 20) as u32)
            .collect(),
        (0..rows)
            .map(|row| ((row / 200 + row / 7) % 5) as u32)
            .collect(),
    ];
    let levels = [40, 20, 5];
    let indexes = codes
        .iter()
        .zip(levels)
        .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).unwrap())
        .collect::<Vec<_>>();
    let design = || {
        BorrowedInterceptFeDesign::new(
            rows,
            (0..3)
                .map(|i| effect(&codes[i], levels[i], &indexes[i]))
                .collect(),
        )
        .unwrap()
    };
    let input = DenseMatrix::from_columns(&[
        (0..rows).map(|row| (row as f64 * 0.31).sin()).collect(),
        (0..rows).map(|row| (row as f64 * 0.17).cos()).collect(),
    ])
    .unwrap();
    let mut topology = PreparedCmgTopology::prepare(&design()).unwrap();
    let mut control_seed = CmgControlSeed::new(60, 1).unwrap();
    assert!(!topology.seed_valid);
    let retained = topology.retained_bytes();
    let mut options = config(2);
    options.map.tolerance = 1e-10;
    for frame in 0..6 {
        let weights = (0..rows)
            .map(|row| {
                let base = 0.7 + (row % 11) as f64 * 0.13;
                if frame >= 2 && row % 3 == 0 {
                    base * 5.0
                } else {
                    base
                }
            })
            .collect::<Vec<_>>();
        if frame == 3 {
            topology.reset_warm_start();
            control_seed.reset();
        }
        if frame == 4 {
            topology.nuisance_seed[0] = f64::NAN;
            control_seed.potentials[0] = f64::NAN;
        }
        let mut absorber = CmgAbsorber::build_unpolished_prepared(
            design(),
            BorrowedMapWeights::Provided(&weights),
            options,
            &mut topology,
        )
        .unwrap();
        assert!(
            !topology.seed_valid,
            "a failed frame cannot leave a usable seed"
        );
        let mut candidate = input.clone();
        let report = absorber
            .project_unpolished_with_seed(&mut candidate, 1e-11, Some(&mut control_seed))
            .unwrap();
        assert!(!control_seed.ready());
        assert_eq!(
            report.projection_work.warm_right_hand_sides,
            usize::from(frame == 1 || frame == 5)
        );
        if frame == 1 || frame == 5 {
            assert_eq!(report.projection_work.warm_iterations, 0);
            assert!(
                report.projection_work.iterations > 0,
                "response must stay cold"
            );
        }
        assert_eq!(
            report.nuisance_setup.warm_right_hand_sides,
            if frame == 1 || frame == 5 { 4 } else { 0 }
        );
        if frame == 1 || frame == 5 {
            assert_eq!(
                report.nuisance_setup.iterations, 0,
                "same-operator guess must freshly certify"
            );
        }
        let oracle = absorb_borrowed_intercept_cg(
            &design(),
            BorrowedMapWeights::Provided(&weights),
            input.clone(),
            options.map,
            BorrowedCgOptions::default(),
        )
        .unwrap();
        assert_matrix_close(&candidate, oracle.residualized(), 1e-8);
        assert!(
            certify_borrowed_intercept_candidate(
                &design(),
                BorrowedMapWeights::Provided(&weights),
                &candidate,
                options.map
            )
            .unwrap()
            .passed
        );
        control_seed.retain_certified(report.projection_work);
        assert!(control_seed.ready());
        topology.retain_certified_seed(absorber);
        assert!(
            topology.seed_valid,
            "frame {frame}: {:?}",
            report.nuisance_setup
        );
        assert_eq!(
            retained,
            topology.retained_bytes(),
            "seed commit cannot allocate"
        );
    }
}

#[test]
fn prepared_nuisance_potentials_are_current_weight_and_remove_only_duplicate_solves() {
    let rows = 1_200;
    let codes = [
        (0..rows).map(|row| (row % 40) as u32).collect::<Vec<_>>(),
        (0..rows)
            .map(|row| ((row / 40 + 7 * row) % 20) as u32)
            .collect(),
        (0..rows)
            .map(|row| ((row / 200 + row / 7) % 5) as u32)
            .collect(),
    ];
    let levels = [40, 20, 5];
    let indexes = codes
        .iter()
        .zip(levels)
        .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).expect("index"))
        .collect::<Vec<_>>();
    let input = DenseMatrix::from_columns(
        &(0..3)
            .map(|column| {
                (0..rows)
                    .map(|row| {
                        ((row + column * 19) as f64 * 0.31).sin() + codes[2][row] as f64 * 0.2
                    })
                    .collect()
            })
            .collect::<Vec<_>>(),
    )
    .expect("input");
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let design = || {
            BorrowedInterceptFeDesign::new(
                rows,
                order
                    .iter()
                    .map(|&index| effect(&codes[index], levels[index], &indexes[index]))
                    .collect(),
            )
            .expect("design")
        };
        let mut topology = PreparedCmgTopology::prepare(&design()).expect("topology");
        let mut previous_potentials = None;
        for frame in 0..2 {
            let weights = (0..rows)
                .map(|row| 0.7 + ((row * (frame + 1)) % 11) as f64 * 0.13)
                .collect::<Vec<_>>();
            let mut options = config(2);
            options.threads = ThreadConfig::from_requested(1 + 3 * frame, true).expect("threads");
            let mut prepared = CmgAbsorber::build_unpolished_prepared(
                design(),
                BorrowedMapWeights::Provided(&weights),
                options,
                &mut topology,
            )
            .expect("prepared frame");
            let mut owned = CmgAbsorber::build(
                design(),
                BorrowedMapWeights::Provided(&weights),
                options,
                false,
            )
            .expect("owned frame");
            let CmgRoute::Graph(graph) = &prepared.route else {
                panic!("graph")
            };
            let GraphCmgWorkspace::CallerBuffers(caller) = &graph.workspace else {
                panic!("caller")
            };
            assert_eq!(caller.nuisance_potentials.len(), 60 * 4);
            if let Some(previous) = previous_potentials.replace(caller.nuisance_potentials.clone())
            {
                assert_ne!(previous, caller.nuisance_potentials);
            }
            let mut previous_values = None;
            for call in 0..2 {
                let mut candidate = input.clone();
                let mut reference = input.clone();
                let first = prepared
                    .project_unpolished(&mut candidate, 1e-11)
                    .expect("prepared projection");
                let old = owned
                    .project_unpolished(&mut reference, 1e-11)
                    .expect("owned projection");
                assert_eq!(first.right_hand_sides, if call == 0 { 4 + 3 } else { 3 });
                assert_eq!(old.right_hand_sides - first.right_hand_sides, 3);
                assert_eq!(first.nuisance_reused_right_hand_sides, 3);
                assert_eq!(old.nuisance_reused_right_hand_sides, 0);
                assert_matrix_close(&candidate, &reference, 5e-9);
                if let Some(previous) = previous_values.replace(candidate.clone()) {
                    assert_eq!(previous.as_slice(), candidate.as_slice());
                }
                let mut certificate_config = options.map;
                certificate_config.tolerance = 1e-9;
                assert!(
                    certify_borrowed_intercept_candidate(
                        &design(),
                        BorrowedMapWeights::Provided(&weights),
                        &candidate,
                        certificate_config,
                    )
                    .expect("fresh full-model certificate")
                    .passed
                );
            }
            // A reused potential is not evidence: a corrupted basis must
            // still be rejected by the independent full-FE certificate.
            let CmgRoute::Graph(graph) = &mut prepared.route else {
                panic!("graph")
            };
            let GraphCmgWorkspace::CallerBuffers(caller) = &mut graph.workspace else {
                panic!("caller")
            };
            caller.nuisance_potentials[0] += 10.0;
            let mut corrupt = input.clone();
            prepared
                .project_unpolished(&mut corrupt, 1e-11)
                .expect("uncertified projection");
            let mut certificate_config = options.map;
            certificate_config.tolerance = 1e-9;
            assert!(
                !certify_borrowed_intercept_candidate(
                    &design(),
                    BorrowedMapWeights::Provided(&weights),
                    &corrupt,
                    certificate_config,
                )
                .expect("independent corrupted-basis certificate")
                .passed
            );
        }
    }
}

#[test]
fn prepared_batches_align_with_the_admitted_pool_without_changing_solutions() {
    let rows = 4_200;
    let levels = [80, 70, 65];
    let codes = [
        (0..rows).map(|row| (row % 80) as u32).collect::<Vec<_>>(),
        (0..rows)
            .map(|row| ((row / 80 + 7 * row) % 70) as u32)
            .collect(),
        (0..rows)
            .map(|row| ((row / 200 + row / 7) % 65) as u32)
            .collect(),
    ];
    let indexes = codes
        .iter()
        .zip(levels)
        .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).expect("index"))
        .collect::<Vec<_>>();
    let design = || {
        BorrowedInterceptFeDesign::new(
            rows,
            (0..3)
                .map(|index| effect(&codes[index], levels[index], &indexes[index]))
                .collect(),
        )
        .expect("design")
    };
    let weights = (0..rows)
        .map(|row| 0.7 + (row % 11) as f64 * 0.13)
        .collect::<Vec<_>>();
    let input = DenseMatrix::from_columns(
        &(0..11)
            .map(|column| {
                (0..rows)
                    .map(|row| ((row + column * 19) as f64 * 0.31).sin())
                    .collect()
            })
            .collect::<Vec<_>>(),
    )
    .expect("input");
    let reference = absorb_borrowed_intercept_cg(
        &design(),
        BorrowedMapWeights::Provided(&weights),
        input.clone(),
        config(10).map,
        BorrowedCgOptions::default(),
    )
    .expect("independent CG oracle");
    let mut previous = None;
    for (threads, capacity, width) in [
        (1, 10, 10),
        (7, 1, 1),
        (7, 6, 6),
        (7, 7, 7),
        (7, 10, 7),
        (7, 14, 14),
        (14, 10, 10),
    ] {
        let mut options = config(capacity);
        options.threads = ThreadConfig::from_requested(threads, true).expect("threads");
        let mut topology = PreparedCmgTopology::prepare(&design()).expect("topology");
        let mut absorber = CmgAbsorber::build_unpolished_prepared(
            design(),
            BorrowedMapWeights::Provided(&weights),
            options,
            &mut topology,
        )
        .expect("prepared setup");
        let CmgRoute::Graph(graph) = &absorber.route else {
            panic!("graph")
        };
        assert_eq!(graph.pcg_batch_width(capacity), width);
        assert_eq!(graph.setup_batches, 64_usize.div_ceil(width));
        let GraphCmgWorkspace::CallerBuffers(caller) = &graph.workspace else {
            panic!("caller")
        };
        assert_eq!(caller.right_hand_sides.len(), 150 * capacity);
        assert_eq!(caller.workspaces.len(), threads.min(capacity));
        let mut projected = input.clone();
        let report = absorber
            .project_unpolished(&mut projected, 1e-11)
            .expect("projection");
        assert_eq!(report.pcg_batch_width, width);
        assert_eq!(
            report.batches,
            64_usize.div_ceil(width) + 11_usize.div_ceil(width)
        );
        assert_eq!(report.right_hand_sides, 64 + 11);
        assert_matrix_close(&projected, reference.residualized(), 5e-9);
        assert!(
            certify_borrowed_intercept_candidate(
                &design(),
                BorrowedMapWeights::Provided(&weights),
                &projected,
                options.map,
            )
            .expect("fresh full-FE certificate")
            .passed
        );
        if let Some(previous) = previous.replace(projected.clone()) {
            assert_eq!(previous.as_slice(), projected.as_slice());
        }
        let owned = CmgAbsorber::build(
            design(),
            BorrowedMapWeights::Provided(&weights),
            options,
            false,
        )
        .expect("OLS setup");
        let CmgRoute::Graph(graph) = &owned.route else {
            panic!("OLS graph")
        };
        assert_eq!(graph.pcg_batch_width(capacity), capacity);
        assert_eq!(graph.setup_batches, 64_usize.div_ceil(capacity));
    }
}

#[test]
fn prepared_cmg_tracks_executor_width_across_one_four_one() {
    let worker = [0, 0, 1, 1, 2, 2, 0, 1, 2, 0, 1, 2];
    let firm = [0, 1, 0, 2, 1, 2, 2, 1, 0, 0, 1, 2];
    let worker_index = LevelIndex::from_codes(&worker, 3).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm, 3).expect("firm index");
    let make_design = || {
        BorrowedInterceptFeDesign::new(
            worker.len(),
            vec![
                effect(&worker, 3, &worker_index),
                effect(&firm, 3, &firm_index),
            ],
        )
        .expect("design")
    };
    let mut prepared = PreparedCmgTopology::prepare(&make_design()).expect("prepared topology");

    for requested in [1, 4, 1] {
        let mut threaded_config = config(2);
        threaded_config.threads =
            ThreadConfig::from_requested(requested, true).expect("thread config");
        let mut absorber = CmgAbsorber::build_unpolished_prepared(
            make_design(),
            BorrowedMapWeights::Unit,
            threaded_config,
            &mut prepared,
        )
        .expect("prepared CMG setup");
        let mut values = DenseMatrix::from_columns(&[
            (0..worker.len())
                .map(|row| (row as f64 * 0.37).sin())
                .collect(),
            (0..worker.len())
                .map(|row| (row as f64 * 0.23).cos())
                .collect(),
        ])
        .expect("projection values");
        let report = absorber
            .project_unpolished(&mut values, 1e-10)
            .expect("prepared CMG projection");
        assert_eq!(report.executor_threads, requested);
    }
}

#[test]
fn upstream_cmg_errors_have_an_explicit_fail_closed_public_class() {
    let cases = vec![
        (
            cmg::CmgError::DimensionMismatch {
                context: "test",
                expected: 2,
                actual: 1,
            },
            ErrorCode::InvalidArgument,
        ),
        (
            cmg::CmgError::VertexOutOfBounds {
                vertex: 2,
                vertex_count: 2,
            },
            ErrorCode::InvalidData,
        ),
        (
            cmg::CmgError::VertexIndexTooWide {
                vertex: usize::MAX,
                maximum: u32::MAX as usize,
            },
            ErrorCode::Allocation,
        ),
        (
            cmg::CmgError::SelfLoop { vertex: 1 },
            ErrorCode::InvalidData,
        ),
        (
            cmg::CmgError::InvalidEdgeWeight {
                u: 0,
                v: 1,
                weight: 0.0,
            },
            ErrorCode::InvalidData,
        ),
        (
            cmg::CmgError::NonFiniteMatrixValue {
                row: 0,
                column: 0,
                value: f64::NAN,
            },
            ErrorCode::InvalidData,
        ),
        (
            cmg::CmgError::NotSymmetric {
                row: 0,
                column: 1,
                forward: 1.0,
                reverse: 2.0,
            },
            ErrorCode::InvalidData,
        ),
        (
            cmg::CmgError::PositiveOffDiagonal {
                row: 0,
                column: 1,
                value: 1.0,
            },
            ErrorCode::InvalidData,
        ),
        (
            cmg::CmgError::NegativeDiagonal {
                index: 0,
                value: -1.0,
            },
            ErrorCode::InvalidData,
        ),
        (
            cmg::CmgError::NotDiagonallyDominant {
                row: 0,
                diagonal: 1.0,
                off_diagonal_sum: 2.0,
            },
            ErrorCode::InvalidData,
        ),
        (
            cmg::CmgError::IncompatibleLaplacianRhs {
                component: 0,
                sum: 1.0,
                tolerance: 0.1,
            },
            ErrorCode::NonConvergence,
        ),
        (
            cmg::CmgError::NonPositivePivot {
                vertex: 0,
                value: 0.0,
            },
            ErrorCode::NonConvergence,
        ),
        (
            cmg::CmgError::InvalidHierarchy { context: "test" },
            ErrorCode::Internal,
        ),
        (
            cmg::CmgError::PcgBreakdown {
                iteration: 1,
                quantity: "test",
                value: 0.0,
            },
            ErrorCode::NonConvergence,
        ),
        (
            cmg::CmgError::MaximumIterations {
                iterations: 10,
                residual_norm: 1.0,
                tolerance: 0.1,
            },
            ErrorCode::NonConvergence,
        ),
        (
            cmg::CmgError::ResidualVerificationFailed {
                iteration: 10,
                residual_norm: 1.0,
                tolerance: 0.1,
            },
            ErrorCode::NonConvergence,
        ),
        (
            cmg::CmgError::ParallelRuntime {
                message: "test".to_owned(),
            },
            ErrorCode::Internal,
        ),
        (
            cmg::CmgError::MemoryBudgetExceeded {
                required_bytes: 2,
                budget_bytes: 1,
            },
            ErrorCode::Allocation,
        ),
        (
            cmg::CmgError::AllocationFailed { context: "test" },
            ErrorCode::Allocation,
        ),
        (
            cmg::CmgError::InvalidOption {
                name: "test",
                value: 0.0,
            },
            ErrorCode::InvalidArgument,
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(cmg_error_code(&error), expected, "{error}");
        assert_eq!(cmg_error(error.clone()).code, expected);
        assert_eq!(cmg_solve_error(error).code, expected);
    }
}

#[test]
fn three_way_profile_uses_two_largest_effects_as_core_in_every_order() {
    let rows = 2_000;
    let worker_levels = 400;
    let firm_levels = 80;
    let year_levels = 5;
    let worker: Vec<u32> = (0..rows)
        .map(|row| (row / (rows / worker_levels)) as u32)
        .collect();
    let firm: Vec<u32> = (0..rows)
        .map(|row| ((row / 5 + row % 5) % firm_levels) as u32)
        .collect();
    let year: Vec<u32> = (0..rows).map(|row| (row % year_levels) as u32).collect();
    let wi = LevelIndex::from_codes(&worker, worker_levels).expect("worker index");
    let fi = LevelIndex::from_codes(&firm, firm_levels).expect("firm index");
    let yi = LevelIndex::from_codes(&year, year_levels).expect("year index");
    let sources = [
        (&worker, worker_levels, &wi),
        (&firm, firm_levels, &fi),
        (&year, year_levels, &yi),
    ];

    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let effects = order
            .iter()
            .map(|index| {
                let (codes, levels, level_index) = sources[*index];
                effect(codes, levels, level_index)
            })
            .collect();
        let design = BorrowedInterceptFeDesign::new(rows, effects).expect("profile design");
        let profile = CmgTopologyProfile::analyze(&design).expect("profile");
        assert_eq!(profile.highest_cardinality_levels, worker_levels);
        assert_eq!(profile.second_cardinality_levels, firm_levels);
        assert_eq!(profile.vertices, worker_levels + firm_levels);
        assert_eq!(profile.nuisance_columns(), year_levels - 1);
        let selected_original_effects = profile
            .core_effects
            .expect("two core effects")
            .map(|position| order[position]);
        assert_eq!(selected_original_effects, [0, 1]);
        assert_eq!(order[profile.effect_order[2]], 2);
    }
}

#[test]
fn swapping_cmg_bipartition_orientation_preserves_the_certified_projection() {
    let rows = 240;
    let large = (0..rows).map(|row| (row % 24) as u32).collect::<Vec<_>>();
    let medium = (0..rows)
        .map(|row| ((row * 5 + row / 24) % 9) as u32)
        .collect::<Vec<_>>();
    let nuisance = (0..rows)
        .map(|row| ((row * 7 + row / 11) % 4) as u32)
        .collect::<Vec<_>>();
    let large_index = LevelIndex::from_codes(&large, 24).expect("large index");
    let medium_index = LevelIndex::from_codes(&medium, 9).expect("medium index");
    let nuisance_index = LevelIndex::from_codes(&nuisance, 4).expect("nuisance index");
    let make_design = || {
        BorrowedInterceptFeDesign::new(
            rows,
            vec![
                effect(&large, 24, &large_index),
                effect(&medium, 9, &medium_index),
                effect(&nuisance, 4, &nuisance_index),
            ],
        )
        .expect("orientation design")
    };
    let canonical_profile = CmgTopologyProfile::analyze(&make_design()).expect("canonical profile");
    assert_eq!(canonical_profile.core_effects, Some([0, 1]));
    let mut reversed_profile = canonical_profile.clone();
    reversed_profile.effect_order.swap(0, 1);
    reversed_profile.core_effects = Some([1, 0]);
    std::mem::swap(
        &mut reversed_profile.highest_cardinality_levels,
        &mut reversed_profile.second_cardinality_levels,
    );

    let values = DenseMatrix::from_columns(&[
        (0..rows).map(|row| (row as f64 * 0.071).sin()).collect(),
        (0..rows).map(|row| (row as f64 * 0.113).cos()).collect(),
    ])
    .expect("orientation values");
    let mut canonical = CmgAbsorber::build_profiled_internal(
        make_design(),
        BorrowedMapWeights::Unit,
        config(2),
        false,
        canonical_profile,
        true,
        None,
        &mut SolverWorkspacePeak::from_bytes(0).unwrap(),
    )
    .expect("canonical orientation");
    let mut reversed = CmgAbsorber::build_profiled_internal(
        make_design(),
        BorrowedMapWeights::Unit,
        config(2),
        false,
        reversed_profile,
        true,
        None,
        &mut SolverWorkspacePeak::from_bytes(0).unwrap(),
    )
    .expect("reversed orientation");
    let canonical = canonical.absorb(values.clone()).expect("canonical solve");
    let reversed = reversed.absorb(values).expect("reversed solve");
    assert!(canonical.report().certified());
    assert!(reversed.report().certified());
    assert_matrix_close(canonical.residualized(), reversed.residualized(), 5e-10);
    assert!(canonical.report().certificate.relative_infinity_norm <= 1e-10);
    assert!(reversed.report().certificate.relative_infinity_norm <= 1e-10);
}

#[test]
fn auto_requested_cmg_skips_map_pilot_and_certifies() {
    let periods = 8;
    let workers = 100;
    let firms = 10;
    let rows = periods * workers;
    let worker: Vec<u32> = (0..rows).map(|row| (row / periods) as u32).collect();
    let firm: Vec<u32> = (0..rows)
        .map(|row| ((row / periods + row % periods) % firms) as u32)
        .collect();
    let worker_index = LevelIndex::from_codes(&worker, workers).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm, firms).expect("firm index");
    let design = BorrowedInterceptFeDesign::new(
        rows,
        vec![
            effect(&worker, workers, &worker_index),
            effect(&firm, firms, &firm_index),
        ],
    )
    .expect("design");
    let profile = CmgTopologyProfile::analyze(&design).expect("profile");
    let mut auto_config = config(3);
    auto_config.requested_solver = FeSolverKind::Auto;
    let mut session = CmgAbsorber::build_profiled_internal(
        design,
        BorrowedMapWeights::Unit,
        auto_config,
        false,
        profile,
        true,
        None,
        &mut SolverWorkspacePeak::from_bytes(0).unwrap(),
    )
    .expect("auto CMG setup");
    let values = DenseMatrix::from_columns(&[(0..rows)
        .map(|row| (row as f64 * 0.031).sin() + (row as f64 * 0.017).cos())
        .collect()])
    .expect("values");
    let result = session.absorb(values).expect("auto CMG projection");
    assert!(result.report().certified());
    assert_eq!(result.report().requested_solver, FeSolverKind::Auto);
    assert_eq!(result.report().used_solver, FeSolverKind::Cmg);
    assert_eq!(result.report().pilot_iterations, 0);
}

#[test]
fn auto_cmg_memory_preflight_rejects_before_graph_construction() {
    let worker = [0, 0, 1, 1, 2, 2, 3, 3];
    let firm = [0, 1, 0, 1, 0, 1, 0, 1];
    let worker_index = LevelIndex::from_codes(&worker, 4).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm, 2).expect("firm index");
    let design = BorrowedInterceptFeDesign::new(
        worker.len(),
        vec![
            effect(&worker, 4, &worker_index),
            effect(&firm, 2, &firm_index),
        ],
    )
    .expect("design");
    let profile = CmgTopologyProfile::analyze(&design).expect("profile");
    let mut constrained = config(1);
    constrained.requested_solver = FeSolverKind::Auto;
    constrained.solver_memory_budget = Some(FeSolverMemoryBudget {
        execution_bytes: 0,
        usable_bytes: 1,
    });
    let error = match CmgAbsorber::build_profiled_internal(
        design,
        BorrowedMapWeights::Unit,
        constrained,
        false,
        profile,
        true,
        None,
        &mut SolverWorkspacePeak::from_bytes(0).unwrap(),
    ) {
        Ok(_) => panic!("CMG memory preflight unexpectedly fit"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(error.message.contains("joint execution/solver memory plan"));
}

#[test]
fn cmg_heap_forecast_uses_actual_small_component_storage_without_runtime_allowances() {
    for components in [false, true] {
        for groups in [1, 256] {
            let n = groups * 8;
            let left = (0..n)
                .map(|i| ((i / 8) * 2 + i % 2) as u32)
                .collect::<Vec<_>>();
            let right = (0..n)
                .map(|i| ((i / 8) * 2 + (i / 2) % 2) as u32)
                .collect::<Vec<_>>();
            let li = LevelIndex::from_codes(&left, groups * 2).unwrap();
            let ri = LevelIndex::from_codes(&right, groups * 2).unwrap();
            let design = BorrowedInterceptFeDesign::new(
                n,
                vec![
                    effect(&left, groups * 2, &li),
                    effect(&right, groups * 2, &ri),
                ],
            )
            .unwrap();
            let mut options = config(2);
            options.components = components;
            options.threads = ThreadConfig::from_requested(2, true).unwrap();
            let profile = CmgTopologyProfile::analyze(&design).unwrap();
            let admission = preflight_cmg_memory(&design, &profile, options).unwrap();
            let mut session = CmgAbsorber::build_profiled_internal(
                design,
                BorrowedMapWeights::Unit,
                options,
                false,
                profile,
                true,
                None,
                &mut SolverWorkspacePeak::from_bytes(0).unwrap(),
            )
            .unwrap();
            let retained = session.retained_workspace_peak().unwrap();
            let expected = session.expected_heap_peak().expected_heap_bytes().unwrap();
            assert!(expected >= retained.expected_heap_bytes().unwrap());
            assert!(expected < admission.construction.bytes().unwrap() / 4);
            let input = (0..n)
                .map(|i| if i % 8 < 4 { -1.0 } else { 1.0 })
                .collect::<Vec<_>>();
            let projected = session
                .absorb(DenseMatrix::from_columns(&[input.clone()]).unwrap())
                .unwrap();
            assert!(projected.report().certified());
            for (&actual, &reference) in projected.residualized().as_slice().iter().zip(&input) {
                assert!((actual - reference).abs() < 1e-10);
            }
            let peak = projected.report().solver_workspace_peak;
            assert!(peak.bytes().unwrap() >= admission.construction.bytes().unwrap());
            assert!(peak.expected_heap_bytes().unwrap() < peak.bytes().unwrap() / 4);
        }
    }
}

#[test]
fn cmg_workspace_failed_nuisance_construction_retains_its_admitted_attempt() {
    let n = 6000;
    let worker = (0..n).map(|row| (row / 3) as u32).collect::<Vec<_>>();
    let firm = (0..n)
        .map(|row| ((row / 3 * 7 + row % 3 * 13) % 200) as u32)
        .collect::<Vec<_>>();
    let nuisance = (0..n)
        .map(|row| ((row / 7 + row % 3) % 3) as u32)
        .collect::<Vec<_>>();
    let wi = LevelIndex::from_codes(&worker, 2000).unwrap();
    let fi = LevelIndex::from_codes(&firm, 200).unwrap();
    let ni = LevelIndex::from_codes(&nuisance, 3).unwrap();
    let design = BorrowedInterceptFeDesign::new(
        n,
        vec![
            effect(&worker, 2000, &wi),
            effect(&firm, 200, &fi),
            effect(&nuisance, 3, &ni),
        ],
    )
    .unwrap();
    let profile = CmgTopologyProfile::analyze(&design).unwrap();
    let mut options = config(2);
    options.map.max_iterations = 1;
    options.requested_solver = FeSolverKind::Auto;
    let unbounded = preflight_cmg_memory(&design, &profile, options).unwrap();
    options.solver_memory_budget = Some(FeSolverMemoryBudget {
        execution_bytes: 512,
        usable_bytes: 1 << 30,
    });
    let expected = preflight_cmg_memory(&design, &profile, options)
        .unwrap()
        .construction;
    assert_eq!(unbounded.construction, expected);
    assert!(unbounded.projection.bytes().is_some());
    let mut attempted = SolverWorkspacePeak::ZERO;
    let result = CmgAbsorber::build_profiled_internal(
        design,
        BorrowedMapWeights::Unit,
        options,
        false,
        profile,
        false,
        None,
        &mut attempted,
    );
    match result {
        Err(error) => assert_eq!(error.code, ErrorCode::NonConvergence, "{}", error.message),
        Ok(_) => panic!("one-iteration nuisance construction unexpectedly succeeded"),
    }
    assert!(attempted.bytes().unwrap() >= expected.bytes().unwrap());
    assert!(attempted.bytes().unwrap() > 0);
    assert_eq!(
        attempted.expected_heap_bytes(),
        None,
        "failed opaque setup is not a zero-cost forecast"
    );
}

#[test]
fn cmg_workspace_report_observes_fixed_pool_reuse() {
    let n = 400;
    let left = (0..n).map(|row| (row % 20) as u32).collect::<Vec<_>>();
    let right = (0..n)
        .map(|row| ((row / 20) % 10) as u32)
        .collect::<Vec<_>>();
    let li = LevelIndex::from_codes(&left, 20).unwrap();
    let ri = LevelIndex::from_codes(&right, 10).unwrap();
    let design =
        BorrowedInterceptFeDesign::new(n, vec![effect(&left, 20, &li), effect(&right, 10, &ri)])
            .unwrap();
    let mut options = config(4);
    options.threads = ThreadConfig::from_requested(4, true).unwrap();
    options.solver_memory_budget = Some(FeSolverMemoryBudget {
        execution_bytes: 0,
        usable_bytes: 1 << 30,
    });
    let mut session = CmgAbsorber::build(design, BorrowedMapWeights::Unit, options, false).unwrap();
    let before = session.retained_workspace_peak().unwrap().bytes().unwrap();
    match &session.route {
        CmgRoute::Graph(graph) => match &graph.workspace {
            GraphCmgWorkspace::OlsBuffers(pool) => assert_eq!(pool.workspaces.len(), 4),
            _ => panic!("expected owned workspace pool"),
        },
        _ => panic!("expected CMG graph"),
    }
    let values = DenseMatrix::from_columns(
        &(0..4)
            .map(|rhs| {
                (0..n)
                    .map(|row| {
                        (rhs + 1) as f64 * ((row % 20) as f64 + if row < 200 { -1.0 } else { 1.0 })
                    })
                    .collect()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let result = session.absorb(values).unwrap();
    let after = session.retained_workspace_peak().unwrap().bytes().unwrap();
    assert_eq!(after, before, "admitted solve banks cannot grow lazily");
    match &session.route {
        CmgRoute::Graph(graph) => match &graph.workspace {
            GraphCmgWorkspace::OlsBuffers(pool) => assert_eq!(pool.workspaces.len(), 4),
            _ => panic!("expected owned workspace pool"),
        },
        _ => panic!("expected CMG graph"),
    }
    assert!(result.report().solver_workspace_peak.bytes().unwrap() >= after);
    for row in 0..n {
        for rhs in 0..4 {
            assert!(
                (result.residualized()[(row, rhs)]
                    - (rhs + 1) as f64 * if row < 200 { -1.0 } else { 1.0 })
                .abs()
                    < 1e-10
            );
        }
    }
}

#[test]
fn cmg_container_admission_accepts_exact_budget_and_rejects_one_byte_short() {
    let left = [0, 0, 1, 1];
    let right = [0, 1, 0, 1];
    let li = LevelIndex::from_codes(&left, 2).unwrap();
    let ri = LevelIndex::from_codes(&right, 2).unwrap();
    let design =
        BorrowedInterceptFeDesign::new(4, vec![effect(&left, 2, &li), effect(&right, 2, &ri)])
            .unwrap();
    let profile = CmgTopologyProfile::analyze(&design).unwrap();
    for threads in [1, 4] {
        let mut options = config(3);
        options.threads = ThreadConfig::from_requested(threads, true).unwrap();
        options.solver_memory_budget = None;
        let expected = preflight_cmg_memory(&design, &profile, options).unwrap();
        let bytes = u128::from(expected.construction.bytes().unwrap());
        options.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 197,
            usable_bytes: 197 + bytes,
        });
        assert_eq!(
            preflight_cmg_memory(&design, &profile, options)
                .unwrap()
                .construction,
            expected.construction
        );
        options.solver_memory_budget.as_mut().unwrap().usable_bytes -= 1;
        assert_eq!(
            preflight_cmg_memory(&design, &profile, options)
                .err()
                .unwrap()
                .code,
            ErrorCode::Allocation
        );
        let prepared = PreparedCmgTopology::prepare(&design).unwrap();
        options.solver_memory_budget = None;
        let expected =
            preflight_prepared_cmg_memory(&design, &profile, options, &prepared).unwrap();
        let bytes = u128::from(expected.construction.bytes().unwrap());
        options.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 197,
            usable_bytes: 197 + bytes,
        });
        assert_eq!(
            preflight_prepared_cmg_memory(&design, &profile, options, &prepared)
                .unwrap()
                .construction,
            expected.construction
        );
        options.solver_memory_budget.as_mut().unwrap().usable_bytes -= 1;
        assert_eq!(
            preflight_prepared_cmg_memory(&design, &profile, options, &prepared)
                .err()
                .unwrap()
                .code,
            ErrorCode::Allocation
        );
    }
}

#[test]
fn cmg_workspace_preflight_rejection_does_not_charge_an_unallocated_attempt() {
    let left = [0, 0, 1, 1];
    let right = [0, 1, 0, 1];
    let li = LevelIndex::from_codes(&left, 2).unwrap();
    let ri = LevelIndex::from_codes(&right, 2).unwrap();
    let design =
        BorrowedInterceptFeDesign::new(4, vec![effect(&left, 2, &li), effect(&right, 2, &ri)])
            .unwrap();
    let mut options = config(1);
    options.requested_solver = FeSolverKind::Auto;
    options.solver_memory_budget = Some(FeSolverMemoryBudget {
        execution_bytes: 512,
        usable_bytes: 513,
    });
    let mut attempted = SolverWorkspacePeak::ZERO;
    let result = CmgAbsorber::build_with_workspace(
        design,
        BorrowedMapWeights::Unit,
        options,
        false,
        &mut attempted,
    );
    assert!(matches!(
        result,
        Err(FeregError {
            code: ErrorCode::Allocation,
            ..
        })
    ));
    assert_eq!(attempted, SolverWorkspacePeak::ZERO);
}

#[test]
fn cmg_workspace_retained_polish_and_released_final_correction_have_different_peaks() {
    let n = 4 * 3 * 128;
    let left = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
    let right = (0..n).map(|row| ((row / 4) % 3) as u32).collect::<Vec<_>>();
    let li = LevelIndex::from_codes(&left, 4).unwrap();
    let ri = LevelIndex::from_codes(&right, 3).unwrap();
    let design = || {
        BorrowedInterceptFeDesign::new(n, vec![effect(&left, 4, &li), effect(&right, 3, &ri)])
            .unwrap()
    };
    let noise = (0..n)
        .map(|row| if (row / 12) % 2 == 0 { -1.0 } else { 1.0 })
        .collect::<Vec<_>>();
    let mut options = config(2);
    options.solver_memory_budget = Some(FeSolverMemoryBudget {
        execution_bytes: 512,
        usable_bytes: 1 << 30,
    });
    options.retain_coefficients = true;
    let input = DenseMatrix::from_columns(
        &(0..5)
            .map(|rhs| {
                (0..n)
                    .map(|row| {
                        (rhs + 1) as f64 * (noise[row] + left[row] as f64 + 0.5 * right[row] as f64)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let mut session =
        CmgAbsorber::build(design(), BorrowedMapWeights::Unit, options, false).unwrap();
    let construction = session.memory.construction.bytes().unwrap();
    let before = session.retained_workspace_peak().unwrap().bytes().unwrap();
    let result = session.absorb(input).unwrap();
    assert_eq!(result.report().used_solver, FeSolverKind::Cmg);
    assert!(result.report().certified());
    let after = session.retained_workspace_peak().unwrap().bytes().unwrap();
    assert!(after >= before);
    let (within, report, coefficients) = result.into_full_parts();
    let coefficients = coefficients.expect("exact crossed projection retains coefficients");
    assert_eq!(coefficients.rhs_count(), 5); // Wider than the admitted CMG batch.
    let (norms, norm_peak) = weighted_column_norms(&within, BorrowedMapWeights::Unit, 1.0).unwrap();
    let polish = absorb_borrowed_intercept_cg(
        &design(),
        BorrowedMapWeights::Unit,
        within.clone(),
        options.map,
        BorrowedCgOptions {
            retain_coefficients: true,
            ..BorrowedCgOptions::polish(&norms)
        },
    )
    .unwrap();
    assert!(polish.report().iterations <= options.map.max_iterations);
    let retained_polish = after
        + coefficients.retained_bytes() as u64
        + 5 * 8
        + polish.report().solver_workspace_peak.bytes().unwrap();
    assert!(report.solver_workspace_peak.bytes().unwrap() >= construction.max(retained_polish));
    assert_coefficients_reconstruct(
        &DenseMatrix::from_columns(
            &(0..5)
                .map(|rhs| {
                    (0..n)
                        .map(|row| {
                            (rhs + 1) as f64
                                * (noise[row] + left[row] as f64 + 0.5 * right[row] as f64)
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )
        .unwrap(),
        &within,
        &[&left, &right],
        &coefficients,
        1e-10,
    );

    let mut terminal =
        CmgAbsorber::build(design(), BorrowedMapWeights::Unit, options, false).unwrap();
    terminal.memory = CmgWorkspacePlan::EMPTY;
    let terminal_retained = terminal.retained_workspace_peak().unwrap().bytes().unwrap();
    let (certificate, workspace) =
        super::super::borrowed_map::certify_borrowed_intercept_candidate_with_workspace(
            &design(),
            BorrowedMapWeights::Unit,
            &within,
            options.map,
        )
        .unwrap();
    assert!(certificate.passed);
    let terminal_result = terminal
        .absorb_final_residual(within.clone(), options.map.tolerance)
        .unwrap();
    assert_eq!(
        terminal_result.report().solver_workspace_peak.bytes(),
        Some(
            (terminal_retained + workspace.bytes().unwrap())
                .max(
                    polish.report().solver_workspace_peak.bytes().unwrap()
                        + norms.capacity() as u64 * 8
                )
                .max(norm_peak.bytes().unwrap())
        )
    );
    assert_eq!(terminal_result.residualized(), polish.residualized());
    assert_eq!(
        terminal_result.report().iterations,
        polish.report().iterations
    );
    assert!(terminal_result.report().fallback_used);
    let (terminal_values, _, terminal_coefficients) = terminal_result.into_full_parts();
    assert_coefficients_reconstruct(
        &within,
        &terminal_values,
        &[&left, &right],
        &terminal_coefficients.expect("released-graph correction retains its own product"),
        1e-10,
    );
    for row in 0..n {
        for rhs in 0..5 {
            assert!((terminal_values[(row, rhs)] - (rhs + 1) as f64 * noise[row]).abs() < 1e-10);
        }
    }

    let input = DenseMatrix::from_columns(&[(0..n)
        .map(|row| noise[row] + if left[row] == 0 { 2e-9 } else { 0.0 })
        .collect()])
    .unwrap();
    assert_released_cmg_final_correction_peak(session, design(), input, &noise);
}

fn assert_released_cmg_final_correction_peak(
    mut session: CmgAbsorber<'_>,
    design: BorrowedInterceptFeDesign<'_>,
    input: DenseMatrix,
    expected_values: &[f64],
) {
    // Isolate the final phase after construction has already been reported.
    // This test does not change the production admission decision.
    session.memory = CmgWorkspacePlan::EMPTY;
    let retained = session.retained_workspace_peak().unwrap().bytes().unwrap();
    let mut strict = session.config.map;
    strict.tolerance = 1e-12;
    let (cert, cert_work) =
        super::super::borrowed_map::certify_borrowed_intercept_candidate_with_workspace(
            &design,
            BorrowedMapWeights::Unit,
            &input,
            strict,
        )
        .unwrap();
    assert!(!cert.passed);
    let (norms, norm_work) = weighted_column_norms(&input, BorrowedMapWeights::Unit, 1.0).unwrap();
    let reference = absorb_borrowed_intercept_cg(
        &design,
        BorrowedMapWeights::Unit,
        input.clone(),
        strict,
        BorrowedCgOptions {
            retain_coefficients: session.config.retain_coefficients,
            ..BorrowedCgOptions::polish(&norms)
        },
    )
    .unwrap();
    assert!(reference.report().iterations > 0);
    let correction =
        reference.report().solver_workspace_peak.bytes().unwrap() + norms.capacity() as u64 * 8;
    let expected = (retained + cert_work.bytes().unwrap())
        .max(correction)
        .max(norm_work.bytes().unwrap());
    assert!(
        expected < retained + correction,
        "the released hierarchy must not be added to CG"
    );
    let result = session.absorb_final_residual(input, 1e-12).unwrap();
    assert_eq!(
        result.report().solver_workspace_peak.bytes(),
        Some(expected)
    );
    assert!(result.report().fallback_used);
    for (residual, expected) in result.residualized().as_slice().iter().zip(expected_values) {
        assert!((residual - expected).abs() < 1e-12);
    }
}

#[test]
fn near_within_cmg_hadamard_batch_preserves_forward_accuracy() {
    let rows = 48;
    let left: Vec<_> = (0..rows).map(|row| (row % 4) as u32).collect();
    let right: Vec<_> = (0..rows).map(|row| ((row / 4) % 3) as u32).collect();
    let li = LevelIndex::from_codes(&left, 4).unwrap();
    let ri = LevelIndex::from_codes(&right, 3).unwrap();
    let u: Vec<_> = (0..rows)
        .map(|row| if row / 12 < 2 { 1.0 } else { -1.0 })
        .collect();
    let v: Vec<_> = (0..rows)
        .map(|row| if (row / 12) % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let x1: Vec<_> = (0..rows)
        .map(|row| u[row] + 2e-9 * (left[row] + 1) as f64)
        .collect();
    let x2: Vec<_> = (0..rows)
        .map(|row| v[row] + 1e-9 * (right[row] + 1) as f64)
        .collect();
    let y: Vec<_> = (0..rows)
        .map(|row| {
            0.7 * x1[row] - 0.4 * x2[row]
                + 2e-9 * (left[row] + 1) as f64
                + 1e-9 * (right[row] + 1) as f64
                + 4.0 * u[row] * v[row]
        })
        .collect();
    let input = DenseMatrix::from_columns(&[y, x1, x2, vec![1.0; rows]]).unwrap();
    let design =
        BorrowedInterceptFeDesign::new(rows, vec![effect(&left, 4, &li), effect(&right, 3, &ri)])
            .unwrap();
    let mut options = config(4);
    options.map.tolerance = 1e-14;
    options.retain_coefficients = true;
    let mut session = CmgAbsorber::build(design, BorrowedMapWeights::Unit, options, false).unwrap();
    let result = session
        .absorb(input.clone())
        .expect("well-conditioned Hadamard design must certify");
    let (within, report, coefficients) = result.into_full_parts();
    assert!(report.certified());
    for row in 0..rows {
        assert!(
            (within[(row, 0)] - (0.7 * u[row] - 0.4 * v[row] + 4.0 * u[row] * v[row])).abs()
                < 1e-12,
            "row={row} value={} expected={}",
            within[(row, 0)],
            0.7 * u[row] - 0.4 * v[row] + 4.0 * u[row] * v[row]
        );
        assert!((within[(row, 1)] - u[row]).abs() < 1e-12);
        assert!((within[(row, 2)] - v[row]).abs() < 1e-12);
        assert!(within[(row, 3)].abs() < 1e-12);
    }
    if let Some(coefficients) = coefficients {
        assert_coefficients_reconstruct(&input, &within, &[&left, &right], &coefficients, 1e-12);
    }
}

#[test]
fn terminal_cmg_polish_resolves_small_weak_cell_potentials() {
    // Equal-weight +/- pairs make the independent within projection exact.
    // The added FE signal cancels on heavy edges, so its nonzero moments can
    // pass a tight certificate while its light-cell forward error remains.
    let levels = 32;
    let rows = 4 * levels;
    let worker: Vec<_> = (0..rows).map(|row| (row / 4) as u32).collect();
    let firm: Vec<_> = (0..rows)
        .map(|row| ((row / 4 + (row / 2) % 2) % levels) as u32)
        .collect();
    let worker_index = LevelIndex::from_codes(&worker, levels).unwrap();
    let firm_index = LevelIndex::from_codes(&firm, levels).unwrap();
    let design = || {
        BorrowedInterceptFeDesign::new(
            rows,
            vec![
                effect(&worker, levels, &worker_index),
                effect(&firm, levels, &firm_index),
            ],
        )
        .unwrap()
    };
    let truth: Vec<_> = (0..rows)
        .map(|row| if row % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let mut options = config(1);
    options.map.tolerance = 1e-14;
    options.map.max_iterations = 256;
    for amplitude in [0.0, 1e-8] {
        let potential: Vec<_> = (0..levels)
            .map(|level| amplitude * ((0.73 * level as f64).sin() + (0.31 * level as f64).cos()))
            .collect();
        let input = DenseMatrix::from_columns(&[truth
            .iter()
            .enumerate()
            .map(|(row, value)| {
                value + potential[firm[row] as usize] - potential[worker[row] as usize]
            })
            .collect()])
        .unwrap();
        let raw: Vec<_> = (0..rows)
            .map(|row| if (row / 2) % 2 == 0 { 1.0 } else { 1e-6 })
            .collect();
        let weights = BorrowedMapWeights::Provided(&raw);
        let certificate =
            certify_borrowed_intercept_candidate(&design(), weights, &input, options.map).unwrap();
        assert!(certificate.passed);
        assert_eq!(certificate.absolute_infinity_norm == 0.0, amplitude == 0.0);
        let session = CmgAbsorber::build(design(), weights, options, false).unwrap();
        let result = session.absorb_final_residual(input, 1e-14).unwrap();
        assert!(result.report().certified());
        let error = truth
            .iter()
            .enumerate()
            .map(|(row, expected)| (result.residualized()[(row, 0)] - expected).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            error <= 1e-10,
            "amplitude={amplitude:e} error={error:e} iterations={}",
            result.report().iterations
        );
        if amplitude == 0.0 {
            assert_eq!(result.residualized().as_slice(), truth.as_slice());
        }
        assert!(result.report().iterations <= options.map.max_iterations);
        assert!(result.report().fallback_used);
        assert_eq!(result.report().used_solver, FeSolverKind::Cg);
    }
}

#[test]
fn retained_cmg_polish_admits_actual_rhs_width_at_the_exact_joint_boundary() {
    let n = 4 * 3 * 4;
    let left = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
    let right = (0..n).map(|row| ((row / 4) % 3) as u32).collect::<Vec<_>>();
    let li = LevelIndex::from_codes(&left, 4).unwrap();
    let ri = LevelIndex::from_codes(&right, 3).unwrap();
    let design = || {
        BorrowedInterceptFeDesign::new(n, vec![effect(&left, 4, &li), effect(&right, 3, &ri)])
            .unwrap()
    };
    let input = DenseMatrix::from_columns(
        &(0..5)
            .map(|rhs| {
                (0..n)
                    .map(|row| {
                        let noise = if (row / 12) % 2 == 0 { -1.0 } else { 1.0 };
                        (rhs + 1) as f64 * (noise + left[row] as f64 + 0.5 * right[row] as f64)
                    })
                    .collect()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let mut options = config(2);
    options.retain_coefficients = true;
    for short in [true, false] {
        let mut session =
            CmgAbsorber::build(design(), BorrowedMapWeights::Unit, options, false).unwrap();
        // Warm the real CMG RHS buffers, then test only the next polishing
        // admission. Its actual five-column result exceeds the graph batch.
        let warm = session.absorb(input.clone()).unwrap();
        let (_, _, coefficients) = warm.into_full_parts();
        let bulk_bytes = coefficients.unwrap().retained_bytes();
        let graph_bytes = u128::from(session.retained_workspace_peak().unwrap().bytes().unwrap());
        let estimate = estimate_fe_solver_memory(n, 5, &[4, 3], 1, None).unwrap();
        let required = graph_bytes
            + 5 * 8
            + bulk_bytes
            + estimate.cg_bytes
            + 4 * estimate.coefficient_batch_bytes;
        session.config.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 512,
            usable_bytes: 512 + required - u128::from(short),
        });
        let result = session.absorb(input.clone());
        if short {
            let error = result.unwrap_err();
            assert_eq!(error.code, ErrorCode::Allocation);
            assert!(error.message.contains("certified CMG polishing requires"));
        } else {
            let (within, report, coefficients) = result.unwrap().into_full_parts();
            assert!(report.certified());
            assert_eq!(report.used_solver, FeSolverKind::Cmg);
            let coefficients = coefficients.expect("retained CMG plus strict correction product");
            assert_eq!(coefficients.rhs_count(), 5);
            assert_coefficients_reconstruct(
                &input,
                &within,
                &[&left, &right],
                &coefficients,
                1e-10,
            );
            for row in 0..n {
                let noise = if (row / 12) % 2 == 0 { -1.0 } else { 1.0 };
                for rhs in 0..5 {
                    assert!((within[(row, rhs)] - (rhs + 1) as f64 * noise).abs() < 1e-10);
                }
            }
        }
    }
}

#[test]
fn terminal_residual_correction_checks_its_cg_budget() {
    let n = 4 * 3 * 4;
    let left = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
    let right = (0..n).map(|row| ((row / 4) % 3) as u32).collect::<Vec<_>>();
    let li = LevelIndex::from_codes(&left, 4).unwrap();
    let ri = LevelIndex::from_codes(&right, 3).unwrap();
    let noise = (0..n)
        .map(|row| if (row / 12) % 2 == 0 { -1.0 } else { 1.0 })
        .collect::<Vec<_>>();
    let input = || {
        DenseMatrix::from_columns(&[(0..n)
            .map(|row| noise[row] + if left[row] == 0 { 2e-9 } else { 0.0 })
            .collect()])
        .unwrap()
    };
    let estimate = estimate_fe_solver_memory(n, 1, &[4, 3], 1, None).unwrap();
    let incremental = estimate.cg_bytes + 3 * estimate.coefficient_batch_bytes + 8;
    for short in [true, false] {
        let design =
            BorrowedInterceptFeDesign::new(n, vec![effect(&left, 4, &li), effect(&right, 3, &ri)])
                .unwrap();
        let mut options = config(1);
        options.map.tolerance = 1e-8;
        let mut session =
            CmgAbsorber::build(design, BorrowedMapWeights::Unit, options, false).unwrap();
        assert!(matches!(&session.route, CmgRoute::Graph(_)));
        // Isolate admission of the terminal correction after construction.
        // Its hierarchy is released before this independent reserve.
        session.config.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 512,
            usable_bytes: 512 + incremental - u128::from(short),
        });
        let result = session.absorb_final_residual(input(), 1e-12);
        if short {
            let error = result.unwrap_err();
            assert_eq!(error.code, ErrorCode::Allocation);
            assert!(error.message.contains("certified CMG polishing requires"));
        } else {
            let result = result.unwrap();
            assert!(result.report().certified());
            assert_eq!(result.report().requested_solver, FeSolverKind::Cmg);
            assert_eq!(result.report().used_solver, FeSolverKind::Cg);
            assert!(result.report().fallback_used);
            assert_eq!(result.report().checkpoint_retries, 0);
            let (residual, _) = result.into_parts();
            assert!(residual
                .as_slice()
                .iter()
                .zip(&noise)
                .all(|(a, b)| (a - b).abs() < 1e-12));
        }
    }
}

#[test]
fn admitted_components_and_ordinary_projection_share_the_independent_oracle() {
    for disconnected in [false, true] {
        let left = [0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
        let right = if disconnected {
            [0, 1, 0, 1, 2, 3, 2, 3, 0, 1, 2, 3]
        } else {
            [0, 1, 1, 2, 2, 3, 3, 0, 0, 1, 2, 3]
        };
        let li = LevelIndex::from_codes(&left, 4).unwrap();
        let ri = LevelIndex::from_codes(&right, 4).unwrap();
        let design = || {
            BorrowedInterceptFeDesign::new(
                left.len(),
                vec![effect(&left, 4, &li), effect(&right, 4, &ri)],
            )
            .unwrap()
        };
        let weights = [1.0, 1e-10, 2.0, 3.0, 1.0, 2.0, 1.0, 4.0, 2.0, 1.0, 3.0, 2.0];
        let values = DenseMatrix::from_columns(&[
            (0..left.len()).map(|r| (r as f64 * 0.3).sin()).collect(),
            (0..left.len()).map(|r| (r as f64 * 0.2).cos()).collect(),
        ])
        .unwrap();
        let reference = absorb_borrowed_intercept_cg(
            &design(),
            BorrowedMapWeights::Provided(&weights),
            values.clone(),
            config(2).map,
            BorrowedCgOptions::default(),
        )
        .unwrap();
        for components in [false, true] {
            let mut options = config(2);
            options.components = components;
            options.retain_coefficients = true;
            let mut session = CmgAbsorber::build(
                design(),
                BorrowedMapWeights::Provided(&weights),
                options,
                false,
            )
            .unwrap();
            let outcome = session.absorb(values.clone()).unwrap();
            assert!(outcome.report().certified());
            assert_eq!(outcome.report().used_solver, FeSolverKind::Cmg);
            assert_matrix_close(outcome.residualized(), reference.residualized(), 5e-9);
            let (residual, _, coefficients) = outcome.into_full_parts();
            assert_coefficients_reconstruct(
                &values,
                &residual,
                &[&left, &right],
                &coefficients.expect("retained component coefficients"),
                5e-9,
            );
        }
    }
}

#[test]
fn weighted_two_way_projection_agrees_with_certified_cg() {
    let worker = [0, 0, 1, 1, 2, 2, 0, 1, 2, 0, 1, 2];
    let firm = [0, 1, 0, 2, 1, 2, 2, 1, 0, 0, 1, 2];
    let worker_index = LevelIndex::from_codes(&worker, 3).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm, 3).expect("firm index");
    let design = BorrowedInterceptFeDesign::new(
        worker.len(),
        vec![
            effect(&worker, 3, &worker_index),
            effect(&firm, 3, &firm_index),
        ],
    )
    .expect("design");
    let reference_design = BorrowedInterceptFeDesign::new(
        worker.len(),
        vec![
            effect(&worker, 3, &worker_index),
            effect(&firm, 3, &firm_index),
        ],
    )
    .expect("reference design");
    let weights = [1.0, 1.5, 0.8, 2.0, 1.2, 0.7, 1.8, 1.1, 0.9, 1.4, 1.6, 1.3];
    let values = DenseMatrix::from_columns(&[
        (0..worker.len())
            .map(|row| (row as f64 * 0.37).sin())
            .collect(),
        (0..worker.len())
            .map(|row| (row as f64 * 0.23).cos())
            .collect(),
    ])
    .expect("values");
    let reference = absorb_borrowed_intercept_cg(
        &reference_design,
        BorrowedMapWeights::Provided(&weights),
        values.clone(),
        config(2).map,
        BorrowedCgOptions::default(),
    )
    .expect("CG reference");
    let mut session = CmgAbsorber::build(
        design,
        BorrowedMapWeights::Provided(&weights),
        config(2),
        false,
    )
    .expect("CMG setup");
    let result = session.absorb(values).expect("CMG projection");
    let (result_values, report) = result.into_parts();
    let (reference_values, _) = reference.into_parts();
    assert_eq!(report.requested_solver, FeSolverKind::Cmg);
    assert_eq!(report.used_solver, FeSolverKind::Cmg);
    assert!(report.certified());
    assert_matrix_close(&result_values, &reference_values, 5e-9);
}

#[test]
fn duplicate_matches_and_disconnected_components_are_supported() {
    let worker = [0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
    let firm = [0, 0, 1, 1, 2, 2, 3, 3, 1, 0, 3, 2];
    let worker_index = LevelIndex::from_codes(&worker, 4).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm, 4).expect("firm index");
    let design = BorrowedInterceptFeDesign::new(
        worker.len(),
        vec![
            effect(&worker, 4, &worker_index),
            effect(&firm, 4, &firm_index),
        ],
    )
    .expect("design");
    let values =
        DenseMatrix::from_columns(&[(0..worker.len()).map(|row| row as f64 - 3.0).collect()])
            .expect("values");
    let mut session =
        CmgAbsorber::build(design, BorrowedMapWeights::Unit, config(1), false).expect("CMG setup");
    let result = session.absorb(values).expect("CMG projection");
    assert!(result.report().certified());
    assert_eq!(result.report().used_solver, FeSolverKind::Cmg);
}

#[test]
fn multiple_rhs_calls_reuse_one_hierarchy() {
    let worker = [0, 0, 1, 1, 2, 2, 0, 1, 2];
    let firm = [0, 1, 0, 1, 1, 2, 2, 2, 0];
    let worker_index = LevelIndex::from_codes(&worker, 3).expect("worker index");
    let firm_index = LevelIndex::from_codes(&firm, 3).expect("firm index");
    let design = BorrowedInterceptFeDesign::new(
        worker.len(),
        vec![
            effect(&worker, 3, &worker_index),
            effect(&firm, 3, &firm_index),
        ],
    )
    .expect("design");
    let mut session =
        CmgAbsorber::build(design, BorrowedMapWeights::Unit, config(2), false).expect("CMG setup");
    let first = session
        .absorb(
            DenseMatrix::from_columns(&[vec![1.0, 0.2, -0.4, 1.3, 2.1, -0.7, 0.9, 1.7, -1.2]])
                .expect("first values"),
        )
        .expect("first projection");
    let second = session
        .absorb(
            DenseMatrix::from_columns(&[vec![-0.5, 0.1, 0.6, -1.0, 0.8, 1.4, -0.2, 0.7, 1.1]])
                .expect("second values"),
        )
        .expect("second projection");
    assert!(first.report().setup_seconds >= 0.0);
    assert_eq!(second.report().setup_seconds, 0.0);
    assert!(first.report().certified() && second.report().certified());
}

#[test]
fn one_effect_uses_exact_direct_projection_but_retains_cmg_request() {
    let group = [0, 0, 1, 1, 2, 2];
    let index = LevelIndex::from_codes(&group, 3).expect("group index");
    let design = BorrowedInterceptFeDesign::new(group.len(), vec![effect(&group, 3, &index)])
        .expect("design");
    let mut session = CmgAbsorber::build(design, BorrowedMapWeights::Unit, config(2), false)
        .expect("direct setup");
    let result = session
        .absorb(DenseMatrix::from_columns(&[vec![1.0, 3.0, 2.0, 6.0, -1.0, 5.0]]).expect("values"))
        .expect("direct projection");
    let (values, report) = result.into_parts();
    assert_eq!(report.requested_solver, FeSolverKind::Cmg);
    assert_eq!(report.used_solver, FeSolverKind::Map);
    assert!(report.certified());
    assert_eq!(values.as_slice(), &[-1.0, 1.0, -2.0, 2.0, -3.0, 3.0]);
}

#[test]
fn absorbed_nuisance_effect_agrees_with_full_multi_effect_cg() {
    let worker = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 0, 1, 2, 3, 4];
    let firm = [0, 1, 0, 2, 1, 3, 2, 3, 0, 1, 2, 3, 0, 1, 2];
    let year = [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2];
    let wi = LevelIndex::from_codes(&worker, 5).expect("worker index");
    let fi = LevelIndex::from_codes(&firm, 4).expect("firm index");
    let yi = LevelIndex::from_codes(&year, 3).expect("year index");
    let make_design = || {
        BorrowedInterceptFeDesign::new(
            worker.len(),
            vec![
                effect(&worker, 5, &wi),
                effect(&firm, 4, &fi),
                effect(&year, 3, &yi),
            ],
        )
        .expect("design")
    };
    let values = DenseMatrix::from_columns(&[
        (0..worker.len())
            .map(|row| (row as f64 * 0.41).sin() + row as f64 * 0.03)
            .collect(),
        (0..worker.len())
            .map(|row| (row as f64 * 0.17).cos())
            .collect(),
    ])
    .expect("values");
    let reference = absorb_borrowed_intercept_cg(
        &make_design(),
        BorrowedMapWeights::Unit,
        values.clone(),
        config(2).map,
        BorrowedCgOptions::default(),
    )
    .expect("CG reference");
    let mut retaining_config = config(2);
    retaining_config.retain_coefficients = true;
    let mut session = CmgAbsorber::build(
        make_design(),
        BorrowedMapWeights::Unit,
        retaining_config,
        false,
    )
    .expect("CMG setup");
    let result = session.absorb(values.clone()).expect("CMG projection");
    let (result_values, report, coefficients) = result.into_full_parts();
    let (reference_values, _) = reference.into_parts();
    assert!(report.certified());
    assert_matrix_close(&result_values, &reference_values, 5e-9);
    let coefficients = coefficients.expect("certified CMG coefficient product");
    assert_coefficients_reconstruct(
        &values,
        &result_values,
        &[&worker, &firm, &year],
        &coefficients,
        5e-9,
    );
}

#[test]
fn multiple_nuisance_effects_agree_with_full_multi_effect_cg() {
    let worker = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 0, 1, 2, 3, 4, 0, 2, 4];
    let firm = [0, 1, 0, 2, 1, 3, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 1, 0];
    let year = [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2];
    let region = [0, 0, 1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0];
    let wi = LevelIndex::from_codes(&worker, 5).expect("worker index");
    let fi = LevelIndex::from_codes(&firm, 4).expect("firm index");
    let yi = LevelIndex::from_codes(&year, 3).expect("year index");
    let ri = LevelIndex::from_codes(&region, 2).expect("region index");
    let make_design = || {
        BorrowedInterceptFeDesign::new(
            worker.len(),
            vec![
                effect(&worker, 5, &wi),
                effect(&firm, 4, &fi),
                effect(&year, 3, &yi),
                effect(&region, 2, &ri),
            ],
        )
        .expect("design")
    };
    let weights = (0..worker.len())
        .map(|row| 0.8 + (row % 5) as f64 * 0.17)
        .collect::<Vec<_>>();
    let values = DenseMatrix::from_columns(&[
        (0..worker.len())
            .map(|row| (row as f64 * 0.37).sin() + row as f64 * 0.02)
            .collect(),
        (0..worker.len())
            .map(|row| (row as f64 * 0.19).cos() - row as f64 * 0.01)
            .collect(),
    ])
    .expect("values");
    let reference = absorb_borrowed_intercept_cg(
        &make_design(),
        BorrowedMapWeights::Provided(&weights),
        values.clone(),
        config(2).map,
        BorrowedCgOptions::default(),
    )
    .expect("CG reference");
    let mut session = CmgAbsorber::build(
        make_design(),
        BorrowedMapWeights::Provided(&weights),
        config(2),
        false,
    )
    .expect("CMG setup");
    let result = session.absorb(values).expect("CMG projection");
    let (result_values, report) = result.into_parts();
    let (reference_values, _) = reference.into_parts();
    assert!(report.certified());
    assert_matrix_close(&result_values, &reference_values, 5e-9);
}

#[test]
fn nested_rank_deficient_nuisance_effect_is_handled() {
    let worker = [0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
    let firm = [0, 1, 0, 2, 1, 2, 0, 1, 2, 2, 0, 1];
    let nested = [0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1];
    let wi = LevelIndex::from_codes(&worker, 4).expect("worker index");
    let fi = LevelIndex::from_codes(&firm, 3).expect("firm index");
    let ni = LevelIndex::from_codes(&nested, 2).expect("nested index");
    let design = BorrowedInterceptFeDesign::new(
        worker.len(),
        vec![
            effect(&worker, 4, &wi),
            effect(&firm, 3, &fi),
            effect(&nested, 2, &ni),
        ],
    )
    .expect("design");
    let values = DenseMatrix::from_columns(&[(0..worker.len())
        .map(|row| (row as f64 * 0.29).sin())
        .collect()])
    .expect("values");
    let mut session =
        CmgAbsorber::build(design, BorrowedMapWeights::Unit, config(1), false).expect("CMG setup");
    let result = session.absorb(values).expect("CMG projection");
    assert!(result.report().certified());
}

#[test]
fn more_than_sixty_four_nuisance_columns_are_rejected() {
    let rows = 134;
    let worker: Vec<u32> = (0..rows).map(|row| (row % 67) as u32).collect();
    let firm: Vec<u32> = (0..rows)
        .map(|row| ((row + row / 67) % 67) as u32)
        .collect();
    let nuisance: Vec<u32> = (0..rows).map(|row| (row % 66) as u32).collect();
    let wi = LevelIndex::from_codes(&worker, 67).expect("worker index");
    let fi = LevelIndex::from_codes(&firm, 67).expect("firm index");
    let ni = LevelIndex::from_codes(&nuisance, 66).expect("nuisance index");
    let design = BorrowedInterceptFeDesign::new(
        rows,
        vec![
            effect(&worker, 67, &wi),
            effect(&firm, 67, &fi),
            effect(&nuisance, 66, &ni),
        ],
    )
    .expect("design");
    let error = match CmgAbsorber::build(design, BorrowedMapWeights::Unit, config(1), false) {
        Ok(_) => panic!("nuisance limit was not enforced"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert!(error.message.contains("at most 64"));
}
