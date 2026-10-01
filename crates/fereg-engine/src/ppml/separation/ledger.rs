use super::{
    cascade, invalid, verify, DirectionProposal, ExactProposal, Method, Methods, Model,
    NumericalCommit,
};
use crate::{
    data::{DropReason, Prepared, ReferenceWeights},
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
    spec::Estimator,
    threads::ThreadTeam,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub drop_singletons: bool,
    pub methods: Methods,
}
#[derive(Debug, Clone, Copy)]
pub struct View<'a> {
    pub(super) sample: &'a Prepared,
    pub(super) generation: usize,
}
impl<'a> View<'a> {
    #[must_use]
    pub fn sample(self) -> &'a Prepared {
        self.sample
    }
    #[must_use]
    pub fn generation(self) -> usize {
        self.generation
    }
}

/// Bounded diagnostics; original-axis reasons stay in the sole Prepared owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Commit {
    pub generation: usize,
    pub primary: usize,
    pub singletons: usize,
    pub exact_followup: usize,
    pub sample_digest: u64,
    pub trace_digest: u64,
}
#[derive(Debug)]
pub struct Ledger {
    sample: Prepared,
    pub(super) policy: Policy,
    generation: usize,
    sample_digest: u64,
    trace_digest: u64,
}
impl Ledger {
    pub fn new(sample: Prepared, policy: Policy) -> Result<Self> {
        Self::new_polled(sample, policy, None)
    }
    pub(crate) fn new_polled(sample: Prepared, policy: Policy, poll: Poll<'_>) -> Result<Self> {
        validate(&sample, poll)?;
        let sample_digest = digest(&sample, poll)?;
        execution::check(poll)?;
        Ok(Self {
            sample,
            policy,
            generation: 0,
            sample_digest,
            trace_digest: sample_digest,
        })
    }
    #[must_use]
    pub fn view(&self) -> View<'_> {
        View {
            sample: &self.sample,
            generation: self.generation,
        }
    }
    #[must_use]
    pub fn sample_digest(&self) -> u64 {
        self.sample_digest
    }
    #[must_use]
    pub fn trace_digest(&self) -> u64 {
        self.trace_digest
    }
    #[must_use]
    pub fn into_sample(self) -> Prepared {
        self.sample
    }

    /// Initial singleton pruning is its own generation, before detector replay.
    pub fn prune_initial_singletons(&mut self) -> Result<Option<Commit>> {
        self.prune_initial_singletons_polled(None)
    }
    pub(crate) fn prune_initial_singletons_polled(
        &mut self,
        poll: Poll<'_>,
    ) -> Result<Option<Commit>> {
        execution::check(poll)?;
        if !self.policy.drop_singletons {
            return Ok(None);
        }
        let (mut active, mut reasons) = masks(self.sample.n_active(), poll)?;
        cascade::singletons(&self.sample, &mut active, &mut reasons, poll)?;
        if !execution::any(&active, |&keep| !keep, poll)? {
            return Ok(None);
        }
        self.publish(&reasons, 0, 0, 1, poll).map(Some)
    }

    pub fn accept_exact(&mut self, proposal: &ExactProposal) -> Result<Option<Commit>> {
        self.accept_exact_polled(proposal, None)
    }
    pub(crate) fn accept_exact_polled(
        &mut self,
        proposal: &ExactProposal,
        poll: Poll<'_>,
    ) -> Result<Option<Commit>> {
        execution::check(poll)?;
        if !self.policy.methods.contains(Method::FixedEffects) {
            return Err(invalid("exact FE separation was not requested"));
        }
        if proposal.generation != self.generation {
            return Err(invalid("stale separation proposal generation"));
        }
        verify::proposal(self.view(), proposal, poll)?;
        self.require_singleton_fixed_point(poll)?;
        if proposal.rows.is_empty() {
            return Ok(None);
        }
        self.commit_rows(&proposal.rows, Method::FixedEffects, poll)
            .map(Some)
    }

    pub fn accept_direction(
        &mut self,
        proposal: &DirectionProposal,
        model: &Model<'_>,
        team: &ThreadTeam,
    ) -> Result<NumericalCommit> {
        let poll = model.probe.dense();
        execution::check(poll)?;
        if !self.policy.methods.contains(proposal.method) || proposal.method == Method::FixedEffects
        {
            return Err(invalid("numerical separation method was not requested"));
        }
        self.require_singleton_fixed_point(poll)?;
        if self.policy.methods.contains(Method::FixedEffects)
            && !self.view().exact_proposal_polled(poll)?.rows.is_empty()
        {
            return Err(invalid(
                "requested exact FE fixed point must precede numerical separation",
            ));
        }
        let validation = self.view().certify_direction(proposal, model, team)?;
        let commit = self.commit_rows(&proposal.rows, proposal.method, poll)?;
        Ok(NumericalCommit { commit, validation })
    }

    fn require_singleton_fixed_point(&self, poll: Poll<'_>) -> Result<()> {
        if self.policy.drop_singletons {
            let (mut active, mut reasons) = masks(self.sample.n_active(), poll)?;
            cascade::singletons(&self.sample, &mut active, &mut reasons, poll)?;
            if execution::any(&active, |&keep| !keep, poll)? {
                return Err(invalid(
                    "initial singleton generation must precede separation detectors",
                ));
            }
        }
        Ok(())
    }

    fn commit_rows(&mut self, rows: &[usize], method: Method, poll: Poll<'_>) -> Result<Commit> {
        let reason = match method {
            Method::FixedEffects => DropReason::FixedEffectSeparation,
            Method::Simplex => DropReason::SimplexSeparation,
            Method::IteratedRectifier => DropReason::IteratedRectifierSeparation,
            Method::Mu => DropReason::MuSeparation,
        };
        let (mut active, mut reasons) = masks(self.sample.n_active(), poll)?;
        for (index, &original) in rows.iter().enumerate() {
            execution::check_chunk(poll, index)?;
            let row = self
                .sample
                .retained_rows()
                .binary_search(&original)
                .map_err(|_| invalid("separation row is absent from this generation"))?;
            active[row] = false;
            reasons[row] = reason;
        }
        cascade::complete(
            &self.sample,
            &mut active,
            &mut reasons,
            self.policy.drop_singletons,
            self.policy.methods.contains(Method::FixedEffects),
            poll,
        )?;
        let total_exact = execution::fold(&reasons, 0, poll, |n, &r| {
            n + usize::from(r == DropReason::FixedEffectSeparation)
        })?;
        self.publish(
            &reasons,
            rows.len(),
            total_exact
                - if method == Method::FixedEffects {
                    rows.len()
                } else {
                    0
                },
            2 + method as u64,
            poll,
        )
    }

    fn publish(
        &mut self,
        reasons: &[DropReason],
        primary: usize,
        exact_followup: usize,
        method: u64,
        poll: Poll<'_>,
    ) -> Result<Commit> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| invalid("separation generation overflows"))?;
        let staged = self.sample.stage_deletions_polled(reasons, poll)?;
        validate(&staged, poll)?;
        let singletons = execution::fold(reasons, 0, poll, |n, &r| {
            n + usize::from(r == DropReason::Singleton)
        })?;
        let sample_digest = digest(&staged, poll)?;
        let mut trace_digest = self.trace_digest;
        for value in [
            generation as u64,
            method,
            primary as u64,
            singletons as u64,
            exact_followup as u64,
            sample_digest,
        ] {
            trace_digest = mix(trace_digest, value);
        }
        // All fallible work precedes replacing the accepted owner and diagnostics.
        execution::check(poll)?;
        self.sample = staged;
        self.generation = generation;
        self.sample_digest = sample_digest;
        self.trace_digest = trace_digest;
        Ok(Commit {
            generation,
            primary,
            singletons,
            exact_followup,
            sample_digest,
            trace_digest,
        })
    }
}

