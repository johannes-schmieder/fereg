//! Exact global elimination of the largest categorical effect.
//!
//! An intercept-only design with effects `E_0..E_k` partitions its weighted
//! normal matrix as `[[D, C], [C', A]]`, with `D` the diagonal mass of the
//! effect that has the most levels. Those coefficients are eliminated exactly:
//! the remaining coefficients solve `S b = g` with `S = A - C' D^-1 C`, which
//! is applied implicitly. Each eliminated group gathers the remaining
//! coefficients of its distinct remaining-code cells, subtracts their weighted
//! group mean and scatters the weighted deviations back. Up to eight
//! right-hand sides advance in lockstep through one traversal, preconditioned
//! by the inverse diagonal of `S` or by block-Jacobi over the retained
//! effects ([`blocks`]). The eliminated
//! coefficients follow by back-substitution, and one observation pass forms
//! the residual while it recomputes every remaining level's true weighted
//! moment.
//!
//! Scope: an exact alternative solver for the same projection. The recurrence
//! stops at a tenth of the requested tolerance on the reduced residual, which
//! is the remaining blocks' moment vector, relative to the column's norm
//! about its eliminated group means. Acceptance uses the moments recomputed
//! from the formed residual with the usual certificate scale. Moments of the
//! eliminated levels vanish by construction up to rounding and are not
//! recomputed. A stall, an admission refusal or a numerical breakdown before
//! the residual pass returns the untouched input to the caller's route; a
//! failed certificate after it retries in place, since projecting a partially
//! residualized column yields the same residual. With the escalation
//! fallback, a Jacobi recurrence that stalls or reaches the iteration
//! ceiling first restarts with block-Jacobi from its current iterate on the
//! true residual `g - S x` (the columns are still untouched, so `g` is
//! formed again).
//!
//! Memory: every allocation is fallible. The engine keeps the geometry, the
//! weight state and one workspace per batch width, so repeated projections of
//! one width with changing weights allocate nothing.

use crate::certificate::{Certificate, MomentFold};
use crate::design::{Design, Members};
use crate::error::{Error, Result};
use crate::projector::{
    BlockSpec, Choice, CoefficientsMut, ColumnReport, ColumnStatus, ColumnsMut, Decline, Facts,
    Fallback, Gram, InputState, MethodUsed, Outcome, Preconditioner, PreconditionerUsed,
    ReductionUsed, Report, Request, Route,
};
use crate::weights::{Validated as ValidatedWeights, Weights as InputWeights};
use rayon::prelude::*;
use std::time::Instant;

/// Retained (non-eliminated) blocks with specialized kernels.
pub(crate) const MAX_BLOCKS: usize = 4;
/// Right-hand sides per lockstep batch.
pub(crate) const BATCH: usize = 8;
/// Below this many scalar updates a stage stays on the calling thread.
const PARALLEL_WORK: usize = 1 << 15;
/// Fixed reduction chunk for reduced-space vector operations, independent of
/// the worker count.
const CHUNK: usize = 2048;
/// Iteration ceiling before the route yields to the caller's solver.
const MAX_ITERATIONS: usize = 1500;
/// A column must halve its stopping measure every this many iterations ...
const STALL_WINDOW: usize = 40;
/// ... and every this many once it meets the acceptance rule.
const KEPT_WINDOW: usize = 8;
/// The recurrence stops at this fraction of the requested tolerance on the
/// column's within norm (see [`run`]).
const STOP_FRACTION: f64 = 0.1;
/// An escalated recurrence (a hard graph, restarted from an iterate that
/// differs across worker counts at rounding level) stops at this fraction of
/// the ordinary goal, so its results agree across worker counts well inside
/// the tolerance. The acceptance rule is unchanged.
const ESCALATED_GOAL_FRACTION: f64 = 0.1;
/// Columns whose within norm is below this fraction of the certificate scale
/// lie inside the eliminated span.
const WITHIN_FLOOR: f64 = 1.0 / (1_u64 << 30) as f64;
/// Acceptance rule: this fraction of the tolerance at the certificate scale
/// (where a column may stop when its recurrence reaches its rounding floor).
const ACCEPT_FRACTION: f64 = 0.5;

pub(crate) mod block;
mod blocks;
mod dense;
mod kernels;
mod passes;
mod pcg;
pub(crate) mod structure;
mod weights;

use blocks::Blocks;
use kernels::{apply, subtract, Ctx, Slotted, Work, WorkSlots};
use passes::{recover, residualize, rhs, RhsNorms};
use pcg::{pcg, Jacobi, Solve, Tally, Targets, CEILING, STALLED};
use structure::{filled, reserve, with_capacity, Structure};
use weights::{validate_keyed, WeightKey, Weights};

/// The caller's cancellation poll.
#[derive(Clone, Copy)]
pub(crate) struct Cancel<'a>(pub(crate) Option<&'a (dyn Fn() -> bool + Sync)>);

