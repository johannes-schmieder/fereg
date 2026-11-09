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

/// Scaled eigenvalues at most this fraction of the block's largest are
/// null: above the rounding of a summed Gram, below any direction whose
/// exact inversion is numerically meaningful.
const NULL: f64 = 1e-10;
/// Levels per task of the inversion and measure passes.
const LEVEL_TASK: usize = 512;
/// A centered slope whose sum of squares on a level is at most this
/// fraction of its raw one (a variation of at most `1e-10` of its
/// magnitude) is constant there: its centered values are rounding.
const CONSTANT: f64 = 1e-20;
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
    /// Its slopes centered at their level means (with an intercept; else
    /// none) and their first center.
    centered: usize,
    center: usize,
}

/// The block inverses of the current weights (empty for an intercept-only
/// design).
#[derive(Default)]
pub(crate) struct Blocks {
    parts: Vec<Part>,
    /// Row-major `C x C` inverses, level by level, in each level's centered
    /// basis.
    values: Vec<f64>,
    /// Per level of an effect with an intercept, each slope's weighted mean
    /// (the centers of its basis), and whether the slope is constant there.
    centers: Vec<f64>,
    constant: Vec<bool>,
    /// Scatter accumulators of a Gram, and the level sums of the means.
    scratch: Vec<f64>,
    sums: Vec<f64>,
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
        (self.values.capacity() + self.centers.capacity() + self.scratch.capacity()) as u128 * 8
            + self.sums.capacity() as u128 * 8
            + self.parts.capacity() as u128 * std::mem::size_of::<Part>() as u128
    }

    pub(crate) fn singular(&self) -> usize {
        self.singular
    }

    /// The centers of level `l` of `part`.
    #[inline]
    fn means(&self, part: &Part, l: usize) -> &[f64] {
        let at = part.center + l * part.centered;
        &self.centers[at..at + part.centered]
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
        let (mut first, mut tasks, mut center) = (0, 0, 0);
        for (e, effect) in effects.iter().enumerate().filter(|(_, e)| e.sloped()) {
            let (channels, levels) = (effect.channels(), effect.levels());
            let centered = if effect.intercept() { channels - 1 } else { 0 };
            super::reserve(&mut self.parts, 1)?;
            self.parts.push(Part {
                effect: e,
                channels,
                levels,
                first,
                task: tasks,
                centered,
                center,
            });
            first += levels * channels * channels;
            tasks += levels.div_ceil(LEVEL_TASK);
            center += levels * centered;
        }
        self.tasks = tasks;
        refill(&mut self.values, first, 0.0)?;
        refill(&mut self.centers, center, 0.0)?;
        refill(&mut self.constant, center, false)?;
        let singular = AtomicUsize::new(0);
        for k in 0..self.parts.len() {
            let part = self.parts[k];
            let effect = &effects[part.effect];
            let width = part.channels * part.channels;
            let span = part.first..part.first + part.levels * width;
            if part.centered > 0 {
                means(st, (part.effect, effect), weights, part, self)?;
            }
            gram(
                st,
                (part.effect, effect),
                weights,
                (span.clone(), part),
                self,
            )?;
            let centers = &self.centers[part.center..part.center + part.levels * part.centered];
            let constant = &self.constant[part.center..part.center + part.levels * part.centered];
            let blocks = &mut self.values[span];
            if !blocks.iter().all(|v| v.is_finite()) {
                return Err(Error::InvalidData("slope values overflow their level Gram"));
            }
            // The raw diagonal of D'WD: a centered slope's sum of squares
            // plus its center's share of the level's weight.
            let offset = st.offsets[part.effect];
            for (l, block) in blocks.chunks_exact(width).enumerate() {
                for c in 0..part.channels {
                    let mut value = block[c * part.channels + c];
                    if c > 0 && part.centered > 0 && !constant[l * part.centered + c - 1] {
                        let mean = centers[l * part.centered + c - 1];
                        value += mean * mean * block[0];
                    }
                    mass[offset + l * part.channels + c] = value;
                }
            }
            let c = part.channels;
            let tasks = part.levels.div_ceil(LEVEL_TASK);
            let mut scratch = super::filled(tasks * invert_scratch(c), 0.0)?;
            let body = |task: usize, (chunk, scratch): (&mut [f64], &mut [f64])| {
                let mut count = 0;
                for (k, block) in chunk.chunks_exact_mut(width).enumerate() {
                    let at = (task * LEVEL_TASK + k) * part.centered;
                    let flat = constant[at..at + part.centered].contains(&true);
                    count += usize::from(invert(block, c, None, scratch) || flat);
                }
                singular.fetch_add(count, Ordering::Relaxed);
            };
            if part.levels * width * c < PARALLEL_WORK {
                blocks
                    .chunks_mut(LEVEL_TASK * width)
                    .zip(scratch.chunks_mut(invert_scratch(c)))
                    .enumerate()
                    .for_each(|(t, parts)| body(t, parts));
            } else {
                blocks
                    .par_chunks_mut(LEVEL_TASK * width)
                    .zip(scratch.par_chunks_mut(invert_scratch(c)))
                    .enumerate()
                    .for_each(|(t, parts)| body(t, parts));
            }
        }
        self.singular = singular.into_inner();
        Ok(())
    }

    /// The level inverses of effect `e` (`channels^2` per level), when it
    /// has blocks.
    pub(super) fn effect(&self, e: usize) -> Option<&[f64]> {
        let part = self.parts.iter().find(|p| p.effect == e)?;
        Some(&self.values[part.first..part.first + part.levels * part.channels.pow(2)])
    }

    /// The centers of every level of effect `e` (its slopes' weighted
    /// level means, `centered` per level; empty without an intercept),
    /// whether each slope is constant on its level (its centered values are
    /// then zero), and their count per level.
    pub(super) fn centers(&self, e: usize) -> (&[f64], &[bool], usize) {
        match self.parts.iter().find(|p| p.effect == e) {
            Some(part) => {
                let span = part.center..part.center + part.levels * part.centered;
                (
                    &self.centers[span.clone()],
                    &self.constant[span],
                    part.centered,
                )
            }
            None => (&[], &[], 0),
        }
    }

    /// Effect `e`'s coefficients `values` (`R` columns, its levels only)
    /// between the raw and the centered basis: moments to the centered
    /// basis (`m_s -= c_s m_0`), or with `raw` coefficients back to the raw
    /// one (`x_0 -= sum_s c_s x_s`). Nothing for an effect without centers.
    pub(super) fn convert<const R: usize>(&self, e: usize, values: &mut [f64], raw: bool) {
        let (centers, _, centered) = self.centers(e);
        if centered == 0 {
            return;
        }
        let c = centered + 1;
        let body = |task: usize, chunk: &mut [f64]| {
            for (k, level) in chunk.chunks_exact_mut(c * R).enumerate() {
                let means = &centers[(task * LEVEL_TASK + k) * centered..][..centered];
                shift::<R>(means, level, raw);
            }
        };
        each_block(values, LEVEL_TASK * c * R, values.len() * centered, &body);
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
                let means = self.means(part, l);
                centered::<R>(
                    (block, c, means),
                    &input[at..at + c * R],
                    Some(out),
                    &mut sink,
                );
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
                    let means = self.means(part, t * LEVEL_TASK + k);
                    centered::<R>((block, c, means), r, out, &mut acc);
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
        for (l, (m, block)) in m[span]
            .chunks_exact(c * R)
            .zip(blocks.chunks_exact(width))
            .enumerate()
        {
            let mut acc = [[0.0; R]; 3];
            centered::<R>((block, c, self.means(part, l)), m, None, &mut acc);
            let mut largest = [0.0_f64; R];
            for (k, value) in m.iter().enumerate() {
                largest[k % R] = largest[k % R].max(value.abs());
            }
            fold.span(&acc[1], &largest, scales, relative, raw_scale)?;
        }
        Ok(())
    }
}

