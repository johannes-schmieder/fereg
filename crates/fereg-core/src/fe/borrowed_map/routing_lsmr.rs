//! Optional categorical-pair replacement at a fixed LSMR checkpoint.
//!
//! The immutable OLS binding owns this state. A refused build does not alter a
//! live recurrence; a committed fixed inverse starts a fresh recurrence with
//! the original response scale and the remaining per-action iteration budget.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use super::*;
use crate::fe::cmg_schur::{CmgSchurPreconditioner, Sampling};
use crate::fe::routing::{RoutingPreset, SetupLedger};

/// Keep the established unseeded batch path exact. Seeded experiments use the
/// session correction interface and report failed native work before fallback.
pub(super) fn solve_prepared_input(
    solver: &within::Solver<'_>,
    rhs: &[&[f64]],
    options: &LsmrOptions,
    initial: Option<&FeCoefficientBatch>,
) -> std::result::Result<within::BatchSolveResult, (super::super::lsmr_batch::BatchError, usize)> {
    use super::super::lsmr_batch::{solve_prepared_batch, BatchError};
    let Some(initial) = initial else {
        return solve_prepared_batch(solver, rhs, options).map_err(|error| (error, 0));
    };
    let window = crate::memory::lsmr_rhs_window(rhs.len(), rayon::current_num_threads());
    let mut batch = None;
    for (window_index, columns) in rhs.chunks(window).enumerate() {
        let mut sessions =
            reserved(columns.len()).map_err(|error| (BatchError::Allocation(error), 0))?;
        let started: Vec<_> = columns
            .par_iter()
            .enumerate()
            .map(|(offset, column)| {
                let seed = initial
                    .rhs(window_index * window + offset)
                    .map_err(|error| (BatchError::Allocation(error), 0))?;
                solver
                    .start_session_categorical_seeded(column, seed, options)
                    .map_err(|error| (BatchError::Upstream(error.into()), 0))
            })
            .collect();
        for session in started {
            sessions.push(session?);
        }
        let results: Vec<_> = sessions
            .par_iter_mut()
            .map(|session| session.run_action(None, options.maxiter, Some(&Cancellation)))
            .collect();
        let spent = consumed(&sessions, batch.as_ref(), options.maxiter);
        crate::interrupt::checkpoint().map_err(|error| (BatchError::Allocation(error), spent))?;
        // A fatal action cannot be hidden behind an earlier positivity refusal.
        let mut failures: Vec<_> = results
            .into_iter()
            .filter_map(|result| result.err())
            .collect();
        if !failures.is_empty() {
            let selected = failures
                .iter()
                .enumerate()
                .min_by_key(|(rhs, error)| {
                    let kind = match error {
                        within::SolveError::InvalidInput { context, message }
                            if *context == "mlsmr"
                                && message.contains("preconditioner not positive definite") =>
                        {
                            2
                        }
                        within::SolveError::Allocation { .. } => 1,
                        _ => 0,
                    };
                    (kind, *rhs)
                })
                .map_or(0, |(rhs, _)| rhs);
            return Err((
                BatchError::Upstream(failures.swap_remove(selected).into()),
                spent,
            ));
        }
        let finished: Vec<_> = sessions
            .into_par_iter()
            .map(within::SolveSession::finish)
            .collect();
        for solved in finished {
            append(
                &mut batch,
                solved,
                rhs.len(),
                solver.n_obs(),
                solver.n_dofs(),
            )
            .map_err(|error| (BatchError::Allocation(error), spent))?;
        }
    }
    batch.ok_or_else(|| {
        (
            BatchError::Allocation(invalid_argument("empty seeded LSMR batch")),
            0,
        )
    })
}

pub(crate) struct GeneralRoutingState<'a> {
    preset: RoutingPreset,
    prepared: BorrowedLsmrPrepared<'a>,
    setup: SetupLedger,
    replacement: Option<within::PairReplacement>,
    replacement_bytes: u128,
    replacement_apply_bytes: u128,
    accuracy: Option<(u64, bool)>,
    /// Additive configuration a retained Jacobi trial escalates to.
    trial: Option<PreconditionerConfig>,
}

impl<'a> GeneralRoutingState<'a> {
    pub(crate) fn new(preset: RoutingPreset) -> Self {
        Self {
            preset,
            prepared: BorrowedLsmrPrepared::default(),
            setup: SetupLedger::new(),
            replacement: None,
            replacement_bytes: 0,
            replacement_apply_bytes: 0,
            accuracy: None,
            trial: None,
        }
    }

    pub(crate) fn has_prepared(&self) -> bool {
        self.prepared.solver.is_some()
    }

    pub(super) fn categorical_prefix(&self) -> Option<crate::fe::routing::CategoricalPrefix> {
        self.preset.categorical_prefix()
    }

    pub(super) fn strict_categorical_prefix(&self) -> bool {
        self.preset.strict_categorical_prefix()
    }

    pub(super) fn geometry_pilot(&self) -> bool {
        self.preset.geometry_pilot()
    }

    /// Whether the retained solver is still an unescalated Jacobi trial, so the
    /// last solve finished every RHS with the diagonal preconditioner.
    pub(super) fn jacobi_trial_active(&self) -> bool {
        self.trial.is_some()
    }

    /// Bind a new accuracy key; it starts a new single optional attempt.
    pub(crate) fn begin(&mut self, tolerance: f64, final_residual: bool) {
        let key = (tolerance.to_bits(), final_residual);
        if self.accuracy == Some(key) {
            return;
        }
        self.accuracy = Some(key);
        self.setup.begin(tolerance, final_residual);
    }

    #[cfg(test)]
    pub(crate) fn has_replacement(&self) -> bool {
        self.replacement.is_some()
    }

    /// Close the optional attempt for the current accuracy key.
    pub(crate) fn close_attempt(&mut self) {
        self.setup.attempted = true;
    }

    /// A failed foreground certificate or action closes the optional attempt.
    /// Cancellation requested meanwhile takes precedence over the failure.
    pub(in crate::fe::borrowed_map) fn certificate_boundary<T>(
        &mut self,
        foreground: Result<T>,
    ) -> Result<T> {
        if foreground.is_err() {
            self.close_attempt();
            crate::interrupt::checkpoint()?;
        }
        foreground
    }

