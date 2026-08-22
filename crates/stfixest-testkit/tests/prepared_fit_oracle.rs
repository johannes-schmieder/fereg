use stfixest_core::{
    fit_prepared_ols_coefficients_map, fit_prepared_ols_unadjusted_map, prepare_ols_data,
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder,
    ExpandedTerm, LevelState, LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsInputBinding,
    OlsInputRole, OlsRequestV1, PreparedOlsAbsorbedDofPolicy, PreparedOlsBreadCoordinateKind,
    PreparedOlsData, PreparedOlsFitConfig, PreparedOlsFitStatus, PreparedOlsUnadjustedConfig,
    RawTermKind, SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};
use stfixest_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const COEFFICIENT_TOLERANCE: f64 = 2e-8;
const RESIDUAL_TOLERANCE: f64 = 2e-8;

#[derive(Clone, Copy)]
enum CaseWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
    Frequency(&'a [u32]),
}

#[test]
fn prepared_coefficients_and_residuals_match_explicit_dummies_for_all_weights() {
    let first_codes = vec![0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2];
    let second_codes = vec![0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1];
    let regressors = vec![
        vec![
            -2.0, -0.5, 1.0, 3.0, -1.0, 0.25, 2.0, 4.0, -3.0, 0.75, 2.5, 5.0,
        ],
        vec![
            1.0, -2.0, 0.5, 4.0, 2.0, -1.0, 3.5, 0.0, -0.5, 2.5, 1.5, 5.0,
        ],
    ];
    let outcome = (0..first_codes.len())
        .map(|row| {
            1.7 * regressors[0][row] - 0.6 * regressors[1][row]
                + [0.5, -1.0, 2.0][first_codes[row] as usize]
                + [0.2, -0.4][second_codes[row] as usize]
                + (row as f64 * 0.13).sin() * 0.2
        })
        .collect::<Vec<_>>();
    let weights = [0.5, 2.0, 1.25, 3.0, 0.75, 4.0, 1.5, 2.5, 5.0, 0.8, 3.5, 1.1];
    let frequencies = [1_u32, 2, 3, 1, 4, 2, 1, 3, 2, 5, 1, 2];

    for case in [
        CaseWeights::Unit,
        CaseWeights::Analytic(&weights),
        CaseWeights::Probability(&weights),
        CaseWeights::Frequency(&frequencies),
    ] {
        run_oracle_case(
            &outcome,
            &regressors,
            &[first_codes.clone(), second_codes.clone()],
            &[0, 1, 0],
            true,
            case,
        );
    }
}

#[test]
fn no_absorb_default_and_noconstant_match_an_explicit_one_level_dummy() {
    let regressors = vec![
        vec![-2.0, -0.5, 1.0, 3.0, -1.0, 0.25, 2.0, 4.0],
        vec![1.0, -2.0, 0.5, 4.0, 2.0, -1.0, 3.5, 0.0],
    ];
    let outcome = (0..regressors[0].len())
        .map(|row| {
            4.0 + 1.7 * regressors[0][row] - 0.6 * regressors[1][row] + 0.2 * (row as f64).sin()
        })
        .collect::<Vec<_>>();
    let analytic = [0.5, 2.0, 1.25, 3.0, 0.75, 4.0, 1.5, 2.5];
    let frequencies = [1_u32, 2, 3, 1, 4, 2, 1, 3];
    for include_intercept in [true, false] {
        for weights in [
            CaseWeights::Unit,
            CaseWeights::Analytic(&analytic),
            CaseWeights::Probability(&analytic),
            CaseWeights::Frequency(&frequencies),
        ] {
            run_oracle_case(&outcome, &regressors, &[], &[], include_intercept, weights);
        }
    }
}

#[test]
fn randomized_well_conditioned_prepared_fits_match_independent_oracle() {
    let mut generator = Generator(0x8a13_72ce_4d9b_f061);
    for replication in 0..20 {
        let n = 30;
        let first_codes = (0..n).map(|row| (row % 3) as u32).collect::<Vec<_>>();
        let second_codes = (0..n)
            .map(|row| ((row * 5 + row / 3) % 4) as u32)
            .collect::<Vec<_>>();
        let first_x = (0..n).map(|_| generator.f64(-3.0, 3.0)).collect::<Vec<_>>();
        let second_x = (0..n)
            .map(|row| 0.2 * first_x[row] + generator.f64(-2.0, 2.0))
            .collect::<Vec<_>>();
        let weights = (0..n).map(|_| generator.f64(0.2, 5.0)).collect::<Vec<_>>();
        let outcome = (0..n)
            .map(|row| {
                2.3 * first_x[row] - 0.8 * second_x[row]
                    + [0.5, -1.0, 1.7][first_codes[row] as usize]
                    + [0.1, -0.4, 0.9, 1.2][second_codes[row] as usize]
                    + generator.f64(-0.15, 0.15)
            })
            .collect::<Vec<_>>();
        let regressors = vec![first_x, second_x];
        let effects = vec![first_codes, second_codes];
        run_oracle_case(
            &outcome,
            &regressors,
            &effects,
            &[0, 1],
            replication % 2 == 0,
            CaseWeights::Analytic(&weights),
        );
    }
}

