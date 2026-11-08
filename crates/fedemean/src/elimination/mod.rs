//! Exact global elimination of the largest categorical effect.
//!
//! An intercept-only design with effects `E_0..E_k` partitions its weighted
//! normal matrix as `[[D, C], [C', A]]`, with `D` the diagonal mass of the
//! effect that has the most levels. Those coefficients are eliminated exactly:
//! the remaining coefficients solve `S b = g` with `S = A - C' D^-1 C`, which
//! is applied implicitly. Each eliminated group gathers the remaining
//! coefficients of its distinct remaining-code cells, subtracts their weighted
//! group mean and scatters the weighted deviations back. Up to eight
//! right-hand sides advance in lockstep through one traversal. The eliminated
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
//! residualized column yields the same residual.
//!
//! Memory: every allocation is fallible. The engine keeps the geometry, the
//! weight state and one workspace per batch width, so repeated projections of
//! one width with changing weights allocate nothing.

use crate::certificate::{Certificate, MomentFold};
use crate::design::{Design, Members};
use crate::error::{Error, Result};
use crate::projector::{
    CoefficientsMut, ColumnReport, ColumnStatus, ColumnsMut, Decline, Facts, Gram, InputState,
    MethodUsed, Outcome, PreconditionerUsed, ReductionUsed, Report, Request, Route,
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
/// Columns whose within norm is below this fraction of the certificate scale
/// lie inside the eliminated span.
const WITHIN_FLOOR: f64 = 1.0 / (1_u64 << 30) as f64;
/// Acceptance rule: this fraction of the tolerance at the certificate scale
/// (where a column may stop when its recurrence reaches its rounding floor).
const ACCEPT_FRACTION: f64 = 0.5;

#[allow(dead_code)] // Stage 3: block-Jacobi local solves.
pub(crate) mod block;
mod kernels;
mod passes;
mod pcg;
pub(crate) mod structure;
mod weights;

use kernels::{Ctx, Slotted, Work, WorkSlots};
use passes::{recover, residualize, rhs};
use pcg::{pcg, Solve, Targets};
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
        if request
            .memory_limit
            .is_some_and(|limit| admission_bytes(st, None, values.ncols()) > limit)
        {
            return Ok(untouched(Decline::MemoryLimit, facts));
        }
        if self.weights.as_ref().is_none_or(|(k, _)| *k != key) {
            let recycled = self.weights.take().map(|(_, w)| w);
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
    cancel: Cancel<'c>,
}

/// The engine's reusable outputs and workspaces lent to one projection.
struct Outputs<'c> {
    slots: &'c mut WorkSlots,
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
}