    pub(crate) fn release_backend(&mut self) {
        self.replacement = None;
        self.replacement_bytes = 0;
        self.replacement_apply_bytes = 0;
        self.prepared = BorrowedLsmrPrepared::default();
        self.trial = None;
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        self.prepared.retained_bytes * (1 + u128::from(self.replacement.is_some()))
            + self.replacement_bytes
            + self.replacement_apply_bytes
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn solve(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        rhs: &[&[f64]],
        options: &LsmrOptions,
        requested: LsmrPreconditioner,
        preconditioner: PreconditionerConfig,
        deterministic: bool,
        memory: &mut BorrowedLsmrCallMemory,
        initial_coefficients: Option<&FeCoefficientBatch>,
    ) -> Result<(within::BatchSolveResult, bool)> {
        // Explicit preconditioner intent remains authoritative.
        if requested != LsmrPreconditioner::Auto
            || !matches!(preconditioner, PreconditionerConfig::Additive { .. })
            || self.prepared.preconditioner_fallback
        {
            self.close_attempt();
            return self.prepared.solve(
                design,
                weights,
                rhs,
                options,
                requested,
                preconditioner,
                deterministic,
                initial_coefficients,
            );
        }
        let started = Instant::now();
        let SessionAdmission {
            parent,
            window,
            setup_seconds,
            apply_bytes,
        } = self.prepare_call(
            design,
            weights,
            rhs.len(),
            options,
            preconditioner,
            deterministic,
            memory,
            initial_coefficients.is_some(),
        )?;
        if apply_bytes.is_none() {
            self.replacement = None;
            self.replacement_bytes = 0;
            self.replacement_apply_bytes = 0;
            self.setup.attempted = true;
            let (mut solved, fallback) = self.prepared.solve(
                design,
                weights,
                rhs,
                options,
                requested,
                PreconditionerConfig::default(),
                deterministic,
                initial_coefficients,
            )?;
            solved.time_setup += setup_seconds;
            solved.time_total = started.elapsed().as_secs_f64();
            return Ok((solved, fallback));
        }
        self.replacement_apply_bytes = apply_bytes.expect("admitted replacement scratch");
        memory.include(parent + self.replacement_bytes + self.replacement_apply_bytes)?;
        self.solve_serial_windows(
            design,
            weights,
            rhs,
            options,
            deterministic,
            memory,
            parent,
            window,
            setup_seconds,
            started,
            initial_coefficients,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn solve_serial_windows(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        rhs: &[&[f64]],
        options: &LsmrOptions,
        deterministic: bool,
        memory: &mut BorrowedLsmrCallMemory,
        parent: u128,
        window: usize,
        mut setup_seconds: f64,
        started: Instant,
        initial_coefficients: Option<&FeCoefficientBatch>,
    ) -> Result<(within::BatchSolveResult, bool)> {
        // Every failed action retries the whole call without a preconditioner.
        let retry = |state: &mut Self, spent, failure, setup_seconds| {
            state.retry_off(
                design,
                weights,
                rhs,
                options,
                deterministic,
                spent,
                failure,
                setup_seconds,
                started,
                initial_coefficients,
            )
        };
        let mut batch: Option<within::BatchSolveResult> = None;
        for (window_index, columns) in rhs.chunks(window.max(1)).enumerate() {
            let solver = self
                .prepared
                .solver
                .as_ref()
                .expect("retained routing solver");
            let trial = self.trial.is_some() && self.replacement.is_none();
            let mut sessions = start_routing_sessions(
                solver,
                columns,
                options,
                initial_coefficients,
                window_index * window,
            )?;
            let pilot = self.replacement.is_none()
                && !self.setup.attempted
                && options.maxiter > self.preset.general_pair_checkpoint();
            let cap = if pilot {
                self.preset.general_pair_checkpoint()
            } else {
                options.maxiter
            };
            let trial_target = trial.then_some(options.tol);
            let progress =
                advance_window(&mut sessions, self.replacement.as_ref(), cap, trial_target);
            if let Some(failure) = action_failure(&progress) {
                let spent = consumed(&sessions, batch.as_ref(), options.maxiter);
                drop(sessions);
                return retry(self, spent, failure, setup_seconds);
            }
            let progress: Vec<_> = progress
                .into_iter()
                .map(|result| result.expect("actions checked"))
                .collect();
            if pilot {
                if progress.iter().any(|progress| !progress.converged) {
                    self.setup.attempted = true;
                    let (candidate, build_seconds) = try_pair_replacement(
                        solver,
                        self.preset,
                        FactoryState {
                            memory,
                            parent,
                            retained: 0,
                            error: None,
                            refused: false,
                        },
                        window,
                    )?;
                    setup_seconds += build_seconds;
                    if let Some(candidate) = candidate {
                        self.replacement = Some(candidate.replacement);
                        self.replacement_apply_bytes = candidate.apply_bytes;
                        self.replacement_bytes = candidate.retained_bytes;
                    }
                }
                let continuation: Vec<_> = sessions
                    .par_iter_mut()
                    .zip(&progress)
                    .map(|(session, progress)| {
                        if !progress.converged {
                            let remaining = session.remaining_iterations();
                            advance(session, self.replacement.as_ref(), remaining)?;
                        }
                        Ok(())
                    })
                    .collect();
                if let Some(failure) = action_failure(&continuation) {
                    let spent = consumed(&sessions, batch.as_ref(), options.maxiter);
                    drop(sessions);
                    return retry(self, spent, failure, setup_seconds);
                }
            }
            // Finish and release every recurrence before allocating the combined
            // result banks. This preserves the existing bounded-window peak.
            let mut finished: Vec<_> = sessions
                .into_par_iter()
                .map(within::SolveSession::finish)
                .collect();
            let escalated = escalated(&progress, trial);
            if escalated.contains(&true) {
                let (seconds, outcome) = self.escalate(
                    design,
                    weights,
                    columns,
                    options,
                    deterministic,
                    &escalated,
                    &mut finished,
                );
                setup_seconds += seconds;
                if let Err(failure) = outcome {
                    let spent = finished
                        .iter()
                        .map(|solved| solved.iterations)
                        .chain(batch.iter().flat_map(|b| b.iterations.iter().copied()))
                        .max()
                        .unwrap_or(0);
                    drop(finished);
                    return retry(self, spent, failure, setup_seconds);
                }
            }
            for solved in finished {
                append(
                    &mut batch,
                    solved,
                    rhs.len(),
                    design.nobs(),
                    design.total_term_slots(),
                )?;
            }
        }
        self.prepared.batches += 1;
        let mut batch = batch.ok_or_else(|| invalid_argument("empty general routing batch"))?;
        batch.time_setup += setup_seconds;
        batch.time_total = started.elapsed().as_secs_f64();
        Ok((batch, false))
    }

    /// Replace a stalled Jacobi trial with the retained additive preconditioner
    /// and continue each escalated RHS from its iterate with the rest of its
    /// budget. Later calls on this binding reuse the additive solver.
    #[allow(clippy::too_many_arguments)]
    fn escalate(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        columns: &[&[f64]],
        options: &LsmrOptions,
        deterministic: bool,
        escalated: &[bool],
        finished: &mut [within::SolveResult],
    ) -> (f64, std::result::Result<(), ActionFailure>) {
        let fatal = |error| ActionFailure {
            error,
            preconditioner_breakdown: false,
        };
        let started = Instant::now();
        let config = self.trial.take().expect("escalating Jacobi trial");
        // Release the trial before construction; the additive bound is admitted.
        self.prepared.solver = None;
        let built = build_retained_borrowed_lsmr(design, weights, config, deterministic);
        let seconds = started.elapsed().as_secs_f64();
        self.prepared.solver = match built {
            Ok(solver) => Some(solver),
            Err(error) => return (seconds, Err(fatal(error))),
        };
        self.prepared.builds += 1;
        let solver = self
            .prepared
            .solver
            .as_ref()
            .expect("additive routing solver");
        let mut continued = match reserved(columns.len()) {
            Ok(values) => values,
            Err(error) => return (seconds, Err(fatal(error))),
        };
        let started: Vec<_> = (0..columns.len())
            .into_par_iter()
            .filter_map(|offset| {
                let remaining = options.maxiter.saturating_sub(finished[offset].iterations);
                (escalated[offset] && remaining > 0).then(|| {
                    let options = LsmrOptions {
                        maxiter: remaining,
                        ..options.clone()
                    };
                    solver
                        .start_session_categorical_seeded(
                            columns[offset],
                            &finished[offset].x,
                            &options,
                        )
                        .map(|session| (offset, session))
                })
            })
            .collect();
        for session in started {
            match session {
                Ok(session) => continued.push(session),
                Err(error) => return (seconds, Err(action_error(error))),
            }
        }
        let progress: Vec<_> = continued
            .par_iter_mut()
            .map(|(_, session)| {
                let cap = session.remaining_iterations();
                advance(session, None, cap)
            })
            .collect();
        if let Some(failure) = action_failure(&progress) {
            return (seconds, Err(failure));
        }
        let continued: Vec<_> = continued
            .into_par_iter()
            .map(|(offset, session)| (offset, session.finish()))
            .collect();
        for (offset, mut solved) in continued {
            let prior = &finished[offset];
            let (iterations, time_solve) = (prior.iterations, prior.time_solve);
            solved.iterations += iterations;
            solved.time_solve += time_solve;
            finished[offset] = solved;
        }
        (seconds, Ok(()))
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_call(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        rhs_count: usize,
        options: &LsmrOptions,
        preconditioner: PreconditionerConfig,
        deterministic: bool,
        memory: &BorrowedLsmrCallMemory,
        seeded: bool,
    ) -> Result<SessionAdmission> {
        let estimate = super::borrowed_solver_memory_estimate(
            design,
            rhs_count,
            rayon::current_num_threads(),
            options.local_size,
        )?;
        if self
            .prepared
            .solver
            .as_ref()
            .is_some_and(|s| s.reproducible() != deterministic)
        {
            self.close_attempt();
            self.release_backend();
        }
        let setup_started = Instant::now();
        if self.prepared.solver.is_none() {
            // The public many-FE policy first retains a Jacobi trial and keeps
            // the additive configuration for a measured escalation. Admission
            // still charges the additive bound, which covers both solvers.
            let additive = matches!(preconditioner, PreconditionerConfig::Additive { .. });
            self.trial = (additive && self.preset.geometry_pilot()).then(|| preconditioner.clone());
            self.prepared.solver = Some(build_retained_borrowed_lsmr(
                design,
                weights,
                if self.trial.is_some() {
                    PreconditionerConfig::Diagonal
                } else {
                    preconditioner
                },
                deterministic,
            )?);
            self.prepared.retained_bytes =
                estimate.lsmr_retained_bytes(LsmrPreconditioner::Schwarz);
            self.prepared.builds += 1;
        }
        let setup_seconds = setup_started.elapsed().as_secs_f64();
        let window = crate::memory::lsmr_rhs_window(rhs_count, rayon::current_num_threads());
        // The existing estimate covers 4m+6n numeric lanes per active RHS;
        // sessions admit 6m+12n for warm state, terminal copies and recovery.
        // Reserve a second full retained bound for replacement metadata and
        // per-RHS wrapper scratch; hierarchy storage is admitted incrementally.
        let rows = design.nobs() as u128;
        let dofs = design.total_term_slots() as u128;
        let warm_extra = rows
            .checked_mul(2)
            .and_then(|cells| cells.checked_add(dofs.checked_mul(6)?))
            .and_then(|cells| cells.checked_mul(window as u128))
            .and_then(|cells| cells.checked_mul(8))
            .and_then(|bytes| bytes.checked_add(window as u128 * 1024))
            .ok_or_else(|| allocation("general routing session overlap overflows"))?;
        let extra = self
            .prepared
            .retained_bytes
            .checked_add(if seeded { 0 } else { warm_extra })
            .ok_or_else(|| allocation("general routing session overlap overflows"))?;
        let parent = estimate
            .lsmr_bytes(LsmrPreconditioner::Schwarz)
            .checked_add(extra)
            .ok_or_else(|| allocation("general routing parent overlap overflows"))?;
        let apply_bytes = self.replacement.as_ref().map_or(Some(0), |replacement| {
            replacement
                .apply_storage_bound()
                .map(u128::from)?
                .checked_mul(window as u128)
        });
        let apply_bytes = apply_bytes.map(|bytes| bytes.max(self.replacement_apply_bytes));
        let foreground = apply_bytes
            .and_then(|apply| {
                parent
                    .checked_add(self.replacement_bytes)?
                    .checked_add(apply)
            })
            .and_then(|bytes| bytes.checked_add(memory.parent_bytes));
        let admitted =
            foreground.is_some_and(|bytes| memory.budget.is_none_or(|budget| budget.fits(bytes)));
        Ok(SessionAdmission {
            parent,
            window,
            setup_seconds,
            apply_bytes: apply_bytes.filter(|_| admitted),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn retry_off(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        rhs: &[&[f64]],
        options: &LsmrOptions,
        deterministic: bool,
        spent: usize,
        failure: ActionFailure,
        setup_seconds: f64,
        started: Instant,
        initial_coefficients: Option<&FeCoefficientBatch>,
    ) -> Result<(within::BatchSolveResult, bool)> {
        if !failure.preconditioner_breakdown {
            return Err(failure.error);
        }
        let Some(remaining) = options.maxiter.checked_sub(spent).filter(|n| *n > 0) else {
            return Err(failure.error);
        };
        self.release_backend();
        self.prepared.preconditioner_fallback = true;
        self.setup.attempted = true;
        let options = LsmrOptions {
            maxiter: remaining,
            ..options.clone()
        };
        let (mut solved, _) = self.prepared.solve(
            design,
            weights,
            rhs,
            &options,
            LsmrPreconditioner::Auto,
            PreconditionerConfig::Off,
            deterministic,
            initial_coefficients,
        )?;
        for iterations in &mut solved.iterations {
            *iterations += spent;
        }
        solved.time_setup += setup_seconds;
        solved.time_total = started.elapsed().as_secs_f64();
        Ok((solved, true))
    }
}

struct SessionAdmission {
    parent: u128,
    window: usize,
    setup_seconds: f64,
    apply_bytes: Option<u128>,
}

struct AdmittedReplacement {
    replacement: within::PairReplacement,
    apply_bytes: u128,
    retained_bytes: u128,
}

fn try_pair_replacement(
    solver: &within::Solver<'_>,
    preset: RoutingPreset,
    state: FactoryState<'_>,
    window: usize,
) -> Result<(Option<AdmittedReplacement>, f64)> {
    let factory = PairFactory {
        preset,
        state: RefCell::new(state),
    };
    let build_started = Instant::now();
    let candidate = solver.build_pair_replacement(&factory);
    let setup_seconds = build_started.elapsed().as_secs_f64();
    let state = factory.state.into_inner();
    if let Some(error) = state.error {
        return Err(error);
    }
    match candidate {
        Ok(Some(replacement)) => {
            let apply_bytes = replacement
                .apply_storage_bound()
                .and_then(|bytes| (bytes as u128).checked_mul(window as u128));
            let live = apply_bytes
                .and_then(|apply| state.parent.checked_add(state.retained)?.checked_add(apply));
            let memory_admitted = live
                .and_then(|bytes| bytes.checked_add(state.memory.parent_bytes))
                .is_some_and(|bytes| state.memory.budget.is_none_or(|budget| budget.fits(bytes)));
            if memory_admitted {
                state
                    .memory
                    .include(live.expect("admitted replacement scratch"))?;
                return Ok((
                    Some(AdmittedReplacement {
                        replacement,
                        apply_bytes: apply_bytes.expect("admitted replacement scratch"),
                        retained_bytes: state.retained,
                    }),
                    setup_seconds,
                ));
            }
        }
        Ok(None) => {}
        Err(_) if state.refused => {}
        Err(error) => return Err(build_error(error)),
    }
    Ok((None, setup_seconds))
}

struct ReducedInverse {
    solver: Mutex<CmgSchurPreconditioner>,
    dimension: usize,
    scale: f64,
}

impl within::FixedReducedInverse for ReducedInverse {
    fn dimension(&self) -> usize {
        self.dimension
    }
    fn scratch_len(&self) -> usize {
        self.dimension
    }
    fn apply_in_place(
        &self,
        rhs: &mut [f64],
        scratch: &mut [f64],
    ) -> std::result::Result<(), within::LocalSolveError> {
        let error = |message: String| within::LocalSolveError::BackendFailed {
            context: "fereg fixed reduced CMG",
            message,
        };
        self.solver
            .lock()
            .map_err(|_| error("poisoned fixed inverse".into()))?
            .apply_reduced_in_place(rhs, scratch, self.scale)
            .map_err(|e| error(e.to_string()))
    }
}

struct PairFactory<'a> {
    preset: RoutingPreset,
    state: RefCell<FactoryState<'a>>,
}

struct FactoryState<'a> {
    memory: &'a mut BorrowedLsmrCallMemory,
    parent: u128,
    retained: u128,
    error: Option<FeregError>,
    refused: bool,
}

fn rejected() -> within::BuildError {
    within::BuildError::LocalSolverBuild("fereg optional pair construction stopped".into())
}

impl within::FixedReducedInverseFactory for PairFactory<'_> {
    // Fixed nomination/ownership metadata is admitted in the aggregate parent
    // envelope before this serial factory runs. Each bounded construction
    // checkpoint polls cancellation only; there is no work allowance.
    fn checkpoint(&self, _work_units: u64) -> std::result::Result<(), within::BuildError> {
        let mut state = self.state.borrow_mut();
        if let Err(error) = crate::interrupt::checkpoint() {
            state.error = Some(error);
            return Err(rejected());
        }
        Ok(())
    }

    fn build(
        &self,
        component: within::PairComponent<'_>,
    ) -> std::result::Result<Arc<dyn within::FixedReducedInverse>, within::BuildError> {
        let mut state = self.state.borrow_mut();
        let parent = state
            .parent
            .checked_add(state.retained)
            .ok_or_else(rejected)?;
        let budget = if let Some(budget) = state.memory.budget {
            Some(FeSolverMemoryBudget {
                execution_bytes: budget
                    .execution_bytes
                    .checked_add(parent)
                    .and_then(|bytes| bytes.checked_add(state.memory.parent_bytes))
                    .ok_or_else(rejected)?,
                usable_bytes: budget.usable_bytes,
            })
        } else {
            None
        };
        let attempt = CmgSchurPreconditioner::try_build_reduced_bounded(
            component.n_eliminated,
            component.n_retained,
            component.row_ptr,
            component.columns,
            component.weights,
            5,
            Sampling {
                samples: self.preset.samples(),
                seed: 0,
            },
            budget,
        );
        let (attempt, scale) = attempt.map_err(|error| {
            state.error = Some(error);
            rejected()
        })?;
        let peak = attempt.peak.bytes().map_or(0, u128::from);
        state.memory.include(parent + peak).map_err(|error| {
            state.error = Some(error);
            rejected()
        })?;
        let Some(preconditioner) = attempt.preconditioner else {
            state.refused = true;
            return Err(rejected());
        };
        let bytes = preconditioner.retained_bytes().map_err(|error| {
            state.error = Some(error);
            rejected()
        })?;
        state.retained = state.retained.checked_add(bytes).ok_or_else(rejected)?;
        Ok(Arc::new(ReducedInverse {
            solver: Mutex::new(preconditioner),
            dimension: component.n_retained,
            scale,
        }))
    }
}

fn start_routing_sessions<'s, 'a>(
    solver: &'s within::Solver<'a>,
    columns: &[&[f64]],
    options: &LsmrOptions,
    initial: Option<&FeCoefficientBatch>,
    first_rhs: usize,
) -> Result<Vec<within::SolveSession<'s, 'a>>> {
    let mut sessions = Vec::new();
    sessions
        .try_reserve_exact(columns.len())
        .map_err(|_| allocation("unable to reserve routing sessions"))?;
    // Each start copies and permutes one full response; independent starts run
    // on the team and the first failure in RHS order is reported.
    let started: Vec<_> = columns
        .par_iter()
        .enumerate()
        .map(|(offset, column)| {
            if let Some(seed) = initial {
                solver
                    .start_session_categorical_seeded(
                        column,
                        seed.rhs(first_rhs + offset)?,
                        options,
                    )
                    .map_err(solve_error)
            } else {
                solver.start_session(column, options).map_err(solve_error)
            }
        })
        .collect();
    for session in started {
        sessions.push(session?);
    }
    Ok(sessions)
}

fn solve_error(error: within::SolveError) -> FeregError {
    if let Err(interrupted) = crate::interrupt::checkpoint() {
        return interrupted;
    }
    super::super::lsmr_batch::BatchError::Upstream(error.into())
        .into_fereg("general routing LSMR failed")
}

#[derive(Debug)]
struct ActionFailure {
    error: FeregError,
    preconditioner_breakdown: bool,
}

fn action_error(error: within::SolveError) -> ActionFailure {
    let preconditioner_breakdown = matches!(&error, within::SolveError::InvalidInput { context, message }
        if *context == "mlsmr" && message.contains("preconditioner not positive definite"));
    let error = solve_error(error);
    ActionFailure {
        preconditioner_breakdown: preconditioner_breakdown && error.code != ErrorCode::Interrupted,
        error,
    }
}

// Every admitted parallel action is joined before selecting the first fatal
// failure in original RHS order. A positivity fallback cannot mask another error.
fn action_failure<T>(results: &[std::result::Result<T, ActionFailure>]) -> Option<ActionFailure> {
    let mut failures = results.iter().filter_map(|result| result.as_ref().err());
    let mut failure = failures.next()?;
    for candidate in failures {
        if failure.preconditioner_breakdown && !candidate.preconditioner_breakdown {
            failure = candidate;
        } else if !candidate.preconditioner_breakdown {
            let selected =
                arbitrate::<()>(Err(failure.error.clone()), Err(candidate.error.clone()))
                    .unwrap_err();
            if selected.code == candidate.error.code && selected.message == candidate.error.message
            {
                failure = candidate;
            }
        }
    }
    Some(ActionFailure {
        error: FeregError::new(failure.error.code, failure.error.message.clone()),
        preconditioner_breakdown: failure.preconditioner_breakdown,
    })
}

/// Fixed terminal-kind order between two outcomes, independent of wall time.
/// Cancellation requested before the decision takes precedence.
fn arbitrate<T>(first: Result<T>, second: Result<()>) -> Result<T> {
    crate::interrupt::checkpoint()?;
    match (first, second) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(front), Err(back)) => Err(if fatal_rank(&front) <= fatal_rank(&back) {
            front
        } else {
            back
        }),
    }
}

fn fatal_rank(error: &FeregError) -> u16 {
    match error.code {
        ErrorCode::Interrupted => 0,
        ErrorCode::Panic => 1,
        ErrorCode::Internal => 2,
        ErrorCode::InvalidData => 3,
        ErrorCode::Allocation => 4,
        ErrorCode::NonConvergence => 6,
        _ => 5,
    }
}

fn consumed(
    sessions: &[within::SolveSession<'_, '_>],
    prior: Option<&within::BatchSolveResult>,
    limit: usize,
) -> usize {
    sessions
        .iter()
        .map(|session| limit.saturating_sub(session.remaining_iterations()))
        .chain(
            prior
                .into_iter()
                .flat_map(|batch| batch.iterations.iter().copied()),
        )
        .max()
        .unwrap_or(0)
}

struct Cancellation;

impl schwarz_precond::EscalationPolicy for Cancellation {
    fn handler(&self) -> Box<dyn schwarz_precond::EscalationHandler> {
        Box::new(Self)
    }
}

impl schwarz_precond::EscalationHandler for Cancellation {
    fn should_escalate(&mut self, _: schwarz_precond::Progress) -> bool {
        crate::interrupt::cancellation_requested()
    }
}

fn advance(
    session: &mut within::SolveSession<'_, '_>,
    replacement: Option<&within::PairReplacement>,
    cap: usize,
) -> std::result::Result<within::ActionProgress, ActionFailure> {
    let interrupted = |error| ActionFailure {
        error,
        preconditioner_breakdown: false,
    };
    crate::interrupt::checkpoint().map_err(interrupted)?;
    let progress = session
        .run_action(replacement, cap, Some(&Cancellation))
        .map_err(action_error)?;
    crate::interrupt::checkpoint().map_err(interrupted)?;
    Ok(progress)
}

/// Jacobi trial: from the third iteration on, project the iterations Jacobi
/// LSMR still needs from its recent contraction, the geometric mean of the last
/// two steps, toward the recurrence target. Escalate once the projected total
/// exceeds `TRIAL_ITERATION_BUDGET`, roughly the cost of building the additive
/// Schwarz preconditioner and running its usual dozen to twenty iterations.
/// LSMR's normal-equation residual is monotone, so a slow contraction is a
/// property of the problem. Cancellation still stops the action.
const TRIAL_ITERATION_BUDGET: f64 = 40.0;

struct JacobiTrial {
    target: f64,
}

struct JacobiTrialRun {
    target: f64,
    history: [f64; 2],
}

impl schwarz_precond::EscalationPolicy for JacobiTrial {
    fn handler(&self) -> Box<dyn schwarz_precond::EscalationHandler> {
        Box::new(JacobiTrialRun {
            target: self.target,
            history: [f64::NAN; 2],
        })
    }
}

impl JacobiTrialRun {
    /// Whether `current` after `iteration` steps projects beyond the budget.
    fn projected_beyond_budget(&self, iteration: usize, current: f64) -> bool {
        let earlier = self.history[0];
        let observed = earlier.is_finite() && earlier > 0.0 && current > self.target;
        if iteration < 3 || !observed {
            return false;
        }
        let contraction = (current / earlier).sqrt();
        if contraction >= 1.0 {
            return true;
        }
        let remaining = (self.target / current).ln() / contraction.ln();
        iteration as f64 + remaining > TRIAL_ITERATION_BUDGET
    }
}

impl schwarz_precond::EscalationHandler for JacobiTrialRun {
    fn should_escalate(&mut self, progress: schwarz_precond::Progress) -> bool {
        if crate::interrupt::cancellation_requested() {
            return true;
        }
        let current = progress.normal_eq_residual;
        let escalate = self.projected_beyond_budget(progress.iteration, current);
        self.history = [self.history[1], current];
        escalate
    }
}

/// Advance a window's sessions to `cap`, under the Jacobi trial's escalation
/// policy when it has a recurrence target.
fn advance_window(
    sessions: &mut [within::SolveSession<'_, '_>],
    replacement: Option<&within::PairReplacement>,
    cap: usize,
    trial_target: Option<f64>,
) -> Vec<std::result::Result<within::ActionProgress, ActionFailure>> {
    sessions
        .par_iter_mut()
        .map(|session| match trial_target {
            Some(target) => advance_with(session, None, cap, &JacobiTrial { target }),
            None => advance(session, replacement, cap),
        })
        .collect()
}

/// Which RHSs stopped their unfinished Jacobi trial for escalation.
fn escalated(progress: &[within::ActionProgress], trial: bool) -> Vec<bool> {
    progress
        .iter()
        .map(|progress| {
            trial
                && !progress.converged
                && progress.stop_reason == schwarz_precond::LsmrStopReason::Escalated
        })
        .collect()
}

fn advance_with(
    session: &mut within::SolveSession<'_, '_>,
    replacement: Option<&within::PairReplacement>,
    cap: usize,
    policy: &dyn schwarz_precond::EscalationPolicy,
) -> std::result::Result<within::ActionProgress, ActionFailure> {
    let interrupted = |error| ActionFailure {
        error,
        preconditioner_breakdown: false,
    };
    crate::interrupt::checkpoint().map_err(interrupted)?;
    let progress = session
        .run_action(replacement, cap, Some(policy))
        .map_err(action_error)?;
    crate::interrupt::checkpoint().map_err(interrupted)?;
    Ok(progress)
}

fn build_error(error: within::BuildError) -> FeregError {
    super::super::lsmr_batch::BatchError::Upstream(error.into())
        .into_fereg("general routing pair construction failed")
}

fn append(
    batch: &mut Option<within::BatchSolveResult>,
    mut solved: within::SolveResult,
    rhs: usize,
    rows: usize,
    dofs: usize,
) -> Result<()> {
    if batch.is_none() {
        let capacity = |size: usize| {
            size.checked_mul(rhs)
                .ok_or_else(|| allocation("routing batch dimensions overflow"))
        };
        let mut x = Vec::new();
        x.try_reserve_exact(capacity(dofs)?)
            .map_err(|_| allocation("unable to reserve routing coefficients"))?;
        let mut demeaned = Vec::new();
        demeaned
            .try_reserve_exact(capacity(rows)?)
            .map_err(|_| allocation("unable to reserve routing residuals"))?;
        *batch = Some(within::BatchSolveResult {
            x,
            demeaned,
            unidentified: std::mem::take(&mut solved.unidentified),
            warnings: std::mem::take(&mut solved.warnings),
            layout: solved.layout.clone(),
            converged: reserved(rhs)?,
            iterations: reserved(rhs)?,
            residual: reserved(rhs)?,
            time_solve: reserved(rhs)?,
            time_setup: 0.0,
            time_total: 0.0,
            n_dofs: dofs,
            n_obs: rows,
        });
    }
    let batch = batch.as_mut().expect("routing batch initialized");
    batch.x.append(&mut solved.x);
    batch.demeaned.append(&mut solved.demeaned);
    batch.converged.push(solved.converged);
    batch.iterations.push(solved.iterations);
    batch.residual.push(solved.residual);
    batch.time_solve.push(solved.time_solve);
    batch.time_setup += solved.time_setup;
    Ok(())
}

fn reserved<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| allocation("unable to reserve routing metadata"))?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

