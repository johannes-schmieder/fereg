//! Keep native PPML work in one installed team while its caller owns SPI polls.
//!
//! Phase requests retain their order. Between requests the caller polls every
//! `cancellable::POLL_INTERVAL`, including with one worker. A host error sets
//! the cooperative signal, releases pending phase requests, and takes precedence
//! over the worker result after all work joins. SPI never reaches a worker.
#![forbid(unsafe_code)]

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::ppml::PpmlInterruptPoint;
use fereg_core::{interrupt, ThreadTeam};
use std::sync::{Condvar, Mutex, MutexGuard};

struct Exchange<T> {
    state: Mutex<State<T>>,
    changed: Condvar,
}

struct State<T> {
    point: Option<PpmlInterruptPoint>,
    reply: Option<Result<()>>,
    output: Option<Result<T>>,
    closed: bool,
}

impl<T> Exchange<T> {
    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn wait<'a>(&self, state: MutexGuard<'a, State<T>>) -> MutexGuard<'a, State<T>> {
        self.changed
            .wait(state)
            .unwrap_or_else(|error| error.into_inner())
    }

    fn poll(&self, point: PpmlInterruptPoint) -> Result<()> {
        let mut state = self.lock();
        if state.closed {
            return Err(stopped());
        }
        state.point = Some(point);
        self.changed.notify_all();
        loop {
            if let Some(reply) = state.reply.take() {
                return reply;
            }
            if state.closed {
                return Err(stopped());
            }
            state = self.wait(state);
        }
    }
}

struct Close<'a, T>(&'a Exchange<T>);

impl<T> Drop for Close<'_, T> {
    fn drop(&mut self) {
        // A caller panic must stop checkpoint-only work before the scope joins.
        if std::thread::panicking() {
            interrupt::request_cancellation();
        }
        self.0.lock().closed = true;
        self.0.changed.notify_all();
    }
}

fn stopped() -> FeregError {
    FeregError::new(ErrorCode::Internal, "PPML scoped execution peer stopped")
}

