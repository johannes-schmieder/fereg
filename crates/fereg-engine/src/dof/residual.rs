//! Distinct structural, fit-statistic and reported-test DoF (SPEC §9).
use crate::{
    data::exact_count,
    error::{ErrorCode, FeregError, Result},
    spec::Estimator,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResidualDof {
    pub effective: usize,
    pub finite: usize,
    pub absorbed: usize,
    pub nested: usize,
    pub structural: usize,
    pub reported: usize,
    pub fit: usize,
    pub correction: usize,
    pub minimum_clusters: Option<usize>,
    pub estimator: Estimator,
}
impl ResidualDof {
    /// `finite` is OLS model DoF or PPML within rank. The caller assigns each
    /// structural constant direction once, across finite and absorbed counts.
    pub fn new(
        estimator: Estimator,
        effective: usize,
        finite: usize,
        absorbed: usize,
        nested: usize,
        cluster_counts: &[usize],
    ) -> Result<Self> {
        exact_count(effective)?;
        let maximum = if estimator == Estimator::Ols { 10 } else { 5 };
        if cluster_counts.len() > maximum || cluster_counts.iter().any(|g| *g < 2 || *g > effective)
        {
            return Err(insufficient("invalid final cluster counts"));
        }
        if cluster_counts.is_empty() && nested != 0 {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "nested categories require clusters",
            ));
        }
        let structural = positive_sub(positive_sub(effective, finite)?, absorbed)?;
        let minimum_clusters = cluster_counts.iter().copied().min();
        let reported = minimum_clusters.map_or(structural, |g| structural.min(g - 1));
        let (fit, correction) = if estimator == Estimator::Ols {
            (
                positive_sub(structural, nested)?,
                positive_sub(structural, usize::from(nested > 0))?,
            )
        } else {
            (structural, structural)
        };
        Ok(Self {
            effective,
            finite,
            absorbed,
            nested,
            structural,
            reported,
            fit,
            correction,
            minimum_clusters,
            estimator,
        })
    }
    pub fn robust_multiplier(self) -> Result<f64> {
        let denominator = if self.estimator == Estimator::Ols {
            self.structural
        } else {
            positive_sub(self.effective, 1)?
        };
        Ok(self.effective as f64 / denominator as f64)
    }
    pub fn cluster_multiplier(self) -> Result<f64> {
        let g = self
            .minimum_clusters
            .ok_or_else(|| insufficient("cluster correction requires clusters"))?;
        let ratio = g as f64 / (g - 1) as f64;
        Ok(if self.estimator == Estimator::Ols {
            (self.effective - 1) as f64 / self.correction as f64 * ratio
        } else {
            ratio
        })
    }
}
fn positive_sub(left: usize, right: usize) -> Result<usize> {
    left.checked_sub(right)
        .filter(|x| *x > 0)
        .ok_or_else(|| insufficient("nonpositive residual degrees of freedom"))
}
fn insufficient(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InsufficientObservations, message)
}
