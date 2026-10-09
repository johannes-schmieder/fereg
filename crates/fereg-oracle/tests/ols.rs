#![deny(clippy::too_many_lines)]
// Self-tests of the independent explicit-dummy OLS oracle.
use fereg_oracle::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleError, OracleMatrix, OracleWeights,
};

const TIGHT: f64 = 2e-10;

#[test]
fn hand_computed_ols_returns_coefficients_bread_iid_and_hc1() {
    let problem = ExplicitOlsProblem {
        outcome: vec![2.0, 0.0, 4.0],
        regressors: vec![vec![1.0; 3], vec![-1.0, 0.0, 1.0]],
        effects: vec![],
        weights: OracleWeights::Unit,
        clusters: vec![],
    };

    let result = fit_explicit_ols(&problem).expect("hand-computed OLS");

    assert_eq!(result.explicit_dummy_rank, 0);
    assert_eq!(result.finite_regressor_rank, 2);
    assert_eq!(result.model_rank, 2);
    assert_eq!(result.residual_degrees_of_freedom, 1);
    assert_close(result.coefficients[0].expect("intercept"), 2.0, TIGHT);
    assert_close(result.coefficients[1].expect("slope"), 1.0, TIGHT);
    assert_slice_close(&result.fitted_values, &[1.0, 2.0, 3.0], TIGHT);
    assert_slice_close(&result.residuals, &[1.0, -2.0, 1.0], TIGHT);
    assert_matrix_close(&result.bread, &[[1.0 / 3.0, 0.0], [0.0, 0.5]], TIGHT);
    assert_matrix_close(&result.iid_vce, &[[2.0, 0.0], [0.0, 3.0]], TIGHT);
    assert_matrix_close(&result.hc1_vce, &[[2.0, 0.0], [0.0, 1.5]], TIGHT);
    assert!(result.certificate.normalized_normal_equation_infinity < 1e-14);
    assert!(result.certificate.bread_inverse_infinity < 1e-14);
}

#[test]
fn hand_computed_one_and_two_way_cluster_vces_use_cr1_inclusion_exclusion() {
    let problem = ExplicitOlsProblem {
        outcome: vec![1.0, 3.0, 2.0, 6.0],
        regressors: vec![vec![1.0; 4]],
        effects: vec![],
        weights: OracleWeights::Unit,
        clusters: vec![vec![0, 0, 1, 1], vec![0, 1, 0, 1]],
    };

    let result = fit_explicit_ols(&problem).expect("hand-computed clustered OLS");

    assert_close(result.coefficients[0].expect("mean"), 3.0, TIGHT);
    assert_matrix_close(&result.bread, &[[0.25]], TIGHT);
    assert_matrix_close(&result.iid_vce, &[[7.0 / 6.0]], TIGHT);
    assert_matrix_close(&result.hc1_vce, &[[7.0 / 6.0]], TIGHT);
    assert_eq!(result.cluster_counts, vec![2, 2]);
    assert_matrix_close(&result.one_way_cluster_vces[0], &[[1.0]], TIGHT);
    assert_matrix_close(&result.one_way_cluster_vces[1], &[[2.25]], TIGHT);
    assert_matrix_close(
        result.clustered_vce.as_ref().expect("two-way VCE"),
        &[[1.5]],
        TIGHT,
    );
}

#[test]
fn explicit_slope_dummies_recover_hand_constructed_fwl_coefficient() {
    let time = vec![-2.0, -1.0, 0.0, 1.0, 2.0, -2.0, -1.0, 0.0, 1.0, 2.0];
    let codes = vec![0, 0, 0, 0, 0, 1, 1, 1, 1, 1];
    let regressor = time.iter().map(|value| value * value).collect::<Vec<_>>();
    let orthogonal_noise = [1.0, -4.0, 6.0, -4.0, 1.0];
    let outcome = (0..time.len())
        .map(|row| {
            let group = codes[row] as usize;
            let absorbed = [1.5, -2.0][group] + [0.75, -1.25][group] * time[row];
            let noise_scale = [0.1, -0.2][group];
            absorbed + 2.5 * regressor[row] + noise_scale * orthogonal_noise[row % 5]
        })
        .collect::<Vec<_>>();
    let effect = DummyEffect::new(codes, 2, true, vec![time]).expect("slope effect");
    let problem = ExplicitOlsProblem {
        outcome,
        regressors: vec![regressor],
        effects: vec![effect],
        weights: OracleWeights::Unit,
        clusters: vec![],
    };

    let result = fit_explicit_ols(&problem).expect("explicit slope fit");

    assert_eq!(result.explicit_dummy_columns, 4);
    assert_eq!(result.explicit_dummy_rank, 4);
    assert_eq!(result.finite_regressor_rank, 1);
    assert_eq!(result.model_rank, 5);
    assert_close(result.coefficients[0].expect("quadratic"), 2.5, 2e-12);
    assert!(result.residuals.iter().any(|value| value.abs() > 0.1));
}

