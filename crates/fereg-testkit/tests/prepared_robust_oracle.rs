use fereg_core::{
    fit_prepared_ols_robust_map, prepare_ols_data, AbsorbTermId, AbsorbedTerm, CoefficientSlot,
    CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm, LevelState,
    LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsInputBinding, OlsInputRole, OlsRequestV1,
    PreparedOlsAbsorbedDofPolicy, PreparedOlsData, PreparedOlsRobustConfig, RawTermKind,
    SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};
use fereg_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const COVARIANCE_TOLERANCE: f64 = 8e-7;

#[derive(Clone, Copy)]
enum CaseWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
    Frequency(&'a [u32]),
}

#[test]
fn prepared_hc1_matches_independent_explicit_dummy_oracle_for_all_weight_rules() {
    let first = vec![0_u32, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2];
    let second = vec![0_u32, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1];
    let regressors = vec![
        vec![
            -2.0, -0.5, 1.0, 3.0, -1.0, 0.25, 2.0, 4.0, -3.0, 0.75, 2.5, 5.0,
        ],
        vec![
            1.0, -2.0, 0.5, 4.0, 2.0, -1.0, 3.5, 0.0, -0.5, 2.5, 1.5, 5.0,
        ],
    ];
    let outcome = (0..first.len())
        .map(|row| {
            1.7 * regressors[0][row] - 0.6 * regressors[1][row]
                + [0.5, -1.0, 2.0][first[row] as usize]
                + [0.2, -0.4][second[row] as usize]
                + 0.17 * (row as f64).sin()
        })
        .collect::<Vec<_>>();
    let weights = [0.5, 2.0, 1.25, 3.0, 0.75, 4.0, 1.5, 2.5, 5.0, 0.8, 3.5, 1.1];
    let frequencies = [1_u32, 2, 3, 1, 4, 2, 1, 3, 2, 5, 1, 2];
    for weights in [
        CaseWeights::Unit,
        CaseWeights::Analytic(&weights),
        CaseWeights::Probability(&weights),
        CaseWeights::Frequency(&frequencies),
    ] {
        assert_oracle_case(
            &outcome,
            &regressors,
            &[first.clone(), second.clone()],
            true,
            weights,
        );
        assert_oracle_case(
            &outcome,
            &regressors,
            &[first.clone(), second.clone()],
            false,
            weights,
        );
    }
}

#[test]
fn randomized_row_permuted_hc1_matches_explicit_dummy_ground_truth() {
    let mut generator = Generator(0x4af3_9b21_62c8_d705);
    for replication in 0..12 {
        let n = 30;
        let first = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
        let second = (0..n)
            .map(|row| ((row * 7 + row / 4) % 3) as u32)
            .collect::<Vec<_>>();
        let x1 = (0..n)
            .map(|_| generator.value(-3.0, 3.0))
            .collect::<Vec<_>>();
        let x2 = (0..n)
            .map(|row| 0.15 * x1[row] + generator.value(-2.0, 2.0))
            .collect::<Vec<_>>();
        let weights = (0..n)
            .map(|_| generator.value(0.2, 5.0))
            .collect::<Vec<_>>();
        let outcome = (0..n)
            .map(|row| {
                1.8 * x1[row] - 0.7 * x2[row]
                    + [0.2, -0.8, 1.1, 2.0][first[row] as usize]
                    + [0.5, -0.4, 0.9][second[row] as usize]
                    + generator.value(-0.3, 0.3)
            })
            .collect::<Vec<_>>();
        let permutation = permutation(n, &mut generator);
        let regressors = vec![permute(&x1, &permutation), permute(&x2, &permutation)];
        let effects = vec![
            permute(&first, &permutation),
            permute(&second, &permutation),
        ];
        assert_oracle_case(
            &permute(&outcome, &permutation),
            &regressors,
            &effects,
            replication % 2 == 0,
            CaseWeights::Analytic(&permute(&weights, &permutation)),
        );
    }
}

