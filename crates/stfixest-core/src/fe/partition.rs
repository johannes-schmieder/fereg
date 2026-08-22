//! Compact categorical partitions for fixed-effect topology sources.
//!
//! A single-source partition borrows its authoritative categorical codes and
//! owns no observation-scale copy. A genuine interaction owns exactly one
//! compact `u32` code per observation. Component indices are canonicalized so
//! reordered and repeated interaction recipes reuse the same partition key.

use crate::error::{ErrorCode, Result, StFixestError};

use super::{factorize_code_interaction, CategoricalCodeView, EncodedInteraction};

#[derive(Debug, Clone, PartialEq, Eq)]
enum CategoricalPartitionKey {
    Single(usize),
    Interaction(Vec<usize>),
}

/// One reusable categorical partition for an intercept-only FE topology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoricalPartition {
    key: CategoricalPartitionKey,
    owned_interaction: Option<EncodedInteraction>,
    nobs: usize,
    n_levels: usize,
}

impl CategoricalPartition {
    /// Construct a zero-copy partition over one authoritative categorical
    /// source.
    pub fn single(
        categorical_source_index: usize,
        source: CategoricalCodeView<'_>,
    ) -> Result<Self> {
        validate_source(source, "single categorical partition source")?;
        Ok(Self {
            key: CategoricalPartitionKey::Single(categorical_source_index),
            owned_interaction: None,
            nobs: source.codes().len(),
            n_levels: source.n_levels(),
        })
    }

    /// Construct a canonical interaction partition from source-table indices.
    ///
    /// Reordered components and duplicate occurrences have no effect on the
    /// induced partition. When canonicalization leaves one unique component,
    /// this returns the zero-copy single-source representation.
    pub fn interaction(
        categorical_source_indices: &[usize],
        sources: &[CategoricalCodeView<'_>],
    ) -> Result<Self> {
        let mut reservation = SystemPartitionReservation;
        Self::interaction_with_reservation(categorical_source_indices, sources, &mut reservation)
    }

    #[must_use]
    pub const fn nobs(&self) -> usize {
        self.nobs
    }

    #[must_use]
    pub const fn n_levels(&self) -> usize {
        self.n_levels
    }

    #[must_use]
    pub const fn is_interaction(&self) -> bool {
        matches!(self.key, CategoricalPartitionKey::Interaction(_))
    }

    /// Canonical unique source-table indices defining this partition.
    #[must_use]
    pub fn categorical_source_indices(&self) -> &[usize] {
        match &self.key {
            CategoricalPartitionKey::Single(index) => std::slice::from_ref(index),
            CategoricalPartitionKey::Interaction(indices) => indices,
        }
    }

    /// Borrow the authoritative code slice and revalidate all source-table
    /// identities and dimensions. Single-source output aliases the raw source;
    /// interaction output aliases this partition's sole owned N-vector.
    pub fn codes<'a>(&'a self, sources: &'a [CategoricalCodeView<'a>]) -> Result<&'a [u32]> {
        match &self.key {
            CategoricalPartitionKey::Single(index) => {
                let source = checked_source(sources, *index, "single categorical partition")?;
                validate_source(source, "single categorical partition source")?;
                if source.codes().len() != self.nobs || source.n_levels() != self.n_levels {
                    return Err(invalid(
                        "single categorical partition source changed after construction",
                    ));
                }
                Ok(source.codes())
            }
            CategoricalPartitionKey::Interaction(indices) => {
                validate_component_sources(indices, sources, self.nobs)?;
                let encoded = self.owned_interaction.as_ref().ok_or_else(|| {
                    invalid("interaction categorical partition lost its owned codes")
                })?;
                if encoded.codes().len() != self.nobs || encoded.n_levels() != self.n_levels {
                    return Err(invalid(
                        "interaction categorical partition dimensions changed after construction",
                    ));
                }
                Ok(encoded.codes())
            }
        }
    }

    fn interaction_with_reservation<R: PartitionReservation>(
        categorical_source_indices: &[usize],
        sources: &[CategoricalCodeView<'_>],
        reservation: &mut R,
    ) -> Result<Self> {
        if categorical_source_indices.len() < 2 {
            return Err(invalid(
                "categorical interaction partition requires at least two source indices",
            ));
        }

        let mut canonical_indices = Vec::new();
        reservation.reserve_vec(
            &mut canonical_indices,
            categorical_source_indices.len(),
            "unable to reserve canonical categorical interaction indices",
        )?;
        for index in categorical_source_indices.iter().copied() {
            checked_source(sources, index, "categorical interaction partition")?;
            push_reserved(
                &mut canonical_indices,
                index,
                "canonical categorical interaction index capacity exhausted",
            )?;
        }
        canonical_indices.sort_unstable();
        canonical_indices.dedup();

        if canonical_indices.len() == 1 {
            let index = canonical_indices[0];
            return Self::single(
                index,
                checked_source(
                    sources,
                    index,
                    "collapsed categorical interaction partition",
                )?,
            );
        }

        let nobs = checked_source(
            sources,
            canonical_indices[0],
            "categorical interaction partition",
        )?
        .codes()
        .len();
        validate_component_sources(&canonical_indices, sources, nobs)?;

        let mut components = Vec::new();
        reservation.reserve_vec(
            &mut components,
            canonical_indices.len(),
            "unable to reserve categorical interaction component views",
        )?;
        for index in canonical_indices.iter().copied() {
            push_reserved(
                &mut components,
                checked_source(sources, index, "categorical interaction partition")?,
                "categorical interaction component-view capacity exhausted",
            )?;
        }
        let encoded = factorize_code_interaction(&components)?;
        let n_levels = encoded.n_levels();
        if encoded.codes().len() != nobs {
            return Err(invalid(
                "categorical interaction factorizer returned a misaligned code vector",
            ));
        }

        Ok(Self {
            key: CategoricalPartitionKey::Interaction(canonical_indices),
            owned_interaction: Some(encoded),
            nobs,
            n_levels,
        })
    }
}

