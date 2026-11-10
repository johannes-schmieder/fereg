//! Lockstep preconditioned CG on the full normal equations
//! `D'WD x = D'W y`, whose residual `D'W (y - D x)` is every level's moment.
//!
//! The system is singular and consistent: per-component constant shifts
//! across effects are null directions of `D'WD`, and its right-hand side
//! lies in the range. CG with a fixed symmetric positive definite `M^-1`
//! (Jacobi, identity or additive Schwarz) stays in that range, so the
//! fitted contribution `D x` converges while `x` itself is determined only
//! up to the null space.

use super::cells::{Actions, Cells};
use super::kernels::{adjoint, forward, Rows, PARALLEL_WORK};
use super::refill;
use super::schwarz::{Schwarz, Scratch};
use super::structure::Structure;
use super::weights::State;
use crate::design::Effect;
use crate::elimination::pcg::{Solve, Tally, Targets, CEILING, STALLED};
use crate::elimination::{Cancel, KEPT_WINDOW, STALL_WINDOW};
use crate::error::Result;
use rayon::prelude::*;
use std::ops::Range;

/// Levels per partial sum of the coefficient-space reductions (fixed: the
/// sums do not depend on the worker count).
const CHUNK: usize = 2048;

/// An escalating Jacobi recurrence is measured after this many iterations
/// ...
pub(super) const PILOT: usize = 24;
/// ... and hands over as slow when its observed rate predicts more than this
/// many iterations in all for a column (a stronger preconditioner's build
/// then pays for itself).
pub(super) const SLOW_ITERATIONS: f64 = 100.0;
/// The decline of a recurrence predicted slow (escalation only).
pub(crate) const SLOW: &str = "slow recurrence";

/// Whether a column whose stopping measure went from `earlier` to `best`
/// over the last `window` of its `done` iterations is predicted to need
/// more than [`SLOW_ITERATIONS`] iterations in all to reach its goal
/// (measure 1) at that geometric rate (the late rate, since CG's early
/// iterations are faster than its tail).
pub(super) fn predicted_slow(earlier: f64, best: f64, (window, done): (usize, usize)) -> bool {
    if best <= 1.0 {
        return false;
    }
    if !(best < earlier && earlier.is_finite()) {
        return true;
    }
    let rate = (earlier / best).ln() / window as f64;
    done as f64 + best.ln() / rate > SLOW_ITERATIONS
}

/// Flat solve workspace (`R` values per level or row) resized per batch
/// width; repeated widths allocate nothing.
#[derive(Default)]
pub(super) struct Work {
    pub(super) x: Vec<f64>,
    /// Per column, the best iterate that met the acceptance rule.
    xs: Vec<f64>,
    pub(super) r: Vec<f64>,
    z: Vec<f64>,
    p: Vec<f64>,
    /// `D'WD p`, and the formed residual's moments after the solve.
    pub(super) q: Vec<f64>,
    /// One value per row and column: `W D p`, `W (y - D x)`.
    pub(super) t: Vec<f64>,
    pub(super) scatter: Vec<f64>,
    /// Coefficient-space partial sums, `3 R` per chunk.
    partials: Vec<f64>,
    /// Observation-pass partial sums.
    pub(super) rows: Vec<f64>,
    /// Modified LSMR: the weighted column `W^1/2 y`.
    pub(super) b: Vec<f64>,
    /// The Schwarz apply workspace (sized by the preconditioner).
    pub(super) schwarz: Scratch,
    /// Partials of the channel-block pass (designs with slopes) ...
    pub(super) measures: Vec<f64>,
    /// ... and of modified LSMR's preconditioner, which runs beside it.
    pub(super) inverse_measures: Vec<f64>,
}

/// Bytes of a [`Work`] for `r` columns.
pub(super) fn work_bytes(st: &Structure, r: usize) -> u128 {
    let (m, n, r) = (st.coefficients() as u128, st.nobs as u128, r as u128);
    let chunks = m.div_ceil(CHUNK as u128);
    let blocks = (st.nobs as u128).div_ceil(super::kernels::ROW_BLOCK as u128);
    let scatter = (st.scatter_blocks() * st.scatter_levels()) as u128;
    8 * r * (6 * m + n + scatter + 3 * chunks + 3 * blocks)
}

