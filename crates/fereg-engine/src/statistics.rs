//! Estimator-specific fit statistics on one accepted reference experiment.
mod ppml;
use crate::{
    data::ReferenceWeights,
    dof::ResidualDof,
    error::{ErrorCode, FeregError, Result},
    execution::{self, Poll},
    spec::Estimator,
};
pub(crate) use ppml::ppml_polled;
pub use ppml::{ppml, PpmlStatistics};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OlsStatistics {
    pub rss: f64,
    pub tss: f64,
    pub within_tss: f64,
    pub mss: f64,
    pub r_squared: Option<f64>,
    pub adjusted_r_squared: Option<f64>,
    pub within_r_squared: Option<f64>,
    pub adjusted_within_r_squared: Option<f64>,
    pub rmse: f64,
    pub log_likelihood: Option<f64>,
    pub null_log_likelihood: Option<f64>,
}
pub fn ols(
    outcome: &[f64],
    within_outcome: &[f64],
    residuals: &[f64],
    weights: ReferenceWeights<'_>,
    structural_intercept: bool,
    dof: ResidualDof,
) -> Result<OlsStatistics> {
    ols_polled(
        outcome,
        within_outcome,
        residuals,
        weights,
        structural_intercept,
        dof,
        None,
    )
}
pub(crate) fn ols_polled(
    outcome: &[f64],
    within_outcome: &[f64],
    residuals: &[f64],
    weights: ReferenceWeights<'_>,
    structural_intercept: bool,
    dof: ResidualDof,
    poll: Poll<'_>,
) -> Result<OlsStatistics> {
    execution::check(poll)?;
    let n = weights.nobs();
    if dof.estimator != Estimator::Ols
        || dof.effective != weights.effective()
        || [outcome, within_outcome, residuals]
            .iter()
            .any(|x| x.len() != n)
        || execution::any(
            outcome.iter().chain(within_outcome).chain(residuals),
            |v| !v.is_finite(),
            poll,
        )?
    {
        return Err(invalid("OLS statistic inputs are not final-sample aligned"));
    }
    let mean = if structural_intercept {
        weights.mean_polled(outcome, poll)?
    } else {
        0.
    };
    let rss = squares_polled(residuals, 0., weights, poll)?;
    let tss = squares_polled(outcome, mean, weights, poll)?;
    let within_tss = squares_polled(within_outcome, 0., weights, poll)?;
    let n = dof.effective as f64;
    let fit_df = dof.fit as f64;
    let centered_df = n - f64::from(u8::from(structural_intercept));
    let within_df = dof
        .fit
        .checked_add(dof.finite)
        .ok_or_else(|| invalid("within baseline DoF overflow"))? as f64;
    let result = OlsStatistics {
        rss,
        tss,
        within_tss,
        mss: tss - rss,
        r_squared: ratio(rss, tss)?,
        within_r_squared: ratio(rss, within_tss)?,
        adjusted_r_squared: if centered_df > 0. {
            ratio(rss / fit_df, tss / centered_df)?
        } else {
            None
        },
        adjusted_within_r_squared: ratio(rss / fit_df, within_tss / within_df)?,
        rmse: (rss / fit_df).sqrt(),
        log_likelihood: gaussian(rss, n)?,
        null_log_likelihood: gaussian(within_tss, n)?,
    };
    if !result.rmse.is_finite() || !result.mss.is_finite() {
        return Err(invalid("OLS statistic is not representable"));
    }
    execution::check(poll)?;
    Ok(result)
}
pub(crate) fn squares_polled(
    values: &[f64],
    center: f64,
    weights: ReferenceWeights<'_>,
    poll: Poll<'_>,
) -> Result<f64> {
    execution::check(poll)?;
    // Normalize before squaring, then undo the scale without forming scale² first.
    let scale = execution::fold(
        values.iter().map(|v| (*v - center).abs()),
        0.,
        poll,
        f64::max,
    )?;
    if scale == 0. {
        return Ok(0.);
    }
    let sum = execution::blocked_sum(
        values.len(),
        |i| {
            let root = weights.value(i).sqrt() * ((values[i] - center) / scale);
            root * root
        },
        poll,
    )?;
    let result = (sum.sqrt() * scale).powi(2);
    if result.is_finite() {
        Ok(result)
    } else {
        Err(invalid("weighted sum of squares is not representable"))
    }
}
fn ratio(numerator: f64, denominator: f64) -> Result<Option<f64>> {
    if denominator == 0. {
        return Ok(None);
    }
    let value = 1. - numerator / denominator;
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err(invalid("fit ratio is not representable"))
    }
}
fn gaussian(ss: f64, n: f64) -> Result<Option<f64>> {
    if ss == 0. {
        return Ok(None);
    }
    let value = -0.5 * n * (1. + (2. * std::f64::consts::PI).ln() + ss.ln() - n.ln());
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err(invalid("Gaussian likelihood is not representable"))
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

#[cfg(test)]
mod interruption_tests;
