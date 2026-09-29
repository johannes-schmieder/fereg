//! One immutable-weight projection frame; every result crosses the common certificate.
use super::{Attempt, Backend, Design, Plan, Projection, RhsDiagnostics, SchurSession, StopReason};
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
}
impl<'a, 'data> Engine<'a, 'data> {
    pub fn new(
        design: &'a Design<'data>,
        estimator: Estimator,
        options: &Options,
        team: &ThreadTeam,
    ) -> Result<Self> {
        let plan = Plan::new(design, estimator, options)?;
        // M3.5's dependency/determinism qualification is a real boundary;
        // incomplete adapters must never silently substitute another solver.
        if matches!(plan.selected(), Backend::Lsmr | Backend::Cmg) {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "retained FE backend is pending new-engine qualification (M3.5)",
            ));
        }
        let schur = if plan.selected() == Backend::SchurPcg {
            Some(SchurSession::new(design, &plan, team)?)
        } else {
            None
        };
        Ok(Self {
            design,
            plan,
            schur,
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
    /// A first-RHS guess only. Direct solves ignore valid guesses; iterative
    /// backends must reconstruct and certify under this engine's fresh weights.
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
        let (coefficients, progress) = match self.schur.as_mut() {
            Some(session) => {
                let candidate = session.solve(rhs, team)?;
                if candidate
                    .progress
                    .iter()
                    .any(|p| p.stop != StopReason::Converged)
                {
                    return Err(FeregError::new(ErrorCode::NonConvergence,
                        "Schur projection did not converge; retained LSMR fallback is pending qualification"));
                }
                (candidate.coefficients, candidate.progress)
            }
            None => {
                let candidate = self.design.direct_candidate(rhs, team)?;
                let mut progress = reserved(rhs.ncols())?;
                for _ in 0..rhs.ncols() {
                    progress.push(Attempt {
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
