//! Borrowed MAP adapter for deduplicated categorical topology sources.
//!
//! Each unique topology source is structurally validated exactly once. Wire-term
//! order is then reconstructed from a small effect-to-topology map, reusing the
//! same borrowed code and membership pointers for repeated terms. The adapter
//! allocates only O(U + G) small tables for U unique sources and G requested
//! effects; it never copies observation-scale codes or membership.

use crate::error::{ErrorCode, FeregError, Result};

use super::design::{
    BorrowedInterceptEffect, BorrowedInterceptFeDesign, ValidatedBorrowedInterceptSource,
};
use super::{CategoricalCodeView, CategoricalTopologyRegistry};

/// Build a validated borrowed intercept-only FE design from a deduplicated
/// categorical topology registry.
///
/// `effect_to_topology` is in requested term order and may repeat topology
/// indices. Every unique registry source is scanned once before any term view is
/// published. Repeated terms are O(1) pointer copies.
pub fn borrow_categorical_topology_design<'a>(
    nobs: usize,
    registry: &'a CategoricalTopologyRegistry,
    source_table: &'a [CategoricalCodeView<'a>],
    effect_to_topology: &[usize],
) -> Result<BorrowedInterceptFeDesign<'a>> {
    let mut reservation = SystemTopologyAdapterReservation;
    borrow_categorical_topology_design_with_reservation(
        nobs,
        registry,
        source_table,
        effect_to_topology,
        &mut reservation,
    )
}

trait TopologyAdapterReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemTopologyAdapterReservation;

impl TopologyAdapterReservation for SystemTopologyAdapterReservation {
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

fn borrow_categorical_topology_design_with_reservation<'a, R: TopologyAdapterReservation>(
    nobs: usize,
    registry: &'a CategoricalTopologyRegistry,
    source_table: &'a [CategoricalCodeView<'a>],
    effect_to_topology: &[usize],
    reservation: &mut R,
) -> Result<BorrowedInterceptFeDesign<'a>> {
    let mut validated_sources: Vec<ValidatedBorrowedInterceptSource<'a>> = Vec::new();
    reservation.reserve_vec(
        &mut validated_sources,
        registry.sources().len(),
        "unable to reserve validated categorical topology sources",
    )?;
    for source in registry.sources() {
        if source.nobs() != nobs {
            return Err(invalid(
                "categorical topology source observation count differs from design",
            ));
        }
        push_reserved(
            &mut validated_sources,
            source.validated_borrowed(source_table)?,
            "validated categorical topology source capacity exhausted",
        )?;
    }

    let mut effects: Vec<BorrowedInterceptEffect<'a>> = Vec::new();
    reservation.reserve_vec(
        &mut effects,
        effect_to_topology.len(),
        "unable to reserve categorical topology term views",
    )?;
    for topology_index in effect_to_topology.iter().copied() {
        let source = validated_sources
            .get(topology_index)
            .ok_or_else(|| invalid("categorical effect-to-topology index is out of range"))?;
        push_reserved(
            &mut effects,
            source.effect(),
            "categorical topology term-view capacity exhausted",
        )?;
    }

    BorrowedInterceptFeDesign::new(nobs, effects)
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

    impl TopologyAdapterReservation for InjectedReservation {
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
                    "injected categorical topology adapter allocation failure",
                ));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test categorical topology adapter reservation failed"))
        }
    }

    #[test]
    fn repeated_interaction_terms_reuse_codes_and_membership() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let mut registry = CategoricalTopologyRegistry::new();
        let interaction = registry.intern(&[0, 1], &sources).expect("interaction");
        let firm_source = registry.intern(&[0], &sources).expect("firm");
        let design = borrow_categorical_topology_design(
            firm.len(),
            &registry,
            &sources,
            &[interaction, interaction, firm_source],
        )
        .expect("borrowed design");

        assert_eq!(design.effects().len(), 3);
        assert_eq!(design.total_term_slots(), 4 + 4 + 2);
        assert!(std::ptr::eq(
            design.effects()[0].codes().as_ptr(),
            design.effects()[1].codes().as_ptr()
        ));
        assert!(std::ptr::eq(
            design.effects()[0].level_index(),
            design.effects()[1].level_index()
        ));
        assert!(std::ptr::eq(
            design.effects()[2].codes().as_ptr(),
            firm.as_ptr()
        ));
    }

    #[test]
    fn reordered_interaction_recipe_reuses_one_unique_source() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let mut registry = CategoricalTopologyRegistry::new();
        let forward = registry.intern(&[0, 1], &sources).expect("forward");
        let reverse = registry.intern(&[1, 0], &sources).expect("reverse");
        assert_eq!(forward, reverse);
        assert_eq!(registry.sources().len(), 1);
        let design = borrow_categorical_topology_design(
            firm.len(),
            &registry,
            &sources,
            &[forward, reverse],
        )
        .expect("borrowed design");
        assert_eq!(design.effects().len(), 2);
        assert!(std::ptr::eq(
            design.effects()[0].codes().as_ptr(),
            design.effects()[1].codes().as_ptr()
        ));
    }

    #[test]
    fn empty_topology_builds_an_empty_design_for_declared_nobs() {
        let registry = CategoricalTopologyRegistry::new();
        let design = borrow_categorical_topology_design(7, &registry, &[], &[])
            .expect("empty borrowed design");
        assert_eq!(design.nobs(), 7);
        assert!(design.effects().is_empty());
        assert_eq!(design.total_term_slots(), 0);
    }

    #[test]
    fn invalid_mapping_and_observation_count_fail_closed() {
        let raw = [0, 1, 0, 1];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        let mut registry = CategoricalTopologyRegistry::new();
        registry.intern(&[0], &sources).expect("source");
        assert_eq!(
            borrow_categorical_topology_design(raw.len(), &registry, &sources, &[1])
                .expect_err("bad mapping")
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            borrow_categorical_topology_design(raw.len() + 1, &registry, &sources, &[0])
                .expect_err("bad nobs")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn source_mutation_is_rejected_before_term_views_escape() {
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
            borrow_categorical_topology_design(firm.len(), &registry, &changed, &[0])
                .expect_err("changed interaction source")
                .code,
            ErrorCode::InvalidData
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
        let mut registry = CategoricalTopologyRegistry::new();
        registry.intern(&[0, 1], &sources).expect("interaction");

        let mut successful = InjectedReservation::default();
        borrow_categorical_topology_design_with_reservation(
            firm.len(),
            &registry,
            &sources,
            &[0, 0],
            &mut successful,
        )
        .expect("successful reservation audit");
        assert_eq!(successful.calls, 2);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = borrow_categorical_topology_design_with_reservation(
                firm.len(),
                &registry,
                &sources,
                &[0, 0],
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
            let error = borrow_categorical_topology_design_with_reservation(
                firm.len(),
                &registry,
                &sources,
                &[0, 0],
                &mut undersized,
            )
            .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }
}
