//! Exact envelope (skyline) Cholesky factorization of a connected weighted
//! graph Laplacian in reverse Cuthill-McKee order, the last vertex
//! grounded. Cheap and exact on banded (chain-like) Schur complements;
//! expander-like graphs exceed the envelope or factorization-work budget of
//! the automatic rule.

use super::super::{filled, with_capacity};
use super::local::Graph;
use crate::elimination::Cancel;
use crate::error::Result;

/// The grounded envelope factor `L = R R'` of one Laplacian.
pub(super) struct Skyline {
    /// Position -> vertex; the last position is grounded.
    perm: Vec<u32>,
    /// First column of each factored row.
    first: Vec<u32>,
    /// Offset of each factored row's envelope in `values` (with a trailing
    /// total).
    start: Vec<usize>,
    values: Vec<f64>,
}

/// The envelope of `graph` in a fixed reverse Cuthill-McKee order: the
/// order and every factored row's first column.
pub(super) struct Envelope {
    perm: Vec<u32>,
    first: Vec<u32>,
    /// Entries of the lower envelope with the diagonal.
    pub(super) entries: usize,
}

/// The last breadth-first level's vertex of least degree (first of equals)
/// from `seed`, marking with `stamp`.
fn farthest(
    graph: &Graph<'_>,
    seed: u32,
    mark: &mut [u32],
    stamp: u32,
    queue: &mut Vec<u32>,
) -> u32 {
    queue.clear();
    queue.push(seed);
    mark[seed as usize] = stamp;
    let mut level = 0..1;
    loop {
        for at in level.clone() {
            let v = queue[at] as usize;
            for &u in graph.neighbours(v) {
                if mark[u as usize] != stamp {
                    mark[u as usize] = stamp;
                    queue.push(u);
                }
            }
        }
        if queue.len() == level.end {
            break;
        }
        level = level.end..queue.len();
    }
    let last = &queue[level];
    let mut best = last[0];
    for &v in last {
        if graph.neighbours(v as usize).len() < graph.neighbours(best as usize).len() {
            best = v;
        }
    }
    best
}

impl Envelope {
    /// Reverse Cuthill-McKee from a pseudo-peripheral vertex of each
    /// component (two sweeps from the vertex of least degree), neighbours
    /// in ascending (degree, index) order; `None` when the envelope exceeds
    /// `budget`: its entries, or the factorization's work `sum w^2` over the
    /// rows' envelope widths `w`.
    pub(super) fn new(graph: &Graph<'_>, budget: Option<(usize, u128)>) -> Result<Option<Self>> {
        let n = graph.n();
        let degree = |v: u32| graph.neighbours(v as usize).len();
        let mut by_degree = with_capacity::<u32>(n)?;
        by_degree.extend(0..n as u32);
        by_degree.sort_unstable_by_key(|&v| (degree(v), v));
        let mut mark = filled(n, u32::MAX)?;
        let mut queue = with_capacity::<u32>(n)?;
        let mut order = with_capacity::<u32>(n)?;
        let mut placed = filled(n, false)?;
        let mut scratch = with_capacity::<u32>(n)?;
        let mut stamp = 0;
        for &seed in &by_degree {
            if placed[seed as usize] {
                continue;
            }
            let end = farthest(graph, seed, &mut mark, stamp, &mut queue);
            let start = farthest(graph, end, &mut mark, stamp + 1, &mut queue);
            stamp += 2;
            let mut head = order.len();
            order.push(start);
            placed[start as usize] = true;
            while head < order.len() {
                let v = order[head] as usize;
                head += 1;
                scratch.clear();
                scratch.extend(
                    graph
                        .neighbours(v)
                        .iter()
                        .copied()
                        .filter(|&u| !placed[u as usize]),
                );
                scratch.sort_unstable_by_key(|&u| (degree(u), u));
                for &u in &scratch {
                    placed[u as usize] = true;
                    order.push(u);
                }
            }
        }
        order.reverse();
        // `mark` becomes each vertex's position.
        for (p, &v) in order.iter().enumerate() {
            mark[v as usize] = p as u32;
        }
        let m = n - 1;
        let mut first = with_capacity::<u32>(m)?;
        let mut entries = 0_usize;
        let mut work = 0_u128;
        for (p, &v) in order[..m].iter().enumerate() {
            let lowest = graph
                .neighbours(v as usize)
                .iter()
                .map(|&u| mark[u as usize])
                .filter(|&q| (q as usize) < p)
                .min()
                .unwrap_or(p as u32);
            first.push(lowest);
            let width = p - lowest as usize + 1;
            entries += width;
            work += (width as u128) * (width as u128);
            if budget.is_some_and(|(most, most_work)| entries > most || work > most_work) {
                return Ok(None);
            }
        }
        Ok(Some(Self {
            perm: order,
            first,
            entries,
        }))
    }

