//! Absorbed degrees of freedom for finalized categorical topology.
//!
//! This module applies the pinned `dof(none)`, `dof(firstpair)`, and pairwise
//! mobility rules directly to deduplicated final-sample categorical partitions,
//! including numeric categorical interactions. Interaction terms are one FE
//! partition: their category count is the number of observed interaction tuples,
//! repeated/reordered interaction terms reuse one topology source, and an exact
//! repeated partition is fully redundant under the mobility search.

use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::{CategoricalCodeView, FinalizedCategoricalTopology};
use crate::metadata::{AbsorbTermId, SourceId};

/// Reference-compatible absorbed-degree-of-freedom policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CategoricalTopologyDofPolicy {
    None,
    FirstPair,
    #[default]
    Pairwise,
}

/// Wire identity and display metadata for one requested absorbed term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoricalTopologyDofEffectSpec {
    pub term_id: AbsorbTermId,
    /// Source tuple in requested wire order. Interaction partition coding is
    /// canonicalized independently; this identity tuple is not reordered.
    pub sources: Vec<SourceId>,
    pub label: String,
}

/// Identity of one row in the absorbed-DoF report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CategoricalTopologyDofEffectIdentity {
    Explicit {
        term_id: AbsorbTermId,
        sources: Vec<SourceId>,
    },
    /// Virtual one-level HDFE synthesized by the pinned no-absorb contract.
    ImplicitNoAbsorb,
}

/// One requested-term row in the absorbed-DoF report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoricalTopologyDofEffect {
    pub identity: CategoricalTopologyDofEffectIdentity,
    pub label: String,
    pub categories: usize,
    pub redundant: usize,
    pub coefficients: usize,
    pub redundancy_exact: bool,
    pub nested: bool,
}

/// Certified absorbed-rank report for finalized categorical topology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoricalTopologyDofReport {
    pub policy: CategoricalTopologyDofPolicy,
    pub effects: Vec<CategoricalTopologyDofEffect>,
    pub initial_categories: usize,
    pub redundant_categories: usize,
    pub absorbed_rank: usize,
    pub nested_categories: usize,
    pub is_approximate: bool,
}

/// Compute absorbed rank for finalized categorical topology.
///
/// `effect_specs` and `topology.effect_to_topology()` are both in requested
/// absorbed-term order. `cluster_dimensions` are final-sample numeric cluster
/// codes; each declared source tuple has already been factorized as one cluster
/// dimension by the caller.
pub fn compute_categorical_topology_absorbed_dof(
    topology: &FinalizedCategoricalTopology,
    effect_specs: &[CategoricalTopologyDofEffectSpec],
    structural_intercept: bool,
    policy: CategoricalTopologyDofPolicy,
    cluster_dimensions: &[&[u32]],
) -> Result<CategoricalTopologyDofReport> {
    let mut reservation = SystemTopologyDofReservation;
    compute_with_reservation(
        topology,
        effect_specs,
        structural_intercept,
        policy,
        cluster_dimensions,
        &mut reservation,
    )
}

