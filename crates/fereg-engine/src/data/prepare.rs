//! Harvested from legacy/fereg-core at b1700e6d; no memory-mode admission.
use super::sample::*;
use crate::{
    error::{ErrorCode, FeregError, Result},
    execution::{self, Poll},
};

pub(super) fn prepare_sample<P: NumericDomainPolicy>(
    input: &NumericPreparationInput<'_>,
    source_uses: Option<&[NumericSourceUse]>,
    policy: &P,
    poll: Poll<'_>,
) -> Result<Prepared> {
    execution::check(poll)?;
    let n_original = input.candidate.len();
    validate_input(input, n_original, poll)?;
    validate_source_uses(input, source_uses, poll)?;
    let n_candidate = execution::fold(input.candidate, 0usize, poll, |count, &candidate| {
        count + usize::from(candidate)
    })?;
    check_output_dimensions(input, n_original, n_candidate)?;

    let mut drop_reasons = reserved_vec(n_original, "drop-reason mask")?;
    let mut original_row_by_active = reserved_vec(n_candidate, "observation map")?;
    let mut outcome = reserved_vec(n_candidate, "outcome")?;

    let mut regressors = reserved_vec(input.regressors.len(), "regressor column table")?;
    for column in 0..input.regressors.len() {
        execution::check_chunk(poll, column)?;
        regressors.push(reserved_vec(n_candidate, "regressor column")?);
    }

    let mut weights = match input.weights {
        WeightInput::None => PreparedWeights::None,
        WeightInput::Provided { kind, .. } => PreparedWeights::Provided {
            kind,
            values: reserved_vec(n_candidate, "weight column")?,
        },
    };
    let mut adjustment = match input.adjustment {
        AdjustmentInput::None => PreparedAdjustment::None,
        AdjustmentInput::Offset(_) => {
            PreparedAdjustment::Offset(reserved_vec(n_candidate, "offset column")?)
        }
        AdjustmentInput::Exposure(_) => {
            PreparedAdjustment::Exposure(reserved_vec(n_candidate, "exposure column")?)
        }
    };

    for original_row in 0..n_original {
        execution::check_chunk(poll, original_row)?;
        let reason = classify_row(input, source_uses, policy, original_row, poll)?;
        drop_reasons.push(reason);
        if reason != DropReason::Retained {
            continue;
        }

        original_row_by_active.push(original_row);
        outcome.push(input.outcome.values[original_row]);
        for (column, (destination, source)) in
            regressors.iter_mut().zip(input.regressors).enumerate()
        {
            execution::check_chunk(poll, column)?;
            destination.push(source.values[original_row]);
        }
        if let (PreparedWeights::Provided { values, .. }, WeightInput::Provided { column, .. }) =
            (&mut weights, input.weights)
        {
            values.push(column.values[original_row]);
        }
        match (&mut adjustment, input.adjustment) {
            (PreparedAdjustment::Offset(values), AdjustmentInput::Offset(column))
            | (PreparedAdjustment::Exposure(values), AdjustmentInput::Exposure(column)) => {
                values.push(column.values[original_row]);
            }
            _ => {}
        }
    }

    debug_assert_eq!(drop_reasons.len(), n_original);
    debug_assert!(!execution::any(
        original_row_by_active.windows(2),
        |rows| rows[0] >= rows[1],
        poll
    )?);
    debug_assert_eq!(outcome.len(), original_row_by_active.len());
    debug_assert!(!execution::any(
        &regressors,
        |column| column.len() != original_row_by_active.len(),
        poll
    )?);

    execution::check(poll)?;
    Ok(Prepared {
        topology: None,
        n_original,
        n_candidate,
        original_row_by_active,
        outcome,
        regressors,
        weights,
        adjustment,
        drop_reasons,
    })
}

fn reserved_vec<T>(elements: usize, label: &str) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values.try_reserve_exact(elements).map_err(|_| {
        FeregError::new(ErrorCode::Allocation, format!("unable to reserve {label}"))
    })?;
    Ok(values)
}

#[cfg(test)]
mod interruption_tests;
