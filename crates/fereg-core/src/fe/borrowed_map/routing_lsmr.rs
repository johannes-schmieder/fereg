//! fereg-controlled categorical LSMR attempts.
//!
//! The immutable OLS binding retains preparation and its selected fixed
//! preconditioner. Coefficient warm starts carry Jacobi escalation and certified
//! corrections with the original response scale and remaining native budget.

use super::super::lsmr_batch::RhsSource;
use super::*;
use crate::fe::routing::RoutingPreset;

/// Run bounded independent attempts on the original RHS. Returned coefficient
/// seeds, rather than retained Krylov recurrences, carry a correction between
/// attempts; every completed or failed native iteration debits the call budget.
pub(super) fn solve_prepared_input(
    solver: &within::Solver<'_>,
    rhs: &[&[f64]],
    options: &LsmrOptions,
    initial: Option<&FeCoefficientBatch>,
) -> std::result::Result<within::BatchSolveResult, (super::super::lsmr_batch::BatchError, usize)> {
    super::super::lsmr_batch::solve_prepared_batch_seeded(
        solver,
        rhs,
        options,
        initial.map(|initial| initial.values.as_slice()),
    )
    .map_err(|error| {
        let spent = error.attempted_iterations();
        (error, spent)
    })
}

pub(crate) struct GeneralRoutingState<'a> {
    preset: RoutingPreset,
    prepared: BorrowedLsmrPrepared<'a>,
    /// Additive configuration a retained Jacobi trial escalates to.
    trial: Option<PreconditionerConfig>,
    /// The trial solver's prepared design, kept so escalation builds the
    /// additive solver without repeating the locality sort. A clone of
    /// borrowed columns is shallow; a sorted copy stays within the additive
    /// solver frame that admission charges while the trial is retained.
    trial_design: Option<within::Design<'a>>,
}

