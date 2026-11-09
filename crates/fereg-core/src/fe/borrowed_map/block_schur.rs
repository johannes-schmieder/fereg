//! fedemean's categorical intercept projections: the exact global
//! elimination of the largest effect (largest-effect Schur complement,
//! lockstep Jacobi PCG escalating to block-Jacobi with CMG local solves) and
//! the full system (PCG or modified LSMR with Jacobi, no or additive Schwarz
//! preconditioning), each with a fused residual and moment certificate.
//!
//! This adapter keeps fereg's side: the developer switch and test guards,
//! the borrowed design view, result construction (`CertifiedBorrowedMap`,
//! `SolveReport` labels and the route actually used), error mapping and the
//! phase and decline records. A decline returns the matrix to the caller's
//! route, and the cache remembers why ([`Cache::decline`]).

use super::*;
use crate::fe::LevelIndex;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// What a cache's projector solves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Plan {
    /// Auto's exact elimination: Jacobi escalating to block-Jacobi, started
    /// on the blocks once the prepared design escalated (the hint). Other
    /// effect counts decline.
    #[default]
    Elimination,
    /// A fixed fedemean choice, labelled as `requested` (with the requested
    /// LSMR preconditioner for modified LSMR).
    Fixed {
        choice: fedemean::Choice,
        requested: FeSolverKind,
        preconditioner: Option<LsmrPreconditioner>,
    },
}

impl Plan {
    /// Auto's full system: PCG with fedemean's automatic preconditioner,
    /// Jacobi escalating to additive Schwarz when the Jacobi recurrence is
    /// slow or stalls (and Schwarz fits the memory limit).
    pub(crate) const fn auto_full() -> Self {
        Self::Fixed {
            choice: fedemean::Choice {
                reduction: fedemean::Reduction::Full,
                method: fedemean::Method::Pcg,
                preconditioner: fedemean::Preconditioner::Auto,
                fallback: fedemean::Fallback::Escalate,
            },
            requested: FeSolverKind::Auto,
            preconditioner: None,
        }
    }

    /// Explicit modified LSMR on the full system with `preconditioner`; the
    /// automatic one escalates from Jacobi to Schwarz as Auto's does,
    /// explicit ones never escalate.
    pub(crate) const fn lsmr(preconditioner: LsmrPreconditioner) -> Self {
        Self::Fixed {
            choice: lsmr_choice(preconditioner),
            requested: FeSolverKind::Lsmr,
            preconditioner: Some(preconditioner),
        }
    }

    /// Explicit PCG with the requested reduction and preconditioner; the
    /// automatic preconditioner of the elimination escalates (Jacobi to
    /// block-Jacobi), explicit ones never do.
    pub(crate) fn pcg(request: crate::fe::PcgRequest) -> Result<Self> {
        use crate::fe::{PcgPreconditioner as P, PcgReduction as R};
        let reduction = match request.reduction {
            R::Auto => fedemean::Reduction::Auto,
            R::Full => fedemean::Reduction::Full,
            R::Eliminate => fedemean::Reduction::EliminateLargest,
        };
        let full = request.reduction == R::Full;
        let eliminate = request.reduction == R::Eliminate;
        let preconditioner = match request.preconditioner {
            P::Auto => fedemean::Preconditioner::Auto,
            P::Jacobi => fedemean::Preconditioner::Jacobi,
            P::None if !eliminate => fedemean::Preconditioner::Identity,
            P::Schwarz if !eliminate => fedemean::Preconditioner::Schwarz(fedemean::SchwarzSpec {
                subdomains: fedemean::Subdomains::Star,
                local: fedemean::SchwarzLocal::Auto,
            }),
            P::Block if !full => fedemean::Preconditioner::BlockJacobi(fedemean::BlockSpec::default()),
            P::Block => {
                return Err(crate::error::unsupported(
                    "solver(cg, reduce(full)) cannot use precondition(block): block-Jacobi preconditions the eliminated system; request reduce(eliminate)",
                ))
            }
            P::None | P::Schwarz => {
                return Err(crate::error::unsupported(
                    "solver(cg, reduce(eliminate)) accepts precondition(auto), precondition(jacobi) or precondition(block)",
                ))
            }
        };
        // Only the automatic choice escalates: Jacobi to block-Jacobi on the
        // elimination, Jacobi to Schwarz on the full system.
        let fallback = if request.preconditioner == P::Auto {
            fedemean::Fallback::Escalate
        } else {
            fedemean::Fallback::Decline
        };
        Ok(Self::Fixed {
            choice: fedemean::Choice {
                reduction,
                method: fedemean::Method::Pcg,
                preconditioner,
                fallback,
            },
            requested: FeSolverKind::Pcg,
            preconditioner: None,
        })
    }

