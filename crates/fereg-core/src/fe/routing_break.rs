//! Developer Break point inside the synchronous bounded CMG builder.
//!
//! Only the hidden `routing_builder_break` injection arms it. The armed build
//! parks at its first cancellation poll after at least one counted
//! construction operation has executed, records readiness for the host's
//! Stata-thread poller, and waits until the host requests cancellation. The
//! following poll then propagates the cancellation as an interrupted native
//! estimation with atomic rollback; nothing resumes or publishes. Workers
//! never call Stata's SPI.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{invalid_argument, Result};

// The native host owns one action, as for the existing cancellation flag.
// Concurrent developer consumers must acquire this guard instead of sharing
// another action's marker.
static BREAK_ARMED: AtomicBool = AtomicBool::new(false);
static BREAK_READY: AtomicBool = AtomicBool::new(false);

/// Whether an armed builder has reached its Break point in this action.
#[doc(hidden)]
pub fn routing_builder_break_ready() -> bool {
    BREAK_READY.load(Ordering::Acquire)
}

/// Exclusive claim on the process-wide developer Break marker for one build.
pub(crate) struct DeveloperBreak;

impl DeveloperBreak {
    pub(crate) fn claim() -> Result<Self> {
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

/// Publish readiness at the given zero-based cancellation poll ordinal, then
/// block the constructing worker until the host requests cancellation.
pub(crate) fn park_until_cancelled(ordinal: u64) {
    BREAK_READY.store(true, Ordering::Release);
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=routing_builder|event=builder_break_ready|task=0|ordinal={ordinal}"
        ));
    }
    while !crate::interrupt::cancellation_requested() {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_is_exclusive_and_drop_clears_readiness() {
        let guard = DeveloperBreak::claim().unwrap();
        assert!(DeveloperBreak::claim().is_err());
        BREAK_READY.store(true, Ordering::Release);
        assert!(routing_builder_break_ready());
        drop(guard);
        assert!(!routing_builder_break_ready());
        drop(DeveloperBreak::claim().unwrap());
    }
}
