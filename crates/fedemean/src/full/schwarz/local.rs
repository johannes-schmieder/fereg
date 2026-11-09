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
//! positive semidefinite with exactly the constants as null space. The
//! `tau` term makes every local action positive definite on its component.
//! In `K`'s coordinates it acts along the component's `(1, -1)` vector,
//! which `D` maps to zero, so it is orthogonal to every consistent residual
//! and never changes PCG or modified LSMR iterates.
//!
//! Every pass is a fixed function of the component: per-row work is
//! independent of the schedule and column sums run over fixed chunks
//! combined in order, so the action does not depend on the worker count.

use super::super::kernels::PARALLEL_WORK;
use super::super::{filled, reserve, with_capacity};
use super::pairs::Pair;
use super::skyline::{Envelope, Skyline};
use super::{Admit, Refusal};
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
/// level and off-diagonal entry exactly.
const ENVELOPE_FACTOR: usize = 16;
/// The fixed seed of approximate Cholesky.
const SEED: u64 = 0x5eed_5c4a_7200_0001;
/// AC(2): every edge split into two samples.
const SPLIT: u32 = 2;
/// Estimated bytes per Schur entry of an approximate factor and its build.
const APPROXIMATE_ENTRY_BYTES: u128 = 48;
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

    fn bytes(&self) -> u128 {
        8 * (self.ptr.capacity() + self.val.capacity()) as u128 + 4 * self.idx.capacity() as u128
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
}

