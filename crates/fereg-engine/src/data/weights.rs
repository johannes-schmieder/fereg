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