    /// A trace or ledger name: the elimination's or the fixed plan's.
    fn ledger(self, elimination: &'static str, fixed: &'static str) -> &'static str {
        if self == Self::Elimination {
            elimination
        } else {
            fixed
        }
    }

    const fn requested(self) -> FeSolverKind {
        match self {
            Self::Elimination => FeSolverKind::Auto,
            Self::Fixed { requested, .. } => requested,
        }
    }
}

/// fedemean's modified LSMR on the full system with `preconditioner`: the
/// automatic one is Jacobi escalating to additive Schwarz (star subdomains,
/// automatic local solves) when slow or stalled; explicit ones never
/// escalate.
pub(crate) const fn lsmr_choice(preconditioner: LsmrPreconditioner) -> fedemean::Choice {
    fedemean::Choice {
        reduction: fedemean::Reduction::Full,
        method: fedemean::Method::Lsmr,
        preconditioner: match preconditioner {
            LsmrPreconditioner::Auto => fedemean::Preconditioner::Auto,
            LsmrPreconditioner::Off => fedemean::Preconditioner::Identity,
            LsmrPreconditioner::Diagonal => fedemean::Preconditioner::Jacobi,
            LsmrPreconditioner::Schwarz => {
                fedemean::Preconditioner::Schwarz(fedemean::SchwarzSpec {
                    subdomains: fedemean::Subdomains::Star,
                    local: fedemean::SchwarzLocal::Auto,
                })
            }
        },
        fallback: match preconditioner {
            LsmrPreconditioner::Auto => fedemean::Fallback::Escalate,
            _ => fedemean::Fallback::Decline,
        },
    }
}

/// Why the last projection of a cache declined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Decline {
    pub(crate) reason: fedemean::Decline,
    /// Iterations spent before the decline.
    pub(crate) iterations: usize,
}

impl Decline {
    /// The reason in words.
    pub(crate) fn describe(self) -> &'static str {
        match self.reason {
            fedemean::Decline::OutsideRoute => "the design is outside the route",
            fedemean::Decline::Shape => "the columns do not match the design",
            fedemean::Decline::MemoryLimit => "the solver workspace exceeds the memory limit",
            fedemean::Decline::Weights => "a fixed-effect level has no positive weight",
            fedemean::Decline::Numerical(reason) => reason,
            _ => "the route declined",
        }
    }
}

/// Caller options of one projection.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Request {
    /// Also return the FE coefficients whose fit forms the residual.
    pub(crate) retain_coefficients: bool,
    /// Also stop on every level's Jacobi coefficient-error proxy (callers
    /// that check fitted predictors row by row, such as PPML).
    pub(crate) coefficient_accuracy: bool,
}

/// Raw-weighted Gram `Σ_i w_i r_i r_i'` of a certified residual matrix:
/// row-major and symmetric, `columns x columns`.
pub(crate) use fedemean::Gram as ResidualGram;

/// Outcome of one projection request.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Outcome<V> {
    Done(CertifiedBorrowedMap),
    /// The route declined; see [`Cache::project`] for the state of the input.
    Declined(V),
}

/// A session's elimination projector (retained geometry, current-weight
/// state and workspaces) and its output switches.
#[derive(Default)]
pub(crate) struct Cache<'a> {
    plan: Plan,
    projector: Option<fedemean::Projector<'a>>,
    /// The prepared design's elimination hint, shared with its other caches
    /// (see [`Cache::set_hint`]).
    hint: Option<&'a AtomicBool>,
    /// The bound projector escalated: fedemean starts its later automatic
    /// batches on the blocks, so a hint needs no rebuild.
    escalated: bool,
    /// Hinted blocks exceeded the memory limit: this cache keeps the
    /// automatic route.
    blocks_refused: bool,
    /// The projector's outputs belong to the last [`Cache::project`] call.
    current: bool,
    /// Accumulate the residual Gram in later projections' residual pass.
    gram_requested: bool,
    /// Keep the norms in the projector instead of attaching a copy to each
    /// result.
    detach_norms: bool,
    /// Coefficient increments of the last call's columns when it declined
    /// after forming some of their residuals in place.
    partial: Option<FeCoefficientBatch>,
    /// The maximum weight that normalized the last certified projection.
    weight_scale: Option<f64>,
    /// Why the last call declined.
    decline: Option<Decline>,
}

impl<'a> Cache<'a> {
    /// A cache whose projector solves `plan`.
    pub(crate) fn with_plan(plan: Plan) -> Self {
        Self {
            plan,
            ..Self::default()
        }
    }

    /// Why the last [`Cache::project`] call declined (`None` after a
    /// certified projection).
    pub(crate) const fn decline(&self) -> Option<Decline> {
        self.decline
    }

