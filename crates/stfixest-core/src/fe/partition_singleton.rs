//! Interaction-aware singleton pruning over deduplicated categorical partitions.
//!
//! Candidate-sample singleton pruning needs only compact partition codes. This
//! adapter interns single-source and interaction recipes once, borrows raw codes
//! for ordinary effects, retains one `u32` vector per unique interaction, and
//! delegates the fixed-point deletion to the certified queue implementation.
//! It does not construct final solver membership and therefore avoids a second
//! observation-scale `usize` array while the candidate sample is still live.

use crate::error::{ErrorCode, Result, StFixestError};

use super::{
    prune_singletons_with_multiplicity, CategoricalCodeView, CategoricalPartitionRegistry,
    SingletonMultiplicity, SingletonResult,
};

/// Complete candidate-sample singleton result and reusable partition registry.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoricalPartitionSingletonResult {
    singleton: SingletonResult,
    registry: CategoricalPartitionRegistry,
    effect_to_partition: Vec<usize>,
}

impl CategoricalPartitionSingletonResult {
    #[must_use]
    pub const fn singleton(&self) -> &SingletonResult {
        &self.singleton
    }

    #[must_use]
    pub const fn registry(&self) -> &CategoricalPartitionRegistry {
        &self.registry
    }

    #[must_use]
    pub fn effect_to_partition(&self) -> &[usize] {
        &self.effect_to_partition
    }

    #[must_use]
    pub fn into_parts(self) -> (SingletonResult, CategoricalPartitionRegistry, Vec<usize>) {
        (self.singleton, self.registry, self.effect_to_partition)
    }
}

