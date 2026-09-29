//! Oracle-owned FE geometry; deliberately independent of production topology.
use crate::explicit_ols::DummyEffect;

#[derive(Debug, Clone)]
pub struct ExplicitFeDesign {
    nobs: usize,
    effects: Vec<DummyEffect>,
}

impl ExplicitFeDesign {
    pub fn new(nobs: usize, effects: Vec<DummyEffect>) -> Result<Self, String> {
        if effects.iter().any(|effect| effect.codes().len() != nobs) {
            return Err("oracle effect lengths differ from observation count".into());
        }
        Ok(Self { nobs, effects })
    }

    pub const fn nobs(&self) -> usize {
        self.nobs
    }

    pub fn effects(&self) -> &[DummyEffect] {
        &self.effects
    }
}

pub(super) fn channels(effect: &DummyEffect) -> usize {
    usize::from(effect.has_intercept()) + effect.slopes().len()
}

pub(super) fn members(effect: &DummyEffect, level: usize) -> Vec<usize> {
    effect
        .codes()
        .iter()
        .enumerate()
        .filter_map(|(row, &code)| (code as usize == level).then_some(row))
        .collect()
}

pub(super) fn basis_value(effect: &DummyEffect, channel: usize, row: usize) -> f64 {
    if effect.has_intercept() && channel == 0 {
        1.0
    } else {
        effect.slopes()[channel - usize::from(effect.has_intercept())][row]
    }
}
