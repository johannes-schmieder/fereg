//! Factor-pair subdomains: the weighted cross-tabulation of two effects and
//! the connected components of their bipartite level graph.
//!
//! On the levels of effects `a` and `b`, `D'WD` has the block
//! `K = [[diag(m_a), C], [C', diag(m_b)]]` with `C` the weighted
//! cross-tabulation (`C_lk` sums the weights of the rows at level `l` of `a`
//! and `k` of `b`), whose rows and columns sum to the level masses. Every
//! connected component of the bipartite graph of `C` is one subdomain. Each
//! component's slots hold its larger side's levels, then its smaller
//! side's, both ascending; a pair's components follow each other in the
//! order of their smallest level (side `a` first).
//!
//! Everything is a fixed function of the design and the weights: per-level
//! sums run over the level's rows in ascending order, so results do not
//! depend on the worker count.
//!
//! Memory: each stage is charged to the build's [`Admit`] before it
//! allocates (its retained arrays and its transients, the latter released
//! when freed); the passes with per-task scratch run in at most
//! [`super::GROUPS`] groups of level tasks, one scratch per group.

use super::super::structure::{sort_levels, Gathered, Structure};
use super::super::{filled, with_capacity};
use super::{Admit, GROUPS};
use crate::design::Effect;
use crate::elimination::Cancel;
use crate::error::Result;
use rayon::prelude::*;
use std::ops::Range;

/// Levels per task of the cross-tabulation passes.
const LEVEL_TASK: usize = 2048;

/// Compressed rows of one side's levels: level `l` owns the entries
/// `ptr[l]..ptr[l + 1]`, whose columns are the other side's levels as
/// indices within their side of the same component (ascending), with their
/// weighted counts.
#[derive(Default)]
pub(super) struct Rows {
    pub(super) ptr: Vec<u32>,
    pub(super) idx: Vec<u32>,
    pub(super) val: Vec<f64>,
}

impl Rows {
    #[inline]
    pub(super) fn row(&self, level: usize) -> (&[u32], &[f64]) {
        let (s, e) = (self.ptr[level] as usize, self.ptr[level + 1] as usize);
        (&self.idx[s..e], &self.val[s..e])
    }

    fn bytes(&self) -> u128 {
        4 * (self.ptr.capacity() + self.idx.capacity()) as u128 + 8 * self.val.capacity() as u128
    }
}

/// One component of a pair's level graph.
pub(super) struct Component {
    /// First slot, relative to the pair.
    pub(super) start: usize,
    /// Levels on side 0 (effect `a`) and side 1.
    pub(super) sizes: [usize; 2],
    /// The side with more levels (side 0 of equals).
    pub(super) big: usize,
}

/// One pair of effects: its cross-tabulation and slot layout.
pub(super) struct Pair {
    /// Side 0 has at least as many levels as side 1.
    pub(super) effects: [usize; 2],
    /// The pair's first slot in the preconditioner's local storage.
    pub(super) first: usize,
    /// Each slot's level (within its side's effect), slot order.
    pub(super) codes: Vec<u32>,
    /// Each level's slot relative to `first`, per side.
    pub(super) slot: [Vec<u32>; 2],
    /// The cross-tabulation by side 0's levels and by side 1's levels.
    pub(super) rows: [Rows; 2],
    /// `1 / (row sum)` of every level, per side.
    pub(super) inverse: [Vec<f64>; 2],
}

impl Pair {
    pub(super) fn slots(&self) -> usize {
        self.codes.len()
    }

    pub(super) fn bytes(&self) -> u128 {
        let levels = self.codes.capacity() + self.slot[0].capacity() + self.slot[1].capacity();
        self.rows[0].bytes()
            + self.rows[1].bytes()
            + 4 * levels as u128
            + 8 * (self.inverse[0].capacity() + self.inverse[1].capacity()) as u128
    }
}

/// Retained bytes of a pair with these level counts and `nnz` cross-tab
/// entries.
fn pair_bytes(levels: [usize; 2], nnz: usize) -> u128 {
    let (a, b, nnz) = (levels[0] as u128, levels[1] as u128, nnz as u128);
    2 * nnz * 12 + 4 * (a + b + 2) + 16 * (a + b)
}

