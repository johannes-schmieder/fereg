//! Canonical reporting coordinates; a normalized constant never adds finite rank.
pub use crate::wls::Reported;
use crate::{
    data::{CompiledColumns, DenseMatrix, ReferenceWeights},
    error::{reserved, ErrorCode, FeregError, Result},
    threads::compensated_sum,
    wls::{reporting, Fit, Status},
};
pub fn report(
    columns: &CompiledColumns,
    raw: &DenseMatrix,
    within: &DenseMatrix,
    outcome: &[f64],
    weights: ReferenceWeights<'_>,
    fit: &Fit,
) -> Result<Reported> {
    if raw.nrows() != weights.nobs()
        || outcome.len() != raw.nrows()
        || outcome.iter().any(|v| !v.is_finite())
    {
        return Err(invalid(
            "reported OLS outcome/weights are not sample aligned",
        ));
    }
    let mut result = reporting::mapped(columns, raw, within, fit, true)?;
    let constant = reporting::deferred_slot(columns);
    let mut means = reserved(fit.kept.len())?;
    for (left, &generated) in fit.kept.iter().enumerate() {
        let mean = if constant.is_some() {
            weights.mean_by(|row| raw[(row, generated)])?
        } else {
            0.
        };
        means.push(mean);
        for row in 0..raw.nrows() {
            result.score_design[(row, left)] = within[(row, generated)] + mean;
        }
    }
    if let Some(slot) = constant {
        normalize(&mut result, slot, raw, outcome, weights, fit, &means)?;
    }
    if result
        .bread
        .as_slice()
        .iter()
        .chain(result.score_design.as_slice())
        .chain(&result.coefficients)
        .any(|v| !v.is_finite())
    {
        return Err(numerical("reported coordinates are not representable"));
    }
    Ok(result)
}
fn normalize(
    result: &mut Reported,
    slot: usize,
    raw: &DenseMatrix,
    outcome: &[f64],
    weights: ReferenceWeights<'_>,
    fit: &Fit,
    means: &[f64],
) -> Result<()> {
    let k = fit.kept.len();
    // Averaging y-Xb avoids subtracting large separately averaged terms. The
    // independent reference formula still certifies the normalization identity.
    let constant = weights.mean_by(|row| {
        outcome[row]
            - compensated_sum(
                fit.kept
                    .iter()
                    .map(|&j| raw[(row, j)] * fit.coefficients[j]),
            )
    })?;
    let mean_y = weights.mean(outcome)?;
    let mean_fit = compensated_sum(
        fit.kept
            .iter()
            .enumerate()
            .map(|(i, &j)| means[i] * fit.coefficients[j]),
    );
    let formula = mean_y - mean_fit;
    let error = (constant - formula).abs() / mean_y.abs().max(constant.abs()).max(1.);
    if !formula.is_finite()
        || !error.is_finite()
        || error > crate::wls::Policy::default().bread_tolerance
    {
        return Err(numerical(
            "reported constant failed its independent mean formula",
        ));
    }
    result.constant_formula_error = error;
    result.reported_constant = constant;
    result.coefficients[slot] = constant;
    result.statuses[slot] = Status::NormalizedAbsorbedIntercept;
    result.coordinates.push(slot);
    for row in 0..raw.nrows() {
        result.score_design[(row, k)] = 1.;
    }
    for i in 0..k {
        let side = -compensated_sum((0..k).map(|j| result.bread[(i, j)] * means[j]));
        result.bread[(i, k)] = side;
        result.bread[(k, i)] = side;
    }
    let corner = 1. / weights.effective() as f64
        + compensated_sum((0..k).map(|i| -means[i] * result.bread[(i, k)]));
    if !corner.is_finite() || corner <= 0. {
        return Err(numerical("reported constant bread is invalid"));
    }
    result.bread[(k, k)] = corner;
    Ok(())
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
