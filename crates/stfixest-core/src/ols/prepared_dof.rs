use crate::error::{ErrorCode, Result, StFixestError};
use crate::metadata::{AbsorbTermId, SourceId};

use super::PreparedOlsData;

/// Reference-compatible absorbed-degree-of-freedom policy for the currently
/// supported intercept-only prepared FE surface.
///
/// `None` is deliberately named after reghdfe's `dof(none)`: it disables
/// mobility searches, but it does not make absorbed fixed effects free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsAbsorbedDofPolicy {
    None,
    FirstPair,
    Pairwise,
}

impl Default for PreparedOlsAbsorbedDofPolicy {
    fn default() -> Self {
        Self::Pairwise
    }
}

/// One wire-ordered row of the prepared absorbed-DoF table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsAbsorbedDofEffectIdentity {
    Explicit {
        term_id: AbsorbTermId,
        source: SourceId,
    },
    /// Virtual one-level HDFE synthesized by the pinned no-absorb contract.
    ImplicitNoAbsorb,
}

/// One wire-ordered row of the prepared absorbed-DoF table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOlsAbsorbedDofEffect {
    pub identity: PreparedOlsAbsorbedDofEffectIdentity,
    pub label: String,
    pub categories: usize,
    pub redundant: usize,
    pub coefficients: usize,
    pub redundancy_exact: bool,
    /// Cluster nesting is not part of this core-only checkpoint.
    pub nested: bool,
}

/// Certified structural absorbed-rank report on the final prepared sample.
///
/// This report deliberately excludes finite-regressor rank, the reported
/// constant, effective frequency-weight N, cluster nesting, and residual DoF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOlsAbsorbedDofReport {
    pub policy: PreparedOlsAbsorbedDofPolicy,
    pub effects: Vec<PreparedOlsAbsorbedDofEffect>,
    pub initial_categories: usize,
    pub redundant_categories: usize,
    pub absorbed_rank: usize,
    pub is_approximate: bool,
}

