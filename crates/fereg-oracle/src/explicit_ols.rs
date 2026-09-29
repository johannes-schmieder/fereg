//! Harvested from fereg-testkit/src/explicit_ols.rs at legacy-v0.5.0-beta.
//! Independent dense oracle; no production numerical implementation is called.

//! Independent small-model explicit-dummy OLS/WLS oracle.
//!
//! The implementation deliberately does not call the production OLS, VCE, or
//! degrees-of-freedom code. It materializes every nuisance dummy and slope
//! column, applies weighted Frisch--Waugh--Lovell projections with a scaled
//! column-pivoted Householder QR, and fails closed when its numerical
//! certificates do not pass.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::ops::Index;

/// One explicitly materialized absorbed effect.
#[derive(Debug, Clone, PartialEq)]
pub struct DummyEffect {
    codes: Vec<u32>,
    n_levels: usize,
    intercept: bool,
    slopes: Vec<Vec<f64>>,
}

impl DummyEffect {
    /// Construct an absorbed term. Each level gets an intercept column when
    /// `intercept` is true and one column per supplied slope.
    pub fn new(
        codes: Vec<u32>,
        n_levels: usize,
        intercept: bool,
        slopes: Vec<Vec<f64>>,
    ) -> Result<Self, OracleError> {
        if n_levels == 0 {
            return Err(OracleError::InvalidInput(
                "an absorbed effect must declare at least one level".to_owned(),
            ));
        }
        if !intercept && slopes.is_empty() {
            return Err(OracleError::InvalidInput(
                "an absorbed effect must contain an intercept or a slope".to_owned(),
            ));
        }
        if codes.iter().any(|&code| code as usize >= n_levels) {
            return Err(OracleError::InvalidInput(
                "an absorbed-effect code exceeds its declared level count".to_owned(),
            ));
        }
        if slopes.iter().any(|slope| slope.len() != codes.len()) {
            return Err(OracleError::InvalidInput(
                "absorbed slope and code lengths differ".to_owned(),
            ));
        }
        if slopes.iter().flatten().any(|value| !value.is_finite()) {
            return Err(OracleError::InvalidInput(
                "absorbed slopes must be finite".to_owned(),
            ));
        }
        Ok(Self {
            codes,
            n_levels,
            intercept,
            slopes,
        })
    }

    #[must_use]
    pub fn codes(&self) -> &[u32] {
        &self.codes
    }

    #[must_use]
    pub const fn n_levels(&self) -> usize {
        self.n_levels
    }

    #[must_use]
    pub const fn has_intercept(&self) -> bool {
        self.intercept
    }

    #[must_use]
    pub fn slopes(&self) -> &[Vec<f64>] {
        &self.slopes
    }
}

/// Weight semantics used by the oracle.
#[derive(Debug, Clone, PartialEq)]
pub enum OracleWeights {
    /// Ordinary least squares.
    Unit,
    /// Positive precision weights for the objective `sum(w_i e_i^2)`.
    Wls(Vec<f64>),
    /// Positive integer row multiplicities. HC1 treats the repetitions as
    /// separate observations, while cluster scores aggregate all repetitions.
    Frequency(Vec<u32>),
}

/// Complete input to the independent oracle. Regressors are column-major and
/// no intercept is added implicitly.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplicitOlsProblem {
    pub outcome: Vec<f64>,
    pub regressors: Vec<Vec<f64>>,
    pub effects: Vec<DummyEffect>,
    pub weights: OracleWeights,
    /// Zero to ten declared cluster dimensions.
    pub clusters: Vec<Vec<u64>>,
}

/// Numerical policy for the small-model oracle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OracleConfig {
    /// Multiplier on `eps * max(n, p)` after every design column is scaled to
    /// unit two-norm.
    pub rank_tolerance_multiplier: f64,
    /// Maximum normalized normal-equation or inverse certificate.
    pub certificate_tolerance: f64,
}

impl Default for OracleConfig {
    fn default() -> Self {
        Self {
            rank_tolerance_multiplier: 128.0,
            certificate_tolerance: 5e-10,
        }
    }
}

