//! Trace-only accumulation of named phase intervals (`FEREG_PHASE_TIMING`).
//!
//! A ledger reads the timing switch once. Disabled, every operation is a
//! branch on that flag: no clock reads, no allocation and no effect on any
//! solver arithmetic or decision. Enabled, it emits one `FEREG_PHASE|<label>`
//! record when dropped, so every exit of the measured operation reports.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub(crate) struct PhaseLedger<const N: usize> {
    enabled: bool,
    label: &'static str,
    names: [&'static str; N],
    nanos: [AtomicU64; N],
    counts: [AtomicU64; N],
    started: Option<Instant>,
    /// The end of the previous serial lap, in nanoseconds after `started`.
    last: AtomicU64,
    /// Integer facts appended to the record (sizes, iterations, outcomes).
    facts: Vec<(&'static str, u64)>,
}

impl<const N: usize> PhaseLedger<N> {
    pub(crate) fn new(label: &'static str, names: [&'static str; N]) -> Self {
        let enabled = super::phase_timing_enabled();
        Self {
            enabled,
            label,
            names,
            nanos: std::array::from_fn(|_| AtomicU64::new(0)),
            counts: std::array::from_fn(|_| AtomicU64::new(0)),
            started: enabled.then(Instant::now),
            last: AtomicU64::new(0),
            facts: Vec::new(),
        }
    }

    /// Charge one serial phase of the calling thread: the interval since the
    /// previous lap (or since creation) goes to `slot`.
    #[inline]
    pub(crate) fn lap(&self, slot: usize) {
        if let Some(started) = self.started {
            let now = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            let last = self.last.swap(now, Ordering::Relaxed);
            self.nanos[slot].fetch_add(now.saturating_sub(last), Ordering::Relaxed);
            self.counts[slot].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// The start of one interval, or none while timing is disabled.
    #[inline]
    pub(crate) fn start(&self) -> Option<Instant> {
        if self.enabled {
            Some(Instant::now())
        } else {
            None
        }
    }

    /// Charge the interval since `started` to `slot`; safe from any worker.
    #[inline]
    pub(crate) fn add(&self, slot: usize, started: Option<Instant>) {
        if let Some(started) = started {
            let nanos = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            self.nanos[slot].fetch_add(nanos, Ordering::Relaxed);
            self.counts[slot].fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn fact(&mut self, name: &'static str, value: u64) {
        if self.enabled {
            if let Some(entry) = self.facts.iter_mut().find(|(key, _)| *key == name) {
                entry.1 = value;
            } else {
                self.facts.push((name, value));
            }
        }
    }

    pub(crate) fn facts(&mut self, facts: &[(&'static str, u64)]) {
        for &(name, value) in facts {
            self.fact(name, value);
        }
    }

    pub(crate) fn seconds(&self, slot: usize) -> f64 {
        self.nanos[slot].load(Ordering::Relaxed) as f64 * 1e-9
    }
}

impl<const N: usize> Drop for PhaseLedger<N> {
    fn drop(&mut self) {
        let Some(started) = self.started else {
            return;
        };
        let mut line = format!(
            "FEREG_PHASE|{}|total={:.9}",
            self.label,
            started.elapsed().as_secs_f64()
        );
        for (slot, name) in self.names.iter().enumerate() {
            line.push_str(&format!(
                "|{name}={:.9}|{name}_count={}",
                self.seconds(slot),
                self.counts[slot].load(Ordering::Relaxed)
            ));
        }
        for (name, value) in &self.facts {
            line.push_str(&format!("|{name}={value}"));
        }
        super::emit_solver_trace(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_ledger_reads_no_clock() {
        let ledger = PhaseLedger::new("test", ["a"]);
        if !ledger.enabled {
            assert!(ledger.start().is_none());
            ledger.add(0, None);
            assert_eq!(ledger.seconds(0), 0.0);
        }
    }

    #[test]
    fn enabled_intervals_accumulate_by_slot() {
        let mut ledger = PhaseLedger {
            enabled: true,
            label: "test",
            names: ["a", "b"],
            nanos: std::array::from_fn(|_| AtomicU64::new(0)),
            counts: std::array::from_fn(|_| AtomicU64::new(0)),
            started: Some(Instant::now()),
            last: AtomicU64::new(0),
            facts: Vec::new(),
        };
        let started = ledger.start();
        ledger.add(1, started);
        ledger.add(1, ledger.start());
        assert_eq!(ledger.counts[0].load(Ordering::Relaxed), 0);
        assert_eq!(ledger.counts[1].load(Ordering::Relaxed), 2);
        ledger.lap(0);
        assert_eq!(ledger.counts[0].load(Ordering::Relaxed), 1);
        ledger.fact("rows", 3);
        ledger.fact("rows", 4);
        assert_eq!(ledger.facts, vec![("rows", 4)]);
        ledger.started = None;
    }
}
