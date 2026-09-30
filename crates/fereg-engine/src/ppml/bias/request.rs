use super::invalid;
use crate::error::Result;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Exact,
    Randomized { probes: u32, seed: u32, stream: u32 },
}
impl Method {
    #[must_use]
    pub const fn randomized_default() -> Self {
        Self::Randomized {
            probes: 256,
            seed: 739391,
            stream: 0,
        }
    }
    pub(crate) fn validate(self) -> Result<()> {
        if matches!(self, Self::Randomized { probes: 0 | 1, .. }) {
            return Err(invalid(
                "randomized bias correction requires at least two probes",
            ));
        }
        Ok(())
    }
    pub(super) fn dimensions(self, rows: usize, units: usize) -> Result<(usize, usize)> {
        self.validate()?;
        match self {
            Self::Exact => {
                if units > 10_000 || rows as u128 * units as u128 > 1_000_000_000 {
                    return Err(invalid(
                        "exact bias correction exceeds its reference work bound; use randomized",
                    ));
                }
                Ok((units, 1))
            }
            Self::Randomized { probes, .. } => Ok((probes as usize, 8.min(probes as usize))),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub save_effects: bool,
}
/// The same anchor applies to all nine channels, including numerical variation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Channel {
    Raw,
    Derivative,
    Curvature,
    Total,
    Corrected,
    SeDerivative,
    SeCurvature,
    Covariance,
    SeTotal,
}
impl Channel {
    pub const ALL: [Self; 9] = [
        Self::Raw,
        Self::Derivative,
        Self::Curvature,
        Self::Total,
        Self::Corrected,
        Self::SeDerivative,
        Self::SeCurvature,
        Self::Covariance,
        Self::SeTotal,
    ];
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Derivative => "d",
            Self::Curvature => "c",
            Self::Total => "total",
            Self::Corrected => "bc",
            Self::SeDerivative => "sed",
            Self::SeCurvature => "sec",
            Self::Covariance => "covdc",
            Self::SeTotal => "setotal",
        }
    }
    #[must_use]
    pub const fn matrix_name(self) -> &'static str {
        match self {
            Self::Raw => "b_bias_raw",
            Self::Derivative => "b_bias_d",
            Self::Curvature => "b_bias_c",
            Self::Total => "b_bias_total",
            Self::Corrected => "b_bc",
            Self::SeDerivative => "bias_mcse_d",
            Self::SeCurvature => "bias_mcse_c",
            Self::Covariance => "bias_cov_dc",
            Self::SeTotal => "bias_mcse_total",
        }
    }
}
