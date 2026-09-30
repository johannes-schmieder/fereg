use super::*;
use crate::{
    error::reserved,
    fe::Engine,
    wls::{self, AbsorbedControls, Policy},
};
impl<'a> Context<'a, '_, '_> {
    pub(super) fn frame(
        &self,
        accepted: &Accepted<'_, '_>,
        tolerance: f64,
        basis: Option<&FrozenBasis>,
        stage: Stage,
        diagnostics: &mut Diagnostics,
    ) -> Result<AbsorbedFit> {
        diagnostics.linear_frames += 1;
        let n = self.problem.sample.n_active();
        let mut h = reserved(n)?;
        h.resize(n, 0.);
        let mut response = reserved(n)?;
        response.resize(n, 0.);
        match stage {
            Stage::Fisher => self.family.working_into(
                &accepted.point().eta,
                accepted.evaluation(),
                &mut h,
                &mut response,
                self.team,
            )?,
            Stage::FinalHessian => self.family.hessian_into(
                &accepted.point().eta,
                accepted.evaluation(),
                &mut h,
                &mut response,
                self.team,
            )?,
        }
        let design = Design::new(self.problem.sample, Weights::Provided(&h))?;
        let options = Options {
            tolerance,
            ..self.options
        };
        let mut engine = Engine::new_polled(
            &design,
            Estimator::Ppml,
            &options,
            self.team,
            self.probe.dense(),
        )?;
        if stage == Stage::Fisher {
            let width = self
                .problem
                .raw
                .ncols()
                .checked_add(1)
                .ok_or_else(|| invalid("PPML RHS bank width overflows"))?;
            engine.seed_response(&accepted.point().absorbed, width)?;
        }
        let policy = Policy {
            score_tolerance: Policy::default().score_tolerance.max(tolerance),
            ..Policy::default()
        };
        wls::solve_absorbed_recorded(
            &mut engine,
            self.problem.raw,
            &response,
            self.problem.roles,
            AbsorbedControls {
                probe: self.probe,
                finite: policy,
                basis,
            },
            &mut diagnostics.work,
            self.team,
        ).map_err(|error| {
            let minimum = accepted.evaluation().mean.iter().copied().fold(f64::INFINITY, f64::min);
            let maximum = accepted.evaluation().mean.iter().copied().fold(0_f64, f64::max);
            FeregError::new(error.code, format!(
                "PPML {stage:?} frame {} failed after {} accepted iterations (inner tolerance={tolerance:e}, mean range={minimum:e}..{maximum:e}): {}",
                diagnostics.linear_frames, diagnostics.accepted_iterations, error.message
            ))
        })
    }
    pub(super) fn represent(
        &self,
        accepted: &Accepted<'_, '_>,
        linear: &AbsorbedFit,
    ) -> Result<Accepted<'a, 'a>> {
        let mut point =
            self.family
                .point_from_linear(linear, self.problem.raw, self.design, self.team)?;
        let mut eta = reserved(accepted.point().eta.len())?;
        eta.extend_from_slice(&accepted.point().eta);
        point.eta = eta;
        self.family.certify_state(
            &point,
            accepted.evaluation(),
            self.problem.raw,
            self.design,
            self.controls.publication_tolerance(),
            self.team,
        )?;
        Accepted::start(self.family, point, self.team)
    }
}
