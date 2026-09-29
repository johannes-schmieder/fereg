//! Canonical reporting coordinates; a normalized constant never adds finite rank.
use crate::{
    data::{CompiledColumns, DenseMatrix, ReferenceWeights, SyntaxStatus},
    error::{reserved, ErrorCode, FeregError, Result},
    threads::compensated_sum,
    wls::{Fit, Role, Status},
};
#[derive(Debug, PartialEq)]
pub struct Reported {
    pub coefficients: Vec<f64>,
    pub statuses: Vec<Status>,
    /// Compact inference coordinate -> original Stata coefficient slot.
    pub coordinates: Vec<usize>,
    pub bread: DenseMatrix,
    pub score_design: DenseMatrix,
    pub finite_rank: usize,
    pub reported_constant: f64,
    pub constant_formula_error: f64,
}
pub fn report(
    columns: &CompiledColumns,
    raw: &DenseMatrix,
    within: &DenseMatrix,
    outcome: &[f64],
    weights: ReferenceWeights<'_>,
    fit: &Fit,
) -> Result<Reported> {
    validate(columns, raw, within, outcome, weights, fit)?;
    let constant = columns.constant_slot.filter(|&slot| {
        columns.roles[columns.slots[slot].generated.unwrap()] == Role::AbsorbedIntercept
    });
    let width = fit.kept.len() + usize::from(constant.is_some());
    let mut result = Reported {
        coefficients: reserved(columns.slots.len())?,
        statuses: reserved(columns.slots.len())?,
        coordinates: reserved(width)?,
        bread: DenseMatrix::zeros(width, width)?,
        score_design: DenseMatrix::zeros(raw.nrows(), width)?,
        finite_rank: fit.kept.len(),
        reported_constant: 0.,
        constant_formula_error: 0.,
    };
    for (slot, column) in columns.slots.iter().enumerate() {
        let (value, status) = match column.generated {
            Some(generated) => (fit.coefficients[generated], fit.statuses[generated]),
            None => (
                0.,
                match column.syntax {
                    SyntaxStatus::Base => Status::Base,
                    SyntaxStatus::ExplicitlyOmitted => Status::ExplicitlyOmitted,
                    SyntaxStatus::Active => {
                        return Err(invalid("active slot has no generated column"))
                    }
                },
            ),
        };
        result.coefficients.push(value);
        result.statuses.push(status);
        if status == Status::Kept {
            result.coordinates.push(slot);
        }
    }
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
        for (right, &other) in fit.kept.iter().enumerate() {
            result.bread[(left, right)] = fit.bread[(generated, other)];
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
fn validate(
    columns: &CompiledColumns,
    raw: &DenseMatrix,
    within: &DenseMatrix,
    outcome: &[f64],
    weights: ReferenceWeights<'_>,
    fit: &Fit,
) -> Result<()> {
    let p = raw.ncols();
    if raw.nrows() != weights.nobs()
        || within.nrows() != raw.nrows()
        || within.ncols() != p
        || outcome.len() != raw.nrows()
        || columns.roles.len() != p
        || columns.recipes.len() != p
        || fit.coefficients.len() != p
        || fit.statuses.len() != p
        || fit.bread.nrows() != p
        || fit.bread.ncols() != p
        || fit.kept.iter().any(|&j| j >= p)
        || fit.kept.windows(2).any(|v| v[0] >= v[1])
    {
        return Err(invalid("reported OLS dimensions or kept indices disagree"));
    }
    if raw
        .as_slice()
        .iter()
        .chain(within.as_slice())
        .chain(outcome)
        .chain(&fit.coefficients)
        .chain(fit.bread.as_slice())
        .any(|v| !v.is_finite())
    {
        return Err(invalid("reported OLS inputs must be finite"));
    }
    let mut generated = 0;
    for column in &columns.slots {
        if let Some(index) = column.generated {
            if index != generated || column.syntax != SyntaxStatus::Active {
                return Err(invalid("noncanonical generated coefficient slots"));
            }
            generated += 1;
        }
    }
    if generated != p {
        return Err(invalid("generated slots do not cover the finite design"));
    }
    for (j, status) in fit.statuses.iter().enumerate() {
        if (columns.roles[j] == Role::AbsorbedIntercept)
            != (*status == Status::AbsorbedInterceptDeferred)
        {
            return Err(invalid("deferred constant role and finite status disagree"));
        }
        if (*status == Status::Kept) != fit.kept.binary_search(&j).is_ok()
            || *status == Status::NormalizedAbsorbedIntercept
            || (*status != Status::Kept && fit.coefficients[j] != 0.)
        {
            return Err(invalid("finite status and reported coefficient disagree"));
        }
    }
    if let Some(slot) = columns.constant_slot {
        if slot.checked_add(1) != Some(columns.slots.len())
            || columns
                .slots
                .get(slot)
                .is_none_or(|column| column.generated.is_none())
        {
            return Err(invalid("reported constant has an invalid canonical slot"));
        }
    }
    if let Some(generated) = columns
        .roles
        .iter()
        .position(|role| *role == Role::AbsorbedIntercept)
    {
        if columns
            .roles
            .iter()
            .filter(|role| **role == Role::AbsorbedIntercept)
            .count()
            != 1
            || columns
                .constant_slot
                .and_then(|slot| columns.slots[slot].generated)
                != Some(generated)
        {
            return Err(invalid("deferred constant is missing its reported slot"));
        }
    }
    Ok(())
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
