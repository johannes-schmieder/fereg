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
//!
//! The two-FE categorical owner checks on its retained workspace. The direct
//! one-FE and many-FE binding routes have no retained owner, so they check on a
//! one-RHS workspace over the same borrowed design; both recompute the moments
//! with the borrowed MAP certificate, never from a solver's own norms.

use super::*;

crate::fault::fault_plan! {
    static FAIL_DERIVED_CERTIFICATE_ONCE: bool = false;
}

/// Arm one failed derived certificate on this thread (test fault plan).
#[cfg(test)]
pub(crate) fn inject_derived_certificate_failure_once() {
    FAIL_DERIVED_CERTIFICATE_ONCE.set(true);
}

fn validate(values: &DenseMatrix, nobs: usize, tolerance: f64, amplification: f64) -> Result<()> {
    if values.nrows() != nobs
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
    Ok(())
}

/// Set the derived scale, `amplification` times the weighted norm of
/// `values`, as the reference norm of a workspace with one active RHS.
fn set_derived_scale(
    values: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    amplification: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    update_workspace_norms(values, weights, workspace, false)?;
    let scale = amplification * workspace.residual_norms[0];
    if !scale.is_finite() {
        return Err(invalid_data("derived residual scale is not finite"));
    }
    workspace.reference_norms[0] = scale;
    Ok(())
}

/// The recomputed §16 moments at the scale set by [`set_derived_scale`].
fn certify_at_derived_scale(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    let mut certificate =
        certify_with_current_norms(design, weights, values, tolerance, workspace)?;
    if FAIL_DERIVED_CERTIFICATE_ONCE.replace(false) {
        certificate.passed = false;
    }
    Ok(certificate)
}

impl ReusableCategoricalMapAbsorber<'_> {
    pub(crate) fn certify_derived(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: &DenseMatrix,
        tolerance: f64,
        amplification: f64,
    ) -> Result<FeCertificate> {
        validate(values, self.design.nobs(), tolerance, amplification)?;
        let weights = validate_weights(weights, values.nrows())?;
        self.workspace.set_rhs_count(1)?;
        set_derived_scale(values, weights, amplification, &mut self.workspace)?;
        self.with_pilot_group_scratch(1, |design, workspace| {
            certify_at_derived_scale(design, weights, values, tolerance, workspace)
        })
    }
}

/// The derived certificate over a direct binding design (one FE or the
/// many-FE route), or `None` when its one-RHS workspace exceeds the admitted
/// solver budget and the caller keeps its projection path.
pub(crate) fn certify_binding_derived(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
    amplification: f64,
    budget: Option<FeSolverMemoryBudget>,
) -> Result<Option<FeCertificate>> {
    validate(values, design.nobs(), tolerance, amplification)?;
    if design.effects().is_empty() {
        return Err(invalid_argument(
            "derived residual certificate needs an absorbed effect",
        ));
    }
    let weights = validate_weights(weights, values.nrows())?;
    let bytes = BorrowedMapWorkspace::planned_payload_bytes(
        design,
        values.nrows(),
        1,
        rayon::current_num_threads(),
    )?;
    if budget.is_some_and(|b| !b.fits(bytes)) {
        return Ok(None);
    }
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), 1, &mut SystemWorkspaceReservation)?;
    set_derived_scale(values, weights, amplification, &mut workspace)?;
    certify_at_derived_scale(design, weights, values, tolerance, &mut workspace).map(Some)
}