#[test]
fn disconnected_two_way_effects_have_exact_explicit_design_rank() {
    let first_codes = vec![0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2];
    let second_codes = vec![0, 0, 1, 1, 0, 0, 1, 1, 2, 2, 2, 2];
    let regressor = vec![
        -1.0, 1.0, -2.0, 2.0, -1.5, 1.5, -0.5, 0.5, -3.0, 3.0, -4.0, 4.0,
    ];
    let outcome = (0..regressor.len())
        .map(|row| {
            [1.0, -2.0, 4.0][first_codes[row] as usize]
                + [0.5, 3.0, -1.0][second_codes[row] as usize]
                + 2.0 * regressor[row]
        })
        .collect::<Vec<_>>();
    let problem = ExplicitOlsProblem {
        outcome: outcome.clone(),
        regressors: vec![regressor],
        effects: vec![
            DummyEffect::new(first_codes, 3, true, vec![]).expect("first effect"),
            DummyEffect::new(second_codes, 3, true, vec![]).expect("second effect"),
        ],
        weights: OracleWeights::Unit,
        clusters: vec![],
    };

    let result = fit_explicit_ols(&problem).expect("disconnected fit");

    assert_eq!(result.explicit_dummy_columns, 6);
    assert_eq!(result.explicit_dummy_rank, 4);
    assert_eq!(result.model_rank, 5);
    assert_close(result.coefficients[0].expect("slope"), 2.0, 2e-12);
    assert_slice_close(&result.fitted_values, &outcome, 5e-12);
    assert!(result.residuals.iter().all(|value| value.abs() < 5e-12));
}

#[test]
fn frequency_weights_match_literal_row_expansion_for_all_reported_vces() {
    let frequencies = vec![1, 2, 3, 1, 2, 1, 3, 2];
    let codes = vec![0, 0, 0, 0, 1, 1, 1, 1];
    let time = vec![-2.0, -1.0, 1.0, 2.0, -2.0, -1.0, 1.0, 2.0];
    let regressor = vec![4.0, 1.0, 1.0, 4.0, 4.5, 0.5, 1.5, 3.5];
    let outcome = vec![4.2, 2.1, 4.0, 10.3, 0.5, -1.1, 4.3, 9.8];
    let first_cluster = vec![0, 0, 1, 1, 2, 2, 3, 3];
    let second_cluster = vec![0, 1, 0, 1, 0, 1, 0, 1];
    let weighted = ExplicitOlsProblem {
        outcome: outcome.clone(),
        regressors: vec![regressor.clone()],
        effects: vec![
            DummyEffect::new(codes.clone(), 2, true, vec![time.clone()]).expect("weighted effect")
        ],
        weights: OracleWeights::Frequency(frequencies.clone()),
        clusters: vec![first_cluster.clone(), second_cluster.clone()],
    };
    let expanded = ExplicitOlsProblem {
        outcome: expand(&outcome, &frequencies),
        regressors: vec![expand(&regressor, &frequencies)],
        effects: vec![DummyEffect::new(
            expand(&codes, &frequencies),
            2,
            true,
            vec![expand(&time, &frequencies)],
        )
        .expect("expanded effect")],
        weights: OracleWeights::Unit,
        clusters: vec![
            expand(&first_cluster, &frequencies),
            expand(&second_cluster, &frequencies),
        ],
    };

    let weighted_result = fit_explicit_ols(&weighted).expect("frequency-weight fit");
    let expanded_result = fit_explicit_ols(&expanded).expect("expanded fit");

    assert_eq!(
        weighted_result.effective_observations,
        expanded.outcome.len()
    );
    assert_eq!(weighted_result.model_rank, expanded_result.model_rank);
    assert_option_slice_close(
        &weighted_result.coefficients,
        &expanded_result.coefficients,
        5e-11,
    );
    assert_oracle_matrix_close(&weighted_result.bread, &expanded_result.bread, 5e-11);
    assert_oracle_matrix_close(&weighted_result.iid_vce, &expanded_result.iid_vce, 5e-10);
    assert_oracle_matrix_close(&weighted_result.hc1_vce, &expanded_result.hc1_vce, 5e-10);
    for (weighted_vce, expanded_vce) in weighted_result
        .one_way_cluster_vces
        .iter()
        .zip(&expanded_result.one_way_cluster_vces)
    {
        assert_oracle_matrix_close(weighted_vce, expanded_vce, 5e-10);
    }
    assert_oracle_matrix_close(
        weighted_result
            .clustered_vce
            .as_ref()
            .expect("weighted VCE"),
        expanded_result
            .clustered_vce
            .as_ref()
            .expect("expanded VCE"),
        5e-10,
    );
    let mut expanded_row = 0;
    for (row, &frequency) in frequencies.iter().enumerate() {
        for _ in 0..frequency {
            assert_close(
                weighted_result.fitted_values[row],
                expanded_result.fitted_values[expanded_row],
                5e-11,
            );
            assert_close(
                weighted_result.residuals[row],
                expanded_result.residuals[expanded_row],
                5e-11,
            );
            expanded_row += 1;
        }
    }
}

