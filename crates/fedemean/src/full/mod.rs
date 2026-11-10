//! The full formulation: the weighted least-squares problem
//! `min_x sum_i w_i (y_i - (D x)_i)^2` over every effect's coefficients at
//! once, for categorical designs with any number of effects, with or
//! without varying slopes.
//!
//! `D` maps coefficients to observations (each row sums its levels'
//! channels: the intercept coefficient and each slope coefficient times the
//! row's slope value) and `D'` sums observations into level channels. A
//! slope at a large offset from zero is taken less its level centers in
//! both, with coefficients in that centered basis inside the projection and
//! raw at its boundary ([`kernels::Basis`], [`blocks`]). Both run over
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
//!   ([`crate::lsmr::mlsmr`] and [`crate::lsmr::mlsmr_columns`]),
//!   lockstep over batches of up to eight columns ([`lsmr`]), each column
//!   with `mlsmr`'s own recurrence and bits. Its stopping tests are proxies:
//!   a pass that does not certify restarts from its iterate at a tighter
//!   tolerance within the iteration budget.
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
//! satisfy both. Additive Schwarz ([`schwarz`], feature `schwarz`) sums
//! local actions `S (P G P + tau 11'/n) S` over factor-pair subdomains:
//! `P G P` is the (exact or approximate) pseudo-inverse of the subdomain's
//! sign-flipped Laplacian with its component mean removed on both sides,
//! and the added `tau 11'/n` makes each local action positive definite on
//! its subdomain, so the sum is SPD on all of coefficient space. The added
//! direction is the subdomain's `(1, -1)` vector, a null direction of `D`,
//! which every consistent residual is orthogonal to: it leaves PCG and
//! modified LSMR iterates unchanged. With slopes, Jacobi's diagonal of an
//! effect with several channels is its levels' channel blocks ([`blocks`]:
//! exact inverses, singular ones augmented on their null directions, which
//! are null directions of `D`), and Schwarz adds those blocks to its pair
//! subdomains on the intercept channels; both stay SPD on all of
//! coefficient space. A blocked level is measured and certified by the norm
//! of its moments in its channel span, `sqrt(m' M_l^-1 m)`.
//!
//! **The elimination with slopes** ([`Action::Eliminate`], two effects)
//! runs here too: from the larger effect's per-level fit (the cold start),
//! PCG on the Schur complement over the smaller effect's coefficients
//! ([`cells`]), the larger effect by back-substitution, and this module's
//! residual and certificate over every level of both effects.
//!
//! **Escalation.** With [`crate::Fallback::Escalate`], a Jacobi recurrence
//! that stalls (PCG; modified LSMR: its normal-equation estimate fails to
//! halve over a stall window) or whose rate after a pilot of
//! [`pcg::PILOT`] iterations predicts more than [`pcg::SLOW_ITERATIONS`]
//! (an easy design keeps Jacobi; a hard one pays for Schwarz's build)
//! continues with Schwarz at the default
//! specification from its current iterate on the true residual, within the
//! remaining iteration budget. Later batches of the projection, and later
//! projections of the projector, start on Schwarz. The full formulation's
//! results do not depend on the worker count, so an escalated recurrence
//! keeps the ordinary goal.
//!
//! Memory: every allocation is fallible; the structure, weight state and
//! one flat workspace (resized per batch width, allocations kept) are
//! retained, so repeated PCG projections allocate nothing. `mlsmr` and
//! `mlsmr_columns` allocate their own vectors per round; the LSMR batch is
//! the widest that fits the memory limit. A Schwarz preconditioner is built
//! for the current weights within the memory limit (or declines) and
//! retained.

mod blocks;
mod cells;
mod handle;
mod kernels;
mod lsmr;
mod pcg;
#[cfg(feature = "schwarz")]
mod schwarz;
pub(crate) mod structure;
mod weights;

