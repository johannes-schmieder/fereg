use fereg_core::{
    fit_prepared_categorical_ols_robust_map, fit_prepared_categorical_ols_unadjusted_map,
    prepare_categorical_ols_data, AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph,
    ComponentKind, ComponentOrder, ExpandedTerm, LevelState, LoadedNumericColumn, LoadedOlsInput,
    MetadataFlag, OlsInputBinding, OlsInputRole, OlsRequestV1, PreparedOlsFitStatus,
    PreparedOlsRobustConfig, PreparedOlsUnadjustedConfig, RawTermKind, SourceEntry, SourceId,
    TermComponent, TermId, TermOrder, TimeSeriesState,
};
use fereg_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const TOLERANCE: f64 = 3e-8;

#[test]
fn single_level_and_omitted_only_recipes_preserve_science_and_original_slots() {
    for omitted in [false, true] {
        for interaction in [false, true] {
            let (mut request, mut loaded, y, x, _) = singleton_fixture();
            request.model.terms.truncate(2);
            request.model.terms[1] = factor_term(2, source(2), 1.0, omitted);
            if interaction {
                let mut component = factor_term(2, source(4), 1.0, true).components.remove(0);
                component.order = ComponentOrder::new(2).expect("component order");
                request.model.terms[1].components.push(component);
                request.model.terms[1].raw_kind = RawTermKind::Interaction;
                request.model.terms[1].canonical_token = "1.f#1b.b".to_owned();
            }
            loaded.numeric_sources[1].1.values.fill(1.0);
            let prepared = prepare_categorical_ols_data(&request, &loaded)
                .expect("original single-level or syntax-omitted graph");
            assert_eq!(prepared.sample().retained_rows(), &[1, 2, 3, 4, 5, 6, 7, 8]);
            assert_eq!(prepared.model().coefficients.len(), 3);
            assert_eq!(prepared.model().coefficients[1].coefficient_slot.get(), 2);
            assert_eq!(
                prepared.model().coefficients[1].status,
                if omitted {
                    fereg_core::PreparedOlsCoefficientStatus::SyntaxOmitted
                } else {
                    fereg_core::PreparedOlsCoefficientStatus::Active
                }
            );
            let oracle = fit_explicit_ols(&ExplicitOlsProblem {
                outcome: y[1..9].to_vec(),
                regressors: vec![x[1..9].to_vec()],
                effects: vec![
                    DummyEffect::new(vec![0, 0, 1, 1, 2, 2, 3, 3], 4, true, Vec::new())
                        .expect("independent final effect"),
                ],
                weights: OracleWeights::Unit,
                clusters: Vec::new(),
            })
            .expect("independent dummy model");
            let fit = fit_prepared_categorical_ols_robust_map(
                &prepared,
                PreparedOlsRobustConfig::default(),
            )
            .expect("certified original-axis robust fit");
            assert_eq!(fit.fit().finite_rank(), 1);
            assert_eq!(fit.dof().residual_dof, oracle.residual_degrees_of_freedom);
            assert_close(
                fit.fit().coefficients()[0].value.expect("identified slope"),
                oracle.coefficients[0].expect("oracle slope"),
            );
            assert_slice_close(fit.fit().residuals(), &oracle.residuals);
            let position = fit
                .covariance_coordinates()
                .iter()
                .position(|coordinate| coordinate.coefficient_slot.get() == 1)
                .expect("slope covariance coordinate");
            let width = fit.covariance_coordinates().len();
            assert_close(
                fit.covariance().values()[position * width + position],
                oracle.hc1_vce[(0, 0)],
            );

            // Even a wholly omitted factor still undergoes original available-
            // sample domain checks. A candidate exclusion alone removes it.
            loaded.numeric_sources[1].1.values[2] = -1.0;
            assert_eq!(
                prepare_categorical_ols_data(&request, &loaded)
                    .expect_err("eligible invalid factor")
                    .code,
                fereg_core::ErrorCode::InvalidFactor
            );
            loaded.candidate[2] = false;
            prepare_categorical_ols_data(&request, &loaded)
                .expect("excluded invalid factor is not a domain failure");
        }
    }
}

