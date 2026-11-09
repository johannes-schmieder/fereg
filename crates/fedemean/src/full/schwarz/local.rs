//! One subdomain's local solve.
//!
//! A component's block `K = [[D1, C], [C', D2]]` (its larger side first,
//! `C >= 0`, `D1` and `D2` the row and column sums of `C`) becomes the graph
//! Laplacian `L = S K S = [[D1, -C], [-C', D2]]` after the sign flip `S` of
//! the smaller side, with the component's constants as its null space. The
//! larger side's diagonal is eliminated exactly: with the Schur complement
//! `T = D2 - C' D1^-1 C` (a Laplacian on the smaller side, its off-diagonal
//! entries formed once in the upper triangle and mirrored, its diagonal
//! their row sums) and `T^g` a symmetric generalized inverse of `T`,
//!
//! ```text
//!   G = [[D1^-1 + D1^-1 C T^g C' D1^-1, D1^-1 C T^g], [T^g C' D1^-1, T^g]]
//! ```
//!
//! is a symmetric generalized inverse of `L` (`L G L = L`), positive
//! semidefinite when `T^g` is. The local action on the component's
//! coefficients `r` is
//!
//! ```text
//!   z = S (P G P + tau 1 1' / n) S r,     tau = 1,
//! ```
//!
//! with `P` the removal of the component mean. For an exact `T^g` (a
//! grounded dense or envelope factorization, or nothing when `T` has one
//! level) `P G P = L^+`; approximate Cholesky gives a symmetric positive
//! definite approximation on the complement of the constants, so `P G P` is
//! positive semidefinite with exactly the constants as null space. When the
//! Schur complement is denser than the component's own bipartite graph
//! (larger-side levels in many smaller-side levels make it nearly dense),
//! approximate Cholesky factors `L` itself instead, which keeps its fill
//! sampled, and `G` is that factor's solve: the same contract. The
//! `tau` term makes every local action positive definite on its component.
//! In `K`'s coordinates it acts along the component's `(1, -1)` vector,
//! which `D` maps to zero, so it is orthogonal to every consistent residual
//! and never changes PCG or modified LSMR iterates.
//!
//! Every pass is a fixed function of the component: per-row work is
//! independent of the schedule and column sums run over fixed chunks
//! combined in order, so the action does not depend on the worker count.

use super::super::kernels::PARALLEL_WORK;
use super::super::{filled, with_capacity};
use super::pairs::Pair;
use super::skyline::{Envelope, Skyline};
use super::{Admit, Refusal, GROUPS};
use crate::elimination::dense::{cholesky, packed, solve as dense_solve};
use crate::elimination::Cancel;
use crate::error::Result;
use crate::projector::SchwarzLocal;
use approx_chol::{Config, CsrRef, Factor};
use rayon::prelude::*;

/// Schur complements with at most this many levels are factored densely ...
const DENSE_LEVELS: usize = 128;
/// ... and up to this many when at least half of their off-diagonal
/// entries are present.
const DENSE_FILLED_LEVELS: usize = 1024;
/// The automatic rule factors an envelope of at most this many entries per
/// level and off-diagonal entry exactly ...
const ENVELOPE_FACTOR: usize = 16;
/// ... when its factorization work `sum w^2` over the rows' envelope widths
/// is at most this many operations per level and off-diagonal entry (a
/// banded Schur complement of half-bandwidth up to about this; wide or
/// expander-like ones, whose exact factor costs up to `n^3`, are approximate).
const ENVELOPE_WORK_FACTOR: u128 = 128;
/// The fixed seed of approximate Cholesky.
const SEED: u64 = 0x5eed_5c4a_7200_0001;
/// AC(2): every edge split into two samples.
const SPLIT: u32 = 2;
/// Estimated bytes per Schur entry of an approximate factor ...
const APPROXIMATE_ENTRY_BYTES: u128 = 48;
/// ... and of approximate Cholesky's peak on its input, the factor
/// included (measured at 70 to 110 on worker-firm components).
const APPROXIMATE_BUILD_BYTES: u128 = 160;
/// The weight of the component constants in the local action.
const TAU: f64 = 1.0;
/// Slots per partial sum of a component's column sums (fixed).
const SUM_CHUNK: usize = 4096;