impl Work {
    /// Size every buffer for `r` columns, keeping allocations.
    pub(super) fn size(&mut self, st: &Structure, r: usize) -> Result<()> {
        let m = st.coefficients() * r;
        for v in [
            &mut self.x,
            &mut self.xs,
            &mut self.r,
            &mut self.z,
            &mut self.p,
            &mut self.q,
        ] {
            if v.len() != m {
                refill(v, m, 0.0)?;
            }
        }
        let sizes = [
            (&mut self.t, st.nobs * r),
            (
                &mut self.scatter,
                st.scatter_blocks() * st.scatter_levels() * r,
            ),
            (
                &mut self.partials,
                st.coefficients().div_ceil(CHUNK) * 3 * r,
            ),
            (
                &mut self.rows,
                st.nobs.div_ceil(super::kernels::ROW_BLOCK) * 3 * r,
            ),
        ];
        for (v, len) in sizes {
            if v.len() != len {
                refill(v, len, 0.0)?;
            }
        }
        Ok(())
    }

    /// Size every recurrence buffer for the Schur system of `cells` (`m`
    /// coefficients, `r` columns; `t` holds the cells' workspace).
    pub(super) fn size_reduced(&mut self, cells: &Cells, m: usize, r: usize) -> Result<()> {
        let sizes = [
            (&mut self.x, m * r),
            (&mut self.xs, m * r),
            (&mut self.r, m * r),
            (&mut self.z, m * r),
            (&mut self.p, m * r),
            (&mut self.q, m * r),
            (&mut self.t, cells.cells() * cells.channels[1] * r),
            (&mut self.scatter, cells.scratch_len(r)),
            (&mut self.partials, m.div_ceil(CHUNK) * 3 * r),
            (&mut self.measures, cells.small_tasks() * 3 * r),
        ];
        for (v, len) in sizes {
            if v.len() != len {
                refill(v, len, 0.0)?;
            }
        }
        Ok(())
    }

    /// Size the channel-block partials of `state` for `r` columns.
    pub(super) fn size_blocks(&mut self, state: &State, r: usize) -> Result<()> {
        let len = state.blocks().map_or(0, |b| b.tasks() * 3 * r);
        if self.measures.len() != len {
            refill(&mut self.measures, len, 0.0)?;
        }
        Ok(())
    }
}

/// The normal-equation operator `D'WD` and its preconditioner: additive
/// Schwarz when given, else Jacobi (with the channel blocks of the effects
/// with slopes) or the identity.
pub(super) struct Normal<'a, 'd> {
    pub(super) st: &'a Structure,
    pub(super) effects: &'a [Effect<'d>],
    pub(super) state: &'a State,
    pub(super) jacobi: bool,
    pub(super) schwarz: Option<&'a Schwarz>,
    /// The elimination with slopes: the operator is the Schur complement of
    /// these cells on the smaller effect's coefficients, preconditioned by
    /// its diagonal blocks.
    pub(super) cells: Option<&'a Cells>,
}

impl Normal<'_, '_> {
    /// The inverse masses and their roots of the system's coefficients.
    fn inverses(&self) -> (&[f64], &[f64]) {
        let state = self.state;
        match self.cells {
            Some(cells) => {
                let b = cells.effects[1];
                let span = self.st.offsets[b]..self.st.offsets[b + 1];
                (
                    &state.inverse_mass[span.clone()],
                    &state.inverse_root_mass[span],
                )
            }
            None => (&state.inverse_mass, &state.inverse_root_mass),
        }
    }

    /// `q = D'W D p` through `t = W D p` (the elimination: `q = S p` with
    /// `t` the cells' workspace).
    pub(super) fn apply<const R: usize>(
        &self,
        p: &[f64],
        t: &mut [f64],
        q: &mut [f64],
        scatter: &mut [f64],
    ) {
        if let Some(cells) = self.cells {
            let [a, _] = cells.effects;
            let actions = Actions::of(self.st, self.state, a, cells.channels[0]);
            cells.apply::<R>(actions, (p, q), t, scatter);
            return;
        }
        let design = (self.effects, self.state.basis());
        forward::<R>(self.st, design, p, self.state.weights(), t);
        let rows = Rows {
            data: t,
            stride: R,
            first: 0,
        };
        adjoint::<R>(self.st, design, rows, None, q, scatter);
    }
}

