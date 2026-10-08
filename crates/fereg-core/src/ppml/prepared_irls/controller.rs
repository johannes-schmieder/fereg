//! Strict recovery and publication readiness; borrowed phase views own no numerical buffers.
use super::*;
use crate::ols::StrictLinearCandidateTransaction;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingCheckpointWork {
    None,
    Refinement,
    Rebuild,
}
#[derive(Clone, Copy)]
enum FinalCheckpointReporting {
    AlreadyObserved,
    ObserveAtPublication,
}
#[derive(Clone, Copy)]
pub(super) struct AcceptedCheckpoint {
    reporting: FinalCheckpointReporting,
}
impl AcceptedCheckpoint {
    pub(super) fn needs_observation(self) -> bool {
        matches!(
            self.reporting,
            FinalCheckpointReporting::ObserveAtPublication
        )
    }
}

pub(super) struct StrictCheckpointControl {
    tolerance: f64,
    pending: PendingCheckpointWork,
    canonical_products_allowed: bool,
    recovery: ProfileRecovery,
    consecutive_passes: usize,
    rejected_cycles: usize,
    accepted: Option<AcceptedCheckpoint>,
}
impl StrictCheckpointControl {
    pub(super) fn new(tolerance: f64, canonical_products_allowed: bool) -> Self {
        Self {
            tolerance,
            pending: PendingCheckpointWork::None,
            canonical_products_allowed,
            recovery: ProfileRecovery::default(),
            consecutive_passes: 0,
            rejected_cycles: 0,
            accepted: None,
        }
    }
    pub(super) fn observe_accepted_iteration(&mut self, passed: bool) {
        self.consecutive_passes = if passed {
            self.consecutive_passes.saturating_add(1)
        } else {
            0
        };
    }
    pub(super) fn may_enter(&self, audit_pending: bool) -> bool {
        self.consecutive_passes >= 2 && !audit_pending
    }
    pub(super) fn accepted_checkpoint(&self) -> Option<AcceptedCheckpoint> {
        self.accepted
    }
    pub(super) fn rejected_cycles(&self) -> usize {
        self.rejected_cycles
    }
    fn accept(&mut self, reporting: FinalCheckpointReporting) {
        // Only a strict witness entered after two accepted passes can mint readiness.
        debug_assert!(self.consecutive_passes >= 2);
        self.accepted = Some(AcceptedCheckpoint { reporting });
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_rejected_cycle<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        candidate: &CandidatePhase<'_, '_, P>,
        facts: &StrictFacts,
        assessment: &StrictAssessment,
        session: &CertifiedLinearSession<'_, P>,
        curve: &mut Option<ProfiledInterceptCurve>,
        previous_inner_tolerance: &mut f64,
        iteration: usize,
        attempt_tolerance: f64,
    ) -> Result<()> {
        if assessment.profile_rejected {
            // An optional nonlinear slope trial cannot contest a rejected canonical checkpoint.
            if session
                .fe_design()
                .effects()
                .iter()
                .any(|e| !e.slopes().is_empty())
            {
                *curve = None;
            }
            let refined = refined_checkpoint_tolerance(self.tolerance);
            self.pending = if refined < self.tolerance {
                PendingCheckpointWork::Refinement
            } else {
                PendingCheckpointWork::None
            };
            if self.pending == PendingCheckpointWork::Refinement
                && crate::diagnostics::solver_trace_enabled()
            {
                let checkpoint_tolerance = self.tolerance;
                crate::diagnostics::emit_solver_trace(&format!(
                    "FEREG_SOLVER_TRACE|phase=checkpoint_profile_refinement|previous={checkpoint_tolerance:.17e}|next={refined:.17e}",
                ));
            }
            self.tolerance = refined;
            *previous_inner_tolerance = previous_inner_tolerance.min(refined);
        }
        // Eligibility uses the tolerance captured before this cycle's refinement.
        let action = self.recovery.observe(
            assessment.profile_rejected
                && facts.basis_unchanged
                && facts.rank == candidate.finite_rank
                && refined_checkpoint_tolerance(attempt_tolerance) == attempt_tolerance,
            facts.basis_unchanged && facts.rank == candidate.finite_rank,
            assessment.difference,
        );
        if crate::diagnostics::solver_trace_enabled() && action != ProfileRecoveryAction::Continue {
            let recovery_action = action;
            let profile_difference = assessment.difference;
            let checkpoint_tolerance = self.tolerance;
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=profile_recovery|iteration={iteration}|action={recovery_action:?}|difference={profile_difference:.17e}|tolerance={checkpoint_tolerance:.17e}",
            ));
        }
        match action {
            ProfileRecoveryAction::Continue => {}
            ProfileRecoveryAction::Rebuild => self.pending = PendingCheckpointWork::Rebuild,
            ProfileRecoveryAction::Exhausted => {
                let profile_difference = assessment.difference;
                let checkpoint_tolerance = self.tolerance;
                return Err(FeregError::new(ErrorCode::NonConvergence, format!(
                    "prepared PPML strict profile stagnated after canonical rebuild and two further strict attempts; iteration={iteration}, difference={profile_difference:.16e}, inner_tolerance={checkpoint_tolerance:.3e}, score={:.3e}",
                    candidate.final_score.relative_infinity_norm,
                )));
            }
        }
        // Count every nonterminal strict cycle, including an ordinary score rejection.
        self.rejected_cycles = self
            .rejected_cycles
            .checked_add(1)
            .ok_or_else(|| internal("prepared PPML strict-profile rejection count overflowed"))?;
        self.consecutive_passes = 0;
        Ok(())
    }
}

