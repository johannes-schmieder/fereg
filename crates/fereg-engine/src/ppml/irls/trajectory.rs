use super::*;
use crate::execution::{self, Poll};
#[derive(Debug, Clone, Copy)]
pub(super) enum Purpose {
    Publication,
    Mu { tolerance: f64 },
}
impl Purpose {
    fn candidate(self, outcome: &[f64], mean: &[f64], poll: Poll<'_>) -> Result<bool> {
        let Self::Mu { tolerance } = self else {
            return Ok(false);
        };
        let threshold = MeanThreshold::new_polled(outcome, mean, tolerance, poll)?;
        execution::any(
            outcome.iter().zip(mean),
            |(&y, &mu)| threshold.candidate(y, mu),
            poll,
        )
    }
    fn exploration(self) -> bool {
        matches!(self, Self::Mu { .. })
    }
}
pub(super) struct Trajectory<'a, 'data> {
    pub accepted: Accepted<'a, 'data>,
    pub final_checkpoint: Option<super::super::checkpoint::Report>,
    pub diagnostics: Diagnostics,
    pub certified_linear_frames: usize,
    pub maximum_deviance_increase_relative: f64,
    pub terminal: MuStop,
}
#[derive(Default)]
struct Integrity {
    linear_frames: usize,
    maximum_increase: f64,
}
impl Integrity {
    fn observe(&mut self, old: f64, new: f64) {
        self.maximum_increase = self.maximum_increase.max((new - old).max(0.) / old.max(1.));
    }
    fn finish<'a, 'data>(
        self,
        accepted: Accepted<'a, 'data>,
        diagnostics: Diagnostics,
        terminal: MuStop,
    ) -> Trajectory<'a, 'data> {
        Trajectory {
            accepted,
            diagnostics,
            terminal,
            final_checkpoint: None,
            certified_linear_frames: self.linear_frames,
            maximum_deviance_increase_relative: self.maximum_increase,
        }
    }
}
impl<'a> Context<'a, '_, '_> {
    pub(super) fn run(
        &self,
        purpose: Purpose,
        interrupt: &mut Interrupt<'_>,
    ) -> Result<Trajectory<'a, 'a>> {
        let mut diagnostics = Diagnostics::default();
        interrupt(InterruptPoint::LinearFrame)?;
        let mut accepted = self.start(&mut diagnostics, interrupt)?;
        // The same canonical auxiliary basis is used by both purposes. It
        // has public omission authority only after the separation sample is
        // final; exploratory state never becomes an estimator result.
        interrupt(InterruptPoint::LinearFrame)?;
        let initial = self.frame(
            &accepted,
            self.controls.strict_tolerance(),
            None,
            Stage::FinalHessian,
            &mut diagnostics,
        )?;
        let mut integrity = Integrity {
            linear_frames: 1,
            maximum_increase: 0.,
        };
        let mut basis = FrozenBasis::from_fit(&initial.finite)?;
        accepted = self.represent(&accepted, &initial)?;
        drop(initial);
        let mut convergence = Convergence::new(self.controls);
        loop {
            interrupt(if purpose.exploration() {
                InterruptPoint::MuIteration
            } else {
                InterruptPoint::IrlsIteration
            })?;
            interrupt(InterruptPoint::LinearFrame)?;
            let linear = self.frame(
                &accepted,
                convergence.next_inner_tolerance(),
                Some(&basis),
                Stage::Fisher,
                &mut diagnostics,
            )?;
            integrity.linear_frames += 1;
            let proposal =
                self.family
                    .point_from_linear(&linear, self.problem.raw, self.design, self.team)?;
            drop(linear);
            let old = accepted.evaluation().objective.deviance;
            accepted = match accepted.try_step_checked(
                &proposal,
                self.team,
                interrupt,
                self.probe.dense(),
            ) {
                Ok(next) => next,
                Err(failure)
                    if !failure.callback
                        && purpose.exploration()
                        && failure.error.code == ErrorCode::NonConvergence =>
                {
                    return Ok(integrity.finish(accepted, diagnostics, MuStop::LineSearchStalled));
                }
                Err(failure) => return Err(failure.error),
            };
            drop(proposal);
            integrity.observe(old, accepted.evaluation().objective.deviance);
            diagnostics.step_halvings += accepted.halvings();
            let scores = self.accepted_scores(&accepted, &basis)?;
            diagnostics.accepted_iterations = convergence.accepted_steps() + 1;
            if purpose.candidate(
                &self.problem.sample.outcome,
                &accepted.evaluation().mean,
                self.probe.dense(),
            )? {
                return Ok(integrity.finish(
                    accepted,
                    diagnostics,
                    MuStop::CandidateThresholdReached,
                ));
            }
            let decision =
                match convergence.observe(old, accepted.evaluation().objective.deviance, scores) {
                    Ok(decision) => decision,
                    Err(error)
                        if purpose.exploration()
                            && error.code == ErrorCode::NonConvergence
                            && convergence.accepted_steps() == self.options.iterations =>
                    {
                        return Ok(integrity.finish(
                            accepted,
                            diagnostics,
                            MuStop::MaximumIterations,
                        ));
                    }
                    Err(error) => return Err(error),
                };
            if decision == Decision::Continue {
                continue;
            }
            interrupt(InterruptPoint::StrictCheckpoint)?;
            let strict = self.checkpoint(&accepted, &basis, Stage::Fisher, &mut diagnostics)?;
            let stable = strict.decision == CheckDecision::Stable;
            let old = accepted.evaluation().objective.deviance;
            let adopted = self.adopt(
                &mut accepted,
                &mut basis,
                strict,
                &mut diagnostics,
                interrupt,
            )?;
            integrity.observe(old, accepted.evaluation().objective.deviance);
            if !stable || !adopted {
                if self.resume(&mut convergence, &mut diagnostics, purpose)? {
                    return Ok(integrity.finish(accepted, diagnostics, MuStop::MaximumIterations));
                }
                continue;
            }
            interrupt(InterruptPoint::StrictCheckpoint)?;
            let final_check =
                self.checkpoint(&accepted, &basis, Stage::FinalHessian, &mut diagnostics)?;
            if final_check.decision == CheckDecision::Stable {
                let mut trajectory = integrity.finish(accepted, diagnostics, MuStop::Converged);
                trajectory.final_checkpoint = Some(final_check);
                return Ok(trajectory);
            }
            let old = accepted.evaluation().objective.deviance;
            self.adopt(
                &mut accepted,
                &mut basis,
                final_check,
                &mut diagnostics,
                interrupt,
            )?;
            integrity.observe(old, accepted.evaluation().objective.deviance);
            if self.resume(&mut convergence, &mut diagnostics, purpose)? {
                return Ok(integrity.finish(accepted, diagnostics, MuStop::MaximumIterations));
            }
        }
    }
    fn accepted_scores(
        &self,
        accepted: &Accepted<'_, '_>,
        basis: &FrozenBasis,
    ) -> Result<super::super::control::Scores> {
        accepted.certify_state(
            self.problem.raw,
            self.design,
            self.controls.publication_tolerance(),
            self.team,
        )?;
        let raw = super::super::checkpoint::score_columns(self.problem.raw, basis.kept())?;
        Ok(self
            .family
            .score_certificate(
                &accepted.point().eta,
                &raw,
                self.design,
                self.controls.tolerance(),
                self.team,
            )?
            .convergence_scores())
    }
    fn resume(
        &self,
        convergence: &mut Convergence,
        diagnostics: &mut Diagnostics,
        purpose: Purpose,
    ) -> Result<bool> {
        diagnostics.strict_resumes += 1;
        match convergence.reject_strict() {
            Ok(()) => Ok(false),
            Err(error)
                if purpose.exploration()
                    && error.code == ErrorCode::NonConvergence
                    && convergence.accepted_steps() == self.options.iterations =>
            {
                Ok(true)
            }
            Err(error) => Err(error),
        }
    }
}
