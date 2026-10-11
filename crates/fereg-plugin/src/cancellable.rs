//! Run native work on the action team while the caller polls the host's
//! interrupt signal.
//!
//! The work closure runs on a worker of the action's team. The calling thread
//! (Stata's main thread) stays in the scope and calls `poll` every
//! [`POLL_INTERVAL`] until the work finishes. When `poll` reports an
//! interruption, the runner requests cooperative cancellation; core loops then
//! stop at their next `fereg_core::interrupt::checkpoint`. The runner always
//! waits for the work to return, so no worker outlives the action, and it
//! returns the interruption even if the work completed first. The caller never
//! touches worker memory, and SPI stays on the caller's thread.
#![forbid(unsafe_code)]

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::{interrupt, ThreadTeam};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

/// Break latency is at most one interval plus the time to the next checkpoint.
#[cfg(not(test))]
pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(20);
#[cfg(test)]
pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(1);

struct State<T> {
    output: Option<Result<T>>,
    finished: bool,
}

struct Shared<T> {
    state: Mutex<State<T>>,
    changed: Condvar,
}

impl<T> Shared<T> {
    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

/// Marks the work finished even if it unwinds, so the caller stops waiting
/// and the scope can propagate the worker's panic.
struct Finished<'a, T>(&'a Shared<T>);

impl<T> Drop for Finished<'_, T> {
    fn drop(&mut self) {
        self.0.lock().finished = true;
        self.0.changed.notify_all();
    }
}

/// Clears any cancellation request when the action leaves the runner.
struct ResetCancellation;

impl Drop for ResetCancellation {
    fn drop(&mut self) {
        interrupt::reset_cancellation();
    }
}

struct CancelOnUnwind;

impl Drop for CancelOnUnwind {
    fn drop(&mut self) {
        if std::thread::panicking() {
            interrupt::request_cancellation();
        }
    }
}

/// Overlap optional Rust-owned preparation with caller-thread loading. Loading
/// errors dominate the optional result; the remaining tail keeps polling SPI.
pub(crate) fn while_loading<T: Send, U>(
    team: ThreadTeam,
    work: impl FnOnce() -> T + Send,
    load: impl FnOnce(&mut dyn crate::input::SpiApi) -> Result<U>,
    spi: &mut dyn crate::input::SpiApi,
) -> Result<(U, T)> {
    interrupt::reset_cancellation();
    let _reset = ResetCancellation;
    let shared = Shared {
        state: Mutex::new(State {
            output: None,
            finished: false,
        }),
        changed: Condvar::new(),
    };
    let mut interruption = None;
    let (loaded, output) = team.in_place_scope(|scope| {
        // A caller panic must request cancellation before the scope joins.
        let _cancel = CancelOnUnwind;
        scope.spawn(|_| {
            let _finished = Finished(&shared);
            let output = work();
            shared.lock().output = Some(Ok(output));
        });
        let loaded = load(spi);
        if loaded.is_err() {
            interrupt::request_cancellation();
        }
        let mut state = shared.lock();
        while !state.finished {
            let (next, timeout) = shared
                .changed
                .wait_timeout(state, POLL_INTERVAL)
                .unwrap_or_else(|error| error.into_inner());
            state = next;
            if timeout.timed_out() && !state.finished && loaded.is_ok() && interruption.is_none() {
                drop(state);
                if let Err(error) = spi.check_interrupted() {
                    interrupt::request_cancellation();
                    interruption = Some(error);
                }
                state = shared.lock();
            }
        }
        (loaded, state.output.take())
    })?;
    let loaded = loaded?;
    if let Some(error) = interruption {
        return Err(error);
    }
    let output = output.ok_or_else(|| {
        FeregError::new(
            ErrorCode::Internal,
            "input preparation worker returned no result",
        )
    })??;
    Ok((loaded, output))
}

