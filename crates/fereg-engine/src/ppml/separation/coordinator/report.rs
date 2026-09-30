//! Public diagnostics stay bounded even when every deletion restarts the chain.
use crate::{
    error::{ErrorCode, FeregError, Result},
    ppml::separation::{ledger::mix, Commit, Ledger, Method, Methods, Policy, Status},
    wls::Work,
};
pub const TRACE_LIMIT: usize = 64;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Detector,
    Replay,
    Deletion,
    InitialSingletons,
    CapacityResolved,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    pub generation: usize,
    pub method: Option<Method>,
    pub status: Option<Status>,
    pub iterations: usize,
    pub primary: usize,
    pub singletons: usize,
    pub exact_followup: usize,
    pub sample_digest: u64,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MethodSummary {
    pub requested: bool,
    pub calls: usize,
    pub replays: usize,
    pub iterations: usize,
    pub replay_iterations: usize,
    pub primary: usize,
    pub final_status: Option<Status>,
    pub last_generation: Option<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub requested: Methods,
    pub methods: [MethodSummary; 4],
    pub rounds: usize,
    pub generation: usize,
    pub singletons: usize,
    pub exact_followup: usize,
    pub capacity_resolutions: usize,
    resolved_generation: Option<usize>,
    pub sample_digest: u64,
    pub ledger_digest: u64,
    pub trace_digest: u64,
    pub trace: [Option<Event>; TRACE_LIMIT],
    pub events: usize,
    pub work: Work,
}
impl Report {
    pub(super) fn new(policy: Policy, ledger: &Ledger) -> Self {
        Self {
            requested: policy.methods,
            methods: [
                Method::FixedEffects,
                Method::Simplex,
                Method::IteratedRectifier,
                Method::Mu,
            ]
            .map(|m| MethodSummary {
                requested: policy.methods.contains(m),
                ..MethodSummary::default()
            }),
            rounds: 0,
            generation: 0,
            singletons: 0,
            exact_followup: 0,
            capacity_resolutions: 0,
            resolved_generation: None,
            sample_digest: ledger.sample_digest(),
            ledger_digest: ledger.trace_digest(),
            trace_digest: mix(ledger.trace_digest(), u64::from(policy.drop_singletons)),
            trace: [None; TRACE_LIMIT],
            events: 0,
            work: Work::default(),
        }
    }
    pub(super) fn begin_round(&mut self, generation: usize) -> Result<()> {
        add(&mut self.rounds, 1)?;
        self.generation = generation;
        self.resolved_generation = None;
        for m in &mut self.methods {
            m.final_status = None;
        }
        Ok(())
    }
    pub(super) fn detector(
        &mut self,
        method: Method,
        status: Status,
        iterations: usize,
        work: Work,
    ) -> Result<()> {
        let row = &mut self.methods[method as usize];
        if !row.requested {
            return Err(internal("unrequested separation detector executed"));
        }
        add(&mut row.calls, 1)?;
        add(&mut row.iterations, iterations)?;
        row.last_generation = Some(self.generation);
        self.work.merge(work)?;
        self.event(EventKind::Detector, Some(method), Some(status), iterations)
    }
    pub(super) fn replay(
        &mut self,
        method: Method,
        status: Status,
        iterations: usize,
        work: Work,
    ) -> Result<()> {
        let row = &mut self.methods[method as usize];
        add(&mut row.replays, 1)?;
        add(&mut row.replay_iterations, iterations)?;
        self.work.merge(work)?;
        self.event(EventKind::Replay, Some(method), Some(status), iterations)
    }
    pub(super) fn terminal(&mut self, method: Method, status: Status) -> Result<()> {
        if !matches!(
            status,
            Status::NoCandidates | Status::NoSeparation | Status::SkippedCapacity
        ) {
            return Err(internal(
                "a deletion proposal cannot seal a separation terminal",
            ));
        }
        self.methods[method as usize].final_status = Some(status);
        Ok(())
    }
    pub(super) fn capacity_resolution(&mut self) -> Result<()> {
        add(&mut self.capacity_resolutions, 1)?;
        self.resolved_generation = Some(self.generation);
        self.event(
            EventKind::CapacityResolved,
            Some(Method::Simplex),
            Some(Status::SkippedCapacity),
            0,
        )
    }
    pub(super) fn commit(&mut self, method: Option<Method>, commit: Commit) -> Result<()> {
        if let Some(m) = method {
            add(&mut self.methods[m as usize].primary, commit.primary)?;
        }
        add(&mut self.singletons, commit.singletons)?;
        add(&mut self.exact_followup, commit.exact_followup)?;
        self.sample_digest = commit.sample_digest;
        self.ledger_digest = commit.trace_digest;
        self.push(Event {
            kind: if method.is_some() {
                EventKind::Deletion
            } else {
                EventKind::InitialSingletons
            },
            generation: commit.generation,
            method,
            status: Some(Status::Certified),
            iterations: 0,
            primary: commit.primary,
            singletons: commit.singletons,
            exact_followup: commit.exact_followup,
            sample_digest: commit.sample_digest,
        })
    }
    pub(super) fn finish(&mut self, ledger: &Ledger) -> Result<()> {
        if self.generation != ledger.view().generation()
            || self.sample_digest != ledger.sample_digest()
            || self.ledger_digest != ledger.trace_digest()
            || (self.methods[Method::Simplex as usize].final_status
                == Some(Status::SkippedCapacity)
                && self.resolved_generation != Some(self.generation))
            || self.methods.iter().any(|m| {
                m.requested
                    && (m.final_status.is_none() || m.last_generation != Some(self.generation))
            })
        {
            return Err(internal(
                "requested separation checks did not complete on the final generation",
            ));
        }
        Ok(())
    }
    fn event(
        &mut self,
        kind: EventKind,
        method: Option<Method>,
        status: Option<Status>,
        iterations: usize,
    ) -> Result<()> {
        self.push(Event {
            kind,
            generation: self.generation,
            method,
            status,
            iterations,
            primary: 0,
            singletons: 0,
            exact_followup: 0,
            sample_digest: self.sample_digest,
        })
    }
    fn push(&mut self, event: Event) -> Result<()> {
        if self.events < TRACE_LIMIT {
            self.trace[self.events] = Some(event);
        }
        add(&mut self.events, 1)?;
        for value in [
            event.kind as u64,
            event.generation as u64,
            event.method.map_or(0, |m| m as u64 + 1),
            event.status.map_or(0, |s| s as u64 + 1),
            event.iterations as u64,
            event.primary as u64,
            event.singletons as u64,
            event.exact_followup as u64,
            event.sample_digest,
        ] {
            self.trace_digest = mix(self.trace_digest, value);
        }
        Ok(())
    }
}
fn add(value: &mut usize, increment: usize) -> Result<()> {
    *value = value
        .checked_add(increment)
        .ok_or_else(|| internal("separation diagnostic count overflowed"))?;
    Ok(())
}
fn internal(message: &str) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}
