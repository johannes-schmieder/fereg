//! Absorbed degrees of freedom for finalized categorical topology.
//!
//! This module applies the pinned `dof(none)`, `dof(firstpair)`, and pairwise
//! mobility rules directly to deduplicated final-sample categorical partitions,
//! including numeric categorical interactions. Interaction terms are one FE
//! partition: their category count is the number of observed interaction tuples,
//! repeated/reordered interaction terms reuse one topology source, and an exact
//! repeated partition is fully redundant under the mobility search.

use std::cmp::Ordering;

use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::{CategoricalCodeView, CategoricalTopologySource, FinalizedCategoricalTopology};
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
/// absorbed-term order. `cluster_dimensions` are final-sample compact numeric
/// cluster codes; each declared source tuple has already been factorized as one
/// cluster dimension by the caller.
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
    let source_views = source_views(topology, reservation)?;
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
        let source = effect_source(topology, effect_index)?;
        if effect_is_nested_within_any_cluster(
            source.codes(&source_views)?,
            source.n_levels(),
            nobs,
            cluster_dimensions,
            reservation,
        )? {
            redundant[effect_index] = source.n_levels();
            exact[effect_index] = true;
            nested[effect_index] = true;
            eligible[effect_index] = false;
            nested_categories = nested_categories
                .checked_add(source.n_levels())
                .ok_or_else(|| allocation("categorical nested-category total overflows"))?;
        }
    }

    let eligible_indices = eligible_indices(&eligible, reservation)?;
    if let Some(first) = eligible_indices.first().copied() {
        exact[first] = true;
    }
    let skip_first = nested_categories == 0;
    for (position, effect_index) in eligible_indices.iter().copied().enumerate() {
        if skip_first && position == 0 {
            continue;
        }
        redundant[effect_index] =
            usize::from(effect_source(topology, effect_index)?.n_levels() > 0);
    }

    apply_mobility_policy(
        topology,
        &source_views,
        &eligible_indices,
        policy,
        &mut redundant,
        &mut exact,
        reservation,
    )?;

    build_report(
        topology,
        effect_specs,
        structural_intercept,
        policy,
        &redundant,
        &exact,
        &nested,
        nested_categories,
        reservation,
    )
}

fn source_views<'a, R: TopologyDofReservation>(
    topology: &'a FinalizedCategoricalTopology,
    reservation: &mut R,
) -> Result<Vec<CategoricalCodeView<'a>>> {
    let mut views = Vec::new();
    reservation.reserve_vec(
        &mut views,
        topology.compacted_sources().len(),
        "unable to reserve categorical absorbed-DoF source views",
    )?;
    for source in topology.compacted_sources() {
        push_reserved(
            &mut views,
            source.view(),
            "categorical absorbed-DoF source-view capacity exhausted",
        )?;
    }
    Ok(views)
}

fn eligible_indices<R: TopologyDofReservation>(
    eligible: &[bool],
    reservation: &mut R,
) -> Result<Vec<usize>> {
    let mut indices = Vec::new();
    reservation.reserve_vec(
        &mut indices,
        eligible.len(),
        "unable to reserve categorical absorbed-DoF eligible indices",
    )?;
    for (effect_index, is_eligible) in eligible.iter().copied().enumerate() {
        if is_eligible {
            push_reserved(
                &mut indices,
                effect_index,
                "categorical absorbed-DoF eligible-index capacity exhausted",
            )?;
        }
    }
    Ok(indices)
}

fn apply_mobility_policy<R: TopologyDofReservation>(
    topology: &FinalizedCategoricalTopology,
    source_views: &[CategoricalCodeView<'_>],
    eligible_indices: &[usize],
    policy: CategoricalTopologyDofPolicy,
    redundant: &mut [usize],
    exact: &mut [bool],
    reservation: &mut R,
) -> Result<()> {
    match policy {
        CategoricalTopologyDofPolicy::None => {}
        CategoricalTopologyDofPolicy::FirstPair => {
            if eligible_indices.len() >= 2 {
                let earlier = eligible_indices[0];
                let later = eligible_indices[1];
                let components = pair_components_for_effects(
                    topology,
                    source_views,
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
                        source_views,
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
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_report<R: TopologyDofReservation>(
    topology: &FinalizedCategoricalTopology,
    effect_specs: &[CategoricalTopologyDofEffectSpec],
    structural_intercept: bool,
    policy: CategoricalTopologyDofPolicy,
    redundant: &[usize],
    exact: &[bool],
    nested: &[bool],
    nested_categories: usize,
    reservation: &mut R,
) -> Result<CategoricalTopologyDofReport> {
    let nobs = topology.retained_rows().len();
    let implicit_no_absorb = structural_intercept && effect_specs.is_empty() && nobs > 0;
    let row_count = effect_specs
        .len()
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
        if cluster_codes.contains(&u32::MAX) {
            return Err(invalid_data(
                "categorical absorbed-DoF cluster codes must be compact and cannot use u32::MAX",
            ));
        }
    }
    Ok(())
}

fn effect_source(
    topology: &FinalizedCategoricalTopology,
    effect_index: usize,
) -> Result<&CategoricalTopologySource> {
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
    match left_rank.cmp(&right_rank) {
        Ordering::Less => parent[left_root] = right_root,
        Ordering::Greater => parent[right_root] = left_root,
        Ordering::Equal => {
            parent[right_root] = left_root;
            rank[left_root] = left_rank
                .checked_add(1)
                .ok_or_else(|| allocation("categorical absorbed-DoF union rank overflows"))?;
        }
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
