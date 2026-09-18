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

impl FinalizedCategoricalTopology {
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
