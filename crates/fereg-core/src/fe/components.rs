use crate::error::{ErrorCode, FeregError, Result};
use crate::memory::fallible_filled;

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

/// Components over every observation. Data-sized allocation failures remain
/// ordinary estimator errors; labels follow first-observed effect/level order.
pub(crate) fn connected_components_all(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
) -> Result<ComponentInfo> {
    let nobs = codes_by_effect.first().map_or(0, |codes| codes.len());
    connected_components_inner(codes_by_effect, level_counts, nobs, None)
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

/// Transient bytes of [`count_large_pair_components`] for one factor pair:
/// union-find parents and ranks plus two per-root block counters.
pub(crate) fn large_pair_component_scratch_bytes(left_levels: usize, right_levels: usize) -> u128 {
    let nodes = left_levels as u128 + right_levels as u128;
    nodes * (2 * std::mem::size_of::<usize>() as u128 + 1 + std::mem::size_of::<usize>() as u128)
}

/// Number of connected components of one factor pair's bipartite incidence
/// graph, over every observation, with at least `threshold` levels in one of
/// their two FE blocks. Levels without observations are isolated singletons.
pub(crate) fn count_large_pair_components(
    left_codes: &[u32],
    left_levels: usize,
    right_codes: &[u32],
    right_levels: usize,
    threshold: usize,
) -> Result<usize> {
    if left_codes.len() != right_codes.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "pair component code columns have inconsistent lengths",
        ));
    }
    let nodes = left_levels
        .checked_add(right_levels)
        .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "pair level count overflow"))?;
    let mut union_find = UnionFind::try_new(nodes)?;
    for (&left, &right) in left_codes.iter().zip(right_codes) {
        let (left, right) = (left as usize, right as usize);
        if left >= left_levels || right >= right_levels {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "component code exceeds declared level count",
            ));
        }
        union_find.union(left, left_levels + right);
    }
    let mut left_count = fallible_filled(nodes, 0_usize, "unable to reserve pair block counts")?;
    let mut right_count = fallible_filled(nodes, 0_usize, "unable to reserve pair block counts")?;
    for node in 0..nodes {
        let root = union_find.find(node);
        if node < left_levels {
            left_count[root] += 1;
        } else {
            right_count[root] += 1;
        }
    }
    Ok((0..nodes)
        .filter(|&root| {
            union_find.parent[root] == root && left_count[root].max(right_count[root]) >= threshold
        })
        .count())
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
    fn all_observation_components_match_the_masked_entry() {
        let worker = [0, 1, 2, 3];
        let firm = [0, 0, 1, 1];
        let all = connected_components_all(&[&worker, &firm], &[4, 2]).expect("components");
        assert_eq!(all.n_components, 2);
        assert_eq!(
            all.component_by_effect_level,
            vec![vec![0, 0, 1, 1], vec![0, 1]]
        );
        let masked =
            connected_components(&[&worker, &firm], &[4, 2], &[true; 4]).expect("components");
        assert_eq!(masked, all);
    }

    #[test]
    fn inactive_observations_neither_connect_nor_observe_levels() {
        let worker = [0, 0, 1, 1];
        let firm = [0, 1, 1, 2];
        let all = connected_components(&[&worker, &firm], &[2, 3], &[true; 4]).expect("all");
        assert_eq!(all.n_components, 1);
        let masked = connected_components(&[&worker, &firm], &[2, 3], &[true, true, false, true])
            .expect("masked");
        assert_eq!(masked.n_components, 2);
        assert_eq!(
            masked.component_by_effect_level,
            vec![vec![0, 1], vec![0, 0, 1]]
        );
        let unseen = connected_components(&[&worker, &firm], &[2, 3], &[true, false, true, false])
            .expect("unseen");
        assert_eq!(
            unseen.component_by_effect_level,
            vec![vec![0, 1], vec![0, 1, usize::MAX]]
        );
    }

    #[test]
    fn multipartite_labels_follow_first_observed_order_and_mark_unseen_levels() {
        let first = [4, 0, 2, 4, 1, 6, 2, 0];
        let second = [3, 1, 5, 3, 0, 4, 5, 1];
        let third = [2, 0, 4, 2, 1, 3, 4, 0];
        let codes = [&first[..], &second[..], &third[..]];
        let levels = [7, 6, 5];
        let fallible = connected_components_all(&codes, &levels).expect("components");
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

    /// Independent breadth-first reference: components of the bipartite
    /// pair graph and their block level counts.
    fn reference_large_pair_components(
        left: &[u32],
        left_levels: usize,
        right: &[u32],
        right_levels: usize,
        threshold: usize,
    ) -> usize {
        let nodes = left_levels + right_levels;
        let mut adjacency = vec![Vec::new(); nodes];
        for (&a, &b) in left.iter().zip(right) {
            adjacency[a as usize].push(left_levels + b as usize);
            adjacency[left_levels + b as usize].push(a as usize);
        }
        let mut seen = vec![false; nodes];
        let mut large = 0;
        for start in 0..nodes {
            if seen[start] {
                continue;
            }
            seen[start] = true;
            let (mut queue, mut blocks) = (vec![start], [0_usize; 2]);
            while let Some(node) = queue.pop() {
                blocks[usize::from(node >= left_levels)] += 1;
                for &next in &adjacency[node] {
                    if !seen[next] {
                        seen[next] = true;
                        queue.push(next);
                    }
                }
            }
            large += usize::from(blocks[0].max(blocks[1]) >= threshold);
        }
        large
    }

    #[test]
    fn large_pair_components_match_an_independent_search() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move |bound: u64| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((state >> 33) % bound) as u32
        };
        for (rows, left_levels, right_levels) in [(40, 30, 6), (200, 90, 40), (500, 60, 300)] {
            let left: Vec<u32> = (0..rows).map(|_| next(left_levels as u64)).collect();
            let right: Vec<u32> = (0..rows).map(|_| next(right_levels as u64)).collect();
            for threshold in [1, 2, 3, 5, 8, 20, 61] {
                assert_eq!(
                    count_large_pair_components(
                        &left,
                        left_levels,
                        &right,
                        right_levels,
                        threshold
                    )
                    .unwrap(),
                    reference_large_pair_components(
                        &left,
                        left_levels,
                        &right,
                        right_levels,
                        threshold
                    ),
                    "rows={rows} threshold={threshold}"
                );
            }
        }
        // Two components with blocks {3, 1} and {1, 2}; the threshold is inclusive.
        let (left, right) = ([0, 1, 2, 3], [0, 0, 0, 1]);
        assert_eq!(
            count_large_pair_components(&left, 4, &right, 2, 3).unwrap(),
            1
        );
        assert_eq!(
            count_large_pair_components(&left, 4, &right, 2, 1).unwrap(),
            2
        );
        assert_eq!(
            count_large_pair_components(&left, 4, &right, 2, 4).unwrap(),
            0
        );
        let error = count_large_pair_components(&left, 4, &[0, 0, 0, 2], 2, 1).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn capacity_overflow_is_an_allocation_error() {
        let empty: [u32; 0] = [];
        let error = connected_components_all(&[&empty], &[usize::MAX])
            .expect_err("oversized component workspace");
        assert_eq!(error.code, ErrorCode::Allocation);
    }
}
