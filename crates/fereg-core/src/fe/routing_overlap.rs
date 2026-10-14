//! One existing executor worker owns a stackful, prepaid preparation cursor.
//!
//! The worker remains occupied even after completion. Only the controller's
//! prescribed boundary releases it; readiness never changes foreground work.
//! Fixed transport/Arc/Rayon bookkeeping uses the standard allocator. Numeric
//! payloads are separately fallibly admitted by the caller and controlled build.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use cmg::{BuildBoundary, BuildControl, BuildDecision, BuildPrefix, PrefixDecision, PrefixOutcome};

use crate::error::{invalid_argument, ErrorCode, FeregError, Result};

// The native host owns one action, as for the existing cancellation flag.
// Concurrent developer consumers must acquire this guard instead of sharing
// another action's marker. Workers never call Stata's SPI.
static BREAK_ARMED: AtomicBool = AtomicBool::new(false);
static BREAK_READY: AtomicBool = AtomicBool::new(false);

crate::fault::fault_plan! {
    static LAUNCH_TRACE_PANIC: bool = false;
}

#[doc(hidden)]
pub fn routing_builder_break_ready() -> bool {
    BREAK_READY.load(Ordering::Acquire)
}

struct DeveloperBreak;
impl DeveloperBreak {
    fn claim() -> Result<Self> {
        BREAK_ARMED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| invalid_argument("another developer builder Break action is active"))?;
        BREAK_READY.store(false, Ordering::Release);
        Ok(Self)
    }
}
impl Drop for DeveloperBreak {
    fn drop(&mut self) {
        BREAK_READY.store(false, Ordering::Release);
        BREAK_ARMED.store(false, Ordering::Release);
    }
}

/// Fixed terminal kind order; task 0 is foreground, task 1 preparation. The
/// order is independent of wall time and original RHS completion order.
pub(crate) fn arbitrate<T>(foreground: Result<T>, preparation: Result<()>) -> Result<T> {
    crate::interrupt::checkpoint()?;
    match (foreground, preparation) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(front), Err(back)) => Err(if fatal_rank(&front) <= fatal_rank(&back) {
            front
        } else {
            back
        }),
    }
}

fn fatal_rank(error: &FeregError) -> u16 {
    match error.code {
        ErrorCode::Interrupted => 0,
        ErrorCode::Panic => 1,
        ErrorCode::Internal => 2,
        ErrorCode::InvalidData => 3,
        ErrorCode::Allocation => 4,
        ErrorCode::NonConvergence => 6,
        _ => 5,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    Run,
    Drain,
    Abandon,
}

struct State<T> {
    issued: u64,
    charged: u64,
    ordinal: u64,
    command: Command,
    revision: u64,
    started: bool,
    drained: bool,
    release: bool,
    exited: bool,
    result: Option<Result<T>>,
}

struct Shared<T> {
    state: Mutex<State<T>>,
    changed: Condvar,
}

impl<T> Shared<T> {
    fn lock(&self) -> MutexGuard<'_, State<T>> {
        // User/kernel work never runs while holding this transport mutex.
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn wait<'a>(&self, guard: MutexGuard<'a, State<T>>) -> MutexGuard<'a, State<T>> {
        // Only Break polling uses a timeout. Grants and routes never use time.
        self.changed
            .wait_timeout(guard, Duration::from_millis(10))
            .unwrap_or_else(|error| error.into_inner())
            .0
    }
}

/// Owned by the immutable projection session, including compatible Low chunks.
/// Dropping the owner drains precisely issued work, abandons its suffix and
/// joins the leased worker before any borrowed input or publication can escape.
pub(super) struct Preparation<T> {
    shared: Arc<Shared<T>>,
    limit: u64,
    epoch: u64,
    joined: bool,
    terminal: Option<Result<T>>,
    permit: Option<crate::parallel::OverlapPermit>,
    developer_break: Option<DeveloperBreak>,
    task_id: u64,
}