/// Prune singleton observations for categorical single-source and interaction
/// effects.
///
/// Each recipe is a nonempty table of categorical-source indices. Recipes are
/// canonicalized and deduplicated before the fixed-point computation, so
/// repeated terms and reordered interactions cannot change degree counts or
/// duplicate observation-scale storage.
pub fn prune_categorical_partition_singletons(
    source_table: &[CategoricalCodeView<'_>],
    effect_recipes: &[&[usize]],
    initially_active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
    keep_singletons: bool,
) -> Result<CategoricalPartitionSingletonResult> {
    let mut reservation = SystemPartitionSingletonReservation;
    prune_categorical_partition_singletons_with_reservation(
        source_table,
        effect_recipes,
        initially_active,
        multiplicity,
        keep_singletons,
        &mut reservation,
    )
}

trait PartitionSingletonReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemPartitionSingletonReservation;

impl PartitionSingletonReservation for SystemPartitionSingletonReservation {
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
}

fn prune_categorical_partition_singletons_with_reservation<R: PartitionSingletonReservation>(
    source_table: &[CategoricalCodeView<'_>],
    effect_recipes: &[&[usize]],
    initially_active: &[bool],
    multiplicity: SingletonMultiplicity<'_>,
    keep_singletons: bool,
    reservation: &mut R,
) -> Result<CategoricalPartitionSingletonResult> {
    for (source_index, source) in source_table.iter().copied().enumerate() {
        if source.codes().len() != initially_active.len() {
            return Err(invalid(format!(
                "categorical source {} has {} rows; expected {}",
                source_index + 1,
                source.codes().len(),
                initially_active.len()
            )));
        }
    }

    let mut registry = CategoricalPartitionRegistry::new();
    let mut effect_to_partition = Vec::new();
    reservation.reserve_vec(
        &mut effect_to_partition,
        effect_recipes.len(),
        "unable to reserve categorical effect-to-partition map",
    )?;
    for recipe in effect_recipes {
        let partition_index = registry.intern(recipe, source_table)?;
        push_reserved(
            &mut effect_to_partition,
            partition_index,
            "categorical effect-to-partition capacity exhausted",
        )?;
    }
    registry.validate_all(source_table)?;

    let unique_count = registry.partitions().len();
    let mut codes_by_partition = Vec::new();
    reservation.reserve_vec(
        &mut codes_by_partition,
        unique_count,
        "unable to reserve categorical singleton code views",
    )?;
    let mut level_counts = Vec::new();
    reservation.reserve_vec(
        &mut level_counts,
        unique_count,
        "unable to reserve categorical singleton level counts",
    )?;
    for partition in registry.partitions() {
        push_reserved(
            &mut codes_by_partition,
            partition.codes(source_table)?,
            "categorical singleton code-view capacity exhausted",
        )?;
        push_reserved(
            &mut level_counts,
            partition.n_levels(),
            "categorical singleton level-count capacity exhausted",
        )?;
    }

    let singleton = prune_singletons_with_multiplicity(
        &codes_by_partition,
        &level_counts,
        initially_active,
        multiplicity,
        keep_singletons,
    )?;
    Ok(CategoricalPartitionSingletonResult {
        singleton,
        registry,
        effect_to_partition,
    })
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn invalid(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidArgument, message)
}

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct InjectedReservation {
        calls: usize,
        fail_at: Option<usize>,
        skip_at: Option<usize>,
    }

    impl PartitionSingletonReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected categorical singleton adapter allocation failure",
                ));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test categorical singleton reservation failed"))
        }
    }

    #[test]
    fn interaction_detects_singletons_not_present_in_components() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let interaction = prune_categorical_partition_singletons(
            &sources,
            &[&[0, 1]],
            &[true; 4],
            SingletonMultiplicity::Unit,
            false,
        )
        .expect("interaction pruning");
        assert_eq!(interaction.singleton().active, vec![false; 4]);
        assert_eq!(interaction.singleton().dropped_observations.len(), 4);

        let components = prune_categorical_partition_singletons(
            &sources,
            &[&[0], &[1]],
            &[true; 4],
            SingletonMultiplicity::Unit,
            false,
        )
        .expect("component pruning");
        assert_eq!(components.singleton().active, vec![true; 4]);
        assert!(components.singleton().dropped_observations.is_empty());
    }

    #[test]
    fn interaction_participates_in_multi_effect_singleton_cascade() {
        let first = [0, 0, 1, 2];
        let constant = [0, 0, 0, 0];
        let second = [0, 1, 1, 2];
        let sources = [
            CategoricalCodeView::new(&first, 3),
            CategoricalCodeView::new(&constant, 1),
            CategoricalCodeView::new(&second, 3),
        ];
        let result = prune_categorical_partition_singletons(
            &sources,
            &[&[0, 1], &[2]],
            &[true; 4],
            SingletonMultiplicity::Unit,
            false,
        )
        .expect("cascade pruning");
        assert_eq!(result.singleton().active, vec![false; 4]);
        assert_eq!(result.singleton().dropped_observations.len(), 4);
        assert_eq!(result.singleton().cascade_events, 4);
    }

    #[test]
    fn repeated_and_reordered_recipes_are_deduplicated() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let region = [0, 0, 1, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
            CategoricalCodeView::new(&region, 2),
        ];
        let result = prune_categorical_partition_singletons(
            &sources,
            &[&[0, 1], &[1, 0], &[0, 1, 0], &[2]],
            &[true; 4],
            SingletonMultiplicity::Unit,
            true,
        )
        .expect("deduplicated pruning");
        assert_eq!(result.registry().partitions().len(), 2);
        assert_eq!(result.effect_to_partition(), &[0, 0, 0, 1]);
        assert_eq!(result.singleton().active, vec![true; 4]);
    }

    #[test]
    fn frequency_multiplicity_applies_to_interaction_levels() {
        let firm = [0, 1];
        let year = [0, 1];
        let weights = [2.0, 1.0];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let result = prune_categorical_partition_singletons(
            &sources,
            &[&[0, 1]],
            &[true; 2],
            SingletonMultiplicity::Frequency(&weights),
            false,
        )
        .expect("frequency interaction pruning");
        assert_eq!(result.singleton().active, vec![true, false]);
        assert_eq!(result.singleton().dropped_observations, vec![1]);
    }

    #[test]
    fn initially_inactive_rows_remain_inactive_but_are_not_reported_as_drops() {
        let firm = [0, 0, 1];
        let year = [0, 1, 0];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let result = prune_categorical_partition_singletons(
            &sources,
            &[&[0, 1]],
            &[true, false, true],
            SingletonMultiplicity::Unit,
            false,
        )
        .expect("inactive-row pruning");
        assert_eq!(result.singleton().active, vec![false, false, false]);
        assert_eq!(result.singleton().dropped_observations.len(), 2);
        assert!(!result.singleton().dropped_observations.contains(&1));
    }

    #[test]
    fn keep_singletons_validates_and_builds_registry_but_preserves_mask() {
        let firm = [0, 1];
        let year = [0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let result = prune_categorical_partition_singletons(
            &sources,
            &[&[0, 1]],
            &[true, false],
            SingletonMultiplicity::Unit,
            true,
        )
        .expect("keep singletons");
        assert_eq!(result.singleton().active, vec![true, false]);
        assert!(result.singleton().dropped_observations.is_empty());
        assert_eq!(result.registry().partitions().len(), 1);
    }

    #[test]
    fn malformed_recipe_and_source_dimensions_fail_closed() {
        let raw = [0, 1];
        let short = [0];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        assert_eq!(
            prune_categorical_partition_singletons(
                &sources,
                &[&[]],
                &[true; 2],
                SingletonMultiplicity::Unit,
                false,
            )
            .expect_err("empty recipe")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            prune_categorical_partition_singletons(
                &sources,
                &[&[1]],
                &[true; 2],
                SingletonMultiplicity::Unit,
                false,
            )
            .expect_err("bad source index")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            prune_categorical_partition_singletons(
                &[CategoricalCodeView::new(&short, 1)],
                &[&[0]],
                &[true; 2],
                SingletonMultiplicity::Unit,
                false,
            )
            .expect_err("row mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn every_adapter_reservation_and_capacity_guard_fails_closed() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let recipes: [&[usize]; 1] = [&[0, 1]];
        let mut successful = InjectedReservation::default();
        prune_categorical_partition_singletons_with_reservation(
            &sources,
            &recipes,
            &[true; 4],
            SingletonMultiplicity::Unit,
            false,
            &mut successful,
        )
        .expect("successful reservation audit");
        assert_eq!(successful.calls, 3);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = prune_categorical_partition_singletons_with_reservation(
                &sources,
                &recipes,
                &[true; 4],
                SingletonMultiplicity::Unit,
                false,
                &mut injected,
            )
            .expect_err("injected failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        for skip_at in 0..successful.calls {
            let mut undersized = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = prune_categorical_partition_singletons_with_reservation(
                &sources,
                &recipes,
                &[true; 4],
                SingletonMultiplicity::Unit,
                false,
                &mut undersized,
            )
            .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }
}
