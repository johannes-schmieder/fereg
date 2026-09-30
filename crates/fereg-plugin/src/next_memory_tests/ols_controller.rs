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