fn masks(n: usize, poll: Poll<'_>) -> Result<(Vec<bool>, Vec<DropReason>)> {
    execution::check(poll)?;
    let mut active = reserved(n)?;
    active.resize(n, true);
    let mut reasons = reserved(n)?;
    reasons.resize(n, DropReason::Retained);
    execution::check(poll)?;
    Ok((active, reasons))
}
fn validate(sample: &Prepared, poll: Poll<'_>) -> Result<()> {
    sample.validate_alignment_polled(poll)?;
    if let Some(topology) = &sample.topology {
        topology.validate_polled(sample, poll)?;
    }
    if execution::any(&sample.outcome, |y| !y.is_finite() || *y < 0., poll)? {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "separation requires finite nonnegative outcomes",
        ));
    }
    if !execution::any(&sample.outcome, |y| *y > 0., poll)? {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "separation leaves no positive outcome",
        ));
    }
    ReferenceWeights::new_polled(&sample.weights, sample.n_active(), Estimator::Ppml, poll)?;
    execution::check(poll)
}
fn digest(sample: &Prepared, poll: Poll<'_>) -> Result<u64> {
    let mut hash = 0xcbf29ce484222325;
    for (row, reason) in sample.drop_reasons.iter().enumerate() {
        execution::check_chunk(poll, row)?;
        hash = mix(mix(hash, row as u64), u64::from(reason.code()));
    }
    Ok(hash)
}
pub(super) fn mix(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(0x100000001b3)
}
