use super::*;

#[test]
fn ordered_scratch_allocation_keeps_the_allocation_failure_contract() {
    let error = lsmr_error(SolveError::Allocation {
        context: "ordered scratch",
    });
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(
        lsmr_error(solve_shape_error("fixture")).code,
        ErrorCode::NonConvergence
    );
}
use crate::fe::EffectTerm;

fn predictor_frame(
    scheduled_tolerance: f64,
    effective_tolerance: f64,
    first_certificate: f64,
    iterations: usize,
    continuation: JointLsmrContinuationCause,
) -> JointLsmrFrameDiagnostic {
    JointLsmrFrameDiagnostic {
        scheduled_tolerance,
        first_pass_tolerance: scheduled_tolerance,
        effective_tolerance,
        first_pass_iterations: iterations,
        continuation_iterations: 0,
        first_pass_recurrence_residual: 1e-6,
        recurrence_residual: 1e-7,
        first_certificate_relative_infinity_norm: first_certificate,
        finite_certificate_max: 1e-8,
        fixed_effect_certificate_max: 2e-8,
        max_abs_log_hessian_weight_change: 0.1,
        weight_change_valid: true,
        predictor_used: false,
        predictor_refreshed_preconditioner: false,
        continuation,
    }
}

#[test]
fn first_pass_predictor_initializes_to_the_scheduled_tolerance() {
    let predictor = JointLsmrFirstPassPredictor::default();
    let policy = predictor.policy(1e-8, true, 0.1, CategoricalLsmrPolicy::CorePair);
    assert_eq!(policy.tolerance.to_bits(), 1e-8_f64.to_bits());
    assert!(!policy.used_predictor);
    assert!(!policy.refresh_preconditioner);
}

#[test]
fn first_pass_predictor_clamps_prior_certificate_misses() {
    let mut predictor = JointLsmrFirstPassPredictor::default();
    predictor.record_certified(predictor_frame(
        1e-8,
        1e-10,
        2e-8,
        200,
        JointLsmrContinuationCause {
            recurrence: false,
            certificate: true,
            staleness: false,
        },
    ));
    let predicted = predictor.policy(1e-9, true, 0.1, CategoricalLsmrPolicy::CorePair);
    assert!((predicted.tolerance - 1e-11).abs() <= 1e-26);
    assert!(predicted.used_predictor);
    assert!(predicted.refresh_preconditioner);

    let mut floor_predictor = JointLsmrFirstPassPredictor::default();
    floor_predictor.record_certified(predictor_frame(
        5e-12,
        5e-14,
        1e-11,
        10,
        JointLsmrContinuationCause {
            recurrence: true,
            certificate: false,
            staleness: false,
        },
    ));
    let floor = floor_predictor.policy(5e-14, true, 0.1, CategoricalLsmrPolicy::CorePair);
    assert_eq!(
        floor.tolerance.to_bits(),
        JOINT_LSMR_NUMERICAL_FLOOR.to_bits()
    );
    assert!(floor.tolerance <= 5e-14);

    let mut passing = JointLsmrFirstPassPredictor::default();
    passing.record_certified(predictor_frame(
        1e-8,
        1e-8,
        5e-9,
        10,
        JointLsmrContinuationCause::default(),
    ));
    let scheduled = passing.policy(1e-9, true, 0.1, CategoricalLsmrPolicy::CorePair);
    assert_eq!(scheduled.tolerance.to_bits(), 1e-9_f64.to_bits());
    assert!(!scheduled.refresh_preconditioner);

    let two_effect = predictor.policy(1e-9, true, 0.1, CategoricalLsmrPolicy::Reuse);
    assert_eq!(two_effect.tolerance.to_bits(), 1e-9_f64.to_bits());
    assert!(!two_effect.used_predictor);
    assert!(!two_effect.refresh_preconditioner);
}

#[test]
fn first_pass_predictor_rejects_weight_and_schedule_jumps() {
    let mut predictor = JointLsmrFirstPassPredictor::default();
    predictor.record_certified(predictor_frame(
        1e-8,
        1e-10,
        2e-8,
        200,
        JointLsmrContinuationCause::default(),
    ));
    for policy in [
        predictor.policy(1e-9, false, 0.1, CategoricalLsmrPolicy::CorePair),
        predictor.policy(
            1e-9,
            true,
            JOINT_LSMR_PREDICTOR_MAX_LOG_WEIGHT_CHANGE + f64::EPSILON,
            CategoricalLsmrPolicy::CorePair,
        ),
        predictor.policy(1e-11, true, 0.1, CategoricalLsmrPolicy::CorePair),
        predictor.policy(2e-8, true, 0.1, CategoricalLsmrPolicy::CorePair),
    ] {
        assert!(!policy.used_predictor);
        assert!(!policy.refresh_preconditioner);
    }
}

#[test]
fn first_pass_predictor_invalidation_is_atomic_and_matches_engine_replacement() {
    let frame = predictor_frame(1e-8, 1e-8, 5e-9, 200, JointLsmrContinuationCause::default());
    let mut predictor = JointLsmrFirstPassPredictor::default();
    predictor.record_certified(frame);
    let snapshot = predictor;
    let _speculative = predictor.policy(1e-9, true, 0.1, CategoricalLsmrPolicy::CorePair);
    assert_eq!(predictor, snapshot);

    predictor.invalidate();
    assert_eq!(predictor, JointLsmrFirstPassPredictor::default());
    let replacement_for_changed_basis = JointLsmrFirstPassPredictor::default();
    assert_eq!(predictor, replacement_for_changed_basis);
}

#[test]
fn joint_lsmr_frame_trace_is_deterministic_and_complete() {
    let mut frame = predictor_frame(
        1e-8,
        1e-9,
        2e-8,
        200,
        JointLsmrContinuationCause {
            recurrence: true,
            certificate: true,
            staleness: true,
        },
    );
    frame.predictor_used = true;
    frame.predictor_refreshed_preconditioner = true;
    let first = joint_lsmr_frame_trace_line(frame);
    let second = joint_lsmr_frame_trace_line(frame);
    assert_eq!(first, second);
    for field in [
        "scheduled=",
        "first_tolerance=",
        "effective=",
        "first_iterations=",
        "continuation_iterations=",
        "first_normal=",
        "final_normal=",
        "first_certificate=",
        "finite_max=",
        "fe_max=",
        "weight_change=",
        "predictor_used=1",
        "predictor_refresh=1",
        "recurrence_continuation=1",
        "certificate_continuation=1",
        "staleness_continuation=1",
    ] {
        assert!(first.contains(field), "trace omitted {field}");
    }
}