#[test]
fn regressor_slope_and_wls_rescaling_preserve_identified_results() {
    let time = vec![-2.0, -1.0, 0.0, 1.0, 2.0, -2.0, -1.0, 0.0, 1.0, 2.0];
    let codes = vec![0, 0, 0, 0, 0, 1, 1, 1, 1, 1];
    let regressor = vec![4.2, 0.7, 0.3, 1.1, 4.0, 3.7, 1.4, -0.2, 1.3, 4.4];
    let outcome = vec![9.0, 3.2, 1.7, 4.1, 10.2, 4.0, 0.2, -1.1, 2.0, 7.7];
    let weights = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 0.7, 1.1, 4.0];
    let baseline = ExplicitOlsProblem {
        outcome: outcome.clone(),
        regressors: vec![regressor.clone()],
        effects: vec![
            DummyEffect::new(codes.clone(), 2, true, vec![time.clone()]).expect("baseline effect")
        ],
        weights: OracleWeights::Wls(weights.clone()),
        clusters: vec![],
    };
    let regressor_scale = -7.0;
    let transformed = ExplicitOlsProblem {
        outcome,
        regressors: vec![regressor
            .iter()
            .map(|value| regressor_scale * value)
            .collect()],
        effects: vec![DummyEffect::new(
            codes,
            2,
            true,
            vec![time.iter().map(|value| 3.0 + 11.0 * value).collect()],
        )
        .expect("transformed effect")],
        weights: OracleWeights::Wls(weights.iter().map(|weight| 17.0 * weight).collect()),
        clusters: vec![],
    };

    let baseline = fit_explicit_ols(&baseline).expect("baseline WLS");
    let transformed = fit_explicit_ols(&transformed).expect("transformed WLS");

    assert_eq!(
        baseline.explicit_dummy_rank,
        transformed.explicit_dummy_rank
    );
    assert_eq!(baseline.model_rank, transformed.model_rank);
    assert_close(
        transformed.coefficients[0].expect("transformed slope"),
        baseline.coefficients[0].expect("baseline slope") / regressor_scale,
        5e-11,
    );
    assert_slice_close(&baseline.fitted_values, &transformed.fitted_values, 5e-11);
    assert_slice_close(&baseline.residuals, &transformed.residuals, 5e-11);
    assert_close(
        transformed.bread[(0, 0)],
        baseline.bread[(0, 0)] / (17.0 * regressor_scale * regressor_scale),
        5e-11,
    );
    assert_close(
        transformed.iid_vce[(0, 0)],
        baseline.iid_vce[(0, 0)] / (regressor_scale * regressor_scale),
        5e-11,
    );
    assert_close(
        transformed.hc1_vce[(0, 0)],
        baseline.hc1_vce[(0, 0)] / (regressor_scale * regressor_scale),
        5e-11,
    );
}

