//! The full formulation: the weighted least-squares problem
//! `min_x sum_i w_i (y_i - (D x)_i)^2` over every effect's coefficients at
//! once, for categorical intercept designs with any number of effects.
//!
//! `D` maps coefficients to observations (each row sums its levels'
//! coefficients) and `D'` sums observations into levels. Both run over
//! blocks fixed by the design (rows; small effects' scatter blocks; large
//! effects' levels, each summed over its rows in ascending order), so the
//! results do not depend on the worker count or the lane setting.
//!
//! Methods:
//! - **PCG** on the singular consistent normal equations `D'WD x = D'W y`,
//!   lockstep over up to eight columns ([`pcg`]). Its residual
//!   `D'W (y - D x)` is every level's moment, so the recurrence stops on the
//!   certificate's own quantity (at a tenth of the tolerance, on the norm of
//!   the starting residual). A cold start begins at the largest effect's
//!   level means, as the elimination does.
//! - **Modified LSMR** on `A = W^1/2 D` with right-hand side `W^1/2 y`
//!   (schwarz-precond's `mlsmr`, feature `schwarz`), one column at a time.
//!   Its stopping tests are proxies: a pass that does not certify restarts
//!   from its iterate at a tighter tolerance within the iteration budget.
//!
//! Success is declared only by the moment certificate recomputed from the
//! formed residual over all levels, with the elimination's fold and scales
//! (every level's `|sum w r| / sqrt(mass)` relative to
//! `max(reference norm, ||r||)`). Weights are normalized by their maximum.
//!
//! **Preconditioner contract.** PCG needs a fixed symmetric positive
//! definite action `z = M^-1 r` (positive definite at least on the range of
//! `D'WD`), and modified LSMR needs `M^-1` nonsingular and SPD on all of
//! coefficient space with `M ~ A'A = D'WD`, although `D'WD` is singular
//! along per-component constant shifts across effects. Jacobi
//! (`M = diag(D'WD)`, the level masses, all positive) and the identity
//! satisfy both. A subdomain (Schwarz) preconditioner whose local solves
//! are pseudo-inverses of singular local Laplacians must make its action
//! nonsingular before it is used here: ground one coordinate per local
//! component (a grounded factorization) or add a positive multiple of the
//! local null-space projector.
//!
//! Memory: every allocation is fallible; the structure, weight state and
//! one flat workspace (resized per batch width, allocations kept) are
//! retained, so repeated PCG projections allocate nothing. `mlsmr` allocates
//! its own vectors per column.

mod kernels;
#[cfg(feature = "schwarz")]
mod lsmr;
mod pcg;
pub(crate) mod structure;
mod weights;

use crate::certificate::{Certificate, MomentFold};
use crate::design::Design;
use crate::elimination::pcg::{Solve, Tally, Targets};
use crate::elimination::weights::{validate_keyed, WeightKey};
use crate::elimination::{
    Cancel, ACCEPT_FRACTION, BATCH, MAX_ITERATIONS, STOP_FRACTION, WITHIN_FLOOR,
};
use crate::error::{Error, Result};
use crate::projector::{
    CoefficientsMut, ColumnReport, ColumnStatus, ColumnsMut, Decline, Facts, Gram, InputState,
    MethodUsed, Outcome, PreconditionerUsed, ReductionUsed, Report, Request, Route,
};
use crate::weights::{Validated, Weights as InputWeights};
use kernels::{adjoint, load, residualize, Rows, ROW_BLOCK};
use pcg::{pcg, Normal, Work};
use std::time::Instant;
use structure::Structure;
use weights::State;

/// The method on the full system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Method {
    Pcg,
    #[cfg_attr(not(feature = "schwarz"), allow(dead_code))]
    Lsmr,
}

/// The fixed preconditioner action `z = M^-1 r` (see the module contract).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    /// `M = diag(D'WD)`: the level masses.
    Jacobi,
    Identity,
}

pub(super) fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| Error::OutOfMemory("unable to reserve the full-system workspace"))
}

