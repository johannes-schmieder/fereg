//! One canonical slot map and identified geometry for both estimators.
use super::{Fit, Role, Status};
use crate::{
    data::{CompiledColumns, DenseMatrix, SyntaxStatus},
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
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
pub(crate) fn mapped(
    columns: &CompiledColumns,
    raw: &DenseMatrix,
    within: &DenseMatrix,
    fit: &Fit,
    extend_constant: bool,
    poll: Poll<'_>,
) -> Result<Reported> {
    validate_polled(columns, raw, within, fit, poll)?;
    let width = fit
        .kept
        .len()
        .checked_add(usize::from(
            extend_constant && deferred_slot(columns).is_some(),
        ))
        .ok_or_else(|| invalid("reported coordinate count overflows"))?;
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
        execution::check_chunk(poll, slot)?;
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
    for (left, &generated) in fit.kept.iter().enumerate() {
        execution::check(poll)?;
        for row in 0..raw.nrows() {
            execution::check_chunk(poll, row)?;
            result.score_design[(row, left)] = within[(row, generated)];
        }
        for (right, &other) in fit.kept.iter().enumerate() {
            execution::check_chunk(poll, right)?;
            result.bread[(left, right)] = fit.bread[(generated, other)];
        }
    }
    execution::check(poll)?;
    Ok(result)
}
/// Called only after mapped has validated the complete canonical metadata.
pub(crate) fn deferred_slot(columns: &CompiledColumns) -> Option<usize> {
    columns.constant_slot.filter(|&slot| {
        columns.roles[columns.slots[slot].generated.unwrap()] == Role::AbsorbedIntercept
    })
}
pub(crate) fn validate(
    columns: &CompiledColumns,
    raw: &DenseMatrix,
    within: &DenseMatrix,
    fit: &Fit,
) -> Result<()> {
    validate_polled(columns, raw, within, fit, None)
}
pub(crate) fn validate_polled(
    columns: &CompiledColumns,
    raw: &DenseMatrix,
    within: &DenseMatrix,
    fit: &Fit,
    poll: Poll<'_>,
) -> Result<()> {
    execution::check(poll)?;
    let p = raw.ncols();
    if within.nrows() != raw.nrows()
        || within.ncols() != p
        || columns.roles.len() != p
        || columns.recipes.len() != p
        || fit.coefficients.len() != p
        || fit.statuses.len() != p
        || fit.bread.nrows() != p
        || fit.bread.ncols() != p
        || execution::any(&fit.kept, |&j| j >= p, poll)?
        || execution::any(fit.kept.windows(2), |v| v[0] >= v[1], poll)?
    {
        return Err(invalid("reported dimensions or kept indices disagree"));
    }
    if execution::any(
        raw.as_slice()
            .iter()
            .chain(within.as_slice())
            .chain(&fit.coefficients)
            .chain(fit.bread.as_slice()),
        |v| !v.is_finite(),
        poll,
    )? {
        return Err(invalid("reported inputs must be finite"));
    }
    let mut generated = 0;
    for (slot, column) in columns.slots.iter().enumerate() {
        execution::check_chunk(poll, slot)?;
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
        execution::check_chunk(poll, j)?;
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
    let (deferred, count) = execution::fold(
        columns.roles.iter().enumerate(),
        (None, 0usize),
        poll,
        |(first, count), (j, role)| {
            if *role == Role::AbsorbedIntercept {
                (first.or(Some(j)), count + 1)
            } else {
                (first, count)
            }
        },
    )?;
    if let Some(generated) = deferred {
        if count != 1
            || columns
                .constant_slot
                .and_then(|slot| columns.slots[slot].generated)
                != Some(generated)
        {
            return Err(invalid("deferred constant is missing its reported slot"));
        }
    }
    execution::check(poll)
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
