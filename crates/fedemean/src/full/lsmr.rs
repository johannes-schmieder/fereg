//! Modified LSMR on `A = W^1/2 D` with right-hand side `W^1/2 y`
//! ([`crate::lsmr::mlsmr`]), over batches of up to [`BATCH`] columns in
//! lockstep.
//!
//! `mlsmr` needs `M ~ A'A = D'WD` as a nonsingular SPD action `M^-1`; see
//! the module contract in [`super`]. Its stopping tests (the residual and
//! normal-equation estimates and their exit audits) are proxies: each pass
//! is followed by the moment certificate on the formed residual over all
//! levels. A pass that does not certify restarts from its iterate (the true
//! residual is recomputed from the columns) at a tolerance tightened by the
//! certificate's shortfall, within the column's iteration budget. A column
//! is written only once it certifies.
//!
//! **Lockstep.** A round runs the batch's uncertified columns together
//! ([`crate::lsmr::mlsmr_columns`]): one pass of `D`, `D'` and `M^-1`
//! per iteration for the running columns, each column with its own
//! recurrence, tolerance, budget, stopping tests, exit audits and restarts
//! and `mlsmr`'s bits (this module's column applications give each column
//! the bits of a single one). A round of one column runs `mlsmr` itself. The
//! batch is the widest that fits the memory limit; see [`batch_width`].
//!
//! With escalation, the projection's first column runs alone. A Jacobi pass
//! whose normal-equation estimate fails to halve over a stall window, or
//! whose pilot predicts a slow pass, hands off (the engine's escalation
//! hook): the column continues with Schwarz from its iterate, the true
//! residual recomputed, within the rest of its budget, and the later rounds
//! and batches start on Schwarz. When the first column certifies on Jacobi,
//! the others run lockstep with escalation, and the first of them to hand
//! off takes the round's other running columns with it.

use super::blocks::Blocks;
use super::kernels::{adjoint, forward, load, residualize, Basis, Rows, ROW_BLOCK};
use super::pcg::{predicted_slow, PILOT};
use super::schwarz::{Refusal, Schwarz, Scratch};
use super::structure::Structure;
use super::{
    add_coefficients, certify, cold_start, ensure, filled, form_gram, initial_mode, load_start,
    lsmr_bytes, numerical, refill, report, reserve, same_projection, targets, untouched,
    with_capacity, with_structure, Job, Mode, Outputs,
};
use crate::certificate::MomentFold;
use crate::design::Effect;
use crate::elimination::pcg::Targets;
use crate::elimination::{BATCH, STALL_WINDOW, STOP_FRACTION};
use crate::error::{Error, Result};
use crate::lsmr::{
    mlsmr_columns, mlsmr_counted_cancellable, Cancellation, ColumnSolve, EscalationHandler,
    EscalationPolicy, LsmrFailure, LsmrResult, LsmrStopReason, MlsmrColumnsOptions, MlsmrOptions,
    Operator, Progress, SolveError,
};
use crate::projector::{
    CoefficientsMut, ColumnReport, ColumnStatus, ColumnsMut, Facts, InputState, Outcome,
    SchwarzSpec,
};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

/// Passes per column: the first and its certified restarts.
const MAX_PASSES: usize = 4;

/// `body` with the const `R` set to the lockstep width `k` (1 to [`BATCH`]).
macro_rules! lanes {
    ($k:expr, $r:ident => $body:expr) => {
        match $k {
            1 => {
                const $r: usize = 1;
                $body
            }
            2 => {
                const $r: usize = 2;
                $body
            }
            3 => {
                const $r: usize = 3;
                $body
            }
            4 => {
                const $r: usize = 4;
                $body
            }
            5 => {
                const $r: usize = 5;
                $body
            }
            6 => {
                const $r: usize = 6;
                $body
            }
            7 => {
                const $r: usize = 7;
                $body
            }
            8 => {
                const $r: usize = 8;
                $body
            }
            k => {
                return Err(SolveError::InvalidInput {
                    context: "fedemean",
                    message: format!("{k} lockstep columns"),
                })
            }
        }
    };
}

