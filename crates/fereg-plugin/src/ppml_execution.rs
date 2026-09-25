//! Keep native PPML work in one installed team while its caller owns SPI polls.
#![forbid(unsafe_code)]

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::ppml::PpmlInterruptPoint;
use fereg_core::ThreadTeam;
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
        self.changed.wait(state).unwrap_or_else(|error| error.into_inner())
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
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<T> {
    if team.threads() == 1 {
        return work(interrupt);
    }
    // The exchange is stack-owned, with one request and reply in flight.
    // Both peers close it on unwinding so the scope can always join.
    let exchange = Exchange {
        state: Mutex::new(State { point: None, reply: None, output: None, closed: false }),
        changed: Condvar::new(),
    };
    team.in_place_scope(|scope| {
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
                let reply = interrupt(point);
                exchange.lock().reply = Some(reply);
                exchange.changed.notify_all();
                continue;
            }
            if state.closed {
                return Err(stopped());
            }
            drop(exchange.wait(state));
        }
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use fereg_core::ThreadConfig;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn team(threads: usize) -> ThreadTeam {
        ThreadTeam::new(ThreadConfig::from_requested(threads, true).unwrap())
    }

    #[test]
    fn caller_keeps_non_send_callback_and_worker_uses_requested_team() {
        let caller = std::thread::current().id();
        for threads in [1, 4, 2, 4] {
            let polls = Rc::new(Cell::new(0));
            let result = run(team(threads), |poll| {
                assert_eq!(std::thread::current().id() == caller, threads == 1);
                if threads > 1 { assert_eq!(rayon::current_num_threads(), threads); }
                for _ in 0..128 { poll(PpmlInterruptPoint::IrlsIteration)?; }
                Ok(37)
            }, &mut |_| {
                assert_eq!(std::thread::current().id(), caller);
                polls.set(polls.get() + 1);
                Ok(())
            }).unwrap();
            assert_eq!(result, 37);
            assert_eq!(polls.get(), 128);
        }
    }

    #[test]
    fn callback_errors_and_both_peer_panics_join_and_allow_retry() {
        for threads in [1, 4] {
            let error = run(team(threads), |poll| {
                poll(PpmlInterruptPoint::Preparation)?;
                panic!("cancelled callback must stop computation");
                #[allow(unreachable_code)] Ok(())
            }, &mut |_| Err(FeregError::new(ErrorCode::Interrupted, "requested break"))).unwrap_err();
            assert_eq!(error.code, ErrorCode::Interrupted);
            for worker_panics in [false, true] {
                assert!(catch_unwind(AssertUnwindSafe(|| run(team(threads), |poll| {
                    if worker_panics { panic!("injected native panic"); }
                    poll(PpmlInterruptPoint::Preparation)?;
                    Ok(())
                }, &mut |_| panic!("injected caller panic")))).is_err());
                assert_eq!(run(team(threads), |_| Ok(11), &mut |_| Ok(())).unwrap(), 11);
            }
        }
    }

    #[test]
    #[cfg(feature = "ffi")]
    fn repeated_scoped_family_work_and_callbacks_allocate_nothing_after_warmup() {
        let _guard = crate::instrumentation::lock_action_observation();
        let probe = crate::instrumentation::ppml_allocator_snapshot_probe(true).unwrap();
        let team = team(4);
        team.install(|| drop(rayon::broadcast(|_| ()))).unwrap();
        run(team, |poll| {
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
        }, &mut |_| Ok(())).unwrap();
    }
}