#[test]
fn virtual_constant_matches_one_level_dummy_for_default_and_noconstant() {
    let x = vec![vec![-3.0, -1.5, -0.5, 0.5, 1.5, 2.5, 4.0, 6.0]];
    let outcome = [1.2, 2.4, 2.8, 4.9, 5.1, 7.4, 7.8, 9.7];
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: outcome.to_vec(),
        regressors: x.clone(),
        effects: vec![DummyEffect::new(vec![0; outcome.len()], 1, true, vec![])
            .expect("one-level virtual-constant dummy")],
        weights: OracleWeights::Unit,
        clusters: vec![],
    })
    .expect("virtual-constant explicit oracle");
    assert_eq!(oracle.explicit_dummy_rank, 1);
    assert_eq!(oracle.model_rank, 2);
    assert_eq!(oracle.residual_degrees_of_freedom, outcome.len() - 2);
    for include_intercept in [true, false] {
        let production = fit_prepared_ols_robust_map(
            &prepare_continuous_case(&outcome, &x, &[], include_intercept, CaseWeights::Unit),
            PreparedOlsRobustConfig::default(),
        )
        .expect("virtual-constant prepared robust fit");
        assert_eq!(production.dof().absorbed_rank, 1);
        assert_eq!(production.dof().model_rank, oracle.model_rank);
        assert_eq!(
            production.dof().residual_dof,
            oracle.residual_degrees_of_freedom
        );
        let expected = if include_intercept {
            oracle
                .normalized_reported_intercept_hc1_vce
                .as_ref()
                .expect("oracle reported virtual constant")
        } else {
            &oracle.hc1_vce
        };
        assert_slice_close(
            production.covariance().values(),
            expected.as_slice(),
            COVARIANCE_TOLERANCE,
        );
        assert!(production.certificate().passed);
    }
}

#[test]
fn disconnected_two_way_fe_matches_explicit_dummy_rank_and_full_hc1() {
    let first = vec![0_u32, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3];
    let second = vec![0_u32, 1, 0, 1, 0, 1, 0, 1, 2, 3, 2, 3, 2, 3, 2, 3];
    let x = vec![vec![
        -3.0, -1.0, 0.5, 2.0, -2.5, -0.25, 1.0, 3.5, -4.0, -1.5, 1.5, 4.0, -3.5, -0.5, 2.5, 5.0,
    ]];
    let outcome = (0..first.len())
        .map(|row| {
            1.6 * x[0][row]
                + [0.5, -0.8, 1.4, -1.2][first[row] as usize]
                + [0.3, -0.4, 0.9, -0.6][second[row] as usize]
                + 0.19 * (row as f64).sin()
        })
        .collect::<Vec<_>>();
    let production = fit_prepared_ols_robust_map(
        &prepare_continuous_case(
            &outcome,
            &x,
            &[first.clone(), second.clone()],
            true,
            CaseWeights::Unit,
        ),
        PreparedOlsRobustConfig::default(),
    )
    .expect("disconnected prepared robust fit");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome,
        regressors: x,
        effects: vec![
            DummyEffect::new(first, 4, true, vec![]).expect("first disconnected effect"),
            DummyEffect::new(second, 4, true, vec![]).expect("second disconnected effect"),
        ],
        weights: OracleWeights::Unit,
        clusters: vec![],
    })
    .expect("disconnected explicit oracle");
    assert_eq!(oracle.explicit_dummy_rank, 6);
    assert_eq!(production.dof().absorbed_rank, oracle.explicit_dummy_rank);
    assert_eq!(production.dof().model_rank, oracle.model_rank);
    assert_eq!(
        production.dof().residual_dof,
        oracle.residual_degrees_of_freedom
    );
    assert_slice_close(
        production.covariance().values(),
        oracle
            .normalized_reported_intercept_hc1_vce
            .as_ref()
            .expect("disconnected reported intercept oracle")
            .as_slice(),
        COVARIANCE_TOLERANCE,
    );
    assert!(production.certificate().passed);
}