/// Fixed level chunks of `0..m`, serially below the parallel threshold.
fn chunks(m: usize) -> impl Iterator<Item = Range<usize>> {
    (0..m.div_ceil(CHUNK)).map(move |c| c * CHUNK..((c + 1) * CHUNK).min(m))
}

/// With `alpha`, `x += alpha p` and `r -= alpha q` first; then `z = M^-1 r`.
/// Returns `r'z`, `max |r_g| / sqrt(m_g)` and `max |r_g| / m_g` per column
/// (a blocked level: `sqrt(r_l' M_l^-1 r_l)` and `max |M_l^-1 r_l|`),
/// as plain sums and maxima over fixed chunks combined in chunk order (the
/// Schwarz action's `r'z` by a separate fixed-chunk dot).
fn update<const R: usize>(
    op: &Normal<'_, '_>,
    work: &mut Work,
    alpha: Option<[f64; R]>,
) -> [[f64; R]; 3] {
    let mut out = fused::<R>(op, work, alpha);
    if let Some(cells) = op.cells {
        let [_, b] = cells.effects;
        let smaller = Actions::of(op.st, op.state, b, cells.channels[1]);
        let [rz, inf, zinf] =
            cells.precondition::<R>(smaller, &work.r, &mut work.z, &mut work.measures);
        for j in 0..R {
            out[0][j] = rz[j];
            out[1][j] = out[1][j].max(inf[j]);
            out[2][j] = out[2][j].max(zinf[j]);
        }
        return out;
    }
    let blocks = op.state.blocks();
    if let Some(schwarz) = op.schwarz {
        let inverse = (&op.state.inverse_mass[..], blocks);
        schwarz.apply::<R>(&work.r, &mut work.z, &mut work.schwarz, inverse);
        out[0] = dot::<R>(&work.r, &work.z, &mut work.partials);
    }
    if let Some(blocks) = blocks {
        // The blocked effects' measures, and with Jacobi their action (the
        // fused pass left them zero: their inverse masses are zero).
        let jacobi = op.jacobi && op.schwarz.is_none();
        let z = jacobi.then_some(&mut work.z[..]);
        let [rz, inf, zinf] = blocks.measure::<R>(&op.st.offsets, &work.r, z, &mut work.measures);
        for j in 0..R {
            if jacobi {
                out[0][j] += rz[j];
            }
            out[1][j] = out[1][j].max(inf[j]);
            out[2][j] = out[2][j].max(zinf[j]);
        }
    }
    out
}