/// Dense row-major matrix returned for identified regressors only. The
/// corresponding original regressor indices are in `kept_regressors`.
#[derive(Debug, Clone, PartialEq)]
pub struct OracleMatrix {
    nrows: usize,
    ncols: usize,
    data: Vec<f64>,
}

impl OracleMatrix {
    pub fn from_row_major(nrows: usize, ncols: usize, data: Vec<f64>) -> Result<Self, OracleError> {
        if nrows.checked_mul(ncols) != Some(data.len()) {
            return Err(OracleError::InvalidInput(
                "oracle matrix dimensions disagree".into(),
            ));
        }
        Ok(Self { nrows, ncols, data })
    }

    fn zeros(nrows: usize, ncols: usize) -> Self {
        Self {
            nrows,
            ncols,
            data: vec![0.0; nrows * ncols],
        }
    }

    #[must_use]
    pub const fn nrows(&self) -> usize {
        self.nrows
    }

    #[must_use]
    pub const fn ncols(&self) -> usize {
        self.ncols
    }

    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }
}

impl Index<(usize, usize)> for OracleMatrix {
    type Output = f64;

    fn index(&self, (row, column): (usize, usize)) -> &Self::Output {
        &self.data[row * self.ncols + column]
    }
}

/// Independent numerical certificates checked before a result is returned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OracleCertificate {
    pub normalized_normal_equation_infinity: f64,
    pub bread_inverse_infinity: f64,
    pub reconstruction_infinity: f64,
}

/// Explicit-dummy OLS/WLS result. Coefficients are `None` for finite
/// regressors that are collinear with the dummies or earlier identified
/// columns. Fitted values and residuals remain identified.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplicitOlsResult {
    pub coefficients: Vec<Option<f64>>,
    pub kept_regressors: Vec<usize>,
    pub dropped_regressors: Vec<usize>,
    pub explicit_dummy_columns: usize,
    pub explicit_dummy_rank: usize,
    pub finite_regressor_rank: usize,
    pub model_rank: usize,
    pub effective_observations: usize,
    pub residual_degrees_of_freedom: usize,
    pub finite_fitted_values: Vec<f64>,
    pub absorbed_fitted_values: Vec<f64>,
    pub fitted_values: Vec<f64>,
    pub residuals: Vec<f64>,
    pub weighted_rss: f64,
    pub bread: OracleMatrix,
    pub iid_vce: OracleMatrix,
    pub hc1_vce: OracleMatrix,
    /// HC1 covariance on `[identified finite regressors, _cons]` when the
    /// explicit nuisance-dummy span contains a structural constant. The
    /// reported constant uses the pinned zero-weighted-mean FE normalization.
    pub normalized_reported_intercept_hc1_vce: Option<OracleMatrix>,
    /// Multiway clustered covariance on `[identified finite regressors,
    /// _cons]` under the same pinned normalization.
    pub normalized_reported_intercept_clustered_vce: Option<OracleMatrix>,
    /// CR1 VCE for each individual cluster dimension.
    pub one_way_cluster_vces: Vec<OracleMatrix>,
    /// Generic inclusion--exclusion CR1 covariance using one common correction
    /// after all uncorrected subset meats are combined.
    pub clustered_vce: Option<OracleMatrix>,
    pub cluster_counts: Vec<usize>,
    pub certificate: OracleCertificate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleError {
    InvalidInput(String),
    InsufficientDegreesOfFreedom(String),
    NumericalFailure(String),
}

impl Display for OracleError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "invalid oracle input: {message}"),
            Self::InsufficientDegreesOfFreedom(message) => {
                write!(
                    formatter,
                    "insufficient oracle degrees of freedom: {message}"
                )
            }
            Self::NumericalFailure(message) => {
                write!(formatter, "oracle numerical certificate failed: {message}")
            }
        }
    }
}

impl Error for OracleError {}

mod fit;
mod inference;
mod qr;
mod validation;
pub use fit::{fit_explicit_ols, fit_explicit_ols_with_config};
use inference::*;
use qr::*;
use validation::*;
