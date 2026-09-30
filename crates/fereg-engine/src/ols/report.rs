//! Canonical reporting coordinates; a normalized constant never adds finite rank.
pub use crate::wls::Reported;
use crate::{
    data::{CompiledColumns, DenseMatrix, ReferenceWeights},
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
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
    report_polled(columns, raw, within, outcome, weights, fit, None)
}
pub(crate) fn report_polled(
    columns: &CompiledColumns,
    raw: &DenseMatrix,
    within: &DenseMatrix,
    outcome: &[f64],
    weights: ReferenceWeights<'_>,
    fit: &Fit,
    poll: Poll<'_>,
) -> Result<Reported> {
    execution::check(poll)?;
    if raw.nrows() != weights.nobs()
        || outcome.len() != raw.nrows()
        || execution::any(outcome, |v| !v.is_finite(), poll)?
    {
        return Err(invalid(
            "reported OLS outcome/weights are not sample aligned",
        ));
    }
    let mut result = reporting::mapped(columns, raw, within, fit, true, poll)?;
    let constant = reporting::deferred_slot(columns);
    let mut means = reserved(fit.kept.len())?;
    for (left, &generated) in fit.kept.iter().enumerate() {
        execution::check(poll)?;
        let mean = if constant.is_some() {
            weights.mean_by_polled(|row| raw[(row, generated)], poll)?
        } else {
            0.
        };
        means.push(mean);
        for row in 0..raw.nrows() {
            execution::check_chunk(poll, row)?;
            result.score_design[(row, left)] = within[(row, generated)] + mean;
        }
    }
    if let Some(slot) = constant {
        normalize(
            &mut result,
            (slot, &means),
            raw,
            outcome,
            weights,
            fit,
            poll,
        )?;
    }
    if execution::any(
        result
            .bread
            .as_slice()
            .iter()
            .chain(result.score_design.as_slice())
            .chain(&result.coefficients),
        |v| !v.is_finite(),
        poll,
    )? {
        return Err(numerical("reported coordinates are not representable"));
    }
    execution::check(poll)?;
    Ok(result)
}
fn normalize(
    result: &mut Reported,
    constant: (usize, &[f64]),
    raw: &DenseMatrix,
    outcome: &[f64],
    weights: ReferenceWeights<'_>,
    fit: &Fit,
    poll: Poll<'_>,
) -> Result<()> {
    let (slot, means) = constant;
    let k = fit.kept.len();
    // Averaging y-Xb avoids subtracting large separately averaged terms. The
    // independent reference formula still certifies the normalization identity.
    let constant = weights.try_mean_by_polled(
        |row| {
            Ok(outcome[row]
                - execution::sum(
                    fit.kept
                        .iter()
                        .map(|&j| raw[(row, j)] * fit.coefficients[j]),
                    poll,
                )?)
        },
        poll,
    )?;
    let mean_y = weights.mean_polled(outcome, poll)?;
    let mean_fit = execution::sum(
        fit.kept
            .iter()
            .enumerate()
            .map(|(i, &j)| means[i] * fit.coefficients[j]),
        poll,
    )?;
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
        execution::check_chunk(poll, row)?;
        result.score_design[(row, k)] = 1.;
    }
    for i in 0..k {
        execution::check(poll)?;
        let side = -execution::sum((0..k).map(|j| result.bread[(i, j)] * means[j]), poll)?;
        result.bread[(i, k)] = side;
        result.bread[(k, i)] = side;
    }
    let corner = 1. / weights.effective() as f64
        + execution::sum((0..k).map(|i| -means[i] * result.bread[(i, k)]), poll)?;
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

#[cfg(test)]
mod interruption_tests;
