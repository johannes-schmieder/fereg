//! Transactional objective acceptance; publication still requires strict scientific checks.
use super::family::{Evaluation, Family, StateCertificate};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    fe::Design,
    threads::ThreadTeam,
};
const MAX_HALVINGS: usize = 25;
const OBJECTIVE_SLACK: f64 = 1e-12;
#[derive(Debug, PartialEq)]
pub struct Point {
    pub finite: Vec<f64>,
    pub absorbed: DenseMatrix,
    pub eta: Vec<f64>,
}
pub struct Accepted<'family, 'data> {
    family: &'family Family<'data>,
    point: Point,
    evaluation: Evaluation,
    step: f64,
    halvings: usize,
}
impl<'family, 'data> Accepted<'family, 'data> {
    /// The estimator owns the coefficient-to-predictor reconstruction check.
    pub fn start(family: &'family Family<'data>, point: Point, team: &ThreadTeam) -> Result<Self> {
        point.validate()?;
        let evaluation = family.evaluate(&point.eta, team)?;
        Ok(Self {
            family,
            point,
            evaluation,
            step: 0.,
            halvings: 0,
        })
    }
    pub fn try_step(&self, candidate: &Point, team: &ThreadTeam) -> Result<Self> {
        candidate.validate()?;
        if candidate.finite.len() != self.point.finite.len()
            || candidate.absorbed.nrows() != self.point.absorbed.nrows()
            || candidate.eta.len() != self.point.eta.len()
        {
            return Err(invalid(
                "PPML trial and accepted parameter dimensions differ",
            ));
        }
        let current = self.evaluation.objective.deviance;
        let slack = OBJECTIVE_SLACK * current.abs().max(1.);
        let mut step = 1.;
        for halvings in 0..=MAX_HALVINGS {
            let point = match self.point.interpolate(candidate, step) {
                Ok(point) => point,
                Err(error) if error.code == ErrorCode::NonConvergence => {
                    step *= 0.5;
                    continue;
                }
                Err(error) => return Err(error),
            };
            match self.family.evaluate(&point.eta, team) {
                Ok(evaluation)
                    if evaluation.objective.deviance <= current
                        || evaluation.objective.deviance - current <= slack =>
                {
                    return Ok(Self {
                        family: self.family,
                        point,
                        evaluation,
                        step,
                        halvings,
                    });
                }
                Ok(_) => {}
                Err(error) if error.code == ErrorCode::NonConvergence => {}
                Err(error) => return Err(error),
            }
            step *= 0.5;
        }
        Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML step-halving exhausted 25 halvings without an acceptable finite objective",
        ))
    }
    #[must_use]
    pub fn point(&self) -> &Point {
        &self.point
    }
    #[must_use]
    pub fn evaluation(&self) -> &Evaluation {
        &self.evaluation
    }
    #[must_use]
    pub fn step(&self) -> f64 {
        self.step
    }
    #[must_use]
    pub fn halvings(&self) -> usize {
        self.halvings
    }
    pub fn certify_state(
        &self,
        raw: &DenseMatrix,
        fe: &Design<'_>,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<StateCertificate> {
        self.family
            .certify_state(&self.point, &self.evaluation, raw, fe, tolerance, team)
    }
}
impl Point {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.absorbed.ncols() != 1 {
            return Err(invalid("PPML FE parameters require one coefficient RHS"));
        }
        if self
            .finite
            .iter()
            .chain(self.absorbed.as_slice())
            .chain(&self.eta)
            .any(|v| !v.is_finite())
        {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML parameter point is nonfinite",
            ));
        }
        Ok(())
    }
    fn interpolate(&self, candidate: &Self, step: f64) -> Result<Self> {
        let finite = interpolate(&self.finite, &candidate.finite, step)?;
        let values = interpolate(
            self.absorbed.as_slice(),
            candidate.absorbed.as_slice(),
            step,
        )?;
        let absorbed = DenseMatrix::from_row_major(self.absorbed.nrows(), 1, values)?;
        let eta = interpolate(&self.eta, &candidate.eta, step)?;
        Ok(Self {
            finite,
            absorbed,
            eta,
        })
    }
}
fn interpolate(current: &[f64], candidate: &[f64], step: f64) -> Result<Vec<f64>> {
    let mut result = reserved(current.len())?;
    for (&old, &new) in current.iter().zip(candidate) {
        // Preserve the exact full proposal; use a convex form only when the
        // difference of two finite endpoints itself overflows.
        let value = if step == 1. {
            new
        } else {
            let difference = new - old;
            if difference.is_finite() {
                old + step * difference
            } else {
                (1. - step) * old + step * new
            }
        };
        if !value.is_finite() {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML interpolated parameter is not representable",
            ));
        }
        result.push(value);
    }
    Ok(result)
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
