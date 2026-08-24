use fereg_core::{
    fit_prepared_ols_clustered_map, prepare_ols_clustered_data, AbsorbTermId, AbsorbedTerm,
    CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm, LevelState,
    LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsClusterTermV1, OlsClusteredRequestV1,
    OlsInputBinding, OlsInputRole, OlsRequestV1, PreparedOlsBreadCoordinateKind,
    PreparedOlsClusteredConfig, PreparedOlsClusteredData, PreparedOlsClusteredPsdState,
    RawTermKind, SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};
use fereg_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const INTERNAL_TOLERANCE: f64 = 2e-10;

#[derive(Clone, Copy)]
enum CaseWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
    Frequency(&'a [u32]),
}

#[test]
fn one_way_noconstant_matches_independent_explicit_dummy_cr1() {
    let n = 24;
    let cluster = (0..n).map(|row| (row % 6) as u32).collect::<Vec<_>>();
    let effect = (0..n)
        .map(|row| ((row / 3 + row % 2) % 4) as u32)
        .collect::<Vec<_>>();
    let x = (0..n)
        .map(|row| (row as f64 * 0.7).sin() + row as f64 / 13.0)
        .collect::<Vec<_>>();
    let outcome = (0..n)
        .map(|row| {
            1.8 * x[row]
                + [0.4, -0.7, 1.1, -0.3][effect[row] as usize]
                + 0.21 * (row as f64 * 1.3).cos()
        })
        .collect::<Vec<_>>();
    let production = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            std::slice::from_ref(&x),
            std::slice::from_ref(&effect),
            &[vec![cluster.clone()]],
            false,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("one-way prepared clustered fit");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome,
        regressors: vec![x],
        effects: vec![DummyEffect::new(effect, 4, true, vec![]).expect("effect")],
        weights: OracleWeights::Unit,
        clusters: vec![cluster.iter().map(|value| u64::from(*value)).collect()],
    })
    .expect("one-way explicit oracle");
    assert_close_slice(
        production.covariance().values(),
        oracle
            .clustered_vce
            .as_ref()
            .expect("oracle clustered covariance")
            .as_slice(),
    );
    assert_eq!(production.cluster_counts(), &[6]);
    assert_eq!(production.dof().reported_residual_dof, 5);
    assert_eq!(production.subsets().len(), 1);
    assert!(production.certificate().passed);
}

#[test]
fn one_two_and_three_way_full_covariances_match_generic_explicit_oracle() {
    let n = 48;
    let first = (0..n).map(|row| (row % 8) as u32).collect::<Vec<_>>();
    let second = (0..n)
        .map(|row| ((row / 2 + row % 3) % 4) as u32)
        .collect::<Vec<_>>();
    let third = (0..n)
        .map(|row| ((row / 5 + row % 2) % 3) as u32)
        .collect::<Vec<_>>();
    let effect = (0..n)
        .map(|row| ((row / 3 + row % 5) % 5) as u32)
        .collect::<Vec<_>>();
    let x1 = (0..n)
        .map(|row| (0.37 * row as f64).sin() + row as f64 / 31.0)
        .collect::<Vec<_>>();
    let x2 = (0..n)
        .map(|row| (0.23 * row as f64).cos() - row as f64 / 47.0)
        .collect::<Vec<_>>();
    let outcome = (0..n)
        .map(|row| {
            1.4 * x1[row] - 0.8 * x2[row]
                + [0.4, -0.6, 1.0, -0.2, 0.7][effect[row] as usize]
                + 0.19 * (1.7 * row as f64).sin()
        })
        .collect::<Vec<_>>();
    let all = [first, second, third];
    for dimensions in 1..=3 {
        let clusters = all[..dimensions]
            .iter()
            .cloned()
            .map(|codes| vec![codes])
            .collect::<Vec<_>>();
        let oracle_clusters = all[..dimensions]
            .iter()
            .map(|codes| codes.iter().map(|value| u64::from(*value)).collect())
            .collect::<Vec<_>>();
        let oracle = fit_explicit_ols(&ExplicitOlsProblem {
            outcome: outcome.clone(),
            regressors: vec![x1.clone(), x2.clone()],
            effects: vec![DummyEffect::new(effect.clone(), 5, true, vec![]).expect("effect")],
            weights: OracleWeights::Unit,
            clusters: oracle_clusters,
        })
        .expect("generic explicit clustered oracle");
        let intercept_cases: &[bool] = &[false, true];
        for &include_intercept in intercept_cases {
            let production = fit_prepared_ols_clustered_map(
                &prepare_case(
                    &outcome,
                    &[x1.clone(), x2.clone()],
                    std::slice::from_ref(&effect),
                    &clusters,
                    include_intercept,
                    CaseWeights::Unit,
                ),
                PreparedOlsClusteredConfig::default(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "generic prepared clustered fit failed for dimensions={dimensions}, include_intercept={include_intercept}: {error:?}"
                )
            });
            let expected = if include_intercept {
                oracle
                    .normalized_reported_intercept_clustered_vce
                    .as_ref()
                    .expect("reported constant cluster oracle")
            } else {
                oracle.clustered_vce.as_ref().expect("cluster oracle")
            };
            assert_close_slice(production.covariance().raw_values(), expected.as_slice());
            assert_eq!(production.subsets().len(), (1_usize << dimensions) - 1);
            assert_eq!(
                production.dof().reported_residual_dof,
                oracle
                    .residual_degrees_of_freedom
                    .min(production.dof().minimum_cluster_count - 1)
            );
            assert!(production.certificate().passed);
        }
    }
}