impl Cancel<'_> {
    #[inline]
    pub(crate) fn check(&self) -> Result<()> {
        if self.0.is_some_and(|cancel| cancel()) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// One design generation's retained geometry, its current-weight state and
/// the reusable workspaces and outputs of its projections.
#[derive(Default)]
pub(crate) struct Engine {
    refused: bool,
    structure: Option<Structure>,
    /// Weights keyed by the borrowed weight slice (address, length, digest);
    /// `None` key is unit weights.
    weights: Option<(WeightKey, Weights)>,
    /// Counts the weight states prepared (the blocks' build key).
    generation: u64,
    blocks: Blocks,
    work: WorkSlots,
    gram: Gram,
    gram_ready: bool,
    /// Residual norms of the last certified projection.
    norms: Vec<f64>,
    /// Per-column reports of the last certified projection.
    columns: Vec<ColumnReport>,
    /// Certificate scales of formed columns (NaN while untouched).
    reference: Vec<f64>,
}

/// What one projection needs from its projector.
pub(crate) struct Call<'c, 'd> {
    pub(crate) design: &'d Design<'d>,
    pub(crate) members: Option<Members<'d>>,
    pub(crate) width: usize,
    pub(crate) choice: Choice,
    pub(crate) cancel: Cancel<'c>,
}

impl Engine {
    pub(crate) fn residual_gram(&self) -> Option<&Gram> {
        self.gram_ready.then_some(&self.gram)
    }

    pub(crate) fn residual_norms(&self) -> &[f64] {
        &self.norms
    }

    pub(crate) fn columns(&self) -> &[ColumnReport] {
        &self.columns
    }

    pub(crate) fn structure(&self) -> Option<&Structure> {
        self.structure.as_ref()
    }

    /// Build the geometry (again, when the lane width changed); `false` when
    /// the design is outside the route, which is remembered.
    pub(crate) fn prepare(&mut self, call: &Call<'_, '_>) -> Result<bool> {
        if self.refused {
            return Ok(false);
        }
        if self
            .structure
            .as_ref()
            .is_some_and(|st| st.width == call.width)
        {
            return Ok(true);
        }
        self.structure = None;
        self.weights = None;
        match Structure::build(call.design, call.members, call.width, &call.cancel)? {
            Some(st) => {
                self.structure = Some(st);
                Ok(true)
            }
            None => {
                self.refused = true;
                Ok(false)
            }
        }
    }

    /// Project with the retained geometry, building it on first use. Any
    /// refusal is remembered so later calls decline at once.
    pub(crate) fn project(
        &mut self,
        call: &Call<'_, '_>,
        weights: InputWeights<'_>,
        mut values: ColumnsMut<'_>,
        coefficients: Option<CoefficientsMut<'_>>,
        request: &Request,
    ) -> Result<Outcome> {
        self.gram_ready = false;
        self.norms.clear();
        self.columns.clear();
        let mut facts = Facts::default();
        if self.refused {
            return Ok(untouched(Decline::OutsideRoute, facts));
        }
        if values.nobs() != call.design.nobs() {
            return Ok(untouched(Decline::Shape, facts));
        }
        let started = Instant::now();
        if !self.prepare(call)? {
            return Ok(untouched(Decline::OutsideRoute, facts));
        }
        facts.structure_seconds = started.elapsed().as_secs_f64();
        let started = Instant::now();
        let st = self.structure.as_ref().expect("structure");
        let members = call.members.expect("structure has members");
        let (validated, key) = validate_keyed(weights, values.nobs())?;
        if request.memory_limit.is_some_and(|limit| {
            admission_bytes(st, None, values.ncols()) + explicit_block_bytes(st, call.choice)
                > limit
        }) {
            return Ok(untouched(Decline::MemoryLimit, facts));
        }
        if self.weights.as_ref().is_none_or(|(k, _)| *k != key) {
            let recycled = self.weights.take().map(|(_, w)| w);
            self.generation += 1;
            match Weights::prepare(st, members, validated, recycled)? {
                Some(w) => self.weights = Some((key, w)),
                None => return Ok(untouched(Decline::Weights, facts)),
            }
        }
        facts.weights_seconds = started.elapsed().as_secs_f64();
        let w = &self.weights.as_ref().expect("weights").1;
        let mut bytes = admission_bytes(st, Some(w), values.ncols());
        if coefficients.is_some() {
            bytes += (st.reduced + st.segments) as u128 * values.ncols() as u128 * 8;
        }
        let outputs = Outputs {
            slots: &mut self.work,
            blocks: &mut self.blocks,
            gram: request.residual_gram.then_some(&mut self.gram),
            norms: &mut self.norms,
            columns: &mut self.columns,
            reference: &mut self.reference,
        };
        let job = Job {
            st,
            w,
            design: call.design,
            members,
            weights: validated,
            bytes,
            request,
            choice: call.choice,
            generation: self.generation,
            cancel: call.cancel,
        };
        let (outcome, gram) = project(&job, &mut values, coefficients, outputs, facts)?;
        match &outcome {
            Outcome::Projected(_) => self.gram_ready = gram,
            Outcome::Declined { .. } => {
                self.norms.clear();
                self.columns.clear();
            }
        }
        Ok(outcome)
    }

    /// Recompute every level's weighted moment of one residual column in one
    /// observation pass and certify it at `amplification` times its weighted
    /// norm. `None` when this engine did not project with these weights.
    pub(crate) fn certify_residual(
        &self,
        design: &Design<'_>,
        weights: InputWeights<'_>,
        column: &[f64],
        tolerance: f64,
        amplification: f64,
    ) -> Result<Option<Certificate>> {
        let (Some(st), Some((key, w))) = (self.structure.as_ref(), self.weights.as_ref()) else {
            return Ok(None);
        };
        if column.len() != st.nobs || design.nobs() != st.nobs {
            return Ok(None);
        }
        let (validated, current) = validate_keyed(weights, column.len())?;
        if *key != current {
            return Ok(None);
        }
        if !(amplification.is_finite() && amplification >= 1.0) {
            return Err(Error::InvalidInput(
                "derived residual amplification is invalid",
            ));
        }
        let elim_codes = design.effects()[st.eliminated].codes();
        let mut block_codes: Vec<&[u32]> = with_capacity(st.blocks.len())?;
        block_codes.extend(st.blocks.iter().map(|&b| design.effects()[b].codes()));
        let data = column;
        let raw = validated.values;
        let scale = validated.raw_scale;
        struct Part {
            remaining: Vec<f64>,
            eliminated: Vec<f64>,
            sq: f64,
            finite: bool,
        }
        let mut parts: Vec<Part> = with_capacity(st.rows.len())?;
        for _ in &st.rows {
            parts.push(Part {
                remaining: filled(st.reduced, 0.0)?,
                eliminated: filled(st.segments, 0.0)?,
                sq: 0.0,
                finite: true,
            });
        }
        parts
            .par_iter_mut()
            .zip(st.rows.par_iter())
            .for_each(|(part, rows)| {
                for row in rows.clone() {
                    let r = data[row];
                    part.finite &= r.is_finite();
                    let wr = raw.map_or(r, |v| v[row] / scale * r);
                    part.sq += wr * r;
                    part.eliminated[elim_codes[row] as usize] += wr;
                    for (codes, &offset) in block_codes.iter().zip(&st.offsets) {
                        part.remaining[(codes[row] + offset) as usize] += wr;
                    }
                }
            });
        let (total, rest) = parts.split_first_mut().expect("one row lane");
        for part in rest.iter() {
            for (a, b) in total.remaining.iter_mut().zip(&part.remaining) {
                *a += b;
            }
            for (a, b) in total.eliminated.iter_mut().zip(&part.eliminated) {
                *a += b;
            }
            total.sq += part.sq;
            total.finite &= part.finite;
        }
        if !total.finite || !total.sq.is_finite() {
            return Err(Error::InvalidData("derived residual values must be finite"));
        }
        let reference = amplification * total.sq.sqrt();
        if !reference.is_finite() {
            return Err(Error::InvalidData("derived residual scale is not finite"));
        }
        let scales = [reference];
        let mut rhs_inf = [0.0];
        let mut fold = MomentFold::default();
        for (s, &moment) in total.eliminated.iter().enumerate() {
            let mass = 1.0 / w.seg_inverse[s];
            fold.level(mass, &[moment / mass], &scales, &mut rhs_inf, w.raw_scale)?;
        }
        for (level, &moment) in total.remaining.iter().enumerate() {
            let mass = w.mass[level];
            fold.level(mass, &[moment / mass], &scales, &mut rhs_inf, w.raw_scale)?;
        }
        fold.finish(tolerance).map(Some)
    }
}

fn untouched(reason: Decline, facts: Facts) -> Outcome {
    Outcome::Declined {
        reason,
        input: InputState::Untouched,
        facts,
    }
}

/// Bytes the solve adds to the retained structure and weights.
fn solve_bytes(st: &Structure, nrhs: usize) -> u128 {
    let r = nrhs.min(BATCH) as u128;
    let m = st.reduced as u128;
    let lanes = st.lanes.len().max(st.rows.len()).max(st.active_lanes.len()) as u128;
    let scratch: u128 = st.active_lanes.iter().map(|&(_, c)| c as u128).sum();
    let partials = 3 * m.div_ceil(CHUNK as u128).max(1);
    8 * r * (6 * m + lanes * m + scratch + st.segments as u128 + partials)
}

/// Bytes of the blocks of an explicit [`Preconditioner::BlockJacobi`].
pub(crate) fn explicit_block_bytes(st: &Structure, choice: Choice) -> u128 {
    match choice.preconditioner {
        Preconditioner::BlockJacobi(spec) => blocks::bound(st, &spec),
        _ => 0,
    }
}

pub(crate) fn admission_bytes(st: &Structure, weights: Option<&Weights>, nrhs: usize) -> u128 {
    let weight_bytes = weights.map_or_else(
        || {
            let m = st.reduced as u128;
            let per_lane = if st.blocks.len() == 1 {
                3 * m * 8
            } else {
                3 * m * 8 + m * 4 + (st.max_seg_cells * st.blocks.len()) as u128 * 4
            };
            (st.cells() as u128 + st.segments as u128 + 5 * m) * 8
                + st.lanes.len() as u128 * per_lane
        },
        |w| w.bytes,
    );
    st.bytes + weight_bytes + solve_bytes(st, nrhs)
}

/// The fixed inputs of one projection.
struct Job<'j, 'c> {
    st: &'j Structure,
    w: &'j Weights,
    design: &'j Design<'j>,
    members: Members<'j>,
    weights: ValidatedWeights<'j>,
    bytes: u128,
    request: &'j Request,
    choice: Choice,
    /// The weight state's generation (the blocks' build key).
    generation: u64,
    cancel: Cancel<'c>,
}

/// The engine's reusable outputs and workspaces lent to one projection.
struct Outputs<'c> {
    slots: &'c mut WorkSlots,
    blocks: &'c mut Blocks,
    /// Requested residual Gram (formed only for at most [`BATCH`] columns).
    gram: Option<&'c mut Gram>,
    /// Residual norms, one per column.
    norms: &'c mut Vec<f64>,
    columns: &'c mut Vec<ColumnReport>,
    reference: &'c mut Vec<f64>,
}