#[test]
fn tiny_common_wls_scale_preserves_hc_and_cluster_vces() {
    let outcome = vec![1.0, 2.5, 1.8, 4.2, 3.1, 7.0, 6.4, 9.2];
    let regressors = vec![vec![1.0; 8], vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 6.0]];
    let weights = vec![0.5, 2.0, 1.25, 3.0, 0.8, 1.5, 2.5, 4.0];
    let clusters = vec![vec![0, 0, 1, 1, 2, 2, 3, 3], vec![0, 1, 0, 1, 0, 1, 0, 1]];
    let baseline = ExplicitOlsProblem {
        outcome: outcome.clone(),
        regressors: regressors.clone(),
        effects: vec![],
        weights: OracleWeights::Wls(weights.clone()),
        clusters: clusters.clone(),
    };
    let tiny_scale = 1e-200;
    let tiny = ExplicitOlsProblem {
        outcome,
        regressors,
        effects: vec![],
        weights: OracleWeights::Wls(weights.iter().map(|weight| tiny_scale * weight).collect()),
        clusters,
    };

    let baseline = fit_explicit_ols(&baseline).expect("baseline-scale WLS");
    let tiny = fit_explicit_ols(&tiny).expect("tiny-scale WLS");

    assert_option_slice_close(&baseline.coefficients, &tiny.coefficients, 5e-11);
    assert_slice_close(&baseline.fitted_values, &tiny.fitted_values, 5e-11);
    assert_slice_close(&baseline.residuals, &tiny.residuals, 5e-11);
    assert_oracle_matrix_close(&baseline.iid_vce, &tiny.iid_vce, 5e-10);
    assert_oracle_matrix_close(&baseline.hc1_vce, &tiny.hc1_vce, 5e-10);
    for (baseline_vce, tiny_vce) in baseline
        .one_way_cluster_vces
        .iter()
        .zip(&tiny.one_way_cluster_vces)
    {
        assert_oracle_matrix_close(baseline_vce, tiny_vce, 5e-10);
    }
    assert_oracle_matrix_close(
        baseline.clustered_vce.as_ref().expect("baseline VCE"),
        tiny.clustered_vce.as_ref().expect("tiny-scale VCE"),
        5e-10,
    );
    assert_close(tiny.weighted_rss, baseline.weighted_rss * tiny_scale, 5e-10);
    for row in 0..baseline.bread.nrows() {
        for column in 0..baseline.bread.ncols() {
            assert_close(
                tiny.bread[(row, column)],
                baseline.bread[(row, column)] / tiny_scale,
                5e-10,
            );
        }
    }
}

#[test]
fn rank_deficiency_returns_dropped_columns_but_identified_fitted_values() {
    let intercept = vec![1.0; 6];
    let duplicate = vec![2.0; 6];
    let problem = ExplicitOlsProblem {
        outcome: vec![1.0, 4.0, 2.0, 8.0, -1.0, 4.0],
        regressors: vec![intercept, duplicate],
        effects: vec![],
        weights: OracleWeights::Unit,
        clusters: vec![],
    };

    let result = fit_explicit_ols(&problem).expect("rank-deficient fit");

    assert_eq!(result.finite_regressor_rank, 1);
    assert_eq!(result.kept_regressors, vec![0]);
    assert_eq!(result.dropped_regressors, vec![1]);
    assert_close(
        result.coefficients[0].expect("retained intercept"),
        3.0,
        TIGHT,
    );
    assert_eq!(result.coefficients[1], None);
    assert_slice_close(&result.fitted_values, &[3.0; 6], TIGHT);
}

#[test]
fn regressor_exactly_collinear_with_group_dummies_has_zero_finite_rank() {
    let problem = ExplicitOlsProblem {
        outcome: vec![1.0, -1.0, 2.0, -2.0, 3.0, -3.0],
        regressors: vec![vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0]],
        effects: vec![
            DummyEffect::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("group effect")
        ],
        weights: OracleWeights::Unit,
        clusters: vec![],
    };

    let result = fit_explicit_ols(&problem).expect("FE-collinear regressor");

    assert_eq!(result.explicit_dummy_rank, 3);
    assert_eq!(result.finite_regressor_rank, 0);
    assert_eq!(result.model_rank, 3);
    assert_eq!(result.coefficients, vec![None]);
    assert_eq!(result.dropped_regressors, vec![0]);
    assert_slice_close(&result.finite_fitted_values, &[0.0; 6], TIGHT);
}

