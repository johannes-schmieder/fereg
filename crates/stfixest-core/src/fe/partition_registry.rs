//! Deduplicated categorical partitions for candidate-sample operations.
//!
//! Singleton pruning needs compact codes but does not need the final solver
//! membership index. This registry therefore retains zero-copy single-source
//! partitions and one `u32` vector per unique interaction, without also paying
//! for a second observation-scale `usize` membership array. Final topology
//! membership is constructed only after the singleton fixed point is known.

use crate::error::{ErrorCode, Result, StFixestError};

use super::{validate_categorical_partition, CategoricalCodeView, CategoricalPartition};

/// Unique candidate-sample categorical partitions in insertion order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CategoricalPartitionRegistry {
    partitions: Vec<CategoricalPartition>,
}

impl CategoricalPartitionRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            partitions: Vec::new(),
        }
    }

    pub fn intern(
        &mut self,
        categorical_source_indices: &[usize],
        source_table: &[CategoricalCodeView<'_>],
    ) -> Result<usize> {
        let mut reservation = SystemPartitionRegistryReservation;
        self.intern_with_reservation(categorical_source_indices, source_table, &mut reservation)
    }

    #[must_use]
    pub fn partitions(&self) -> &[CategoricalPartition] {
        &self.partitions
    }

    pub fn partition(&self, index: usize) -> Result<&CategoricalPartition> {
        self.partitions
            .get(index)
            .ok_or_else(|| invalid("categorical partition registry index is out of range"))
    }

    pub fn total_levels(&self) -> Result<usize> {
        self.partitions
            .iter()
            .try_fold(0_usize, |total, partition| {
                total.checked_add(partition.n_levels()).ok_or_else(|| {
                    allocation("candidate categorical partition total level count overflows")
                })
            })
    }

    pub fn validate_all(&self, source_table: &[CategoricalCodeView<'_>]) -> Result<()> {
        for partition in &self.partitions {
            validate_categorical_partition(partition, source_table)?;
        }
        Ok(())
    }

    fn intern_with_reservation<R: PartitionRegistryReservation>(
        &mut self,
        categorical_source_indices: &[usize],
        source_table: &[CategoricalCodeView<'_>],
        reservation: &mut R,
    ) -> Result<usize> {
        if categorical_source_indices.is_empty() {
            return Err(invalid(
                "categorical partition recipe requires at least one source index",
            ));
        }

        let mut canonical = Vec::new();
        reservation.reserve_vec(
            &mut canonical,
            categorical_source_indices.len(),
            "unable to reserve canonical categorical partition recipe",
        )?;
        for index in categorical_source_indices.iter().copied() {
            if index >= source_table.len() {
                return Err(invalid(
                    "categorical partition recipe source index is out of range",
                ));
            }
            push_reserved(
                &mut canonical,
                index,
                "canonical categorical partition recipe capacity exhausted",
            )?;
        }
        canonical.sort_unstable();
        canonical.dedup();

        if let Some(existing) = self
            .partitions
            .iter()
            .position(|partition| partition.categorical_source_indices() == canonical.as_slice())
        {
            return Ok(existing);
        }

        let partition = if canonical.len() == 1 {
            CategoricalPartition::single(canonical[0], source_table[canonical[0]])?
        } else {
            CategoricalPartition::interaction(&canonical, source_table)?
        };
        reservation.reserve_vec(
            &mut self.partitions,
            1,
            "unable to reserve candidate categorical partition",
        )?;
        let index = self.partitions.len();
        push_reserved(
            &mut self.partitions,
            partition,
            "candidate categorical partition capacity exhausted",
        )?;
        Ok(index)
    }
}

trait PartitionRegistryReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemPartitionRegistryReservation;

impl PartitionRegistryReservation for SystemPartitionRegistryReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve(additional)
            .map_err(|_| allocation(label))
    }
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

    impl PartitionRegistryReservation for InjectedReservation {
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
                    "injected candidate partition registry allocation failure",
                ));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve(additional)
                .map_err(|_| allocation("test candidate partition reservation failed"))
        }
    }

    #[test]
    fn reordered_and_repeated_recipes_share_one_partition() {
        let firm = [0, 0, 1, 1, 2, 2];
        let year = [0, 1, 0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&year, 2),
        ];
        let mut registry = CategoricalPartitionRegistry::new();
        let first = registry.intern(&[0, 1], &sources).expect("first");
        assert_eq!(registry.intern(&[1, 0], &sources).expect("reverse"), first);
        assert_eq!(
            registry.intern(&[0, 1, 0, 1], &sources).expect("repeated"),
            first
        );
        assert_eq!(registry.partitions().len(), 1);
        assert_eq!(registry.total_levels().expect("levels"), 6);
        registry.validate_all(&sources).expect("validation");
    }

    #[test]
    fn duplicate_only_recipe_reuses_borrowed_single_source() {
        let raw = [0, 1, 0, 2];
        let sources = [CategoricalCodeView::new(&raw, 3)];
        let mut registry = CategoricalPartitionRegistry::new();
        let single = registry.intern(&[0], &sources).expect("single");
        let duplicate = registry.intern(&[0, 0], &sources).expect("duplicate");
        assert_eq!(single, duplicate);
        assert!(std::ptr::eq(
            registry
                .partition(single)
                .expect("partition")
                .codes(&sources)
                .expect("codes")
                .as_ptr(),
            raw.as_ptr()
        ));
    }

    #[test]
    fn invalid_recipe_does_not_publish_partial_partition() {
        let raw = [0, 1];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        let mut registry = CategoricalPartitionRegistry::new();
        assert_eq!(
            registry
                .intern(&[], &sources)
                .expect_err("empty recipe")
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            registry
                .intern(&[1], &sources)
                .expect_err("bad source")
                .code,
            ErrorCode::InvalidArgument
        );
        assert!(registry.partitions().is_empty());
    }

    #[test]
    fn source_mutation_is_detected_before_candidate_use() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let original = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let mut registry = CategoricalPartitionRegistry::new();
        registry.intern(&[0, 1], &original).expect("partition");

        let changed_year = [1, 0, 0, 1];
        let changed = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&changed_year, 2),
        ];
        assert_eq!(
            registry
                .validate_all(&changed)
                .expect_err("changed source")
                .code,
            ErrorCode::InvalidData
        );
    }

    #[test]
    fn every_registry_reservation_and_capacity_guard_fails_closed() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let mut success_registry = CategoricalPartitionRegistry::new();
        let mut success = InjectedReservation::default();
        success_registry
            .intern_with_reservation(&[0, 1], &sources, &mut success)
            .expect("successful audit");
        assert_eq!(success.calls, 2);

        for fail_at in 0..success.calls {
            let mut registry = CategoricalPartitionRegistry::new();
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = registry
                .intern_with_reservation(&[0, 1], &sources, &mut injected)
                .expect_err("injected failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
            assert!(registry.partitions().is_empty());
        }

        for skip_at in 0..success.calls {
            let mut registry = CategoricalPartitionRegistry::new();
            let mut undersized = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = registry
                .intern_with_reservation(&[0, 1], &sources, &mut undersized)
                .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
            assert!(registry.partitions().is_empty());
        }
    }
}
