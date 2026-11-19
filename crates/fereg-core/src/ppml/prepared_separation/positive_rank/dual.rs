//! Last-resort exact support search on small generations. Giving the
//! unrestricted positive-outcome rows pivot priority avoids imposing arbitrary
//! ratios on their weights. The zero rows must all enter the final sign test.
use super::*;

pub(in super::super) fn exact_dual(
    method: SeparationMethod,
    model: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    positive: usize,
    limit: usize,
    config: SeparationConfig,
) -> Result<Option<DetectionProposal>> {
    let n = model.outcome().len();
    let boundary_rows = n - positive;
    // Modular echelon, ids, column, free weights and reconstruction scratch.
    let metadata = topology.effect_specs.iter().fold(0usize, |sum, effect| {
        sum.saturating_add(7)
            .saturating_add(effect.slopes.len().saturating_mul(2))
    });
    let words = n
        .saturating_mul(n.saturating_add(8))
        .saturating_add(metadata);
    if positive == 0 || boundary_rows == 0 || words > limit {
        return Ok(None);
    }
    let design = primal::RawRows::new(model, true)?;
    let width = design.width;
    // The structural constant may duplicate an explicit or absorbed column.
    // Retaining its equation makes the certificate independent of that alias.
    let structural = usize::from(model.fit_view().fit_structural_intercept());
    let columns = width
        .checked_add(structural)
        .ok_or_else(|| allocation("exact dual width overflow"))?;
    let work = primal::ProofBudget::new(model, method, config)
        .work_cells
        .saturating_mul(8)
        .min(32 * 1024 * 1024);
    // Try distinct pivot orders among boundary rows. Each pass has a fixed
    // conservative work charge, including both traversals of the raw design.
    // The admitted dimensions fit in usize; use u128 for the products.
    let inner = columns.min(n) as u128 + 2 + 2 * topology.effect_specs.len() as u128;
    let pass = (n as u128).saturating_mul(
        (columns as u128)
            .saturating_mul(inner)
            .saturating_add(n as u128),
    );
    if columns == 0 || pass > work as u128 {
        return Ok(None);
    }
    let pass = pass as usize;
    let mut rows = fallible_collect(
        (0..n)
            .filter(|&r| model.outcome()[r] > 0.)
            .chain((0..n).filter(|&r| model.outcome()[r] == 0.)),
        n,
        "unable to reserve exact dual row order",
    )?;
    let attempts = boundary_rows.min(work / pass);
    for _ in 0..attempts {
        if crate::ppml_separation::raw_positive_dual(
            columns,
            n,
            positive,
            None,
            config.certificate_tolerance,
            pass,
            |j, values| {
                if j == width {
                    values.fill(1.);
                } else {
                    for (value, &r) in values.iter_mut().zip(&rows) {
                        *value = design.value(r, j)?;
                    }
                }
                Ok(())
            },
        )? {
            return Ok(Some(no_separation(
                method,
                SeparationCertificateProof::ExactPositiveDual { boundary_rows },
            )));
        }
        rows[positive..].rotate_left(1);
    }
    Ok(None)
}
