//! Harvested from legacy/fereg-core at b1700e6d; no memory-mode admission.
use super::sample::*;
use crate::error::{ErrorCode, FeregError, Result};

pub(super) fn prepare_sample<P: NumericDomainPolicy>(
    input: &NumericPreparationInput<'_>,
    source_uses: Option<&[NumericSourceUse]>,
    policy: &P,
) -> Result<Prepared> {
    let n_original = input.candidate.len();
    validate_input(input, n_original)?;
    validate_source_uses(input, source_uses)?;
    let n_candidate = input
        .candidate
        .iter()
        .filter(|candidate| **candidate)
        .count();
    check_output_dimensions(input, n_original, n_candidate)?;

    let mut drop_reasons = reserved_vec(n_original, "drop-reason mask")?;
    let mut original_row_by_active = reserved_vec(n_candidate, "observation map")?;
    let mut outcome = reserved_vec(n_candidate, "outcome")?;

    let mut regressors = reserved_vec(input.regressors.len(), "regressor column table")?;
    for _ in 0..input.regressors.len() {
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
        let reason = classify_row(input, source_uses, policy, original_row)?;
        drop_reasons.push(reason);
        if reason != DropReason::Retained {
            continue;
        }

        original_row_by_active.push(original_row);
        outcome.push(input.outcome.values[original_row]);
        for (destination, source) in regressors.iter_mut().zip(input.regressors) {
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
    debug_assert!(original_row_by_active
        .windows(2)
        .all(|rows| rows[0] < rows[1]));
    debug_assert_eq!(outcome.len(), original_row_by_active.len());
    debug_assert!(regressors
        .iter()
        .all(|column| column.len() == original_row_by_active.len()));

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