pub(super) fn with_capacity<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    reserve(&mut values, count)?;
    Ok(values)
}

pub(super) fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>> {
    let mut values = with_capacity(count)?;
    values.resize(count, value);
    Ok(values)
}

/// `values` becomes `count` copies of `value`, keeping its allocation.
pub(super) fn refill<T: Clone>(values: &mut Vec<T>, count: usize, value: T) -> Result<()> {
    values.clear();
    reserve(values, count)?;
    values.resize(count, value);
    Ok(())
}

/// What one projection needs from its projector.
pub(crate) struct Call<'c, 'd> {
    pub(crate) design: &'d Design<'d>,
    pub(crate) method: Method,
    pub(crate) action: Action,
    pub(crate) cancel: Cancel<'c>,
}

/// One design generation's geometry, current-weight state, workspace and
/// the outputs of its last projection.
#[derive(Default)]
pub(crate) struct Engine {
    refused: bool,
    structure: Option<Structure>,
    weights: Option<(WeightKey, State)>,
    work: Work,
    gram: Gram,
    gram_ready: bool,
    norms: Vec<f64>,
    columns: Vec<ColumnReport>,
    reference: Vec<f64>,
}

/// The fixed inputs of one projection.
pub(super) struct Job<'j, 'c> {
    st: &'j Structure,
    state: &'j State,
    design: &'j Design<'j>,
    weights: Validated<'j>,
    request: &'j Request,
    method: Method,
    action: Action,
    cap: usize,
    bytes: u128,
    cancel: Cancel<'c>,
}

/// The engine's reusable outputs lent to one projection.
pub(super) struct Outputs<'o> {
    work: &'o mut Work,
    gram: Option<&'o mut Gram>,
    norms: &'o mut Vec<f64>,
    columns: &'o mut Vec<ColumnReport>,
    reference: &'o mut Vec<f64>,
}

/// Bytes of the weight state.
fn state_bytes(st: &Structure, method: Method, weighted: bool) -> u128 {
    let roots = u128::from(weighted) * (1 + u128::from(method == Method::Lsmr));
    8 * (3 * st.coefficients() as u128 + roots * st.nobs as u128)
}

