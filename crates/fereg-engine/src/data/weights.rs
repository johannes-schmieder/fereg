//! Harvested from legacy/fereg-core at b1700e6d; no memory-mode admission.
/// Weight syntax identified by the Stata control plane.
///
/// This type records the actual requested kind. Domain acceptance, objective
/// scaling, normalization, and covariance semantics belong to an explicit
/// caller-supplied policy and later estimator stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightKind {
    Frequency,
    Analytic,
    Probability,
    Importance,
}

impl WeightKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Frequency => "fweight",
            Self::Analytic => "aweight",
            Self::Probability => "pweight",
            Self::Importance => "iweight",
        }
    }
}

/// Borrowed objective weights shared by projection and finite least squares.
/// A reference view preserves the final sample's normalization without a copy;
/// provided values also support immutable IRLS working-weight frames.
#[derive(Debug, Clone, Copy)]
pub enum Weights<'a> {
    Unit,
    Provided(&'a [f64]),
    Reference(super::ReferenceWeights<'a>),
}
impl Weights<'_> {
    #[must_use]
    pub fn at(self, row: usize) -> f64 {
        match self {
            Self::Unit => 1.,
            Self::Provided(values) => values[row],
            Self::Reference(weights) => weights.value(row),
        }
    }
    pub(crate) fn validate(self, nobs: usize) -> crate::error::Result<()> {
        use crate::error::{ErrorCode, FeregError};
        let length = match self {
            Self::Unit => nobs,
            Self::Provided(values) => values.len(),
            Self::Reference(weights) => weights.nobs(),
        };
        if length != nobs {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "objective weights and observations differ",
            ));
        }
        if let Self::Provided(values) = self {
            if values
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.)
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "objective weights must be positive and finite",
                ));
            }
        }
        Ok(())
    }
}
