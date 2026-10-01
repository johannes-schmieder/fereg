use super::*;
use crate::{
    data::{CompiledColumns, Loaded, LoadedAdjustment, LoadedSource, NumericColumn, SourcePlan},
    execution,
    memory::{Estimate, Inputs, Resources},
    ols, ppml,
    spec::metadata::*,
    wls::Role,
};
use std::{cell::Cell, rc::Rc};

fn model(width: usize) -> ModelSpec {
    let source = |value| SourceId::new(value).unwrap();
    ModelSpec {
        model: CompiledTermGraph {
            sources: ["x", "g", "h"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| SourceEntry {
                    id: source(index as u32 + 1),
                    canonical_name: name.into(),
                })
                .collect(),
            terms: (0..width)
                .map(|index| ExpandedTerm {
                    id: TermId::new(index as u32 + 1).unwrap(),
                    order: TermOrder::new(index as u32 + 1).unwrap(),
                    coefficient_slot: CoefficientSlot::new(index as u32 + 1).unwrap(),
                    raw_kind: RawTermKind::Variable,
                    whole_column_omitted: index % 3 == 2,
                    canonical_token: "x".into(),
                    components: vec![TermComponent {
                        order: ComponentOrder::new(1).unwrap(),
                        source: source(1),
                        kind: ComponentKind::Continuous,
                        level_state: LevelState::Absent,
                        level_value: 0.,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::False,
                        raw_operator: "c".into(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    }],
                })
                .collect(),
        },
        absorbed_terms: (0..2)
            .map(|index| AbsorbedTerm {
                id: AbsorbTermId::new(index + 1).unwrap(),
                intercept: true,
                categorical_sources: vec![source(index + 2)],
                slope_sources: vec![],
                label: if index == 0 { "g".into() } else { "h".into() },
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: false,
    }
}
fn inputs() -> Inputs {
    Inputs {
        original_rows: 12,
        active_rows: 12,
        sources: 3,
        auxiliary_numeric_sources: 0,
        string_sources: 0,
        string_bytes: 0,
        metadata_bytes: 0,
        weighted: false,
        adjustment: false,
        parent_bytes: 0,
    }
}
fn every_boundary<T: std::fmt::Debug + PartialEq>(
    run: impl Fn(&mut dyn FnMut() -> Result<()>) -> Result<T>,
) -> usize {
    let caller = std::thread::current().id();
    let count = Rc::new(Cell::new(0));
    let expected = run(&mut || Ok(())).unwrap();
    assert_eq!(
        run(&mut || {
            assert_eq!(std::thread::current().id(), caller);
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap(),
        expected
    );
    let total = count.get();
    assert!(total > 0);
    let failure = FeregError::new(ErrorCode::NonConvergence, "preflight host r(430)");
    for stop in 1..=total {
        count.set(0);
        let result = run(&mut || {
            assert_eq!(std::thread::current().id(), caller);
            count.set(count.get() + 1);
            if count.get() == stop {
                Err(failure.clone())
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err(), failure);
        assert_eq!(count.get(), stop);
        assert_eq!(run(&mut || Ok(())).unwrap(), expected);
    }
    total
}
fn construct(
    spec: &ModelSpec,
    estimator: Estimator,
    workers: usize,
    bias: Option<ppml::bias::Request>,
    poll: &mut dyn FnMut() -> Result<()>,
) -> Result<Estimate> {
    let options = Options {
        threads: workers,
        ..Options::for_estimator(estimator)
    };
    let saved = (0..spec.absorbed_terms.len())
        .map(|effect| (effect, 0))
        .collect();
    match estimator {
        Estimator::Ols => {
            let mut request = ols::Request::default();
            request.outputs.saved = saved;
            let call = ols::Call::new_with(
                spec,
                &options,
                &request,
                inputs(),
                None,
                Resources::default(),
                &mut || poll(),
            )?;
            Ok(call.memory().clone())
        }
        Estimator::Ppml => {
            let mut request = ppml::Request {
                bias,
                ..Default::default()
            };
            request.outputs.common.saved = saved;
            request.bias_unit_source = bias.map(|_| ppml::BiasUnitSource::Outcome);
            let call = ppml::Call::new_with(
                spec,
                &options,
                &request,
                inputs(),
                None,
                Resources::default(),
                &mut || poll(),
            )?;
            Ok(call.memory().clone())
        }
    }
}
fn wire(spec: &ModelSpec) -> ModelWireV2 {
    ModelWireV2::from_specification(&CompiledModelSpecification {
        model: spec.model.clone(),
        absorbed_terms: spec.absorbed_terms.clone(),
    })
    .unwrap()
}

#[test]
fn constructors_cancel_every_preflight_boundary_without_loading_or_retaining_callbacks() {
    let spec = model(2053);
    let columns = CompiledColumns::new(&spec, Estimator::Ols).unwrap();
    assert_eq!(columns.slots.len(), 2054);
    assert_eq!(columns.recipes.len(), 2053 - 2053 / 3 + 1);
    assert_eq!(columns.constant_slot, Some(2053));
    assert_eq!(columns.roles.last(), Some(&Role::AbsorbedIntercept));
    assert!(SourcePlan::new(&spec).unwrap().uses()[0].ordinary);
    for estimator in [Estimator::Ols, Estimator::Ppml] {
        let methods = if estimator == Estimator::Ppml {
            vec![
                None,
                Some(ppml::bias::Method::Exact),
                Some(ppml::bias::Method::randomized_default()),
            ]
        } else {
            vec![None]
        };
        for method in methods {
            let bias = method.map(|method| ppml::bias::Request {
                method,
                save_effects: true,
            });
            let mut reference = None;
            for workers in [1, 8] {
                let count = every_boundary(|poll| construct(&spec, estimator, workers, bias, poll));
                assert!(count > 20);
                assert_eq!(*reference.get_or_insert(count), count);
            }
        }
    }
}

#[test]
fn wide_components_and_absorbed_recipes_cancel_validation_compilation_and_wire_decoding() {
    let mut component_model = model(2);
    component_model.model.terms[0].raw_kind = RawTermKind::Interaction;
    let component = TermComponent {
        order: ComponentOrder::new(1).unwrap(),
        source: SourceId::new(2).unwrap(),
        kind: ComponentKind::Categorical,
        level_state: LevelState::Present,
        level_value: 1.,
        base: MetadataFlag::False,
        omitted: MetadataFlag::False,
        raw_operator: "1".into(),
        time_series_state: TimeSeriesState::None,
        raw_time_series_operator: String::new(),
    };
    component_model.model.terms[0].components = (0..2053)
        .map(|index| TermComponent {
            order: ComponentOrder::new(index + 1).unwrap(),
            ..component.clone()
        })
        .collect();
    let mut recipe_model = model(1);
    recipe_model.absorbed_terms[0].categorical_sources = vec![SourceId::new(2).unwrap(); 2053];
    let mut effect_model = model(1);
    effect_model.absorbed_terms = (0..1027)
        .map(|index| AbsorbedTerm {
            id: AbsorbTermId::new(index + 1).unwrap(),
            categorical_sources: vec![SourceId::new(2 + index % 2).unwrap()],
            ..effect_model.absorbed_terms[0].clone()
        })
        .collect();
    for spec in [model(2053), component_model, recipe_model, effect_model] {
        let encoded = wire(&spec);
        assert_eq!(ModelSpec::from_wire(&encoded).unwrap(), spec);
        assert!(every_boundary(|poll| ModelSpec::from_wire_with(&encoded, &mut || poll())) > 10);
        for estimator in [Estimator::Ols, Estimator::Ppml] {
            assert!(
                every_boundary(|poll| {
                    CompiledColumns::new_polled(&spec, estimator, &mut || poll())
                }) > 5
            );
            assert!(every_boundary(|poll| construct(&spec, estimator, 1, None, poll)) > 10);
        }
    }
}

#[test]
fn deep_name_and_cluster_uniqueness_checks_poll_and_keep_error_precedence() {
    let mut spec = model(0);
    spec.absorbed_terms.clear();
    spec.model.sources = (1..=2053)
        .map(|index| SourceEntry {
            id: SourceId::new(index).unwrap(),
            canonical_name: format!("g{index}"),
        })
        .collect();
    spec.clusters = vec![ClusterTerm {
        label: "tuple".into(),
        sources: spec.model.sources.iter().map(|source| source.id).collect(),
    }];
    let mut checks = 0;
    spec.validate_polled(&mut || {
        checks += 1;
        Ok(())
    })
    .unwrap();
    assert!(checks > 1000);
    let failure = FeregError::new(ErrorCode::NonConvergence, "deep names r(430)");
    let mut checks = 0;
    assert_eq!(
        spec.model
            .validate_polled(&mut || {
                checks += 1;
                if checks == 3 {
                    Err(failure.clone())
                } else {
                    Ok(())
                }
            })
            .unwrap_err(),
        failure
    );
    let names: Vec<_> = spec
        .model
        .sources
        .iter()
        .map(|source| source.canonical_name.as_str())
        .collect();
    assert_eq!(
        every_boundary(|poll| execution::any_with(&names, |name| *name == "g2053", &mut || poll())),
        2
    );
    spec.clusters[0].sources.push(SourceId::new(2053).unwrap());
    let original = spec.validate().unwrap_err();
    assert_eq!(original.code, ErrorCode::InvalidArgument);
    assert_eq!(spec.validate_polled(&mut || Ok(())).unwrap_err(), original);
    spec.model.sources[2052].canonical_name = " g1 ".into();
    let original = spec.validate().unwrap_err();
    assert!(original.message.contains("source names"));
    assert_eq!(spec.validate_polled(&mut || Ok(())).unwrap_err(), original);
}

#[test]
fn malformed_wire_metadata_and_unsupported_requests_keep_original_errors() {
    let spec = model(2053);
    let encoded = wire(&spec);
    let mut invalid = encoded.clone();
    invalid.major = 3.;
    let mut bad_fields = encoded.clone();
    bad_fields.components.rows.last_mut().unwrap()[12] = f64::NAN;
    let mut orphan = encoded.clone();
    orphan.components.rows.last_mut().unwrap()[0] = 1.;
    for bad in [invalid, bad_fields, orphan] {
        let original = ModelSpec::from_wire(&bad).unwrap_err();
        assert_eq!(
            ModelSpec::from_wire_with(&bad, &mut || Ok(())).unwrap_err(),
            original
        );
    }
    let mut bad = spec.clone();
    bad.model.terms.last_mut().unwrap().components[0].time_series_state =
        TimeSeriesState::PendingMaterialization;
    bad.model.terms.last_mut().unwrap().components[0].raw_time_series_operator = "L".into();
    let original = CompiledColumns::new(&bad, Estimator::Ols).unwrap_err();
    assert_eq!(original.code, ErrorCode::Unsupported);
    assert_eq!(
        CompiledColumns::new_polled(&bad, Estimator::Ols, &mut || Ok(())).unwrap_err(),
        original
    );
    for estimator in [Estimator::Ols, Estimator::Ppml] {
        assert_eq!(
            construct(&bad, estimator, 1, None, &mut || Ok(())).unwrap_err(),
            original
        );
    }
    let mut empty = model(1);
    empty.absorbed_terms.clear();
    empty.model.sources.truncate(1);
    let mut compatible = wire(&empty);
    compatible.minor = 0.;
    for encoded in [compatible, wire(&empty)] {
        assert_eq!(ModelSpec::from_wire(&encoded).unwrap(), empty);
        assert!(every_boundary(|poll| ModelSpec::from_wire_with(&encoded, &mut || poll())) > 0);
    }
}

fn loaded(ppml: bool) -> Loaded {
    let column = |values| NumericColumn {
        values,
        missing: None,
    };
    Loaded {
        candidate: vec![true; 12],
        outcome: column(
            (0..12)
                .map(|row| {
                    let x = (row % 4) as f64;
                    if ppml {
                        2_f64.powf(x)
                    } else {
                        3. + 2. * x
                    }
                })
                .collect(),
        ),
        sources: vec![
            LoadedSource::Numeric(column((0..12).map(|row| (row % 4) as f64).collect())),
            LoadedSource::Numeric(column((0..12).map(|row| (row % 2) as f64).collect())),
            LoadedSource::Numeric(column((0..12).map(|row| (row / 4) as f64).collect())),
        ],
        weights: None,
        adjustment: LoadedAdjustment::None,
    }
}
fn same_covariance(actual: &crate::vce::Covariance, expected: &crate::vce::Covariance) {
    assert_eq!(actual.values, expected.values);
    let bits = |values: &[f64]| {
        values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        bits(&actual.standard_errors),
        bits(&expected.standard_errors)
    );
    assert_eq!(actual.correction.to_bits(), expected.correction.to_bits());
    assert_eq!(actual.cluster_counts, expected.cluster_counts);
    assert_eq!(actual.subset_counts, expected.subset_counts);
    assert_eq!(
        actual.reconstruction_error.to_bits(),
        expected.reconstruction_error.to_bits()
    );
    assert_eq!(actual.psd, expected.psd);
}
#[test]
fn aborted_or_panicking_preflight_can_restart_with_exact_complete_estimates() {
    let spec = model(1);
    let caller = std::thread::current().id();
    let failure = FeregError::new(ErrorCode::NonConvergence, "preflight restart r(430)");
    for workers in [1, 8] {
        let options = Options {
            threads: workers,
            tolerance: 1e-12,
            ..Options::for_estimator(Estimator::Ols)
        };
        let request = ols::Request::default();
        let data = loaded(false);
        let make = || {
            ols::Call::new(
                &spec,
                &options,
                &request,
                inputs(),
                None,
                Resources::default(),
            )
        };
        let expected = make().unwrap().run(&data).unwrap();
        assert!((expected.coefficients[0] - 2.).abs() < 1e-10);
        let mut count = 0;
        assert_eq!(
            ols::Call::new_with(
                &spec,
                &options,
                &request,
                inputs(),
                None,
                Resources::default(),
                &mut || {
                    assert_eq!(std::thread::current().id(), caller);
                    count += 1;
                    if count == 4 {
                        Err(failure.clone())
                    } else {
                        Ok(())
                    }
                }
            )
            .err()
            .unwrap(),
            failure
        );
        let actual = make().unwrap().run(&data).unwrap();
        assert_eq!(actual.coefficients, expected.coefficients);
        same_covariance(&actual.covariance, &expected.covariance);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ols::Call::new_with(
                &spec,
                &options,
                &request,
                inputs(),
                None,
                Resources::default(),
                &mut || {
                    panic!("preflight host panic");
                },
            )
        }));
        assert!(result.is_err());
        assert_eq!(
            make().unwrap().run(&data).unwrap().coefficients,
            expected.coefficients
        );
        let options = Options {
            threads: workers,
            tolerance: 1e-12,
            ..Options::for_estimator(Estimator::Ppml)
        };
        let request = ppml::Request::default();
        let data = loaded(true);
        let make = || {
            ppml::Call::new(
                &spec,
                &options,
                &request,
                inputs(),
                None,
                Resources::default(),
            )
        };
        let expected = make().unwrap().run(&data, None).unwrap();
        assert!((expected.coefficients[0] - 2_f64.ln()).abs() < 1e-10);
        assert_eq!(
            ppml::Call::new_with(
                &spec,
                &options,
                &request,
                inputs(),
                None,
                Resources::default(),
                &mut || Err(failure.clone())
            )
            .err()
            .unwrap(),
            failure
        );
        let actual = make().unwrap().run(&data, None).unwrap();
        assert_eq!(actual.coefficients, expected.coefficients);
        same_covariance(&actual.covariance, &expected.covariance);
        let denied = Resources {
            usable_bytes: Some(0),
            ..Default::default()
        };
        assert_eq!(
            ppml::Call::new_with(
                &spec,
                &options,
                &request,
                inputs(),
                None,
                denied,
                &mut || Ok(())
            )
            .err()
            .unwrap()
            .code,
            ErrorCode::Allocation
        );
    }
}
