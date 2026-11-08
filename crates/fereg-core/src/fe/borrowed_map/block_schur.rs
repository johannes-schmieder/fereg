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
//! Memory: every allocation is fallible. The cache keeps the geometry, the
//! weight state and one workspace per batch width, so repeated projections of
//! one width with changing weights allocate nothing (unless the caller asks
//! for the residual norms attached to each result).

use super::*;
use crate::fe::LevelIndex;
use std::ops::Range;

/// Developer switch: `FEREG_BLOCK_SCHUR=0` keeps the established routes.
pub(crate) fn enabled() -> bool {
    #[cfg(test)]
    if TEST_DISABLED.load(std::sync::atomic::Ordering::SeqCst) > 0 {
        return false;
    }
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("FEREG_BLOCK_SCHUR").map_or(true, |v| v != "0"))
}

#[cfg(test)]
static TEST_DISABLED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Tests of the established routes' own machinery disable the elimination
/// while the guard lives. The switch is process-wide, so the guard also holds
/// an exclusive lock that tests comparing repeated runs share
/// ([`stable_route_for_test`]); their runs never straddle a switch.
#[cfg(test)]
pub(crate) struct DisabledForTest(#[allow(dead_code)] std::sync::RwLockWriteGuard<'static, ()>);

#[cfg(test)]
static TEST_ROUTE_LOCK: std::sync::RwLock<()> = std::sync::RwLock::new(());

#[cfg(test)]
pub(crate) fn disable_for_test() -> DisabledForTest {
    let lock = TEST_ROUTE_LOCK
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    TEST_DISABLED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    DisabledForTest(lock)
}

#[cfg(test)]
impl Drop for DisabledForTest {
    fn drop(&mut self) {
        TEST_DISABLED.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Shared side of the route lock for tests that compare repeated runs.
#[cfg(test)]
pub(crate) fn stable_route_for_test() -> std::sync::RwLockReadGuard<'static, ()> {
    TEST_ROUTE_LOCK
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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

mod kernels;
mod passes;
mod pcg;
mod structure;
mod weights;

use kernels::{Ctx, Slotted, Work, WorkSlots};
use passes::{recover, residualize, rhs};
use pcg::{pcg, Solve, Targets};
pub(crate) use structure::Structure;
use structure::{filled, reserve, with_capacity};
pub(crate) use weights::Weights;
use weights::{weight_key, WeightKey};

/// Caller options of one projection.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Request {
    /// Also return the FE coefficients whose fit forms the residual.
    pub(crate) retain_coefficients: bool,
    /// Also bound every level's Jacobi coefficient-error estimate (callers
    /// that check fitted predictors row by row, such as PPML).
    pub(crate) coefficient_accuracy: bool,
}

/// Raw-weighted Gram `Σ_i w_i r_i r_i'` of a certified residual matrix:
/// row-major and symmetric, `columns x columns`. Plain sums within each row
/// lane, combined in lane order (deterministic at a fixed worker count).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ResidualGram {
    pub(crate) columns: usize,
    pub(crate) values: Vec<f64>,
}

/// Outcome of one projection request.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Outcome<V> {
    Done(CertifiedBorrowedMap),
    /// The route declined; see [`project`] for the state of the input.
    Declined(V),
}

/// A session's retained geometry, its current-weight state and the reusable
/// workspaces of its projections.
#[derive(Default)]
pub(crate) struct Cache {
    refused: bool,
    structure: Option<Structure>,
    /// Weights keyed by the borrowed weight slice (address, length); `None`
    /// key is unit weights.
    weights: Option<(WeightKey, Weights)>,
    work: WorkSlots,
    /// Accumulate the residual Gram in later projections' residual pass.
    gram_requested: bool,
    gram: ResidualGram,
    gram_ready: bool,
    /// Residual norms of the last certified projection.
    norms: Vec<f64>,
    /// Keep the norms here instead of attaching a copy to each result.
    detach_norms: bool,
}

impl Cache {
    /// Also accumulate the residual Gram `Σ w r r'` at the caller's raw
    /// weights (unit weights: 1) in the residual pass of later projections
    /// with at most [`BATCH`] columns; see [`Cache::residual_gram`].
    #[allow(dead_code)] // For the PPML pre-solve's normal equations.
    pub(crate) fn request_residual_gram(&mut self, on: bool) {
        self.gram_requested = on;
    }

