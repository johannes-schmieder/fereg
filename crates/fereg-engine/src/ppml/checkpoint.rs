//! Fresh strict solves are speculative; accepted parameters are never mutated here.
mod profile;
use super::{
    control::Controls,
    family::{Evaluation, Family, ScoreCertificate, StateCertificate},
    step::Point,
};
use crate::{
    data::{DenseMatrix, Weights},
    error::{reserved, ErrorCode, FeregError, Result},
    execution::Probe,
    fe::{Design, Engine},
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls::{self, AbsorbedFit, FrozenBasis, Policy, Role},
};
pub use profile::Movement;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Fisher,
    FinalHessian,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    ResumeAccepted,
    ResumeCandidate,
    /// A stable Fisher checkpoint still requires a separate final-Hessian
    /// checkpoint. Final bread belongs to that checkpoint's input predictor.
    Stable,
}
#[derive(Debug, PartialEq)]
pub struct Candidate {
    pub point: Point,
    pub evaluation: Evaluation,
    /// These weights belong to the input accepted predictor, not the candidate.
    pub hessian_weights: Vec<f64>,
    pub linear: AbsorbedFit,
    pub movement: Movement,
    pub basis_unchanged: bool,
    pub scores: ScoreCertificate,
    pub state: StateCertificate,
}
#[derive(Debug, PartialEq)]
pub struct Report {
    pub stage: Stage,
    pub decision: Decision,
    /// Absent only when a finite linear candidate has no representable Poisson objective.
    pub candidate: Option<Candidate>,
    pub accepted_scores: ScoreCertificate,
    pub accepted_state: StateCertificate,
    /// Includes the linear frame even when its Poisson candidate is unrepresentable.
    pub work: wls::Work,
}
/// All inputs must describe the same final separation/singleton sample and
/// canonical generated-column order. The driver owns generation alignment.
pub struct Checkpoint<'a, 'data> {
    pub family: &'a Family<'data>,
    pub design: &'a Design<'data>,
    pub raw: &'a DenseMatrix,
    pub roles: &'a [Role],
    pub basis: &'a FrozenBasis,
    pub options: Options,
}
impl Checkpoint<'_, '_> {
    pub fn run(
        &self,
        point: &Point,
        evaluation: &Evaluation,
        stage: Stage,
        team: &ThreadTeam,
    ) -> Result<Report> {
        self.run_polled(point, evaluation, stage, team, Probe::default())
    }
    pub(crate) fn run_polled(
        &self,
        point: &Point,
        evaluation: &Evaluation,
        stage: Stage,
        team: &ThreadTeam,
        probe: Probe<'_>,
    ) -> Result<Report> {
        let controls = Controls::new(&self.options)?;
        if self.basis.statuses().len() != point.finite.len()
            || self
                .basis
                .statuses()
                .iter()
                .zip(&point.finite)
                .any(|(status, beta)| *status != wls::Status::Kept && *beta != 0.)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "accepted PPML coefficients do not match the canonical omission map",
            ));
        }
        let accepted_state = self.family.certify_state(
            point,
            evaluation,
            self.raw,
            self.design,
            controls.publication_tolerance(),
            team,
        )?;
        let score_raw = score_columns(self.raw, self.basis.kept())?;
        let accepted_scores = self.family.score_certificate(
            &point.eta,
            &score_raw,
            self.design,
            controls.tolerance(),
            team,
        )?;
        let mut h = reserved(point.eta.len())?;
        h.resize(point.eta.len(), 0.);
        let mut response = reserved(point.eta.len())?;
        response.resize(point.eta.len(), 0.);
        match stage {
            Stage::Fisher => {
                self.family
                    .working_into(&point.eta, evaluation, &mut h, &mut response, team)?
            }
            Stage::FinalHessian => {
                self.family
                    .hessian_into(&point.eta, evaluation, &mut h, &mut response, team)?
            }
        }
        let fresh = Design::new(self.design.sample(), Weights::Provided(&h))?;
        let options = Options {
            tolerance: controls.strict_tolerance(),
            ..self.options
        };
        let mut engine =
            Engine::new_polled(&fresh, Estimator::Ppml, &options, team, probe.dense())?;
        // Strictness tightens FE projection; finite score/inverse guards keep
        // their shared scientific policy, as in the legacy strict solve.
        let policy = Policy::default();
        let mut work = wls::Work::default();
        let linear = wls::solve_absorbed_recorded(
            &mut engine,
            self.raw,
            &response,
            self.roles,
            wls::AbsorbedControls {
                probe,
                finite: policy,
                basis: None,
            },
            &mut work,
            team,
        )?;
        linear.revalidate_basis_polled(
            &fresh,
            self.raw,
            self.roles,
            wls::AbsorbedControls {
                finite: policy,
                basis: Some(self.basis),
                probe,
            },
            team,
        )?;
        let basis_unchanged = self.basis.matches(&linear.finite);
        if stage == Stage::FinalHessian && !basis_unchanged {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML final Hessian changed the canonical rank basis",
            ));
        }
        let candidate = self
            .family
            .point_from_linear(&linear, self.raw, &fresh, team)?;
        let candidate_evaluation = match self.family.evaluate(&candidate.eta, team) {
            Ok(value) => value,
            Err(error) if error.code == ErrorCode::NonConvergence => {
                return Ok(Report {
                    stage,
                    decision: Decision::ResumeAccepted,
                    candidate: None,
                    accepted_scores,
                    accepted_state,
                    work,
                });
            }
            Err(error) => return Err(error),
        };
        let movement = Movement::between(point, evaluation, &candidate, &candidate_evaluation)?;
        let candidate_raw = score_columns(self.raw, &linear.finite.kept)?;
        let scores = self.family.score_certificate(
            &candidate.eta,
            &candidate_raw,
            &fresh,
            controls.tolerance(),
            team,
        )?;
        let state = self.family.certify_state(
            &candidate,
            &candidate_evaluation,
            self.raw,
            &fresh,
            controls.publication_tolerance(),
            team,
        )?;
        let decision = if basis_unchanged
            && movement.maximum() <= controls.publication_tolerance()
            && accepted_scores.passed
            && scores.passed
        {
            Decision::Stable
        } else {
            Decision::ResumeCandidate
        };
        Ok(Report {
            stage,
            decision,
            accepted_scores,
            accepted_state,
            work,
            candidate: Some(Candidate {
                point: candidate,
                evaluation: candidate_evaluation,
                hessian_weights: h,
                linear,
                movement,
                basis_unchanged,
                scores,
                state,
            }),
        })
    }
}
pub(super) fn score_columns(raw: &DenseMatrix, kept: &[usize]) -> Result<DenseMatrix> {
    if kept.iter().any(|&column| column >= raw.ncols()) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML score basis is out of range",
        ));
    }
    let mut selected = DenseMatrix::zeros(raw.nrows(), kept.len())?;
    for row in 0..raw.nrows() {
        for (local, &column) in kept.iter().enumerate() {
            selected[(row, local)] = raw[(row, column)];
        }
    }
    Ok(selected)
}