pub(crate) fn run<T: Send>(
    team: ThreadTeam,
    work: impl FnOnce() -> Result<T> + Send,
    poll: &mut dyn FnMut() -> Result<()>,
) -> Result<T> {
    interrupt::reset_cancellation();
    let _reset = ResetCancellation;
    let shared = Shared {
        state: Mutex::new(State {
            output: None,
            finished: false,
        }),
        changed: Condvar::new(),
    };
    let mut interruption = None;
    let output = team.in_place_scope(|scope| {
        let worker_shared = &shared;
        scope.spawn(move |_| {
            let _finished = Finished(worker_shared);
            let output = work();
            worker_shared.lock().output = Some(output);
        });
        let mut state = shared.lock();
        loop {
            if state.finished {
                return state.output.take();
            }
            let (next, timeout) = shared
                .changed
                .wait_timeout(state, POLL_INTERVAL)
                .unwrap_or_else(|error| error.into_inner());
            state = next;
            if timeout.timed_out() && !state.finished && interruption.is_none() {
                drop(state);
                if let Err(error) = poll() {
                    interrupt::request_cancellation();
                    interruption = Some(error);
                }
                state = shared.lock();
            }
        }
    })?;
    if let Some(error) = interruption {
        return Err(error);
    }
    output.unwrap_or_else(|| {
        Err(FeregError::new(
            ErrorCode::Internal,
            "cancellable native work finished without a result",
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fereg_core::ThreadConfig;
    use std::cell::Cell;
    use std::rc::Rc;

    fn team(threads: usize) -> ThreadTeam {
        ThreadTeam::new(ThreadConfig::from_requested(threads, true).expect("threads"))
    }

    #[test]
    fn completed_work_returns_its_output_and_keeps_polls_on_the_caller() {
        for threads in [1, 3] {
            let caller = std::thread::current().id();
            // A non-Send poll proves the callback stays on the calling thread.
            let polls = Rc::new(Cell::new(0_usize));
            let observed = Rc::clone(&polls);
            // The worker waits for the first poll instead of a fixed sleep, so
            // a saturated host cannot finish the work before the caller polls.
            let polled = std::sync::atomic::AtomicBool::new(false);
            let mut poll = || {
                assert_eq!(std::thread::current().id(), caller);
                observed.set(observed.get() + 1);
                polled.store(true, std::sync::atomic::Ordering::Release);
                Ok(())
            };
            let output = run(
                team(threads),
                || {
                    assert_ne!(std::thread::current().id(), caller);
                    let started = std::time::Instant::now();
                    while !polled.load(std::sync::atomic::Ordering::Acquire)
                        && started.elapsed() < std::time::Duration::from_secs(30)
                    {
                        std::thread::sleep(POLL_INTERVAL);
                    }
                    Ok(threads * 2)
                },
                &mut poll,
            )
            .expect("work");
            assert_eq!(output, threads * 2);
            assert!(polls.get() >= 1);
            assert!(!interrupt::cancellation_requested());
        }
    }

    #[test]
    fn work_errors_propagate_without_polling_after_completion() {
        let error = run(
            team(2),
            || -> Result<()> { Err(FeregError::new(ErrorCode::InvalidData, "bad input")) },
            &mut || Ok(()),
        )
        .expect_err("work error");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    /// The cancellation flag is process-wide, so this runs in a child process.
    #[test]
    fn requested_break_stops_work_at_its_next_checkpoint() {
        const CHILD_ENV: &str = "FEREG_CANCELLABLE_BREAK_TEST_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .env(CHILD_ENV, "1")
                .args([
                    "cancellable::tests::requested_break_stops_work_at_its_next_checkpoint",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .expect("launch isolated cancellation test");
            assert!(status.success(), "isolated cancellation test failed");
            return;
        }
        for threads in [1, 4] {
            let checkpoints = std::sync::atomic::AtomicUsize::new(0);
            let mut polls = 0_usize;
            let error = run(
                team(threads),
                || -> Result<()> {
                    loop {
                        checkpoints.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        interrupt::checkpoint()?;
                        std::thread::sleep(Duration::from_micros(200));
                    }
                },
                &mut || {
                    polls += 1;
                    Err(FeregError::new(ErrorCode::Interrupted, "injected Break"))
                },
            )
            .expect_err("Break stops the work");
            assert_eq!(error.code, ErrorCode::Interrupted);
            assert_eq!(error.message, "injected Break");
            assert_eq!(polls, 1, "polling stops after the first interruption");
            assert!(checkpoints.load(std::sync::atomic::Ordering::Relaxed) >= 1);
            assert!(
                !interrupt::cancellation_requested(),
                "the runner resets the flag"
            );
        }
    }

    #[test]
    fn worker_panic_propagates_instead_of_waiting_forever() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(
                team(2),
                || -> Result<()> { panic!("injected cancellable worker panic") },
                &mut || Ok(()),
            )
        }));
        assert!(result.is_err());
        assert!(!interrupt::cancellation_requested());
    }
}
