//! Ordinary grammar finishes directly into the shared immutable FE topology.

use super::*;
use crate::fe::{finalize_categorical_topology, FinalizedCategoricalTopology};

impl PreparedOlsData {
    pub(crate) fn validate_fe_bindings(&self) -> Result<()> {
        let topology = self.fe_topology();
        let nobs = self.sample.n_active();
        if topology.nobs() != nobs || topology.effect_to_topology().len() != self.effect_specs.len()
        {
            return Err(invalid(
                "borrowed FE design topology is not aligned with the final sample",
            ));
        }
        for (effect, source_index) in self.effect_specs.iter().zip(topology.effect_to_topology()) {
            let registry = topology.registry().source(*source_index)?;
            if !effect.intercept
                || !effect.slopes.is_empty()
                || effect.sources.len() != 1
                || registry.partition().is_interaction()
                || registry.partition().categorical_source_indices().len() != 1
            {
                return Err(invalid(
                    "borrowed FE design source identities are inconsistent",
                ));
            }
            let candidate = topology.compacted_sources()
                [registry.partition().categorical_source_indices()[0]]
                .candidate_source_index();
            let (categorical_index, source) = self
                .categorical_sources
                .iter()
                .enumerate()
                .find(|(_, source)| source.source() == effect.sources[0])
                .ok_or_else(|| invalid("borrowed FE design categorical source is out of range"))?;
            if !source.references_topology_candidate(candidate)
                || source.codes(topology)?.len() != nobs
                || self
                    .sample
                    .regressors
                    .get(source.prepared_source_index())
                    .map_or_else(
                        || {
                            !self.sample.regressors.encoded_matches(
                                source.prepared_source_index(),
                                categorical_index,
                                nobs,
                            )
                        },
                        |values| values.len() != nobs,
                    )
            {
                return Err(invalid(
                    "borrowed FE design source identities are inconsistent",
                ));
            }
        }
        Ok(())
    }
}

pub(super) fn build_borrowed_intercept_fe_design_with_reservation<'a, R: FeTopologyReservation>(
    prepared: &'a PreparedOlsData,
    reservation: &mut R,
) -> Result<BorrowedInterceptFeDesign<'a>> {
    prepared.validate_fe_bindings()?;
    let mut effects = Vec::new();
    reservation.reserve_vec(
        &mut effects,
        prepared.effect_specs.len(),
        "unable to reserve borrowed FE design effects",
    )?;
    for source_index in prepared.fe_topology.effect_to_topology() {
        let source = ValidatedBorrowedInterceptSource::from_finalized_source(
            &prepared.fe_topology,
            *source_index,
        )
        .ok_or_else(|| invalid("borrowed FE design topology source is out of range"))?;
        push_reserved(
            &mut effects,
            source.effect(),
            "borrowed FE design effect capacity exhausted",
        )?;
    }
    BorrowedInterceptFeDesign::new(prepared.sample.n_active(), effects)
}

// Transfer already checked its exact immutable membership. Rebuilt topology
// keeps validation at the original publication point, after candidate buffers
// are released, so interaction scratch and failure ordering do not move earlier.
pub(crate) enum PreparationTopology {
    Transferred(crate::fe::ValidatedFinalizedTopology),
    Rebuilt(FinalizedCategoricalTopology),
}

impl PreparationTopology {
    pub(crate) fn get(&self) -> &FinalizedCategoricalTopology {
        match self {
            Self::Transferred(owner) => owner.get(),
            Self::Rebuilt(topology) => topology,
        }
    }

    pub(crate) fn into_validated(self) -> Result<crate::fe::ValidatedFinalizedTopology> {
        match self {
            Self::Transferred(owner) => Ok(owner),
            Self::Rebuilt(topology) => crate::fe::ValidatedFinalizedTopology::new(topology),
        }
    }
}

pub(crate) fn finalize_preparation_sources(
    mut sources: Vec<PreparedCategoricalSource>,
    recipes: &[&[usize]],
    sample: &PreparedNumericSample,
    active: &[bool],
    transfer: bool,
    membership: &mut Vec<(usize, crate::fe::LevelIndex)>,
) -> Result<(PreparationTopology, Vec<PreparedCategoricalSource>)> {
    let topology = if transfer {
        PreparationTopology::Transferred(
            crate::fe::ValidatedFinalizedTopology::from_single_source_indexes(
                sources.len(),
                sample.n_active(),
                recipes,
                |index| {
                    let position = membership
                        .iter()
                        .position(|(i, _)| *i == index)
                        .ok_or_else(|| invalid("transferred topology source is missing"))?;
                    let membership = membership.swap_remove(position).1;
                    let source = &mut sources[index];
                    Ok((
                        std::mem::take(&mut source.encoded.codes),
                        source.encoded.n_levels(),
                        membership,
                    ))
                },
            )?,
        )
    } else {
        PreparationTopology::Rebuilt(finalize_categorical_topology(
            &crate::ols::prepared_categorical_data::categorical_views(&sources)?,
            recipes,
            active,
        )?)
    };
    Ok((topology, sources))
}
