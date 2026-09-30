//! Fitted means nominate candidates; only a fresh cone/span proof can delete.
use super::{invalid, Controls, DirectionProposal, Method, Model, Status, View};
use crate::{
    data::Weights,
    error::{reserved, ErrorCode, FeregError, Result},
    fe::{Design, Engine, Plan},
    ppml::irls::{self, Diagnostics, MuCertificate, Problem},
    spec::{Estimator, Solver},
    threads::ThreadTeam,
    wls::{self, Work},
};
pub const DEFAULT_MU_TOLERANCE: f64 = 1e-8;
#[derive(Debug, Clone, PartialEq)]
pub struct MuReport {
    pub generation: usize,
    pub sample_identity: u64,
    pub controls: Controls,
    pub solver: Solver,
    pub linear_tolerance: f64,
    pub budget: usize,
    pub mu_tolerance: f64,
    pub reference_mean: f64,
    pub candidates: usize,
    pub status: Status,
    pub direction: Option<DirectionProposal>,
    pub trajectory: MuCertificate,
    pub trajectory_diagnostics: Diagnostics,
    pub projection_frames: usize,
    pub work: Work,
}
/// No fitted parameters, factors or means can cross the terminal boundary.
#[derive(Debug, PartialEq)]
pub struct MuTerminal {
    generation: usize,
    sample_identity: u64,
    iterations: usize,
    work: Work,
}
impl MuTerminal {
    #[must_use]
    pub const fn generation(&self) -> usize {
        self.generation
    }
    #[must_use]
    pub const fn sample_identity(&self) -> u64 {
        self.sample_identity
    }
    #[must_use]
    pub const fn iterations(&self) -> usize {
        self.iterations
    }
    #[must_use]
    pub const fn work(&self) -> Work {
        self.work
    }
}
impl View<'_> {
    pub fn fitted_mu(
        self,
        model: &Model<'_>,
        mu_tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<MuReport> {
        model.validate(self)?;
        let design = Design::new(self.sample(), Weights::Unit)?;
        Plan::new(&design, Estimator::Ppml, &model.options)?;
        drop(design);
        let (mean, trajectory, diagnostics) = irls::explore_mu(
            &Problem {
                sample: self.sample(),
                raw: model.raw,
                roles: model.roles,
            },
            model.options,
            mu_tolerance,
            team,
        )?
        .into_parts();
        let threshold = irls::MeanThreshold::new(&self.sample().outcome, &mean, mu_tolerance);
        let mut candidate = reserved(self.sample().n_active())?;
        candidate.extend(self.sample().outcome.iter().zip(&mean).map(|(&y, &mu)| {
            if threshold.candidate(y, mu) {
                1.
            } else {
                0.
            }
        }));
        let candidates = candidate.iter().filter(|&&v| v == 1.).count();
        let mut report = MuReport {
            generation: self.generation(),
            sample_identity: self.numerical_identity(),
            controls: model.controls,
            solver: model.options.solver,
            linear_tolerance: model.options.tolerance,
            budget: model.options.iterations,
            mu_tolerance,
            reference_mean: threshold.reference,
            candidates,
            status: Status::NoCandidates,
            direction: None,
            trajectory,
            work: diagnostics.work,
            trajectory_diagnostics: diagnostics,
            projection_frames: 0,
        };
        // Even a no-candidate check consumes its fitted trajectory whole. A
        // final estimator fit always starts fresh after separation completes.
        drop(mean);
        if candidates == 0 {
            return Ok(report);
        }
        let direction = self.mu_projection(&candidate, model, &mut report.work, team)?;
        report.projection_frames = 1;
        let proposal = self.direction_proposal(
            Method::Mu,
            report.trajectory.accepted_iterations,
            direction,
            model.controls,
        )?;
        // This is still only a cone claim. The ledger independently certifies
        // its original-model span before accepting any row.
        report.status = Status::Certified;
        report.direction = Some(proposal);
        Ok(report)
    }
    pub fn replay_mu(
        self,
        observed: &MuReport,
        model: &Model<'_>,
        mu_tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<MuTerminal> {
        if observed.generation != self.generation()
            || observed.sample_identity != self.numerical_identity()
            || observed.mu_tolerance != mu_tolerance
        {
            return Err(invalid(
                "Mu report belongs to a different generation or threshold",
            ));
        }
        if observed.status != Status::NoCandidates || observed.direction.is_some() {
            return Err(failed(
                "Mu terminal replay requires a fitted no-candidate result",
            ));
        }
        let fresh = self.fitted_mu(model, mu_tolerance, team)?;
        if &fresh != observed {
            return Err(failed(
                "Mu fitted trajectory failed fresh deterministic replay",
            ));
        }
        Ok(MuTerminal {
            generation: fresh.generation,
            sample_identity: fresh.sample_identity,
            iterations: fresh.trajectory.accepted_iterations,
            work: fresh.work,
        })
    }
    fn mu_projection(
        self,
        candidate: &[f64],
        model: &Model<'_>,
        work: &mut Work,
        team: &ThreadTeam,
    ) -> Result<Vec<f64>> {
        let design = Design::new(self.sample(), Weights::Unit)?;
        let mut engine = Engine::new(&design, Estimator::Ppml, &model.span_options()?, team)?;
        let fit = wls::solve_absorbed_recorded(
            &mut engine,
            model.raw,
            candidate,
            model.roles,
            wls::AbsorbedControls {
                finite: model.linear_policy(),
                basis: None,
            },
            work,
            team,
        )?;
        let mut direction =
            super::numerical::fitted(model.raw, &design, &fit, team)?.into_row_major();
        direction.iter_mut().for_each(|v| *v = -*v);
        Ok(direction)
    }
}
fn failed(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