#[test]
fn analytic_probability_scaling_and_frequency_expansion_follow_cluster_score_rules() {
    let n = 24;
    let cluster = (0..n).map(|row| (row % 6) as u32).collect::<Vec<_>>();
    let effect = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
    let x = (0..n)
        .map(|row| (row as f64 * 0.41).sin() + row as f64 / 19.0)
        .collect::<Vec<_>>();
    let outcome = (0..n)
        .map(|row| {
            1.3 * x[row] + [0.2, -0.4, 0.8, -0.7][effect[row] as usize] + 0.11 * (row as f64).cos()
        })
        .collect::<Vec<_>>();
    let weights = (0..n)
        .map(|row| 0.4 + ((row * 7) % 11) as f64 / 3.0)
        .collect::<Vec<_>>();
    let scaled = weights.iter().map(|value| 1e7 * value).collect::<Vec<_>>();
    let frequencies = (0..n)
        .map(|row| u32::try_from(row % 4 + 1).expect("frequency"))
        .collect::<Vec<_>>();
    let analytic = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            std::slice::from_ref(&x),
            std::slice::from_ref(&effect),
            &[vec![cluster.clone()]],
            true,
            CaseWeights::Analytic(&weights),
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("analytic clustered fit");
    let probability = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            std::slice::from_ref(&x),
            std::slice::from_ref(&effect),
            &[vec![cluster.clone()]],
            true,
            CaseWeights::Probability(&weights),
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("probability clustered fit");
    let probability_scaled = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            std::slice::from_ref(&x),
            std::slice::from_ref(&effect),
            &[vec![cluster.clone()]],
            true,
            CaseWeights::Probability(&scaled),
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("scaled probability clustered fit");
    assert_close_slice(
        analytic.covariance().values(),
        probability.covariance().values(),
    );
    assert_close_slice(
        probability.covariance().values(),
        probability_scaled.covariance().values(),
    );

    let compact = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            std::slice::from_ref(&x),
            std::slice::from_ref(&effect),
            &[vec![cluster.clone()]],
            false,
            CaseWeights::Frequency(&frequencies),
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("frequency clustered fit");
    let expanded_outcome = expand(&outcome, &frequencies);
    let expanded_x = expand(&x, &frequencies);
    let expanded_effect = expand(&effect, &frequencies);
    let expanded_cluster = expand(&cluster, &frequencies);
    let expanded = fit_prepared_ols_clustered_map(
        &prepare_case(
            &expanded_outcome,
            &[expanded_x],
            &[expanded_effect],
            &[vec![expanded_cluster]],
            false,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("expanded clustered fit");
    assert_close_slice(
        compact.covariance().values(),
        expanded.covariance().values(),
    );
}

#[test]
fn cluster_nesting_precedes_mobility_and_keeps_all_three_dof_quantities_distinct() {
    let n = 30;
    let nested_effect = (0..n).map(|row| (row % 6) as u32).collect::<Vec<_>>();
    let containing_cluster = nested_effect
        .iter()
        .map(|code| code / 2)
        .collect::<Vec<_>>();
    let x = (0..n)
        .map(|row| (0.31 * row as f64).sin() + row as f64 / 23.0)
        .collect::<Vec<_>>();
    let outcome = (0..n)
        .map(|row| {
            1.1 * x[row]
                + [0.2, -0.5, 0.8, -0.3, 1.0, -0.7][nested_effect[row] as usize]
                + 0.09 * (row as f64).cos()
        })
        .collect::<Vec<_>>();
    let prepared = prepare_case(
        &outcome,
        &[x],
        &[nested_effect],
        &[vec![containing_cluster]],
        false,
        CaseWeights::Unit,
    );
    let nested = fit_prepared_ols_clustered_map(&prepared, PreparedOlsClusteredConfig::default())
        .expect("nested clustered fit");
    assert_eq!(nested.absorbed_dof().absorbed_rank, 0);
    assert_eq!(nested.absorbed_dof().nested_categories, 6);
    assert!(nested.absorbed_dof().effects[0].nested);
    assert_eq!(nested.dof().unclustered_residual_dof, 29);
    assert_eq!(nested.dof().reported_residual_dof, 2);
    assert_eq!(nested.dof().fit_residual_dof, 23);
    assert_eq!(nested.dof().correction_denominator_dof, 28);

    let without_nesting = fit_prepared_ols_clustered_map(
        &prepared,
        PreparedOlsClusteredConfig {
            account_cluster_nesting: false,
            ..PreparedOlsClusteredConfig::default()
        },
    )
    .expect("clustered fit without nesting adjustment");
    assert_eq!(without_nesting.absorbed_dof().absorbed_rank, 6);
    assert_eq!(without_nesting.absorbed_dof().nested_categories, 0);
    assert_eq!(without_nesting.dof().unclustered_residual_dof, 23);
    assert_eq!(without_nesting.dof().reported_residual_dof, 2);
    assert_eq!(without_nesting.dof().fit_residual_dof, 23);
    assert_eq!(without_nesting.dof().correction_denominator_dof, 23);
}

#[test]
fn declared_interaction_is_one_dimension_and_matches_precombined_cluster_codes() {
    let n = 36;
    let first = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
    let second = (0..n)
        .map(|row| ((row / 3 + row % 2) % 3) as u32)
        .collect::<Vec<_>>();
    let combined = first
        .iter()
        .zip(&second)
        .map(|(left, right)| left * 3 + right)
        .collect::<Vec<_>>();
    let x = (0..n)
        .map(|row| (0.29 * row as f64).sin() + row as f64 / 29.0)
        .collect::<Vec<_>>();
    let outcome = x
        .iter()
        .enumerate()
        .map(|(row, value)| 1.7 * value + 0.13 * (row as f64).cos())
        .collect::<Vec<_>>();
    let interaction = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            std::slice::from_ref(&x),
            &[],
            &[vec![first, second]],
            false,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("interaction-cluster fit");
    let precombined = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            &[x],
            &[],
            &[vec![combined]],
            false,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("precombined-cluster fit");
    assert_eq!(interaction.cluster_counts(), precombined.cluster_counts());
    assert_eq!(interaction.subsets().len(), 1);
    assert_close_slice(
        interaction.covariance().values(),
        precombined.covariance().values(),
    );
}

#[test]
fn missing_cluster_is_reason_six_before_fe_singleton_pruning_and_small_groups_are_retained() {
    let outcome = vec![1.0, 2.0, 2.5, 4.0, 5.0, 6.5, 7.0, 8.0];
    let x = vec![-2.0, -1.0, 0.0, 1.0, -1.5, 0.5, 2.0, 3.0];
    let effect = vec![0_u32, 0, 1, 1, 2, 2, 3, 3];
    let cluster = vec![0_u32, 0, 1, 1, 2, 2, 3, 4];
    let (mut request, mut loaded) = case_inputs(
        &outcome,
        &[x],
        &[effect],
        &[vec![cluster]],
        false,
        CaseWeights::Unit,
    );
    request.ols.keep_singletons = false;
    let cluster_source = loaded.numeric_sources.last_mut().expect("cluster source");
    cluster_source.1.missing[2] = true;
    let prepared = prepare_ols_clustered_data(&request, &loaded).expect("missing cluster prepare");
    assert_eq!(
        prepared.ols().sample.drop_reason(2),
        Some(fereg_core::DropReason::MissingCluster)
    );
    assert_eq!(prepared.ols().singletons.num_singletons, 1);
    assert_eq!(
        prepared.ols().sample.drop_reason(3),
        Some(fereg_core::DropReason::Singleton)
    );
    assert_eq!(prepared.ols().sample.n_active(), 6);
    assert!(prepared
        .clusters()
        .dimensions()
        .iter()
        .all(|dimension| dimension.groups() >= 2));
}

#[test]
fn rank_zero_and_roundoff_exact_fit_are_defined_without_nan_sentinels() {
    let n = 18;
    let cluster = (0..n).map(|row| (row % 3) as u32).collect::<Vec<_>>();
    let effect = (0..n).map(|row| (row % 6) as u32).collect::<Vec<_>>();
    let outcome = effect
        .iter()
        .map(|code| [0.2, -0.3, 0.7, -0.8, 1.1, -0.5][*code as usize])
        .collect::<Vec<_>>();
    let rank_zero = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            &[],
            std::slice::from_ref(&effect),
            &[vec![cluster.clone()]],
            false,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("rank-zero clustered fit");
    assert_eq!(rank_zero.covariance().dimension(), 0);
    assert_eq!(rank_zero.statistics().f_statistic, None);
    assert_eq!(
        rank_zero.covariance().psd().state,
        PreparedOlsClusteredPsdState::OneWayStrict
    );
    assert!(rank_zero.covariance().psd().strict_final_full_psd);

    let x = (0..n).map(|row| row as f64 - 8.5).collect::<Vec<_>>();
    let exact_outcome = (0..n)
        .map(|row| 2.0 * x[row] + outcome[row])
        .collect::<Vec<_>>();
    let singular = fit_prepared_ols_clustered_map(
        &prepare_case(
            &exact_outcome,
            &[x],
            &[effect],
            &[vec![cluster]],
            false,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("zero-meat clustered fit");
    assert!(singular
        .statistics()
        .f_statistic
        .is_none_or(|value| value.is_finite()));
    assert!(singular
        .covariance()
        .values()
        .iter()
        .all(|value| value.is_finite()));
}

#[test]
fn sealed_stata18_cluster_covariances_and_counts_match_pinned_receipt() {
    // Immutable receipt:
    // benchmarks/reports/captured/darwin-arm64/stata18/ols/
    // 20260821T030000Z-a5181f9
    // payload sha256 6e861febcedfc8cc76570ee627c2010d4738750c732c600bc2cf0f29e88e8002
    // reghdfe commit 29b2b203f534413369d675d0c0f674d61291f536.
    // Interaction receipt command:
    // reghdfe y x1, absorb(worker firm) vce(cluster cluster_a#cluster_b)
    let csv = include_str!("../../../tests/fixtures/ols/ols_core.csv");
    let outcome = csv_numeric_column(csv, "y");
    let x = csv_numeric_column(csv, "x1");
    let worker = csv_u32_column(csv, "worker");
    let firm = csv_u32_column(csv, "firm");
    let first = csv_u32_column(csv, "cluster_a");
    let second = csv_u32_column(csv, "cluster_b");
    let third = csv_u32_column(csv, "cluster_c");
    let cases = [
        (
            vec![vec![first.clone()]],
            vec![8_usize],
            vec![
                "+1.55d7b24d54e16X-006",
                "-1.8ed0faa7131f0X-008",
                "-1.8ed0faa7131f0X-008",
                "+1.d1492463b1f98X-00a",
            ],
        ),
        (
            vec![vec![first.clone()], vec![second.clone()]],
            vec![8_usize, 4],
            vec![
                "+1.0202f4787935bX-005",
                "-1.2c88da1cac911X-007",
                "-1.2c88da1cac911X-007",
                "+1.5e0658149d738X-009",
            ],
        ),
        (
            vec![
                vec![first.clone()],
                vec![second.clone()],
                vec![third.clone()],
            ],
            vec![8_usize, 4, 3],
            vec![
                "+1.337c143e3831eX-006",
                "-1.6a5e0063700aaX-008",
                "-1.6a5e0063700aaX-008",
                "+1.aae99ff9c51f2X-00a",
            ],
        ),
        (
            vec![vec![first, second]],
            vec![16_usize],
            vec![
                "+1.c714def42ee8cX-006",
                "-1.0976d76bf8296X-007",
                "-1.0976d76bf8296X-007",
                "+1.547154bc25ea1X-009",
            ],
        ),
    ];
    for (case_index, (clusters, counts, expected)) in cases.into_iter().enumerate() {
        let oracle_clusters = clusters
            .iter()
            .filter(|sources| sources.len() == 1)
            .map(|sources| {
                sources[0]
                    .iter()
                    .map(|value| u64::from(*value))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let oracle = (oracle_clusters.len() == clusters.len()).then(|| {
            fit_explicit_ols(&ExplicitOlsProblem {
                outcome: outcome.clone(),
                regressors: vec![x.clone()],
                effects: vec![
                    DummyEffect::new(
                        worker.iter().map(|value| value - 1).collect(),
                        usize::try_from(*worker.iter().max().expect("worker max"))
                            .expect("worker levels"),
                        true,
                        vec![],
                    )
                    .expect("worker dummy"),
                    DummyEffect::new(
                        firm.iter().map(|value| value - 1).collect(),
                        usize::try_from(*firm.iter().max().expect("firm max"))
                            .expect("firm levels"),
                        true,
                        vec![],
                    )
                    .expect("firm dummy"),
                ],
                weights: OracleWeights::Unit,
                clusters: oracle_clusters,
            })
            .expect("sealed explicit oracle")
        });
        let result = fit_prepared_ols_clustered_map(
            &prepare_case(
                &outcome,
                std::slice::from_ref(&x),
                &[worker.clone(), firm.clone()],
                &clusters,
                true,
                CaseWeights::Unit,
            ),
            PreparedOlsClusteredConfig::default(),
        )
        .expect("sealed clustered fit");
        assert_eq!(result.cluster_counts(), counts);
        let expected = expected
            .iter()
            .map(|value| stata_hex(value))
            .collect::<Vec<_>>();
        if let Some(oracle) = oracle {
            let oracle_correction = counts.iter().copied().min().expect("minimum groups") as f64
                / (counts.iter().copied().min().expect("minimum groups") - 1) as f64
                * (oracle.effective_observations - 1) as f64
                / (oracle.effective_observations - oracle.model_rank) as f64;
            let rescaled = oracle
                .normalized_reported_intercept_clustered_vce
                .expect("reported oracle")
                .as_slice()
                .iter()
                .map(|value| value / oracle_correction * result.covariance().correction())
                .collect::<Vec<_>>();
            assert_close_slice_with_tolerance(result.covariance().raw_values(), &rescaled, 2e-10);
        }
        let psd = result.covariance().psd();
        let effective = result.weights().effective_observations as f64;
        let minimum_groups = result.dof().minimum_cluster_count as f64;
        let expected_correction =
            (effective - 1.0) / result.dof().correction_denominator_dof as f64 * minimum_groups
                / (minimum_groups - 1.0);
        assert!((result.covariance().correction() - expected_correction).abs() <= f64::EPSILON);
        let determinant = result.covariance().values()[0] * result.covariance().values()[3]
            - result.covariance().values()[1] * result.covariance().values()[2];
        if case_index == 1 || case_index == 2 {
            assert_eq!(
                psd.state,
                PreparedOlsClusteredPsdState::MultiwayFullClippedSlopeRestored
            );
            assert!(psd.full_adjusted);
            assert!(psd.slope_block_restored);
            assert!(!psd.strict_final_full_psd);
            assert!(psd.minimum_pre_restore_full_eigenvalue >= -1e-16);
            assert!(
                psd.minimum_restored_slope_eigenvalue
                    .expect("restored slope")
                    >= -1e-16
            );
            assert!(psd.minimum_final_full_eigenvalue < 0.0);
            assert!(determinant < 0.0);
        } else {
            assert_eq!(psd.state, PreparedOlsClusteredPsdState::OneWayStrict);
            assert!(!psd.full_adjusted);
            assert!(!psd.slope_block_restored);
            assert!(psd.strict_final_full_psd);
            assert!(psd.minimum_final_full_eigenvalue >= -1e-16);
            assert!(determinant >= -1e-16);
        }
        assert!(psd.transform_reconstruction_relative_error <= INTERNAL_TOLERANCE);
        assert!(
            result
                .certificate()
                .independent_influence_relative_infinity_norm
                <= INTERNAL_TOLERANCE
        );
        assert!(
            result
                .certificate()
                .corrected_covariance_relative_infinity_norm
                <= INTERNAL_TOLERANCE
        );
        assert!(result.certificate().final_structure_relative_error <= INTERNAL_TOLERANCE);
        match case_index {
            0 => {
                assert_eq!(result.dof().nested_absorbed_categories, 8);
                assert_eq!(result.dof().absorbed_rank, 11);
                assert_eq!(result.dof().reported_residual_dof, 7);
                assert_eq!(result.dof().fit_residual_dof, 4);
                assert_eq!(result.dof().correction_denominator_dof, 11);
                assert_eq!(
                    result
                        .subsets()
                        .iter()
                        .map(|subset| subset.sign)
                        .collect::<Vec<_>>(),
                    vec![1]
                );
                assert_eq!(
                    result
                        .subsets()
                        .iter()
                        .map(|subset| subset.mask)
                        .collect::<Vec<_>>(),
                    vec![1]
                );
            }
            1 => {
                assert_eq!(result.dof().nested_absorbed_categories, 20);
                assert_eq!(result.dof().absorbed_rank, 0);
                assert_eq!(result.dof().reported_residual_dof, 3);
                assert_eq!(result.dof().fit_residual_dof, 3);
                assert_eq!(result.dof().correction_denominator_dof, 22);
                assert_eq!(
                    result
                        .subsets()
                        .iter()
                        .map(|subset| subset.sign)
                        .collect::<Vec<_>>(),
                    vec![1, 1, -1]
                );
                assert_eq!(
                    result
                        .subsets()
                        .iter()
                        .map(|subset| subset.mask)
                        .collect::<Vec<_>>(),
                    vec![1, 2, 3]
                );
            }
            2 => {
                assert_eq!(result.dof().nested_absorbed_categories, 20);
                assert_eq!(result.dof().absorbed_rank, 0);
                assert_eq!(result.dof().reported_residual_dof, 2);
                assert_eq!(result.dof().fit_residual_dof, 3);
                assert_eq!(result.dof().correction_denominator_dof, 22);
                assert_eq!(
                    result
                        .subsets()
                        .iter()
                        .map(|subset| subset.sign)
                        .collect::<Vec<_>>(),
                    vec![1, 1, 1, -1, -1, -1, 1]
                );
                assert_eq!(
                    result
                        .subsets()
                        .iter()
                        .map(|subset| subset.mask)
                        .collect::<Vec<_>>(),
                    vec![1, 2, 4, 3, 5, 6, 7]
                );
            }
            3 => {
                assert_eq!(result.weights().physical_observations, 24);
                assert_eq!(result.weights().effective_observations, 24);
                assert_eq!(clusters.len(), 1);
                assert_eq!(clusters[0].len(), 2);
                assert_eq!(result.cluster_counts(), &[16]);
                assert_eq!(result.dof().absorbed_rank, 18);
                assert_eq!(result.dof().nested_absorbed_categories, 0);
                assert_eq!(result.dof().unclustered_residual_dof, 5);
                assert_eq!(result.dof().reported_residual_dof, 5);
                assert_eq!(result.dof().fit_residual_dof, 5);
                assert_eq!(result.dof().correction_denominator_dof, 5);
                assert_eq!(result.subsets().len(), 1);
                assert_eq!(result.subsets()[0].sign, 1);
                assert_eq!(result.subsets()[0].mask, 1);
                assert_eq!(result.covariance().dimension(), 2);
                assert_eq!(
                    result
                        .covariance_coordinates()
                        .iter()
                        .map(|coordinate| coordinate.kind)
                        .collect::<Vec<_>>(),
                    vec![
                        PreparedOlsBreadCoordinateKind::FiniteRegressor,
                        PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept,
                    ]
                );
            }
            _ => unreachable!("sealed case index"),
        }
        // These absolute caps describe only the immutable receipt's DEFAULT
        // reghdfe MAP stopping artifact. They are applied after exact sample,
        // cluster, subset, DoF, coordinate, and PSD-state assertions. They
        // never relax fereg's <=2e-10 internal oracle/transform/certificate
        // checks, imitate the upstream early stop, or reclip restored CGM V.
        let receipt_cap = match case_index {
            0 => 1e-7,
            1 => 2e-6,
            2 => 4e-6,
            3 => 1e-7,
            _ => unreachable!("sealed receipt cap index"),
        };
        assert_absolute_close_slice(result.covariance().values(), &expected, receipt_cap);
        assert!(result.certificate().passed);
    }
}

#[test]
fn cluster_dimension_bounds_and_single_group_dimensions_fail_closed() {
    let outcome = vec![1.0, 2.0, 3.0, 4.0];
    let x = vec![-1.0, 0.0, 1.0, 2.0];
    let (empty_request, empty_loaded) = case_inputs(
        &outcome,
        std::slice::from_ref(&x),
        &[],
        &[],
        false,
        CaseWeights::Unit,
    );
    let error = prepare_ols_clustered_data(&empty_request, &empty_loaded)
        .expect_err("zero cluster dimensions must fail");
    assert_eq!(error.code, fereg_core::ErrorCode::InvalidArgument);

    let dimensions = (0..11)
        .map(|dimension| {
            vec![(0..outcome.len())
                .map(|row| u32::try_from((row + dimension) % 2).expect("code"))
                .collect::<Vec<_>>()]
        })
        .collect::<Vec<_>>();
    let (too_many_request, too_many_loaded) = case_inputs(
        &outcome,
        std::slice::from_ref(&x),
        &[],
        &dimensions,
        false,
        CaseWeights::Unit,
    );
    let error = prepare_ols_clustered_data(&too_many_request, &too_many_loaded)
        .expect_err("eleven cluster dimensions must fail");
    assert_eq!(error.code, fereg_core::ErrorCode::InvalidArgument);

    let (one_group_request, one_group_loaded) = case_inputs(
        &outcome,
        &[x],
        &[],
        &[vec![vec![0_u32; outcome.len()]]],
        false,
        CaseWeights::Unit,
    );
    let error = prepare_ols_clustered_data(&one_group_request, &one_group_loaded)
        .expect_err("one-group cluster dimension must fail");
    assert_eq!(error.code, fereg_core::ErrorCode::InvalidData);
}

#[test]
fn disconnected_and_repeated_fe_spans_match_explicit_dummy_cluster_oracles() {
    let first = vec![0_u32, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3];
    let second = vec![0_u32, 1, 0, 1, 0, 1, 0, 1, 2, 3, 2, 3, 2, 3, 2, 3];
    let cluster = (0..first.len())
        .map(|row| ((row * 3 + row / 2) % 5) as u32)
        .collect::<Vec<_>>();
    let x = vec![
        -3.0, -1.0, 0.5, 2.0, -2.5, -0.25, 1.0, 3.5, -4.0, -1.5, 1.5, 4.0, -3.5, -0.5, 2.5, 5.0,
    ];
    let outcome = (0..first.len())
        .map(|row| {
            1.6 * x[row]
                + [0.5, -0.8, 1.4, -1.2][first[row] as usize]
                + [0.3, -0.4, 0.9, -0.6][second[row] as usize]
                + 0.19 * (row as f64).sin()
        })
        .collect::<Vec<_>>();
    let production = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            std::slice::from_ref(&x),
            &[first.clone(), second.clone()],
            &[vec![cluster.clone()]],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("disconnected clustered fit");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: outcome.clone(),
        regressors: vec![x.clone()],
        effects: vec![
            DummyEffect::new(first, 4, true, vec![]).expect("first effect"),
            DummyEffect::new(second, 4, true, vec![]).expect("second effect"),
        ],
        weights: OracleWeights::Unit,
        clusters: vec![cluster.iter().map(|value| u64::from(*value)).collect()],
    })
    .expect("disconnected explicit cluster oracle");
    assert_eq!(
        production.absorbed_dof().absorbed_rank,
        oracle.explicit_dummy_rank
    );
    assert_close_slice(
        production.covariance().values(),
        oracle
            .normalized_reported_intercept_clustered_vce
            .as_ref()
            .expect("reported cluster oracle")
            .as_slice(),
    );

    let repeated = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            &[x],
            &[cluster.clone(), cluster.clone()],
            &[vec![(0..cluster.len())
                .map(|row| (row % 4) as u32)
                .collect()]],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("repeated-FE clustered fit");
    assert_eq!(repeated.absorbed_dof().effects.len(), 2);
    assert_eq!(repeated.absorbed_dof().absorbed_rank, 5);
    assert!(repeated.certificate().passed);
}

#[test]
fn virtual_constant_matches_one_level_dummy_for_cluster_default_and_noconstant() {
    let x = vec![
        -3.0, -1.5, -0.5, 0.5, 1.5, 2.5, 4.0, 6.0, 7.0, 8.5, 9.0, 10.0,
    ];
    let cluster = vec![0_u32, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
    let outcome = x
        .iter()
        .enumerate()
        .map(|(row, value)| 1.4 + 1.2 * value + 0.17 * (row as f64).sin())
        .collect::<Vec<_>>();
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: outcome.clone(),
        regressors: vec![x.clone()],
        effects: vec![
            DummyEffect::new(vec![0; outcome.len()], 1, true, vec![]).expect("one-level dummy")
        ],
        weights: OracleWeights::Unit,
        clusters: vec![cluster.iter().map(|value| u64::from(*value)).collect()],
    })
    .expect("virtual constant cluster oracle");
    for include_intercept in [false, true] {
        let production = fit_prepared_ols_clustered_map(
            &prepare_case(
                &outcome,
                std::slice::from_ref(&x),
                &[],
                &[vec![cluster.clone()]],
                include_intercept,
                CaseWeights::Unit,
            ),
            PreparedOlsClusteredConfig::default(),
        )
        .expect("virtual constant clustered fit");
        assert_eq!(production.absorbed_dof().absorbed_rank, 1);
        assert_eq!(production.absorbed_dof().nested_categories, 0);
        let expected = if include_intercept {
            oracle
                .normalized_reported_intercept_clustered_vce
                .as_ref()
                .expect("reported constant cluster oracle")
        } else {
            oracle.clustered_vce.as_ref().expect("cluster oracle")
        };
        assert_close_slice(production.covariance().values(), expected.as_slice());
    }
}

#[test]
fn paired_multiway_clusters_reduce_to_one_way_without_psd_adjustment() {
    let n = 24;
    let cluster = (0..n).map(|row| (row % 6) as u32).collect::<Vec<_>>();
    let effect = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
    let x = (0..n)
        .map(|row| (0.31 * row as f64).sin() + row as f64 / 17.0)
        .collect::<Vec<_>>();
    let outcome = (0..n)
        .map(|row| {
            1.7 * x[row] + [0.4, -0.5, 0.8, -0.2][effect[row] as usize] + 0.13 * (row as f64).cos()
        })
        .collect::<Vec<_>>();
    let one_way = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            std::slice::from_ref(&x),
            std::slice::from_ref(&effect),
            &[vec![cluster.clone()]],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("one-way paired reference");
    let paired = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            &[x],
            &[effect],
            &[vec![cluster.clone()], vec![cluster]],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("paired multiway fit");
    assert_close_slice_with_tolerance(
        paired.covariance().raw_values(),
        one_way.covariance().raw_values(),
        2e-10,
    );
    assert_close_slice_with_tolerance(
        paired.covariance().values(),
        one_way.covariance().values(),
        2e-10,
    );
    assert_eq!(
        paired.covariance().psd().state,
        PreparedOlsClusteredPsdState::MultiwayRawPsd
    );
    assert!(!paired.covariance().psd().full_adjusted);
    assert!(paired.covariance().psd().strict_final_full_psd);
    assert_eq!(paired.subsets().len(), 3);
}

#[test]
fn intercept_only_clustered_model_has_defined_scalar_covariance_and_no_wald() {
    let cluster = vec![0_u32, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
    let outcome = (0..cluster.len())
        .map(|row| 2.0 + 0.3 * (0.7 * row as f64).sin())
        .collect::<Vec<_>>();
    let production = fit_prepared_ols_clustered_map(
        &prepare_case(
            &outcome,
            &[],
            &[],
            &[vec![cluster.clone()]],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsClusteredConfig::default(),
    )
    .expect("intercept-only clustered fit");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome,
        regressors: vec![],
        effects: vec![DummyEffect::new(vec![0; cluster.len()], 1, true, vec![])
            .expect("one-level intercept dummy")],
        weights: OracleWeights::Unit,
        clusters: vec![cluster.iter().map(|value| u64::from(*value)).collect()],
    })
    .expect("intercept-only explicit oracle");
    assert_eq!(production.covariance().dimension(), 1);
    assert_close_slice(
        production.covariance().values(),
        oracle
            .normalized_reported_intercept_clustered_vce
            .expect("reported intercept oracle")
            .as_slice(),
    );
    assert_eq!(production.statistics().f_statistic, None);
    assert_eq!(
        production.covariance().psd().state,
        PreparedOlsClusteredPsdState::OneWayStrict
    );
    assert!(production.certificate().passed);
}

/// `clusters` is declared dimensions; each dimension contains one or more
/// source columns and therefore permits interaction clusters without changing
/// the multiway dimension count.
fn prepare_case(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effects: &[Vec<u32>],
    clusters: &[Vec<Vec<u32>>],
    include_intercept: bool,
    weights: CaseWeights<'_>,
) -> PreparedOlsClusteredData {
    let (request, loaded) = case_inputs(
        outcome,
        regressors,
        effects,
        clusters,
        include_intercept,
        weights,
    );
    prepare_ols_clustered_data(&request, &loaded).expect("prepare clustered case")
}

fn case_inputs(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effects: &[Vec<u32>],
    clusters: &[Vec<Vec<u32>>],
    include_intercept: bool,
    weights: CaseWeights<'_>,
) -> (OlsClusteredRequestV1, LoadedOlsInput) {
    let cluster_source_count = clusters.iter().map(Vec::len).sum::<usize>();
    let source_count = regressors.len() + effects.len() + cluster_source_count;
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
            stata_var_position: u32::try_from(source_count + 3).expect("weight binding"),
            role,
        });
    }

    let cluster_start = regressors.len() + effects.len();
    let mut next_cluster_source = cluster_start;
    let cluster_terms = clusters
        .iter()
        .enumerate()
        .map(|(dimension, columns)| {
            let sources = (0..columns.len())
                .map(|_| {
                    let source = source_id(next_cluster_source);
                    next_cluster_source += 1;
                    source
                })
                .collect::<Vec<_>>();
            OlsClusterTermV1 {
                label: format!("cluster{}", dimension + 1),
                sources,
            }
        })
        .collect::<Vec<_>>();
    let numeric_sources = regressors
        .iter()
        .cloned()
        .chain(effects.iter().map(|codes| {
            codes
                .iter()
                .map(|code| f64::from(*code))
                .collect::<Vec<_>>()
        }))
        .chain(clusters.iter().flatten().map(|codes| {
            codes
                .iter()
                .map(|code| f64::from(*code))
                .collect::<Vec<_>>()
        }))
        .enumerate()
        .map(|(index, values)| (source_id(index), loaded_column(values)))
        .collect();
    let raw_weight = weight_values(weights);
    (
        OlsClusteredRequestV1 {
            ols: OlsRequestV1 {
                model: CompiledTermGraph { sources, terms },
                absorbed_terms: effects
                    .iter()
                    .enumerate()
                    .map(|(index, _)| AbsorbedTerm {
                        id: AbsorbTermId::new(u32::try_from(index + 1).expect("effect id"))
                            .expect("effect"),
                        intercept: true,
                        categorical_sources: vec![source_id(regressors.len() + index)],
                        slope_sources: vec![],
                        label: format!("g{}", index + 1),
                        saved_name: String::new(),
                    })
                    .collect(),
                bindings,
                include_intercept,
                keep_singletons: true,
            },
            clusters: cluster_terms,
        },
        LoadedOlsInput {
            candidate: vec![true; outcome.len()],
            outcome: loaded_column(outcome.to_vec()),
            numeric_sources,
            analytic_weight: matches!(weights, CaseWeights::Analytic(_))
                .then(|| loaded_column(raw_weight.clone())),
            frequency_weight: matches!(weights, CaseWeights::Frequency(_))
                .then(|| loaded_column(raw_weight.clone())),
            probability_weight: matches!(weights, CaseWeights::Probability(_))
                .then(|| loaded_column(raw_weight)),
        },
    )
}

