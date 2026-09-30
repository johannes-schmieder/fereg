//! Temporary independent heap observation; production has no allocator coupling.
mod fixture;
use crate::ffi::{allocation_usage, begin_ols_allocation_observation};
use fereg_engine::{
    data::{
        ClusterFrame, CompiledColumns, DenseMatrix, NumericColumnView, ReferenceWeights,
        SourcePlan, Weights,
    },
    dof::{compute_absorbed_dof, DofPolicy, ResidualDof},
    fe::{Design, Engine},
    memory::{Effect, Inputs, Layout, Plan, Request},
    ols, outputs,
    ppml::{bias, inference, irls},
    spec::{Estimator, ModelSpec, Options, Solver},
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    vce, wls,
};
#[derive(Debug, Clone, Copy)]
struct Case {
    estimator: Estimator,
    solver: Solver,
    rows: usize,
    weighted: bool,
    clustered: bool,
    bias: Option<bias::Request>,
}
fn plan(model: &ModelSpec, case: Case) -> Plan {
    let layout = Layout::new(
        model,
        Inputs {
            original_rows: case.rows,
            active_rows: case.rows,
            sources: 4,
            string_sources: 0,
            string_bytes: 0,
            metadata_bytes: 16_384,
            weighted: case.weighted,
            adjustment: case.estimator == Estimator::Ppml,
            parent_bytes: 0,
        },
        case.estimator,
    )
    .unwrap();
    // Generator's known source-level bounds are available without grouping.
    // Host preflight can retain Layout's looser candidate-row bounds instead.
    let effects = [
        Effect {
            levels: 4,
            channels: 1,
        },
        Effect {
            levels: 8,
            channels: 1,
        },
    ];
    Plan::new(Request {
        effects: &effects,
        bias: case.bias,
        observation_outputs: if case.estimator == Estimator::Ppml {
            8
        } else {
            7
        },
        publication_matrix_cells: 512,
        ..layout.request(case.solver)
    })
    .unwrap()
}
fn hash(values: &[f64], state: &mut u64) {
    for value in values {
        *state = state.rotate_left(7) ^ value.to_bits();
    }
}
fn run(model: &ModelSpec, case: Case, team: &ThreadTeam, workspace: u128) -> u64 {
    let loaded = fixture::loaded(case.rows, case.weighted, case.estimator == Estimator::Ppml);
    let columns = CompiledColumns::new(model, case.estimator).unwrap();
    let sample = if case.estimator == Estimator::Ols {
        ols::prepare(model, &loaded).unwrap()
    } else {
        let sample = SourcePlan::new(model)
            .unwrap()
            .prepare(&loaded, &fixture::Positive)
            .unwrap();
        prepare_fixed_effects(model, sample).unwrap()
    };
    let raw = columns.generate(&sample).unwrap();
    let clusters = ClusterFrame::new(model, &sample).unwrap();
    let views = clusters.views().unwrap();
    let q = ReferenceWeights::new(&sample.weights, sample.n_active(), case.estimator).unwrap();
    let options = Options {
        solver: case.solver,
        ..Options::for_estimator(case.estimator)
    };
    let request = outputs::Request {
        residuals: true,
        xb: true,
        xbd: true,
        absorbed: true,
        drop_reason: true,
        saved: vec![(0, 0), (1, 0)],
    };
    let kind = if case.clustered {
        vce::Kind::Cluster
    } else {
        vce::Kind::Robust
    };
    let mut result = 0;
    if case.estimator == Estimator::Ppml {
        let fit = irls::fit_final_sample(
            &irls::Problem {
                sample: &sample,
                raw: &raw,
                roles: &columns.roles,
            },
            options,
            team,
        )
        .unwrap();
        let inference = inference::estimate(&columns, &fit, &views, kind, 1e-8, team).unwrap();
        let output = outputs::ppml(
            &fit,
            &outputs::PpmlRequest {
                common: request,
                mean: true,
                residual_type: None,
            },
            1e-8,
            team,
        )
        .unwrap();
        let units: Option<Vec<_>> = case
            .bias
            .map(|_| (0..case.rows).map(|r| r as f64).collect());
        let corrected = bias::compute(
            &fit,
            model,
            case.bias,
            units.as_deref().map(NumericColumnView::new),
            team,
            &mut || Ok(()),
        )
        .unwrap();
        if let Some(correction) = &corrected {
            hash(correction.ordinary.as_slice(), &mut result);
            for bank in &correction.saved.columns {
                hash(&bank.values, &mut result);
            }
            let diagnostics = correction.diagnostic_table(workspace).unwrap();
            assert!(diagnostics.values.iter().all(|v| v.is_finite()));
            assert_eq!(
                correction.saved.columns.len(),
                if case.bias.unwrap().save_effects {
                    18
                } else {
                    0
                }
            );
        }
        hash(&inference.coefficients, &mut result);
        hash(inference.covariance.values.as_slice(), &mut result);
        for bank in &output.columns {
            hash(&bank.values, &mut result);
        }
        // Native result/shadow matrices coexist with the accepted/corrected state.
        let publication = DenseMatrix::zeros(1, 512).unwrap();
        std::hint::black_box((&fit, &output, &corrected, &inference, &publication));
    } else {
        let design = Design::new(&sample, Weights::Reference(q)).unwrap();
        let mut engine = Engine::new(&design, Estimator::Ols, &options, team).unwrap();
        let fit = wls::solve_absorbed(
            &mut engine,
            &raw,
            &sample.outcome,
            &columns.roles,
            wls::Policy::default(),
            team,
        )
        .unwrap();
        let reported =
            ols::report(&columns, &raw, &fit.within, &sample.outcome, q, &fit.finite).unwrap();
        let codes: Vec<_> = views.iter().map(|v| v.codes).collect();
        let absorbed = compute_absorbed_dof(&sample, true, DofPolicy::Pairwise, &codes).unwrap();
        let counts: Vec<_> = views.iter().map(|v| v.n_levels).collect();
        let dof = ResidualDof::new(
            Estimator::Ols,
            q.effective(),
            reported.finite_rank,
            absorbed.absorbed_rank,
            absorbed.nested_categories,
            &counts,
        )
        .unwrap();
        let covariance = vce::estimate(
            &vce::Problem {
                design: &reported.score_design,
                bread: &reported.bread,
                residual: &fit.residuals,
                weights: q,
                clusters: &views,
                dof,
            },
            kind,
            1e-8,
            team,
        )
        .unwrap();
        let output = outputs::ols(
            &outputs::OlsFrame {
                design: &design,
                finite_design: &raw,
                coefficients: &fit.finite.coefficients,
                absorbed_coefficients: &fit.absorbed_coefficients,
                residuals: &fit.residuals,
                reported_constant: reported.reported_constant,
            },
            &request,
            1e-8,
            team,
        )
        .unwrap();
        hash(&reported.coefficients, &mut result);
        hash(covariance.values.as_slice(), &mut result);
        for bank in &output.columns {
            hash(&bank.values, &mut result);
        }
        let publication = DenseMatrix::zeros(1, 512).unwrap();
        std::hint::black_box((&engine, &fit, &output, &covariance, &publication));
    }
    result
}
fn observe(model: &ModelSpec, case: Case) -> u64 {
    let mut fingerprint = None;
    let mut maximum_peak = 0;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let admitted = plan(model, case).admit(None).unwrap();
        // Warm only the executor; input and numerical owners are created afresh
        // inside every observation window, including the repeated call.
        team.install(|| {});
        for repeat in 0..2 {
            let (before, _) = begin_ols_allocation_observation();
            let value = run(
                model,
                case,
                &team,
                admitted.estimate().bias_workspace_bytes(),
            );
            let usage = allocation_usage();
            let peak = usage.peak_bytes.saturating_sub(before.current_bytes);
            maximum_peak = maximum_peak.max(peak);
            assert!(
                u128::from(peak) <= admitted.estimate().peak_bytes(),
                "{case:?} workers={workers}: observed {peak}, estimate {}",
                admitted.estimate().peak_bytes()
            );
            // First use can allocate executor-owned TLS/epoch bookkeeping.
            // Repeating a fit on that team must retain no additional bytes.
            if repeat > 0 {
                assert!(
                    usage.current_bytes <= before.current_bytes,
                    "{case:?} workers={workers}: repeat retained {} bytes",
                    usage.current_bytes.saturating_sub(before.current_bytes)
                );
            }
            if let Some(first) = fingerprint {
                assert_eq!(first, value);
            }
            fingerprint = Some(value);
        }
        drop(team);
        // Joining is not a process-heap equality oracle: Rayon/Crossbeam defer
        // reclamation of scheduler buffers beyond a pool's lifetime. The audit
        // above isolates repeated numerical owners from that runtime behavior.
    }
    maximum_peak
}
#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn whole_call_estimates_cover_observed_ols_ppml_and_bias_owners() {
    let _guard = crate::instrumentation::lock_action_observation();
    let mut maximum_peak = 0;
    for estimator in [Estimator::Ols, Estimator::Ppml] {
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            for weighted in [false, true] {
                for clustered in [false, true] {
                    let model = fixture::model(clustered);
                    let base = Case {
                        estimator,
                        solver,
                        rows: 160,
                        weighted,
                        clustered,
                        bias: None,
                    };
                    let modes: Vec<_> = if estimator == Estimator::Ols {
                        vec![None]
                    } else {
                        vec![
                            None,
                            Some(bias::Request {
                                method: bias::Method::Exact,
                                save_effects: true,
                            }),
                            Some(bias::Request {
                                method: bias::Method::Randomized {
                                    probes: 8,
                                    seed: 739391,
                                    stream: 0,
                                },
                                save_effects: false,
                            }),
                            Some(bias::Request {
                                method: bias::Method::Randomized {
                                    probes: 64,
                                    seed: 739391,
                                    stream: 0,
                                },
                                save_effects: true,
                            }),
                        ]
                    };
                    for bias in modes {
                        let case = Case { bias, ..base };
                        maximum_peak = maximum_peak.max(observe(&model, case));
                    }
                }
            }
        }
    }
    eprintln!("240 fresh numerical calls: maximum observed native heap growth {maximum_peak} bytes; estimates cover every window; repeated-call growth is zero");
}