#[allow(clippy::large_enum_variant)]
enum Step {
    /// Declined before any residual was formed.
    Declined(&'static str),
    /// The residual of this batch was formed in place (inline: repeated
    /// projections allocate nothing).
    Formed(Formed),
}

struct Formed {
    certified: Option<MomentFold>,
    norms: [f64; BATCH],
    reference: [f64; BATCH],
    relative: [f64; BATCH],
    scales: [f64; BATCH],
    iterations: [usize; BATCH],
    floor: [bool; BATCH],
    /// Whether the residual Gram was formed.
    gram: bool,
    /// Whether the recurrence used the blocks (explicitly or escalated).
    blocks: bool,
}

/// How one batch preconditions its recurrence.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Jacobi,
    /// Jacobi, escalating to the blocks when the recurrence stalls or
    /// reaches the iteration ceiling.
    Escalate,
    Blocks,
}

/// The fixed choices of one projection's batches.
struct Plan<'p> {
    tolerance: f64,
    cap: usize,
    cancel: Cancel<'p>,
    /// The blocks of [`Mode::Blocks`] and of escalations.
    spec: BlockSpec,
    /// The weight state's generation (the blocks' build key).
    generation: u64,
    /// The blocks are built only within this workspace size ...
    memory_limit: Option<u128>,
    /// ... beyond the projection's own workspace.
    bytes: u128,
}

