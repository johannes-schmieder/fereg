//! Final-sample compaction and topology construction for categorical effects.
//!
//! Candidate partitions are useful for singleton pruning, but their codes still
//! live on the pre-pruning row axis. This module compacts only referenced raw
//! categorical sources to retained rows, preserves each final level's original
//! compact identity, re-factorizes interactions on that final sample, and then
//! constructs compressed solver membership. Unreferenced source columns are not
//! copied.

use crate::error::{ErrorCode, FeregError, Result};

use super::{CategoricalCodeView, CategoricalTopologyRegistry};

/// One retained raw categorical source on the final observation axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactedCategoricalSource {
    candidate_source_index: usize,
    codes: Vec<u32>,
    original_levels: Vec<u32>,
}

impl CompactedCategoricalSource {
    #[must_use]
    pub const fn candidate_source_index(&self) -> usize {
        self.candidate_source_index
    }

    #[must_use]
    pub fn codes(&self) -> &[u32] {
        &self.codes
    }

    #[must_use]
    pub fn original_levels(&self) -> &[u32] {
        &self.original_levels
    }

    #[must_use]
    pub fn nobs(&self) -> usize {
        self.codes.len()
    }

    #[must_use]
    pub fn n_levels(&self) -> usize {
        self.original_levels.len()
    }

    #[must_use]
    pub fn view(&self) -> CategoricalCodeView<'_> {
        CategoricalCodeView::new(&self.codes, self.original_levels.len())
    }
}

/// Final compact source table, effect mapping, and deduplicated solver topology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedCategoricalTopology {
    compacted_sources: Vec<CompactedCategoricalSource>,
    candidate_to_compacted_source: Vec<Option<usize>>,
    remapped_effect_recipes: Vec<Vec<usize>>,
    effect_to_topology: Vec<usize>,
    retained_rows: Vec<usize>,
    registry: CategoricalTopologyRegistry,
}

// The private field prevents a caller from pairing a validation receipt with
// replaced codes or membership. No mutable or consuming accessor is provided.
mod validated_owner {
    use super::{FinalizedCategoricalTopology, Result};

    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct ValidatedFinalizedTopology(FinalizedCategoricalTopology);

    impl ValidatedFinalizedTopology {
        pub(crate) fn new(topology: FinalizedCategoricalTopology) -> Result<Self> {
            topology.validate()?;
            Ok(Self(topology))
        }

        pub(crate) const fn get(&self) -> &FinalizedCategoricalTopology {
            &self.0
        }
    }

    impl std::ops::Deref for ValidatedFinalizedTopology {
        type Target = FinalizedCategoricalTopology;

        fn deref(&self) -> &Self::Target { self.get() }
    }
}

pub(crate) use validated_owner::ValidatedFinalizedTopology;

impl ValidatedFinalizedTopology {
    pub(crate) fn single_source(&self) -> Result<Option<ValidatedSingleTopology<'_>>> {
        let Some(source) = super::design::ValidatedBorrowedInterceptSource::from_finalized_owner(self)
        else {
            return self.get().validate_single_source();
        };
        Ok(Some(ValidatedSingleTopology { topology: self.get(), source }))
    }
}

/// A same-borrow validation receipt, bound to the immutable finalized owner.
/// Recovery can build its single-effect view without scanning membership again.
pub(crate) struct ValidatedSingleTopology<'a> {
    topology: &'a FinalizedCategoricalTopology,
    source: super::design::ValidatedBorrowedInterceptSource<'a>,
}

impl<'a> ValidatedSingleTopology<'a> {
    pub(crate) fn matches_owner(&self, topology: &FinalizedCategoricalTopology) -> bool {
        std::ptr::eq(self.topology, topology)
    }

    pub(crate) fn into_design(self, nobs: usize) -> Result<super::BorrowedInterceptFeDesign<'a>> {
        if self.source.effect().codes().len() != nobs {
            return Err(invalid("categorical topology source observation count differs from design"));
        }
        if self.topology.effect_to_topology[0] != 0 {
            return Err(invalid("categorical effect-to-topology index is out of range"));
        }
        let mut effects = Vec::new();
        effects.try_reserve_exact(1)
            .map_err(|_| allocation("unable to reserve categorical topology term views"))?;
        effects.push(self.source.effect());
        super::BorrowedInterceptFeDesign::new(nobs, effects)
    }
}

impl FinalizedCategoricalTopology {
    /// Preserve an unchanged compact row axis while moving single-source
    /// membership. Source storage follows candidate order; registry storage
    /// follows first effect use, exactly as in ordinary finalization.
    pub(crate) fn from_single_source_indexes(
        candidate_source_count: usize,
        nobs: usize,
        effect_recipes: &[&[usize]],
        take_source: impl FnMut(usize) -> Result<(Vec<u32>, usize, super::design::LevelIndex)>,
    ) -> Result<Self> {
        Self::from_single_source_indexes_with_reservation(
            candidate_source_count, nobs, effect_recipes, take_source,
            &mut SystemTopologyFinalizeReservation,
        )
    }

