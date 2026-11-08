//! Observation passes of the full formulation: the design operator `D`
//! (coefficients to observations), its adjoint `D'` (observations to levels)
//! and the right-hand-side and residual passes, for `R` lockstep columns
//! stored flat (`R` values per row or level). Every reduction runs over
//! blocks fixed by the design and is combined in block order, so results do
//! not depend on the worker count.

use super::structure::Structure;
use crate::design::Effect;
use rayon::prelude::*;

/// Rows per block of the observation passes.
pub(super) const ROW_BLOCK: usize = 4096;
/// Levels per task of a gathered adjoint.
const LEVEL_BLOCK: usize = 256;
/// Below this many scalar updates a pass stays on the calling thread.
pub(super) const PARALLEL_WORK: usize = 1 << 15;

/// `R` values per row of an observation-major array: row `i` starts at
/// `data[i * stride + first]` (`stride` 0 repeats one row).
#[derive(Clone, Copy)]
pub(super) struct Rows<'a> {
    pub(super) data: &'a [f64],
    pub(super) stride: usize,
    pub(super) first: usize,
}

impl Rows<'_> {
    #[inline(always)]
    fn get<const R: usize>(&self, row: usize) -> [f64; R] {
        let start = row * self.stride + self.first;
        let mut out = [0.0; R];
        out.copy_from_slice(&self.data[start..start + R]);
        out
    }
}

/// `body(block, chunk)` over consecutive chunks of `data`, in parallel when
/// the pass does at least [`PARALLEL_WORK`] updates.
fn each_block<F>(data: &mut [f64], block: usize, work: usize, body: &F)
where
    F: Fn(usize, &mut [f64]) + Sync,
{
    if work < PARALLEL_WORK {
        data.chunks_mut(block)
            .enumerate()
            .for_each(|(b, chunk)| body(b, chunk));
    } else {
        data.par_chunks_mut(block)
            .enumerate()
            .for_each(|(b, chunk)| body(b, chunk));
    }
}

/// [`each_block`] over two arrays with matching block counts.
fn each_block2<F>(a: (&mut [f64], usize), b: (&mut [f64], usize), work: usize, body: &F)
where
    F: Fn(usize, &mut [f64], &mut [f64]) + Sync,
{
    let ((a, a_block), (b, b_block)) = (a, b);
    if work < PARALLEL_WORK {
        a.chunks_mut(a_block)
            .zip(b.chunks_mut(b_block))
            .enumerate()
            .for_each(|(k, (a, b))| body(k, a, b));
    } else {
        a.par_chunks_mut(a_block)
            .zip(b.par_chunks_mut(b_block))
            .enumerate()
            .for_each(|(k, (a, b))| body(k, a, b));
    }
}

