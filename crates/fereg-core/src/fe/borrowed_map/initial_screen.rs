//! The exact one-group initial rejection screen at solver entry
//! (ARCHITECTURE.md, "Initial FE rejection screen"). A rejection only skips
//! the zero-iteration shortcut's complete gather; it grants no certificate.
use super::*;

crate::fault::fault_plan! {
    // Test-only comparison arm: always run the complete initial certificate.
    static UNSCREENED: bool = false;
}

/// Whether the unchanged initial matrix provably fails its complete initial
/// certificate. Callers have just computed the reference norms from it.
pub(super) fn rejects(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<bool> {
    if UNSCREENED.get() {
        return Ok(false);
    }
    initial_group_rejects(design, weights, values, tolerance, workspace)
}

/// The complete initial certificate, or `None` when the screen proves that it
/// fails. A rejection is never published; it only skips the shortcut.
pub(super) fn initial_certificate(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<Option<FeCertificate>> {
    if rejects(design, weights, values, tolerance, workspace)? {
        return Ok(None);
    }
    certify_initial(design, weights, values, tolerance, workspace).map(Some)
}

/// The MAP wrapper's entry certificate: a structural constant keeps its own
/// complete certificate; intercept designs use [`initial_certificate`].
pub(super) fn entry_certificate(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<Option<FeCertificate>> {
    if design.effects().is_empty() {
        return certify_initial_structural_constant(weights, values, tolerance, workspace)
            .map(Some);
    }
    initial_certificate(design, weights, values, tolerance, workspace)
}

/// A bounded pilot's certificate: the screened initial certificate before the
/// first sweep, otherwise the complete certificate.
pub(super) fn pilot_certificate(
    initial: bool,
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<Option<FeCertificate>> {
    if initial {
        initial_certificate(design, weights, values, tolerance, workspace)
    } else {
        certify(design, weights, values, tolerance, workspace).map(Some)
    }
}

/// Run `function` with the complete initial certificate on this thread.
#[cfg(test)]
pub(super) fn unscreened<T>(function: impl FnOnce() -> T) -> T {
    UNSCREENED.set(true);
    let result = function();
    UNSCREENED.set(false);
    result
}
