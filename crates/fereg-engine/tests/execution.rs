use fereg_engine::{execution, threads::ThreadTeam, ErrorCode, FeregError};
use rayon::prelude::*;
use std::{
    cell::RefCell,
    panic::{catch_unwind, AssertUnwindSafe},
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

#[test]
fn concurrent_requests_are_delivered_once_on_the_caller() {
    let caller = std::thread::current().id();
    for threads in [1, 2, 8] {
        let team = ThreadTeam::new(threads).unwrap();
        let seen = Rc::new(RefCell::new(vec![0; 64]));
        let result = execution::run(
            &team,
            |poll| {
                assert_ne!(std::thread::current().id(), caller);
                assert_eq!(rayon::current_num_threads(), threads);
                let values: Vec<_> = (0..32)
                    .into_par_iter()
                    .map(|rhs| {
                        assert!(poll(2 * rhs));
                        assert!(poll(2 * rhs + 1));
                        rhs * rhs
                    })
                    .collect();
                Ok(values)
            },
            &mut |point| {
                assert_eq!(std::thread::current().id(), caller);
                seen.borrow_mut()[point] += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result, (0..32).map(|rhs| rhs * rhs).collect::<Vec<_>>());
        assert_eq!(*seen.borrow(), vec![1; 64]);
    }
}

struct Owner<'a>(&'a AtomicUsize);
impl Drop for Owner<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn cancellation_preserves_original_error_and_joins_all_concurrent_owners() {
    for threads in [1, 8] {
        let team = ThreadTeam::new(threads).unwrap();
        for code in [
            ErrorCode::Interrupted,
            ErrorCode::Allocation,
            ErrorCode::NonConvergence,
        ] {
            let dropped = AtomicUsize::new(0);
            let sentinel = FeregError::new(code, "host sentinel");
            let mut callbacks = 0;
            let result = execution::run(
                &team,
                |poll| {
                    (0..32).into_par_iter().for_each(|rhs| {
                        let _owner = Owner(&dropped);
                        for step in 0..8 {
                            if !poll((rhs, step)) {
                                break;
                            }
                        }
                    });
                    // Even ignored worker cancellation cannot publish a result.
                    Ok(123)
                },
                &mut |_| {
                    callbacks += 1;
                    if callbacks == 3 {
                        Err(sentinel.clone())
                    } else {
                        Ok(())
                    }
                },
            );
            assert_eq!(result, Err(sentinel));
            assert_eq!(callbacks, 3);
            assert_eq!(dropped.load(Ordering::SeqCst), 32);
            assert_eq!(
                execution::run(
                    &team,
                    |poll| {
                        assert!(poll(()));
                        Ok(11)
                    },
                    &mut |_| Ok(())
                )
                .unwrap(),
                11
            );
        }
    }
}

#[test]
fn either_peer_panic_closes_the_exchange_and_permits_fresh_work() {
    for threads in [1, 8] {
        let team = ThreadTeam::new(threads).unwrap();
        for worker_panics in [false, true] {
            let dropped = AtomicUsize::new(0);
            let mut callbacks = 0;
            let panicked = catch_unwind(AssertUnwindSafe(|| {
                execution::run(
                    &team,
                    |poll| {
                        (0..32).into_par_iter().for_each(|rhs| {
                            let _owner = Owner(&dropped);
                            for step in 0..8 {
                                if worker_panics && rhs == 17 && step == 2 {
                                    panic!("worker sentinel");
                                }
                                if !poll((rhs, step)) {
                                    break;
                                }
                            }
                        });
                        Ok(())
                    },
                    &mut |_| {
                        callbacks += 1;
                        if !worker_panics && callbacks == 3 {
                            panic!("caller sentinel");
                        }
                        Ok(())
                    },
                )
            }));
            assert!(panicked.is_err());
            // Rayon may cancel unstarted jobs, but every constructed owner retires.
            assert!(dropped.load(Ordering::SeqCst) > 0);
            assert_eq!(
                execution::run(
                    &team,
                    |poll| {
                        assert!(poll(()));
                        Ok(37)
                    },
                    &mut |_| Ok(())
                )
                .unwrap(),
                37
            );
        }
    }
}

#[test]
fn nested_relay_on_a_team_worker_fails_without_waiting() {
    for threads in [1, 8] {
        let team = ThreadTeam::new(threads).unwrap();
        let error = team
            .install(|| execution::run(&team, |_| Ok(()), &mut |()| panic!("no host callback")))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Internal);
    }
}
