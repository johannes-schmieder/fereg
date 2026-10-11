//! Optional preparation subphase clocks. These diagnostics do not change the
//! authoritative PPML phase accounting or any preparation decision.

use std::time::Instant;

pub(super) struct PreparationTrace {
    mark: Option<Instant>,
}

impl PreparationTrace {
    pub(super) fn new() -> Self {
        Self {
            mark: crate::diagnostics::phase_timing_enabled().then(Instant::now),
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.mark.is_some()
    }

    pub(super) fn lap(&mut self) -> f64 {
        let Some(mark) = self.mark.as_mut() else {
            return 0.0;
        };
        let now = Instant::now();
        let seconds = now.duration_since(*mark).as_secs_f64();
        *mark = now;
        seconds
    }

    pub(super) fn team_workers() -> usize {
        // Inspect an installed team only; diagnostics must not initialize the
        // global Rayon pool for direct serial core callers.
        rayon::current_thread_index().map_or(1, |_| rayon::current_num_threads())
    }
}
