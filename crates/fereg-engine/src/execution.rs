//! Scoped worker polling with host callbacks confined to the calling thread.
mod sort;
use crate::{
    threads::{CompensatedSum, ThreadTeam, REDUCTION_CHUNK},
    ErrorCode, FeregError, Result,
};
pub(crate) use sort::sort_unstable_by;
use std::sync::{Condvar, Mutex, MutexGuard};

/// Worker-safe cooperative poll. Thread-bound host APIs belong in `run`'s callback.
pub(crate) type Poll<'a> = Option<&'a (dyn Fn() -> bool + Sync)>;

/// Borrowed worker-safe cancellation signal; host callbacks require a scoped relay.
#[derive(Clone, Copy, Default)]
pub struct Probe<'a> {
    iterations: Poll<'a>,
    dense: Poll<'a>,
}
impl<'a> Probe<'a> {
    pub fn new(poll: &'a (dyn Fn() -> bool + Sync)) -> Self {
        Self {
            iterations: Some(poll),
            dense: None,
        }
    }
    pub(crate) fn with_dense(
        iterations: &'a (dyn Fn() -> bool + Sync),
        dense: &'a (dyn Fn() -> bool + Sync),
    ) -> Self {
        Self {
            iterations: Some(iterations),
            dense: Some(dense),
        }
    }
    pub(crate) fn poll(self) -> Poll<'a> {
        self.iterations
    }
    pub(crate) fn dense(self) -> Poll<'a> {
        self.dense
    }
}
impl std::fmt::Debug for Probe<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Probe")
            .field("iterations", &self.iterations.is_some())
            .field("dense", &self.dense.is_some())
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

/// Fixed work partitions bound host-poll latency without changing additions.
pub(crate) fn check_chunk(poll: Poll<'_>, position: usize) -> Result<()> {
    if poll.is_some() {
        check_chunk_with(position, &mut || check(poll))?;
    }
    Ok(())
}

/// Preflight has no worker team yet; its borrowed callback stays on the caller.
pub(crate) fn check_chunk_with(
    position: usize,
    poll: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    if position != 0 && position % REDUCTION_CHUNK == 0 {
        poll()?;
    }
    Ok(())
}
pub(crate) fn fold<I, T>(
    values: I,
    initial: T,
    poll: Poll<'_>,
    mut add: impl FnMut(T, I::Item) -> T,
) -> Result<T>
where
    I: IntoIterator,
{
    let mut result = initial;
    for (position, value) in values.into_iter().enumerate() {
        check_chunk(poll, position)?;
        result = add(result, value);
    }
    Ok(result)
}
pub(crate) fn any<I>(
    values: I,
    predicate: impl FnMut(I::Item) -> bool,
    poll: Poll<'_>,
) -> Result<bool>
where
    I: IntoIterator,
{
    any_with(values, predicate, &mut || check(poll))
}
pub(crate) fn any_with<I>(
    values: I,
    mut predicate: impl FnMut(I::Item) -> bool,
    poll: &mut impl FnMut() -> Result<()>,
) -> Result<bool>
where
    I: IntoIterator,
{
    for (position, value) in values.into_iter().enumerate() {
        check_chunk_with(position, poll)?;
        if predicate(value) {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(crate) fn sum(values: impl IntoIterator<Item = f64>, poll: Poll<'_>) -> Result<f64> {
    Ok(
        fold(values, CompensatedSum::default(), poll, |mut sum, value| {
            sum.add(value);
            sum
        })?
        .total(),
    )
}
pub(crate) fn try_sum(
    values: impl IntoIterator<Item = Result<f64>>,
    poll: Poll<'_>,
) -> Result<f64> {
    let mut sum = CompensatedSum::default();
    for (position, value) in values.into_iter().enumerate() {
        check_chunk(poll, position)?;
        sum.add(value?);
    }
    Ok(sum.total())
}
pub(crate) fn blocked_sum(
    length: usize,
    value: impl Fn(usize) -> f64,
    poll: Poll<'_>,
) -> Result<f64> {
    try_blocked_sum(length, |row| Ok(value(row)), poll)
}
pub(crate) fn try_blocked_sum(
    length: usize,
    value: impl Fn(usize) -> Result<f64>,
    poll: Poll<'_>,
) -> Result<f64> {
    let mut sum = CompensatedSum::default();
    for start in (0..length).step_by(REDUCTION_CHUNK) {
        check(poll)?;
        sum.add(try_sum(
            (start..start.saturating_add(REDUCTION_CHUNK).min(length)).map(&value),
            None,
        )?);
    }
    Ok(sum.total())
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

#[cfg(test)]
pub(crate) mod interruption_tests;
