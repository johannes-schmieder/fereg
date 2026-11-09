//! Each level's channel block of `D'WD` for the effects with slopes (or
//! without an intercept): those effects' part of the full formulation's
//! Jacobi preconditioner, of its stopping measures and of its certificate.
//!
//! **Blocks.** Level `l` of such an effect owns the `C x C` block
//! `G_l = sum_{i in l} w_i v_i v_i'`, `v_i` the row's channel values (1 for
//! the intercept, then the slope values). With `S = diag(G_l)^-1/2` (a zero
//! diagonal, a slope that is zero on every row of the level, takes the
//! level's largest diagonal instead), `S G_l S = Q Lambda Q'` by cyclic
//! Jacobi rotations, and the block's action is
//! `M_l^-1 = S Q Phi Q' S` with `phi_k = 1 / lambda_k` when
//! `lambda_k > NULL * lambda_max`, else `1 / lambda_max`.
//!
//! **Singular blocks.** A level whose channels are linearly dependent on
//! its rows (a slope constant on the level next to the intercept, a slope
//! that is zero there, fewer distinct rows than channels) has a singular
//! `G_l`; its null directions are null directions of `D` itself. On the
//! range of `G_l` the action is the exact inverse (`M_l^-1 = G_l^+`); the
//! null directions get the level's largest scaled curvature, which keeps
//! `M_l^-1` symmetric positive definite, as modified LSMR requires. Every
//! consistent residual `D'W t` restricted to the level lies in the range of
//! `G_l`, so the null part of the action never acts on it (up to rounding):
//! the recurrences, their iterates and their fitted values are those of the
//! exact pseudo-inverse. An all-zero block (a slope-only effect whose slope
//! is zero on the level) acts as the identity. The threshold `NULL` is
//! relative to the scaled block, so the rule does not depend on the slope's
//! units; it only shapes the preconditioner and the certificate's weighting
//! of numerically null directions, never the projection.
//!
//! **Measure and certificate.** A level's moments `m_l = (D'W r)_l` are
//! measured by `sqrt(m_l' M_l^-1 m_l)`: the norm of the residual's
//! projection onto the span of the level's channels when `G_l` is
//! nonsingular (for one intercept channel, `|m| / sqrt(mass)`), which bounds
//! every channel's own `|m_c| / sqrt((G_l)_cc)`, and does not depend on how
//! the channels are parametrized (centering a slope changes nothing).
//!
//! **Determinism.** Grams are summed per level over its rows in ascending
//! order (gathered effects) or by fixed row blocks merged in block order
//! (scattered effects); the inverses and the measure passes run over fixed
//! level tasks combined in task order.