trait PreparedDofReservation {
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

struct SystemPreparedDofReservation;

impl PreparedDofReservation for SystemPreparedDofReservation {
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

/// Compute the reference-compatible absorbed rank for the current prepared
/// single-source, intercept-only numeric FE surface.
///
/// Effect order, level counts, and memberships come exclusively from
/// [`PreparedOlsData::fe_topology`]. `PreparedOlsData` is accepted because it
/// retains the one authoritative categorical-code slice per unique source;
/// those slices are borrowed for linear-time pair edges and never copied.
pub fn compute_prepared_ols_absorbed_dof(
    prepared: &PreparedOlsData,
    policy: PreparedOlsAbsorbedDofPolicy,
) -> Result<PreparedOlsAbsorbedDofReport> {
    let mut reservation = SystemPreparedDofReservation;
    compute_with_reservation(prepared, policy, &mut reservation)
}

fn compute_with_reservation<R: PreparedDofReservation>(
    prepared: &PreparedOlsData,
    policy: PreparedOlsAbsorbedDofPolicy,
    reservation: &mut R,
) -> Result<PreparedOlsAbsorbedDofReport> {
    validate_prepared_topology(prepared)?;
    let effects = prepared.fe_topology.effects();

    let mut redundant = reserved_filled(
        reservation,
        effects.len(),
        0_usize,
        "unable to reserve prepared absorbed-DoF redundancies",
    )?;
    let mut exact = reserved_filled(
        reservation,
        effects.len(),
        false,
        "unable to reserve prepared absorbed-DoF exactness flags",
    )?;
    if !effects.is_empty() {
        exact[0] = true;
    }
    for (effect_index, effect_redundant) in redundant.iter_mut().enumerate().skip(1) {
        let levels = effect_level_count(prepared, effect_index)?;
        *effect_redundant = usize::from(levels > 0);
    }

    match policy {
        PreparedOlsAbsorbedDofPolicy::None => {}
        PreparedOlsAbsorbedDofPolicy::FirstPair => {
            if effects.len() >= 2 {
                let components = pair_components_for_effects(prepared, 0, 1, reservation)?;
                redundant[1] = redundant[1].max(components);
                exact[1] = true;
            }
        }
        PreparedOlsAbsorbedDofPolicy::Pairwise => {
            for later in 1..effects.len() {
                for earlier in 0..later {
                    let components =
                        pair_components_for_effects(prepared, earlier, later, reservation)?;
                    redundant[later] = redundant[later].max(components);
                    if earlier == 0 && later == 1 {
                        exact[later] = true;
                    }
                }
            }
        }
    }

    let mut rows = Vec::new();
    let implicit_no_absorb =
        prepared.structural_intercept && effects.is_empty() && prepared.sample.n_active() > 0;
    reservation.reserve_vec(
        &mut rows,
        effects
            .len()
            .checked_add(usize::from(implicit_no_absorb))
            .ok_or_else(|| allocation("prepared absorbed-DoF row count overflows"))?,
        "unable to reserve prepared absorbed-DoF effect rows",
    )?;
    let mut initial_categories = 0_usize;
    let mut redundant_categories = 0_usize;
    let mut absorbed_rank = 0_usize;
    let mut is_approximate = false;
    if implicit_no_absorb {
        let label = fallible_string_copy(
            "_cons",
            reservation,
            "unable to reserve implicit no-absorb DoF label",
        )?;
        push_reserved(
            &mut rows,
            PreparedOlsAbsorbedDofEffect {
                identity: PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb,
                label,
                categories: 1,
                redundant: 0,
                coefficients: 1,
                redundancy_exact: true,
                nested: false,
            },
            "prepared absorbed-DoF implicit-row capacity exhausted",
        )?;
        initial_categories = 1;
        absorbed_rank = 1;
    }
    for (effect_index, effect) in effects.iter().enumerate() {
        let categories = effect_level_count(prepared, effect_index)?;
        let effect_redundant = redundant[effect_index];
        if effect_redundant > categories {
            return Err(invalid_data(
                "prepared absorbed-DoF redundancy exceeds its category count",
            ));
        }
        let coefficients = categories
            .checked_sub(effect_redundant)
            .ok_or_else(|| invalid_data("prepared absorbed-DoF subtraction underflowed"))?;
        initial_categories = initial_categories
            .checked_add(categories)
            .ok_or_else(|| allocation("prepared absorbed-DoF category total overflows"))?;
        redundant_categories = redundant_categories
            .checked_add(effect_redundant)
            .ok_or_else(|| allocation("prepared absorbed-DoF redundancy total overflows"))?;
        absorbed_rank = absorbed_rank
            .checked_add(coefficients)
            .ok_or_else(|| allocation("prepared absorbed rank overflows"))?;
        is_approximate |= !exact[effect_index];
        let label = fallible_string_copy(
            &effect.label,
            reservation,
            "unable to reserve prepared absorbed-DoF label",
        )?;
        push_reserved(
            &mut rows,
            PreparedOlsAbsorbedDofEffect {
                identity: PreparedOlsAbsorbedDofEffectIdentity::Explicit {
                    term_id: effect.term_id,
                    source: effect.source,
                },
                label,
                categories,
                redundant: effect_redundant,
                coefficients,
                redundancy_exact: exact[effect_index],
                nested: false,
            },
            "prepared absorbed-DoF effect-row capacity exhausted",
        )?;
    }
    let recomputed_rank = initial_categories
        .checked_sub(redundant_categories)
        .ok_or_else(|| invalid_data("prepared absorbed-DoF totals are inconsistent"))?;
    if recomputed_rank != absorbed_rank {
        return Err(invalid_data(
            "prepared absorbed-DoF rank certificate did not pass",
        ));
    }

    Ok(PreparedOlsAbsorbedDofReport {
        policy,
        effects: rows,
        initial_categories,
        redundant_categories,
        absorbed_rank,
        is_approximate,
    })
}

fn validate_prepared_topology(prepared: &PreparedOlsData) -> Result<()> {
    let topology = &prepared.fe_topology;
    let nobs = prepared.sample.n_active();
    if !prepared.structural_intercept {
        return Err(invalid_argument(
            "prepared absorbed-DoF intercept metadata is inconsistent with the supported surface",
        ));
    }
    if topology.nobs() != nobs {
        return Err(invalid_argument(
            "prepared absorbed-DoF topology is not aligned with the final sample",
        ));
    }

    let mut total_unique_levels = 0_usize;
    for source in topology.sources() {
        let categorical = prepared
            .categorical_sources
            .get(source.categorical_source_index())
            .ok_or_else(|| {
                invalid_argument("prepared absorbed-DoF categorical source is out of range")
            })?;
        if categorical.source != source.source()
            || categorical.encoded.n_levels() != source.n_levels()
            || categorical.encoded.codes.len() != nobs
            || source.nobs() != nobs
        {
            return Err(invalid_argument(
                "prepared absorbed-DoF source identity or dimensions are inconsistent",
            ));
        }
        if nobs == 0 {
            if source.n_levels() != 0 {
                return Err(invalid_argument(
                    "empty prepared absorbed-DoF source retains levels",
                ));
            }
        } else if source.n_levels() == 0 {
            return Err(invalid_argument(
                "nonempty prepared absorbed-DoF source has no levels",
            ));
        }
        if categorical
            .encoded
            .codes
            .iter()
            .any(|code| *code as usize >= source.n_levels())
        {
            return Err(invalid_data(
                "prepared absorbed-DoF code exceeds its declared level count",
            ));
        }

        let mut membership_count = 0_usize;
        for level in 0..source.n_levels() {
            let members = source.members(level)?;
            if members.is_empty() {
                return Err(invalid_argument(
                    "prepared absorbed-DoF source contains an empty final level",
                ));
            }
            membership_count = membership_count
                .checked_add(members.len())
                .ok_or_else(|| allocation("prepared absorbed-DoF membership count overflows"))?;
            for observation in members {
                if *observation >= nobs || categorical.encoded.codes[*observation] as usize != level
                {
                    return Err(invalid_argument(
                        "prepared absorbed-DoF membership does not match its codes",
                    ));
                }
            }
        }
        if membership_count != nobs {
            return Err(invalid_argument(
                "prepared absorbed-DoF membership does not cover the final sample",
            ));
        }
        total_unique_levels = total_unique_levels
            .checked_add(source.n_levels())
            .ok_or_else(|| allocation("prepared absorbed-DoF unique-level total overflows"))?;
    }
    if total_unique_levels != topology.total_levels() {
        return Err(invalid_argument(
            "prepared absorbed-DoF topology level total is inconsistent",
        ));
    }

    for effect in topology.effects() {
        let source = topology
            .sources()
            .get(effect.topology_source_index)
            .ok_or_else(|| {
                invalid_argument("prepared absorbed-DoF effect source is out of range")
            })?;
        if source.source() != effect.source
            || source.categorical_source_index() != effect.categorical_source_index
        {
            return Err(invalid_argument(
                "prepared absorbed-DoF effect identity is inconsistent",
            ));
        }
    }
    Ok(())
}

fn effect_level_count(prepared: &PreparedOlsData, effect_index: usize) -> Result<usize> {
    let effect = prepared
        .fe_topology
        .effects()
        .get(effect_index)
        .ok_or_else(|| invalid_argument("prepared absorbed-DoF effect is out of range"))?;
    prepared
        .fe_topology
        .sources()
        .get(effect.topology_source_index)
        .map(|source| source.n_levels())
        .ok_or_else(|| invalid_argument("prepared absorbed-DoF topology source is out of range"))
}

fn pair_components_for_effects<R: PreparedDofReservation>(
    prepared: &PreparedOlsData,
    earlier: usize,
    later: usize,
    reservation: &mut R,
) -> Result<usize> {
    let earlier_effect =
        prepared.fe_topology.effects().get(earlier).ok_or_else(|| {
            invalid_argument("earlier prepared absorbed-DoF effect is out of range")
        })?;
    let later_effect =
        prepared.fe_topology.effects().get(later).ok_or_else(|| {
            invalid_argument("later prepared absorbed-DoF effect is out of range")
        })?;
    let earlier_source = prepared
        .fe_topology
        .sources()
        .get(earlier_effect.topology_source_index)
        .ok_or_else(|| invalid_argument("earlier prepared absorbed-DoF source is out of range"))?;
    let later_source = prepared
        .fe_topology
        .sources()
        .get(later_effect.topology_source_index)
        .ok_or_else(|| invalid_argument("later prepared absorbed-DoF source is out of range"))?;
    let earlier_codes = &prepared
        .categorical_sources
        .get(earlier_source.categorical_source_index())
        .ok_or_else(|| invalid_argument("earlier categorical source is out of range"))?
        .encoded
        .codes;
    let later_codes = &prepared
        .categorical_sources
        .get(later_source.categorical_source_index())
        .ok_or_else(|| invalid_argument("later categorical source is out of range"))?
        .encoded
        .codes;
    pair_component_count(
        earlier_codes,
        earlier_source.n_levels(),
        later_codes,
        later_source.n_levels(),
        reservation,
    )
}

fn pair_component_count<R: PreparedDofReservation>(
    left_codes: &[u32],
    left_levels: usize,
    right_codes: &[u32],
    right_levels: usize,
    reservation: &mut R,
) -> Result<usize> {
    if left_codes.len() != right_codes.len() {
        return Err(invalid_argument(
            "prepared absorbed-DoF pair has inconsistent observation counts",
        ));
    }
    if left_codes.is_empty() {
        if left_levels != 0 || right_levels != 0 {
            return Err(invalid_argument(
                "empty prepared absorbed-DoF pair retains levels",
            ));
        }
        return Ok(0);
    }
    if left_levels == 0 || right_levels == 0 {
        return Err(invalid_argument(
            "nonempty prepared absorbed-DoF pair has no levels",
        ));
    }
    if left_codes.iter().any(|code| *code as usize >= left_levels)
        || right_codes
            .iter()
            .any(|code| *code as usize >= right_levels)
    {
        return Err(invalid_data(
            "prepared absorbed-DoF pair code exceeds its declared level count",
        ));
    }

    let nodes = left_levels
        .checked_add(right_levels)
        .ok_or_else(|| allocation("prepared absorbed-DoF pair node count overflows"))?;
    let mut parent = reserved_empty(
        reservation,
        nodes,
        "unable to reserve prepared absorbed-DoF union-find parents",
    )?;
    for node in 0..nodes {
        push_reserved(
            &mut parent,
            node,
            "prepared absorbed-DoF parent capacity exhausted",
        )?;
    }
    let mut ranks = reserved_filled(
        reservation,
        nodes,
        0_u8,
        "unable to reserve prepared absorbed-DoF union-find ranks",
    )?;

    for (&left, &right) in left_codes.iter().zip(right_codes) {
        let right_node = left_levels
            .checked_add(right as usize)
            .ok_or_else(|| allocation("prepared absorbed-DoF right node overflows"))?;
        union(&mut parent, &mut ranks, left as usize, right_node)?;
    }

    let mut components = 0_usize;
    for node in 0..nodes {
        if find_root(&mut parent, node)? == node {
            components = components
                .checked_add(1)
                .ok_or_else(|| allocation("prepared absorbed-DoF component count overflows"))?;
        }
    }
    Ok(components)
}

fn find_root(parent: &mut [usize], start: usize) -> Result<usize> {
    if start >= parent.len() {
        return Err(invalid_argument(
            "prepared absorbed-DoF union-find node is out of range",
        ));
    }
    let mut root = start;
    let mut traversed = 0_usize;
    loop {
        let next = *parent.get(root).ok_or_else(|| {
            invalid_argument("prepared absorbed-DoF union-find parent is out of range")
        })?;
        if next == root {
            break;
        }
        root = next;
        traversed = traversed
            .checked_add(1)
            .ok_or_else(|| allocation("prepared absorbed-DoF traversal count overflows"))?;
        if traversed >= parent.len() {
            return Err(invalid_argument(
                "prepared absorbed-DoF union-find contains a cycle",
            ));
        }
    }

    let mut node = start;
    loop {
        let next = *parent.get(node).ok_or_else(|| {
            invalid_argument("prepared absorbed-DoF union-find parent is out of range")
        })?;
        if next == node {
            break;
        }
        parent[node] = root;
        node = next;
    }
    Ok(root)
}

fn union(parent: &mut [usize], ranks: &mut [u8], left: usize, right: usize) -> Result<()> {
    if parent.len() != ranks.len() || left >= parent.len() || right >= parent.len() {
        return Err(invalid_argument(
            "prepared absorbed-DoF union-find dimensions are inconsistent",
        ));
    }
    let left_root = find_root(parent, left)?;
    let right_root = find_root(parent, right)?;
    if left_root == right_root {
        return Ok(());
    }
    match ranks[left_root].cmp(&ranks[right_root]) {
        std::cmp::Ordering::Less => parent[left_root] = right_root,
        std::cmp::Ordering::Greater => parent[right_root] = left_root,
        std::cmp::Ordering::Equal => {
            parent[right_root] = left_root;
            ranks[left_root] = ranks[left_root]
                .checked_add(1)
                .ok_or_else(|| allocation("prepared absorbed-DoF union rank overflows"))?;
        }
    }
    Ok(())
}

fn fallible_string_copy<R: PreparedDofReservation>(
    value: &str,
    reservation: &mut R,
    label: &'static str,
) -> Result<String> {
    let mut output = String::new();
    reservation.reserve_string(&mut output, value.len(), label)?;
    if output.capacity() < value.len() {
        return Err(allocation("prepared absorbed-DoF label capacity exhausted"));
    }
    output.push_str(value);
    Ok(output)
}

fn reserved_empty<T, R: PreparedDofReservation>(
    reservation: &mut R,
    elements: usize,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    if elements == 0 {
        return Ok(output);
    }
    reservation.reserve_vec(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "prepared absorbed-DoF reservation returned insufficient capacity",
        ));
    }
    Ok(output)
}

