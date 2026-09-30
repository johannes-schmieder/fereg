#![forbid(unsafe_code)]
use fereg_engine::{
    data::*,
    memory::Inputs,
    outputs::{self, Kind},
    ppml::{self, bias, separation, Call, Phase, Request, Resources},
    spec::{ClusterTerm, Estimator, Options, Solver},
    threads::ThreadTeam,
    vce, *,
};
fn column(values: Vec<f64>) -> NumericColumn {
    NumericColumn {
        values,
        missing: None,
    }
}
fn term(slot: u32, omitted: bool) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(slot).unwrap(),
        order: TermOrder::new(slot).unwrap(),
        coefficient_slot: CoefficientSlot::new(slot).unwrap(),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: omitted,
        canonical_token: format!("x{slot}"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).unwrap(),
            source: SourceId::new(1).unwrap(),
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.,
            base: MetadataFlag::False,
            omitted: MetadataFlag::False,
            raw_operator: "c".into(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}
fn model(clustered: bool) -> ModelSpec {
    ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=3)
                .map(|id| SourceEntry {
                    id: SourceId::new(id).unwrap(),
                    canonical_name: format!("s{id}"),
                })
                .collect(),
            terms: vec![term(1, false), term(2, false), term(3, true)],
        },
        absorbed_terms: (2..=3)
            .map(|id| AbsorbedTerm {
                id: AbsorbTermId::new(id - 1).unwrap(),
                categorical_sources: vec![SourceId::new(id).unwrap()],
                intercept: true,
                slope_sources: vec![],
                label: format!("g{id}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: if clustered {
            vec![ClusterTerm {
                label: "g".into(),
                sources: vec![SourceId::new(2).unwrap()],
            }]
        } else {
            vec![]
        },
        include_intercept: true,
        keep_singletons: true,
    }
}
fn fixture(kind: Option<WeightKind>) -> (Loaded, NumericColumn) {
    let n = 120;
    let mut loaded = Loaded {
        candidate: vec![true; n],
        outcome: column(
            (0..n)
                .map(|i| 0.5 + ((i * 17) % 11) as f64 * 0.25)
                .collect(),
        ),
        sources: vec![
            LoadedSource::Numeric(column((0..n).map(|i| (i as f64 * 1.7).sin()).collect())),
            LoadedSource::Numeric(column((0..n).map(|i| (i % 4) as f64).collect())),
            LoadedSource::Strings((0..n).map(|i| format!("h{}", (i / 4) % 5)).collect()),
        ],
        weights: kind.map(|kind| (kind, column((0..n).map(|i| (1 + i % 3) as f64).collect()))),
        adjustment: LoadedAdjustment::Exposure(column(
            (0..n)
                .map(|i| ((i % 7) as f64 * 0.08 - 0.24).exp())
                .collect(),
        )),
    };
    loaded.candidate[0] = false;
    loaded.outcome.values[0] = -1.;
    loaded.outcome.values[1] = f64::NAN;
    let LoadedSource::Numeric(x) = &mut loaded.sources[0] else {
        unreachable!()
    };
    x.values[2] = f64::NAN;
    let mut units = column((0..n).map(|i| (i % 7) as f64 - 3.5).collect());
    units.values[..3].fill(f64::NAN);
    (loaded, units)
}
fn options(solver: Solver, threads: usize) -> Options {
    Options {
        solver,
        threads,
        ..Options::for_estimator(Estimator::Ppml)
    }
}
fn request(saved: bool, method: Option<bias::Method>) -> Request {
    Request {
        outputs: if saved {
            outputs::PpmlRequest {
                mean: true,
                common: outputs::Request {
                    residuals: true,
                    xb: true,
                    xbd: true,
                    absorbed: true,
                    drop_reason: true,
                    saved: vec![(0, 0), (1, 0)],
                },
                residual_type: None,
            }
        } else {
            outputs::PpmlRequest::default()
        },
        bias: method.map(|method| bias::Request {
            method,
            save_effects: saved,
        }),
        ..Request::default()
    }
}
fn same(a: &ppml::PpmlFit, b: &ppml::PpmlFit) {
    let bits = |values: &[f64]| values.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&a.coefficients), bits(&b.coefficients));
    assert_eq!(a.statuses, b.statuses);
    assert_eq!(a.coordinates, b.coordinates);
    assert_eq!(a.covariance.values, b.covariance.values);
    assert_eq!(
        bits(&a.covariance.standard_errors),
        bits(&b.covariance.standard_errors)
    );
    assert_eq!(bits(&a.z_statistics), bits(&b.z_statistics));
    assert_eq!(a.covariance.psd, b.covariance.psd);
    assert_eq!(a.covariance.cluster_counts, b.covariance.cluster_counts);
    assert_eq!(a.covariance.subset_counts, b.covariance.subset_counts);
    assert_eq!(a.dof, b.dof);
    assert_eq!(a.absorbed_dof, b.absorbed_dof);
    assert_eq!(a.wald, b.wald);
    assert_eq!(a.statistics, b.statistics);
    assert_eq!(a.weights, b.weights);
    assert_eq!(a.sample, b.sample);
    assert_eq!(a.diagnostics, b.diagnostics);
    assert_eq!(a.separation, b.separation);
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
#[test]
fn constant_only_matches_independent_weighted_poisson_and_robust_sandwich() {
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: vec![],
            terms: vec![],
        },
        absorbed_terms: vec![],
        clusters: vec![],
        include_intercept: true,
        keep_singletons: false,
    };
    for kind in [
        None,
        Some(WeightKind::Frequency),
        Some(WeightKind::Probability),
    ] {
        let y: Vec<_> = (0..20).map(|i| (i % 7) as f64 * 0.25).collect();
        let w: Vec<_> = (0..20).map(|i| (1 + i % 3) as f64).collect();
        let loaded = Loaded {
            candidate: vec![true; 20],
            outcome: column(y.clone()),
            sources: vec![],
            weights: kind.map(|kind| (kind, column(w.clone()))),
            adjustment: LoadedAdjustment::None,
        };
        let request = Request {
            outputs: outputs::PpmlRequest {
                mean: true,
                ..Default::default()
            },
            ..Request::default()
        };
        let fit = ppml::fit_requested(
            &model,
            &loaded,
            &options(Solver::Auto, 1),
            &request,
            None,
            Resources::default(),
        )
        .unwrap();
        let sum: f64 = w.iter().sum();
        let effective = if kind == Some(WeightKind::Frequency) {
            sum
        } else {
            20.
        };
        let q: Vec<_> = w
            .iter()
            .map(|w| match kind {
                None => 1.,
                Some(WeightKind::Frequency) => *w,
                _ => *w * 20. / sum,
            })
            .collect();
        let mean = y.iter().zip(&q).map(|(y, q)| y * q).sum::<f64>() / effective;
        let bread = 1. / (effective * mean);
        let meat = y
            .iter()
            .zip(&q)
            .map(|(y, q)| {
                (y - mean).powi(2)
                    * if kind == Some(WeightKind::Probability) {
                        q * q
                    } else {
                        *q
                    }
            })
            .sum::<f64>();
        close(fit.coefficients[0], mean.ln());
        close(
            fit.covariance.values[(0, 0)],
            bread * meat * bread * effective / (effective - 1.),
        );
        for &mu in fit.outputs.get(Kind::Mean).unwrap() {
            close(mu, mean);
        }
        assert_eq!(fit.weights.effective_rows, effective as usize);
        assert!(fit.bias.is_none());
        assert_eq!(fit.memory.bias_workspace_bytes(), 0);
    }
}
#[test]
fn complete_calls_preserve_science_across_outputs_bias_modes_and_workers() {
    for kind in [
        None,
        Some(WeightKind::Frequency),
        Some(WeightKind::Probability),
    ] {
        let (loaded, units) = fixture(kind);
        for clustered in [false, true] {
            let model = model(clustered);
            for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
                let baseline = ppml::fit(&model, &loaded, &options(solver, 1)).unwrap();
                assert_eq!(
                    baseline.vce,
                    if clustered {
                        vce::Kind::Cluster
                    } else {
                        vce::Kind::Robust
                    }
                );
                assert_eq!(baseline.stripes, ["x1", "x2", "x3", "_cons"]);
                assert!(baseline.outputs.columns.is_empty());
                for method in [
                    None,
                    Some(bias::Method::Exact),
                    Some(bias::Method::Randomized {
                        probes: 17,
                        seed: 739391,
                        stream: 3,
                    }),
                ] {
                    let mut previous: Option<ppml::PpmlFit> = None;
                    for workers in [1, 8] {
                        let requested = request(true, method);
                        let fit = ppml::fit_requested(
                            &model,
                            &loaded,
                            &options(solver, workers),
                            &requested,
                            method.map(|_| &units),
                            Resources::default(),
                        )
                        .unwrap();
                        same(&baseline, &fit);
                        assert_eq!(fit.outputs.columns.len(), 8);
                        if let Some(before) = previous.as_ref() {
                            for (a, b) in before.outputs.columns.iter().zip(&fit.outputs.columns) {
                                for (a, b) in a.values.iter().zip(&b.values) {
                                    assert_eq!(a.to_bits(), b.to_bits());
                                }
                            }
                            if let (Some(a), Some(b)) = (&before.bias, &fit.bias) {
                                assert_eq!(a.ordinary, b.ordinary);
                                assert_eq!(a.diagnostics, b.diagnostics);
                                for row in (0..21).filter(|&i| i != 9 && i != 10) {
                                    assert_eq!(a.table.values[row], b.table.values[row]);
                                }
                            }
                        }
                        for &row in &fit.sample.retained_rows {
                            close(
                                fit.outputs.get(Kind::Xb).unwrap()[row]
                                    + fit.outputs.get(Kind::Absorbed).unwrap()[row],
                                fit.outputs.get(Kind::Xbd).unwrap()[row],
                            );
                            close(
                                fit.outputs
                                    .get(Kind::Saved {
                                        effect: 0,
                                        channel: 0,
                                    })
                                    .unwrap()[row]
                                    + fit
                                        .outputs
                                        .get(Kind::Saved {
                                            effect: 1,
                                            channel: 0,
                                        })
                                        .unwrap()[row],
                                fit.outputs.get(Kind::Absorbed).unwrap()[row],
                            );
                        }
                        if let Some(bias) = &fit.bias {
                            assert_eq!(bias.ordinary.nrows(), 9);
                            assert_eq!(bias.saved.columns.len(), 18);
                            assert_eq!(
                                bias.table.values[10],
                                fit.memory.bias_workspace_bytes() as f64
                            );
                            for values in &bias.saved.columns {
                                for (value, reason) in
                                    values.values.iter().zip(&fit.sample.drop_reasons)
                                {
                                    assert_eq!(value.is_finite(), *reason == DropReason::Retained);
                                }
                            }
                        }
                        previous = Some(fit);
                    }
                }
            }
        }
    }
}
#[test]
fn initial_singletons_and_exact_separation_share_the_complete_ledger() {
    let (mut loaded, _) = fixture(None);
    let mut model = model(false);
    model.keep_singletons = false;
    let LoadedSource::Numeric(g) = &mut loaded.sources[1] else {
        unreachable!()
    };
    g.values[117] = 98.;
    g.values[118..].fill(99.);
    loaded.outcome.values[118..].fill(0.);
    let fit = ppml::fit(&model, &loaded, &options(Solver::Auto, 1)).unwrap();
    assert_eq!(fit.sample.drop_reasons[117], DropReason::Singleton);
    assert_eq!(
        fit.sample.drop_reasons[118..],
        [DropReason::FixedEffectSeparation; 2]
    );
    assert_eq!(fit.separation.singletons, 1);
    assert_eq!(fit.separation.methods[0].primary, 2);
    assert_eq!(fit.separation.generation, 2);
    assert_eq!(fit.sample.retained_rows.len(), 114);
    assert_eq!(
        fit.separation.trace[0].unwrap().kind,
        separation::EventKind::InitialSingletons
    );
    let request = request(false, Some(bias::Method::Exact));
    let units = column(vec![1.; 120]);
    assert_eq!(
        ppml::fit_requested(
            &model,
            &loaded,
            &options(Solver::Auto, 1),
            &request,
            Some(&units),
            Resources::default()
        )
        .unwrap_err()
        .return_code(),
        498
    );
}
#[test]
fn one_admission_binds_model_and_auxiliary_banks_and_fails_early() {
    let (loaded, units) = fixture(None);
    let model = model(false);
    let request = request(true, Some(bias::Method::Exact));
    let options = options(Solver::Auto, 1);
    let inputs = Inputs::from_loaded(&loaded, 0, 0)
        .unwrap()
        .with_numeric_auxiliary(&units)
        .unwrap();
    let make = |inputs, resources| Call::new(&model, &options, &request, inputs, None, resources);
    let memory = make(inputs, Resources::default()).unwrap().memory().clone();
    assert_eq!(
        make(
            inputs,
            Resources {
                usable_bytes: Some(memory.minimum_bytes() - 1),
                ..Default::default()
            }
        )
        .err()
        .unwrap()
        .return_code(),
        909
    );
    let fit = make(
        inputs,
        Resources {
            usable_bytes: Some(memory.minimum_bytes()),
            ..Default::default()
        },
    )
    .unwrap()
    .run(&loaded, Some(&units))
    .unwrap();
    assert_eq!(fit.memory, memory);
    assert_eq!(
        make(inputs, Resources::default())
            .unwrap()
            .run(&loaded, None)
            .unwrap_err()
            .return_code(),
        498
    );
    assert_eq!(
        make(inputs, Resources::default())
            .unwrap()
            .run_with_team(
                &loaded,
                Some(&units),
                &ThreadTeam::new(2).unwrap(),
                &mut |_| Ok(())
            )
            .unwrap_err()
            .return_code(),
        498
    );
    let mut units = units;
    units.values.reserve(100_000);
    assert_eq!(
        make(inputs, Resources::default())
            .unwrap()
            .run(&loaded, Some(&units))
            .unwrap_err()
            .return_code(),
        498
    );
    assert_eq!(
        make(
            Inputs {
                active_rows: 118,
                ..inputs
            },
            Resources::default()
        )
        .unwrap()
        .run(&loaded, Some(&units))
        .unwrap_err()
        .return_code(),
        498
    );
    assert_eq!(
        make(
            inputs,
            Resources {
                publication_matrix_cells: usize::MAX,
                ..Default::default()
            }
        )
        .err()
        .unwrap()
        .return_code(),
        909
    );
}
#[test]
fn all_caller_boundaries_discard_partial_results_and_allow_fresh_calls() {
    let (loaded, units) = fixture(None);
    let model = model(false);
    let request = request(true, Some(bias::Method::Exact));
    let options = options(Solver::Auto, 2);
    let inputs = Inputs::from_loaded(&loaded, 0, 0)
        .unwrap()
        .with_numeric_auxiliary(&units)
        .unwrap();
    let caller = std::thread::current().id();
    for stop in [
        Phase::Loaded,
        Phase::Prepared,
        Phase::Separated,
        Phase::Estimated,
        Phase::Inferred,
        Phase::Outputs,
        Phase::BiasCorrection,
        Phase::Corrected,
    ] {
        let mut reached = false;
        let call = Call::new(
            &model,
            &options,
            &request,
            inputs,
            None,
            Resources::default(),
        )
        .unwrap();
        assert_eq!(
            call.run_with(&loaded, Some(&units), &mut |phase| {
                assert_eq!(std::thread::current().id(), caller);
                if phase == stop {
                    reached = true;
                    Err(FeregError::new(ErrorCode::Interrupted, "fixture Break"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .return_code(),
            1
        );
        assert!(reached);
        ppml::fit_requested(
            &model,
            &loaded,
            &options,
            &request,
            Some(&units),
            Resources::default(),
        )
        .unwrap();
    }
}
#[test]
fn unsupported_requests_and_candidate_domain_precedence_fail_closed() {
    for kind in [WeightKind::Analytic, WeightKind::Importance] {
        let (loaded, _) = fixture(Some(kind));
        assert_eq!(
            ppml::fit(&model(false), &loaded, &options(Solver::Auto, 1))
                .unwrap_err()
                .return_code(),
            101
        );
    }
    let (mut loaded, units) = fixture(Some(WeightKind::Frequency));
    let model = model(false);
    let options = options(Solver::Auto, 1);
    let invalid = Request {
        vce: Some(vce::Kind::Iid),
        ..Default::default()
    };
    assert_eq!(
        ppml::fit_requested(
            &model,
            &loaded,
            &options,
            &invalid,
            None,
            Resources::default()
        )
        .unwrap_err()
        .return_code(),
        498
    );
    assert_eq!(
        ppml::fit_requested(
            &model,
            &loaded,
            &options,
            &Request::default(),
            Some(&units),
            Resources::default()
        )
        .unwrap_err()
        .return_code(),
        498
    );
    loaded.weights.as_mut().unwrap().1.values[1] = -1.;
    ppml::fit(&model, &loaded, &options).unwrap();
    loaded.weights.as_mut().unwrap().1.values[3] = 0.;
    assert_eq!(
        ppml::fit(&model, &loaded, &options)
            .unwrap_err()
            .return_code(),
        401
    );
    loaded.weights.as_mut().unwrap().1.values[3] = 1.;
    loaded.outcome.values[3] = -0.5;
    assert_eq!(
        ppml::fit(&model, &loaded, &options)
            .unwrap_err()
            .return_code(),
        459
    );
    loaded.outcome.values[3] = 1.;
    let LoadedAdjustment::Exposure(e) = &mut loaded.adjustment else {
        unreachable!()
    };
    e.values[3] = 0.;
    assert_eq!(
        ppml::fit(&model, &loaded, &options)
            .unwrap_err()
            .return_code(),
        459
    );
    loaded.outcome.values.fill(0.);
    loaded.weights.as_mut().unwrap().1.values[1] = 1.;
    let LoadedAdjustment::Exposure(e) = &mut loaded.adjustment else {
        unreachable!()
    };
    e.values[3] = 1.;
    assert_eq!(
        ppml::fit(&model, &loaded, &options)
            .unwrap_err()
            .return_code(),
        2001
    );
}
#[test]
fn ordinary_factors_are_validated_after_availability_before_ledger_deletion() {
    let (mut loaded, _) = fixture(Some(WeightKind::Probability));
    let mut model = model(false);
    model.keep_singletons = false;
    let term = &mut model.model.terms[2];
    term.raw_kind = RawTermKind::Factor;
    term.canonical_token = "0b.factor".into();
    let component = &mut term.components[0];
    component.kind = ComponentKind::Categorical;
    component.level_state = LevelState::Present;
    component.base = MetadataFlag::True;
    component.raw_operator = "0b".into();
    let LoadedSource::Numeric(x) = &mut loaded.sources[0] else {
        unreachable!()
    };
    for (i, value) in x.values.iter_mut().enumerate() {
        *value = (i % 3) as f64;
    }
    x.values[0] = -0.5;
    x.values[1] = -0.5;
    // Adjustment missingness excludes this invalid ordinary factor first.
    x.values[3] = -0.5;
    let LoadedAdjustment::Exposure(exposure) = &mut loaded.adjustment else {
        unreachable!()
    };
    exposure.values[3] = f64::NAN;
    ppml::fit(&model, &loaded, &options(Solver::Auto, 1)).unwrap();
    let LoadedAdjustment::Exposure(exposure) = &mut loaded.adjustment else {
        unreachable!()
    };
    exposure.values[3] = 1.;
    assert_eq!(
        ppml::fit(&model, &loaded, &options(Solver::Auto, 1))
            .unwrap_err()
            .return_code(),
        452
    );
    let LoadedSource::Numeric(x) = &mut loaded.sources[0] else {
        unreachable!()
    };
    x.values[3] = 0.;
    x.values[117] = -0.5;
    let LoadedSource::Numeric(g) = &mut loaded.sources[1] else {
        unreachable!()
    };
    g.values[117] = 99.;
    assert_eq!(
        ppml::fit(&model, &loaded, &options(Solver::Auto, 1))
            .unwrap_err()
            .return_code(),
        452
    );
}
#[test]
fn a_model_bias_unit_source_shares_its_loaded_owner_and_matches_explicit_units() {
    let (loaded, _) = fixture(None);
    let model = model(false);
    let options = options(Solver::Auto, 1);
    let LoadedSource::Numeric(x) = &loaded.sources[0] else {
        unreachable!()
    };
    let units = column(x.values.clone());
    let mut request = request(false, Some(bias::Method::Exact));
    let separate = ppml::fit_requested(
        &model,
        &loaded,
        &options,
        &request,
        Some(&units),
        Resources::default(),
    )
    .unwrap();
    request.bias_unit_source = Some(ppml::BiasUnitSource::Model(SourceId::new(1).unwrap()));
    let reused = ppml::fit_requested(
        &model,
        &loaded,
        &options,
        &request,
        None,
        Resources::default(),
    )
    .unwrap();
    same(&separate, &reused);
    assert_eq!(
        separate.bias.as_ref().unwrap().ordinary,
        reused.bias.as_ref().unwrap().ordinary
    );
    assert_eq!(
        separate.bias.as_ref().unwrap().diagnostics,
        reused.bias.as_ref().unwrap().diagnostics
    );
    assert_eq!(
        separate.memory.minimum_bytes() - reused.memory.minimum_bytes(),
        loaded.candidate.len() as u128 * 8
    );
    request.bias_unit_source = Some(ppml::BiasUnitSource::Model(SourceId::new(3).unwrap()));
    assert_eq!(
        ppml::fit_requested(
            &model,
            &loaded,
            &options,
            &request,
            None,
            Resources::default()
        )
        .unwrap_err()
        .return_code(),
        498
    );
    request.bias_unit_source = Some(ppml::BiasUnitSource::Model(SourceId::new(4).unwrap()));
    assert_eq!(
        ppml::fit_requested(
            &model,
            &loaded,
            &options,
            &request,
            None,
            Resources::default()
        )
        .unwrap_err()
        .return_code(),
        498
    );
    request.bias_unit_source = None;
    assert_eq!(
        ppml::fit_requested(
            &model,
            &loaded,
            &options,
            &request,
            Some(x),
            Resources::default()
        )
        .unwrap_err()
        .return_code(),
        498
    );
}
#[test]
fn outcome_weight_and_adjustment_bias_bindings_preserve_raw_original_ids() {
    let (loaded, _) = fixture(Some(WeightKind::Frequency));
    let model = model(false);
    let options = options(Solver::Auto, 1);
    let LoadedAdjustment::Exposure(exposure) = &loaded.adjustment else {
        unreachable!()
    };
    for (source, original) in [
        (ppml::BiasUnitSource::Outcome, &loaded.outcome),
        (
            ppml::BiasUnitSource::Weight,
            &loaded.weights.as_ref().unwrap().1,
        ),
        (ppml::BiasUnitSource::Adjustment, exposure),
    ] {
        let explicit = column(original.values.clone());
        let mut request = request(false, Some(bias::Method::Exact));
        let separate = ppml::fit_requested(
            &model,
            &loaded,
            &options,
            &request,
            Some(&explicit),
            Resources::default(),
        )
        .unwrap();
        request.bias_unit_source = Some(source);
        let reused = ppml::fit_requested(
            &model,
            &loaded,
            &options,
            &request,
            None,
            Resources::default(),
        )
        .unwrap();
        same(&separate, &reused);
        assert_eq!(
            separate.bias.as_ref().unwrap().ordinary,
            reused.bias.as_ref().unwrap().ordinary
        );
        assert_eq!(
            separate.bias.as_ref().unwrap().diagnostics,
            reused.bias.as_ref().unwrap().diagnostics
        );
        assert_eq!(
            separate.bias.as_ref().unwrap().table.values[1],
            reused.bias.as_ref().unwrap().table.values[1]
        );
        assert_eq!(
            separate.memory.minimum_bytes() - reused.memory.minimum_bytes(),
            loaded.candidate.len() as u128 * 8
        );
    }
}
