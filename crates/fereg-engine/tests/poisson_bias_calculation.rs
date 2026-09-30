use fereg_engine::{
    data::*,
    outputs::Kind,
    ppml::{bias, inference, irls},
    spec::{metadata::*, Estimator, Options, Solver},
    statistics,
    threads::ThreadTeam,
    vce, AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ErrorCode, FeregError, ModelSpec,
};
mod support;
fn model(sample: &Prepared) -> ModelSpec {
    let terms = ["x", "duplicate", "omitted"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| ExpandedTerm {
            id: TermId::new(i as u32 + 1).unwrap(),
            order: TermOrder::new(i as u32 + 1).unwrap(),
            coefficient_slot: CoefficientSlot::new(i as u32 + 1).unwrap(),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: i == 2,
            canonical_token: name.into(),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).unwrap(),
                source: SourceId::new(if i == 2 { 4 } else { 3 }).unwrap(),
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".into(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        })
        .collect();
    ModelSpec {
        model: CompiledTermGraph {
            sources: (0..sample.regressors.len())
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("s{i}"),
                })
                .collect(),
            terms,
        },
        absorbed_terms: (0..2)
            .map(|i| AbsorbedTerm {
                id: AbsorbTermId::new(i + 1).unwrap(),
                categorical_sources: vec![SourceId::new(i + 1).unwrap()],
                intercept: true,
                slope_sources: vec![],
                label: format!("g{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    }
}
fn fixture(weight: Option<WeightKind>) -> (Prepared, ModelSpec, Vec<f64>) {
    let mut sample = support::prepared(&[6, 4], false, 20, 2);
    let n = sample.n_active();
    sample.regressors[2] = (0..n).map(|i| (i as f64 * 1.7).sin()).collect();
    sample.outcome = (0..n).map(|i| 0.5 + ((i * 7) % 11) as f64 * 0.5).collect();
    sample.weights = weight.map_or(PreparedWeights::None, |kind| PreparedWeights::Provided {
        kind,
        values: (0..n).map(|i| (1 + i % 5) as f64).collect(),
    });
    sample.adjustment = PreparedAdjustment::Exposure(
        (0..n)
            .map(|i| ((i % 7) as f64 * 0.08 - 0.24).exp())
            .collect(),
    );
    sample.n_original = 2 * n + 3;
    sample.n_candidate = 2 * n + 2;
    sample.original_row_by_active = (0..n).map(|i| 2 * i + 1).collect();
    sample.drop_reasons = vec![DropReason::MissingOutcome; sample.n_original];
    sample.drop_reasons[0] = DropReason::ExcludedByIfIn;
    let mut units = vec![f64::NAN; sample.n_original];
    for (i, &row) in sample.original_row_by_active.iter().enumerate() {
        sample.drop_reasons[row] = DropReason::Retained;
        units[row] = (i % 7) as f64 - 3.5;
    }
    let model = model(&sample);
    (sample, model, units)
}
fn stable(mut result: bias::Correction) -> bias::Correction {
    // Elapsed time is a resource diagnostic, not a numerical result.
    result.elapsed_seconds = 0.;
    for column in &mut result.saved.columns {
        for value in &mut column.values {
            if value.is_nan() {
                *value = -999.;
            }
        }
    }
    result
}
#[test]
fn exact_and_fixed_probes_preserve_ordinary_results_slots_and_worker_identity() {
    for weight in [
        None,
        Some(WeightKind::Frequency),
        Some(WeightKind::Probability),
    ] {
        let (sample, model, units) = fixture(weight);
        let columns = CompiledColumns::new(&model, Estimator::Ppml).unwrap();
        let raw = columns.generate(&sample).unwrap();
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            for method in [
                bias::Method::Exact,
                bias::Method::Randomized {
                    probes: 2,
                    seed: 17,
                    stream: 3,
                },
                bias::Method::Randomized {
                    probes: 17,
                    seed: 739391,
                    stream: 0,
                },
            ] {
                let mut previous = None;
                for workers in [1, 2, 4, 8] {
                    let team = ThreadTeam::new(workers).unwrap();
                    let fit = irls::fit_final_sample(
                        &irls::Problem {
                            sample: &sample,
                            raw: &raw,
                            roles: &columns.roles,
                        },
                        Options {
                            solver,
                            ..Options::for_estimator(Estimator::Ppml)
                        },
                        &team,
                    )
                    .unwrap();
                    let before = statistics::ppml(&fit).unwrap();
                    let covariance =
                        inference::estimate(&columns, &fit, &[], vce::Kind::Robust, 1e-8, &team)
                            .unwrap()
                            .covariance
                            .values;
                    let result = bias::compute(
                        &fit,
                        &model,
                        Some(bias::Request {
                            method,
                            save_effects: true,
                        }),
                        Some(NumericColumnView::new(&units)),
                        &team,
                        &mut || Ok(()),
                    )
                    .unwrap()
                    .unwrap();
                    assert_eq!(result.stripes, ["x", "duplicate", "omitted", "_cons"]);
                    assert_eq!(result.finite_slots, [0]);
                    assert_eq!(
                        result.groups,
                        if method == bias::Method::Exact {
                            1
                        } else {
                            result.probes.min(8)
                        }
                    );
                    assert_eq!(result.diagnostics.solves, result.probes + 2 * result.groups);
                    assert_eq!(result.diagnostics.raw_recoveries, 1);
                    assert_eq!(result.saved.columns.len(), 18);
                    assert_eq!(result.ordinary[(0, 0)], fit.point.finite[0]);
                    for channel in bias::Channel::ALL {
                        for slot in 1..4 {
                            assert_eq!(result.ordinary[(channel as usize, slot)], 0.);
                        }
                        for effect in 0..2 {
                            let values = result.saved.get(Kind::Bias { effect, channel }).unwrap();
                            for (&value, &reason) in values.iter().zip(&sample.drop_reasons) {
                                assert_eq!(value.is_finite(), reason == DropReason::Retained);
                            }
                            let topology = sample.topology.as_ref().unwrap();
                            let partition = topology.effect_partition(effect).unwrap();
                            for level in 0..partition.n_levels() {
                                let row = partition.membership().members(level).unwrap()[0];
                                let coordinate = if effect == 0 {
                                    level
                                } else {
                                    result.first_levels + level
                                };
                                assert_eq!(
                                    values[sample.retained_rows()[row]],
                                    result.coefficients.value(coordinate, channel).unwrap()
                                );
                            }
                        }
                    }
                    for channel in [bias::Channel::Derivative, bias::Channel::Curvature] {
                        assert!(
                            (0..result.coefficients.dimension()).any(|j| result
                                .coefficients
                                .value(j, channel)
                                .unwrap()
                                .abs()
                                > 1e-5),
                            "{weight:?}/{solver:?}/{method:?}/{channel:?}"
                        );
                    }
                    if method == bias::Method::Exact {
                        for coordinate in 0..result.coefficients.dimension() {
                            for channel in [
                                bias::Channel::SeDerivative,
                                bias::Channel::SeCurvature,
                                bias::Channel::Covariance,
                                bias::Channel::SeTotal,
                            ] {
                                assert_eq!(
                                    result.coefficients.value(coordinate, channel).unwrap(),
                                    0.
                                );
                            }
                        }
                    }
                    assert_eq!(before, statistics::ppml(&fit).unwrap());
                    assert_eq!(
                        covariance,
                        inference::estimate(&columns, &fit, &[], vce::Kind::Robust, 1e-8, &team)
                            .unwrap()
                            .covariance
                            .values
                    );
                    let result = stable(result);
                    if let Some(prior) = &previous {
                        assert_eq!(prior, &result);
                    } else {
                        previous = Some(result);
                    }
                }
            }
        }
    }
}
#[test]
fn option_off_and_refusals_never_publish_partial_correction() {
    let (sample, model, units) = fixture(None);
    let columns = CompiledColumns::new(&model, Estimator::Ppml).unwrap();
    let raw = columns.generate(&sample).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let fit = irls::fit_final_sample(
        &irls::Problem {
            sample: &sample,
            raw: &raw,
            roles: &columns.roles,
        },
        Options::for_estimator(Estimator::Ppml),
        &team,
    )
    .unwrap();
    let mut invalid_model = model.clone();
    invalid_model.absorbed_terms.clear();
    assert!(
        bias::compute(&fit, &invalid_model, None, None, &team, &mut || panic!(
            "option-off callback"
        ))
        .unwrap()
        .is_none()
    );
    assert_eq!(
        bias::compute(
            &fit,
            &model,
            None,
            Some(NumericColumnView::new(&units)),
            &team,
            &mut || panic!("ancillary callback")
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidArgument
    );
    let request = bias::Request {
        method: bias::Method::Exact,
        save_effects: true,
    };
    assert_eq!(
        bias::compute(&fit, &model, Some(request), None, &team, &mut || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    let request_short = bias::Request {
        method: bias::Method::Randomized {
            probes: 1,
            seed: 0,
            stream: 0,
        },
        ..request
    };
    assert_eq!(
        bias::compute(
            &fit,
            &model,
            Some(request_short),
            Some(NumericColumnView::new(&units)),
            &team,
            &mut || Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidArgument
    );
    invalid_model = model.clone();
    invalid_model.model.terms[2].raw_kind = RawTermKind::Factor;
    let component = &mut invalid_model.model.terms[2].components[0];
    component.kind = ComponentKind::Categorical;
    component.level_state = LevelState::Present;
    component.level_value = 2.;
    component.raw_operator = "2o".into();
    let error = bias::compute(
        &fit,
        &invalid_model,
        Some(request),
        Some(NumericColumnView::new(&units)),
        &team,
        &mut || Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Unsupported, "{error}");
    assert!(error.message.contains("including omitted terms"));
    invalid_model = model.clone();
    invalid_model.absorbed_terms.swap(0, 1);
    assert_eq!(
        bias::compute(
            &fit,
            &invalid_model,
            Some(request),
            Some(NumericColumnView::new(&units)),
            &team,
            &mut || Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidArgument
    );
    let mut visits = 0;
    bias::compute(
        &fit,
        &model,
        Some(request),
        Some(NumericColumnView::new(&units)),
        &team,
        &mut || {
            visits += 1;
            Ok(())
        },
    )
    .unwrap();
    for stop in 1..=visits {
        let mut calls = 0;
        let error = bias::compute(
            &fit,
            &model,
            Some(request),
            Some(NumericColumnView::new(&units)),
            &team,
            &mut || {
                calls += 1;
                if calls == stop {
                    Err(FeregError::new(ErrorCode::Interrupted, "Break"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert_eq!(calls, stop);
    }
}

#[test]
fn frequency_replication_keeps_unit_dependence_and_both_increments() {
    let (weighted, model, units) = fixture(Some(WeightKind::Frequency));
    let PreparedWeights::Provided {
        values: weights, ..
    } = &weighted.weights
    else {
        unreachable!()
    };
    let rows: Vec<_> = weights
        .iter()
        .enumerate()
        .flat_map(|(i, &w)| std::iter::repeat(i).take(w as usize))
        .collect();
    let PreparedAdjustment::Exposure(exposure) = &weighted.adjustment else {
        unreachable!()
    };
    let n = rows.len();
    let replicated = fereg_engine::topology::prepare_fixed_effects(
        &model,
        Prepared {
            topology: None,
            n_original: n,
            n_candidate: n,
            original_row_by_active: (0..n).collect(),
            outcome: rows.iter().map(|&i| weighted.outcome[i]).collect(),
            regressors: weighted
                .regressors
                .iter()
                .map(|column| rows.iter().map(|&i| column[i]).collect())
                .collect(),
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::Exposure(rows.iter().map(|&i| exposure[i]).collect()),
            drop_reasons: vec![DropReason::Retained; n],
        },
    )
    .unwrap();
    let replicated_units: Vec<_> = rows
        .iter()
        .map(|&i| units[weighted.retained_rows()[i]])
        .collect();
    let columns = CompiledColumns::new(&model, Estimator::Ppml).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let a = columns.generate(&weighted).unwrap();
    let b = columns.generate(&replicated).unwrap();
    let fit = |sample, raw| {
        irls::fit_final_sample(
            &irls::Problem {
                sample,
                raw,
                roles: &columns.roles,
            },
            Options::for_estimator(Estimator::Ppml),
            &team,
        )
        .unwrap()
    };
    let one = fit(&weighted, &a);
    let copies = fit(&replicated, &b);
    for method in [bias::Method::Exact, bias::Method::randomized_default()] {
        let request = Some(bias::Request {
            method,
            save_effects: false,
        });
        let left = bias::compute(
            &one,
            &model,
            request,
            Some(NumericColumnView::new(&units)),
            &team,
            &mut || Ok(()),
        )
        .unwrap()
        .unwrap();
        let right = bias::compute(
            &copies,
            &model,
            request,
            Some(NumericColumnView::new(&replicated_units)),
            &team,
            &mut || Ok(()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(left.units, right.units);
        for effect in 0..2 {
            assert_eq!(
                &left.support[effect].values()[..8],
                &right.support[effect].values()[..8]
            );
        }
        for coordinate in 0..left.coefficients.dimension() {
            for channel in bias::Channel::ALL {
                let a = left.coefficients.value(coordinate, channel).unwrap();
                let b = right.coefficients.value(coordinate, channel).unwrap();
                assert!(
                    (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
                    "{method:?}/{channel:?}/{coordinate}: {a} != {b}"
                );
            }
        }
    }
}

#[test]
fn exact_limits_refuse_work_without_changing_randomized_support() {
    let team = ThreadTeam::new(1).unwrap();
    for (repetitions, units_count) in [(5001, 10002), (50001, 10000)] {
        let sample = support::prepared(&[2, 2], false, repetitions, 2);
        let model = model(&sample);
        let columns = CompiledColumns::new(&model, Estimator::Ppml).unwrap();
        let raw = columns.generate(&sample).unwrap();
        let fit = irls::fit_final_sample(
            &irls::Problem {
                sample: &sample,
                raw: &raw,
                roles: &columns.roles,
            },
            Options::for_estimator(Estimator::Ppml),
            &team,
        )
        .unwrap();
        let units: Vec<_> = (0..sample.n_active())
            .map(|i| (i % units_count) as f64)
            .collect();
        let exact = bias::Request {
            method: bias::Method::Exact,
            save_effects: false,
        };
        let error = bias::compute(
            &fit,
            &model,
            Some(exact),
            Some(NumericColumnView::new(&units)),
            &team,
            &mut || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(error.message.contains("reference work bound"));
        let randomized = bias::Request {
            method: bias::Method::Randomized {
                probes: 2,
                seed: 0,
                stream: 0,
            },
            ..exact
        };
        let result = bias::compute(
            &fit,
            &model,
            Some(randomized),
            Some(NumericColumnView::new(&units)),
            &team,
            &mut || Ok(()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(result.units, units_count);
        assert_eq!(result.diagnostics.solves, 6);
        assert!(result.saved.columns.is_empty());
        for j in 0..result.coefficients.dimension() {
            for channel in bias::Channel::ALL {
                assert_eq!(result.coefficients.value(j, channel).unwrap(), 0.);
            }
        }
    }
}

#[test]
fn diagnostic_wire_preserves_requests_and_reports_full_system_scales() {
    let team = ThreadTeam::new(1).unwrap();
    let (sample, model, units) = fixture(Some(WeightKind::Probability));
    let columns = CompiledColumns::new(&model, Estimator::Ppml).unwrap();
    let raw = columns.generate(&sample).unwrap();
    let fit = irls::fit_final_sample(
        &irls::Problem {
            sample: &sample,
            raw: &raw,
            roles: &columns.roles,
        },
        Options::for_estimator(Estimator::Ppml),
        &team,
    )
    .unwrap();
    for method in [
        bias::Method::Exact,
        bias::Method::Randomized {
            probes: 3,
            seed: u32::MAX,
            stream: u32::MAX,
        },
    ] {
        let result = bias::compute(
            &fit,
            &model,
            Some(bias::Request {
                method,
                save_effects: true,
            }),
            Some(NumericColumnView::new(&units)),
            &team,
            &mut || Ok(()),
        )
        .unwrap()
        .unwrap();
        // An injected call-plan allowance is serialized, never chosen here.
        let bytes = 1_234_567_890;
        let values = result.diagnostic_table(bytes).unwrap().values;
        assert!(values.iter().all(|value| value.is_finite()));
        assert_eq!(
            &values[..7],
            if method == bias::Method::Exact {
                &[1., 7., 0., 0., 0., 1., 9.]
            } else {
                &[2., 7., 3., 4_294_967_295., 4_294_967_295., 3., 9.]
            }
        );
        assert!(values[7] <= fit.options().tolerance);
        assert!(values[8] <= fit.options().tolerance);
        assert!(values[9] >= 0.);
        assert_eq!(values[10], bytes as f64);
        let finite = result.ordinary[(bias::Channel::Total as usize, 0)].abs();
        let maximum = result
            .saved
            .columns
            .iter()
            .filter(|column| {
                matches!(
                    column.kind,
                    Kind::Bias {
                        channel: bias::Channel::Total,
                        ..
                    }
                )
            })
            .flat_map(|column| &column.values)
            .filter(|value| value.is_finite())
            .map(|value| value.abs())
            .fold(finite, f64::max);
        assert_eq!(values[11], maximum);
        assert!(maximum > finite, "the maximum must include FE coordinates");
        assert_eq!(
            values[12],
            fit.evaluation
                .mean
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min)
        );
        assert_eq!(
            values[13],
            fit.evaluation.mean.iter().copied().fold(0., f64::max)
        );
        assert_eq!(
            &values[14..19],
            &[17., 18., 1., 1., fit.options().tolerance]
        );
        let mut minimum = f64::INFINITY;
        let mut leverage = 0_f64;
        for effect in 0..2 {
            let partition = sample
                .topology
                .as_ref()
                .unwrap()
                .effect_partition(effect)
                .unwrap();
            for level in 0..partition.n_levels() {
                let rows = partition.membership().members(level).unwrap();
                let information: f64 = rows.iter().map(|&row| fit.hessian_weights[row]).sum();
                minimum = minimum.min(information);
                for &row in rows {
                    leverage = leverage.max(fit.hessian_weights[row] / information);
                }
            }
        }
        assert!((values[19] - minimum).abs() <= 1e-13 * minimum);
        assert!((values[20] - leverage).abs() <= 1e-13);
    }
}
