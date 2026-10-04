use super::{compensated_add, invalid_argument};
use crate::error::Result;

pub(super) fn validate_bread(bread: Option<&[f64]>, dimension: usize) -> Result<()> {
    if bread.is_some_and(|values| {
        dimension.checked_mul(dimension) != Some(values.len())
            || values.iter().any(|value| !value.is_finite())
    }) {
        return Err(invalid_argument("sandwich influence bread is invalid"));
    }
    Ok(())
}

pub(super) fn matrix_times_vector(
    bread: &[f64],
    score: &[f64],
    output: &mut [f64],
    reverse: bool,
) -> Result<()> {
    let dimension = score.len();
    if output.len() != dimension || dimension.checked_mul(dimension) != Some(bread.len()) {
        return Err(invalid_argument("sandwich influence dimensions disagree"));
    }
    for (row, destination) in output.iter_mut().enumerate() {
        let (mut sum, mut correction) = (0.0, 0.0);
        for position in 0..dimension {
            let column = if reverse {
                dimension - 1 - position
            } else {
                position
            };
            let left = bread[row * dimension + column];
            let right = score[column];
            let product = left * right;
            // Retain multiplication error as well as sum error. Cancellation
            // in B*s must not replace a genuinely small influence by noise.
            compensated_add(product, &mut sum, &mut correction)?;
            compensated_add(left.mul_add(right, -product), &mut sum, &mut correction)?;
        }
        *destination = sum + correction;
    }
    Ok(())
}

pub(super) fn transform_group(
    score: &mut [f64],
    corrections: &mut [f64],
    workspace: &mut [f64],
    bread: &[f64],
    reverse: bool,
) -> Result<()> {
    if score.len() != corrections.len() || score.len() != workspace.len() {
        return Err(invalid_argument("sandwich influence group banks disagree"));
    }
    let dimension = score.len();
    validate_bread(Some(bread), dimension)?;
    for (row, destination) in workspace.iter_mut().enumerate() {
        let (mut sum, mut rounding) = (0.0, 0.0);
        for position in 0..dimension {
            let column = if reverse {
                dimension - 1 - position
            } else {
                position
            };
            let left = bread[row * dimension + column];
            // Transform both parts before rounding the group score. Collapsing
            // them first destroys low bits in a nearly cancelled influence.
            for right in [score[column], corrections[column]] {
                let product = left * right;
                compensated_add(product, &mut sum, &mut rounding)?;
                compensated_add(left.mul_add(right, -product), &mut sum, &mut rounding)?;
            }
        }
        *destination = sum + rounding;
    }
    score.copy_from_slice(workspace);
    corrections.fill(0.0);
    Ok(())
}

pub(super) fn dense_row(
    bread: &[f64],
    raw: &[f64],
    workspace: &mut [f64],
    multipliers: super::SandwichScorePair,
    production: &mut [f64],
    certificate: &mut [f64],
) -> Result<()> {
    let dimension = raw.len();
    for (reverse, multiplier, matrix) in [
        (false, multipliers.production, production),
        (true, multipliers.certificate, certificate),
    ] {
        matrix_times_vector(bread, raw, workspace, reverse)?;
        for value in workspace.iter_mut() {
            *value *= multiplier;
            if !value.is_finite() {
                return Err(super::invalid_data(
                    "dense sandwich scaled score is not finite",
                ));
            }
        }
        for left in 0..dimension {
            for right in left..dimension {
                matrix[left * dimension + right] += if reverse {
                    workspace[right] * workspace[left]
                } else {
                    workspace[left] * workspace[right]
                };
            }
        }
    }
    Ok(())
}
