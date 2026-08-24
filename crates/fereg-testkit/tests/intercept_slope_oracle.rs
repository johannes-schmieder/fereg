use fereg_core::CategoricalTopologyDofPolicy;
use fereg_core::{
    fit_prepared_categorical_ols_unadjusted_map, prepare_categorical_ols_data, AbsorbTermId,
    AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ErrorCode,
    ExpandedTerm, LevelState, LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsInputBinding,
    OlsInputRole, OlsRequestV1, PreparedOlsAbsorbedDofPolicy, PreparedOlsFitStatus,
    PreparedOlsUnadjustedConfig, RawTermKind, SourceEntry, SourceId, TermComponent, TermId,
    TermOrder, TimeSeriesState,
};
use fereg_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const TOLERANCE: f64 = 2e-8;

#[test]
fn slope_only_plus_intercept_case_matches_explicit_dummy_oracle() {
    let (base_y, base_x1, base_firm, base_time, base_worker) = overabsorbed_case();
    let mut y = base_y.clone();
    let mut x1 = base_x1.clone();
    let mut firm = base_firm.clone();
    let mut time = base_time.clone();
    let mut worker = base_worker.clone();
    y.extend_from_slice(&base_y);
    x1.extend_from_slice(&base_x1);
    firm.extend_from_slice(&base_firm);
    time.extend_from_slice(&base_time);
    worker.extend_from_slice(&base_worker);

    let prepared = prepare_case(&y, &x1, &firm, &time, &worker, false);
    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("certified slope-only plus intercept fit");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: y,
        regressors: vec![x1],
        effects: vec![
            DummyEffect::new(firm, 8, false, vec![time]).expect("firm slope-only"),
            DummyEffect::new(worker, 12, true, Vec::new()).expect("worker intercept"),
        ],
        weights: OracleWeights::Unit,
        clusters: Vec::new(),
    })
    .expect("independent slope-only plus intercept oracle");
    assert_eq!(result.fit().finite_rank(), oracle.finite_regressor_rank);
    assert_slice_close(result.fit().residuals(), &oracle.residuals);
    assert_close(result.statistics().rss, oracle.weighted_rss);
}

#[test]
fn multiple_slope_only_terms_match_explicit_dummy_oracle_with_positive_dof() {
    let nobs = 60_usize;
    let firm = (0..nobs)
        .map(|row| u32::try_from(row % 5).expect("firm code"))
        .collect::<Vec<_>>();
    let worker = (0..nobs)
        .map(|row| u32::try_from((row / 5) % 6).expect("worker code"))
        .collect::<Vec<_>>();
    let time = (0..nobs)
        .map(|row| (row % 7) as f64 - 3.0)
        .collect::<Vec<_>>();
    let x2 = (0..nobs)
        .map(|row| ((row * 11 + 1) % 23) as f64 / 4.0 - 2.5)
        .collect::<Vec<_>>();
    let x1 = (0..nobs)
        .map(|row| ((row * 5 + 2) % 19) as f64 / 3.0 - 3.0)
        .collect::<Vec<_>>();
    let y = (0..nobs)
        .map(|row| {
            let firm_slope = 0.08 * (f64::from(firm[row]) + 1.0);
            let worker_time = -0.05 * (f64::from(worker[row]) + 1.0);
            let worker_x2 = 0.03 * (f64::from(worker[row]) + 2.0);
            let noise = ((row * 13 + 4) % 17) as f64 / 100.0 - 0.08;
            1.5 + 0.7 * x1[row]
                + (firm_slope + worker_time) * time[row]
                + worker_x2 * x2[row]
                + noise
        })
        .collect::<Vec<_>>();

    let prepared = prepare_multiple_slope_only_case(&y, &x1, &firm, &time, &worker, &x2);
    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("certified positive-DoF multiple slope-only fit");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: y.clone(),
        regressors: vec![x1],
        effects: vec![
            DummyEffect::new(firm, 5, false, vec![time.clone()]).expect("firm slope-only"),
            DummyEffect::new(worker, 6, false, vec![time, x2]).expect("worker multiple slopes"),
        ],
        weights: OracleWeights::Unit,
        clusters: Vec::new(),
    })
    .expect("independent multiple slope-only explicit-dummy oracle");

    assert!(result.fit().absorption_report().certified());
    assert!(result.fit().residual_absorption_report().certified());
    assert!(result.fit().rank_certificate().passed);
    assert!(result.fit().score_certificate().passed);
    assert!(result.fit().bread().certificate().passed);
    assert!(result.certificate().passed);
    assert!(!prepared.structural_intercept());
    assert!(!prepared.report_intercept());
    assert_eq!(result.fit().finite_rank(), 1);
    assert_eq!(oracle.finite_regressor_rank, 1);
    for (production, independent) in result.fit().coefficients().iter().zip(&oracle.coefficients) {
        assert_eq!(production.status, PreparedOlsFitStatus::Kept);
        assert_close(
            production.value.expect("production coefficient"),
            independent.expect("oracle coefficient"),
        );
    }
    assert_slice_close(result.fit().residuals(), &oracle.residuals);
    assert_slice_close(result.fit().bread().values(), oracle.bread.as_slice());
    assert_close(result.statistics().rss, oracle.weighted_rss);

    let absorbed = result.absorbed_dof();
    assert_eq!(absorbed.effects.len(), 3);
    assert_eq!(absorbed.initial_categories, 17);
    assert_eq!(absorbed.redundant_categories, 0);
    assert_eq!(absorbed.absorbed_rank, 17);
    assert!(absorbed
        .effects
        .iter()
        .all(|row| row.slope_source.is_some()));
    assert!(absorbed
        .effects
        .iter()
        .all(|row| !row.redundancy_exact && !row.nested));
    assert!(absorbed.is_approximate);
    let dof = result.dof();
    assert_eq!(dof.finite_rank, 1);
    assert_eq!(dof.absorbed_rank, 17);
    assert_eq!(dof.model_rank, 18);
    assert_eq!(dof.residual_dof, 42);
    assert!(!dof.has_structural_intercept);
    assert!(dof.absorbed_rank_is_approximate);
}

