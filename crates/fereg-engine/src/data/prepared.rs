//! One stable compact axis, including later singleton and separation deletions.
use super::sample::{DropReason, Prepared, PreparedAdjustment, PreparedWeights};
use crate::error::{ErrorCode, FeregError, Result};

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
    pub fn compact_active_rows(mut self, keep: &[bool], reason: DropReason) -> Result<Self> {
        if matches!(reason, DropReason::Retained | DropReason::ExcludedByIfIn)
            || keep.len() != self.n_active()
        {
            return Err(invalid("invalid compact-row mask or deletion reason"));
        }
        self.validate_alignment()?;
        self.topology = None;
        for (&row, &retained) in self.original_row_by_active.iter().zip(keep) {
            if !retained {
                self.drop_reasons[row] = reason;
            }
        }
        retain_rows(&mut self.original_row_by_active, keep);
        retain_rows(&mut self.outcome, keep);
        for column in &mut self.regressors {
            retain_rows(column, keep);
        }
        if let PreparedWeights::Provided { values, .. } = &mut self.weights {
            retain_rows(values, keep);
        }
        match &mut self.adjustment {
            PreparedAdjustment::Offset(values) | PreparedAdjustment::Exposure(values) => {
                retain_rows(values, keep)
            }
            PreparedAdjustment::None => (),
        }
        Ok(self)
    }

    pub fn validate_alignment(&self) -> Result<()> {
        let n = self.n_active();
        if self
            .topology
            .as_ref()
            .is_some_and(|topology| topology.nobs() != n)
            || self.outcome.len() != n
            || self.regressors.iter().any(|column| column.len() != n)
            || matches!(&self.weights, PreparedWeights::Provided {values,..} if values.len()!=n)
            || matches!(&self.adjustment, PreparedAdjustment::Offset(values) | PreparedAdjustment::Exposure(values) if values.len()!=n)
            || self.n_candidate > self.n_original
            || n > self.n_candidate
            || self
                .drop_reasons
                .iter()
                .filter(|&&reason| reason == DropReason::Retained)
                .count()
                != n
            || self
                .drop_reasons
                .iter()
                .filter(|&&reason| reason != DropReason::ExcludedByIfIn)
                .count()
                != self.n_candidate
            || self.drop_reasons.len() != self.n_original
            || self
                .original_row_by_active
                .iter()
                .enumerate()
                .any(|(index, &row)| {
                    row >= self.n_original
                        || self.drop_reasons[row] != DropReason::Retained
                        || (index > 0 && self.original_row_by_active[index - 1] >= row)
                })
        {
            return Err(invalid(
                "prepared data are not aligned on one observation axis",
            ));
        }
        Ok(())
    }
}

pub(crate) fn retain_rows<T>(values: &mut Vec<T>, keep: &[bool]) {
    let mut row = 0;
    values.retain(|_| {
        let retain = keep[row];
        row += 1;
        retain
    });
}

fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