/// `level` in raw coordinates for a block in its level's centered basis:
/// `v = U M~^-1 U' r` with `U` the shift of the slopes by `means` (none:
/// `level` itself). `r'v` and its root come from the centered moments,
/// `max |v|` from the raw coefficients.
#[inline]
pub(super) fn centered<const R: usize>(
    (block, c, means): (&[f64], usize, &[f64]),
    r: &[f64],
    mut out: Option<&mut [f64]>,
    acc: &mut [[f64; R]; 3],
) {
    if means.is_empty() {
        level::<R>(block, c, r, out, acc);
        return;
    }
    // The centered moment of channel `b`, column `j`.
    let moment = |b: usize, j: usize| {
        if b == 0 {
            r[j]
        } else {
            r[b * R + j] - means[b - 1] * r[j]
        }
    };
    let (mut quad, mut lead, mut shift) = ([0.0; R], [0.0; R], [0.0; R]);
    for a in 0..c {
        let row = &block[a * c..(a + 1) * c];
        let mut v = [0.0; R];
        for (b, &m) in row.iter().enumerate() {
            for (j, v) in v.iter_mut().enumerate() {
                *v += m * moment(b, j);
            }
        }
        for (j, (quad, v)) in quad.iter_mut().zip(&v).enumerate() {
            *quad += moment(a, j) * v;
        }
        if a == 0 {
            lead = v;
            continue;
        }
        // Raw coefficients: the slopes as they are, the intercept less
        // the slopes' centers.
        for j in 0..R {
            shift[j] += means[a - 1] * v[j];
            acc[2][j] = acc[2][j].max(v[j].abs());
        }
        if let Some(out) = out.as_deref_mut() {
            out[a * R..(a + 1) * R].copy_from_slice(&v);
        }
    }
    for j in 0..R {
        let intercept = lead[j] - shift[j];
        acc[0][j] += quad[j];
        acc[1][j] = acc[1][j].max(quad[j].max(0.0).sqrt());
        acc[2][j] = acc[2][j].max(intercept.abs());
        if let Some(out) = out.as_deref_mut() {
            out[j] = intercept;
        }
    }
}