#[test]
fn positive_dof_intercept_slope_case_matches_explicit_dummy_oracle() {
    let (base_y, base_x1, base_firm, base_time, base_worker) = overabsorbed_case();
    let mut y = base_y.clone();
    let mut x1 = base_x1.clone();
    let mut firm = base_firm.clone();
    let mut time = base_time.clone();
    let mut worker = base_worker.clone();
    y.extend_from_slice(&base_y);
    x1.extend_from_slice(&base_x1);
    firm.extend_from_slice(&base_firm);
    time.extend_from_slice(&base_time);
    worker.extend_from_slice(&base_worker);

    let prepared = prepare_case(&y, &x1, &firm, &time, &worker, true);
    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("certified positive-DoF intercept-slope fit");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: y.clone(),
        regressors: vec![x1.clone()],
        effects: vec![
            DummyEffect::new(firm, 8, true, vec![time]).expect("firm intercept and slope"),
            DummyEffect::new(worker, 12, true, Vec::new()).expect("worker intercept"),
        ],
        weights: OracleWeights::Unit,
        clusters: Vec::new(),
    })
    .expect("independent positive-DoF explicit-dummy oracle");

    assert!(result.fit().absorption_report().certified());
    assert!(result.fit().residual_absorption_report().certified());
    assert!(result.fit().rank_certificate().passed);
    assert!(result.fit().score_certificate().passed);
    assert!(result.fit().bread().certificate().passed);
    assert!(result.certificate().passed);
    assert_eq!(result.fit().finite_rank(), 0);
    assert_eq!(
        result.fit().coefficients()[0].status,
        PreparedOlsFitStatus::CollinearWithFixedEffects
    );
    assert_eq!(
        result.fit().coefficients()[1].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_eq!(oracle.explicit_dummy_columns, 28);
    assert_eq!(oracle.explicit_dummy_rank, 22);
    assert_eq!(oracle.finite_regressor_rank, 0);
    assert_eq!(oracle.model_rank, 22);
    assert_eq!(oracle.residual_degrees_of_freedom, 26);
    assert!(oracle.coefficients[0].is_none());
    let mean_y = y.iter().sum::<f64>() / y.len() as f64;
    assert_close(
        result.fit().coefficients()[1].value.expect("intercept"),
        mean_y,
    );
    assert_close(oracle.weighted_rss, 0.04);
    assert_close(result.statistics().rss, oracle.weighted_rss);
    assert_slice_close(result.fit().residuals(), &oracle.residuals);

    let absorbed = result.absorbed_dof();
    assert_eq!(absorbed.effects.len(), 3);
    assert_eq!(absorbed.initial_categories, 28);
    assert_eq!(absorbed.redundant_categories, 2);
    assert_eq!(absorbed.absorbed_rank, 26);
    assert_eq!(
        absorbed
            .effects
            .iter()
            .map(|row| (row.categories, row.redundant, row.coefficients))
            .collect::<Vec<_>>(),
        vec![(8, 0, 8), (8, 0, 8), (12, 2, 10)]
    );
    assert!(absorbed.effects[0].slope_source.is_none());
    assert!(absorbed.effects[1].slope_source.is_some());
    assert!(absorbed.effects[2].slope_source.is_none());
    assert!(!absorbed.effects[1].redundancy_exact);
    assert!(absorbed.is_approximate);
    let dof = result.dof();
    assert_eq!(dof.finite_rank, 0);
    assert_eq!(dof.absorbed_rank, 26);
    assert_eq!(dof.model_rank, 26);
    assert_eq!(dof.residual_dof, 22);
    assert!(dof.absorbed_rank_is_approximate);

    let expected_bread = [1.0 / y.len() as f64];
    assert_slice_close(result.fit().bread().values(), &expected_bread);
    let reference_scale = oracle.weighted_rss / dof.residual_dof as f64;
    let expected_covariance = expected_bread.map(|value| reference_scale * value);
    assert_slice_close(result.covariance().values(), &expected_covariance);
    assert!(result.covariance().values()[0] >= 0.0);
    assert_close(
        result.statistics().root_mean_squared_error,
        reference_scale.sqrt(),
    );
}

