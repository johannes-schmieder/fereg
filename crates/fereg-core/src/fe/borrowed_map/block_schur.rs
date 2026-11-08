//! The exact global elimination of the largest categorical effect, provided
//! by the fedemean library (largest-effect Schur complement, lockstep Jacobi
//! PCG escalating to block-Jacobi with CMG local solves, fused residual and
//! moment certificate).
//!
//! This adapter keeps fereg's side: the developer switch and test guards,
//! the borrowed design view, result construction (`CertifiedBorrowedMap`,
//! `SolveReport`; OLS reports the route as `elimination`), error mapping and
//! the phase and decline records. A decline returns the matrix to the
//! caller's route.

use super::*;
use crate::fe::LevelIndex;

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
    projector: Option<fedemean::Projector<'a>>,
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
}

impl<'a> Cache<'a> {
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

    /// Project with the retained geometry, building it on first use. A
    /// decline returns the matrix: untouched, unless a later batch or a
    /// correction declined after earlier residuals were formed, whose
    /// projection is then still the request. A refusal of the design is
    /// remembered so later calls go straight to the caller's route.
    pub(crate) fn project(
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
        if values.nrows() != design.nobs() || !(map.tolerance.is_finite() && map.tolerance > 0.0) {
            return Ok(Outcome::Declined(values));
        }
        let (detach, gram) = (self.detach_norms, self.gram_requested);
        let projector = bind(&mut self.projector, design)?;
        let control = fedemean::Control::with_cancel(&crate::interrupt::cancellation_requested);
        let setup = crate::diagnostics::phase_ledger::PhaseLedger::new(
            "block_schur_setup",
            ["structure", "weights"],
        );
        let mut coefficients = None;
        let mut structure_seconds = 0.0;
        if request.retain_coefficients {
            let started = std::time::Instant::now();
            if !projector.prepare(&control).map_err(fedemean_error)? {
                return Ok(Outcome::Declined(values));
            }
            structure_seconds = started.elapsed().as_secs_f64();
            coefficients = Some(FeCoefficientBatch::intercept_zeros(design, values.ncols())?);
        }
        let mut fe_request = fedemean::Request::new(map.tolerance, map.max_iterations);
        fe_request.coefficient_stopping = request.coefficient_accuracy;
        fe_request.residual_gram = gram;
        fe_request.memory_limit = budget.map(|b| b.usable_bytes.saturating_sub(b.execution_bytes));
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
            .map_err(fedemean_error)?;
        self.current = true;
        let facts = match &outcome {
            fedemean::Outcome::Projected(report) => report.facts,
            fedemean::Outcome::Declined { facts, .. } => *facts,
            _ => fedemean::Facts::default(),
        };
        setup.add_seconds(0, structure_seconds + facts.structure_seconds);
        setup.add_seconds(1, facts.weights_seconds);
        let report = match outcome {
            fedemean::Outcome::Projected(report) => report,
            fedemean::Outcome::Declined { reason, input, .. } => {
                if let fedemean::Decline::Numerical(reason) = reason {
                    solve_record(&facts);
                    trace_escalation(None, &facts);
                    trace_decline(reason, &facts);
                }
                if matches!(input, fedemean::InputState::SameProjection { .. }) {
                    self.partial = coefficients;
                }
                return Ok(Outcome::Declined(values));
            }
            _ => return Ok(Outcome::Declined(values)),
        };
        solve_record(&facts);
        trace_escalation(Some(&report), &facts);
        self.weight_scale = Some(report.weight_scale);
        let solve_report = SolveReport {
            requested_solver: FeSolverKind::Auto,
            iterations: facts.iterations,
            solver_workspace_peak: SolverWorkspacePeak::from_bytes(report.workspace_bytes)?,
            // The certificate is fused into the residual pass, so its time is
            // part of the solve.
            solve_seconds: facts.solve_seconds,
            certification_seconds: 0.0,
            ..SolveReport::single_route(
                FeSolverKind::Elimination,
                SolveStopReason::Certified,
                fe_certificate(report.certificate),
            )
        };
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

/// The projector of `design`, (re)built when the cache held none or another
/// design.
fn bind<'c, 'a>(
    slot: &'c mut Option<fedemean::Projector<'a>>,
    design: &BorrowedInterceptFeDesign<'a>,
) -> Result<&'c mut fedemean::Projector<'a>> {
    if !slot
        .as_ref()
        .is_some_and(|projector| same_design(projector.design(), design))
    {
        *slot = Some(fedemean::Projector::new(design_view(design)?, settings()));
    }
    Ok(slot.as_mut().expect("bound projector"))
}

/// The projector's route: Jacobi PCG on the reduced system; a batch whose
/// recurrence stalls or reaches the ceiling continues from its iterate with
/// block-Jacobi (exact small blocks, CMG on large ones) before the route
/// declines. Recurrences that finish under Jacobi are unchanged.
fn settings() -> fedemean::Settings {
    fedemean::Settings {
        choice: fedemean::Choice {
            fallback: fedemean::Fallback::Escalate,
            ..fedemean::Choice::default()
        },
        ..fedemean::Settings::default()
    }
}

/// fedemean's view of the borrowed design: codes and level counts of every
/// effect, and the eliminated effect's level members.
fn design_view<'a>(design: &BorrowedInterceptFeDesign<'a>) -> Result<fedemean::Design<'a>> {
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

fn fe_certificate(certificate: fedemean::Certificate) -> FeCertificate {
    FeCertificate {
        absolute_infinity_norm: certificate.absolute_infinity_norm,
        relative_infinity_norm: certificate.relative_infinity_norm,
        relative_two_norm: certificate.relative_two_norm,
        passed: certificate.passed,
    }
}

fn fedemean_error(error: fedemean::Error) -> FeregError {
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
fn solve_record(facts: &fedemean::Facts) {
    let mut ledger = crate::diagnostics::phase_ledger::PhaseLedger::new("block_schur", ["solve"]);
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

/// One trace line per call whose batches escalated to block-Jacobi, naming
/// each retained block's local solve (`effect:levels:solve`; unknown after a
/// decline).
fn trace_escalation(report: Option<&fedemean::Report>, facts: &fedemean::Facts) {
    if facts.escalations == 0 || !crate::diagnostics::solver_trace_enabled() {
        return;
    }
    let mut blocks = String::new();
    if let Some(fedemean::PreconditionerUsed::Block(used)) = report.map(|r| r.route.preconditioner)
    {
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
    eprintln!(
        "FEREG_SOLVER_TRACE|phase=elimination_escalation|outcome={}|escalations={}|iterations={}|block_us={}|pcg_us={}|blocks={}",
        if report.is_some() { "certified" } else { "declined" },
        facts.escalations,
        facts.iterations,
        (facts.block_seconds * 1e6) as u64,
        (facts.pcg_seconds * 1e6) as u64,
        if blocks.is_empty() { "unknown" } else { &blocks },
    );
}

fn trace_decline(reason: &str, facts: &fedemean::Facts) {
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