impl<T: Send + 'static> Preparation<T> {
    pub(super) fn launch_with_break(
        limit: u64,
        epoch: u64,
        task_id: u64,
        break_builder: bool,
        build: impl FnOnce(&mut dyn BuildControl) -> Result<T> + Send + 'static,
    ) -> Result<Self> {
        if rayon::current_thread_index().is_none() || rayon::current_num_threads() < 2 {
            return Err(invalid_argument(
                "overlap requires an installed multi-worker team",
            ));
        }
        let permit = crate::parallel::claim_exclusive_overlap()?;
        let developer_break = break_builder.then(DeveloperBreak::claim).transpose()?;
        let issued = epoch.min(limit);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                issued,
                charged: 0,
                ordinal: 0,
                command: Command::Run,
                revision: 0,
                started: false,
                drained: false,
                release: false,
                exited: false,
                result: None,
            }),
            changed: Condvar::new(),
        });
        let worker = shared.clone();
        rayon::spawn(move || {
            // The coordinator waits without helping the executor until another
            // worker takes this lease. A blocking builder must not run on the
            // coordinator as a nested Rayon join-help task.
            {
                worker.lock().started = true;
                worker.changed.notify_all();
            }
            let mut control = Control {
                shared: worker.clone(),
                prefix: BuildPrefix::new(task_id, limit),
                error: None,
                revision: None,
                break_builder,
                break_reached: false,
            };
            let outcome = catch_unwind(AssertUnwindSafe(|| build(&mut control)));
            let mut result = control.error.take().map_or_else(
                || {
                    outcome.unwrap_or_else(|_| {
                        Err(FeregError::new(
                            ErrorCode::Panic,
                            "optional CMG preparation panicked",
                        ))
                    })
                },
                Err,
            );
            if let Some(error) = result
                .as_ref()
                .err()
                .filter(|_| control.prefix.snapshot().outcome.is_none())
            {
                let terminal = if error.code == ErrorCode::Interrupted {
                    PrefixOutcome::Interrupted
                } else {
                    PrefixOutcome::Fatal {
                        kind: fatal_rank(error),
                    }
                };
                if let Err(error) = control.prefix.finish(terminal) {
                    result = Err(FeregError::new(
                        ErrorCode::Internal,
                        format!("optional CMG terminal protocol failed: {error:?}"),
                    ));
                }
            }
            let mut state = worker.lock();
            state.charged = control.prefix.snapshot().charged;
            state.ordinal = control.prefix.snapshot().next_ordinal;
            state.result = Some(result);
            state.drained = true;
            worker.changed.notify_all();
            while !state.release {
                state = worker.wait(state);
            }
            // No executor work or numerical ownership remains after this flag.
            state.exited = true;
            worker.changed.notify_all();
        });
        // Establish cleanup ownership before diagnostics or any later unwind.
        // Even a failing stderr write must abandon and join the leased worker.
        let preparation = Self {
            shared,
            limit,
            epoch,
            joined: false,
            terminal: None,
            permit: Some(permit),
            developer_break,
            task_id,
        };
        let mut state = preparation.shared.lock();
        while !state.started {
            state = preparation.shared.wait(state);
        }
        drop(state);
        if LAUNCH_TRACE_PANIC.get() {
            LAUNCH_TRACE_PANIC.set(false);
            panic!("optional preparation launch diagnostic witness");
        }
        trace("launch", task_id, issued, None, None);
        Ok(preparation)
    }

    /// Called only after the foreground's certificate decision at an epoch.
    pub(super) fn grant_epoch(&mut self) -> Result<()> {
        let mut state = self.shared.lock();
        state.issued = state.issued.saturating_add(self.epoch).min(self.limit);
        state.command = Command::Run;
        state.revision += 1;
        state.drained = false;
        self.shared.changed.notify_all();
        trace("grant", self.task_id, state.issued, None, None);
        Ok(())
    }

    /// Retain the parked continuation and ledger for the next committed chunk.
    pub(super) fn drain(&mut self) -> Result<u64> {
        let mut state = self.shared.lock();
        state.command = Command::Drain;
        state.revision += 1;
        self.shared.changed.notify_all();
        while !state.drained && state.result.is_none() {
            state = self.shared.wait(state);
        }
        trace(
            "drain",
            self.task_id,
            state.issued,
            Some(state.charged),
            Some(state.ordinal),
        );
        crate::interrupt::checkpoint()?;
        if let Some(Err(error)) = &state.result {
            return Err(error.clone());
        }
        Ok(state.charged)
    }

    /// At an uncertified h, issue the full allowance and join at that frozen
    /// iterate. At a committed-RHS boundary, drain G and abandon only its suffix.
    pub(super) fn finish_accounted(mut self, checkpoint: bool) -> (Result<T>, u64) {
        self.stop(checkpoint);
        let state = self.shared.lock();
        let (issued, charged, ordinal) = (state.issued, state.charged, state.ordinal);
        drop(state);
        trace("join", self.task_id, issued, Some(charged), Some(ordinal));
        let result = self.terminal.take().expect("joined preparation result");
        (crate::interrupt::checkpoint().and(result), charged)
    }

    /// Accounting read at a prescribed drain/terminal boundary, never readiness.
    pub(super) fn charged(&self) -> u64 {
        self.shared.lock().charged
    }
}