/// Bytes a projection of `ncols` columns adds to the caller's data: the
/// structure, the weight state and the solve workspace (for modified LSMR,
/// with an estimate of `mlsmr`'s own vectors).
pub(crate) fn admission_bytes(
    st: &Structure,
    method: Method,
    ncols: usize,
    weighted: bool,
) -> u128 {
    let solve = match method {
        Method::Pcg => pcg::work_bytes(st, ncols.min(BATCH)),
        Method::Lsmr => {
            let (m, n) = (st.coefficients() as u128, st.nobs as u128);
            pcg::work_bytes(st, 1) + 8 * (5 * n + 8 * m)
        }
    };
    st.bytes + state_bytes(st, method, weighted) + solve
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

    /// Build the geometry; `false` when the design is outside the route,
    /// which is remembered.
    pub(crate) fn prepare(&mut self, call: &Call<'_, '_>) -> Result<bool> {
        if self.refused {
            return Ok(false);
        }
        if self.structure.is_some() {
            return Ok(true);
        }
        match Structure::build(call.design, &call.cancel)? {
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

    /// Project from `start` (canonical coefficient layout, validated by the
    /// caller) or cold; see [`crate::Projector::project_from`].
    pub(crate) fn project(
        &mut self,
        call: &Call<'_, '_>,
        weights: InputWeights<'_>,
        mut values: ColumnsMut<'_>,
        start: Option<&[f64]>,
        coefficients: Option<CoefficientsMut<'_>>,
        request: &Request,
    ) -> Result<Outcome> {
        self.gram_ready = false;
        self.norms.clear();
        self.columns.clear();
        let mut facts = Facts::default();
        if values.nobs() != call.design.nobs() {
            return Ok(untouched(Decline::Shape, facts));
        }
        let started = Instant::now();
        if !self.prepare(call)? {
            return Ok(untouched(Decline::OutsideRoute, facts));
        }
        facts.structure_seconds = started.elapsed().as_secs_f64();
        if values.ncols() == 0 {
            return Ok(untouched(Decline::Shape, facts));
        }
        let started = Instant::now();
        let st = self.structure.as_ref().expect("structure");
        let effects = call.design.effects();
        let (validated, key) = validate_keyed(weights, values.nobs())?;
        let weighted = validated.values.is_some();
        let bytes = admission_bytes(st, call.method, values.ncols(), weighted);
        if request.memory_limit.is_some_and(|limit| bytes > limit) {
            return Ok(untouched(Decline::MemoryLimit, facts));
        }
        if self.weights.as_ref().is_none_or(|(k, _)| *k != key) {
            let mut state = self.weights.take().map(|(_, s)| s).unwrap_or_default();
            let roots = call.method == Method::Lsmr;
            if !state.fill(st, effects, validated, roots, &mut self.work.scatter)? {
                return Ok(untouched(Decline::Weights, facts));
            }
            self.weights = Some((key, state));
        }
        facts.weights_seconds = started.elapsed().as_secs_f64();
        let job = Job {
            st,
            state: &self.weights.as_ref().expect("weights").1,
            design: call.design,
            weights: validated,
            request,
            method: call.method,
            action: call.action,
            cap: request.max_iterations.clamp(1, MAX_ITERATIONS),
            bytes,
            cancel: call.cancel,
        };
        let outputs = Outputs {
            work: &mut self.work,
            gram: request.residual_gram.then_some(&mut self.gram),
            norms: &mut self.norms,
            columns: &mut self.columns,
            reference: &mut self.reference,
        };
        let (outcome, gram) = match call.method {
            Method::Pcg => project_pcg(&job, &mut values, start, coefficients, outputs, facts)?,
            #[cfg(feature = "schwarz")]
            Method::Lsmr => lsmr::project(&job, &mut values, start, coefficients, outputs, facts)?,
            #[cfg(not(feature = "schwarz"))]
            Method::Lsmr => return Err(Error::Unsupported("modified LSMR needs feature schwarz")),
        };
        match &outcome {
            Outcome::Projected(_) => self.gram_ready = gram,
            Outcome::Declined { .. } => {
                self.norms.clear();
                self.columns.clear();
            }
        }
        Ok(outcome)
    }

    /// Recompute every level's weighted moment of one residual column and
    /// certify it at `amplification` times its weighted norm. `None` when
    /// this engine did not project with these weights.
    pub(crate) fn certify_residual(
        &self,
        design: &Design<'_>,
        weights: InputWeights<'_>,
        column: &[f64],
        tolerance: f64,
        amplification: f64,
    ) -> Result<Option<Certificate>> {
        let (Some(st), Some((key, state))) = (self.structure.as_ref(), self.weights.as_ref())
        else {
            return Ok(None);
        };
        if column.len() != st.nobs || design.nobs() != st.nobs {
            return Ok(None);
        }
        let (_, current) = validate_keyed(weights, column.len())?;
        if *key != current {
            return Ok(None);
        }
        if !(amplification.is_finite() && amplification >= 1.0) {
            return Err(Error::InvalidInput(
                "derived residual amplification is invalid",
            ));
        }
        let effects = design.effects();
        let mut t = filled(st.nobs, 0.0)?;
        let mut rows = filled(st.nobs.div_ceil(ROW_BLOCK) * 3, 0.0)?;
        let mut moments = filled(st.coefficients(), 0.0)?;
        let mut scatter = filled(st.scatter_blocks() * st.scatter_levels(), 0.0)?;
        let src = Rows {
            data: column,
            stride: 1,
            first: 0,
        };
        let [sq, big, _] = load::<1>(st, effects, src, None, state.weights(), &mut t, &mut rows);
        if !(sq[0].is_finite() && big[0].is_finite()) {
            return Err(Error::InvalidData("derived residual values must be finite"));
        }
        let t = Rows {
            data: &t,
            stride: 1,
            first: 0,
        };
        adjoint::<1>(st, effects, t, None, &mut moments, &mut scatter);
        let reference = amplification * sq[0].sqrt();
        if !reference.is_finite() {
            return Err(Error::InvalidData("derived residual scale is not finite"));
        }
        let mut fold = MomentFold::default();
        let mut rhs_inf = [0.0];
        for (g, &moment) in moments.iter().enumerate() {
            let mass = state.mass[g];
            fold.level(
                mass,
                &[moment / mass],
                &[reference],
                &mut rhs_inf,
                state.raw_scale,
            )?;
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

/// A numerical decline of the projection.
fn numerical(reason: &'static str, input: InputState, facts: Facts, job: &Job<'_, '_>) -> Outcome {
    Outcome::Declined {
        reason: Decline::Numerical(reason),
        input,
        facts: with_structure(facts, job),
    }
}

fn with_structure(mut facts: Facts, job: &Job<'_, '_>) -> Facts {
    facts.reduced = job.st.coefficients();
    facts
}

fn same_projection(scales: &[f64]) -> Result<InputState> {
    let mut reference_scales = with_capacity(scales.len())?;
    reference_scales.extend_from_slice(scales);
    Ok(InputState::SameProjection { reference_scales })
}

/// The route actually used.
fn route(job: &Job<'_, '_>) -> Route {
    Route {
        reduction: ReductionUsed::Full,
        method: match job.method {
            Method::Pcg => MethodUsed::Pcg,
            Method::Lsmr => MethodUsed::Lsmr,
        },
        preconditioner: match job.action {
            Action::Jacobi => PreconditionerUsed::Jacobi,
            Action::Identity => PreconditionerUsed::Identity,
        },
        escalations: 0,
    }
}

/// Form the raw-weighted residual Gram of all columns when requested (at
/// most [`BATCH`] columns); `true` when formed.
fn form_gram(
    job: &Job<'_, '_>,
    values: &ColumnsMut<'_>,
    gram: Option<&mut Gram>,
    work: &mut Work,
) -> Result<bool> {
    let ncols = values.ncols();
    let Some(gram) = gram.filter(|_| ncols <= BATCH) else {
        return Ok(false);
    };
    let blocks = job.st.nobs.div_ceil(ROW_BLOCK);
    if work.rows.len() < blocks * ncols * ncols {
        refill(&mut work.rows, blocks * ncols * ncols, 0.0)?;
    }
    refill(&mut gram.values, ncols * ncols, 0.0)?;
    gram.columns = ncols;
    let raw = job.weights.values;
    kernels::gram(
        values.as_slice(),
        ncols,
        raw,
        &mut work.rows,
        &mut gram.values,
    );
    Ok(true)
}

/// A successful projection's report.
fn report(job: &Job<'_, '_>, certificate: Certificate, facts: Facts, started: Instant) -> Report {
    let mut facts = with_structure(facts, job);
    facts.solve_seconds = started.elapsed().as_secs_f64();
    Report {
        route: route(job),
        facts,
        certificate,
        weight_scale: job.state.raw_scale,
        workspace_bytes: job.bytes,
    }
}

/// Certificate scales and stopping targets of one batch; `None` when a
/// column's scale is outside the plain-sum range. `stats` are the load
/// pass's `[sum w y^2, max |y|, sum w (y - D start)^2]`.
fn targets<const R: usize>(
    job: &Job<'_, '_>,
    stats: &[[f64; R]; 3],
    reference: Option<&[f64]>,
) -> Option<([f64; R], Targets<R>)> {
    let tolerance = job.request.tolerance;
    let mut scale = [0.0; R];
    let mut within = [0.0; R];
    for j in 0..R {
        let norm = stats[0][j].sqrt();
        if !norm.is_finite() || (stats[1][j] > 0.0 && !(1e-150..=1e150).contains(&norm)) {
            return None;
        }
        scale[j] = reference.map_or(norm, |r| r[j]);
        // The recurrence's accuracy is set at the starting residual's norm
        // (for a cold start, the column about the largest effect's level
        // means); a column the start explains to rounding keeps the
        // acceptance rule.
        let centered = stats[2][j].max(0.0).sqrt().min(scale[j]);
        within[j] = if centered >= WITHIN_FLOOR * scale[j] {
            centered
        } else {
            f64::NAN
        };
    }
    let coefficient = job.request.coefficient_stopping;
    let root_total = job.state.total_mass.sqrt();
    let mut targets = Targets {
        goal: [0.0; R],
        goal_coefficient: coefficient.then_some([0.0; R]),
        accept: [0.0; R],
        accept_coefficient: coefficient.then_some([0.0; R]),
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

pub(super) struct Certified<const R: usize> {
    /// The batch's fold when it passed.
    fold: Option<MomentFold>,
    norms: [f64; R],
    relative: [f64; R],
    scales: [f64; R],
}

/// Fold every level's recomputed moment at `max(reference, ||r||)`.
fn certify<const R: usize>(
    state: &State,
    moments: &[f64],
    sq: [f64; R],
    reference: [f64; R],
    tolerance: f64,
) -> Certified<R> {
    let mut fold = MomentFold::default();
    let mut out = Certified {
        fold: None,
        norms: sq.map(f64::sqrt),
        relative: [0.0; R],
        scales: [0.0; R],
    };
    for ((scale, reference), norm) in out.scales.iter_mut().zip(reference).zip(out.norms) {
        *scale = reference.max(norm);
    }
    if !out.scales.iter().all(|scale| scale.is_finite()) {
        return out;
    }
    let mut means = [0.0; R];
    for (g, moment) in moments.chunks_exact(R).enumerate() {
        let mass = state.mass[g];
        let inverse = state.inverse_mass[g];
        for j in 0..R {
            means[j] = moment[j] * inverse;
        }
        let scales = &out.scales;
        if fold
            .level(mass, &means, scales, &mut out.relative, state.raw_scale)
            .is_err()
        {
            return out;
        }
    }
    if fold.relative_infinity_norm <= tolerance {
        out.fold = Some(fold);
    }
    out
}

/// Add column `j`'s coefficients `x[g * stride + offset]` (canonical order)
/// to the caller's buffer at its layout.
fn add_coefficients(
    st: &Structure,
    x: &[f64],
    (stride, offset): (usize, usize),
    out: &mut CoefficientsMut<'_>,
    column: usize,
) {
    let layout = out.layout;
    let base = column * layout.per_rhs;
    for (e, &first) in st.offsets[..st.offsets.len() - 1].iter().enumerate() {
        let start = base + layout.effect_offsets[e];
        let levels = layout.level_counts[e];
        for (l, value) in out.values[start..start + levels].iter_mut().enumerate() {
            *value += x[(first + l) * stride + offset];
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum Step {
    Declined(&'static str),
    /// Inline, so repeated projections allocate nothing.
    Formed(Formed),
}

/// One batch's formed residual and its certificate.
struct Formed {
    certified: Option<MomentFold>,
    norms: [f64; BATCH],
    reference: [f64; BATCH],
    relative: [f64; BATCH],
    scales: [f64; BATCH],
    iterations: [usize; BATCH],
    floor: [bool; BATCH],
}

/// One PCG batch `first..first + R` and its outputs.
struct Batch<'b, 'v, 'c> {
    values: &'b mut ColumnsMut<'v>,
    first: usize,
    reference: Option<&'b [f64]>,
    start: Option<&'b [f64]>,
    coefficients: Option<&'b mut CoefficientsMut<'c>>,
}

/// The starting iterate: the caller's (canonical layout) or, cold, the
/// largest effect's weighted level means `g_l / m_l` from `t = W y`, with
/// every other effect at zero.
fn starting_iterate<const R: usize>(job: &Job<'_, '_>, batch: &Batch<'_, '_, '_>, work: &mut Work) {
    let st = job.st;
    let m = st.coefficients();
    if let Some(start) = batch.start {
        for (g, x) in work.x.chunks_exact_mut(R).enumerate() {
            for (j, x) in x.iter_mut().enumerate() {
                *x = start[(batch.first + j) * m + g];
            }
        }
        return;
    }
    let effects = job.design.effects();
    let t = Rows {
        data: &work.t,
        stride: R,
        first: 0,
    };
    adjoint::<R>(st, effects, t, None, &mut work.q, &mut work.scatter);
    work.x.fill(0.0);
    let (lo, hi) = (st.offsets[st.largest], st.offsets[st.largest + 1]);
    for g in lo..hi {
        let inverse = job.state.inverse_mass[g];
        for j in 0..R {
            work.x[g * R + j] = work.q[g * R + j] * inverse;
        }
    }
}

/// One batch: the starting residual and the recurrence read the columns;
/// only a converged recurrence forms the residual in place.
fn run<const R: usize>(
    job: &Job<'_, '_>,
    batch: Batch<'_, '_, '_>,
    work: &mut Work,
    facts: &mut Facts,
) -> Result<Step> {
    let (st, state) = (job.st, job.state);
    let effects = job.design.effects();
    let ncols = batch.values.ncols();
    work.size(st, R)?;
    let started = Instant::now();
    let src = Rows {
        data: batch.values.as_slice(),
        stride: ncols,
        first: batch.first,
    };
    if batch.start.is_none() {
        load::<R>(
            st,
            effects,
            src,
            None,
            state.weights(),
            &mut work.t,
            &mut work.rows,
        );
    }
    starting_iterate::<R>(job, &batch, work);
    let x = Some(&work.x[..]);
    let stats = load::<R>(
        st,
        effects,
        src,
        x,
        state.weights(),
        &mut work.t,
        &mut work.rows,
    );
    let t = Rows {
        data: &work.t,
        stride: R,
        first: 0,
    };
    adjoint::<R>(st, effects, t, None, &mut work.r, &mut work.scatter);
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    let Some((scale, targets)) = targets::<R>(job, &stats, batch.reference) else {
        return Ok(Step::Declined("RHS scale outside the plain-sum range"));
    };
    let started = Instant::now();
    let mut tally = Tally::default();
    let op = Normal {
        st,
        effects,
        state,
        action: job.action,
    };
    let solved = pcg::<R>(&op, work, &targets, job.cap, &job.cancel, &mut tally)?;
    facts.pcg_seconds += started.elapsed().as_secs_f64();
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
    if let Some(coefficients) = batch.coefficients {
        for j in 0..R {
            add_coefficients(st, &work.x, (R, j), coefficients, batch.first + j);
        }
    }
    let target = (batch.values.as_mut_slice(), ncols, batch.first);
    let sq = residualize::<R>(
        st,
        effects,
        &work.x,
        target,
        state.weights(),
        &mut work.t,
        &mut work.rows,
    );
    let t = Rows {
        data: &work.t,
        stride: R,
        first: 0,
    };
    adjoint::<R>(st, effects, t, None, &mut work.q, &mut work.scatter);
    let certified = certify::<R>(state, &work.q, sq, scale, job.request.tolerance);
    facts.residual_seconds += started.elapsed().as_secs_f64();
    let mut out = Formed {
        certified: certified.fold,
        norms: [0.0; BATCH],
        reference: [0.0; BATCH],
        relative: [0.0; BATCH],
        scales: [0.0; BATCH],
        iterations: [0; BATCH],
        floor: [false; BATCH],
    };
    out.norms[..R].copy_from_slice(&certified.norms);
    out.reference[..R].copy_from_slice(&scale);
    out.relative[..R].copy_from_slice(&certified.relative);
    out.scales[..R].copy_from_slice(&certified.scales);
    out.iterations[..R].copy_from_slice(&tally.per_column);
    out.floor[..R].copy_from_slice(&floor);
    Ok(Step::Formed(out))
}

fn dispatch(
    job: &Job<'_, '_>,
    width: usize,
    batch: Batch<'_, '_, '_>,
    work: &mut Work,
    facts: &mut Facts,
) -> Result<Step> {
    match width {
        1 => run::<1>(job, batch, work, facts),
        2 => run::<2>(job, batch, work, facts),
        3 => run::<3>(job, batch, work, facts),
        4 => run::<4>(job, batch, work, facts),
        5 => run::<5>(job, batch, work, facts),
        6 => run::<6>(job, batch, work, facts),
        7 => run::<7>(job, batch, work, facts),
        8 => run::<8>(job, batch, work, facts),
        _ => Err(Error::InvalidInput("full-system batch width is invalid")),
    }
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

/// PCG over batches of up to eight columns. A decline leaves the columns
/// untouched unless a later batch or a correction declined after earlier
/// residuals were formed ([`InputState::SameProjection`]); a failed
/// certificate corrects the formed residual in place from zero (at most
/// twice). Also reports whether the residual Gram was formed.
fn project_pcg(
    job: &Job<'_, '_>,
    values: &mut ColumnsMut<'_>,
    start: Option<&[f64]>,
    mut coefficients: Option<CoefficientsMut<'_>>,
    outputs: Outputs<'_>,
    mut facts: Facts,
) -> Result<(Outcome, bool)> {
    let ncols = values.ncols();
    let Outputs {
        work,
        gram,
        norms,
        columns,
        reference: formed_scales,
    } = outputs;
    let started = Instant::now();
    let mut fold = MomentFold::default();
    for buffer in [&mut *norms, &mut *formed_scales] {
        buffer.clear();
        reserve(buffer, ncols)?;
    }
    columns.clear();
    reserve(columns, ncols)?;
    formed_scales.resize(ncols, f64::NAN);
    facts.columns = ncols;
    let mut first = 0;
    while first < ncols {
        let width = (ncols - first).min(BATCH);
        let mut reference: Option<[f64; BATCH]> = None;
        let mut attempts = 0;
        let mut iterations = [0_usize; BATCH];
        loop {
            let batch = Batch {
                values: &mut *values,
                first,
                reference: reference.as_ref().map(|r| &r[..width]),
                start: start.filter(|_| attempts == 0),
                coefficients: coefficients.as_mut(),
            };
            let formed = match dispatch(job, width, batch, work, &mut facts)? {
                Step::Declined(reason) => {
                    let input = if first > 0 || attempts > 0 {
                        same_projection(formed_scales)?
                    } else {
                        InputState::Untouched
                    };
                    return Ok((numerical(reason, input, facts, job), false));
                }
                Step::Formed(formed) => formed,
            };
            if attempts == 0 {
                formed_scales[first..first + width].copy_from_slice(&formed.reference[..width]);
            }
            for (total, &count) in iterations.iter_mut().zip(&formed.iterations) {
                *total += count;
            }
            if let Some(part) = &formed.certified {
                fold.merge(part);
                norms.extend_from_slice(&formed.norms[..width]);
                columns.extend((0..width).map(|j| column_report(&formed, &iterations, j)));
                break;
            }
            if attempts == 2 {
                let input = same_projection(formed_scales)?;
                let reason = "certificate failed after corrections";
                return Ok((numerical(reason, input, facts, job), false));
            }
            attempts += 1;
            facts.corrections += 1;
            reference = Some(formed.reference);
        }
        first += width;
    }
    let certificate = fold.finish(job.request.tolerance)?;
    if !certificate.passed {
        let input = same_projection(formed_scales)?;
        return Ok((
            numerical("combined certificate failed", input, facts, job),
            false,
        ));
    }
    let gram_formed = form_gram(job, values, gram, work)?;
    Ok((
        Outcome::Projected(report(job, certificate, facts, started)),
        gram_formed,
    ))
}
