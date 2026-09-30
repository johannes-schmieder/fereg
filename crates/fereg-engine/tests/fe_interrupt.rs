use fereg_engine::{
    data::DenseMatrix,
    fe::{Design, Engine, MonitorControl, PlanOverride, Weights},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    ErrorCode, FeregError,
};
use std::{
    cell::Cell,
    panic::{catch_unwind, AssertUnwindSafe},
    rc::Rc,
};
mod support;

fn options(solver: Solver) -> Options {
    Options {
        solver,
        iterations: 1000,
        tolerance: 1e-10,
        ..Options::for_estimator(Estimator::Ols)
    }
}
fn bank(n: usize) -> DenseMatrix {
    DenseMatrix::from_columns(&[
        (0..n).map(|i| (i as f64 * 0.13).sin()).collect(),
        vec![0.; n],
        (0..n).map(|i| (i as f64 * 0.23).cos()).collect(),
    ])
    .unwrap()
}

#[test]
fn polling_preserves_exact_projections_and_all_executed_boundaries_abort() {
    let caller = std::thread::current().id();
    let sample = support::prepared(&[5, 3], false, 3, 0);
    let weights: Vec<_> = (0..sample.n_active())
        .map(|i| 0.7 + (i % 13) as f64 / 7.)
        .collect();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let rhs = bank(sample.n_active());
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        let mut reference = None;
        for workers in [1, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let expected = Engine::new(&design, Estimator::Ols, &options(solver), &team)
                .unwrap()
                .project(&rhs, &team)
                .unwrap();
            let count = Rc::new(Cell::new(0));
            let polled = Engine::new(&design, Estimator::Ols, &options(solver), &team)
                .unwrap()
                .project_with(&rhs, &team, &mut || {
                    assert_eq!(std::thread::current().id(), caller);
                    count.set(count.get() + 1);
                    Ok(())
                })
                .unwrap();
            assert_eq!(expected, polled);
            if let Some(reference) = &reference {
                assert_eq!(reference, &polled);
            } else {
                reference = Some(polled);
            }
            assert!(count.get() > 3);
            if solver != Solver::Auto {
                assert!(
                    count.get()
                        >= expected
                            .diagnostics
                            .iter()
                            .map(|r| r.iterations)
                            .sum::<usize>()
                );
            }
            for stop in 1..=count.get() {
                let mut calls = 0_usize;
                let sentinel =
                    FeregError::new(ErrorCode::NonConvergence, "host poll r430 sentinel");
                let error = Engine::new(&design, Estimator::Ols, &options(solver), &team)
                    .unwrap()
                    .project_with(&rhs, &team, &mut || {
                        calls += 1;
                        if calls == stop {
                            Err(sentinel.clone())
                        } else {
                            Ok(())
                        }
                    })
                    .unwrap_err();
                assert_eq!(error, sentinel);
                assert_eq!(calls, stop);
            }
            let retried = Engine::new(&design, Estimator::Ols, &options(solver), &team)
                .unwrap()
                .project(&rhs, &team)
                .unwrap();
            assert_eq!(expected, retried);
        }
    }
}

#[test]
fn setup_refinement_and_monitored_fallback_use_the_same_caller_relay() {
    let sample = support::prepared(&[7, 5, 3], false, 3, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = bank(sample.n_active());
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        for workers in [1, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let make = || {
                Engine::with_controls(
                    &design,
                    Estimator::Ols,
                    &options(solver),
                    PlanOverride::Automatic,
                    MonitorControl::StopAfter(1),
                    &team,
                )
                .unwrap()
            };
            let expected = make().project(&rhs, &team).unwrap();
            let mut calls = 0_usize;
            let polled = make()
                .project_with(&rhs, &team, &mut || {
                    calls += 1;
                    Ok(())
                })
                .unwrap();
            assert_eq!(expected, polled);
            assert!(calls > expected.diagnostics.iter().map(|r| r.attempts.len()).sum());
            if solver == Solver::Auto {
                assert!(polled.diagnostics.iter().any(|r| r.fallback.is_some()));
            }
            if solver == Solver::Cmg {
                assert!(!polled.setup_diagnostics[0].right_hand_sides.is_empty());
            }
            for stop in [1, 3, calls / 2, calls] {
                let mut called = 0;
                let sentinel = FeregError::new(ErrorCode::Interrupted, "Break");
                assert_eq!(
                    make()
                        .project_with(&rhs, &team, &mut || {
                            called += 1;
                            if called == stop {
                                Err(sentinel.clone())
                            } else {
                                Ok(())
                            }
                        })
                        .unwrap_err(),
                    sentinel
                );
            }
            assert!(catch_unwind(AssertUnwindSafe(|| make().project_with(
                &rhs,
                &team,
                &mut || panic!("host panic")
            )))
            .is_err());
            assert_eq!(make().project(&rhs, &team).unwrap(), expected);
        }
    }
}

#[test]
fn direct_and_empty_effect_frames_also_poll_on_the_caller() {
    for levels in [vec![], vec![5]] {
        let sample = support::prepared(&levels, false, 30, 0);
        let design = Design::new(&sample, Weights::Unit).unwrap();
        let rhs = bank(sample.n_active());
        let team = ThreadTeam::new(8).unwrap();
        let mut calls = 0_usize;
        let expected = Engine::new(&design, Estimator::Ols, &options(Solver::Auto), &team)
            .unwrap()
            .project(&rhs, &team)
            .unwrap();
        let actual = Engine::new(&design, Estimator::Ols, &options(Solver::Auto), &team)
            .unwrap()
            .project_with(&rhs, &team, &mut || {
                calls += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(actual, expected);
        assert!(calls > 0);
    }
}