#[test]
fn continuous_dof_counts_constant_intercept_slopes_and_zero_slope_only_levels() {
    let (base_y, base_x1, base_firm, base_time, base_worker) = overabsorbed_case();
    let mut y = base_y.clone();
    let mut x1 = base_x1.clone();
    let mut firm = base_firm.clone();
    let mut worker = base_worker.clone();
    y.extend_from_slice(&base_y);
    x1.extend_from_slice(&base_x1);
    firm.extend_from_slice(&base_firm);
    worker.extend_from_slice(&base_worker);

    let mut constant_time = base_time.clone();
    constant_time.extend_from_slice(&base_time);
    for (value, code) in constant_time.iter_mut().zip(&firm) {
        if *code == 0 {
            *value = 5.0;
        }
    }
    let intercept = prepare_case(&y, &x1, &firm, &constant_time, &worker, true);
    let report = intercept
        .absorbed_dof(CategoricalTopologyDofPolicy::Continuous, &[])
        .expect("continuous intercept-slope DoF");
    assert_eq!(report.initial_categories, 28);
    assert_eq!(report.redundant_categories, 2);
    assert_eq!(report.absorbed_rank, 26);
    assert_eq!(
        report
            .effects
            .iter()
            .map(|row| (row.categories, row.redundant, row.coefficients))
            .collect::<Vec<_>>(),
        vec![(8, 0, 8), (8, 1, 7), (12, 1, 11)]
    );

    let mut zero_time = base_time.clone();
    zero_time.extend_from_slice(&base_time);
    for (value, code) in zero_time.iter_mut().zip(&firm) {
        if *code == 0 {
            *value = 0.0;
        }
    }
    let slope_only = prepare_case(&y, &x1, &firm, &zero_time, &worker, false);
    let report = slope_only
        .absorbed_dof(CategoricalTopologyDofPolicy::Continuous, &[])
        .expect("continuous slope-only DoF");
    assert_eq!(report.initial_categories, 20);
    assert_eq!(report.redundant_categories, 1);
    assert_eq!(report.absorbed_rank, 19);
    assert_eq!(report.effects[0].redundant, 1);
    assert_eq!(report.effects[1].redundant, 0);
}

#[test]
fn positive_intercept_slope_case_has_positive_continuous_residual_dof() {
    let (base_y, base_x1, base_firm, base_time, base_worker) = overabsorbed_case();
    let mut y = base_y.clone();
    let mut x1 = base_x1.clone();
    let mut firm = base_firm.clone();
    let mut time = base_time.clone();
    let mut worker = base_worker.clone();
    y.extend_from_slice(&base_y);
    x1.extend_from_slice(&base_x1);
    firm.extend_from_slice(&base_firm);
    time.extend_from_slice(&base_time);
    worker.extend_from_slice(&base_worker);
    let prepared = prepare_case(&y, &x1, &firm, &time, &worker, true);
    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig {
            absorbed_dof: PreparedOlsAbsorbedDofPolicy::Continuous,
            ..PreparedOlsUnadjustedConfig::default()
        },
    )
    .expect("continuous DoF fit remains positive");
    assert_eq!(result.absorbed_dof().absorbed_rank, 27);
    assert_eq!(result.dof().residual_dof, 21);
    assert!(result
        .covariance()
        .values()
        .iter()
        .all(|value| *value >= 0.0));
}

