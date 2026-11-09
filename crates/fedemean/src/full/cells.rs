//! Channel cells of a pair of effects: the weighted cross blocks
//! `X_c = sum_{i in c} w_i a_i b_i'` of every cell `c`, a level `l` of the
//! larger effect and a level `k` of the smaller one observed together
//! (`a_i`, `b_i` the row's channel values of the two effects).
//!
//! On the pair's coefficients `D'WD` is `[[G_a, X], [X', G_b]]` with
//! block-diagonal level Grams. Eliminating the larger side gives the Schur
//! complement `S = G_b - X' M_a^-1 X`, `M_a^-1` the larger side's level
//! actions ([`super::blocks`]: the exact pseudo-inverse on each `G_l`'s
//! range, and `X_c` lies in it, so `S` is the exact Schur complement). Its
//! diagonal block of level `k`, `S_k = G_k - sum_{c in k} X_c' M_l^-1 X_c`,
//! is formed exactly and acted on by the blocks' rule (exact inverse; a
//! singular block augmented on its null directions, which are null
//! directions of the positive semidefinite `S`).
//!
//! Uses:
//! - **Elimination with slopes** (two effects): PCG on `S` with Jacobi on
//!   its diagonal blocks. `S p = G_b p - X' M_a^-1 X p` runs through the
//!   cells, and the larger side follows by back-substitution.
//! - **A Schwarz subdomain of a pair with slopes:** the block `LDL'` action
//!   `[[I, -M_a^-1 X], [0, I]] diag(M_a^-1, S_d^-1) [[I, 0], [-X' M_a^-1, I]]`,
//!   with `S_d` the diagonal blocks of `S`. It is symmetric positive
//!   definite on the pair's coefficients and exact on the larger side.
//!
//! Every block is formed from row values in the levels' centered bases (each
//! slope less its level mean, [`super::blocks`]), so the cells work in those
//! coordinates: callers move moments into them and coefficients back.
//!
//! Determinism: the cells are a fixed function of the design. Their sums run
//! over each larger level's rows in ascending order, and each smaller
//! level's sums over its cells in ascending order, on tasks fixed by the
//! design.

use super::blocks::{invert, invert_scratch, level, Blocks};
use super::kernels::PARALLEL_WORK;
use super::structure::{sort_levels, Gathered, Structure};
use super::weights::State;
use super::{filled, refill, with_capacity};
use crate::design::Effect;
use crate::elimination::Cancel;
use crate::error::{Error, Result};
use rayon::prelude::*;
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Cells per task of the passes over the larger side's levels ...
const TASK_CELLS: usize = 4096;
/// ... and levels per task over the smaller side's (both fixed).
const LEVEL_TASK: usize = 512;

/// One effect's level actions `M_l^-1` (`c x c` per level): its blocks, or
/// for one intercept channel its inverse masses.
#[derive(Clone, Copy)]
pub(super) struct Actions<'a> {
    values: &'a [f64],
    width: usize,
}

impl<'a> Actions<'a> {
    pub(super) fn of(st: &Structure, state: &'a State, e: usize, channels: usize) -> Self {
        let span = st.offsets[e]..st.offsets[e + 1];
        Self::from((state.blocks(), &state.inverse_mass), (e, span), channels)
    }

    /// From the blocks and inverse masses of a weight state; `span` is
    /// effect `e`'s coefficients.
    pub(super) fn from(
        (blocks, inverse_mass): (Option<&'a Blocks>, &'a [f64]),
        (e, span): (usize, Range<usize>),
        channels: usize,
    ) -> Self {
        match blocks.and_then(|blocks| blocks.effect(e)) {
            Some(values) => Self {
                values,
                width: channels * channels,
            },
            None => Self {
                values: &inverse_mass[span],
                width: 1,
            },
        }
    }

    #[inline]
    fn level(&self, l: usize) -> &'a [f64] {
        &self.values[l * self.width..(l + 1) * self.width]
    }
}