    fn from_single_source_indexes_with_reservation<R: TopologyFinalizeReservation>(
        candidate_source_count: usize,
        nobs: usize,
        effect_recipes: &[&[usize]],
        mut take_source: impl FnMut(usize) -> Result<(Vec<u32>, usize, super::design::LevelIndex)>,
        reservation: &mut R,
    ) -> Result<Self> {
        if effect_recipes.is_empty() || effect_recipes.iter().any(|r| r.len() != 1 || r[0] >= candidate_source_count) {
            return Err(invalid("transferred effect source identity is out of range"));
        }
        let mut referenced = Vec::new();
        reservation.reserve_vec(&mut referenced, effect_recipes.len(),
            "unable to reserve transferred source identities")?;
        for recipe in effect_recipes {
            let source = recipe[0];
            push_reserved(&mut referenced, source, "transferred source identity capacity exhausted")?;
        }
        referenced.sort_unstable();
        referenced.dedup();
        let mut source_map = reserved_filled(reservation, candidate_source_count, None,
            "unable to reserve transferred source map")?;
        let mut compacted_sources = Vec::new();
        let mut memberships = Vec::new();
        reservation.reserve_vec(&mut compacted_sources, referenced.len(),
            "unable to reserve transferred compact sources")?;
        reservation.reserve_vec(&mut memberships, referenced.len(),
            "unable to reserve transferred membership owners")?;
        for candidate_source_index in referenced {
            let compacted_index = compacted_sources.len();
            let (codes, n_levels, index) = take_source(candidate_source_index)?;
            if codes.len() != nobs || n_levels > u32::MAX as usize {
                return Err(invalid("transferred categorical row or level axis differs"));
            }
            // This checks every code/member relationship and every declared
            // level before the unchanged level identities become authoritative.
            let source = super::CategoricalTopologySource::single_with_index(
                compacted_index, CategoricalCodeView::new(&codes, n_levels), index,
            )?;
            let mut original_levels = reserved_filled(reservation, n_levels, 0,
                "unable to reserve transferred level identities")?;
            for (level, original) in original_levels.iter_mut().enumerate() {
                *original = level as u32;
            }
            source_map[candidate_source_index] = Some(compacted_index);
            push_reserved(&mut compacted_sources,
                CompactedCategoricalSource { candidate_source_index, codes, original_levels },
                "transferred source capacity exhausted")?;
            push_reserved(&mut memberships, Some(source), "transferred membership capacity exhausted")?;
        }
        let mut registry_sources = Vec::new();
        reservation.reserve_vec(&mut registry_sources, compacted_sources.len(),
            "unable to reserve transferred topology registry")?;
        let mut topology_map = reserved_filled(reservation, compacted_sources.len(), None,
            "unable to reserve transferred topology map")?;
        let mut remapped_effect_recipes = Vec::new();
        let mut effect_to_topology = Vec::new();
        reservation.reserve_vec(&mut remapped_effect_recipes, effect_recipes.len(),
            "unable to reserve transferred effect recipes")?;
        reservation.reserve_vec(&mut effect_to_topology, effect_recipes.len(),
            "unable to reserve transferred effect map")?;
        for recipe in effect_recipes {
            let candidate_source = recipe[0];
            let compacted = source_map[candidate_source]
                .ok_or_else(|| invalid("transferred effect source was not retained"))?;
            let topology = if let Some(index) = topology_map[compacted] { index } else {
                let index = registry_sources.len();
                let source = memberships[compacted].take()
                    .ok_or_else(|| invalid("transferred membership owner was already consumed"))?;
                push_reserved(&mut registry_sources, source, "transferred registry capacity exhausted")?;
                topology_map[compacted] = Some(index);
                index
            };
            let recipe = reserved_filled(reservation, 1, compacted,
                "unable to reserve transferred effect recipe")?;
            push_reserved(&mut remapped_effect_recipes, recipe, "transferred recipe capacity exhausted")?;
            push_reserved(&mut effect_to_topology, topology, "transferred effect capacity exhausted")?;
        }
        let mut retained_rows = reserved_filled(reservation, nobs, 0,
            "unable to reserve transferred row identities")?;
        for (row, original) in retained_rows.iter_mut().enumerate() { *original = row; }
        Ok(Self {
            compacted_sources, candidate_to_compacted_source: source_map,
            remapped_effect_recipes, effect_to_topology, retained_rows,
            registry: CategoricalTopologyRegistry::from_transferred_sources(registry_sources),
        })
    }

    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        bytes(&self.compacted_sources)
            + self.compacted_sources.iter().map(|source| bytes(&source.codes) + bytes(&source.original_levels)).sum::<u128>()
            + bytes(&self.candidate_to_compacted_source)
            + bytes(&self.remapped_effect_recipes)
            + self.remapped_effect_recipes.iter().map(bytes).sum::<u128>()
            + bytes(&self.effect_to_topology)
            + bytes(&self.retained_rows)
            + self.registry.owned_buffer_bytes()
    }

    #[must_use]
    pub fn compacted_sources(&self) -> &[CompactedCategoricalSource] {
        &self.compacted_sources
    }

    #[must_use]
    pub fn candidate_to_compacted_source(&self) -> &[Option<usize>] {
        &self.candidate_to_compacted_source
    }

    #[must_use]
    pub fn remapped_effect_recipes(&self) -> &[Vec<usize>] {
        &self.remapped_effect_recipes
    }

    #[must_use]
    pub fn effect_to_topology(&self) -> &[usize] {
        &self.effect_to_topology
    }

    #[must_use]
    pub fn retained_rows(&self) -> &[usize] {
        &self.retained_rows
    }

    #[must_use]
    pub const fn registry(&self) -> &CategoricalTopologyRegistry {
        &self.registry
    }

    pub fn source_for_candidate(
        &self,
        candidate_source_index: usize,
    ) -> Result<&CompactedCategoricalSource> {
        let compacted_index = self
            .candidate_to_compacted_source
            .get(candidate_source_index)
            .copied()
            .flatten()
            .ok_or_else(|| invalid("candidate categorical source is not retained"))?;
        self.compacted_sources.get(compacted_index).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "candidate-to-compacted categorical source map is inconsistent",
            )
        })
    }

    /// Revalidate all final topology sources against the retained compact source
    /// table.
    pub fn validate(&self) -> Result<()> {
        let views = self.source_views()?;
        self.registry.validate_all(&views)
    }

    pub(crate) fn validate_single_source(&self) -> Result<Option<ValidatedSingleTopology<'_>>> {
        if self.compacted_sources.len() != 1 || self.registry.sources().len() != 1
            || self.effect_to_topology.len() != 1
        {
            return Ok(None);
        }
        let views = [self.compacted_sources[0].view()];
        let source = self.registry.sources()[0].validated_borrowed(&views)?;
        Ok(Some(ValidatedSingleTopology { topology: self, source }))
    }

    /// Validation releases each re-factorized interaction before checking the
    /// next source. Dense mappings are bounded by the displaced row ordering.
    pub(crate) fn validation_workspace_bytes(&self) -> Result<u128> {
        let view_bytes = std::mem::size_of::<CategoricalCodeView<'_>>() as u128;
        let views = (self.compacted_sources.len() as u128)
            .checked_mul(view_bytes)
            .ok_or_else(|| allocation("categorical validation views overflow"))?;
        let mut peak = 0;
        for source in self.registry.sources() {
            if source.partition().is_interaction() {
                let component_views = (source.partition().categorical_source_indices().len()
                    as u128)
                    .checked_mul(view_bytes)
                    .ok_or_else(|| allocation("categorical validation components overflow"))?;
                let factorization = (source.nobs() as u128)
                    .checked_mul(
                        (std::mem::size_of::<usize>() + std::mem::size_of::<u32>()) as u128,
                    )
                    .ok_or_else(|| allocation("categorical validation factorization overflows"))?;
                peak = peak.max(
                    component_views
                        .checked_add(factorization)
                        .ok_or_else(|| allocation("categorical validation peak overflows"))?,
                );
            }
        }
        views
            .checked_add(peak)
            .ok_or_else(|| allocation("categorical validation workspace overflows"))
    }

    fn source_views(&self) -> Result<Vec<CategoricalCodeView<'_>>> {
        let mut views = Vec::new();
        views
            .try_reserve_exact(self.compacted_sources.len())
            .map_err(|_| allocation("unable to reserve final categorical source views"))?;
        for source in &self.compacted_sources {
            if views.len() == views.capacity() {
                return Err(allocation(
                    "final categorical source-view capacity exhausted",
                ));
            }
            views.push(source.view());
        }
        Ok(views)
    }
}