fn singleton_fixture() -> (OlsRequestV1, LoadedOlsInput, Vec<f64>, Vec<f64>, Vec<f64>) {
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

    (request, loaded, y, x, factor)
}

#[test]
fn singleton_deleted_base_preserves_original_axis_and_identified_contrasts() {
    let (request, loaded, y, x, factor) = singleton_fixture();
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
        vec![1, 2, 3, 4, 5]
    );
    assert_eq!(prepared.model().coefficients.len(), 5);
    assert_eq!(prepared.model().columns.len(), 4);
    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("certified original-axis iid fit");
    assert!(result.fit().rank_certificate().passed);
    assert!(result.fit().score_certificate().passed);
    assert!(result.certificate().passed);
    assert_eq!(result.fit().coefficients().len(), 5);
    assert_eq!(
        result.fit().coefficients()[0].status,
        PreparedOlsFitStatus::Kept
    );
    assert_eq!(
        result.fit().coefficients()[1].status,
        PreparedOlsFitStatus::SyntaxOmitted
    );
    assert_eq!(
        result.fit().coefficients()[4].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_eq!(result.fit().finite_rank(), 2);

    let retained_y = y[1..9].to_vec();
    let retained_x = x[1..9].to_vec();
    let retained_factor_two = factor[1..9]
        .iter()
        .map(|value| f64::from(*value == 2.0))
        .collect::<Vec<_>>();
    let retained_factor_three = factor[1..9]
        .iter()
        .map(|value| f64::from(*value == 3.0))
        .collect::<Vec<_>>();
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: retained_y,
        regressors: vec![retained_x, retained_factor_two, retained_factor_three],
        effects: vec![
            DummyEffect::new(vec![0, 0, 1, 1, 2, 2, 3, 3], 4, true, Vec::new())
                .expect("explicit final interaction effect"),
        ],
        weights: OracleWeights::Unit,
        clusters: Vec::new(),
    })
    .expect("explicit-dummy final-sample oracle");
    assert_eq!(oracle.finite_regressor_rank, 2);
    let values = result
        .fit()
        .coefficients()
        .iter()
        .map(|coefficient| coefficient.value.unwrap_or(0.0))
        .collect::<Vec<_>>();
    assert_close(values[0], oracle.coefficients[0].expect("oracle x"));
    assert_close(
        values[3] - values[2],
        oracle.coefficients[2].unwrap_or(0.0) - oracle.coefficients[1].unwrap_or(0.0),
    );
    assert_slice_close(result.fit().residuals(), &oracle.residuals);
    assert_eq!(
        result.dof().residual_dof,
        oracle.residual_degrees_of_freedom
    );
    assert_eq!(result.dof().absorbed_rank, oracle.explicit_dummy_rank);

    check_contrast_covariance(
        result.covariance().values(),
        result.covariance_coordinates(),
        oracle.iid_vce.as_slice(),
        &oracle.kept_regressors,
    );
    let robust =
        fit_prepared_categorical_ols_robust_map(&prepared, PreparedOlsRobustConfig::default())
            .expect("certified original-axis robust fit");
    assert_slice_close(robust.fit().residuals(), &oracle.residuals);
    check_contrast_covariance(
        robust.covariance().values(),
        robust.covariance_coordinates(),
        oracle.hc1_vce.as_slice(),
        &oracle.kept_regressors,
    );
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