/// Hand off when the best normal-equation estimate of a pass fails to halve
/// over a stall window (the PCG rule), or when its rate after the pilot
/// iterations predicts a slow pass toward `goal` (the pass tolerance on that
/// estimate, which starts at 1); `slow` records the latter. A column that
/// hands off sets `together`, and the round's other columns hand off at
/// their next test.
struct Stall {
    goal: f64,
    slow: Arc<AtomicBool>,
    together: Arc<AtomicBool>,
}

struct StallRun {
    best: f64,
    checkpoint: f64,
    /// The best estimate at half the pilot.
    half: f64,
    goal: f64,
    slow: Arc<AtomicBool>,
    together: Arc<AtomicBool>,
}

impl EscalationPolicy for Stall {
    fn handler(&self) -> Box<dyn EscalationHandler> {
        Box::new(StallRun {
            best: 1.0,
            checkpoint: 1.0,
            half: 1.0,
            goal: self.goal,
            slow: Arc::clone(&self.slow),
            together: Arc::clone(&self.together),
        })
    }
}

impl StallRun {
    fn stalled_or_slow(&mut self, progress: Progress) -> bool {
        self.best = self.best.min(progress.normal_eq_residual);
        if progress.iteration == PILOT / 2 {
            self.half = self.best;
        }
        let window = (PILOT - PILOT / 2, PILOT);
        if progress.iteration == PILOT
            && predicted_slow(self.half / self.goal, self.best / self.goal, window)
        {
            self.slow.store(true, Ordering::Relaxed);
            return true;
        }
        if progress.iteration % STALL_WINDOW != 0 {
            return false;
        }
        let stalled = self.best.is_nan() || self.best > 0.5 * self.checkpoint;
        self.checkpoint = self.best;
        stalled
    }
}

impl EscalationHandler for StallRun {
    fn should_escalate(&mut self, progress: Progress) -> bool {
        if self.together.load(Ordering::Relaxed) {
            return true;
        }
        let escalate = self.stalled_or_slow(progress);
        if escalate {
            self.together.store(true, Ordering::Relaxed);
        }
        escalate
    }
}

/// `W^1/2 D` and its adjoint `D' W^1/2` (in the internal basis), also on
/// `k` interleaved columns with each column's bits.
struct Design<'a, 'd> {
    st: &'a Structure,
    effects: &'a [Effect<'d>],
    basis: Basis<'a>,
    roots: Option<&'a [f64]>,
    /// The scatter accumulators of the adjoint (applies run one at a time).
    scatter: Mutex<Vec<f64>>,
}

impl Operator for Design<'_, '_> {
    fn nrows(&self) -> usize {
        self.st.nobs
    }

    fn ncols(&self) -> usize {
        self.st.coefficients()
    }

    fn apply(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        forward::<1>(self.st, (self.effects, self.basis), x, self.roots, y);
        Ok(())
    }

    fn apply_adjoint(&self, u: &[f64], v: &mut [f64]) -> std::result::Result<(), SolveError> {
        self.apply_adjoint_columns(1, u, v)
    }

    fn apply_columns(
        &self,
        k: usize,
        x: &[f64],
        y: &mut [f64],
    ) -> std::result::Result<(), SolveError> {
        let design = (self.effects, self.basis);
        lanes!(k, R => forward::<R>(self.st, design, x, self.roots, y));
        Ok(())
    }

    fn apply_adjoint_columns(
        &self,
        k: usize,
        u: &[f64],
        v: &mut [f64],
    ) -> std::result::Result<(), SolveError> {
        let mut scatter = self.scatter.lock().unwrap_or_else(PoisonError::into_inner);
        let rows = Rows {
            data: u,
            stride: k,
            first: 0,
        };
        let design = (self.effects, self.basis);
        lanes!(k, R => adjoint::<R>(self.st, design, rows, self.roots, v, &mut scatter));
        Ok(())
    }
}

