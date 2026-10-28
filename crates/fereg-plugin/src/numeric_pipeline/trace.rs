//! Trace-only producer/consumer intervals of one numeric input pipeline
//! (`FEREG_PHASE|numeric_pipeline`). Disabled, no clock is read.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub(super) struct PipelineTrace {
    started: Option<Instant>,
    nanos: [AtomicU64; 3],
    rows: usize,
    width: usize,
}

impl PipelineTrace {
    pub(super) const READ: usize = 0;
    pub(super) const WAIT: usize = 1;
    pub(super) const CONSUME: usize = 2;

    pub(super) fn new(rows: usize, width: usize) -> Self {
        Self {
            started: fereg_core::diagnostics::phase_timing_enabled().then(Instant::now),
            nanos: std::array::from_fn(|_| AtomicU64::new(0)),
            rows,
            width,
        }
    }

    pub(super) fn start(&self) -> Option<Instant> {
        self.started.map(|_| Instant::now())
    }

    pub(super) fn add(&self, slot: usize, started: Option<Instant>) {
        if let Some(started) = started {
            let nanos = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            self.nanos[slot].fetch_add(nanos, Ordering::Relaxed);
        }
    }
}

impl Drop for PipelineTrace {
    fn drop(&mut self) {
        let Some(started) = self.started else {
            return;
        };
        let seconds = |slot: usize| self.nanos[slot].load(Ordering::Relaxed) as f64 * 1e-9;
        eprintln!(
            "FEREG_PHASE|numeric_pipeline|total={:.9}|read={:.9}|wait={:.9}|consume={:.9}|rows={}|width={}",
            started.elapsed().as_secs_f64(),
            seconds(Self::READ),
            seconds(Self::WAIT),
            seconds(Self::CONSUME),
            self.rows,
            self.width
        );
    }
}