#[test]
fn old_overabsorbed_intercept_slope_case_fails_before_inference_publication() {
    let (y, x1, firm, time, worker) = overabsorbed_case();
    let prepared = prepare_case(&y, &x1, &firm, &time, &worker, true);
    let error = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect_err("nonpositive conservative residual DoF must fail closed");
    assert_eq!(error.code, ErrorCode::InsufficientObservations);
    assert!(error
        .message
        .contains("no positive residual degrees of freedom"));

    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: y,
        regressors: vec![x1],
        effects: vec![
            DummyEffect::new(firm, 8, true, vec![time]).expect("firm intercept and slope"),
            DummyEffect::new(worker, 12, true, Vec::new()).expect("worker intercept"),
        ],
        weights: OracleWeights::Unit,
        clusters: Vec::new(),
    })
    .expect("independent explicit-dummy oracle");
    assert_eq!(oracle.explicit_dummy_columns, 28);
    assert_eq!(oracle.explicit_dummy_rank, 22);
    assert_eq!(oracle.finite_regressor_rank, 0);
    assert_eq!(oracle.model_rank, 22);
    assert_eq!(oracle.residual_degrees_of_freedom, 2);
    assert!(oracle.coefficients[0].is_none());
    assert_close(oracle.weighted_rss, 0.02);
}

#[allow(clippy::type_complexity)]
fn overabsorbed_case() -> (Vec<f64>, Vec<f64>, Vec<u32>, Vec<f64>, Vec<u32>) {
    let y = vec![
        4.20, 4.95, 5.10, 5.65, 5.90, 6.55, 6.70, 7.35, 7.60, 8.05, 8.50, 8.85, 3.80, 4.45, 4.90,
        5.25, 5.80, 6.15, 6.60, 6.95, 7.40, 7.85, 8.20, 8.75,
    ];
    let x1 = vec![
        -1.50, -1.10, -0.80, -0.50, -0.20, 0.10, 0.40, 0.70, 1.00, 1.30, 1.60, 1.90, -1.40, -1.00,
        -0.70, -0.40, -0.10, 0.20, 0.50, 0.80, 1.10, 1.40, 1.70, 2.00,
    ];
    let firm = vec![
        0_u32, 1, 0, 1, 1, 2, 2, 3, 3, 0, 0, 1, 4, 5, 4, 5, 5, 6, 6, 7, 7, 4, 4, 5,
    ];
    let worker = vec![
        0_u32, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11,
    ];
    let time = vec![
        0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0, 0.0, 1.0,
        2.0, 3.0, 0.0, 1.0, 2.0, 3.0,
    ];
    (y, x1, firm, time, worker)
}

