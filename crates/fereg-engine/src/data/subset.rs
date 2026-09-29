//! Stage one replacement owner without changing the accepted generation.
use super::{DropReason, Prepared, PreparedAdjustment, PreparedWeights};
use crate::error::{reserved, ErrorCode, FeregError, Result};

impl Prepared {
    pub(crate) fn stage_deletions(&self, reasons: &[DropReason]) -> Result<Self> {
        self.validate_alignment()?;
        if reasons.len() != self.n_active()
            || reasons.iter().any(|reason| {
                !matches!(
                    reason,
                    DropReason::Retained
                        | DropReason::Singleton
                        | DropReason::FixedEffectSeparation
                        | DropReason::SimplexSeparation
                        | DropReason::IteratedRectifierSeparation
                        | DropReason::MuSeparation
                )
            })
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "invalid staged sample deletion reasons",
            ));
        }
        let n = reasons
            .iter()
            .filter(|&&r| r == DropReason::Retained)
            .count();
        let mut keep = reserved(reasons.len())?;
        keep.extend(reasons.iter().map(|&r| r == DropReason::Retained));
        let mut drop_reasons = reserved(self.n_original)?;
        drop_reasons.extend_from_slice(&self.drop_reasons);
        for (&row, &reason) in self.retained_rows().iter().zip(reasons) {
            drop_reasons[row] = reason;
        }
        let mut regressors = reserved(self.regressors.len())?;
        for column in &self.regressors {
            regressors.push(subset(column, &keep, n)?);
        }
        let weights = match &self.weights {
            PreparedWeights::None => PreparedWeights::None,
            PreparedWeights::Provided { kind, values } => PreparedWeights::Provided {
                kind: *kind,
                values: subset(values, &keep, n)?,
            },
        };
        let adjustment = match &self.adjustment {
            PreparedAdjustment::None => PreparedAdjustment::None,
            PreparedAdjustment::Offset(values) => {
                PreparedAdjustment::Offset(subset(values, &keep, n)?)
            }
            PreparedAdjustment::Exposure(values) => {
                PreparedAdjustment::Exposure(subset(values, &keep, n)?)
            }
        };
        let staged = Self {
            topology: self
                .topology
                .as_ref()
                .map(|t| t.subset(&keep))
                .transpose()?,
            n_original: self.n_original,
            n_candidate: self.n_candidate,
            original_row_by_active: subset(self.retained_rows(), &keep, n)?,
            outcome: subset(&self.outcome, &keep, n)?,
            regressors,
            weights,
            adjustment,
            drop_reasons,
        };
        staged.validate_alignment()?;
        if let Some(topology) = &staged.topology {
            topology.validate(&staged)?;
        }
        Ok(staged)
    }
}

fn subset<T: Copy>(source: &[T], keep: &[bool], n: usize) -> Result<Vec<T>> {
    let mut result = reserved(n)?;
    result.extend(
        source
            .iter()
            .zip(keep)
            .filter_map(|(&v, &k)| k.then_some(v)),
    );
    Ok(result)
}
