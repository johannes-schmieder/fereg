//! Lockstep preconditioned CG on the reduced system.

use super::kernels::{apply, copy_columns, dot, Work};
use super::structure::Structure;
use super::weights::Weights;
use super::{Cancel, CHUNK, KEPT_WINDOW, PARALLEL_WORK, STALL_WINDOW};
use crate::error::Result;
use rayon::prelude::*;
use std::ops::Range;

/// Decline of a recurrence that stopped progressing ...
pub(crate) const STALLED: &str = "stalled reduced recurrence";
/// ... and of one that reached the iteration ceiling (both may escalate).
pub(crate) const CEILING: &str = "iteration ceiling";

pub(crate) enum Solve<const R: usize> {
    Converged {
        /// Columns stopped at the acceptance rule after a breakdown.
        accepted: usize,
        /// Columns returned to an iterate meeting the acceptance rule.
        floor: [bool; R],
    },
    Declined(&'static str),
}

/// Iteration counts of the recurrences of one batch.
pub(crate) struct Tally<const R: usize> {
    pub(crate) iterations: usize,
    /// The sum over iterations of the active columns.
    pub(crate) column_iterations: usize,
    /// Iterations each column was active.
    pub(crate) per_column: [usize; R],
}

impl<const R: usize> Default for Tally<R> {
    fn default() -> Self {
        Self {
            iterations: 0,
            column_iterations: 0,
            per_column: [0; R],
        }
    }
}

/// The preconditioner `M^-1` of the recurrence. The fused vector passes form
/// `z = D^-1 r` with the stopping measures; any other preconditioner then
/// overwrites `z` where its action differs, and `r'z` is a separate
/// fixed-chunk dot.
pub(super) trait Precondition<const R: usize> {
    /// `M^-1 = D^-1`: the fused passes also sum `r'z`.
    const JACOBI: bool;
    /// `z = M^-1 r`, given `z = D^-1 r`.
    fn correct(&mut self, r: &[[f64; R]], z: &mut [[f64; R]]) -> Result<()>;
}

/// The inverse diagonal of the reduced matrix.
pub(super) struct Jacobi;

impl<const R: usize> Precondition<R> for Jacobi {
    const JACOBI: bool = true;

    fn correct(&mut self, _: &[[f64; R]], _: &mut [[f64; R]]) -> Result<()> {
        Ok(())
    }
}

/// `value / target` with an exact zero target accepting only exact zeros.
pub(crate) fn ratio(value: f64, target: f64) -> f64 {
    if target > 0.0 {
        value / target
    } else if value == 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}

/// Stopping thresholds of one batch: `goal` is where a column stops;
/// `accept` (the certificate-scale rule) is where a column may stop when its
/// recurrence breaks down near the rounding floor.
pub(crate) struct Targets<const R: usize> {
    pub(crate) goal: [f64; R],
    pub(crate) goal_coefficient: Option<[f64; R]>,
    pub(crate) accept: [f64; R],
    pub(crate) accept_coefficient: Option<[f64; R]>,
}

impl<const R: usize> Targets<R> {
    /// The same acceptance rule with the goals scaled by `factor`.
    pub(super) fn tightened(&self, factor: f64) -> Self {
        Self {
            goal: self.goal.map(|g| g * factor),
            goal_coefficient: self.goal_coefficient.map(|t| t.map(|g| g * factor)),
            accept: self.accept,
            accept_coefficient: self.accept_coefficient,
        }
    }

    pub(super) fn measure(
        target: &[f64; R],
        coefficient: Option<&[f64; R]>,
        inf: f64,
        zinf: f64,
        c: usize,
    ) -> f64 {
        ratio(inf, target[c]).max(coefficient.map_or(0.0, |t| ratio(zinf, t[c])))
    }

    pub(crate) fn goal(&self, inf: f64, zinf: f64, c: usize) -> f64 {
        Self::measure(&self.goal, self.goal_coefficient.as_ref(), inf, zinf, c)
    }

