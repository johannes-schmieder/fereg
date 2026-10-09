//! Production-type adapters for fereg-oracle's independent explicit-dummy
//! oracles. Inputs are copied into oracle-owned types; no production FE
//! operator, moment or solver participates in the oracle computation.

use fereg_core::{DenseMatrix, FeDesign};
use fereg_oracle::explicit_dummy::{self as oracle, design::ExplicitFeDesign};
use fereg_oracle::explicit_ols::{DummyEffect, OracleMatrix};

pub use fereg_oracle::explicit_dummy::{ExplicitDummyCertificate, ExplicitDummyProjectionAudit};

/// Copy a production FE design into the oracle's independent design type.
pub fn oracle_design(design: &FeDesign) -> Result<ExplicitFeDesign, String> {
    let effects = design
        .effects()
        .iter()
        .map(|effect| {
            DummyEffect::new(
                effect.codes().to_vec(),
                effect.n_levels(),
                effect.has_intercept(),
                effect.slopes().to_vec(),
            )
            .map_err(|error| format!("{error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    ExplicitFeDesign::new(design.nobs(), effects)
}

/// Copy a production row-major matrix into the oracle's matrix type.
pub fn oracle_matrix(matrix: &DenseMatrix) -> Result<OracleMatrix, String> {
    OracleMatrix::from_row_major(matrix.nrows(), matrix.ncols(), matrix.as_slice().to_vec())
        .map_err(|error| format!("{error:?}"))
}

/// See [`fereg_oracle::explicit_dummy::fe_remaining_projection_with_reference`].
pub fn fe_remaining_projection_with_reference(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    reference: &DenseMatrix,
) -> Result<ExplicitDummyProjectionAudit, String> {
    oracle::fe_remaining_projection_with_reference(
        &oracle_design(design)?,
        weights,
        &oracle_matrix(residuals)?,
        &oracle_matrix(reference)?,
    )
}

/// See [`fereg_oracle::explicit_dummy::fe_projection_certificate`].
pub fn fe_projection_certificate(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
) -> Result<ExplicitDummyCertificate, String> {
    oracle::fe_projection_certificate(&oracle_design(design)?, weights, &oracle_matrix(residuals)?)
}

/// See [`fereg_oracle::explicit_dummy::fe_projection_certificate_with_reference`].
pub fn fe_projection_certificate_with_reference(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    reference: &DenseMatrix,
) -> Result<ExplicitDummyCertificate, String> {
    oracle::fe_projection_certificate_with_reference(
        &oracle_design(design)?,
        weights,
        &oracle_matrix(residuals)?,
        &oracle_matrix(reference)?,
    )
}
