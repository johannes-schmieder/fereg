use fereg_engine::{
    data::*,
    ols,
    spec::{ClusterTerm, Estimator},
    *,
};
fn column(values: Vec<f64>) -> NumericColumn {
    NumericColumn {
        values,
        missing: None,
    }
}
fn term(categorical: bool) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(1).unwrap(),
        order: TermOrder::new(1).unwrap(),
        coefficient_slot: CoefficientSlot::new(1).unwrap(),
        raw_kind: if categorical {
            RawTermKind::Factor
        } else {
            RawTermKind::Variable
        },
        whole_column_omitted: false,
        canonical_token: if categorical { "1.x" } else { "x" }.into(),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).unwrap(),
            source: SourceId::new(1).unwrap(),
            kind: if categorical {
                ComponentKind::Categorical
            } else {
                ComponentKind::Continuous
            },
            level_state: if categorical {
                LevelState::Present
            } else {
                LevelState::Absent
            },
            level_value: if categorical { 1. } else { 0. },
            base: MetadataFlag::False,
            omitted: MetadataFlag::False,
            raw_operator: if categorical { "1" } else { "c" }.into(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}
fn model(n: usize) -> ModelSpec {
    ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=n)
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32).unwrap(),
                    canonical_name: format!("s{i}"),
                })
                .collect(),
            terms: vec![term(false)],
        },
        absorbed_terms: vec![],
        clusters: vec![],
        include_intercept: true,
        keep_singletons: false,
    }
}
fn input(x: Vec<f64>) -> Loaded {
    Loaded {
        candidate: vec![true; x.len()],
        outcome: column(vec![2.; x.len()]),
        sources: vec![LoadedSource::Numeric(column(x))],
        weights: None,
        adjustment: LoadedAdjustment::None,
    }
}
fn replace_value(input: &mut Loaded, source: usize, row: usize, value: f64) {
    let LoadedSource::Numeric(column) = &mut input.sources[source] else {
        unreachable!()
    };
    column.values[row] = value;
}
#[test]
fn source_roles_missing_precedence_singletons_and_final_normalization_share_one_ledger() {
    let mut spec = model(4);
    spec.model.terms[0].whole_column_omitted = true;
    spec.absorbed_terms = vec![AbsorbedTerm {
        id: AbsorbTermId::new(1).unwrap(),
        intercept: true,
        categorical_sources: vec![SourceId::new(2).unwrap()],
        slope_sources: vec![SourceId::new(3).unwrap()],
        label: "g##c.s".into(),
        saved_name: String::new(),
    }];
    spec.clusters = vec![ClusterTerm {
        label: "h".into(),
        sources: vec![SourceId::new(4).unwrap()],
    }];
    let mut loaded = input((0..12).map(f64::from).collect());
    loaded.sources.push(LoadedSource::Numeric(column(vec![
        -0.5,
        -0.5,
        -0.5,
        -0.5,
        f64::NAN,
        -0.5,
        -0.5,
        -0.5,
        -0.5,
        1.5,
        2.5,
        2.5,
    ])));
    loaded.sources.push(LoadedSource::Numeric(column(vec![
        1.,
        2.,
        3.,
        4.,
        5.,
        6.,
        f64::NAN,
        8.,
        9.,
        10.,
        11.,
        12.,
    ])));
    loaded.sources.push(LoadedSource::Strings(
        (0..12)
            .map(|i| {
                if i == 5 {
                    String::new()
                } else {
                    format!("h{}", i % 2)
                }
            })
            .collect(),
    ));
    loaded.candidate[0] = false;
    loaded.outcome.values[1] = f64::NAN;
    replace_value(&mut loaded, 0, 2, f64::NAN);
    let mut raw: Vec<_> = (1..=12).map(f64::from).collect();
    raw[3] = 0.;
    loaded.weights = Some((WeightKind::Analytic, column(raw)));
    let uses = SourcePlan::new(&spec).unwrap();
    assert!(uses.uses()[2].fixed_effect);
    assert!(!uses.uses()[2].ordinary);
    assert!(uses.uses()[3].cluster);
    let sample = ols::prepare(&spec, &loaded).unwrap();
    assert_eq!(sample.original_row_by_active, vec![7, 8, 10, 11]);
    assert_eq!(
        sample.drop_reasons,
        vec![
            DropReason::ExcludedByIfIn,
            DropReason::MissingOutcome,
            DropReason::MissingRegressor,
            DropReason::InvalidOrMissingWeight,
            DropReason::MissingFixedEffect,
            DropReason::MissingCluster,
            DropReason::MissingFixedEffect,
            DropReason::Retained,
            DropReason::Retained,
            DropReason::Singleton,
            DropReason::Retained,
            DropReason::Retained,
        ]
    );
    let weights = ReferenceWeights::new(&sample.weights, 4, Estimator::Ols).unwrap();
    for (i, expected) in [0.8, 0.9, 1.1, 1.2].iter().enumerate() {
        assert!((weights.value(i) - expected).abs() < 1e-15);
    }
    sample.topology.as_ref().unwrap().validate(&sample).unwrap();
    assert_eq!(sample.regressors[2], vec![8., 9., 11., 12.]);
    assert_eq!(loaded.candidate.len(), 12);
}
#[test]
fn ordinary_factors_follow_available_sample_domains_and_preserve_original_identity() {
    let mut spec = model(2);
    spec.model.terms[0] = term(true);
    let mut continuous = term(false);
    continuous.id = TermId::new(2).unwrap();
    continuous.order = TermOrder::new(2).unwrap();
    continuous.coefficient_slot = CoefficientSlot::new(2).unwrap();
    continuous.components[0].source = SourceId::new(2).unwrap();
    continuous.canonical_token = "z".into();
    spec.model.terms.push(continuous);
    for invalid in [-1., 1.5] {
        let mut loaded = input(vec![invalid; 8]);
        replace_value(&mut loaded, 0, 6, 0.);
        replace_value(&mut loaded, 0, 7, 1.);
        loaded.candidate[0] = false;
        loaded.outcome.values[1] = f64::NAN;
        loaded.sources.push(LoadedSource::Numeric(column(vec![
            1.,
            1.,
            f64::NAN,
            1.,
            1.,
            1.,
            1.,
            1.,
        ])));
        loaded.weights = Some((
            WeightKind::Analytic,
            column(vec![1., 1., 1., 0., f64::NAN, 1., 1., 1.]),
        ));
        let LoadedSource::Numeric(factor) = &mut loaded.sources[0] else {
            unreachable!()
        };
        factor.missing = Some(vec![false, false, false, false, false, true, false, false]);
        let sample = ols::prepare(&spec, &loaded).unwrap();
        assert_eq!(sample.original_row_by_active, vec![6, 7]);
        assert_eq!(sample.regressors[0], vec![0., 1.]);
        assert_eq!(
            sample.drop_reasons,
            vec![
                DropReason::ExcludedByIfIn,
                DropReason::MissingOutcome,
                DropReason::MissingRegressor,
                DropReason::InvalidOrMissingWeight,
                DropReason::InvalidOrMissingWeight,
                DropReason::MissingRegressor,
                DropReason::Retained,
                DropReason::Retained,
            ]
        );
        replace_value(&mut loaded, 0, 7, invalid);
        let error = ols::prepare(&spec, &loaded).unwrap_err();
        assert_eq!(error.return_code(), 452);
        assert!(error.message.contains("observation 8"));
    }
}
#[test]
fn factor_domains_cannot_be_hidden_by_omission_or_singleton_deletion() {
    let mut spec = model(1);
    spec.model.terms[0] = term(true);
    spec.absorbed_terms = vec![AbsorbedTerm {
        id: AbsorbTermId::new(1).unwrap(),
        intercept: true,
        categorical_sources: vec![SourceId::new(1).unwrap()],
        slope_sources: vec![],
        label: "g".into(),
        saved_name: String::new(),
    }];
    for invalid in [-1., 1.5] {
        for omitted in [false, true] {
            spec.model.terms[0].whole_column_omitted = omitted;
            let loaded = input(vec![invalid, 1., 1., 2., 2.]);
            assert_eq!(ols::prepare(&spec, &loaded).unwrap_err().return_code(), 452);
        }
    }
}
#[test]
fn factor_domains_follow_fe_and_cluster_availability_without_restricting_their_codes() {
    let mut spec = model(3);
    spec.model.terms[0] = term(true);
    spec.absorbed_terms = vec![AbsorbedTerm {
        id: AbsorbTermId::new(1).unwrap(),
        intercept: true,
        categorical_sources: vec![SourceId::new(2).unwrap()],
        slope_sources: vec![],
        label: "g".into(),
        saved_name: String::new(),
    }];
    spec.clusters = vec![ClusterTerm {
        label: "h".into(),
        sources: vec![SourceId::new(3).unwrap()],
    }];
    let mut loaded = input(vec![-1., 1.5, 0., 1., 0., 1.]);
    loaded.sources.push(LoadedSource::Numeric(column(vec![
        f64::NAN,
        2.5,
        -1.5,
        -1.5,
        2.5,
        2.5,
    ])));
    loaded.sources.push(LoadedSource::Numeric(column(vec![
        1.,
        f64::NAN,
        -3.5,
        -3.5,
        4.5,
        4.5,
    ])));
    let sample = ols::prepare(&spec, &loaded).unwrap();
    assert_eq!(sample.original_row_by_active, vec![2, 3, 4, 5]);
    assert_eq!(sample.drop_reasons[0], DropReason::MissingFixedEffect);
    assert_eq!(sample.drop_reasons[1], DropReason::MissingCluster);
    sample.topology.as_ref().unwrap().validate(&sample).unwrap();
}
#[test]
fn estimator_domains_are_enforced_with_pinned_codes_and_availability_order() {
    let spec = model(1);
    for (kind, value, code) in [
        (WeightKind::Analytic, -1., 402),
        (WeightKind::Frequency, 0., 401),
        (WeightKind::Frequency, 1.5, 401),
        (WeightKind::Frequency, u32::MAX as f64 + 1., 401),
        (WeightKind::Probability, 0., 459),
        (WeightKind::Probability, -1., 459),
        (WeightKind::Importance, 1., 101),
    ] {
        let mut loaded = input(vec![1., 2., 3.]);
        loaded.weights = Some((kind, column(vec![value, 1., 1.])));
        assert_eq!(
            ols::prepare(&spec, &loaded).unwrap_err().return_code(),
            code
        );
        loaded.outcome.values[0] = f64::NAN;
        if kind != WeightKind::Importance {
            assert_eq!(ols::prepare(&spec, &loaded).unwrap().n_active(), 2);
        } else {
            loaded.candidate.fill(false);
            assert_eq!(ols::prepare(&spec, &loaded).unwrap_err().return_code(), 101);
        }
    }
    for adjustment in [
        LoadedAdjustment::Offset(column(vec![0.; 3])),
        LoadedAdjustment::Exposure(column(vec![1.; 3])),
    ] {
        let mut loaded = input(vec![1., 2., 3.]);
        loaded.candidate.fill(false);
        loaded.adjustment = adjustment;
        assert_eq!(
            ols::prepare(&spec, &loaded).unwrap_err().code,
            ErrorCode::Unsupported
        );
    }
}
#[test]
fn frequency_multiplicity_survives_singleton_pruning_and_empty_samples_fail() {
    let mut spec = model(1);
    spec.absorbed_terms = vec![AbsorbedTerm {
        id: AbsorbTermId::new(1).unwrap(),
        intercept: true,
        categorical_sources: vec![SourceId::new(1).unwrap()],
        slope_sources: vec![],
        label: "g".into(),
        saved_name: String::new(),
    }];
    let mut loaded = input(vec![-1.5, 2.5]);
    loaded.weights = Some((WeightKind::Frequency, column(vec![2., 1.])));
    let sample = ols::prepare(&spec, &loaded).unwrap();
    assert_eq!(sample.original_row_by_active, vec![0]);
    assert_eq!(
        ReferenceWeights::new(&sample.weights, 1, Estimator::Ols)
            .unwrap()
            .effective(),
        2
    );
    loaded.weights = None;
    assert_eq!(
        ols::prepare(&spec, &loaded).unwrap_err().code,
        ErrorCode::InsufficientObservations
    );
    spec.keep_singletons = true;
    assert_eq!(ols::prepare(&spec, &loaded).unwrap().n_active(), 2);
}
#[test]
fn malformed_banks_and_unsupported_sources_fail_without_indexing() {
    let spec = model(1);
    let mut loaded = input(vec![1., 2., 3.]);
    loaded.outcome.missing = Some(vec![false]);
    assert_eq!(
        ols::prepare(&spec, &loaded).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    loaded.outcome.missing = None;
    loaded.sources = vec![LoadedSource::Strings(vec!["a".into(); 3])];
    assert_eq!(
        ols::prepare(&spec, &loaded).unwrap_err().code,
        ErrorCode::Unsupported
    );
    loaded.sources.clear();
    assert_eq!(
        ols::prepare(&spec, &loaded).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert!(SourcePlan::new(&model(2)).is_err());
}

#[test]
fn absorbed_slopes_require_numeric_sources_even_when_categorical_strings_are_valid() {
    let mut spec = model(2);
    spec.absorbed_terms = vec![AbsorbedTerm {
        id: AbsorbTermId::new(1).unwrap(),
        intercept: false,
        categorical_sources: vec![SourceId::new(1).unwrap()],
        slope_sources: vec![SourceId::new(2).unwrap()],
        label: "g#c.s".into(),
        saved_name: String::new(),
    }];
    let mut loaded = input(vec![1., 1., 2., 2.]);
    loaded
        .sources
        .push(LoadedSource::Strings(vec!["s".into(); 4]));
    assert_eq!(
        ols::prepare(&spec, &loaded).unwrap_err().code,
        ErrorCode::Unsupported
    );
    loaded.sources[1] = LoadedSource::Numeric(column(vec![1., 2., 3., 4.]));
    let sample = ols::prepare(&spec, &loaded).unwrap();
    assert!(!sample.topology.as_ref().unwrap().effects()[0].intercept);
    assert_eq!(sample.topology.as_ref().unwrap().effects().len(), 1);
}