/// The cells of one pair and their blocks for the current weights.
pub(crate) struct Cells {
    /// The larger effect (more levels, first of equals) and the smaller.
    pub(super) effects: [usize; 2],
    pub(super) channels: [usize; 2],
    levels: [usize; 2],
    /// Cells of larger level `l`: `ptr[l]..ptr[l + 1]`, their smaller
    /// levels `idx` ascending.
    ptr: Vec<usize>,
    idx: Vec<u32>,
    /// Each cell's larger level.
    owner: Vec<u32>,
    /// Cells of smaller level `k`, ascending: `by[by_ptr[k]..by_ptr[k + 1]]`.
    by_ptr: Vec<usize>,
    by: Vec<u32>,
    /// Larger-level task bounds (about [`TASK_CELLS`] cells each).
    bounds: Vec<usize>,
    /// The larger effect's level index when the structure has none.
    index: Option<Gathered>,
    /// Per cell `X_c` (`ca x cb`) then `Q_c = sum w b b'` (`cb x cb`), as
    /// formed ...
    xq: Vec<f64>,
    /// ... and `X_c` alone, contiguous for the applies.
    x: Vec<f64>,
    /// Per smaller level `G_k` and the action on `S_k`.
    gram: Vec<f64>,
    schur: Vec<f64>,
    singular: usize,
}

/// The channel values of `effect` at `row` into `out`.
#[inline(always)]
fn values(effect: &Effect<'_>, row: usize, out: &mut [f64]) {
    let mut c = 0;
    if effect.intercept() {
        out[0] = 1.0;
        c = 1;
    }
    for slope in effect.slopes() {
        out[c] = slope[row];
        c += 1;
    }
}

/// Run `body(levels, cell part, level part)` over the larger-level tasks
/// `bounds`, halving recursively without allocation: the parts are the
/// tasks' slices of `cells` (`wc` values per cell) and `levels` (`wl` per
/// larger level), either of which may be empty (width 0).
fn tasks<F>(
    (bounds, ptr): (&[usize], &[usize]),
    (cells, wc): (&mut [f64], usize),
    (levels, wl): (&mut [f64], usize),
    (scratch, ws): (&mut [f64], usize),
    body: &F,
) where
    F: Fn(Range<usize>, (&mut [f64], &mut [f64]), &mut [f64]) + Sync,
{
    if bounds.len() <= 2 {
        if bounds.len() == 2 {
            body(bounds[0]..bounds[1], (cells, levels), &mut scratch[..ws]);
        }
        return;
    }
    let mid = bounds.len() / 2;
    let (c_left, c_right) = cells.split_at_mut((ptr[bounds[mid]] - ptr[bounds[0]]) * wc);
    let (l_left, l_right) = levels.split_at_mut((bounds[mid] - bounds[0]) * wl);
    let (s_left, s_right) = scratch.split_at_mut(mid * ws);
    let (left, right) = (&bounds[..=mid], &bounds[mid..]);
    if (ptr[bounds[bounds.len() - 1]] - ptr[bounds[0]]) * (wc + wl).max(1) < PARALLEL_WORK {
        tasks((left, ptr), (c_left, wc), (l_left, wl), (s_left, ws), body);
        tasks(
            (right, ptr),
            (c_right, wc),
            (l_right, wl),
            (s_right, ws),
            body,
        );
    } else {
        rayon::join(
            || tasks((left, ptr), (c_left, wc), (l_left, wl), (s_left, ws), body),
            || {
                tasks(
                    (right, ptr),
                    (c_right, wc),
                    (l_right, wl),
                    (s_right, ws),
                    body,
                )
            },
        );
    }
}