#[test]
fn repeated_identical_fe_terms_match_one_span_oracle_and_ranks() {
    let codes = vec![0_u32, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
    let x = vec![
        -3.0, -1.0, 1.0, -2.0, 0.5, 3.0, -4.0, 1.5, 4.5, -2.5, 2.0, 5.0,
    ];
    let outcome = (0..codes.len())
        .map(|row| {
            1.9 * x[row] + [0.4, -0.7, 1.3, -1.1][codes[row] as usize] + 0.13 * (row as f64).cos()
        })
        .collect::<Vec<_>>();
    let production = fit_prepared_ols_robust_map(
        &prepare_repeated_effect_case(&outcome, &x, &codes),
        PreparedOlsRobustConfig::default(),
    )
    .expect("repeated-FE prepared robust fit");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome,
        regressors: vec![x],
        effects: vec![DummyEffect::new(codes, 4, true, vec![]).expect("single-span dummy")],
        weights: OracleWeights::Unit,
        clusters: vec![],
    })
    .expect("single-span repeated-FE oracle");
    assert_eq!(production.absorbed_dof().effects.len(), 2);
    assert_eq!(production.dof().absorbed_rank, 4);
    assert_eq!(production.dof().absorbed_rank, oracle.explicit_dummy_rank);
    assert_eq!(production.dof().model_rank, oracle.model_rank);
    assert_eq!(
        production.dof().residual_dof,
        oracle.residual_degrees_of_freedom
    );
    assert_slice_close(
        production.covariance().values(),
        oracle
            .normalized_reported_intercept_hc1_vce
            .as_ref()
            .expect("repeated-FE reported intercept oracle")
            .as_slice(),
        COVARIANCE_TOLERANCE,
    );
    assert!(production.certificate().passed);
}

#[test]
fn sealed_stata18_one_fe_robust_and_pweight_covariances_match() {
    // Immutable reference receipt:
    // benchmarks/reports/captured/darwin-arm64/stata18/ols/
    // 20260821T030000Z-a5181f9
    // payload sha256 6e861febcedfc8cc76570ee627c2010d4738750c732c600bc2cf0f29e88e8002
    // reghdfe commit 29b2b203f534413369d675d0c0f674d61291f536.
    let core = include_str!("../../../tests/fixtures/ols/ols_core.csv");
    let one_fe = fit_prepared_ols_robust_map(
        &prepare_one_fe_factor_case(core),
        PreparedOlsRobustConfig::default(),
    )
    .expect("sealed one-FE robust inference");
    assert_eq!(one_fe.dof().absorbed_rank, 8);
    assert_eq!(one_fe.dof().finite_model_dof, 3);
    assert_eq!(one_fe.dof().residual_dof, 13);
    let expected_one_fe = [
        "+1.72b3a43070d30X-00b",
        "-1.aacc28464cca4X-00c",
        "-1.661d37ef6b7e8X-00a",
        "+1.5d036d0784774X-00c",
        "-1.aacc28464cca4X-00c",
        "+1.8ed6ca0971b04X-009",
        "+1.9fc6c28fd6b26X-009",
        "-1.27fccae5cd880X-009",
        "-1.661d37ef6b7e8X-00a",
        "+1.9fc6c28fd6b26X-009",
        "+1.ba5c875aca418X-008",
        "-1.977aa4e28c20cX-009",
        "+1.5d036d0784774X-00c",
        "-1.27fccae5cd880X-009",
        "-1.977aa4e28c20cX-009",
        "+1.31b4b891013a3X-009",
    ]
    .map(stata_hex);
    assert_slice_close(one_fe.covariance().values(), &expected_one_fe, 3e-6);

    let x1 = csv_numeric_column(core, "x1");
    let industry = csv_numeric_column(core, "industry");
    let firm = zero_based(&csv_u32_column(core, "firm"));
    let factor_regressors = vec![
        x1,
        industry
            .iter()
            .map(|level| f64::from(*level == 2.0))
            .collect(),
        industry
            .iter()
            .map(|level| f64::from(*level == 3.0))
            .collect(),
    ];
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: csv_numeric_column(core, "y"),
        regressors: factor_regressors,
        effects: vec![DummyEffect::new(firm, 8, true, vec![]).expect("firm dummy")],
        weights: OracleWeights::Unit,
        clusters: vec![],
    })
    .expect("explicit one-FE robust oracle");
    assert_slice_close(
        one_fe.covariance().values(),
        oracle
            .normalized_reported_intercept_hc1_vce
            .as_ref()
            .expect("oracle normalized intercept")
            .as_slice(),
        COVARIANCE_TOLERANCE,
    );

    let weighted = include_str!("../../../tests/fixtures/ols/ols_weights.csv");
    let outcome = csv_numeric_column(weighted, "y");
    let x = csv_numeric_column(weighted, "x");
    let groups = zero_based(&csv_u32_column(weighted, "group"));
    let pweights = csv_numeric_column(weighted, "pw_valid");
    let pweight = fit_prepared_ols_robust_map(
        &prepare_continuous_case(
            &outcome,
            std::slice::from_ref(&x),
            std::slice::from_ref(&groups),
            true,
            CaseWeights::Probability(&pweights),
        ),
        PreparedOlsRobustConfig::default(),
    )
    .expect("sealed pweight robust inference");
    assert_eq!(pweight.weights().physical_observations, 12);
    assert_eq!(pweight.weights().effective_observations, 12);
    assert_eq!(pweight.dof().residual_dof, 5);
    assert_close(
        pweight.weights().raw_weight_sum.expect("raw pweight sum"),
        pweights.iter().sum(),
        1e-12,
    );
    let expected_pweight = [
        "+1.464d18a4af078X-005",
        "-1.4e42e16b3d1d8X-006",
        "-1.4e42e16b3d1d8X-006",
        "+1.6ca0cbb6a2a2aX-007",
    ]
    .map(stata_hex);
    assert_slice_close(pweight.covariance().values(), &expected_pweight, 3e-6);
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome,
        regressors: vec![x],
        effects: vec![DummyEffect::new(groups, 6, true, vec![]).expect("group dummy")],
        weights: OracleWeights::Wls(pweights),
        clusters: vec![],
    })
    .expect("explicit pweight robust oracle");
    assert_slice_close(
        pweight.covariance().values(),
        oracle
            .normalized_reported_intercept_hc1_vce
            .as_ref()
            .expect("pweight oracle normalized intercept")
            .as_slice(),
        COVARIANCE_TOLERANCE,
    );
}

