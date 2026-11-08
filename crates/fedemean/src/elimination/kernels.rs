//! The reduced operator and its reduced-space vector kernels.

use super::structure::{filled, with_capacity, Structure};
use super::weights::Weights;
use super::{CHUNK, PARALLEL_WORK};
use crate::design::Members;
use crate::error::Result;
use crate::weights::Validated as ValidatedWeights;
use rayon::prelude::*;

pub(super) struct Ctx<'d, 'w, const NB: usize> {
    pub(super) st: &'d Structure,
    pub(super) w: &'d Weights,
    pub(super) index: Members<'d>,
    pub(super) elim_codes: &'d [u32],
    pub(super) codes_b: [&'d [u32]; NB],
    pub(super) offs: [u32; NB],
    pub(super) weights: ValidatedWeights<'w>,
    pub(super) coefficient_accuracy: bool,
}

#[inline(always)]
pub(super) fn gather<const R: usize>(cc: &[u32], p: &[[f64; R]]) -> [f64; R] {
    let mut g = p[cc[0] as usize];
    for &c in &cc[1..] {
        let q = &p[c as usize];
        for j in 0..R {
            g[j] += q[j];
        }
    }
    g
}

#[inline(always)]
pub(super) fn scatter<const R: usize>(cc: &[u32], z: &[f64; R], acc: &mut [[f64; R]]) {
    for &c in cc {
        let a = &mut acc[c as usize];
        for j in 0..R {
            a[j] += z[j];
        }
    }
}

/// One lane of `acc = S p` restricted to its active segments (before the
/// merge).
pub(super) fn apply_lane<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    p: &[[f64; R]],
    segments: &[u32],
    acc: &mut [[f64; R]],
    u: &mut [[f64; R]],
) {
    acc.fill([0.0; R]);
    if NB == 1 {
        // One retained block: every cell has one code, so
        // `S p = M_a p - sum_c w_c t_s(c) e_j(c)` with `t_s` the segment's
        // weighted mean of `p` and `M_a` the diagonal active mass; this lane
        // accumulates the second term ([`apply`] forms the difference).
        for &s in segments {
            let s = s as usize;
            let (c0, c1) = (st.seg_cells[s], st.seg_cells[s + 1]);
            let codes = &st.codes[c0..c1];
            let mut t = [0.0; R];
            if UNIT {
                for &c in codes {
                    let g = &p[c as usize];
                    for j in 0..R {
                        t[j] += g[j];
                    }
                }
            } else {
                for (&c, &wc) in codes.iter().zip(&w.cell[c0..c1]) {
                    let g = &p[c as usize];
                    for j in 0..R {
                        t[j] += wc * g[j];
                    }
                }
            }
            let inv = w.seg_inverse[s];
            for v in &mut t {
                *v *= inv;
            }
            if UNIT {
                for &c in codes {
                    let a = &mut acc[c as usize];
                    for j in 0..R {
                        a[j] += t[j];
                    }
                }
            } else {
                for (&c, &wc) in codes.iter().zip(&w.cell[c0..c1]) {
                    let a = &mut acc[c as usize];
                    for j in 0..R {
                        a[j] += wc * t[j];
                    }
                }
            }
        }
        return;
    }
    for &s in segments {
        let s = s as usize;
        let (c0, c1) = (st.seg_cells[s], st.seg_cells[s + 1]);
        let codes = &st.codes[c0 * NB..c1 * NB];
        let u = &mut u[..c1 - c0];
        let mut t = [0.0; R];
        if UNIT {
            for (cc, uc) in codes.chunks_exact(NB).zip(u.iter_mut()) {
                let g = gather::<R>(cc, p);
                for j in 0..R {
                    t[j] += g[j];
                }
                *uc = g;
            }
        } else {
            let cw = &w.cell[c0..c1];
            for ((cc, uc), &wc) in codes.chunks_exact(NB).zip(u.iter_mut()).zip(cw) {
                let g = gather::<R>(cc, p);
                for j in 0..R {
                    t[j] += wc * g[j];
                }
                *uc = g;
            }
        }
        let inv = w.seg_inverse[s];
        for v in &mut t {
            *v *= inv;
        }
        if UNIT {
            for (cc, uc) in codes.chunks_exact(NB).zip(u.iter()) {
                let mut z = *uc;
                for j in 0..R {
                    z[j] -= t[j];
                }
                scatter::<R>(cc, &z, acc);
            }
        } else {
            let cw = &w.cell[c0..c1];
            for ((cc, uc), &wc) in codes.chunks_exact(NB).zip(u.iter()).zip(cw) {
                let mut z = *uc;
                for j in 0..R {
                    z[j] = wc * (z[j] - t[j]);
                }
                scatter::<R>(cc, &z, acc);
            }
        }
    }
}