trait PartitionReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemPartitionReservation;

impl PartitionReservation for SystemPartitionReservation {
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

fn validate_component_sources(
    indices: &[usize],
    sources: &[CategoricalCodeView<'_>],
    nobs: usize,
) -> Result<()> {
    if indices.len() < 2 {
        return Err(invalid(
            "interaction categorical partition requires at least two unique sources",
        ));
    }
    for index in indices.iter().copied() {
        let source = checked_source(sources, index, "categorical interaction partition")?;
        validate_source(source, "categorical interaction partition source")?;
        if source.codes().len() != nobs {
            return Err(invalid(
                "categorical interaction partition sources have different observation counts",
            ));
        }
    }
    Ok(())
}

fn validate_source(source: CategoricalCodeView<'_>, label: &str) -> Result<()> {
    let nobs = source.codes().len();
    if nobs == 0 {
        if source.n_levels() != 0 {
            return Err(invalid(format!(
                "empty {label} declares {} levels",
                source.n_levels()
            )));
        }
        return Ok(());
    }
    if source.n_levels() == 0 {
        return Err(invalid(format!("nonempty {label} declares zero levels")));
    }
    if source.n_levels() > u32::MAX as usize {
        return Err(invalid(format!("{label} level count exceeds u32::MAX")));
    }
    if source
        .codes()
        .iter()
        .any(|code| *code as usize >= source.n_levels())
    {
        return Err(invalid_data(format!(
            "{label} contains a code outside its declared level range"
        )));
    }
    Ok(())
}

fn checked_source<'a>(
    sources: &'a [CategoricalCodeView<'a>],
    index: usize,
    label: &str,
) -> Result<CategoricalCodeView<'a>> {
    sources
        .get(index)
        .copied()
        .ok_or_else(|| invalid(format!("{label} source index is out of range")))
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
        calls: usize,
        fail_at: Option<usize>,
        skip_label: Option<&'static str>,
    }

    impl PartitionReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected categorical-partition allocation failure",
                ));
            }
            if self.skip_label == Some(label) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test categorical-partition reservation failed"))
        }
    }

    #[test]
    fn single_partition_borrows_authoritative_codes() {
        let raw = [0, 1, 0, 2];
        let sources = [CategoricalCodeView::new(&raw, 3)];
        let partition = CategoricalPartition::single(0, sources[0]).expect("single partition");
        let codes = partition.codes(&sources).expect("borrowed codes");
        assert!(!partition.is_interaction());
        assert_eq!(partition.categorical_source_indices(), &[0]);
        assert_eq!(partition.nobs(), raw.len());
        assert_eq!(partition.n_levels(), 3);
        assert!(std::ptr::eq(codes.as_ptr(), raw.as_ptr()));
    }

    #[test]
    fn reordered_interactions_share_a_canonical_partition() {
        let firm = [2, 0, 2, 1, 0, 2, 1];
        let year = [1, 2, 1, 0, 2, 0, 0];
        let sources = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&year, 3),
        ];
        let forward =
            CategoricalPartition::interaction(&[0, 1], &sources).expect("forward partition");
        let reversed =
            CategoricalPartition::interaction(&[1, 0], &sources).expect("reversed partition");
        assert_eq!(forward, reversed);
        assert!(forward.is_interaction());
        assert_eq!(forward.categorical_source_indices(), &[0, 1]);
        assert_eq!(forward.n_levels(), 4);
        assert_eq!(
            forward.codes(&sources).expect("forward codes"),
            &[3, 0, 3, 1, 0, 2, 1]
        );
    }

    #[test]
    fn duplicate_only_interaction_collapses_to_zero_copy_single_source() {
        let raw = [0, 1, 0, 2];
        let sources = [CategoricalCodeView::new(&raw, 3)];
        let partition =
            CategoricalPartition::interaction(&[0, 0, 0], &sources).expect("collapsed partition");
        assert!(!partition.is_interaction());
        assert_eq!(partition.categorical_source_indices(), &[0]);
        assert!(std::ptr::eq(
            partition.codes(&sources).expect("borrowed codes").as_ptr(),
            raw.as_ptr()
        ));
    }

    #[test]
    fn duplicate_components_do_not_change_a_genuine_interaction() {
        let left = [0, 0, 1, 1];
        let right = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&left, 2),
            CategoricalCodeView::new(&right, 2),
        ];
        let plain = CategoricalPartition::interaction(&[0, 1], &sources).expect("plain partition");
        let repeated =
            CategoricalPartition::interaction(&[1, 0, 1, 0], &sources).expect("repeated partition");
        assert_eq!(plain, repeated);
    }

    #[test]
    fn malformed_source_tables_fail_closed() {
        let valid = [0, 1];
        let invalid = [0, 2];
        let sources = [
            CategoricalCodeView::new(&valid, 2),
            CategoricalCodeView::new(&invalid, 2),
        ];
        assert_eq!(
            CategoricalPartition::interaction(&[0], &sources)
                .expect_err("one source")
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            CategoricalPartition::interaction(&[0, 2], &sources)
                .expect_err("bad index")
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            CategoricalPartition::interaction(&[0, 1], &sources)
                .expect_err("bad code")
                .code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            CategoricalPartition::single(0, CategoricalCodeView::new(&[], 1))
                .expect_err("empty source with levels")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn source_mutation_is_detected_when_codes_are_borrowed() {
        let raw = [0, 1, 0, 1];
        let original = [CategoricalCodeView::new(&raw, 2)];
        let partition = CategoricalPartition::single(0, original[0]).expect("partition");
        let changed_levels = [CategoricalCodeView::new(&raw, 3)];
        assert_eq!(
            partition
                .codes(&changed_levels)
                .expect_err("changed level count")
                .code,
            ErrorCode::InvalidArgument
        );
        let short = [0, 1];
        let changed_rows = [CategoricalCodeView::new(&short, 2)];
        assert_eq!(
            partition
                .codes(&changed_rows)
                .expect_err("changed row count")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn every_partition_reservation_and_capacity_guard_fails_closed() {
        let left = [0, 0, 1, 1];
        let right = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&left, 2),
            CategoricalCodeView::new(&right, 2),
        ];
        let mut successful = InjectedReservation::default();
        CategoricalPartition::interaction_with_reservation(&[0, 1], &sources, &mut successful)
            .expect("successful reservation audit");
        assert_eq!(successful.calls, 2);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = CategoricalPartition::interaction_with_reservation(
                &[0, 1],
                &sources,
                &mut injected,
            )
            .expect_err("injected allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        for label in [
            "unable to reserve canonical categorical interaction indices",
            "unable to reserve categorical interaction component views",
        ] {
            let mut undersized = InjectedReservation {
                skip_label: Some(label),
                ..InjectedReservation::default()
            };
            let error = CategoricalPartition::interaction_with_reservation(
                &[0, 1],
                &sources,
                &mut undersized,
            )
            .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "{label}");
        }
    }
}
