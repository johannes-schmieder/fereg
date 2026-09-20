//! Certified categorical topology sources for intercept-only fixed effects.
//!
//! This layer joins a categorical partition with one compressed level-membership
//! index. Single-source partitions continue to borrow their authoritative codes;
//! interaction partitions retain exactly one owned `u32` code per observation.
//! The resulting source can be structurally certified once and then borrowed by
//! MAP, DoF, singleton, and cluster-nesting consumers without rebuilding or
//! copying observation-scale state.

use crate::error::{ErrorCode, FeregError, Result};

use super::design::{LevelIndex, ValidatedBorrowedInterceptSource};
use super::partition::CategoricalPartition;
use super::partition_validation::validate_categorical_partition;
use super::CategoricalCodeView;

/// One reusable categorical partition plus compressed level membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoricalTopologySource {
    partition: CategoricalPartition,
    level_index: LevelIndex,
}

impl CategoricalTopologySource {
    pub(crate) fn single_with_index(
        categorical_source_index: usize,
        source: CategoricalCodeView<'_>,
        level_index: LevelIndex,
    ) -> Result<Self> {
        let partition = CategoricalPartition::single(categorical_source_index, source)?;
        ValidatedBorrowedInterceptSource::new(source.codes(), source.n_levels(), &level_index)?;
        Ok(Self { partition, level_index })
    }

    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        self.partition.owned_buffer_bytes() + self.level_index.owned_buffer_bytes()
    }

    /// Construct a topology source that borrows one authoritative code column.
    pub fn single(
        categorical_source_index: usize,
        sources: &[CategoricalCodeView<'_>],
    ) -> Result<Self> {
        let source = sources
            .get(categorical_source_index)
            .copied()
            .ok_or_else(|| invalid("categorical topology source index is out of range"))?;
        let partition = CategoricalPartition::single(categorical_source_index, source)?;
        Self::from_partition(partition, sources)
    }

    /// Construct a topology source for a canonical categorical interaction.
    pub fn interaction(
        categorical_source_indices: &[usize],
        sources: &[CategoricalCodeView<'_>],
    ) -> Result<Self> {
        let partition = CategoricalPartition::interaction(categorical_source_indices, sources)?;
        Self::from_partition(partition, sources)
    }

    /// Attach compressed membership to an already constructed partition.
    pub fn from_partition(
        partition: CategoricalPartition,
        sources: &[CategoricalCodeView<'_>],
    ) -> Result<Self> {
        validate_categorical_partition(&partition, sources)?;
        let codes = partition.codes(sources)?;
        let level_index = match LevelIndex::try_from_ordered_codes(codes, partition.n_levels())? {
            Some(index) => index,
            None => LevelIndex::from_codes(codes, partition.n_levels())?,
        };
        ValidatedBorrowedInterceptSource::new(codes, partition.n_levels(), &level_index)?;
        Ok(Self {
            partition,
            level_index,
        })
    }

    #[must_use]
    pub const fn partition(&self) -> &CategoricalPartition {
        &self.partition
    }

    #[must_use]
    pub const fn nobs(&self) -> usize {
        self.partition.nobs()
    }

    #[must_use]
    pub const fn n_levels(&self) -> usize {
        self.partition.n_levels()
    }

    #[must_use]
    pub const fn level_index(&self) -> &LevelIndex {
        &self.level_index
    }

    pub fn members(&self, level: usize) -> Result<&[usize]> {
        self.level_index.checked_members(level)
    }

    /// Borrow the authoritative or owned compact code vector.
    pub fn codes<'a>(&'a self, sources: &[CategoricalCodeView<'a>]) -> Result<&'a [u32]> {
        self.partition.codes(sources)
    }

    /// Revalidate exact component identity, code ranges, compact membership,
    /// and full observation coverage before constructing solver views.
    pub(crate) fn validated_borrowed<'a>(
        &'a self,
        sources: &[CategoricalCodeView<'a>],
    ) -> Result<ValidatedBorrowedInterceptSource<'a>> {
        validate_categorical_partition(&self.partition, sources)?;
        let codes = self.partition.codes(sources)?;
        ValidatedBorrowedInterceptSource::new(codes, self.n_levels(), &self.level_index)
    }
}

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_topology_source_borrows_codes_and_builds_exact_membership() {
        let raw = [2, 0, 1, 2, 0, 1];
        let sources = [CategoricalCodeView::new(&raw, 3)];
        let topology = CategoricalTopologySource::single(0, &sources).expect("topology");
        let codes = topology.codes(&sources).expect("codes");
        assert!(std::ptr::eq(codes.as_ptr(), raw.as_ptr()));
        assert_eq!(topology.nobs(), raw.len());
        assert_eq!(topology.n_levels(), 3);
        assert_eq!(topology.members(0).expect("level 0"), &[1, 4]);
        assert_eq!(topology.members(1).expect("level 1"), &[2, 5]);
        assert_eq!(topology.members(2).expect("level 2"), &[0, 3]);
        topology
            .validated_borrowed(&sources)
            .expect("borrowed certificate");
    }

    #[test]
    fn interaction_topology_owns_one_compact_partition_and_exact_membership() {
        let firm = [2, 0, 2, 1, 0, 2, 1];
        let year = [1, 2, 1, 0, 2, 0, 0];
        let sources = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&year, 3),
        ];
        let topology = CategoricalTopologySource::interaction(&[1, 0], &sources).expect("topology");
        assert!(topology.partition().is_interaction());
        assert_eq!(topology.partition().categorical_source_indices(), &[0, 1]);
        assert_eq!(
            topology.codes(&sources).expect("codes"),
            &[3, 0, 3, 1, 0, 2, 1]
        );
        assert_eq!(topology.members(0).expect("level 0"), &[1, 4]);
        assert_eq!(topology.members(1).expect("level 1"), &[3, 6]);
        assert_eq!(topology.members(2).expect("level 2"), &[5]);
        assert_eq!(topology.members(3).expect("level 3"), &[0, 2]);
        topology
            .validated_borrowed(&sources)
            .expect("borrowed certificate");
    }

    #[test]
    fn changed_interaction_component_fails_before_solver_view() {
        let firm = [0, 0, 1, 1, 2, 2];
        let year = [0, 1, 0, 1, 0, 1];
        let original = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&year, 2),
        ];
        let topology =
            CategoricalTopologySource::interaction(&[0, 1], &original).expect("topology");

        let changed_year = [1, 0, 0, 1, 0, 1];
        let changed = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&changed_year, 2),
        ];
        assert_eq!(
            topology
                .validated_borrowed(&changed)
                .expect_err("changed component")
                .code,
            ErrorCode::InvalidData
        );
    }

    #[test]
    fn changed_single_source_fails_membership_certificate() {
        let raw = [0, 0, 1, 1, 2, 2];
        let original = [CategoricalCodeView::new(&raw, 3)];
        let topology = CategoricalTopologySource::single(0, &original).expect("topology");

        let changed = [0, 1, 0, 1, 2, 2];
        let changed_sources = [CategoricalCodeView::new(&changed, 3)];
        assert_eq!(
            topology
                .validated_borrowed(&changed_sources)
                .expect_err("changed raw source")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn unused_declared_level_is_rejected_at_topology_boundary() {
        let raw = [0, 0, 2, 2];
        let sources = [CategoricalCodeView::new(&raw, 3)];
        assert_eq!(
            CategoricalTopologySource::single(0, &sources)
                .expect_err("unused level")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn duplicate_only_interaction_remains_zero_copy() {
        let raw = [0, 1, 0, 2];
        let sources = [CategoricalCodeView::new(&raw, 3)];
        let topology =
            CategoricalTopologySource::interaction(&[0, 0], &sources).expect("collapsed topology");
        assert!(!topology.partition().is_interaction());
        assert!(std::ptr::eq(
            topology.codes(&sources).expect("codes").as_ptr(),
            raw.as_ptr()
        ));
    }

    #[test]
    fn malformed_source_index_fails_closed() {
        let raw = [0, 1];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        assert_eq!(
            CategoricalTopologySource::single(1, &sources)
                .expect_err("bad source")
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            CategoricalTopologySource::interaction(&[0, 1], &sources)
                .expect_err("bad interaction source")
                .code,
            ErrorCode::InvalidArgument
        );
    }
}
