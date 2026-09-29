//! Reference-scale OLS fit statistics. Undefined quantities remain absent.
use crate::{
    data::ReferenceWeights,
    dof::ResidualDof,
    error::{ErrorCode, FeregError, Result},
    spec::Estimator,
    threads::blocked_sum,
};
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
    let n = weights.nobs();
    if dof.estimator != Estimator::Ols
        || dof.effective != weights.effective()
        || [outcome, within_outcome, residuals]
            .iter()
            .any(|x| x.len() != n || x.iter().any(|v| !v.is_finite()))
    {
        return Err(invalid("OLS statistic inputs are not final-sample aligned"));
    }
    let mean = if structural_intercept {
        weighted_mean(outcome, weights)?
    } else {
        0.
    };
    let rss = squares(residuals, 0., weights)?;
    let tss = squares(outcome, mean, weights)?;
    let within_tss = squares(within_outcome, 0., weights)?;
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
    Ok(result)
}
pub(crate) fn squares(values: &[f64], center: f64, weights: ReferenceWeights<'_>) -> Result<f64> {
    // Normalize before squaring, then undo the scale without forming scale² first.
    let scale = values
        .iter()
        .map(|v| (*v - center).abs())
        .fold(0., f64::max);
    if scale == 0. {
        return Ok(0.);
    }
    let sum = blocked_sum(values.len(), |i| {
        let root = weights.value(i).sqrt() * ((values[i] - center) / scale);
        root * root
    });
    let result = (sum.sqrt() * scale).powi(2);
    if result.is_finite() {
        Ok(result)
    } else {
        Err(invalid("weighted sum of squares is not representable"))
    }
}
fn weighted_mean(values: &[f64], weights: ReferenceWeights<'_>) -> Result<f64> {
    let scale = values.iter().copied().map(f64::abs).fold(0., f64::max);
    if scale == 0. {
        return Ok(0.);
    }
    let sum_w = blocked_sum(values.len(), |i| weights.value(i));
    let mean = blocked_sum(values.len(), |i| {
        weights.value(i) / sum_w * (values[i] / scale)
    }) * scale;
    if mean.is_finite() {
        Ok(mean)
    } else {
        Err(invalid("weighted mean is not representable"))
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