impl Inner {
    pub(super) fn bytes(&self) -> u128 {
        match self {
            Self::Trivial => 0,
            Self::Dense { order, factor } => {
                4 * order.capacity() as u128 + 8 * factor.capacity() as u128
            }
            Self::Skyline(sky) => sky.bytes(),
            Self::Approximate { bytes, .. } => *bytes,
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
            Self::Approximate { factor, .. } => {
                let n = t.len() / R;
                let (b, work) = scratch.split_at_mut(n);
                for j in 0..R {
                    for (i, b) in b.iter_mut().enumerate() {
                        *b = t[i * R + j];
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
        if matches!(self.inner, Inner::Trivial) {
            0
        } else {
            self.sizes[1] + 1
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

/// The upper-triangle Schur rows `T_ij`, `j > i`, of small rows `range`:
/// per row, the larger-side neighbours in order and theirs in order.
fn upper(
    pair: &Pair,
    piece: &Piece,
    range: std::ops::Range<usize>,
) -> Result<(Vec<usize>, Vec<u32>, Vec<f64>)> {
    let [nb, ns] = piece.sizes;
    let codes = &pair.codes[piece.relative..piece.relative + nb + ns];
    let (big_codes, small_codes) = codes.split_at(nb);
    let (by_big, by_small) = (&pair.rows[piece.big], &pair.rows[1 - piece.big]);
    let inverse = &pair.inverse[piece.big];
    let mut sum = filled(ns, 0.0)?;
    let mut stamp = filled(ns, u32::MAX)?;
    let mut touched = with_capacity::<u32>(ns)?;
    let mut lengths = with_capacity::<usize>(range.len())?;
    let (mut idx, mut val) = (Vec::new(), Vec::new());
    for i in range {
        touched.clear();
        let (ks, vs) = by_small.row(small_codes[i] as usize);
        for (&k, &v) in ks.iter().zip(vs) {
            let code = big_codes[k as usize] as usize;
            let f = v * inverse[code];
            let (js, us) = by_big.row(code);
            for (&j, &u) in js.iter().zip(us) {
                if (j as usize) <= i {
                    continue;
                }
                if stamp[j as usize] != i as u32 {
                    stamp[j as usize] = i as u32;
                    sum[j as usize] = 0.0;
                    touched.push(j);
                }
                sum[j as usize] += f * u;
            }
        }
        touched.sort_unstable();
        reserve(&mut idx, touched.len())?;
        reserve(&mut val, touched.len())?;
        idx.extend_from_slice(&touched);
        val.extend(touched.iter().map(|&j| sum[j as usize]));
        lengths.push(touched.len());
    }
    Ok((lengths, idx, val))
}

/// The component's Schur complement, exactly symmetric: upper rows in
/// parallel tasks, mirrored into full rows (columns ascending).
fn schur(pair: &Pair, piece: &Piece, cancel: &Cancel<'_>) -> Result<Schur> {
    let ns = piece.sizes[1];
    let task = (ns / 64).clamp(1, 4096);
    let tasks = ns.div_ceil(task);
    let mut parts = with_capacity(tasks)?;
    parts.resize_with(tasks, || Ok((Vec::new(), Vec::new(), Vec::new())));
    parts.par_iter_mut().enumerate().for_each(|(t, part)| {
        *part = cancel
            .check()
            .and_then(|()| upper(pair, piece, t * task..((t + 1) * task).min(ns)));
    });
    let mut lengths = filled(ns, 0_usize)?;
    let mut upper_rows = with_capacity::<(usize, usize)>(ns)?;
    let mut entries = 0;
    for part in &parts {
        let (part_lengths, _, _) = part.as_ref().map_err(|e| *e)?;
        for &len in part_lengths {
            upper_rows.push((entries, len));
            entries += len;
        }
    }
    let (mut up_idx, mut up_val) = (with_capacity(entries)?, with_capacity(entries)?);
    for part in parts {
        let (_, idx, val) = part?;
        up_idx.extend_from_slice(&idx);
        up_val.extend_from_slice(&val);
    }
    for (i, &(at, len)) in upper_rows.iter().enumerate() {
        lengths[i] += len;
        for &j in &up_idx[at..at + len] {
            lengths[j as usize] += 1;
        }
    }
    let mut ptr = filled(ns + 1, 0_usize)?;
    for i in 0..ns {
        ptr[i + 1] = ptr[i] + lengths[i];
    }
    let mut idx = filled(ptr[ns], 0_u32)?;
    let mut val = filled(ptr[ns], 0.0)?;
    // `lengths` becomes each row's fill cursor: mirrored entries first (in
    // ascending row order), then the row's own upper entries.
    lengths.copy_from_slice(&ptr[..ns]);
    for (i, &(at, len)) in upper_rows.iter().enumerate() {
        for (&j, &w) in up_idx[at..at + len].iter().zip(&up_val[at..at + len]) {
            let cursor = &mut lengths[j as usize];
            idx[*cursor] = i as u32;
            val[*cursor] = w;
            *cursor += 1;
        }
    }
    for (i, &(at, len)) in upper_rows.iter().enumerate() {
        let cursor = lengths[i];
        idx[cursor..cursor + len].copy_from_slice(&up_idx[at..at + len]);
        val[cursor..cursor + len].copy_from_slice(&up_val[at..at + len]);
    }
    Ok(Schur { ptr, idx, val })
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

/// Approximate Cholesky of the Laplacian of `graph` (diagonal inserted in
/// column order).
fn approximate(graph: &Graph<'_>, bytes: u128) -> Result<std::result::Result<Inner, Refusal>> {
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

/// The local solve of `piece` by `local` (see [`SchwarzLocal`]), each
/// factor admitted before it is built (the Schur complement too).
pub(super) fn inner(
    pair: &Pair,
    piece: &Piece,
    local: SchwarzLocal,
    admit: &Admit,
    cancel: &Cancel<'_>,
) -> Result<std::result::Result<Inner, Refusal>> {
    let ns = piece.sizes[1];
    if ns == 1 {
        return Ok(Ok(Inner::Trivial));
    }
    let schur = schur(pair, piece, cancel)?;
    if !admit.take(schur.bytes()) {
        return Ok(Err(Refusal::MemoryLimit));
    }
    let graph = schur.graph();
    let nnz = graph.idx.len();
    let approximate_bytes = APPROXIMATE_ENTRY_BYTES * (ns + nnz) as u128;
    let exact = matches!(local, SchwarzLocal::Exact);
    let filled = ns <= DENSE_LEVELS || (ns <= DENSE_FILLED_LEVELS && 2 * nnz >= ns * (ns - 1));
    let floor = Refusal::Numerical("an exact Schur factorization reached the rounding floor");
    let approximately = |admit: &Admit| {
        if admit.take(approximate_bytes) {
            approximate(&graph, approximate_bytes)
        } else {
            Ok(Err(Refusal::MemoryLimit))
        }
    };
    if matches!(local, SchwarzLocal::Approximate) {
        return approximately(admit);
    }
    if filled {
        let bytes = 8 * (ns * (ns - 1) / 2) as u128 + 4 * ns as u128;
        if !admit.take(bytes) {
            return Ok(Err(Refusal::MemoryLimit));
        }
        return match dense(&graph)? {
            Some(inner) => Ok(Ok(inner)),
            None if exact => Ok(Err(floor)),
            None => approximately(admit),
        };
    }
    let budget = (!exact).then_some(ENVELOPE_FACTOR * (ns + nnz));
    let Some(envelope) = Envelope::new(&graph, budget)? else {
        return approximately(admit);
    };
    if !admit.take(envelope.bytes()) {
        return Ok(Err(Refusal::MemoryLimit));
    }
    match Skyline::new(&graph, envelope, cancel)? {
        Some(sky) => Ok(Ok(Inner::Skyline(sky))),
        None if exact => Ok(Err(floor)),
        None => approximately(admit),
    }
}