pub(crate) fn run<T: Send>(
    team: ThreadTeam,
    work: impl FnOnce(&mut dyn FnMut(PpmlInterruptPoint) -> Result<()>) -> Result<T> + Send,
    // None is a periodic host poll, not a new core phase boundary.
    poll_host: &mut dyn FnMut(Option<PpmlInterruptPoint>) -> Result<()>,
) -> Result<T> {
    interrupt::reset_cancellation();
    // This guard outlives the scope: workers observe the signal until joined.
    let _reset = crate::cancellable::ResetCancellation;
    // The exchange is stack-owned, with one request and reply in flight.
    // Both peers close it on unwinding so the scope can always join.
    let exchange = Exchange {
        state: Mutex::new(State {
            point: None,
            reply: None,
            output: None,
            closed: false,
        }),
        changed: Condvar::new(),
    };
    let mut interruption: Option<FeregError> = None;
    let output = team.in_place_scope(|scope| {
        let _caller_close = Close(&exchange);
        let worker_exchange = &exchange;
        scope.spawn(move |_| {
            let _worker_close = Close(worker_exchange);
            let output = work(&mut |point| worker_exchange.poll(point));
            worker_exchange.lock().output = Some(output);
        });
        loop {
            let mut state = exchange.lock();
            if let Some(output) = state.output.take() {
                return output;
            }
            if let Some(point) = state.point.take() {
                drop(state);
                let reply = if let Some(error) = &interruption {
                    Err(error.clone())
                } else {
                    let reply = poll_host(Some(point));
                    if let Err(error) = &reply {
                        interrupt::request_cancellation();
                        interruption = Some(error.clone());
                    }
                    reply
                };
                exchange.lock().reply = Some(reply);
                exchange.changed.notify_all();
                continue;
            }
            if state.closed {
                return Err(stopped());
            }
            let (state, timeout) = exchange
                .changed
                .wait_timeout(state, crate::cancellable::POLL_INTERVAL)
                .unwrap_or_else(|error| error.into_inner());
            if timeout.timed_out()
                && !state.closed
                && state.output.is_none()
                && state.point.is_none()
                && interruption.is_none()
            {
                drop(state);
                if let Err(error) = poll_host(None) {
                    interrupt::request_cancellation();
                    interruption = Some(error);
                }
            }
        }
    })?;
    // The worker may have completed while the host callback was in flight.
    if let Some(error) = interruption {
        return Err(error);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use fereg_core::ThreadConfig;
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    fn isolated(name: &str) -> bool {
        const CHILD: &str = "FEREG_PPML_EXECUTION_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            return false;
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args([name, "--exact", "--test-threads=1"])
            .status()
            .expect("launch isolated PPML cancellation witness");
        assert!(status.success());
        true
    }

    fn await_flag(flag: &AtomicBool) {
        let start = Instant::now();
        while !flag.load(Ordering::Acquire) {
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "peer did not respond"
            );
            std::thread::yield_now();
        }
    }

    struct Finished<'a>(&'a AtomicBool);
    impl Drop for Finished<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    fn team(threads: usize) -> ThreadTeam {
        ThreadTeam::new(ThreadConfig::from_requested(threads, true).unwrap())
    }

    #[test]
    fn caller_keeps_non_send_callback_and_worker_uses_requested_team() {
        let caller = std::thread::current().id();
        for threads in [1, 4, 2, 4] {
            let polls = Rc::new(Cell::new(0));
            let result = run(
                team(threads),
                |poll| {
                    assert_ne!(std::thread::current().id(), caller);
                    assert_eq!(rayon::current_num_threads(), threads);
                    for _ in 0..128 {
                        poll(PpmlInterruptPoint::IrlsIteration)?;
                    }
                    Ok(37)
                },
                &mut |point| {
                    assert_eq!(std::thread::current().id(), caller);
                    if point.is_some() {
                        polls.set(polls.get() + 1);
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(result, 37);
            assert_eq!(polls.get(), 128);
        }
    }

    #[test]
    fn callback_errors_and_both_peer_panics_join_and_allow_retry() {
        if isolated(
            "ppml_execution::tests::callback_errors_and_both_peer_panics_join_and_allow_retry",
        ) {
            return;
        }
        for threads in [1, 4] {
            let error = run(
                team(threads),
                |poll| {
                    poll(PpmlInterruptPoint::Preparation)?;
                    panic!("cancelled callback must stop computation");
                    #[allow(unreachable_code)]
                    Ok(())
                },
                &mut |_| Err(FeregError::new(ErrorCode::Interrupted, "requested break")),
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::Interrupted);
            for worker_panics in [false, true] {
                assert!(catch_unwind(AssertUnwindSafe(|| run(
                    team(threads),
                    |poll| {
                        if worker_panics {
                            panic!("injected native panic");
                        }
                        poll(PpmlInterruptPoint::Preparation)?;
                        Ok(())
                    },
                    &mut |_| panic!("injected caller panic")
                )))
                .is_err());
                assert_eq!(run(team(threads), |_| Ok(11), &mut |_| Ok(())).unwrap(), 11);
            }
        }
    }

    #[test]
    fn idle_break_reaches_checkpoints_joins_and_resets_for_the_next_action() {
        if isolated("ppml_execution::tests::idle_break_reaches_checkpoints_joins_and_resets_for_the_next_action") {
            return;
        }
        let caller = std::thread::current().id();
        for threads in [1, 4, 1, 4] {
            let finished = AtomicBool::new(false);
            let mut polls = 0;
            let error = run(
                team(threads),
                |_| -> Result<()> {
                    let _finished = Finished(&finished);
                    let start = Instant::now();
                    loop {
                        interrupt::checkpoint()?;
                        assert!(start.elapsed() < Duration::from_secs(30));
                        std::thread::yield_now();
                    }
                },
                &mut |point| {
                    assert_eq!(std::thread::current().id(), caller);
                    assert!(point.is_none(), "work requested no phase callback");
                    polls += 1;
                    Err(FeregError::new(ErrorCode::Interrupted, "idle host Break"))
                },
            )
            .unwrap_err();
            assert_eq!(error.message, "idle host Break");
            assert_eq!(error.code, ErrorCode::Interrupted);
            assert_eq!(polls, 1);
            assert!(
                finished.load(Ordering::Acquire),
                "worker must join before return"
            );
            assert!(!interrupt::cancellation_requested());
            assert_eq!(run(team(threads), |_| Ok(37), &mut |_| Ok(())).unwrap(), 37);
        }
    }

    #[test]
    fn host_error_wins_worker_success_and_releases_later_phase_requests() {
        if isolated("ppml_execution::tests::host_error_wins_worker_success_and_releases_later_phase_requests") {
            return;
        }
        for threads in [1, 4] {
            for later_phase in [false, true] {
                let host_code = if later_phase {
                    ErrorCode::Internal
                } else {
                    ErrorCode::Interrupted
                };
                let callback_entered = AtomicBool::new(false);
                let finished = AtomicBool::new(false);
                let mut polls = 0;
                let error = run(
                    team(threads),
                    |poll| {
                        let _finished = Finished(&finished);
                        await_flag(&callback_entered);
                        if later_phase {
                            let start = Instant::now();
                            while !interrupt::cancellation_requested() {
                                assert!(start.elapsed() < Duration::from_secs(30));
                                std::thread::yield_now();
                            }
                            let error = poll(PpmlInterruptPoint::Preparation).unwrap_err();
                            assert_eq!(error.message, "host failure");
                            assert_eq!(error.code, host_code);
                        }
                        Ok(41)
                    },
                    &mut |point| {
                        assert!(point.is_none());
                        polls += 1;
                        callback_entered.store(true, Ordering::Release);
                        if !later_phase {
                            // The worker completes successfully while this host
                            // callback remains in flight, before it reports error.
                            await_flag(&finished);
                        }
                        Err(FeregError::new(host_code, "host failure"))
                    },
                )
                .unwrap_err();
                assert_eq!(error.message, "host failure");
                assert_eq!(error.code, host_code);
                assert_eq!(polls, 1, "no SPI calls after the first host error");
                assert!(finished.load(Ordering::Acquire));
                assert!(!interrupt::cancellation_requested());
            }
        }
    }

    #[test]
    fn caller_panic_cancels_checkpoint_only_work_before_join() {
        if isolated("ppml_execution::tests::caller_panic_cancels_checkpoint_only_work_before_join")
        {
            return;
        }
        for threads in [1, 4] {
            let finished = AtomicBool::new(false);
            assert!(catch_unwind(AssertUnwindSafe(|| run(
                team(threads),
                |_| -> Result<()> {
                    let _finished = Finished(&finished);
                    let start = Instant::now();
                    loop {
                        interrupt::checkpoint()?;
                        assert!(start.elapsed() < Duration::from_secs(30));
                        std::thread::yield_now();
                    }
                },
                &mut |_| panic!("caller panic during idle poll"),
            )))
            .is_err());
            assert!(finished.load(Ordering::Acquire));
            assert!(!interrupt::cancellation_requested());
            assert_eq!(run(team(threads), |_| Ok(19), &mut |_| Ok(())).unwrap(), 19);
        }
    }

    #[test]
    #[cfg(feature = "ffi")]
    #[ignore = "process-global allocator audit; run with --test-threads=1"]
    fn repeated_scoped_family_work_and_callbacks_allocate_nothing_after_warmup() {
        let _guard = crate::instrumentation::lock_action_observation();
        let probe = crate::instrumentation::ppml_allocator_snapshot_probe(true).unwrap();
        for threads in [1, 4] {
            let team = team(threads);
            team.install(|| drop(rayon::broadcast(|_| ()))).unwrap();
            run(
                team,
                |poll| {
                    for _ in 0..16 {
                        poll(PpmlInterruptPoint::IrlsIteration)?;
                        team.install(|| rayon::join(|| 1, || 2))?;
                    }
                    let before = probe();
                    for _ in 0..256 {
                        poll(PpmlInterruptPoint::IrlsIteration)?;
                        team.install(|| rayon::join(|| 1, || 2))?;
                    }
                    let after = probe();
                    assert_eq!(before.allocation_events, after.allocation_events);
                    assert_eq!(before.current_bytes, after.current_bytes);
                    Ok(())
                },
                &mut |_| Ok(()),
            )
            .unwrap();
        }
    }
}