impl<T> Preparation<T> {
    fn stop(&mut self, checkpoint: bool) {
        if self.joined {
            return;
        }
        let mut state = self.shared.lock();
        if checkpoint {
            state.issued = self.limit;
            state.command = Command::Run;
        } else {
            state.command = Command::Abandon;
        }
        state.revision += 1;
        self.shared.changed.notify_all();
        while state.result.is_none() {
            state = self.shared.wait(state);
        }
        // A worker's final transport Arc can outlive its exit acknowledgement.
        // Transfer every numerical payload to the controller before release.
        self.terminal = state.result.take();
        state.release = true;
        self.shared.changed.notify_all();
        while !state.exited {
            state = self.shared.wait(state);
        }
        self.joined = true;
    }
}

impl<T> Drop for Preparation<T> {
    fn drop(&mut self) {
        self.stop(false);
        // Discarded private state is destroyed while the ownership reservation
        // remains held. The shared transport is already empty on every path.
        self.terminal.take();
        self.permit.take();
        self.developer_break.take();
    }
}

struct Control<T> {
    shared: Arc<Shared<T>>,
    prefix: BuildPrefix,
    error: Option<FeregError>,
    revision: Option<u64>,
    break_builder: bool,
    break_reached: bool,
}

impl<T> BuildControl for Control<T> {
    fn checkpoint(&mut self, boundary: BuildBoundary) -> BuildDecision {
        let mut state = self.shared.lock();
        loop {
            if self.break_builder && !self.break_reached && self.prefix.snapshot().charged > 0 {
                // This callback follows an already returned positive work gate,
                // so at least that operation has actually executed on the lease.
                self.break_reached = true;
                BREAK_READY.store(true, Ordering::Release);
                self.shared.changed.notify_all();
                trace(
                    "builder_break_ready",
                    self.prefix
                        .snapshot()
                        .last_admitted
                        .map_or(0, |p| p.task_id),
                    state.issued,
                    Some(self.prefix.snapshot().charged),
                    Some(self.prefix.snapshot().next_ordinal),
                );
                while !crate::interrupt::cancellation_requested()
                    && state.command != Command::Abandon
                {
                    state = self.shared.wait(state);
                }
            }
            if crate::interrupt::cancellation_requested() {
                self.prefix.interrupt();
            }
            let command = if self.revision != Some(state.revision) {
                self.revision = Some(state.revision);
                self.prefix
                    .grant(state.issued)
                    .and_then(|()| match state.command {
                        Command::Run => Ok(()),
                        Command::Drain => self.prefix.drain_issued_prefix(),
                        Command::Abandon => self.prefix.abandon_unissued_suffix(),
                    })
            } else {
                Ok(())
            };
            let decision = command.and_then(|()| self.prefix.admit(boundary));
            state.charged = self.prefix.snapshot().charged;
            state.ordinal = self.prefix.snapshot().next_ordinal;
            match decision {
                Ok(PrefixDecision::Continue) => return BuildDecision::Continue,
                Ok(PrefixDecision::StopUnused) => return BuildDecision::StopUnused,
                Ok(PrefixDecision::Interrupted) => return BuildDecision::Interrupted,
                Ok(PrefixDecision::Park) => {
                    state.drained = true;
                    self.shared.changed.notify_all();
                    state = self.shared.wait(state);
                }
                Err(error) => {
                    self.error = Some(FeregError::new(
                        ErrorCode::Internal,
                        format!("optional CMG prefix protocol failed: {error:?}"),
                    ));
                    return BuildDecision::StopUnused;
                }
            }
        }
    }
}

