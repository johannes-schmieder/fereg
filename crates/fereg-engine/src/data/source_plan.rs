//! One metadata-derived use table for the unified source bank.
use super::{Loaded, LoadedSource, NumericDomainPolicy, NumericSourceUse, Prepared};
use crate::{
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
    ComponentKind, ModelSpec,
};

pub struct SourcePlan {
    uses: Vec<NumericSourceUse>,
    ordinary_categorical: Vec<bool>,
    numeric_required: Vec<bool>,
}
impl SourcePlan {
    pub fn new(spec: &ModelSpec) -> Result<Self> {
        spec.validate()?;
        let mut uses = reserved(spec.model.sources.len())?;
        uses.resize(
            spec.model.sources.len(),
            NumericSourceUse {
                ordinary: false,
                fixed_effect: false,
                cluster: false,
            },
        );
        let mut ordinary_categorical = reserved(uses.len())?;
        ordinary_categorical.resize(uses.len(), false);
        let mut numeric_required = reserved(uses.len())?;
        numeric_required.resize(uses.len(), false);
        // Omitted columns still contribute their declared source availability.
        for term in &spec.model.terms {
            for component in &term.components {
                let source = component.source.get() as usize - 1;
                uses[source].ordinary = true;
                numeric_required[source] = true;
                ordinary_categorical[source] |= component.kind == ComponentKind::Categorical;
            }
        }
        for effect in &spec.absorbed_terms {
            for source in effect
                .categorical_sources
                .iter()
                .chain(&effect.slope_sources)
            {
                uses[source.get() as usize - 1].fixed_effect = true;
            }
        }
        for source in spec
            .absorbed_terms
            .iter()
            .flat_map(|effect| &effect.slope_sources)
        {
            numeric_required[source.get() as usize - 1] = true;
        }
        for cluster in &spec.clusters {
            for source in &cluster.sources {
                uses[source.get() as usize - 1].cluster = true;
            }
        }
        if uses
            .iter()
            .any(|usage| !usage.ordinary && !usage.fixed_effect && !usage.cluster)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "model registry contains an unused source",
            ));
        }
        Ok(Self {
            uses,
            ordinary_categorical,
            numeric_required,
        })
    }
    #[must_use]
    pub fn uses(&self) -> &[NumericSourceUse] {
        &self.uses
    }
    pub fn prepare<P: NumericDomainPolicy>(&self, loaded: &Loaded, policy: &P) -> Result<Prepared> {
        self.prepare_polled(loaded, policy, None)
    }
    pub(crate) fn prepare_polled<P: NumericDomainPolicy>(
        &self,
        loaded: &Loaded,
        policy: &P,
        poll: Poll<'_>,
    ) -> Result<Prepared> {
        execution::check(poll)?;
        loaded.validate_alignment_polled(poll)?;
        if loaded.sources.len() != self.uses.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "loaded source bank and model registry differ",
            ));
        }
        for (index, source) in loaded.sources.iter().enumerate() {
            execution::check_chunk(poll, index)?;
            if !self.numeric_required[index] {
                continue;
            }
            let LoadedSource::Numeric(_) = source else {
                return Err(FeregError::new(
                    ErrorCode::Unsupported,
                    "ordinary regressors and absorbed slopes require numeric sources",
                ));
            };
        }
        let sample = loaded.prepare_polled(&self.uses, policy, poll)?;
        // Built-in Stata validates factors on the available sample, before
        // estimator-specific deletions can hide an invalid factor value.
        for (source, &categorical) in self.ordinary_categorical.iter().enumerate() {
            execution::check_chunk(poll, source)?;
            if !categorical {
                continue;
            }
            for (row, &value) in sample.regressors[source].iter().enumerate() {
                execution::check_chunk(poll, row)?;
                if value < 0. || value.fract() != 0. {
                    return Err(FeregError::new(
                        ErrorCode::InvalidFactor,
                        format!(
                            "ordinary factor source {} must contain nonnegative integers; invalid value at observation {}",
                            source + 1,
                            sample.original_row_by_active[row] + 1
                        ),
                    ));
                }
            }
        }
        execution::check(poll)?;
        Ok(sample)
    }
}

#[cfg(test)]
mod interruption_tests;