use super::kernels::{each_block, PARALLEL_WORK};
use super::refill;
use super::structure::Structure;
use crate::certificate::MomentFold;
use crate::design::Effect;
use crate::error::{Error, Result};
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The most channels of one effect (the block solves use fixed stack
/// storage).
pub(crate) const MAX_CHANNELS: usize = 16;
/// Scaled eigenvalues at most this fraction of the block's largest are
/// null: above the rounding of a summed Gram, below any direction whose
/// exact inversion is numerically meaningful.
const NULL: f64 = 1e-10;
/// Levels per task of the inversion and measure passes.
const LEVEL_TASK: usize = 512;
/// Rotation sweeps of the eigendecomposition (it converges in a few).
const SWEEPS: usize = 32;

/// One effect with slopes: where its levels' blocks are.
#[derive(Clone, Copy)]
struct Part {
    effect: usize,
    channels: usize,
    levels: usize,
    /// The first value of its blocks (`channels^2` per level).
    first: usize,
    /// Its first measure task.
    task: usize,
}

/// The block inverses of the current weights (empty for an intercept-only
/// design).
#[derive(Default)]
pub(crate) struct Blocks {
    parts: Vec<Part>,
    /// Row-major `C x C` inverses, level by level.
    values: Vec<f64>,
    /// Scatter accumulators of a Gram.
    scratch: Vec<f64>,
    tasks: usize,
    /// Levels whose block has a null direction.
    singular: usize,
}

/// Bytes of the blocks of every effect, the scatter accumulators of the
/// largest scattered one (see [`Blocks::fill`]) and the measure partials
/// of a full batch.
pub(super) fn block_bytes(st: &Structure, effects: &[Effect<'_>]) -> u128 {
    let (mut values, mut scratch, mut tasks) = (0_u128, 0_u128, 0_u128);
    for (e, effect) in effects.iter().enumerate().filter(|(_, e)| e.sloped()) {
        let span = (effect.levels() * effect.channels() * effect.channels()) as u128;
        values += span;
        tasks += effect.levels().div_ceil(LEVEL_TASK) as u128;
        if !st.gathered.iter().any(|g| g.effect == e) {
            scratch = scratch.max(span * st.scatter_blocks() as u128);
        }
    }
    8 * (values + scratch + 3 * crate::elimination::BATCH as u128 * tasks)
}

impl Blocks {
    pub(super) fn bytes(&self) -> u128 {
        (self.values.capacity() + self.scratch.capacity()) as u128 * 8
            + self.parts.capacity() as u128 * std::mem::size_of::<Part>() as u128
    }

    pub(crate) fn singular(&self) -> usize {
        self.singular
    }

    /// Measure tasks (partials of [`Blocks::measure`]: `3 R` each).
    pub(super) fn tasks(&self) -> usize {
        self.tasks
    }

    /// Form every block of the current weights, write the channel masses
    /// (the Gram diagonals) into `mass`, and invert the blocks.
    pub(super) fn fill(
        &mut self,
        st: &Structure,
        effects: &[Effect<'_>],
        weights: Option<&[f64]>,
        mass: &mut [f64],
    ) -> Result<()> {
        self.parts.clear();
        let (mut first, mut tasks) = (0, 0);
        for (e, effect) in effects.iter().enumerate().filter(|(_, e)| e.sloped()) {
            let (channels, levels) = (effect.channels(), effect.levels());
            super::reserve(&mut self.parts, 1)?;
            self.parts.push(Part {
                effect: e,
                channels,
                levels,
                first,
                task: tasks,
            });
            first += levels * channels * channels;
            tasks += levels.div_ceil(LEVEL_TASK);
        }
        self.tasks = tasks;
        refill(&mut self.values, first, 0.0)?;
        let singular = AtomicUsize::new(0);
        for k in 0..self.parts.len() {
            let part = self.parts[k];
            let effect = &effects[part.effect];
            let width = part.channels * part.channels;
            let span = part.first..part.first + part.levels * width;
            gram(st, (part.effect, effect), weights, span.clone(), self)?;
            let blocks = &mut self.values[span];
            if !blocks.iter().all(|v| v.is_finite()) {
                return Err(Error::InvalidData("slope values overflow their level Gram"));
            }
            let offset = st.offsets[part.effect];
            for (l, block) in blocks.chunks_exact(width).enumerate() {
                for c in 0..part.channels {
                    mass[offset + l * part.channels + c] = block[c * part.channels + c];
                }
            }
            let c = part.channels;
            let body = |_: usize, chunk: &mut [f64]| {
                let mut count = 0;
                for block in chunk.chunks_exact_mut(width) {
                    count += usize::from(invert(block, c));
                }
                singular.fetch_add(count, Ordering::Relaxed);
            };
            let work = part.levels * width * c;
            each_block(blocks, LEVEL_TASK * width, work, &body);
        }
        self.singular = singular.into_inner();
        Ok(())
    }

    /// Whether effect `e` has blocks.
    #[cfg_attr(not(feature = "schwarz"), allow(dead_code))]
    pub(super) fn covers(&self, e: usize) -> bool {
        self.parts.iter().any(|p| p.effect == e)
    }

    /// `out_l = M_l^-1 in_l` on the levels of effect `e`, whose first
    /// coefficient is `first` (its coordinates only, `R` columns); `false`
    /// when it has no blocks.
    pub(super) fn solve_effect<const R: usize>(
        &self,
        e: usize,
        first: usize,
        input: &[f64],
        out: &mut [f64],
    ) -> bool {
        let Some(part) = self.parts.iter().find(|p| p.effect == e) else {
            return false;
        };
        let (c, width) = (part.channels, part.channels * part.channels);
        let span = (first * R)..((first + part.levels * c) * R);
        let blocks = &self.values[part.first..part.first + part.levels * width];
        let input = &input[span.clone()];
        let body = |task: usize, out: &mut [f64]| {
            let mut sink = [[0.0; R]; 3];
            let start = task * LEVEL_TASK;
            for (k, out) in out.chunks_exact_mut(c * R).enumerate() {
                let (l, at) = (start + k, (start + k) * c * R);
                let block = &blocks[l * width..(l + 1) * width];
                level::<R>(block, c, &input[at..at + c * R], Some(out), &mut sink);
            }
        };
        each_block(
            &mut out[span],
            LEVEL_TASK * c * R,
            part.levels * width * R,
            &body,
        );
        true
    }

    /// Over every blocked level of `r` (internal layout, `R` columns):
    /// `v_l = M_l^-1 r_l`, written into `z` when given. Returns per column
    /// `[sum_l r_l' v_l, max_l sqrt(r_l' v_l), max |v|]` from per-task
    /// partials combined in task order (`partials`: [`Blocks::tasks`]
    /// times `3 R`).
    pub(super) fn measure<const R: usize>(
        &self,
        st: &Structure,
        r: &[f64],
        mut z: Option<&mut [f64]>,
        partials: &mut [f64],
    ) -> [[f64; R]; 3] {
        for part in &self.parts {
            let c = part.channels;
            let width = c * c;
            let span = (st.offsets[part.effect] * R)..(st.offsets[part.effect + 1] * R);
            let blocks = &self.values[part.first..part.first + part.levels * width];
            let tasks = part.levels.div_ceil(LEVEL_TASK);
            let partials = &mut partials[part.task * 3 * R..(part.task + tasks) * 3 * R];
            let body = |t: usize, r: &[f64], z: Option<&mut [f64]>, slot: &mut [f64]| {
                let mut acc = [[0.0; R]; 3];
                let blocks = &blocks[t * LEVEL_TASK * width..];
                let mut z = z;
                for (k, (r, block)) in r
                    .chunks_exact(c * R)
                    .zip(blocks.chunks_exact(width))
                    .enumerate()
                {
                    let out = z.as_deref_mut().map(|z| &mut z[k * c * R..(k + 1) * c * R]);
                    level::<R>(block, c, r, out, &mut acc);
                }
                for (k, values) in acc.iter().enumerate() {
                    slot[k * R..(k + 1) * R].copy_from_slice(values);
                }
            };
            let chunk = LEVEL_TASK * c * R;
            let r = &r[span.clone()];
            let parallel = part.levels * width * R >= PARALLEL_WORK;
            match (z.as_deref_mut(), parallel) {
                (Some(z), true) => z[span]
                    .par_chunks_mut(chunk)
                    .zip(r.par_chunks(chunk))
                    .zip(partials.par_chunks_mut(3 * R))
                    .enumerate()
                    .for_each(|(t, ((z, r), slot))| body(t, r, Some(z), slot)),
                (Some(z), false) => z[span]
                    .chunks_mut(chunk)
                    .zip(r.chunks(chunk))
                    .zip(partials.chunks_mut(3 * R))
                    .enumerate()
                    .for_each(|(t, ((z, r), slot))| body(t, r, Some(z), slot)),
                (None, true) => r
                    .par_chunks(chunk)
                    .zip(partials.par_chunks_mut(3 * R))
                    .enumerate()
                    .for_each(|(t, (r, slot))| body(t, r, None, slot)),
                (None, false) => r
                    .chunks(chunk)
                    .zip(partials.chunks_mut(3 * R))
                    .enumerate()
                    .for_each(|(t, (r, slot))| body(t, r, None, slot)),
            }
        }
        let mut out = [[0.0; R]; 3];
        for slot in partials[..self.tasks * 3 * R].chunks_exact(3 * R) {
            for j in 0..R {
                out[0][j] += slot[j];
                out[1][j] = out[1][j].max(slot[R + j]);
                out[2][j] = out[2][j].max(slot[2 * R + j]);
            }
        }
        out
    }

    /// Fold the levels of effect `e` (whose blocks these are) of the
    /// moments `m` (internal layout, `R` columns) into the certificate:
    /// `sqrt(m_l' M_l^-1 m_l)` relative to each column's scale, and the
    /// largest raw channel moment. `Err` like [`MomentFold::level`].
    pub(super) fn fold<const R: usize>(
        &self,
        st: &Structure,
        e: usize,
        m: &[f64],
        (scales, raw_scale): (&[f64], f64),
        (fold, relative): (&mut MomentFold, &mut [f64]),
    ) -> Result<()> {
        let part = self
            .parts
            .iter()
            .find(|p| p.effect == e)
            .expect("blocked effect");
        let c = part.channels;
        let width = c * c;
        let span = (st.offsets[e] * R)..(st.offsets[e + 1] * R);
        let blocks = &self.values[part.first..part.first + part.levels * width];
        for (m, block) in m[span].chunks_exact(c * R).zip(blocks.chunks_exact(width)) {
            let mut acc = [[0.0; R]; 3];
            level::<R>(block, c, m, None, &mut acc);
            let mut largest = [0.0_f64; R];
            for (k, value) in m.iter().enumerate() {
                largest[k % R] = largest[k % R].max(value.abs());
            }
            fold.span(&acc[1], &largest, scales, relative, raw_scale)?;
        }
        Ok(())
    }
}

/// One level: `v = M^-1 r` (`c` channels, `R` columns, `r` channel-major
/// by column: `r[ch * R + j]`), written to `out` when given; accumulates
/// `r'v`, `max sqrt(r'v)` and `max |v|` per column into `acc`.
#[inline]
fn level<const R: usize>(
    block: &[f64],
    c: usize,
    r: &[f64],
    mut out: Option<&mut [f64]>,
    acc: &mut [[f64; R]; 3],
) {
    let mut quad = [0.0; R];
    for a in 0..c {
        let row = &block[a * c..(a + 1) * c];
        let mut v = [0.0; R];
        for (b, &m) in row.iter().enumerate() {
            for j in 0..R {
                v[j] += m * r[b * R + j];
            }
        }
        for j in 0..R {
            quad[j] += r[a * R + j] * v[j];
            acc[2][j] = acc[2][j].max(v[j].abs());
        }
        if let Some(out) = out.as_deref_mut() {
            out[a * R..(a + 1) * R].copy_from_slice(&v);
        }
    }
    for (j, quad) in quad.iter().enumerate() {
        acc[0][j] += quad;
        acc[1][j] = acc[1][j].max(quad.max(0.0).sqrt());
    }
}

/// The blocks of one effect into `blocks.values[span]` (row-major, both
/// triangles): each level's rows in ascending order for a gathered effect,
/// fixed row blocks merged in block order for a scattered one.
fn gram(
    st: &Structure,
    (e, effect): (usize, &Effect<'_>),
    weights: Option<&[f64]>,
    span: std::ops::Range<usize>,
    blocks: &mut Blocks,
) -> Result<()> {
    let c = effect.channels();
    let width = c * c;
    let lead = usize::from(effect.intercept());
    let slopes = effect.slopes();
    let add = |row: usize, g: &mut [f64]| {
        let w = weights.map_or(1.0, |w| w[row]);
        let value = |ch: usize| {
            if ch < lead {
                1.0
            } else {
                slopes[ch - lead][row]
            }
        };
        for a in 0..c {
            let wa = w * value(a);
            for b in 0..=a {
                g[a * c + b] += wa * value(b);
            }
        }
    };
    let out = &mut blocks.values[span];
    if let Some(index) = st.gathered.iter().find(|g| g.effect == e) {
        let body = |task: usize, out: &mut [f64]| {
            for (k, g) in out.chunks_exact_mut(width).enumerate() {
                g.fill(0.0);
                let level = task * LEVEL_TASK + k;
                for &row in &index.rows[index.starts[level]..index.starts[level + 1]] {
                    add(row as usize, g);
                }
            }
        };
        each_block(out, LEVEL_TASK * width, st.nobs * width, &body);
    } else {
        let span = effect.levels() * width;
        let rows = st.scatter_rows;
        refill(&mut blocks.scratch, st.scatter_blocks() * span, 0.0)?;
        let codes = effect.codes();
        let body = |block: usize, acc: &mut [f64]| {
            let start = block * rows;
            let end = (start + rows).min(st.nobs);
            for (k, &code) in codes[start..end].iter().enumerate() {
                let code = code as usize;
                add(start + k, &mut acc[code * width..(code + 1) * width]);
            }
        };
        each_block(&mut blocks.scratch, span, st.nobs * width, &body);
        out.fill(0.0);
        for part in blocks.scratch.chunks_exact(span) {
            for (o, p) in out.iter_mut().zip(part) {
                *o += p;
            }
        }
    }
    for g in out.chunks_exact_mut(width) {
        for a in 0..c {
            for b in 0..a {
                g[b * c + a] = g[a * c + b];
            }
        }
    }
    Ok(())
}

/// Replace the `c x c` Gram `block` by its action `M^-1` (module docs);
/// `true` when it has a null direction.
fn invert(block: &mut [f64], c: usize) -> bool {
    if c == 1 {
        let g = block[0];
        block[0] = if g > 0.0 { 1.0 / g } else { 1.0 };
        return g <= 0.0;
    }
    let top = (0..c).map(|k| block[k * c + k]).fold(0.0_f64, f64::max);
    if top <= 0.0 {
        block.fill(0.0);
        for k in 0..c {
            block[k * c + k] = 1.0;
        }
        return true;
    }
    let mut s = [0.0; MAX_CHANNELS];
    for (k, s) in s[..c].iter_mut().enumerate() {
        let d = block[k * c + k];
        *s = 1.0 / if d > 0.0 { d } else { top }.sqrt();
    }
    let mut a = [0.0; MAX_CHANNELS * MAX_CHANNELS];
    for i in 0..c {
        for k in 0..c {
            a[i * c + k] = s[i] * block[i * c + k] * s[k];
        }
    }
    let mut q = [0.0; MAX_CHANNELS * MAX_CHANNELS];
    eigen(&mut a[..c * c], &mut q[..c * c], c);
    let largest = (0..c).map(|k| a[k * c + k]).fold(f64::MIN, f64::max);
    let mut phi = [0.0; MAX_CHANNELS];
    let mut singular = false;
    for (k, phi) in phi[..c].iter_mut().enumerate() {
        let lambda = a[k * c + k];
        *phi = if lambda > NULL * largest {
            1.0 / lambda
        } else {
            singular = true;
            1.0 / largest
        };
    }
    for i in 0..c {
        for k in 0..c {
            let mut sum = 0.0;
            for (m, phi) in phi[..c].iter().enumerate() {
                sum += q[i * c + m] * phi * q[k * c + m];
            }
            block[i * c + k] = s[i] * sum * s[k];
        }
    }
    singular
}

/// Cyclic Jacobi eigendecomposition of the symmetric `c x c` matrix `a`
/// (row-major): on return its diagonal holds the eigenvalues and the
/// columns of `q` the eigenvectors.
fn eigen(a: &mut [f64], q: &mut [f64], c: usize) {
    q.fill(0.0);
    for k in 0..c {
        q[k * c + k] = 1.0;
    }
    for _ in 0..SWEEPS {
        let mut rotated = false;
        for p in 0..c {
            for r in p + 1..c {
                // Off-diagonals at the rounding of their diagonals are done.
                let apr = a[p * c + r];
                if apr.abs() <= f64::EPSILON * (a[p * c + p] * a[r * c + r]).abs().sqrt() {
                    continue;
                }
                rotated = true;
                let theta = (a[r * c + r] - a[p * c + p]) / (2.0 * apr);
                let t = theta.signum() / (theta.abs() + theta.mul_add(theta, 1.0).sqrt());
                let cs = 1.0 / t.mul_add(t, 1.0).sqrt();
                let sn = t * cs;
                for k in 0..c {
                    let (kp, kr) = (a[k * c + p], a[k * c + r]);
                    a[k * c + p] = cs * kp - sn * kr;
                    a[k * c + r] = sn * kp + cs * kr;
                }
                for k in 0..c {
                    let (pk, rk) = (a[p * c + k], a[r * c + k]);
                    a[p * c + k] = cs * pk - sn * rk;
                    a[r * c + k] = sn * pk + cs * rk;
                }
                for k in 0..c {
                    let (kp, kr) = (q[k * c + p], q[k * c + r]);
                    q[k * c + p] = cs * kp - sn * kr;
                    q[k * c + r] = sn * kp + cs * kr;
                }
                // The rotation annihilates the pair exactly.
                a[p * c + r] = 0.0;
                a[r * c + p] = 0.0;
            }
        }
        if !rotated {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn multiply(a: &[f64], b: &[f64], c: usize) -> Vec<f64> {
        let mut out = vec![0.0; c * c];
        for i in 0..c {
            for k in 0..c {
                out[i * c + k] = (0..c).map(|m| a[i * c + m] * b[m * c + k]).sum();
            }
        }
        out
    }

    /// A nonsingular block is inverted exactly, whatever its scale.
    #[test]
    fn nonsingular_blocks_are_inverted() {
        let gram = [4.0, 2.0e3, 1.0, 2.0e3, 1.5e6, 30.0, 1.0, 30.0, 9.0];
        let mut block = gram;
        assert!(!invert(&mut block, 3));
        let product = multiply(&gram, &block, 3);
        for i in 0..3 {
            for k in 0..3 {
                let expected = if i == k { 1.0 } else { 0.0 };
                assert!((product[i * 3 + k] - expected).abs() < 1e-9, "{product:?}");
            }
        }
    }

    /// A constant slope beside the intercept: the action is the
    /// pseudo-inverse on the range (`G M G = G`), symmetric positive
    /// definite, and a zero slope gets a positive curvature.
    #[test]
    fn singular_blocks_act_as_the_pseudo_inverse_on_the_range() {
        // Three rows of weight 1 and slope 5 (constant).
        let gram = [3.0, 15.0, 15.0, 75.0];
        let mut block = gram;
        assert!(invert(&mut block, 2));
        let gmg = multiply(&gram, &multiply(&block, &gram, 2), 2);
        for (x, y) in gmg.iter().zip(&gram) {
            assert!((x - y).abs() < 1e-9 * y.abs().max(1.0), "{gmg:?}");
        }
        let det = block[0] * block[3] - block[1] * block[2];
        assert!(block[0] > 0.0 && det > 0.0 && (block[1] - block[2]).abs() < 1e-15);
        // Intercept with a zero slope.
        let mut zero = [2.0, 0.0, 0.0, 0.0];
        assert!(invert(&mut zero, 2));
        assert_eq!(zero[0], 0.5);
        assert!(zero[3] > 0.0 && zero[1] == 0.0);
        let mut empty = [0.0];
        assert!(invert(&mut empty, 1));
        assert_eq!(empty[0], 1.0);
    }
}