/// One lane's accumulator, cell scratch and pass results.
pub(super) struct LaneWork<const R: usize> {
    pub(super) acc: Vec<[f64; R]>,
    pub(super) u: Vec<[f64; R]>,
    /// RHS pass: `Σ w v^2`, `max |v|`, centered `Σ w v^2`; residual pass:
    /// `Σ w r^2` in the first row.
    pub(super) stats: [[f64; R]; 3],
    /// Lower triangle of the residual Gram.
    pub(super) lower: [[f64; R]; R],
}

/// `lanes[0].acc += lanes[1].acc + ... + lanes[L-1].acc`, in lane order.
pub(super) fn merge<const R: usize>(lanes: &mut [LaneWork<R>]) {
    let Some((first, rest)) = lanes.split_first_mut() else {
        return;
    };
    if rest.is_empty() {
        return;
    }
    let rest: &[LaneWork<R>] = rest;
    let add = |base: usize, chunk: &mut [[f64; R]]| {
        for other in rest {
            let len = chunk.len();
            for (v, o) in chunk.iter_mut().zip(&other.acc[base..base + len]) {
                for j in 0..R {
                    v[j] += o[j];
                }
            }
        }
    };
    if first.acc.len() * R * rest.len() < PARALLEL_WORK {
        add(0, &mut first.acc);
    } else {
        first
            .acc
            .par_chunks_mut(CHUNK)
            .enumerate()
            .for_each(|(i, chunk)| add(i * CHUNK, chunk));
    }
}

/// The solve workspace of one batch width; every buffer is overwritten
/// before it is read, so a retained workspace serves later solves as is.
pub(super) struct Work<const R: usize> {
    pub(super) x: Vec<[f64; R]>,
    /// Per column, the best iterate that met the acceptance rule.
    pub(super) xs: Vec<[f64; R]>,
    pub(super) r: Vec<[f64; R]>,
    pub(super) z: Vec<[f64; R]>,
    pub(super) p: Vec<[f64; R]>,
    pub(super) lanes: Vec<LaneWork<R>>,
    pub(super) seg: Vec<[f64; R]>,
    /// Partial sums of the reduced-space reductions, one per `CHUNK`.
    pub(super) partials: Vec<[[f64; R]; 3]>,
}

impl<const R: usize> Work<R> {
    pub(super) fn lane_count(st: &Structure) -> usize {
        st.lanes.len().max(st.rows.len()).max(st.active_lanes.len())
    }

    pub(super) fn new(st: &Structure) -> Result<Self> {
        let m = st.reduced;
        let count = Self::lane_count(st);
        let mut lanes = with_capacity(count)?;
        for l in 0..count {
            let cells = st.active_lanes.get(l).map_or(0, |&(_, cells)| cells);
            lanes.push(LaneWork {
                acc: filled(m, [0.0; R])?,
                u: filled(cells, [0.0; R])?,
                stats: [[0.0; R]; 3],
                lower: [[0.0; R]; R],
            });
        }
        Ok(Self {
            x: filled(m, [0.0; R])?,
            xs: filled(m, [0.0; R])?,
            r: filled(m, [0.0; R])?,
            z: filled(m, [0.0; R])?,
            p: filled(m, [0.0; R])?,
            lanes,
            seg: filled(st.segments, [0.0; R])?,
            partials: filled(m.div_ceil(CHUNK).max(1), [[0.0; R]; 3])?,
        })
    }

