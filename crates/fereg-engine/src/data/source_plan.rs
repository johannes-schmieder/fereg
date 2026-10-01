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
        Self::new_polled(spec, &mut || Ok(()))
    }
    pub(crate) fn new_polled(
        spec: &ModelSpec,
        poll: &mut impl FnMut() -> Result<()>,
    ) -> Result<Self> {
        spec.validate_polled(poll)?;
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
        for (index, term) in spec.model.terms.iter().enumerate() {
            execution::check_chunk_with(index, poll)?;
            for (position, component) in term.components.iter().enumerate() {
                execution::check_chunk_with(position, poll)?;
                let source = component.source.get() as usize - 1;
                uses[source].ordinary = true;
                numeric_required[source] = true;
                ordinary_categorical[source] |= component.kind == ComponentKind::Categorical;
            }
        }
        for (index, effect) in spec.absorbed_terms.iter().enumerate() {
            execution::check_chunk_with(index, poll)?;
            for (position, source) in effect
                .categorical_sources
                .iter()
                .chain(&effect.slope_sources)
                .enumerate()
            {
                execution::check_chunk_with(position, poll)?;
                uses[source.get() as usize - 1].fixed_effect = true;
            }
        }
        for (index, source) in spec
            .absorbed_terms
            .iter()
            .flat_map(|effect| &effect.slope_sources)
            .enumerate()
        {
            execution::check_chunk_with(index, poll)?;
            numeric_required[source.get() as usize - 1] = true;
        }
        for (index, cluster) in spec.clusters.iter().enumerate() {
            execution::check_chunk_with(index, poll)?;
            for (position, source) in cluster.sources.iter().enumerate() {
                execution::check_chunk_with(position, poll)?;
                uses[source.get() as usize - 1].cluster = true;
            }
        }
        if execution::any_with(
            &uses,
            |usage| !usage.ordinary && !usage.fixed_effect && !usage.cluster,
            poll,
        )? {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "model registry contains an unused source",
            ));
        }
        poll()?;
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