/// The fused vector pass of [`update`]; `z` (and `r'z`) only for Jacobi or
/// the identity.
fn fused<const R: usize>(
    op: &Normal<'_, '_>,
    work: &mut Work,
    alpha: Option<[f64; R]>,
) -> [[f64; R]; 3] {
    let (inv_mass, inv_root) = op.inverses();
    let (jacobi, diagonal) = (op.jacobi, op.schwarz.is_none() && op.cells.is_none());
    let Work {
        x,
        r,
        z,
        p,
        q,
        partials,
        ..
    } = work;
    let (p, q) = (&p[..], &q[..]);
    let body = |c: usize, x: &mut [f64], r: &mut [f64], z: &mut [f64], part: &mut [f64]| {
        let first = c * CHUNK;
        let mut s = [0.0; R];
        let mut h = [0.0_f64; R];
        let mut hz = [0.0_f64; R];
        for k in 0..r.len() / R {
            let g = first + k;
            let (ir, im) = (inv_root[g], inv_mass[g]);
            for j in 0..R {
                let a = k * R + j;
                if let Some(alpha) = alpha {
                    x[a] += alpha[j] * p[g * R + j];
                    r[a] -= alpha[j] * q[g * R + j];
                }
                if diagonal {
                    z[a] = if jacobi { im * r[a] } else { r[a] };
                    s[j] += r[a] * z[a];
                }
                h[j] = h[j].max(r[a].abs() * ir);
                hz[j] = hz[j].max(r[a].abs() * im);
            }
        }
        part[..R].copy_from_slice(&s);
        part[R..2 * R].copy_from_slice(&h);
        part[2 * R..3 * R].copy_from_slice(&hz);
    };
    let span = CHUNK * R;
    let m = r.len() / R;
    if r.len() < PARALLEL_WORK {
        for (c, (((x, r), z), part)) in x
            .chunks_mut(span)
            .zip(r.chunks_mut(span))
            .zip(z.chunks_mut(span))
            .zip(partials.chunks_mut(3 * R))
            .enumerate()
        {
            body(c, x, r, z, part);
        }
    } else {
        x.par_chunks_mut(span)
            .zip(r.par_chunks_mut(span))
            .zip(z.par_chunks_mut(span))
            .zip(partials.par_chunks_mut(3 * R))
            .enumerate()
            .for_each(|(c, (((x, r), z), part))| body(c, x, r, z, part));
    }
    let mut out = [[0.0; R]; 3];
    for part in partials[..m.div_ceil(CHUNK) * 3 * R].chunks_exact(3 * R) {
        for j in 0..R {
            out[0][j] += part[j];
            out[1][j] = out[1][j].max(part[R + j]);
            out[2][j] = out[2][j].max(part[2 * R + j]);
        }
    }
    out
}

/// Per-column `a . b` over fixed chunks combined in chunk order.
fn dot<const R: usize>(a: &[f64], b: &[f64], partials: &mut [f64]) -> [f64; R] {
    let m = a.len() / R;
    let body = |range: Range<usize>| {
        let mut s = [0.0; R];
        for g in range {
            for j in 0..R {
                s[j] += a[g * R + j] * b[g * R + j];
            }
        }
        s
    };
    let mut total = [0.0; R];
    if a.len() < PARALLEL_WORK {
        for range in chunks(m) {
            let s = body(range);
            for j in 0..R {
                total[j] += s[j];
            }
        }
        return total;
    }
    partials
        .par_chunks_mut(3 * R)
        .take(m.div_ceil(CHUNK))
        .enumerate()
        .for_each(|(c, slot)| {
            slot[..R].copy_from_slice(&body(c * CHUNK..((c + 1) * CHUNK).min(m)));
        });
    for slot in partials[..m.div_ceil(CHUNK) * 3 * R].chunks_exact(3 * R) {
        for j in 0..R {
            total[j] += slot[j];
        }
    }
    total
}

/// `to[., j] = from[., j]` for the columns of `mask`.
fn copy_columns<const R: usize>(to: &mut [f64], from: &[f64], mask: [bool; R]) {
    let body = |to: &mut [f64], from: &[f64]| {
        for (t, f) in to.chunks_exact_mut(R).zip(from.chunks_exact(R)) {
            for j in 0..R {
                if mask[j] {
                    t[j] = f[j];
                }
            }
        }
    };
    if to.len() < PARALLEL_WORK {
        body(to, from);
    } else {
        to.par_chunks_mut(CHUNK * R)
            .zip(from.par_chunks(CHUNK * R))
            .for_each(|(to, from)| body(to, from));
    }
}

/// `p = z + beta p` (`p = z` without `beta`) on the columns of `mask`,
/// zero elsewhere.
fn directions<const R: usize>(p: &mut [f64], z: &[f64], mask: [bool; R], beta: Option<[f64; R]>) {
    let body = |p: &mut [f64], z: &[f64]| {
        for (p, z) in p.chunks_exact_mut(R).zip(z.chunks_exact(R)) {
            for j in 0..R {
                p[j] = match (mask[j], beta) {
                    (false, _) => 0.0,
                    (true, None) => z[j],
                    (true, Some(beta)) => z[j] + beta[j] * p[j],
                };
            }
        }
    };
    if p.len() < PARALLEL_WORK {
        body(p, z);
    } else {
        p.par_chunks_mut(CHUNK * R)
            .zip(z.par_chunks(CHUNK * R))
            .for_each(|(p, z)| body(p, z));
    }
}