    pub(super) fn fits(&self, st: &Structure) -> bool {
        self.x.len() == st.reduced
            && self.seg.len() == st.segments
            && self.lanes.len() == Self::lane_count(st)
    }
}

/// One retained workspace per batch width.
#[derive(Default)]
pub(super) struct WorkSlots {
    pub(super) w1: Option<Work<1>>,
    pub(super) w2: Option<Work<2>>,
    pub(super) w3: Option<Work<3>>,
    pub(super) w4: Option<Work<4>>,
    pub(super) w5: Option<Work<5>>,
    pub(super) w6: Option<Work<6>>,
    pub(super) w7: Option<Work<7>>,
    pub(super) w8: Option<Work<8>>,
}

pub(super) trait Slotted: Sized {
    fn slot(slots: &mut WorkSlots) -> &mut Option<Self>;
}

macro_rules! slotted {
    ($($r:literal => $field:ident),*) => {
        $(impl Slotted for Work<$r> {
            fn slot(slots: &mut WorkSlots) -> &mut Option<Self> {
                &mut slots.$field
            }
        })*
    };
}
slotted!(1 => w1, 2 => w2, 3 => w3, 4 => w4, 5 => w5, 6 => w6, 7 => w7, 8 => w8);

pub(super) fn apply<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    p: &[[f64; R]],
    lanes: &mut [LaneWork<R>],
) {
    let count = st.active_lanes.len();
    if count == 1 {
        let segments = &st.active[st.active_lanes[0].0.clone()];
        let lane = &mut lanes[0];
        apply_lane::<R, NB, UNIT>(st, w, p, segments, &mut lane.acc, &mut lane.u);
    } else {
        lanes[..count]
            .par_iter_mut()
            .zip(st.active_lanes.par_iter())
            .for_each(|(lane, (range, _))| {
                let segments = &st.active[range.clone()];
                apply_lane::<R, NB, UNIT>(st, w, p, segments, &mut lane.acc, &mut lane.u);
            });
        merge(&mut lanes[..count]);
    }
    if NB == 1 {
        for ((q, p), &mass) in lanes[0].acc.iter_mut().zip(p).zip(&w.active_mass) {
            for j in 0..R {
                q[j] = mass * p[j] - q[j];
            }
        }
    }
}

/// Per-RHS `a . b`, as plain sums over fixed chunks combined in chunk order.
pub(super) fn dot<const R: usize>(
    a: &[[f64; R]],
    b: &[[f64; R]],
    partials: &mut [[[f64; R]; 3]],
) -> [f64; R] {
    let body = |a: &[[f64; R]], b: &[[f64; R]]| {
        let mut s = [0.0; R];
        for (x, y) in a.iter().zip(b) {
            for j in 0..R {
                s[j] += x[j] * y[j];
            }
        }
        s
    };
    let m = a.len();
    let mut total = [0.0; R];
    if m * R < PARALLEL_WORK {
        let s = body(a, b);
        for j in 0..R {
            total[j] += s[j];
        }
    } else {
        a.par_chunks(CHUNK)
            .zip(b.par_chunks(CHUNK))
            .zip(partials.par_iter_mut())
            .for_each(|((a, b), slot)| slot[0] = body(a, b));
        for slot in &partials[..m.div_ceil(CHUNK)] {
            for (t, s) in total.iter_mut().zip(&slot[0]) {
                *t += s;
            }
        }
    }
    total
}

/// `to[.][c] = from[.][c]` for the columns of `mask`.
pub(super) fn copy_columns<const R: usize>(
    to: &mut [[f64; R]],
    from: &[[f64; R]],
    mask: [bool; R],
) {
    let body = |to: &mut [[f64; R]], from: &[[f64; R]]| {
        for (t, f) in to.iter_mut().zip(from) {
            for c in 0..R {
                if mask[c] {
                    t[c] = f[c];
                }
            }
        }
    };
    if to.len() * R < PARALLEL_WORK {
        body(to, from);
    } else {
        to.par_chunks_mut(CHUNK)
            .zip(from.par_chunks(CHUNK))
            .for_each(|(to, from)| body(to, from));
    }
}