impl<'a> GeneralRoutingState<'a> {
    pub(crate) fn new(preset: RoutingPreset) -> Self {
        Self {
            preset,
            prepared: BorrowedLsmrPrepared::default(),
            trial: None,
            trial_design: None,
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

    pub(crate) fn release_backend(&mut self) {
        self.prepared = BorrowedLsmrPrepared::default();
        self.trial = None;
        self.trial_design = None;
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        self.prepared.retained_bytes
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn solve<R: RhsSource + ?Sized>(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        rhs: &R,
        options: &LsmrOptions,
        requested: LsmrPreconditioner,
        preconditioner: PreconditionerConfig,
        deterministic: bool,
        memory: &mut BorrowedLsmrCallMemory,
        initial_coefficients: Option<&FeCoefficientBatch>,
    ) -> Result<(within::BatchSolveResult, bool)> {
        if self
            .preset
            .retired_general_pair_request(Some(design.effects().len()))
        {
            return Err(invalid_argument(
                "general-FE pair-upgrade routing is no longer supported",
            ));
        }
        // Explicit preconditioner intent remains authoritative.
        if requested != LsmrPreconditioner::Auto
            || !matches!(preconditioner, PreconditionerConfig::Additive { .. })
            || self.prepared.preconditioner_fallback
        {
            let columns = rhs.all_columns()?;
            return self.prepared.solve(
                design,
                weights,
                &columns.views(),
                options,
                requested,
                preconditioner,
                deterministic,
                initial_coefficients,
            );
        }
        let started = Instant::now();
        let CallAdmission {
            parent,
            window,
            setup_seconds,
            admitted,
        } = self.prepare_call(
            design,
            weights,
            rhs.count(),
            options,
            preconditioner,
            deterministic,
            memory,
            initial_coefficients.is_some(),
        )?;
        if !admitted {
            let columns = rhs.all_columns()?;
            let (mut solved, fallback) = self.prepared.solve(
                design,
                weights,
                &columns.views(),
                options,
                requested,
                crate::fe::additive_schwarz(),
                deterministic,
                initial_coefficients,
            )?;
            solved.time_setup += setup_seconds;
            solved.time_total = started.elapsed().as_secs_f64();
            return Ok((solved, fallback));
        }
        memory.include(parent)?;
        self.solve_controlled_windows(
            design,
            weights,
            rhs,
            options,
            deterministic,
            window,
            setup_seconds,
            started,
            initial_coefficients,
        )
    }

    /// Public-policy attempts need a current coefficient vector at each
    /// deliberate stop, not the preceding Krylov recurrence. The original RHS
    /// keeps both convergence references unchanged across warm attempts.
    #[allow(clippy::too_many_arguments)]
    fn solve_controlled_windows<R: RhsSource + ?Sized>(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        rhs: &R,
        options: &LsmrOptions,
        deterministic: bool,
        window: usize,
        mut setup_seconds: f64,
        started: Instant,
        initial_coefficients: Option<&FeCoefficientBatch>,
    ) -> Result<(within::BatchSolveResult, bool)> {
        let mut batch: Option<within::BatchSolveResult> = None;
        for first in (0..rhs.count()).step_by(window.max(1)) {
            let positions: Vec<_> = (first..(first + window.max(1)).min(rhs.count())).collect();
            let solver = self
                .prepared
                .solver
                .as_ref()
                .expect("retained routing solver");
            let trial = self.trial.is_some();
            let results = controlled_window(
                solver,
                rhs,
                &positions,
                options,
                trial,
                initial_coefficients,
            )?;
            let spent = results
                .iter()
                .map(|result| match result {
                    Ok(result) => result.solution.iterations,
                    Err((_, iterations)) => *iterations,
                })
                .chain(
                    batch
                        .iter()
                        .flat_map(|batch| batch.iterations.iter().copied()),
                )
                .max()
                .unwrap_or(0);
            let failure = controlled_failure(&results);
            crate::interrupt::checkpoint()?;
            if let Some(failure) = failure {
                // The whole batch restarts on the permitted numerical fallback.
                // Release discarded response banks before that allocation.
                drop(results);
                drop(batch);
                return self.retry_off(
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
                );
            }
            let escalated: Vec<_> = results
                .iter()
                .map(|result| {
                    let result = result.as_ref().expect("controlled failures checked");
                    trial
                        && !result.solution.converged
                        && result.stop_reason == schwarz_precond::LsmrStopReason::Escalated
                })
                .collect();
            let mut finished: Vec<_> = results
                .into_iter()
                .map(|result| result.expect("controlled failures checked").solution)
                .collect();
            if escalated.contains(&true) {
                let (seconds, outcome) = self.escalate(
                    design,
                    weights,
                    rhs,
                    &positions,
                    options,
                    deterministic,
                    &escalated,
                    &mut finished,
                );
                setup_seconds += seconds;
                if let Err(failure) = outcome {
                    let spent = finished
                        .iter()
                        .map(|result| result.iterations)
                        .chain(
                            batch
                                .iter()
                                .flat_map(|batch| batch.iterations.iter().copied()),
                        )
                        .max()
                        .unwrap_or(0);
                    drop(finished);
                    drop(batch);
                    return self.retry_off(
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
                    );
                }
            }
            for solved in finished {
                append(
                    &mut batch,
                    solved,
                    rhs.count(),
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

    /// Build the fixed additive preconditioner selected by fereg and restart
    /// only the unfinished trial RHSs, with their remaining native budgets.
    #[allow(clippy::too_many_arguments)]
    fn escalate<R: RhsSource + ?Sized>(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        rhs: &R,
        positions: &[usize],
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
        self.prepared.solver = None;
        let built = match self.trial_design.take() {
            Some(shared) => build_retained_lsmr_from(shared, weights, config, deterministic),
            None => build_retained_borrowed_lsmr(design, weights, config, deterministic),
        };
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
        let restarted: Vec<_> = (0..positions.len())
            .filter(|&offset| escalated[offset] && options.maxiter > finished[offset].iterations)
            .collect();
        let selected: Vec<_> = restarted.iter().map(|&offset| positions[offset]).collect();
        let transported = match rhs.columns(&selected) {
            Ok(columns) => columns,
            Err(error) => return (seconds, Err(fatal(error))),
        };
        let columns = transported.views();
        let results: Vec<_> = restarted
            .par_iter()
            .zip(&columns)
            .map(|(&offset, column)| {
                let options = LsmrOptions {
                    maxiter: options.maxiter - finished[offset].iterations,
                    ..options.clone()
                };
                controlled_attempt(solver, column, &options, Some(&finished[offset].x), None)
            })
            .collect();
        let failure = controlled_failure(&results);
        // Debit failed iterations too, before propagating any fatal outcome.
        for (offset, result) in restarted.into_iter().zip(results) {
            merge_escalated_attempt(&mut finished[offset], result);
        }
        match crate::interrupt::checkpoint() {
            Err(error) => (seconds, Err(fatal(error))),
            Ok(()) => (seconds, failure.map_or(Ok(()), Err)),
        }
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
    ) -> Result<CallAdmission> {
        let estimate = super::borrowed_solver_memory_estimate(
            design,
            rhs_count,
            rayon::current_num_threads(),
            options.local_size,
        )?;
        let setup_started = Instant::now();
        if self.prepared.solver.is_none() {
            // The public many-FE policy first retains a Jacobi trial and keeps
            // the additive configuration for a measured escalation. Admission
            // still charges the additive bound, which covers both solvers.
            let additive = matches!(preconditioner, PreconditionerConfig::Additive { .. });
            self.trial = (additive && self.preset.geometry_pilot()).then(|| preconditioner.clone());
            self.prepared.solver = Some(if self.trial.is_some() {
                // Prepare the design once for the trial and its escalation.
                let shared = prepared_within_design(design)?;
                self.trial_design = Some(shared.clone());
                build_retained_lsmr_from(
                    shared,
                    weights,
                    PreconditionerConfig::Diagonal,
                    deterministic,
                )?
            } else {
                build_retained_borrowed_lsmr(design, weights, preconditioner, deterministic)?
            });
            self.prepared.retained_bytes =
                estimate.lsmr_retained_bytes(LsmrPreconditioner::Schwarz);
            self.prepared.builds += 1;
        }
        self.prepared.retained_bytes = self
            .prepared
            .retained_bytes
            .max(estimate.lsmr_retained_bytes(LsmrPreconditioner::Schwarz));
        let setup_seconds = setup_started.elapsed().as_secs_f64();
        // The candidate bounded-window policy keeps at most one live session
        // per worker; admission still charges the established window bound.
        let window = if self.preset.bounded_session_windows() {
            rayon::current_num_threads().clamp(1, rhs_count.max(1))
        } else {
            crate::memory::lsmr_rhs_window(rhs_count, rayon::current_num_threads())
        };
        // The ordinary model includes caller transport and all cold solve
        // buffers. Warm handoff additionally retains the current coefficients,
        // a seed copy and the transported escalation RHS. A seeded caller has
        // already charged its warm banks in BorrowedLsmrCallMemory.
        let rows = design.nobs() as u128;
        let dofs = design.total_term_slots() as u128;
        let warm_extra = rows
            .checked_mul(2)
            .and_then(|cells| cells.checked_add(dofs.checked_mul(6)?))
            .and_then(|cells| cells.checked_mul(window as u128))
            .and_then(|cells| cells.checked_mul(8))
            .and_then(|bytes| bytes.checked_add(window as u128 * 1024))
            .ok_or_else(|| allocation("controlled LSMR warm overlap overflows"))?;
        let historical = self
            .prepared
            .retained_bytes
            .saturating_sub(estimate.lsmr_retained_bytes(LsmrPreconditioner::Schwarz));
        let parent = estimate
            .lsmr_bytes(LsmrPreconditioner::Schwarz)
            .checked_add(historical)
            .and_then(|bytes| bytes.checked_add(if seeded { 0 } else { warm_extra }))
            .ok_or_else(|| allocation("controlled LSMR parent overlap overflows"))?;
        let admitted = parent
            .checked_add(memory.parent_bytes)
            .is_some_and(|bytes| memory.budget.is_none_or(|budget| budget.fits(bytes)));
        Ok(CallAdmission {
            parent,
            window,
            setup_seconds,
            admitted,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn retry_off<R: RhsSource + ?Sized>(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        rhs: &R,
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
        let options = LsmrOptions {
            maxiter: remaining,
            ..options.clone()
        };
        let columns = rhs.all_columns()?;
        let (mut solved, _) = self.prepared.solve(
            design,
            weights,
            &columns.views(),
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

struct CallAdmission {
    parent: u128,
    window: usize,
    setup_seconds: f64,
    admitted: bool,
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

type ControlledAttempt = std::result::Result<within::ControlledSolveResult, (ActionFailure, usize)>;

// Coefficients replace the prior iterate only after a successful attempt.
// Every attempted iteration still consumes the shared allowance on failure.
fn merge_escalated_attempt(prior: &mut within::SolveResult, result: ControlledAttempt) {
    match result {
        Ok(result) => {
            let mut solved = result.solution;
            solved.iterations += prior.iterations;
            solved.time_solve += prior.time_solve;
            *prior = solved;
        }
        Err((_, attempted)) => prior.iterations += attempted,
    }
}

// Keep the window's transported RHS columns alive until every independent
// attempt has joined, then release the transport before deciding on fallback.
fn controlled_window<R: RhsSource + ?Sized>(
    solver: &within::Solver<'_>,
    rhs: &R,
    positions: &[usize],
    options: &LsmrOptions,
    trial: bool,
    initial: Option<&FeCoefficientBatch>,
) -> Result<Vec<ControlledAttempt>> {
    let transported = rhs.columns(positions)?;
    let columns = transported.views();
    let policy = JacobiTrial {
        target: options.tol,
    };
    Ok(columns
        .par_iter()
        .zip(positions)
        .map(|(column, &position)| {
            let seed = initial
                .map(|seed| seed.rhs(position))
                .transpose()
                .map_err(|error| {
                    (
                        ActionFailure {
                            error,
                            preconditioner_breakdown: false,
                        },
                        0,
                    )
                })?;
            controlled_attempt(
                solver,
                column,
                options,
                seed,
                trial.then_some(&policy as &dyn schwarz_precond::EscalationPolicy),
            )
        })
        .collect())
}

fn controlled_attempt(
    solver: &within::Solver<'_>,
    rhs: &[f64],
    options: &LsmrOptions,
    seed: Option<&[f64]>,
    policy: Option<&dyn schwarz_precond::EscalationPolicy>,
) -> ControlledAttempt {
    crate::interrupt::checkpoint().map_err(|error| {
        (
            ActionFailure {
                error,
                preconditioner_breakdown: false,
            },
            0,
        )
    })?;
    solver
        .solve_controlled(
            rhs,
            options,
            within::SolveControl {
                initial_coefficients: seed,
                escalation: policy,
                cancellation: within::Cancellation::new(&crate::interrupt::cancellation_requested),
                residual_output: within::ResidualOutput::PreparedModel,
            },
        )
        .map_err(|failure| (action_error(failure.source), failure.iterations))
}

fn controlled_failure<T>(
    results: &[std::result::Result<T, (ActionFailure, usize)>],
) -> Option<ActionFailure> {
    let errors: Vec<_> = results
        .iter()
        .map(|result| {
            result
                .as_ref()
                .map(|_| ())
                .map_err(|(failure, _)| ActionFailure {
                    error: failure.error.clone(),
                    preconditioner_breakdown: failure.preconditioner_breakdown,
                })
        })
        .collect();
    action_failure(&errors)
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

fn append(
    batch: &mut Option<within::BatchSolveResult>,
    solved: within::SolveResult,
    rhs: usize,
    rows: usize,
    dofs: usize,
) -> Result<()> {
    super::super::lsmr_batch::append_solved(batch, solved, rhs, rows, dofs)
        .map_err(|error| error.into_fereg("general routing LSMR batch assembly failed"))
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
    fn failed_escalation_keeps_prior_work_in_the_off_retry_budget() {
        // Upstream failing-operator tests establish the error's attempted
        // count. Feed that counted outcome at the merge boundary, then solve
        // the actual Off retry with exactly the remaining shared allowance.
        let codes = [[0, 0, 1, 1, 1], [0, 0, 0, 1, 1]];
        let indices = codes.map(|codes| LevelIndex::from_codes(&codes, 2).unwrap());
        let effects = codes
            .iter()
            .zip(&indices)
            .map(|(codes, index)| {
                ValidatedBorrowedInterceptSource::new(codes, 2, index)
                    .unwrap()
                    .effect()
            })
            .collect();
        let design = BorrowedInterceptFeDesign::new(5, effects).unwrap();
        let raw_weights = [1.0; 5];
        let weights = validate_weights(BorrowedMapWeights::Provided(&raw_weights), 5).unwrap();
        let original = [1.0, 0.0, 1.0, 1.0, 0.0];
        let rhs = [&original[..]];
        let options = LsmrOptions {
            tol: 1e-10,
            maxiter: 500,
            local_size: Some(4),
        };
        let attempted = 2;
        let off = build_retained_borrowed_lsmr(&design, weights, PreconditionerConfig::Off, true)
            .unwrap();
        let mut prior = off.solve(&[0.0; 5], &options).unwrap();
        prior.iterations = 3;
        let failure = action_error(within::SolveError::InvalidInput {
            context: "mlsmr",
            message: "preconditioner not positive definite".to_owned(),
        });
        merge_escalated_attempt(
            &mut prior,
            Err((
                ActionFailure {
                    error: failure.error.clone(),
                    preconditioner_breakdown: true,
                },
                attempted,
            )),
        );
        let spent = 3 + attempted;
        assert_eq!(prior.iterations, spent);
        let mut route = GeneralRoutingState::new(RoutingPreset::geometry_many());
        let remaining = LsmrOptions {
            maxiter: options.maxiter - spent,
            ..options.clone()
        };
        let expected = off.solve(&original, &remaining).unwrap();
        let (actual, fallback) = route
            .retry_off(
                &design,
                weights,
                &rhs[..],
                &options,
                true,
                spent,
                ActionFailure {
                    error: failure.error.clone(),
                    preconditioner_breakdown: true,
                },
                0.0,
                Instant::now(),
                None,
            )
            .unwrap();
        assert!(fallback);
        assert_eq!(actual.iterations, [spent + expected.iterations]);
        assert_eq!(actual.demeaned, expected.demeaned);
        assert_eq!(actual.x, expected.x);
        let exhausted = LsmrOptions {
            maxiter: spent,
            ..options
        };
        let error = route
            .retry_off(
                &design,
                weights,
                &rhs[..],
                &exhausted,
                true,
                spent,
                failure,
                0.0,
                Instant::now(),
                None,
            )
            .unwrap_err();
        assert!(error
            .message
            .contains("preconditioner not positive definite"));
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
    fn seeded_schwarz_matches_original_coefficients_and_independent_moments() {
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
            build_retained_borrowed_lsmr(&design, weights, crate::fe::additive_schwarz(), true)
                .unwrap();
        let expected: Vec<_> = rhs
            .iter()
            .enumerate()
            .map(|(column, original)| {
                solver
                    .solve_controlled(
                        original,
                        &options,
                        within::SolveControl {
                            initial_coefficients: Some(seed.rhs(column).unwrap()),
                            residual_output: within::ResidualOutput::PreparedModel,
                            ..within::SolveControl::default()
                        },
                    )
                    .unwrap()
                    .solution
            })
            .collect();
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
        let (actual, fallback) = route
            .solve(
                &design,
                weights,
                &rhs[..],
                &options,
                LsmrPreconditioner::Auto,
                crate::fe::additive_schwarz(),
                true,
                &mut memory,
                Some(&seed),
            )
            .unwrap();
        assert!(!fallback);
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
    fn controlled_seeds_keep_original_problem_with_all_preconditioners() {
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
            crate::fe::additive_schwarz(),
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
    fn held_parent_bytes_decline_warm_attempt_before_allocation() {
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
        let mut route = GeneralRoutingState::new(RoutingPreset::portfolio_unseeded());
        let admission = route
            .prepare_call(
                &design,
                weights,
                rhs.len(),
                &options,
                crate::fe::additive_schwarz(),
                true,
                &memory,
                false,
            )
            .unwrap();
        assert!(admission.admitted);
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
                crate::fe::additive_schwarz(),
                true,
                &memory,
                false,
            )
            .unwrap();
        assert!(!refused.admitted);
        let (_, fallback) = route
            .solve(
                &design,
                weights,
                &rhs[..],
                &options,
                LsmrPreconditioner::Auto,
                crate::fe::additive_schwarz(),
                true,
                &mut memory,
                None,
            )
            .unwrap();
        assert!(!fallback);
    }
}