/// A symmetric weighted graph by its off-diagonal rows (columns ascending).
#[derive(Clone, Copy)]
pub(super) struct Graph<'a> {
    ptr: &'a [usize],
    idx: &'a [u32],
    val: &'a [f64],
}

impl<'a> Graph<'a> {
    pub(super) fn n(&self) -> usize {
        self.ptr.len() - 1
    }

    pub(super) fn neighbours(&self, v: usize) -> &'a [u32] {
        &self.idx[self.ptr[v]..self.ptr[v + 1]]
    }

    pub(super) fn weights(&self, v: usize) -> &'a [f64] {
        &self.val[self.ptr[v]..self.ptr[v + 1]]
    }

    fn degree(&self, v: usize) -> f64 {
        self.weights(v).iter().sum()
    }
}

/// An exactly symmetric Schur complement's off-diagonal weights.
struct Schur {
    ptr: Vec<usize>,
    idx: Vec<u32>,
    val: Vec<f64>,
}

impl Schur {
    fn graph(&self) -> Graph<'_> {
        Graph {
            ptr: &self.ptr,
            idx: &self.idx,
            val: &self.val,
        }
    }
}

/// The solve of a component's Schur complement.
pub(super) enum Inner {
    /// One level on the smaller side: `T = 0`.
    Trivial,
    /// Grounded dense Cholesky factor (packed lower rows) in `order`, the
    /// grounded level (largest degree, first of equals) last.
    Dense {
        order: Vec<u32>,
        factor: Vec<f64>,
    },
    Skyline(Skyline),
    Approximate {
        factor: Factor<f64>,
        bytes: u128,
    },
    /// Approximate Cholesky of the whole component's Laplacian `L` (no
    /// elimination stage).
    Component {
        factor: Factor<f64>,
        bytes: u128,
    },
}

impl Inner {
    pub(super) fn bytes(&self) -> u128 {
        match self {
            Self::Trivial => 0,
            Self::Dense { order, factor } => {
                4 * order.capacity() as u128 + 8 * factor.capacity() as u128
            }
            Self::Skyline(sky) => sky.bytes(),
            Self::Approximate { bytes, .. } | Self::Component { bytes, .. } => *bytes,
        }
    }

    /// `t` (`n` levels, `R` interleaved columns, summing to zero) becomes
    /// `T^g t`; `scratch` holds `(n + 1) max(R, 2)` values.
    fn solve<const R: usize>(&self, t: &mut [f64], scratch: &mut [f64]) {
        match self {
            Self::Trivial => t.fill(0.0),
            Self::Dense { order, factor } => {
                let m = order.len() - 1;
                let z = &mut scratch[..m * R];
                for (p, &v) in order[..m].iter().enumerate() {
                    let v = v as usize;
                    z[p * R..(p + 1) * R].copy_from_slice(&t[v * R..(v + 1) * R]);
                }
                dense_solve(factor, z, m, R);
                for (p, &v) in order[..m].iter().enumerate() {
                    let v = v as usize;
                    t[v * R..(v + 1) * R].copy_from_slice(&z[p * R..(p + 1) * R]);
                }
                let ground = order[m] as usize;
                t[ground * R..(ground + 1) * R].fill(0.0);
            }
            Self::Skyline(sky) => sky.solve::<R>(t, scratch),
            Self::Approximate { factor, .. } | Self::Component { factor, .. } => {
                approximate_solve::<R>(factor, t, scratch, [0.0; R]);
            }
        }
    }
}

/// `t` (`n` levels, `R` interleaved columns) minus `shift` becomes the
/// approximate factor's solve; `scratch` holds `2 n + 2` values.
fn approximate_solve<const R: usize>(
    factor: &Factor<f64>,
    t: &mut [f64],
    scratch: &mut [f64],
    shift: [f64; R],
) {
    let n = t.len() / R;
    let (b, work) = scratch.split_at_mut(n);
    for j in 0..R {
        for (i, b) in b.iter_mut().enumerate() {
            *b = t[i * R + j] - shift[j];
        }
        if factor.solve_into(b, &mut work[..factor.n()]).is_err() {
            t.fill(f64::NAN);
            return;
        }
        for (i, &x) in work[..n].iter().enumerate() {
            t[i * R + j] = x;
        }
    }
}

