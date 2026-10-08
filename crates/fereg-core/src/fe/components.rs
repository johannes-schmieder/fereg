use crate::error::{ErrorCode, FeregError, Result};
use crate::memory::fallible_filled;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInfo {
    pub n_components: usize,
    pub component_by_effect_level: Vec<Vec<usize>>,
}

#[derive(Debug)]
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn try_new(size: usize) -> Result<Self> {
        let mut parent = Vec::new();
        parent.try_reserve_exact(size).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve connected-component parents",
            )
        })?;
        parent.extend(0..size);
        let rank = fallible_filled(size, 0_u8, "unable to reserve connected-component ranks")?;
        Ok(Self { parent, rank })
    }

    fn find(&mut self, value: usize) -> usize {
        if self.parent[value] != value {
            let root = self.find(self.parent[value]);
            self.parent[value] = root;
        }
        self.parent[value]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return;
        }
        match self.rank[left_root].cmp(&self.rank[right_root]) {
            std::cmp::Ordering::Less => self.parent[left_root] = right_root,
            std::cmp::Ordering::Greater => self.parent[right_root] = left_root,
            std::cmp::Ordering::Equal => {
                self.parent[right_root] = left_root;
                self.rank[left_root] += 1;
            }
        }
    }

    /// Freeze every node at its direct root before the parent storage is
    /// reused as the root-to-public-label table. No `find` or `union` call is
    /// valid after component labels begin to be installed.
    fn compress_all(&mut self) {
        for node in 0..self.parent.len() {
            self.find(node);
        }
    }

    /// Return the first-observed public label for one node after
    /// [`Self::compress_all`]. A value at least `nodes` is an encoded label;
    /// direct parent pointers remain below `nodes`. This reuses the existing
    /// O(levels) parent allocation instead of reserving a second dense label
    /// vector during saved-FE normalization.
    fn label_compressed_component(&mut self, node: usize, next_label: &mut usize) -> Result<usize> {
        let nodes = self.parent.len();
        let direct = *self.parent.get(node).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Internal,
                "connected-component label node is out of range",
            )
        })?;
        if direct >= nodes {
            return Ok(direct - nodes);
        }
        let marker = self.parent[direct];
        if marker >= nodes {
            return Ok(marker - nodes);
        }
        if marker != direct {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "connected-component parents were not fully compressed before labeling",
            ));
        }
        let label = *next_label;
        self.parent[direct] = nodes.checked_add(label).ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "connected-component label encoding overflows usize",
            )
        })?;
        *next_label = (*next_label).checked_add(1).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "component label count overflow")
        })?;
        Ok(label)
    }
}

/// Connected components of a multipartite fixed-effect graph.
pub fn connected_components(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
    active: &[bool],
) -> Result<ComponentInfo> {
    connected_components_inner(codes_by_effect, level_counts, active.len(), Some(active))
}

#[cfg(test)]
pub(crate) fn connected_components_all(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
) -> Result<ComponentInfo> {
    let nobs = codes_by_effect.first().map_or(0, |codes| codes.len());
    connected_components_inner(codes_by_effect, level_counts, nobs, None)
}

/// Fallible-allocation sibling for fit paths whose data-sized allocation
/// failures must remain ordinary estimator errors rather than allocator
/// panics. Component labels retain the same first-observed effect/level order
/// as `connected_components_all`.
pub(crate) fn connected_components_all_fallible(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
) -> Result<ComponentInfo> {
    let nobs = codes_by_effect.first().map_or(0, |codes| codes.len());
    connected_components_inner_fallible(codes_by_effect, level_counts, nobs)
}

fn connected_components_inner(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
    nobs: usize,
    active: Option<&[bool]>,
) -> Result<ComponentInfo> {
    if codes_by_effect.len() != level_counts.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "component effect and level-count tables have different lengths",
        ));
    }
    if codes_by_effect.iter().any(|codes| codes.len() != nobs) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "component code columns have inconsistent lengths",
        ));
    }
    let mut offsets = vec![0_usize; level_counts.len() + 1];
    for effect in 0..level_counts.len() {
        offsets[effect + 1] = offsets[effect]
            .checked_add(level_counts[effect])
            .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "level count overflow"))?;
    }
    let mut union_find = UnionFind::new(offsets[level_counts.len()]);
    let mut observed = vec![false; offsets[level_counts.len()]];
    for observation in 0..nobs {
        if active.is_some_and(|active| !active[observation]) || codes_by_effect.is_empty() {
            continue;
        }
        let first = offsets[0] + codes_by_effect[0][observation] as usize;
        if first >= offsets[1] {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "component code exceeds declared level count",
            ));
        }
        observed[first] = true;
        for effect in 1..codes_by_effect.len() {
            let node = offsets[effect] + codes_by_effect[effect][observation] as usize;
            if node >= offsets[effect + 1] {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "component code exceeds declared level count",
                ));
            }
            observed[node] = true;
            union_find.union(first, node);
        }
    }

    let mut labels = BTreeMap::new();
    let mut next_label = 0_usize;
    let mut component_by_effect_level = Vec::with_capacity(level_counts.len());
    for effect in 0..level_counts.len() {
        let mut effect_components = Vec::with_capacity(level_counts[effect]);
        for level in 0..level_counts[effect] {
            let node = offsets[effect] + level;
            if !observed[node] {
                effect_components.push(usize::MAX);
                continue;
            }
            let root = union_find.find(node);
            let label = *labels.entry(root).or_insert_with(|| {
                let label = next_label;
                next_label += 1;
                label
            });
            effect_components.push(label);
        }
        component_by_effect_level.push(effect_components);
    }
    Ok(ComponentInfo {
        n_components: next_label,
        component_by_effect_level,
    })
}