    /// Drop the retained projector (its geometry, weight state and
    /// preconditioner); the next projection rebuilds it.
    pub(crate) fn release(&mut self) {
        self.projector = None;
        self.current = false;
        self.escalated = false;
    }

    /// Also accumulate the residual Gram `Σ w r r'` at the caller's raw
    /// weights (unit weights: 1) in the residual pass of later projections
    /// with at most eight columns; see [`Cache::residual_gram`].
    pub(crate) fn request_residual_gram(&mut self, on: bool) {
        self.gram_requested = on;
    }

    /// The residual Gram of the last [`Cache::project`] call, when it was
    /// requested and the projection certified.
    pub(crate) fn residual_gram(&self) -> Option<&ResidualGram> {
        self.projector
            .as_ref()
            .filter(|_| self.current)
            .and_then(fedemean::Projector::residual_gram)
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
        match self.projector.as_ref() {
            Some(projector) if self.current => projector.residual_norms(),
            _ => &[],
        }
    }

    /// Share the prepared design's elimination hint: once any of its caches
    /// escalated, projectors bound later start on the blocks
    /// (`BlockJacobi` at the default spec, which escalation also uses). A
    /// certified projection of an escalated projector sets it.
    pub(crate) fn set_hint(&mut self, hint: Option<&'a AtomicBool>) {
        self.hint = hint;
    }

    /// Project with the retained geometry, building it on first use. A
    /// decline returns the matrix: untouched, unless a later batch or a
    /// correction declined after earlier residuals were formed, whose
    /// projection is then still the request. A refusal of the design is
    /// remembered so later calls go straight to the caller's route.
    pub(crate) fn project(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        map: MapConfig,
        budget: Option<FeSolverMemoryBudget>,
        request: Request,
    ) -> Result<Outcome<DenseMatrix>> {
        let refused = self.blocks_refused;
        match self.attempt(design, weights, values, map, budget, request)? {
            // Hinted blocks beyond the memory limit decline untouched; the
            // same call then runs once on the automatic route.
            Outcome::Declined(values) if self.blocks_refused != refused => {
                self.attempt(design, weights, values, map, budget, request)
            }
            outcome => Ok(outcome),
        }
    }

