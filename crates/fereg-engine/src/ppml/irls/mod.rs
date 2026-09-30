//! Shared bounded IRLS for final publication and fitted-mean separation exploration.
mod exploration;
mod frame;
mod trajectory;
pub(crate) use exploration::{explore_mu, MeanThreshold};
pub use exploration::{MuCertificate, MuStop};
use trajectory::{Purpose, Trajectory};
pub(crate) mod input;
mod start;
use super::{
    checkpoint::{Checkpoint, Decision as CheckDecision, Stage},
    control::{Controls, Convergence, Decision},
    family::{Evaluation, Family, ScoreCertificate, StateCertificate},
    step::{Accepted, Point},
};
use crate::{
    data::{DenseMatrix, Prepared, ReferenceWeights, Weights},
    error::{ErrorCode, FeregError, Result},
    fe::Design,
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls::{AbsorbedFit, FrozenBasis, Role, Work},
};
/// No sample mutation or cached numerical state crosses this call. Raw offsets
/// and exposures are read from the sole prepared sample owner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Problem<'a> {
    pub sample: &'a Prepared,
    pub raw: &'a DenseMatrix,
    pub roles: &'a [Role],
}
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Diagnostics {
    pub accepted_iterations: usize,
    pub step_halvings: usize,
    pub linear_frames: usize,
    pub strict_checks: usize,
    pub fisher_checks: usize,
    pub hessian_checks: usize,
    pub basis_rebases: usize,
    pub strict_resumes: usize,
    pub analytic_start: bool,
    pub work: Work,
}
#[derive(Debug, PartialEq)]
pub struct FittedState {
    pub point: Point,
    pub evaluation: Evaluation,
    pub hessian_weights: Vec<f64>,
    pub linear: AbsorbedFit,
    pub scores: ScoreCertificate,
    pub state: StateCertificate,
    pub diagnostics: Diagnostics,
}
/// The accepted state borrows the exact immutable sample, raw columns and
/// role map which passed strict final-Hessian validation. Read access cannot
/// mutate coefficients, means or bread and retain their publication authority.
///
/// ```compile_fail
/// fn change_mean(fit: &mut fereg_engine::ppml::irls::Fit<'_>) {
///     fit.evaluation.mean[0] = 0.;
/// }
/// ```
#[derive(Debug, PartialEq)]
pub struct Fit<'data> {
    problem: Problem<'data>,
    options: Options,
    accepted: FittedState,
}
impl<'data> Fit<'data> {
    #[must_use]
    pub const fn problem(&self) -> &Problem<'data> {
        &self.problem
    }
    #[must_use]
    pub const fn options(&self) -> Options {
        self.options
    }
    /// Consuming the certificate releases its input borrows. The detached
    /// numerical state cannot be supplied to accepted-state inference/outputs.
    #[must_use]
    pub fn into_state(self) -> FittedState {
        self.accepted
    }
}
impl std::ops::Deref for Fit<'_> {
    type Target = FittedState;
    fn deref(&self) -> &Self::Target {
        &self.accepted
    }
}
struct Context<'a, 'data> {
    problem: &'a Problem<'data>,
    family: &'a Family<'a>,
    design: &'a Design<'a>,
    options: Options,
    controls: Controls,
    team: &'a ThreadTeam,
    offset: Option<&'a [f64]>,
}
pub fn fit_final_sample<'data>(
    problem: &Problem<'data>,
    options: Options,
    team: &ThreadTeam,
) -> Result<Fit<'data>> {
    with_context(problem, options, team, |context| {
        let trajectory = context.run(Purpose::Publication)?;
        let report = trajectory
            .final_checkpoint
            .ok_or_else(|| invalid("publication trajectory has no accepted Hessian checkpoint"))?;
        context.finish(trajectory.accepted, report, trajectory.diagnostics)
    })
}
fn with_context<'data, T>(
    problem: &Problem<'data>,
    options: Options,
    team: &ThreadTeam,
    run: impl FnOnce(Context<'_, 'data>) -> Result<T>,
) -> Result<T> {
    let controls = Controls::new(&options)?;
    problem.sample.validate_alignment()?;
    if problem.sample.n_active() == 0 {
        return Err(FeregError::new(
            ErrorCode::InsufficientObservations,
            "PPML final sample is empty",
        ));
    }
    if problem.raw.nrows() != problem.sample.n_active()
        || problem.raw.ncols() != problem.roles.len()
    {
        return Err(invalid("PPML final sample and finite columns differ"));
    }
    if problem.raw.as_slice().iter().any(|v| !v.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML finite columns are nonfinite",
        ));
    }
    let offset = input::offset(problem.sample)?;
    let q = ReferenceWeights::new(
        &problem.sample.weights,
        problem.sample.n_active(),
        Estimator::Ppml,
    )?;
    let family = Family::new(&problem.sample.outcome, q, offset.as_deref())?;
    let design = Design::new(problem.sample, Weights::Reference(q))?;
    run(Context {
        problem,
        family: &family,
        design: &design,
        options,
        controls,
        team,
        offset: offset.as_deref(),
    })
}
impl<'a, 'data> Context<'a, 'data> {
    fn checkpoint(
        &self,
        accepted: &Accepted<'_, '_>,
        basis: &FrozenBasis,
        stage: Stage,
        diagnostics: &mut Diagnostics,
    ) -> Result<super::checkpoint::Report> {
        diagnostics.linear_frames += 1;
        diagnostics.strict_checks += 1;
        match stage {
            Stage::Fisher => diagnostics.fisher_checks += 1,
            Stage::FinalHessian => diagnostics.hessian_checks += 1,
        }
        let report = Checkpoint {
            family: self.family,
            design: self.design,
            raw: self.problem.raw,
            roles: self.problem.roles,
            basis,
            options: self.options,
        }
        .run(accepted.point(), accepted.evaluation(), stage, self.team)?;
        diagnostics.work.merge(report.work)?;
        Ok(report)
    }
    fn adopt(
        &self,
        accepted: &mut Accepted<'a, 'a>,
        basis: &mut FrozenBasis,
        report: super::checkpoint::Report,
        diagnostics: &mut Diagnostics,
    ) -> Result<bool> {
        let Some(candidate) = report.candidate else {
            return Ok(false);
        };
        let next = match accepted.try_step(&candidate.point, self.team) {
            Ok(next) => next,
            Err(error) if error.code == ErrorCode::NonConvergence => return Ok(false),
            Err(error) => return Err(error),
        };
        let halvings = next.halvings();
        let next = if candidate.basis_unchanged {
            next
        } else {
            let new_basis = FrozenBasis::from_fit(&candidate.linear.finite)?;
            diagnostics.basis_rebases += 1;
            let rebase = self
                .frame(
                    &next,
                    self.controls.strict_tolerance(),
                    Some(&new_basis),
                    Stage::FinalHessian,
                    diagnostics,
                )
                .and_then(|linear| self.represent(&next, &linear));
            match rebase {
                Ok(next) => {
                    *basis = new_basis;
                    next
                }
                Err(error) if error.code == ErrorCode::NonConvergence => return Ok(false),
                Err(error) => return Err(error),
            }
        };
        diagnostics.step_halvings += halvings;
        *accepted = next;
        Ok(true)
    }
    fn finish(
        &self,
        accepted: Accepted<'_, '_>,
        report: super::checkpoint::Report,
        diagnostics: Diagnostics,
    ) -> Result<Fit<'data>> {
        let candidate = report
            .candidate
            .ok_or_else(|| invalid("stable Hessian checkpoint has no candidate"))?;
        let (old_point, evaluation) = accepted.into_parts();
        // The bread and gauge belong to the exact input eta/mu. Replacing eta
        // by the numerically nearby linear reconstruction would stale qmu.
        let mut point = candidate.point;
        point.eta = old_point.eta;
        let state = self.family.certify_state(
            &point,
            &evaluation,
            self.problem.raw,
            self.design,
            self.controls.publication_tolerance(),
            self.team,
        )?;
        Ok(Fit {
            problem: *self.problem,
            options: self.options,
            accepted: FittedState {
                point,
                evaluation,
                hessian_weights: candidate.hessian_weights,
                linear: candidate.linear,
                scores: report.accepted_scores,
                state,
                diagnostics,
            },
        })
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
