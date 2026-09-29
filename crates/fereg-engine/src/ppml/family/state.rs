//! Original-model reconstruction and objective replay; no solver state is trusted.
use super::{invalid, numerical, Evaluation, Family};
use crate::{data::DenseMatrix, error::Result, fe::Design, ppml::step::Point, threads::ThreadTeam};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct StateCertificate {
    pub predictor_absolute_max: f64,
    pub predictor_relative_max: f64,
    pub mean_relative_max: f64,
    pub deviance_relative_error: f64,
    pub likelihood_relative_error: f64,
}
impl Family<'_> {
    /// Reconstruct eta from original X/beta and D/alpha, then replay mu and the
    /// objective. Score, strict-basis and final-Hessian checks remain separate.
    pub fn certify_state(
        &self,
        point: &Point,
        evaluation: &Evaluation,
        raw: &DenseMatrix,
        fe: &Design<'_>,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<StateCertificate> {
        point.validate()?;
        let n = self.outcome.len();
        if !tolerance.is_finite()
            || tolerance <= 0.
            || tolerance >= 1.
            || point.eta.len() != n
            || raw.nrows() != n
            || fe.nobs() != n
            || raw.ncols() != point.finite.len()
            || evaluation.mean.len() != n
            || evaluation
                .mean
                .iter()
                .any(|mu| !mu.is_finite() || *mu <= 0.)
            || raw.as_slice().iter().any(|x| !x.is_finite())
            || !evaluation.objective.deviance.is_finite()
            || evaluation.objective.deviance < 0.
            || !evaluation.objective.log_likelihood.is_finite()
        {
            return Err(invalid("PPML state certificate has malformed inputs"));
        }
        let absorbed = fe.multiply(&point.absorbed, team)?;
        let replay = self.evaluate(&point.eta, team)?;
        let mut result = StateCertificate::default();
        for row in 0..n {
            let offset = self.offset.map_or(0., |offset| offset[row]);
            let finite = raw.row_combination(row, &point.finite, 0.);
            let d = absorbed[(row, 0)];
            let eta = point.eta[row];
            let reconstructed = offset + finite + d;
            let error = (eta - reconstructed).abs();
            if !finite.is_finite() || !reconstructed.is_finite() || !error.is_finite() {
                return Err(numerical(
                    "PPML predictor reconstruction is not representable",
                ));
            }
            // Preserve the legacy output certificate's component-aware scale.
            let scale = eta
                .abs()
                .max(offset.abs())
                .max(finite.abs())
                .max(d.abs())
                .max(1.);
            result.predictor_absolute_max = result.predictor_absolute_max.max(error);
            result.predictor_relative_max = result.predictor_relative_max.max(error / scale);
            let old = evaluation.mean[row];
            let new = replay.mean[row];
            result.mean_relative_max = result
                .mean_relative_max
                .max((old - new).abs() / old.max(new).max(f64::MIN_POSITIVE));
        }
        result.deviance_relative_error =
            relative(evaluation.objective.deviance, replay.objective.deviance);
        result.likelihood_relative_error = relative(
            evaluation.objective.log_likelihood,
            replay.objective.log_likelihood,
        );
        if [
            result.predictor_relative_max,
            result.mean_relative_max,
            result.deviance_relative_error,
            result.likelihood_relative_error,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v > tolerance)
        {
            return Err(numerical(
                "PPML original-model reconstruction or objective replay did not certify",
            ));
        }
        Ok(result)
    }
}
fn relative(left: f64, right: f64) -> f64 {
    (left - right).abs() / left.abs().max(right.abs()).max(1.)
}
