//! Deduplicated registry of categorical fixed-effect topology sources.
//!
//! Recipes are canonicalized by sorted unique categorical-source indices before
//! lookup. Repeated terms, reordered interactions, and duplicate components
//! therefore reuse one topology source. A unique interaction pays for exactly
//! one derived `u32` code vector and one compressed membership index.

use crate::error::{ErrorCode, FeregError, Result};

use super::{CategoricalCodeView, CategoricalTopologySource};

/// Reusable unique categorical topology sources in deterministic insertion order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CategoricalTopologyRegistry {
    sources: Vec<CategoricalTopologySource>,
}

impl CategoricalTopologyRegistry {
    pub(crate) fn from_single_source(source: CategoricalTopologySource) -> Result<Self> {
        let mut sources = Vec::new();
        sources.try_reserve_exact(1)
            .map_err(|_| allocation("unable to reserve transferred categorical topology source"))?;
        sources.push(source);
        Ok(Self { sources })
    }

    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(&self.sources)
            + self.sources.iter().map(CategoricalTopologySource::owned_buffer_bytes).sum::<u128>()
    }

    #[must_use]
    pub const fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Intern one single-source or interaction recipe and return its stable
    /// registry index.
    pub fn intern(
        &mut self,
        categorical_source_indices: &[usize],
        source_table: &[CategoricalCodeView<'_>],
    ) -> Result<usize> {
        let mut reservation = SystemTopologyRegistryReservation;
        self.intern_with_reservation(categorical_source_indices, source_table, &mut reservation)
    }

    #[must_use]
    pub fn sources(&self) -> &[CategoricalTopologySource] {
        &self.sources
    }

    pub fn source(&self, index: usize) -> Result<&CategoricalTopologySource> {
        self.sources
            .get(index)
            .ok_or_else(|| invalid("categorical topology registry index is out of range"))
    }

    pub fn total_levels(&self) -> Result<usize> {
        self.sources.iter().try_fold(0_usize, |total, source| {
            total
                .checked_add(source.n_levels())
                .ok_or_else(|| allocation("categorical topology total level count overflows"))
        })
    }

    /// Revalidate every unique source once before constructing numerical views.
    pub fn validate_all(&self, source_table: &[CategoricalCodeView<'_>]) -> Result<()> {
        for source in &self.sources {
            source.validated_borrowed(source_table)?;
        }
        Ok(())
    }

    fn intern_with_reservation<R: TopologyRegistryReservation>(
        &mut self,
        categorical_source_indices: &[usize],
        source_table: &[CategoricalCodeView<'_>],
        reservation: &mut R,
    ) -> Result<usize> {
        if categorical_source_indices.is_empty() {
            return Err(invalid(
                "categorical topology recipe requires at least one source index",
            ));
        }

        let mut canonical = Vec::new();
        reservation.reserve_vec(
            &mut canonical,
            categorical_source_indices.len(),
            "unable to reserve canonical categorical topology recipe",
        )?;
        for index in categorical_source_indices.iter().copied() {
            if index >= source_table.len() {
                return Err(invalid(
                    "categorical topology recipe source index is out of range",
                ));
            }
            push_reserved(
                &mut canonical,
                index,
                "canonical categorical topology recipe capacity exhausted",
            )?;
        }
        canonical.sort_unstable();
        canonical.dedup();

        if let Some(existing) = self.sources.iter().position(|source| {
            source.partition().categorical_source_indices() == canonical.as_slice()
        }) {
            return Ok(existing);
        }

        let topology = if canonical.len() == 1 {
            CategoricalTopologySource::single(canonical[0], source_table)?
        } else {
            CategoricalTopologySource::interaction(&canonical, source_table)?
        };
        reservation.reserve_vec(
            &mut self.sources,
            1,
            "unable to reserve categorical topology registry source",
        )?;
        let index = self.sources.len();
        push_reserved(
            &mut self.sources,
            topology,
            "categorical topology registry source capacity exhausted",
        )?;
        Ok(index)
    }
}

trait TopologyRegistryReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemTopologyRegistryReservation;

impl TopologyRegistryReservation for SystemTopologyRegistryReservation {
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

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
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

    impl TopologyRegistryReservation for InjectedReservation {
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
                    "injected categorical topology registry allocation failure",
                ));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve(additional)
                .map_err(|_| allocation("test topology registry reservation failed"))
        }
    }

    #[test]
    fn repeated_and_reordered_interactions_reuse_one_source() {
        let firm = [0, 0, 1, 1, 2, 2];
        let year = [0, 1, 0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&year, 2),
        ];
        let mut registry = CategoricalTopologyRegistry::new();
        let first = registry.intern(&[0, 1], &sources).expect("first");
        let reversed = registry.intern(&[1, 0], &sources).expect("reversed");
        let repeated = registry.intern(&[0, 1, 0, 1], &sources).expect("repeated");
        assert_eq!(first, reversed);
        assert_eq!(first, repeated);
        assert_eq!(registry.sources().len(), 1);
        assert_eq!(registry.total_levels().expect("levels"), 6);
        registry.validate_all(&sources).expect("validation");
    }

    #[test]
    fn duplicate_only_recipe_reuses_zero_copy_single_source() {
        let firm = [0, 1, 0, 2];
        let sources = [CategoricalCodeView::new(&firm, 3)];
        let mut registry = CategoricalTopologyRegistry::new();
        let single = registry.intern(&[0], &sources).expect("single");
        let duplicate = registry.intern(&[0, 0, 0], &sources).expect("duplicate");
        assert_eq!(single, duplicate);
        assert_eq!(registry.sources().len(), 1);
        let codes = registry
            .source(single)
            .expect("source")
            .codes(&sources)
            .expect("codes");
        assert!(std::ptr::eq(codes.as_ptr(), firm.as_ptr()));
    }

    #[test]
    fn distinct_partitions_keep_deterministic_insertion_order() {
        let first = [0, 0, 1, 1];
        let second = [0, 1, 0, 1];
        let third = [0, 1, 2, 0];
        let sources = [
            CategoricalCodeView::new(&first, 2),
            CategoricalCodeView::new(&second, 2),
            CategoricalCodeView::new(&third, 3),
        ];
        let mut registry = CategoricalTopologyRegistry::new();
        assert_eq!(registry.intern(&[1], &sources).expect("second"), 0);
        assert_eq!(registry.intern(&[0, 1], &sources).expect("pair"), 1);
        assert_eq!(registry.intern(&[2], &sources).expect("third"), 2);
        assert_eq!(registry.sources().len(), 3);
        assert_eq!(registry.total_levels().expect("levels"), 2 + 4 + 3);
    }

    #[test]
    fn invalid_recipes_fail_without_mutating_registry() {
        let raw = [0, 1];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        let mut registry = CategoricalTopologyRegistry::new();
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
        assert!(registry.sources().is_empty());
    }

    #[test]
    fn changed_component_is_detected_by_registry_validation() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let original = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let mut registry = CategoricalTopologyRegistry::new();
        registry.intern(&[0, 1], &original).expect("interaction");

        let changed_year = [1, 0, 0, 1];
        let changed = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&changed_year, 2),
        ];
        assert_eq!(
            registry
                .validate_all(&changed)
                .expect_err("changed component")
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
        let mut successful_registry = CategoricalTopologyRegistry::new();
        let mut successful = InjectedReservation::default();
        successful_registry
            .intern_with_reservation(&[0, 1], &sources, &mut successful)
            .expect("successful audit");
        assert_eq!(successful.calls, 2);

        for fail_at in 0..successful.calls {
            let mut registry = CategoricalTopologyRegistry::new();
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = registry
                .intern_with_reservation(&[0, 1], &sources, &mut injected)
                .expect_err("injected failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
            assert!(registry.sources().is_empty());
        }

        for skip_at in 0..successful.calls {
            let mut registry = CategoricalTopologyRegistry::new();
            let mut undersized = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = registry
                .intern_with_reservation(&[0, 1], &sources, &mut undersized)
                .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
            assert!(registry.sources().is_empty());
        }
    }
}