fn reserved_filled<T: Clone, R: PreparedDofReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = reserved_empty(reservation, elements, label)?;
    output.resize(elements, value);
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
    use crate::metadata::{AbsorbedTerm, CompiledTermGraph, SourceEntry, SourceId};
    use crate::ols::{
        prepare_ols_data, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
        OlsRequestV1,
    };

    #[derive(Clone, Copy)]
    enum TestWeights<'a> {
        Unit,
        Analytic(&'a [f64]),
        Frequency(&'a [u32]),
        Probability(&'a [f64]),
    }

    #[derive(Default)]
    struct InjectedReservation {
        calls: usize,
        max_additional: usize,
        fail_at: Option<usize>,
        undersize_at: Option<usize>,
        records: Vec<(usize, &'static str)>,
    }

    impl PreparedDofReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            self.max_additional = self.max_additional.max(additional);
            self.records.push((additional, label));
            self.reserve(target, additional)
        }

        fn reserve_string(
            &mut self,
            target: &mut String,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            self.max_additional = self.max_additional.max(additional);
            self.records.push((additional, label));
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected prepared absorbed-DoF allocation failure",
                ));
            }
            if self.undersize_at != Some(call) {
                target
                    .try_reserve_exact(additional)
                    .map_err(|_| allocation("test prepared absorbed-DoF reservation failed"))?;
            }
            Ok(())
        }
    }

    impl InjectedReservation {
        fn reserve<T>(&mut self, target: &mut Vec<T>, additional: usize) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected prepared absorbed-DoF allocation failure",
                ));
            }
            if self.undersize_at != Some(call) {
                target
                    .try_reserve_exact(additional)
                    .map_err(|_| allocation("test prepared absorbed-DoF reservation failed"))?;
            }
            Ok(())
        }
    }

    #[test]
    fn sealed_reference_dof_anchors_match() {
        // Frozen licensed-Stata receipt `ols/20260821T030000Z-a5181f9`
        // records one-FE df_a=8, dof(none) df_a=19, and
        // dof(firstpair) df_a=21 on `ols_core.csv`.
        let one_fe = prepared_case(&[("firm", &[0, 1, 2, 3, 4, 5, 6, 7])], TestWeights::Unit);
        let report =
            compute_prepared_ols_absorbed_dof(&one_fe, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("one-FE DoF");
        assert_eq!(report.absorbed_rank, 8);
        assert_eq!(report.initial_categories, 8);
        assert_eq!(report.redundant_categories, 0);
        assert_eq!(report.effects[0].categories, 8);
        assert_eq!(report.effects[0].coefficients, 8);
        assert!(report.effects[0].redundancy_exact);

        let worker = (0..24).map(|row| (row % 12) as u32).collect::<Vec<_>>();
        let firm = (0..24).map(|row| (row % 8) as u32).collect::<Vec<_>>();
        let none = prepared_case(&[("worker", &worker), ("firm", &firm)], TestWeights::Unit);
        let report = compute_prepared_ols_absorbed_dof(&none, PreparedOlsAbsorbedDofPolicy::None)
            .expect("dof(none)");
        assert_eq!(report.absorbed_rank, 19);
        assert_eq!(report.initial_categories, 20);
        assert_eq!(report.redundant_categories, 1);
        assert_eq!(report.effects[0].categories, 12);
        assert_eq!(report.effects[0].redundant, 0);
        assert_eq!(report.effects[0].coefficients, 12);
        assert!(report.effects[0].redundancy_exact);
        assert_eq!(report.effects[1].categories, 8);
        assert_eq!(report.effects[1].redundant, 1);
        assert_eq!(report.effects[1].coefficients, 7);
        assert!(!report.effects[1].redundancy_exact);
        assert!(report.is_approximate);

        let worker = (0..24).map(|row| (row / 2) as u32).collect::<Vec<_>>();
        let firm = (0..24)
            .map(|row| {
                let worker = row / 2;
                let local = worker % 6;
                let component_offset = if worker < 6 { 0 } else { 4 };
                (component_offset + (local + row % 2) % 4) as u32
            })
            .collect::<Vec<_>>();
        let year = (0..24).map(|row| (row % 4) as u32).collect::<Vec<_>>();
        let first_pair = prepared_case(
            &[("worker", &worker), ("firm", &firm), ("year", &year)],
            TestWeights::Unit,
        );
        let report =
            compute_prepared_ols_absorbed_dof(&first_pair, PreparedOlsAbsorbedDofPolicy::FirstPair)
                .expect("dof(firstpair)");
        assert_eq!(report.absorbed_rank, 21);
        assert_eq!(report.initial_categories, 24);
        assert_eq!(report.redundant_categories, 3);
        assert_eq!(report.effects[0].categories, 12);
        assert_eq!(report.effects[0].redundant, 0);
        assert_eq!(report.effects[0].coefficients, 12);
        assert_eq!(report.effects[1].categories, 8);
        assert_eq!(report.effects[1].redundant, 2);
        assert_eq!(report.effects[1].coefficients, 6);
        assert!(report.effects[1].redundancy_exact);
        assert_eq!(report.effects[2].categories, 4);
        assert_eq!(report.effects[2].redundant, 1);
        assert_eq!(report.effects[2].coefficients, 3);
        assert!(!report.effects[2].redundancy_exact);
        assert!(report.is_approximate);
    }

    #[test]
    fn pairwise_policy_differs_from_first_pair_and_global_multipartite_formula() {
        assert_eq!(
            PreparedOlsAbsorbedDofPolicy::default(),
            PreparedOlsAbsorbedDofPolicy::Pairwise
        );
        let a = [0, 1];
        let b = [0, 0];
        let c = [0, 1];
        let prepared = prepared_case(&[("a", &a), ("b", &b), ("c", &c)], TestWeights::Unit);
        let none = compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::None)
            .expect("none");
        let first =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::FirstPair)
                .expect("first pair");
        let pairwise =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("pairwise");
        assert_eq!(none.absorbed_rank, 3);
        assert_eq!(first.absorbed_rank, 3);
        assert_eq!(pairwise.absorbed_rank, 2);
        assert_eq!(pairwise.effects[2].redundant, 2);

        let a = [0, 1];
        let b = [0, 1];
        let c = [0, 0];
        let prepared = prepared_case(&[("a", &a), ("b", &b), ("c", &c)], TestWeights::Unit);
        let none = compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::None)
            .expect("none");
        let first =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::FirstPair)
                .expect("first pair");
        assert_eq!(none.absorbed_rank, 3);
        assert_eq!(first.absorbed_rank, 2);
    }

    #[test]
    fn repeated_sources_are_not_counted_twice_under_pairwise_policy() {
        let g = [0, 1, 0, 1];
        let h = [0, 0, 0, 0];
        let prepared = prepared_case_ordered(
            &[("g", &g), ("h", &h)],
            &[("g_first", 0), ("h", 1), ("g_again", 0)],
            TestWeights::Unit,
        );
        let pairwise =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("pairwise repeated source");
        assert_eq!(prepared.fe_topology.sources().len(), 2);
        assert_eq!(pairwise.absorbed_rank, 2);
        assert_eq!(pairwise.effects[2].redundant, 2);
        assert_eq!(pairwise.effects[2].coefficients, 0);
        assert!(pairwise.effects.iter().all(|effect| !effect.nested));
        assert_eq!(
            pairwise
                .effects
                .iter()
                .map(|effect| match effect.identity {
                    PreparedOlsAbsorbedDofEffectIdentity::Explicit { term_id, .. } => term_id.get(),
                    PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb => {
                        panic!("unexpected implicit row")
                    }
                })
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            pairwise
                .effects
                .iter()
                .map(|effect| effect.label.as_str())
                .collect::<Vec<_>>(),
            ["g_first", "h", "g_again"]
        );
    }

    #[test]
    fn topology_rank_is_weight_kind_and_row_order_invariant() {
        let g = [0, 0, 1, 1, 2, 2];
        let h = [0, 1, 0, 1, 0, 1];
        let analytic = [0.5, 2.0, 1.0, 4.0, 3.0, 1.5];
        let probability = [5.0, 1.0, 3.0, 2.0, 8.0, 4.0];
        let frequency = [2_u32, 1, 3, 1, 4, 2];
        let reports = [
            prepared_case(&[("g", &g), ("h", &h)], TestWeights::Unit),
            prepared_case(&[("g", &g), ("h", &h)], TestWeights::Analytic(&analytic)),
            prepared_case(
                &[("g", &g), ("h", &h)],
                TestWeights::Probability(&probability),
            ),
            prepared_case(&[("g", &g), ("h", &h)], TestWeights::Frequency(&frequency)),
        ]
        .iter()
        .map(|prepared| {
            compute_prepared_ols_absorbed_dof(prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("weighted topology DoF")
                .absorbed_rank
        })
        .collect::<Vec<_>>();
        assert!(reports.iter().all(|rank| *rank == reports[0]));

        let permutation = [5_usize, 2, 0, 4, 1, 3];
        let permuted_g = permutation.map(|row| g[row]);
        let permuted_h = permutation.map(|row| h[row]);
        let permuted = prepared_case(&[("g", &permuted_g), ("h", &permuted_h)], TestWeights::Unit);
        assert_eq!(
            compute_prepared_ols_absorbed_dof(&permuted, PreparedOlsAbsorbedDofPolicy::Pairwise,)
                .expect("permuted DoF")
                .absorbed_rank,
            reports[0]
        );
    }

    #[test]
    fn report_does_not_depend_on_outcome_or_unrelated_prepared_columns() {
        let g = [0, 0, 1, 1];
        let h = [0, 1, 0, 1];
        let mut prepared = prepared_case(&[("g", &g), ("h", &h)], TestWeights::Unit);
        let expected =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("baseline structural DoF");
        prepared
            .sample
            .outcome
            .copy_from_slice(&[9.0, -4.0, 2.5, 1.0]);
        prepared.sample.regressors.push(vec![3.0, 8.0, -1.0, 0.5]);
        let actual =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("structural DoF with unrelated numeric changes");
        assert_eq!(actual, expected);
    }

    #[test]
    fn computation_does_not_reserve_observation_scale_workspaces() {
        let g = (0..4096).map(|row| (row % 2) as u32).collect::<Vec<_>>();
        let h = (0..4096)
            .map(|row| ((row / 2) % 2) as u32)
            .collect::<Vec<_>>();
        let prepared = prepared_case(&[("g", &g), ("h", &h)], TestWeights::Unit);
        let mut reservation = InjectedReservation::default();
        compute_with_reservation(
            &prepared,
            PreparedOlsAbsorbedDofPolicy::Pairwise,
            &mut reservation,
        )
        .expect("level-scale absorbed-DoF workspaces");
        assert_eq!(prepared.fe_topology.nobs(), 4096);
        assert_eq!(reservation.max_additional, 4);
    }

    #[test]
    fn no_absorb_has_one_implicit_rank_while_empty_explicit_topology_is_zero_rank() {
        let no_effects = prepared_case_n(4, &[], TestWeights::Unit);
        let report =
            compute_prepared_ols_absorbed_dof(&no_effects, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("no-effect DoF");
        assert_eq!(report.absorbed_rank, 1);
        assert_eq!(report.initial_categories, 1);
        assert_eq!(report.effects.len(), 1);
        assert_eq!(
            report.effects[0].identity,
            PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb
        );
        assert_eq!(report.effects[0].label, "_cons");
        assert!(!report.is_approximate);

        let empty: [u32; 0] = [];
        let empty_effect = prepared_case(&[("empty", &empty)], TestWeights::Unit);
        let report = compute_prepared_ols_absorbed_dof(
            &empty_effect,
            PreparedOlsAbsorbedDofPolicy::Pairwise,
        )
        .expect("empty structural DoF");
        assert_eq!(report.absorbed_rank, 0);
        assert_eq!(report.effects[0].categories, 0);
    }

    #[test]
    fn corrupted_codes_and_topology_mismatches_fail_closed() {
        let g = [0, 0, 1, 1];
        let mut out_of_range = prepared_case(&[("g", &g)], TestWeights::Unit);
        out_of_range.categorical_sources[0].encoded.codes[0] = 99;
        assert_eq!(
            compute_prepared_ols_absorbed_dof(
                &out_of_range,
                PreparedOlsAbsorbedDofPolicy::Pairwise,
            )
            .expect_err("out-of-range code")
            .code,
            ErrorCode::InvalidData
        );

        let mut membership_mismatch = prepared_case(&[("g", &g)], TestWeights::Unit);
        membership_mismatch.categorical_sources[0]
            .encoded
            .codes
            .swap(0, 2);
        assert_eq!(
            compute_prepared_ols_absorbed_dof(
                &membership_mismatch,
                PreparedOlsAbsorbedDofPolicy::Pairwise,
            )
            .expect_err("membership mismatch")
            .code,
            ErrorCode::InvalidArgument
        );

        let mut length_mismatch = prepared_case(&[("g", &g)], TestWeights::Unit);
        length_mismatch.categorical_sources[0].encoded.codes.pop();
        assert_eq!(
            compute_prepared_ols_absorbed_dof(
                &length_mismatch,
                PreparedOlsAbsorbedDofPolicy::Pairwise,
            )
            .expect_err("length mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn pair_validation_rejects_lengths_codes_and_node_overflow() {
        let mut reservation = InjectedReservation::default();
        assert_eq!(
            pair_component_count(&[0], 1, &[], 0, &mut reservation)
                .expect_err("mismatched lengths")
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            pair_component_count(&[1], 1, &[0], 1, &mut reservation)
                .expect_err("bad code")
                .code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            pair_component_count(&[0], usize::MAX, &[0], 1, &mut reservation)
                .expect_err("node overflow")
                .code,
            ErrorCode::Allocation
        );
    }

    #[test]
    fn pair_union_find_matches_independent_graph_search() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        for _case in 0..256 {
            let left_levels = 1 + (next_test_u64(&mut state) % 5) as usize;
            let right_levels = 1 + (next_test_u64(&mut state) % 5) as usize;
            let n = 32;
            let mut left = (0..n)
                .map(|_| (next_test_u64(&mut state) as usize % left_levels) as u32)
                .collect::<Vec<_>>();
            let mut right = (0..n)
                .map(|_| (next_test_u64(&mut state) as usize % right_levels) as u32)
                .collect::<Vec<_>>();
            for (level, code) in left.iter_mut().take(left_levels).enumerate() {
                *code = u32::try_from(level).expect("small left level");
            }
            for (level, code) in right.iter_mut().take(right_levels).enumerate() {
                *code = u32::try_from(level).expect("small right level");
            }

            let expected = graph_search_components(&left, left_levels, &right, right_levels);
            let mut reservation = InjectedReservation::default();
            assert_eq!(
                pair_component_count(&left, left_levels, &right, right_levels, &mut reservation,)
                    .expect("union-find components"),
                expected
            );
        }
    }

    #[test]
    fn iterative_find_compresses_paths_and_rejects_corrupt_parents() {
        let mut chain = vec![1, 2, 3, 3];
        assert_eq!(find_root(&mut chain, 0).expect("chain root"), 3);
        assert_eq!(chain, [3, 3, 3, 3]);

        let mut cycle = vec![1, 0];
        assert_eq!(
            find_root(&mut cycle, 0).expect_err("cycle must fail").code,
            ErrorCode::InvalidArgument
        );
        let mut out_of_range = vec![2, 1];
        assert_eq!(
            find_root(&mut out_of_range, 0)
                .expect_err("bad parent must fail")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn every_reservation_failure_and_undersized_capacity_fails_closed() {
        let a = [0, 1, 0, 1];
        let b = [0, 0, 1, 1];
        let c = [0, 1, 1, 0];
        let prepared = prepared_case(&[("a", &a), ("b", &b), ("c", &c)], TestWeights::Unit);
        let mut successful = InjectedReservation::default();
        compute_with_reservation(
            &prepared,
            PreparedOlsAbsorbedDofPolicy::Pairwise,
            &mut successful,
        )
        .expect("successful injected reservation");
        assert!(successful.calls > 0);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            assert_eq!(
                compute_with_reservation(
                    &prepared,
                    PreparedOlsAbsorbedDofPolicy::Pairwise,
                    &mut injected,
                )
                .expect_err("injected allocation failure")
                .code,
                ErrorCode::Allocation,
                "reservation call {fail_at}"
            );
        }
        for undersize_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                undersize_at: Some(undersize_at),
                ..InjectedReservation::default()
            };
            assert_eq!(
                compute_with_reservation(
                    &prepared,
                    PreparedOlsAbsorbedDofPolicy::Pairwise,
                    &mut injected,
                )
                .expect_err("undersized reservation")
                .code,
                ErrorCode::Allocation,
                "reservation call {undersize_at}"
            );
        }
    }

    #[test]
    fn implicit_no_absorb_row_and_label_reservations_fail_closed() {
        let prepared = prepared_case_n(4, &[], TestWeights::Unit);
        let expected = vec![
            (1, "unable to reserve prepared absorbed-DoF effect rows"),
            (5, "unable to reserve implicit no-absorb DoF label"),
        ];
        let mut successful = InjectedReservation::default();
        let report = compute_with_reservation(
            &prepared,
            PreparedOlsAbsorbedDofPolicy::Pairwise,
            &mut successful,
        )
        .expect("successful implicit no-absorb DoF reservation");
        assert_eq!(successful.records, expected);
        assert_eq!(successful.calls, expected.len());
        assert_eq!(report.effects.len(), 1);
        assert_eq!(
            report.effects[0].identity,
            PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb
        );
        assert_eq!(report.effects[0].label, "_cons");

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            assert_eq!(
                compute_with_reservation(
                    &prepared,
                    PreparedOlsAbsorbedDofPolicy::Pairwise,
                    &mut injected,
                )
                .expect_err("implicit no-absorb allocation failure")
                .code,
                ErrorCode::Allocation,
                "reservation call {fail_at}"
            );
        }
        for undersize_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                undersize_at: Some(undersize_at),
                ..InjectedReservation::default()
            };
            assert_eq!(
                compute_with_reservation(
                    &prepared,
                    PreparedOlsAbsorbedDofPolicy::Pairwise,
                    &mut injected,
                )
                .expect_err("undersized implicit no-absorb reservation")
                .code,
                ErrorCode::Allocation,
                "reservation call {undersize_at}"
            );
        }
    }

    fn prepared_case(effects: &[(&str, &[u32])], weights: TestWeights<'_>) -> PreparedOlsData {
        let n = effects.first().map_or(0, |(_, codes)| codes.len());
        prepared_case_n(n, effects, weights)
    }

    fn prepared_case_n(
        n: usize,
        effects: &[(&str, &[u32])],
        weights: TestWeights<'_>,
    ) -> PreparedOlsData {
        assert!(effects.iter().all(|(_, codes)| codes.len() == n));
        let effect_order = effects
            .iter()
            .enumerate()
            .map(|(index, (label, _))| (*label, index))
            .collect::<Vec<_>>();
        prepared_case_ordered_n(n, effects, &effect_order, weights)
    }

    fn prepared_case_ordered(
        sources_with_codes: &[(&str, &[u32])],
        effect_order: &[(&str, usize)],
        weights: TestWeights<'_>,
    ) -> PreparedOlsData {
        let n = sources_with_codes
            .first()
            .map_or(0, |(_, codes)| codes.len());
        prepared_case_ordered_n(n, sources_with_codes, effect_order, weights)
    }

    fn prepared_case_ordered_n(
        n: usize,
        sources_with_codes: &[(&str, &[u32])],
        effect_order: &[(&str, usize)],
        weights: TestWeights<'_>,
    ) -> PreparedOlsData {
        assert!(sources_with_codes.iter().all(|(_, codes)| codes.len() == n));
        assert!(effect_order
            .iter()
            .all(|(_, source_index)| *source_index < sources_with_codes.len()));
        let sources = sources_with_codes
            .iter()
            .enumerate()
            .map(|(index, (label, _))| SourceEntry {
                id: source_id(index),
                canonical_name: (*label).to_owned(),
            })
            .collect::<Vec<_>>();
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ];
        bindings.extend((0..sources_with_codes.len()).map(|index| OlsInputBinding {
            stata_var_position: u32::try_from(index + 3).expect("small source position"),
            role: OlsInputRole::NumericSource(source_id(index)),
        }));
        if let Some(role) = weight_role(weights) {
            bindings.push(OlsInputBinding {
                stata_var_position: u32::try_from(sources_with_codes.len() + 3)
                    .expect("small weight position"),
                role,
            });
        }
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources,
                terms: Vec::new(),
            },
            absorbed_terms: effect_order
                .iter()
                .enumerate()
                .map(|(index, (label, source_index))| AbsorbedTerm {
                    id: AbsorbTermId::new(u32::try_from(index + 1).expect("small term"))
                        .expect("term id"),
                    intercept: true,
                    categorical_sources: vec![source_id(*source_index)],
                    slope_sources: Vec::new(),
                    label: (*label).to_owned(),
                    saved_name: String::new(),
                })
                .collect(),
            bindings,
            include_intercept: false,
            keep_singletons: true,
        };
        let raw_weights = weight_values(weights);
        let loaded = LoadedOlsInput {
            candidate: vec![true; n],
            outcome: loaded_column(vec![1.0; n]),
            numeric_sources: sources_with_codes
                .iter()
                .enumerate()
                .map(|(index, (_, codes))| {
                    (
                        source_id(index),
                        loaded_column(codes.iter().map(|code| f64::from(*code)).collect()),
                    )
                })
                .collect(),
            analytic_weight: matches!(weights, TestWeights::Analytic(_))
                .then(|| loaded_column(raw_weights.clone())),
            frequency_weight: matches!(weights, TestWeights::Frequency(_))
                .then(|| loaded_column(raw_weights.clone())),
            probability_weight: matches!(weights, TestWeights::Probability(_))
                .then(|| loaded_column(raw_weights)),
        };
        prepare_ols_data(&request, &loaded).expect("prepare absorbed-DoF case")
    }

    fn source_id(index: usize) -> SourceId {
        SourceId::new(u32::try_from(index + 1).expect("small source")).expect("source id")
    }

    fn loaded_column(values: Vec<f64>) -> LoadedNumericColumn {
        LoadedNumericColumn {
            missing: vec![false; values.len()],
            values,
        }
    }

    fn weight_role(weights: TestWeights<'_>) -> Option<OlsInputRole> {
        match weights {
            TestWeights::Unit => None,
            TestWeights::Analytic(_) => Some(OlsInputRole::AnalyticWeight),
            TestWeights::Frequency(_) => Some(OlsInputRole::FrequencyWeight),
            TestWeights::Probability(_) => Some(OlsInputRole::ProbabilityWeight),
        }
    }

    fn weight_values(weights: TestWeights<'_>) -> Vec<f64> {
        match weights {
            TestWeights::Unit => Vec::new(),
            TestWeights::Analytic(values) | TestWeights::Probability(values) => values.to_vec(),
            TestWeights::Frequency(values) => {
                values.iter().map(|value| f64::from(*value)).collect()
            }
        }
    }

    fn next_test_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    fn graph_search_components(
        left: &[u32],
        left_levels: usize,
        right: &[u32],
        right_levels: usize,
    ) -> usize {
        let nodes = left_levels + right_levels;
        let mut adjacency = vec![Vec::new(); nodes];
        for (&left_code, &right_code) in left.iter().zip(right) {
            let left_node = left_code as usize;
            let right_node = left_levels + right_code as usize;
            adjacency[left_node].push(right_node);
            adjacency[right_node].push(left_node);
        }
        let mut seen = vec![false; nodes];
        let mut components = 0;
        for start in 0..nodes {
            if seen[start] {
                continue;
            }
            components += 1;
            seen[start] = true;
            let mut stack = vec![start];
            while let Some(node) = stack.pop() {
                for neighbor in &adjacency[node] {
                    if !seen[*neighbor] {
                        seen[*neighbor] = true;
                        stack.push(*neighbor);
                    }
                }
            }
        }
        components
    }
}
