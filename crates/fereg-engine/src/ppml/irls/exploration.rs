use super::*;
pub(crate) struct MeanThreshold {
    pub(crate) reference: f64,
    tolerance: f64,
}
impl MeanThreshold {
    pub(crate) fn new(outcome: &[f64], mean: &[f64], tolerance: f64) -> Self {
        let reference = outcome
            .iter()
            .zip(mean)
            .filter_map(|(&y, &mu)| (y > 0.).then_some(mu))
            .fold(f64::INFINITY, f64::min);
        Self {
            reference,
            tolerance,
        }
    }
    pub(crate) fn candidate(&self, y: f64, mu: f64) -> bool {
        y == 0. && mu <= self.tolerance * self.reference
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuStop {
    Converged,
    MaximumIterations,
    LineSearchStalled,
    CandidateThresholdReached,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MuCertificate {
    pub accepted_iterations: usize,
    pub certified_linear_frames: usize,
    pub maximum_deviance_increase_relative: f64,
    pub mean_reconstruction_relative: f64,
    pub state: StateCertificate,
    pub terminal: MuStop,
}
/// Only the common accepted IRLS trajectory constructs this state. It has no
/// conversion to Fit; a separation restart or publication discards it whole.
pub(crate) struct Exploration {
    mean: Vec<f64>,
    certificate: MuCertificate,
    diagnostics: Diagnostics,
}
impl Exploration {
    pub(crate) fn into_parts(self) -> (Vec<f64>, MuCertificate, Diagnostics) {
        (self.mean, self.certificate, self.diagnostics)
    }
}
pub(crate) fn explore_mu(
    problem: &Problem<'_>,
    options: Options,
    tolerance: f64,
    team: &ThreadTeam,
) -> Result<Exploration> {
    if !tolerance.is_finite() || tolerance <= 0. || tolerance >= 1. {
        return Err(invalid(
            "Mu threshold must be finite and strictly between zero and one",
        ));
    }
    with_context(problem, options, team, |context| {
        let trajectory = context.run(Purpose::Mu { tolerance })?;
        context.finish_exploration(trajectory)
    })
}
impl<'a> Context<'a, '_> {
    fn finish_exploration(&self, trajectory: Trajectory<'a, 'a>) -> Result<Exploration> {
        let state = trajectory.accepted.certify_state(
            self.problem.raw,
            self.design,
            self.controls.publication_tolerance(),
            self.team,
        )?;
        let certificate = MuCertificate {
            accepted_iterations: trajectory.diagnostics.accepted_iterations,
            certified_linear_frames: trajectory.certified_linear_frames,
            maximum_deviance_increase_relative: trajectory.maximum_deviance_increase_relative,
            mean_reconstruction_relative: state.mean_relative_max,
            state,
            terminal: trajectory.terminal,
        };
        if certificate.accepted_iterations == 0
            || certificate.certified_linear_frames < certificate.accepted_iterations
            || certificate.maximum_deviance_increase_relative > 1e-12 + 64. * f64::EPSILON
            || certificate.mean_reconstruction_relative > 64. * f64::EPSILON
        {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "Mu exploratory trajectory did not certify",
            ));
        }
        let (_, evaluation) = trajectory.accepted.into_parts();
        Ok(Exploration {
            mean: evaluation.mean,
            certificate,
            diagnostics: trajectory.diagnostics,
        })
    }
}
