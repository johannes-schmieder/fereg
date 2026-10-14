//! General categorical overlap keeps the builder's owned cursor across Low
//! actions. Foreground partitions continue to use the unchanged installed team.

use super::*;
use crate::fe::routing_overlap::{arbitrate, Preparation};
use cmg::{BuildBoundary, BuildControl, BuildDecision, BuildOperation};

// Fixed service, synchronization, closure and error/cleanup transport. Numeric
// hierarchy allocations and within metadata have their own admitted bounds.
pub(super) const SERVICE_BYTES: u128 = 4096;

#[derive(Default)]
struct BuildMemory {
    allocated: u128,
    retained: u128,
    parent: u128,
    budget: Option<FeSolverMemoryBudget>,
}

struct PairProduct {
    replacement: Option<within::PairReplacement>,
    seconds: f64,
}

pub(super) struct PairPreparation {
    service: Preparation<PairProduct>,
    memory: Arc<Mutex<BuildMemory>>,
    charged: u64,
}

impl PairPreparation {
    /// The previous call drained its issued prefix before returning. Refresh
    /// the joint envelope before this call starts foreground work or grants.
    /// The setup allowance and issued/charged prefix are deliberately unchanged.
    pub(super) fn refresh_envelope(
        &self,
        parent: u128,
        budget: Option<FeSolverMemoryBudget>,
    ) -> bool {
        let mut memory = self.memory.lock().unwrap_or_else(|e| e.into_inner());
        if parent
            .checked_add(memory.allocated)
            .is_none_or(|live| budget.is_some_and(|budget| !budget.fits(live)))
        {
            return false;
        }
        memory.parent = parent;
        memory.budget = budget;
        true
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        SERVICE_BYTES + self.allocated_bytes()
    }

    pub(super) fn allocated_bytes(&self) -> u128 {
        self.memory
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .allocated
    }
}

impl GeneralRoutingState<'_> {
    pub(crate) fn drain_preparation(&mut self) -> Result<()> {
        self.park_for_certificate()?;
        self.certificate_boundary(Ok(()))
    }

    fn park_for_certificate(&mut self) -> Result<()> {
        if let Some(preparation) = &mut self.preparation {
            let result = preparation.service.drain();
            let charged = preparation.service.charged();
            self.setup
                .charge(charged.saturating_sub(preparation.charged))?;
            preparation.charged = charged;
            if let Err(error) = result {
                if error.code == ErrorCode::Interrupted {
                    return Err(error);
                }
                self.preparation_error = Some(error);
            }
        }
        Ok(())
    }

    pub(in crate::fe::borrowed_map) fn certificate_boundary<T>(
        &mut self,
        foreground: Result<T>,
    ) -> Result<T> {
        if self.preparation_error.is_some() || foreground.is_err() {
            return arbitrate(foreground, self.finish_preparation());
        }
        foreground
    }

    pub(crate) fn finish_preparation(&mut self) -> Result<()> {
        // This boundary closes the complete RHS set or a failed action. Low
        // handoffs use drain instead; a new accuracy key explicitly resets it.
        self.setup.attempted = true;
        let mut result = self.preparation_error.take().map_or(Ok(()), Err);
        if let Some(preparation) = self.preparation.take() {
            let (terminal, charged) = preparation.service.finish_accounted(false);
            self.setup
                .charge(charged.saturating_sub(preparation.charged))?;
            result = arbitrate(result, terminal.map(|_| ()));
        }
        result
    }
}