/// One batch `first..first+R` of the columns and its outputs.
struct Batch<'b, 'v, 'c> {
    values: &'b mut ColumnsMut<'v>,
    first: usize,
    reference: Option<&'b [f64]>,
    coefficients: Option<&'b mut CoefficientsMut<'c>>,
    gram: Option<&'b mut Gram>,
    mode: Mode,
}

/// The engine's solve state lent to one batch.
struct Lent<'l> {
    slots: &'l mut WorkSlots,
    blocks: &'l mut Blocks,
    facts: &'l mut Facts,
}

/// One batch: RHS and recurrence read the columns; only a converged
/// recurrence forms the residual in place.
fn run<const R: usize, const NB: usize, const UNIT: bool, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    plan: &Plan<'_>,
    batch: Batch<'_, '_, '_>,
    lent: Lent<'_>,
) -> Result<Step>
where
    Work<R>: Slotted,
{
    let st = ctx.st;
    let Lent {
        slots,
        blocks,
        facts,
    } = lent;
    let slot = <Work<R> as Slotted>::slot(slots);
    if slot.as_ref().is_none_or(|work| !work.fits(st)) {
        *slot = None;
        *slot = Some(Work::<R>::new(st)?);
    }
    let work = slot.as_mut().expect("workspace");
    let started = Instant::now();
    let norms = rhs::<R, NB, WEIGHTED>(ctx, batch.values, batch.first, work);
    std::mem::swap(&mut work.r, &mut work.lanes[0].acc);
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    let Some((scale, targets)) = targets::<R, NB>(ctx, &norms, batch.reference, plan.tolerance)
    else {
        return Ok(Step::Declined("RHS scale outside the plain-sum range"));
    };
    let mut tally = Tally::default();
    let (solved, used) = solve::<R, NB, UNIT, WEIGHTED>(
        ctx,
        plan,
        (batch.values, batch.first, batch.mode),
        work,
        blocks,
        &targets,
        &mut tally,
        facts,
    )?;
    // Also a declined recurrence reports the iterations it spent.
    facts.iterations = facts.iterations.max(tally.iterations);
    let floor = match solved {
        Solve::Converged { accepted, floor } => {
            facts.column_iterations += tally.column_iterations;
            facts.accepted += accepted;
            floor
        }
        Solve::Declined(reason) => return Ok(Step::Declined(reason)),
    };
    let started = Instant::now();
    recover::<R, NB, UNIT>(st, ctx.w, work);
    if let Some(coefficients) = batch.coefficients {
        add_coefficients::<R>(st, work, coefficients, batch.first);
    }
    let formed = batch.gram.is_some();
    let sq = residualize::<R, NB, WEIGHTED>(ctx, batch.values, batch.first, work, batch.gram)?;
    let certified = certify_batch::<R>(ctx.w, &work.lanes[0].acc, sq, scale, plan.tolerance);
    facts.residual_seconds += started.elapsed().as_secs_f64();
    let mut out = Formed {
        certified: certified.fold,
        norms: [0.0; BATCH],
        reference: [0.0; BATCH],
        relative: [0.0; BATCH],
        scales: [0.0; BATCH],
        iterations: [0; BATCH],
        floor: [false; BATCH],
        gram: formed,
        blocks: used,
    };
    out.norms[..R].copy_from_slice(&certified.norms);
    out.reference[..R].copy_from_slice(&scale);
    out.relative[..R].copy_from_slice(&certified.relative);
    out.scales[..R].copy_from_slice(&certified.scales);
    out.iterations[..R].copy_from_slice(&tally.per_column);
    out.floor[..R].copy_from_slice(&floor);
    Ok(Step::Formed(out))
}

