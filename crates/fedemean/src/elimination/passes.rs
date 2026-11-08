//! Observation passes: reduced RHS, back-substitution and the residual.

use super::kernels::{gather, merge, Ctx, LaneWork, Work};
use super::structure::{each_lane, refill, Structure};
use super::weights::Weights;
use crate::error::Result;
use crate::projector::{ColumnsMut, Gram};

/// Column statistics of one batch from the RHS pass.
pub(super) struct RhsNorms<const R: usize> {
    /// `Σ w v^2`.
    pub(super) sq: [f64; R],
    /// `max |v|`.
    pub(super) big: [f64; R],
    /// `Σ w (v - m_s)^2` about the eliminated groups' weighted means `m_s`.
    pub(super) centered: [f64; R],
}

/// Reduced RHS: `g = A-part of D'Wv` minus the eliminated groups' means, the
/// segment sums `Σ w v` (kept for back-substitution) and the column norms.
pub(super) fn rhs<const R: usize, const NB: usize, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    values: &ColumnsMut<'_>,
    first: usize,
    work: &mut Work<R>,
) -> RhsNorms<R> {
    let st = ctx.st;
    let stride = values.ncols();
    let data = values.as_slice();
    let count = st.lanes.len();
    let weights = ctx.weights.values.unwrap_or(&[]);
    let scale = ctx.weights.raw_scale;
    let Work { seg, lanes, .. } = work;
    each_lane(
        0,
        &st.seg_bounds,
        1,
        seg,
        &mut lanes[..count],
        &|l, seg: &mut [[f64; R]], lane: &mut LaneWork<R>| {
            let acc = &mut lane.acc;
            acc.fill([0.0; R]);
            let mut sq = [0.0; R];
            let mut big = [0.0_f64; R];
            let mut centered = [0.0; R];
            for (s, out) in st.lanes[l].segments.clone().zip(seg.iter_mut()) {
                let members = ctx.index.level(s);
                let mut sum = [0.0; R];
                for &row in members {
                    let v = &data[row * stride + first..row * stride + first + R];
                    let w = if WEIGHTED { weights[row] / scale } else { 1.0 };
                    for j in 0..R {
                        let wv = w * v[j];
                        sum[j] += wv;
                        sq[j] += wv * v[j];
                        big[j] = big[j].max(v[j].abs());
                    }
                }
                *out = sum;
                // Every row of a one-cell segment shares its remaining codes,
                // so their deviations from the segment mean add nothing to
                // `g`.
                let single = st.seg_cells[s + 1] - st.seg_cells[s] == 1;
                let inv = ctx.w.seg_inverse[s];
                let mut mean = sum;
                for value in &mut mean {
                    *value *= inv;
                }
                for &row in members {
                    let v = &data[row * stride + first..row * stride + first + R];
                    let w = if WEIGHTED { weights[row] / scale } else { 1.0 };
                    let mut z = [0.0; R];
                    for j in 0..R {
                        let d = v[j] - mean[j];
                        z[j] = w * d;
                        centered[j] += z[j] * d;
                    }
                    if single {
                        continue;
                    }
                    for b in 0..NB {
                        let a = &mut acc[(ctx.codes_b[b][row] + ctx.offs[b]) as usize];
                        for j in 0..R {
                            a[j] += z[j];
                        }
                    }
                }
            }
            lane.stats = [sq, big, centered];
        },
    );
    merge(&mut lanes[..count]);
    let mut norms = RhsNorms {
        sq: [0.0; R],
        big: [0.0; R],
        centered: [0.0; R],
    };
    for lane in &lanes[..count] {
        let [s, b, c] = lane.stats;
        for j in 0..R {
            norms.sq[j] += s[j];
            norms.big[j] = norms.big[j].max(b[j]);
            norms.centered[j] += c[j];
        }
    }
    norms
}

