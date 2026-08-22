//! Independent explicit-dummy oracles used by numerical-core integration tests.

use stfixest_core::{DenseMatrix, FeDesign};

/// Basis-invariant FE projection diagnostics computed by an independent
/// column-pivoted Householder QR implementation.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplicitDummyCertificate {
    pub relative_infinity_norm: f64,
    pub relative_two_norm: f64,
}

/// Compute the certificate ratios from explicitly materialized local dummy
/// blocks. This intentionally does not call the production certificate code.
pub fn fe_projection_certificate(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
) -> std::result::Result<ExplicitDummyCertificate, String> {
    fe_projection_certificate_with_reference(design, weights, residuals, residuals)
}

/// Compute explicit-dummy certificate ratios using pre-projection right-hand
/// sides as the reference scale. This matches the scientific solver gate while
/// remaining independent of the production certificate implementation.
pub fn fe_projection_certificate_with_reference(
    design: &FeDesign,
    weights: &[f64],
    residuals: &DenseMatrix,
    reference: &DenseMatrix,
) -> std::result::Result<ExplicitDummyCertificate, String> {
    if design.nobs() != residuals.nrows() || weights.len() != residuals.nrows() {
        return Err("oracle inputs have inconsistent observation counts".to_owned());
    }
    if reference.nrows() != residuals.nrows() || reference.ncols() != residuals.ncols() {
        return Err("oracle reference dimensions differ from the residuals".to_owned());
    }
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
        || residuals.as_slice().iter().any(|value| !value.is_finite())
        || reference.as_slice().iter().any(|value| !value.is_finite())
    {
        return Err("oracle inputs must be finite with positive weights".to_owned());
    }

    let rhs_norms = (0..residuals.ncols())
        .map(|rhs| {
            let residual_norm = stable_norm(
                (0..residuals.nrows()).map(|row| weights[row].sqrt() * residuals[(row, rhs)]),
            );
            let reference_norm = stable_norm(
                (0..reference.nrows()).map(|row| weights[row].sqrt() * reference[(row, rhs)]),
            );
            residual_norm.max(reference_norm)
        })
        .collect::<Vec<_>>();
    if rhs_norms.iter().any(|norm| !norm.is_finite()) {
        return Err("oracle weighted RHS norm is not finite".to_owned());
    }

    let mut maximum = 0.0_f64;
    let mut square_sum = 0.0_f64;
    let mut count = 0_usize;
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let observations = effect.level_index().members(level);
            let mut columns = Vec::with_capacity(effect.channels());
            for channel in 0..effect.channels() {
                let mut column = observations
                    .iter()
                    .map(|&observation| {
                        weights[observation].sqrt() * effect.basis_value(channel, observation)
                    })
                    .collect::<Vec<_>>();
                let scale = column
                    .iter()
                    .fold(0.0_f64, |current, value| current.max(value.abs()));
                if scale == 0.0 {
                    continue;
                }
                for value in &mut column {
                    *value /= scale;
                }
                let norm = stable_norm(column.iter().copied());
                for value in &mut column {
                    *value /= norm;
                }
                columns.push(column);
            }
            let right_hand_sides = (0..residuals.ncols())
                .map(|rhs| {
                    observations
                        .iter()
                        .map(|&observation| {
                            if rhs_norms[rhs] == 0.0 {
                                0.0
                            } else {
                                weights[observation].sqrt() * residuals[(observation, rhs)]
                                    / rhs_norms[rhs]
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            for ratio in householder_projection_ratios(columns, right_hand_sides) {
                maximum = maximum.max(ratio);
                square_sum = ratio.mul_add(ratio, square_sum);
                count += 1;
            }
        }
    }

    Ok(ExplicitDummyCertificate {
        relative_infinity_norm: maximum,
        relative_two_norm: if count == 0 {
            0.0
        } else {
            (square_sum / count as f64).sqrt()
        },
    })
}

fn householder_projection_ratios(
    mut columns: Vec<Vec<f64>>,
    mut right_hand_sides: Vec<Vec<f64>>,
) -> Vec<f64> {
    let nrows = right_hand_sides
        .first()
        .map_or_else(|| columns.first().map_or(0, Vec::len), Vec::len);
    let ncols = columns.len();
    let threshold = f64::EPSILON * nrows.max(ncols).max(1) as f64 * 8.0;
    let mut rank = 0_usize;

    for step in 0..nrows.min(ncols) {
        let pivot = (step..ncols)
            .max_by(|&left, &right| {
                tail_norm(&columns[left], step).total_cmp(&tail_norm(&columns[right], step))
            })
            .expect("nonempty Householder pivot range");
        let pivot_norm = tail_norm(&columns[pivot], step);
        if pivot_norm <= threshold {
            break;
        }
        columns.swap(step, pivot);

        let alpha = if columns[step][step].is_sign_negative() {
            pivot_norm
        } else {
            -pivot_norm
        };
        let mut reflector = columns[step][step..].to_vec();
        reflector[0] -= alpha;
        let reflector_norm = stable_norm(reflector.iter().copied());
        if reflector_norm <= threshold {
            break;
        }
        for value in &mut reflector {
            *value /= reflector_norm;
        }
        for column in &mut columns[step..] {
            apply_reflector(&reflector, &mut column[step..]);
        }
        for rhs in &mut right_hand_sides {
            apply_reflector(&reflector, &mut rhs[step..]);
        }
        rank += 1;
    }

    right_hand_sides
        .iter()
        .map(|rhs| stable_norm(rhs[..rank].iter().copied()))
        .collect()
}

fn apply_reflector(reflector: &[f64], values: &mut [f64]) {
    let inner_product = reflector
        .iter()
        .zip(values.iter())
        .fold(0.0_f64, |sum, (&left, &right)| left.mul_add(right, sum));
    for (&direction, value) in reflector.iter().zip(values) {
        *value = (-2.0 * inner_product).mul_add(direction, *value);
    }
}

fn tail_norm(values: &[f64], start: usize) -> f64 {
    stable_norm(values[start..].iter().copied())
}

fn stable_norm(values: impl Iterator<Item = f64>) -> f64 {
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for value in values {
        let absolute = value.abs();
        if absolute == 0.0 {
            continue;
        }
        if scale < absolute {
            let ratio = scale / absolute;
            scaled_square_sum = 1.0 + scaled_square_sum * ratio * ratio;
            scale = absolute;
        } else {
            let ratio = absolute / scale;
            scaled_square_sum += ratio * ratio;
        }
    }
    if scale == 0.0 {
        0.0
    } else {
        scale * scaled_square_sum.sqrt()
    }
}