/// Certificate scales and stopping targets of one batch; `None` when a
/// column's scale is outside the plain-sum range.
fn targets<const R: usize, const NB: usize>(
    ctx: &Ctx<'_, '_, NB>,
    norms: &RhsNorms<R>,
    reference: Option<&[f64]>,
    tolerance: f64,
) -> Option<([f64; R], Targets<R>)> {
    let mut scale = [0.0; R];
    let mut within = [0.0; R];
    for j in 0..R {
        let norm = norms.sq[j].sqrt();
        if !norm.is_finite() || (norms.big[j] > 0.0 && !(1e-150..=1e150).contains(&norm)) {
            return None;
        }
        scale[j] = reference.map_or(norm, |r| r[j]);
        // The reduced problem sees only the column without its eliminated
        // group means, so its accuracy is set at that norm (a large common
        // level would otherwise loosen it). A column inside the eliminated
        // span has only rounding left there and keeps the acceptance rule.
        let centered = norms.centered[j].max(0.0).sqrt().min(scale[j]);
        within[j] = if centered >= WITHIN_FLOOR * scale[j] {
            centered
        } else {
            f64::NAN
        };
    }
    // Coefficient stopping proxy: Jacobi estimates within the tolerance of
    // the column's root-mean-square magnitude.
    let root_total = ctx.w.total_mass.sqrt();
    let mut targets = Targets {
        goal: [0.0; R],
        goal_coefficient: ctx.coefficient_accuracy.then_some([0.0; R]),
        accept: [0.0; R],
        accept_coefficient: ctx.coefficient_accuracy.then_some([0.0; R]),
    };
    for j in 0..R {
        targets.accept[j] = ACCEPT_FRACTION * tolerance * scale[j];
        targets.goal[j] = if within[j].is_nan() {
            targets.accept[j]
        } else {
            STOP_FRACTION * tolerance * within[j]
        };
        if let Some(t) = targets.accept_coefficient.as_mut() {
            t[j] = targets.accept[j] / root_total;
        }
        if let Some(t) = targets.goal_coefficient.as_mut() {
            t[j] = targets.goal[j] / root_total;
        }
    }
    Some((scale, targets))
}