/// Without feature `schwarz` there is no Schwarz preconditioner: the
/// choices never resolve to one, and this uninhabited stand-in keeps the
/// routes' code shared.
#[cfg(not(feature = "schwarz"))]
mod schwarz {
    use super::blocks::Blocks;
    use super::structure::Structure;
    use super::weights::State;
    use crate::design::Effect;
    use crate::elimination::Cancel;
    use crate::error::{Error, Result};
    use crate::projector::{PreconditionerUsed, SchwarzSpec};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Refusal {
        MemoryLimit,
        #[allow(dead_code)]
        Numerical(&'static str),
    }

    pub(crate) enum Schwarz {}

    #[derive(Default)]
    pub(crate) struct Scratch;

    impl Schwarz {
        pub(super) fn build(
            _: &Structure,
            _: &[Effect<'_>],
            _: &State,
            _: (SchwarzSpec, u64),
            _: (Option<u128>, usize),
            _: &Cancel<'_>,
        ) -> Result<std::result::Result<Self, Refusal>> {
            Err(Error::Unsupported("additive Schwarz needs feature schwarz"))
        }

        pub(super) fn estimate(
            _: &Structure,
            _: &[Effect<'_>],
            _: &State,
            _: SchwarzSpec,
            _: (Option<u128>, usize),
            _: &Cancel<'_>,
        ) -> Result<std::result::Result<u128, Refusal>> {
            Err(Error::Unsupported("additive Schwarz needs feature schwarz"))
        }

        pub(super) fn current(&self, _: SchwarzSpec, _: u64) -> bool {
            match *self {}
        }

        pub(super) fn bytes(&self) -> u128 {
            match *self {}
        }

        pub(super) fn scratch_bytes(&self, _: usize) -> u128 {
            match *self {}
        }

        pub(super) fn peak_bytes(&self, _: usize) -> u128 {
            match *self {}
        }

        pub(super) fn used(&self) -> PreconditionerUsed {
            match *self {}
        }

        pub(super) fn size(&self, _: &mut Scratch, _: usize) -> Result<()> {
            match *self {}
        }

        pub(super) fn apply<const R: usize>(
            &self,
            _: &[f64],
            _: &mut [f64],
            _: &mut Scratch,
            _: (&[f64], Option<&Blocks>),
        ) {
            match *self {}
        }
    }
}

use crate::certificate::{Certificate, MomentFold};
use crate::design::{Design, Effect};
use crate::elimination::pcg::{Solve, Tally, Targets, CEILING, STALLED};
use crate::elimination::weights::{validate_keyed, WeightKey};
use crate::elimination::{Cancel, ACCEPT_FRACTION, BATCH, STOP_FRACTION, WITHIN_FLOOR};
use crate::error::{Error, Result};
use crate::projector::{
    CoefficientsMut, ColumnReport, ColumnStatus, ColumnsMut, Decline, Facts, Gram, InputState,
    MethodUsed, Outcome, PreconditionerUsed, ReductionUsed, Report, Request, Route, SchwarzSpec,
};
use crate::weights::{Validated, Weights as InputWeights};
use cells::Cells;
pub use handle::FePreconditioner;
use kernels::{adjoint, load, residualize, Basis, Rows, ROW_BLOCK};
use pcg::{pcg, Normal, Work};
use schwarz::{Refusal, Schwarz};
use std::time::Instant;
use structure::Structure;
use weights::State;

/// Iteration ceiling of the full formulation's methods (see
/// [`Request::max_iterations`]).
const MAX_ITERATIONS: usize = 1_000_000;

/// The method on the full system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Method {
    Pcg,
    Lsmr,
}

/// The preconditioner choice (see the module contract).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    /// `M = diag(D'WD)`: the level masses.
    Jacobi,
    Identity,
    /// Additive Schwarz as specified; declines when it does not fit.
    Schwarz(SchwarzSpec),
    /// Schwarz at the default specification when it fits, else Jacobi.
    #[cfg_attr(not(feature = "schwarz"), allow(dead_code))]
    Auto,
    /// Jacobi, escalating to Schwarz at the default specification.
    #[cfg_attr(not(feature = "schwarz"), allow(dead_code))]
    Escalate,
    /// The elimination with slopes (two effects): PCG on the Schur
    /// complement of the larger effect's level blocks over the smaller
    /// effect's coefficients ([`cells`]), with Jacobi on its diagonal blocks.
    Eliminate,
}

/// How one batch (or column) preconditions its recurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Jacobi,
    Identity,
    Schwarz,
    /// Jacobi, escalating to Schwarz on a stall.
    Escalate,
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
    /// Counts the weight states prepared (the Schwarz build key).
    generation: u64,
    /// A recurrence of this design escalated: later escalating projections
    /// start on Schwarz, since the design's graph makes Jacobi stall.
    hard: bool,
    schwarz: Option<Schwarz>,
    /// The elimination's cells and its Schur recurrence's workspace.
    cells: Option<Cells>,
    reduced: Work,
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
    /// The weight state's generation and [`Engine::hard`].
    generation: u64,
    hard: bool,
    /// The elimination's cells (with slopes).
    cells: Option<&'j Cells>,
    cap: usize,
    bytes: u128,
    cancel: Cancel<'c>,
}

/// The engine's reusable outputs lent to one projection.
pub(super) struct Outputs<'o> {
    work: &'o mut Work,
    /// The elimination's Schur workspace.
    reduced: &'o mut Work,
    schwarz: &'o mut Option<Schwarz>,
    gram: Option<&'o mut Gram>,
    norms: &'o mut Vec<f64>,
    columns: &'o mut Vec<ColumnReport>,
    reference: &'o mut Vec<f64>,
}

/// Bytes of the weight state.
fn state_bytes(st: &Structure, method: Method, weighted: bool) -> u128 {
    let roots = u128::from(weighted) * (1 + u128::from(method == Method::Lsmr));
    8 * (3 * st.coefficients() as u128 + roots * st.nobs as u128) + st.block_bytes
}