#[test]
fn absorbed_dof_policy_changes_only_the_hc1_scalar() {
    let n = 40;
    let first = (0..n).map(|row| (row / 10) as u32).collect::<Vec<_>>();
    let second = (0..n)
        .map(|row| {
            let first_code = first[row];
            if first_code < 2 {
                (row % 2) as u32
            } else {
                2 + (row % 2) as u32
            }
        })
        .collect::<Vec<_>>();
    let third = (0..n).map(|row| (row % 5) as u32).collect::<Vec<_>>();
    let effects = vec![first, second, third];
    let x = (0..n)
        .map(|row| ((row * 17 % 31) as f64 - 15.0) / 4.0)
        .collect::<Vec<_>>();
    let outcome = (0..n)
        .map(|row| {
            1.4 * x[row] + 0.3 * f64::from(effects[0][row]) - 0.2 * f64::from(effects[1][row])
                + 0.5 * f64::from(effects[2][row])
                + 0.11 * (row as f64).cos()
        })
        .collect::<Vec<_>>();
    let prepared = prepare_continuous_case(
        &outcome,
        std::slice::from_ref(&x),
        &effects,
        true,
        CaseWeights::Unit,
    );
    let fit = |policy| {
        fit_prepared_ols_robust_map(
            &prepared,
            PreparedOlsRobustConfig {
                absorbed_dof: policy,
                ..PreparedOlsRobustConfig::default()
            },
        )
        .expect("DoF-policy robust fit")
    };
    let none = fit(PreparedOlsAbsorbedDofPolicy::None);
    let first = fit(PreparedOlsAbsorbedDofPolicy::FirstPair);
    let pairwise = fit(PreparedOlsAbsorbedDofPolicy::Pairwise);
    assert!(
        none.dof().residual_dof != first.dof().residual_dof
            || first.dof().residual_dof != pairwise.dof().residual_dof
    );
    for result in [&first, &pairwise] {
        assert_eq!(none.fit().coefficients(), result.fit().coefficients());
        let none_scale = none.covariance().hc1_correction();
        let result_scale = result.covariance().hc1_correction();
        for (none_value, result_value) in none
            .covariance()
            .values()
            .iter()
            .zip(result.covariance().values())
        {
            assert_close(none_value / none_scale, result_value / result_scale, 2e-8);
        }
    }
}

fn assert_oracle_case(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effects: &[Vec<u32>],
    include_intercept: bool,
    weights: CaseWeights<'_>,
) {
    let production = fit_prepared_ols_robust_map(
        &prepare_continuous_case(outcome, regressors, effects, include_intercept, weights),
        PreparedOlsRobustConfig::default(),
    )
    .expect("prepared robust oracle case");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: outcome.to_vec(),
        regressors: regressors.to_vec(),
        effects: effects
            .iter()
            .map(|codes| {
                DummyEffect::new(codes.clone(), level_count(codes), true, vec![])
                    .expect("oracle dummy")
            })
            .collect(),
        weights: oracle_weights(weights),
        clusters: vec![],
    })
    .expect("independent explicit-dummy robust fit");
    assert_eq!(production.dof().model_rank, oracle.model_rank);
    assert_eq!(
        production.dof().residual_dof,
        oracle.residual_degrees_of_freedom
    );
    let expected = if include_intercept {
        oracle
            .normalized_reported_intercept_hc1_vce
            .as_ref()
            .expect("normalized reported intercept oracle")
    } else {
        &oracle.hc1_vce
    };
    assert_slice_close(
        production.covariance().values(),
        expected.as_slice(),
        COVARIANCE_TOLERANCE,
    );
    assert!(production.certificate().passed);
}