/// One subdomain: a component of one pair.
pub(super) struct Piece {
    pub(super) pair: usize,
    /// The pair side of the larger side.
    pub(super) big: usize,
    /// First slot, relative to the pair and in the local storage.
    pub(super) relative: usize,
    pub(super) start: usize,
    /// Levels of the larger and the smaller side.
    pub(super) sizes: [usize; 2],
    /// Coefficient offsets of the larger and the smaller side's effects.
    pub(super) offsets: [usize; 2],
    pub(super) inner: Inner,
    /// First unit of the inner-solve scratch and of the partial sums.
    pub(super) inner_first: usize,
    pub(super) partial_first: usize,
}

/// `body(row, values)` over the `R`-value rows of `x`, in parallel when
/// `parallel` (per-row work is independent of the schedule).
fn rows<const R: usize, F>(x: &mut [f64], parallel: bool, body: F)
where
    F: Fn(usize, &mut [f64]) + Sync + Send,
{
    if parallel {
        x.par_chunks_mut(R)
            .enumerate()
            .for_each(|(i, row)| body(i, row));
    } else {
        for (i, row) in x.chunks_exact_mut(R).enumerate() {
            body(i, row);
        }
    }
}

/// Column means of the `R`-value rows of `x`: sums over fixed chunks of
/// rows, combined in chunk order.
fn means<const R: usize>(x: &[f64], partials: &mut [f64]) -> [f64; R] {
    let n = x.len() / R;
    let sum = |chunk: &[f64]| {
        let mut s = [0.0; R];
        for row in chunk.chunks_exact(R) {
            for j in 0..R {
                s[j] += row[j];
            }
        }
        s
    };
    let mut total = [0.0; R];
    if n <= SUM_CHUNK {
        total = sum(x);
    } else {
        let parts = &mut partials[..n.div_ceil(SUM_CHUNK) * R];
        parts
            .par_chunks_mut(R)
            .zip(x.par_chunks(SUM_CHUNK * R))
            .for_each(|(part, chunk)| part.copy_from_slice(&sum(chunk)));
        for part in parts.chunks_exact(R) {
            for j in 0..R {
                total[j] += part[j];
            }
        }
    }
    total.map(|t| t / n as f64)
}

impl Piece {
    pub(super) fn n(&self) -> usize {
        self.sizes[0] + self.sizes[1]
    }

    pub(super) fn inner_units(&self) -> usize {
        match self.inner {
            Inner::Trivial => 0,
            Inner::Component { .. } => self.n() + 1,
            _ => self.sizes[1] + 1,
        }
    }

    pub(super) fn partial_units(&self) -> usize {
        let n = self.n();
        if n > SUM_CHUNK {
            n.div_ceil(SUM_CHUNK)
        } else {
            0
        }
    }

    /// The local action on `r` (all coefficients, `R` interleaved columns)
    /// into `x`, this piece's slots.
    pub(super) fn solve<const R: usize>(
        &self,
        pair: &Pair,
        r: &[f64],
        x: &mut [f64],
        scratch: &mut [f64],
        partials: &mut [f64],
    ) {
        let [nb, _] = self.sizes;
        let codes = &pair.codes[self.relative..self.relative + self.n()];
        let parallel = self.n() * R >= PARALLEL_WORK;
        let [big, small] = self.offsets;
        rows::<R, _>(x, parallel, |s, out| {
            let (g, sign) = if s < nb {
                (big + codes[s] as usize, 1.0)
            } else {
                (small + codes[s] as usize, -1.0)
            };
            for j in 0..R {
                out[j] = sign * r[g * R + j];
            }
        });
        let mean = means::<R>(x, partials);
        if let Inner::Component { factor, .. } = &self.inner {
            approximate_solve::<R>(factor, x, scratch, mean);
        } else {
            let inverse = &pair.inverse[self.big];
            let (xb, xs) = x.split_at_mut(nb * R);
            rows::<R, _>(xb, parallel, |k, out| {
                let d = inverse[codes[k] as usize];
                for j in 0..R {
                    out[j] = (out[j] - mean[j]) * d;
                }
            });
            if matches!(self.inner, Inner::Trivial) {
                xs.fill(0.0);
            } else {
                self.eliminated::<R>((xb, xs), codes, pair, (scratch, partials), mean);
            }
        }
        let out_mean = means::<R>(x, partials);
        rows::<R, _>(x, parallel, |s, out| {
            let sign = if s < nb { 1.0 } else { -1.0 };
            for j in 0..R {
                out[j] = sign * (out[j] - out_mean[j] + TAU * mean[j]);
            }
        });
    }

