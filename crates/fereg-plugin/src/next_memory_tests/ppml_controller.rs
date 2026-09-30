//! Include complete separation/final-fit/correction owners in the sole plan.
use super::{fixture, hash};
use crate::ffi::{self, allocation_usage, begin_ols_allocation_observation};
use fereg_engine::{
    data::NumericColumn,
    memory::Inputs,
    outputs,
    ppml::{self, bias, Call, Phase, Resources},
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