/// Compact referenced categorical sources and construct final solver topology.
pub fn finalize_categorical_topology(
    candidate_sources: &[CategoricalCodeView<'_>],
    effect_recipes: &[&[usize]],
    active: &[bool],
) -> Result<FinalizedCategoricalTopology> {
    let mut reservation = SystemTopologyFinalizeReservation;
    finalize_categorical_topology_with_reservation(
        candidate_sources,
        effect_recipes,
        active,
        &mut reservation,
    )
}

trait TopologyFinalizeReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemTopologyFinalizeReservation;

impl TopologyFinalizeReservation for SystemTopologyFinalizeReservation {
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

fn finalize_categorical_topology_with_reservation<R: TopologyFinalizeReservation>(
    candidate_sources: &[CategoricalCodeView<'_>],
    effect_recipes: &[&[usize]],
    active: &[bool],
    reservation: &mut R,
) -> Result<FinalizedCategoricalTopology> {
    for (source_index, source) in candidate_sources.iter().copied().enumerate() {
        if source.codes().len() != active.len() {
            return Err(invalid(format!(
                "candidate categorical source {} has {} rows; expected {}",
                source_index + 1,
                source.codes().len(),
                active.len()
            )));
        }
    }

    let retained_count = active.iter().filter(|value| **value).count();
    let mut retained_rows = Vec::new();
    reservation.reserve_vec(
        &mut retained_rows,
        retained_count,
        "unable to reserve retained categorical row map",
    )?;
    for (row, retained) in active.iter().copied().enumerate() {
        if retained {
            push_reserved(
                &mut retained_rows,
                row,
                "retained categorical row-map capacity exhausted",
            )?;
        }
    }

    let total_recipe_arity = effect_recipes.iter().try_fold(0_usize, |total, recipe| {
        total
            .checked_add(recipe.len())
            .ok_or_else(|| allocation("categorical effect recipe arity overflows"))
    })?;
    let mut referenced_candidates = Vec::new();
    reservation.reserve_vec(
        &mut referenced_candidates,
        total_recipe_arity,
        "unable to reserve referenced categorical source indices",
    )?;
    for recipe in effect_recipes {
        if recipe.is_empty() {
            return Err(invalid(
                "final categorical topology recipe requires at least one source",
            ));
        }
        for source_index in recipe.iter().copied() {
            if source_index >= candidate_sources.len() {
                return Err(invalid(
                    "final categorical topology recipe source index is out of range",
                ));
            }
            push_reserved(
                &mut referenced_candidates,
                source_index,
                "referenced categorical source-index capacity exhausted",
            )?;
        }
    }
    referenced_candidates.sort_unstable();
    referenced_candidates.dedup();

    let mut candidate_to_compacted_source = reserved_filled(
        reservation,
        candidate_sources.len(),
        None,
        "unable to reserve candidate-to-compacted categorical source map",
    )?;
    let mut compacted_sources = Vec::new();
    reservation.reserve_vec(
        &mut compacted_sources,
        referenced_candidates.len(),
        "unable to reserve compacted categorical sources",
    )?;
    for candidate_source_index in referenced_candidates.iter().copied() {
        let compacted_index = compacted_sources.len();
        let compacted = compact_categorical_source_with_reservation(
            candidate_source_index,
            candidate_sources[candidate_source_index],
            active,
            retained_count,
            reservation,
        )?;
        candidate_to_compacted_source[candidate_source_index] = Some(compacted_index);
        push_reserved(
            &mut compacted_sources,
            compacted,
            "compacted categorical source capacity exhausted",
        )?;
    }

    let mut remapped_effect_recipes = Vec::new();
    reservation.reserve_vec(
        &mut remapped_effect_recipes,
        effect_recipes.len(),
        "unable to reserve remapped categorical effect recipes",
    )?;
    for recipe in effect_recipes {
        let mut remapped = Vec::new();
        reservation.reserve_vec(
            &mut remapped,
            recipe.len(),
            "unable to reserve one remapped categorical effect recipe",
        )?;
        for candidate_source_index in recipe.iter().copied() {
            let compacted_index = candidate_to_compacted_source[candidate_source_index]
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "referenced categorical source was not compacted",
                    )
                })?;
            push_reserved(
                &mut remapped,
                compacted_index,
                "remapped categorical effect recipe capacity exhausted",
            )?;
        }
        push_reserved(
            &mut remapped_effect_recipes,
            remapped,
            "remapped categorical effect-recipe table capacity exhausted",
        )?;
    }

    let mut final_views = Vec::new();
    reservation.reserve_vec(
        &mut final_views,
        compacted_sources.len(),
        "unable to reserve final categorical source views",
    )?;
    for source in &compacted_sources {
        push_reserved(
            &mut final_views,
            source.view(),
            "final categorical source-view capacity exhausted",
        )?;
    }

    let mut registry = CategoricalTopologyRegistry::new();
    let mut effect_to_topology = Vec::new();
    reservation.reserve_vec(
        &mut effect_to_topology,
        remapped_effect_recipes.len(),
        "unable to reserve final effect-to-topology map",
    )?;
    for recipe in &remapped_effect_recipes {
        let topology_index = registry.intern(recipe, &final_views)?;
        push_reserved(
            &mut effect_to_topology,
            topology_index,
            "final effect-to-topology capacity exhausted",
        )?;
    }
    registry.validate_all(&final_views)?;

    Ok(FinalizedCategoricalTopology {
        compacted_sources,
        candidate_to_compacted_source,
        remapped_effect_recipes,
        effect_to_topology,
        retained_rows,
        registry,
    })
}