    pub(super) struct Fixture {
        codes: Vec<Vec<u32>>,
        indices: Vec<LevelIndex>,
        rhs: Vec<Vec<f64>>,
    }

    impl Fixture {
        pub(super) fn new() -> Self {
            let mut codes = vec![Vec::new(); 3];
            let mut rhs = vec![Vec::new(), Vec::new()];
            for worker in 0..160 {
                for firm in [worker, worker + 1] {
                    for year in 0..3 {
                        codes[0].push(worker as u32);
                        codes[1].push(firm as u32);
                        codes[2].push(year);
                        let row = codes[0].len() as f64;
                        rhs[0].push((row * 0.137).sin() + worker as f64 * 0.01);
                        rhs[1].push((row * 0.319).cos() - firm as f64 * 0.02);
                    }
                }
            }
            let indices = codes
                .iter()
                .zip([160, 161, 3])
                .map(|(column, levels)| LevelIndex::from_codes(column, levels).unwrap())
                .collect();
            Self {
                codes,
                indices,
                rhs,
            }
        }

        pub(super) fn design(&self) -> BorrowedInterceptFeDesign<'_> {
            let effects = self
                .codes
                .iter()
                .zip(&self.indices)
                .zip([160, 161, 3])
                .map(|((column, index), levels)| {
                    ValidatedBorrowedInterceptSource::new(column, levels, index)
                        .unwrap()
                        .effect()
                })
                .collect();
            BorrowedInterceptFeDesign::new(self.codes[0].len(), effects).unwrap()
        }
    }

    #[test]
    fn jacobi_trial_projects_contraction_against_the_iteration_budget() {
        let run = |residuals: &[f64]| {
            let mut handler = JacobiTrialRun {
                target: 1e-10,
                history: [f64::NAN; 2],
            };
            residuals.iter().enumerate().position(|(index, &current)| {
                let escalate = handler.projected_beyond_budget(index + 1, current);
                handler.history = [handler.history[1], current];
                escalate
            })
        };
        // A steady 0.4 contraction reaches the target near iteration 25.
        let mixing: Vec<_> = (0..30).map(|k| 0.25 * 0.4_f64.powi(k)).collect();
        assert_eq!(run(&mixing), None);
        // A fast first step followed by a plateau escalates once two steps show it.
        assert_eq!(run(&[0.25, 0.028, 0.026, 0.024, 0.018]), Some(3));
        // Stagnation and growth escalate; reaching the target never does.
        assert_eq!(run(&[0.3, 0.3, 0.3]), Some(2));
        assert_eq!(run(&[0.3, 0.1, 1e-11, 1e-11, 1e-11]), None);
        // The first two iterations never decide.
        assert_eq!(run(&[0.9, 0.95]), None);
    }

    #[test]
    fn portfolio_general_seeded_schwarz_never_scans_or_builds_optional_pairs() {
        let fixture = Fixture::new();
        let design = fixture.design();
        let weights = validate_weights(BorrowedMapWeights::Unit, design.nobs()).unwrap();
        let options = LsmrOptions {
            tol: 1e-14,
            maxiter: 200,
            local_size: None,
        };
        let rhs: Vec<_> = fixture.rhs.iter().map(Vec::as_slice).collect();
        let mut seed = FeCoefficientBatch::intercept_zeros(&design, rhs.len()).unwrap();
        for (index, value) in seed.values.iter_mut().enumerate() {
            *value = (index as f64 * 0.173).sin();
        }
        let solver =
            build_retained_borrowed_lsmr(&design, weights, PreconditionerConfig::default(), true)
                .unwrap();
        let expected: Vec<_> = rhs
            .iter()
            .enumerate()
            .map(|(column, original)| {
                let mut session = solver
                    .start_session_categorical_seeded(original, seed.rhs(column).unwrap(), &options)
                    .unwrap();
                session.run_action(None, options.maxiter, None).unwrap();
                session.finish()
            })
            .collect();
        assert!(expected.iter().any(|result| result.iterations > 16));
        let estimate = estimate_fe_solver_memory(
            design.nobs(),
            rhs.len(),
            &[160, 161, 3],
            rayon::current_num_threads(),
            None,
        )
        .unwrap();
        let mut memory =
            BorrowedLsmrCallMemory::new(estimate, LsmrPreconditioner::Schwarz, None, 0).unwrap();
        let mut route = GeneralRoutingState::new(RoutingPreset::portfolio_v1());
        route.begin(1e-14, false);
        let (actual, fallback) = route
            .solve(
                &design,
                weights,
                &rhs,
                &options,
                LsmrPreconditioner::Auto,
                PreconditionerConfig::default(),
                true,
                &mut memory,
                Some(&seed),
            )
            .unwrap();
        assert!(!fallback);
        assert!(!route.setup.attempted);
        assert!(route.replacement.is_none());
        for (column, expected) in expected.iter().enumerate() {
            assert_eq!(actual.iterations[column], expected.iterations);
            assert_eq!(actual.x(column), expected.x);
            assert_eq!(actual.demeaned(column), expected.demeaned);
            for (codes, levels) in fixture.codes.iter().zip([160, 161, 3]) {
                let mut moments = vec![0.0; levels];
                for (row, &level) in codes.iter().enumerate() {
                    moments[level as usize] += actual.demeaned(column)[row];
                }
                assert!(moments.iter().all(|moment| moment.abs() < 1e-8));
            }
        }
    }

    #[test]
    fn refused_pair_build_resumes_exact_incumbent_recurrence() {
        let fixture = Fixture::new();
        let design = fixture.design();
        let weights = validate_weights(BorrowedMapWeights::Unit, design.nobs()).unwrap();
        let options = LsmrOptions {
            tol: 1e-14,
            maxiter: 200,
            local_size: None,
        };
        let solver =
            build_retained_borrowed_lsmr(&design, weights, PreconditionerConfig::default(), true)
                .unwrap();
        let expected: Vec<_> = fixture
            .rhs
            .iter()
            .map(|rhs| {
                let mut session = solver.start_session(rhs, &options).unwrap();
                let pilot = session.run_action(None, 4, None).unwrap();
                assert!(!pilot.converged, "fixture must exercise a live checkpoint");
                let remaining = session.remaining_iterations();
                session.run_action(None, remaining, None).unwrap();
                session.finish()
            })
            .collect();
        let estimate = estimate_fe_solver_memory(
            design.nobs(),
            2,
            &[160, 161, 3],
            rayon::current_num_threads(),
            None,
        )
        .unwrap();
        let mut memory =
            BorrowedLsmrCallMemory::new(estimate, LsmrPreconditioner::Schwarz, None, 0).unwrap();
        let mut route = GeneralRoutingState::new(RoutingPreset::new(4, 0).unwrap());
        route.begin(1e-8, false);
        route.prepared.solver = Some(solver);
        route.prepared.retained_bytes = estimate.lsmr_retained_bytes(LsmrPreconditioner::Schwarz);
        // Admit the incumbent sessions exactly; the optional reduced hierarchy
        // then has no memory and its refusal resumes the live recurrence.
        let admission = route
            .prepare_call(
                &design,
                weights,
                2,
                &options,
                PreconditionerConfig::default(),
                true,
                &memory,
                false,
            )
            .unwrap();
        assert!(admission.apply_bytes.is_some());
        memory.budget = Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: admission.parent,
        });
        let rhs: Vec<_> = fixture.rhs.iter().map(Vec::as_slice).collect();
        let (actual, fallback) = route
            .solve(
                &design,
                weights,
                &rhs,
                &options,
                LsmrPreconditioner::Auto,
                PreconditionerConfig::default(),
                true,
                &mut memory,
                None,
            )
            .unwrap();
        assert!(!fallback);
        assert!(route.setup.attempted);
        assert!(route.replacement.is_none());
        for (column, expected) in expected.iter().enumerate() {
            assert_eq!(actual.iterations[column], expected.iterations);
            assert_eq!(actual.x(column), expected.x);
            assert_eq!(actual.demeaned(column), expected.demeaned);
        }
    }

    #[test]
    fn seeded_sessions_keep_original_problem_with_all_preconditioners() {
        let fixture = Fixture::new();
        let design = fixture.design();
        let weights = validate_weights(BorrowedMapWeights::Unit, design.nobs()).unwrap();
        let options = LsmrOptions {
            tol: 1e-13,
            maxiter: 1000,
            local_size: Some(4),
        };
        let rhs: Vec<_> = fixture.rhs.iter().map(Vec::as_slice).collect();
        let mut seed = FeCoefficientBatch::intercept_zeros(&design, rhs.len()).unwrap();
        for (index, value) in seed.values.iter_mut().enumerate() {
            *value = (index as f64 * 0.173).sin();
        }
        for preconditioner in [
            PreconditionerConfig::Off,
            PreconditionerConfig::Diagonal,
            PreconditionerConfig::default(),
        ] {
            let solver =
                build_retained_borrowed_lsmr(&design, weights, preconditioner, true).unwrap();
            let seeded = solve_prepared_input(&solver, &rhs, &options, Some(&seed)).unwrap();
            let reference = solve_prepared_input(&solver, &rhs, &options, None).unwrap();
            assert!(seeded.iterations.iter().all(|&n| n <= options.maxiter));
            for (actual, expected) in seeded.demeaned.iter().zip(&reference.demeaned) {
                assert!(
                    (actual - expected).abs() <= 1e-8,
                    "seed changed original projection"
                );
            }
            for (column, original) in rhs.iter().enumerate() {
                let x = seeded.x(column);
                for row in 0..design.nobs() {
                    let fitted = x[fixture.codes[0][row] as usize]
                        + x[160 + fixture.codes[1][row] as usize]
                        + x[321 + fixture.codes[2][row] as usize];
                    assert!((original[row] - seeded.demeaned(column)[row] - fitted).abs() < 1e-10);
                }
            }
        }
    }

    #[test]
    fn held_parent_bytes_decline_optional_sessions_before_allocation() {
        let fixture = Fixture::new();
        let design = fixture.design();
        let weights = validate_weights(BorrowedMapWeights::Unit, design.nobs()).unwrap();
        let options = LsmrOptions {
            tol: 1e-13,
            maxiter: 200,
            local_size: None,
        };
        let rhs: Vec<_> = fixture.rhs.iter().map(Vec::as_slice).collect();
        let estimate = estimate_fe_solver_memory(
            design.nobs(),
            rhs.len(),
            &[160, 161, 3],
            rayon::current_num_threads(),
            None,
        )
        .unwrap();
        let mut memory =
            BorrowedLsmrCallMemory::new(estimate, LsmrPreconditioner::Schwarz, None, 0).unwrap();
        let mut route = GeneralRoutingState::new(RoutingPreset::new(8, 0).unwrap());
        route.begin(1e-8, false);
        let admission = route
            .prepare_call(
                &design,
                weights,
                rhs.len(),
                &options,
                PreconditionerConfig::default(),
                true,
                &memory,
                false,
            )
            .unwrap();
        assert!(admission.apply_bytes.is_some());
        memory.parent_bytes = 512;
        memory.budget = Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: admission.parent + 511,
        });
        let refused = route
            .prepare_call(
                &design,
                weights,
                rhs.len(),
                &options,
                PreconditionerConfig::default(),
                true,
                &memory,
                false,
            )
            .unwrap();
        assert!(refused.apply_bytes.is_none());
        let (_, fallback) = route
            .solve(
                &design,
                weights,
                &rhs,
                &options,
                LsmrPreconditioner::Auto,
                PreconditionerConfig::default(),
                true,
                &mut memory,
                None,
            )
            .unwrap();
        assert!(!fallback);
        assert!(route.replacement.is_none());
        assert!(route.setup.attempted);
    }

    #[test]
    fn generation_setup_is_shared_and_backend_release_keeps_attempt_record() {
        let mut state = GeneralRoutingState::new(RoutingPreset::new(8, 1).unwrap());
        state.begin(1e-8, false);
        state.setup.attempted = true;
        state.begin(1e-8, false);
        assert!(state.setup.attempted);
        state.release_backend();
        assert!(state.setup.attempted);
        state.begin(1e-12, true);
        assert!(!state.setup.attempted);
    }

    #[test]
    fn accepted_pair_upgrade_is_certified_and_reused_without_new_setup() {
        let fixture = Fixture::new();
        let design = fixture.design();
        let weights = validate_weights(BorrowedMapWeights::Unit, design.nobs()).unwrap();
        let options = LsmrOptions {
            tol: 1e-14,
            maxiter: 200,
            local_size: None,
        };
        let estimate = estimate_fe_solver_memory(
            design.nobs(),
            2,
            &[160, 161, 3],
            rayon::current_num_threads(),
            None,
        )
        .unwrap();
        let mut memory =
            BorrowedLsmrCallMemory::new(estimate, LsmrPreconditioner::Schwarz, None, 0).unwrap();
        let mut route = GeneralRoutingState::new(RoutingPreset::new(4, 1).unwrap());
        route.begin(1e-8, false);
        let rhs: Vec<_> = fixture.rhs.iter().map(Vec::as_slice).collect();
        let (first, fallback) = route
            .solve(
                &design,
                weights,
                &rhs,
                &options,
                LsmrPreconditioner::Auto,
                PreconditionerConfig::default(),
                true,
                &mut memory,
                None,
            )
            .unwrap();
        assert!(!fallback);
        assert!(
            route.replacement.is_some(),
            "fixture must complete the optional upgrade"
        );
        assert!(route.replacement_apply_bytes > 0);
        assert_eq!(route.prepared.builds, 1);
        let retained = route.replacement_bytes;
        let next: Vec<_> = fixture.rhs[1]
            .iter()
            .enumerate()
            .map(|(row, value)| value * 1.7 + (row as f64 * 0.413).sin())
            .collect();
        route.begin(1e-8, false);
        let (second, fallback) = route
            .solve(
                &design,
                weights,
                &[&next],
                &options,
                LsmrPreconditioner::Auto,
                PreconditionerConfig::default(),
                true,
                &mut memory,
                None,
            )
            .unwrap();
        assert!(!fallback);
        assert!(route.replacement.is_some());
        assert_eq!(route.replacement_bytes, retained);
        assert_eq!(route.prepared.builds, 1);
        for (solved, originals) in [(&first, rhs), (&second, vec![next.as_slice()])] {
            assert!(solved
                .iterations
                .iter()
                .all(|iterations| *iterations <= options.maxiter));
            for (column, original) in originals.iter().enumerate() {
                // Independent unweighted FE moment witness in original factor
                // slots, plus reconstruction from retained FE coefficients.
                let norm = original
                    .iter()
                    .map(|value| value * value)
                    .sum::<f64>()
                    .sqrt();
                let residual = solved.demeaned(column);
                let coefficients = solved.x(column);
                let mut offset = 0;
                for (codes, levels) in fixture.codes.iter().zip([160, 161, 3]) {
                    let mut sums = vec![0.0; levels];
                    let mut counts = vec![0usize; levels];
                    for (row, &level) in codes.iter().enumerate() {
                        sums[level as usize] += residual[row];
                        counts[level as usize] += 1;
                    }
                    for (sum, count) in sums.iter().zip(counts) {
                        assert!(sum.abs() <= 1e-9 * norm * (count as f64).sqrt());
                    }
                    assert_eq!(coefficients[offset..offset + levels].len(), levels);
                    offset += levels;
                }
                for row in 0..design.nobs() {
                    let fitted = coefficients[fixture.codes[0][row] as usize]
                        + coefficients[160 + fixture.codes[1][row] as usize]
                        + coefficients[321 + fixture.codes[2][row] as usize];
                    assert!((original[row] - residual[row] - fitted).abs() <= 1e-10 * norm);
                }
            }
        }
    }
}