trait TopologyDofReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;

    fn reserve_string(
        &mut self,
        target: &mut String,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemTopologyDofReservation;

impl TopologyDofReservation for SystemTopologyDofReservation {
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

    fn reserve_string(
        &mut self,
        target: &mut String,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

fn compute_with_reservation<R: TopologyDofReservation>(
    topology: &FinalizedCategoricalTopology,
    effect_specs: &[CategoricalTopologyDofEffectSpec],
    structural_intercept: bool,
    policy: CategoricalTopologyDofPolicy,
    cluster_dimensions: &[&[u32]],
    reservation: &mut R,
) -> Result<CategoricalTopologyDofReport> {
    topology.validate()?;
    validate_inputs(
        topology,
        effect_specs,
        structural_intercept,
        cluster_dimensions,
    )?;
    let nobs = topology.retained_rows().len();
    let mut source_views: Vec<CategoricalCodeView<'_>> = Vec::new();
    reservation.reserve_vec(
        &mut source_views,
        topology.compacted_sources().len(),
        "unable to reserve categorical absorbed-DoF source views",
    )?;
    for source in topology.compacted_sources() {
        push_reserved(
            &mut source_views,
            source.view(),
            "categorical absorbed-DoF source-view capacity exhausted",
        )?;
    }

    let effect_count = effect_specs.len();
    let mut redundant = reserved_filled(
        reservation,
        effect_count,
        0_usize,
        "unable to reserve categorical absorbed-DoF redundancies",
    )?;
    let mut exact = reserved_filled(
        reservation,
        effect_count,
        false,
        "unable to reserve categorical absorbed-DoF exactness flags",
    )?;
    let mut nested = reserved_filled(
        reservation,
        effect_count,
        false,
        "unable to reserve categorical absorbed-DoF nesting flags",
    )?;
    let mut eligible = reserved_filled(
        reservation,
        effect_count,
        true,
        "unable to reserve categorical absorbed-DoF eligibility flags",
    )?;

    let mut nested_categories = 0_usize;
    for effect_index in 0..effect_count {
        let topology_source = effect_source(topology, effect_index)?;
        if effect_is_nested_within_any_cluster(
            topology_source.codes(&source_views)?,
            topology_source.n_levels(),
            nobs,
            cluster_dimensions,
            reservation,
        )? {
            redundant[effect_index] = topology_source.n_levels();
            exact[effect_index] = true;
            nested[effect_index] = true;
            eligible[effect_index] = false;
            nested_categories = nested_categories
                .checked_add(topology_source.n_levels())
                .ok_or_else(|| allocation("categorical nested-category total overflows"))?;
        }
    }

    let mut eligible_indices = Vec::new();
    reservation.reserve_vec(
        &mut eligible_indices,
        effect_count,
        "unable to reserve categorical absorbed-DoF eligible indices",
    )?;
    for (effect_index, is_eligible) in eligible.iter().copied().enumerate() {
        if is_eligible {
            push_reserved(
                &mut eligible_indices,
                effect_index,
                "categorical absorbed-DoF eligible-index capacity exhausted",
            )?;
        }
    }
    if let Some(first) = eligible_indices.first().copied() {
        exact[first] = true;
    }
    let skip_first = nested_categories == 0;
    for (position, effect_index) in eligible_indices.iter().copied().enumerate() {
        if skip_first && position == 0 {
            continue;
        }
        redundant[effect_index] = usize::from(effect_source(topology, effect_index)?.n_levels() > 0);
    }

    match policy {
        CategoricalTopologyDofPolicy::None => {}
        CategoricalTopologyDofPolicy::FirstPair => {
            if eligible_indices.len() >= 2 {
                let earlier = eligible_indices[0];
                let later = eligible_indices[1];
                let components = pair_components_for_effects(
                    topology,
                    &source_views,
                    earlier,
                    later,
                    reservation,
                )?;
                redundant[later] = redundant[later].max(components);
                exact[later] = true;
            }
        }
        CategoricalTopologyDofPolicy::Pairwise => {
            for later_position in 1..eligible_indices.len() {
                let later = eligible_indices[later_position];
                for (earlier_position, earlier) in eligible_indices
                    .iter()
                    .copied()
                    .enumerate()
                    .take(later_position)
                {
                    let components = pair_components_for_effects(
                        topology,
                        &source_views,
                        earlier,
                        later,
                        reservation,
                    )?;
                    redundant[later] = redundant[later].max(components);
                    if earlier_position == 0 && later_position == 1 {
                        exact[later] = true;
                    }
                }
            }
        }
    }

    let implicit_no_absorb = structural_intercept && effect_count == 0 && nobs > 0;
    let row_count = effect_count
        .checked_add(usize::from(implicit_no_absorb))
        .ok_or_else(|| allocation("categorical absorbed-DoF report row count overflows"))?;
    let mut rows = Vec::new();
    reservation.reserve_vec(
        &mut rows,
        row_count,
        "unable to reserve categorical absorbed-DoF rows",
    )?;
    let mut initial_categories = 0_usize;
    let mut redundant_categories = 0_usize;
    let mut absorbed_rank = 0_usize;
    let mut is_approximate = false;

    if implicit_no_absorb {
        push_reserved(
            &mut rows,
            CategoricalTopologyDofEffect {
                identity: CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb,
                label: fallible_string_copy(
                    "_cons",
                    reservation,
                    "unable to reserve implicit no-absorb DoF label",
                )?,
                categories: 1,
                redundant: 0,
                coefficients: 1,
                redundancy_exact: true,
                nested: false,
            },
            "categorical absorbed-DoF implicit-row capacity exhausted",
        )?;
        initial_categories = 1;
        absorbed_rank = 1;
    }

    for (effect_index, spec) in effect_specs.iter().enumerate() {
        let categories = effect_source(topology, effect_index)?.n_levels();
        let effect_redundant = redundant[effect_index];
        if effect_redundant > categories {
            return Err(invalid_data(
                "categorical absorbed-DoF redundancy exceeds category count",
            ));
        }
        let coefficients = categories
            .checked_sub(effect_redundant)
            .ok_or_else(|| invalid_data("categorical absorbed-DoF subtraction underflowed"))?;
        initial_categories = initial_categories
            .checked_add(categories)
            .ok_or_else(|| allocation("categorical absorbed-DoF category total overflows"))?;
        redundant_categories = redundant_categories
            .checked_add(effect_redundant)
            .ok_or_else(|| allocation("categorical absorbed-DoF redundancy total overflows"))?;
        absorbed_rank = absorbed_rank
            .checked_add(coefficients)
            .ok_or_else(|| allocation("categorical absorbed rank overflows"))?;
        is_approximate |= !exact[effect_index];

        push_reserved(
            &mut rows,
            CategoricalTopologyDofEffect {
                identity: CategoricalTopologyDofEffectIdentity::Explicit {
                    term_id: spec.term_id,
                    sources: fallible_vec_copy(
                        &spec.sources,
                        reservation,
                        "unable to reserve categorical absorbed-DoF source identity",
                    )?,
                },
                label: fallible_string_copy(
                    &spec.label,
                    reservation,
                    "unable to reserve categorical absorbed-DoF label",
                )?,
                categories,
                redundant: effect_redundant,
                coefficients,
                redundancy_exact: exact[effect_index],
                nested: nested[effect_index],
            },
            "categorical absorbed-DoF effect-row capacity exhausted",
        )?;
    }

    let recomputed_rank = initial_categories
        .checked_sub(redundant_categories)
        .ok_or_else(|| invalid_data("categorical absorbed-DoF totals are inconsistent"))?;
    if recomputed_rank != absorbed_rank {
        return Err(invalid_data(
            "categorical absorbed-DoF rank certificate did not pass",
        ));
    }

    Ok(CategoricalTopologyDofReport {
        policy,
        effects: rows,
        initial_categories,
        redundant_categories,
        absorbed_rank,
        nested_categories,
        is_approximate,
    })
}

fn validate_inputs(
    topology: &FinalizedCategoricalTopology,
    effect_specs: &[CategoricalTopologyDofEffectSpec],
    structural_intercept: bool,
    cluster_dimensions: &[&[u32]],
) -> Result<()> {
    let nobs = topology.retained_rows().len();
    if !structural_intercept {
        return Err(invalid_argument(
            "categorical absorbed-DoF requires the structural intercept contract",
        ));
    }
    if effect_specs.len() != topology.effect_to_topology().len() {
        return Err(invalid_argument(
            "categorical absorbed-DoF metadata and topology effect counts differ",
        ));
    }
    for spec in effect_specs {
        if spec.sources.is_empty() {
            return Err(invalid_argument(
                "categorical absorbed-DoF explicit effect has an empty source tuple",
            ));
        }
        if spec.label.is_empty() {
            return Err(invalid_argument(
                "categorical absorbed-DoF explicit effect has an empty label",
            ));
        }
    }
    if cluster_dimensions.len() > 10 {
        return Err(invalid_argument(
            "categorical absorbed-DoF nesting exceeds ten cluster dimensions",
        ));
    }
    for cluster_codes in cluster_dimensions {
        if cluster_codes.len() != nobs {
            return Err(invalid_argument(
                "categorical absorbed-DoF cluster view is not final-sample aligned",
            ));
        }
    }
    Ok(())
}

fn effect_source(
    topology: &FinalizedCategoricalTopology,
    effect_index: usize,
) -> Result<&crate::fe::CategoricalTopologySource> {
    let topology_index = topology
        .effect_to_topology()
        .get(effect_index)
        .copied()
        .ok_or_else(|| invalid_argument("categorical absorbed-DoF effect is out of range"))?;
    topology.registry().source(topology_index)
}

fn effect_is_nested_within_any_cluster<R: TopologyDofReservation>(
    fe_codes: &[u32],
    n_levels: usize,
    nobs: usize,
    cluster_dimensions: &[&[u32]],
    reservation: &mut R,
) -> Result<bool> {
    if cluster_dimensions.is_empty() {
        return Ok(false);
    }
    if fe_codes.len() != nobs {
        return Err(invalid_argument(
            "categorical absorbed-DoF FE codes are not sample aligned",
        ));
    }
    for cluster_codes in cluster_dimensions {
        let mut cluster_by_level = reserved_filled(
            reservation,
            n_levels,
            u32::MAX,
            "unable to reserve categorical absorbed-DoF nesting map",
        )?;
        let mut is_nested = true;
        for (&fe, &cluster) in fe_codes.iter().zip(*cluster_codes) {
            let mapped = cluster_by_level.get_mut(fe as usize).ok_or_else(|| {
                invalid_data("categorical absorbed-DoF nested FE code is invalid")
            })?;
            if *mapped == u32::MAX {
                *mapped = cluster;
            } else if *mapped != cluster {
                is_nested = false;
                break;
            }
        }
        if is_nested {
            return Ok(true);
        }
    }
    Ok(false)
}

fn pair_components_for_effects<R: TopologyDofReservation>(
    topology: &FinalizedCategoricalTopology,
    source_views: &[CategoricalCodeView<'_>],
    earlier_effect: usize,
    later_effect: usize,
    reservation: &mut R,
) -> Result<usize> {
    let earlier = effect_source(topology, earlier_effect)?;
    let later = effect_source(topology, later_effect)?;
    let earlier_codes = earlier.codes(source_views)?;
    let later_codes = later.codes(source_views)?;
    if earlier_codes.len() != later_codes.len() {
        return Err(invalid_argument(
            "categorical absorbed-DoF pair sources are not sample aligned",
        ));
    }
    let vertex_count = earlier
        .n_levels()
        .checked_add(later.n_levels())
        .ok_or_else(|| allocation("categorical absorbed-DoF pair vertex count overflows"))?;
    if vertex_count == 0 {
        return Ok(0);
    }
    let mut parent = Vec::new();
    reservation.reserve_vec(
        &mut parent,
        vertex_count,
        "unable to reserve categorical absorbed-DoF union-find parents",
    )?;
    for vertex in 0..vertex_count {
        push_reserved(
            &mut parent,
            vertex,
            "categorical absorbed-DoF parent capacity exhausted",
        )?;
    }
    let mut rank = reserved_filled(
        reservation,
        vertex_count,
        0_u8,
        "unable to reserve categorical absorbed-DoF union-find ranks",
    )?;
    for (&earlier_code, &later_code) in earlier_codes.iter().zip(later_codes) {
        let left = earlier_code as usize;
        let right = earlier
            .n_levels()
            .checked_add(later_code as usize)
            .ok_or_else(|| allocation("categorical absorbed-DoF pair index overflows"))?;
        if left >= earlier.n_levels() || right >= vertex_count {
            return Err(invalid_data(
                "categorical absorbed-DoF pair code exceeds its level range",
            ));
        }
        union(&mut parent, &mut rank, left, right)?;
    }

    let mut components = 0_usize;
    for vertex in 0..vertex_count {
        if find(&mut parent, vertex)? == vertex {
            components = components
                .checked_add(1)
                .ok_or_else(|| allocation("categorical absorbed-DoF component count overflows"))?;
        }
    }
    Ok(components)
}

fn find(parent: &mut [usize], vertex: usize) -> Result<usize> {
    let mut root = vertex;
    while parent
        .get(root)
        .copied()
        .ok_or_else(|| invalid_data("categorical absorbed-DoF union-find index is invalid"))?
        != root
    {
        root = parent[root];
    }
    let mut current = vertex;
    while parent[current] != current {
        let next = parent[current];
        parent[current] = root;
        current = next;
    }
    Ok(root)
}

fn union(parent: &mut [usize], rank: &mut [u8], left: usize, right: usize) -> Result<()> {
    let left_root = find(parent, left)?;
    let right_root = find(parent, right)?;
    if left_root == right_root {
        return Ok(());
    }
    let left_rank = *rank
        .get(left_root)
        .ok_or_else(|| invalid_data("categorical absorbed-DoF union rank is invalid"))?;
    let right_rank = *rank
        .get(right_root)
        .ok_or_else(|| invalid_data("categorical absorbed-DoF union rank is invalid"))?;
    if left_rank < right_rank {
        parent[left_root] = right_root;
    } else if left_rank > right_rank {
        parent[right_root] = left_root;
    } else {
        parent[right_root] = left_root;
        rank[left_root] = left_rank
            .checked_add(1)
            .ok_or_else(|| allocation("categorical absorbed-DoF union rank overflows"))?;
    }
    Ok(())
}

fn reserved_filled<T: Clone, R: TopologyDofReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "categorical absorbed-DoF reservation returned insufficient capacity",
        ));
    }
    output.resize(elements, value);
    Ok(output)
}