fn trace(event: &str, task: u64, issued: u64, charged: Option<u64>, ordinal: Option<u64>) {
    if crate::diagnostics::solver_trace_enabled() {
        let charged = charged.map_or(String::new(), |value| format!("|charged={value}"));
        let ordinal = ordinal.map_or(String::new(), |value| format!("|ordinal={value}"));
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=routing_overlap|event={event}|task={task}|issued={issued}{charged}{ordinal}"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cmg::BuildOperation;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn isolated(name: &str) -> bool {
        const FLAG: &str = "FEREG_ROUTING_OVERLAP_TEST_CHILD";
        if std::env::var(FLAG).ok().as_deref() == Some(name) {
            return false;
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(FLAG, name)
            .args([
                &format!("fe::routing_overlap::tests::{name}"),
                "--exact",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        assert!(
            status.success(),
            "isolated exclusive overlap witness failed"
        );
        true
    }

    fn launch<T: Send + 'static>(
        build: impl FnOnce(&mut dyn BuildControl) -> Result<T> + Send + 'static,
    ) -> Preparation<T> {
        Preparation::launch_with_break(30, 7, 1, false, build).unwrap()
    }

    fn finish<T: Send + 'static>(
        preparation: Preparation<T>,
        checkpoint: bool,
    ) -> Result<(T, u64)> {
        let (result, charged) = preparation.finish_accounted(checkpoint);
        result.map(|value| (value, charged))
    }

    fn team(workers: usize) -> crate::parallel::ThreadTeam {
        crate::parallel::ThreadTeam::new(
            crate::options::ThreadConfig::from_requested(workers, true).unwrap(),
        )
        .with_exclusive_overlap()
    }

    fn work(control: &mut dyn BuildControl, n: usize) -> Result<Vec<usize>> {
        let mut executed = Vec::new();
        for ordinal in 0..n {
            match control.checkpoint(BuildBoundary {
                phase: "owned-service-witness",
                chunk_id: ordinal as u64,
                charged_work: ordinal as u64 * 3,
                operation: BuildOperation::Work { units: 3 },
            }) {
                BuildDecision::Continue => executed.push(ordinal),
                BuildDecision::StopUnused => break,
                BuildDecision::Interrupted => {
                    return Err(FeregError::new(
                        ErrorCode::Interrupted,
                        "witness interrupted",
                    ))
                }
            }
        }
        Ok(executed)
    }

    #[test]
    fn discarded_payload_drops_on_controller_before_permit_release() {
        if isolated("discarded_payload_drops_on_controller_before_permit_release") {
            return;
        }
        struct Payload(Arc<Mutex<Option<(std::thread::ThreadId, bool)>>>);
        impl Drop for Payload {
            fn drop(&mut self) {
                *self.0.lock().unwrap() = Some((
                    std::thread::current().id(),
                    crate::parallel::claim_exclusive_overlap().is_err(),
                ));
            }
        }
        team(2)
            .install(|| {
                let dropped = Arc::new(Mutex::new(None));
                let worker_dropped = dropped.clone();
                let preparation = launch(move |_| Ok(Payload(worker_dropped)));
                // Keep transport alive exactly as a worker's last Arc may be
                // alive after acknowledgement. It must no longer own payload.
                let transport = preparation.shared.clone();
                let controller = std::thread::current().id();
                drop(preparation);
                assert_eq!(*dropped.lock().unwrap(), Some((controller, true)));
                assert!(transport.lock().result.is_none());
                assert!(crate::parallel::claim_exclusive_overlap().is_ok());
            })
            .unwrap();
    }

    #[test]
    fn launch_diagnostic_unwind_joins_the_issued_prefix() {
        if isolated("launch_diagnostic_unwind_joins_the_issued_prefix") {
            return;
        }
        let executed = Arc::new(AtomicUsize::new(0));
        team(2)
            .install(|| {
                let worker_executed = executed.clone();
                LAUNCH_TRACE_PANIC.set(true);
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    launch(move |control| {
                        let work = work(control, 20)?;
                        worker_executed.store(work.len(), Ordering::Release);
                        Ok(())
                    })
                }));
                assert!(outcome.is_err());
                assert_eq!(executed.load(Ordering::Acquire), 2);
                assert!(crate::parallel::claim_exclusive_overlap().is_ok());
            })
            .unwrap();
    }

    #[test]
    fn developer_break_interrupts_executed_work_and_resets_after_join() {
        if isolated("developer_break_interrupts_executed_work_and_resets_after_join") {
            return;
        }
        crate::interrupt::reset_cancellation();
        let executed = Arc::new(AtomicUsize::new(0));
        team(2)
            .install(|| {
                let worker_executed = executed.clone();
                let preparation = Preparation::launch_with_break(30, 7, 1, true, move |control| {
                    for ordinal in 0..2 {
                        match control.checkpoint(BuildBoundary {
                            phase: "developer-break-witness",
                            chunk_id: ordinal,
                            charged_work: ordinal * 3,
                            operation: BuildOperation::Work { units: 3 },
                        }) {
                            BuildDecision::Continue => {
                                worker_executed.fetch_add(1, Ordering::Release);
                            }
                            BuildDecision::Interrupted => {
                                return Err(FeregError::new(
                                    ErrorCode::Interrupted,
                                    "builder Break witness",
                                ));
                            }
                            BuildDecision::StopUnused => {
                                panic!("issued witness work was abandoned")
                            }
                        }
                    }
                    Ok(())
                })
                .unwrap();
                let mut state = preparation.shared.lock();
                while !routing_builder_break_ready() {
                    state = preparation.shared.wait(state);
                }
                assert_eq!(executed.load(Ordering::Acquire), 1);
                assert_eq!(state.charged, 3);
                assert!(DeveloperBreak::claim().is_err());
                crate::interrupt::request_cancellation();
                drop(state);
                let (result, charged) = preparation.finish_accounted(false);
                assert_eq!(result.unwrap_err().code, ErrorCode::Interrupted);
                assert_eq!(charged, 3);
                assert!(!routing_builder_break_ready());
                assert!(!BREAK_ARMED.load(Ordering::Acquire));
                assert!(crate::parallel::claim_exclusive_overlap().is_ok());

                crate::interrupt::reset_cancellation();
                let (result, charged) = finish(launch(|control| work(control, 2)), true).unwrap();
                assert_eq!(result, vec![0, 1]);
                assert_eq!(charged, 6);
                assert!(!routing_builder_break_ready());
            })
            .unwrap();
    }

    #[test]
    fn low_drains_keep_one_cursor_and_one_cumulative_allowance() {
        if isolated("low_drains_keep_one_cursor_and_one_cumulative_allowance") {
            return;
        }
        for workers in [2, 4] {
            team(workers)
                .install(|| {
                    let mut preparation = launch(|c| work(c, 20));
                    assert_eq!(preparation.drain().unwrap(), 6);
                    preparation.grant_epoch().unwrap();
                    assert_eq!(preparation.drain().unwrap(), 12);
                    // A third compatible chunk adds to G; it never resets to 7.
                    preparation.grant_epoch().unwrap();
                    assert_eq!(preparation.drain().unwrap(), 21);
                    let (executed, charged) = finish(preparation, false).unwrap();
                    assert_eq!(charged, 21);
                    assert_eq!(executed, (0..7).collect::<Vec<_>>());
                })
                .unwrap();
        }
    }

    #[test]
    fn checkpoint_finishes_full_grant_and_completed_worker_stays_leased() {
        if isolated("checkpoint_finishes_full_grant_and_completed_worker_stays_leased") {
            return;
        }
        team(2)
            .install(|| {
                let mut preparation = launch(|c| work(c, 2));
                assert_eq!(preparation.drain().unwrap(), 6);
                assert!(crate::parallel::claim_exclusive_overlap().is_err());
                let (executed, charged) = finish(preparation, true).unwrap();
                assert_eq!((executed, charged), (vec![0, 1], 6));
                let permit = crate::parallel::claim_exclusive_overlap().unwrap();
                drop(permit);

                let mut preparation = launch(|c| work(c, 20));
                assert_eq!(preparation.drain().unwrap(), 6);
                let (executed, charged) = finish(preparation, true).unwrap();
                assert_eq!(charged, 30);
                assert_eq!(executed, (0..10).collect::<Vec<_>>());
            })
            .unwrap();
    }

    #[test]
    fn drop_joins_the_issued_prefix_and_abandons_unissued_faults() {
        if isolated("drop_joins_the_issued_prefix_and_abandons_unissued_faults") {
            return;
        }
        let count = Arc::new(AtomicUsize::new(0));
        team(2)
            .install(|| {
                let output = count.clone();
                let preparation = launch(move |control| {
                    let executed = work(control, 20)?;
                    output.store(executed.len(), Ordering::Release);
                    Ok(())
                });
                drop(preparation);
                assert_eq!(count.load(Ordering::Acquire), 2);
                assert!(crate::parallel::claim_exclusive_overlap().is_ok());
            })
            .unwrap();
    }

    #[test]
    fn fatal_and_panic_are_joined_before_terminal_arbitration() {
        if isolated("fatal_and_panic_are_joined_before_terminal_arbitration") {
            return;
        }
        team(2)
            .install(|| {
                let mut preparation = launch::<()>(|control| {
                    work(control, 1)?;
                    Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "admitted witness fault",
                    ))
                });
                assert_eq!(
                    preparation.drain().unwrap_err().code,
                    ErrorCode::InvalidData
                );
                let back = finish(preparation, false).map(|_| ());
                assert_eq!(
                    arbitrate(Ok(()), back).unwrap_err().code,
                    ErrorCode::InvalidData
                );
                let preparation = launch::<()>(|_| panic!("witness panic"));
                assert_eq!(
                    finish(preparation, false).unwrap_err().code,
                    ErrorCode::Panic
                );
                assert!(crate::parallel::claim_exclusive_overlap().is_ok());
            })
            .unwrap();
    }

    #[test]
    fn schedule_perturbation_does_not_change_the_issued_prefix() {
        if isolated("schedule_perturbation_does_not_change_the_issued_prefix") {
            return;
        }
        for workers in [2, 4] {
            let mut records = Vec::new();
            for builder_first in [false, true] {
                let started = Arc::new(std::sync::Barrier::new(2));
                let release = Arc::new(std::sync::Barrier::new(2));
                let record = team(workers)
                    .install(|| {
                        let worker_started = started.clone();
                        let worker_release = release.clone();
                        let mut preparation = launch(move |control| {
                            worker_started.wait();
                            worker_release.wait();
                            work(control, 20)
                        });
                        started.wait();
                        if builder_first {
                            release.wait();
                            preparation.drain().unwrap();
                        }
                        // A synthetic joined foreground epoch cannot alter G.
                        assert_eq!(rayon::join(|| 2 + 3, || 7 - 2), (5, 5));
                        preparation.grant_epoch().unwrap();
                        if !builder_first {
                            release.wait();
                        }
                        assert_eq!(preparation.drain().unwrap(), 12);
                        finish(preparation, false).unwrap()
                    })
                    .unwrap();
                records.push(record);
            }
            assert_eq!(records[0], records[1]);
        }
    }
}