fn connected_components_inner_fallible(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
    nobs: usize,
) -> Result<ComponentInfo> {
    if codes_by_effect.len() != level_counts.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "component effect and level-count tables have different lengths",
        ));
    }
    if codes_by_effect.iter().any(|codes| codes.len() != nobs) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "component code columns have inconsistent lengths",
        ));
    }

    let offset_count = level_counts
        .len()
        .checked_add(1)
        .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "component offset count overflow"))?;
    let mut offsets = fallible_filled(
        offset_count,
        0_usize,
        "unable to reserve connected-component offsets",
    )?;
    for effect in 0..level_counts.len() {
        offsets[effect + 1] = offsets[effect]
            .checked_add(level_counts[effect])
            .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "level count overflow"))?;
    }
    let nodes = offsets[level_counts.len()];
    let mut union_find = UnionFind::try_new(nodes)?;
    let mut observed = fallible_filled(
        nodes,
        false,
        "unable to reserve connected-component observation flags",
    )?;
    for observation in 0..nobs {
        if codes_by_effect.is_empty() {
            continue;
        }
        let first = offsets[0] + codes_by_effect[0][observation] as usize;
        if first >= offsets[1] {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "component code exceeds declared level count",
            ));
        }
        observed[first] = true;
        for effect in 1..codes_by_effect.len() {
            let node = offsets[effect] + codes_by_effect[effect][observation] as usize;
            if node >= offsets[effect + 1] {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "component code exceeds declared level count",
                ));
            }
            observed[node] = true;
            union_find.union(first, node);
        }
    }

    // Freeze all direct roots before reusing `parent[root]` as the public
    // root-label table. Public labels are still assigned by the exact legacy
    // effect/level traversal below, so only storage ownership changes.
    union_find.compress_all();
    let mut next_label = 0_usize;
    let mut component_by_effect_level = Vec::new();
    component_by_effect_level
        .try_reserve_exact(level_counts.len())
        .map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve connected-component effect rows",
            )
        })?;
    for effect in 0..level_counts.len() {
        let mut effect_components = Vec::new();
        effect_components
            .try_reserve_exact(level_counts[effect])
            .map_err(|_| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "unable to reserve connected-component level labels",
                )
            })?;
        for level in 0..level_counts[effect] {
            let node = offsets[effect] + level;
            if !observed[node] {
                effect_components.push(usize::MAX);
                continue;
            }
            effect_components.push(union_find.label_compressed_component(node, &mut next_label)?);
        }
        component_by_effect_level.push(effect_components);
    }
    Ok(ComponentInfo {
        n_components: next_label,
        component_by_effect_level,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_disconnected_bipartite_groups() {
        let worker = [0, 1, 2, 3];
        let firm = [0, 0, 1, 1];
        let info =
            connected_components(&[&worker, &firm], &[4, 2], &[true; 4]).expect("components");
        assert_eq!(info.n_components, 2);
        assert_eq!(info.component_by_effect_level[0], vec![0, 0, 1, 1]);
    }

    #[test]
    fn fallible_all_matches_existing_component_labels() {
        let worker = [0, 1, 2, 3];
        let firm = [0, 0, 1, 1];
        let existing =
            connected_components_all(&[&worker, &firm], &[4, 2]).expect("existing components");
        let fallible = connected_components_all_fallible(&[&worker, &firm], &[4, 2])
            .expect("fallible components");
        assert_eq!(fallible, existing);
    }

    #[test]
    fn fallible_all_preserves_multipartite_first_observed_labels_and_unseen_levels() {
        let first = [4, 0, 2, 4, 1, 6, 2, 0];
        let second = [3, 1, 5, 3, 0, 4, 5, 1];
        let third = [2, 0, 4, 2, 1, 3, 4, 0];
        let codes = [&first[..], &second[..], &third[..]];
        let levels = [7, 6, 5];
        let existing = connected_components_all(&codes, &levels).expect("existing components");
        let fallible =
            connected_components_all_fallible(&codes, &levels).expect("fallible components");
        assert_eq!(fallible, existing);
        assert_eq!(fallible.n_components, 5);
        assert_eq!(
            fallible.component_by_effect_level,
            vec![
                vec![0, 1, 2, usize::MAX, 3, usize::MAX, 4],
                vec![1, 0, usize::MAX, 3, 4, 2],
                vec![0, 1, 3, 4, 2],
            ]
        );
    }

    #[test]
    fn compressed_component_labels_reuse_parent_allocation() {
        let mut union_find = UnionFind::try_new(6).expect("union find");
        union_find.union(0, 2);
        union_find.union(1, 3);
        union_find.union(4, 5);
        union_find.compress_all();

        let parent_pointer = union_find.parent.as_ptr();
        let parent_capacity = union_find.parent.capacity();
        let mut next_label = 0;
        let labels = [2, 3, 0, 5, 4]
            .into_iter()
            .map(|node| {
                union_find
                    .label_compressed_component(node, &mut next_label)
                    .expect("component label")
            })
            .collect::<Vec<_>>();

        assert_eq!(labels, [0, 1, 0, 2, 2]);
        assert_eq!(next_label, 3);
        assert_eq!(union_find.parent.as_ptr(), parent_pointer);
        assert_eq!(union_find.parent.capacity(), parent_capacity);
    }

    #[test]
    fn fallible_all_reports_capacity_overflow_as_allocation_error() {
        let empty: [u32; 0] = [];
        let error = connected_components_all_fallible(&[&empty], &[usize::MAX])
            .expect_err("oversized component workspace");
        assert_eq!(error.code, ErrorCode::Allocation);
    }
}