/// Per-column recurrence state (as for the eliminated system).
struct Columns<const R: usize> {
    active: [bool; R],
    rz: [f64; R],
    /// The best stopping measure at half the pilot.
    initial: [f64; R],
    best: [f64; R],
    checkpoint_best: [f64; R],
    kept: [bool; R],
    kept_measure: [f64; R],
    kept_checkpoint: [f64; R],
    accepted: usize,
    floor: [bool; R],
}

impl<const R: usize> Columns<R> {
    /// Return the active columns of `mask` to their kept iterates; `false`
    /// when one of them has none.
    fn fall_back(&mut self, mask: [bool; R], x: &mut [f64], xs: &[f64]) -> bool {
        if mask.iter().zip(&self.kept).any(|(&on, &kept)| on && !kept) {
            return false;
        }
        if mask.contains(&true) {
            copy_columns(x, xs, mask);
            for ((active, floor), &on) in self.active.iter_mut().zip(&mut self.floor).zip(&mask) {
                if on {
                    *active = false;
                    *floor = true;
                    self.accepted += 1;
                }
            }
        }
        true
    }
}

/// `z = M^-1 r`, the measures of the initial residual, `p = z` on the active
/// columns and the initial iterate kept where it meets the acceptance rule;
/// `None` when the residual is not finite.
fn start<const R: usize>(
    op: &Normal<'_, '_>,
    work: &mut Work,
    targets: &Targets<R>,
) -> Option<Columns<R>> {
    let [rz, inf, zinf] = update::<R>(op, work, None);
    let mut active = [false; R];
    let mut progress = [0.0_f64; R];
    for j in 0..R {
        if !(inf[j].is_finite() && rz[j].is_finite() && zinf[j].is_finite()) {
            return None;
        }
        progress[j] = targets.goal(inf[j], zinf[j], j);
        active[j] = progress[j] > 1.0;
    }
    directions(&mut work.p, &work.z, active, None);
    work.xs.copy_from_slice(&work.x);
    let mut kept = [false; R];
    let mut kept_measure = [f64::INFINITY; R];
    for j in 0..R {
        if active[j] && targets.accepted(inf[j], zinf[j], j) {
            kept[j] = true;
            kept_measure[j] = progress[j];
        }
    }
    Some(Columns {
        active,
        rz,
        initial: progress,
        best: progress,
        checkpoint_best: progress,
        kept,
        kept_measure,
        kept_checkpoint: progress,
        accepted: 0,
        floor: [false; R],
    })
}

/// Stop the columns at their goal, keep improved iterates that meet the
/// acceptance rule and return `beta`; `None` when a measure is not finite.
fn advance<const R: usize>(
    state: &mut Columns<R>,
    targets: &Targets<R>,
    work: &mut Work,
    [next, inf, zinf]: [[f64; R]; 3],
) -> Option<[f64; R]> {
    let mut beta = [0.0; R];
    let mut save = [false; R];
    for j in 0..R {
        if !state.active[j] {
            continue;
        }
        if !(inf[j].is_finite() && next[j].is_finite() && zinf[j].is_finite()) {
            return None;
        }
        let current = targets.goal(inf[j], zinf[j], j);
        state.best[j] = state.best[j].min(current);
        if current <= 1.0 {
            state.active[j] = false;
        } else {
            beta[j] = next[j] / state.rz[j];
            if current < state.kept_measure[j] && targets.accepted(inf[j], zinf[j], j) {
                save[j] = true;
                state.kept[j] = true;
                state.kept_measure[j] = current;
            }
        }
        state.rz[j] = next[j];
    }
    if save.contains(&true) {
        copy_columns(&mut work.xs, &work.x, save);
    }
    Some(beta)
}