/// `(D x)_row`: the row's coefficient of every effect, summed in effect
/// order.
#[inline(always)]
fn fitted<const R: usize>(
    st: &Structure,
    effects: &[Effect<'_>],
    x: &[f64],
    row: usize,
) -> [f64; R] {
    let mut f = [0.0; R];
    for (effect, &offset) in effects.iter().zip(&st.offsets) {
        let g = (offset + effect.codes()[row] as usize) * R;
        for j in 0..R {
            f[j] += x[g + j];
        }
    }
    f
}

/// `out = scale * D x` (`scale` per row; `None` is one).
pub(super) fn forward<const R: usize>(
    st: &Structure,
    effects: &[Effect<'_>],
    x: &[f64],
    scale: Option<&[f64]>,
    out: &mut [f64],
) {
    let body = |block: usize, out: &mut [f64]| {
        let first = block * ROW_BLOCK;
        for (k, o) in out.chunks_exact_mut(R).enumerate() {
            let row = first + k;
            let f = fitted::<R>(st, effects, x, row);
            let s = scale.map_or(1.0, |s| s[row]);
            for j in 0..R {
                o[j] = s * f[j];
            }
        }
    };
    each_block(out, ROW_BLOCK * R, st.nobs * R * effects.len(), &body);
}

/// `out = D' (scale * src)`: each level's sum over its rows. `scatter`
/// holds one accumulator per scatter block.
pub(super) fn adjoint<const R: usize>(
    st: &Structure,
    effects: &[Effect<'_>],
    src: Rows<'_>,
    scale: Option<&[f64]>,
    out: &mut [f64],
    scatter: &mut [f64],
) {
    if !st.scattered.is_empty() {
        scatter_pass::<R>(st, effects, src, scale, out, scatter);
    }
    for index in &st.gathered {
        let offset = st.offsets[index.effect];
        let levels = effects[index.effect].levels();
        let body = |block: usize, out: &mut [f64]| {
            let first = block * LEVEL_BLOCK;
            for (k, o) in out.chunks_exact_mut(R).enumerate() {
                let level = first + k;
                let mut sum = [0.0; R];
                for &row in &index.rows[index.starts[level]..index.starts[level + 1]] {
                    let row = row as usize;
                    let v = src.get::<R>(row);
                    let w = scale.map_or(1.0, |s| s[row]);
                    for j in 0..R {
                        sum[j] += w * v[j];
                    }
                }
                o.copy_from_slice(&sum);
            }
        };
        let out = &mut out[offset * R..(offset + levels) * R];
        each_block(out, LEVEL_BLOCK * R, st.nobs * R, &body);
    }
}

/// The scattered effects' part of [`adjoint`]: each fixed row block sums
/// into its own accumulator; the accumulators are added in block order.
fn scatter_pass<const R: usize>(
    st: &Structure,
    effects: &[Effect<'_>],
    src: Rows<'_>,
    scale: Option<&[f64]>,
    out: &mut [f64],
    scatter: &mut [f64],
) {
    let width = st.scatter_levels() * R;
    let blocks = st.scatter_blocks();
    let rows = st.scatter_rows;
    let body = |block: usize, acc: &mut [f64]| {
        acc.fill(0.0);
        let start = block * rows;
        for row in start..(start + rows).min(st.nobs) {
            let mut v = src.get::<R>(row);
            let w = scale.map_or(1.0, |s| s[row]);
            for value in &mut v {
                *value *= w;
            }
            for (&e, &base) in st.scattered.iter().zip(&st.scatter_offsets) {
                let a = (base + effects[e].codes()[row] as usize) * R;
                for j in 0..R {
                    acc[a + j] += v[j];
                }
            }
        }
    };
    let scatter = &mut scatter[..blocks * width];
    each_block(scatter, width, st.nobs * R * st.scattered.len(), &body);
    let (total, rest) = scatter.split_at_mut(width);
    for part in rest.chunks_exact(width) {
        for (a, b) in total.iter_mut().zip(part) {
            *a += b;
        }
    }
    for (&e, &base) in st.scattered.iter().zip(&st.scatter_offsets) {
        let (offset, levels) = (st.offsets[e], effects[e].levels());
        out[offset * R..(offset + levels) * R]
            .copy_from_slice(&total[base * R..(base + levels) * R]);
    }
}

/// `out = scale * (y - D x)` (`scale * y` without `x`) for the columns
/// `src`; returns `[sum scale y^2, max |y|, sum scale (y - D x)^2]` per
/// column, from per-block partials combined in block order.
pub(super) fn load<const R: usize>(
    st: &Structure,
    effects: &[Effect<'_>],
    src: Rows<'_>,
    x: Option<&[f64]>,
    scale: Option<&[f64]>,
    out: &mut [f64],
    partials: &mut [f64],
) -> [[f64; R]; 3] {
    let body = |block: usize, out: &mut [f64], part: &mut [f64]| {
        let first = block * ROW_BLOCK;
        let mut stats = [[0.0; R], [0.0_f64; R], [0.0; R]];
        for (k, o) in out.chunks_exact_mut(R).enumerate() {
            let row = first + k;
            let y = src.get::<R>(row);
            let w = scale.map_or(1.0, |s| s[row]);
            let f = x.map_or([0.0; R], |x| fitted::<R>(st, effects, x, row));
            for j in 0..R {
                let d = y[j] - f[j];
                stats[0][j] += w * y[j] * y[j];
                stats[1][j] = stats[1][j].max(y[j].abs());
                o[j] = w * d;
                stats[2][j] += o[j] * d;
            }
        }
        for (k, stat) in stats.iter().enumerate() {
            part[k * R..(k + 1) * R].copy_from_slice(stat);
        }
    };
    let blocks = st.nobs.div_ceil(ROW_BLOCK);
    let partials = &mut partials[..blocks * 3 * R];
    let work = st.nobs * R * (1 + x.map_or(0, |_| effects.len()));
    each_block2((out, ROW_BLOCK * R), (partials, 3 * R), work, &body);
    let mut stats = [[0.0; R], [0.0_f64; R], [0.0; R]];
    for part in partials.chunks_exact(3 * R) {
        for j in 0..R {
            stats[0][j] += part[j];
            stats[1][j] = stats[1][j].max(part[R + j]);
            stats[2][j] += part[2 * R + j];
        }
    }
    stats
}

/// Replace the columns `first..first + R` of `values` (observation-major,
/// `stride` columns) by `y - D x`, write `scale * r` into `out` and return
/// `sum scale r^2` per column (per-block partials combined in block order).
#[allow(clippy::too_many_arguments)]
pub(super) fn residualize<const R: usize>(
    st: &Structure,
    effects: &[Effect<'_>],
    x: &[f64],
    (values, stride, first): (&mut [f64], usize, usize),
    scale: Option<&[f64]>,
    out: &mut [f64],
    partials: &mut [f64],
) -> [f64; R] {
    let body = |block: usize, values: &mut [f64], out: &mut [f64], part: &mut [f64]| {
        let start = block * ROW_BLOCK;
        let mut sq = [0.0; R];
        for (k, (row_values, o)) in values
            .chunks_exact_mut(stride)
            .zip(out.chunks_exact_mut(R))
            .enumerate()
        {
            let row = start + k;
            let f = fitted::<R>(st, effects, x, row);
            let w = scale.map_or(1.0, |s| s[row]);
            let v = &mut row_values[first..first + R];
            for j in 0..R {
                let r = v[j] - f[j];
                v[j] = r;
                o[j] = w * r;
                sq[j] += o[j] * r;
            }
        }
        part[..R].copy_from_slice(&sq);
    };
    let blocks = st.nobs.div_ceil(ROW_BLOCK);
    let partials = &mut partials[..blocks * R];
    if st.nobs * R * effects.len() < PARALLEL_WORK {
        for (k, ((values, out), part)) in values
            .chunks_mut(ROW_BLOCK * stride)
            .zip(out.chunks_mut(ROW_BLOCK * R))
            .zip(partials.chunks_mut(R))
            .enumerate()
        {
            body(k, values, out, part);
        }
    } else {
        values
            .par_chunks_mut(ROW_BLOCK * stride)
            .zip(out.par_chunks_mut(ROW_BLOCK * R))
            .zip(partials.par_chunks_mut(R))
            .enumerate()
            .for_each(|(k, ((values, out), part))| body(k, values, out, part));
    }
    let mut sq = [0.0; R];
    for part in partials.chunks_exact(R) {
        for j in 0..R {
            sq[j] += part[j];
        }
    }
    sq
}

/// The raw-weighted Gram `sum_i w_i r_i r_i'` of all `ncols` columns of
/// `values` (at most eight), row-major; per-block lower triangles combined
/// in block order.
pub(super) fn gram(
    values: &[f64],
    ncols: usize,
    raw: Option<&[f64]>,
    partials: &mut [f64],
    gram: &mut [f64],
) {
    let width = ncols * ncols;
    let nobs = values.len() / ncols.max(1);
    let body = |block: usize, part: &mut [f64]| {
        part.fill(0.0);
        let start = block * ROW_BLOCK;
        for row in start..(start + ROW_BLOCK).min(nobs) {
            let v = &values[row * ncols..(row + 1) * ncols];
            let w = raw.map_or(1.0, |raw| raw[row]);
            for a in 0..ncols {
                let wa = w * v[a];
                for b in 0..=a {
                    part[a * ncols + b] += wa * v[b];
                }
            }
        }
    };
    let blocks = nobs.div_ceil(ROW_BLOCK);
    let partials = &mut partials[..blocks * width];
    each_block(partials, width, nobs * width, &body);
    gram.fill(0.0);
    for part in partials.chunks_exact(width) {
        for (total, value) in gram.iter_mut().zip(part) {
            *total += value;
        }
    }
    for a in 0..ncols {
        for b in 0..a {
            gram[b * ncols + a] = gram[a * ncols + b];
        }
    }
}