fn fallible_vec_copy<T: Copy, R: TopologyDofReservation>(
    input: &[T],
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, input.len(), label)?;
    if output.capacity() < input.len() {
        return Err(allocation(
            "categorical absorbed-DoF vector reservation returned insufficient capacity",
        ));
    }
    output.extend_from_slice(input);
    Ok(output)
}

fn fallible_string_copy<R: TopologyDofReservation>(
    input: &str,
    reservation: &mut R,
    label: &'static str,
) -> Result<String> {
    let mut output = String::new();
    reservation.reserve_string(&mut output, input.len(), label)?;
    if output.capacity() < input.len() {
        return Err(allocation(
            "categorical absorbed-DoF string reservation returned insufficient capacity",
        ));
    }
    output.push_str(input);
    Ok(output)
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn invalid_argument(message: impl Into<String>) -> StFixestError {
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
    use crate::fe::finalize_categorical_topology;

    #[derive(Default)]
    struct InjectedReservation {
        calls: usize,
        fail_at: Option<usize>,
        skip_at: Option<usize>,
    }

    impl TopologyDofReservation for InjectedReservation {
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
                    "injected categorical absorbed-DoF allocation failure",
                ));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test categorical absorbed-DoF reservation failed"))
        }

        fn reserve_string(
            &mut self,
            target: &mut String,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected categorical absorbed-DoF string allocation failure",
                ));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test categorical absorbed-DoF string reservation failed"))
        }
    }

    fn source(value: u32) -> SourceId {
        SourceId::new(value).expect("source id")
    }

    fn term(value: u32) -> AbsorbTermId {
        AbsorbTermId::new(value).expect("term id")
    }

    fn spec(term_id: u32, sources: &[u32], label: &str) -> CategoricalTopologyDofEffectSpec {
        CategoricalTopologyDofEffectSpec {
            term_id: term(term_id),
            sources: sources.iter().copied().map(source).collect(),
            label: label.to_owned(),
        }
    }

    #[test]
    fn one_interaction_has_one_exact_coefficient_per_observed_tuple() {
        let firm = [0, 0, 0, 0, 1, 1, 1, 1];
        let year = [0, 0, 1, 1, 0, 0, 1, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let topology = finalize_categorical_topology(&sources, &[&[0, 1]], &[true; 8])
            .expect("topology");
        let report = compute_categorical_topology_absorbed_dof(
            &topology,
            &[spec(1, &[1, 2], "firm#year")],
            true,
            CategoricalTopologyDofPolicy::Pairwise,
            &[],
        )
        .expect("DoF");
        assert_eq!(report.initial_categories, 4);
        assert_eq!(report.redundant_categories, 0);
        assert_eq!(report.absorbed_rank, 4);
        assert!(!report.is_approximate);
        assert_eq!(report.effects[0].categories, 4);
        assert_eq!(report.effects[0].coefficients, 4);
    }

    #[test]
    fn repeated_interaction_is_exactly_fully_redundant() {
        let firm = [0, 0, 0, 0, 1, 1, 1, 1];
        let year = [0, 0, 1, 1, 0, 0, 1, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let topology =
            finalize_categorical_topology(&sources, &[&[0, 1], &[1, 0]], &[true; 8])
                .expect("topology");
        let report = compute_categorical_topology_absorbed_dof(
            &topology,
            &[
                spec(1, &[1, 2], "firm#year"),
                spec(2, &[2, 1], "year#firm"),
            ],
            true,
            CategoricalTopologyDofPolicy::Pairwise,
            &[],
        )
        .expect("DoF");
        assert_eq!(report.initial_categories, 8);
        assert_eq!(report.redundant_categories, 4);
        assert_eq!(report.absorbed_rank, 4);
        assert!(report.effects[1].redundancy_exact);
        assert_eq!(report.effects[1].redundant, 4);
        assert_eq!(report.effects[1].coefficients, 0);
    }

    #[test]
    fn connected_single_effect_and_interaction_follow_pair_components() {
        let firm = [0, 0, 0, 0, 1, 1, 1, 1];
        let year = [0, 0, 1, 1, 0, 0, 1, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let topology =
            finalize_categorical_topology(&sources, &[&[0], &[0, 1]], &[true; 8])
                .expect("topology");
        let report = compute_categorical_topology_absorbed_dof(
            &topology,
            &[
                spec(1, &[1], "firm"),
                spec(2, &[1, 2], "firm#year"),
            ],
            true,
            CategoricalTopologyDofPolicy::Pairwise,
            &[],
        )
        .expect("DoF");
        // Each firm connects to its two firm-year cells, giving two components.
        assert_eq!(report.effects[0].coefficients, 2);
        assert_eq!(report.effects[1].redundant, 2);
        assert_eq!(report.effects[1].coefficients, 2);
        assert_eq!(report.absorbed_rank, 4);
    }

    #[test]
    fn none_policy_marks_later_effect_approximate() {
        let first = [0, 0, 1, 1];
        let second = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&first, 2),
            CategoricalCodeView::new(&second, 2),
        ];
        let topology = finalize_categorical_topology(&sources, &[&[0], &[1]], &[true; 4])
            .expect("topology");
        let report = compute_categorical_topology_absorbed_dof(
            &topology,
            &[spec(1, &[1], "first"), spec(2, &[2], "second")],
            true,
            CategoricalTopologyDofPolicy::None,
            &[],
        )
        .expect("DoF");
        assert_eq!(report.effects[0].redundant, 0);
        assert!(report.effects[0].redundancy_exact);
        assert_eq!(report.effects[1].redundant, 1);
        assert!(!report.effects[1].redundancy_exact);
        assert!(report.is_approximate);
    }

    #[test]
    fn interaction_nested_in_cluster_is_fully_redundant() {
        let firm = [0, 0, 0, 0, 1, 1, 1, 1];
        let year = [0, 0, 1, 1, 0, 0, 1, 1];
        let cluster = [0, 0, 1, 1, 2, 2, 3, 3];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let topology = finalize_categorical_topology(&sources, &[&[0, 1]], &[true; 8])
            .expect("topology");
        let report = compute_categorical_topology_absorbed_dof(
            &topology,
            &[spec(1, &[1, 2], "firm#year")],
            true,
            CategoricalTopologyDofPolicy::Pairwise,
            &[&cluster],
        )
        .expect("DoF");
        assert!(report.effects[0].nested);
        assert_eq!(report.effects[0].redundant, 4);
        assert_eq!(report.effects[0].coefficients, 0);
        assert_eq!(report.nested_categories, 4);
        assert_eq!(report.absorbed_rank, 0);
    }

    #[test]
    fn implicit_no_absorb_structural_constant_is_exact() {
        let raw = [0, 1, 0];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        let topology =
            finalize_categorical_topology(&sources, &[], &[true; 3]).expect("topology");
        let report = compute_categorical_topology_absorbed_dof(
            &topology,
            &[],
            true,
            CategoricalTopologyDofPolicy::Pairwise,
            &[],
        )
        .expect("DoF");
        assert_eq!(report.effects.len(), 1);
        assert_eq!(report.initial_categories, 1);
        assert_eq!(report.absorbed_rank, 1);
        assert!(matches!(
            report.effects[0].identity,
            CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb
        ));
    }

    #[test]
    fn empty_final_sample_has_no_implicit_constant_rank() {
        let raw = [0, 1];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        let topology =
            finalize_categorical_topology(&sources, &[], &[false; 2]).expect("topology");
        let report = compute_categorical_topology_absorbed_dof(
            &topology,
            &[],
            true,
            CategoricalTopologyDofPolicy::Pairwise,
            &[],
        )
        .expect("DoF");
        assert!(report.effects.is_empty());
        assert_eq!(report.absorbed_rank, 0);
    }

    #[test]
    fn metadata_and_cluster_view_corruption_fail_closed() {
        let raw = [0, 0, 1, 1];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        let topology = finalize_categorical_topology(&sources, &[&[0]], &[true; 4])
            .expect("topology");
        assert_eq!(
            compute_categorical_topology_absorbed_dof(
                &topology,
                &[],
                true,
                CategoricalTopologyDofPolicy::Pairwise,
                &[],
            )
            .expect_err("metadata mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            compute_categorical_topology_absorbed_dof(
                &topology,
                &[spec(1, &[1], "raw")],
                true,
                CategoricalTopologyDofPolicy::Pairwise,
                &[&[0, 1]],
            )
            .expect_err("cluster mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            compute_categorical_topology_absorbed_dof(
                &topology,
                &[CategoricalTopologyDofEffectSpec {
                    term_id: term(1),
                    sources: Vec::new(),
                    label: "bad".to_owned(),
                }],
                true,
                CategoricalTopologyDofPolicy::Pairwise,
                &[],
            )
            .expect_err("empty source tuple")
            .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn every_reservation_and_capacity_guard_fails_closed() {
        let firm = [0, 0, 0, 0, 1, 1, 1, 1];
        let year = [0, 0, 1, 1, 0, 0, 1, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let topology =
            finalize_categorical_topology(&sources, &[&[0], &[0, 1]], &[true; 8])
                .expect("topology");
        let specs = [
            spec(1, &[1], "firm"),
            spec(2, &[1, 2], "firm#year"),
        ];
        let mut successful = InjectedReservation::default();
        compute_with_reservation(
            &topology,
            &specs,
            true,
            CategoricalTopologyDofPolicy::Pairwise,
            &[],
            &mut successful,
        )
        .expect("successful allocation audit");
        assert!(successful.calls > 10);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = compute_with_reservation(
                &topology,
                &specs,
                true,
                CategoricalTopologyDofPolicy::Pairwise,
                &[],
                &mut injected,
            )
            .expect_err("injected allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        for skip_at in 0..successful.calls {
            let mut undersized = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = compute_with_reservation(
                &topology,
                &specs,
                true,
                CategoricalTopologyDofPolicy::Pairwise,
                &[],
                &mut undersized,
            )
            .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }
}