fn continuous_term(index: usize, source: SourceId) -> ExpandedTerm {
    let one_based = u32::try_from(index + 1).expect("term index");
    ExpandedTerm {
        id: TermId::new(one_based).expect("term"),
        order: TermOrder::new(one_based).expect("order"),
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
    SourceId::new(u32::try_from(index + 1).expect("source index")).expect("source")
}

fn loaded_column(values: Vec<f64>) -> LoadedNumericColumn {
    LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
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
                .expect("field")
                .parse()
                .expect("number")
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
    let (whole, fraction) = mantissa
        .trim_start_matches(['+', '-'])
        .split_once('.')
        .expect("Stata hex mantissa");
    let mut parsed = u64::from_str_radix(whole, 16).expect("whole") as f64;
    let mut scale = 1.0 / 16.0;
    for digit in fraction.bytes() {
        parsed += f64::from(char::from(digit).to_digit(16).expect("digit")) * scale;
        scale /= 16.0;
    }
    let exponent_negative = exponent.starts_with('-');
    let exponent =
        i32::from_str_radix(exponent.trim_start_matches(['+', '-']), 16).expect("exponent");
    let value = parsed
        * 2.0_f64.powi(if exponent_negative {
            -exponent
        } else {
            exponent
        });
    if negative {
        -value
    } else {
        value
    }
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
        CaseWeights::Unit => vec![],
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => values.to_vec(),
        CaseWeights::Frequency(values) => values.iter().map(|value| f64::from(*value)).collect(),
    }
}

fn expand<T: Copy>(values: &[T], frequencies: &[u32]) -> Vec<T> {
    values
        .iter()
        .zip(frequencies)
        .flat_map(|(value, frequency)| std::iter::repeat_n(*value, *frequency as usize))
        .collect()
}

fn assert_close_slice(actual: &[f64], expected: &[f64]) {
    assert_close_slice_with_tolerance(actual, expected, INTERNAL_TOLERANCE);
}

fn assert_close_slice_with_tolerance(actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        let scale = actual.abs().max(expected.abs()).max(1.0);
        assert!(
            (actual - expected).abs() <= tolerance * scale,
            "actual={actual:.17e}, expected={expected:.17e}"
        );
    }
}

fn assert_absolute_close_slice(actual: &[f64], expected: &[f64], cap: f64) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() <= cap,
            "actual={actual:.17e}, expected={expected:.17e}, absolute cap={cap:.3e}"
        );
    }
}