    /// Bytes of the factor of this envelope.
    pub(super) fn bytes(&self) -> u128 {
        8 * self.entries as u128 + 16 * self.perm.len() as u128
    }
}

impl Skyline {
    /// Factor `graph` on `envelope`; `None` when a pivot reaches the
    /// rounding floor of its diagonal.
    pub(super) fn new(
        graph: &Graph<'_>,
        envelope: Envelope,
        cancel: &Cancel<'_>,
    ) -> Result<Option<Self>> {
        let Envelope {
            perm,
            first,
            entries,
        } = envelope;
        let m = first.len();
        let mut position = filled(m + 1, 0_u32)?;
        for (p, &v) in perm.iter().enumerate() {
            position[v as usize] = p as u32;
        }
        let mut start = with_capacity::<usize>(m + 1)?;
        start.push(0);
        for (p, &f) in first.iter().enumerate() {
            start.push(start[p] + p - f as usize + 1);
        }
        let mut values = filled(entries, 0.0)?;
        let mut diagonal = filled(m, 0.0)?;
        for p in 0..m {
            let v = perm[p] as usize;
            let (fp, sp) = (first[p] as usize, start[p]);
            let mut degree = 0.0;
            for (&u, &w) in graph.neighbours(v).iter().zip(graph.weights(v)) {
                degree += w;
                let q = position[u as usize] as usize;
                if q < p {
                    values[sp + q - fp] = -w;
                }
            }
            values[sp + p - fp] = degree;
            diagonal[p] = degree;
        }
        let floor = (4 * m).max(16) as f64 * f64::EPSILON;
        for p in 0..m {
            if p % 256 == 0 {
                cancel.check()?;
            }
            let (fp, sp) = (first[p] as usize, start[p]);
            let (done, row) = values.split_at_mut(sp);
            let row = &mut row[..p - fp + 1];
            for q in fp..p {
                let (fq, sq) = (first[q] as usize, start[q]);
                let k0 = fp.max(fq);
                let earlier = &done[sq..sq + q - fq + 1];
                let mut s = row[q - fp];
                for (x, y) in row[k0 - fp..q - fp].iter().zip(&earlier[k0 - fq..q - fq]) {
                    s -= x * y;
                }
                row[q - fp] = s / earlier[q - fq];
            }
            let mut d = row[p - fp];
            for x in &row[..p - fp] {
                d -= x * x;
            }
            if !(d.is_finite() && d > floor * diagonal[p]) {
                return Ok(None);
            }
            row[p - fp] = d.sqrt();
        }
        Ok(Some(Self {
            perm,
            first,
            start,
            values,
        }))
    }

    pub(super) fn bytes(&self) -> u128 {
        8 * (self.values.capacity() + self.start.capacity()) as u128
            + 4 * (self.perm.capacity() + self.first.capacity()) as u128
    }

    /// `t` (`n` levels, `R` interleaved columns) becomes the grounded
    /// solution, zero at the grounded vertex; `z` holds `(n - 1) R`.
    pub(super) fn solve<const R: usize>(&self, t: &mut [f64], z: &mut [f64]) {
        let m = self.first.len();
        let z = &mut z[..m * R];
        for (p, &v) in self.perm[..m].iter().enumerate() {
            let v = v as usize;
            z[p * R..(p + 1) * R].copy_from_slice(&t[v * R..(v + 1) * R]);
        }
        for p in 0..m {
            let (fp, sp) = (self.first[p] as usize, self.start[p]);
            let row = &self.values[sp..sp + p - fp + 1];
            let (done, rest) = z.split_at_mut(p * R);
            let zp = &mut rest[..R];
            for (k, &l) in (fp..p).zip(row) {
                for j in 0..R {
                    zp[j] -= l * done[k * R + j];
                }
            }
            for value in zp.iter_mut() {
                *value /= row[p - fp];
            }
        }
        for p in (0..m).rev() {
            let (fp, sp) = (self.first[p] as usize, self.start[p]);
            let row = &self.values[sp..sp + p - fp + 1];
            let (head, rest) = z.split_at_mut(p * R);
            let zp = &mut rest[..R];
            for value in zp.iter_mut() {
                *value /= row[p - fp];
            }
            for (k, &l) in (fp..p).zip(row) {
                for j in 0..R {
                    head[k * R + j] -= l * zp[j];
                }
            }
        }
        for (p, &v) in self.perm[..m].iter().enumerate() {
            let v = v as usize;
            t[v * R..(v + 1) * R].copy_from_slice(&z[p * R..(p + 1) * R]);
        }
        let ground = self.perm[m] as usize;
        t[ground * R..(ground + 1) * R].fill(0.0);
    }
}