/// A bound on the bytes of a pair's components (at most one per level).
pub(super) fn components_bytes(levels: [usize; 2]) -> u128 {
    (levels[0] + levels[1]) as u128 * std::mem::size_of::<Component>() as u128
}

/// Level tasks per group of `tasks` (at most [`GROUPS`] groups).
fn per_group(tasks: usize) -> usize {
    tasks.div_ceil(GROUPS).max(1)
}

/// Groups of `tasks` level tasks.
fn groups(tasks: usize) -> u128 {
    tasks.div_ceil(per_group(tasks)) as u128
}

/// The rows of each level of `effect`: the structure's index when it has
/// one, else a new one.
fn level_index<'s>(
    st: &'s Structure,
    effects: &[Effect<'_>],
    effect: usize,
    owned: &'s mut Option<Gathered>,
) -> Result<&'s Gathered> {
    if let Some(index) = st.gathered.iter().find(|g| g.effect == effect) {
        return Ok(index);
    }
    let mut index = Gathered {
        effect,
        starts: filled(effects[effect].levels() + 1, 0)?,
        rows: filled(st.nobs, 0)?,
    };
    sort_levels(effects[effect].codes(), &mut index)?;
    Ok(owned.insert(index))
}

/// Distinct side-1 levels of every side-0 level: the cross-tab row pointers.
fn count(
    index: &Gathered,
    codes: &[u32],
    levels: [usize; 2],
    cancel: &Cancel<'_>,
) -> Result<Vec<u32>> {
    let mut ptr = filled(levels[0] + 1, 0_u32)?;
    let group = LEVEL_TASK * per_group(levels[0].div_ceil(LEVEL_TASK));
    ptr[1..]
        .par_chunks_mut(group)
        .enumerate()
        .try_for_each(|(g, out)| -> Result<()> {
            cancel.check()?;
            let mut stamp = filled(levels[1], u32::MAX)?;
            for (k, count) in out.iter_mut().enumerate() {
                let level = g * group + k;
                let rows = &index.rows[index.starts[level]..index.starts[level + 1]];
                let mut distinct = 0;
                for &row in rows {
                    let b = codes[row as usize] as usize;
                    if stamp[b] != level as u32 {
                        stamp[b] = level as u32;
                        distinct += 1;
                    }
                }
                *count = distinct;
            }
            Ok(())
        })?;
    for level in 0..levels[0] {
        ptr[level + 1] += ptr[level];
    }
    Ok(ptr)
}

/// Fill the cross-tab rows of `ptr`: per side-0 level, its side-1 levels in
/// ascending order with their weights summed over its rows in order.
fn fill(
    index: &Gathered,
    codes: &[u32],
    weights: Option<&[f64]>,
    (levels, ptr): ([usize; 2], Vec<u32>),
    cancel: &Cancel<'_>,
) -> Result<Rows> {
    let nnz = ptr[levels[0]] as usize;
    let mut idx = filled(nnz, 0_u32)?;
    let mut val = filled(nnz, 0.0)?;
    let group = LEVEL_TASK * per_group(levels[0].div_ceil(LEVEL_TASK));
    let tasks = levels[0].div_ceil(group);
    let mut parts: Vec<(Range<usize>, &mut [u32], &mut [f64])> = with_capacity(tasks)?;
    let (mut rest_idx, mut rest_val) = (&mut idx[..], &mut val[..]);
    for task in 0..tasks {
        let range = task * group..((task + 1) * group).min(levels[0]);
        let len = (ptr[range.end] - ptr[range.start]) as usize;
        let (head_idx, tail_idx) = std::mem::take(&mut rest_idx).split_at_mut(len);
        let (head_val, tail_val) = std::mem::take(&mut rest_val).split_at_mut(len);
        parts.push((range, head_idx, head_val));
        (rest_idx, rest_val) = (tail_idx, tail_val);
    }
    parts
        .into_par_iter()
        .try_for_each(|(range, idx, val)| -> Result<()> {
            cancel.check()?;
            let mut sum = filled(levels[1], 0.0)?;
            let mut stamp = filled(levels[1], u32::MAX)?;
            let mut touched = with_capacity::<u32>(levels[1])?;
            let mut at = 0;
            for level in range {
                touched.clear();
                for &row in &index.rows[index.starts[level]..index.starts[level + 1]] {
                    let row = row as usize;
                    let b = codes[row] as usize;
                    if stamp[b] != level as u32 {
                        stamp[b] = level as u32;
                        sum[b] = 0.0;
                        touched.push(b as u32);
                    }
                    sum[b] += weights.map_or(1.0, |w| w[row]);
                }
                touched.sort_unstable();
                for &b in &touched {
                    idx[at] = b;
                    val[at] = sum[b as usize];
                    at += 1;
                }
            }
            Ok(())
        })?;
    Ok(Rows { ptr, idx, val })
}