#[test]
fn randomized_noisy_wls_is_invariant_to_observation_permutation() {
    let mut generator = Generator::new(0x51_46_57_4c_53_4f_4c_53);
    for replication in 0..24 {
        let nobs = 36;
        let first_codes = (0..nobs).map(|row| (row % 4) as u32).collect::<Vec<_>>();
        let second_codes = (0..nobs)
            .map(|row| ((row * 5 + row / 4) % 3) as u32)
            .collect::<Vec<_>>();
        let time = (0..nobs)
            .map(|row| row as f64 / 7.0 + generator.f64(-0.4, 0.4))
            .collect::<Vec<_>>();
        let first_x = (0..nobs)
            .map(|_| generator.f64(-2.0, 2.0))
            .collect::<Vec<_>>();
        let second_x = (0..nobs)
            .map(|row| 0.3 * first_x[row] + generator.f64(-1.5, 1.5))
            .collect::<Vec<_>>();
        let weights = (0..nobs)
            .map(|_| generator.f64(0.25, 4.0))
            .collect::<Vec<_>>();
        let outcome = (0..nobs)
            .map(|row| {
                1.7 * first_x[row] - 0.6 * second_x[row]
                    + [0.5, -1.0, 2.0, 0.1][first_codes[row] as usize]
                    + [0.2, -0.4, 0.8][second_codes[row] as usize]
                    + [0.3, -0.2, 0.1, 0.5][first_codes[row] as usize] * time[row]
                    + generator.f64(-0.3, 0.3)
            })
            .collect::<Vec<_>>();
        let clusters = vec![
            (0..nobs).map(|row| (row % 6) as u64).collect::<Vec<_>>(),
            (0..nobs)
                .map(|row| ((row / 3) % 4) as u64)
                .collect::<Vec<_>>(),
        ];
        let problem = ExplicitOlsProblem {
            outcome: outcome.clone(),
            regressors: vec![first_x.clone(), second_x.clone()],
            effects: vec![
                DummyEffect::new(first_codes.clone(), 4, true, vec![time.clone()])
                    .expect("first randomized effect"),
                DummyEffect::new(second_codes.clone(), 3, true, vec![])
                    .expect("second randomized effect"),
            ],
            weights: OracleWeights::Wls(weights.clone()),
            clusters: clusters.clone(),
        };
        let mut permutation = (0..nobs).collect::<Vec<_>>();
        generator.shuffle(&mut permutation);
        let permuted = ExplicitOlsProblem {
            outcome: permute(&outcome, &permutation),
            regressors: vec![
                permute(&first_x, &permutation),
                permute(&second_x, &permutation),
            ],
            effects: vec![
                DummyEffect::new(
                    permute(&first_codes, &permutation),
                    4,
                    true,
                    vec![permute(&time, &permutation)],
                )
                .expect("permuted first effect"),
                DummyEffect::new(permute(&second_codes, &permutation), 3, true, vec![])
                    .expect("permuted second effect"),
            ],
            weights: OracleWeights::Wls(permute(&weights, &permutation)),
            clusters: clusters
                .iter()
                .map(|cluster| permute(cluster, &permutation))
                .collect(),
        };

        let original = fit_explicit_ols(&problem)
            .unwrap_or_else(|error| panic!("randomized original {replication}: {error}"));
        let permuted = fit_explicit_ols(&permuted)
            .unwrap_or_else(|error| panic!("randomized permutation {replication}: {error}"));

        assert_eq!(original.explicit_dummy_rank, permuted.explicit_dummy_rank);
        assert_eq!(
            original.finite_regressor_rank,
            permuted.finite_regressor_rank
        );
        assert_option_slice_close(&original.coefficients, &permuted.coefficients, 2e-9);
        assert_oracle_matrix_close(&original.bread, &permuted.bread, 2e-9);
        assert_oracle_matrix_close(&original.hc1_vce, &permuted.hc1_vce, 2e-9);
        assert_oracle_matrix_close(
            original.clustered_vce.as_ref().expect("original clusters"),
            permuted.clustered_vce.as_ref().expect("permuted clusters"),
            2e-9,
        );
        for (permuted_row, &original_row) in permutation.iter().enumerate() {
            assert_close(
                original.fitted_values[original_row],
                permuted.fitted_values[permuted_row],
                2e-9,
            );
            assert_close(
                original.residuals[original_row],
                permuted.residuals[permuted_row],
                2e-9,
            );
        }
    }
}