    /// The Schur stage: `t = (x_s - mean) + C' (D1^-1 x_b)`, centered, then
    /// `x_s = T^g t` and `x_b += D1^-1 C x_s` (`x_b` holds `D1^-1 x_b`).
    fn eliminated<const R: usize>(
        &self,
        (xb, xs): (&mut [f64], &mut [f64]),
        codes: &[u32],
        pair: &Pair,
        (scratch, partials): (&mut [f64], &mut [f64]),
        mean: [f64; R],
    ) {
        let parallel = self.n() * R >= PARALLEL_WORK;
        let nb = self.sizes[0];
        let (by_big, by_small) = (&pair.rows[self.big], &pair.rows[1 - self.big]);
        let small_codes = &codes[nb..];
        let reduced = &*xb;
        rows::<R, _>(xs, parallel, |i, out| {
            let mut s = [0.0; R];
            for j in 0..R {
                s[j] = out[j] - mean[j];
            }
            let (ks, vs) = by_small.row(small_codes[i] as usize);
            for (&k, &v) in ks.iter().zip(vs) {
                let k = k as usize;
                for j in 0..R {
                    s[j] += v * reduced[k * R + j];
                }
            }
            out.copy_from_slice(&s);
        });
        let t_mean = means::<R>(xs, partials);
        for row in xs.chunks_exact_mut(R) {
            for j in 0..R {
                row[j] -= t_mean[j];
            }
        }
        self.inner.solve::<R>(xs, scratch);
        let solved = &*xs;
        let inverse = &pair.inverse[self.big];
        rows::<R, _>(xb, parallel, |k, out| {
            let code = codes[k] as usize;
            let (js, vs) = by_big.row(code);
            let mut s = [0.0; R];
            for (&i, &v) in js.iter().zip(vs) {
                let i = i as usize;
                for j in 0..R {
                    s[j] += v * solved[i * R + j];
                }
            }
            let d = inverse[code];
            for j in 0..R {
                out[j] += d * s[j];
            }
        });
    }
}

/// Every path from small row `i` of a component through a larger-side
/// level to a smaller level `j > i`, in order: `f(j, first, weight)`, with
/// `first` the path's first visit of `j` (by `stamp`, marked `mark`) and
/// `weight` its term of the Schur entry `T_ij`.
#[inline]
fn paths<F>((pair, piece): (&Pair, &Piece), i: usize, (stamp, mark): (&mut [u32], u32), mut f: F)
where
    F: FnMut(usize, bool, f64),
{
    let [nb, ns] = piece.sizes;
    let codes = &pair.codes[piece.relative..piece.relative + nb + ns];
    let (big_codes, small_codes) = codes.split_at(nb);
    let (by_big, by_small) = (&pair.rows[piece.big], &pair.rows[1 - piece.big]);
    let inverse = &pair.inverse[piece.big];
    let (ks, vs) = by_small.row(small_codes[i] as usize);
    for (&k, &v) in ks.iter().zip(vs) {
        let code = big_codes[k as usize] as usize;
        let factor = v * inverse[code];
        let (js, us) = by_big.row(code);
        for (&j, &u) in js.iter().zip(us) {
            if (j as usize) <= i {
                continue;
            }
            let first = stamp[j as usize] != mark;
            stamp[j as usize] = mark;
            f(j as usize, first, factor * u);
        }
    }
}

