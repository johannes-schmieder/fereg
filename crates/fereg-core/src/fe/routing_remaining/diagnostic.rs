//! Diagnostic-only economic intervals. No timestamp is read by a decision.
use std::time::{Duration, Instant};

pub(super) struct Trace {
    started: Instant,
    excluded: Duration,
    grant: Option<u128>,
    build: Option<u128>,
    startup: Option<u128>,
    restart: Option<u128>,
    continuation: Option<u128>,
    installed: bool,
}

impl Trace {
    pub fn new() -> Option<Self> {
        (crate::diagnostics::phase_timing_enabled() && crate::diagnostics::solver_trace_enabled())
            .then(|| Self {
                started: Instant::now(),
                excluded: Duration::ZERO,
                grant: None,
                build: None,
                startup: None,
                restart: None,
                continuation: None,
                installed: false,
            })
    }

    pub fn stamp(&self) -> u128 {
        self.stamp_at(Instant::now())
    }

    pub fn stamp_at(&self, at: Instant) -> u128 {
        at.duration_since(self.started)
            .checked_sub(self.excluded)
            .expect("excluded intervals do not overlap")
            .as_nanos()
    }

    pub fn exclude(&mut self, started: Instant) {
        self.excluded += started.elapsed();
    }

    pub fn grant(&mut self, stamp: u128) {
        self.grant = Some(stamp);
    }

    pub fn built(&mut self, elapsed: Duration, installed: bool) {
        self.build = Some(elapsed.as_nanos());
        self.installed = installed;
    }

    pub fn awaiting_startup(&self) -> bool {
        self.installed && self.continuation.is_none()
    }

    pub fn startup(&mut self, elapsed: Duration) {
        self.startup = Some(elapsed.as_nanos());
    }

    pub fn initialized(&mut self) {
        self.initialized_at(self.stamp());
    }

    fn initialized_at(&mut self, stamp: u128) {
        if !self.awaiting_startup() {
            return;
        }
        self.restart = self
            .grant
            .and_then(|grant| stamp.checked_sub(grant))
            .and_then(|elapsed| elapsed.checked_sub(self.build?))
            .and_then(|elapsed| elapsed.checked_sub(self.startup?));
        self.continuation = Some(stamp);
    }

    pub fn finish(&self, stamp: u128, fields: &str) {
        let continuation = self.continuation.and_then(|start| stamp.checked_sub(start));
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_POLICY_LABEL|action_end|{fields}|elapsed_ns={stamp}|build_ns={}|restart_ns={}|startup_ns={}|continuation_ns={}|installed={}",
            super::label_integer(self.build), super::label_integer(self.restart),
            super::label_integer(self.startup), super::label_integer(continuation), u8::from(self.installed)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn economic_clock_partitions_once_and_excludes_observations() {
        let start = Instant::now();
        let mut trace = Trace {
            started: start,
            excluded: Duration::from_nanos(7),
            grant: None,
            build: None,
            startup: None,
            restart: None,
            continuation: None,
            installed: false,
        };
        assert_eq!(trace.stamp_at(start + Duration::from_nanos(17)), 10);
        trace.grant(10);
        trace.built(Duration::from_nanos(30), true);
        assert!(trace.awaiting_startup());
        trace.startup(Duration::from_nanos(20));
        trace.initialized_at(95);
        assert_eq!(trace.restart, Some(35));
        assert!(!trace.awaiting_startup());
        // Later correction initialization cannot replace the first startup.
        trace.initialized_at(110);
        assert_eq!(trace.continuation, Some(95));
        let end = trace.stamp_at(start + Duration::from_nanos(132));
        assert_eq!(end, 125);
        assert_eq!(
            end - trace.grant.unwrap(),
            trace.build.unwrap() + trace.startup.unwrap() + trace.restart.unwrap() + end
                - trace.continuation.unwrap()
        );
    }

    #[test]
    fn refused_or_incomplete_build_never_supplies_a_complete_partition() {
        let mut trace = Trace {
            started: Instant::now(),
            excluded: Duration::ZERO,
            grant: Some(10),
            build: None,
            startup: None,
            restart: None,
            continuation: None,
            installed: false,
        };
        trace.built(Duration::from_nanos(30), false);
        trace.initialized_at(95);
        assert_eq!(trace.continuation, None);
        trace.built(Duration::from_nanos(30), true);
        trace.initialized_at(95);
        assert_eq!(trace.restart, None); // Missing startup label is not zero.
    }
}