fn compact_categorical_source_with_reservation<R: TopologyFinalizeReservation>(
    candidate_source_index: usize,
    source: CategoricalCodeView<'_>,
    active: &[bool],
    retained_count: usize,
    reservation: &mut R,
) -> Result<CompactedCategoricalSource> {
    validate_candidate_source(source)?;

    let mut candidate_to_final = reserved_filled(
        reservation,
        source.n_levels(),
        u32::MAX,
        "unable to reserve categorical level compaction map",
    )?;
    for (row, retained) in active.iter().copied().enumerate() {
        if retained {
            candidate_to_final[source.codes()[row] as usize] = 0;
        }
    }

    let final_level_count = candidate_to_final
        .iter()
        .filter(|mapped| **mapped != u32::MAX)
        .count();
    if final_level_count > u32::MAX as usize {
        return Err(invalid_data(
            "final categorical source has too many levels for u32 codes",
        ));
    }
    let mut original_levels = Vec::new();
    reservation.reserve_vec(
        &mut original_levels,
        final_level_count,
        "unable to reserve original categorical level identities",
    )?;
    let mut next_level = 0_u32;
    for (candidate_level, mapped) in candidate_to_final.iter_mut().enumerate() {
        if *mapped == u32::MAX {
            continue;
        }
        *mapped = next_level;
        push_reserved(
            &mut original_levels,
            u32::try_from(candidate_level).map_err(|_| {
                invalid_data("candidate categorical level cannot be represented as u32")
            })?,
            "original categorical level-identity capacity exhausted",
        )?;
        next_level = next_level
            .checked_add(1)
            .ok_or_else(|| allocation("final categorical level count overflows"))?;
    }

    let mut codes = Vec::new();
    reservation.reserve_vec(
        &mut codes,
        retained_count,
        "unable to reserve final compact categorical codes",
    )?;
    for (row, retained) in active.iter().copied().enumerate() {
        if !retained {
            continue;
        }
        let mapped = candidate_to_final[source.codes()[row] as usize];
        if mapped == u32::MAX {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "retained categorical observation has no compact level",
            ));
        }
        push_reserved(
            &mut codes,
            mapped,
            "final compact categorical code capacity exhausted",
        )?;
    }
    if codes.len() != retained_count || original_levels.len() != final_level_count {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "final categorical source dimensions are inconsistent",
        ));
    }

    Ok(CompactedCategoricalSource {
        candidate_source_index,
        codes,
        original_levels,
    })
}