    fn attempt(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'_>,
        mut values: DenseMatrix,
        map: MapConfig,
        budget: Option<FeSolverMemoryBudget>,
        request: Request,
    ) -> Result<Outcome<DenseMatrix>> {
        self.current = false;
        self.partial = None;
        self.weight_scale = None;
        self.decline = Some(Decline {
            reason: fedemean::Decline::Shape,
            iterations: 0,
        });
        if values.nrows() != design.nobs() || !(map.tolerance.is_finite() && map.tolerance > 0.0) {
            return Ok(Outcome::Declined(values));
        }
        let (detach, gram) = (self.detach_norms, self.gram_requested);
        let plan = self.plan;
        let hinted = plan == Plan::Elimination
            && !self.blocks_refused
            && self.hint.is_some_and(|h| h.load(Ordering::Relaxed));
        let projector = bind(
            &mut self.projector,
            design,
            settings(plan, hinted),
            &mut self.escalated,
        )?;
        // CMG local solves borrow the action's team (serial outside one).
        let host = crate::parallel::HostPool::current()?;
        // A developer LSMR Break witness observes the solve's polls only:
        // the structure is built first under the ordinary poll.
        let armed = plan.requested() == FeSolverKind::Lsmr
            && crate::fe::lsmr_break::armed(crate::fe::DeveloperLsmrBackend::Fedemean);
        let polls = std::sync::atomic::AtomicUsize::new(0);
        let witness = || crate::fe::lsmr_break::fedemean_poll(&polls);
        let poll: &(dyn Fn() -> bool + Sync) = if armed {
            projector
                .prepare(&fedemean::Control::with_cancel(
                    &crate::interrupt::cancellation_requested,
                ))
                .map_err(fedemean_error)?;
            &witness
        } else {
            &crate::interrupt::cancellation_requested
        };
        let mut control = fedemean::Control::with_cancel(poll);
        if let Some(host) = &host {
            control = control.with_pool(host.pool());
        }
        let setup = crate::diagnostics::phase_ledger::PhaseLedger::new(
            plan.ledger("block_schur_setup", "fedemean_setup"),
            ["structure", "weights"],
        );
        let mut coefficients = None;
        let mut structure_seconds = 0.0;
        if request.retain_coefficients {
            let started = std::time::Instant::now();
            if !projector.prepare(&control).map_err(fedemean_error)? {
                self.decline = Some(Decline {
                    reason: fedemean::Decline::OutsideRoute,
                    iterations: 0,
                });
                return Ok(Outcome::Declined(values));
            }
            structure_seconds = started.elapsed().as_secs_f64();
            coefficients = Some(FeCoefficientBatch::intercept_zeros(design, values.ncols())?);
        }
        let fe_request = fedemean_request(map, request, gram, budget);
        let fe_weights = match weights {
            BorrowedMapWeights::Unit => fedemean::Weights::Unit,
            BorrowedMapWeights::Provided(values) => fedemean::Weights::Values(values),
        };
        let (nrows, ncols) = (values.nrows(), values.ncols());
        let columns = fedemean::ColumnsMut::new(values.as_mut_slice(), nrows, ncols)
            .map_err(fedemean_error)?;
        let fe_coefficients = coefficients.as_mut().map(|batch| {
            fedemean::CoefficientsMut::new(
                &mut batch.values,
                // `effect_offsets` carries a trailing total.
                fedemean::CoefficientLayout::new(
                    batch.coefficients_per_rhs,
                    &batch.effect_offsets[..batch.level_counts.len()],
                    &batch.level_counts,
                ),
            )
        });
        let outcome = projector
            .project(fe_weights, columns, fe_coefficients, &fe_request, &control)
            .map_err(|error| {
                if armed && error == fedemean::Error::Cancelled {
                    crate::fe::lsmr_break::record_cancelled(
                        crate::fe::DeveloperLsmrBackend::Fedemean,
                    );
                }
                fedemean_error(error)
            })?;
        self.current = true;
        let facts = match &outcome {
            fedemean::Outcome::Projected(report) => report.facts,
            fedemean::Outcome::Declined { facts, .. } => *facts,
            _ => fedemean::Facts::default(),
        };
        self.escalated |= facts.escalations > 0;
        setup.add_seconds(0, structure_seconds + facts.structure_seconds);
        setup.add_seconds(1, facts.weights_seconds);
        let report = match outcome {
            fedemean::Outcome::Projected(report) => report,
            fedemean::Outcome::Declined { reason, input, .. } => {
                self.declined(hinted, (reason, &input), &facts, coefficients);
                return Ok(Outcome::Declined(values));
            }
            _ => return Ok(Outcome::Declined(values)),
        };
        self.decline = None;
        solve_record(plan, &facts);
        trace_escalation(plan, Some(&report), &facts);
        if let Some(hint) = self.hint.filter(|_| self.escalated) {
            hint.store(true, Ordering::Relaxed);
        }
        self.weight_scale = Some(report.weight_scale);
        let solve_report = solve_report(plan, &report, &facts)?;
        let result = match coefficients {
            Some(coefficients) => CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                values,
                solve_report,
                coefficients,
            ),
            None => CertifiedBorrowedMap::from_certified_parts(values, solve_report),
        }?;
        if detach {
            return Ok(Outcome::Done(result));
        }
        let norms = projector.residual_norms();
        let mut attached = Vec::new();
        attached
            .try_reserve_exact(norms.len())
            .map_err(|_| allocation("unable to reserve block-elimination workspace"))?;
        attached.extend_from_slice(norms);
        Ok(Outcome::Done(result.with_residual_norms(attached)?))
    }

    /// Record a decline: its reason, a refused hinted block build, the trace
    /// of a numerical decline, and the coefficient increments of columns
    /// already residualized in place.
    fn declined(
        &mut self,
        hinted: bool,
        (reason, input): (fedemean::Decline, &fedemean::InputState),
        facts: &fedemean::Facts,
        coefficients: Option<FeCoefficientBatch>,
    ) {
        self.decline = Some(Decline {
            reason,
            iterations: facts.iterations,
        });
        self.blocks_refused |= hinted && reason == fedemean::Decline::MemoryLimit;
        if let fedemean::Decline::Numerical(reason) = reason {
            solve_record(self.plan, facts);
            trace_escalation(self.plan, None, facts);
            trace_decline(self.plan, reason, facts);
        }
        if matches!(input, fedemean::InputState::SameProjection { .. }) {
            self.partial = coefficients;
        }
    }

    /// The maximum weight (1 for unit weights) that normalized the norms and
    /// certificate of the last certified projection.
    pub(crate) fn weight_scale(&self) -> Option<f64> {
        self.weight_scale
    }

    /// Add the coefficient increments of a declined call (see
    /// [`Cache::project`]) to the fallback route's `result` for the same
    /// columns.
    pub(crate) fn add_partial_coefficients(
        &mut self,
        result: Result<CertifiedBorrowedMap>,
    ) -> Result<CertifiedBorrowedMap> {
        let mut result = result?;
        if let Some(increments) = self.partial.take() {
            result.add_coefficient_increments(&increments)?;
        }
        Ok(result)
    }

    /// Recompute every level's weighted moment of one residual column in one
    /// observation pass and certify it at `amplification` times its weighted
    /// norm (the derived scale of SPEC §16). `None` when this cache did not
    /// project this design with these weights.
    pub(crate) fn certify_derived(
        &self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        values: &DenseMatrix,
        tolerance: f64,
        amplification: f64,
    ) -> Result<Option<FeCertificate>> {
        let Some(projector) = self.projector.as_ref() else {
            return Ok(None);
        };
        if values.ncols() != 1 || !same_design(projector.design(), design) {
            return Ok(None);
        }
        let weights = match weights {
            BorrowedMapWeights::Unit => fedemean::Weights::Unit,
            BorrowedMapWeights::Provided(values) => fedemean::Weights::Values(values),
        };
        projector
            .certify_residual(weights, values.as_slice(), tolerance, amplification)
            .map(|certificate| certificate.map(fe_certificate))
            .map_err(fedemean_error)
    }
}

