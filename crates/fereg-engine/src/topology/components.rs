//! Multipartite components, with canonical labels in effect/level traversal order.
use crate::error::{reserved, ErrorCode, FeregError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInfo {
    pub n_components: usize,
    pub component_by_effect_level: Vec<Vec<usize>>,
}
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}
impl UnionFind {
    fn new(n: usize) -> Result<Self> {
        let mut parent = reserved(n)?;
        parent.extend(0..n);
        let mut rank = reserved(n)?;
        rank.resize(n, 0);
        Ok(Self { parent, rank })
    }
    fn find(&mut self, mut node: usize) -> usize {
        let mut root = node;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        while self.parent[node] != root {
            let next = self.parent[node];
            self.parent[node] = root;
            node = next;
        }
        root
    }
    fn union(&mut self, a: usize, b: usize) {
        let mut a = self.find(a);
        let mut b = self.find(b);
        if a == b {
            return;
        }
        if self.rank[a] < self.rank[b] {
            std::mem::swap(&mut a, &mut b);
        }
        self.parent[b] = a;
        if self.rank[a] == self.rank[b] {
            self.rank[a] += 1;
        }
    }
}

pub fn connected_components(
    codes: &[&[u32]],
    levels: &[usize],
    active: &[bool],
) -> Result<ComponentInfo> {
    components(codes, levels, active.len(), Some(active))
}
pub(crate) fn connected_components_all(
    codes: &[&[u32]],
    levels: &[usize],
    nobs: usize,
) -> Result<ComponentInfo> {
    components(codes, levels, nobs, None)
}
fn components(
    codes: &[&[u32]],
    levels: &[usize],
    nobs: usize,
    active: Option<&[bool]>,
) -> Result<ComponentInfo> {
    if codes.len() != levels.len() || codes.iter().any(|column| column.len() != nobs) {
        return Err(invalid("component code, level and row dimensions differ"));
    }
    let mut offsets = reserved(levels.len().checked_add(1).ok_or_else(allocation)?)?;
    offsets.push(0usize);
    for &count in levels {
        offsets.push(
            offsets
                .last()
                .unwrap()
                .checked_add(count)
                .ok_or_else(allocation)?,
        );
    }
    let nodes = *offsets.last().unwrap();
    let mut union = UnionFind::new(nodes)?;
    let mut labels = reserved(nodes)?;
    labels.resize(nodes, usize::MAX);
    for row in 0..nobs {
        if active.is_some_and(|active| !active[row]) {
            continue;
        }
        for (effect, column) in codes.iter().enumerate() {
            let level = column[row] as usize;
            if level >= levels[effect] {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "component code exceeds declared level count",
                ));
            }
            let node = offsets[effect] + level;
            labels[node] = 0;
            if effect > 0 {
                union.union(codes[0][row] as usize, node);
            }
        }
    }
    // Freeze roots before reusing the parent allocation for public labels.
    for node in 0..nodes {
        union.find(node);
    }
    let mut count = 0usize;
    for (node, label) in labels.iter_mut().enumerate() {
        if *label == usize::MAX {
            continue;
        }
        let root = union.parent[node];
        if root >= nodes {
            *label = root - nodes;
            continue;
        }
        if union.parent[root] < nodes {
            union.parent[root] = nodes.checked_add(count).ok_or_else(allocation)?;
            count += 1;
        }
        *label = union.parent[root] - nodes;
    }
    let mut by_effect = reserved(levels.len())?;
    for effect in 0..levels.len() {
        let mut row = reserved(levels[effect])?;
        row.extend_from_slice(&labels[offsets[effect]..offsets[effect + 1]]);
        by_effect.push(row);
    }
    Ok(ComponentInfo {
        n_components: count,
        component_by_effect_level: by_effect,
    })
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn allocation() -> FeregError {
    FeregError::new(ErrorCode::Allocation, "component dimensions overflow")
}