/// `M^-1`: Jacobi `diag(D'WD)^-1` (the inverse level masses, and the
/// channel blocks of the effects with slopes), the identity or additive
/// Schwarz; each symmetric positive definite on all of coefficient space.
struct Inverse<'a> {
    inverse_mass: &'a [f64],
    jacobi: bool,
    /// Schwarz with its apply workspace (applies run one at a time).
    schwarz: Option<(&'a Schwarz, Mutex<Scratch>)>,
    /// The channel blocks with their partials (applies run one at a time).
    blocks: Option<(&'a Blocks, &'a Structure, Mutex<Vec<f64>>)>,
    /// The channel blocks Schwarz adds (a design with slopes).
    slopes: Option<&'a Blocks>,
}

impl Operator for Inverse<'_> {
    fn nrows(&self) -> usize {
        self.inverse_mass.len()
    }

    fn ncols(&self) -> usize {
        self.inverse_mass.len()
    }

    fn apply(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        self.apply_columns(1, x, y)
    }

    fn apply_adjoint(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), SolveError> {
        self.apply(x, y)
    }

    fn apply_columns(
        &self,
        k: usize,
        x: &[f64],
        y: &mut [f64],
    ) -> std::result::Result<(), SolveError> {
        match (&self.schwarz, self.jacobi) {
            (Some((schwarz, scratch)), _) => {
                let mut scratch = scratch.lock().unwrap_or_else(PoisonError::into_inner);
                let inverse = (self.inverse_mass, self.slopes);
                lanes!(k, R => schwarz.apply::<R>(x, y, &mut scratch, inverse));
            }
            (None, true) => {
                for ((y, x), d) in y
                    .chunks_exact_mut(k)
                    .zip(x.chunks_exact(k))
                    .zip(self.inverse_mass)
                {
                    for (y, x) in y.iter_mut().zip(x) {
                        *y = x * d;
                    }
                }
                if let Some((blocks, st, partials)) = &self.blocks {
                    let mut partials = partials.lock().unwrap_or_else(PoisonError::into_inner);
                    lanes!(k, R => {
                        blocks.measure::<R>(&st.offsets, x, Some(y), &mut partials);
                    });
                }
            }
            (None, false) => y.copy_from_slice(x),
        }
        Ok(())
    }

    fn apply_adjoint_columns(
        &self,
        k: usize,
        x: &[f64],
        y: &mut [f64],
    ) -> std::result::Result<(), SolveError> {
        self.apply_columns(k, x, y)
    }
}

/// The widest batch (at most `ncols` and [`BATCH`]) whose solve fits the
/// memory limit beside the projection's other bytes and the Schwarz
/// preconditioner `schwarz` will apply with, with its bytes; one column
/// when none wider fits.
fn batch_width(job: &Job<'_, '_>, ncols: usize, schwarz: Option<&Schwarz>) -> (usize, u128) {
    let other = job.bytes - lsmr_bytes(job.st, 1);
    let bytes = |w: usize| other + lsmr_bytes(job.st, w);
    let fits = |w: usize| {
        let preconditioner = schwarz.map_or(0, |s| s.peak_bytes(w));
        job.request
            .memory_limit
            .is_none_or(|limit| bytes(w) + preconditioner <= limit)
    };
    let width = (2..=ncols.clamp(1, BATCH))
        .rev()
        .find(|&w| fits(w))
        .unwrap_or(1);
    (width, bytes(width))
}

/// One column's certified solve.
struct Solved {
    x: Vec<f64>,
    /// The original column's weighted norm (its reference scale).
    reference: f64,
    report: ColumnReport,
    fold: MomentFold,
}

