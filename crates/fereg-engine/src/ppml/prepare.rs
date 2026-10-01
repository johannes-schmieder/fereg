use crate::{
    data::{DomainDecision, Loaded, NumericDomainPolicy, Prepared, SourcePlan, WeightKind},
    execution::{self, Poll},
    topology::prepare_unpruned,
    ErrorCode, FeregError, ModelSpec, Result,
};

/// Availability/domain checks precede the ledger's singleton generations.
pub(super) fn prepare(
    model: &ModelSpec,
    loaded: &Loaded,
    sources: &SourcePlan,
    poll: Poll<'_>,
) -> Result<Prepared> {
    let sample = sources.prepare_polled(loaded, &PpmlPolicy, poll)?;
    if !execution::any(&sample.outcome, |&y| y > 0., poll)? {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "PPML has no positive outcome after preparation",
        ));
    }
    prepare_unpruned(model, sample)
}
struct PpmlPolicy;
impl NumericDomainPolicy for PpmlPolicy {
    fn outcome(&self, _: usize, value: f64) -> Result<DomainDecision> {
        if value < 0. {
            Err(FeregError::new(
                ErrorCode::InvalidData,
                "PPML outcome must be nonnegative",
            ))
        } else {
            Ok(DomainDecision::Retain)
        }
    }
    fn weight(&self, _: usize, kind: WeightKind, value: f64) -> Result<DomainDecision> {
        match kind {
            WeightKind::Frequency
                if value > 0. && value.fract() == 0. && value <= u32::MAX as f64 =>
            {
                Ok(DomainDecision::Retain)
            }
            WeightKind::Frequency => Err(FeregError::new(
                ErrorCode::InvalidFrequencyWeight,
                "frequency weight must be a positive integer representable as u32",
            )),
            WeightKind::Probability if value > 0. => Ok(DomainDecision::Retain),
            WeightKind::Probability => Err(FeregError::new(
                ErrorCode::InvalidData,
                "probability weight must be positive and finite",
            )),
            WeightKind::Analytic | WeightKind::Importance => Err(FeregError::new(
                ErrorCode::UnsupportedWeight,
                "PPML supports only frequency and probability weights",
            )),
        }
    }
    fn offset(&self, _: usize, _: f64) -> Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
    fn exposure(&self, _: usize, value: f64) -> Result<DomainDecision> {
        if value > 0. {
            Ok(DomainDecision::Retain)
        } else {
            Err(FeregError::new(
                ErrorCode::InvalidData,
                "PPML exposure must be positive and finite",
            ))
        }
    }
}