/// One batch `first..first+R`: RHS and recurrence read `values`; only a
/// converged recurrence forms the residual in place.
#[allow(clippy::too_many_arguments)]
fn run<const R: usize, const NB: usize, const UNIT: bool, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    values: &mut ColumnsMut<'_>,
    first: usize,
    reference: Option<&[f64]>,
    tolerance: f64,
    cap: usize,
    facts: &mut Facts,
    coefficients: Option<&mut CoefficientsMut<'_>>,
    slots: &mut WorkSlots,
    gram: Option<&mut Gram>,
    cancel: &Cancel<'_>,
) -> Result<Step>
where
    Work<R>: Slotted,
{
    let st = ctx.st;
    let slot = <Work<R> as Slotted>::slot(slots);
    if slot.as_ref().is_none_or(|work| !work.fits(st)) {
        *slot = None;
        *slot = Some(Work::<R>::new(st)?);
    }
    let work = slot.as_mut().expect("workspace");
    let started = Instant::now();
    let norms = rhs::<R, NB, WEIGHTED>(ctx, values, first, work);
    std::mem::swap(&mut work.r, &mut work.lanes[0].acc);
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    let mut scale = [0.0; R];
    let mut within = [0.0; R];
    for j in 0..R {
        let norm = norms.sq[j].sqrt();
        if !norm.is_finite() || (norms.big[j] > 0.0 && !(1e-150..=1e150).contains(&norm)) {
            return Ok(Step::Declined("RHS scale outside the plain-sum range"));
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
    let started = Instant::now();
    let solved = pcg::<R, NB, UNIT>(st, ctx.w, work, &targets, cap, cancel)?;
    facts.pcg_seconds += started.elapsed().as_secs_f64();
    let (iterations, per_column, floor) = match solved {
        Solve::Converged {
            iterations,
            column_iterations,
            accepted,
            per_column,
            floor,
        } => {
            facts.column_iterations += column_iterations;
            facts.accepted += accepted;
            (iterations, per_column, floor)
        }
        Solve::Declined(reason) => return Ok(Step::Declined(reason)),
    };
    facts.iterations = facts.iterations.max(iterations);
    let started = Instant::now();
    recover::<R, NB, UNIT>(st, ctx.w, work);
    if let Some(batch) = coefficients {
        // Corrections add their increments to the coefficients of the
        // preceding solve, so the sum fits the original columns.
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
    let formed = gram.is_some();
    let sq = residualize::<R, NB, WEIGHTED>(ctx, values, first, work, gram)?;
    let certified = certify_batch::<R>(ctx.w, &work.lanes[0].acc, sq, scale, tolerance);
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
    };
    out.norms[..R].copy_from_slice(&certified.norms);
    out.reference[..R].copy_from_slice(&scale);
    out.relative[..R].copy_from_slice(&certified.relative);
    out.scales[..R].copy_from_slice(&certified.scales);
    out.iterations[..R].copy_from_slice(&per_column);
    out.floor[..R].copy_from_slice(&floor);
    Ok(Step::Formed(out))
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

#[allow(clippy::too_many_arguments)]
fn dispatch<const NB: usize>(
    ctx: &Ctx<'_, '_, NB>,
    values: &mut ColumnsMut<'_>,
    first: usize,
    width: usize,
    reference: Option<&[f64]>,
    tolerance: f64,
    cap: usize,
    facts: &mut Facts,
    coefficients: Option<&mut CoefficientsMut<'_>>,
    slots: &mut WorkSlots,
    gram: Option<&mut Gram>,
    cancel: &Cancel<'_>,
) -> Result<Step> {
    let unit = ctx.w.cell.is_empty();
    let weighted = ctx.weights.values.is_some();
    macro_rules! go {
        ($r:literal) => {
            match (unit, weighted) {
                (true, false) => run::<$r, NB, true, false>(
                    ctx,
                    values,
                    first,
                    reference,
                    tolerance,
                    cap,
                    facts,
                    coefficients,
                    slots,
                    gram,
                    cancel,
                ),
                (true, true) => run::<$r, NB, true, true>(
                    ctx,
                    values,
                    first,
                    reference,
                    tolerance,
                    cap,
                    facts,
                    coefficients,
                    slots,
                    gram,
                    cancel,
                ),
                (false, false) => run::<$r, NB, false, false>(
                    ctx,
                    values,
                    first,
                    reference,
                    tolerance,
                    cap,
                    facts,
                    coefficients,
                    slots,
                    gram,
                    cancel,
                ),
                (false, true) => run::<$r, NB, false, true>(
                    ctx,
                    values,
                    first,
                    reference,
                    tolerance,
                    cap,
                    facts,
                    coefficients,
                    slots,
                    gram,
                    cancel,
                ),
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
    let tolerance = job.request.tolerance;
    let cap = job.request.max_iterations.clamp(1, MAX_ITERATIONS);
    let started = Instant::now();
    let mut fold = MomentFold::default();
    residual_norms.clear();
    reserve(residual_norms, ncols)?;
    columns.clear();
    reserve(columns, ncols)?;
    formed_scales.clear();
    reserve(formed_scales, ncols)?;
    formed_scales.resize(ncols, f64::NAN);
    let mut gram_formed = false;
    let mut start = 0;
    while start < ncols {
        let width = (ncols - start).min(BATCH);
        let mut reference: Option<[f64; BATCH]> = None;
        let mut attempts = 0;
        let mut iterations = [0_usize; BATCH];
        loop {
            let step = dispatch::<NB>(
                &ctx,
                values,
                start,
                width,
                reference.as_ref().map(|r| &r[..width]),
                tolerance,
                cap,
                &mut facts,
                coefficients.as_mut(),
                slots,
                gram.as_deref_mut(),
                &job.cancel,
            )?;
            match step {
                Step::Declined(reason) => {
                    let input = if start > 0 || attempts > 0 {
                        same_projection(formed_scales)?
                    } else {
                        InputState::Untouched
                    };
                    return Ok((
                        Outcome::Declined {
                            reason: Decline::Numerical(reason),
                            input,
                            facts: with_structure(facts, st, ncols),
                        },
                        false,
                    ));
                }
                Step::Formed(formed) => {
                    if attempts == 0 {
                        formed_scales[start..start + width]
                            .copy_from_slice(&formed.reference[..width]);
                    }
                    for (total, &count) in iterations.iter_mut().zip(&formed.iterations) {
                        *total += count;
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
                        return Ok((
                            Outcome::Declined {
                                reason: Decline::Numerical("certificate failed after corrections"),
                                input: same_projection(formed_scales)?,
                                facts: with_structure(facts, st, ncols),
                            },
                            false,
                        ));
                    }
                    attempts += 1;
                    facts.corrections += 1;
                    reference = Some(formed.reference);
                }
            }
        }
        start += width;
    }
    let certificate = fold.finish(tolerance)?;
    facts = with_structure(facts, st, ncols);
    if !certificate.passed {
        return Ok((
            Outcome::Declined {
                reason: Decline::Numerical("combined certificate failed"),
                input: same_projection(formed_scales)?,
                facts,
            },
            false,
        ));
    }
    facts.solve_seconds = started.elapsed().as_secs_f64();
    let report = Report {
        route: Route {
            reduction: ReductionUsed::Eliminated {
                effect: st.eliminated,
            },
            method: MethodUsed::Pcg,
            preconditioner: PreconditionerUsed::Jacobi,
            escalations: 0,
        },
        facts,
        certificate,
        weight_scale: job.w.raw_scale,
        workspace_bytes: job.bytes,
    };
    Ok((Outcome::Projected(report), gram_formed))
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