    pub(crate) fn accepted(&self, inf: f64, zinf: f64, c: usize) -> bool {
        Self::measure(&self.accept, self.accept_coefficient.as_ref(), inf, zinf, c) <= 1.0
    }
}

/// Per-column recurrence state of one lockstep solve.
struct Columns<const R: usize> {
    active: [bool; R],
    rz: [f64; R],
    best: [f64; R],
    checkpoint_best: [f64; R],
    /// Whether `xs` holds an iterate that met the acceptance rule, and its
    /// stopping measure.
    kept: [bool; R],
    kept_measure: [f64; R],
    kept_checkpoint: [f64; R],
    /// Columns stopped at their kept iterate.
    accepted: usize,
    floor: [bool; R],
}

impl<const R: usize> Columns<R> {
    /// Return the active columns of `mask` to their kept iterates; `false`
    /// (decline) when one of them has none.
    fn fall_back(&mut self, mask: [bool; R], x: &mut [[f64; R]], xs: &[[f64; R]]) -> bool {
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

/// `z = M^-1 r`, `rz` and the stopping measures of the initial residual;
/// `x = 0` (`from_zero`, else the current iterate), `p = z` on the active
/// columns and the initial iterate kept where it meets the acceptance rule.
/// `None` when the residual is not finite.
fn start<const R: usize, M: Precondition<R>>(
    w: &Weights,
    work: &mut Work<R>,
    targets: &Targets<R>,
    precond: &mut M,
    from_zero: bool,
) -> Result<Option<Columns<R>>> {
    let inv_diag = &w.inverse_diagonal;
    let inv_root = &w.inverse_root_mass;
    let mut rz = [0.0; R];
    let mut inf = [0.0_f64; R];
    let mut zinf = [0.0_f64; R];
    {
        let (r, z) = (&work.r, &mut work.z);
        for j in 0..r.len() {
            for c in 0..R {
                z[j][c] = inv_diag[j] * r[j][c];
                if M::JACOBI {
                    rz[c] += r[j][c] * z[j][c];
                }
                inf[c] = inf[c].max(r[j][c].abs() * inv_root[j]);
                zinf[c] = zinf[c].max(z[j][c].abs());
            }
        }
    }
    if !M::JACOBI {
        precond.correct(&work.r, &mut work.z)?;
        rz = dot::<R>(&work.r, &work.z, &mut work.partials);
    }
    if from_zero {
        work.x.fill([0.0; R]);
    }
    let mut active = [false; R];
    let mut progress = [0.0_f64; R];
    for c in 0..R {
        if !inf[c].is_finite() || !rz[c].is_finite() || !zinf[c].is_finite() {
            return Ok(None);
        }
        progress[c] = targets.goal(inf[c], zinf[c], c);
        active[c] = progress[c] > 1.0;
    }
    for (p, z) in work.p.iter_mut().zip(&work.z) {
        for c in 0..R {
            p[c] = if active[c] { z[c] } else { 0.0 };
        }
    }
    // The best iterate meeting the acceptance rule, per column (`xs`).
    let mut kept = [false; R];
    let mut kept_measure = [f64::INFINITY; R];
    if from_zero {
        work.xs.fill([0.0; R]);
    } else {
        work.xs.copy_from_slice(&work.x);
    }
    for c in 0..R {
        if active[c] && targets.accepted(inf[c], zinf[c], c) {
            kept[c] = true;
            kept_measure[c] = progress[c];
        }
    }
    Ok(Some(Columns {
        active,
        rz,
        best: progress,
        checkpoint_best: progress,
        kept,
        kept_measure,
        kept_checkpoint: progress,
        accepted: 0,
        floor: [false; R],
    }))
}

/// `x += alpha p`, `r -= alpha q`, `z = M^-1 r` with `q = S p` in the first
/// lane; returns `r'z`, `max |r_j| / sqrt(m_j)` and `max |D^-1 r|_j` per RHS,
/// as plain sums and maxima over fixed chunks combined in chunk order.
fn step<const R: usize, M: Precondition<R>>(
    w: &Weights,
    work: &mut Work<R>,
    alpha: [f64; R],
    precond: &mut M,
) -> Result<[[f64; R]; 3]> {
    let m = work.x.len();
    let serial = m * R < PARALLEL_WORK;
    let inv_diag = &w.inverse_diagonal;
    let inv_root = &w.inverse_root_mass;
    let mut next = [0.0; R];
    let mut inf = [0.0_f64; R];
    let mut zinf = [0.0_f64; R];
    let Work {
        x,
        r,
        z,
        p,
        lanes,
        partials,
        ..
    } = work;
    let q = &lanes[0].acc;
    let body = |range: Range<usize>, x: &mut [[f64; R]], r: &mut [[f64; R]], z: &mut [[f64; R]]| {
        let mut s = [0.0; R];
        let mut h = [0.0_f64; R];
        let mut hz = [0.0_f64; R];
        for (k, j) in range.enumerate() {
            for c in 0..R {
                x[k][c] += alpha[c] * p[j][c];
                r[k][c] -= alpha[c] * q[j][c];
                z[k][c] = inv_diag[j] * r[k][c];
                if M::JACOBI {
                    s[c] += r[k][c] * z[k][c];
                }
                h[c] = h[c].max(r[k][c].abs() * inv_root[j]);
                hz[c] = hz[c].max(z[k][c].abs());
            }
        }
        [s, h, hz]
    };
    let mut fold = |[s, h, hz]: [[f64; R]; 3]| {
        for c in 0..R {
            next[c] += s[c];
            inf[c] = inf[c].max(h[c]);
            zinf[c] = zinf[c].max(hz[c]);
        }
    };
    if serial {
        fold(body(0..m, x, r, z));
    } else {
        x.par_chunks_mut(CHUNK)
            .zip(r.par_chunks_mut(CHUNK))
            .zip(z.par_chunks_mut(CHUNK))
            .zip(partials.par_iter_mut())
            .enumerate()
            .for_each(|(i, (((x, r), z), slot))| {
                let start = i * CHUNK;
                *slot = body(start..start + x.len(), x, r, z);
            });
        for &slot in &partials[..m.div_ceil(CHUNK)] {
            fold(slot);
        }
    }
    if !M::JACOBI {
        precond.correct(&work.r, &mut work.z)?;
        next = dot::<R>(&work.r, &work.z, &mut work.partials);
    }
    Ok([next, inf, zinf])
}

/// `p = z + beta p` on the columns of `mask`, zero elsewhere.
fn update_directions<const R: usize>(work: &mut Work<R>, mask: [bool; R], beta: [f64; R]) {
    let serial = work.p.len() * R < PARALLEL_WORK;
    let z = &work.z;
    let update = |p: &mut [[f64; R]], z: &[[f64; R]]| {
        for (p, z) in p.iter_mut().zip(z) {
            for c in 0..R {
                p[c] = if mask[c] { z[c] + beta[c] * p[c] } else { 0.0 };
            }
        }
    };
    if serial {
        update(&mut work.p, z);
    } else {
        work.p
            .par_chunks_mut(CHUNK)
            .zip(z.par_chunks(CHUNK))
            .for_each(|(p, z)| update(p, z));
    }
}

/// Take one step's measures: stop the columns at their goal, keep improved
/// iterates that meet the acceptance rule (in `xs`) and return `beta`;
/// `None` when a measure is not finite.
fn advance<const R: usize>(
    state: &mut Columns<R>,
    targets: &Targets<R>,
    work: &mut Work<R>,
    [next, inf, zinf]: [[f64; R]; 3],
) -> Option<[f64; R]> {
    let mut beta = [0.0; R];
    let mut save = [false; R];
    for c in 0..R {
        if !state.active[c] {
            continue;
        }
        if !(inf[c].is_finite() && next[c].is_finite() && zinf[c].is_finite()) {
            return None;
        }
        let current = targets.goal(inf[c], zinf[c], c);
        state.best[c] = state.best[c].min(current);
        if current <= 1.0 {
            state.active[c] = false;
        } else {
            beta[c] = next[c] / state.rz[c];
            if current < state.kept_measure[c] && targets.accepted(inf[c], zinf[c], c) {
                save[c] = true;
                state.kept[c] = true;
                state.kept_measure[c] = current;
            }
        }
        state.rz[c] = next[c];
    }
    if save.contains(&true) {
        copy_columns(&mut work.xs, &work.x, save);
    }
    Some(beta)
}

/// Lockstep preconditioned CG on `S x = r0` from `x = 0` (`from_zero`) or
/// from the current iterate, with `r0 = g - S x` already in `work.r`; counts
/// add to `tally`. A column stops when its largest scaled moment
/// `|r_j| / sqrt(m_j)` is at most its goal and, when a coefficient goal is
/// given, its largest Jacobi estimate `|r_j| / d_j` is at most that goal
/// (whatever the preconditioner). A column whose curvature vanishes, whose
/// progress stalls or that reaches the iteration ceiling returns to its best
/// iterate that met the acceptance rule; without one the route declines.
#[allow(clippy::too_many_arguments)]
pub(super) fn pcg<const R: usize, const NB: usize, const UNIT: bool, M: Precondition<R>>(
    st: &Structure,
    w: &Weights,
    work: &mut Work<R>,
    targets: &Targets<R>,
    cap: usize,
    cancel: &Cancel<'_>,
    precond: &mut M,
    from_zero: bool,
    tally: &mut Tally<R>,
) -> Result<Solve<R>> {
    let Some(mut state) = start(w, work, targets, precond, from_zero)? else {
        return Ok(Solve::Declined("nonfinite initial reduced residual"));
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
        apply::<R, NB, UNIT>(st, w, &work.p, &mut work.lanes);
        let pq = dot::<R>(&work.p, &work.lanes[0].acc, &mut work.partials);
        let mut alpha = [0.0; R];
        let mut flat = [false; R];
        for c in 0..R {
            if state.active[c] {
                if pq[c].is_finite() && pq[c] > 0.0 {
                    alpha[c] = state.rz[c] / pq[c];
                } else {
                    // No curvature left along the direction: the recurrence
                    // has reached its rounding floor.
                    flat[c] = true;
                }
            }
        }
        if !state.fall_back(flat, &mut work.x, &work.xs) {
            return Ok(Solve::Declined("nonpositive reduced curvature"));
        }
        if !state.active.contains(&true) {
            break;
        }
        let measures = step(w, work, alpha, precond)?;
        iterations += 1;
        tally.iterations += 1;
        let Some(beta) = advance(&mut state, targets, work, measures) else {
            return Ok(Solve::Declined("nonfinite reduced residual"));
        };
        if iterations % KEPT_WINDOW == 0 {
            // Little progress over the last window: fall back to the kept
            // iterate (short window), or leave hard graphs to the caller's
            // preconditioned solvers (long window).
            let long = iterations % STALL_WINDOW == 0;
            let mut stalled = [false; R];
            for (c, stalled) in stalled.iter_mut().enumerate() {
                let reference = if state.kept[c] {
                    state.kept_checkpoint[c]
                } else {
                    state.checkpoint_best[c]
                };
                *stalled =
                    state.active[c] && (state.kept[c] || long) && state.best[c] > 0.5 * reference;
            }
            if !state.fall_back(stalled, &mut work.x, &work.xs) {
                return Ok(Solve::Declined(STALLED));
            }
            state.kept_checkpoint = state.best;
            if long {
                state.checkpoint_best = state.best;
            }
        }
        update_directions(work, state.active, beta);
    }
    Ok(Solve::Converged {
        accepted: state.accepted,
        floor: state.floor,
    })
}