// The two fits can choose different representatives of d2+d3=1. Compare
// covariance only on the identified x and d3-d2 contrasts.
fn check_contrast_covariance(
    values: &[f64],
    coordinates: &[fereg_core::PreparedOlsBreadCoordinate],
    oracle: &[f64],
    kept: &[usize],
) {
    let weights = coordinates
        .iter()
        .map(|coordinate| match coordinate.coefficient_slot.get() {
            1 => [1.0, 0.0],
            3 => [0.0, -1.0],
            4 => [0.0, 1.0],
            _ => [0.0, 0.0],
        })
        .collect::<Vec<_>>();
    let original_weights = [[1.0, 0.0], [0.0, -1.0], [0.0, 1.0]];
    let oracle_weights = kept
        .iter()
        .map(|&slot| original_weights[slot])
        .collect::<Vec<_>>();
    for a in 0..2 {
        for b in 0..2 {
            let actual = (0..weights.len())
                .flat_map(|i| (0..weights.len()).map(move |j| (i, j)))
                .map(|(i, j)| weights[i][a] * values[i * weights.len() + j] * weights[j][b])
                .sum();
            let expected = (0..kept.len())
                .flat_map(|i| (0..kept.len()).map(move |j| (i, j)))
                .map(|(i, j)| {
                    oracle_weights[i][a] * oracle[i * kept.len() + j] * oracle_weights[j][b]
                })
                .sum();
            assert_close(actual, expected);
        }
    }
}

fn small_fixture(
    terms: Vec<ExpandedTerm>,
    data: Vec<Vec<f64>>,
    outcome: Vec<f64>,
    absorbed: AbsorbedTerm,
    intercept: bool,
) -> (OlsRequestV1, LoadedOlsInput) {
    let count = data.len();
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: (1..=count)
                .map(|id| source_entry(id as u32, &format!("s{id}")))
                .collect(),
            terms,
        },
        absorbed_terms: vec![absorbed],
        bindings: (1..=count + 2)
            .map(|position| OlsInputBinding {
                stata_var_position: position as u32,
                role: match position {
                    1 => OlsInputRole::Candidate,
                    2 => OlsInputRole::Outcome,
                    id => OlsInputRole::NumericSource(source(id as u32 - 2)),
                },
            })
            .collect(),
        include_intercept: intercept,
        keep_singletons: true,
    };
    let mut outcome = loaded_column(outcome);
    outcome.missing[0] = true;
    outcome.missing[9] = true;
    let loaded = LoadedOlsInput {
        candidate: vec![true; 10],
        outcome,
        numeric_sources: data
            .into_iter()
            .enumerate()
            .map(|(i, values)| (source(i as u32 + 1), loaded_column(values)))
            .collect(),
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    (request, loaded)
}

fn effect(group: u32, slope: Option<u32>) -> AbsorbedTerm {
    AbsorbedTerm {
        id: AbsorbTermId::new(1).expect("effect"),
        intercept: slope.is_none(),
        categorical_sources: vec![source(group)],
        slope_sources: slope.map(source).into_iter().collect(),
        label: "effect".to_owned(),
        saved_name: String::new(),
    }
}

#[test]
fn vanished_base_does_not_remove_a_slope_only_model_direction() {
    let f = vec![1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0, 1.0];
    let slope = vec![0.0, -3.0, -1.0, 1.0, 3.0, -3.0, -1.0, 1.0, 3.0, 0.0];
    let errors = [0.1, -0.1, -0.1, 0.1, 0.1, -0.1, -0.1, 0.1];
    let y = (0..10)
        .map(|i| {
            if (1..9).contains(&i) {
                (if f[i] == 2.0 { 2.0 } else { 5.0 }) + 0.7 * slope[i] + errors[i - 1]
            } else {
                0.0
            }
        })
        .collect();
    let terms = (1..=3)
        .map(|i| factor_term(i, source(1), f64::from(i), i == 1))
        .collect();
    let (request, loaded) = small_fixture(
        terms,
        vec![f, vec![1.0; 10], slope],
        y,
        effect(2, Some(3)),
        false,
    );
    let prepared =
        prepare_categorical_ols_data(&request, &loaded).expect("slope-only original graph");
    let result =
        fit_prepared_categorical_ols_robust_map(&prepared, PreparedOlsRobustConfig::default())
            .expect("slope-only robust fit");
    assert_eq!(result.fit().finite_rank(), 2);
    assert_eq!(result.dof().absorbed_rank, 1);
    assert_eq!(result.dof().residual_dof, 5);
    assert_close(result.fit().coefficients()[1].value.expect("d2"), 2.0);
    assert_close(result.fit().coefficients()[2].value.expect("d3"), 5.0);
    assert_slice_close(result.fit().residuals(), &errors);
    assert_eq!(result.covariance().dimension(), 2);
    assert_slice_close(result.covariance().values(), &[0.004, 0.0, 0.0, 0.004]);
}