/// Bytes of the elimination's cells and its Schur workspace for `width`
/// columns (six recurrence vectors and the cells' products).
fn cells_bytes(cells: &Cells, width: usize) -> u128 {
    let [ca, cb] = cells.channels;
    let small = cells.small_tasks() as u128 * 512 * cb as u128;
    let w = width as u128;
    cells.bytes() + 8 * w * (6 * small + (cells.cells() * cb) as u128 + 3 * ca as u128)
}

/// Bytes of the modified LSMR solve for batches of `width` columns (beside
/// the structure and weight state). One column: fedemean's workspace and an
/// estimate of `mlsmr`'s own vectors. Wider: per column, its right-hand
/// side, `mlsmr_columns`' two observation and seven coefficient vectors, the
/// column's start, restart base, result and certified iterate and its
/// scatter accumulators; once, `mlsmr_columns`' single-column stream (three
/// observation and five coefficient vectors).
pub(super) fn lsmr_bytes(st: &Structure, width: usize) -> u128 {
    let (m, n) = (st.coefficients() as u128, st.nobs as u128);
    if width <= 1 {
        return pcg::work_bytes(st, 1) + 8 * (5 * n + 8 * m);
    }
    let scatter = (st.scatter_blocks() * st.scatter_levels()) as u128;
    let w = width as u128;
    pcg::work_bytes(st, 1) + 8 * (w * (3 * n + 11 * m + scatter) + 3 * n + 5 * m)
}

/// Bytes a projection of `ncols` columns uses beyond the caller's data: the
/// structure, the weight state and the solve workspace (modified LSMR at its
/// widest batch; under a tighter limit it runs narrower, down to one column).
pub(crate) fn admission_bytes(
    st: &Structure,
    method: Method,
    ncols: usize,
    weighted: bool,
) -> u128 {
    let solve = match method {
        Method::Pcg => pcg::work_bytes(st, ncols.min(BATCH)),
        Method::Lsmr => lsmr_bytes(st, ncols.clamp(1, BATCH)),
    };
    st.bytes + state_bytes(st, method, weighted) + solve
}