    /// The residual Gram of the last [`Cache::project`] call, when it was
    /// requested and the projection certified.
    #[allow(dead_code)] // For the PPML pre-solve's normal equations.
    pub(crate) fn residual_gram(&self) -> Option<&ResidualGram> {
        self.gram_ready.then_some(&self.gram)
    }

    /// Keep the residual norms of later certified projections only in the
    /// cache ([`Cache::residual_norms`]) instead of attaching a copy to each
    /// result, so repeated projections allocate nothing.
    #[allow(dead_code)] // For allocation-free PPML iterations.
    pub(crate) fn detach_residual_norms(&mut self, on: bool) {
        self.detach_norms = on;
    }

    /// `sqrt(Σ (w / max w) r^2)` per column of the last certified projection
    /// (empty after a decline).
    #[allow(dead_code)] // For allocation-free PPML iterations.
    pub(crate) fn residual_norms(&self) -> &[f64] {
        &self.norms
    }

    /// Project with the retained geometry, building it on first use. Any
    /// refusal is remembered so later calls go straight to the caller's route.
    pub(crate) fn project(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        map: MapConfig,
        budget: Option<FeSolverMemoryBudget>,
        request: Request,
    ) -> Result<Outcome<DenseMatrix>> {
        self.gram_ready = false;
        self.norms.clear();
        if self.refused || values.nrows() != design.nobs() {
            return Ok(Outcome::Declined(values));
        }
        let ledger = crate::diagnostics::phase_ledger::PhaseLedger::new(
            "block_schur_setup",
            ["structure", "weights"],
        );
        if self.structure.is_none() {
            self.structure = Structure::build(design)?;
            if self.structure.is_none() {
                self.refused = true;
                return Ok(Outcome::Declined(values));
            }
        }
        ledger.lap(0);
        let st = self.structure.as_ref().expect("structure");
        let validated = validate_weights(weights, values.nrows())?;
        let key = weight_key(validated.values)?;
        if budget.is_some_and(|b| !b.fits(admission_bytes(st, None, values.ncols()))) {
            return Ok(Outcome::Declined(values));
        }
        if self.weights.as_ref().is_none_or(|(k, _)| *k != key) {
            let recycled = self.weights.take().map(|(_, w)| w);
            match Weights::prepare(st, design, validated, recycled)? {
                Some(w) => self.weights = Some((key, w)),
                None => return Ok(Outcome::Declined(values)),
            }
        }
        ledger.lap(1);
        let w = &self.weights.as_ref().expect("weights").1;
        let mut bytes = admission_bytes(st, Some(w), values.ncols());
        if request.retain_coefficients {
            bytes += (st.reduced + st.segments) as u128 * values.ncols() as u128 * 8;
        }
        let outputs = Outputs {
            slots: &mut self.work,
            gram: self.gram_requested.then_some(&mut self.gram),
            norms: &mut self.norms,
            detach_norms: self.detach_norms,
        };
        let (outcome, gram) = project(
            st, w, design, validated, values, map, bytes, request, outputs,
        )?;
        match outcome {
            Outcome::Done(_) => self.gram_ready = gram,
            Outcome::Declined(_) => self.norms.clear(),
        }
        Ok(outcome)
    }