/// Why a batch's solve stopped short of a certificate.
enum Short {
    Declined(&'static str),
    Failed(Error),
}

impl From<Error> for Short {
    fn from(error: Error) -> Self {
        Self::Failed(error)
    }
}

/// One column of a batch: its certificate scale and targets, its start, and
/// the iterations, pass and tolerance of its current preconditioner.
struct Lane {
    j: usize,
    scale: f64,
    targets: Targets<1>,
    start: Vec<f64>,
    spent: usize,
    pass: usize,
    tolerance: f64,
    solved: Option<Solved>,
}

/// The lockstep width of one projection's rounds and the bytes it reports.
struct Width {
    /// Columns per round; one after an escalation that only fits so.
    round: usize,
    /// Whether an escalating projection's pilot column has run.
    piloted: bool,
    /// The workspace at the projection's width and at one column.
    bytes: u128,
    narrow: u128,
    peak: u128,
}

/// Modified LSMR over batches of the columns; see the module docs. Also
/// reports whether the residual Gram was formed.
pub(super) fn project(
    job: &Job<'_, '_>,
    values: &mut ColumnsMut<'_>,
    start: Option<&[f64]>,
    mut coefficients: Option<CoefficientsMut<'_>>,
    outputs: Outputs<'_>,
    mut facts: Facts,
) -> Result<(Outcome, bool)> {
    let st = job.st;
    let ncols = values.ncols();
    let Outputs {
        work,
        reduced: _,
        schwarz: slot,
        gram,
        norms,
        columns,
        reference: formed,
    } = outputs;
    let started = Instant::now();
    let mut mode = match initial_mode(job, slot, 1, &mut facts)? {
        Ok(mode) => mode,
        Err(reason) => return Ok((untouched(reason, with_structure(facts, job)), false)),
    };
    let (width, bytes) = batch_width(job, ncols, slot.as_ref().filter(|_| mode == Mode::Schwarz));
    let narrow = job.bytes;
    let job = &Job { bytes, ..*job };
    let mut lockstep = Width {
        round: width,
        piloted: false,
        bytes,
        narrow,
        peak: bytes,
    };
    work.size(st, 1)?;
    work.size_blocks(job.state, 1)?;
    let tasks = job
        .state
        .blocks()
        .map_or(0, |blocks| blocks.tasks() * 3 * width);
    let scatter = st.scatter_blocks() * st.scatter_levels() * width;
    for (buffer, len) in [
        (&mut work.inverse_measures, tasks),
        (&mut work.scatter, scatter),
        (&mut work.b, st.nobs * width),
    ] {
        if buffer.len() != len {
            refill(buffer, len, 0.0)?;
        }
    }
    for buffer in [&mut *norms, &mut *formed] {
        buffer.clear();
        reserve(buffer, ncols)?;
    }
    columns.clear();
    reserve(columns, ncols)?;
    formed.resize(ncols, f64::NAN);
    facts.columns = ncols;
    let mut fold = MomentFold::default();
    let mut first = 0;
    while first < ncols {
        let w = (ncols - first).min(width);
        let batch = (&*values, first, w, start);
        let lent = (&mut *work, &mut *slot);
        let solved = match solve_batch(job, batch, lent, (&mut mode, &mut lockstep), &mut facts) {
            Ok(solved) => solved,
            Err(Short::Failed(error)) => return Err(error),
            Err(Short::Declined(reason)) => {
                let input = if first > 0 {
                    same_projection(formed)?
                } else {
                    InputState::Untouched
                };
                return Ok((numerical(reason, input, facts, job), false));
            }
        };
        let began = Instant::now();
        for (i, solved) in solved.into_iter().enumerate() {
            let j = first + i;
            let target = (values.as_mut_slice(), ncols, j);
            let weights = job.state.weights();
            residualize::<1>(
                st,
                (job.design.effects(), job.state.basis()),
                &solved.x,
                target,
                weights,
                &mut work.t[..st.nobs],
                &mut work.rows,
            );
            if let Some(coefficients) = coefficients.as_mut() {
                add_coefficients(
                    (st, (job.design.effects(), job.state.basis())),
                    &solved.x,
                    (1, 0),
                    coefficients,
                    j,
                );
            }
            formed[j] = solved.reference;
            fold.merge(&solved.fold);
            norms.push(solved.report.residual_norm);
            columns.push(solved.report);
        }
        facts.residual_seconds += began.elapsed().as_secs_f64();
        first += w;
    }
    let certificate = fold.finish(job.request.tolerance)?;
    if !certificate.passed {
        let input = same_projection(formed)?;
        return Ok((
            numerical("combined certificate failed", input, facts, job),
            false,
        ));
    }
    let gram_formed = form_gram(job, values, gram, work)?;
    let schwarz = slot.as_ref().filter(|_| mode == Mode::Schwarz);
    let mut report = report(
        job,
        (certificate, facts),
        started,
        (schwarz, lockstep.round),
    );
    report.workspace_bytes = report.workspace_bytes.max(lockstep.peak);
    Ok((Outcome::Projected(report), gram_formed))
}

/// The certificate of `x` on column `src` (without writing the column):
/// `t = W (y - D x)`, its moments over all levels in `work.q`, the fold at
/// `max(scale, ||y - D x||)` and the stopping measures `max |m_g| /
/// sqrt(mass_g)` and `max |m_g| / mass_g`.
fn check(
    job: &Job<'_, '_>,
    (src, x, scale): (Rows<'_>, &[f64], f64),
    work: &mut super::Work,
) -> (super::Certified<1>, f64, f64) {
    let (st, state) = (job.st, job.state);
    let effects = job.design.effects();
    let weights = state.weights();
    let stats = load::<1>(
        st,
        (effects, state.basis()),
        src,
        Some(x),
        weights,
        &mut work.t[..st.nobs],
        &mut work.rows,
    );
    let t = Rows {
        data: &work.t[..st.nobs],
        stride: 1,
        first: 0,
    };
    adjoint::<1>(
        st,
        (effects, state.basis()),
        t,
        None,
        &mut work.q,
        &mut work.scatter,
    );
    let tolerance = job.request.tolerance;
    let certified = certify::<1>(
        (st, effects, state),
        &work.q,
        [stats[2][0]],
        [scale],
        tolerance,
    );
    let (mut inf, mut zinf) = (0.0_f64, 0.0_f64);
    for ((m, ir), im) in work
        .q
        .iter()
        .zip(&state.inverse_root_mass)
        .zip(&state.inverse_mass)
    {
        inf = inf.max(m.abs() * ir);
        zinf = zinf.max(m.abs() * im);
    }
    if let Some(blocks) = state.blocks() {
        let [_, block_inf, block_zinf] =
            blocks.measure::<1>(&st.offsets, &work.q, None, &mut work.measures);
        inf = inf.max(block_inf[0]);
        zinf = zinf.max(block_zinf[0]);
    }
    (certified, inf, zinf)
}

/// Column `j` of `values` as rows.
fn source<'v>(values: &'v ColumnsMut<'_>, j: usize) -> Rows<'v> {
    Rows {
        data: values.as_slice(),
        stride: values.ncols(),
        first: j,
    }
}

/// Column `j`'s start from `warm` or cold, as PCG does: from the largest
/// effect's level means, with the goal at a tenth of the tolerance on the
/// starting residual's norm.
fn setup(
    job: &Job<'_, '_>,
    (values, j, warm): (&ColumnsMut<'_>, usize, Option<&[f64]>),
    work: &mut super::Work,
    facts: &mut Facts,
) -> std::result::Result<Lane, Short> {
    let (st, state) = (job.st, job.state);
    let effects = job.design.effects();
    let started = Instant::now();
    let src = source(values, j);
    let weights = state.weights();
    load::<1>(
        st,
        (effects, state.basis()),
        src,
        None,
        weights,
        &mut work.t[..st.nobs],
        &mut work.rows,
    );
    let mut start = filled(st.coefficients(), 0.0)?;
    match warm {
        Some(warm) => load_start::<1>((st, (effects, state.basis())), warm, 0, &mut start),
        None => {
            let t = Rows {
                data: &work.t[..st.nobs],
                stride: 1,
                first: 0,
            };
            adjoint::<1>(
                st,
                (effects, state.basis()),
                t,
                None,
                &mut work.q,
                &mut work.scatter,
            );
            cold_start::<1>(job, &work.q, &mut start);
        }
    }
    let stats = load::<1>(
        st,
        (effects, state.basis()),
        src,
        Some(&start),
        weights,
        &mut work.t[..st.nobs],
        &mut work.rows,
    );
    let Some((scale, targets)) = targets::<1>(job, &stats, None) else {
        return Err(Short::Declined("RHS scale outside the plain-sum range"));
    };
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    Ok(Lane {
        j,
        scale: scale[0],
        targets,
        start,
        spent: 0,
        pass: 0,
        tolerance: STOP_FRACTION * job.request.tolerance,
        solved: None,
    })
}

/// Solve and certify the batch `first..first + w`: rounds of its
/// uncertified columns (at most `lockstep.round` at once) until each
/// certifies. A round whose columns stalled escalates: they continue on
/// Schwarz (`mode` becomes Schwarz for the later rounds and batches).
fn solve_batch(
    job: &Job<'_, '_>,
    (values, first, w, start): (&ColumnsMut<'_>, usize, usize, Option<&[f64]>),
    (work, slot): (&mut super::Work, &mut Option<Schwarz>),
    (mode, lockstep): (&mut Mode, &mut Width),
    facts: &mut Facts,
) -> std::result::Result<Vec<Solved>, Short> {
    let m = job.st.coefficients();
    let mut lanes = with_capacity(w)?;
    for j in first..first + w {
        let warm = start.map(|start| &start[j * m..(j + 1) * m]);
        lanes.push(setup(job, (values, j, warm), work, facts)?);
    }
    let slow = Arc::new(AtomicBool::new(false));
    loop {
        let pending: Vec<usize> = (0..w).filter(|&i| lanes[i].solved.is_none()).collect();
        if pending.is_empty() {
            break;
        }
        // An escalating projection's first column runs alone: its pilot
        // decides the preconditioner of the later rounds at one column's
        // cost, as one column at a time did.
        let pilot = *mode == Mode::Escalate && !lockstep.piloted;
        let width = if pilot { 1 } else { lockstep.round };
        let run = &pending[..pending.len().min(width)];
        let lent = (&mut *work, &*slot);
        let stalled = round(job, (&mut lanes, run, values), lent, (*mode, &slow), facts)?;
        lockstep.piloted |= pilot;
        if stalled.is_empty() {
            continue;
        }
        let default = SchwarzSpec::default();
        let mut built = ensure(job, default, slot, lockstep.round, facts)?;
        if matches!(built, Err(Refusal::MemoryLimit)) && lockstep.round > 1 {
            // Schwarz beside one column at a time.
            let narrow = Job {
                bytes: lockstep.narrow,
                ..*job
            };
            built = ensure(&narrow, default, slot, 1, facts)?;
            if built.is_ok() {
                lockstep.round = 1;
            }
        }
        if built.is_ok() {
            facts.escalations += 1;
            *mode = Mode::Schwarz;
            let schwarz = slot.as_ref().map_or(0, |s| s.peak_bytes(lockstep.round));
            let held = if lockstep.round > 1 {
                lockstep.bytes
            } else {
                lockstep.narrow
            };
            lockstep.peak = lockstep.peak.max(held + schwarz);
        } else if slow.load(Ordering::Relaxed) {
            // A slow (not stalled) recurrence whose Schwarz does not fit
            // continues with Jacobi.
            *mode = Mode::Jacobi;
        } else {
            return Err(Short::Declined("stalled modified LSMR recurrence"));
        }
        for (i, x) in stalled {
            let lane = &mut lanes[i];
            lane.start = x;
            lane.pass = 0;
            lane.tolerance = STOP_FRACTION * job.request.tolerance;
        }
    }
    Ok(lanes
        .into_iter()
        .map(|lane| lane.solved.expect("every lane certified"))
        .collect())
}

/// `b = W^1/2 y` of the columns `js`, interleaved.
fn right_hand_sides(job: &Job<'_, '_>, values: &ColumnsMut<'_>, js: &[usize], b: &mut [f64]) {
    let (k, stride) = (js.len(), values.ncols());
    let data = values.as_slice();
    let roots = job.state.roots();
    let body = |block: usize, b: &mut [f64]| {
        for (r, row) in b.chunks_exact_mut(k).enumerate() {
            let i = block * ROW_BLOCK + r;
            for (b, &j) in row.iter_mut().zip(js) {
                let y = data[i * stride + j];
                *b = roots.map_or(y, |roots| roots[i] * y);
            }
        }
    };
    b.par_chunks_mut(ROW_BLOCK * k)
        .enumerate()
        .for_each(|(block, b)| body(block, b));
}

/// One lockstep round of the lanes `run` in `mode` from their starts, with
/// the operator and preconditioner of that mode (the workspaces lent from
/// `work` for their duration), then each column's [`settle`]. Returns the
/// columns that handed off (escalation), with their iterates.
fn round(
    job: &Job<'_, '_>,
    (lanes, run, values): (&mut [Lane], &[usize], &ColumnsMut<'_>),
    (work, slot): (&mut super::Work, &Option<Schwarz>),
    (mode, slow): (Mode, &Arc<AtomicBool>),
    facts: &mut Facts,
) -> std::result::Result<Vec<(usize, Vec<f64>)>, Short> {
    let (st, state) = (job.st, job.state);
    let k = run.len();
    let started = Instant::now();
    let js: Vec<usize> = run.iter().map(|&i| lanes[i].j).collect();
    right_hand_sides(job, values, &js, &mut work.b[..st.nobs * k]);
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    let schwarz = slot.as_ref().filter(|_| mode == Mode::Schwarz);
    let mut scratch = std::mem::take(&mut work.schwarz);
    if let Some(schwarz) = schwarz {
        schwarz.size(&mut scratch, k)?;
    }
    let operator = Design {
        st,
        effects: job.design.effects(),
        basis: state.basis(),
        roots: state.roots(),
        scatter: Mutex::new(std::mem::take(&mut work.scatter)),
    };
    let jacobi = mode != Mode::Identity;
    let partials = std::mem::take(&mut work.inverse_measures);
    let inverse = Inverse {
        inverse_mass: &state.inverse_mass,
        jacobi,
        schwarz: schwarz.map(|s| (s, Mutex::new(scratch))),
        blocks: state
            .blocks()
            .filter(|_| jacobi && schwarz.is_none())
            .map(|blocks| (blocks, st, Mutex::new(partials))),
        slopes: state.blocks(),
    };
    let together = Arc::new(AtomicBool::new(false));
    let stalls: Vec<Stall> = run
        .iter()
        .map(|&i| Stall {
            goal: lanes[i].tolerance,
            slow: Arc::clone(slow),
            together: Arc::clone(&together),
        })
        .collect();
    let escalate = mode == Mode::Escalate;
    let specs: Vec<ColumnSolve<'_>> = run
        .iter()
        .zip(&stalls)
        .map(|(&i, stall)| ColumnSolve {
            tol: lanes[i].tolerance,
            maxiter: job.cap.saturating_sub(lanes[i].spent),
            warm_start: Some(&lanes[i].start),
            escalation: escalate.then_some(stall as &dyn EscalationPolicy),
        })
        .collect();
    let poll = || job.cancel.0.is_some_and(|cancel| cancel());
    let cancellation = match job.cancel.0 {
        Some(_) => Cancellation::new(&poll),
        None => Cancellation::default(),
    };
    let started = Instant::now();
    let b = &work.b[..st.nobs * k];
    let results: Vec<std::result::Result<LsmrResult, LsmrFailure>> = if k == 1 {
        let spec = specs[0];
        let options = MlsmrOptions {
            warm_start: spec.warm_start,
            escalation: spec.escalation,
            local_size: None,
        };
        vec![mlsmr_counted_cancellable(
            &operator,
            b,
            &inverse,
            spec.tol,
            spec.maxiter,
            options,
            cancellation,
        )]
    } else {
        let options = MlsmrColumnsOptions::default();
        mlsmr_columns(&operator, b, &inverse, &specs, options, cancellation)
    };
    facts.pcg_seconds += started.elapsed().as_secs_f64();
    drop(specs);
    work.scatter = operator
        .scatter
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some((_, scratch)) = inverse.schwarz {
        work.schwarz = scratch.into_inner().unwrap_or_else(PoisonError::into_inner);
    }
    if let Some((_, _, partials)) = inverse.blocks {
        work.inverse_measures = partials
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
    }
    let mut stalled = Vec::new();
    for (&i, result) in run.iter().zip(results) {
        if let Some(x) = settle(job, (&mut lanes[i], result, values), work, facts)? {
            stalled.push((i, x));
        }
    }
    Ok(stalled)
}

/// One column's round result: its iterate handed back after an escalation
/// (`Some`), else its certificate: a column that meets its goal and the
/// certificate is solved; one that misses restarts from its iterate at a
/// tolerance tightened by the shortfall; the last pass (or an exhausted
/// budget) also succeeds at the acceptance rule.
fn settle(
    job: &Job<'_, '_>,
    (lane, result, values): (
        &mut Lane,
        std::result::Result<LsmrResult, LsmrFailure>,
        &ColumnsMut<'_>,
    ),
    work: &mut super::Work,
    facts: &mut Facts,
) -> std::result::Result<Option<Vec<f64>>, Short> {
    let result = match result {
        Ok(result) => result,
        Err(failure) => {
            facts.iterations = facts.iterations.max(lane.spent + failure.iterations);
            return Err(match failure.source {
                SolveError::Cancelled => Short::Failed(Error::Cancelled),
                SolveError::Allocation { .. } => Short::Failed(Error::OutOfMemory(
                    "unable to reserve the modified LSMR workspace",
                )),
                _ => Short::Declined("modified LSMR broke down"),
            });
        }
    };
    lane.spent += result.iterations;
    let budget = job.cap.saturating_sub(lane.spent);
    facts.iterations = facts.iterations.max(lane.spent);
    if result.stop_reason == LsmrStopReason::Escalated && budget > 0 {
        return Ok(Some(result.x));
    }
    let started = Instant::now();
    let src = source(values, lane.j);
    let (certified, inf, zinf) = check(job, (src, &result.x, lane.scale), work);
    facts.residual_seconds += started.elapsed().as_secs_f64();
    let measure = lane.targets.goal(inf, zinf, 0);
    let last = budget == 0 || lane.pass + 1 == MAX_PASSES;
    let status = if measure <= 1.0 {
        Some(ColumnStatus::Converged)
    } else if last && lane.targets.accepted(inf, zinf, 0) {
        Some(ColumnStatus::AcceptedAtFloor)
    } else {
        None
    };
    if let (Some(fold), Some(status)) = (certified.fold, status) {
        facts.column_iterations += lane.spent;
        facts.accepted += usize::from(status == ColumnStatus::AcceptedAtFloor);
        lane.solved = Some(Solved {
            x: result.x,
            reference: lane.scale,
            report: ColumnReport {
                relative_moment: certified.relative[0],
                scale: certified.scales[0],
                residual_norm: certified.norms[0],
                iterations: lane.spent,
                status,
            },
            fold,
        });
        return Ok(None);
    }
    if last {
        return Err(Short::Declined(
            "modified LSMR did not certify within its budget",
        ));
    }
    // Tighten by the goal's shortfall, at least tenfold.
    let factor = if measure.is_finite() && measure > 1.0 {
        (0.5 / measure).clamp(1e-4, 0.1)
    } else {
        0.1
    };
    lane.tolerance = (lane.tolerance * factor).max(f64::EPSILON);
    lane.pass += 1;
    lane.start = result.x;
    Ok(None)
}
