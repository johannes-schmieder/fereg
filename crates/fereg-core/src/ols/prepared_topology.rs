//! Ordinary grammar finishes directly into the shared immutable FE topology.

use super::*;

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
            let source = self
                .categorical_sources
                .iter()
                .find(|source| source.source() == effect.sources[0])
                .ok_or_else(|| invalid("borrowed FE design categorical source is out of range"))?;
            if !source.references_topology_candidate(candidate)
                || source.codes(topology)?.len() != nobs
                || self
                    .sample
                    .regressors
                    .get(source.prepared_source_index())
                    .is_none_or(|values| values.len() != nobs)
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
