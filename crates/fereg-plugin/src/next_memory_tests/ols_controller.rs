//! Exercise the sole controller, including native shadows and failed owners.
use super::{fixture, hash};
use crate::ffi::{self, allocation_usage, begin_ols_allocation_observation};
use fereg_engine::{
    memory::Inputs,
    ols::{self, Call, Phase, Resources},
    outputs,
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
};
fn request(saved: bool) -> ols::Request {
    ols::Request {
        outputs: if saved {
            outputs::Request {
                residuals: true,
                xb: true,
                xbd: true,
                absorbed: true,
                drop_reason: true,
                saved: vec![(0, 0), (1, 0)],
            }
        } else {
            outputs::Request::default()
        },
        ..ols::Request::default()
    }
}
#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn complete_controller_owns_only_requested_products_and_stays_within_one_plan() {
    let _guard = crate::instrumentation::lock_action_observation();
    let mut maximum = 0;
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        for weighted in [false, true] {
            for clustered in [false, true] {
                let model = fixture::model(clustered);
                let mut scientific = None;
                for workers in [1, 8] {
                    let team = ThreadTeam::new(workers).unwrap();
                    let options = Options {
                        solver,
                        threads: workers,
                        ..Options::for_estimator(Estimator::Ols)
                    };
                    for saved in [false, true] {
                        let request = request(saved);
                        for repeat in 0..2 {
                            let (before, _) = begin_ols_allocation_observation();
                            let loaded = fixture::loaded(4096, weighted, false);
                            let call = Call::new(
                                &model,
                                &options,
                                &request,
                                Inputs::from_loaded(&loaded, 0, 0).unwrap(),
                                loaded.weights.as_ref().map(|(kind, _)| *kind),
                                Resources {
                                    publication_matrix_cells: 512,
                                    ..Resources::default()
                                },
                            )
                            .unwrap();
                            let estimate = call.memory().peak_bytes();
                            let fit = call.run_with_team(&loaded, &team, &mut |_| Ok(())).unwrap();
                            let mut value = 0;
                            hash(&fit.coefficients, &mut value);
                            hash(fit.covariance.values.as_slice(), &mut value);
                            if let Some(first) = scientific {
                                assert_eq!(first, value);
                            } else {
                                scientific = Some(value);
                            }
                            assert_eq!(fit.outputs.columns.len(), if saved { 7 } else { 0 });
                            let shadow = vec![0_f64; 512];
                            std::hint::black_box(&shadow);
                            let peak = allocation_usage()
                                .peak_bytes
                                .saturating_sub(before.current_bytes);
                            maximum = maximum.max(peak);
                            assert!(
                                peak as u128 <= estimate,
                                "{solver:?}, workers={workers}: {peak} > {estimate}"
                            );
                            drop(shadow);
                            drop(fit);
                            drop(loaded);
                            if repeat > 0 {
                                assert!(
                                    allocation_usage().current_bytes <= before.current_bytes,
                                    "repeated controller retained numerical storage"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("96 complete OLS controller calls: maximum tracked native growth {maximum} bytes; one plan, requested-only outputs and exact scientific hashes across workers");
}
#[test]
#[ignore = "process-global allocator failure; run with --test-threads=1"]
fn controller_allocation_failure_discards_preparation_or_inference_and_can_restart() {
    use fereg_core::ppml::{PpmlPhase, PpmlPhaseAllocatorEvent};
    let _guard = crate::instrumentation::lock_action_observation();
    let model = fixture::model(false);
    let loaded = fixture::loaded(160, false, false);
    let request = request(true);
    let options = Options {
        threads: 1,
        ..Options::for_estimator(Estimator::Ols)
    };
    let team = ThreadTeam::new(1).unwrap();
    let make = || {
        Call::new(
            &model,
            &options,
            &request,
            Inputs::from_loaded(&loaded, 0, 0).unwrap(),
            None,
            Resources::default(),
        )
        .unwrap()
    };
    // Complete all executor/TLS initialization before the global injector.
    drop(
        make()
            .run_with_team(&loaded, &team, &mut |_| Ok(()))
            .unwrap(),
    );
    for boundary in [Phase::Loaded, Phase::Estimated] {
        let call = make();
        let (before, _) = begin_ols_allocation_observation();
        let failed = call.run_with_team(&loaded, &team, &mut |phase| {
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
                .run_with_team(&loaded, &team, &mut |_| Ok(()))
                .unwrap(),
        );
    }
}

#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn ols_fe_and_dense_cancellation_and_host_panic_release_complete_call_owners() {
    let _guard = crate::instrumentation::lock_action_observation();
    let model = fixture::model(false);
    let loaded = fixture::loaded(1024, true, false);
    let request = request(true);
    let inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
    let sentinel = fereg_engine::FeregError::new(
        fereg_engine::ErrorCode::NonConvergence,
        "host FE cancellation must not enter numerical recovery",
    );
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        for workers in [1, 8] {
            let options = Options {
                solver,
                threads: workers,
                ..Options::for_estimator(Estimator::Ols)
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
                .run_with_team(&loaded, &team, &mut |_| Ok(()))
                .unwrap();
            let mut expected = 0;
            hash(&baseline.coefficients, &mut expected);
            hash(baseline.covariance.values.as_slice(), &mut expected);
            drop(baseline);
            for checkpoint in [Phase::FixedEffects, Phase::DenseLinearAlgebra] {
                for inference in [false, true] {
                    if inference && checkpoint == Phase::FixedEffects {
                        continue;
                    }
                    for panic in [false, true] {
                        // Captured panic text belongs to the test harness and outlives
                        // the call; exclude that owner from this numerical-storage audit.
                        let saved_hook = panic.then(std::panic::take_hook);
                        if panic {
                            std::panic::set_hook(Box::new(|_| {}));
                        }
                        let (before, _) = begin_ols_allocation_observation();
                        let mut polls = 0;
                        let mut inferred_region = false;
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            make().run_with_team(&loaded, &team, &mut |phase| {
                                match phase {
                                    Phase::Estimated => inferred_region = true,
                                    Phase::Inferred => inferred_region = false,
                                    _ => (),
                                }
                                if phase == checkpoint && (!inference || inferred_region) {
                                    polls += 1;
                                    if polls == 3 {
                                        assert!(!panic, "tracked host panic");
                                        return Err(sentinel.clone());
                                    }
                                }
                                Ok(())
                            })
                        }));
                        if let Some(hook) = saved_hook {
                            std::panic::set_hook(hook);
                        }
                        assert_eq!(polls, 3);
                        if panic {
                            assert!(result.is_err());
                        } else {
                            assert_eq!(result.as_ref().unwrap().as_ref().unwrap_err(), &sentinel);
                        }
                        drop(result);
                        assert!(allocation_usage().current_bytes <= before.current_bytes);
                        let retry = make()
                            .run_with_team(&loaded, &team, &mut |_| Ok(()))
                            .unwrap();
                        let mut value = 0;
                        hash(&retry.coefficients, &mut value);
                        hash(retry.covariance.values.as_slice(), &mut value);
                        assert_eq!(value, expected);
                        drop(retry);
                        assert!(allocation_usage().current_bytes <= before.current_bytes);
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn tuple_coding_and_dof_cancellation_release_every_complete_inference_owner() {
    use fereg_engine::{
        data::{LoadedSource, NumericColumn},
        spec::ClusterTerm,
        SourceId,
    };
    let _guard = crate::instrumentation::lock_action_observation();
    let mut model = fixture::model(false);
    model.clusters = vec![ClusterTerm {
        label: "h#unit".into(),
        sources: vec![SourceId::new(2).unwrap(), SourceId::new(4).unwrap()],
    }];
    let mut loaded = fixture::loaded(4099, true, false);
    loaded.sources[3] = LoadedSource::Numeric(NumericColumn {
        // Many row comparisons, few covariance groups: exhaustive covariance
        // kernel cancellation is qualified separately.
        values: (0..4099).map(|i| ((i * 37) % 17) as f64).collect(),
        missing: None,
    });
    let request = request(true);
    let inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
    let caller = std::thread::current().id();
    let sentinel = fereg_engine::FeregError::new(
        fereg_engine::ErrorCode::NonConvergence,
        "metadata host r(430) must discard every inference owner",
    );
    let mut aborted = 0;
    let mut scientific = None;
    let mut checkpoint_count = None;
    for workers in [1, 8] {
        let options = Options {
            threads: workers,
            ..Options::for_estimator(Estimator::Ols)
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
        let mut region = false;
        let mut total = 0;
        let baseline = make()
            .run_with_team(&loaded, &team, &mut |phase| {
                assert_eq!(std::thread::current().id(), caller);
                match phase {
                    Phase::Estimated => region = true,
                    Phase::Inferred => region = false,
                    Phase::DenseLinearAlgebra if region => total += 1,
                    _ => (),
                }
                Ok(())
            })
            .unwrap();
        let mut expected = 0;
        hash(&baseline.coefficients, &mut expected);
        hash(baseline.covariance.values.as_slice(), &mut expected);
        let estimate = baseline.memory.peak_bytes();
        assert_eq!(*scientific.get_or_insert(expected), expected);
        assert_eq!(*checkpoint_count.get_or_insert(total), total);
        assert!(total > 100);
        drop(baseline);
        for stop in 1..=total {
            let (before, _) = begin_ols_allocation_observation();
            let mut region = false;
            let mut polls = 0;
            let error = make()
                .run_with_team(&loaded, &team, &mut |phase| {
                    assert_eq!(std::thread::current().id(), caller);
                    match phase {
                        Phase::Estimated => region = true,
                        Phase::Inferred => region = false,
                        Phase::DenseLinearAlgebra if region => {
                            polls += 1;
                            if polls == stop {
                                return Err(sentinel.clone());
                            }
                        }
                        _ => (),
                    }
                    Ok(())
                })
                .unwrap_err();
            assert_eq!(error, sentinel);
            assert_eq!(polls, stop);
            drop(error);
            assert!(allocation_usage().current_bytes <= before.current_bytes);
            assert!(
                u128::from(
                    allocation_usage()
                        .peak_bytes
                        .saturating_sub(before.current_bytes)
                ) <= estimate
            );
            let retry = make()
                .run_with_team(&loaded, &team, &mut |_| Ok(()))
                .unwrap();
            let mut value = 0;
            hash(&retry.coefficients, &mut value);
            hash(retry.covariance.values.as_slice(), &mut value);
            assert_eq!(value, expected);
            drop(retry);
            assert!(allocation_usage().current_bytes <= before.current_bytes);
            aborted += 1;
        }
    }
    eprintln!("{aborted} tracked complete inference aborts: tuple sorting, cluster banks, metadata validation, DoF, reporting, covariance and statistics; no retained numerical owners, exact fresh retries and one plan");
}

#[test]
#[ignore = "process-global allocator audit; run with --test-threads=1"]
fn source_preparation_cancellation_releases_encoded_strings_and_finite_columns() {
    use fereg_engine::data::LoadedSource;
    let _guard = crate::instrumentation::lock_action_observation();
    let model = fixture::model(false);
    let mut loaded = fixture::loaded(1029, true, false);
    loaded.sources[1] = LoadedSource::Strings(
        (0..1029)
            .map(|row| {
                if row % 41 == 0 {
                    String::new()
                } else {
                    format!("Ω{:02}", row % 8)
                }
            })
            .collect(),
    );
    let request = request(true);
    let inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
    let caller = std::thread::current().id();
    let sentinel = fereg_engine::FeregError::new(
        fereg_engine::ErrorCode::NonConvergence,
        "preparation host r(430) must discard encoded and compact owners",
    );
    let mut scientific = None;
    let mut checkpoint_count = None;
    let mut aborted = 0;
    for workers in [1, 8] {
        let options = Options {
            threads: workers,
            ..Options::for_estimator(Estimator::Ols)
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
        let mut region = false;
        let mut total = 0;
        let baseline = make()
            .run_with_team(&loaded, &team, &mut |phase| {
                assert_eq!(std::thread::current().id(), caller);
                match phase {
                    Phase::Loaded => region = true,
                    Phase::Prepared => region = false,
                    Phase::DenseLinearAlgebra if region => total += 1,
                    _ => (),
                }
                Ok(())
            })
            .unwrap();
        let mut expected = 0;
        hash(&baseline.coefficients, &mut expected);
        hash(baseline.covariance.values.as_slice(), &mut expected);
        hash(
            &[baseline.statistics.rss, baseline.statistics.tss],
            &mut expected,
        );
        let estimate = baseline.memory.peak_bytes();
        assert_eq!(*scientific.get_or_insert(expected), expected);
        assert_eq!(*checkpoint_count.get_or_insert(total), total);
        assert!(total > 20);
        drop(baseline);
        for stop in 1..=total {
            let (before, _) = begin_ols_allocation_observation();
            let mut region = false;
            let mut polls = 0;
            let error = make()
                .run_with_team(&loaded, &team, &mut |phase| {
                    assert_eq!(std::thread::current().id(), caller);
                    match phase {
                        Phase::Loaded => region = true,
                        Phase::Prepared => region = false,
                        Phase::DenseLinearAlgebra if region => {
                            polls += 1;
                            if polls == stop {
                                return Err(sentinel.clone());
                            }
                        }
                        _ => (),
                    }
                    Ok(())
                })
                .unwrap_err();
            assert_eq!(error, sentinel);
            assert_eq!(polls, stop);
            drop(error);
            assert!(allocation_usage().current_bytes <= before.current_bytes);
            assert!(
                u128::from(
                    allocation_usage()
                        .peak_bytes
                        .saturating_sub(before.current_bytes)
                ) <= estimate
            );
            let retry = make()
                .run_with_team(&loaded, &team, &mut |_| Ok(()))
                .unwrap();
            let mut value = 0;
            hash(&retry.coefficients, &mut value);
            hash(retry.covariance.values.as_slice(), &mut value);
            hash(&[retry.statistics.rss, retry.statistics.tss], &mut value);
            assert_eq!(value, expected);
            drop(retry);
            assert!(allocation_usage().current_bytes <= before.current_bytes);
            aborted += 1;
        }
    }
    eprintln!("{aborted} tracked complete source-preparation aborts: encoded strings, compact source banks and finite columns; no retained owners, original host errors and exact fresh retries within one plan");
}