/// fedemean's workspace for modified-LSMR projections of a categorical
/// intercept design with `level_counts` on `rows` observations: its
/// structure (the level indexes of effects above its scatter limit), weight
/// state and LSMR vectors, plus its additive-Schwarz grant
/// ([`schwarz_grant_bytes`]). It is the memory limit of such projections:
/// fedemean declines a larger workspace, and its automatic preconditioner
/// keeps Jacobi when the Schwarz preconditioner does not fit beside it.
pub(crate) fn lsmr_workspace_bytes(
    rows: usize,
    level_counts: &[usize],
    threads: usize,
) -> Result<u128> {
    /// fedemean's largest scattered effect and its scatter block count.
    const SCATTER_LEVELS: usize = 1024;
    const SCATTER_BLOCKS: u128 = 64;
    if rows == 0 || threads == 0 || level_counts.is_empty() || level_counts.contains(&0) {
        return Err(invalid_argument(
            "PPML LSMR memory estimate requires positive rows, threads and effect level counts",
        ));
    }
    let n = rows as u128;
    let levels = level_counts.iter().try_fold(0_u128, |total, &levels| {
        checked_add_memory(total, levels as u128, "PPML LSMR total levels")
    })?;
    let mut structure = 0_u128;
    let mut scatter = 0_u128;
    for &count in level_counts {
        if count <= SCATTER_LEVELS {
            scatter = checked_add_memory(scatter, count as u128, "PPML LSMR scatter slots")?;
        } else {
            structure = checked_sum_memory(&[
                structure,
                bytes_for_memory::<usize>(count as u128 + 1, "PPML LSMR level starts")?,
                bytes_for_memory::<u32>(n, "PPML LSMR level rows")?,
            ])?;
        }
    }
    // Weight state (masses, inverses, square roots), the LSMR vectors
    // (observation- and coefficient-sized) and the scatter accumulators.
    let values = checked_sum_memory(&[
        checked_mul_memory(levels, 18, "PPML LSMR coefficient vectors")?,
        checked_mul_memory(n, 9, "PPML LSMR observation vectors")?,
        checked_mul_memory(scatter, SCATTER_BLOCKS, "PPML LSMR scatter blocks")?,
    ])?;
    checked_sum_memory(&[
        structure,
        bytes_for_memory::<f64>(values, "PPML LSMR fedemean workspace")?,
        schwarz_grant_bytes(n, level_counts)?,
    ])
}

/// [`lsmr_workspace_bytes`] for an explicit `preconditioner`: without the
/// Schwarz grant when it never builds Schwarz (identity or Jacobi).
pub(crate) fn lsmr_workspace_bytes_for(
    rows: usize,
    level_counts: &[usize],
    threads: usize,
    preconditioner: LsmrPreconditioner,
) -> Result<u128> {
    let bytes = lsmr_workspace_bytes(rows, level_counts, threads)?;
    Ok(match preconditioner {
        LsmrPreconditioner::Off | LsmrPreconditioner::Diagonal => {
            bytes - schwarz_grant_bytes(rows as u128, level_counts)?
        }
        LsmrPreconditioner::Auto | LsmrPreconditioner::Schwarz => bytes,
    })
}