/// Rows per group of the Schur passes over `ns` small rows (at most
/// [`GROUPS`] groups).
fn group_rows(ns: usize) -> usize {
    ns.div_ceil(GROUPS).max(1)
}

/// The Schur complement's shape before it is formed: each small row's
/// upper entries (`j > i`) and lower entries (its mirrors).
struct Shape {
    upper: Vec<usize>,
    lower: Vec<usize>,
}

impl Shape {
    /// Upper entries in all.
    fn entries(&self) -> usize {
        self.upper.iter().sum()
    }

    /// Bytes of its counts.
    fn bytes(ns: usize) -> u128 {
        16 * ns as u128
    }
}

/// Bytes of the Schur passes' per-group scratch over `ns` small rows: the
/// stamps, and when forming the sums and touched levels.
fn group_scratch(ns: usize, form: bool) -> u128 {
    let groups = ns.div_ceil(group_rows(ns)) as u128;
    groups * ns as u128 * if form { 16 } else { 4 }
}

/// Count the component's Schur entries per row (no values).
fn shape(pair: &Pair, piece: &Piece, cancel: &Cancel<'_>) -> Result<Shape> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let ns = piece.sizes[1];
    let per = group_rows(ns);
    let mut upper = filled(ns, 0_usize)?;
    let lower: Vec<AtomicUsize> = {
        let mut lower = with_capacity(ns)?;
        lower.extend((0..ns).map(|_| AtomicUsize::new(0)));
        lower
    };
    upper
        .par_chunks_mut(per)
        .enumerate()
        .try_for_each(|(g, out)| -> Result<()> {
            cancel.check()?;
            let mut stamp = filled(ns, u32::MAX)?;
            for (r, count) in out.iter_mut().enumerate() {
                let i = g * per + r;
                paths((pair, piece), i, (&mut stamp, i as u32), |j, first, _| {
                    if first {
                        *count += 1;
                        lower[j].fetch_add(1, Ordering::Relaxed);
                    }
                });
            }
            Ok(())
        })?;
    let mut counts = filled(ns, 0_usize)?;
    for (count, lower) in counts.iter_mut().zip(lower) {
        *count = lower.into_inner();
    }
    Ok(Shape {
        upper,
        lower: counts,
    })
}

/// The component's Schur complement of `shape`, exactly symmetric: upper
/// rows formed in parallel groups into their places, then mirrored (each
/// row's mirrored entries first, in ascending row order, then its own
/// upper entries, columns ascending).
fn schur(pair: &Pair, piece: &Piece, shape: &Shape, cancel: &Cancel<'_>) -> Result<Schur> {
    let ns = piece.sizes[1];
    let mut ptr = filled(ns + 1, 0_usize)?;
    for i in 0..ns {
        ptr[i + 1] = ptr[i] + shape.lower[i] + shape.upper[i];
    }
    let mut idx = filled(ptr[ns], 0_u32)?;
    let mut val = filled(ptr[ns], 0.0)?;
    {
        // Each row's upper part, by row.
        let mut rows = with_capacity::<(&mut [u32], &mut [f64])>(ns)?;
        let (mut rest_idx, mut rest_val) = (&mut idx[..], &mut val[..]);
        for i in 0..ns {
            let (lower, upper) = (shape.lower[i], shape.upper[i]);
            let (_, tail_idx) = std::mem::take(&mut rest_idx).split_at_mut(lower);
            let (row_idx, tail_idx) = tail_idx.split_at_mut(upper);
            let (_, tail_val) = std::mem::take(&mut rest_val).split_at_mut(lower);
            let (row_val, tail_val) = tail_val.split_at_mut(upper);
            rows.push((row_idx, row_val));
            (rest_idx, rest_val) = (tail_idx, tail_val);
        }
        let per = group_rows(ns);
        rows.par_chunks_mut(per)
            .enumerate()
            .try_for_each(|(g, rows)| -> Result<()> {
                cancel.check()?;
                let mut sum = filled(ns, 0.0)?;
                let mut stamp = filled(ns, u32::MAX)?;
                let mut touched = with_capacity::<u32>(ns)?;
                for (r, (idx, val)) in rows.iter_mut().enumerate() {
                    let i = g * per + r;
                    touched.clear();
                    paths((pair, piece), i, (&mut stamp, i as u32), |j, first, w| {
                        if first {
                            sum[j] = 0.0;
                            touched.push(j as u32);
                        }
                        sum[j] += w;
                    });
                    touched.sort_unstable();
                    idx.copy_from_slice(&touched);
                    for (v, &j) in val.iter_mut().zip(&touched) {
                        *v = sum[j as usize];
                    }
                }
                Ok(())
            })?;
    }
    // Mirror each row's upper entries into the later rows' lower parts.
    let mut cursor = with_capacity::<usize>(ns)?;
    cursor.extend_from_slice(&ptr[..ns]);
    for i in 0..ns {
        let at = ptr[i] + shape.lower[i];
        let (head_idx, tail_idx) = idx.split_at_mut(ptr[i + 1]);
        let (head_val, tail_val) = val.split_at_mut(ptr[i + 1]);
        for (&j, &w) in head_idx[at..].iter().zip(&head_val[at..]) {
            let slot = cursor[j as usize] - ptr[i + 1];
            tail_idx[slot] = i as u32;
            tail_val[slot] = w;
            cursor[j as usize] += 1;
        }
    }
    Ok(Schur { ptr, idx, val })
}

