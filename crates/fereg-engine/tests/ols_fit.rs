#![forbid(unsafe_code)]
use fereg_engine::{
    data::*,
    memory::Inputs,
    ols::{self, Call, Phase, Request, Resources},
    outputs::{self, Kind},
    spec::{ClusterTerm, Estimator, Options, Solver},
    threads::ThreadTeam,
    vce,
    wls::Status,
    *,
};
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
fn model(effects: bool, cluster: bool) -> ModelSpec {
    ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=if effects || cluster { 3 } else { 1 })
                .map(|id| SourceEntry {
                    id: SourceId::new(id).unwrap(),
                    canonical_name: format!("s{id}"),
                })
                .collect(),
            terms: vec![term(1, false), term(2, false), term(3, true)],
        },
        absorbed_terms: if effects {
            (2..=3)
                .map(|id| AbsorbedTerm {
                    id: AbsorbTermId::new(id - 1).unwrap(),
                    intercept: true,
                    categorical_sources: vec![SourceId::new(id).unwrap()],
                    slope_sources: vec![],
                    label: format!("g{id}"),
                    saved_name: String::new(),
                })
                .collect()
        } else {
            vec![]
        },
        clusters: if cluster {
            (2..=3)
                .map(|id| ClusterTerm {
                    label: format!("c{id}"),
                    sources: vec![SourceId::new(id).unwrap()],
                })
                .collect()
        } else {
            vec![]
        },
        include_intercept: true,
        keep_singletons: true,
    }
}
fn column(values: Vec<f64>) -> NumericColumn {
    NumericColumn {
        values,
        missing: None,
    }
}
fn loaded(groups: bool, kind: Option<WeightKind>) -> Loaded {
    let n = 160;
    let mut sources = vec![LoadedSource::Numeric(column(
        (0..n).map(|i| ((i * 17) % 101) as f64 / 13.).collect(),
    ))];
    if groups {
        sources.extend([
            LoadedSource::Numeric(column((0..n).map(|i| (i % 4) as f64).collect())),
            LoadedSource::Strings((0..n).map(|i| format!("g{}", (i / 4) % 5)).collect()),
        ]);
    }
    let mut result = Loaded {
        candidate: vec![true; n],
        outcome: column(
            (0..n)
                .map(|i| {
                    2. + 0.7 * ((i * 17) % 101) as f64 / 13.
                        + (i as f64).sin()
                        + if groups {
                            (i % 4) as f64 * 0.2 + ((i / 4) % 5) as f64 * 0.1
                        } else {
                            0.
                        }
                })
                .collect(),
        ),
        sources,
        weights: kind.map(|kind| (kind, column((0..n).map(|i| (i % 3 + 1) as f64).collect()))),
        adjustment: LoadedAdjustment::None,
    };
    result.candidate[0] = false;
    result.outcome.values[1] = f64::NAN;
    let LoadedSource::Numeric(x) = &mut result.sources[0] else {
        unreachable!()
    };
    x.values[2] = f64::NAN;
    result
}
fn options(solver: Solver, threads: usize) -> Options {
    Options {
        solver,
        threads,
        tolerance: 1e-12,
        ..Options::for_estimator(Estimator::Ols)
    }
}
fn requested(effects: bool) -> Request {
    Request {
        outputs: outputs::Request {
            residuals: true,
            xb: true,
            xbd: true,
            absorbed: true,
            drop_reason: true,
            saved: if effects {
                vec![(0, 0), (1, 0)]
            } else {
                vec![(0, 0)]
            },
        },
        ..Request::default()
    }
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 2e-9 * b.abs().max(1.),
        "{a:.17e} != {b:.17e}"
    );
}
fn same(a: &ols::OlsFit, b: &ols::OlsFit) {
    assert_eq!(a.coefficients, b.coefficients);
    assert_eq!(a.statuses, b.statuses);
    assert_eq!(a.covariance.values, b.covariance.values);
    assert_eq!(a.dof, b.dof);
    assert_eq!(a.absorbed_dof, b.absorbed_dof);
    assert_eq!(a.sample, b.sample);
    assert_eq!(a.statistics, b.statistics);
    assert_eq!(a.diagnostics, b.diagnostics);
    assert_eq!(a.wald, b.wald);
}
#[test]
fn complete_fit_matches_closed_form_weighted_regression_and_sandwich() {
    for weight in [
        None,
        Some(WeightKind::Analytic),
        Some(WeightKind::Frequency),
        Some(WeightKind::Probability),
    ] {
        let input = loaded(false, weight);
        let spec = model(false, false);
        let LoadedSource::Numeric(x) = &input.sources[0] else {
            unreachable!()
        };
        let n = input.candidate.len() - 3;
        let raw: Vec<_> = (3..input.candidate.len())
            .map(|i| input.weights.as_ref().map_or(1., |(_, w)| w.values[i]))
            .collect();
        let total: f64 = raw.iter().sum();
        let effective = if weight == Some(WeightKind::Frequency) {
            total
        } else {
            n as f64
        };
        let w: Vec<_> = raw.iter().map(|v| v * effective / total).collect();
        let mx = (3..input.candidate.len())
            .map(|i| w[i - 3] * x.values[i])
            .sum::<f64>()
            / effective;
        let my = (3..input.candidate.len())
            .map(|i| w[i - 3] * input.outcome.values[i])
            .sum::<f64>()
            / effective;
        let sxx = (3..input.candidate.len())
            .map(|i| w[i - 3] * (x.values[i] - mx).powi(2))
            .sum::<f64>();
        let beta = (3..input.candidate.len())
            .map(|i| w[i - 3] * (x.values[i] - mx) * (input.outcome.values[i] - my))
            .sum::<f64>()
            / sxx;
        let cons = my - mx * beta;
        let bread = [
            [1. / sxx, -mx / sxx],
            [-mx / sxx, 1. / effective + mx * mx / sxx],
        ];
        let residuals: Vec<_> = (3..input.candidate.len())
            .map(|i| input.outcome.values[i] - beta * x.values[i] - cons)
            .collect();
        let rss = w
            .iter()
            .zip(&residuals)
            .map(|(w, r)| w * r * r)
            .sum::<f64>();
        for kind in [vce::Kind::Iid, vce::Kind::Robust] {
            if weight == Some(WeightKind::Probability) && kind == vce::Kind::Iid {
                continue;
            }
            let mut request = requested(false);
            request.vce = Some(kind);
            let mut baseline = None;
            for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
                let fit = ols::fit_requested(
                    &spec,
                    &input,
                    &options(solver, 1),
                    &request,
                    Resources::default(),
                )
                .unwrap();
                close(fit.coefficients[0], beta);
                close(fit.coefficients[3], cons);
                close(fit.statistics.rss, rss);
                assert_eq!(
                    fit.statuses,
                    [
                        Status::Kept,
                        Status::NumericallyRankDeficient,
                        Status::ExplicitlyOmitted,
                        Status::NormalizedAbsorbedIntercept
                    ]
                );
                assert_eq!(fit.dof.structural, effective as usize - 2);
                assert_eq!(fit.dof.finite, 1);
                assert_eq!(fit.wald.tested, [0]);
                let mut expected = [[0.; 2]; 2];
                for a in 0..2 {
                    for b in 0..2 {
                        expected[a][b] = if kind == vce::Kind::Iid {
                            bread[a][b] * rss / (effective - 2.)
                        } else {
                            (3..input.candidate.len())
                                .map(|i| {
                                    let influence = [
                                        bread[0][0] * x.values[i] + bread[0][1],
                                        bread[1][0] * x.values[i] + bread[1][1],
                                    ];
                                    let score_weight = if weight == Some(WeightKind::Frequency) {
                                        w[i - 3]
                                    } else {
                                        w[i - 3].powi(2)
                                    };
                                    influence[a]
                                        * influence[b]
                                        * residuals[i - 3].powi(2)
                                        * score_weight
                                })
                                .sum::<f64>()
                                * effective
                                / (effective - 2.)
                        };
                        close(
                            fit.covariance.values[([0, 3][a], [0, 3][b])],
                            expected[a][b],
                        );
                    }
                }
                close(fit.t_statistics[3], cons / expected[1][1].sqrt());
                assert!(fit.t_statistics[1].is_nan());
                assert_eq!(fit.sample.retained_rows, (3..160).collect::<Vec<_>>());
                for i in 0..160 {
                    let r = fit.outputs.get(Kind::Residuals).unwrap()[i];
                    if i < 3 {
                        assert!(r.is_nan());
                    } else {
                        close(r, residuals[i - 3]);
                    }
                }
                if let Some(first) = &baseline {
                    close(fit.coefficients[0], *first);
                } else {
                    baseline = Some(beta);
                }
            }
        }
        let fit = ols::fit(&spec, &input, &options(Solver::Auto, 1)).unwrap();
        assert_eq!(
            fit.vce,
            if weight == Some(WeightKind::Probability) {
                vce::Kind::Robust
            } else {
                vce::Kind::Iid
            }
        );
        assert!(fit.outputs.columns.is_empty());
        assert!(fit.outputs.certificate.is_none());
    }
}
#[test]
fn row_outputs_and_worker_count_do_not_change_the_accepted_estimator() {
    for cluster in [false, true] {
        for weight in [
            None,
            Some(WeightKind::Frequency),
            Some(WeightKind::Probability),
        ] {
            let spec = model(true, cluster);
            let input = loaded(true, weight);
            let request = requested(true);
            for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
                let base = ols::fit(&spec, &input, &options(solver, 1)).unwrap();
                for workers in [1, 2, 4, 8] {
                    let fit = ols::fit_requested(
                        &spec,
                        &input,
                        &options(solver, workers),
                        &request,
                        Resources::default(),
                    )
                    .unwrap();
                    same(&base, &fit);
                    assert_eq!(
                        fit.vce,
                        if cluster {
                            vce::Kind::Cluster
                        } else if weight == Some(WeightKind::Probability) {
                            vce::Kind::Robust
                        } else {
                            vce::Kind::Iid
                        }
                    );
                    assert_eq!(fit.requested_solver, solver);
                    for i in 3..160 {
                        let get = |kind| fit.outputs.get(kind).unwrap()[i];
                        close(
                            get(Kind::Xbd) + get(Kind::Residuals),
                            input.outcome.values[i],
                        );
                        close(get(Kind::Xbd), get(Kind::Xb) + get(Kind::Absorbed));
                        close(
                            get(Kind::Absorbed),
                            get(Kind::Saved {
                                effect: 0,
                                channel: 0,
                            }) + get(Kind::Saved {
                                effect: 1,
                                channel: 0,
                            }),
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn one_preflight_precedes_loading_and_rejects_changed_banks_or_capacity() {
    let spec = model(true, false);
    let input = loaded(true, None);
    let request = requested(true);
    let options = options(Solver::Lsmr, 1);
    let inputs = Inputs::from_loaded(&input, 0, 0).unwrap();
    let call = Call::new(
        &spec,
        &options,
        &request,
        inputs,
        None,
        Resources::default(),
    )
    .unwrap();
    let minimum = call.memory().minimum_bytes();
    let estimate = call.memory().clone();
    assert!(
        Call::new(
            &spec,
            &options,
            &request,
            inputs,
            None,
            Resources {
                usable_bytes: Some(minimum - 1),
                ..Resources::default()
            }
        )
        .err()
        .unwrap()
        .return_code()
            == 909
    );
    let call = Call::new(
        &spec,
        &options,
        &request,
        inputs,
        None,
        Resources {
            usable_bytes: Some(minimum),
            ..Resources::default()
        },
    )
    .unwrap();
    let fit = call.run(&input).unwrap();
    assert_eq!(fit.memory, estimate);
    for altered in [
        Inputs {
            original_rows: 159,
            active_rows: 157,
            ..inputs
        },
        Inputs {
            active_rows: 156,
            ..inputs
        },
        Inputs {
            string_sources: 0,
            ..inputs
        },
    ] {
        let call = Call::new(
            &spec,
            &options,
            &request,
            altered,
            None,
            Resources::default(),
        )
        .unwrap();
        assert_eq!(
            call.run(&input).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
    }
    let call = Call::new(
        &spec,
        &options,
        &request,
        inputs,
        None,
        Resources::default(),
    )
    .unwrap();
    assert_eq!(
        call.run_with_team(&input, &ThreadTeam::new(2).unwrap(), &mut |_| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    let mut oversized = loaded(true, None);
    oversized.outcome.values.reserve(100_000);
    let call = Call::new(
        &spec,
        &options,
        &request,
        inputs,
        None,
        Resources::default(),
    )
    .unwrap();
    assert_eq!(
        call.run(&oversized).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    let huge = Resources {
        publication_matrix_cells: usize::MAX,
        ..Resources::default()
    };
    assert_eq!(
        Call::new(&spec, &options, &request, inputs, None, huge)
            .err()
            .unwrap()
            .code,
        ErrorCode::Allocation
    );
}
#[test]
fn every_interrupt_boundary_discards_the_call_and_fresh_fit_succeeds() {
    let spec = model(true, false);
    let input = loaded(true, None);
    let request = requested(true);
    let options = options(Solver::Auto, 2);
    let caller = std::thread::current().id();
    for stop in [
        Phase::Loaded,
        Phase::Prepared,
        Phase::Estimated,
        Phase::Inferred,
        Phase::Outputs,
    ] {
        let call = Call::new(
            &spec,
            &options,
            &request,
            Inputs::from_loaded(&input, 0, 0).unwrap(),
            None,
            Resources::default(),
        )
        .unwrap();
        let mut reached = false;
        assert_eq!(
            call.run_with(&input, &mut |phase| {
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
        ols::fit(&spec, &input, &options).unwrap();
    }
    assert_eq!(input.candidate.len(), 160);
    assert!(input.outcome.values[1].is_nan());
}
#[test]
fn unsupported_requests_and_invalid_products_fail_before_estimation() {
    let spec = model(false, false);
    let input = loaded(false, Some(WeightKind::Probability));
    let request = Request {
        vce: Some(vce::Kind::Iid),
        ..Request::default()
    };
    assert_eq!(
        ols::fit_requested(
            &spec,
            &input,
            &options(Solver::Auto, 1),
            &request,
            Resources::default()
        )
        .unwrap_err()
        .return_code(),
        498
    );
    for saved in [vec![(0, 0), (0, 0)], vec![(0, 1)], vec![(1, 0)]] {
        let request = Request {
            outputs: outputs::Request {
                saved,
                ..outputs::Request::default()
            },
            ..Request::default()
        };
        assert_eq!(
            ols::fit_requested(
                &spec,
                &input,
                &options(Solver::Auto, 1),
                &request,
                Resources::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidArgument
        );
    }
    let mut empty = loaded(false, None);
    empty.candidate.fill(false);
    assert_eq!(
        ols::fit(&spec, &empty, &options(Solver::Auto, 1))
            .unwrap_err()
            .return_code(),
        2001
    );
}
