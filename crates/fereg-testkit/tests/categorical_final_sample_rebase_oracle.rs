use fereg_core::{
    fit_prepared_categorical_ols_unadjusted_map, prepare_categorical_ols_data, AbsorbTermId,
    AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm,
    LevelState, LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsInputBinding, OlsInputRole,
    OlsRequestV1, PreparedOlsFitStatus, PreparedOlsUnadjustedConfig, RawTermKind, SourceEntry,
    SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};
use fereg_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const TOLERANCE: f64 = 3e-8;

#[test]
fn singleton_deleted_factor_level_rebases_and_matches_explicit_dummy_oracle() {
    let y = vec![1.1, 2.5, 3.2, 4.8, 5.1, 7.4, 8.0, 9.7, 10.3, 11.9];
    let x = vec![-1.4, -0.9, -0.2, 0.4, 1.1, -1.0, -0.3, 0.5, 1.3, 2.0];
    let factor = vec![1.0, 2.0, 3.0, 2.0, 3.0, 2.0, 3.0, 2.0, 3.0, 1.0];
    let effect_a = vec![1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0, 6.0];
    let effect_b = vec![1.0; 10];
    let sources = vec![
        source_entry(1, "x"),
        source_entry(2, "f"),
        source_entry(3, "a"),
        source_entry(4, "b"),
    ];
    let terms = vec![
        continuous_term(1, source(1), "x"),
        factor_term(2, source(2), 1.0, true),
        factor_term(3, source(2), 2.0, false),
        factor_term(4, source(2), 3.0, false),
    ];
    let request = OlsRequestV1 {
        model: CompiledTermGraph { sources, terms },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("effect id"),
            intercept: true,
            categorical_sources: vec![source(3), source(4)],
            slope_sources: Vec::new(),
            label: "a#b".to_owned(),
            saved_name: String::new(),
        }],
        bindings: (1_u32..=6)
            .map(|position| OlsInputBinding {
                stata_var_position: position,
                role: match position {
                    1 => OlsInputRole::Candidate,
                    2 => OlsInputRole::Outcome,
                    value => OlsInputRole::NumericSource(source(value - 2)),
                },
            })
            .collect(),
        include_intercept: true,
        keep_singletons: false,
    };
    let loaded = LoadedOlsInput {
        candidate: vec![true; y.len()],
        outcome: loaded_column(y.clone()),
        numeric_sources: vec![
            (source(1), loaded_column(x.clone())),
            (source(2), loaded_column(factor.clone())),
            (source(3), loaded_column(effect_a)),
            (source(4), loaded_column(effect_b)),
        ],
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };

    let prepared = prepare_categorical_ols_data(&request, &loaded)
        .expect("final-sample categorical preparation");
    assert_eq!(prepared.sample().retained_rows(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(prepared.singletons().num_singletons, 2);
    assert_eq!(prepared.report_plan().original_coefficient_count(), 5);
    assert_eq!(
        prepared
            .report_plan()
            .original_slots()
            .iter()
            .map(|slot| slot.get())
            .collect::<Vec<_>>(),
        vec![1, 3, 4, 5]
    );
    assert_eq!(prepared.model().coefficients.len(), 4);
    assert_eq!(prepared.model().columns.len(), 3);

    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("certified rebased iid fit");
    assert!(result.fit().rank_certificate().passed);
    assert!(result.fit().score_certificate().passed);
    assert!(result.certificate().passed);
    assert_eq!(result.fit().coefficients().len(), 4);
    assert_eq!(
        result.fit().coefficients()[0].status,
        PreparedOlsFitStatus::Kept
    );
    assert_eq!(
        result.fit().coefficients()[1].status,
        PreparedOlsFitStatus::SyntaxOmitted
    );
    assert_eq!(
        result.fit().coefficients()[2].status,
        PreparedOlsFitStatus::Kept
    );
    assert_eq!(
        result.fit().coefficients()[3].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );

    let retained_y = y[1..9].to_vec();
    let retained_x = x[1..9].to_vec();
    let retained_factor_three = factor[1..9]
        .iter()
        .map(|value| f64::from(*value == 3.0))
        .collect::<Vec<_>>();
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: retained_y,
        regressors: vec![retained_x, retained_factor_three],
        effects: vec![
            DummyEffect::new(vec![0, 0, 1, 1, 2, 2, 3, 3], 4, true, Vec::new())
                .expect("explicit final interaction effect"),
        ],
        weights: OracleWeights::Unit,
        clusters: Vec::new(),
    })
    .expect("explicit-dummy final-sample oracle");
    assert_eq!(oracle.kept_regressors, vec![0, 1]);
    assert_close(
        result.fit().coefficients()[0].value.expect("x coefficient"),
        oracle.coefficients[0].expect("oracle x coefficient"),
    );
    assert_close(
        result.fit().coefficients()[2]
            .value
            .expect("factor-three coefficient"),
        oracle.coefficients[1].expect("oracle factor-three coefficient"),
    );
    assert_slice_close(result.fit().residuals(), &oracle.residuals);
    assert_eq!(
        result.dof().residual_dof,
        oracle.residual_degrees_of_freedom
    );
    assert_eq!(result.dof().absorbed_rank, oracle.explicit_dummy_rank);

    let covariance = result.covariance();
    assert_eq!(covariance.dimension(), 3);
    for row in 0..2 {
        for column in 0..2 {
            assert_close(
                covariance.values()[row * 3 + column],
                oracle.iid_vce[(row, column)],
            );
        }
    }
}

fn source(value: u32) -> SourceId {
    SourceId::new(value).expect("source id")
}

fn source_entry(value: u32, name: &str) -> SourceEntry {
    SourceEntry {
        id: source(value),
        canonical_name: name.to_owned(),
    }
}

fn continuous_term(value: u32, source: SourceId, name: &str) -> ExpandedTerm {
    term(
        value,
        RawTermKind::Variable,
        name,
        TermComponent {
            order: ComponentOrder::new(1).expect("component order"),
            source,
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.0,
            base: MetadataFlag::False,
            omitted: MetadataFlag::Unavailable,
            raw_operator: "c".to_owned(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        },
        false,
    )
}

fn factor_term(value: u32, source: SourceId, level: f64, base: bool) -> ExpandedTerm {
    term(
        value,
        RawTermKind::Factor,
        &format!("{level}.f"),
        TermComponent {
            order: ComponentOrder::new(1).expect("component order"),
            source,
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
                format!("{level}b")
            } else {
                level.to_string()
            },
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        },
        base,
    )
}

fn term(
    value: u32,
    raw_kind: RawTermKind,
    token: &str,
    component: TermComponent,
    omitted: bool,
) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(value).expect("term id"),
        order: TermOrder::new(value).expect("term order"),
        coefficient_slot: CoefficientSlot::new(value).expect("coefficient slot"),
        raw_kind,
        whole_column_omitted: omitted,
        canonical_token: token.to_owned(),
        components: vec![component],
    }
}

fn loaded_column(values: Vec<f64>) -> LoadedNumericColumn {
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
    for (actual, expected) in actual.iter().copied().zip(expected.iter().copied()) {
        assert_close(actual, expected);
    }
}
