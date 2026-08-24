use stfixest_core::{
    fit_prepared_categorical_ols_unadjusted_map, prepare_categorical_ols_data, AbsorbTermId,
    AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ErrorCode,
    ExpandedTerm, LevelState, LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsInputBinding,
    OlsInputRole, OlsRequestV1, PreparedOlsFitStatus, PreparedOlsUnadjustedConfig, RawTermKind,
    SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};
use stfixest_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const TOLERANCE: f64 = 2e-8;

#[test]
fn positive_dof_intercept_slope_case_matches_explicit_dummy_oracle() {
    let mut y = Vec::with_capacity(48);
    let mut x1 = Vec::with_capacity(48);
    let mut firm = Vec::with_capacity(48);
    let mut worker = Vec::with_capacity(48);
    let mut time = Vec::with_capacity(48);
    for firm_value in 1_u32..=4 {
        for worker_value in 1_u32..=6 {
            for time_value in 0_u32..=1 {
                let x_value = firm_value * worker_value
                    + 2 * worker_value * time_value
                    + firm_value * firm_value;
                let error = i64::from((7 * firm_value + 3 * worker_value + 5 * time_value) % 7) - 3;
                let outcome = 10_i64
                    + 3 * i64::from(firm_value)
                    + (2 * i64::from(firm_value) - 1) * i64::from(time_value)
                    + 2 * i64::from(worker_value)
                    + 3 * i64::from(x_value)
                    + error;
                y.push(outcome as f64);
                x1.push(f64::from(x_value));
                firm.push(firm_value - 1);
                worker.push(worker_value - 1);
                time.push(f64::from(time_value));
            }
        }
    }

    let prepared = prepare_case(&y, &x1, &firm, &time, &worker);
    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("certified positive-DoF intercept-slope fit");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: y.clone(),
        regressors: vec![x1.clone()],
        effects: vec![
            DummyEffect::new(firm, 4, true, vec![time]).expect("firm intercept and slope"),
            DummyEffect::new(worker, 6, true, Vec::new()).expect("worker intercept"),
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
    assert_eq!(result.fit().finite_rank(), 1);
    assert_eq!(
        result.fit().coefficients()[0].status,
        PreparedOlsFitStatus::Kept
    );
    assert_eq!(
        result.fit().coefficients()[1].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_eq!(oracle.explicit_dummy_columns, 14);
    assert_eq!(oracle.explicit_dummy_rank, 13);
    assert_eq!(oracle.finite_regressor_rank, 1);
    assert_eq!(oracle.model_rank, 14);
    assert_eq!(oracle.residual_degrees_of_freedom, 34);
    assert_close(
        result.fit().coefficients()[0]
            .value
            .expect("x1 coefficient"),
        oracle.coefficients[0].expect("oracle x1 coefficient"),
    );
    assert_close(
        oracle.coefficients[0].expect("oracle coefficient"),
        47.0 / 15.0,
    );
    let mean_y = y.iter().sum::<f64>() / y.len() as f64;
    let mean_x = x1.iter().sum::<f64>() / x1.len() as f64;
    assert_close(
        result.fit().coefficients()[1].value.expect("intercept"),
        mean_y - mean_x * oracle.coefficients[0].expect("oracle coefficient"),
    );
    assert_close(oracle.weighted_rss, 1_141.0 / 15.0);
    assert_close(result.statistics().rss, oracle.weighted_rss);
    assert_slice_close(result.fit().residuals(), &oracle.residuals);

    let absorbed = result.absorbed_dof();
    assert_eq!(absorbed.effects.len(), 3);
    assert_eq!(absorbed.initial_categories, 14);
    assert_eq!(absorbed.redundant_categories, 1);
    assert_eq!(absorbed.absorbed_rank, 13);
    assert_eq!(
        absorbed
            .effects
            .iter()
            .map(|row| (row.categories, row.redundant, row.coefficients))
            .collect::<Vec<_>>(),
        vec![(4, 0, 4), (4, 0, 4), (6, 1, 5)]
    );
    assert!(absorbed.effects[0].slope_source.is_none());
    assert!(absorbed.effects[1].slope_source.is_some());
    assert!(absorbed.effects[2].slope_source.is_none());
    assert!(!absorbed.effects[1].redundancy_exact);
    assert!(absorbed.is_approximate);
    let dof = result.dof();
    assert_eq!(dof.finite_rank, 1);
    assert_eq!(dof.absorbed_rank, 13);
    assert_eq!(dof.model_rank, 14);
    assert_eq!(dof.residual_dof, 34);
    assert!(dof.absorbed_rank_is_approximate);

    let finite_bread = oracle.bread[(0, 0)];
    let expected_bread = [
        finite_bread,
        -finite_bread * mean_x,
        -finite_bread * mean_x,
        1.0 / y.len() as f64 + mean_x * mean_x * finite_bread,
    ];
    assert_slice_close(result.fit().bread().values(), &expected_bread);
    let reference_scale = oracle.weighted_rss / dof.residual_dof as f64;
    let expected_covariance = expected_bread.map(|value| reference_scale * value);
    assert_slice_close(result.covariance().values(), &expected_covariance);
    assert!(result.covariance().values()[0] >= 0.0);
    assert!(result.covariance().values()[3] >= 0.0);
    assert_close(
        result.statistics().root_mean_squared_error,
        reference_scale.sqrt(),
    );
}

#[test]
fn old_overabsorbed_intercept_slope_case_fails_before_inference_publication() {
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
    let prepared = prepare_case(&y, &x1, &firm, &time, &worker);
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

fn prepare_case(
    y: &[f64],
    x1: &[f64],
    firm: &[u32],
    time: &[f64],
    worker: &[u32],
) -> stfixest_core::PreparedCategoricalOlsData {
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
                intercept: true,
                categorical_sources: vec![source(1)],
                slope_sources: vec![source(2)],
                label: "firm##c.time".to_owned(),
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
