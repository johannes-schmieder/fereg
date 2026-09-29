//! Harvested publication envelope: small coefficients require absolute accuracy.
use crate::{
    error::{ErrorCode, FeregError, Result},
    ppml::{family::Evaluation, step::Point},
};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Movement {
    pub predictor: f64,
    pub coefficients: f64,
    pub deviance: f64,
    pub log_likelihood: f64,
}
impl Movement {
    pub fn between(
        left: &Point,
        left_eval: &Evaluation,
        right: &Point,
        right_eval: &Evaluation,
    ) -> Result<Self> {
        if left.eta.len() != right.eta.len() || left.finite.len() != right.finite.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML checkpoint parameter dimensions differ",
            ));
        }
        left.validate()?;
        right.validate()?;
        let coefficients = left
            .finite
            .iter()
            .zip(&right.finite)
            .map(|(a, b)| {
                let magnitude = a.abs().max(b.abs());
                (a - b).abs() / (1e-3 + magnitude).min(magnitude.max(1.))
            })
            .fold(0., f64::max);
        let predictor = left
            .eta
            .iter()
            .zip(&right.eta)
            .map(|(&a, &b)| relative(a, b))
            .fold(0., f64::max);
        let result = Self {
            predictor,
            coefficients,
            deviance: relative(left_eval.objective.deviance, right_eval.objective.deviance),
            log_likelihood: relative(
                left_eval.objective.log_likelihood,
                right_eval.objective.log_likelihood,
            ),
        };
        if [
            result.predictor,
            result.coefficients,
            result.deviance,
            result.log_likelihood,
        ]
        .iter()
        .any(|v| !v.is_finite())
        {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML checkpoint movement is not representable",
            ));
        }
        Ok(result)
    }
    pub fn maximum(self) -> f64 {
        self.predictor
            .max(self.coefficients)
            .max(self.deviance)
            .max(self.log_likelihood)
    }
}
fn relative(a: f64, b: f64) -> f64 {
    (a - b).abs() / a.abs().max(b.abs()).max(1.)
}