#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn multiple_reduction_blocks_cover_medium_inference_and_outputs() {
    let _guard = crate::instrumentation::lock_action_observation();
    let mut maximum_peak = 0;
    for estimator in [Estimator::Ols, Estimator::Ppml] {
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            for clustered in [false, true] {
                let model = fixture::model(clustered);
                let case = Case {
                    estimator,
                    solver,
                    rows: 4096,
                    weighted: true,
                    clustered,
                    bias: None,
                };
                maximum_peak = maximum_peak.max(observe(&model, case));
            }
        }
    }
    eprintln!("48 medium numerical calls: maximum observed native heap growth {maximum_peak} bytes; fixed-block replay and publication overlap covered");
}

#[test]
#[ignore = "process-global allocator failure; run with --test-threads=1"]
fn allocation_failure_returns_909_and_releases_the_failed_session() {
    use crate::ffi;
    use fereg_core::ppml::{PpmlPhase, PpmlPhaseAllocatorEvent};
    let _guard = crate::instrumentation::lock_action_observation();
    let model = fixture::model(false);
    let loaded = fixture::loaded(160, false, false);
    let sample = ols::prepare(&model, &loaded).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let options = Options::for_estimator(Estimator::Ols);
    let columns = CompiledColumns::new(&model, Estimator::Ols).unwrap();
    let raw = columns.generate(&sample).unwrap();
    // Finish executor/TLS initialization before arming the process-global
    // injector; only the main-thread session reservation is under test.
    {
        let mut warm = Engine::new(&design, Estimator::Ols, &options, &team).unwrap();
        wls::solve_absorbed(
            &mut warm,
            &raw,
            &sample.outcome,
            &columns.roles,
            wls::Policy::default(),
            &team,
        )
        .unwrap();
    }
    let (before, _) = begin_ols_allocation_observation();
    ffi::inject_ppml_allocation_failure(PpmlPhase::FeProjection);
    ffi::ppml_phase_allocation_probe(PpmlPhase::FeProjection, PpmlPhaseAllocatorEvent::Begin);
    let failed = Engine::new(&design, Estimator::Ols, &options, &team);
    ffi::ppml_phase_allocation_probe(PpmlPhase::FeProjection, PpmlPhaseAllocatorEvent::End);
    assert!(ffi::ppml_allocation_failure_fired());
    assert!(!ffi::ppml_allocation_failure_armed());
    let error = match failed {
        Ok(_) => panic!("allocation failure must refuse the session"),
        Err(error) => error,
    };
    assert_eq!(error.return_code(), 909);
    drop(error);
    assert!(allocation_usage().current_bytes <= before.current_bytes);
    // The same immutable inputs remain usable after the failed reservation.
    let mut engine = Engine::new(&design, Estimator::Ols, &options, &team).unwrap();
    wls::solve_absorbed(
        &mut engine,
        &raw,
        &sample.outcome,
        &columns.roles,
        wls::Policy::default(),
        &team,
    )
    .unwrap();
}