impl<'a> GeneralRoutingState<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn solve_overlap_windows(
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
        initial: Option<&FeCoefficientBatch>,
    ) -> Result<(within::BatchSolveResult, bool)> {
        // Take the owner temporarily so the controller can drain/update its
        // ledger while sessions borrow the exact originating solver identity.
        let solver = self
            .prepared
            .solver
            .take()
            .expect("retained overlap solver");
        let result = self.overlap_actions(
            &solver,
            design,
            rhs,
            options,
            memory,
            parent,
            window,
            initial,
            &mut setup_seconds,
        );
        self.prepared.solver = Some(solver);
        let mut batch = match result {
            Ok(batch) => batch,
            Err((failure, spent)) => {
                let foreground = Err(failure.error.clone());
                let error = arbitrate::<()>(foreground, self.finish_preparation()).unwrap_err();
                let failure = ActionFailure {
                    preconditioner_breakdown: failure.preconditioner_breakdown
                        && error.code == failure.error.code
                        && error.message == failure.error.message,
                    error,
                };
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
                    initial,
                );
            }
        };
        self.prepared.batches += 1;
        batch.time_setup += setup_seconds;
        batch.time_total = started.elapsed().as_secs_f64();
        Ok((batch, false))
    }

    #[allow(clippy::too_many_arguments)]
    fn overlap_actions(
        &mut self,
        solver: &within::Solver<'_>,
        design: &BorrowedInterceptFeDesign<'_>,
        rhs: &[&[f64]],
        options: &LsmrOptions,
        memory: &mut BorrowedLsmrCallMemory,
        parent: u128,
        window: usize,
        initial: Option<&FeCoefficientBatch>,
        setup_seconds: &mut f64,
    ) -> std::result::Result<within::BatchSolveResult, (ActionFailure, usize)> {
        let failure = |error| {
            (
                ActionFailure {
                    error,
                    preconditioner_breakdown: false,
                },
                0,
            )
        };
        let mut batch = None;
        for (window_index, columns) in rhs.chunks(window.max(1)).enumerate() {
            let mut sessions = reserved(columns.len()).map_err(failure)?;
            for (offset, column) in columns.iter().enumerate() {
                let session = if let Some(seed) = initial {
                    solver.start_session_categorical_seeded(
                        column,
                        seed.rhs(window_index * window + offset).map_err(failure)?,
                        options,
                    )
                } else {
                    solver.start_session(column, options)
                };
                sessions.push(session.map_err(|error| failure(solve_error(error)))?);
            }
            let mut results = reserved(columns.len()).map_err(failure)?;
            let mut progress = reserved(columns.len()).map_err(failure)?;
            let pilot = self.replacement.is_none()
                && (!self.setup.attempted || self.preparation.is_some())
                && options.maxiter > self.preset.checkpoint();
            let mut epoch = 0;
            loop {
                let cap = if pilot && epoch < self.preset.checkpoint() {
                    4
                } else {
                    options.maxiter
                };
                sessions
                    .par_iter_mut()
                    .map(|session| advance(session, self.replacement.as_ref(), cap))
                    .collect_into_vec(&mut results);
                let spent = consumed(&sessions, batch.as_ref(), options.maxiter);
                if let Some(error) = action_failure(&results) {
                    return Err((error, spent));
                }
                progress.clear();
                progress.extend(
                    results
                        .iter()
                        .map(|p| *p.as_ref().expect("actions checked")),
                );
                epoch = progress
                    .iter()
                    .map(|p| p.total_iterations)
                    .max()
                    .unwrap_or(0);
                // A recurrence-ready window proceeds directly to its mandatory
                // original-design certificate outside this transport. It cannot
                // grant another prefix, including when ready exactly at h.
                if progress.iter().all(|p| {
                    p.converged
                        || p.stop_reason == schwarz_precond::LsmrStopReason::FalseConvergence
                }) || progress.iter().all(|p| p.iterations == 0)
                    || sessions.iter().all(|s| s.remaining_iterations() == 0)
                {
                    self.park_for_certificate().map_err(failure)?;
                    break;
                }
                if !pilot || epoch > self.preset.checkpoint() {
                    self.park_for_certificate().map_err(failure)?;
                    break;
                }
                if epoch < 4 || epoch % 4 != 0 {
                    self.park_for_certificate().map_err(failure)?;
                    break;
                }
                if epoch == self.preset.checkpoint() {
                    self.join_pair_at_checkpoint(
                        solver,
                        options,
                        columns.len(),
                        window,
                        parent,
                        memory,
                        setup_seconds,
                    )
                    .map_err(failure)?;
                    // The h decision is frozen even if construction refused.
                    // The next action uses all remaining native allowance.
                    epoch += 1;
                    continue;
                }
                if self.preparation.is_some() {
                    self.preparation
                        .as_mut()
                        .expect("retained preparation")
                        .service
                        .grant_epoch()
                        .map_err(failure)?;
                } else if !self.setup.attempted {
                    self.setup.attempted = true;
                    let setup_started = Instant::now();
                    self.preparation = launch_pair(
                        solver,
                        self.preset,
                        &mut self.setup,
                        &mut self.incumbent_work,
                        &mut self.design_work,
                        self.base_work,
                        self.initial_rhs,
                        parent,
                        memory,
                    )
                    .map_err(failure)?;
                    *setup_seconds += setup_started.elapsed().as_secs_f64();
                }
                if self.preparation.is_none() {
                    epoch = self.preset.checkpoint() + 1;
                }
            }
            if let Some(preparation) = &self.preparation {
                memory
                    .include(parent + preparation.allocated_bytes())
                    .map_err(failure)?;
            }
            for session in sessions {
                append(
                    &mut batch,
                    session.finish(),
                    rhs.len(),
                    design.nobs(),
                    design.total_term_slots(),
                )
                .map_err(failure)?;
            }
        }
        batch.ok_or_else(|| failure(invalid_argument("empty general overlap batch")))
    }

    #[allow(clippy::too_many_arguments)]
    fn join_pair_at_checkpoint(
        &mut self,
        solver: &within::Solver<'_>,
        options: &LsmrOptions,
        rhs: usize,
        window: usize,
        parent: u128,
        memory: &mut BorrowedLsmrCallMemory,
        setup_seconds: &mut f64,
    ) -> Result<()> {
        if let Some(preparation) = self.preparation.take() {
            let ((product, charged), build_memory) = (
                preparation.service.finish_accounted(true),
                preparation.memory,
            );
            self.setup
                .charge(charged.saturating_sub(preparation.charged))?;
            let product = product?;
            let (retained, allocated) = {
                let usage = build_memory.lock().unwrap_or_else(|e| e.into_inner());
                (usage.retained, usage.allocated)
            };
            memory.include(
                parent
                    .checked_add(allocated)
                    .ok_or_else(|| allocation("pair construction peak overflows"))?,
            )?;
            *setup_seconds += product.seconds;
            self.install_pair(
                solver,
                product.replacement,
                retained,
                options,
                rhs,
                window,
                parent,
                memory,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn install_pair(
        &mut self,
        solver: &within::Solver<'_>,
        candidate: Option<within::PairReplacement>,
        retained: u128,
        options: &LsmrOptions,
        rhs: usize,
        window: usize,
        parent: u128,
        memory: &mut BorrowedLsmrCallMemory,
    ) -> Result<()> {
        let Some(candidate) = candidate else {
            return Ok(());
        };
        let apply = candidate
            .apply_storage_bound()
            .and_then(|bytes| (bytes as u128).checked_mul(window as u128));
        let live = apply.and_then(|apply| parent.checked_add(retained)?.checked_add(apply));
        let startup = warm_restart_work(
            solver,
            &candidate,
            self.design_work,
            rhs,
            options.local_size,
        );
        if let Some((startup, live, apply)) = startup
            .zip(live)
            .zip(apply)
            .map(|((startup, live), apply)| (startup, live, apply))
            .filter(|(startup, live, _)| {
                *startup <= self.setup.limit.saturating_sub(self.setup.used)
                    && memory
                        .budget
                        .is_none_or(|budget| budget.fits(*live + memory.parent_bytes))
            })
        {
            memory.include(live)?;
            self.setup.charge(startup)?;
            self.replacement = Some(candidate);
            self.replacement_bytes = retained;
            self.replacement_apply_bytes = apply;
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn launch_pair(
    solver: &within::Solver<'_>,
    preset: RoutingPreset,
    setup: &mut SetupLedger,
    incumbent: &mut Option<u64>,
    design: &mut Option<u64>,
    base: Option<u64>,
    rhs: usize,
    parent: u128,
    memory: &mut BorrowedLsmrCallMemory,
) -> Result<Option<PairPreparation>> {
    if !prepare_work(solver, setup, incumbent, design, base, rhs)? {
        return Ok(None);
    }
    // Nomination and owned fixed inputs are constructed in a fixed order on the
    // controller before leasing a worker. Their work remains in the same ledger.
    let factory = PairFactory {
        preset,
        state: RefCell::new(FactoryState {
            setup,
            memory,
            parent,
            retained: 0,
            error: None,
            refused: false,
        }),
    };
    let plan = solver.prepare_pair_replacement(&factory);
    let state = factory.state.into_inner();
    if let Some(error) = state.error {
        return Err(error);
    }
    let plan = match plan {
        Ok(plan) => plan,
        Err(_) if state.refused => None,
        Err(error) => return Err(build_error(error)),
    };
    let Some(plan) = plan else {
        return Ok(None);
    };
    let limit = state.setup.builder_allowance();
    if limit == 0 {
        return Ok(None);
    }
    let epoch = state.setup.limit / 32;
    let budget = state.memory.budget;
    let parent = parent
        .checked_add(state.memory.parent_bytes)
        .ok_or_else(|| allocation("owned pair preparation overlap overflows"))?;
    let shared = Arc::new(Mutex::new(BuildMemory {
        parent,
        budget,
        ..BuildMemory::default()
    }));
    let build_memory = shared.clone();
    let service =
        Preparation::launch_with_break(limit, epoch, 1, preset.breaks_builder(), move |control| {
            let started = Instant::now();
            let mut gate = PairControl {
                inner: control,
                memory: build_memory.clone(),
                waited: std::time::Duration::ZERO,
            };
            let factory = OwnedFactory {
                preset,
                state: RefCell::new(OwnedFactoryState {
                    control: &mut gate,
                    budget,
                    parent,
                    limit,
                    used: 0,
                    retained: 0,
                    error: None,
                    refused: false,
                }),
            };
            let candidate = plan.build(&factory);
            let OwnedFactoryState {
                retained,
                error,
                refused,
                ..
            } = factory.state.into_inner();
            build_memory
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retained = retained;
            if let Some(error) = error {
                return Err(error);
            }
            let replacement = match candidate {
                Ok(candidate) => Some(candidate),
                Err(_) if refused => None,
                Err(error) => return Err(build_error(error)),
            };
            Ok(PairProduct {
                replacement,
                seconds: started.elapsed().saturating_sub(gate.waited).as_secs_f64(),
            })
        })?;
    Ok(Some(PairPreparation {
        service,
        memory: shared,
        charged: 0,
    }))
}

struct PairControl<'a> {
    inner: &'a mut dyn BuildControl,
    memory: Arc<Mutex<BuildMemory>>,
    waited: std::time::Duration,
}

impl BuildControl for PairControl<'_> {
    fn checkpoint(&mut self, boundary: BuildBoundary) -> BuildDecision {
        if let BuildOperation::Allocation { bytes } = boundary.operation {
            let mut memory = self.memory.lock().unwrap_or_else(|e| e.into_inner());
            let Some(allocated) = memory.allocated.checked_add(bytes as u128) else {
                return BuildDecision::StopUnused;
            };
            if memory
                .parent
                .checked_add(allocated)
                .is_none_or(|live| memory.budget.is_some_and(|budget| !budget.fits(live)))
            {
                return BuildDecision::StopUnused;
            }
            memory.allocated = allocated;
        }
        let started = Instant::now();
        let decision = self.inner.checkpoint(boundary);
        self.waited += started.elapsed();
        decision
    }
}

struct OwnedFactory<'a> {
    preset: RoutingPreset,
    state: RefCell<OwnedFactoryState<'a>>,
}

struct OwnedFactoryState<'a> {
    control: &'a mut dyn BuildControl,
    // This initial CMG-local filter can remain more restrictive after a Low
    // handoff. PairControl's refreshed envelope is authoritative at every
    // allocation, including inside a component already in progress.
    budget: Option<FeSolverMemoryBudget>,
    parent: u128,
    limit: u64,
    used: u64,
    retained: u128,
    error: Option<FeregError>,
    refused: bool,
}

impl OwnedFactoryState<'_> {
    fn allocation(
        &mut self,
        site: &'static str,
        bytes: usize,
    ) -> std::result::Result<(), within::BuildError> {
        let boundary = BuildBoundary {
            phase: site,
            chunk_id: self.used,
            charged_work: self.used,
            operation: BuildOperation::Allocation { bytes },
        };
        match self.control.checkpoint(boundary) {
            BuildDecision::Continue => Ok(()),
            BuildDecision::StopUnused => {
                self.refused = true;
                Err(rejected())
            }
            BuildDecision::Interrupted => {
                self.error = Some(FeregError::new(
                    ErrorCode::Interrupted,
                    "pair preparation interrupted",
                ));
                Err(rejected())
            }
        }
    }
}

