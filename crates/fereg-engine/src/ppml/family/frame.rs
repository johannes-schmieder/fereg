//! Fresh working and final-Hessian frames share validation and weight arithmetic.
use super::{invalid, mean_from_eta, numerical, Evaluation, Family};
use crate::{
    error::{reserved, Result},
    threads::{ThreadTeam, REDUCTION_CHUNK},
};
use rayon::prelude::*;
#[derive(Clone, Copy)]
enum Response {
    Fisher,
    Hessian,
}
#[derive(Clone, Copy)]
struct Frame<'a> {
    eta: &'a [f64],
    evaluation: &'a Evaluation,
    response: Response,
}
impl Family<'_> {
    /// Fill private scratch only. Any error invalidates the entire proposed frame.
    pub fn working_into(
        &self,
        eta: &[f64],
        evaluation: &Evaluation,
        h: &mut [f64],
        z: &mut [f64],
        team: &ThreadTeam,
    ) -> Result<()> {
        self.fill_into(
            Frame {
                eta,
                evaluation,
                response: Response::Fisher,
            },
            h,
            z,
            team,
        )
    }

    /// Rebuild h=qmu at exactly this predictor, with target eta-offset.
    /// No Fisher correction may move the state that owns the final bread.
    pub fn hessian_into(
        &self,
        eta: &[f64],
        evaluation: &Evaluation,
        h: &mut [f64],
        target: &mut [f64],
        team: &ThreadTeam,
    ) -> Result<()> {
        self.fill_into(
            Frame {
                eta,
                evaluation,
                response: Response::Hessian,
            },
            h,
            target,
            team,
        )
    }
    fn fill_into(
        &self,
        frame: Frame<'_>,
        h: &mut [f64],
        z: &mut [f64],
        team: &ThreadTeam,
    ) -> Result<()> {
        let Frame {
            eta, evaluation, ..
        } = frame;
        let n = self.outcome.len();
        if [eta.len(), evaluation.mean.len(), h.len(), z.len()]
            .iter()
            .any(|&length| length != n)
        {
            return Err(invalid("Poisson working-frame lengths differ"));
        }
        let mut errors = reserved(n.div_ceil(REDUCTION_CHUNK))?;
        errors.resize_with(n.div_ceil(REDUCTION_CHUNK), || None);
        team.install(|| {
            h.par_chunks_mut(REDUCTION_CHUNK)
                .zip(z.par_chunks_mut(REDUCTION_CHUNK))
                .zip(errors.par_iter_mut())
                .enumerate()
                .for_each(|(block, ((h, z), error))| {
                    *error = Some(self.working_block(frame, block * REDUCTION_CHUNK, h, z));
                });
        });
        for error in errors {
            error.expect("visited working block")?;
        }
        Ok(())
    }
    fn working_block(
        &self,
        frame: Frame<'_>,
        start: usize,
        h: &mut [f64],
        z: &mut [f64],
    ) -> Result<()> {
        let Frame {
            eta,
            evaluation,
            response: kind,
        } = frame;
        for (local, (weight, response)) in h.iter_mut().zip(z).enumerate() {
            let row = start + local;
            let mu = mean_from_eta(eta[row])?;
            if mu.to_bits() != evaluation.mean[row].to_bits() {
                return Err(invalid(
                    "Poisson means belong to a different predictor frame",
                ));
            }
            *weight = self.prior.value(row) * mu;
            let predictor = eta[row] - self.offset.map_or(0., |values| values[row]);
            *response = match kind {
                Response::Hessian => predictor,
                Response::Fisher if self.outcome[row] == 0. => predictor - 1.,
                // Preserve Newton corrections near saturation: exp(x)-1
                // rounds a small correction to zero and freezes FE noise.
                Response::Fisher => predictor + (self.terms[row][0] - eta[row]).exp_m1(),
            };
            if !weight.is_finite() || *weight <= 0. || !response.is_finite() {
                return Err(numerical("Poisson working state is not representable"));
            }
        }
        Ok(())
    }
}
