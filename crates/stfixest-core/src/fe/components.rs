use crate::error::{ErrorCode, Result, StFixestError};
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
}

/// Connected components of a multipartite fixed-effect graph.
pub fn connected_components(
    codes_by_effect: &[&[u32]],
    level_counts: &[usize],
    active: &[bool],
) -> Result<ComponentInfo> {
    if codes_by_effect.len() != level_counts.len() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "component effect and level-count tables have different lengths",
        ));
    }
    let nobs = active.len();
    if codes_by_effect.iter().any(|codes| codes.len() != nobs) {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "component code columns have inconsistent lengths",
        ));
    }
    let mut offsets = vec![0_usize; level_counts.len() + 1];
    for effect in 0..level_counts.len() {
        offsets[effect + 1] = offsets[effect]
            .checked_add(level_counts[effect])
            .ok_or_else(|| StFixestError::new(ErrorCode::Allocation, "level count overflow"))?;
    }
    let mut union_find = UnionFind::new(offsets[level_counts.len()]);
    let mut observed = vec![false; offsets[level_counts.len()]];
    for (observation, is_active) in active.iter().copied().enumerate() {
        if !is_active || codes_by_effect.is_empty() {
            continue;
        }
        let first = offsets[0] + codes_by_effect[0][observation] as usize;
        if first >= offsets[1] {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                "component code exceeds declared level count",
            ));
        }
        observed[first] = true;
        for effect in 1..codes_by_effect.len() {
            let node = offsets[effect] + codes_by_effect[effect][observation] as usize;
            if node >= offsets[effect + 1] {
                return Err(StFixestError::new(
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
}
