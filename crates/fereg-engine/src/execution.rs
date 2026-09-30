//! Scoped worker polling with host callbacks confined to the calling thread.
use crate::{threads::ThreadTeam, ErrorCode, FeregError, Result};
use std::sync::{Condvar, Mutex, MutexGuard};

/// Worker-safe cooperative poll. Thread-bound host APIs belong in `run`'s callback.
pub(crate) type Poll<'a> = Option<&'a (dyn Fn() -> bool + Sync)>;

/// Borrowed worker-safe cancellation signal; host callbacks require a scoped relay.
#[derive(Clone, Copy, Default)]
pub struct Probe<'a>(Poll<'a>);
impl<'a> Probe<'a> {
    pub fn new(poll: &'a (dyn Fn() -> bool + Sync)) -> Self {
        Self(Some(poll))
    }
    pub(crate) fn poll(self) -> Poll<'a> {
        self.0
    }
}
impl std::fmt::Debug for Probe<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Probe")
            .field("enabled", &self.0.is_some())
            .finish()
    }
}

pub(crate) fn check(poll: Poll<'_>) -> Result<()> {
    if poll.is_some_and(|poll| !poll()) {
        Err(interrupted())
    } else {
        Ok(())
    }
}

fn interrupted() -> FeregError {
    FeregError::new(
        ErrorCode::Interrupted,
        "scoped numerical execution interrupted",
    )
}

struct State<T, P> {
    point: Option<P>,
    answered: bool,
    output: Option<Result<T>>,
    closed: bool,
}
struct Exchange<T, P> {
    state: Mutex<State<T, P>>,
    changed: Condvar,
}
impl<T, P: Copy> Exchange<T, P> {
    fn lock(&self) -> MutexGuard<'_, State<T, P>> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
    fn wait<'a>(&self, state: MutexGuard<'a, State<T, P>>) -> MutexGuard<'a, State<T, P>> {
        self.changed
            .wait(state)
            .unwrap_or_else(|error| error.into_inner())
    }
    fn poll(&self, point: P) -> bool {
        let mut state = self.lock();
        // Keep ownership of the request until its sender consumes the reply;
        // a sibling RHS cannot steal an acknowledgement or overwrite a point.
        while state.point.is_some() && !state.closed {
            state = self.wait(state);
        }
        if state.closed {
            return false;
        }
        state.point = Some(point);
        self.changed.notify_all();
        while !state.answered && !state.closed {
            state = self.wait(state);
        }
        let keep_running = !state.closed;
        state.point = None;
        state.answered = false;
        self.changed.notify_all();
        keep_running
    }
}
struct Close<'a, T, P>(&'a Exchange<T, P>);
impl<T, P> Drop for Close<'_, T, P> {
    fn drop(&mut self) {
        self.0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .closed = true;
        self.0.changed.notify_all();
    }
}

/// Run Rust-owned work in the supplied team while its caller performs host polls.
///
/// The callback may capture non-Send state. Concurrent worker requests are
/// serialized; all scoped work joins on error or either peer's panic. Callback
/// errors retain their original identity and override a worker's cancellation
/// result, so numerical recovery cannot reinterpret a host error as stagnation.
/// Nested calls from this team's workers fail rather than block a one-worker pool.
pub fn run<T: Send, P: Copy + Send>(
    team: &ThreadTeam,
    work: impl FnOnce(&(dyn Fn(P) -> bool + Sync)) -> Result<T> + Send,
    interrupt: &mut dyn FnMut(P) -> Result<()>,
) -> Result<T> {
    if team.is_worker() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "scoped host polling must start on the calling thread",
        ));
    }
    let exchange = Exchange {
        state: Mutex::new(State {
            point: None,
            answered: false,
            output: None,
            closed: false,
        }),
        changed: Condvar::new(),
    };
    let mut callback_failure = None;
    let result = team.in_place_scope(|scope| {
        let _caller_close = Close(&exchange);
        let worker_exchange = &exchange;
        scope.spawn(move |_| {
            let _worker_close = Close(worker_exchange);
            let output = work(&|point| worker_exchange.poll(point));
            worker_exchange.lock().output = Some(output);
        });
        loop {
            let mut state = exchange.lock();
            if let Some(output) = state.output.take() {
                return output;
            }
            if state.closed {
                return Err(interrupted());
            }
            if let Some(point) = state.point.filter(|_| !state.answered) {
                drop(state);
                let reply = interrupt(point);
                let mut state = exchange.lock();
                if let Err(error) = reply {
                    callback_failure = Some(error);
                    state.closed = true;
                } else {
                    state.answered = true;
                }
                exchange.changed.notify_all();
            } else {
                drop(exchange.wait(state));
            }
        }
    });
    callback_failure.map_or(result, Err)
}
