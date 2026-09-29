//! Canonical omission identities, independent of coefficients and working weights.
use super::{invalid, Fit, Problem, Role, Status};
use crate::error::{reserved, Result};

/// The estimator owns sample/generation alignment. Construct only from its
/// canonical rank solve, never from a start or an ordinary IRLS factorization.
#[derive(Debug, PartialEq, Eq)]
pub struct FrozenBasis {
    statuses: Vec<Status>,
    kept: Vec<usize>,
}
impl FrozenBasis {
    pub fn from_fit(fit: &Fit) -> Result<Self> {
        if fit.kept.iter().copied().ne(fit
            .statuses
            .iter()
            .enumerate()
            .filter_map(|(i, status)| (*status == Status::Kept).then_some(i)))
            || fit.statuses.contains(&Status::NormalizedAbsorbedIntercept)
        {
            return Err(invalid(
                "frozen basis requires canonical WLS slot identities",
            ));
        }
        let mut statuses = reserved(fit.statuses.len())?;
        statuses.extend_from_slice(&fit.statuses);
        let mut kept = reserved(fit.kept.len())?;
        kept.extend_from_slice(&fit.kept);
        Ok(Self { statuses, kept })
    }
    pub fn statuses(&self) -> &[Status] {
        &self.statuses
    }
    pub fn kept(&self) -> &[usize] {
        &self.kept
    }
    /// Equal rank alone misses a pivot/omission change in public coordinates.
    pub fn matches(&self, fit: &Fit) -> bool {
        self.statuses == fit.statuses && self.kept == fit.kept
    }
    pub(super) fn select(&self, problem: &Problem<'_>) -> Result<(Vec<usize>, Vec<Status>)> {
        if self.statuses.len() != problem.slots.len()
            || self
                .statuses
                .iter()
                .zip(problem.slots)
                .any(|(status, slot)| {
                    let role = match status {
                        Status::Base => Role::Base,
                        Status::ExplicitlyOmitted => Role::ExplicitlyOmitted,
                        Status::AbsorbedInterceptDeferred => Role::AbsorbedIntercept,
                        _ => Role::Ordinary,
                    };
                    role != slot.role
                })
        {
            return Err(invalid("frozen finite WLS basis and slot roles differ"));
        }
        let mut columns = reserved(self.kept.len())?;
        columns.extend_from_slice(&self.kept);
        let mut statuses = reserved(self.statuses.len())?;
        statuses.extend_from_slice(&self.statuses);
        Ok((columns, statuses))
    }
}
