//! One final FE topology shared by projection, singleton, DoF and output consumers.
mod build;
mod components;
mod implicit;
mod membership;
pub use implicit::prepare_implicit_constant;
mod singleton;
mod subset;
use crate::{
    data::Prepared,
    error::{ErrorCode, FeregError, Result},
    AbsorbTermId, SourceId,
};
pub use build::prepare_fixed_effects;
pub(crate) use build::prepare_unpruned;
pub use components::{connected_components, ComponentInfo};
pub(crate) use components::{connected_components_all, connected_components_all_polled};
pub use membership::LevelIndex;
pub use singleton::{
    prune_singletons, prune_singletons_with_multiplicity, SingletonMultiplicity, SingletonResult,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlopeChannel {
    pub source: SourceId,
    pub label: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    /// None denotes the pinned, unreported one-level structural constant.
    pub term_id: Option<AbsorbTermId>,
    pub partition: usize,
    pub categorical_sources: Vec<SourceId>,
    pub intercept: bool,
    pub slopes: Vec<SlopeChannel>,
    pub label: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    recipe: Vec<SourceId>,
    codes: Vec<u32>,
    original_levels: Vec<u32>,
    membership: LevelIndex,
}
impl Partition {
    #[must_use]
    pub fn recipe(&self) -> &[SourceId] {
        &self.recipe
    }
    #[must_use]
    pub fn codes(&self) -> &[u32] {
        &self.codes
    }
    #[must_use]
    pub fn n_levels(&self) -> usize {
        self.original_levels.len()
    }
    #[must_use]
    pub fn original_levels(&self) -> &[u32] {
        &self.original_levels
    }
    #[must_use]
    pub fn membership(&self) -> &LevelIndex {
        &self.membership
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeTopology {
    nobs: usize,
    partitions: Vec<Partition>,
    effects: Vec<Effect>,
    components: ComponentInfo,
}
impl FeTopology {
    #[must_use]
    pub fn nobs(&self) -> usize {
        self.nobs
    }
    #[must_use]
    pub fn partitions(&self) -> &[Partition] {
        &self.partitions
    }
    #[must_use]
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    #[must_use]
    pub fn components(&self) -> &ComponentInfo {
        &self.components
    }
    pub fn effect_partition(&self, effect: usize) -> Result<&Partition> {
        let effect = self
            .effects
            .get(effect)
            .ok_or_else(|| invalid("FE term is out of range"))?;
        self.partitions
            .get(effect.partition)
            .ok_or_else(|| invalid("FE partition is out of range"))
    }
    pub fn validate(&self, sample: &Prepared) -> Result<()> {
        if self.nobs != sample.n_active() {
            return Err(invalid("FE topology and prepared rows differ"));
        }
        for partition in &self.partitions {
            if partition.codes.len() != self.nobs
                || partition.membership.n_levels() != partition.n_levels()
            {
                return Err(invalid("FE partition dimensions disagree"));
            }
            partition.membership.validate(&partition.codes)?;
            if (0..partition.n_levels()).any(|level| {
                partition
                    .membership
                    .members(level)
                    .is_ok_and(|rows| rows.is_empty())
            }) {
                return Err(invalid("final FE partition contains an unused level"));
            }
        }
        for (index, effect) in self.effects.iter().enumerate() {
            self.effect_partition(index)?;
            for slope in &effect.slopes {
                let values = sample
                    .regressors
                    .get(slope.source.get() as usize - 1)
                    .ok_or_else(|| invalid("FE slope source is absent"))?;
                if values.len() != self.nobs {
                    return Err(invalid("FE slope values are not sample aligned"));
                }
                if values.iter().any(|value| !value.is_finite()) {
                    return Err(FeregError::new(
                        ErrorCode::InvalidData,
                        "FE slope source contains a nonfinite final-sample value",
                    ));
                }
            }
        }
        Ok(())
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