/// The memory fereg grants fedemean's additive Schwarz preconditioner on a
/// categorical intercept design: fedemean's star of factor pairs (the effect
/// with the most levels paired with each other effect), each with its
/// cross-tabulation at its largest, `E = min(rows, L_a L_e)` entries, kept in
/// both orientations with its level maps (24 bytes per entry and 20 per
/// level), the same entries and the smaller side's levels again at
/// approximate-Cholesky size (48 bytes each) for its components' Schur
/// complements and local factors, and its apply columns (16 bytes per
/// level). fedemean admits each part from its exact size within the memory
/// limit as it builds; this is a grant, not a prediction of those sizes.
pub(crate) fn schwarz_grant_bytes(rows: u128, level_counts: &[usize]) -> Result<u128> {
    let Some(largest) = (0..level_counts.len()).max_by_key(|&e| (level_counts[e], usize::MAX - e))
    else {
        return Ok(0);
    };
    let big = level_counts[largest] as u128;
    let mut total = 0_u128;
    for (e, &other) in level_counts.iter().enumerate() {
        if e == largest {
            continue;
        }
        let other = other as u128;
        let entries = checked_mul_memory(big, other, "Schwarz grant cross-tab cells")?.min(rows);
        let pair_levels = checked_add_memory(big, other, "Schwarz grant pair levels")?;
        total = checked_sum_memory(&[
            total,
            checked_mul_memory(entries, 24, "Schwarz grant cross-tabulation")?,
            checked_mul_memory(pair_levels, 20, "Schwarz grant level maps")?,
            checked_mul_memory(
                checked_add_memory(entries, other, "Schwarz grant local entries")?,
                48,
                "Schwarz grant local solves",
            )?,
            checked_mul_memory(pair_levels, 16, "Schwarz grant apply columns")?,
        ])?;
    }
    Ok(total)
}

fn checked_add_memory(left: u128, right: u128, context: &'static str) -> Result<u128> {
    left.checked_add(right).ok_or_else(|| allocation(context))
}

fn checked_mul_memory(left: u128, right: u128, context: &'static str) -> Result<u128> {
    left.checked_mul(right).ok_or_else(|| allocation(context))
}

fn checked_sum_memory(values: &[u128]) -> Result<u128> {
    values.iter().try_fold(0_u128, |total, &value| {
        checked_add_memory(total, value, "fedemean LSMR workspace sum overflowed")
    })
}

fn bytes_for_memory<T>(elements: u128, context: &'static str) -> Result<u128> {
    checked_mul_memory(elements, std::mem::size_of::<T>() as u128, context)
}

/// fedemean's request of one projection: tolerance, iteration budget, the
/// coefficient stopping rule, the residual Gram and the memory limit beyond
/// the caller's execution bytes.
fn fedemean_request(
    map: MapConfig,
    request: Request,
    gram: bool,
    budget: Option<FeSolverMemoryBudget>,
) -> fedemean::Request {
    let mut fe_request = fedemean::Request::new(map.tolerance, map.max_iterations);
    fe_request.coefficient_stopping = request.coefficient_accuracy;
    fe_request.residual_gram = gram;
    fe_request.memory_limit = budget.map(|b| b.usable_bytes.saturating_sub(b.execution_bytes));
    fe_request
}

/// The certified projection's report: the plan's request, the route's
/// labels and tagged route, its work, workspace and certificate.
fn solve_report(
    plan: Plan,
    report: &fedemean::Report,
    facts: &fedemean::Facts,
) -> Result<SolveReport> {
    let (used, used_preconditioner) = labels(&report.route);
    let mut solve_report = SolveReport {
        requested_solver: plan.requested(),
        iterations: facts.iterations,
        solver_workspace_peak: SolverWorkspacePeak::from_bytes(report.workspace_bytes)?,
        // The certificate is fused into the residual pass, so its time is
        // part of the solve.
        solve_seconds: facts.solve_seconds,
        certification_seconds: 0.0,
        requested_preconditioner: match plan {
            Plan::Fixed {
                preconditioner: Some(requested),
                ..
            } if used == FeSolverKind::Lsmr => Some(requested),
            _ => used_preconditioner,
        },
        used_preconditioner,
        ..SolveReport::single_route(
            used,
            SolveStopReason::Certified,
            fe_certificate(report.certificate),
        )
    };
    solve_report.set_route(route(&report.route));
    Ok(solve_report)
}

/// The projector of `design`, (re)built when the cache held none, another
/// design, or other settings (unless it escalated itself, and so already
/// starts on the blocks).
fn bind<'c, 'a>(
    slot: &'c mut Option<fedemean::Projector<'a>>,
    design: &BorrowedInterceptFeDesign<'a>,
    settings: fedemean::Settings,
    escalated: &mut bool,
) -> Result<&'c mut fedemean::Projector<'a>> {
    if !slot.as_ref().is_some_and(|projector| {
        same_design(projector.design(), design) && (*projector.settings() == settings || *escalated)
    }) {
        *slot = Some(fedemean::Projector::new(design_view(design)?, settings));
        *escalated = false;
    }
    Ok(slot.as_mut().expect("bound projector"))
}

