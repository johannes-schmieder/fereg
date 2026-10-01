//! One stable compact axis, including later singleton and separation deletions.
use super::sample::{DropReason, Prepared, PreparedAdjustment, PreparedWeights};
use crate::{
    error::{ErrorCode, FeregError, Result},
    execution::{self, Poll},
};

impl Prepared {
    #[must_use]
    pub fn n_active(&self) -> usize {
        self.original_row_by_active.len()
    }

    #[must_use]
    pub fn n_original(&self) -> usize {
        self.n_original
    }

    #[must_use]
    pub fn is_retained(&self, row: usize) -> bool {
        self.drop_reason(row) == Some(DropReason::Retained)
    }

    #[must_use]
    pub fn drop_reason(&self, row: usize) -> Option<DropReason> {
        self.drop_reasons.get(row).copied()
    }

    #[must_use]
    pub fn retained_rows(&self) -> &[usize] {
        &self.original_row_by_active
    }

    /// Consume the owner so a failed validation cannot expose a partially compacted sample.
    pub fn compact_active_rows(self, keep: &[bool], reason: DropReason) -> Result<Self> {
        self.compact_active_rows_polled(keep, reason, None)
    }
    pub(crate) fn compact_active_rows_polled(
        mut self,
        keep: &[bool],
        reason: DropReason,
        poll: Poll<'_>,
    ) -> Result<Self> {
        execution::check(poll)?;
        if matches!(reason, DropReason::Retained | DropReason::ExcludedByIfIn)
            || keep.len() != self.n_active()
        {
            return Err(invalid("invalid compact-row mask or deletion reason"));
        }
        self.validate_alignment_polled(poll)?;
        self.topology = None;
        for (position, (&row, &retained)) in
            self.original_row_by_active.iter().zip(keep).enumerate()
        {
            execution::check_chunk(poll, position)?;
            if !retained {
                self.drop_reasons[row] = reason;
            }
        }
        retain_rows_polled(&mut self.original_row_by_active, keep, poll)?;
        retain_rows_polled(&mut self.outcome, keep, poll)?;
        for (index, column) in self.regressors.iter_mut().enumerate() {
            execution::check_chunk(poll, index)?;
            retain_rows_polled(column, keep, poll)?;
        }
        if let PreparedWeights::Provided { values, .. } = &mut self.weights {
            retain_rows_polled(values, keep, poll)?;
        }
        match &mut self.adjustment {
            PreparedAdjustment::Offset(values) | PreparedAdjustment::Exposure(values) => {
                retain_rows_polled(values, keep, poll)?
            }
            PreparedAdjustment::None => (),
        }
        execution::check(poll)?;
        Ok(self)
    }

    pub fn validate_alignment(&self) -> Result<()> {
        self.validate_alignment_polled(None)
    }
    pub(crate) fn validate_alignment_polled(&self, poll: Poll<'_>) -> Result<()> {
        execution::check(poll)?;
        let n = self.n_active();
        if self
            .topology
            .as_ref()
            .is_some_and(|topology| topology.nobs() != n)
            || self.outcome.len() != n
            || execution::any(&self.regressors, |column| column.len() != n, poll)?
            || matches!(&self.weights, PreparedWeights::Provided {values,..} if values.len()!=n)
            || matches!(&self.adjustment, PreparedAdjustment::Offset(values) | PreparedAdjustment::Exposure(values) if values.len()!=n)
            || self.n_candidate > self.n_original
            || n > self.n_candidate
            || execution::fold(&self.drop_reasons, 0usize, poll, |count, &reason| {
                count + usize::from(reason == DropReason::Retained)
            })? != n
            || execution::fold(&self.drop_reasons, 0usize, poll, |count, &reason| {
                count + usize::from(reason != DropReason::ExcludedByIfIn)
            })? != self.n_candidate
            || self.drop_reasons.len() != self.n_original
            || execution::any(
                self.original_row_by_active.iter().enumerate(),
                |(index, &row)| {
                    row >= self.n_original
                        || self.drop_reasons[row] != DropReason::Retained
                        || (index > 0 && self.original_row_by_active[index - 1] >= row)
                },
                poll,
            )?
        {
            return Err(invalid(
                "prepared data are not aligned on one observation axis",
            ));
        }
        execution::check(poll)
    }
}

// Every row bank is Copy. A stable in-place prefix needs no second bank or
// cancellation unwind; callers discard their owned work on any failure.
pub(crate) fn retain_rows_polled<T: Copy>(
    values: &mut Vec<T>,
    keep: &[bool],
    poll: Poll<'_>,
) -> Result<()> {
    execution::check(poll)?;
    let mut written = 0;
    for row in 0..values.len() {
        execution::check_chunk(poll, row)?;
        if keep[row] {
            if written != row {
                values[written] = values[row];
            }
            written += 1;
        }
    }
    values.truncate(written);
    execution::check(poll)
}

fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
