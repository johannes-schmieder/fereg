//! Include complete separation/final-fit/correction owners in the sole plan.
use super::{fixture, hash};
use crate::ffi::{self, allocation_usage, begin_ols_allocation_observation};
use fereg_engine::{
    data::{LoadedSource, NumericColumn},
    memory::Inputs,
    outputs,
    ppml::{self, bias, separation, Call, InterruptPoint, Phase, Resources},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
};
fn request(saved: bool, method: Option<bias::Method>) -> ppml::Request {
    ppml::Request {
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
        ..Default::default()
    }
}
fn units(rows: usize) -> NumericColumn {
    NumericColumn {
        values: (0..rows).map(|row| (row % 7) as f64 - 3.5).collect(),
        missing: None,
    }
}
#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn complete_ppml_controller_releases_numerical_owners_and_honors_one_plan() {
    let _guard = crate::instrumentation::lock_action_observation();
    let mut maximum = 0;
    let mut calls = 0;
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        for weighted in [false, true] {
            for clustered in [false, true] {
                let model = fixture::model(clustered);
                let mut scientific = None;
                for method in [
                    None,
                    Some(bias::Method::Exact),
                    Some(bias::Method::Randomized {
                        probes: 17,
                        seed: 739391,
                        stream: 3,
                    }),
                ] {
                    let mut correction = None;
                    for workers in [1, 8] {
                        let team = ThreadTeam::new(workers).unwrap();
                        let options = Options {
                            solver,
                            threads: workers,
                            ..Options::for_estimator(Estimator::Ppml)
                        };
                        for saved in [false, true] {
                            let request = request(saved, method);
                            for repeat in 0..2 {
                                let (before, _) = begin_ols_allocation_observation();
                                let loaded = fixture::loaded(1024, weighted, true);
                                let units = method.map(|_| units(1024));
                                let mut inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
                                if let Some(column) = &units {
                                    inputs = inputs.with_numeric_auxiliary(column).unwrap();
                                }
                                let call = Call::new(
                                    &model,
                                    &options,
                                    &request,
                                    inputs,
                                    loaded.weights.as_ref().map(|(kind, _)| *kind),
                                    Resources {
                                        publication_matrix_cells: 512,
                                        ..Default::default()
                                    },
                                )
                                .unwrap();
                                let estimate = call.memory().peak_bytes();
                                let fit = call
                                    .run_with_team(&loaded, units.as_ref(), &team, &mut |_| Ok(()))
                                    .unwrap();
                                let mut value = 0;
                                hash(&fit.coefficients, &mut value);
                                hash(fit.covariance.values.as_slice(), &mut value);
                                if let Some(first) = scientific {
                                    assert_eq!(first, value);
                                } else {
                                    scientific = Some(value);
                                }
                                assert_eq!(fit.outputs.columns.len(), if saved { 8 } else { 0 });
                                if let Some(bias) = &fit.bias {
                                    let mut value = 0;
                                    hash(bias.ordinary.as_slice(), &mut value);
                                    if let Some(first) = correction {
                                        assert_eq!(first, value);
                                    } else {
                                        correction = Some(value);
                                    }
                                    assert_eq!(
                                        bias.saved.columns.len(),
                                        if saved { 18 } else { 0 }
                                    );
                                    assert_eq!(
                                        bias.table.values[10],
                                        fit.memory.bias_workspace_bytes() as f64
                                    );
                                }
                                let shadow = vec![0_f64; 512];
                                std::hint::black_box(&shadow);
                                let peak = allocation_usage()
                                    .peak_bytes
                                    .saturating_sub(before.current_bytes);
                                maximum = maximum.max(peak);
                                assert!(
                                    peak as u128 <= estimate,
                                    "{solver:?}/{method:?}, workers={workers}: {peak} > {estimate}"
                                );
                                drop(shadow);
                                drop(fit);
                                drop(units);
                                drop(loaded);
                                if repeat > 0 {
                                    assert!(
                                        allocation_usage().current_bytes <= before.current_bytes,
                                        "repeated PPML controller retained numerical storage"
                                    );
                                }
                                calls += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(calls, 288);
    eprintln!("{calls} complete PPML controller calls: maximum tracked native growth {maximum} bytes; exact scientific/bias hashes, requested-only products and one plan");
}
#[test]
#[ignore = "process-global allocator failure; run with --test-threads=1"]
fn ppml_controller_allocation_failure_discards_all_phases_and_can_restart() {
    use fereg_core::ppml::{PpmlPhase, PpmlPhaseAllocatorEvent};
    let _guard = crate::instrumentation::lock_action_observation();
    let model = fixture::model(false);
    let loaded = fixture::loaded(160, false, true);
    let units = units(160);
    let request = request(true, Some(bias::Method::Exact));
    let options = Options {
        threads: 1,
        ..Options::for_estimator(Estimator::Ppml)
    };
    let team = ThreadTeam::new(1).unwrap();
    let make = || {
        Call::new(
            &model,
            &options,
            &request,
            Inputs::from_loaded(&loaded, 0, 0)
                .unwrap()
                .with_numeric_auxiliary(&units)
                .unwrap(),
            None,
            Resources::default(),
        )
        .unwrap()
    };
    // Worker/TLS setup is complete before the global injector is armed.
    drop(
        make()
            .run_with_team(&loaded, Some(&units), &team, &mut |_| Ok(()))
            .unwrap(),
    );
    for boundary in [
        Phase::Loaded,
        Phase::Separated,
        Phase::Estimated,
        Phase::Outputs,
        Phase::Corrected,
    ] {
        let call = make();
        let (before, _) = begin_ols_allocation_observation();
        let failed = call.run_with_team(&loaded, Some(&units), &team, &mut |phase| {
            if phase == boundary {
                ffi::inject_ppml_allocation_failure(PpmlPhase::FeProjection);
                ffi::ppml_phase_allocation_probe(
                    PpmlPhase::FeProjection,
                    PpmlPhaseAllocatorEvent::Begin,
                );
            }
            Ok(())
        });
        ffi::ppml_phase_allocation_probe(PpmlPhase::FeProjection, PpmlPhaseAllocatorEvent::End);
        assert!(ffi::ppml_allocation_failure_fired());
        assert!(!ffi::ppml_allocation_failure_armed());
        assert_eq!(failed.unwrap_err().return_code(), 909);
        assert!(allocation_usage().current_bytes <= before.current_bytes);
        drop(
            make()
                .run_with_team(&loaded, Some(&units), &team, &mut |_| Ok(()))
                .unwrap(),
        );
    }
}
#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn ppml_numerical_break_releases_owners_at_every_checkpoint_kind() {
    let _guard = crate::instrumentation::lock_action_observation();
    let model = fixture::model(false);
    let mut loaded = fixture::loaded(1024, true, true);
    let LoadedSource::Numeric(x) = &mut loaded.sources[2] else {
        unreachable!()
    };
    x.values.fill(0.);
    x.values[3] = 1.;
    x.values[67] = -1.;
    loaded.outcome.values[3] = 0.;
    loaded.outcome.values[67] = 0.;
    let units = units(1024);
    let mut request = request(true, Some(bias::Method::Exact));
    request.separation = separation::Methods::parse("fe simplex ir mu").unwrap();
    for workers in [1, 8] {
        let options = Options {
            threads: workers,
            ..Options::for_estimator(Estimator::Ppml)
        };
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
                loaded.weights.as_ref().map(|(kind, _)| *kind),
                Resources::default(),
            )
            .unwrap()
        };
        let baseline = make()
            .run_with_team(&loaded, Some(&units), &team, &mut |_| Ok(()))
            .unwrap();
        let mut expected = 0;
        hash(&baseline.coefficients, &mut expected);
        hash(baseline.covariance.values.as_slice(), &mut expected);
        drop(baseline);
        for stop in [
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
        ] {
            let (before, _) = begin_ols_allocation_observation();
            let mut reached = false;
            let error = make()
                .run_with_team(&loaded, Some(&units), &team, &mut |phase| {
                    if phase == Phase::Numerical(stop) {
                        reached = true;
                        Err(fereg_engine::FeregError::new(
                            fereg_engine::ErrorCode::Interrupted,
                            "tracked Break",
                        ))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
            assert!(reached, "unreached {stop:?}");
            assert_eq!(error.return_code(), 1);
            drop(error);
            assert!(
                allocation_usage().current_bytes <= before.current_bytes,
                "{stop:?} retained numerical storage"
            );
            let fit = make()
                .run_with_team(&loaded, Some(&units), &team, &mut |_| Ok(()))
                .unwrap();
            let mut value = 0;
            hash(&fit.coefficients, &mut value);
            hash(fit.covariance.values.as_slice(), &mut value);
            assert_eq!(value, expected);
            drop(fit);
            assert!(allocation_usage().current_bytes <= before.current_bytes);
        }
    }
}

#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn ppml_fe_cancellation_releases_separation_final_irls_and_bias_owners() {
    let _guard = crate::instrumentation::lock_action_observation();
    let model = fixture::model(false);
    let mut loaded = fixture::loaded(1024, true, true);
    let LoadedSource::Numeric(x) = &mut loaded.sources[2] else {
        unreachable!()
    };
    x.values.fill(0.);
    x.values[3] = 1.;
    x.values[67] = -1.;
    loaded.outcome.values[3] = 0.;
    loaded.outcome.values[67] = 0.;
    let units = units(1024);
    let inputs = Inputs::from_loaded(&loaded, 0, 0)
        .unwrap()
        .with_numeric_auxiliary(&units)
        .unwrap();
    let sentinel = fereg_engine::FeregError::new(
        fereg_engine::ErrorCode::NonConvergence,
        "host FE cancellation must not enter numerical recovery",
    );
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
            for workers in [1, 8] {
                let options = Options {
                    solver,
                    threads: workers,
                    ..Options::for_estimator(Estimator::Ppml)
                };
                let team = ThreadTeam::new(workers).unwrap();
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
                let baseline = make()
                    .run_with_team(&loaded, Some(&units), &team, &mut |_| Ok(()))
                    .unwrap();
                let mut expected = 0;
                hash(&baseline.coefficients, &mut expected);
                hash(baseline.covariance.values.as_slice(), &mut expected);
                hash(
                    baseline.bias.as_ref().unwrap().ordinary.as_slice(),
                    &mut expected,
                );
                drop(baseline);
                for cancelled_region in 0..3 {
                    let (before, _) = begin_ols_allocation_observation();
                    let mut region = 0;
                    let mut polls = 0;
                    let error = make()
                        .run_with_team(&loaded, Some(&units), &team, &mut |phase| {
                            match phase {
                                Phase::Separated => region = 1,
                                Phase::Outputs => region = 2,
                                Phase::Numerical(InterruptPoint::FixedEffects)
                                    if region == cancelled_region =>
                                {
                                    polls += 1;
                                    if polls == 3 {
                                        return Err(sentinel.clone());
                                    }
                                }
                                _ => (),
                            }
                            Ok(())
                        })
                        .unwrap_err();
                    assert_eq!(polls, 3);
                    assert_eq!(error, sentinel);
                    drop(error);
                    assert!(allocation_usage().current_bytes <= before.current_bytes);
                    let retry = make()
                        .run_with_team(&loaded, Some(&units), &team, &mut |_| Ok(()))
                        .unwrap();
                    let mut value = 0;
                    hash(&retry.coefficients, &mut value);
                    hash(retry.covariance.values.as_slice(), &mut value);
                    hash(retry.bias.as_ref().unwrap().ordinary.as_slice(), &mut value);
                    assert_eq!(value, expected);
                    drop(retry);
                    assert!(allocation_usage().current_bytes <= before.current_bytes);
                }
            }
        }
    }
}