fn prepare_case(
    y: &[f64],
    x1: &[f64],
    firm: &[u32],
    time: &[f64],
    worker: &[u32],
    firm_intercept: bool,
) -> fereg_core::PreparedCategoricalOlsData {
    let sources = ["x1", "firm", "time", "worker"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| SourceEntry {
            id: source(index),
            canonical_name: name.to_owned(),
        })
        .collect();
    let model = CompiledTermGraph {
        sources,
        terms: vec![ExpandedTerm {
            id: TermId::new(1).expect("term"),
            order: TermOrder::new(1).expect("order"),
            coefficient_slot: CoefficientSlot::new(1).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: "c.x1".to_owned(),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source: source(0),
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.0,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".to_owned(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        }],
    };
    let bindings = vec![
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
            role: OlsInputRole::NumericSource(source(0)),
        },
        OlsInputBinding {
            stata_var_position: 4,
            role: OlsInputRole::NumericSource(source(1)),
        },
        OlsInputBinding {
            stata_var_position: 5,
            role: OlsInputRole::NumericSource(source(2)),
        },
        OlsInputBinding {
            stata_var_position: 6,
            role: OlsInputRole::NumericSource(source(3)),
        },
    ];
    let request = OlsRequestV1 {
        model,
        absorbed_terms: vec![
            AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("firm term"),
                intercept: firm_intercept,
                categorical_sources: vec![source(1)],
                slope_sources: vec![source(2)],
                label: if firm_intercept {
                    "firm##c.time".to_owned()
                } else {
                    "firm#c.time".to_owned()
                },
                saved_name: String::new(),
            },
            AbsorbedTerm {
                id: AbsorbTermId::new(2).expect("worker term"),
                intercept: true,
                categorical_sources: vec![source(3)],
                slope_sources: Vec::new(),
                label: "worker".to_owned(),
                saved_name: String::new(),
            },
        ],
        bindings,
        include_intercept: true,
        keep_singletons: false,
    };
    let loaded = LoadedOlsInput {
        candidate: vec![true; y.len()],
        outcome: column(y.iter().copied()),
        numeric_sources: vec![
            (source(0), column(x1.iter().copied())),
            (source(1), column(firm.iter().copied().map(f64::from))),
            (source(2), column(time.iter().copied())),
            (source(3), column(worker.iter().copied().map(f64::from))),
        ],
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    prepare_categorical_ols_data(&request, &loaded).expect("prepared frozen case")
}

fn prepare_multiple_slope_only_case(
    y: &[f64],
    x1: &[f64],
    firm: &[u32],
    time: &[f64],
    worker: &[u32],
    x2: &[f64],
) -> fereg_core::PreparedCategoricalOlsData {
    let sources = ["x1", "firm", "time", "worker", "x2"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| SourceEntry {
            id: source(index),
            canonical_name: name.to_owned(),
        })
        .collect();
    let model = CompiledTermGraph {
        sources,
        terms: vec![ExpandedTerm {
            id: TermId::new(1).expect("term"),
            order: TermOrder::new(1).expect("order"),
            coefficient_slot: CoefficientSlot::new(1).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: "c.x1".to_owned(),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source: source(0),
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.0,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".to_owned(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        }],
    };
    let bindings = (1..=7)
        .zip([
            OlsInputRole::Candidate,
            OlsInputRole::Outcome,
            OlsInputRole::NumericSource(source(0)),
            OlsInputRole::NumericSource(source(1)),
            OlsInputRole::NumericSource(source(2)),
            OlsInputRole::NumericSource(source(3)),
            OlsInputRole::NumericSource(source(4)),
        ])
        .map(|(stata_var_position, role)| OlsInputBinding {
            stata_var_position,
            role,
        })
        .collect();
    let request = OlsRequestV1 {
        model,
        absorbed_terms: vec![
            AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("firm term"),
                intercept: false,
                categorical_sources: vec![source(1)],
                slope_sources: vec![source(2)],
                label: "firm#c.time".to_owned(),
                saved_name: String::new(),
            },
            AbsorbedTerm {
                id: AbsorbTermId::new(2).expect("worker term"),
                intercept: false,
                categorical_sources: vec![source(3)],
                slope_sources: vec![source(2), source(4)],
                label: "worker#c.(time x2)".to_owned(),
                saved_name: String::new(),
            },
        ],
        bindings,
        include_intercept: true,
        keep_singletons: false,
    };
    let loaded = LoadedOlsInput {
        candidate: vec![true; y.len()],
        outcome: column(y.iter().copied()),
        numeric_sources: vec![
            (source(0), column(x1.iter().copied())),
            (source(1), column(firm.iter().copied().map(f64::from))),
            (source(2), column(time.iter().copied())),
            (source(3), column(worker.iter().copied().map(f64::from))),
            (source(4), column(x2.iter().copied())),
        ],
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    prepare_categorical_ols_data(&request, &loaded).expect("prepared multiple slope-only case")
}

fn source(index: usize) -> SourceId {
    SourceId::new(u32::try_from(index + 1).expect("small source")).expect("source")
}

fn column(values: impl Iterator<Item = f64>) -> LoadedNumericColumn {
    let values = values.collect::<Vec<_>>();
    LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    }
}

fn assert_close(actual: f64, expected: f64) {
    let scale = actual.abs().max(expected.abs()).max(1.0);
    assert!(
        (actual - expected).abs() <= TOLERANCE * scale,
        "actual={actual:.17e}, expected={expected:.17e}"
    );
}

fn assert_slice_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (&actual, &expected) in actual.iter().zip(expected) {
        assert_close(actual, expected);
    }
}