/// Lockstep PCG on `D'WD x = D'W y` from the iterate in `work.x`, with its
/// true residual `D'W (y - D x)` in `work.r`; the stopping rules, kept
/// iterates and stall windows are those of the eliminated system. A column
/// stops when its largest scaled moment `|r_g| / sqrt(m_g)` (and, with a
/// coefficient goal, its Jacobi estimate `|r_g| / m_g`) meets its goal.
/// With `pilot`, a recurrence that a column's rate after [`PILOT`]
/// iterations predicts slow ([`predicted_slow`]) declines as [`SLOW`] (for
/// escalation).
pub(super) fn pcg<const R: usize>(
    op: &Normal<'_, '_>,
    work: &mut Work,
    targets: &Targets<R>,
    (cap, pilot): (usize, bool),
    cancel: &Cancel<'_>,
    tally: &mut Tally<R>,
) -> Result<Solve<R>> {
    let Some(mut state) = start(op, work, targets) else {
        return Ok(Solve::Declined("nonfinite initial moments"));
    };
    let mut iterations = 0;
    while state.active.contains(&true) {
        if iterations >= cap {
            if !state.fall_back(state.active, &mut work.x, &work.xs) {
                return Ok(Solve::Declined(CEILING));
            }
            break;
        }
        tally.column_iterations += state.active.iter().filter(|&&on| on).count();
        for (count, &on) in tally.per_column.iter_mut().zip(&state.active) {
            *count += usize::from(on);
        }
        cancel.check()?;
        op.apply::<R>(&work.p, &mut work.t, &mut work.q, &mut work.scatter);
        let pq = dot::<R>(&work.p, &work.q, &mut work.partials);
        let mut alpha = [0.0; R];
        let mut flat = [false; R];
        for j in 0..R {
            if state.active[j] {
                if pq[j].is_finite() && pq[j] > 0.0 {
                    alpha[j] = state.rz[j] / pq[j];
                } else {
                    flat[j] = true;
                }
            }
        }
        if !state.fall_back(flat, &mut work.x, &work.xs) {
            return Ok(Solve::Declined("nonpositive curvature"));
        }
        if !state.active.contains(&true) {
            break;
        }
        let measures = update::<R>(op, work, Some(alpha));
        iterations += 1;
        tally.iterations += 1;
        let Some(beta) = advance(&mut state, targets, work, measures) else {
            return Ok(Solve::Declined("nonfinite moments"));
        };
        if pilot && iterations == PILOT / 2 {
            state.initial = state.best;
        }
        let window = (PILOT - PILOT / 2, PILOT);
        if pilot
            && iterations == PILOT
            && (0..R)
                .any(|j| state.active[j] && predicted_slow(state.initial[j], state.best[j], window))
        {
            return Ok(Solve::Declined(SLOW));
        }
        if iterations % KEPT_WINDOW == 0 {
            let long = iterations % STALL_WINDOW == 0;
            let mut stalled = [false; R];
            for (j, stalled) in stalled.iter_mut().enumerate() {
                let reference = if state.kept[j] {
                    state.kept_checkpoint[j]
                } else {
                    state.checkpoint_best[j]
                };
                *stalled =
                    state.active[j] && (state.kept[j] || long) && state.best[j] > 0.5 * reference;
            }
            if !state.fall_back(stalled, &mut work.x, &work.xs) {
                return Ok(Solve::Declined(STALLED));
            }
            state.kept_checkpoint = state.best;
            if long {
                state.checkpoint_best = state.best;
            }
        }
        directions(&mut work.p, &work.z, state.active, Some(beta));
    }
    Ok(Solve::Converged {
        accepted: state.accepted,
        floor: state.floor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The late geometric rate predicts the total: a column reaching its
    /// goal is never slow; one whose measure did not fall, or falls too
    /// slowly for the limit, is.
    #[test]
    fn the_pilot_predicts_slow_recurrences_from_their_late_rate() {
        let window = (PILOT - PILOT / 2, PILOT);
        assert!(!predicted_slow(10.0, 1.0, window));
        assert!(predicted_slow(5.0, 5.0, window));
        assert!(predicted_slow(f64::INFINITY, 5.0, window));
        // A tenfold drop over 12 iterations from 1e4: 4 more decades, 48
        // more iterations, 72 in all.
        assert!(!predicted_slow(1e5, 1e4, window));
        // A twofold drop from 1e4: about 160 more iterations.
        assert!(predicted_slow(2e4, 1e4, window));
    }
}
