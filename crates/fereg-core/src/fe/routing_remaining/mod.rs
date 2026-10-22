//! Caller-owned state for the two-FE remaining-cost policy.
//!
//! Observation can defer an optional attempt. It cannot certify a result,
//! change an iteration allowance or reject an already admitted hierarchy.
mod diagnostic;
#[cfg(test)]
mod integration_tests;
mod model;
mod progress;
#[cfg(test)]
mod tests;

use crate::error::{allocation, Result};
use crate::memory::{fallible_empty, FeSolverMemoryBudget};
use model::{CheapStructure, Coefficients, Decision, Execution, FutureGroup, Unknown, Work};
use progress::{forecast_lane, Forecast, LogWindow};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Predict,
    Never,
    Force(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Demand {
    Packed {
        regressors: usize,
    },
    Outcome {
        regressors: usize,
        block: usize,
    },
    Regressors {
        start: usize,
        end: usize,
        total: usize,
        block: usize,
    },
    Final,
    OneOff,
}

impl Demand {
    fn label(self) -> String {
        let (role, start, end, total, block) = match self {
            Self::Packed { regressors } => ("packed", 0, regressors, regressors, regressors),
            Self::Outcome { regressors, block } => ("outcome", 0, 0, regressors, block),
            Self::Regressors {
                start,
                end,
                total,
                block,
            } => ("regressor", start, end, total, block),
            Self::Final => ("final", 0, 0, 0, 0),
            Self::OneOff => ("oneoff", 0, 0, 0, 0),
        };
        format!("role={role}|start={start}|end={end}|total={total}|block={block}")
    }
    fn role(self) -> Option<usize> {
        match self {
            Self::Packed { .. } => Some(0),
            Self::Outcome { .. } => Some(1),
            Self::Regressors { .. } => Some(2),
            Self::Final => Some(3),
            Self::OneOff => None,
        }
    }

    fn future(self) -> Option<(usize, usize, bool)> {
        match self {
            Self::Packed { .. } => Some((0, 1, true)),
            Self::Outcome { regressors, block } => (block > 0).then_some((regressors, block, true)),
            Self::Regressors {
                start,
                end,
                total,
                block,
            } => (block > 0 && start < end && end <= total && end - start <= block).then_some((
                total.checked_sub(end)?,
                block,
                true,
            )),
            Self::Final | Self::OneOff => Some((0, 1, false)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    steps: u128,
    live: u128,
}

#[derive(Clone, Copy, Debug)]
struct History {
    width: usize,
    actions: u128,
    original_width: u128,
    counts: Counts,
}

impl History {
    fn add(&mut self, counts: Counts) -> Option<()> {
        let next = Self {
            width: self.width,
            actions: self.actions.checked_add(1)?,
            original_width: self.original_width.checked_add(self.width as u128)?,
            counts: Counts {
                steps: self.counts.steps.checked_add(counts.steps)?,
                live: self.counts.live.checked_add(counts.live)?,
            },
        };
        *self = next;
        Some(())
    }

    fn work(self) -> std::result::Result<Work, Unknown> {
        Work::from_history(
            self.width,
            self.actions,
            self.counts.steps,
            self.counts.live,
            self.original_width,
        )
    }
}

/// One support byte per role × weights × scaling × recurrence × progress.
/// Bit zero admits current work; bits 1/2/3 admit none/compatible/mixed-width
/// future history. All live progress classes must be admitted.
struct Calibration {
    coefficients: Coefficients,
    support: [u8; 96],
}

// Filled only from the frozen calibration protocol. An uncalibrated model
// cannot supply economic build credit. Forced controls remain diagnostic.
const CALIBRATION: Option<Calibration> = None;

pub(crate) struct State {
    mode: Mode,
    accuracy: Option<(u64, bool)>,
    demand: Demand,
    width: usize,
    stride: usize,
    history: Option<History>,
    counts: Option<Counts>,
    logs: Vec<[f64; 2]>,
    epoch: usize,
    last_age: Option<usize>,
    trace: bool,
    diagnostic: Option<diagnostic::Trace>,
}

impl State {
    pub(crate) const fn shell_bytes() -> u128 {
        std::mem::size_of::<Self>() as u128
    }

    pub(crate) fn new(mode: Mode) -> Self {
        Self {
            mode,
            accuracy: None,
            demand: Demand::OneOff,
            width: 0,
            stride: 0,
            history: None,
            counts: Some(Counts::default()),
            logs: Vec::new(),
            epoch: 0,
            last_age: None,
            trace: crate::diagnostics::solver_trace_enabled(),
            diagnostic: None,
        }
    }

    pub(crate) fn begin(
        &mut self,
        demand: Demand,
        width: usize,
        stride: usize,
        tolerance: f64,
        final_residual: bool,
    ) {
        self.release_observations();
        let key = (tolerance.to_bits(), final_residual);
        if self.accuracy != Some(key) {
            self.history = None;
            self.accuracy = Some(key);
        }
        self.demand = demand;
        self.width = width;
        self.stride = stride;
        self.counts = Some(Counts::default());
        self.epoch = 0;
        self.last_age = None;
        self.diagnostic = diagnostic::Trace::new();
    }

    /// Only called after direct selection, with the shell already charged to
    /// the parent's budget. Lack of optional observation memory is a deferral.
    pub(crate) fn prepare_observations(
        &mut self,
        budget: Option<FeSolverMemoryBudget>,
    ) -> Result<u128> {
        let started = self.measurement_started();
        let result = self.allocate_observations(budget);
        self.exclude_measurement(started);
        result
    }

    fn allocate_observations(&mut self, budget: Option<FeSolverMemoryBudget>) -> Result<u128> {
        let bytes = (self.width as u128)
            .checked_mul(16)
            .ok_or_else(|| allocation("remaining-cost observation size overflows"))?;
        if budget.is_some_and(|b| !b.fits(bytes)) {
            return Ok(0);
        }
        crate::interrupt::checkpoint()?;
        self.logs = fallible_empty(self.width, "unable to reserve remaining-cost observations")?;
        let actual = self.logs.capacity() as u128 * 16;
        if budget.is_some_and(|b| !b.fits(actual)) {
            self.release_observations();
            return Err(allocation(
                "remaining-cost observation capacity exceeds admission",
            ));
        }
        while self.logs.len() < self.width {
            crate::interrupt::checkpoint()?;
            let end = self.logs.len().saturating_add(1024).min(self.width);
            self.logs.resize(end, [f64::NAN; 2]);
        }
        crate::interrupt::checkpoint()?;
        Ok(actual)
    }

    pub(crate) fn release_observations(&mut self) {
        self.logs = Vec::new();
        self.last_age = None;
    }

    pub(crate) fn correction(&mut self) -> Result<()> {
        let started = self.measurement_started();
        self.epoch += 1; // The solver permits one correction per action.
        for part in self.logs.chunks_mut(1024) {
            crate::interrupt::checkpoint()?;
            part.fill([f64::NAN; 2]);
        }
        self.last_age = None;
        self.exclude_measurement(started);
        Ok(())
    }

    pub(crate) fn observes(&self, age: usize) -> bool {
        !self.logs.is_empty() && progress::is_observation_age(age)
    }

    pub(crate) fn step(&mut self, live: usize) {
        let started = self.diagnostic.as_ref().map(|_| Instant::now());
        self.counts = self.counts.and_then(|counts| {
            Some(Counts {
                steps: counts.steps.checked_add(1)?,
                live: counts.live.checked_add(live as u128)?,
            })
        });
        if let (Some(trace), Some(started)) = (&mut self.diagnostic, started) {
            trace.exclude(started);
        }
    }

    /// Successful caller completion, including the zero-iteration certificate.
    /// A failed projection never calls this and cannot enter later history.
    pub(crate) fn complete(&mut self) {
        if let Some(trace) = &self.diagnostic {
            let stamp = trace.stamp();
            trace.finish(
                stamp,
                &format!(
                    "{}|steps={}|live={}",
                    self.action_label(),
                    label_integer(self.counts.map(|c| c.steps)),
                    label_integer(self.counts.map(|c| c.live))
                ),
            );
        }
        self.release_observations();
        if self.demand.role() == Some(2) {
            if self
                .history
                .is_none_or(|history| history.width != self.width)
            {
                self.history = Some(History {
                    width: self.width,
                    actions: 0,
                    original_width: 0,
                    counts: Counts::default(),
                });
            }
            self.history = self.history.and_then(|mut history| {
                history.add(self.counts?)?;
                Some(history)
            });
        }
    }

    fn future_groups(&self) -> std::result::Result<([FutureGroup; 3], usize, usize), Unknown> {
        let width_matches = match self.demand {
            Demand::Packed { regressors } => regressors.checked_add(1) == Some(self.width),
            Demand::Outcome { .. } | Demand::Final => self.width == 1,
            Demand::Regressors { start, end, .. } => end.checked_sub(start) == Some(self.width),
            Demand::OneOff => true,
        };
        if !width_matches || self.width == 0 || self.stride < self.width {
            return Err(Unknown::InvalidInput);
        }
        let (columns, block, final_call) = self.demand.future().ok_or(Unknown::InvalidInput)?;
        let blank = FutureGroup {
            width: 0,
            stride: self.stride,
            actions: 0,
            work: None,
        };
        let mut groups = [blank; 3];
        let mut len = 0;
        let mut history_mode = 0;
        for (width, actions, regressor) in [
            (block, columns / block, true),
            (columns % block, usize::from(columns % block != 0), true),
            (1, usize::from(final_call), false),
        ] {
            if actions == 0 {
                continue;
            }
            let work = if regressor {
                match self.history {
                    Some(history) if history.width == width => {
                        history_mode = history_mode.max(1);
                        Some(history.work()?)
                    }
                    Some(_) => {
                        history_mode = 2;
                        None
                    }
                    None => None,
                }
            } else {
                None
            };
            groups[len] = FutureGroup {
                width,
                stride: self.stride,
                actions: actions as u128,
                work,
            };
            len += 1;
        }
        Ok((groups, len, history_mode))
    }

    fn price(
        &self,
        structure: CheapStructure,
        workers: usize,
        work: Work,
        classes: u8,
        weighted: bool,
        scaled: bool,
    ) -> std::result::Result<Decision, Unknown> {
        let calibration = CALIBRATION.as_ref().ok_or(Unknown::InvalidInput)?;
        let role = self.demand.role().ok_or(Unknown::InvalidInput)?;
        let (future, len, history_mode) = self.future_groups()?;
        let base = (((role * 2 + usize::from(weighted)) * 2 + usize::from(scaled)) * 2
            + usize::from(self.epoch > 0))
            * 3;
        let required = 1 | if len == 0 { 0 } else { 1 << (history_mode + 1) };
        for class in 0..3 {
            if classes & (1 << class) != 0
                && calibration.support[base + class] & required != required
            {
                return Err(Unknown::InvalidInput);
            }
        }
        calibration.coefficients.evaluate(
            structure,
            Execution {
                r: self.width,
                s: self.stride,
                workers,
            },
            work,
            &future[..len],
        )
    }

    /// `proxy` reads the same group-moment expression as readiness, but scans
    /// every entry with bounded polling. It must not mutate scientific state.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn observe(
        &mut self,
        age: usize,
        active: &[f64],
        target: f64,
        remaining: usize,
        dimensions: [usize; 4],
        workers: usize,
        weighted: bool,
        scaled: bool,
        mut proxy: impl FnMut(usize) -> Result<f64>,
        fingerprint: impl FnOnce() -> Result<u64>,
    ) -> Result<bool> {
        if !progress::is_observation_age(age)
            || self.logs.len() != active.len()
            || self.logs.is_empty()
        {
            return Ok(false);
        }
        let measured = self.diagnostic.as_ref().map(|trace| {
            let started = Instant::now();
            (started, trace.stamp_at(started))
        });
        let contiguous = if age == 0 {
            self.last_age.is_none()
        } else {
            self.last_age == age.checked_sub(8)
        };
        let log_target = (target > 0.0 && target.is_finite()).then(|| target.ln());
        let mut forecast = Ok(Forecast::new());
        let mut live = false;
        let mut finite_activity = true;
        for (rhs, &activity) in active.iter().enumerate() {
            crate::interrupt::checkpoint()?;
            live |= activity != 0.0;
            finite_activity &= activity.is_finite();
            let value = if activity > 0.0 {
                proxy(rhs)?
            } else {
                f64::NAN
            };
            let current = (value > 0.0 && value.is_finite()).then(|| value.ln());
            let [previous, middle] = self.logs[rhs];
            if progress::is_decision_age(age) {
                forecast = forecast.and_then(|mut forecast| {
                    if !contiguous {
                        return Err(Unknown::InvalidInput);
                    }
                    let lane = forecast_lane(
                        LogWindow {
                            age,
                            previous: previous.is_finite().then_some(previous),
                            middle: middle.is_finite().then_some(middle),
                            current,
                        },
                        log_target.unwrap_or(f64::NAN),
                        activity,
                        remaining,
                    )?;
                    forecast.push(lane)?;
                    Ok(forecast)
                });
            }
            self.logs[rhs] = [middle, current.unwrap_or(f64::NAN)];
        }
        self.last_age = Some(age);
        if !progress::is_decision_age(age) {
            if let (Some(trace), Some((started, _))) = (&mut self.diagnostic, measured) {
                trace.exclude(started);
            }
            return Ok(false);
        }
        let forecast = forecast.and_then(Forecast::finish);
        let [n, p, q, d] = dimensions;
        let decision = forecast.and_then(|(work, classes)| {
            self.price(
                CheapStructure { n, p, q, d },
                workers,
                work,
                classes,
                weighted,
                scaled,
            )
        });
        let build = match self.mode {
            Mode::Predict => matches!(decision, Ok(Decision::Build(_))),
            Mode::Never => false,
            Mode::Force(checkpoint) => {
                age == checkpoint && remaining > 0 && live && finite_activity
            }
        };
        if self.trace {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=remaining_checkpoint|demand={:?}|epoch={}|local={age}|remaining={remaining}|r={}|stride={}|workers={workers}|forecast={forecast:?}|decision={decision:?}|build={}",
                self.demand, self.epoch, self.width, self.stride, u8::from(build)));
        }
        if let Some((_, stamp)) = measured {
            let fingerprint = if self.mode == Mode::Predict {
                None
            } else {
                Some(fingerprint()?)
            };
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_POLICY_LABEL|checkpoint|{}|epoch={}|local={age}|remaining={remaining}|n={n}|p={p}|q={q}|d={d}|workers={workers}|weighted={}|scaled={}|forecast_h={}|forecast_d={}|classes={}|history_width={}|history_actions={}|history_steps={}|history_live={}|history_original={}|stamp_ns={stamp}|state={}|build={}",
                self.action_label(), self.epoch, u8::from(weighted), u8::from(scaled),
                label_integer(forecast.ok().map(|(w, _)| w.h)),
                label_integer(forecast.ok().map(|(w, _)| w.d)),
                label_integer(forecast.ok().map(|(_, c)| u128::from(c))),
                label_integer(self.history.map(|h| h.width as u128)),
                label_integer(self.history.map(|h| h.actions)),
                label_integer(self.history.map(|h| h.counts.steps)),
                label_integer(self.history.map(|h| h.counts.live)),
                label_integer(self.history.map(|h| h.original_width)),
                label_integer(fingerprint.map(u128::from)), u8::from(build)));
        }
        if build {
            self.release_observations();
        }
        if let (Some(trace), Some((started, stamp))) = (&mut self.diagnostic, measured) {
            trace.exclude(started);
            if build {
                trace.grant(stamp);
            }
        }
        Ok(build)
    }

    pub(crate) fn built(&mut self, elapsed: Duration, installed: bool) {
        if let Some(trace) = &mut self.diagnostic {
            trace.built(elapsed, installed);
        }
    }

    fn action_label(&self) -> String {
        let (bits, final_residual) = self.accuracy.expect("action begins before measurement");
        format!(
            "{}|r={}|stride={}|tolerance_bits={bits}|final={}",
            self.demand.label(),
            self.width,
            self.stride,
            u8::from(final_residual)
        )
    }

    pub(crate) fn measurement_started(&self) -> Option<Instant> {
        self.diagnostic.as_ref().map(|_| Instant::now())
    }

    pub(crate) fn measurement_scope(&self) -> crate::diagnostics::Silence {
        crate::diagnostics::Silence::new(self.diagnostic.is_some())
    }

    pub(crate) fn trace_enabled(&self) -> bool {
        self.trace
    }

    pub(crate) fn exclude_measurement(&mut self, started: Option<Instant>) {
        if let (Some(trace), Some(started)) = (&mut self.diagnostic, started) {
            trace.exclude(started);
        }
    }

    pub(crate) fn startup_started(&self) -> Option<Instant> {
        self.diagnostic
            .as_ref()
            .filter(|trace| trace.awaiting_startup())
            .map(|_| Instant::now())
    }

    pub(crate) fn startup_finished(&mut self, started: Option<Instant>) {
        if let (Some(trace), Some(started)) = (&mut self.diagnostic, started) {
            trace.startup(started.elapsed());
        }
    }

    pub(crate) fn initialized(&mut self) {
        if let Some(trace) = &mut self.diagnostic {
            trace.initialized();
        }
    }
}

fn label_integer(value: Option<u128>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| value.to_string())
}