    /// Recompute every level's weighted moment of one residual column in one
    /// observation pass and certify it at `amplification` times its weighted
    /// norm (the derived scale of SPEC §16). `None` when this cache did not
    /// project with these weights.
    pub(crate) fn certify_derived(
        &self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        values: &DenseMatrix,
        tolerance: f64,
        amplification: f64,
    ) -> Result<Option<FeCertificate>> {
        let (Some(st), Some((key, w))) = (self.structure.as_ref(), self.weights.as_ref()) else {
            return Ok(None);
        };
        if values.ncols() != 1 || values.nrows() != st.nobs || design.nobs() != st.nobs {
            return Ok(None);
        }
        let validated = validate_weights(weights, values.nrows())?;
        if *key != weight_key(validated.values)? {
            return Ok(None);
        }
        if !(amplification.is_finite() && amplification >= 1.0) {
            return Err(invalid_argument(
                "derived residual amplification is invalid",
            ));
        }
        let elim_codes = design.effects()[st.eliminated].codes();
        let mut block_codes: Vec<&[u32]> = with_capacity(st.blocks.len())?;
        block_codes.extend(st.blocks.iter().map(|&b| design.effects()[b].codes()));
        let data = values.as_slice();
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
            return Err(invalid_data("derived residual values must be finite"));
        }
        let reference = amplification * total.sq.sqrt();
        if !reference.is_finite() {
            return Err(invalid_data("derived residual scale is not finite"));
        }
        let scales = [reference];
        let mut rhs_inf = [0.0];
        let mut fold = CertificateFold::default();
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

/// Per-call timing and iteration facts (trace only).
#[derive(Default, Clone, Copy)]
struct Facts {
    iterations: usize,
    /// Sum over batches and iterations of the active columns.
    column_iterations: usize,
    /// Columns stopped at the acceptance rule after a breakdown.
    accepted: usize,
    rhs_seconds: f64,
    pcg_seconds: f64,
    residual_seconds: f64,
}

enum Step {
    /// Declined before any residual was formed.
    Declined(&'static str),
    /// The residual of this batch was formed in place.
    Formed {
        fold: Option<CertificateFold>,
        norms: [f64; BATCH],
        reference: [f64; BATCH],
        /// Whether the residual Gram was formed.
        gram: bool,
    },
}

/// One batch `first..first+R`: RHS and recurrence read `values`; only a
/// converged recurrence forms the residual in place.
#[allow(clippy::too_many_arguments)]
fn run<const R: usize, const NB: usize, const UNIT: bool, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    values: &mut DenseMatrix,
    first: usize,
    reference: Option<&[f64]>,
    tolerance: f64,
    cap: usize,
    facts: &mut Facts,
    coefficients: Option<&mut FeCoefficientBatch>,
    slots: &mut WorkSlots,
    gram: Option<&mut ResidualGram>,
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
    // Coefficient accuracy: Jacobi error estimates within the tolerance of
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
    let solved = pcg::<R, NB, UNIT>(st, ctx.w, work, &targets, cap)?;
    facts.pcg_seconds += started.elapsed().as_secs_f64();
    let iterations = match solved {
        Solve::Converged(iterations, columns, accepted) => {
            facts.column_iterations += columns;
            facts.accepted += accepted;
            iterations
        }
        Solve::Declined(reason) => return Ok(Step::Declined(reason)),
    };
    facts.iterations = facts.iterations.max(iterations);
    let started = Instant::now();
    recover::<R, NB, UNIT>(st, ctx.w, work);
    if let Some(batch) = coefficients {
        // Corrections add their increments to the coefficients of the
        // preceding solve, so the sum fits the original columns.
        let per = batch.coefficients_per_rhs;
        for j in 0..R {
            let base = (first + j) * per;
            let start = base + batch.effect_offsets[st.eliminated];
            for (value, alpha) in batch.values[start..start + st.segments]
                .iter_mut()
                .zip(&work.seg)
            {
                *value += alpha[j];
            }
            for (b, &effect) in st.blocks.iter().enumerate() {
                let start = base + batch.effect_offsets[effect];
                let levels = batch.level_counts[effect];
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
    let (fold, norms) = certify_batch::<R>(ctx.w, &work.lanes[0].acc, sq, scale, tolerance);
    facts.residual_seconds += started.elapsed().as_secs_f64();
    let mut padded_norms = [0.0; BATCH];
    padded_norms[..R].copy_from_slice(&norms);
    let mut padded_scale = [0.0; BATCH];
    padded_scale[..R].copy_from_slice(&scale);
    Ok(Step::Formed {
        fold,
        norms: padded_norms,
        reference: padded_scale,
        gram: formed,
    })
}

/// Fold the recomputed remaining-level moments at `max(reference, ||r||)`.
fn certify_batch<const R: usize>(
    w: &Weights,
    moments: &[[f64; R]],
    sq: [f64; R],
    reference: [f64; R],
    tolerance: f64,
) -> (Option<CertificateFold>, [f64; R]) {
    let mut fold = CertificateFold::default();
    let mut scales = [0.0; R];
    let mut norms = [0.0; R];
    for j in 0..R {
        norms[j] = sq[j].sqrt();
        scales[j] = reference[j].max(norms[j]);
        if !scales[j].is_finite() {
            return (None, norms);
        }
    }
    let mut rhs_inf = [0.0; R];
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
            return (None, norms);
        }
    }
    let passed = fold.relative_infinity_norm <= tolerance;
    (passed.then_some(fold), norms)
}

fn merge_fold(total: &mut CertificateFold, part: &CertificateFold) {
    total.absolute_infinity_norm = total
        .absolute_infinity_norm
        .max(part.absolute_infinity_norm);
    total.relative_infinity_norm = total
        .relative_infinity_norm
        .max(part.relative_infinity_norm);
    total.relative_square_sum += part.relative_square_sum;
    total.relative_count += part.relative_count;
}

#[allow(clippy::too_many_arguments)]
fn dispatch<const NB: usize>(
    ctx: &Ctx<'_, '_, NB>,
    values: &mut DenseMatrix,
    first: usize,
    width: usize,
    reference: Option<&[f64]>,
    tolerance: f64,
    cap: usize,
    facts: &mut Facts,
    coefficients: Option<&mut FeCoefficientBatch>,
    slots: &mut WorkSlots,
    gram: Option<&mut ResidualGram>,
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
        _ => Err(invalid_argument("block-elimination batch width is invalid")),
    }
}

/// The cache's reusable outputs and workspaces lent to one projection.
struct Outputs<'c> {
    slots: &'c mut WorkSlots,
    /// Requested residual Gram (formed only for at most [`BATCH`] columns).
    gram: Option<&'c mut ResidualGram>,
    /// Residual norms, one per column.
    norms: &'c mut Vec<f64>,
    detach_norms: bool,
}

