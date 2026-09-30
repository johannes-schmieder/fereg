//! Bounded FE work accounting, including speculative and unsuccessful frames.
use crate::{
    error::{ErrorCode, FeregError, Result},
    fe::{Attempt, Backend, FallbackReason, RhsDiagnostics, SetupDiagnostics, StopReason},
    spec::Solver,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub right_hand_sides: usize,
    pub summed_iterations: usize,
    pub maximum_rhs_iterations: usize,
    pub attempts: usize,
    pub refinements: usize,
}
impl Counts {
    fn merge(&mut self, other: Self) -> Result<()> {
        add(&mut self.right_hand_sides, other.right_hand_sides)?;
        add(&mut self.summed_iterations, other.summed_iterations)?;
        self.maximum_rhs_iterations = self
            .maximum_rhs_iterations
            .max(other.maximum_rhs_iterations);
        add(&mut self.attempts, other.attempts)?;
        add(&mut self.refinements, other.refinements)
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AttemptWork {
    counts: Counts,
    stops: [usize; 8],
}
impl AttemptWork {
    #[must_use]
    pub const fn counts(&self) -> Counts {
        self.counts
    }
    #[must_use]
    pub fn stopped(&self, reason: StopReason) -> usize {
        self.stops[stop(reason)]
    }
    fn observe(&mut self, attempt: &Attempt) -> Result<()> {
        self.counts.merge(Counts {
            right_hand_sides: 1,
            summed_iterations: attempt.iterations,
            maximum_rhs_iterations: attempt.iterations,
            attempts: 1,
            refinements: attempt.refinements,
        })?;
        add(&mut self.stops[stop(attempt.stop)], 1)
    }
    fn merge(&mut self, other: Self) -> Result<()> {
        self.counts.merge(other.counts)?;
        merge_counts(&mut self.stops, other.stops)
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Solves {
    counts: Counts,
    backends: [AttemptWork; 5],
}
impl Solves {
    fn observe(&mut self, iterations: usize, attempts: &[Attempt]) -> Result<()> {
        let mut observed = Counts {
            right_hand_sides: 1,
            maximum_rhs_iterations: iterations,
            ..Counts::default()
        };
        for attempt in attempts {
            add(&mut observed.summed_iterations, attempt.iterations)?;
            add(&mut observed.attempts, 1)?;
            add(&mut observed.refinements, attempt.refinements)?;
            self.backends[backend(attempt.backend)].observe(attempt)?;
        }
        if iterations != observed.summed_iterations {
            return Err(invalid("FE RHS and attempt iteration counts disagree"));
        }
        self.counts.merge(observed)
    }
    fn merge(&mut self, other: Self) -> Result<()> {
        self.counts.merge(other.counts)?;
        for (ours, theirs) in self.backends.iter_mut().zip(other.backends) {
            ours.merge(theirs)?;
        }
        Ok(())
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionWork {
    calls: usize,
    solves: Solves,
    requested: [usize; 3],
    selected: [usize; 5],
    used: [usize; 5],
    checkpoints: [usize; 5],
    stops: [usize; 8],
    fallbacks: [usize; 4],
}
impl ProjectionWork {
    #[must_use]
    pub const fn calls(&self) -> usize {
        self.calls
    }
    #[must_use]
    pub const fn counts(&self) -> Counts {
        self.solves.counts
    }
    #[must_use]
    pub fn backend(&self, value: Backend) -> &AttemptWork {
        &self.solves.backends[backend(value)]
    }
    #[must_use]
    pub fn requested(&self, value: Solver) -> usize {
        self.requested[solver(value)]
    }
    #[must_use]
    pub fn selected(&self, value: Backend) -> usize {
        self.selected[backend(value)]
    }
    #[must_use]
    pub fn used(&self, value: Backend) -> usize {
        self.used[backend(value)]
    }
    #[must_use]
    pub fn checkpoint(&self, value: Backend) -> usize {
        self.checkpoints[backend(value)]
    }
    #[must_use]
    pub fn stopped(&self, reason: StopReason) -> usize {
        self.stops[stop(reason)]
    }
    #[must_use]
    pub fn fallback(&self, reason: FallbackReason) -> usize {
        self.fallbacks[fallback(reason)]
    }
    fn observe(&mut self, rows: &[RhsDiagnostics]) -> Result<()> {
        add(&mut self.calls, 1)?;
        for row in rows {
            self.solves.observe(row.iterations, &row.attempts)?;
            add(&mut self.requested[solver(row.requested)], 1)?;
            add(&mut self.selected[backend(row.selected)], 1)?;
            add(&mut self.used[backend(row.used)], 1)?;
            add(&mut self.checkpoints[backend(row.checkpoint_backend)], 1)?;
            add(&mut self.stops[stop(row.stop)], 1)?;
            if let Some(reason) = row.fallback {
                add(&mut self.fallbacks[fallback(reason)], 1)?;
            }
        }
        Ok(())
    }
    fn merge(&mut self, other: Self) -> Result<()> {
        add(&mut self.calls, other.calls)?;
        self.solves.merge(other.solves)?;
        merge_counts(&mut self.requested, other.requested)?;
        merge_counts(&mut self.selected, other.selected)?;
        merge_counts(&mut self.used, other.used)?;
        merge_counts(&mut self.checkpoints, other.checkpoints)?;
        merge_counts(&mut self.stops, other.stops)?;
        merge_counts(&mut self.fallbacks, other.fallbacks)
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SetupWork {
    calls: usize,
    solves: Solves,
    stops: [usize; 8],
    backends: [usize; 5],
}
impl SetupWork {
    #[must_use]
    pub const fn calls(&self) -> usize {
        self.calls
    }
    #[must_use]
    pub const fn counts(&self) -> Counts {
        self.solves.counts
    }
    #[must_use]
    pub fn backend(&self, value: Backend) -> &AttemptWork {
        &self.solves.backends[backend(value)]
    }
    #[must_use]
    pub fn backend_calls(&self, value: Backend) -> usize {
        self.backends[backend(value)]
    }
    #[must_use]
    pub fn stopped(&self, reason: StopReason) -> usize {
        self.stops[stop(reason)]
    }
    fn observe(&mut self, rows: &[SetupDiagnostics]) -> Result<()> {
        for row in rows {
            add(&mut self.calls, 1)?;
            add(&mut self.stops[stop(row.stop)], 1)?;
            add(&mut self.backends[backend(row.backend)], 1)?;
            for attempt in &row.right_hand_sides {
                self.solves
                    .observe(attempt.iterations, std::slice::from_ref(attempt))?;
            }
        }
        Ok(())
    }
    fn merge(&mut self, other: Self) -> Result<()> {
        add(&mut self.calls, other.calls)?;
        self.solves.merge(other.solves)?;
        merge_counts(&mut self.backends, other.backends)?;
        merge_counts(&mut self.stops, other.stops)
    }
}
/// One fixed-size summary; no history grows with RHS count or outer iterations.
/// Detailed final-frame reports remain on AbsorbedFit and the FE engine.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Work {
    projection: ProjectionWork,
    recovery: ProjectionWork,
    setup: SetupWork,
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum Phase {
    Projection,
    Recovery,
}
impl Work {
    #[must_use]
    pub const fn projection(&self) -> &ProjectionWork {
        &self.projection
    }
    #[must_use]
    pub const fn recovery(&self) -> &ProjectionWork {
        &self.recovery
    }
    #[must_use]
    pub const fn setup(&self) -> &SetupWork {
        &self.setup
    }
    #[must_use]
    pub fn counts(&self) -> Counts {
        self.checked_counts().expect("validated FE work counts")
    }
    /// Merging is atomic even on overflow; counters never saturate silently.
    pub fn merge(&mut self, other: Self) -> Result<()> {
        let mut next = *self;
        next.projection.merge(other.projection)?;
        next.recovery.merge(other.recovery)?;
        next.setup.merge(other.setup)?;
        next.checked_counts()?;
        *self = next;
        Ok(())
    }
    pub(super) fn observe(
        &mut self,
        phase: Phase,
        rows: &[RhsDiagnostics],
        setup: &[SetupDiagnostics],
    ) -> Result<()> {
        let mut next = *self;
        match phase {
            Phase::Projection => next.projection.observe(rows)?,
            Phase::Recovery => next.recovery.observe(rows)?,
        }
        next.setup.observe(setup)?;
        next.checked_counts()?;
        *self = next;
        Ok(())
    }
    fn checked_counts(&self) -> Result<Counts> {
        let mut total = self.projection.counts();
        total.merge(self.recovery.counts())?;
        total.merge(self.setup.counts())?;
        Ok(total)
    }
}
fn merge_counts<const N: usize>(ours: &mut [usize; N], theirs: [usize; N]) -> Result<()> {
    for (ours, theirs) in ours.iter_mut().zip(theirs) {
        add(ours, theirs)?;
    }
    Ok(())
}
fn add(ours: &mut usize, value: usize) -> Result<()> {
    *ours = ours
        .checked_add(value)
        .ok_or_else(|| invalid("FE work count overflowed"))?;
    Ok(())
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}
fn solver(value: Solver) -> usize {
    match value {
        Solver::Auto => 0,
        Solver::Lsmr => 1,
        Solver::Cmg => 2,
    }
}
fn backend(value: Backend) -> usize {
    match value {
        Backend::NoEffects => 0,
        Backend::Direct => 1,
        Backend::SchurPcg => 2,
        Backend::Lsmr => 3,
        Backend::Cmg => 4,
    }
}
fn stop(value: StopReason) -> usize {
    match value {
        StopReason::NoEffects => 0,
        StopReason::Direct => 1,
        StopReason::Converged => 2,
        StopReason::Stagnation => 3,
        StopReason::ProjectedExhaustion => 4,
        StopReason::BudgetExhausted => 5,
        StopReason::NumericalBreakdown => 6,
        StopReason::CertificateFailure => 7,
    }
}
fn fallback(value: FallbackReason) -> usize {
    match value {
        FallbackReason::MeasuredStagnation => 0,
        FallbackReason::ProjectedExhaustion => 1,
        FallbackReason::NumericalNonConvergence => 2,
        FallbackReason::CertificateFailure => 3,
    }
}
