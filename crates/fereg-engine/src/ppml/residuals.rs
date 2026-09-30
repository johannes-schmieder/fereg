//! SPEC/ADR 0016 residuals on the raw outcome scale, independent of prior weights.
use super::family::deviance_contribution;
use crate::error::{ErrorCode, FeregError, Result};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ResidualKind {
    #[default]
    Response,
    Score,
    Pearson,
    Working,
    Deviance,
    Anscombe,
}
pub fn value(y: f64, mu: f64, kind: ResidualKind) -> Result<f64> {
    if !y.is_finite() || y < 0. || !mu.is_finite() || mu <= 0. {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML residual is outside the Poisson domain",
        ));
    }
    let raw = y - mu;
    let value = match kind {
        ResidualKind::Response | ResidualKind::Score => raw,
        ResidualKind::Pearson => raw / mu.sqrt(),
        ResidualKind::Working => raw / mu,
        ResidualKind::Deviance => deviance_contribution(y, mu)?,
        ResidualKind::Anscombe => {
            let cube = mu.cbrt();
            1.5 * (y.cbrt().powi(2) - cube.powi(2)) / cube.sqrt()
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(FeregError::new(
            ErrorCode::NonConvergence,
            format!("PPML {kind:?} residual is not representable"),
        ))
    }
}
