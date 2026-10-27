//! Certificate of a residual formed from certified projections (SPEC §16).
//!
//! Fast OLS forms its final residual as `r = ỹ - X̃b` from certified within
//! columns. Linearity bounds its local-span moments by the combination of the
//! columns' certified moments, so this certificate recomputes them from the
//! declared design and weights and accepts them at the scale of the combined
//! terms, `s = ||sqrt(W) ỹ|| + Σ|b_j| ||sqrt(W) x̃_j||`, instead of solving
//! again. The caller passes `s / ||sqrt(W) r||` (at least one by the triangle
//! inequality), so a common weight normalization cancels. A failure leaves the
//! caller's projection path.

use super::*;

impl ReusableCategoricalMapAbsorber<'_> {
    pub(crate) fn certify_derived(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: &DenseMatrix,
        tolerance: f64,
        amplification: f64,
    ) -> Result<FeCertificate> {
        if values.nrows() != self.design.nobs()
            || values.ncols() != 1
            || !amplification.is_finite()
            || amplification < 1.0
            || !(tolerance.is_finite() && tolerance > 0.0)
        {
            return Err(invalid_argument(
                "derived residual certificate dimensions, scale or tolerance are invalid",
            ));
        }
        if values.as_slice().iter().any(|value| !value.is_finite()) {
            return Err(invalid_data("derived residual values must be finite"));
        }
        let weights = validate_weights(weights, values.nrows())?;
        self.workspace.set_rhs_count(1)?;
        update_workspace_norms(values, weights, &mut self.workspace, false)?;
        let scale = amplification * self.workspace.residual_norms[0];
        if !scale.is_finite() {
            return Err(invalid_data("derived residual scale is not finite"));
        }
        self.workspace.reference_norms[0] = scale;
        self.with_pilot_group_scratch(1, |design, workspace| {
            certify_with_current_norms(design, weights, values, tolerance, workspace)
        })
    }
}