#[test]
fn augmented_operator_obeys_adjoint_identity() {
    let left = vec![0, 0, 1, 1];
    let right = vec![0, 1, 0, 1];
    let design = FeDesign::new(
        4,
        vec![
            EffectTerm::new(left, 2, true, Vec::new()).expect("left"),
            EffectTerm::new(right, 2, true, Vec::new()).expect("right"),
        ],
    )
    .expect("design");
    let active = DenseMatrix::from_row_major(4, 1, vec![1.0, 2.0, -1.0, 0.5]).expect("active");
    let offsets = vec![0, 2, 4];
    let weights = vec![1.0, 2.0_f64.sqrt(), 0.5_f64.sqrt(), 1.5_f64.sqrt()];
    let operator =
        AugmentedCategoricalOperator::new(&design, &active, &offsets, &weights).expect("operator");
    let x = vec![0.25, 1.0, -0.5, 0.75, -1.25];
    let y = vec![0.3, -0.2, 0.8, 1.1];
    let mut ax = vec![0.0; operator.nrows()];
    let mut aty = vec![0.0; operator.ncols()];
    operator.apply(&x, &mut ax).expect("apply");
    operator.apply_adjoint(&y, &mut aty).expect("adjoint");
    let left_product = ax.iter().zip(&y).map(|(a, b)| a * b).sum::<f64>();
    let right_product = x.iter().zip(&aty).map(|(a, b)| a * b).sum::<f64>();
    assert!((left_product - right_product).abs() <= 1e-13);
}

