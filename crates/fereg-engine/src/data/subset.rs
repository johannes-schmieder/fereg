//! Stage one replacement owner without changing the accepted generation.
use super::{DropReason, Prepared, PreparedAdjustment, PreparedWeights};
use crate::{
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
};

impl Prepared {
    pub(crate) fn stage_deletions_polled(
        &self,
        reasons: &[DropReason],
        poll: Poll<'_>,
    ) -> Result<Self> {
        self.validate_alignment_polled(poll)?;
        if reasons.len() != self.n_active()
            || execution::any(
                reasons,
                |reason| {
                    !matches!(
                        reason,
                        DropReason::Retained
                            | DropReason::Singleton
                            | DropReason::FixedEffectSeparation
                            | DropReason::SimplexSeparation
                            | DropReason::IteratedRectifierSeparation
                            | DropReason::MuSeparation
                    )
                },
                poll,
            )?
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "invalid staged sample deletion reasons",
            ));
        }
        let n = execution::fold(reasons, 0, poll, |n, &r| {
            n + usize::from(r == DropReason::Retained)
        })?;
        let mut keep = reserved(reasons.len())?;
        for (row, &reason) in reasons.iter().enumerate() {
            execution::check_chunk(poll, row)?;
            keep.push(reason == DropReason::Retained);
        }
        let mut drop_reasons = reserved(self.n_original)?;
        for (row, &reason) in self.drop_reasons.iter().enumerate() {
            execution::check_chunk(poll, row)?;
            drop_reasons.push(reason);
        }
        for (index, (&row, &reason)) in self.retained_rows().iter().zip(reasons).enumerate() {
            execution::check_chunk(poll, index)?;
            drop_reasons[row] = reason;
        }
        let mut regressors = reserved(self.regressors.len())?;
        for (index, column) in self.regressors.iter().enumerate() {
            execution::check_chunk(poll, index)?;
            regressors.push(subset(column, &keep, n, poll)?);
        }
        let weights = match &self.weights {
            PreparedWeights::None => PreparedWeights::None,
            PreparedWeights::Provided { kind, values } => PreparedWeights::Provided {
                kind: *kind,
                values: subset(values, &keep, n, poll)?,
            },
        };
        let adjustment = match &self.adjustment {
            PreparedAdjustment::None => PreparedAdjustment::None,
            PreparedAdjustment::Offset(values) => {
                PreparedAdjustment::Offset(subset(values, &keep, n, poll)?)
            }
            PreparedAdjustment::Exposure(values) => {
                PreparedAdjustment::Exposure(subset(values, &keep, n, poll)?)
            }
        };
        let staged = Self {
            topology: self
                .topology
                .as_ref()
                .map(|t| t.subset_polled(&keep, poll))
                .transpose()?,
            n_original: self.n_original,
            n_candidate: self.n_candidate,
            original_row_by_active: subset(self.retained_rows(), &keep, n, poll)?,
            outcome: subset(&self.outcome, &keep, n, poll)?,
            regressors,
            weights,
            adjustment,
            drop_reasons,
        };
        staged.validate_alignment_polled(poll)?;
        if let Some(topology) = &staged.topology {
            topology.validate_polled(&staged, poll)?;
        }
        execution::check(poll)?;
        Ok(staged)
    }
}

fn subset<T: Copy>(source: &[T], keep: &[bool], n: usize, poll: Poll<'_>) -> Result<Vec<T>> {
    let mut result = reserved(n)?;
    for (row, (&value, &retain)) in source.iter().zip(keep).enumerate() {
        execution::check_chunk(poll, row)?;
        if retain {
            result.push(value);
        }
    }
    Ok(result)
}
