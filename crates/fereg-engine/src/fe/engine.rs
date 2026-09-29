//! One immutable-weight projection frame; every result crosses the common certificate.
use super::{
    Attempt, Backend, Design, LsmrSession, Plan, Projection, RhsDiagnostics, SchurSession,
    StopReason,
};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    spec::{Estimator, Options},
    threads::ThreadTeam,
};

/// Numerical factors borrow exactly one design/weight frame. Recreate the
/// engine when weights or sample change; only coefficient guesses may transfer.
pub struct Engine<'a, 'data> {
    design: &'a Design<'data>,
    plan: Plan,
    schur: Option<SchurSession<'a, 'data>>,
    lsmr: Option<LsmrSession<'a, 'data>>,
}
impl<'a, 'data> Engine<'a, 'data> {
    pub fn new(
        design: &'a Design<'data>,
        estimator: Estimator,
        options: &Options,
        team: &ThreadTeam,
    ) -> Result<Self> {
        let plan = Plan::new(design, estimator, options)?;
        // An incomplete adapter cannot silently substitute another solver.
        if plan.selected() == Backend::Cmg {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "CMG is pending new-engine adapter qualification (M3.5)",
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
            schur,
            lsmr,
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
    /// Optional first-RHS guess. Schur uses it; direct and cold LSMR solves
    /// ignore valid guesses. Every backend certifies under this frame's weights.
    pub fn seed_response(&mut self, coefficients: &DenseMatrix, rhs_count: usize) -> Result<()> {
        super::design::validate_matrix(coefficients, self.design.layout().total)?;
        if coefficients.ncols() != 1 || rhs_count == 0 {
            return Err(FeregError::new(
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
        let (coefficients, progress) = if let Some(session) = &self.lsmr {
            let candidate = session.solve_refined(
                rhs,
                self.plan.linear_budget(),
                self.plan.tolerance(),
                team,
            )?;
            if let Some(progress) = candidate
                .progress
                .iter()
                .find(|p| !matches!(p.stop, StopReason::Converged | StopReason::NoEffects))
            {
                return Err(FeregError::new(ErrorCode::NonConvergence, format!(
                    "explicit LSMR stopped with {:?} after {} iterations and {} residual corrections",
                    progress.stop, progress.iterations, progress.refinements,
                )));
            }
            (candidate.coefficients, candidate.progress)
        } else {
            match self.schur.as_mut() {
                Some(session) => {
                    let candidate = session.solve(rhs, team)?;
                    if candidate
                        .progress
                        .iter()
                        .any(|p| p.stop != StopReason::Converged)
                    {
                        return Err(FeregError::new(ErrorCode::NonConvergence,
                        "Schur projection did not converge; monitored LSMR fallback is pending integration"));
                    }
                    (candidate.coefficients, candidate.progress)
                }
                None => {
                    let candidate = self.design.direct_candidate(rhs, team)?;
                    let mut progress = reserved(rhs.ncols())?;
                    for _ in 0..rhs.ncols() {
                        progress.push(Attempt {
                            refinements: 0,
                            backend: self.plan.selected(),
                            iterations: 0,
                            stop: if self.plan.selected() == Backend::NoEffects {
                                StopReason::NoEffects
                            } else {
                                StopReason::Direct
                            },
                        });
                    }
                    (candidate.coefficients, progress)
                }
            }
        };
        let mut diagnostics = reserved(progress.len())?;
        for attempt in progress {
            let mut attempts = reserved(1)?;
            let iterations = attempt.iterations;
            let stop = attempt.stop;
            attempts.push(attempt);
            diagnostics.push(RhsDiagnostics {
                requested: self.plan.requested(),
                selected: self.plan.selected(),
                used: self.plan.selected(),
                checkpoint_backend: self.plan.selected(),
                attempts,
                iterations,
                stop,
                fallback: None,
            });
        }
        self.design
            .finish_candidate(rhs, coefficients, diagnostics, self.plan.tolerance(), team)
    }
}