/// The component's bipartite level graph (larger side first, then the
/// smaller side, both ascending) by its off-diagonal weights: the
/// cross-tabulation `C` and its transpose.
fn component(pair: &Pair, piece: &Piece) -> Result<Schur> {
    let [nb, ns] = piece.sizes;
    let codes = &pair.codes[piece.relative..piece.relative + nb + ns];
    let (by_big, by_small) = (&pair.rows[piece.big], &pair.rows[1 - piece.big]);
    let edges: usize = codes[..nb]
        .iter()
        .map(|&code| by_big.row(code as usize).0.len())
        .sum();
    let mut ptr = with_capacity(nb + ns + 1)?;
    let (mut idx, mut val) = (with_capacity(2 * edges)?, with_capacity(2 * edges)?);
    ptr.push(0);
    for &code in &codes[..nb] {
        let (js, vs) = by_big.row(code as usize);
        idx.extend(js.iter().map(|&j| nb as u32 + j));
        val.extend_from_slice(vs);
        ptr.push(idx.len());
    }
    for &code in &codes[nb..] {
        let (ks, vs) = by_small.row(code as usize);
        idx.extend_from_slice(ks);
        val.extend_from_slice(vs);
        ptr.push(idx.len());
    }
    Ok(Schur { ptr, idx, val })
}

/// One piece's charge to the build's [`Admit`]: raised to each stage's
/// peak before the stage allocates, never lowered (the build releases it
/// once every piece is factored).
struct Charge<'a> {
    admit: &'a Admit,
    held: u128,
}

impl Charge<'_> {
    /// Raise the charge to `peak` bytes; `false` beyond the limit.
    fn to(&mut self, peak: u128) -> bool {
        if peak <= self.held {
            return true;
        }
        let more = peak - self.held;
        self.held = peak;
        self.admit.take(more)
    }
}

/// Bytes of a graph of `n` levels and `nnz` off-diagonal entries ...
fn graph_bytes(n: usize, nnz: usize) -> u128 {
    8 * (n as u128 + 1) + 12 * nnz as u128
}

/// ... of its approximate Cholesky input (diagonal inserted) ...
fn input_bytes(n: usize, nnz: usize) -> u128 {
    4 * (n as u128 + 1) + 12 * (nnz + n) as u128
}

/// ... and an estimate of approximate Cholesky's peak on it, the factor
/// included.
fn approximate_peak(n: usize, nnz: usize) -> u128 {
    APPROXIMATE_BUILD_BYTES * (n + nnz) as u128
}

/// The larger-to-smaller edges of a component.
fn edges(pair: &Pair, piece: &Piece) -> usize {
    let [nb, _] = piece.sizes;
    let codes = &pair.codes[piece.relative..piece.relative + nb];
    let by_big = &pair.rows[piece.big];
    codes.iter().map(|&c| by_big.row(c as usize).0.len()).sum()
}