fn prepare_continuous_case(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effects: &[Vec<u32>],
    include_intercept: bool,
    weights: CaseWeights<'_>,
) -> PreparedOlsData {
    let source_count = regressors.len() + effects.len();
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
    let raw_weight = weight_values(weights);
    prepare_ols_data(
        &OlsRequestV1 {
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
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()],
            outcome: loaded_column(outcome.to_vec()),
            numeric_sources: regressors
                .iter()
                .cloned()
                .chain(effects.iter().map(|codes| {
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
        },
    )
    .expect("prepared continuous robust case")
}

fn prepare_one_fe_factor_case(csv: &str) -> PreparedOlsData {
    let outcome = csv_numeric_column(csv, "y");
    let x1 = csv_numeric_column(csv, "x1");
    let industry = csv_numeric_column(csv, "industry");
    let firm = csv_numeric_column(csv, "firm");
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
            canonical_token: format!("{level}.industry"),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
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
                raw_operator: level.to_string(),
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
                id: AbsorbTermId::new(1).expect("firm effect"),
                intercept: true,
                categorical_sources: vec![firm_source],
                slope_sources: vec![],
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
    .expect("prepared one-FE factor robust case")
}

fn prepare_repeated_effect_case(outcome: &[f64], x: &[f64], codes: &[u32]) -> PreparedOlsData {
    let x_source = source_id(0);
    let effect_source = source_id(1);
    let repeated = |term: u32, label: &str| AbsorbedTerm {
        id: AbsorbTermId::new(term).expect("repeated effect id"),
        intercept: true,
        categorical_sources: vec![effect_source],
        slope_sources: vec![],
        label: label.to_owned(),
        saved_name: String::new(),
    };
    prepare_ols_data(
        &OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![
                    SourceEntry {
                        id: x_source,
                        canonical_name: "x".to_owned(),
                    },
                    SourceEntry {
                        id: effect_source,
                        canonical_name: "g".to_owned(),
                    },
                ],
                terms: vec![continuous_term(0, x_source)],
            },
            absorbed_terms: vec![repeated(1, "g_first"), repeated(2, "g_second")],
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
                    role: OlsInputRole::NumericSource(effect_source),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        },
        &LoadedOlsInput {
            candidate: vec![true; outcome.len()],
            outcome: loaded_column(outcome.to_vec()),
            numeric_sources: vec![
                (x_source, loaded_column(x.to_vec())),
                (
                    effect_source,
                    loaded_column(codes.iter().map(|code| f64::from(*code)).collect()),
                ),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
    )
    .expect("prepared repeated-FE robust case")
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

fn zero_based(values: &[u32]) -> Vec<u32> {
    let minimum = values.iter().copied().min().expect("nonempty codes");
    values.iter().map(|value| value - minimum).collect()
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
        CaseWeights::Unit => vec![],
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

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    let scale = actual.abs().max(expected.abs()).max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance * scale,
        "actual={actual:.17e}, expected={expected:.17e}, tolerance={tolerance:.3e}"
    );
}

fn assert_slice_close(actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_close(*actual, *expected, tolerance);
    }
}

fn permute<T: Copy>(values: &[T], permutation: &[usize]) -> Vec<T> {
    permutation.iter().map(|row| values[*row]).collect()
}

fn permutation(n: usize, generator: &mut Generator) -> Vec<usize> {
    let mut values = (0..n).collect::<Vec<_>>();
    for index in (1..n).rev() {
        let swap = (generator.next() as usize) % (index + 1);
        values.swap(index, swap);
    }
    values
}

struct Generator(u64);

impl Generator {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn value(&mut self, lower: f64, upper: f64) -> f64 {
        let unit = (self.next() >> 11) as f64 / ((1_u64 << 53) as f64);
        lower + (upper - lower) * unit
    }
}