/// The cross-tab by side-1 levels (columns: side-0 levels, ascending).
fn transpose(rows: &Rows, levels: [usize; 2]) -> Result<Rows> {
    let nnz = rows.idx.len();
    let mut ptr = filled(levels[1] + 1, 0_u32)?;
    for &b in &rows.idx {
        ptr[b as usize + 1] += 1;
    }
    for level in 0..levels[1] {
        ptr[level + 1] += ptr[level];
    }
    let mut cursor = with_capacity::<u32>(levels[1])?;
    cursor.extend_from_slice(&ptr[..levels[1]]);
    let mut idx = filled(nnz, 0_u32)?;
    let mut val = filled(nnz, 0.0)?;
    for a in 0..levels[0] {
        let (cols, vals) = rows.row(a);
        for (&b, &v) in cols.iter().zip(vals) {
            let at = &mut cursor[b as usize];
            idx[*at as usize] = a as u32;
            val[*at as usize] = v;
            *at += 1;
        }
    }
    Ok(Rows { ptr, idx, val })
}

fn root(parent: &mut [u32], mut v: u32) -> u32 {
    while parent[v as usize] != v {
        let up = parent[parent[v as usize] as usize];
        parent[v as usize] = up;
        v = up;
    }
    v
}

/// Component labels of the `a + b` vertices (side-1 level `k` is vertex
/// `a + k`), in the order of each component's smallest vertex, and the
/// label count.
fn components(rows: &Rows, levels: [usize; 2], cancel: &Cancel<'_>) -> Result<(Vec<u32>, usize)> {
    let total = levels[0] + levels[1];
    let mut parent = with_capacity::<u32>(total)?;
    parent.extend(0..total as u32);
    for a in 0..levels[0] {
        if a % (1 << 16) == 0 {
            cancel.check()?;
        }
        for &b in rows.row(a).0 {
            let (x, y) = (
                root(&mut parent, a as u32),
                root(&mut parent, (levels[0] + b as usize) as u32),
            );
            parent[x.max(y) as usize] = x.min(y);
        }
    }
    // Unions and path halving only point a vertex at a smaller one, so in
    // ascending order a vertex's parent already holds its label.
    let mut count = 0_u32;
    for v in 0..total {
        let p = parent[v] as usize;
        parent[v] = if p == v {
            count += 1;
            count - 1
        } else {
            parent[p]
        };
    }
    Ok((parent, count as usize))
}

/// A pair's components, each slot's level and each level's slot per side.
type Layout = (Vec<Component>, Vec<u32>, [Vec<u32>; 2]);

