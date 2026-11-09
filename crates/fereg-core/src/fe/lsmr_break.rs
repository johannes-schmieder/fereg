//! Sanctioned developer Break witness after an executed upstream LSMR iteration.
//!
//! The host owns the guard and polls SPI. Only an armed fedemean modified
//! LSMR solve (OLS `solver(lsmr)`, feppml `solver(lsmr)`) observes the
//! witness, through its cancellation polls; ordinary calls poll as usual.
//! An armed call parks after completed recurrence work and cannot publish a fit.

use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use crate::error::{invalid_argument, Result};

const BACKEND: u8 = 3;
const READY: u8 = 4;
const CANCELLED: u8 = 8;
const CLAIMED: u8 = 16;
static STATE: AtomicU8 = AtomicU8::new(0);

/// The actual upstream entry point observed by a hidden Break injection.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DeveloperLsmrBackend {
    /// fedemean's modified LSMR (OLS and PPML `solver(lsmr)` on categorical
    /// intercepts), observed through its cancellation polls.
    Fedemean = 3,
}

/// Exclusive action-scoped ownership of the developer iteration marker.
#[doc(hidden)]
pub struct DeveloperLsmrBreak(());

impl DeveloperLsmrBreak {
    #[doc(hidden)]
    pub fn claim(backend: DeveloperLsmrBackend) -> Result<Self> {
        STATE
            .compare_exchange(0, backend as u8, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| invalid_argument("another developer LSMR Break action is active"))?;
        Ok(Self(()))
    }
}

impl Drop for DeveloperLsmrBreak {
    fn drop(&mut self) {
        STATE.store(0, Ordering::Release);
    }
}

/// Whether the selected upstream solver has completed recurrence work.
#[doc(hidden)]
pub fn lsmr_break_ready() -> bool {
    STATE.load(Ordering::Acquire) & READY != 0
}

/// Whether the Break is armed for `backend`.
pub(crate) fn armed(backend: DeveloperLsmrBackend) -> bool {
    STATE.load(Ordering::Acquire) & BACKEND == backend as u8
}

/// The cancellation poll of an armed fedemean solve. A modified LSMR pass
/// polls before its setup and at the start of every iteration, so from its
/// third poll on one iteration has completed: the solve parks as the
/// upstream policies do, then reports the host's cancellation.
pub(crate) fn fedemean_poll(polls: &AtomicUsize) -> bool {
    let poll = polls.fetch_add(1, Ordering::Relaxed) + 1;
    if poll >= 3 {
        park(DeveloperLsmrBackend::Fedemean as u8, poll - 2);
    }
    crate::interrupt::cancellation_requested()
}

/// Mark the first completed iteration ready (once) and wait for the host's
/// cancellation.
fn park(kind: u8, iteration: usize) {
    if STATE
        .compare_exchange(kind, kind | CLAIMED, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        trace(kind, "iteration_break_ready", Some(iteration));
        // The host may arm Ctrl-C as soon as it observes READY. Commit the
        // trace first, so another RHS cannot record cancellation before it.
        STATE.store(kind | READY, Ordering::Release);
    }
    while !crate::interrupt::cancellation_requested() {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// Record an actual upstream Cancelled result, rather than host-error precedence.
pub(crate) fn record_cancelled(backend: DeveloperLsmrBackend) {
    let kind = backend as u8;
    if STATE
        .compare_exchange(
            kind | READY,
            kind | READY | CANCELLED,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
    {
        trace(kind, "iteration_break_cancelled", None);
    }
}

fn trace(kind: u8, event: &str, iteration: Option<usize>) {
    if crate::diagnostics::solver_trace_enabled() {
        debug_assert_eq!(kind, DeveloperLsmrBackend::Fedemean as u8);
        let iteration = iteration.map_or(String::new(), |value| format!("|iteration={value}"));
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=lsmr_break|event={event}|backend=fedemean{iteration}"
        ));
    }
}

#[cfg(test)]
mod tests;
