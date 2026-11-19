//! Bounded replay after the independently formed OLS score design fails.
use super::PreparedOlsScoreProduct;
use crate::error::{ErrorCode, Result};
use crate::ols::prepared_fit::{
    fit_prepared_ols_coefficients_view_map_for_inference_with_loaded_moments,
    InferenceProjectionWork,
};
use crate::ols::prepared_fit_view::PreparedOlsFitView;
use crate::ols::prepared_products::CertifiedPreparedOlsInference;
use crate::ols::{CertifiedPreparedOlsFit, LoadedOlsRows, PreparedOlsFitConfig};

pub(super) const SCORE_DESIGN_CERTIFICATE_FAILED: &str =
    "prepared robust score-design certificate did not pass";

pub(in crate::ols) fn fit_with_score_retry<P, A, D, C, K, M>(
    prepared: &P,
    mut config: PreparedOlsFitConfig,
    rows: Option<LoadedOlsRows>,
    mut compose: impl FnMut(
        CertifiedPreparedOlsFit,
        PreparedOlsScoreProduct,
    ) -> Result<CertifiedPreparedOlsInference<A, D, C, K, M>>,
) -> Result<CertifiedPreparedOlsInference<A, D, C, K, M>>
where
    P: PreparedOlsFitView + Sync + ?Sized,
{
    let (fit, scores) = fit_prepared_ols_coefficients_view_map_for_inference_with_loaded_moments(
        prepared, config, rows, false,
    )?;
    let work: [InferenceProjectionWork; 2] = [
        fit.absorption_report().into(),
        fit.residual_absorption_report().into(),
    ];
    let original = match compose(fit, scores) {
        Ok(result) => return Ok(result),
        Err(error)
            if error.code == ErrorCode::InvalidData
                && error.message == SCORE_DESIGN_CERTIFICATE_FAILED =>
        {
            error
        }
        Err(error) => return Err(error),
    };
    // All failed fit and score owners have been dropped. The original row
    // mask, supplied model, requested solver and rank policy are unchanged.
    // Use the same projection ceiling as the established IID recovery.
    crate::interrupt::checkpoint()?;
    config.absorb.map.tolerance = config.absorb.map.tolerance.min(1e-14);
    config.absorb.lsmr.tolerance = config.absorb.lsmr.tolerance.min(1e-14);
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!("FEREG_SOLVER_TRACE|phase=ols_score_retry|projection=strict|moments=compensated");
    }
    let replay = fit_prepared_ols_coefficients_view_map_for_inference_with_loaded_moments(
        prepared, config, None, true,
    )
    .and_then(|(fit, scores)| compose(fit, scores));
    let mut result = match replay {
        Ok(result) => result,
        Err(error)
            if matches!(
                error.code,
                ErrorCode::InvalidData | ErrorCode::NonConvergence
            ) =>
        {
            if crate::diagnostics::solver_trace_enabled() {
                eprintln!("FEREG_SOLVER_TRACE|phase=ols_score_retry_failed|error={error}");
            }
            return Err(original);
        }
        Err(error) => return Err(error),
    };
    result.fit.record_inference_retry_work(work)?;
    Ok(result)
}