/// The recurrence of one batch in its mode. An escalating batch whose Jacobi
/// recurrence stalls or reaches the ceiling (never one that breaks down)
/// continues with the blocks from its current iterate, when they are built
/// within the memory limit. Also reports whether the blocks were used.
#[allow(clippy::too_many_arguments)]
fn solve<const R: usize, const NB: usize, const UNIT: bool, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    plan: &Plan<'_>,
    (values, first, mode): (&ColumnsMut<'_>, usize, Mode),
    work: &mut Work<R>,
    blocks: &mut Blocks,
    targets: &Targets<R>,
    tally: &mut Tally<R>,
    facts: &mut Facts,
) -> Result<(Solve<R>, bool)> {
    let (st, w) = (ctx.st, ctx.w);
    let (cap, cancel) = (plan.cap, &plan.cancel);
    if mode == Mode::Blocks {
        // Explicit blocks are built before the first batch, and the
        // corrections of an escalated batch keep the escalation's blocks.
        let started = Instant::now();
        let solved =
            pcg::<R, NB, UNIT, Blocks>(st, w, work, targets, cap, cancel, blocks, true, tally)?;
        facts.pcg_seconds += started.elapsed().as_secs_f64();
        return Ok((solved, true));
    }
    let started = Instant::now();
    let before = tally.iterations;
    let solved =
        pcg::<R, NB, UNIT, Jacobi>(st, w, work, targets, cap, cancel, &mut Jacobi, true, tally)?;
    facts.pcg_seconds += started.elapsed().as_secs_f64();
    // The escalated recurrence spends only what the Jacobi recurrence left
    // of the caller's iteration budget.
    let remaining = cap.saturating_sub(tally.iterations - before);
    let escalate = mode == Mode::Escalate
        && remaining > 0
        && matches!(solved, Solve::Declined(STALLED | CEILING));
    if !escalate || !build(st, w, plan, R, blocks, facts)? {
        return Ok((solved, false));
    }
    // A fresh recurrence from the current iterate on the true residual
    // `g - S x`, with `g` formed again from the untouched columns.
    let started = Instant::now();
    rhs::<R, NB, WEIGHTED>(ctx, values, first, work);
    std::mem::swap(&mut work.r, &mut work.lanes[0].acc);
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    let started = Instant::now();
    apply::<R, NB, UNIT>(st, w, &work.x, &mut work.lanes);
    subtract(&mut work.r, &work.lanes[0].acc);
    let tight = targets.tightened(ESCALATED_GOAL_FRACTION);
    let solved =
        pcg::<R, NB, UNIT, Blocks>(st, w, work, &tight, remaining, cancel, blocks, false, tally)?;
    facts.pcg_seconds += started.elapsed().as_secs_f64();
    facts.escalations += 1;
    Ok((solved, true))
}

/// Build the blocks for the current weight state unless they are current,
/// with CMG applications of up to `lanes` lanes per call, within the bytes
/// the memory limit leaves beyond the projection's workspace; `false` when
/// they do not fit.
fn build(
    st: &Structure,
    w: &Weights,
    plan: &Plan<'_>,
    lanes: usize,
    blocks: &mut Blocks,
    facts: &mut Facts,
) -> Result<bool> {
    let budget = match plan.memory_limit {
        Some(limit) => match limit.checked_sub(plan.bytes) {
            Some(budget) => Some(budget),
            None => return Ok(false),
        },
        None => None,
    };
    let build = blocks::Build {
        spec: plan.spec,
        generation: plan.generation,
        lanes,
        budget,
    };
    let started = Instant::now();
    let built = blocks.ensure(st, w, build, &plan.cancel)?;
    facts.block_seconds += started.elapsed().as_secs_f64();
    Ok(built)
}

/// Add the batch's coefficient increments. Corrections add their increments
/// to the coefficients of the preceding solve, so the sum fits the original
/// columns.
fn add_coefficients<const R: usize>(
    st: &Structure,
    work: &Work<R>,
    batch: &mut CoefficientsMut<'_>,
    first: usize,
) {
    let per = batch.layout.per_rhs;
    for j in 0..R {
        let base = (first + j) * per;
        let start = base + batch.layout.effect_offsets[st.eliminated];
        for (value, alpha) in batch.values[start..start + st.segments]
            .iter_mut()
            .zip(&work.seg)
        {
            *value += alpha[j];
        }
        for (b, &effect) in st.blocks.iter().enumerate() {
            let start = base + batch.layout.effect_offsets[effect];
            let levels = batch.layout.level_counts[effect];
            let offset = st.offsets[b] as usize;
            for (value, x) in batch.values[start..start + levels]
                .iter_mut()
                .zip(&work.x[offset..offset + levels])
            {
                *value += x[j];
            }
        }
    }
}

struct Certified<const R: usize> {
    /// The batch's fold when it passed.
    fold: Option<MomentFold>,
    norms: [f64; R],
    relative: [f64; R],
    scales: [f64; R],
}

/// Fold the recomputed remaining-level moments at `max(reference, ||r||)`.
fn certify_batch<const R: usize>(
    w: &Weights,
    moments: &[[f64; R]],
    sq: [f64; R],
    reference: [f64; R],
    tolerance: f64,
) -> Certified<R> {
    let mut fold = MomentFold::default();
    let mut scales = [0.0; R];
    let mut norms = [0.0; R];
    let mut rhs_inf = [0.0; R];
    for j in 0..R {
        norms[j] = sq[j].sqrt();
        scales[j] = reference[j].max(norms[j]);
        if !scales[j].is_finite() {
            return Certified {
                fold: None,
                norms,
                relative: rhs_inf,
                scales,
            };
        }
    }
    let mut means = [0.0; R];
    for (level, moment) in moments.iter().enumerate() {
        let mass = w.mass[level];
        let inverse = 1.0 / mass;
        for j in 0..R {
            means[j] = moment[j] * inverse;
        }
        if fold
            .level(mass, &means, &scales, &mut rhs_inf, w.raw_scale)
            .is_err()
        {
            return Certified {
                fold: None,
                norms,
                relative: rhs_inf,
                scales,
            };
        }
    }
    let passed = fold.relative_infinity_norm <= tolerance;
    Certified {
        fold: passed.then_some(fold),
        norms,
        relative: rhs_inf,
        scales,
    }
}