/// Slots of every level and the components; each level's index within its
/// side of its component replaces the cross-tab columns.
fn layout(label: &[u32], count: usize, levels: [usize; 2], rows: &mut [Rows; 2]) -> Result<Layout> {
    let mut sizes = filled(count, [0_usize; 2])?;
    for (v, &c) in label.iter().enumerate() {
        sizes[c as usize][usize::from(v >= levels[0])] += 1;
    }
    let mut parts = with_capacity::<Component>(count)?;
    let mut start = 0;
    for &sizes in &sizes {
        let big = usize::from(sizes[1] > sizes[0]);
        parts.push(Component { start, sizes, big });
        start += sizes[0] + sizes[1];
    }
    let mut codes = filled(start, 0_u32)?;
    let mut slot = [filled(levels[0], 0_u32)?, filled(levels[1], 0_u32)?];
    let mut local = [filled(levels[0], 0_u32)?, filled(levels[1], 0_u32)?];
    // `sizes` counts the levels placed per component and side.
    sizes.iter_mut().for_each(|s| *s = [0; 2]);
    for (v, &c) in label.iter().enumerate() {
        let side = usize::from(v >= levels[0]);
        let level = v - side * levels[0];
        let part = &parts[c as usize];
        let placed = &mut sizes[c as usize][side];
        let at = part.start
            + *placed
            + if side == part.big {
                0
            } else {
                part.sizes[part.big]
            };
        local[side][level] = *placed as u32;
        slot[side][level] = at as u32;
        codes[at] = level as u32;
        *placed += 1;
    }
    for side in 0..2 {
        let other = &local[1 - side];
        rows[side]
            .idx
            .par_iter_mut()
            .for_each(|i| *i = other[*i as usize]);
    }
    Ok((parts, codes, slot))
}

/// `1 / (row sum)` of every level of `rows`.
fn inverse(rows: &Rows, levels: usize) -> Result<Vec<f64>> {
    let mut out = filled(levels, 0.0)?;
    out.par_iter_mut().enumerate().for_each(|(level, out)| {
        *out = 1.0 / rows.row(level).1.iter().sum::<f64>();
    });
    Ok(out)
}

/// Build pair `effects` (side 0 with at least as many levels) at global slot
/// `first`; `None` when its storage exceeds the admission.
pub(super) fn build(
    st: &Structure,
    effects: &[Effect<'_>],
    weights: Option<&[f64]>,
    (sides, first): ([usize; 2], usize),
    admit: &Admit,
    cancel: &Cancel<'_>,
) -> Result<Option<(Pair, Vec<Component>)>> {
    let levels = [effects[sides[0]].levels(), effects[sides[1]].levels()];
    let (a, b, n) = (levels[0] as u128, levels[1] as u128, st.nobs as u128);
    let tasks = groups(levels[0].div_ceil(LEVEL_TASK));
    // A level index of side 0 (with its sort cursor) unless the structure
    // has one, the cross-tab row pointers and the counting stamps.
    let gathered = st.gathered.iter().any(|g| g.effect == sides[0]);
    let index_bytes = if gathered { 0 } else { 16 * a + 4 * n + 8 };
    let counting = index_bytes + 4 * (a + 1) + 4 * b * tasks;
    if !admit.take(counting) {
        return Ok(None);
    }
    let mut owned = None;
    let index = level_index(st, effects, sides[0], &mut owned)?;
    let codes = effects[sides[1]].codes();
    let ptr = count(index, codes, levels, cancel)?;
    let nnz = ptr[levels[0]] as u128;
    // The pair's storage, then the fill's scratch, the transpose cursor, or
    // the component labels, the layout's local indices and sizes and the
    // components (released by the caller once they are pieces).
    admit.give(4 * b * tasks);
    let retained = pair_bytes(levels, nnz as usize);
    let task = std::mem::size_of::<(Range<usize>, &mut [u32], &mut [f64])>() as u128;
    let scratch = (16 * b * tasks + task * tasks)
        .max(4 * b)
        .max(24 * (a + b) + components_bytes(levels));
    if !admit.take(retained + scratch) {
        return Ok(None);
    }
    let by_a = fill(index, codes, weights, (levels, ptr), cancel)?;
    drop(owned);
    admit.give(index_bytes);
    cancel.check()?;
    let by_b = transpose(&by_a, levels)?;
    let (label, count) = components(&by_a, levels, cancel)?;
    let mut rows = [by_a, by_b];
    let (parts, codes, slot) = layout(&label, count, levels, &mut rows)?;
    drop(label);
    let inverse = [inverse(&rows[0], levels[0])?, inverse(&rows[1], levels[1])?];
    admit.give(scratch - components_bytes(levels));
    let pair = Pair {
        effects: sides,
        first,
        codes,
        slot,
        rows,
        inverse,
    };
    Ok(Some((pair, parts)))
}