#[test]
fn observation_permutation_preserves_identified_fit() {
    let first_codes = vec![0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
    let second_codes = vec![0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2];
    let x = vec![
        -2.0, 0.5, 3.0, -1.0, 1.5, 4.0, -3.0, 2.0, 5.0, -0.5, 2.5, 6.0,
    ];
    let weights = vec![0.5, 2.0, 3.0, 1.0, 4.0, 1.5, 2.5, 0.75, 5.0, 1.2, 3.3, 0.9];
    let outcome = (0..x.len())
        .map(|row| {
            1.9 * x[row]
                + [0.5, -1.0, 2.0, 0.2][first_codes[row] as usize]
                + [0.3, -0.6, 0.8][second_codes[row] as usize]
                + 0.1 * (row as f64).cos()
        })
        .collect::<Vec<_>>();
    let effects = vec![first_codes, second_codes];
    let baseline = solve_case(
        &outcome,
        std::slice::from_ref(&x),
        &effects,
        &[0, 1],
        false,
        CaseWeights::Analytic(&weights),
    );
    let permutation = [11_usize, 2, 7, 0, 9, 4, 6, 1, 10, 5, 8, 3];
    let permuted_outcome = permute(&outcome, &permutation);
    let permuted_x = permute(&x, &permutation);
    let permuted_effects = effects
        .iter()
        .map(|codes| permute(codes, &permutation))
        .collect::<Vec<_>>();
    let permuted_weights = permute(&weights, &permutation);
    let permuted = solve_case(
        &permuted_outcome,
        std::slice::from_ref(&permuted_x),
        &permuted_effects,
        &[0, 1],
        false,
        CaseWeights::Analytic(&permuted_weights),
    );
    assert_close(baseline.0, permuted.0, COEFFICIENT_TOLERANCE);
    let mut restored = vec![0.0; baseline.1.len()];
    for (new_row, old_row) in permutation.iter().copied().enumerate() {
        restored[old_row] = permuted.1[new_row];
    }
    assert_slice_close(&baseline.1, &restored, RESIDUAL_TOLERANCE);

    let baseline_inference = fit_prepared_ols_unadjusted_map(
        &prepare_case(
            &outcome,
            std::slice::from_ref(&x),
            &effects,
            &[0, 1],
            false,
            CaseWeights::Analytic(&weights),
        ),
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("baseline permutation inference");
    let permuted_inference = fit_prepared_ols_unadjusted_map(
        &prepare_case(
            &permuted_outcome,
            std::slice::from_ref(&permuted_x),
            &permuted_effects,
            &[0, 1],
            false,
            CaseWeights::Analytic(&permuted_weights),
        ),
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("permuted inference");
    assert_eq!(baseline_inference.dof(), permuted_inference.dof());
    assert_slice_close(
        baseline_inference.covariance().values(),
        permuted_inference.covariance().values(),
        5e-8,
    );
    assert_close(
        baseline_inference.statistics().rss,
        permuted_inference.statistics().rss,
        5e-8,
    );
    assert_close(
        baseline_inference.statistics().within_tss,
        permuted_inference.statistics().within_tss,
        5e-8,
    );
}

#[test]
fn analytic_inference_is_invariant_to_extreme_common_weight_scales() {
    let groups = vec![0_u32, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
    let x = vec![
        -2.0, -0.5, 1.0, -1.0, 2.0, 4.0, -3.0, 1.5, 5.0, 0.2, 2.8, 6.0,
    ];
    let y = (0..x.len())
        .map(|row| {
            1.8 * x[row] + [0.5, -1.0, 1.4, 0.2][groups[row] as usize] + 0.08 * (row as f64).cos()
        })
        .collect::<Vec<_>>();
    let base = [0.25, 2.0, 1.5, 3.0, 0.75, 4.0, 1.2, 2.7, 5.0, 0.6, 3.3, 1.1];
    let tiny = base.map(|weight| weight * 1e-150);
    let huge = base.map(|weight| weight * 1e150);
    let effects = vec![groups];
    let solve = |weights: &[f64]| {
        fit_prepared_ols_unadjusted_map(
            &prepare_case(
                &y,
                std::slice::from_ref(&x),
                &effects,
                &[0],
                true,
                CaseWeights::Analytic(weights),
            ),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("extreme-scale analytic inference")
    };
    let baseline = solve(&base);
    for scaled in [solve(&tiny), solve(&huge)] {
        assert_eq!(baseline.dof(), scaled.dof());
        assert_slice_close(
            baseline.covariance().values(),
            scaled.covariance().values(),
            2e-8,
        );
        assert_close(baseline.statistics().rss, scaled.statistics().rss, 2e-8);
        assert_close(
            baseline.statistics().within_tss,
            scaled.statistics().within_tss,
            2e-8,
        );
    }
}

#[test]
fn virtual_constant_is_invariant_to_extreme_aweight_scaling_and_row_permutation() {
    let x = vec![
        -2.0, -0.5, 1.0, -1.0, 2.0, 4.0, -3.0, 1.5, 5.0, 0.2, 2.8, 6.0,
    ];
    let y = (0..x.len())
        .map(|row| 3.5 + 1.8 * x[row] + 0.08 * (row as f64).cos())
        .collect::<Vec<_>>();
    let base = [0.25, 2.0, 1.5, 3.0, 0.75, 4.0, 1.2, 2.7, 5.0, 0.6, 3.3, 1.1];
    let tiny = base.map(|weight| weight * 1e-150);
    let huge = base.map(|weight| weight * 1e150);
    let solve = |outcome: &[f64], regressor: &[f64], weights: &[f64]| {
        fit_prepared_ols_unadjusted_map(
            &prepare_case(
                outcome,
                &[regressor.to_vec()],
                &[],
                &[],
                false,
                CaseWeights::Analytic(weights),
            ),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("virtual-constant analytic inference")
    };
    let baseline = solve(&y, &x, &base);
    for scaled in [solve(&y, &x, &tiny), solve(&y, &x, &huge)] {
        assert_eq!(baseline.dof(), scaled.dof());
        assert_slice_close(baseline.fit().residuals(), scaled.fit().residuals(), 2e-8);
        assert_slice_close(
            baseline.covariance().values(),
            scaled.covariance().values(),
            2e-8,
        );
        assert_close(baseline.statistics().rss, scaled.statistics().rss, 2e-8);
        assert_close(baseline.statistics().tss, scaled.statistics().tss, 2e-8);
    }

    let permutation = [11_usize, 2, 7, 0, 9, 4, 6, 1, 10, 5, 8, 3];
    let permuted_y = permute(&y, &permutation);
    let permuted_x = permute(&x, &permutation);
    let permuted_weights = permute(&base, &permutation);
    let permuted = solve(&permuted_y, &permuted_x, &permuted_weights);
    assert_eq!(baseline.dof(), permuted.dof());
    assert_close(
        baseline.fit().coefficients()[0]
            .value
            .expect("baseline slope"),
        permuted.fit().coefficients()[0]
            .value
            .expect("permuted slope"),
        COEFFICIENT_TOLERANCE,
    );
    let mut restored = vec![0.0; y.len()];
    for (new_row, old_row) in permutation.iter().copied().enumerate() {
        restored[old_row] = permuted.fit().residuals()[new_row];
    }
    assert_slice_close(baseline.fit().residuals(), &restored, RESIDUAL_TOLERANCE);
    assert_slice_close(
        baseline.covariance().values(),
        permuted.covariance().values(),
        2e-8,
    );
}

#[test]
fn frequency_inference_matches_literal_row_expansion() {
    let groups = vec![0_u32, 0, 1, 1, 2, 2];
    let x = vec![-2.0, 0.5, -1.0, 2.0, 1.5, 4.0];
    let y = vec![0.4, 5.1, -2.2, 3.7, 4.0, 8.3];
    let frequencies = [1_u32, 3, 2, 4, 1, 2];
    let weighted = fit_prepared_ols_unadjusted_map(
        &prepare_case(
            &y,
            std::slice::from_ref(&x),
            std::slice::from_ref(&groups),
            &[0],
            true,
            CaseWeights::Frequency(&frequencies),
        ),
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("frequency inference");
    let mut expanded_y = Vec::new();
    let mut expanded_x = Vec::new();
    let mut expanded_groups = Vec::new();
    for row in 0..y.len() {
        for _ in 0..frequencies[row] {
            expanded_y.push(y[row]);
            expanded_x.push(x[row]);
            expanded_groups.push(groups[row]);
        }
    }
    let expanded = fit_prepared_ols_unadjusted_map(
        &prepare_case(
            &expanded_y,
            &[expanded_x],
            &[expanded_groups],
            &[0],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("literal frequency expansion inference");
    assert_eq!(weighted.weights().effective_observations, expanded_y.len());
    assert_eq!(weighted.dof(), expanded.dof());
    assert_slice_close(
        weighted.covariance().values(),
        expanded.covariance().values(),
        5e-8,
    );
    assert_close(weighted.statistics().rss, expanded.statistics().rss, 5e-8);
    assert_close(
        weighted.statistics().within_tss,
        expanded.statistics().within_tss,
        5e-8,
    );
}

#[test]
fn sealed_stata18_reference_anchors_match_reported_intercept_and_bread() {
    // Immutable reference receipt:
    // benchmarks/reports/captured/darwin-arm64/stata18/ols/
    // 20260821T030000Z-a5181f9
    // payload sha256 6e861febcedfc8cc76570ee627c2010d4738750c732c600bc2cf0f29e88e8002
    // reghdfe commit 29b2b203f534413369d675d0c0f674d61291f536.
    let core = include_str!("../../../tests/fixtures/ols/ols_core.csv");
    let weights_fixture = include_str!("../../../tests/fixtures/ols/ols_weights.csv");

    assert_sealed_reference_case(
        &csv_numeric_column(core, "y"),
        &[
            csv_numeric_column(core, "x1"),
            csv_numeric_column(core, "x2"),
        ],
        &[],
        &[],
        true,
        CaseWeights::Unit,
        SealedReferenceExpected {
            coefficients: &[
                "+1.6094dc78ce848X+000",
                "-1.57252c6b41c29X-006",
                "+1.84d9face23e96X+002",
            ],
            unadjusted_vce: Some((
                "+1.5000000000000X+004",
                "+1.2a14530d58416X+000",
                &[
                    "+1.1759580b65592X-009",
                    "+1.ec3b5507a2059X-00d",
                    "-1.3c2a429049131X-00b",
                    "+1.ec3b5507a2059X-00d",
                    "+1.83d2fd489555fX-00a",
                    "+1.9837b6c9dcb53X-00f",
                    "-1.3c2a429049131X-00b",
                    "+1.9837b6c9dcb53X-00f",
                    "+1.465e95b087a5cX-009",
                ],
            )),
        },
    );

    let worker = csv_u32_column(core, "worker");
    let firm = csv_u32_column(core, "firm");
    assert_sealed_reference_case(
        &csv_numeric_column(core, "y"),
        &[],
        &[worker, firm],
        &[0, 1],
        true,
        CaseWeights::Unit,
        SealedReferenceExpected {
            coefficients: &["+1.9eaaaaa000000X+002"],
            unadjusted_vce: Some((
                "+1.8000000000000X+002",
                "+1.46041bbe76f57X+000",
                &["+1.21cac353f7f69X-007"],
            )),
        },
    );

    let outcome = csv_numeric_column(weights_fixture, "y");
    let regressor = vec![csv_numeric_column(weights_fixture, "x")];
    let group = vec![csv_u32_column(weights_fixture, "group")];
    let analytic_weights = csv_numeric_column(weights_fixture, "w_valid");
    assert_sealed_reference_case(
        &outcome,
        &regressor,
        &group,
        &[0],
        true,
        CaseWeights::Analytic(&analytic_weights),
        SealedReferenceExpected {
            coefficients: &["+1.ec1c26732c450X+000", "+1.025c8c5c40273X+002"],
            unadjusted_vce: Some((
                "+1.4000000000000X+002",
                "+1.cf6b1cff85537X-005",
                &[
                    "+1.599e09a857cddX-005",
                    "-1.47ef40b47d972X-005",
                    "-1.47ef40b47d972X-005",
                    "+1.3ee1529cf1611X-005",
                ],
            )),
        },
    );

    let rescaled_weights = csv_numeric_column(weights_fixture, "w_scaled");
    assert_sealed_reference_case(
        &outcome,
        &regressor,
        &group,
        &[0],
        true,
        CaseWeights::Analytic(&rescaled_weights),
        SealedReferenceExpected {
            coefficients: &["+1.ec1c26840b380X+000", "+1.025c8c582b537X+002"],
            unadjusted_vce: Some((
                "+1.4000000000000X+002",
                "+1.cf6b1cc818f35X-005",
                &[
                    "+1.599e09858b2a4X-005",
                    "-1.47ef40a07671cX-005",
                    "-1.47ef40a07671cX-005",
                    "+1.3ee1529557a10X-005",
                ],
            )),
        },
    );

    let frequencies = csv_u32_column(weights_fixture, "fw_valid");
    assert_sealed_reference_case(
        &outcome,
        &regressor,
        &group,
        &[0],
        true,
        CaseWeights::Frequency(&frequencies),
        SealedReferenceExpected {
            coefficients: &["+1.071c74a8cc936X+001", "+1.02bfab6db776aX+002"],
            unadjusted_vce: Some((
                "+1.3000000000000X+004",
                "+1.6dcbce835d647X-004",
                &[
                    "+1.37267e35e5f40X-007",
                    "-1.517a7c751b09aX-008",
                    "-1.517a7c751b09aX-008",
                    "+1.85bac823e9064X-009",
                ],
            )),
        },
    );

    let probability_weights = csv_numeric_column(weights_fixture, "pw_valid");
    assert_sealed_reference_case(
        &outcome,
        &regressor,
        &group,
        &[0],
        true,
        CaseWeights::Probability(&probability_weights),
        SealedReferenceExpected {
            coefficients: &["+1.0a633d2c1dd4cX+001", "+1.02c391a4fa903X+002"],
            unadjusted_vce: None,
        },
    );
}

#[test]
fn sealed_stata18_inference_statistics_weights_and_dof_policies_match() {
    // Same immutable Stata 18/reghdfe receipt and payload digest documented by
    // `sealed_stata18_reference_anchors_match_reported_intercept_and_bread`.
    let core = include_str!("../../../tests/fixtures/ols/ols_core.csv");
    let weights_fixture = include_str!("../../../tests/fixtures/ols/ols_weights.csv");
    let outcome = csv_numeric_column(core, "y");
    let x1 = csv_numeric_column(core, "x1");
    let x2 = csv_numeric_column(core, "x2");
    let worker = csv_u32_column(core, "worker");
    let firm = csv_u32_column(core, "firm");
    let year = csv_u32_column(core, "year");

    let no_fe = fit_prepared_ols_unadjusted_map(
        &prepare_case(
            &outcome,
            &[x1.clone(), x2],
            &[],
            &[],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("sealed no-FE iid inference");
    assert_eq!(no_fe.weights().physical_observations, 24);
    assert_eq!(no_fe.weights().effective_observations, 24);
    assert_eq!(no_fe.dof().finite_rank, 2);
    assert_eq!(no_fe.dof().finite_model_dof, 2);
    assert_eq!(no_fe.dof().absorbed_rank, 1);
    assert_eq!(no_fe.dof().residual_dof, 21);
    assert!(no_fe.dof().has_structural_intercept);
    assert_close(
        no_fe.statistics().rss,
        stata_hex("+1.2a14530d58416X+000"),
        3e-6,
    );
    assert_close(
        no_fe.statistics().tss,
        stata_hex("+1.9d325a41a745bX+005"),
        3e-6,
    );
    assert_close(
        no_fe.statistics().within_tss,
        stata_hex("+1.9d325a41a745bX+005"),
        3e-6,
    );
    assert_close(
        no_fe.statistics().r_squared.expect("sealed R2"),
        stata_hex("+1.f475270c974d5X-001"),
        3e-6,
    );
    assert_close(
        no_fe
            .statistics()
            .adjusted_r_squared
            .expect("sealed adjusted R2"),
        stata_hex("+1.f35bbd0dca488X-001"),
        3e-6,
    );
    assert_close(
        no_fe.statistics().root_mean_squared_error,
        stata_hex("+1.e23e4fe17df11X-003"),
        3e-6,
    );
    assert_close(
        no_fe.statistics().f_statistic.expect("sealed F"),
        stata_hex("+1.c74319df063baX+008"),
        3e-6,
    );

    let disconnected = fit_prepared_ols_unadjusted_map(
        &prepare_case(
            &outcome,
            &[],
            &[worker.clone(), firm.clone()],
            &[0, 1],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("sealed disconnected FE-only iid inference");
    assert_eq!(disconnected.dof().finite_rank, 0);
    assert_eq!(disconnected.dof().finite_model_dof, 0);
    assert_eq!(disconnected.dof().absorbed_rank, 18);
    assert_eq!(disconnected.dof().residual_dof, 6);
    assert!(disconnected.dof().has_normalized_absorbed_intercept);
    assert_eq!(disconnected.covariance().dimension(), 1);
    assert_close(
        disconnected.statistics().rss,
        stata_hex("+1.46041bbe76f57X+000"),
        3e-6,
    );
    assert_close(
        disconnected.statistics().tss,
        stata_hex("+1.9d325a41a745bX+005"),
        3e-6,
    );
    assert!(disconnected.statistics().f_statistic.is_none());

    let one_fe_factor = fit_prepared_ols_unadjusted_map(
        &prepare_one_fe_factor_case(core),
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("sealed one-FE factor iid statistics");
    assert_eq!(one_fe_factor.dof().absorbed_rank, 8);
    assert_eq!(one_fe_factor.dof().finite_model_dof, 3);
    assert_eq!(one_fe_factor.dof().residual_dof, 13);
    assert_close(
        one_fe_factor
            .statistics()
            .within_r_squared
            .expect("sealed within R2"),
        stata_hex("+1.fe76cdff850d0X-001"),
        3e-6,
    );
    assert_close(
        one_fe_factor
            .statistics()
            .adjusted_within_r_squared
            .expect("sealed adjusted within R2"),
        stata_hex("+1.fe1c113a7c5edX-001"),
        3e-6,
    );

    let weighted_outcome = csv_numeric_column(weights_fixture, "y");
    let weighted_x = vec![csv_numeric_column(weights_fixture, "x")];
    let weighted_group = vec![csv_u32_column(weights_fixture, "group")];
    for (column, expected_sum) in [
        ("w_valid", "+1.9cccccd000000X+004"),
        ("w_scaled", "+1.0200000000000X+008"),
    ] {
        let raw = csv_numeric_column(weights_fixture, column);
        let inference = fit_prepared_ols_unadjusted_map(
            &prepare_case(
                &weighted_outcome,
                &weighted_x,
                &weighted_group,
                &[0],
                true,
                CaseWeights::Analytic(&raw),
            ),
            PreparedOlsUnadjustedConfig::default(),
        )
        .expect("sealed analytic-weight iid inference");
        assert_eq!(inference.weights().effective_observations, 12);
        assert_eq!(inference.dof().residual_dof, 5);
        assert_close(
            inference
                .weights()
                .raw_weight_sum
                .expect("analytic raw sum"),
            stata_hex(expected_sum),
            3e-7,
        );
    }
    let frequencies = csv_u32_column(weights_fixture, "fw_valid");
    let frequency = fit_prepared_ols_unadjusted_map(
        &prepare_case(
            &weighted_outcome,
            &weighted_x,
            &weighted_group,
            &[0],
            true,
            CaseWeights::Frequency(&frequencies),
        ),
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("sealed frequency-weight iid inference");
    assert_eq!(frequency.weights().effective_observations, 26);
    assert_eq!(frequency.dof().residual_dof, 19);
    assert_eq!(frequency.weights().raw_weight_sum, Some(26.0));

    let none = fit_prepared_ols_unadjusted_map(
        &prepare_case(
            &outcome,
            std::slice::from_ref(&x1),
            &[worker.clone(), firm.clone()],
            &[0, 1],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsUnadjustedConfig {
            absorbed_dof: PreparedOlsAbsorbedDofPolicy::None,
            ..PreparedOlsUnadjustedConfig::default()
        },
    )
    .expect("sealed dof(none) iid inference");
    assert_eq!(none.dof().absorbed_rank, 19);
    assert_eq!(none.dof().residual_dof, 4);
    assert_slice_close(
        none.covariance().values(),
        &[
            stata_hex("+1.f80df410ae9caX-006"),
            stata_hex("-1.260823b465db6X-007"),
            stata_hex("-1.260823b465db6X-007"),
            stata_hex("+1.a4d459ecc8bf9X-009"),
        ],
        3e-6,
    );

    let first_pair = fit_prepared_ols_unadjusted_map(
        &prepare_case(
            &outcome,
            &[],
            &[worker, firm, year],
            &[0, 1, 2],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsUnadjustedConfig {
            absorbed_dof: PreparedOlsAbsorbedDofPolicy::FirstPair,
            ..PreparedOlsUnadjustedConfig::default()
        },
    )
    .expect("sealed dof(firstpair) iid inference");
    assert_eq!(first_pair.dof().absorbed_rank, 21);
    assert_eq!(first_pair.dof().finite_rank, 0);
    assert_eq!(first_pair.dof().residual_dof, 3);
    assert_eq!(first_pair.covariance().dimension(), 1);
    assert_close(
        first_pair.covariance().values()[0],
        stata_hex("+1.78d531440dbd9X-00b"),
        3e-6,
    );
}

struct SealedReferenceExpected<'a> {
    coefficients: &'a [&'a str],
    unadjusted_vce: Option<(&'a str, &'a str, &'a [&'a str])>,
}

fn assert_sealed_reference_case(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effects: &[Vec<u32>],
    effect_order: &[usize],
    include_intercept: bool,
    weights: CaseWeights<'_>,
    expected: SealedReferenceExpected<'_>,
) {
    let prepared = prepare_case(
        outcome,
        regressors,
        effects,
        effect_order,
        include_intercept,
        weights,
    );
    let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
        .expect("sealed-reference prepared fit");
    assert_eq!(fit.coefficients().len(), expected.coefficients.len());
    for (actual, expected) in fit.coefficients().iter().zip(expected.coefficients) {
        assert_close(
            actual.value.expect("sealed-reference coefficient"),
            stata_hex(expected),
            3e-7,
        );
    }
    if include_intercept {
        assert_eq!(
            fit.coefficients()
                .last()
                .expect("sealed reported intercept")
                .status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert_eq!(fit.bread().dimension(), fit.finite_rank() + 1);
        assert_eq!(
            fit.bread()
                .coordinates()
                .last()
                .expect("sealed intercept coordinate")
                .kind,
            PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept
        );
    }
    if let Some((df_residual, rss, covariance)) = expected.unadjusted_vce {
        assert_eq!(fit.bread().values().len(), covariance.len());
        let multiplier = stata_hex(df_residual) / stata_hex(rss);
        let expected = covariance
            .iter()
            .map(|value| stata_hex(value) * multiplier)
            .collect::<Vec<_>>();
        assert_slice_close(fit.bread().values(), &expected, 3e-6);

        let inference =
            fit_prepared_ols_unadjusted_map(&prepared, PreparedOlsUnadjustedConfig::default())
                .expect("sealed-reference prepared iid inference");
        assert_eq!(
            inference.dof().residual_dof,
            stata_hex(df_residual) as usize
        );
        assert_close(inference.statistics().rss, stata_hex(rss), 3e-6);
        let expected_covariance = covariance
            .iter()
            .map(|value| stata_hex(value))
            .collect::<Vec<_>>();
        assert_slice_close(inference.covariance().values(), &expected_covariance, 3e-6);
        assert!(inference.certificate().passed);
    } else {
        let error =
            fit_prepared_ols_unadjusted_map(&prepared, PreparedOlsUnadjustedConfig::default())
                .expect_err("probability-weight iid inference must fail closed");
        assert_eq!(error.code, stfixest_core::ErrorCode::Unsupported);
    }
}

fn csv_numeric_column(csv: &str, name: &str) -> Vec<f64> {
    let mut lines = csv.lines();
    let headers = lines
        .next()
        .expect("fixture header")
        .split(',')
        .collect::<Vec<_>>();
    let position = headers
        .iter()
        .position(|header| *header == name)
        .expect("fixture column");
    lines
        .map(|line| {
            line.split(',')
                .nth(position)
                .expect("fixture field")
                .parse::<f64>()
                .expect("numeric fixture field")
        })
        .collect()
}

fn csv_u32_column(csv: &str, name: &str) -> Vec<u32> {
    csv_numeric_column(csv, name)
        .into_iter()
        .map(|value| value as u32)
        .collect()
}

fn stata_hex(value: &str) -> f64 {
    let (mantissa, exponent) = value.split_once('X').expect("Stata hex separator");
    let negative = mantissa.starts_with('-');
    let unsigned = mantissa.trim_start_matches(['+', '-']);
    let (whole, fraction) = unsigned.split_once('.').expect("Stata hex mantissa");
    let whole = u64::from_str_radix(whole, 16).expect("Stata hex whole") as f64;
    let mut parsed = whole;
    let mut scale = 1.0 / 16.0;
    for digit in fraction.bytes() {
        parsed += f64::from(char::from(digit).to_digit(16).expect("Stata hex digit")) * scale;
        scale /= 16.0;
    }
    let exponent_negative = exponent.starts_with('-');
    let exponent = i32::from_str_radix(exponent.trim_start_matches(['+', '-']), 16)
        .expect("Stata hex exponent");
    let exponent = if exponent_negative {
        -exponent
    } else {
        exponent
    };
    let parsed = parsed * 2.0_f64.powi(exponent);
    if negative {
        -parsed
    } else {
        parsed
    }
}

fn run_oracle_case(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effect_sources: &[Vec<u32>],
    effect_order: &[usize],
    include_intercept: bool,
    weights: CaseWeights<'_>,
) {
    let prepared = prepare_case(
        outcome,
        regressors,
        effect_sources,
        effect_order,
        include_intercept,
        weights,
    );
    let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
        .expect("certified prepared fit");
    assert!(fit.absorption_report().certified());
    assert!(fit.residual_absorption_report().certified());
    assert!(fit.rank_certificate().passed);
    assert!(fit.score_certificate().passed);

    let absorbed_reported_intercept = include_intercept;
    let oracle_regressors = regressors.to_vec();
    let explicit_effects = || {
        if effect_order.is_empty() {
            vec![
                DummyEffect::new(vec![0; outcome.len()], 1, true, Vec::new())
                    .expect("implicit one-level dummy effect"),
            ]
        } else {
            effect_order
                .iter()
                .map(|source| {
                    let codes = effect_sources[*source].clone();
                    DummyEffect::new(codes.clone(), level_count(&codes), true, Vec::new())
                        .expect("dummy effect")
                })
                .collect()
        }
    };
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: outcome.to_vec(),
        regressors: oracle_regressors,
        effects: explicit_effects(),
        weights: oracle_weights(weights),
        clusters: Vec::new(),
    })
    .expect("explicit-dummy oracle");

    if !matches!(weights, CaseWeights::Probability(_)) {
        let inference = fit_prepared_ols_unadjusted_map(
            &prepared,
            PreparedOlsUnadjustedConfig {
                absorbed_dof: PreparedOlsAbsorbedDofPolicy::Pairwise,
                ..PreparedOlsUnadjustedConfig::default()
            },
        )
        .expect("certified prepared iid inference");
        assert!(inference.certificate().passed);
        assert_eq!(
            inference.weights().effective_observations,
            oracle.effective_observations
        );
        assert_eq!(inference.dof().model_rank, oracle.model_rank);
        assert_eq!(
            inference.dof().residual_dof,
            oracle.residual_degrees_of_freedom
        );

        let reference_weights = reference_case_weights(weights, outcome.len());
        let reference_rss = oracle
            .residuals
            .iter()
            .zip(&reference_weights)
            .map(|(residual, weight)| residual * residual * weight)
            .sum::<f64>();
        assert_close(inference.statistics().rss, reference_rss, 5e-8);
        let overall_center = weighted_mean(outcome, &reference_weights);
        let reference_tss = outcome
            .iter()
            .zip(&reference_weights)
            .map(|(value, weight)| weight * (value - overall_center).powi(2))
            .sum::<f64>();
        assert_close(inference.statistics().tss, reference_tss, 5e-8);
        let within_oracle = fit_explicit_ols(&ExplicitOlsProblem {
            outcome: outcome.to_vec(),
            regressors: Vec::new(),
            effects: explicit_effects(),
            weights: oracle_weights(weights),
            clusters: Vec::new(),
        })
        .expect("explicit-dummy within projection");
        let reference_within_tss = within_oracle
            .residuals
            .iter()
            .zip(&reference_weights)
            .map(|(value, weight)| weight * value.powi(2))
            .sum::<f64>();
        assert_close(
            inference.statistics().within_tss,
            reference_within_tss,
            5e-8,
        );

        let coordinates = inference.covariance_coordinates();
        let finite_dimension = oracle.kept_regressors.len();
        for row in 0..finite_dimension {
            for column in 0..finite_dimension {
                assert_eq!(
                    coordinates[row].kind,
                    PreparedOlsBreadCoordinateKind::FiniteRegressor
                );
                assert_close(
                    inference.covariance().values()[row * coordinates.len() + column],
                    oracle.iid_vce[(row, column)],
                    8e-8,
                );
            }
        }
    }

    assert_eq!(
        fit.coefficients().len(),
        oracle.coefficients.len() + usize::from(absorbed_reported_intercept)
    );
    for (index, (actual, expected)) in fit.coefficients()[..oracle.coefficients.len()]
        .iter()
        .zip(&oracle.coefficients)
        .enumerate()
    {
        match expected {
            Some(expected) => {
                assert_eq!(actual.status, PreparedOlsFitStatus::Kept, "slot {index}");
                assert_close(
                    actual.value.expect("identified coefficient"),
                    *expected,
                    COEFFICIENT_TOLERANCE,
                );
            }
            None => assert_ne!(actual.status, PreparedOlsFitStatus::Kept),
        }
    }
    if absorbed_reported_intercept {
        let normalized = normalized_case_weights(weights, outcome.len());
        let expected_intercept = weighted_mean(
            &(0..outcome.len())
                .map(|row| {
                    oracle.coefficients.iter().enumerate().fold(
                        outcome[row],
                        |value, (column, coefficient)| {
                            coefficient.map_or(value, |coefficient| {
                                (-coefficient).mul_add(regressors[column][row], value)
                            })
                        },
                    )
                })
                .collect::<Vec<_>>(),
            &normalized,
        );
        let actual = fit.coefficients().last().expect("reported intercept slot");
        assert_eq!(
            actual.status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert_close(
            actual.value.expect("normalized reported intercept"),
            expected_intercept,
            COEFFICIENT_TOLERANCE,
        );

        let kept = &oracle.kept_regressors;
        let finite_dimension = kept.len();
        let dimension = finite_dimension + 1;
        let reference_multiplier = match weights {
            CaseWeights::Analytic(raw) | CaseWeights::Probability(raw) => {
                raw.iter().sum::<f64>() / outcome.len() as f64
            }
            CaseWeights::Unit | CaseWeights::Frequency(_) => 1.0,
        };
        let mut expected_bread = vec![0.0_f64; dimension * dimension];
        for row in 0..finite_dimension {
            for column in 0..finite_dimension {
                expected_bread[row * dimension + column] =
                    oracle.bread[(row, column)] * reference_multiplier;
            }
        }
        let means = kept
            .iter()
            .map(|column| weighted_mean(&regressors[*column], &normalized))
            .collect::<Vec<_>>();
        let reference_weight_sum = match weights {
            CaseWeights::Frequency(values) => values.iter().map(|value| *value as f64).sum(),
            CaseWeights::Unit | CaseWeights::Analytic(_) | CaseWeights::Probability(_) => {
                outcome.len() as f64
            }
        };
        let mut side = vec![0.0_f64; finite_dimension];
        for row in 0..finite_dimension {
            side[row] = -(0..finite_dimension)
                .map(|column| expected_bread[row * dimension + column] * means[column])
                .sum::<f64>();
            expected_bread[row * dimension + finite_dimension] = side[row];
            expected_bread[finite_dimension * dimension + row] = side[row];
        }
        expected_bread[finite_dimension * dimension + finite_dimension] = reference_weight_sum
            .recip()
            - means
                .iter()
                .zip(&side)
                .map(|(mean, side)| mean * side)
                .sum::<f64>();
        assert_eq!(fit.bread().dimension(), dimension);
        assert_eq!(fit.bread().coordinates().len(), dimension);
        assert!(fit.bread().coordinates()[..finite_dimension]
            .iter()
            .all(|coordinate| coordinate.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor));
        assert_eq!(
            fit.bread().coordinates()[finite_dimension].kind,
            PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept
        );
        assert_slice_close(fit.bread().values(), &expected_bread, 5e-8);
        assert!(fit.bread().certificate().passed);
    }
    assert_eq!(fit.finite_rank(), oracle.finite_regressor_rank);
    assert_slice_close(fit.residuals(), &oracle.residuals, RESIDUAL_TOLERANCE);
}

fn normalized_case_weights(weights: CaseWeights<'_>, nobs: usize) -> Vec<f64> {
    match weights {
        CaseWeights::Unit => vec![1.0; nobs],
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => values.to_vec(),
        CaseWeights::Frequency(values) => values.iter().map(|value| *value as f64).collect(),
    }
}

fn reference_case_weights(weights: CaseWeights<'_>, nobs: usize) -> Vec<f64> {
    match weights {
        CaseWeights::Unit => vec![1.0; nobs],
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => {
            let multiplier = nobs as f64 / values.iter().sum::<f64>();
            values.iter().map(|value| value * multiplier).collect()
        }
        CaseWeights::Frequency(values) => values.iter().map(|value| f64::from(*value)).collect(),
    }
}

fn weighted_mean(values: &[f64], weights: &[f64]) -> f64 {
    values
        .iter()
        .zip(weights)
        .map(|(value, weight)| value * weight)
        .sum::<f64>()
        / weights.iter().sum::<f64>()
}

fn solve_case(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effect_sources: &[Vec<u32>],
    effect_order: &[usize],
    include_intercept: bool,
    weights: CaseWeights<'_>,
) -> (f64, Vec<f64>) {
    let prepared = prepare_case(
        outcome,
        regressors,
        effect_sources,
        effect_order,
        include_intercept,
        weights,
    );
    let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
        .expect("certified prepared fit");
    (
        fit.coefficients()[0].value.expect("identified slope"),
        fit.residuals().to_vec(),
    )
}

fn prepare_case(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effect_sources: &[Vec<u32>],
    effect_order: &[usize],
    include_intercept: bool,
    weights: CaseWeights<'_>,
) -> PreparedOlsData {
    let n = outcome.len();
    assert!(regressors.iter().all(|values| values.len() == n));
    assert!(effect_sources.iter().all(|values| values.len() == n));
    let source_count = regressors.len() + effect_sources.len();
    let sources = (0..source_count)
        .map(|index| SourceEntry {
            id: source_id(index),
            canonical_name: format!("s{}", index + 1),
        })
        .collect::<Vec<_>>();
    let terms = (0..regressors.len())
        .map(|index| continuous_term(index, source_id(index)))
        .collect::<Vec<_>>();
    let mut bindings = vec![
        OlsInputBinding {
            stata_var_position: 1,
            role: OlsInputRole::Candidate,
        },
        OlsInputBinding {
            stata_var_position: 2,
            role: OlsInputRole::Outcome,
        },
    ];
    bindings.extend((0..source_count).map(|index| OlsInputBinding {
        stata_var_position: u32::try_from(index + 3).expect("small binding"),
        role: OlsInputRole::NumericSource(source_id(index)),
    }));
    if let Some(role) = weight_role(weights) {
        bindings.push(OlsInputBinding {
            stata_var_position: u32::try_from(source_count + 3).expect("small weight binding"),
            role,
        });
    }
    let raw_weight = weight_values(weights);
    let loaded = LoadedOlsInput {
        candidate: vec![true; n],
        outcome: loaded_column(outcome.to_vec()),
        numeric_sources: regressors
            .iter()
            .cloned()
            .chain(effect_sources.iter().map(|codes| {
                codes
                    .iter()
                    .map(|code| f64::from(*code))
                    .collect::<Vec<_>>()
            }))
            .enumerate()
            .map(|(index, values)| (source_id(index), loaded_column(values)))
            .collect(),
        analytic_weight: matches!(weights, CaseWeights::Analytic(_))
            .then(|| loaded_column(raw_weight.clone())),
        frequency_weight: matches!(weights, CaseWeights::Frequency(_))
            .then(|| loaded_column(raw_weight.clone())),
        probability_weight: matches!(weights, CaseWeights::Probability(_))
            .then(|| loaded_column(raw_weight)),
    };
    let request = OlsRequestV1 {
        model: CompiledTermGraph { sources, terms },
        absorbed_terms: effect_order
            .iter()
            .enumerate()
            .map(|(index, source)| AbsorbedTerm {
                id: AbsorbTermId::new(u32::try_from(index + 1).expect("small effect"))
                    .expect("effect id"),
                intercept: true,
                categorical_sources: vec![source_id(regressors.len() + *source)],
                slope_sources: Vec::new(),
                label: format!("g{}_effect_{}", source + 1, index + 1),
                saved_name: String::new(),
            })
            .collect(),
        bindings,
        include_intercept,
        keep_singletons: true,
    };
    prepare_ols_data(&request, &loaded).expect("prepared oracle case")
}

fn prepare_one_fe_factor_case(core: &str) -> PreparedOlsData {
    let outcome = csv_numeric_column(core, "y");
    let x1 = csv_numeric_column(core, "x1");
    let industry = csv_numeric_column(core, "industry");
    let firm = csv_numeric_column(core, "firm");
    let x_source = source_id(0);
    let industry_source = source_id(1);
    let firm_source = source_id(2);
    let mut terms = vec![continuous_term(0, x_source)];
    for (index, level, base) in [(2_u32, 1.0, true), (3, 2.0, false), (4, 3.0, false)] {
        terms.push(ExpandedTerm {
            id: TermId::new(index).expect("factor term"),
            order: TermOrder::new(index).expect("factor order"),
            coefficient_slot: CoefficientSlot::new(index).expect("factor slot"),
            raw_kind: RawTermKind::Factor,
            whole_column_omitted: base,
            canonical_token: if base {
                "1b.industry".to_owned()
            } else {
                format!("{level}.industry")
            },
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("factor component"),
                source: industry_source,
                kind: ComponentKind::Categorical,
                level_state: LevelState::Present,
                level_value: level,
                base: if base {
                    MetadataFlag::True
                } else {
                    MetadataFlag::False
                },
                omitted: if base {
                    MetadataFlag::True
                } else {
                    MetadataFlag::False
                },
                raw_operator: if base {
                    "1b".to_owned()
                } else {
                    level.to_string()
                },
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        });
    }
    prepare_ols_data(
        &OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![
                    SourceEntry {
                        id: x_source,
                        canonical_name: "x1".to_owned(),
                    },
                    SourceEntry {
                        id: industry_source,
                        canonical_name: "industry".to_owned(),
                    },
                    SourceEntry {
                        id: firm_source,
                        canonical_name: "firm".to_owned(),
                    },
                ],
                terms,
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("firm absorb term"),
                intercept: true,
                categorical_sources: vec![firm_source],
                slope_sources: Vec::new(),
                label: "firm".to_owned(),
                saved_name: String::new(),
            }],
            bindings: vec![
                OlsInputBinding {
                    stata_var_position: 1,
                    role: OlsInputRole::Candidate,
                },
                OlsInputBinding {
                    stata_var_position: 2,
                    role: OlsInputRole::Outcome,
                },
                OlsInputBinding {
                    stata_var_position: 3,
                    role: OlsInputRole::NumericSource(x_source),
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: OlsInputRole::NumericSource(industry_source),
                },
                OlsInputBinding {
                    stata_var_position: 5,
                    role: OlsInputRole::NumericSource(firm_source),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        },
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()],
            outcome: loaded_column(outcome),
            numeric_sources: vec![
                (x_source, loaded_column(x1)),
                (industry_source, loaded_column(industry)),
                (firm_source, loaded_column(firm)),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
    )
    .expect("prepared one-FE factor case")
}

fn continuous_term(index: usize, source: SourceId) -> ExpandedTerm {
    let one_based = u32::try_from(index + 1).expect("small term");
    ExpandedTerm {
        id: TermId::new(one_based).expect("term id"),
        order: TermOrder::new(one_based).expect("term order"),
        coefficient_slot: CoefficientSlot::new(one_based).expect("slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: format!("c.x{one_based}"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component"),
            source,
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.0,
            base: MetadataFlag::False,
            omitted: MetadataFlag::Unavailable,
            raw_operator: "c".to_owned(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

fn source_id(index: usize) -> SourceId {
    SourceId::new(u32::try_from(index + 1).expect("small source")).expect("source id")
}

fn loaded_column(values: Vec<f64>) -> LoadedNumericColumn {
    LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    }
}

fn level_count(codes: &[u32]) -> usize {
    codes
        .iter()
        .copied()
        .max()
        .map_or(0, |value| value as usize + 1)
}

fn weight_role(weights: CaseWeights<'_>) -> Option<OlsInputRole> {
    match weights {
        CaseWeights::Unit => None,
        CaseWeights::Analytic(_) => Some(OlsInputRole::AnalyticWeight),
        CaseWeights::Probability(_) => Some(OlsInputRole::ProbabilityWeight),
        CaseWeights::Frequency(_) => Some(OlsInputRole::FrequencyWeight),
    }
}

fn weight_values(weights: CaseWeights<'_>) -> Vec<f64> {
    match weights {
        CaseWeights::Unit => Vec::new(),
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => values.to_vec(),
        CaseWeights::Frequency(values) => values.iter().map(|value| f64::from(*value)).collect(),
    }
}

fn oracle_weights(weights: CaseWeights<'_>) -> OracleWeights {
    match weights {
        CaseWeights::Unit => OracleWeights::Unit,
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => {
            OracleWeights::Wls(values.to_vec())
        }
        CaseWeights::Frequency(values) => OracleWeights::Frequency(values.to_vec()),
    }
}

fn permute<T: Copy>(values: &[T], permutation: &[usize]) -> Vec<T> {
    permutation.iter().map(|row| values[*row]).collect()
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    let scale = actual.abs().max(expected.abs()).max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance * scale,
        "actual={actual:.17e}, expected={expected:.17e}, tolerance={tolerance:.3e}"
    );
}

fn assert_slice_close(actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().copied().zip(expected.iter().copied()) {
        assert_close(actual, expected, tolerance);
    }
}

struct Generator(u64);

impl Generator {
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
}