/// The projector's settings. The elimination: Jacobi PCG on the reduced
/// system; a batch whose recurrence stalls or reaches the ceiling continues
/// from its iterate with block-Jacobi (exact small blocks, CMG on large ones)
/// before the route declines. Recurrences that finish under Jacobi are
/// unchanged. A `hinted` projector (a design that escalated before) starts
/// on those blocks. A fixed plan uses its choice.
fn settings(plan: Plan, hinted: bool) -> fedemean::Settings {
    if let Plan::Fixed { choice, .. } = plan {
        return fedemean::Settings {
            choice,
            ..fedemean::Settings::default()
        };
    }
    let preconditioner = if hinted {
        fedemean::Preconditioner::BlockJacobi(fedemean::BlockSpec::default())
    } else {
        fedemean::Preconditioner::Auto
    };
    fedemean::Settings {
        choice: fedemean::Choice {
            // The elimination explicitly: other effect counts decline to the
            // established routes (fedemean's Auto would use its full system).
            reduction: fedemean::Reduction::EliminateLargest,
            preconditioner,
            fallback: fedemean::Fallback::Escalate,
            ..fedemean::Choice::default()
        },
        ..fedemean::Settings::default()
    }
}

/// fedemean's view of the borrowed design: codes and level counts of every
/// effect, and the eliminated effect's level members.
pub(crate) fn design_view<'a>(
    design: &BorrowedInterceptFeDesign<'a>,
) -> Result<fedemean::Design<'a>> {
    let effects = design.effects();
    let eliminated =
        (0..effects.len()).max_by_key(|&e| (effects[e].n_levels(), std::cmp::Reverse(e)));
    let mut view = Vec::new();
    view.try_reserve_exact(effects.len())
        .map_err(|_| allocation("unable to reserve block-elimination workspace"))?;
    for (e, effect) in effects.iter().enumerate() {
        let mut entry = fedemean::Effect::new(effect.codes(), effect.n_levels());
        if Some(e) == eliminated {
            entry = entry
                .with_members(members(effect.level_index())?)
                .map_err(fedemean_error)?;
        }
        view.push(entry);
    }
    fedemean::Design::new(design.nobs(), view).map_err(fedemean_error)
}

fn members(index: &LevelIndex) -> Result<fedemean::Members<'_>> {
    fedemean::Members::new(index.membership_offsets(), index.observations()).map_err(fedemean_error)
}

fn same_design(view: &fedemean::Design<'_>, design: &BorrowedInterceptFeDesign<'_>) -> bool {
    view.nobs() == design.nobs()
        && view.effects().len() == design.effects().len()
        && view
            .effects()
            .iter()
            .zip(design.effects())
            .all(|(v, d)| std::ptr::eq(v.codes(), d.codes()) && v.levels() == d.n_levels())
}

/// fedemean's route in fereg's reporting terms: the reduction, the method,
/// the preconditioner with each retained block's local solve (by design
/// effect), and the escalated batches.
pub(crate) fn route(route: &fedemean::Route) -> crate::fe::FeRoute {
    use crate::fe::{FeLocalSolve, FeMethod, FePreconditioner, FeReduction};
    let preconditioner = match route.preconditioner {
        fedemean::PreconditionerUsed::Block(blocks) => {
            let mut locals = [None; crate::fe::FE_ROUTE_MAX_EFFECTS];
            for block in blocks.iter().flatten() {
                if let Some(local) = locals.get_mut(block.effect) {
                    *local = Some(match block.local {
                        fedemean::LocalUsed::Cmg => FeLocalSolve::Cmg,
                        fedemean::LocalUsed::Dense => FeLocalSolve::Dense,
                        _ => FeLocalSolve::Jacobi,
                    });
                }
            }
            FePreconditioner::Block(locals)
        }
        fedemean::PreconditionerUsed::Identity => FePreconditioner::None,
        fedemean::PreconditionerUsed::Schwarz { local, .. } => {
            FePreconditioner::Schwarz(crate::fe::FeSchwarzSolves {
                trivial: local.trivial,
                dense: local.dense,
                skyline: local.skyline,
                approximate: local.approximate,
            })
        }
        _ => FePreconditioner::Jacobi,
    };
    crate::fe::FeRoute {
        reduction: match route.reduction {
            fedemean::ReductionUsed::Full => FeReduction::Full,
            _ => FeReduction::Eliminate,
        },
        method: match route.method {
            fedemean::MethodUsed::Lsmr => FeMethod::Lsmr,
            _ => FeMethod::Cg,
        },
        escalations: u32::from(route.escalations),
        ..crate::fe::FeRoute::new(FeReduction::Eliminate, FeMethod::Cg, preconditioner)
    }
}

/// The report labels of a route: the elimination, CG on the full system
/// (`Cg`, its route tagged) or modified LSMR with its preconditioner.
fn labels(route: &fedemean::Route) -> (FeSolverKind, Option<LsmrPreconditioner>) {
    match (route.reduction, route.method) {
        (fedemean::ReductionUsed::Full, fedemean::MethodUsed::Lsmr) => (
            FeSolverKind::Lsmr,
            Some(match route.preconditioner {
                fedemean::PreconditionerUsed::Identity => LsmrPreconditioner::Off,
                fedemean::PreconditionerUsed::Schwarz { .. } => LsmrPreconditioner::Schwarz,
                _ => LsmrPreconditioner::Diagonal,
            }),
        ),
        (fedemean::ReductionUsed::Full, _) => (FeSolverKind::Cg, None),
        _ => (FeSolverKind::Elimination, None),
    }
}

