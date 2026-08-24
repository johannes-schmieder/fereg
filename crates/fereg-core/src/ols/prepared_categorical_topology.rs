//! Final-sample categorical fixed-effect topology for prepared OLS.
//!
//! Raw single-source codes stay borrowed from the prepared categorical source
//! table. A genuine interaction retains exactly one compact `u32` code per
//! observation plus compressed membership; repeated and reordered recipes
//! reuse the same numerical topology source.

use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    borrow_categorical_topology_design, BorrowedInterceptFeDesign, CategoricalCodeView,
    CategoricalTopologyRegistry,
};

use super::{PreparedCategoricalEffect, PreparedCategoricalEffectRequest};

/// Memory-efficient final-sample topology for categorical absorbed terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCategoricalFeTopology {
    registry: CategoricalTopologyRegistry,
    effects: Vec<PreparedCategoricalEffect>,
    effect_to_topology: Vec<usize>,
    nobs: usize,
    total_levels: usize,
}

impl PreparedCategoricalFeTopology {
    pub fn build(
        source_table: &[CategoricalCodeView<'_>],
        requests: &[PreparedCategoricalEffectRequest],
        nobs: usize,
    ) -> Result<Self> {
        let mut reservation = SystemPreparedTopologyReservation;
        Self::build_with_reservation(source_table, requests, nobs, &mut reservation)
    }

    #[must_use]
    pub const fn nobs(&self) -> usize {
        self.nobs
    }

    #[must_use]
    pub const fn total_levels(&self) -> usize {
        self.total_levels
    }

    #[must_use]
    pub const fn registry(&self) -> &CategoricalTopologyRegistry {
        &self.registry
    }

    #[must_use]
    pub fn effects(&self) -> &[PreparedCategoricalEffect] {
        &self.effects
    }

    #[must_use]
    pub fn effect_to_topology(&self) -> &[usize] {
        &self.effect_to_topology
    }

    pub fn borrowed_design<'a>(
        &'a self,
        source_table: &'a [CategoricalCodeView<'a>],
    ) -> Result<BorrowedInterceptFeDesign<'a>> {
        borrow_categorical_topology_design(
            self.nobs,
            &self.registry,
            source_table,
            &self.effect_to_topology,
        )
    }

    pub fn validate(&self, source_table: &[CategoricalCodeView<'_>]) -> Result<()> {
        validate_source_table(source_table, self.nobs)?;
        self.registry.validate_all(source_table)?;
        if self.effect_to_topology.len() != self.effects.len() {
            return Err(invalid(
                "prepared categorical topology effect map and metadata differ in length",
            ));
        }
        for (effect_index, effect) in self.effects.iter().enumerate() {
            effect.validate(source_table.len())?;
            if self.effect_to_topology[effect_index] != effect.topology_source_index {
                return Err(invalid(
                    "prepared categorical topology effect map and effect metadata disagree",
                ));
            }
            self.registry.source(effect.topology_source_index)?;
        }
        if self.registry.total_levels()? != self.total_levels {
            return Err(invalid(
                "prepared categorical topology retained level total changed after construction",
            ));
        }
        Ok(())
    }

    fn build_with_reservation<R: PreparedTopologyReservation>(
        source_table: &[CategoricalCodeView<'_>],
        requests: &[PreparedCategoricalEffectRequest],
        nobs: usize,
        reservation: &mut R,
    ) -> Result<Self> {
        validate_source_table(source_table, nobs)?;
        let mut registry = CategoricalTopologyRegistry::new();
        let mut effects = Vec::new();
        reservation.reserve_vec(
            &mut effects,
            requests.len(),
            "unable to reserve prepared categorical absorbed effects",
        )?;
        let mut effect_to_topology = Vec::new();
        reservation.reserve_vec(
            &mut effect_to_topology,
            requests.len(),
            "unable to reserve prepared categorical effect-to-topology map",
        )?;

        for request in requests {
            request.validate(source_table.len())?;
            let topology_source_index =
                registry.intern(&request.categorical_source_indices, source_table)?;
            let effect = PreparedCategoricalEffect {
                term_id: request.term_id,
                label: fallible_string_copy(
                    &request.label,
                    reservation,
                    "unable to reserve prepared categorical effect label",
                )?,
                sources: fallible_vec_copy(
                    &request.sources,
                    reservation,
                    "unable to reserve prepared categorical effect source identities",
                )?,
                categorical_source_indices: fallible_vec_copy(
                    &request.categorical_source_indices,
                    reservation,
                    "unable to reserve prepared categorical effect source indices",
                )?,
                topology_source_index,
            };
            push_reserved(
                &mut effects,
                effect,
                "prepared categorical absorbed-effect capacity exhausted",
            )?;
            push_reserved(
                &mut effect_to_topology,
                topology_source_index,
                "prepared categorical effect-to-topology capacity exhausted",
            )?;
        }
        registry.validate_all(source_table)?;
        let total_levels = registry.total_levels()?;
        let topology = Self {
            registry,
            effects,
            effect_to_topology,
            nobs,
            total_levels,
        };
        topology.validate(source_table)?;
        Ok(topology)
    }
}

pub(crate) trait PreparedTopologyReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;

    fn reserve_string(
        &mut self,
        target: &mut String,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

pub(crate) struct SystemPreparedTopologyReservation;

impl PreparedTopologyReservation for SystemPreparedTopologyReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }

    fn reserve_string(
        &mut self,
        target: &mut String,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

pub(crate) fn validate_source_table(
    source_table: &[CategoricalCodeView<'_>],
    nobs: usize,
) -> Result<()> {
    for (source_index, source) in source_table.iter().copied().enumerate() {
        if source.codes().len() != nobs {
            return Err(invalid(format!(
                "prepared categorical source {} has {} rows; expected {nobs}",
                source_index + 1,
                source.codes().len(),
            )));
        }
        if nobs == 0 {
            if source.n_levels() != 0 {
                return Err(invalid(format!(
                    "empty prepared categorical source {} declares {} levels",
                    source_index + 1,
                    source.n_levels(),
                )));
            }
        } else if source.n_levels() == 0
            || source
                .codes()
                .iter()
                .any(|code| *code as usize >= source.n_levels())
        {
            return Err(invalid_data(format!(
                "prepared categorical source {} has invalid compact codes or level count",
                source_index + 1,
            )));
        }
    }
    Ok(())
}

fn fallible_vec_copy<T: Copy, R: PreparedTopologyReservation>(
    input: &[T],
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, input.len(), label)?;
    if output.capacity() < input.len() {
        return Err(allocation(
            "prepared categorical topology vector reservation returned insufficient capacity",
        ));
    }
    output.extend_from_slice(input);
    Ok(output)
}

fn fallible_string_copy<R: PreparedTopologyReservation>(
    input: &str,
    reservation: &mut R,
    label: &'static str,
) -> Result<String> {
    let mut output = String::new();
    reservation.reserve_string(&mut output, input.len(), label)?;
    if output.capacity() < input.len() {
        return Err(allocation(
            "prepared categorical topology string reservation returned insufficient capacity",
        ));
    }
    output.push_str(input);
    Ok(output)
}

pub(crate) fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

pub(crate) fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

pub(crate) fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}