/// The columns a projection of `ncols` must admit at once: PCG's batch;
/// modified LSMR one, a wider batch running only where it fits.
fn minimum_columns(method: Method, ncols: usize) -> usize {
    match method {
        Method::Pcg => ncols,
        Method::Lsmr => 1,
    }
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

    /// Bytes of the retained Schwarz preconditioner (at least its build's
    /// peak) or the elimination's cells, and their workspaces for `ncols`
    /// columns (0 when none is built).
    pub(crate) fn schwarz_bytes(&self, ncols: usize) -> u128 {
        let width = ncols.clamp(1, BATCH);
        self.schwarz.as_ref().map_or(0, |s| s.peak_bytes(width))
            + self.cells.as_ref().map_or(0, |c| cells_bytes(c, width))
    }

    /// Build the geometry (once; an FE preconditioner handle may have built
    /// the structure first); `false` when the design is outside the route,
    /// which is remembered.
    pub(crate) fn prepare(&mut self, call: &Call<'_, '_>) -> Result<bool> {
        if self.refused {
            return Ok(false);
        }
        if self.structure.is_none() {
            match Structure::build(call.design, &call.cancel)? {
                Some(st) => self.structure = Some(st),
                None => {
                    self.refused = true;
                    return Ok(false);
                }
            }
        }
        if call.action == Action::Eliminate && self.cells.is_none() {
            let st = self.structure.as_ref().expect("structure");
            let other = usize::from(st.largest == 0);
            let pair = [st.largest, other];
            let effects = call.design.effects();
            self.cells = Some(Cells::build(st, effects, pair, &call.cancel)?);
        }
        Ok(true)
    }

    /// An FE preconditioner handle of `choice` for `weights` (see
    /// [`crate::Projector::fe_preconditioner`]).
    pub(crate) fn fe_preconditioner(
        &mut self,
        design: &Design<'_>,
        request: (InputWeights<'_>, crate::projector::Preconditioner),
        memory_limit: Option<u128>,
        cancel: &Cancel<'_>,
    ) -> Result<std::result::Result<FePreconditioner, Decline>> {
        FePreconditioner::build(self, design, request, memory_limit, cancel)
    }

    /// The handle's weight-independent workspace bound (see
    /// [`crate::Projector::fe_preconditioner_bytes`]).
    pub(crate) fn fe_preconditioner_bytes(
        &mut self,
        design: &Design<'_>,
        choice: crate::projector::Preconditioner,
        memory_limit: Option<u128>,
        cancel: &Cancel<'_>,
    ) -> Result<std::result::Result<u128, Decline>> {
        FePreconditioner::estimate(self, design, choice, memory_limit, cancel)
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
        let width = values.ncols().clamp(1, BATCH);
        let minimum = minimum_columns(call.method, values.ncols());
        let bytes = admission_bytes(st, call.method, minimum, weighted)
            + self.cells.as_ref().map_or(0, |c| cells_bytes(c, width));
        if request.memory_limit.is_some_and(|limit| bytes > limit) {
            return Ok(untouched(Decline::MemoryLimit, facts));
        }
        if self.weights.as_ref().is_none_or(|(k, _)| *k != key) {
            let mut state = self.weights.take().map(|(_, s)| s).unwrap_or_default();
            let roots = call.method == Method::Lsmr;
            self.generation += 1;
            if !state.fill(st, effects, validated, roots, &mut self.work.scatter)? {
                return Ok(untouched(Decline::Weights, facts));
            }
            if let Some(cells) = self.cells.as_mut() {
                cells.fill(st, effects, &state)?;
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
            generation: self.generation,
            hard: self.hard,
            cells: self.cells.as_ref(),
            cap: request.max_iterations.clamp(1, MAX_ITERATIONS),
            bytes,
            cancel: call.cancel,
        };
        let outputs = Outputs {
            work: &mut self.work,
            reduced: &mut self.reduced,
            schwarz: &mut self.schwarz,
            gram: request.residual_gram.then_some(&mut self.gram),
            norms: &mut self.norms,
            columns: &mut self.columns,
            reference: &mut self.reference,
        };
        let (outcome, gram) = match call.method {
            Method::Pcg => project_pcg(&job, &mut values, start, coefficients, outputs, facts)?,
            Method::Lsmr => lsmr::project(&job, &mut values, start, coefficients, outputs, facts)?,
        };
        let facts = match &outcome {
            Outcome::Projected(report) => report.facts,
            Outcome::Declined { facts, .. } => *facts,
        };
        self.hard |= facts.escalations > 0;
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
        let [sq, big, _] = load::<1>(
            st,
            (effects, state.basis()),
            src,
            None,
            state.weights(),
            &mut t,
            &mut rows,
        );
        if !(sq[0].is_finite() && big[0].is_finite()) {
            return Err(Error::InvalidData("derived residual values must be finite"));
        }
        let t = Rows {
            data: &t,
            stride: 1,
            first: 0,
        };
        adjoint::<1>(
            st,
            (effects, state.basis()),
            t,
            None,
            &mut moments,
            &mut scatter,
        );
        let reference = amplification * sq[0].sqrt();
        if !reference.is_finite() {
            return Err(Error::InvalidData("derived residual scale is not finite"));
        }
        let mut fold = MomentFold::default();
        let mut rhs_inf = [0.0];
        for (e, effect) in effects.iter().enumerate() {
            if let Some(blocks) = state.blocks().filter(|_| effect.sloped()) {
                let scales = (&[reference][..], state.raw_scale);
                blocks.fold::<1>(st, e, &moments, scales, (&mut fold, &mut rhs_inf))?;
                continue;
            }
            for (moment, &mass) in moments[st.offsets[e]..st.offsets[e + 1]]
                .iter()
                .zip(&state.mass[st.offsets[e]..])
            {
                fold.level(
                    mass,
                    &[moment / mass],
                    &[reference],
                    &mut rhs_inf,
                    state.raw_scale,
                )?;
            }
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
    if let Some(cells) = job.cells {
        let b = cells.effects[1];
        facts.reduced = job.st.offsets[b + 1] - job.st.offsets[b];
        facts.cells = cells.cells();
    }
    facts
}

fn same_projection(scales: &[f64]) -> Result<InputState> {
    let mut reference_scales = with_capacity(scales.len())?;
    reference_scales.extend_from_slice(scales);
    Ok(InputState::SameProjection { reference_scales })
}

/// The route actually used: Schwarz when any recurrence used it.
fn route(job: &Job<'_, '_>, schwarz: Option<&Schwarz>, escalations: usize) -> Route {
    if let Some(cells) = job.cells {
        return Route {
            reduction: ReductionUsed::Eliminated {
                effect: cells.effects[0],
            },
            method: MethodUsed::Pcg,
            preconditioner: PreconditionerUsed::ChannelBlocks {
                singular: u32::try_from(cells.singular()).unwrap_or(u32::MAX),
            },
            escalations: 0,
        };
    }
    Route {
        reduction: ReductionUsed::Full,
        method: match job.method {
            Method::Pcg => MethodUsed::Pcg,
            Method::Lsmr => MethodUsed::Lsmr,
        },
        preconditioner: match (schwarz, job.action, job.state.blocks()) {
            (Some(schwarz), _, _) => schwarz.used(),
            (None, Action::Identity, _) => PreconditionerUsed::Identity,
            (None, _, Some(blocks)) => PreconditionerUsed::ChannelBlocks {
                singular: u32::try_from(blocks.singular()).unwrap_or(u32::MAX),
            },
            (None, _, None) => PreconditionerUsed::Jacobi,
        },
        escalations: u8::try_from(escalations).unwrap_or(u8::MAX),
    }
}

/// Build the Schwarz preconditioner `spec` for the current weights unless
/// it is current, with the apply workspace of `width` columns, within the
/// memory limit beyond the projection's workspace. A stale one is dropped
/// first.
fn ensure(
    job: &Job<'_, '_>,
    spec: SchwarzSpec,
    slot: &mut Option<Schwarz>,
    width: usize,
    facts: &mut Facts,
) -> Result<std::result::Result<(), Refusal>> {
    let budget = match job.request.memory_limit {
        Some(limit) => match limit.checked_sub(job.bytes) {
            Some(budget) => Some(budget),
            None => return Ok(Err(Refusal::MemoryLimit)),
        },
        None => None,
    };
    if let Some(schwarz) = slot.as_ref().filter(|s| s.current(spec, job.generation)) {
        let bytes = schwarz.bytes() + schwarz.scratch_bytes(width);
        return Ok(if budget.is_none_or(|budget| bytes <= budget) {
            Ok(())
        } else {
            Err(Refusal::MemoryLimit)
        });
    }
    *slot = None;
    let started = Instant::now();
    let built = Schwarz::build(
        job.st,
        job.design.effects(),
        job.state,
        (spec, job.generation),
        (budget, width),
        &job.cancel,
    );
    facts.block_seconds += started.elapsed().as_secs_f64();
    Ok(built?.map(|schwarz| *slot = Some(schwarz)))
}

/// The first recurrence's mode; a decline when an explicit Schwarz
/// preconditioner does not build. Automatic Schwarz that does not build is
/// Jacobi; an escalating projection of a design that escalated before
/// starts on Schwarz when it builds.
fn initial_mode(
    job: &Job<'_, '_>,
    slot: &mut Option<Schwarz>,
    width: usize,
    facts: &mut Facts,
) -> Result<std::result::Result<Mode, Decline>> {
    let default = SchwarzSpec::default();
    Ok(Ok(match job.action {
        Action::Jacobi => Mode::Jacobi,
        Action::Identity => Mode::Identity,
        Action::Schwarz(spec) => match ensure(job, spec, slot, width, facts)? {
            Ok(()) => Mode::Schwarz,
            Err(Refusal::MemoryLimit) => return Ok(Err(Decline::MemoryLimit)),
            Err(Refusal::Numerical(reason)) => return Ok(Err(Decline::Numerical(reason))),
        },
        Action::Auto => match ensure(job, default, slot, width, facts)? {
            Ok(()) => Mode::Schwarz,
            Err(_) => Mode::Jacobi,
        },
        Action::Escalate => {
            if job.hard && ensure(job, default, slot, width, facts)?.is_ok() {
                Mode::Schwarz
            } else {
                Mode::Escalate
            }
        }
        Action::Eliminate => Mode::Jacobi,
    }))
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

/// A successful projection's report; `schwarz` when a recurrence used it
/// (with its workspace for `width` columns).
fn report(
    job: &Job<'_, '_>,
    (certificate, facts): (Certificate, Facts),
    started: Instant,
    (schwarz, width): (Option<&Schwarz>, usize),
) -> Report {
    let mut facts = with_structure(facts, job);
    facts.solve_seconds = started.elapsed().as_secs_f64();
    Report {
        route: route(job, schwarz, facts.escalations),
        facts,
        certificate,
        weight_scale: job.state.raw_scale,
        workspace_bytes: job.bytes + schwarz.map_or(0, |s| s.peak_bytes(width)),
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

/// Fold every level's recomputed moment at `max(reference, ||r||)` (a
/// blocked level: its channel-span norm).
fn certify<const R: usize>(
    (st, effects, state): (&Structure, &[Effect<'_>], &State),
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
    let mut levels =
        |range: std::ops::Range<usize>, fold: &mut MomentFold, out: &mut Certified<R>| {
            for g in range {
                let mass = state.mass[g];
                let inverse = state.inverse_mass[g];
                for j in 0..R {
                    means[j] = moments[g * R + j] * inverse;
                }
                let scales = &out.scales;
                if fold
                    .level(mass, &means, scales, &mut out.relative, state.raw_scale)
                    .is_err()
                {
                    return false;
                }
            }
            true
        };
    let Some(blocks) = state.blocks() else {
        if !levels(0..moments.len() / R, &mut fold, &mut out) {
            return out;
        }
        if fold.relative_infinity_norm <= tolerance {
            out.fold = Some(fold);
        }
        return out;
    };
    for (e, effect) in effects.iter().enumerate() {
        let folded = if effect.sloped() {
            let scales = (&out.scales[..], state.raw_scale);
            let target = (&mut fold, &mut out.relative[..]);
            blocks.fold::<R>(st, e, moments, scales, target).is_ok()
        } else {
            levels(st.offsets[e]..st.offsets[e + 1], &mut fold, &mut out)
        };
        if !folded {
            return out;
        }
    }
    if fold.relative_infinity_norm <= tolerance {
        out.fold = Some(fold);
    }
    out
}

/// `f(e, internal, within)` for every coefficient in effect and level
/// order: `internal` its index in the level-major internal layout, `within`
/// its channel-major position in its effect (`channel * levels + level`).
fn each_coefficient(
    st: &Structure,
    effects: &[Effect<'_>],
    mut f: impl FnMut(usize, usize, usize),
) {
    for (e, effect) in effects.iter().enumerate() {
        let (levels, channels) = (effect.levels(), effect.channels());
        for l in 0..levels {
            for c in 0..channels {
                f(e, st.offsets[e] + l * channels + c, c * levels + l);
            }
        }
    }
}

/// Add column `j`'s coefficients `x[g * stride + offset]` (internal
/// layout and basis) to the caller's buffer at its (channel-major) layout
/// in the raw basis: an intercept less its offset slopes' origins times
/// their coefficients.
fn add_coefficients(
    (st, (effects, basis)): (&Structure, (&[Effect<'_>], Basis<'_>)),
    x: &[f64],
    (stride, offset): (usize, usize),
    out: &mut CoefficientsMut<'_>,
    column: usize,
) {
    let layout = out.layout;
    let base = column * layout.per_rhs;
    let values = &mut *out.values;
    let offsets = basis.offsets();
    each_coefficient(st, effects, |e, g, within| {
        let mut value = x[g * stride + offset];
        if offsets && within < effects[e].levels() {
            for (s, &origin) in basis.level(e, within).iter().enumerate() {
                if origin != 0.0 {
                    value -= origin * x[(g + 1 + s) * stride + offset];
                }
            }
        }
        values[base + layout.effect_offsets[e] + within] += value;
    });
}

/// The warm start of columns `first..first + R` (canonical layout and raw
/// basis, `m` values per column) into `x` (internal layout and basis, `R`
/// per coefficient): an intercept plus its offset slopes' origins times
/// their coefficients.
fn load_start<const R: usize>(
    (st, (effects, basis)): (&Structure, (&[Effect<'_>], Basis<'_>)),
    start: &[f64],
    first: usize,
    x: &mut [f64],
) {
    let m = st.coefficients();
    each_coefficient(st, effects, |e, g, within| {
        for (j, x) in x[g * R..(g + 1) * R].iter_mut().enumerate() {
            *x = start[(first + j) * m + st.offsets[e] + within];
        }
    });
    for (e, effect) in effects.iter().enumerate().filter(|_| basis.offsets()) {
        for l in 0..effect.levels() {
            let g = (st.offsets[e] + l * effect.channels()) * R;
            for (s, &origin) in basis.level(e, l).iter().enumerate() {
                if origin != 0.0 {
                    for j in 0..R {
                        x[g + j] += origin * x[g + (1 + s) * R + j];
                    }
                }
            }
        }
    }
}

/// The cold start in `x` (zero elsewhere): the largest effect's per-level
/// least-squares fit of `q = D'W y`, its level means `q_l / m_l` (an effect
/// with blocks: `M_l^-1 q_l`).
fn cold_start<const R: usize>(job: &Job<'_, '_>, q: &[f64], x: &mut [f64]) {
    let (st, state) = (job.st, job.state);
    let first = st.offsets[st.largest];
    if state
        .blocks()
        .is_some_and(|blocks| blocks.solve_effect::<R>(st.largest, first, q, x))
    {
        return;
    }
    for g in st.offsets[st.largest]..st.offsets[st.largest + 1] {
        let inverse = state.inverse_mass[g];
        for j in 0..R {
            x[g * R + j] = q[g * R + j] * inverse;
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
    /// Whether a recurrence used Schwarz (explicitly or escalated).
    schwarz: bool,
}

/// One PCG batch `first..first + R` and its outputs.
struct Batch<'b, 'v, 'c> {
    values: &'b mut ColumnsMut<'v>,
    /// The elimination's Schur workspace.
    reduced: &'b mut Work,
    first: usize,
    reference: Option<&'b [f64]>,
    start: Option<&'b [f64]>,
    coefficients: Option<&'b mut CoefficientsMut<'c>>,
    mode: Mode,
}

/// The starting iterate: the caller's (canonical layout) or, cold, the
/// largest effect's per-level fit of `t = W y` ([`cold_start`]), with every
/// other effect at zero.
fn starting_iterate<const R: usize>(job: &Job<'_, '_>, batch: &Batch<'_, '_, '_>, work: &mut Work) {
    let st = job.st;
    let effects = job.design.effects();
    if let Some(start) = batch.start {
        let design = (effects, job.state.basis());
        load_start::<R>((st, design), start, batch.first, &mut work.x);
        return;
    }
    let t = Rows {
        data: &work.t,
        stride: R,
        first: 0,
    };
    let design = (effects, job.state.basis());
    adjoint::<R>(st, design, t, None, &mut work.q, &mut work.scatter);
    work.x.fill(0.0);
    cold_start::<R>(job, &work.q, &mut work.x);
}

/// The batch's recurrence in its mode. An escalating Jacobi recurrence that
/// stalls, reaches the iteration ceiling or is predicted slow after its
/// pilot iterations (never one that breaks down) continues with Schwarz from
/// its current iterate on the true residual `D'W (y - D x)`, within the
/// remaining budget, when Schwarz builds within the memory limit; a slow one
/// whose Schwarz does not build continues with Jacobi.
fn recurrence<const R: usize>(
    job: &Job<'_, '_>,
    batch: &Batch<'_, '_, '_>,
    (work, slot): (&mut Work, &mut Option<Schwarz>),
    (targets, tally): (&Targets<R>, &mut Tally<R>),
    facts: &mut Facts,
) -> Result<Solve<R>> {
    let (st, state) = (job.st, job.state);
    let effects = job.design.effects();
    let mode = batch.mode;
    let started = Instant::now();
    let solved = {
        let schwarz = match mode {
            Mode::Schwarz => slot.as_ref(),
            _ => None,
        };
        if let Some(schwarz) = schwarz {
            schwarz.size(&mut work.schwarz, R)?;
        }
        let op = Normal {
            st,
            effects,
            state,
            jacobi: mode != Mode::Identity,
            schwarz,
            cells: None,
        };
        let pilot = mode == Mode::Escalate;
        pcg::<R>(&op, work, targets, (job.cap, pilot), &job.cancel, tally)?
    };
    facts.pcg_seconds += started.elapsed().as_secs_f64();
    let remaining = job.cap.saturating_sub(tally.iterations);
    let escalate = mode == Mode::Escalate
        && remaining > 0
        && matches!(solved, Solve::Declined(STALLED | CEILING | pcg::SLOW));
    if !escalate {
        return Ok(solved);
    }
    let built = ensure(job, SchwarzSpec::default(), slot, R, facts)?.is_ok();
    if !built && !matches!(solved, Solve::Declined(pcg::SLOW)) {
        return Ok(solved);
    }
    let started = Instant::now();
    let src = Rows {
        data: batch.values.as_slice(),
        stride: batch.values.ncols(),
        first: batch.first,
    };
    let weights = state.weights();
    load::<R>(
        st,
        (effects, state.basis()),
        src,
        Some(&work.x),
        weights,
        &mut work.t,
        &mut work.rows,
    );
    let t = Rows {
        data: &work.t,
        stride: R,
        first: 0,
    };
    adjoint::<R>(
        st,
        (effects, state.basis()),
        t,
        None,
        &mut work.r,
        &mut work.scatter,
    );
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    let started = Instant::now();
    let schwarz = slot.as_ref().filter(|_| built);
    if let Some(schwarz) = schwarz {
        schwarz.size(&mut work.schwarz, R)?;
    }
    let op = Normal {
        st,
        effects,
        state,
        jacobi: true,
        schwarz,
        cells: None,
    };
    let solved = pcg::<R>(&op, work, targets, (remaining, false), &job.cancel, tally)?;
    facts.pcg_seconds += started.elapsed().as_secs_f64();
    facts.escalations += usize::from(built);
    Ok(solved)
}

/// The elimination with slopes: PCG on the Schur complement of `cells` from
/// the larger effect's per-level fit in `work.x` (the cold start), given the
/// true residual `work.r` of every coefficient; the smaller effect's iterate
/// and the larger effect's back-substitution `x_a -= M_a^-1 X x_b` go into
/// `work.x`.
fn eliminated<const R: usize>(
    job: &Job<'_, '_>,
    cells: &Cells,
    (work, reduced): (&mut Work, &mut Work),
    (targets, tally): (&Targets<R>, &mut Tally<R>),
    facts: &mut Facts,
) -> Result<Solve<R>> {
    let (st, state) = (job.st, job.state);
    let [a, b] = cells.effects;
    let small = st.offsets[b] * R..st.offsets[b + 1] * R;
    let large = st.offsets[a] * R..st.offsets[a + 1] * R;
    reduced.size_reduced(cells, st.offsets[b + 1] - st.offsets[b], R)?;
    reduced.x.copy_from_slice(&work.x[small.clone()]);
    reduced.r.copy_from_slice(&work.r[small.clone()]);
    // The cells work in the levels' centered bases.
    if let Some(blocks) = state.blocks() {
        blocks.convert::<R>(b, &mut reduced.r, false);
    }
    let started = Instant::now();
    let op = Normal {
        st,
        effects: job.design.effects(),
        state,
        jacobi: true,
        schwarz: None,
        cells: Some(cells),
    };
    let solved = pcg::<R>(&op, reduced, targets, (job.cap, false), &job.cancel, tally)?;
    facts.pcg_seconds += started.elapsed().as_secs_f64();
    work.x[small.clone()].copy_from_slice(&reduced.x);
    let actions = cells::Actions::of(st, state, a, cells.channels[0]);
    let beta = &mut work.q[large.clone()];
    let lent = (&mut [][..], beta);
    cells.larger::<R>(
        actions,
        (None, Some(&reduced.x)),
        lent,
        &mut reduced.scatter,
    );
    if let Some(blocks) = state.blocks() {
        blocks.convert::<R>(b, &mut work.x[small], true);
        blocks.convert::<R>(a, &mut work.q[large.clone()], true);
    }
    let Work { x, q, .. } = work;
    for (x, beta) in x[large.clone()].iter_mut().zip(&q[large]) {
        *x -= beta;
    }
    Ok(solved)
}

/// One batch: the starting residual and the recurrence read the columns;
/// only a converged recurrence forms the residual in place.
fn run<const R: usize>(
    job: &Job<'_, '_>,
    batch: Batch<'_, '_, '_>,
    (work, slot): (&mut Work, &mut Option<Schwarz>),
    facts: &mut Facts,
) -> Result<Step> {
    let (st, state) = (job.st, job.state);
    let effects = job.design.effects();
    let ncols = batch.values.ncols();
    work.size(st, R)?;
    work.size_blocks(state, R)?;
    let started = Instant::now();
    let src = Rows {
        data: batch.values.as_slice(),
        stride: ncols,
        first: batch.first,
    };
    if batch.start.is_none() {
        load::<R>(
            st,
            (effects, state.basis()),
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
        (effects, state.basis()),
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
    adjoint::<R>(
        st,
        (effects, state.basis()),
        t,
        None,
        &mut work.r,
        &mut work.scatter,
    );
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    let Some((scale, targets)) = targets::<R>(job, &stats, batch.reference) else {
        return Ok(Step::Declined("RHS scale outside the plain-sum range"));
    };
    let mut tally = Tally::default();
    let escalations = facts.escalations;
    let solved = match job.cells {
        Some(cells) => {
            let lent = (&mut *work, &mut *batch.reduced);
            eliminated::<R>(job, cells, lent, (&targets, &mut tally), facts)?
        }
        None => recurrence::<R>(job, &batch, (work, slot), (&targets, &mut tally), facts)?,
    };
    let schwarz = batch.mode == Mode::Schwarz || facts.escalations > escalations;
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
            add_coefficients(
                (st, (effects, state.basis())),
                &work.x,
                (R, j),
                coefficients,
                batch.first + j,
            );
        }
    }
    let target = (batch.values.as_mut_slice(), ncols, batch.first);
    let sq = residualize::<R>(
        st,
        (effects, state.basis()),
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
    adjoint::<R>(
        st,
        (effects, state.basis()),
        t,
        None,
        &mut work.q,
        &mut work.scatter,
    );
    let tolerance = job.request.tolerance;
    let certified = certify::<R>((st, effects, state), &work.q, sq, scale, tolerance);
    facts.residual_seconds += started.elapsed().as_secs_f64();
    let mut out = Formed {
        certified: certified.fold,
        norms: [0.0; BATCH],
        reference: [0.0; BATCH],
        relative: [0.0; BATCH],
        scales: [0.0; BATCH],
        iterations: [0; BATCH],
        floor: [false; BATCH],
        schwarz,
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
    lent: (&mut Work, &mut Option<Schwarz>),
    facts: &mut Facts,
) -> Result<Step> {
    match width {
        1 => run::<1>(job, batch, lent, facts),
        2 => run::<2>(job, batch, lent, facts),
        3 => run::<3>(job, batch, lent, facts),
        4 => run::<4>(job, batch, lent, facts),
        5 => run::<5>(job, batch, lent, facts),
        6 => run::<6>(job, batch, lent, facts),
        7 => run::<7>(job, batch, lent, facts),
        8 => run::<8>(job, batch, lent, facts),
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
/// twice). A batch that escalated keeps Schwarz for its corrections and the
/// later batches. Also reports whether the residual Gram was formed.
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
        reduced,
        schwarz: slot,
        gram,
        norms,
        columns,
        reference: formed_scales,
    } = outputs;
    let started = Instant::now();
    let mut initial = match initial_mode(job, slot, ncols.min(BATCH), &mut facts)? {
        Ok(mode) => mode,
        Err(reason) => return Ok((untouched(reason, with_structure(facts, job)), false)),
    };
    let mut fold = MomentFold::default();
    for buffer in [&mut *norms, &mut *formed_scales] {
        buffer.clear();
        reserve(buffer, ncols)?;
    }
    columns.clear();
    reserve(columns, ncols)?;
    formed_scales.resize(ncols, f64::NAN);
    facts.columns = ncols;
    let mut used = false;
    let mut first = 0;
    while first < ncols {
        let width = (ncols - first).min(BATCH);
        let mut reference: Option<[f64; BATCH]> = None;
        let mut attempts = 0;
        let mut iterations = [0_usize; BATCH];
        let mut mode = initial;
        loop {
            let batch = Batch {
                values: &mut *values,
                reduced: &mut *reduced,
                first,
                reference: reference.as_ref().map(|r| &r[..width]),
                start: start.filter(|_| attempts == 0),
                coefficients: coefficients.as_mut(),
                mode,
            };
            let formed = match dispatch(job, width, batch, (&mut *work, &mut *slot), &mut facts)? {
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
            if formed.schwarz {
                used = true;
                mode = Mode::Schwarz;
                initial = Mode::Schwarz;
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
    let schwarz = (used, ncols.min(BATCH));
    let schwarz = (slot.as_ref().filter(|_| schwarz.0), schwarz.1);
    Ok((
        Outcome::Projected(report(job, (certificate, facts), started, schwarz)),
        gram_formed,
    ))
}
