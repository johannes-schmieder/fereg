//! Stable inference coordinates used only after exceptional rank repair.
use super::*;

/// Map quotient scores [within X, 1] into the reported parameter axis.
/// With finite bread C and original weighted means mu, J is
/// [[C, 0], [-mu' C, 1 / sum(w)]]. Unlike the public normalized-intercept
/// bread, J never recovers small slope terms by subtracting large FE means.
pub(super) fn build_bread<R: PreparedInferenceReservation>(
    fit: &mut CertifiedPreparedOlsFit,
    intercept: Option<usize>,
    memory_budget: Option<crate::memory::FeSolverMemoryBudget>,
    reservation: &mut R,
) -> Result<Option<Vec<f64>>> {
    if !fit.uses_quotient_inference() || intercept.is_none() {
        return Ok(None);
    }
    let intercept = intercept.expect("normalized quotient intercept");
    let dimension = fit.bread().dimension();
    let square = dimension
        .checked_mul(dimension)
        .ok_or_else(|| allocation("quotient inference map dimensions overflow"))?;
    let retained = fit
        .recovered_fe_coefficients()
        .map_or(0, |batch| batch.retained_bytes());
    let bytes = (square as u128 * 8)
        .checked_add(retained)
        .ok_or_else(|| allocation("quotient inference workspace overflows"))?;
    // FE sessions have ended. The original inference frame excludes their
    // products; admit the actual returned FE batch alongside this new map.
    if memory_budget.is_some_and(|budget| !budget.fits(bytes)) {
        return Err(allocation(
            "quotient inference map exceeds the requested memory limit",
        ));
    }
    let mut map = reserved_filled(
        reservation,
        square,
        0.,
        "unable to reserve quotient inference map",
    )?;
    map.copy_from_slice(fit.bread().values());
    for row in 0..dimension {
        crate::interrupt::checkpoint()?;
        map[row * dimension + intercept] = 0.;
    }
    let mass = fit.bread().weight_scale().reference_weight_sum;
    if !mass.is_finite() || mass <= 0. {
        return Err(invalid_data("quotient inference weight mass is invalid"));
    }
    map[intercept * dimension + intercept] = 1. / mass;
    fit.record_inference_workspace(bytes)?;
    Ok(Some(map))
}

#[cfg(test)]
mod tests;