fn interaction(value: u32, f: f64, z: f64, omitted: bool) -> ExpandedTerm {
    let mut result = factor_term(value, source(1), f, f == 1.0);
    result.raw_kind = RawTermKind::Interaction;
    result.whole_column_omitted = omitted;
    result.canonical_token = format!("{f}.f#{z}.z");
    let mut second = factor_term(value, source(2), z, z == 1.0)
        .components
        .remove(0);
    second.order = ComponentOrder::new(2).expect("second component");
    result.components.push(second);
    result
}

#[test]
fn vanished_base_does_not_remove_nonhierarchical_cell_effects() {
    let f = vec![1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0, 1.0];
    let z = vec![1.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 2.0, 2.0, 2.0];
    let errors = [0.1, -0.1, 0.1, -0.1, 0.1, -0.1, 0.1, -0.1];
    let y = (0..10)
        .map(|i| {
            1.0 + if z[i] == 2.0 {
                if f[i] == 2.0 {
                    2.0
                } else {
                    5.0
                }
            } else {
                0.0
            } + if (1..9).contains(&i) {
                errors[i - 1]
            } else {
                0.0
            }
        })
        .collect();
    let terms = vec![
        interaction(1, 1.0, 1.0, true),
        interaction(2, 1.0, 2.0, true),
        interaction(3, 2.0, 2.0, false),
        interaction(4, 3.0, 2.0, false),
    ];
    let (request, loaded) =
        small_fixture(terms, vec![f, z, vec![1.0; 10]], y, effect(3, None), true);
    let prepared = prepare_categorical_ols_data(&request, &loaded).expect("cell graph");
    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("nonhierarchical cell fit");
    assert_eq!(result.fit().finite_rank(), 2);
    assert_eq!(result.dof().absorbed_rank, 1);
    assert_eq!(result.dof().residual_dof, 5);
    assert_close(result.fit().coefficients()[2].value.expect("d22"), 2.0);
    assert_close(result.fit().coefficients()[3].value.expect("d32"), 5.0);
    assert_slice_close(result.fit().residuals(), &errors);
}

#[test]
fn whole_column_omission_is_authoritative_for_base_marked_components() {
    let f = vec![1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 1.0];
    let z = vec![1.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 2.0, 2.0, 2.0];
    let y = (0..10)
        .map(|i| {
            1.0 + if z[i] == 2.0 {
                if f[i] == 1.0 {
                    2.0
                } else {
                    5.0
                }
            } else {
                0.0
            }
        })
        .collect();
    let terms = vec![
        interaction(1, 1.0, 1.0, true),
        interaction(2, 1.0, 2.0, false),
        interaction(3, 2.0, 1.0, true),
        interaction(4, 2.0, 2.0, false),
    ];
    let (mut request, loaded) =
        small_fixture(terms, vec![f, z, vec![1.0; 10]], y, effect(3, None), true);
    let prepared =
        prepare_categorical_ols_data(&request, &loaded).expect("active base-component cell");
    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("whole-column active fit");
    assert_eq!(
        result.fit().coefficients()[1].status,
        PreparedOlsFitStatus::Kept
    );
    assert_close(
        result.fit().coefficients()[1]
            .value
            .expect("base-component d12"),
        2.0,
    );
    request.model.terms[1].whole_column_omitted = true;
    let omitted =
        prepare_categorical_ols_data(&request, &loaded).expect("whole-column omitted cell");
    assert_eq!(
        omitted.model().coefficients[1].status,
        fereg_core::PreparedOlsCoefficientStatus::SyntaxOmitted
    );
    assert_eq!(omitted.report_plan().original_coefficient_count(), 5);
}
