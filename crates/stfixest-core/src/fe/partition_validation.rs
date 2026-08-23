//! Exact structural validation for prepared categorical partitions.
//!
//! Interaction partitions own derived observation codes. Before those codes are
//! admitted to singleton, MAP, DoF, or cluster-nesting kernels, this module
//! re-derives the partition once from its authoritative component sources and
//! checks exact equality. Numerical kernels may then borrow the certified codes
//! without paying this O(A N log N) validation cost on every iteration.

use crate::error::{ErrorCode, Result, StFixestError};

use super::{factorize_code_interaction, CategoricalCodeView, CategoricalPartition};

/// Validate that a categorical partition still represents its authoritative
/// source table exactly.
///
/// A single-source partition treats that source as authoritative and performs
/// the partition's bounded code/range/dimension checks. A genuine interaction
/// is re-factorized and must match both retained level count and every owned
/// observation code exactly.
pub fn validate_categorical_partition(
    partition: &CategoricalPartition,
    sources: &[CategoricalCodeView<'_>],
) -> Result<()> {
    let mut reservation = SystemPartitionValidationReservation;
    validate_categorical_partition_with_reservation(partition, sources, &mut reservation)
}

trait PartitionValidationReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemPartitionValidationReservation;

impl PartitionValidationReservation for SystemPartitionValidationReservation {
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

fn validate_categorical_partition_with_reservation<R: PartitionValidationReservation>(
    partition: &CategoricalPartition,
    sources: &[CategoricalCodeView<'_>],
    reservation: &mut R,
) -> Result<()> {
    let retained_codes = partition.codes(sources)?;
    if !partition.is_interaction() {
        return Ok(());
    }

    let source_indices = partition.categorical_source_indices();
    if source_indices.len() < 2 {
        return Err(invalid(
            "interaction categorical partition has fewer than two canonical sources",
        ));
    }
    let mut components = Vec::new();
    reservation.reserve_vec(
        &mut components,
        source_indices.len(),
        "unable to reserve exact categorical-partition validation views",
    )?;
    for source_index in source_indices.iter().copied() {
        let source = sources.get(source_index).copied().ok_or_else(|| {
            invalid("exact categorical-partition validation source index is out of range")
        })?;
        push_reserved(
            &mut components,
            source,
            "exact categorical-partition validation view capacity exhausted",
        )?;
    }

    let expected = factorize_code_interaction(&components)?;
    if expected.n_levels() != partition.n_levels() || expected.codes() != retained_codes {
        return Err(invalid_data(
            "categorical interaction partition no longer matches its authoritative sources",
        ));
    }
    Ok(())
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

fn invalid_data(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidData, message)
}

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct InjectedReservation {
        fail: bool,
        skip: bool,
        calls: usize,
    }

    impl PartitionValidationReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            self.calls += 1;
            if self.fail {
                return Err(allocation(
                    "injected exact categorical-partition validation failure",
                ));
            }
            if self.skip {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test exact-partition reservation failed"))
        }
    }

    #[test]
    fn exact_interaction_validation_accepts_unchanged_sources() {
        let firm = [2, 0, 2, 1, 0, 2, 1];
        let year = [1, 2, 1, 0, 2, 0, 0];
        let sources = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&year, 3),
        ];
        let partition = CategoricalPartition::interaction(&[1, 0], &sources).expect("partition");
        validate_categorical_partition(&partition, &sources).expect("exact validation");
    }

    #[test]
    fn same_dimension_component_mutation_is_detected() {
        let firm = [0, 0, 1, 1, 2, 2];
        let year = [0, 1, 0, 1, 0, 1];
        let original = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&year, 2),
        ];
        let partition = CategoricalPartition::interaction(&[0, 1], &original).expect("partition");

        let changed_year = [1, 0, 0, 1, 0, 1];
        let changed = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&changed_year, 2),
        ];
        let error = validate_categorical_partition(&partition, &changed)
            .expect_err("source mutation must fail");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn single_source_remains_authoritative_and_zero_copy() {
        let raw = [0, 1, 0, 2];
        let original = [CategoricalCodeView::new(&raw, 3)];
        let partition = CategoricalPartition::single(0, original[0]).expect("single partition");
        validate_categorical_partition(&partition, &original).expect("single validation");

        let changed = [2, 1, 0, 2];
        let changed_sources = [CategoricalCodeView::new(&changed, 3)];
        validate_categorical_partition(&partition, &changed_sources)
            .expect("single raw source is authoritative");
        assert!(std::ptr::eq(
            partition
                .codes(&changed_sources)
                .expect("borrowed changed codes")
                .as_ptr(),
            changed.as_ptr()
        ));
    }

    #[test]
    fn validation_workspace_failure_and_capacity_guard_fail_closed() {
        let left = [0, 0, 1, 1];
        let right = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&left, 2),
            CategoricalCodeView::new(&right, 2),
        ];
        let partition = CategoricalPartition::interaction(&[0, 1], &sources).expect("partition");

        let mut success = InjectedReservation::default();
        validate_categorical_partition_with_reservation(&partition, &sources, &mut success)
            .expect("successful validation");
        assert_eq!(success.calls, 1);

        let mut failed = InjectedReservation {
            fail: true,
            ..InjectedReservation::default()
        };
        assert_eq!(
            validate_categorical_partition_with_reservation(&partition, &sources, &mut failed)
                .expect_err("injected failure")
                .code,
            ErrorCode::Allocation
        );

        let mut undersized = InjectedReservation {
            skip: true,
            ..InjectedReservation::default()
        };
        assert_eq!(
            validate_categorical_partition_with_reservation(&partition, &sources, &mut undersized)
                .expect_err("capacity guard")
                .code,
            ErrorCode::Allocation
        );
    }
}