fn dispatch<const NB: usize>(
    ctx: &Ctx<'_, '_, NB>,
    plan: &Plan<'_>,
    width: usize,
    batch: Batch<'_, '_, '_>,
    lent: Lent<'_>,
) -> Result<Step> {
    let unit = ctx.w.cell.is_empty();
    let weighted = ctx.weights.values.is_some();
    macro_rules! go {
        ($r:literal) => {
            match (unit, weighted) {
                (true, false) => run::<$r, NB, true, false>(ctx, plan, batch, lent),
                (true, true) => run::<$r, NB, true, true>(ctx, plan, batch, lent),
                (false, false) => run::<$r, NB, false, false>(ctx, plan, batch, lent),
                (false, true) => run::<$r, NB, false, true>(ctx, plan, batch, lent),
            }
        };
    }
    match width {
        1 => go!(1),
        2 => go!(2),
        3 => go!(3),
        4 => go!(4),
        5 => go!(5),
        6 => go!(6),
        7 => go!(7),
        8 => go!(8),
        _ => Err(Error::InvalidInput(
            "block-elimination batch width is invalid",
        )),
    }
}

/// Project the observation-major columns `values` against the design. A
/// decline leaves the columns untouched, unless a later batch or a
/// correction declined after earlier residuals were formed, whose projection
/// is then still the request ([`InputState::SameProjection`]). Also reports
/// whether the residual Gram was formed.
fn project(
    job: &Job<'_, '_>,
    values: &mut ColumnsMut<'_>,
    coefficients: Option<CoefficientsMut<'_>>,
    outputs: Outputs<'_>,
    facts: Facts,
) -> Result<(Outcome, bool)> {
    let st = job.st;
    if values.nobs() != st.nobs || values.ncols() == 0 || job.design.nobs() != st.nobs {
        return Ok((untouched(Decline::Shape, facts), false));
    }
    match st.blocks.len() {
        1 => project_nb::<1>(job, values, coefficients, outputs, facts),
        2 => project_nb::<2>(job, values, coefficients, outputs, facts),
        3 => project_nb::<3>(job, values, coefficients, outputs, facts),
        4 => project_nb::<4>(job, values, coefficients, outputs, facts),
        _ => Ok((untouched(Decline::OutsideRoute, facts), false)),
    }
}

/// The batches' fixed choices and the initial mode of every batch.
fn plan_of<'p>(job: &Job<'_, 'p>) -> (Plan<'p>, Mode) {
    let choice = job.choice;
    let mode = match (choice.preconditioner, choice.fallback) {
        (Preconditioner::BlockJacobi(_), _) => Mode::Blocks,
        (Preconditioner::Auto, Fallback::Escalate) => Mode::Escalate,
        _ => Mode::Jacobi,
    };
    let spec = match choice.preconditioner {
        Preconditioner::BlockJacobi(spec) => spec,
        _ => BlockSpec::default(),
    };
    let plan = Plan {
        tolerance: job.request.tolerance,
        cap: job.request.max_iterations.clamp(1, MAX_ITERATIONS),
        cancel: job.cancel,
        spec,
        generation: job.generation,
        memory_limit: job.request.memory_limit,
        bytes: job.bytes,
    };
    (plan, mode)
}