/// Approximate Cholesky of the Schur complement `schur` (`nnz` entries),
/// or of the whole component when its bipartite graph has fewer entries
/// (the Schur complement, when formed, freed first), each charged first.
fn approximately(
    (pair, piece): (&Pair, &Piece),
    (schur, nnz): (Option<Schur>, usize),
    charge: &mut Charge<'_>,
) -> Result<std::result::Result<Inner, Refusal>> {
    let edges = edges(pair, piece);
    let ns = piece.sizes[1];
    if 2 * edges < nnz || schur.is_none() {
        drop(schur);
        let (n, entries) = (piece.n(), 2 * edges);
        let bytes = APPROXIMATE_ENTRY_BYTES * (n + entries) as u128 + 12 * (n + entries) as u128;
        let peak = (graph_bytes(n, entries) + input_bytes(n, entries))
            .max(input_bytes(n, entries) + approximate_peak(n, entries));
        if !charge.to(peak) {
            return Ok(Err(Refusal::MemoryLimit));
        }
        let whole = component(pair, piece)?;
        return Ok(approximate(whole, bytes)?.map(|inner| match inner {
            Inner::Approximate { factor, bytes } => Inner::Component { factor, bytes },
            other => other,
        }));
    }
    let schur = schur.expect("formed");
    let bytes = APPROXIMATE_ENTRY_BYTES * (ns + nnz) as u128;
    let peak = (graph_bytes(ns, nnz) + input_bytes(ns, nnz))
        .max(input_bytes(ns, nnz) + approximate_peak(ns, nnz));
    if charge.to(peak) {
        approximate(schur, bytes)
    } else {
        Ok(Err(Refusal::MemoryLimit))
    }
}

/// Grounded dense Cholesky factor of `graph`; `None` at the rounding floor.
fn dense(graph: &Graph<'_>) -> Result<Option<Inner>> {
    let n = graph.n();
    let m = n - 1;
    let mut ground = 0;
    let mut largest = graph.degree(0);
    for v in 1..n {
        let degree = graph.degree(v);
        if degree > largest {
            (ground, largest) = (v, degree);
        }
    }
    let mut order = with_capacity::<u32>(n)?;
    order.extend((0..n as u32).filter(|&v| v as usize != ground));
    order.push(ground as u32);
    let mut position = filled(n, 0_u32)?;
    for (p, &v) in order.iter().enumerate() {
        position[v as usize] = p as u32;
    }
    let mut factor = filled(m * (m + 1) / 2, 0.0)?;
    for (p, &v) in order[..m].iter().enumerate() {
        let v = v as usize;
        factor[packed(p, p)] = graph.degree(v);
        for (&u, &w) in graph.neighbours(v).iter().zip(graph.weights(v)) {
            let q = position[u as usize] as usize;
            if q < p {
                factor[packed(p, q)] = -w;
            }
        }
    }
    Ok(cholesky(&mut factor, m).then_some(Inner::Dense { order, factor }))
}

/// Approximate Cholesky of the Laplacian of `source` (diagonal inserted in
/// column order), freed once its input is formed.
fn approximate(source: Schur, bytes: u128) -> Result<std::result::Result<Inner, Refusal>> {
    let graph = source.graph();
    let n = graph.n();
    let entries = graph.idx.len() + n;
    let mut ptr = with_capacity::<u32>(n + 1)?;
    let mut idx = with_capacity::<u32>(entries)?;
    let mut val = with_capacity::<f64>(entries)?;
    ptr.push(0);
    for v in 0..n {
        let degree = graph.degree(v);
        let mut placed = false;
        for (&u, &w) in graph.neighbours(v).iter().zip(graph.weights(v)) {
            if !placed && u as usize > v {
                idx.push(v as u32);
                val.push(degree);
                placed = true;
            }
            idx.push(u);
            val.push(-w);
        }
        if !placed {
            idx.push(v as u32);
            val.push(degree);
        }
        ptr.push(idx.len() as u32);
    }
    drop(source);
    let config = Config {
        seed: SEED,
        split_merge: Some(SPLIT),
        ..Config::default()
    };
    let factored = CsrRef::new(&ptr, &idx, &val, n as u32)
        .and_then(|csr| approx_chol::factorize_with(csr, config));
    Ok(match factored {
        Ok(factor) => Ok(Inner::Approximate { factor, bytes }),
        Err(_) => Err(Refusal::Numerical(
            "approximate Cholesky refused a Schur complement",
        )),
    })
}

