//! Experimental PPML sharing of immutable component indexing only.
//!
//! A projection may stage topology, but only the outer accepted-frame boundary
//! publishes it. No weights, masses, iterates, trees or numerical inverses live
//! here. A support change requires a new FE owner, even at identical dimensions.

use super::{BorrowedInterceptFeDesign, ComponentInfo, MapGeometryGeneration};
use crate::error::{allocation, invalid_argument, Result};

pub(crate) struct PpmlTopology {
    generation: MapGeometryGeneration,
    valid: bool,
    accepted: Option<ComponentInfo>,
    pending: Option<ComponentInfo>,
    unreported_accept: Option<u64>,
    allowance: u128,
}

pub(crate) fn retained_bytes(
    mut levels: impl Iterator<Item = usize>,
    effects: usize,
) -> Result<u128> {
    levels
        .try_fold(0_u128, |total, count| total.checked_add(count as u128))
        .and_then(|total| total.checked_mul(std::mem::size_of::<usize>() as u128))
        .and_then(|total| {
            total.checked_add(effects as u128 * std::mem::size_of::<Vec<usize>>() as u128)
        })
        .ok_or_else(|| allocation("PPML retained topology capacity overflows"))
}

pub(crate) fn components(design: &BorrowedInterceptFeDesign<'_>) -> Result<ComponentInfo> {
    let mut codes = [&[][..]; 3];
    let mut levels = [0; 3];
    for (index, effect) in design.effects().iter().enumerate() {
        codes[index] = effect.codes();
        levels[index] = effect.n_levels();
    }
    super::connected_components_all(
        &codes[..design.effects().len()],
        &levels[..design.effects().len()],
    )
}

impl PpmlTopology {
    pub(crate) fn new(generation: MapGeometryGeneration, allowance: u128) -> Self {
        Self {
            generation,
            valid: true,
            accepted: None,
            pending: None,
            unreported_accept: None,
            allowance,
        }
    }

    pub(crate) fn begin(&mut self, generation: MapGeometryGeneration) -> Result<()> {
        if !self.valid
            || (generation.sample, generation.topology)
                != (self.generation.sample, self.generation.topology)
        {
            self.invalidate();
            return Err(invalid_argument(
                "PPML topology requires a new support owner",
            ));
        }
        if generation.weights != self.generation.weights {
            self.reject();
        }
        self.generation = generation;
        Ok(())
    }

    pub(crate) fn prepare(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
    ) -> Result<&ComponentInfo> {
        if !self.valid {
            return Err(invalid_argument("PPML topology support was invalidated"));
        }
        // Lifecycle hooks are allocation-free, including transaction Drop.
        // Report accepted ownership only at the next ordinary preparation.
        if let Some(weights) = self.unreported_accept.take() {
            self.trace("accepted", weights);
        }
        if self.accepted.is_none() && self.pending.is_none() {
            let required = retained_bytes(
                design.effects().iter().map(|effect| effect.n_levels()),
                design.effects().len(),
            )?;
            crate::memory::require_admitted_capacity(required, self.allowance)?;
            let candidate = components(design)?;
            let actual = Self::component_bytes(&candidate);
            crate::memory::require_admitted_capacity(actual, self.allowance)?;
            self.pending = Some(candidate);
            self.trace("built", self.generation.weights);
        } else {
            self.trace("reused", self.generation.weights);
        }
        Ok(self
            .accepted
            .as_ref()
            .or(self.pending.as_ref())
            .expect("prepared topology"))
    }

    pub(crate) fn accept(&mut self, generation: MapGeometryGeneration) {
        if self.valid && generation == self.generation {
            if let Some(pending) = self.pending.take() {
                self.accepted = Some(pending);
                self.unreported_accept = Some(generation.weights);
            }
        } else {
            self.reject();
        }
    }

    pub(crate) fn reject(&mut self) {
        self.pending = None;
    }

    pub(crate) fn invalidate(&mut self) {
        self.valid = false;
        self.accepted = None;
        self.unreported_accept = None;
        self.reject();
    }

    fn component_bytes(components: &ComponentInfo) -> u128 {
        components.component_by_effect_level.capacity() as u128
            * std::mem::size_of::<Vec<usize>>() as u128
            + components
                .component_by_effect_level
                .iter()
                .map(|labels| labels.capacity() as u128 * std::mem::size_of::<usize>() as u128)
                .sum::<u128>()
    }

    pub(crate) fn bytes(&self) -> u128 {
        self.accepted
            .as_ref()
            .or(self.pending.as_ref())
            .map_or(0, Self::component_bytes)
    }

    fn trace(&self, event: &str, weights: u64) {
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=ppml_topology|event={event}|sample={}|topology={}|weights={}|accepted={}|pending={}|bytes={}",
                self.generation.sample,
                self.generation.topology,
                weights,
                u8::from(self.accepted.is_some()),
                u8::from(self.pending.is_some()),
                self.bytes(),
            ));
        }
    }

    #[cfg(test)]
    pub(crate) fn state(&self) -> (bool, bool) {
        (self.accepted.is_some(), self.pending.is_some())
    }
}
