#![forbid(unsafe_code)]
use fereg_engine::{
    data::*,
    memory::Inputs,
    outputs::{self, Kind},
    ppml::{self, bias, separation, Call, InterruptPoint, Phase, Request, Resources},
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
fn interrupted_fixture() -> (Loaded, NumericColumn) {
    let (mut loaded, units) = fixture(Some(WeightKind::Frequency));
    let LoadedSource::Numeric(x) = &mut loaded.sources[0] else {
        unreachable!()
    };
    // Opposite boundary contrasts share both FE levels. They need phase-I
    // rather than a single signed ray, and neither row may be separated.
    x.values[3..].fill(0.);
    x.values[3] = 1.;
    x.values[23] = -1.;
    loaded.outcome.values[3] = 0.;
    loaded.outcome.values[23] = 0.;
    (loaded, units)
}
fn same_outputs(a: &outputs::Outputs, b: &outputs::Outputs) {
    assert_eq!(a.certificate, b.certificate);
    assert_eq!(a.columns.len(), b.columns.len());
    for (a, b) in a.columns.iter().zip(&b.columns) {
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.values.len(), b.values.len());
        for (a, b) in a.values.iter().zip(&b.values) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }
}
fn same_products(a: &ppml::PpmlFit, b: &ppml::PpmlFit) {
    same_outputs(&a.outputs, &b.outputs);
    match (&a.bias, &b.bias) {
        (Some(a), Some(b)) => {
            assert_eq!(a.request, b.request);
            assert_eq!(a.ordinary, b.ordinary);
            assert_eq!(a.support, b.support);
            assert_eq!(a.first_levels, b.first_levels);
            assert_eq!(a.second_levels, b.second_levels);
            assert_eq!(a.finite_slots, b.finite_slots);
            assert_eq!(a.diagnostics, b.diagnostics);
            same_outputs(&a.saved, &b.saved);
            for row in (0..21).filter(|&row| row != 9 && row != 10) {
                assert_eq!(a.table.values[row], b.table.values[row]);
            }
        }
        (None, None) => (),
        _ => panic!("different bias products"),
    }
}
#[test]
fn numerical_callbacks_preserve_all_solvers_bias_outputs_and_worker_identity() {
    use std::{cell::Cell, rc::Rc};
    let (loaded, units) = interrupted_fixture();
    let model = model(true);
    let mut request = request(true, Some(bias::Method::Exact));
    request.separation = separation::Methods::parse("fe simplex ir mu").unwrap();
    let inputs = Inputs::from_loaded(&loaded, 0, 0)
        .unwrap()
        .with_numeric_auxiliary(&units)
        .unwrap();
    let caller = std::thread::current().id();
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        let mut first_trace = None;
        let mut first_fit = None;
        for workers in [1, 8] {
            let options = options(solver, workers);
            let plain = ppml::fit_requested(
                &model,
                &loaded,
                &options,
                &request,
                Some(&units),
                Resources::default(),
            )
            .unwrap();
            let count = Rc::new(Cell::new(0));
            let mut trace = vec![];
            let polled = Call::new(
                &model,
                &options,
                &request,
                inputs,
                Some(WeightKind::Frequency),
                Resources::default(),
            )
            .unwrap()
            .run_with(&loaded, Some(&units), &mut |phase| {
                assert_eq!(std::thread::current().id(), caller);
                count.set(count.get() + 1);
                if let Phase::Numerical(point) = phase {
                    trace.push(point);
                }
                Ok(())
            })
            .unwrap();
            assert!(count.get() > trace.len());
            same(&plain, &polled);
            same_products(&plain, &polled);
            assert_eq!(plain.separation.generation, 0);
            for point in [
                InterruptPoint::SeparationRound,
                InterruptPoint::SimplexModel,
                InterruptPoint::SimplexReduction,
                InterruptPoint::SimplexPivot,
                InterruptPoint::RectifierIteration,
                InterruptPoint::MuIteration,
                InterruptPoint::IrlsIteration,
                InterruptPoint::LineSearch,
                InterruptPoint::LinearFrame,
                InterruptPoint::StrictCheckpoint,
                InterruptPoint::FixedEffects,
                InterruptPoint::DenseLinearAlgebra,
            ] {
                assert!(trace.contains(&point), "missing {point:?}: {trace:?}");
            }
            if let Some(first) = &first_trace {
                assert_eq!(first, &trace);
            } else {
                first_trace = Some(trace);
            }
            if let Some(first) = &first_fit {
                same(first, &polled);
                same_products(first, &polled);
            } else {
                first_fit = Some(polled);
            }
        }
    }
}
#[test]
fn every_outer_numerical_boundary_aborts_without_a_result_and_allows_fresh_restart() {
    let (loaded, _) = interrupted_fixture();
    let model = model(false);
    let mut request = request(true, None);
    request.separation = separation::Methods::parse("fe simplex ir mu").unwrap();
    let options = options(Solver::Auto, 8);
    let team = ThreadTeam::new(8).unwrap();
    let inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
    let make = || {
        Call::new(
            &model,
            &options,
            &request,
            inputs,
            Some(WeightKind::Frequency),
            Resources::default(),
        )
        .unwrap()
    };
    let caller = std::thread::current().id();
    // FE and dense actions have exhaustive kernel cancellation tests. Replaying
    // the full caller relay after each inner poll grows quadratically; complete
    // calls cover those actions separately in each separation/IRLS/bias region.
    let outer = |point| {
        !matches!(
            point,
            InterruptPoint::FixedEffects | InterruptPoint::DenseLinearAlgebra
        )
    };
    let mut trace = vec![];
    let baseline = make()
        .run_with_team(&loaded, None, &team, &mut |phase| {
            match phase {
                Phase::Numerical(point) if outer(point) => trace.push(point),
                _ => (),
            }
            Ok(())
        })
        .unwrap();
    assert!(trace.len() > 20);
    eprintln!(
        "{} complete-call outer numerical checkpoints qualified",
        trace.len()
    );
    for stop in 0..trace.len() {
        let mut reached = 0;
        let error = make()
            .run_with_team(&loaded, None, &team, &mut |phase| {
                assert_eq!(std::thread::current().id(), caller);
                match phase {
                    Phase::Numerical(point) if outer(point) => {
                        assert_eq!(point, trace[reached]);
                        reached += 1;
                        if reached == stop + 1 {
                            return Err(FeregError::new(
                                ErrorCode::Interrupted,
                                "outer fixture Break",
                            ));
                        }
                    }
                    _ => (),
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.return_code(), 1);
        assert_eq!(reached, stop + 1);
        let retry = make()
            .run_with_team(&loaded, None, &team, &mut |_| Ok(()))
            .unwrap();
        same(&baseline, &retry);
        same_products(&baseline, &retry);
    }
    for stop in [0, trace.len() / 2, trace.len() - 1] {
        let mut reached = 0;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            make().run_with_team(&loaded, None, &team, &mut |phase| {
                match phase {
                    Phase::Numerical(point) if outer(point) => {
                        reached += 1;
                        assert_ne!(reached, stop + 1, "caller callback panic");
                    }
                    _ => (),
                }
                Ok(())
            })
        }));
        assert!(panic.is_err());
        same(
            &baseline,
            &make()
                .run_with_team(&loaded, None, &team, &mut |_| Ok(()))
                .unwrap(),
        );
    }
}
#[test]
fn break_after_ledger_deletion_and_during_capacity_resolution_keeps_inputs_intact() {
    for capacity_skip in [false, true] {
        let (mut loaded, _) = if capacity_skip {
            interrupted_fixture()
        } else {
            fixture(None)
        };
        let mut model = model(false);
        let mut request = request(true, None);
        if capacity_skip {
            request.separation = separation::Methods::parse("simplex ir mu").unwrap();
            request.separation_controls.simplex.max_cells = 1;
        } else {
            model.keep_singletons = false;
            let LoadedSource::Numeric(g) = &mut loaded.sources[1] else {
                unreachable!()
            };
            g.values[117] = 98.;
            g.values[118..].fill(99.);
            loaded.outcome.values[118..].fill(0.);
        }
        let options = options(Solver::Auto, 8);
        let team = ThreadTeam::new(8).unwrap();
        let inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
        let make = || {
            Call::new(
                &model,
                &options,
                &request,
                inputs,
                loaded.weights.as_ref().map(|(kind, _)| *kind),
                Resources::default(),
            )
            .unwrap()
        };
        let mut trace = vec![];
        let baseline = make()
            .run_with_team(&loaded, None, &team, &mut |phase| {
                if let Phase::Numerical(point) = phase {
                    trace.push(point);
                }
                Ok(())
            })
            .unwrap();
        let stop_point = if capacity_skip {
            assert_eq!(baseline.separation.capacity_resolutions, 1);
            InterruptPoint::RectifierIteration
        } else {
            assert_eq!(baseline.separation.generation, 2);
            InterruptPoint::SeparationRound
        };
        let stops: Vec<_> = trace
            .iter()
            .enumerate()
            .filter_map(|(i, &p)| (p == stop_point).then_some(i))
            .collect();
        assert!(stops.len() >= 2);
        for stop in stops.into_iter().skip(1) {
            let mut polls = 0;
            let error = make()
                .run_with_team(&loaded, None, &team, &mut |phase| {
                    if let Phase::Numerical(_) = phase {
                        polls += 1;
                        if polls == stop + 1 {
                            return Err(FeregError::new(
                                ErrorCode::Interrupted,
                                "ledger/replay Break",
                            ));
                        }
                    }
                    Ok(())
                })
                .unwrap_err();
            assert_eq!(error.return_code(), 1);
            assert_eq!(polls, stop + 1);
            let retry = make()
                .run_with_team(&loaded, None, &team, &mut |_| Ok(()))
                .unwrap();
            same(&baseline, &retry);
            same_products(&baseline, &retry);
        }
    }
}
#[test]
fn callback_errors_cannot_be_treated_as_a_stalled_mu_trajectory() {
    let mut request = request(false, None);
    request.separation = separation::Methods::only(separation::Method::Mu);
    let options = options(Solver::Auto, 8);
    for effects in [1, 2] {
        let (mut loaded, _) = interrupted_fixture();
        let mut model = model(false);
        model.absorbed_terms.truncate(effects);
        model.model.sources.truncate(effects + 1);
        loaded.sources.truncate(effects + 1);
        for stop in [1, 2] {
            for code in [
                ErrorCode::Interrupted,
                ErrorCode::Allocation,
                ErrorCode::NonConvergence,
            ] {
                let mut line_searches = 0;
                let error = Call::new(
                    &model,
                    &options,
                    &request,
                    Inputs::from_loaded(&loaded, 0, 0).unwrap(),
                    Some(WeightKind::Frequency),
                    Resources::default(),
                )
                .unwrap()
                .run_with(&loaded, None, &mut |phase| {
                    if phase == Phase::Numerical(InterruptPoint::LineSearch) {
                        line_searches += 1;
                        if line_searches == stop {
                            return Err(FeregError::new(code, "fatal poll sentinel"));
                        }
                    }
                    Ok(())
                })
                .unwrap_err();
                assert_eq!(line_searches, stop);
                assert_eq!(error.code, code);
                assert!(error.message.contains("fatal poll sentinel"));
            }
        }
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

#[test]
fn fe_and_dense_cancellation_cover_separation_final_irls_and_both_bias_modes() {
    use std::{cell::Cell, rc::Rc};
    let (loaded, units) = interrupted_fixture();
    let model = model(false);
    let caller = std::thread::current().id();
    for method in [
        bias::Method::Exact,
        bias::Method::Randomized {
            probes: 4,
            seed: 739391,
            stream: 9,
        },
    ] {
        let mut request = request(true, Some(method));
        request.separation = separation::Methods::parse("fe simplex ir mu").unwrap();
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            let mut reference = None;
            for workers in [1, 8] {
                let options = options(solver, workers);
                let team = ThreadTeam::new(workers).unwrap();
                let inputs = Inputs::from_loaded(&loaded, 0, 0)
                    .unwrap()
                    .with_numeric_auxiliary(&units)
                    .unwrap();
                let make = || {
                    Call::new(
                        &model,
                        &options,
                        &request,
                        inputs,
                        Some(WeightKind::Frequency),
                        Resources::default(),
                    )
                    .unwrap()
                };
                let plain = make().run(&loaded, Some(&units)).unwrap();
                for checkpoint in [
                    InterruptPoint::FixedEffects,
                    InterruptPoint::DenseLinearAlgebra,
                ] {
                    let counts = Rc::new([Cell::new(0_usize), Cell::new(0), Cell::new(0)]);
                    let mut region = 0;
                    let polled = make()
                        .run_with_team(&loaded, Some(&units), &team, &mut |phase| {
                            assert_eq!(std::thread::current().id(), caller);
                            match phase {
                                Phase::Separated => region = 1,
                                Phase::Outputs => region = 2,
                                Phase::Numerical(point) if point == checkpoint => {
                                    counts[region].set(counts[region].get() + 1)
                                }
                                _ => (),
                            }
                            Ok(())
                        })
                        .unwrap();
                    same(&plain, &polled);
                    same_products(&plain, &polled);
                    assert!(counts.iter().all(|c| c.get() > 2));
                    if let Some(reference) = &reference {
                        same(reference, &polled);
                        same_products(reference, &polled);
                    } else {
                        reference = Some(polled);
                    }
                    for cancelled_region in 0..3 {
                        for stop in [
                            1,
                            counts[cancelled_region].get() / 2,
                            counts[cancelled_region].get(),
                        ] {
                            let sentinel = FeregError::new(
                                ErrorCode::NonConvergence,
                                "host FE callback r430 must abort",
                            );
                            let mut calls = 0;
                            let mut region = 0;
                            let error = make()
                                .run_with_team(&loaded, Some(&units), &team, &mut |phase| {
                                    match phase {
                                        Phase::Separated => region = 1,
                                        Phase::Outputs => region = 2,
                                        Phase::Numerical(point)
                                            if point == checkpoint
                                                && region == cancelled_region =>
                                        {
                                            calls += 1;
                                            if calls == stop {
                                                return Err(sentinel.clone());
                                            }
                                        }
                                        _ => (),
                                    }
                                    Ok(())
                                })
                                .unwrap_err();
                            assert_eq!(error, sentinel);
                            assert_eq!(calls, stop);
                            let retry = make().run(&loaded, Some(&units)).unwrap();
                            same(&plain, &retry);
                            same_products(&plain, &retry);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn inference_cancellation_keeps_host_error_identity_and_accepted_ppml_results() {
    use std::{cell::Cell, rc::Rc};
    let (loaded, _) = fixture(Some(WeightKind::Frequency));
    let caller = std::thread::current().id();
    for clustered in [false, true] {
        let model = model(clustered);
        let request = request(true, None);
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            for workers in [1, 8] {
                let options = options(solver, workers);
                let team = ThreadTeam::new(workers).unwrap();
                let make = || {
                    Call::new(
                        &model,
                        &options,
                        &request,
                        Inputs::from_loaded(&loaded, 0, 0).unwrap(),
                        Some(WeightKind::Frequency),
                        Resources::default(),
                    )
                    .unwrap()
                };
                let plain = make().run(&loaded, None).unwrap();
                let count = Rc::new(Cell::new(0_usize));
                let mut region = false;
                let polled = make()
                    .run_with_team(&loaded, None, &team, &mut |phase| {
                        assert_eq!(std::thread::current().id(), caller);
                        match phase {
                            Phase::Estimated => region = true,
                            Phase::Inferred => region = false,
                            Phase::Numerical(InterruptPoint::DenseLinearAlgebra) if region => {
                                count.set(count.get() + 1)
                            }
                            _ => (),
                        }
                        Ok(())
                    })
                    .unwrap();
                same(&plain, &polled);
                same_products(&plain, &polled);
                assert!(count.get() > 2);
                for stop in [1, count.get() / 2, count.get()] {
                    let sentinel =
                        FeregError::new(ErrorCode::NonConvergence, "PPML covariance host sentinel");
                    let mut calls = 0;
                    let mut region = false;
                    let error = make()
                        .run_with_team(&loaded, None, &team, &mut |phase| {
                            match phase {
                                Phase::Estimated => region = true,
                                Phase::Inferred => region = false,
                                Phase::Numerical(InterruptPoint::DenseLinearAlgebra) if region => {
                                    calls += 1;
                                    if calls == stop {
                                        return Err(sentinel.clone());
                                    }
                                }
                                _ => (),
                            }
                            Ok(())
                        })
                        .unwrap_err();
                    assert_eq!(error, sentinel);
                    assert_eq!(calls, stop);
                    let retry = make().run(&loaded, None).unwrap();
                    same(&plain, &retry);
                    same_products(&plain, &retry);
                }
            }
        }
    }
}