#[test]
fn invalid_inputs_and_undefined_inference_fail_closed() {
    let invalid_outcome = ExplicitOlsProblem {
        outcome: vec![1.0, f64::NAN, 2.0],
        regressors: vec![vec![1.0; 3]],
        effects: vec![],
        weights: OracleWeights::Unit,
        clusters: vec![],
    };
    assert!(matches!(
        fit_explicit_ols(&invalid_outcome),
        Err(OracleError::InvalidInput(_))
    ));

    let invalid_weight = ExplicitOlsProblem {
        outcome: vec![1.0, 2.0, 3.0],
        regressors: vec![vec![1.0; 3]],
        effects: vec![],
        weights: OracleWeights::Wls(vec![1.0, 0.0, 2.0]),
        clusters: vec![],
    };
    assert!(matches!(
        fit_explicit_ols(&invalid_weight),
        Err(OracleError::InvalidInput(_))
    ));

    let saturated = ExplicitOlsProblem {
        outcome: vec![1.0, 2.0],
        regressors: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        effects: vec![],
        weights: OracleWeights::Unit,
        clusters: vec![],
    };
    assert!(matches!(
        fit_explicit_ols(&saturated),
        Err(OracleError::InsufficientDegreesOfFreedom(_))
    ));

    let one_cluster = ExplicitOlsProblem {
        outcome: vec![1.0, 2.0, 4.0],
        regressors: vec![vec![1.0; 3]],
        effects: vec![],
        weights: OracleWeights::Unit,
        clusters: vec![vec![0, 0, 0]],
    };
    assert!(matches!(
        fit_explicit_ols(&one_cluster),
        Err(OracleError::InsufficientDegreesOfFreedom(_))
    ));

    let overflowing_rss = ExplicitOlsProblem {
        outcome: vec![1e308, -1e308],
        regressors: vec![],
        effects: vec![],
        weights: OracleWeights::Unit,
        clusters: vec![],
    };
    assert!(matches!(
        fit_explicit_ols(&overflowing_rss),
        Err(OracleError::NumericalFailure(_))
    ));
}

fn expand<T: Copy>(values: &[T], frequencies: &[u32]) -> Vec<T> {
    values
        .iter()
        .zip(frequencies)
        .flat_map(|(&value, &frequency)| std::iter::repeat_n(value, frequency as usize))
        .collect()
}

fn permute<T: Copy>(values: &[T], permutation: &[usize]) -> Vec<T> {
    permutation.iter().map(|&row| values[row]).collect()
}

fn assert_close(left: f64, right: f64, tolerance: f64) {
    let scale = left.abs().max(right.abs()).max(1.0);
    assert!(
        (left - right).abs() <= tolerance * scale,
        "left={left:.17e}, right={right:.17e}, tolerance={tolerance:.3e}"
    );
}

fn assert_slice_close(left: &[f64], right: &[f64], tolerance: f64) {
    assert_eq!(left.len(), right.len());
    for (&left, &right) in left.iter().zip(right) {
        assert_close(left, right, tolerance);
    }
}

fn assert_option_slice_close(left: &[Option<f64>], right: &[Option<f64>], tolerance: f64) {
    assert_eq!(left.len(), right.len());
    for (left, right) in left.iter().zip(right) {
        match (left, right) {
            (Some(left), Some(right)) => assert_close(*left, *right, tolerance),
            (None, None) => {}
            _ => panic!("coefficient identification differs: {left:?} versus {right:?}"),
        }
    }
}

fn assert_matrix_close<const ROWS: usize, const COLUMNS: usize>(
    actual: &OracleMatrix,
    expected: &[[f64; COLUMNS]; ROWS],
    tolerance: f64,
) {
    assert_eq!(actual.nrows(), ROWS);
    assert_eq!(actual.ncols(), COLUMNS);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            assert_close(actual[(row, column)], expected[row][column], tolerance);
        }
    }
}

fn assert_oracle_matrix_close(left: &OracleMatrix, right: &OracleMatrix, tolerance: f64) {
    assert_eq!(left.nrows(), right.nrows());
    assert_eq!(left.ncols(), right.ncols());
    assert_slice_close(left.as_slice(), right.as_slice(), tolerance);
}

struct Generator(u64);

impl Generator {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn f64(&mut self, lower: f64, upper: f64) -> f64 {
        let unit = (self.next() >> 11) as f64 / (1_u64 << 53) as f64;
        (upper - lower).mul_add(unit, lower)
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for upper in (1..values.len()).rev() {
            let selected = (self.next() as usize) % (upper + 1);
            values.swap(upper, selected);
        }
    }
}