/// `seg[s] = (Σ_s w v - Σ_c w_c (A x)_c) / W_s`, the eliminated coefficients.
pub(super) fn recover<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    work: &mut Work<R>,
) {
    let count = st.lanes.len();
    let Work { x, seg, lanes, .. } = work;
    let x = &*x;
    each_lane(
        0,
        &st.seg_bounds,
        1,
        seg,
        &mut lanes[..count],
        &|l, seg: &mut [[f64; R]], _: &mut LaneWork<R>| {
            for (s, out) in st.lanes[l].segments.clone().zip(seg.iter_mut()) {
                let (c0, c1) = (st.seg_cells[s], st.seg_cells[s + 1]);
                let codes = &st.codes[c0 * NB..c1 * NB];
                let mut t = [0.0; R];
                if UNIT {
                    for cc in codes.chunks_exact(NB) {
                        let g = gather::<R>(cc, x);
                        for j in 0..R {
                            t[j] += g[j];
                        }
                    }
                } else {
                    for (cc, &wc) in codes.chunks_exact(NB).zip(&w.cell[c0..c1]) {
                        let g = gather::<R>(cc, x);
                        for j in 0..R {
                            t[j] += wc * g[j];
                        }
                    }
                }
                let inv = w.seg_inverse[s];
                for j in 0..R {
                    out[j] = (out[j] - t[j]) * inv;
                }
            }
        },
    );
}

/// Form the residual in place and accumulate its remaining-level moments into
/// the first lane's accumulator; returns `Σ w r^2` per RHS. With `gram`, also
/// forms the raw-weighted residual Gram `Σ w r r'` (plain sums per row lane,
/// combined in lane order).
pub(super) fn residualize<const R: usize, const NB: usize, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    values: &mut ColumnsMut<'_>,
    first: usize,
    work: &mut Work<R>,
    gram: Option<&mut Gram>,
) -> Result<[f64; R]> {
    let st = ctx.st;
    let stride = values.ncols();
    let weights = ctx.weights.values.unwrap_or(&[]);
    let scale = ctx.weights.raw_scale;
    let count = st.rows.len();
    let with_gram = gram.is_some();
    let Work { x, seg, lanes, .. } = work;
    let (x, alpha) = (&*x, &*seg);
    each_lane(
        0,
        &st.row_bounds,
        stride,
        values.as_mut_slice(),
        &mut lanes[..count],
        &|l, data: &mut [f64], lane: &mut LaneWork<R>| {
            let acc = &mut lane.acc;
            acc.fill([0.0; R]);
            let mut sq = [0.0; R];
            let mut lower = [[0.0; R]; R];
            for (row, values) in st.rows[l].clone().zip(data.chunks_exact_mut(stride)) {
                let mut f = alpha[ctx.elim_codes[row] as usize];
                for b in 0..NB {
                    let q = &x[(ctx.codes_b[b][row] + ctx.offs[b]) as usize];
                    for j in 0..R {
                        f[j] += q[j];
                    }
                }
                let w = if WEIGHTED { weights[row] / scale } else { 1.0 };
                let v = &mut values[first..first + R];
                let mut wr = [0.0; R];
                for j in 0..R {
                    let r = v[j] - f[j];
                    v[j] = r;
                    wr[j] = w * r;
                    sq[j] += wr[j] * r;
                }
                for b in 0..NB {
                    let a = &mut acc[(ctx.codes_b[b][row] + ctx.offs[b]) as usize];
                    for j in 0..R {
                        a[j] += wr[j];
                    }
                }
                if with_gram {
                    let raw = if WEIGHTED { weights[row] } else { 1.0 };
                    for (a, lower_row) in lower.iter_mut().enumerate() {
                        let wa = raw * v[a];
                        for (entry, &vb) in lower_row[..=a].iter_mut().zip(&v[..=a]) {
                            *entry += wa * vb;
                        }
                    }
                }
            }
            lane.stats[0] = sq;
            lane.lower = lower;
        },
    );
    merge(&mut lanes[..count]);
    let mut sq = [0.0; R];
    for lane in &lanes[..count] {
        for (total, part) in sq.iter_mut().zip(&lane.stats[0]) {
            *total += part;
        }
    }
    if let Some(gram) = gram {
        let mut lower = [[0.0; R]; R];
        for lane in &lanes[..count] {
            for (total, part) in lower.iter_mut().zip(&lane.lower) {
                for (a, b) in total.iter_mut().zip(part) {
                    *a += b;
                }
            }
        }
        refill(&mut gram.values, R * R, 0.0)?;
        gram.columns = R;
        for (a, lower_row) in lower.iter().enumerate() {
            for (b, &value) in lower_row[..=a].iter().enumerate() {
                gram.values[a * R + b] = value;
                gram.values[b * R + a] = value;
            }
        }
    }
    Ok(sq)
}
