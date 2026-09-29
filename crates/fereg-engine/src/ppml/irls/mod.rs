//! Bounded IRLS on a sample already finalized by separation and singleton pruning.
mod frame;
mod input;
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
pub struct Problem<'a> {
    pub sample: &'a Prepared,
    pub raw: &'a DenseMatrix,
    pub roles: &'a [Role],
}
#[derive(Debug, Default, PartialEq, Eq)]
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
pub struct Fit {
    pub point: Point,
    pub evaluation: Evaluation,
    pub hessian_weights: Vec<f64>,
    pub linear: AbsorbedFit,
    pub scores: ScoreCertificate,
    pub state: StateCertificate,
    pub diagnostics: Diagnostics,
}
struct Context<'a, 'data> {
    problem: &'a Problem<'data>,
    family: &'a Family<'data>,
    design: &'a Design<'data>,
    options: Options,
    controls: Controls,
    team: &'a ThreadTeam,
    offset: Option<&'a [f64]>,
}
pub fn fit_final_sample(problem: &Problem<'_>, options: Options, team: &ThreadTeam) -> Result<Fit> {
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
    Context {
        problem,
        family: &family,
        design: &design,
        options,
        controls,
        team,
        offset: offset.as_deref(),
    }
    .run()
}
impl<'a, 'data> Context<'a, 'data> {
    fn run(&self) -> Result<Fit> {
        let mut diagnostics = Diagnostics::default();
        let mut accepted = self.start(&mut diagnostics)?;
        // Only a canonical solve over the final sample has omission authority.
        // Start helpers supply guesses and cannot choose a retained basis.
        let initial = self.frame(
            &accepted,
            self.controls.strict_tolerance(),
            None,
            Stage::FinalHessian,
            &mut diagnostics,
        )?;
        let mut basis = FrozenBasis::from_fit(&initial.finite)?;
        accepted = self.represent(&accepted, &initial)?;
        drop(initial);
        let mut convergence = Convergence::new(self.controls);
        loop {
            let linear = self.frame(
                &accepted,
                convergence.next_inner_tolerance(),
                Some(&basis),
                Stage::Fisher,
                &mut diagnostics,
            )?;
            let proposal =
                self.family
                    .point_from_linear(&linear, self.problem.raw, self.design, self.team)?;
            drop(linear);
            let old = accepted.evaluation().objective.deviance;
            accepted = accepted.try_step(&proposal, self.team)?;
            drop(proposal);
            diagnostics.step_halvings += accepted.halvings();
            accepted.certify_state(
                self.problem.raw,
                self.design,
                self.controls.publication_tolerance(),
                self.team,
            )?;
            let raw = super::checkpoint::score_columns(self.problem.raw, basis.kept())?;
            let scores = self.family.score_certificate(
                &accepted.point().eta,
                &raw,
                self.design,
                self.controls.tolerance(),
                self.team,
            )?;
            let decision = convergence.observe(
                old,
                accepted.evaluation().objective.deviance,
                scores.convergence_scores(),
            )?;
            diagnostics.accepted_iterations = convergence.accepted_steps();
            if decision == Decision::Continue {
                continue;
            }
            let strict = self.checkpoint(&accepted, &basis, Stage::Fisher, &mut diagnostics)?;
            let stable = strict.decision == CheckDecision::Stable;
            let adopted = self.adopt(&mut accepted, &mut basis, strict, &mut diagnostics)?;
            if !stable || !adopted {
                diagnostics.strict_resumes += 1;
                convergence.reject_strict()?;
                continue;
            }
            let final_check =
                self.checkpoint(&accepted, &basis, Stage::FinalHessian, &mut diagnostics)?;
            if final_check.decision == CheckDecision::Stable {
                return self.finish(accepted, final_check, diagnostics);
            }
            self.adopt(&mut accepted, &mut basis, final_check, &mut diagnostics)?;
            diagnostics.strict_resumes += 1;
            convergence.reject_strict()?;
        }
    }
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
        accepted: &mut Accepted<'a, 'data>,
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
    ) -> Result<Fit> {
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
            point,
            evaluation,
            hessian_weights: candidate.hessian_weights,
            linear: candidate.linear,
            scores: report.accepted_scores,
            state,
            diagnostics,
        })
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
