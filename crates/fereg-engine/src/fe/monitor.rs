//! Observed-progress policy; no model-size, weight-kind or worker-count routing.
use super::StopReason;

/// Internal qualification controls; never public Stata syntax.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MonitorControl {
    #[default]
    Automatic,
    Disabled,
    StopAfter(usize),
}
pub(super) struct Monitor {
    best: f64,
    checkpoint: f64,
    fastest_rate: f64,
    windows: usize,
}
impl Monitor {
    pub(super) fn new(norm: f64) -> Self {
        Self {
            best: norm,
            checkpoint: norm,
            fastest_rate: 0.,
            windows: 0,
        }
    }
    pub(super) fn observe(
        &mut self,
        norm: f64,
        target: f64,
        iterations: usize,
        budget: usize,
        control: MonitorControl,
    ) -> Option<StopReason> {
        match control {
            MonitorControl::Disabled => return None,
            MonitorControl::StopAfter(limit) => {
                return (iterations >= limit).then_some(StopReason::Stagnation);
            }
            MonitorControl::Automatic => (),
        }
        self.best = self.best.min(norm);
        // Two complete windows avoid extrapolating the initial PCG transient.
        if iterations % 8 != 0 || iterations >= budget {
            return None;
        }
        let rate = (self.best / self.checkpoint).ln() / 8.;
        self.fastest_rate = self.fastest_rate.min(rate);
        self.windows += 1;
        let stalled = self.best >= 0.99 * self.checkpoint;
        self.checkpoint = self.best;
        if self.windows < 2 {
            return None;
        }
        if stalled {
            return Some(StopReason::Stagnation);
        }
        // Use the fastest observed window, so a slower recent window alone
        // cannot predict exhaustion. Only unfinished columns are monitored.
        let required = (target / self.best).ln() / self.fastest_rate;
        (required.is_finite() && required > (budget - iterations) as f64)
            .then_some(StopReason::ProjectedExhaustion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decisions_require_observed_progress_and_leave_remaining_budget() {
        let mut stalled = Monitor::new(1.);
        assert_eq!(
            stalled.observe(1., 1e-10, 8, 100, MonitorControl::Automatic),
            None
        );
        assert_eq!(
            stalled.observe(1., 1e-10, 16, 100, MonitorControl::Automatic),
            Some(StopReason::Stagnation)
        );
        let mut slow = Monitor::new(1.);
        assert_eq!(
            slow.observe(0.5, 1e-10, 8, 100, MonitorControl::Automatic),
            None
        );
        assert_eq!(
            slow.observe(0.25, 1e-10, 16, 100, MonitorControl::Automatic),
            Some(StopReason::ProjectedExhaustion)
        );
        let mut fast = Monitor::new(1.);
        assert_eq!(
            fast.observe(1e-3, 1e-10, 8, 100, MonitorControl::Automatic),
            None
        );
        assert_eq!(
            fast.observe(1e-6, 1e-10, 16, 100, MonitorControl::Automatic),
            None
        );
        assert_eq!(
            fast.observe(1e-6, 1e-10, 100, 100, MonitorControl::Automatic),
            None
        );
    }
}
