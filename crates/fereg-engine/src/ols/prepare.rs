use crate::{
    data::{
        CompiledColumns, DomainDecision, Loaded, LoadedAdjustment, NumericDomainPolicy, Prepared,
        SourcePlan, WeightKind,
    },
    spec::Estimator,
    topology::{prepare_fixed_effects, prepare_implicit_constant},
    ErrorCode, FeregError, ModelSpec, Result,
};

/// Finalize one authoritative OLS sample before normalization or any solve.
pub fn prepare(spec: &ModelSpec, loaded: &Loaded) -> Result<Prepared> {
    let columns = CompiledColumns::new(spec, Estimator::Ols)?;
    prepare_compiled(spec, loaded, &columns)
}
pub(super) fn prepare_compiled(
    spec: &ModelSpec,
    loaded: &Loaded,
    columns: &CompiledColumns,
) -> Result<Prepared> {
    if loaded
        .weights
        .as_ref()
        .is_some_and(|(kind, _)| *kind == WeightKind::Importance)
    {
        return Err(FeregError::new(
            ErrorCode::UnsupportedWeight,
            "OLS does not support importance weights",
        ));
    }
    if !matches!(loaded.adjustment, LoadedAdjustment::None) {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "OLS does not support offset or exposure",
        ));
    }
    let sample = SourcePlan::new(spec)?.prepare(loaded, &OlsPolicy)?;
    let sample = prepare_fixed_effects(spec, sample)?;
    if sample.n_active() == 0 {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "OLS has no observations after preparation",
        ));
    }
    if columns.implicit_constant {
        prepare_implicit_constant(sample)
    } else {
        Ok(sample)
    }
}
struct OlsPolicy;
impl NumericDomainPolicy for OlsPolicy {
    fn outcome(&self, _: usize, _: f64) -> Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
    fn weight(&self, _: usize, kind: WeightKind, value: f64) -> Result<DomainDecision> {
        match kind {
            WeightKind::Analytic if value == 0. => Ok(DomainDecision::Drop),
            WeightKind::Analytic if value < 0. => Err(FeregError::new(
                ErrorCode::NegativeWeight,
                "analytic weight is negative",
            )),
            WeightKind::Analytic => Ok(DomainDecision::Retain),
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
            WeightKind::Importance => Err(FeregError::new(
                ErrorCode::UnsupportedWeight,
                "OLS does not support importance weights",
            )),
        }
    }
    fn offset(&self, _: usize, _: f64) -> Result<DomainDecision> {
        Err(FeregError::new(
            ErrorCode::Unsupported,
            "OLS does not support offset",
        ))
    }
    fn exposure(&self, _: usize, _: f64) -> Result<DomainDecision> {
        Err(FeregError::new(
            ErrorCode::Unsupported,
            "OLS does not support exposure",
        ))
    }
}
