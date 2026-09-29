use super::{cascade, invalid, verify, ExactProposal};
use crate::{
    data::{DropReason, Prepared, ReferenceWeights},
    error::{reserved, ErrorCode, FeregError, Result},
    spec::Estimator,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub drop_singletons: bool,
    pub exact_fe: bool,
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
    policy: Policy,
    generation: usize,
    sample_digest: u64,
    trace_digest: u64,
}
impl Ledger {
    pub fn new(sample: Prepared, policy: Policy) -> Result<Self> {
        validate(&sample)?;
        let sample_digest = digest(&sample);
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
        if !self.policy.drop_singletons {
            return Ok(None);
        }
        let (mut active, mut reasons) = masks(self.sample.n_active())?;
        cascade::singletons(&self.sample, &mut active, &mut reasons)?;
        if active.iter().all(|&keep| keep) {
            return Ok(None);
        }
        self.publish(&reasons, 0, 0, 1).map(Some)
    }

    pub fn accept_exact(&mut self, proposal: &ExactProposal) -> Result<Option<Commit>> {
        if !self.policy.exact_fe {
            return Err(invalid("exact FE separation was not requested"));
        }
        if proposal.generation != self.generation {
            return Err(invalid("stale separation proposal generation"));
        }
        verify::proposal(self.view(), proposal)?;
        if self.policy.drop_singletons {
            let (mut active, mut reasons) = masks(self.sample.n_active())?;
            cascade::singletons(&self.sample, &mut active, &mut reasons)?;
            if active.iter().any(|&keep| !keep) {
                return Err(invalid(
                    "initial singleton generation must precede exact FE separation",
                ));
            }
        }
        if proposal.rows.is_empty() {
            return Ok(None);
        }
        let (mut active, mut reasons) = masks(self.sample.n_active())?;
        for &original in &proposal.rows {
            let row = self
                .sample
                .retained_rows()
                .binary_search(&original)
                .map_err(|_| invalid("separation row is absent from this generation"))?;
            active[row] = false;
            reasons[row] = DropReason::FixedEffectSeparation;
        }
        cascade::complete(
            &self.sample,
            &mut active,
            &mut reasons,
            self.policy.drop_singletons,
            self.policy.exact_fe,
        )?;
        let total_exact = reasons
            .iter()
            .filter(|&&r| r == DropReason::FixedEffectSeparation)
            .count();
        self.publish(
            &reasons,
            proposal.rows.len(),
            total_exact - proposal.rows.len(),
            2,
        )
        .map(Some)
    }

    fn publish(
        &mut self,
        reasons: &[DropReason],
        primary: usize,
        exact_followup: usize,
        method: u64,
    ) -> Result<Commit> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| invalid("separation generation overflows"))?;
        let staged = self.sample.stage_deletions(reasons)?;
        validate(&staged)?;
        let singletons = reasons
            .iter()
            .filter(|&&r| r == DropReason::Singleton)
            .count();
        let sample_digest = digest(&staged);
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

fn masks(n: usize) -> Result<(Vec<bool>, Vec<DropReason>)> {
    let mut active = reserved(n)?;
    active.resize(n, true);
    let mut reasons = reserved(n)?;
    reasons.resize(n, DropReason::Retained);
    Ok((active, reasons))
}
fn validate(sample: &Prepared) -> Result<()> {
    sample.validate_alignment()?;
    if let Some(topology) = &sample.topology {
        topology.validate(sample)?;
    }
    if sample.outcome.iter().any(|y| !y.is_finite() || *y < 0.) {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "separation requires finite nonnegative outcomes",
        ));
    }
    if !sample.outcome.iter().any(|y| *y > 0.) {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "separation leaves no positive outcome",
        ));
    }
    ReferenceWeights::new(&sample.weights, sample.n_active(), Estimator::Ppml)?;
    Ok(())
}
fn digest(sample: &Prepared) -> u64 {
    let mut hash = 0xcbf29ce484222325;
    for (row, reason) in sample.drop_reasons.iter().enumerate() {
        hash = mix(mix(hash, row as u64), u64::from(reason.code()));
    }
    hash
}
fn mix(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(0x100000001b3)
}
