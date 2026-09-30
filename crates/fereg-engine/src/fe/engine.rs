//! One immutable-weight frame and one bounded, independently certified dispatch path.
use super::{
    diagnostics, Attempt, Backend, CmgSession, Design, FallbackReason, LsmrSession, MonitorControl,
    Plan, PlanOverride, Projection, RhsDiagnostics, SchurSession, SetupDiagnostics, StopReason,
};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    spec::{Estimator, Options},
    threads::ThreadTeam,
};

/// Factors borrow this exact sample and weight frame. Reports survive an error
/// so exhausted or failed work remains observable without publishing a result.
pub struct Engine<'a, 'data> {
    design: &'a Design<'data>,
    plan: Plan,
    monitor: MonitorControl,
    schur: Option<SchurSession<'a, 'data>>,
    lsmr: Option<LsmrSession<'a, 'data>>,
    cmg: Option<CmgSession<'a, 'data>>,
    cmg_setup: Option<StopReason>,
    cmg_setup_error: Option<FeregError>,
    last_diagnostics: Vec<RhsDiagnostics>,
    last_setup: Vec<SetupDiagnostics>,
}
impl<'a, 'data> Engine<'a, 'data> {
    pub fn new(
        design: &'a Design<'data>,
        estimator: Estimator,
        options: &Options,
        team: &ThreadTeam,
    ) -> Result<Self> {
        Self::with_controls(
            design,
            estimator,
            options,
            PlanOverride::Automatic,
            MonitorControl::Automatic,
            team,
        )
    }
    #[doc(hidden)]
    pub fn with_controls(
        design: &'a Design<'data>,
        estimator: Estimator,
        options: &Options,
        override_plan: PlanOverride,
        monitor: MonitorControl,
        team: &ThreadTeam,
    ) -> Result<Self> {
        let plan = Plan::with_override(design, estimator, options, override_plan)?;
        if monitor == MonitorControl::StopAfter(0) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "monitor checkpoint must be positive",
            ));
        }
        let schur = if plan.selected() == Backend::SchurPcg {
            Some(SchurSession::new(design, &plan, team)?)
        } else {
            None
        };
        let lsmr = if plan.selected() == Backend::Lsmr {
            Some(LsmrSession::new(design, team)?)
        } else {
            None
        };
        Ok(Self {
            design,
            plan,
            monitor,
            schur,
            lsmr,
            cmg: None,
            cmg_setup: None,
            cmg_setup_error: None,
            last_diagnostics: reserved(0)?,
            last_setup: reserved(0)?,
        })
    }
    #[must_use]
    pub fn design(&self) -> &'a Design<'data> {
        self.design
    }
    #[must_use]
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
    #[must_use]
    pub fn last_diagnostics(&self) -> &[RhsDiagnostics] {
        &self.last_diagnostics
    }
    #[must_use]
    pub fn last_setup_diagnostics(&self) -> &[SetupDiagnostics] {
        &self.last_setup
    }
    /// Only Schur consumes coefficient seeds. Factors and certificates never transfer.
    pub fn seed_response(&mut self, coefficients: &DenseMatrix, rhs_count: usize) -> Result<()> {
        super::design::validate_matrix(coefficients, self.design.layout().total)?;
        if coefficients.ncols() != 1 || rhs_count == 0 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "FE response seed requires one coefficient RHS and a positive bank width",
            ));
        }
        if let Some(session) = &mut self.schur {
            session.seed_response(coefficients, rhs_count)?;
        }
        Ok(())
    }
    pub fn project(&mut self, rhs: &DenseMatrix, team: &ThreadTeam) -> Result<Projection> {
        super::design::validate_matrix(rhs, self.design.nobs())?;
        self.last_diagnostics = reserved(rhs.ncols())?;
        self.last_setup = reserved(1)?;
        let (mut coefficients, progress) = self.primary(rhs, team)?;
        for attempt in progress {
            let mut attempts = reserved(2)?;
            let (iterations, stop, backend) = (attempt.iterations, attempt.stop, attempt.backend);
            attempts.push(attempt);
            self.last_diagnostics.push(RhsDiagnostics {
                requested: self.plan.requested(),
                selected: self.plan.selected(),
                used: backend,
                checkpoint_backend: backend,
                attempts,
                iterations,
                stop,
                fallback: None,
            });
        }
        if let Some(mut accepted) = self.accept_bank(rhs, &coefficients, team)? {
            accepted.setup_diagnostics = diagnostics::copy_setup(&self.last_setup)?;
            return Ok(accepted);
        }
        // The primary bank has already run. Preserve every column's work even
        // if resolving an earlier column fails and aborts result publication.
        for column in 0..rhs.ncols() {
            let original = DenseMatrix::from_columns(&[rhs.column(column)?])?;
            let candidate = DenseMatrix::from_columns(&[coefficients.column(column)?])?;
            let accepted = self.resolve(column, &original, candidate, team)?;
            for row in 0..coefficients.nrows() {
                coefficients[(row, column)] = accepted[(row, 0)];
            }
        }
        let mut result = self.design.certified_projection(
            rhs,
            coefficients,
            diagnostics::copy_rhs(&self.last_diagnostics)?,
            self.plan.tolerance(),
            team,
        )?;
        result.setup_diagnostics = diagnostics::copy_setup(&self.last_setup)?;
        Ok(result)
    }
    fn accept_bank(
        &self,
        rhs: &DenseMatrix,
        coefficients: &DenseMatrix,
        team: &ThreadTeam,
    ) -> Result<Option<Projection>> {
        if self
            .last_diagnostics
            .iter()
            .any(|report| !success(report.stop))
        {
            return Ok(None);
        }
        // Gauge shifts and certificate reductions are independent per RHS.
        // Sharing their geometry across the bank avoids rebuilding it for each
        // column. Keep the original candidate until every column passes so a
        // numerical failure can still be localized through the bounded path.
        let mut values = reserved(coefficients.as_slice().len())?;
        values.extend_from_slice(coefficients.as_slice());
        let candidate =
            DenseMatrix::from_row_major(coefficients.nrows(), coefficients.ncols(), values)?;
        match self.design.finish_candidate(
            rhs,
            candidate,
            diagnostics::copy_rhs(&self.last_diagnostics)?,
            self.plan.tolerance(),
            team,
        ) {
            Ok(result) => Ok(Some(result)),
            Err(failure) if failure.code == ErrorCode::NonConvergence => Ok(None),
            Err(failure) => Err(failure),
        }
    }
    fn primary(
        &mut self,
        rhs: &DenseMatrix,
        team: &ThreadTeam,
    ) -> Result<(DenseMatrix, Vec<Attempt>)> {
        if rhs.ncols() == 0 {
            return Ok((
                DenseMatrix::zeros(self.design.layout().total, 0)?,
                reserved(0)?,
            ));
        }
        match self.plan.selected() {
            Backend::Lsmr => {
                let candidate = self
                    .lsmr
                    .as_ref()
                    .expect("explicit LSMR session")
                    .solve_refined(rhs, self.plan.linear_budget(), self.plan.tolerance(), team)?;
                Ok((candidate.coefficients, candidate.progress))
            }
            Backend::SchurPcg => {
                let candidate = self.schur.as_mut().expect("Schur session").solve_refined(
                    rhs,
                    self.monitor,
                    team,
                );
                match candidate {
                    Ok(candidate) => Ok((candidate.coefficients, candidate.progress)),
                    // Schur retains progress for every failure after iteration
                    // starts. A numerical Err here is initial-frame preparation.
                    Err(failure) if failure.code == ErrorCode::NonConvergence => Ok((
                        DenseMatrix::zeros(self.design.layout().total, rhs.ncols())?,
                        initial_progress(
                            rhs.ncols(),
                            Backend::SchurPcg,
                            StopReason::NumericalBreakdown,
                        )?,
                    )),
                    Err(failure) => Err(failure),
                }
            }
            Backend::Cmg if self.plan.cmg_core().is_some() => {
                if self.prepare_cmg(team)? != StopReason::Converged {
                    return Ok((
                        DenseMatrix::zeros(self.design.layout().total, rhs.ncols())?,
                        initial_progress(
                            rhs.ncols(),
                            Backend::Cmg,
                            StopReason::NumericalBreakdown,
                        )?,
                    ));
                }
                let candidate = self.cmg.as_mut().expect("prepared CMG").solve_refined(
                    rhs,
                    self.plan.linear_budget(),
                    self.plan.tolerance(),
                    team,
                )?;
                Ok((candidate.coefficients, candidate.progress))
            }
            _ => {
                let candidate = self.design.direct_candidate(rhs, team)?;
                let backend = if self.design.layout().effects.is_empty() {
                    Backend::NoEffects
                } else {
                    Backend::Direct
                };
                let stop = if backend == Backend::NoEffects {
                    StopReason::NoEffects
                } else {
                    StopReason::Direct
                };
                let progress = initial_progress(rhs.ncols(), backend, stop)?;
                Ok((candidate.coefficients, progress))
            }
        }
    }
    fn prepare_cmg(&mut self, team: &ThreadTeam) -> Result<StopReason> {
        if let Some(failure) = &self.cmg_setup_error {
            return Err(failure.clone());
        }
        if let Some(stop) = self.cmg_setup {
            return Ok(stop);
        }
        let mut session =
            match CmgSession::new(self.design, self.plan.cmg_core().expect("CMG pair")) {
                Ok(session) => session,
                Err(failure) if failure.code == ErrorCode::NonConvergence => {
                    self.cmg_setup = Some(StopReason::NumericalBreakdown);
                    self.last_setup.push(SetupDiagnostics {
                        backend: Backend::Cmg,
                        right_hand_sides: reserved(0)?,
                        stop: StopReason::NumericalBreakdown,
                    });
                    return Ok(StopReason::NumericalBreakdown);
                }
                Err(failure) => {
                    self.cmg_setup_error = Some(failure.clone());
                    return Err(failure);
                }
            };
        let outcome = session.prepare(self.plan.linear_budget(), self.plan.tolerance(), team);
        let mut right_hand_sides = reserved(session.setup_progress().len())?;
        right_hand_sides.extend_from_slice(session.setup_progress());
        let stop = if outcome.is_ok() {
            StopReason::Converged
        } else {
            right_hand_sides
                .iter()
                .find(|p| p.stop != StopReason::Converged)
                .map_or(StopReason::NumericalBreakdown, |p| p.stop)
        };
        self.last_setup.push(SetupDiagnostics {
            backend: Backend::Cmg,
            right_hand_sides,
            stop,
        });
        self.cmg_setup = Some(stop);
        self.cmg = Some(session);
        match outcome {
            Err(failure) if failure.code != ErrorCode::NonConvergence => {
                self.cmg_setup_error = Some(failure.clone());
                Err(failure)
            }
            _ => Ok(stop),
        }
    }
    fn resolve(
        &mut self,
        index: usize,
        original: &DenseMatrix,
        mut coefficients: DenseMatrix,
        team: &ThreadTeam,
    ) -> Result<DenseMatrix> {
        if success(self.last_diagnostics[index].stop)
            && !self.certifies(original, &mut coefficients, team)?
        {
            self.last_diagnostics[index].stop = StopReason::CertificateFailure;
            self.last_diagnostics[index].attempts[0].stop = StopReason::CertificateFailure;
        }
        let report = &self.last_diagnostics[index];
        let (iterations, stop, backend) = (report.iterations, report.stop, report.used);
        if success(stop) {
            return Ok(coefficients);
        }
        let remaining = self
            .plan
            .linear_budget()
            .checked_sub(iterations)
            .ok_or_else(|| {
                error(
                    ErrorCode::Internal,
                    "backend exceeded the linear iteration budget",
                )
            })?;
        if self.plan.fallback().is_none() || remaining == 0 {
            return Err(error(ErrorCode::NonConvergence, &format!("{} stopped with {stop:?} after {iterations} iterations; no permitted remaining attempt", backend.label())));
        }
        self.last_diagnostics[index].fallback = Some(fallback_reason(stop));
        if self.lsmr.is_none() {
            self.lsmr = Some(LsmrSession::new(self.design, team)?);
        }
        let mut fallback = self
            .lsmr
            .as_ref()
            .expect("fallback LSMR session")
            .solve_refined(original, remaining, self.plan.tolerance(), team)?;
        let mut work = fallback.progress.remove(0);
        if success(work.stop) && !self.certifies(original, &mut fallback.coefficients, team)? {
            work.stop = StopReason::CertificateFailure;
        }
        let report = &mut self.last_diagnostics[index];
        report.iterations += work.iterations;
        report.stop = work.stop;
        report.used = Backend::Lsmr;
        report.checkpoint_backend = Backend::Lsmr;
        report.attempts.push(work);
        if !success(report.stop) {
            return Err(error(
                ErrorCode::NonConvergence,
                &format!(
                    "LSMR fallback stopped with {:?} for RHS {index} after {} total iterations",
                    report.stop, report.iterations
                ),
            ));
        }
        Ok(fallback.coefficients)
    }
    fn certifies(
        &self,
        rhs: &DenseMatrix,
        coefficients: &mut DenseMatrix,
        team: &ThreadTeam,
    ) -> Result<bool> {
        let result = (|| {
            self.design
                .normalize_coefficients(coefficients, self.plan.tolerance(), team)?;
            let residuals = self.design.reconstruct(rhs, coefficients, team)?;
            self.design
                .certify(rhs, coefficients, &residuals, self.plan.tolerance(), team)
        })();
        match result {
            Ok(_) => Ok(true),
            Err(failure) if failure.code == ErrorCode::NonConvergence => Ok(false),
            Err(failure) => Err(failure),
        }
    }
}
fn success(stop: StopReason) -> bool {
    matches!(
        stop,
        StopReason::Converged | StopReason::NoEffects | StopReason::Direct
    )
}
fn initial_progress(count: usize, backend: Backend, stop: StopReason) -> Result<Vec<Attempt>> {
    let mut progress = reserved(count)?;
    progress.resize(
        count,
        Attempt {
            backend,
            stop,
            iterations: 0,
            refinements: 0,
        },
    );
    Ok(progress)
}
fn fallback_reason(stop: StopReason) -> FallbackReason {
    match stop {
        StopReason::Stagnation => FallbackReason::MeasuredStagnation,
        StopReason::ProjectedExhaustion | StopReason::BudgetExhausted => {
            FallbackReason::ProjectedExhaustion
        }
        StopReason::CertificateFailure => FallbackReason::CertificateFailure,
        _ => FallbackReason::NumericalNonConvergence,
    }
}
fn error(code: ErrorCode, message: &str) -> FeregError {
    FeregError::new(code, message)
}