struct StrictFacts {
    basis_unchanged: bool,
    certified: bool,
    rank: usize,
    active_columns: Option<Vec<usize>>,
}
struct StrictAssessment {
    objective: Option<PoissonObjective>,
    consistent: bool,
    difference: f64,
    profile_rejected: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum CandidateResolution {
    Rollback,
    Committed,
    ReplaceBasis,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StrictCycleOutcome {
    JointAccepted,
    CheckpointAccepted,
    Resume,
}

pub(super) struct CandidatePhase<'a, 'prepared, P: PreparedOlsFitView + Sync + ?Sized> {
    pub(super) prepared: &'prepared P,
    pub(super) workspace: &'a mut PpmlIterationWorkspace,
    pub(super) inputs: &'a PreparedPoissonInputs<'a>,
    pub(super) outcome: &'a [f64],
    pub(super) prior_weights: &'a [f64],
    pub(super) offset: &'a [f64],
    pub(super) config: PreparedPpmlIrlsConfig,
    pub(super) finite_rank: usize,
    pub(super) objective: &'a mut PoissonObjective,
    pub(super) total_inner_iterations: &'a mut usize,
    pub(super) total_step_halvings: &'a mut usize,
    pub(super) score_workspace: &'a mut PpmlScoreWorkspace,
    pub(super) final_score: &'a mut PpmlCertificate,
}
impl<P: PreparedOlsFitView + Sync + ?Sized> CandidatePhase<'_, '_, P> {
    fn inspect(
        &mut self,
        transaction: &StrictLinearCandidateTransaction<'_, '_, P>,
        specialized: Option<PpmlSolverEngine>,
        refinement: bool,
        solver_report: &mut Option<PpmlSolverReport>,
    ) -> Result<StrictFacts> {
        let basis_unchanged = transaction.basis_unchanged();
        #[cfg(test)]
        let basis_unchanged = basis_unchanged && !take_strict_basis_change(1);
        let strict = transaction.candidate();
        let certified = strict.certified();
        let rank = strict.frame().finite_rank();
        let active_columns = if !basis_unchanged && specialized.is_some() {
            Some(fallible_copy(
                strict.basis().kept_generated_columns(),
                "unable to reserve PPML strict active-column identity",
            )?)
        } else {
            None
        };
        accumulate_inner_iterations(
            self.total_inner_iterations,
            strict.frame().absorption_report(),
        )?;
        record_checkpoint_solver_observation(
            solver_report
                .as_mut()
                .ok_or_else(|| internal("PPML strict solve lacks solver diagnostics"))?,
            strict.frame().absorption_report(),
            refinement,
        )?;
        fill_candidate_eta_from_linear_output(
            &strict,
            self.offset,
            self.workspace.candidate_eta_mut(),
        )?;
        fill_candidate_coefficients_from_linear_output(
            &strict,
            self.workspace.candidate_coefficients_mut(),
        )?;
        Ok(StrictFacts {
            basis_unchanged,
            certified,
            rank,
            active_columns,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn assess(
        &mut self,
        transaction: &StrictLinearCandidateTransaction<'_, '_, P>,
        facts: &StrictFacts,
        direction: &OrdinaryDirection,
        curve: &Option<ProfiledInterceptCurve>,
        control: &mut StrictCheckpointControl,
        iteration: usize,
        clock: &PpmlPhaseClock,
    ) -> Result<StrictAssessment> {
        #[cfg(test)]
        let injected_strict_movement = take_strict_resume(1);
        #[cfg(test)]
        if injected_strict_movement {
            inject_candidate_profile_movement(self.workspace);
        }
        let objective = match self
            .workspace
            .evaluate_candidate_objective(self.inputs, self.config.ppml.bounds)
        {
            Ok(objective) => Some(objective),
            Err(error) if error.code == ErrorCode::NonConvergence => None,
            Err(error) => return Err(error),
        };
        let slots = direction
            .active_slots()
            .or_else(|| curve.as_ref().map(ProfiledInterceptCurve::active_slots));
        let consistent = strict_candidate_decision_with_coefficient_slots(
            facts.certified,
            facts.basis_unchanged,
            self.finite_rank,
            facts.rank,
            self.workspace.eta(),
            self.workspace.coefficients(),
            *self.objective,
            self.workspace.candidate_eta(),
            self.workspace.candidate_coefficients(),
            objective,
            self.config.ppml,
            slots,
        ) == FinalHessianSynchronization::Certified;
        self.trace_assessment(transaction, facts, objective, consistent, iteration, clock);
        if control.canonical_products_allowed && !consistent {
            control.canonical_products_allowed = false;
        }
        let difference = if consistent {
            0.0
        } else {
            objective.map_or(f64::INFINITY, |candidate| {
                strict_profile_difference(
                    self.workspace.eta(),
                    self.workspace.coefficients(),
                    *self.objective,
                    self.workspace.candidate_eta(),
                    self.workspace.candidate_coefficients(),
                    candidate,
                    slots,
                )
            })
        };
        let profile_rejected = facts.certified
            && facts.basis_unchanged
            && facts.rank == self.finite_rank
            && !consistent;
        // A rebuild requests one refinement observation; consume it after assessment and trace.
        control.pending = PendingCheckpointWork::None;
        Ok(StrictAssessment {
            objective,
            consistent,
            difference,
            profile_rejected,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn trace_assessment(
        &self,
        transaction: &StrictLinearCandidateTransaction<'_, '_, P>,
        facts: &StrictFacts,
        objective: Option<PoissonObjective>,
        consistent: bool,
        iteration: usize,
        clock: &PpmlPhaseClock,
    ) {
        if !crate::diagnostics::solver_trace_enabled() {
            return;
        }
        let mut worst_row = 0_usize;
        let mut worst_difference = 0.0_f64;
        let mut failing_rows = 0_usize;
        let tolerance = self
            .config
            .ppml
            .tolerance
            .min(self.config.ppml.score_tolerance)
            .min(1e-8);
        for (row, (&accepted, &candidate)) in self
            .workspace
            .eta()
            .iter()
            .zip(self.workspace.candidate_eta())
            .enumerate()
        {
            let difference = relative_scalar_difference(accepted, candidate);
            failing_rows += usize::from(difference > tolerance);
            if difference > worst_difference {
                worst_row = row;
                worst_difference = difference;
            }
        }
        let strict = transaction.candidate();
        let row = worst_row;
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=strict_worst_row|iteration={iteration}|seconds={:.6}|original_row={}|failing_rows={failing_rows}|tolerance={tolerance:.17e}|accepted_eta={:.17e}|candidate_eta={:.17e}|working_response={:.17e}|residual={:.17e}|finite_predictor={:.17e}|absorbed_predictor={:.17e}|outcome={:.17e}|mean={:.17e}|hessian_weight={:.17e}|deviance_change={:.17e}|ll_change={:.17e}",
            clock.elapsed().as_secs_f64(), self.prepared.fit_sample().retained_rows()[row] + 1,
            self.workspace.eta()[row], self.workspace.candidate_eta()[row], strict.response()[row],
            strict.frame().residuals()[row], strict.finite_predictor()[row], strict.absorbed_predictor()[row],
            self.outcome[row], self.workspace.mean()[row], self.workspace.hessian_weights()[row],
            objective.map_or(f64::INFINITY, |candidate| relative_scalar_difference(self.objective.deviance, candidate.deviance)),
            objective.map_or(f64::INFINITY, |candidate| relative_scalar_difference(self.objective.log_likelihood, candidate.log_likelihood)),
        ));
        let strict_rank = facts.rank;
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=strict_decision|iteration={iteration}|certified={}|basis_unchanged={}|rank={strict_rank}|eta_change={:.17e}|beta_change={:.17e}|consistent={}",
            u8::from(facts.certified), u8::from(facts.basis_unchanged),
            relative_infinity_difference(self.workspace.eta(), self.workspace.candidate_eta()),
            relative_infinity_difference(self.workspace.coefficients(), self.workspace.candidate_coefficients()), u8::from(consistent),
        ));
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve(
        &mut self,
        transaction: StrictLinearCandidateTransaction<'_, '_, P>,
        facts: &StrictFacts,
        assessment: &StrictAssessment,
        canonical_allowed: bool,
        profile_present: bool,
        curve_present: bool,
        interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    ) -> Result<CandidateResolution> {
        if assessment.consistent {
            if !canonical_allowed && !profile_present && !curve_present {
                self.workspace.fill_prepared_line_search_trial_with(
                    1.0,
                    self.config.ppml.bounds,
                    self.inputs,
                    |_| Ok(()),
                )?;
                self.workspace.accept_trial()?;
                *self.objective = assessment
                    .objective
                    .expect("consistent strict objective is present");
            }
            transaction.commit();
            return Ok(CandidateResolution::Committed);
        }
        let step = if facts.certified && assessment.objective.is_some() {
            let mut poll = || interrupt(PpmlInterruptPoint::IrlsLineSearchTrial);
            self.workspace.accept_candidate_interruptible(
                self.inputs,
                *self.objective,
                self.config.ppml,
                &mut poll,
            )
        } else {
            Err(numerical_nonconvergence(
                "prepared PPML strict final weighted-linear solve did not certify",
            ))
        };
        match step {
            Ok(step) => {
                // This transaction checks the count before changing the accepted objective.
                let halvings = self
                    .total_step_halvings
                    .checked_add(step.step_halvings)
                    .ok_or_else(|| {
                        internal("prepared PPML strict step-halving count overflowed")
                    })?;
                *self.objective = step.objective;
                *self.total_step_halvings = halvings;
                transaction.commit();
                Ok(if facts.active_columns.is_some() {
                    CandidateResolution::ReplaceBasis
                } else {
                    CandidateResolution::Committed
                })
            }
            Err(error) if error.code == ErrorCode::NonConvergence => {
                transaction.rollback();
                Ok(CandidateResolution::Rollback)
            }
            Err(error) => Err(error),
        }
    }

    fn refresh_score(&mut self, design: &FeDesign) -> Result<()> {
        *self.final_score = prepared_score_certificate(
            self.prepared,
            design,
            self.outcome,
            self.workspace.mean(),
            self.prior_weights,
            self.config.ppml.score_tolerance,
            self.score_workspace,
        )?;
        Ok(())
    }
    fn resume_hessian_candidate(
        &mut self,
        assessment: &mut StrictAssessment,
        design: &FeDesign,
        interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    ) -> Result<()> {
        assessment.profile_rejected = true;
        let objective = self
            .workspace
            .evaluate_candidate_objective(self.inputs, self.config.ppml.bounds)?;
        assessment.difference = strict_profile_difference(
            self.workspace.eta(),
            self.workspace.coefficients(),
            *self.objective,
            self.workspace.candidate_eta(),
            self.workspace.candidate_coefficients(),
            objective,
            None,
        );
        let mut poll = || interrupt(PpmlInterruptPoint::IrlsLineSearchTrial);
        match self.workspace.accept_candidate_interruptible(
            self.inputs,
            *self.objective,
            self.config.ppml,
            &mut poll,
        ) {
            Ok(step) => {
                // Hessian resume historically updates the objective before the count check.
                *self.objective = step.objective;
                *self.total_step_halvings = self
                    .total_step_halvings
                    .checked_add(step.step_halvings)
                    .ok_or_else(|| {
                        internal("prepared PPML Hessian-resume step-halving count overflowed")
                    })?;
            }
            Err(error) if error.code == ErrorCode::NonConvergence => {}
            Err(error) => return Err(error),
        }
        self.refresh_score(design)
    }
}

pub(super) struct StrictCycle<'a, 'prepared, P: PreparedOlsFitView + Sync + ?Sized> {
    pub(super) candidate: CandidatePhase<'a, 'prepared, P>,
    pub(super) session: &'a mut CertifiedLinearSession<'prepared, P>,
    pub(super) direction: &'a mut OrdinaryDirection,
    pub(super) curve: &'a mut Option<ProfiledInterceptCurve>,
    pub(super) control: &'a mut StrictCheckpointControl,
    pub(super) memory: &'a mut PpmlMemoryTelemetry,
    pub(super) solver: &'a mut Option<PpmlSolverReport>,
    pub(super) frame_generation: &'a mut u64,
    pub(super) previous_inner_tolerance: &'a mut f64,
    pub(super) interrupt: &'a mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    pub(super) iteration: usize,
    pub(super) solver_admission: PublicSolverMemoryAdmission,
    pub(super) solver_memory_budget: FeSolverMemoryBudget,
}
impl<P: PreparedOlsFitView + Sync + ?Sized> StrictCycle<'_, '_, P> {
    fn try_joint(&mut self) -> Result<bool> {
        if self.control.canonical_products_allowed
            || self.curve.is_some()
            || self.control.pending != PendingCheckpointWork::None
            || !(self.session.supports_joint_ppml_direction() || self.direction.lsmr().is_some())
        {
            return Ok(false);
        }
        try_joint_strict_final_frame(
            self.session,
            self.candidate.workspace,
            self.candidate.inputs,
            self.candidate.offset,
            self.candidate.config,
            self.control.tolerance,
            self.frame_generation,
            self.candidate.total_inner_iterations,
            self.candidate.finite_rank,
            *self.candidate.objective,
            self.solver
                .as_mut()
                .ok_or_else(|| internal("joint strict solve lacks diagnostics"))?,
            self.direction.lsmr_mut(),
        )
    }
    fn prepare_attempt(&mut self) -> Result<(f64, Option<PpmlSolverEngine>)> {
        // Strict construction starts from accepted eta, rather than the stale ordinary frame.
        self.candidate.workspace.refresh_prepared_working_state(
            self.candidate.inputs,
            self.candidate.config.ppml.bounds,
        )?;
        *self.frame_generation = next_frame_generation(*self.frame_generation)?;
        refresh_linear_frame(
            self.session,
            self.candidate.workspace,
            *self.frame_generation,
        )?;
        let tolerance = self.control.tolerance;
        *self.previous_inner_tolerance = self.previous_inner_tolerance.min(tolerance);
        self.session
            .set_config(ppml_linear_config(self.candidate.config, tolerance)?)?;
        let engine = self.direction.specialized_engine();
        if self.control.pending == PendingCheckpointWork::Rebuild {
            let mut poll = || (self.interrupt)(PpmlInterruptPoint::StrictFinalSolve);
            self.session
                .rebuild_retained_inputs_interruptible(&mut poll)?;
            self.memory
                .record_fallback(PpmlFallbackKind::WarmProjection);
            self.control.pending = PendingCheckpointWork::Refinement;
        }
        Ok((tolerance, engine))
    }
    fn apply_commit(
        &mut self,
        facts: &StrictFacts,
        resolution: CandidateResolution,
        engine: Option<PpmlSolverEngine>,
    ) -> Result<()> {
        if resolution != CandidateResolution::Rollback {
            if !facts.basis_unchanged {
                *self.curve = None;
            }
            self.direction.after_commit(facts.basis_unchanged);
        }
        if resolution == CandidateResolution::ReplaceBasis {
            let columns = facts.active_columns.as_deref().ok_or_else(|| {
                internal("accepted PPML strict basis change lacks active-column identity")
            })?;
            self.direction.replace_basis(
                engine,
                self.candidate.prepared,
                columns,
                self.session.fe_design(),
                self.candidate.config,
                self.solver_admission,
                self.solver_memory_budget,
            )?;
            if let Some(cmg) = self.direction.cmg() {
                record_cmg_heap_forecast(self.memory, cmg, self.solver_admission)?;
            }
        }
        Ok(())
    }
    fn synchronize(
        &mut self,
        facts: &StrictFacts,
        assessment: &mut StrictAssessment,
    ) -> Result<()> {
        (self.interrupt)(PpmlInterruptPoint::StrictFinalSolve)?;
        let (synchronized, reporting) = if (self.control.canonical_products_allowed
            || self.direction.profile().is_some()
            || self.curve.is_some())
            && facts.basis_unchanged
            && reuse_canonical_final_products(
                self.session,
                self.candidate.workspace,
                self.candidate.prior_weights,
                self.candidate.offset,
                self.candidate.config,
                *self.frame_generation,
            )? {
            (
                FinalHessianSynchronization::Certified,
                FinalCheckpointReporting::AlreadyObserved,
            )
        } else {
            (
                synchronize_final_hessian_frame(
                    self.session,
                    self.candidate.workspace,
                    self.candidate.inputs,
                    self.candidate.offset,
                    self.candidate.config,
                    self.control.tolerance,
                    self.frame_generation,
                    self.candidate.total_inner_iterations,
                    self.candidate.finite_rank,
                    *self.candidate.objective,
                    self.direction.profile().map(ProfiledOneFe::active_slots),
                )?,
                FinalCheckpointReporting::ObserveAtPublication,
            )
        };
        if synchronized == FinalHessianSynchronization::Certified {
            self.control.accept(reporting);
        } else if synchronized == FinalHessianSynchronization::ResumeWithCandidate {
            self.candidate.resume_hessian_candidate(
                assessment,
                self.session.fe_design(),
                self.interrupt,
            )?;
        }
        Ok(())
    }
    fn finish_phase(&mut self, clock: PpmlPhaseClock) {
        self.memory
            .record_phase(PpmlPhase::StrictFinalSolve, clock.elapsed());
        record_phase_allocation(
            self.memory,
            self.candidate.config.phase_allocation_probe,
            PpmlPhase::StrictFinalSolve,
        );
    }

    pub(super) fn run(mut self) -> Result<StrictCycleOutcome> {
        (self.interrupt)(PpmlInterruptPoint::StrictFinalSolve)?;
        begin_phase_allocation(
            self.candidate.config.phase_allocation_probe,
            PpmlPhase::StrictFinalSolve,
        );
        let clock = PpmlPhaseClock::start();
        if self.try_joint()? {
            self.control
                .accept(FinalCheckpointReporting::ObserveAtPublication);
            self.finish_phase(clock);
            return Ok(StrictCycleOutcome::JointAccepted);
        }
        let (attempt_tolerance, engine) = self.prepare_attempt()?;
        let transaction = self.session.strict_candidate()?;
        let facts = self.candidate.inspect(
            &transaction,
            engine,
            self.control.pending == PendingCheckpointWork::Refinement,
            self.solver,
        )?;
        let mut assessment = self.candidate.assess(
            &transaction,
            &facts,
            self.direction,
            self.curve,
            self.control,
            self.iteration,
            &clock,
        )?;
        let resolution = self.candidate.resolve(
            transaction,
            &facts,
            &assessment,
            self.control.canonical_products_allowed,
            self.direction.profile().is_some(),
            self.curve.is_some(),
            self.interrupt,
        )?;
        self.apply_commit(&facts, resolution, engine)?;
        self.candidate.refresh_score(self.session.fe_design())?;
        if assessment.consistent && self.candidate.final_score.passed {
            self.synchronize(&facts, &mut assessment)?;
        }
        if crate::diagnostics::solver_trace_enabled() {
            let iteration = self.iteration;
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=strict_result|iteration={iteration}|committed={}|score={:.17e}|score_passed={}|converged={}",
                u8::from(resolution != CandidateResolution::Rollback), self.candidate.final_score.relative_infinity_norm,
                u8::from(self.candidate.final_score.passed), u8::from(self.control.accepted.is_some()),
            ));
        }
        self.finish_phase(clock);
        if self.control.accepted.is_some() {
            return Ok(StrictCycleOutcome::CheckpointAccepted);
        }
        self.control.finish_rejected_cycle(
            &self.candidate,
            &facts,
            &assessment,
            self.session,
            self.curve,
            self.previous_inner_tolerance,
            self.iteration,
            attempt_tolerance,
        )?;
        Ok(StrictCycleOutcome::Resume)
    }
}