fn fe_certificate(certificate: fedemean::Certificate) -> FeCertificate {
    FeCertificate {
        absolute_infinity_norm: certificate.absolute_infinity_norm,
        relative_infinity_norm: certificate.relative_infinity_norm,
        relative_two_norm: certificate.relative_two_norm,
        passed: certificate.passed,
    }
}

pub(crate) fn fedemean_error(error: fedemean::Error) -> FeregError {
    match error {
        fedemean::Error::Cancelled => crate::interrupt::interrupted(),
        fedemean::Error::OutOfMemory(message) => allocation(message),
        fedemean::Error::InvalidInput(message) => invalid_argument(message),
        fedemean::Error::InvalidData(message) => invalid_data(message),
        fedemean::Error::Unsupported(message) => crate::error::unsupported(message),
        _ => invalid_data(error.message()),
    }
}

/// The solve's phase record (trace only).
fn solve_record(plan: Plan, facts: &fedemean::Facts) {
    let name = plan.ledger("block_schur", "fedemean");
    let mut ledger = crate::diagnostics::phase_ledger::PhaseLedger::new(name, ["solve"]);
    ledger.add_seconds(0, facts.solve_seconds);
    ledger.facts(&[
        ("iterations", facts.iterations as u64),
        ("column_iterations", facts.column_iterations as u64),
        ("accepted_columns", facts.accepted as u64),
        ("columns", facts.columns as u64),
        ("cells", facts.cells as u64),
        ("active_segments", facts.active_segments as u64),
        ("reduced", facts.reduced as u64),
        ("lanes", facts.lanes as u64),
        ("corrections", facts.corrections as u64),
        ("rhs_us", (facts.rhs_seconds * 1e6) as u64),
        ("pcg_us", (facts.pcg_seconds * 1e6) as u64),
        ("residual_us", (facts.residual_seconds * 1e6) as u64),
        ("escalations", facts.escalations as u64),
        ("block_us", (facts.block_seconds * 1e6) as u64),
    ]);
}

/// One trace line per call whose batches escalated, naming each retained
/// block's local solve (`effect:levels:solve`) or the Schwarz subdomains'
/// local solves (unknown after a decline).
fn trace_escalation(plan: Plan, report: Option<&fedemean::Report>, facts: &fedemean::Facts) {
    if facts.escalations == 0 || !crate::diagnostics::solver_trace_enabled() {
        return;
    }
    let mut blocks = String::new();
    match report.map(|r| r.route.preconditioner) {
        Some(fedemean::PreconditionerUsed::Block(used)) => {
            for block in used.iter().flatten() {
                let local = match block.local {
                    fedemean::LocalUsed::Cmg => "cmg",
                    fedemean::LocalUsed::Dense => "dense",
                    fedemean::LocalUsed::Jacobi => "jacobi",
                    _ => "other",
                };
                let separator = if blocks.is_empty() { "" } else { "," };
                blocks += &format!("{separator}{}:{}:{local}", block.effect, block.levels);
            }
        }
        Some(fedemean::PreconditionerUsed::Schwarz { subdomains, local }) => {
            blocks = format!(
                "schwarz:{subdomains}:trivial={},dense={},skyline={},approximate={}",
                local.trivial, local.dense, local.skyline, local.approximate
            );
        }
        _ => {}
    }
    eprintln!(
        "FEREG_SOLVER_TRACE|phase={}|outcome={}|escalations={}|iterations={}|block_us={}|pcg_us={}|blocks={}",
        plan.ledger("elimination_escalation", "fedemean_escalation"),
        if report.is_some() { "certified" } else { "declined" },
        facts.escalations,
        facts.iterations,
        (facts.block_seconds * 1e6) as u64,
        (facts.pcg_seconds * 1e6) as u64,
        if blocks.is_empty() { "unknown" } else { &blocks },
    );
}

fn trace_decline(plan: Plan, reason: &str, facts: &fedemean::Facts) {
    if crate::diagnostics::solver_trace_enabled() || crate::diagnostics::phase_timing_enabled() {
        eprintln!(
            "FEREG_PHASE|{}|reason={reason}|iterations={}|pcg_us={}",
            plan.ledger("block_schur_decline", "fedemean_decline"),
            facts.iterations,
            (facts.pcg_seconds * 1e6) as u64
        );
    }
}

#[cfg(test)]
mod tests;