impl within::FixedReducedInverseFactory for OwnedFactory<'_> {
    fn allocation(
        &self,
        site: &'static str,
        bytes: usize,
    ) -> std::result::Result<(), within::BuildError> {
        self.state.borrow_mut().allocation(site, bytes)
    }

    fn checkpoint(&self, mut units: u64) -> std::result::Result<(), within::BuildError> {
        let mut state = self.state.borrow_mut();
        while units > 0 {
            let chunk = units.min(1024);
            if chunk > state.limit.saturating_sub(state.used) {
                state.refused = true;
                return Err(rejected());
            }
            let boundary = BuildBoundary {
                phase: "within_pair",
                chunk_id: state.used,
                charged_work: state.used,
                operation: BuildOperation::Work {
                    units: chunk as u16,
                },
            };
            match state.control.checkpoint(boundary) {
                BuildDecision::Continue => state.used += chunk,
                BuildDecision::StopUnused => {
                    state.refused = true;
                    return Err(rejected());
                }
                BuildDecision::Interrupted => {
                    state.error = Some(FeregError::new(
                        ErrorCode::Interrupted,
                        "pair preparation interrupted",
                    ));
                    return Err(rejected());
                }
            }
            units -= chunk;
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
        let budget = if let Some(budget) = state.budget {
            Some(FeSolverMemoryBudget {
                execution_bytes: budget
                    .execution_bytes
                    .checked_add(parent)
                    .ok_or_else(rejected)?,
                usable_bytes: budget.usable_bytes,
            })
        } else {
            None
        };
        let allowance = state.limit.saturating_sub(state.used);
        let attempt = CmgSchurPreconditioner::try_build_reduced_bounded_controlled(
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
            allowance,
            state.control,
        );
        let (attempt, scale) = attempt.map_err(|error| {
            state.error = Some(error);
            rejected()
        })?;
        state.used = state
            .used
            .checked_add(attempt.work_used)
            .ok_or_else(rejected)?;
        let Some(preconditioner) = attempt.preconditioner else {
            state.refused = true;
            return Err(rejected());
        };
        let retained = preconditioner.retained_bytes().map_err(|error| {
            state.error = Some(error);
            rejected()
        })?;
        state.retained = state.retained.checked_add(retained).ok_or_else(rejected)?;
        state.allocation(
            "within_reduced_inverse_owner",
            std::mem::size_of::<ReducedInverse>()
                .checked_add(2 * std::mem::size_of::<usize>())
                .and_then(|bytes| bytes.checked_add(std::mem::align_of::<ReducedInverse>() - 1))
                .ok_or_else(rejected)?,
        )?;
        Ok(Arc::new(ReducedInverse {
            solver: Mutex::new(preconditioner),
            dimension: component.n_retained,
            scale,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boundary(chunk_id: u64, charged_work: u64, operation: BuildOperation) -> BuildBoundary {
        BuildBoundary {
            phase: "low envelope witness",
            chunk_id,
            charged_work,
            operation,
        }
    }

    fn prepare<'a>(
        route: &mut GeneralRoutingState<'a>,
        design: &BorrowedInterceptFeDesign<'a>,
        rhs: usize,
        options: &LsmrOptions,
        memory: &BorrowedLsmrCallMemory,
    ) -> SessionAdmission {
        let weights = validate_weights(BorrowedMapWeights::Unit, design.nobs()).unwrap();
        route
            .prepare_call(
                design,
                weights,
                rhs,
                options,
                PreconditionerConfig::default(),
                true,
                memory,
                false,
            )
            .unwrap()
    }

    fn run_refresh_case(rhs: usize, held: u128, tighten_budget: bool) {
        let fixture = super::super::tests::Fixture::new();
        let design = fixture.design();
        let options = LsmrOptions {
            tol: 1e-6,
            maxiter: 200,
            local_size: None,
        };
        let mut route = GeneralRoutingState::new(RoutingPreset::overlap(32, 0).unwrap());
        route.begin(1e-4, false, &design, 1, None).unwrap();
        let memory = |rhs, held| {
            let estimate =
                estimate_fe_solver_memory(design.nobs(), rhs, &[160, 161, 3], 2, None).unwrap();
            BorrowedLsmrCallMemory::new(estimate, LsmrPreconditioner::Schwarz, None, held).unwrap()
        };
        let initial_memory = memory(1, 0);
        let mut next_memory = memory(rhs, held);
        let initial = prepare(&mut route, &design, 1, &options, &initial_memory);
        let next = prepare(&mut route, &design, rhs, &options, &next_memory);
        let parent = next.parent + held;
        let budget = FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: parent + 39,
        };
        let old_budget = if tighten_budget {
            FeSolverMemoryBudget {
                usable_bytes: parent + 1024,
                ..budget
            }
        } else {
            budget
        };
        assert!(old_budget.fits(initial.parent + 40));
        assert!(budget.fits(parent + 16));
        assert!(!budget.fits(parent + 40));
        let shared = Arc::new(Mutex::new(BuildMemory {
            parent: initial.parent,
            budget: Some(old_budget),
            ..BuildMemory::default()
        }));
        let future = Arc::new(Mutex::new(None));
        let (worker_memory, worker_future) = (shared.clone(), future.clone());
        let service = Preparation::launch_with_break(8, 1, 1, false, move |inner| {
            let mut gate = PairControl {
                inner,
                memory: worker_memory,
                waited: std::time::Duration::ZERO,
            };
            assert_eq!(
                gate.checkpoint(boundary(0, 0, BuildOperation::Allocation { bytes: 16 })),
                BuildDecision::Continue
            );
            assert_eq!(
                gate.checkpoint(boundary(1, 0, BuildOperation::Work { units: 1 })),
                BuildDecision::Continue
            );
            if gate.checkpoint(boundary(2, 1, BuildOperation::Work { units: 1 }))
                == BuildDecision::Continue
            {
                *worker_future.lock().unwrap() =
                    Some(gate.checkpoint(boundary(3, 2, BuildOperation::Allocation { bytes: 24 })));
            }
            Ok(PairProduct {
                replacement: None,
                seconds: 0.0,
            })
        })
        .unwrap();
        route.preparation = Some(PairPreparation {
            service,
            memory: shared.clone(),
            charged: 0,
        });
        route.park_for_certificate().unwrap();
        let work = route.work();
        assert_eq!(shared.lock().unwrap().allocated, 16);
        assert_eq!(*future.lock().unwrap(), None);
        next_memory.budget = Some(budget);
        assert!(prepare(&mut route, &design, rhs, &options, &next_memory)
            .apply_bytes
            .is_some());
        assert_eq!(route.work(), work, "refresh must not issue or charge work");
        assert_eq!(shared.lock().unwrap().parent, parent);
        assert_eq!(shared.lock().unwrap().budget, Some(budget));
        // Already-held payload must refuse the new foreground admission, too.
        next_memory.budget = Some(FeSolverMemoryBudget {
            usable_bytes: parent + 15,
            ..budget
        });
        assert!(prepare(&mut route, &design, rhs, &options, &next_memory)
            .apply_bytes
            .is_none());
        assert_eq!(shared.lock().unwrap().budget, Some(budget));
        assert_eq!(route.work(), work);
        let preparation = route.preparation.as_mut().unwrap();
        assert_eq!(preparation.service.charged(), 1);
        preparation.service.grant_epoch().unwrap();
        route.park_for_certificate().unwrap();
        assert_eq!(*future.lock().unwrap(), Some(BuildDecision::StopUnused));
        assert_eq!(
            shared.lock().unwrap().allocated,
            16,
            "refusal precedes allocation"
        );
        assert_eq!(route.work(), (work.0 + 1, work.1));
        route.finish_preparation().unwrap();
    }

    #[test]
    fn parked_builder_refreshes_wider_and_same_width_memory_envelopes() {
        const NAME: &str = "parked_builder_refreshes_wider_and_same_width_memory_envelopes";
        const KEY: &str = "FEREG_GENERAL_MEMORY_TEST_CHILD";
        if std::env::var(KEY).ok().as_deref() != Some(NAME) {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(KEY, NAME)
                .args([
                    format!("fe::borrowed_map::routing_lsmr::overlap::tests::{NAME}"),
                    "--exact".into(),
                    "--test-threads=1".into(),
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        crate::parallel::ThreadTeam::new(
            crate::options::ThreadConfig::from_requested(2, true).unwrap(),
        )
        .with_exclusive_overlap()
        .install(|| {
            for (rhs, held, tighten) in [(2, 0, false), (1, 1024, false), (1, 0, true)] {
                run_refresh_case(rhs, held, tighten);
            }
        })
        .unwrap();
    }
}
