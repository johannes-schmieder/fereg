//! Immutable Poisson inputs; trial evaluation never mutates an accepted state.
mod frame;
mod math;
mod score;
mod start;
mod state;
use crate::{
    data::{ReferenceWeights, WeightKind},
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
    threads::{CompensatedSum, ThreadTeam, REDUCTION_CHUNK},
};
use rayon::prelude::*;
pub use score::{ScoreCertificate, ScoreNorms};
pub use start::NullObjective;
pub use state::StateCertificate;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Objective {
    pub deviance: f64,
    pub log_likelihood: f64,
}
#[derive(Debug, PartialEq)]
pub struct Evaluation {
    pub mean: Vec<f64>,
    pub objective: Objective,
}
pub struct Family<'a> {
    outcome: &'a [f64],
    prior: ReferenceWeights<'a>,
    offset: Option<&'a [f64]>,
    terms: Vec<[f64; 2]>,
}
impl<'a> Family<'a> {
    pub fn new(
        outcome: &'a [f64],
        prior: ReferenceWeights<'a>,
        offset: Option<&'a [f64]>,
    ) -> Result<Self> {
        Self::new_polled(outcome, prior, offset, None)
    }
    pub(crate) fn new_polled(
        outcome: &'a [f64],
        prior: ReferenceWeights<'a>,
        offset: Option<&'a [f64]>,
        poll: Poll<'_>,
    ) -> Result<Self> {
        execution::check(poll)?;
        if outcome.len() != prior.nobs() || offset.is_some_and(|v| v.len() != outcome.len()) {
            return Err(invalid("Poisson input lengths differ"));
        }
        if matches!(
            prior.kind(),
            Some(WeightKind::Analytic | WeightKind::Importance)
        ) {
            return Err(FeregError::new(
                ErrorCode::UnsupportedWeight,
                "PPML supports only frequency and probability weights",
            ));
        }
        let mut terms = reserved(outcome.len())?;
        let mut positive = false;
        for (row, &y) in outcome.iter().enumerate() {
            execution::check_chunk(poll, row)?;
            if !y.is_finite() || y < 0. || offset.is_some_and(|values| !values[row].is_finite()) {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    format!("invalid Poisson outcome or offset at observation {row}"),
                ));
            }
            positive |= y > 0.;
            terms.push([if y > 0. { y.ln() } else { 0. }, math::log_gamma(y + 1.)]);
        }
        if !positive {
            return Err(FeregError::new(
                ErrorCode::InsufficientObservations,
                "Poisson sample has no positive outcome",
            ));
        }
        execution::check(poll)?;
        Ok(Self {
            outcome,
            prior,
            offset,
            terms,
        })
    }
    pub fn evaluate(&self, eta: &[f64], team: &ThreadTeam) -> Result<Evaluation> {
        if eta.len() != self.outcome.len() {
            return Err(invalid("Poisson predictor length differs"));
        }
        let mut mean = reserved(eta.len())?;
        mean.resize(eta.len(), 0.);
        let mut partials = reserved(eta.len().div_ceil(REDUCTION_CHUNK))?;
        partials.resize_with(eta.len().div_ceil(REDUCTION_CHUNK), || None);
        team.install(|| {
            mean.par_chunks_mut(REDUCTION_CHUNK)
                .zip(partials.par_iter_mut())
                .enumerate()
                .for_each(|(block, (means, output))| {
                    *output = Some(self.evaluate_block(eta, block * REDUCTION_CHUNK, means));
                });
        });
        let mut deviance = CompensatedSum::default();
        let mut likelihood = CompensatedSum::default();
        for partial in partials {
            let value = partial.expect("visited objective block")?;
            deviance.add(value.deviance);
            likelihood.add(value.log_likelihood);
        }
        let objective = Objective {
            deviance: deviance.total(),
            log_likelihood: likelihood.total(),
        };
        if !objective.deviance.is_finite() || !objective.log_likelihood.is_finite() {
            return Err(numerical("Poisson objective is not representable"));
        }
        Ok(Evaluation { mean, objective })
    }
    fn evaluate_block(&self, eta: &[f64], start: usize, mean: &mut [f64]) -> Result<Objective> {
        let mut deviance = CompensatedSum::default();
        let mut likelihood = CompensatedSum::default();
        for (local, mu) in mean.iter_mut().enumerate() {
            let row = start + local;
            *mu = mean_from_eta(eta[row])?;
            let y = self.outcome[row];
            let q = self.prior.value(row);
            let d = 2. * q * math::half_deviance(y, *mu, eta[row], Some(self.terms[row][0]))?;
            let ll = q * (y * eta[row] - *mu - self.terms[row][1]);
            if !d.is_finite() || !ll.is_finite() {
                return Err(numerical("Poisson row objective is not representable"));
            }
            deviance.add(d);
            likelihood.add(ll);
        }
        Ok(Objective {
            deviance: deviance.total(),
            log_likelihood: likelihood.total(),
        })
    }
}
pub fn mean_from_eta(eta: f64) -> Result<f64> {
    let mu = eta.exp();
    if !eta.is_finite() || !mu.is_finite() || mu <= 0. {
        Err(numerical(
            "Poisson mean is not finite and positive; reduce the proposed step",
        ))
    } else {
        Ok(mu)
    }
}
pub fn deviance_contribution(y: f64, mu: f64) -> Result<f64> {
    if !y.is_finite() || y < 0. {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "invalid Poisson outcome",
        ));
    }
    if !mu.is_finite() || mu <= 0. {
        return Err(numerical("invalid Poisson mean"));
    }
    let value = 2. * math::half_deviance(y, mu, mu.ln(), None)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(numerical("Poisson deviance contribution overflows"))
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