fn project_nb<const NB: usize>(
    job: &Job<'_, '_>,
    values: &mut ColumnsMut<'_>,
    mut coefficients: Option<CoefficientsMut<'_>>,
    outputs: Outputs<'_>,
    mut facts: Facts,
) -> Result<(Outcome, bool)> {
    let st = job.st;
    let design = job.design;
    let ncols = values.ncols();
    let Outputs {
        slots,
        blocks,
        gram,
        norms: residual_norms,
        columns,
        reference: formed_scales,
    } = outputs;
    let mut gram = gram.filter(|_| ncols <= BATCH);
    let ctx = Ctx::<NB> {
        st,
        w: job.w,
        index: job.members,
        elim_codes: design.effects()[st.eliminated].codes(),
        codes_b: std::array::from_fn(|b| design.effects()[st.blocks[b]].codes()),
        offs: std::array::from_fn(|b| st.offsets[b]),
        weights: job.weights,
        coefficient_accuracy: job.request.coefficient_stopping,
    };
    let (plan, initial) = plan_of(job);
    let started = Instant::now();
    if initial == Mode::Blocks && !build(st, job.w, &plan, ncols.min(BATCH), blocks, &mut facts)? {
        return Ok((untouched(Decline::MemoryLimit, facts), false));
    }
    let mut fold = MomentFold::default();
    residual_norms.clear();
    reserve(residual_norms, ncols)?;
    columns.clear();
    reserve(columns, ncols)?;
    formed_scales.clear();
    reserve(formed_scales, ncols)?;
    formed_scales.resize(ncols, f64::NAN);
    let mut gram_formed = false;
    let mut used_blocks = false;
    let mut start = 0;
    while start < ncols {
        let width = (ncols - start).min(BATCH);
        let mut reference: Option<[f64; BATCH]> = None;
        let mut attempts = 0;
        let mut iterations = [0_usize; BATCH];
        let mut mode = initial;
        loop {
            let batch = Batch {
                values: &mut *values,
                first: start,
                reference: reference.as_ref().map(|r| &r[..width]),
                coefficients: coefficients.as_mut(),
                gram: gram.as_deref_mut(),
                mode,
            };
            let lent = Lent {
                slots: &mut *slots,
                blocks: &mut *blocks,
                facts: &mut facts,
            };
            match dispatch::<NB>(&ctx, &plan, width, batch, lent)? {
                Step::Declined(reason) => {
                    let input = if start > 0 || attempts > 0 {
                        same_projection(formed_scales)?
                    } else {
                        InputState::Untouched
                    };
                    return Ok(numerical(reason, input, facts, st, ncols));
                }
                Step::Formed(formed) => {
                    if attempts == 0 {
                        formed_scales[start..start + width]
                            .copy_from_slice(&formed.reference[..width]);
                    }
                    for (total, &count) in iterations.iter_mut().zip(&formed.iterations) {
                        *total += count;
                    }
                    // Corrections of an escalated batch keep the blocks.
                    if formed.blocks {
                        used_blocks = true;
                        mode = Mode::Blocks;
                    }
                    if let Some(part) = &formed.certified {
                        fold.merge(part);
                        residual_norms.extend_from_slice(&formed.norms[..width]);
                        columns.extend((0..width).map(|j| column_report(&formed, &iterations, j)));
                        gram_formed = formed.gram;
                        break;
                    }
                    // The residual is a valid input with the same projection;
                    // correct it in place at the original scale.
                    if attempts == 2 {
                        let input = same_projection(formed_scales)?;
                        let reason = "certificate failed after corrections";
                        return Ok(numerical(reason, input, facts, st, ncols));
                    }
                    attempts += 1;
                    facts.corrections += 1;
                    reference = Some(formed.reference);
                }
            }
        }
        start += width;
    }
    let certificate = fold.finish(plan.tolerance)?;
    facts = with_structure(facts, st, ncols);
    if !certificate.passed {
        let input = same_projection(formed_scales)?;
        return Ok(numerical(
            "combined certificate failed",
            input,
            facts,
            st,
            ncols,
        ));
    }
    facts.solve_seconds = started.elapsed().as_secs_f64();
    let report = Report {
        route: Route {
            reduction: ReductionUsed::Eliminated {
                effect: st.eliminated,
            },
            method: MethodUsed::Pcg,
            preconditioner: if used_blocks {
                PreconditionerUsed::Block(blocks.used())
            } else {
                PreconditionerUsed::Jacobi
            },
            escalations: u8::try_from(facts.escalations).unwrap_or(u8::MAX),
        },
        facts,
        certificate,
        weight_scale: job.w.raw_scale,
        workspace_bytes: job.bytes + if used_blocks { blocks.bytes() } else { 0 },
    };
    Ok((Outcome::Projected(report), gram_formed))
}

/// A numerical decline of the projection.
fn numerical(
    reason: &'static str,
    input: InputState,
    facts: Facts,
    st: &Structure,
    ncols: usize,
) -> (Outcome, bool) {
    let facts = with_structure(facts, st, ncols);
    let reason = Decline::Numerical(reason);
    (
        Outcome::Declined {
            reason,
            input,
            facts,
        },
        false,
    )
}

fn column_report(formed: &Formed, iterations: &[usize; BATCH], j: usize) -> ColumnReport {
    ColumnReport {
        relative_moment: formed.relative[j],
        scale: formed.scales[j],
        residual_norm: formed.norms[j],
        iterations: iterations[j],
        status: if formed.floor[j] {
            ColumnStatus::AcceptedAtFloor
        } else {
            ColumnStatus::Converged
        },
    }
}

fn with_structure(mut facts: Facts, st: &Structure, ncols: usize) -> Facts {
    facts.columns = ncols;
    facts.cells = st.cells();
    facts.active_segments = st.active.len();
    facts.reduced = st.reduced;
    facts.lanes = st.lanes.len();
    facts
}

fn same_projection(scales: &[f64]) -> Result<InputState> {
    let mut reference_scales = with_capacity(scales.len())?;
    reference_scales.extend_from_slice(scales);
    Ok(InputState::SameProjection { reference_scales })
}