fn validate_candidate_source(source: CategoricalCodeView<'_>) -> Result<()> {
    if source.codes().is_empty() {
        if source.n_levels() != 0 {
            return Err(invalid(
                "empty candidate categorical source declares nonzero levels",
            ));
        }
        return Ok(());
    }
    if source.n_levels() == 0 {
        return Err(invalid(
            "nonempty candidate categorical source declares zero levels",
        ));
    }
    if source.n_levels() > u32::MAX as usize {
        return Err(invalid(
            "candidate categorical source level count exceeds u32::MAX",
        ));
    }
    if source
        .codes()
        .iter()
        .any(|code| *code as usize >= source.n_levels())
    {
        return Err(invalid_data(
            "candidate categorical source code exceeds its declared level count",
        ));
    }
    Ok(())
}

fn reserved_filled<T: Clone, R: TopologyFinalizeReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "final categorical topology reservation returned insufficient capacity",
        ));
    }
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

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
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

    impl TopologyFinalizeReservation for InjectedReservation {
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
                    "injected final categorical topology allocation failure",
                ));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test final categorical topology reservation failed"))
        }
    }

    #[test]
    fn single_source_validation_receipt_matches_adapter_and_rejects_corruption() {
        for rows in [1, 257, 16_385] {
            let levels = rows.min(31);
            let codes: Vec<_> = (0..rows).map(|r| (r % levels) as u32).collect();
            let make = || finalize_categorical_topology(
                &[CategoricalCodeView::new(&codes, levels)], &[&[0]], &vec![true; rows],
            ).unwrap();
            let topology = make();
            let views = topology.source_views().unwrap();
            for nobs in [rows, rows + 1] {
                let expected = super::super::borrow_categorical_topology_design(
                    nobs, topology.registry(), &views, topology.effect_to_topology(),
                );
                let actual = topology.validate_single_source().unwrap().unwrap().into_design(nobs);
                assert_eq!(actual, expected);
            }
            for fault in 0..3 {
                let mut malformed = make();
                match fault {
                    0 => malformed.compacted_sources[0].codes[0] = u32::MAX,
                    1 => malformed.compacted_sources[0].original_levels.clear(),
                    _ => malformed.effect_to_topology[0] = 1,
                }
                let expected = malformed.validate();
                let actual = malformed.validate_single_source();
                match (expected, actual) {
                    (Err(a), Err(b)) => assert_eq!(a, b),
                    (Ok(()), Ok(Some(receipt))) => {
                        let views = malformed.source_views().unwrap();
                        let expected = super::super::borrow_categorical_topology_design(
                            rows, malformed.registry(), &views, malformed.effect_to_topology(),
                        );
                        assert_eq!(receipt.into_design(rows), expected);
                    }
                    _ => panic!("single-source validation changed rejection"),
                }
            }
            let repeated = finalize_categorical_topology(
                &[CategoricalCodeView::new(&codes, levels)], &[&[0], &[0]], &vec![true; rows],
            ).unwrap();
            assert!(repeated.validate_single_source().unwrap().is_none());
        }
    }

    #[test]
    fn validated_topology_owner_preserves_storage_and_rejects_corruption() {
        for codes in [vec![], vec![0], vec![0, 0, 1, 1, 2, 2], vec![2, 0, 1, 2, 0, 1]] {
            let levels = codes.iter().max().map_or(0, |code| *code as usize + 1);
            for source_index in [0, 2] {
                let sources = vec![CategoricalCodeView::new(&codes, levels); 3];
                let topology = finalize_categorical_topology(
                    &sources, &[&[source_index]], &vec![true; codes.len()],
                ).unwrap();
                let codes_pointer = topology.compacted_sources()[0].codes().as_ptr();
                let index_pointer = topology.registry().sources()[0].level_index().checked_members(0)
                    .ok().map(|members| members.as_ptr());
                let bytes = topology.owned_buffer_bytes();
                let expected_topology = topology.clone();
                let expected = expected_topology.validate_single_source().unwrap().unwrap()
                    .into_design(codes.len()).unwrap();
                let owner = ValidatedFinalizedTopology::new(topology).unwrap();
                assert_eq!(owner.owned_buffer_bytes(), bytes);
                assert_eq!(std::mem::size_of_val(&owner), std::mem::size_of::<FinalizedCategoricalTopology>());
                assert_eq!(codes_pointer, owner.compacted_sources()[0].codes().as_ptr());
                assert_eq!(index_pointer, owner.registry().sources()[0].level_index().checked_members(0)
                    .ok().map(|members| members.as_ptr()));
                let receipt = owner.single_source().unwrap().unwrap();
                assert!(receipt.matches_owner(owner.get()));
                assert!(!receipt.matches_owner(&expected_topology));
                assert_eq!(receipt.into_design(codes.len()).unwrap(), expected);
                assert!(owner.single_source().unwrap().unwrap().into_design(codes.len()+1).is_err());

                if codes.len() > 1 {
                    let mut malformed = expected_topology.clone();
                    let changed = codes.iter().position(|code| *code != codes[0]).unwrap();
                    malformed.compacted_sources[0].codes.swap(0, changed);
                    let expected_error = malformed.validate().unwrap_err();
                    assert_eq!(ValidatedFinalizedTopology::new(malformed).unwrap_err(), expected_error);
                    let mut malformed = expected_topology.clone();
                    malformed.compacted_sources[0].codes[0] = levels as u32;
                    let expected_error = malformed.validate().unwrap_err();
                    assert_eq!(ValidatedFinalizedTopology::new(malformed).unwrap_err(), expected_error);
                }
            }
        }

        let first = [0, 0, 1, 1, 2, 2];
        let second = [0, 1, 0, 1, 0, 1];
        let sources = [CategoricalCodeView::new(&first, 3), CategoricalCodeView::new(&second, 2)];
        for recipes in [vec![], vec![&[0][..], &[0][..]], vec![&[0][..], &[1][..]], vec![&[0, 1][..]]] {
            let topology = finalize_categorical_topology(&sources, &recipes, &[true; 6]).unwrap();
            let owner = ValidatedFinalizedTopology::new(topology.clone()).unwrap();
            assert_eq!(owner.get(), &topology);
            assert!(owner.single_source().unwrap().is_none());
            owner.validate().unwrap();
        }
    }

    #[test]
    fn transferred_partitions_preserve_order_aliases_and_membership_owners() {
        use super::super::design::LevelIndex;
        for n in [0, 1, 8, 41] {
            let codes = [
                (0..n).map(|i| (i % 3) as u32).collect::<Vec<_>>(),
                (0..n).map(|i| (i / 4) as u32).collect::<Vec<_>>(),
                (0..n).map(|i| ((n - i - 1) % 2) as u32).collect::<Vec<_>>(),
            ];
            let levels = codes.each_ref().map(|c| c.iter().max().map_or(0, |v| *v as usize + 1));
            let views: Vec<_> = codes.iter().zip(levels).map(|(c, k)| CategoricalCodeView::new(c, k)).collect();
            for effects in [vec![2, 0], vec![2, 0, 2, 1], vec![1, 1], vec![0, 1, 2]] {
                let recipes: Vec<_> = effects.iter().map(std::slice::from_ref).collect();
                let expected = finalize_categorical_topology(&views, &recipes, &vec![true; n]).unwrap();
                let mut inputs: Vec<_> = codes.iter().zip(levels).map(|(c, k)| {
                    Some((c.clone(), LevelIndex::from_codes(c, k).unwrap()))
                }).collect();
                let pointers: Vec<_> = inputs.iter().map(|v| v.as_ref().unwrap().0.as_ptr()).collect();
                let members: Vec<_> = inputs.iter().map(|v| v.as_ref().unwrap().1.checked_members(0).ok().map(|m| m.as_ptr())).collect();
                let actual = FinalizedCategoricalTopology::from_single_source_indexes(
                    3, n, &recipes, |i| { let (codes, index) = inputs[i].take().expect("one move per source"); Ok((codes, levels[i], index)) },
                ).unwrap();
                assert_eq!(actual, expected);
                actual.validate().unwrap();
                for (i, source) in actual.compacted_sources().iter().enumerate() {
                    let candidate = source.candidate_source_index();
                    assert_eq!(source.codes().as_ptr(), pointers[candidate]);
                    let topology = actual.registry().sources().iter().find(|s|
                        s.partition().categorical_source_indices() == [i]).unwrap();
                    assert_eq!(topology.level_index().checked_members(0).ok().map(|m| m.as_ptr()), members[candidate]);
                    assert!(inputs[candidate].is_none());
                }
            }
        }
    }

    #[test]
    fn transferred_partitions_reject_bad_axes_and_every_reservation_failure() {
        use super::super::design::LevelIndex;
        let input = || (vec![0, 1, 0, 1], 2, LevelIndex::from_codes(&[0, 1, 0, 1], 2).unwrap());
        for effects in [vec![], vec![0, 2]] {
            let recipes: Vec<_> = effects.iter().map(std::slice::from_ref).collect();
            assert!(FinalizedCategoricalTopology::from_single_source_indexes(2, 4, &recipes, |_| Ok(input())).is_err());
        }
        for changed in [vec![0, 0, 1, 1], vec![0, 1], vec![0, 1, 0, 2]] {
            assert!(FinalizedCategoricalTopology::from_single_source_indexes(2, 4, &[&[1], &[0]], |_| {
                Ok((changed.clone(), 2, input().2))
            }).is_err());
        }
        for recipes in [&[&[0, 1][..]][..], &[&[][..]][..]] {
            assert!(FinalizedCategoricalTopology::from_single_source_indexes(2, 4, recipes, |_| Ok(input())).is_err());
        }
        for levels in [0, 1, 3] {
            assert!(FinalizedCategoricalTopology::from_single_source_indexes(2, 4, &[&[1]], |_| {
                Ok((input().0, levels, input().2))
            }).is_err());
        }
        assert!(FinalizedCategoricalTopology::from_single_source_indexes(2, 4, &[&[1]], |_| {
            Ok((vec![0, 0, 2, 2], 3, LevelIndex::from_codes(&[0, 0, 2, 2], 3)?))
        }).is_err());
        let mut successful = InjectedReservation::default();
        FinalizedCategoricalTopology::from_single_source_indexes_with_reservation(
            3, 4, &[&[2], &[0], &[2]], |_| Ok(input()), &mut successful,
        ).unwrap();
        for position in 0..successful.calls {
            for skip in [false, true] {
                let mut reservation = InjectedReservation {
                    fail_at: (!skip).then_some(position), skip_at: skip.then_some(position),
                    ..InjectedReservation::default()
                };
                assert_eq!(FinalizedCategoricalTopology::from_single_source_indexes_with_reservation(
                    3, 4, &[&[2], &[0], &[2]], |_| Ok(input()), &mut reservation,
                ).unwrap_err().code, ErrorCode::Allocation);
            }
        }
    }

    #[test]
    fn compacts_referenced_sources_and_preserves_original_level_identity() {
        let used = [4, 4, 1, 1, 3];
        let unused = [0, 1, 2, 3, 4];
        let sources = [
            CategoricalCodeView::new(&used, 5),
            CategoricalCodeView::new(&unused, 5),
        ];
        let result =
            finalize_categorical_topology(&sources, &[&[0]], &[true, false, true, true, false])
                .expect("finalization");
        assert_eq!(result.retained_rows(), &[0, 2, 3]);
        assert_eq!(result.compacted_sources().len(), 1);
        let compacted = result.source_for_candidate(0).expect("used source");
        assert_eq!(compacted.codes(), &[1, 0, 0]);
        assert_eq!(compacted.original_levels(), &[1, 4]);
        assert_eq!(result.candidate_to_compacted_source(), &[Some(0), None]);
        assert_eq!(result.remapped_effect_recipes(), &[vec![0]]);
        assert_eq!(result.effect_to_topology(), &[0]);
        assert!(result.source_for_candidate(1).is_err());
        result.validate().expect("validation");
    }

    #[test]
    fn interaction_is_refactorized_on_the_final_sample() {
        let firm = [0, 0, 1, 1, 2, 2];
        let year = [0, 1, 0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&year, 2),
        ];
        let result = finalize_categorical_topology(
            &sources,
            &[&[0, 1]],
            &[true, false, true, true, false, true],
        )
        .expect("interaction finalization");
        assert_eq!(result.retained_rows(), &[0, 2, 3, 5]);
        assert_eq!(result.compacted_sources()[0].codes(), &[0, 1, 1, 2]);
        assert_eq!(result.compacted_sources()[1].codes(), &[0, 0, 1, 1]);
        let topology = result.registry().source(0).expect("interaction topology");
        assert!(topology.partition().is_interaction());
        assert_eq!(
            topology
                .codes(&[
                    result.compacted_sources()[0].view(),
                    result.compacted_sources()[1].view(),
                ])
                .expect("interaction codes"),
            &[0, 1, 2, 3]
        );
        assert_eq!(topology.n_levels(), 4);
        result.validate().expect("validation");
    }

    #[test]
    fn repeated_recipes_share_final_topology_and_sources() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let result =
            finalize_categorical_topology(&sources, &[&[0, 1], &[1, 0], &[0, 1, 0]], &[true; 4])
                .expect("deduplicated finalization");
        assert_eq!(result.compacted_sources().len(), 2);
        assert_eq!(result.registry().sources().len(), 1);
        assert_eq!(result.effect_to_topology(), &[0, 0, 0]);
    }

    #[test]
    fn no_effects_copy_no_sources_but_preserve_retained_rows() {
        let raw = [0, 1, 0];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        let result = finalize_categorical_topology(&sources, &[], &[true, false, true])
            .expect("empty topology");
        assert!(result.compacted_sources().is_empty());
        assert!(result.registry().sources().is_empty());
        assert!(result.effect_to_topology().is_empty());
        assert_eq!(result.retained_rows(), &[0, 2]);
        assert_eq!(result.candidate_to_compacted_source(), &[None]);
    }

    #[test]
    fn empty_final_sample_is_structurally_valid() {
        let firm = [0, 1];
        let year = [0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let result = finalize_categorical_topology(&sources, &[&[0, 1]], &[false, false])
            .expect("empty final sample");
        assert!(result.retained_rows().is_empty());
        assert_eq!(result.compacted_sources().len(), 2);
        assert!(result.compacted_sources()[0].codes().is_empty());
        assert_eq!(result.compacted_sources()[0].n_levels(), 0);
        assert_eq!(result.registry().source(0).expect("topology").n_levels(), 0);
        result.validate().expect("validation");
    }

    #[test]
    fn malformed_inputs_fail_closed() {
        let raw = [0, 1];
        let short = [0];
        let invalid = [0, 2];
        assert_eq!(
            finalize_categorical_topology(
                &[CategoricalCodeView::new(&short, 1)],
                &[&[0]],
                &[true; 2],
            )
            .expect_err("row mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            finalize_categorical_topology(
                &[CategoricalCodeView::new(&raw, 2)],
                &[&[]],
                &[true; 2],
            )
            .expect_err("empty recipe")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            finalize_categorical_topology(
                &[CategoricalCodeView::new(&raw, 2)],
                &[&[1]],
                &[true; 2],
            )
            .expect_err("bad source index")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            finalize_categorical_topology(
                &[CategoricalCodeView::new(&invalid, 2)],
                &[&[0]],
                &[true; 2],
            )
            .expect_err("bad code")
            .code,
            ErrorCode::InvalidData
        );
    }

    #[test]
    fn every_finalization_reservation_and_capacity_guard_fails_closed() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let recipes: [&[usize]; 1] = [&[0, 1]];
        let active = [true; 4];

        let mut successful = InjectedReservation::default();
        finalize_categorical_topology_with_reservation(
            &sources,
            &recipes,
            &active,
            &mut successful,
        )
        .expect("successful reservation audit");
        assert!(successful.calls > 10);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = finalize_categorical_topology_with_reservation(
                &sources,
                &recipes,
                &active,
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
            let error = finalize_categorical_topology_with_reservation(
                &sources,
                &recipes,
                &active,
                &mut undersized,
            )
            .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }
}