/// The local solve of `piece` by `local` (see [`SchwarzLocal`]) and the
/// bytes charged for it: each stage charged before it allocates (the Schur
/// complement's shape counted first, so that it is charged exactly).
pub(super) fn inner(
    pair: &Pair,
    piece: &Piece,
    local: SchwarzLocal,
    admit: &Admit,
    cancel: &Cancel<'_>,
) -> Result<std::result::Result<(Inner, u128), Refusal>> {
    let ns = piece.sizes[1];
    if ns == 1 {
        return Ok(Ok((Inner::Trivial, 0)));
    }
    let mut charge = Charge { admit, held: 0 };
    let built = solve(pair, piece, local, &mut charge, cancel)?;
    Ok(built.map(|inner| (inner, charge.held)))
}

/// [`inner`] after its trivial case.
fn solve(
    pair: &Pair,
    piece: &Piece,
    local: SchwarzLocal,
    charge: &mut Charge<'_>,
    cancel: &Cancel<'_>,
) -> Result<std::result::Result<Inner, Refusal>> {
    let ns = piece.sizes[1];
    let refused = Ok(Err(Refusal::MemoryLimit));
    if !charge.to(Shape::bytes(ns) + 8 * ns as u128 + group_scratch(ns, false)) {
        return refused;
    }
    let shape = shape(pair, piece, cancel)?;
    let nnz = 2 * shape.entries();
    if matches!(local, SchwarzLocal::Approximate) && 2 * edges(pair, piece) < nnz {
        // The component is factored instead: no Schur complement.
        drop(shape);
        return approximately((pair, piece), (None, nnz), charge);
    }
    let formed = graph_bytes(ns, nnz);
    let forming = Shape::bytes(ns) + formed + 40 * ns as u128 + group_scratch(ns, true);
    if !charge.to(forming) {
        return refused;
    }
    let schur = schur(pair, piece, &shape, cancel)?;
    drop(shape);
    let exact = matches!(local, SchwarzLocal::Exact);
    let filled = ns <= DENSE_LEVELS || (ns <= DENSE_FILLED_LEVELS && 2 * nnz >= ns * (ns - 1));
    let floor = Refusal::Numerical("an exact Schur factorization reached the rounding floor");
    if matches!(local, SchwarzLocal::Approximate) {
        return approximately((pair, piece), (Some(schur), nnz), charge);
    }
    if filled {
        let bytes = 8 * (ns * (ns - 1) / 2) as u128 + 8 * ns as u128;
        if !charge.to(formed + bytes) {
            return refused;
        }
        return match dense(&schur.graph())? {
            Some(inner) => Ok(Ok(inner)),
            None if exact => Ok(Err(floor)),
            None => approximately((pair, piece), (Some(schur), nnz), charge),
        };
    }
    let budget = (!exact).then_some((
        ENVELOPE_FACTOR * (ns + nnz),
        ENVELOPE_WORK_FACTOR * (ns + nnz) as u128,
    ));
    // The ordering's transients beside the Schur complement.
    if !charge.to(formed + 25 * ns as u128) {
        return refused;
    }
    let Some(envelope) = Envelope::new(&schur.graph(), budget)? else {
        return approximately((pair, piece), (Some(schur), nnz), charge);
    };
    if !charge.to(formed + envelope.bytes() + 12 * ns as u128) {
        return refused;
    }
    match Skyline::new(&schur.graph(), envelope, cancel)? {
        Some(sky) => Ok(Ok(Inner::Skyline(sky))),
        None if exact => Ok(Err(floor)),
        None => approximately((pair, piece), (Some(schur), nnz), charge),
    }
}