impl Cells {
    /// The cells of `pair` (`[larger, smaller]`).
    pub(super) fn build(
        st: &Structure,
        effects: &[Effect<'_>],
        pair: [usize; 2],
        cancel: &Cancel<'_>,
    ) -> Result<Self> {
        let [a, b] = pair;
        let levels = [effects[a].levels(), effects[b].levels()];
        let index = if st.gathered.iter().any(|g| g.effect == a) {
            None
        } else {
            let mut index = Gathered {
                effect: a,
                starts: filled(levels[0] + 1, 0)?,
                rows: filled(st.nobs, 0)?,
            };
            sort_levels(effects[a].codes(), &mut index)?;
            Some(index)
        };
        let rows = index
            .as_ref()
            .or_else(|| st.gathered.iter().find(|g| g.effect == a))
            .expect("level index");
        let codes = effects[b].codes();
        cancel.check()?;
        // Each larger level's distinct smaller levels, ascending, each level
        // closed by a marker; a task holds at most its rows plus its levels.
        let count = levels[0].div_ceil(LEVEL_TASK);
        let mut lists: Vec<Result<Vec<u32>>> = with_capacity(count)?;
        (0..count)
            .into_par_iter()
            .map(|t| -> Result<Vec<u32>> {
                cancel.check()?;
                let range = t * LEVEL_TASK..((t + 1) * LEVEL_TASK).min(levels[0]);
                let bound = rows.starts[range.end] - rows.starts[range.start] + range.len();
                let mut stamp = filled(levels[1], u32::MAX)?;
                let mut out = with_capacity(bound)?;
                for l in range {
                    let first = out.len();
                    for &row in &rows.rows[rows.starts[l]..rows.starts[l + 1]] {
                        let k = codes[row as usize];
                        if stamp[k as usize] != l as u32 {
                            stamp[k as usize] = l as u32;
                            out.push(k);
                        }
                    }
                    out[first..].sort_unstable();
                    out.push(u32::MAX);
                }
                Ok(out)
            })
            .collect_into_vec(&mut lists);
        let mut total = 0;
        for list in &lists {
            total += list.as_ref().map_err(|e| *e)?.len();
        }
        let total = total - levels[0];
        let mut ptr = with_capacity(levels[0] + 1)?;
        ptr.push(0);
        let (mut idx, mut owner) = (with_capacity(total)?, with_capacity(total)?);
        let mut l = 0_u32;
        for list in &lists {
            for &k in list.as_ref().map_err(|e| *e)? {
                if k == u32::MAX {
                    ptr.push(idx.len());
                    l += 1;
                } else {
                    idx.push(k);
                    owner.push(l);
                }
            }
        }
        drop(lists);
        let cells = idx.len();
        let mut by_ptr = filled(levels[1] + 1, 0_usize)?;
        for &k in &idx {
            by_ptr[k as usize + 1] += 1;
        }
        for k in 0..levels[1] {
            by_ptr[k + 1] += by_ptr[k];
        }
        let mut cursor = with_capacity(levels[1])?;
        cursor.extend_from_slice(&by_ptr[..levels[1]]);
        let mut by = filled(cells, 0_u32)?;
        for (c, &k) in idx.iter().enumerate() {
            by[cursor[k as usize]] = c as u32;
            cursor[k as usize] += 1;
        }
        let mut bounds = with_capacity(levels[0] / 2 + 2)?;
        bounds.push(0);
        for l in 0..levels[0] {
            if ptr[l + 1] - ptr[*bounds.last().expect("bound")] >= TASK_CELLS {
                bounds.push(l + 1);
            }
        }
        if *bounds.last().expect("bound") != levels[0] {
            bounds.push(levels[0]);
        }
        Ok(Self {
            effects: pair,
            channels: [effects[a].channels(), effects[b].channels()],
            levels,
            ptr,
            idx,
            owner,
            by_ptr,
            by,
            bounds,
            index,
            xq: Vec::new(),
            x: Vec::new(),
            gram: Vec::new(),
            schur: Vec::new(),
            singular: 0,
        })
    }

    pub(super) fn cells(&self) -> usize {
        self.idx.len()
    }

    /// The coefficients of the larger and the smaller effect.
    #[cfg_attr(not(feature = "schwarz"), allow(dead_code))]
    pub(super) fn coefficients(&self) -> [usize; 2] {
        [
            self.levels[0] * self.channels[0],
            self.levels[1] * self.channels[1],
        ]
    }

    pub(super) fn singular(&self) -> usize {
        self.singular
    }

    /// Smaller-level tasks (partials of [`Cells::precondition`]).
    pub(super) fn small_tasks(&self) -> usize {
        self.levels[1].div_ceil(LEVEL_TASK)
    }

    /// Retained bytes, and the bytes of the weight state (`xq`, `gram`,
    /// `schur`) when it is not formed yet.
    pub(super) fn bytes(&self) -> u128 {
        let [ca, cb] = self.channels;
        let (cells, small) = (self.cells() as u128, self.levels[1] as u128);
        let index = self.index.as_ref().map_or(0, |g| {
            8 * g.starts.capacity() as u128 + 4 * g.rows.capacity() as u128
        });
        let structure = 8
            * (self.ptr.capacity() + self.by_ptr.capacity() + self.bounds.capacity()) as u128
            + 4 * (self.idx.capacity() + self.owner.capacity() + self.by.capacity()) as u128;
        let state = 8 * (cells * (2 * ca * cb + cb * cb) as u128 + 2 * small * (cb * cb) as u128);
        index + structure + state
    }

    /// Form every cell's blocks and the Schur diagonal actions for the
    /// current weights (`state`, with the larger effect's level actions).
    pub(super) fn fill(
        &mut self,
        st: &Structure,
        effects: &[Effect<'_>],
        state: &State,
    ) -> Result<()> {
        let [a, b] = self.effects;
        let [ca, cb] = self.channels;
        let (wx, wq) = (ca * cb, cb * cb);
        let stride = wx + wq;
        let count = self.cells();
        refill(&mut self.xq, count * stride, 0.0)?;
        let rows = self
            .index
            .as_ref()
            .or_else(|| st.gathered.iter().find(|g| g.effect == a))
            .expect("level index");
        let weights = state.weights();
        let (ea, eb) = (&effects[a], &effects[b]);
        let (ptr, idx) = (&self.ptr, &self.idx);
        // Both effects' channels in their levels' centered bases.
        let centers = |e: usize| {
            state
                .blocks()
                .map_or((&[][..], &[][..], 0), |blocks| blocks.centers(e))
        };
        let ((centers_a, flat_a, na), (centers_b, flat_b, nb)) = (centers(a), centers(b));
        let body = |range: Range<usize>, (xq, _): (&mut [f64], &mut [f64]), scratch: &mut [f64]| {
            let base = ptr[range.start];
            let (va, vb) = scratch.split_at_mut(ca);
            for l in range {
                let cells = &idx[ptr[l]..ptr[l + 1]];
                for &row in &rows.rows[rows.starts[l]..rows.starts[l + 1]] {
                    let row = row as usize;
                    let k = eb.codes()[row];
                    let c = ptr[l] + cells.binary_search(&k).expect("cell") - base;
                    let w = weights.map_or(1.0, |w| w[row]);
                    values(ea, row, va);
                    values(eb, row, vb);
                    let at = l * na;
                    for (s, v) in va[1..=na].iter_mut().enumerate() {
                        *v = if flat_a[at + s] {
                            0.0
                        } else {
                            *v - centers_a[at + s]
                        };
                    }
                    let at = k as usize * nb;
                    for (s, v) in vb[1..=nb].iter_mut().enumerate() {
                        *v = if flat_b[at + s] {
                            0.0
                        } else {
                            *v - centers_b[at + s]
                        };
                    }
                    let block = &mut xq[c * stride..(c + 1) * stride];
                    for i in 0..ca {
                        let wa = w * va[i];
                        for j in 0..cb {
                            block[i * cb + j] += wa * vb[j];
                        }
                    }
                    for i in 0..cb {
                        let wb = w * vb[i];
                        for j in 0..cb {
                            block[wx + i * cb + j] += wb * vb[j];
                        }
                    }
                }
            }
        };
        let mut scratch = filled(self.large_tasks() * (ca + cb), 0.0)?;
        tasks(
            (&self.bounds, &self.ptr),
            (&mut self.xq, stride),
            (&mut [], 0),
            (&mut scratch, ca + cb),
            &body,
        );
        if !self.xq.iter().all(|v| v.is_finite()) {
            return Err(Error::InvalidData("slope values overflow their cell Grams"));
        }
        refill(&mut self.x, count * wx, 0.0)?;
        for (x, xq) in self
            .x
            .chunks_exact_mut(wx)
            .zip(self.xq.chunks_exact(stride))
        {
            x.copy_from_slice(&xq[..wx]);
        }
        self.schur_blocks(st, state)
    }

    /// The smaller levels' Grams `G_k` (sums of the cells' `Q_c`) and the
    /// actions on their Schur blocks `S_k = G_k - sum X_c' M_l^-1 X_c`, each
    /// judged null against `G_k` (a constant slope's at its raw scale).
    fn schur_blocks(&mut self, st: &Structure, state: &State) -> Result<()> {
        let [a, b] = self.effects;
        let [ca, cb] = self.channels;
        let (wx, wq) = (ca * cb, cb * cb);
        let stride = wx + wq;
        refill(&mut self.gram, self.levels[1] * wq, 0.0)?;
        refill(&mut self.schur, self.levels[1] * wq, 0.0)?;
        let actions = Actions::of(st, state, a, ca);
        let (centers_b, flat_b, nb) = state
            .blocks()
            .map_or((&[][..], &[][..], 0), |blocks| blocks.centers(b));
        let singular = AtomicUsize::new(0);
        let (xq, by_ptr, by, owner) = (&self.xq, &self.by_ptr, &self.by, &self.owner);
        let ws = wx + wq + invert_scratch(cb);
        let mut scratch = filled(self.small_tasks() * ws, 0.0)?;
        let schur =
            |task: usize, ((gram, out), scratch): ((&mut [f64], &mut [f64]), &mut [f64])| {
                let mut count = 0;
                let (t, rest) = scratch.split_at_mut(wx);
                let (reference, rest) = rest.split_at_mut(wq);
                for (j, (g, s)) in gram
                    .chunks_exact_mut(wq)
                    .zip(out.chunks_exact_mut(wq))
                    .enumerate()
                {
                    let k = task * LEVEL_TASK + j;
                    for &c in &by[by_ptr[k]..by_ptr[k + 1]] {
                        let block = &xq[c as usize * stride..(c as usize + 1) * stride];
                        let (x, q) = block.split_at(wx);
                        let m = actions.level(owner[c as usize] as usize);
                        for (g, q) in g.iter_mut().zip(q) {
                            *g += q;
                        }
                        // s -= X' M X, through t = M X (ca x cb).
                        for i in 0..ca {
                            for jj in 0..cb {
                                t[i * cb + jj] =
                                    (0..ca).map(|p| m[i * ca + p] * x[p * cb + jj]).sum();
                            }
                        }
                        for i in 0..cb {
                            for jj in 0..cb {
                                let d: f64 = (0..ca).map(|p| x[p * cb + i] * t[p * cb + jj]).sum();
                                s[i * cb + jj] -= d;
                            }
                        }
                    }
                    for (s, g) in s.iter_mut().zip(g.iter()) {
                        *s += g;
                    }
                    // The reference scale of a constant slope is its raw one.
                    reference.copy_from_slice(g);
                    for (sl, &flat) in flat_b[k * nb..(k + 1) * nb].iter().enumerate() {
                        if flat {
                            let mean = centers_b[k * nb + sl];
                            reference[(sl + 1) * cb + sl + 1] = mean * mean * g[0];
                        }
                    }
                    count += usize::from(invert(s, cb, Some(&*reference), rest));
                }
                singular.fetch_add(count, Ordering::Relaxed);
            };
        let span = LEVEL_TASK * wq;
        if self.levels[1] * wq * ca < PARALLEL_WORK {
            for (t, parts) in self
                .gram
                .chunks_mut(span)
                .zip(self.schur.chunks_mut(span))
                .zip(scratch.chunks_mut(ws))
                .enumerate()
            {
                schur(t, parts);
            }
        } else {
            self.gram
                .par_chunks_mut(span)
                .zip(self.schur.par_chunks_mut(span))
                .zip(scratch.par_chunks_mut(ws))
                .enumerate()
                .for_each(|(t, parts)| schur(t, parts));
        }
        if !self.schur.iter().all(|v| v.is_finite()) {
            return Err(Error::InvalidData("a Schur block is not finite"));
        }
        self.singular = singular.into_inner();
        Ok(())
    }

    /// Over the larger levels: `b_l = seed_l + sum_{c in l} X_c p_k(c)` (the
    /// terms present), `beta_l = M_l^-1 b_l` into `beta` and
    /// `X_c' beta_l` into `cells` (each when nonempty). `seed` and `beta`
    /// are the larger effect's coefficients (`ca R` per level), `p` the
    /// smaller's (`cb R`), `cells` `cb R` per cell; `scratch` holds
    /// [`Cells::scratch_len`] values.
    pub(super) fn larger<const R: usize>(
        &self,
        actions: Actions<'_>,
        (seed, p): (Option<&[f64]>, Option<&[f64]>),
        (cells, beta): (&mut [f64], &mut [f64]),
        scratch: &mut [f64],
    ) {
        let [ca, cb] = self.channels;
        let wx = ca * cb;
        let (ptr, idx, xs) = (&self.ptr, &self.idx, &self.x);
        let (write_cells, write_beta) = (!cells.is_empty(), !beta.is_empty());
        let body =
            |range: Range<usize>, (cells, beta): (&mut [f64], &mut [f64]), scratch: &mut [f64]| {
                let (first, base) = (range.start, ptr[range.start]);
                let mut sink = [[0.0; R]; 3];
                let (rhs, solved) = scratch.split_at_mut(ca * R);
                for l in range {
                    match seed {
                        Some(seed) => {
                            rhs[..ca * R].copy_from_slice(&seed[l * ca * R..(l + 1) * ca * R]);
                        }
                        None => rhs[..ca * R].fill(0.0),
                    }
                    if let Some(p) = p {
                        for (&k, c) in idx[ptr[l]..ptr[l + 1]].iter().zip(ptr[l]..) {
                            let x = &xs[c * wx..(c + 1) * wx];
                            let pk = &p[k as usize * cb * R..(k as usize + 1) * cb * R];
                            for i in 0..ca {
                                for jj in 0..cb {
                                    let v = x[i * cb + jj];
                                    for j in 0..R {
                                        rhs[i * R + j] += v * pk[jj * R + j];
                                    }
                                }
                            }
                        }
                    }
                    let m = actions.level(l);
                    level::<R>(m, ca, rhs, Some(&mut *solved), &mut sink);
                    if write_beta {
                        let at = (l - first) * ca * R;
                        beta[at..at + ca * R].copy_from_slice(solved);
                    }
                    if write_cells {
                        for c in ptr[l]..ptr[l + 1] {
                            let x = &xs[c * wx..(c + 1) * wx];
                            let out = &mut cells[(c - base) * cb * R..(c - base + 1) * cb * R];
                            out.fill(0.0);
                            for i in 0..ca {
                                for jj in 0..cb {
                                    let v = x[i * cb + jj];
                                    for j in 0..R {
                                        out[jj * R + j] += v * solved[i * R + j];
                                    }
                                }
                            }
                        }
                    }
                }
            };
        let wc = if write_cells { cb * R } else { 0 };
        let wl = if write_beta { ca * R } else { 0 };
        let bounds = (&self.bounds[..], &self.ptr[..]);
        tasks(
            bounds,
            (cells, wc),
            (beta, wl),
            (scratch, 2 * ca * R),
            &body,
        );
    }

    /// Larger-level tasks.
    fn large_tasks(&self) -> usize {
        self.bounds.len().saturating_sub(1)
    }

    /// The scratch values [`Cells::larger`] and [`Cells::smaller`] need for
    /// `r` columns (per task: two larger levels' coefficients, or one
    /// smaller level's).
    pub(super) fn scratch_len(&self, r: usize) -> usize {
        let [ca, cb] = self.channels;
        (self.large_tasks() * 2 * ca * r).max(self.small_tasks() * cb * r)
    }

    /// Over the smaller levels, with `s_k = sum_{c in k} cells_c`:
    /// `out_k = G_k input_k - s_k` (the Schur product), or with `solve`
    /// `out_k = S_k^-1 (input_k - s_k)` (`scratch`: [`Cells::scratch_len`]).
    pub(super) fn smaller<const R: usize>(
        &self,
        (cells, input): (&[f64], &[f64]),
        out: &mut [f64],
        (solve, scratch): (bool, &mut [f64]),
    ) {
        let cb = self.channels[1];
        let width = cb * cb;
        let (by_ptr, by) = (&self.by_ptr, &self.by);
        let (gram, schur) = (&self.gram, &self.schur);
        let body = |task: usize, (out, s): (&mut [f64], &mut [f64])| {
            let mut sink = [[0.0; R]; 3];
            for (j, out) in out.chunks_exact_mut(cb * R).enumerate() {
                let k = task * LEVEL_TASK + j;
                let input = &input[k * cb * R..(k + 1) * cb * R];
                let block = k * width..(k + 1) * width;
                let parts = by[by_ptr[k]..by_ptr[k + 1]]
                    .iter()
                    .map(|&c| &cells[c as usize * cb * R..(c as usize + 1) * cb * R]);
                if solve {
                    s.copy_from_slice(input);
                    for part in parts {
                        for (s, v) in s.iter_mut().zip(part) {
                            *s -= v;
                        }
                    }
                    level::<R>(&schur[block], cb, s, Some(out), &mut sink);
                } else {
                    level::<R>(&gram[block], cb, input, Some(out), &mut sink);
                    for part in parts {
                        for (o, v) in out.iter_mut().zip(part) {
                            *o -= v;
                        }
                    }
                }
            }
        };
        let span = LEVEL_TASK * cb * R;
        let scratch = &mut scratch[..self.small_tasks() * cb * R];
        if out.len() * cb < PARALLEL_WORK {
            for (t, parts) in out
                .chunks_mut(span)
                .zip(scratch.chunks_mut(cb * R))
                .enumerate()
            {
                body(t, parts);
            }
        } else {
            out.par_chunks_mut(span)
                .zip(scratch.par_chunks_mut(cb * R))
                .enumerate()
                .for_each(|(t, parts)| body(t, parts));
        }
    }

    /// `q = S p` on the smaller effect's coefficients (`cells`: `cb R` per
    /// cell of workspace; `scratch`: [`Cells::scratch_len`]).
    pub(super) fn apply<const R: usize>(
        &self,
        actions: Actions<'_>,
        (p, q): (&[f64], &mut [f64]),
        cells: &mut [f64],
        scratch: &mut [f64],
    ) {
        self.larger::<R>(actions, (None, Some(p)), (&mut *cells, &mut []), scratch);
        self.smaller::<R>((cells, p), q, (false, scratch));
    }

    /// Jacobi on `S`: `z_k = S_k^-1 r_k`, with the measures of the smaller
    /// effect's level actions `smaller`. Returns per column `[r'z,
    /// max sqrt(r_k' M_k^-1 r_k), max |M_k^-1 r_k|]` from per-task partials
    /// combined in task order (`partials`: [`Cells::small_tasks`] times
    /// `3 R`).
    pub(super) fn precondition<const R: usize>(
        &self,
        smaller: Actions<'_>,
        r: &[f64],
        z: &mut [f64],
        partials: &mut [f64],
    ) -> [[f64; R]; 3] {
        let cb = self.channels[1];
        let width = cb * cb;
        let schur = &self.schur;
        let body = |task: usize, (r, z, slot): (&[f64], &mut [f64], &mut [f64])| {
            let (mut solve, mut measure) = ([[0.0; R]; 3], [[0.0; R]; 3]);
            for (j, (r, z)) in r
                .chunks_exact(cb * R)
                .zip(z.chunks_exact_mut(cb * R))
                .enumerate()
            {
                let k = task * LEVEL_TASK + j;
                level::<R>(
                    &schur[k * width..(k + 1) * width],
                    cb,
                    r,
                    Some(z),
                    &mut solve,
                );
                level::<R>(smaller.level(k), cb, r, None, &mut measure);
            }
            slot[..R].copy_from_slice(&solve[0]);
            slot[R..2 * R].copy_from_slice(&measure[1]);
            slot[2 * R..3 * R].copy_from_slice(&measure[2]);
        };
        let span = LEVEL_TASK * cb * R;
        let partials = &mut partials[..self.small_tasks() * 3 * R];
        if r.len() * cb < PARALLEL_WORK {
            for (t, ((r, z), slot)) in r
                .chunks(span)
                .zip(z.chunks_mut(span))
                .zip(partials.chunks_mut(3 * R))
                .enumerate()
            {
                body(t, (r, z, slot));
            }
        } else {
            r.par_chunks(span)
                .zip(z.par_chunks_mut(span))
                .zip(partials.par_chunks_mut(3 * R))
                .enumerate()
                .for_each(|(t, ((r, z), slot))| body(t, (r, z, slot)));
        }
        let mut out = [[0.0; R]; 3];
        for slot in partials.chunks_exact(3 * R) {
            for j in 0..R {
                out[0][j] += slot[j];
                out[1][j] = out[1][j].max(slot[R + j]);
                out[2][j] = out[2][j].max(slot[2 * R + j]);
            }
        }
        out
    }
}