/// Project the row-major RHS `values` against the design. A decline returns
/// the matrix: untouched, unless a later batch or a correction declined after
/// earlier residuals were formed, whose projection is then still the request.
/// Also reports whether the residual Gram was formed.
#[allow(clippy::too_many_arguments)]
fn project(
    st: &Structure,
    w: &Weights,
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: DenseMatrix,
    map: MapConfig,
    bytes: u128,
    request: Request,
    outputs: Outputs<'_>,
) -> Result<(Outcome<DenseMatrix>, bool)> {
    if values.nrows() != st.nobs || values.ncols() == 0 || design.nobs() != st.nobs {
        return Ok((Outcome::Declined(values), false));
    }
    match st.blocks.len() {
        1 => project_nb::<1>(st, w, design, weights, values, map, bytes, request, outputs),
        2 => project_nb::<2>(st, w, design, weights, values, map, bytes, request, outputs),
        3 => project_nb::<3>(st, w, design, weights, values, map, bytes, request, outputs),
        4 => project_nb::<4>(st, w, design, weights, values, map, bytes, request, outputs),
        _ => Ok((Outcome::Declined(values), false)),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_nb<const NB: usize>(
    st: &Structure,
    w: &Weights,
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    mut values: DenseMatrix,
    map: MapConfig,
    bytes: u128,
    request: Request,
    outputs: Outputs<'_>,
) -> Result<(Outcome<DenseMatrix>, bool)> {
    let mut ledger = crate::diagnostics::phase_ledger::PhaseLedger::new("block_schur", ["solve"]);
    let ncols = values.ncols();
    let Outputs {
        slots,
        gram,
        norms: residual_norms,
        detach_norms,
    } = outputs;
    let mut gram = gram.filter(|_| ncols <= BATCH);
    let ctx = Ctx::<NB> {
        st,
        w,
        index: design.effects()[st.eliminated].level_index(),
        elim_codes: design.effects()[st.eliminated].codes(),
        codes_b: std::array::from_fn(|b| design.effects()[st.blocks[b]].codes()),
        offs: std::array::from_fn(|b| st.offsets[b]),
        weights,
        coefficient_accuracy: request.coefficient_accuracy,
    };
    let tolerance = map.tolerance;
    let cap = map.max_iterations.clamp(1, MAX_ITERATIONS);
    let started = Instant::now();
    let mut facts = Facts::default();
    let mut fold = CertificateFold::default();
    residual_norms.clear();
    reserve(residual_norms, ncols)?;
    let mut corrections = 0;
    let mut gram_formed = false;
    let mut coefficients = if request.retain_coefficients {
        Some(FeCoefficientBatch::intercept_zeros(design, ncols)?)
    } else {
        None
    };
    let mut start = 0;
    while start < ncols {
        let width = (ncols - start).min(BATCH);
        let mut reference: Option<[f64; BATCH]> = None;
        let mut attempts = 0;
        loop {
            let step = dispatch::<NB>(
                &ctx,
                &mut values,
                start,
                width,
                reference.as_ref().map(|r| &r[..width]),
                tolerance,
                cap,
                &mut facts,
                coefficients.as_mut(),
                slots,
                gram.as_deref_mut(),
            )?;
            match step {
                Step::Declined(reason) => {
                    trace_decline(reason, facts);
                    return Ok((Outcome::Declined(values), false));
                }
                Step::Formed {
                    fold: Some(part),
                    norms,
                    gram: formed,
                    ..
                } => {
                    merge_fold(&mut fold, &part);
                    residual_norms.extend_from_slice(&norms[..width]);
                    gram_formed = formed;
                    break;
                }
                Step::Formed {
                    fold: None,
                    reference: scale,
                    ..
                } => {
                    // The residual is a valid input with the same projection;
                    // correct it in place at the original scale.
                    if attempts == 2 {
                        trace_decline("certificate failed after corrections", facts);
                        return Ok((Outcome::Declined(values), false));
                    }
                    attempts += 1;
                    corrections += 1;
                    reference = Some(scale);
                }
            }
        }
        start += width;
    }
    ledger.lap(0);
    ledger.facts(&[
        ("iterations", facts.iterations as u64),
        ("column_iterations", facts.column_iterations as u64),
        ("accepted_columns", facts.accepted as u64),
        ("columns", ncols as u64),
        ("cells", st.cells() as u64),
        ("active_segments", st.active.len() as u64),
        ("reduced", st.reduced as u64),
        ("lanes", st.lanes.len() as u64),
        ("corrections", corrections as u64),
        ("rhs_us", (facts.rhs_seconds * 1e6) as u64),
        ("pcg_us", (facts.pcg_seconds * 1e6) as u64),
        ("residual_us", (facts.residual_seconds * 1e6) as u64),
    ]);
    let certificate = fold.finish(tolerance)?;
    if !certificate.passed {
        trace_decline("combined certificate failed", facts);
        return Ok((Outcome::Declined(values), false));
    }
    let report = SolveReport {
        requested_solver: FeSolverKind::Auto,
        iterations: facts.iterations,
        solver_workspace_peak: SolverWorkspacePeak::from_bytes(bytes)?,
        // The certificate is fused into the residual pass, so its time is
        // part of the solve.
        solve_seconds: started.elapsed().as_secs_f64(),
        certification_seconds: 0.0,
        ..SolveReport::single_route(
            FeSolverKind::Elimination,
            SolveStopReason::Certified,
            certificate,
        )
    };
    let result = match coefficients {
        Some(coefficients) => CertifiedBorrowedMap::from_certified_parts_with_coefficients(
            values,
            report,
            coefficients,
        ),
        None => CertifiedBorrowedMap::from_certified_parts(values, report),
    }?;
    let result = if detach_norms {
        result
    } else {
        let mut attached = with_capacity(ncols)?;
        attached.extend_from_slice(residual_norms);
        result.with_residual_norms(attached)?
    };
    Ok((Outcome::Done(result), gram_formed))
}

fn trace_decline(reason: &str, facts: Facts) {
    if crate::diagnostics::solver_trace_enabled() || crate::diagnostics::phase_timing_enabled() {
        eprintln!(
            "FEREG_PHASE|block_schur_decline|reason={reason}|iterations={}|pcg_us={}",
            facts.iterations,
            (facts.pcg_seconds * 1e6) as u64
        );
    }
}

#[cfg(test)]
mod tests;