#[test]
fn augmented_products_match_explicit_design_across_widths_and_teams() {
    for rows in [17, 131] {
        let design = FeDesign::new(
            rows,
            vec![
                EffectTerm::new(
                    (0..rows).map(|r| (r % 3) as u32).collect(),
                    3,
                    true,
                    Vec::new(),
                )
                .unwrap(),
                EffectTerm::new(
                    (0..rows).map(|r| ((r / 3) % 5) as u32).collect(),
                    5,
                    true,
                    Vec::new(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        for columns in [0, 1, 6, 17] {
            let active = DenseMatrix::from_row_major(
                rows,
                columns,
                (0..rows * columns)
                    .map(|i| ((i * 17 + 3) as f64).sin())
                    .collect(),
            )
            .unwrap();
            let coefficients: Vec<_> = (0..columns + 8)
                .map(|i| ((i * 7 + 2) as f64).cos())
                .collect();
            let input: Vec<_> = (0..rows).map(|r| ((r * 11 + 1) as f64).sin()).collect();
            for generation in 0..2 {
                let roots: Vec<_> = (0..rows)
                    .map(|r| 10.0_f64.powi(((r * 7 + generation * 3) % 13) as i32 - 6))
                    .collect();
                // Independently materialize the mathematical weighted design.
                let mut explicit = vec![vec![0.0; columns + 8]; rows];
                for row in 0..rows {
                    for col in 0..columns {
                        explicit[row][col] = roots[row] * active[(row, col)];
                    }
                    explicit[row][columns + row % 3] = roots[row];
                    explicit[row][columns + 3 + (row / 3) % 5] = roots[row];
                }
                let expected_forward: Vec<f64> = explicit
                    .iter()
                    .map(|row| row.iter().zip(&coefficients).map(|(a, b)| a * b).sum())
                    .collect();
                let expected_adjoint: Vec<f64> = (0..columns + 8)
                    .map(|col| (0..rows).map(|r| explicit[r][col] * input[r]).sum())
                    .collect();
                for threads in [1, 4] {
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap();
                    pool.install(|| {
                        let operator =
                            AugmentedCategoricalOperator::new(&design, &active, &[0, 3, 8], &roots)
                                .unwrap();
                        let mut forward = vec![0.0; rows];
                        let mut adjoint = vec![0.0; columns + 8];
                        operator.apply(&coefficients, &mut forward).unwrap();
                        operator.apply_adjoint(&input, &mut adjoint).unwrap();
                        for row in 0..rows {
                            let scale: f64 = explicit[row]
                                .iter()
                                .zip(&coefficients)
                                .map(|(a, b)| (a * b).abs())
                                .sum();
                            assert!(
                                (forward[row] - expected_forward[row]).abs()
                                    <= 1e-12 * scale.max(1.0)
                            );
                        }
                        for col in 0..columns + 8 {
                            let scale: f64 =
                                (0..rows).map(|r| (explicit[r][col] * input[r]).abs()).sum();
                            assert!(
                                (adjoint[col] - expected_adjoint[col]).abs()
                                    <= 1e-12 * scale.max(1.0)
                            );
                        }
                        let prior = adjoint.clone();
                        operator.apply_adjoint(&input, &mut adjoint).unwrap();
                        assert_eq!(prior, adjoint);
                    });
                }
            }
        }
    }
}

#[test]
fn core_pair_bridge_matches_its_dense_symmetric_block_factor() {
    let first = vec![0, 0, 1, 1, 2, 2, 0, 1, 2, 0, 1, 2];
    let second = vec![0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 1, 0];
    let third = vec![0, 1, 2, 0, 1, 2, 2, 0, 1, 1, 2, 0];
    let design = FeDesign::new(
        first.len(),
        vec![
            EffectTerm::new(first, 3, true, Vec::new()).expect("first"),
            EffectTerm::new(second, 2, true, Vec::new()).expect("second"),
            EffectTerm::new(third, 3, true, Vec::new()).expect("third"),
        ],
    )
    .expect("design");
    let weights = vec![
        0.75, 1.25, 0.9, 1.4, 0.65, 1.1, 1.6, 0.8, 1.35, 0.95, 1.2, 0.7,
    ];
    let offsets = vec![0, 3, 5, 8];
    let core_effects = [0, 2];
    let core_width = 6;
    let effects = within_effects_selected(&design, &core_effects).expect("within effects");
    let solver = Solver::new(
        effects,
        Some(weights.clone()),
        within::PreconditionerConfig::default(),
    )
    .expect("core solver");
    let core = solver
        .preconditioner()
        .cloned()
        .expect("core preconditioner");
    assert_eq!(core.nrows(), core_width);

    let mut diagonal = vec![0.0; design.total_coefficients()];
    fill_fe_diagonal_inverse(&design, &weights, &mut diagonal).expect("diagonal");
    let scratch = Mutex::new(CorePairBridgeScratch::new(core_width).expect("scratch"));
    let preconditioner = BlockPreconditioner {
        finite_inverse: &[],
        finite_factor: None,
        fe: FePreconditioner::CorePair {
            core: &core,
            core_effects,
            diagonal: &diagonal,
            design: &design,
            effect_offsets: &offsets,
            weights: &weights,
            scratch: &scratch,
        },
    };
    let input = vec![0.4, -0.7, 0.2, 0.9, -0.3, 0.6, -0.8, 0.5];
    let mut actual = vec![0.0; input.len()];
    preconditioner
        .apply(&input, &mut actual)
        .expect("bridge apply");

    let mut dense_core = vec![vec![0.0; core_width]; core_width];
    for column in 0..core_width {
        let mut basis = vec![0.0; core_width];
        let mut image = vec![0.0; core_width];
        basis[column] = 1.0;
        core.apply(&basis, &mut image).expect("core basis apply");
        for row in 0..core_width {
            dense_core[row][column] = image[row];
        }
    }
    let extra_effects = [1];
    let extra_width = design.effects()[extra_effects[0]].n_levels();
    let mut dense_cross = vec![vec![0.0; extra_width]; core_width];
    for (row, weight) in weights.iter().copied().enumerate() {
        let mut packed_core_start = 0_usize;
        for core_effect in core_effects {
            let core_index =
                packed_core_start + design.effects()[core_effect].codes()[row] as usize;
            let extra_effect = extra_effects[0];
            let extra_index = design.effects()[extra_effect].codes()[row] as usize;
            dense_cross[core_index][extra_index] += weight;
            packed_core_start += design.effects()[core_effect].n_levels();
        }
    }
    let dense_apply = |matrix: &[Vec<f64>], vector: &[f64]| {
        matrix
            .iter()
            .map(|row| {
                row.iter()
                    .zip(vector)
                    .map(|(left, right)| left * right)
                    .sum()
            })
            .collect::<Vec<f64>>()
    };
    let mut core_input = Vec::with_capacity(core_width);
    for core_effect in core_effects {
        core_input.extend_from_slice(&input[offsets[core_effect]..offsets[core_effect + 1]]);
    }
    let extra_effect = extra_effects[0];
    let extra_input = &input[offsets[extra_effect]..offsets[extra_effect + 1]];
    let core_image = dense_apply(&dense_core, &core_input);
    let mut extra_image = vec![0.0; extra_width];
    for extra in 0..extra_width {
        let cross = (0..core_width)
            .map(|core_index| dense_cross[core_index][extra] * core_image[core_index])
            .sum::<f64>();
        extra_image[extra] = diagonal[offsets[extra_effect] + extra] * (extra_input[extra] - cross);
    }
    let cross_image = dense_apply(&dense_cross, &extra_image);
    let core_correction = dense_apply(&dense_core, &cross_image);
    let mut expected = vec![0.0; input.len()];
    let mut packed_core_start = 0_usize;
    for core_effect in core_effects {
        let width = design.effects()[core_effect].n_levels();
        for level in 0..width {
            expected[offsets[core_effect] + level] =
                core_image[packed_core_start + level] - core_correction[packed_core_start + level];
        }
        packed_core_start += width;
    }
    expected[offsets[extra_effect]..offsets[extra_effect + 1]].copy_from_slice(&extra_image);
    for (observed, reference) in actual.iter().zip(&expected) {
        assert!((observed - reference).abs() <= 2e-13);
    }

    let other = vec![-0.2, 0.3, 0.8, -0.4, 0.6, 0.1, -0.5, 0.7];
    let mut other_image = vec![0.0; other.len()];
    let mut adjoint_image = vec![0.0; input.len()];
    preconditioner
        .apply(&other, &mut other_image)
        .expect("second bridge apply");
    preconditioner
        .apply_adjoint(&input, &mut adjoint_image)
        .expect("bridge adjoint");
    let left = actual.iter().zip(&other).map(|(a, b)| a * b).sum::<f64>();
    let right = input
        .iter()
        .zip(&other_image)
        .map(|(a, b)| a * b)
        .sum::<f64>();
    assert!((left - right).abs() <= 2e-12);
    assert!(adjoint_image
        .iter()
        .zip(&actual)
        .all(|(adjoint, forward)| (adjoint - forward).abs() <= 2e-13));
    let quadratic = input.iter().zip(&actual).map(|(a, b)| a * b).sum::<f64>();
    assert!(quadratic >= -2e-12);
}

#[test]
fn full_finite_preconditioner_inverts_current_weight_gram_independently() {
    let active = DenseMatrix::from_row_major(
        5,
        3,
        vec![
            1., -2., 0.5, 0.25, 3., -1.5, -0.75, 1.25, 2., 4., -0.5, 0.125, 1.5, 2.5, -3.,
        ],
    )
    .unwrap();
    let mut factor = vec![0.; 9];
    let truth = [0.7, -1.2, 0.4];
    let mut prior = Vec::new();
    for weights in [vec![1.; 5], vec![0.01, 4., 2., 0.7, 9.]] {
        let mut gram = [0.; 9];
        for i in 0..3 {
            for j in 0..3 {
                gram[3 * i + j] = (0..5)
                    .map(|r| weights[r] * active[(r, i)] * active[(r, j)])
                    .sum();
            }
        }
        let diagonal: Vec<_> = (0..3).map(|i| 1. / gram[3 * i + i]).collect();
        assert!(prepare_finite_factor(
            &active,
            &weights,
            &diagonal,
            &mut factor
        ));
        assert_ne!(factor, prior);
        prior = factor.clone();
        let rhs: Vec<_> = (0..3)
            .map(|i| (0..3).map(|j| gram[3 * i + j] * truth[j]).sum())
            .collect();
        let preconditioner = BlockPreconditioner {
            finite_inverse: &diagonal,
            finite_factor: Some(&factor),
            fe: FePreconditioner::Diagonal(&[]),
        };
        let mut solved = vec![0.; 3];
        preconditioner.apply(&rhs, &mut solved).unwrap();
        for j in 0..3 {
            assert!((solved[j] - truth[j]).abs() < 2e-12);
        }
        let mut adjoint = vec![0.; 3];
        preconditioner.apply_adjoint(&rhs, &mut adjoint).unwrap();
        assert_eq!(solved, adjoint);
        assert!(rhs.iter().zip(&solved).map(|(x, y)| x * y).sum::<f64>() > 0.);
    }
}

#[test]
fn full_finite_preconditioner_declines_singular_or_nonfinite_frames() {
    let active = DenseMatrix::from_row_major(3, 2, vec![1., 2., -1., -2., 2., 4.]).unwrap();
    let diagonal = [1. / 6., 1. / 24.];
    let mut factor = vec![0.; 4];
    assert!(!prepare_finite_factor(
        &active,
        &[1.; 3],
        &diagonal,
        &mut factor
    ));
    assert!(!prepare_finite_factor(
        &active,
        &[f64::INFINITY; 3],
        &diagonal,
        &mut factor
    ));
    assert!(!prepare_finite_factor(
        &active,
        &[1.; 3],
        &[0., 1.],
        &mut factor
    ));
    assert!(!prepare_finite_factor(
        &active,
        &[1.; 3],
        &diagonal,
        &mut []
    ));
    let preconditioner = BlockPreconditioner {
        finite_inverse: &diagonal,
        finite_factor: None,
        fe: FePreconditioner::Diagonal(&[]),
    };
    let mut result = [0.; 2];
    preconditioner.apply(&[6., 24.], &mut result).unwrap();
    assert_eq!(result, [1., 1.]);
}

#[test]
fn frame_preparation_preserves_column_order_diagonals() {
    let active = DenseMatrix::from_row_major(
        5,
        3,
        vec![
            1.0, -2.0, 0.5, 0.25, 3.0, -1.5, -0.75, 1.25, 2.0, 4.0, -0.5, 0.125, 1.5, 2.5, -3.0,
        ],
    )
    .expect("active design");
    let outcome = [0.0, 2.0, 1.0, 4.0, 0.5];
    let mean = [1.25, 1.5, 0.75, 2.0, 0.625];
    let weights = [0.75, 1.25, 2.0, 0.5, 3.0];
    let mut sqrt_weights = vec![0.0; 5];
    let mut target = vec![0.0; 5];
    let mut rhs = vec![0.0; 5];
    let mut finite_inverse = vec![0.0; 3];

    let weight_change = prepare_frame_and_finite_diagonal(
        &active,
        &outcome,
        &mean,
        &weights,
        &mut sqrt_weights,
        &mut target,
        &mut rhs,
        &mut finite_inverse,
        false,
    )
    .expect("prepared frame");
    assert_eq!(weight_change, 0.0);

    let mut expected_diagonal = vec![0.0_f64; active.ncols()];
    for (column, diagonal) in expected_diagonal.iter_mut().enumerate() {
        for (row, &weight) in weights.iter().enumerate() {
            let value = active[(row, column)];
            *diagonal = (weight * value).mul_add(value, *diagonal);
        }
        *diagonal = 1.0 / *diagonal;
    }
    let expected_target = outcome
        .iter()
        .zip(&mean)
        .map(|(&value, &mu)| if value == 0.0 { -1.0 } else { value / mu - 1.0 })
        .collect::<Vec<_>>();
    assert_eq!(finite_inverse, expected_diagonal);
    assert_eq!(target, expected_target);
    for row in 0..weights.len() {
        assert_eq!(sqrt_weights[row], weights[row].sqrt());
        assert_eq!(rhs[row], sqrt_weights[row] * target[row]);
    }
}

#[test]
fn exact_certificate_is_thread_count_deterministic() {
    let rows = 2_048;
    let level_counts = [131, 137, 149];
    let effects = level_counts
        .iter()
        .enumerate()
        .map(|(effect, &levels)| {
            let codes = (0..rows)
                .map(|row| ((row * (37 + effect * 12) + 5 * effect) % levels) as u32)
                .collect::<Vec<_>>();
            EffectTerm::new(codes, levels, true, Vec::new()).expect("effect")
        })
        .collect::<Vec<_>>();
    let design = FeDesign::new(rows, effects).expect("design");
    let active_values = (0..rows)
        .flat_map(|row| {
            (0..4).map(move |column| ((row * (column + 3) + column * 11) % 97) as f64 / 53.0)
        })
        .collect::<Vec<_>>();
    let active = DenseMatrix::from_row_major(rows, 4, active_values).expect("active");
    let weights = (0..rows)
        .map(|row| 0.5 + (row % 29) as f64 / 17.0)
        .collect::<Vec<_>>();
    let target = (0..rows)
        .map(|row| (row % 41) as f64 / 19.0 - 0.75)
        .collect::<Vec<_>>();
    let residual = (0..rows)
        .map(|row| (row % 31) as f64 / 23.0 - 0.5)
        .collect::<Vec<_>>();

    let evaluate = |threads| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool");
        let mut finite_scratch = vec![DirectionCertificate::default(); active.ncols()];
        let mut fe_scratch = vec![DirectionCertificate::default(); design.total_coefficients()];
        let certificate = pool
            .install(|| {
                certify_joint_with_scratch(
                    &design,
                    &active,
                    &weights,
                    &target,
                    &residual,
                    1e-8,
                    &mut finite_scratch,
                    &mut fe_scratch,
                )
            })
            .expect("certificate");
        (certificate, finite_scratch, fe_scratch)
    };

    assert_eq!(evaluate(1), evaluate(4));
}

#[test]
fn exact_certificate_rejects_an_uncorrected_target() {
    let codes = vec![0, 0, 1, 1];
    let design = FeDesign::new(
        4,
        vec![EffectTerm::new(codes, 2, true, Vec::new()).expect("effect")],
    )
    .expect("design");
    let active = DenseMatrix::from_row_major(4, 0, Vec::new()).expect("active");
    let certificate = certify_joint(
        &design,
        &active,
        &[1.0; 4],
        &[1.0, 1.0, -1.0, -1.0],
        &[1.0, 1.0, -1.0, -1.0],
        1e-8,
    )
    .expect("certificate");
    assert!(!certificate.passed);
    assert!(certificate.relative_infinity_norm > 0.7);
}

#[test]
fn exact_certificate_does_not_turn_overflowing_squares_into_a_false_pass() {
    let design = FeDesign::new(
        2,
        vec![EffectTerm::new(vec![0, 0], 1, true, Vec::new()).expect("effect")],
    )
    .expect("design");
    let active = DenseMatrix::from_row_major(2, 0, Vec::new()).expect("active");
    let values = [1e200, 1e200];
    let certificate = certify_joint(&design, &active, &[1.0; 2], &values, &values, 1e-8)
        .expect("finite scaled certificate");
    assert!(!certificate.passed);
    assert!((certificate.relative_infinity_norm - 1.0).abs() <= 1e-15);
}

#[test]
fn exact_certificate_does_not_erase_a_finite_moment_through_split_underflow() {
    let design = FeDesign::new(
        2,
        vec![EffectTerm::new(vec![0, 0], 1, true, Vec::new()).expect("effect")],
    )
    .expect("design");
    let active = DenseMatrix::from_row_major(2, 1, vec![1e300, 1e300]).expect("active");
    let values = [1e-200, 1e-200];
    let error = certify_joint(&design, &active, &[1e-300; 2], &values, &values, 1e-8)
        .expect_err("unrepresentable reference norm must fail closed");
    assert_eq!(error.code, ErrorCode::NonConvergence);
}

#[test]
fn weighted_moment_reorders_a_nonzero_subnormal_split_factor() {
    let weight = 1e-300_f64;
    let residual = 6e-174_f64;
    let basis = 1e308_f64;
    let root_weight = weight.sqrt();
    let split = (root_weight * residual) * (root_weight * basis);
    let expected = finite_product3(weight, residual, basis).expect("finite reordered product");
    assert!(root_weight.mul_add(residual, 0.0).is_subnormal());
    assert!((split - expected).abs() / expected.abs() > 0.1);

    let (moment, weighted_basis) = finite_weighted_moment(weight, residual, basis, root_weight)
        .expect("robust weighted moment");
    assert_eq!(moment, expected);
    assert_eq!(weighted_basis, root_weight * basis);
}

#[test]
fn topology_identity_rejects_an_equal_shaped_replacement() {
    let first = FeDesign::new(
        4,
        vec![EffectTerm::new(vec![0, 0, 1, 1], 2, true, Vec::new()).expect("first")],
    )
    .expect("first design");
    let replacement = FeDesign::new(
        4,
        vec![EffectTerm::new(vec![0, 1, 0, 1], 2, true, Vec::new()).expect("replacement")],
    )
    .expect("replacement design");
    let identity = FeTopologyIdentity::capture(&first);
    assert!(identity.matches(&first));
    assert!(!identity.matches(&replacement));

    let mut solver = ReusableCategoricalJointLsmr {
        team: test_team(1),
        active_design: DenseMatrix::from_row_major(4, 0, Vec::new()).expect("active"),
        active_slots: Vec::new(),
        topology: identity,
        effect_offsets: vec![0, 2],
        core_effects: None,
        sqrt_weights: vec![0.0; 4],
        rhs: vec![0.0; 4],
        target: vec![0.0; 4],
        delta_eta: vec![0.0; 4],
        residual: vec![0.0; 4],
        warm_start: vec![0.0; 2],
        finite_inverse: Vec::new(),
        finite_factor: Vec::new(),
        finite_factor_ready: false,
        fe_inverse: vec![0.0; 2],
        finite_certificate: Vec::new(),
        fe_certificate: vec![DirectionCertificate::default(); 2],
        warm_start_valid: false,
        first_pass_predictor: JointLsmrFirstPassPredictor::default(),
        preconditioner: None,
        core_pair_bridge_scratch: Mutex::new(CorePairBridgeScratch::default()),
        policy: CategoricalLsmrPolicy::Diagonal,
        instrumentation: JointLsmrInstrumentation::default(),
    };
    solver
        .first_pass_predictor
        .record_certified(predictor_frame(
            1e-8,
            1e-8,
            5e-9,
            10,
            JointLsmrContinuationCause::default(),
        ));
    let predictor_before_failure = solver.first_pass_predictor;
    let error = solver
        .solve(&replacement, &[1.0; 4], &[1.0; 4], &[1.0; 4], 1e-8, 100)
        .expect_err("replacement topology must fail before operator use");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(solver.first_pass_predictor, predictor_before_failure);
}

#[test]
fn topology_identity_rejects_a_different_effect_partition_with_the_same_total_levels() {
    let first = FeDesign::new(
        4,
        vec![
            EffectTerm::new(vec![0, 0, 0, 0], 1, true, Vec::new()).expect("first effect"),
            EffectTerm::new(vec![0, 1, 2, 0], 3, true, Vec::new()).expect("second effect"),
        ],
    )
    .expect("first design");
    let replacement = FeDesign::new(
        4,
        vec![
            EffectTerm::new(vec![0, 1, 0, 1], 2, true, Vec::new()).expect("first effect"),
            EffectTerm::new(vec![0, 0, 1, 1], 2, true, Vec::new()).expect("second effect"),
        ],
    )
    .expect("replacement design");
    assert_eq!(first.total_coefficients(), replacement.total_coefficients());
    let identity = FeTopologyIdentity::capture(&first);
    assert!(!identity.matches(&replacement));
}

#[test]
fn joint_solver_reuses_a_certified_incremental_state_across_weight_frames() {
    for eager_factor in [false, true] {
        let first_codes = vec![0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
        let second_codes = vec![0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0];
        let design = FeDesign::new(
            12,
            vec![
                EffectTerm::new(first_codes.clone(), 4, true, Vec::new()).expect("first"),
                EffectTerm::new(second_codes.clone(), 2, true, Vec::new()).expect("second"),
            ],
        )
        .expect("design");
        let x = vec![0.0, 1.0, 1.5, 0.5, 2.0, 3.0, 2.5, 4.0, 1.2, 2.2, 3.2, 4.2];
        let active_design = DenseMatrix::from_row_major(12, 1, x.clone()).expect("active design");
        let first_effect = [0.12, -0.08, 0.05, -0.03];
        let second_effect = [0.07, -0.06];
        let target = x
            .iter()
            .enumerate()
            .map(|(row, value)| {
                0.08 * value
                    + first_effect[first_codes[row] as usize]
                    + second_effect[second_codes[row] as usize]
            })
            .collect::<Vec<_>>();
        let outcome = target.iter().map(|value| 1.0 + value).collect::<Vec<_>>();
        let mut solver = ReusableCategoricalJointLsmr {
            team: test_team(2),
            active_design,
            active_slots: vec![0],
            topology: FeTopologyIdentity::capture(&design),
            effect_offsets: vec![0, 4, 6],
            core_effects: None,
            sqrt_weights: vec![0.0; 12],
            rhs: vec![0.0; 12],
            target: vec![0.0; 12],
            delta_eta: vec![0.0; 12],
            residual: vec![0.0; 12],
            warm_start: vec![0.0; 7],
            finite_inverse: vec![0.0; 1],
            finite_factor: Vec::new(),
            finite_factor_ready: false,
            // A Schwarz frame must never inspect or overwrite the unused FE
            // diagonal. NaNs make an accidental read fail the solve, while an
            // accidental refresh is visible below.
            fe_inverse: vec![f64::NAN; 6],
            finite_certificate: vec![DirectionCertificate::default(); 1],
            fe_certificate: vec![DirectionCertificate::default(); 6],
            warm_start_valid: false,
            first_pass_predictor: JointLsmrFirstPassPredictor::default(),
            preconditioner: None,
            core_pair_bridge_scratch: Mutex::new(CorePairBridgeScratch::default()),
            policy: CategoricalLsmrPolicy::Reuse,
            instrumentation: JointLsmrInstrumentation::default(),
        };
        if eager_factor {
            assert!(solver
                .rebuild_schwarz(&design, &[1.0; 12])
                .expect("initial factor"));
        }
        let unrelated_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("unrelated pool");
        let first = unrelated_pool
            .install(|| solver.solve(&design, &outcome, &[1.0; 12], &[1.0; 12], 1e-10, 1_000))
            .expect("first correction");
        assert_eq!(first.executing_threads, 2);
        assert_eq!(first.retries, 0);
        assert!(first.certificate.passed);
        assert!(first.certification_seconds > 0.0);
        assert!(first.requested_schwarz);
        assert_eq!(first.used_schwarz, eager_factor);
        assert!(!first.rebuilt_preconditioner);
        assert_eq!(
            solver.instrumentation.schwarz_rebuilds,
            usize::from(eager_factor)
        );
        assert_eq!(solver.instrumentation.escalation_armed, 1);
        let bridge_scratch = solver
            .core_pair_bridge_scratch
            .lock()
            .expect("bridge scratch");
        assert!(bridge_scratch.core_rhs.is_empty());
        assert!(bridge_scratch.core_correction.is_empty());
        drop(bridge_scratch);
        let expected_finite_inverse = 1.0 / x.iter().map(|value| value * value).sum::<f64>();
        assert!((solver.finite_inverse[0] - expected_finite_inverse).abs() <= 1e-15);
        if eager_factor {
            assert!(solver.fe_inverse.iter().all(|value| value.is_nan()));
        } else {
            assert!(solver
                .fe_inverse
                .iter()
                .all(|value| value.is_finite() && *value > 0.0));
            assert!(solver.preconditioner.is_none());
        }
        for (actual, expected) in solver.delta_eta.iter().zip(&target) {
            assert!((actual - expected).abs() <= 1e-9);
        }

        solver.accept_step(0.5);
        let mean = target
            .iter()
            .map(|value| 1.0 + 0.5 * value)
            .collect::<Vec<_>>();
        let weights = mean.clone();
        let second = solver
            .solve(&design, &outcome, &mean, &weights, 1e-10, 1_000)
            .expect("second correction");
        assert_eq!(second.retries, 0);
        assert!(second.certificate.passed);
        assert!(second.certification_seconds > 0.0);
        assert_eq!(second.used_schwarz, eager_factor);
        assert!(!second.rebuilt_preconditioner);
        assert!(!second.frame.predictor_used);
        assert!(!second.frame.predictor_refreshed_preconditioner);
        assert_eq!(
            solver.instrumentation.schwarz_rebuilds,
            usize::from(eager_factor)
        );
        assert_eq!(solver.instrumentation.escalation_armed, 2);
        let expected_finite_inverse = 1.0
            / x.iter()
                .zip(&weights)
                .map(|(value, weight)| weight * value * value)
                .sum::<f64>();
        assert!((solver.finite_inverse[0] - expected_finite_inverse).abs() <= 1e-15);
        if eager_factor {
            assert!(solver.fe_inverse.iter().all(|value| value.is_nan()));
        } else {
            assert!(solver
                .fe_inverse
                .iter()
                .all(|value| value.is_finite() && *value > 0.0));
            assert!(solver.preconditioner.is_none());
        }
        assert!(solver.warm_start_valid);
        assert!(solver.delta_eta.iter().all(|value| value.is_finite()));

        solver.reset_warm_start();
        assert!(!solver.warm_start_valid);
        assert_eq!(
            solver.first_pass_predictor,
            JointLsmrFirstPassPredictor::default()
        );
    }
}

#[test]
fn joint_solver_tracks_retained_team_across_one_four_one() {
    let first_codes = vec![0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
    let second_codes = vec![0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0];
    let design = FeDesign::new(
        12,
        vec![
            EffectTerm::new(first_codes.clone(), 4, true, Vec::new()).expect("first"),
            EffectTerm::new(second_codes.clone(), 2, true, Vec::new()).expect("second"),
        ],
    )
    .expect("design");
    let active_values = vec![0.0, 1.0, 1.5, 0.5, 2.0, 3.0, 2.5, 4.0, 1.2, 2.2, 3.2, 4.2];
    let first_effect = [0.12, -0.08, 0.05, -0.03];
    let second_effect = [0.07, -0.06];
    let target = active_values
        .iter()
        .enumerate()
        .map(|(row, value)| {
            0.08 * value
                + first_effect[first_codes[row] as usize]
                + second_effect[second_codes[row] as usize]
        })
        .collect::<Vec<_>>();
    let outcome = target.iter().map(|value| 1.0 + value).collect::<Vec<_>>();
    let ambient = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .expect("ambient pool");

    for requested in [1, 4, 1] {
        let mut solver = ReusableCategoricalJointLsmr {
            team: test_team(requested),
            active_design: DenseMatrix::from_row_major(12, 1, active_values.clone())
                .expect("active design"),
            active_slots: vec![0],
            topology: FeTopologyIdentity::capture(&design),
            effect_offsets: vec![0, 4, 6],
            core_effects: None,
            sqrt_weights: vec![0.0; 12],
            rhs: vec![0.0; 12],
            target: vec![0.0; 12],
            delta_eta: vec![0.0; 12],
            residual: vec![0.0; 12],
            warm_start: vec![0.0; 7],
            finite_inverse: vec![0.0; 1],
            finite_factor: Vec::new(),
            finite_factor_ready: false,
            fe_inverse: vec![0.0; 6],
            finite_certificate: vec![DirectionCertificate::default(); 1],
            fe_certificate: vec![DirectionCertificate::default(); 6],
            warm_start_valid: false,
            first_pass_predictor: JointLsmrFirstPassPredictor::default(),
            preconditioner: None,
            core_pair_bridge_scratch: Mutex::new(CorePairBridgeScratch::default()),
            policy: CategoricalLsmrPolicy::Diagonal,
            instrumentation: JointLsmrInstrumentation::default(),
        };
        let report = ambient
            .install(|| solver.solve(&design, &outcome, &[1.0; 12], &[1.0; 12], 1e-10, 1_000))
            .expect("joint correction");
        assert_eq!(report.executing_threads, requested);
        assert!(report.certificate.passed);
    }
}

#[test]
fn joint_solver_supports_zero_finite_columns_with_three_effects() {
    let rows = 60;
    let first_codes = (0..rows).map(|row| (row % 5) as u32).collect::<Vec<_>>();
    let second_codes = (0..rows)
        .map(|row| ((row / 2) % 4) as u32)
        .collect::<Vec<_>>();
    let third_codes = (0..rows)
        .map(|row| ((row / 3) % 3) as u32)
        .collect::<Vec<_>>();
    let design = FeDesign::new(
        rows,
        vec![
            EffectTerm::new(first_codes.clone(), 5, true, Vec::new()).expect("first"),
            EffectTerm::new(second_codes.clone(), 4, true, Vec::new()).expect("second"),
            EffectTerm::new(third_codes.clone(), 3, true, Vec::new()).expect("third"),
        ],
    )
    .expect("design");
    let first_effect = [-0.04, -0.02, 0.0, 0.02, 0.04];
    let second_effect = [-0.03, -0.01, 0.01, 0.03];
    let third_effect = [-0.02, 0.0, 0.02];
    let target = (0..rows)
        .map(|row| {
            first_effect[first_codes[row] as usize]
                + second_effect[second_codes[row] as usize]
                + third_effect[third_codes[row] as usize]
        })
        .collect::<Vec<_>>();
    let outcome = target.iter().map(|value| 1.0 + value).collect::<Vec<_>>();
    let mut solver = ReusableCategoricalJointLsmr {
        team: test_team(4),
        active_design: DenseMatrix::from_row_major(rows, 0, Vec::new())
            .expect("empty active design"),
        active_slots: Vec::new(),
        topology: FeTopologyIdentity::capture(&design),
        effect_offsets: vec![0, 5, 9, 12],
        core_effects: Some([0, 1]),
        sqrt_weights: vec![0.0; rows],
        rhs: vec![0.0; rows],
        target: vec![0.0; rows],
        delta_eta: vec![0.0; rows],
        residual: vec![0.0; rows],
        warm_start: vec![0.0; 12],
        finite_inverse: Vec::new(),
        finite_factor: Vec::new(),
        finite_factor_ready: false,
        fe_inverse: vec![0.0; 12],
        finite_certificate: Vec::new(),
        fe_certificate: vec![DirectionCertificate::default(); 12],
        warm_start_valid: false,
        first_pass_predictor: JointLsmrFirstPassPredictor::default(),
        preconditioner: None,
        core_pair_bridge_scratch: Mutex::new(
            CorePairBridgeScratch::new(9).expect("bridge scratch"),
        ),
        policy: CategoricalLsmrPolicy::CorePair,
        instrumentation: JointLsmrInstrumentation::default(),
    };

    let report = solver
        .solve(&design, &outcome, &[1.0; 60], &[1.0; 60], 1e-10, 1_000)
        .expect("FE-only correction");
    assert!(report.certificate.passed);
    assert!(report.requested_schwarz);
    assert!(report.used_schwarz);
    assert_eq!(solver.active_rank(), 0);
    assert!(solver.finite_inverse.is_empty());
    for (actual, expected) in solver.delta_eta.iter().zip(&target) {
        assert!((actual - expected).abs() <= 1e-9);
    }
}

#[test]
fn joint_memory_estimate_scales_with_retained_design_and_thread_team() {
    let base = estimate_joint_lsmr_workspace_bytes(
        10_000,
        2,
        &[2_000, 1_000],
        1,
        CategoricalLsmrPolicy::Reuse,
        None,
    )
    .expect("base estimate");
    let wider = estimate_joint_lsmr_workspace_bytes(
        10_000,
        8,
        &[2_000, 1_000],
        1,
        CategoricalLsmrPolicy::Reuse,
        None,
    )
    .expect("wide estimate");
    let parallel = estimate_joint_lsmr_workspace_bytes(
        10_000,
        8,
        &[2_000, 1_000],
        8,
        CategoricalLsmrPolicy::Reuse,
        None,
    )
    .expect("parallel estimate");
    let more_levels = estimate_joint_lsmr_workspace_bytes(
        10_000,
        8,
        &[2_001, 1_001],
        8,
        CategoricalLsmrPolicy::Reuse,
        None,
    )
    .expect("larger topology estimate");
    assert!(wider > base);
    assert!(parallel > wider);
    assert!(more_levels > parallel);
}

#[test]
fn checkpoint_memory_charges_batch_outputs_and_concurrent_recurrences() {
    let one_thread =
        estimate_checkpoint_lsmr_workspace_bytes(50_000, 11, &[8_000, 7_000, 6_000], 1)
            .expect("one-thread checkpoint estimate");
    let seven_threads =
        estimate_checkpoint_lsmr_workspace_bytes(50_000, 11, &[8_000, 7_000, 6_000], 7)
            .expect("seven-thread checkpoint estimate");
    let wider = estimate_checkpoint_lsmr_workspace_bytes(50_000, 17, &[8_000, 7_000, 6_000], 7)
        .expect("wide checkpoint estimate");
    assert!(seven_threads > one_thread);
    assert!(wider > seven_threads);
    let three_dense_batches = 3_u128 * 50_000 * 11 * std::mem::size_of::<f64>() as u128;
    assert!(one_thread > three_dense_batches);
}

#[test]
fn schwarz_memory_charges_dense_cross_tabs_at_both_dispatch_boundaries() {
    assert_eq!(
        categorical_dense_cross_tab_bytes(8_000, &[2_500, 2_000]).unwrap(),
        40_000_000
    );
    assert_eq!(
        categorical_dense_cross_tab_bytes(8_000, &[2_501, 2_000]).unwrap(),
        0
    );
    // Above the cap, dense is still used when no larger than sparse scratch.
    assert_eq!(
        categorical_dense_cross_tab_bytes(4_000_000, &[3_000, 2_000]).unwrap(),
        48_000_000
    );
    assert_eq!(
        categorical_dense_cross_tab_bytes(3_999_999, &[3_000, 2_000]).unwrap(),
        0
    );
    assert!(categorical_dense_cross_tab_bytes(u128::MAX, &[2, 3]).is_err());
}

#[test]
fn sparse_checkpoint_memory_includes_concurrent_dense_nuisance_pairs() {
    let levels = [2_666, 2_000, 1_600];
    let dense = (2_666_u128 * 1_600 + 2_000 * 1_600) * 8;
    assert_eq!(
        categorical_dense_cross_tab_bytes(8_000, &levels).unwrap(),
        dense
    );
    let checkpoint =
        CategoricalLsmrMemoryShape::new(8_000, 0, &levels, 7, CategoricalLsmrPolicy::Reuse, None)
            .unwrap();
    let ordinary = CategoricalLsmrMemoryShape::new(
        8_000,
        10,
        &levels,
        7,
        CategoricalLsmrPolicy::CorePair,
        None,
    )
    .unwrap();
    assert!(checkpoint.schwarz.build_peak_bytes > dense);
    assert!(ordinary.schwarz.build_peak_bytes < dense);
    assert!(estimate_checkpoint_lsmr_workspace_bytes(8_000, 11, &levels, 7).unwrap() > dense);
    let permuted = CategoricalLsmrMemoryShape::new(
        8_000,
        0,
        &[1_600, 2_666, 2_000],
        7,
        CategoricalLsmrPolicy::Reuse,
        None,
    )
    .unwrap();
    assert_eq!(checkpoint.schwarz, permuted.schwarz);
}

#[test]
fn joint_policy_keeps_two_effect_schwarz_and_uses_one_core_pair_for_multiway() {
    assert_eq!(joint_lsmr_policy(1), CategoricalLsmrPolicy::Reuse);
    assert_eq!(joint_lsmr_policy(2), CategoricalLsmrPolicy::Reuse);
    assert_eq!(joint_lsmr_policy(3), CategoricalLsmrPolicy::CorePair);
    assert_eq!(joint_lsmr_policy(4), CategoricalLsmrPolicy::CorePair);
}

#[test]
fn joint_core_pair_uses_the_same_two_largest_effects_under_every_permutation() {
    let rows = 120;
    let sources = [
        (
            (0..rows).map(|row| (row % 20) as u32).collect::<Vec<_>>(),
            20,
        ),
        ((0..rows).map(|row| (row % 9) as u32).collect::<Vec<_>>(), 9),
        ((0..rows).map(|row| (row % 4) as u32).collect::<Vec<_>>(), 4),
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
            .map(|&effect| {
                EffectTerm::new(
                    sources[effect].0.clone(),
                    sources[effect].1,
                    true,
                    Vec::new(),
                )
                .expect("effect")
            })
            .collect();
        let design = FeDesign::new(rows, effects).expect("permuted design");
        let selected = largest_retained_core_pair(&design).expect("core pair");
        assert_eq!([order[selected[0]], order[selected[1]]], [0, 1]);
    }
}

#[test]
fn joint_memory_charges_only_the_selected_schwarz_topology() {
    let levels = [500, 2_000, 1_000];
    let diagonal_one = estimate_joint_lsmr_workspace_bytes(
        10_000,
        8,
        &levels,
        1,
        CategoricalLsmrPolicy::Diagonal,
        None,
    )
    .expect("serial diagonal estimate");
    let diagonal_eight = estimate_joint_lsmr_workspace_bytes(
        10_000,
        8,
        &levels,
        8,
        CategoricalLsmrPolicy::Diagonal,
        None,
    )
    .expect("parallel diagonal estimate");
    let schwarz = estimate_joint_lsmr_workspace_bytes(
        10_000,
        8,
        &levels,
        8,
        CategoricalLsmrPolicy::Reuse,
        None,
    )
    .expect("Schwarz estimate");
    let core_pair = estimate_joint_lsmr_workspace_bytes(
        10_000,
        8,
        &levels,
        8,
        CategoricalLsmrPolicy::CorePair,
        None,
    )
    .expect("core-pair estimate");
    assert_eq!(diagonal_one, diagonal_eight);
    assert!(diagonal_eight < core_pair);
    assert!(core_pair < schwarz);
    let permuted = estimate_joint_lsmr_workspace_bytes(
        10_000,
        8,
        &[1_000, 500, 2_000],
        8,
        CategoricalLsmrPolicy::CorePair,
        None,
    )
    .expect("permuted core-pair estimate");
    assert_eq!(core_pair, permuted);
}

#[test]
fn core_pair_memory_charges_exactly_two_core_width_bridge_buffers() {
    let levels = [500, 2_000, 1_000];
    let expected = 2_u128 * (levels[1] + levels[2]) as u128 * std::mem::size_of::<f64>() as u128;
    assert_eq!(
        core_pair_bridge_scratch_bytes(&levels, CategoricalLsmrPolicy::CorePair)
            .expect("core-pair bridge bytes"),
        expected
    );
    assert_eq!(
        core_pair_bridge_scratch_bytes(&[1_000, 500, 2_000], CategoricalLsmrPolicy::CorePair)
            .expect("permuted core-pair bridge bytes"),
        expected
    );
    assert_eq!(
        core_pair_bridge_scratch_bytes(&levels, CategoricalLsmrPolicy::Reuse)
            .expect("reuse bridge bytes"),
        0
    );
    assert_eq!(
        core_pair_bridge_scratch_bytes(&levels, CategoricalLsmrPolicy::Diagonal)
            .expect("diagonal bridge bytes"),
        0
    );
}

#[test]
fn joint_memory_estimate_rejects_invalid_and_overflowing_shapes() {
    let invalid = estimate_joint_lsmr_workspace_bytes(
        100,
        2,
        &[20, 0],
        4,
        CategoricalLsmrPolicy::Diagonal,
        None,
    )
    .expect_err("zero levels must fail");
    assert_eq!(invalid.code, ErrorCode::InvalidArgument);

    let overflow = estimate_joint_lsmr_workspace_bytes(
        usize::MAX,
        usize::MAX,
        &[usize::MAX, usize::MAX],
        usize::MAX,
        CategoricalLsmrPolicy::Reuse,
        None,
    )
    .expect_err("adversarial dimensions must fail closed");
    assert_eq!(overflow.code, ErrorCode::Allocation);
}
fn test_team(threads: usize) -> InstalledThreadTeam {
    ThreadTeam::new(ThreadConfig::from_requested(threads, true).expect("thread config"))
        .acquire()
        .expect("installed thread team")
}