/// One level's values (`R` columns, intercept first) between the raw and
/// the centered basis of `means`: moments to centered (`m_s -= c_s m_0`),
/// or with `raw` coefficients to raw (`x_0 -= sum_s c_s x_s`).
#[inline]
pub(super) fn shift<const R: usize>(means: &[f64], values: &mut [f64], raw: bool) {
    let (lead, slopes) = values.split_at_mut(R);
    for (s, &mean) in means.iter().enumerate() {
        let slope = &mut slopes[s * R..(s + 1) * R];
        for j in 0..R {
            if raw {
                lead[j] -= mean * slope[j];
            } else {
                slope[j] -= mean * lead[j];
            }
        }
    }
}

/// One level: `v = M^-1 r` (`c` channels, `R` columns, `r` channel-major
/// by column: `r[ch * R + j]`), written to `out` when given; accumulates
/// `r'v`, `max sqrt(r'v)` and `max |v|` per column into `acc`.
#[inline]
pub(super) fn level<const R: usize>(
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

/// Per level of effect `e`, `add(row, level, sums)` over its rows into
/// `out` (`width` sums per level): each level's rows in ascending order for
/// a gathered effect, fixed row blocks (accumulators in `scratch`) merged in
/// block order for a scattered one.
fn level_sums<F>(
    st: &Structure,
    (e, effect): (usize, &Effect<'_>),
    width: usize,
    (out, scratch): (&mut [f64], &mut Vec<f64>),
    add: &F,
) -> Result<()>
where
    F: Fn(usize, usize, &mut [f64]) + Sync,
{
    if let Some(index) = st.gathered.iter().find(|g| g.effect == e) {
        let body = |task: usize, out: &mut [f64]| {
            for (k, g) in out.chunks_exact_mut(width).enumerate() {
                g.fill(0.0);
                let level = task * LEVEL_TASK + k;
                for &row in &index.rows[index.starts[level]..index.starts[level + 1]] {
                    add(row as usize, level, g);
                }
            }
        };
        each_block(out, LEVEL_TASK * width, st.nobs * width, &body);
        return Ok(());
    }
    let span = effect.levels() * width;
    let rows = st.scatter_rows;
    refill(scratch, st.scatter_blocks() * span, 0.0)?;
    let codes = effect.codes();
    let body = |block: usize, acc: &mut [f64]| {
        let start = block * rows;
        let end = (start + rows).min(st.nobs);
        for (k, &code) in codes[start..end].iter().enumerate() {
            let code = code as usize;
            add(start + k, code, &mut acc[code * width..(code + 1) * width]);
        }
    };
    each_block(scratch, span, st.nobs * width, &body);
    out.fill(0.0);
    for part in scratch.chunks_exact(span) {
        for (o, p) in out.iter_mut().zip(part) {
            *o += p;
        }
    }
    Ok(())
}

/// The centers of `part` (an effect with an intercept): each slope's
/// weighted mean on each level (zero on a level without weight).
fn means(
    st: &Structure,
    (e, effect): (usize, &Effect<'_>),
    weights: Option<&[f64]>,
    part: Part,
    blocks: &mut Blocks,
) -> Result<()> {
    let width = part.channels;
    let slopes = effect.slopes();
    let add = |row: usize, _: usize, g: &mut [f64]| {
        let w = weights.map_or(1.0, |w| w[row]);
        g[0] += w;
        for (g, slope) in g[1..].iter_mut().zip(slopes) {
            *g += w * slope[row];
        }
    };
    refill(&mut blocks.sums, part.levels * width, 0.0)?;
    let out = (&mut blocks.sums[..], &mut blocks.scratch);
    level_sums(st, (e, effect), width, out, &add)?;
    let centers = &mut blocks.centers[part.center..part.center + part.levels * part.centered];
    for (centers, sums) in centers
        .chunks_exact_mut(part.centered)
        .zip(blocks.sums.chunks_exact(width))
    {
        for (center, &sum) in centers.iter_mut().zip(&sums[1..]) {
            *center = if sums[0] > 0.0 { sum / sums[0] } else { 0.0 };
        }
    }
    Ok(())
}

/// The blocks of one effect into `blocks.values[span]` (row-major, both
/// triangles), in each level's centered basis (the slopes less their level
/// means, for an effect with an intercept), from the centered row values so
/// that a large common offset does not cancel the slopes' variation.
fn gram(
    st: &Structure,
    (e, effect): (usize, &Effect<'_>),
    weights: Option<&[f64]>,
    (span, part): (std::ops::Range<usize>, Part),
    blocks: &mut Blocks,
) -> Result<()> {
    let c = effect.channels();
    let width = c * c;
    let lead = usize::from(effect.intercept());
    let slopes = effect.slopes();
    let centers = &blocks.centers[part.center..part.center + part.levels * part.centered];
    let add = |row: usize, level: usize, g: &mut [f64]| {
        let w = weights.map_or(1.0, |w| w[row]);
        let value = |ch: usize| {
            if ch < lead {
                1.0
            } else if part.centered > 0 {
                slopes[ch - lead][row] - centers[level * part.centered + ch - lead]
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
    let out = (&mut blocks.values[span], &mut blocks.scratch);
    level_sums(st, (e, effect), width, (&mut *out.0, &mut *out.1), &add)?;
    let constant = &mut blocks.constant[part.center..part.center + part.levels * part.centered];
    for (l, g) in out.0.chunks_exact_mut(width).enumerate() {
        for a in 0..c {
            for b in 0..a {
                g[b * c + a] = g[a * c + b];
            }
        }
        // A constant slope's centered column is rounding: it is decoupled,
        // with its raw sum of squares as its curvature (a null direction
        // whose rounding the center would otherwise amplify).
        for s in 0..part.centered {
            let (ch, mean) = (s + 1, centers[l * part.centered + s]);
            let centered = g[ch * c + ch];
            if centered <= CONSTANT * (centered + mean * mean * g[0]) {
                constant[l * part.centered + s] = true;
                for k in 0..c {
                    g[ch * c + k] = 0.0;
                    g[k * c + ch] = 0.0;
                }
                g[ch * c + ch] = mean * mean * g[0];
            }
        }
    }
    Ok(())
}

/// Replace the `c x c` positive semidefinite `block` by its action `M^-1`
/// (module docs); `true` when it has a null direction. With `reference`, a
/// Schur block's un-eliminated Gram, the scaling and the null threshold and
/// curvature come from the reference's diagonal (its unit scale), so a
/// block that cancels to rounding is null; otherwise from the block's own.
pub(super) fn invert(
    block: &mut [f64],
    c: usize,
    reference: Option<&[f64]>,
    scratch: &mut [f64],
) -> bool {
    let (d, rest) = scratch[..invert_scratch(c)].split_at_mut(c);
    for (k, d) in d.iter_mut().enumerate() {
        *d = reference.map_or(block[k * c + k], |r| r[k * c + k]);
    }
    if c == 1 {
        let (g, scale) = (block[0], d[0]);
        let kept = match reference {
            None => g > 0.0,
            Some(_) => g > NULL * scale,
        };
        block[0] = match (kept, reference) {
            (true, _) => 1.0 / g,
            (false, None) => 1.0,
            (false, Some(_)) if scale > 0.0 => 1.0 / scale,
            (false, Some(_)) => 1.0,
        };
        return !kept;
    }
    let top = d.iter().copied().fold(0.0_f64, f64::max);
    if top <= 0.0 {
        block.fill(0.0);
        for k in 0..c {
            block[k * c + k] = 1.0;
        }
        return true;
    }
    let (s, rest) = rest.split_at_mut(c);
    let (phi, rest) = rest.split_at_mut(c);
    let (a, q) = rest.split_at_mut(c * c);
    for (k, s) in s.iter_mut().enumerate() {
        *s = 1.0 / if d[k] > 0.0 { d[k] } else { top }.sqrt();
    }
    for i in 0..c {
        for k in 0..c {
            a[i * c + k] = s[i] * block[i * c + k] * s[k];
        }
    }
    eigen(a, q, c);
    let largest = (0..c).map(|k| a[k * c + k]).fold(f64::MIN, f64::max);
    // The reference scaling has a unit diagonal: its unit is the scale.
    let largest = if reference.is_some() { 1.0 } else { largest };
    let mut singular = false;
    for (k, phi) in phi.iter_mut().enumerate() {
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
            for (m, phi) in phi.iter().enumerate() {
                sum += q[i * c + m] * phi * q[k * c + m];
            }
            block[i * c + k] = s[i] * sum * s[k];
        }
    }
    singular
}

/// The scratch values [`invert`] needs for `c` channels.
pub(super) const fn invert_scratch(c: usize) -> usize {
    2 * c * c + 3 * c
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
        assert!(!invert(&mut block, 3, None, &mut [0.0; 32]));
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
        assert!(invert(&mut block, 2, None, &mut [0.0; 32]));
        let gmg = multiply(&gram, &multiply(&block, &gram, 2), 2);
        for (x, y) in gmg.iter().zip(&gram) {
            assert!((x - y).abs() < 1e-9 * y.abs().max(1.0), "{gmg:?}");
        }
        let det = block[0] * block[3] - block[1] * block[2];
        assert!(block[0] > 0.0 && det > 0.0 && (block[1] - block[2]).abs() < 1e-15);
        // Intercept with a zero slope.
        let mut zero = [2.0, 0.0, 0.0, 0.0];
        assert!(invert(&mut zero, 2, None, &mut [0.0; 32]));
        assert_eq!(zero[0], 0.5);
        assert!(zero[3] > 0.0 && zero[1] == 0.0);
        let mut empty = [0.0];
        assert!(invert(&mut empty, 1, None, &mut [0.0; 32]));
        assert_eq!(empty[0], 1.0);
    }
}
