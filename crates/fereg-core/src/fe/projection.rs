use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::EffectTerm;
use crate::linalg::solve_linear_system;

/// Reusable buffers for one weighted fixed-effect projection.
#[derive(Debug, Clone)]
pub struct ProjectionWorkspace {
    max_channels: usize,
    rhs_count: usize,
    gram: Vec<f64>,
    right_hand_sides: Vec<f64>,
}

impl ProjectionWorkspace {
    pub fn new(max_channels: usize, rhs_count: usize) -> Result<Self> {
        let gram_length = max_channels.checked_mul(max_channels).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "projection Gram dimensions overflow")
        })?;
        let rhs_length = max_channels.checked_mul(rhs_count).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "projection RHS dimensions overflow")
        })?;
        Ok(Self {
            max_channels,
            rhs_count,
            gram: fallible_zeros(gram_length)?,
            right_hand_sides: fallible_zeros(rhs_length)?,
        })
    }

    pub(crate) fn storage_capacities(&self) -> (usize, usize) {
        (self.gram.capacity(), self.right_hand_sides.capacity())
    }
}

fn fallible_zeros(elements: usize) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    values.try_reserve_exact(elements).map_err(|_| {
        FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve generic FE projection workspace",
        )
    })?;
    values.resize(elements, 0.0);
    Ok(values)
}

/// Subtract the weighted least-squares projection onto one effect term.
pub fn project_effect_in_place(
    effect: &EffectTerm,
    weights: &[f64],
    values: &mut DenseMatrix,
    workspace: &mut ProjectionWorkspace,
) -> Result<()> {
    if weights.len() != values.nrows() || effect.codes().len() != values.nrows() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "projection inputs have inconsistent observation counts",
        ));
    }
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "projection weights must be finite and positive",
        ));
    }
    let channels = effect.channels();
    let rhs_count = values.ncols();
    if channels > workspace.max_channels || rhs_count > workspace.rhs_count {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "projection workspace is smaller than the requested projection",
        ));
    }

    for level in 0..effect.n_levels() {
        let gram_length = channels * channels;
        let rhs_length = channels * rhs_count;
        workspace.gram[..gram_length].fill(0.0);
        workspace.right_hand_sides[..rhs_length].fill(0.0);

        for observation in effect.level_index().members(level) {
            let weight = weights[*observation];
            let row_start = *observation * rhs_count;
            let row = &values.as_slice()[row_start..row_start + rhs_count];
            for left_channel in 0..channels {
                let left_basis = effect.basis_value(left_channel, *observation);
                for right_channel in 0..channels {
                    let right_basis = effect.basis_value(right_channel, *observation);
                    let index = left_channel * channels + right_channel;
                    workspace.gram[index] =
                        (weight * left_basis).mul_add(right_basis, workspace.gram[index]);
                }
                let start = left_channel * rhs_count;
                let moments = &mut workspace.right_hand_sides[start..start + rhs_count];
                for (moment, value) in moments.iter_mut().zip(row) {
                    *moment = (weight * left_basis).mul_add(*value, *moment);
                }
            }
        }

        solve_linear_system(
            &mut workspace.gram[..gram_length],
            &mut workspace.right_hand_sides[..rhs_length],
            channels,
            rhs_count,
            1e-12,
        )?;

        // Batch independent RHSs while keeping each fitted value's channel FMA
        // order and its single final subtraction. The tile adds no heap state.
        for observation in effect.level_index().members(level) {
            let row_start = *observation * rhs_count;
            let row = &mut values.as_mut_slice()[row_start..row_start + rhs_count];
            for (tile_index, tile) in row.chunks_mut(32).enumerate() {
                let mut fitted = [0.0_f64; 32];
                let rhs_start = tile_index * 32;
                for channel in 0..channels {
                    let basis = effect.basis_value(channel, *observation);
                    let start = channel * rhs_count + rhs_start;
                    let coefficients = &workspace.right_hand_sides[start..start + tile.len()];
                    for (value, coefficient) in fitted[..tile.len()].iter_mut().zip(coefficients) {
                        *value = basis.mul_add(*coefficient, *value);
                    }
                }
                for (value, fit) in tile.iter_mut().zip(fitted) {
                    *value -= fit;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rhs_tiles_preserve_scalar_projection_bits() {
        let rows = 97;
        let codes: Vec<u32> = (0..rows)
            .map(|row| ((19 * row + row / 7) % 11) as u32)
            .collect();
        for intercept in [false, true] {
            for slope_count in 0..=5 {
                if !intercept && slope_count == 0 {
                    continue;
                }
                for scale in [1e-60, 1.0, 1e60] {
                    let slopes = (0..slope_count)
                        .map(|channel| {
                            (0..rows)
                                .map(|row| {
                                    let x = ((row * 13 % 37) as f64 - 18.0) / 7.0;
                                    scale
                                        * match channel {
                                            0 => x,
                                            1 => 2.0 * x,
                                            2 => 0.0,
                                            3 => (row % 5) as f64 - 2.0,
                                            _ => 1.0,
                                        }
                                })
                                .collect()
                        })
                        .collect();
                    // Includes empty levels, dependent and zero channels, and
                    // projection Gram thresholds dominated by the intercept.
                    let effect = EffectTerm::new(codes.clone(), 13, intercept, slopes).unwrap();
                    for weighted in [false, true] {
                        let weights: Vec<_> = (0..rows)
                            .map(|row| {
                                if weighted {
                                    0.25 + (row % 9) as f64 / 4.0
                                } else {
                                    1.0
                                }
                            })
                            .collect();
                        for rhs_count in [0, 1, 2, 7, 31, 32, 33, 64, 65, 129] {
                            let mut actual = DenseMatrix::zeros(rows, rhs_count).unwrap();
                            for row in 0..rows {
                                for rhs in 0..rhs_count {
                                    actual[(row, rhs)] = match rhs % 4 {
                                        0 => ((row * 17 + rhs * 3) % 41) as f64 - 20.0,
                                        1 => 0.0,
                                        2 => -0.0,
                                        _ => 1e100 * ((row % 7) as f64 - 3.0),
                                    };
                                }
                            }
                            let mut expected = actual.clone();
                            let mut actual_work =
                                ProjectionWorkspace::new(7, rhs_count + 3).unwrap();
                            let mut expected_work = actual_work.clone();
                            for _ in 0..3 {
                                project_effect_in_place(
                                    &effect,
                                    &weights,
                                    &mut actual,
                                    &mut actual_work,
                                )
                                .unwrap();
                                scalar_project_effect_in_place(
                                    &effect,
                                    &weights,
                                    &mut expected,
                                    &mut expected_work,
                                )
                                .unwrap();
                                for (a, b) in actual.as_slice().iter().zip(expected.as_slice()) {
                                    assert_eq!(a.to_bits(), b.to_bits(), "intercept={intercept} slopes={slope_count} scale={scale} weighted={weighted} rhs={rhs_count}");
                                }
                                assert_eq!(actual_work.gram, expected_work.gram);
                                assert_eq!(
                                    actual_work.right_hand_sides,
                                    expected_work.right_hand_sides
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn scalar_project_effect_in_place(
        effect: &EffectTerm,
        weights: &[f64],
        values: &mut DenseMatrix,
        workspace: &mut ProjectionWorkspace,
    ) -> Result<()> {
        if weights.len() != values.nrows() || effect.codes().len() != values.nrows() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "projection inputs have inconsistent observation counts",
            ));
        }
        if weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "projection weights must be finite and positive",
            ));
        }
        let channels = effect.channels();
        let rhs_count = values.ncols();
        if channels > workspace.max_channels || rhs_count > workspace.rhs_count {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "projection workspace is smaller than the requested projection",
            ));
        }

        for level in 0..effect.n_levels() {
            let gram_length = channels * channels;
            let rhs_length = channels * rhs_count;
            workspace.gram[..gram_length].fill(0.0);
            workspace.right_hand_sides[..rhs_length].fill(0.0);

            for observation in effect.level_index().members(level) {
                let weight = weights[*observation];
                for left_channel in 0..channels {
                    let left_basis = effect.basis_value(left_channel, *observation);
                    for right_channel in 0..channels {
                        let right_basis = effect.basis_value(right_channel, *observation);
                        let index = left_channel * channels + right_channel;
                        workspace.gram[index] =
                            (weight * left_basis).mul_add(right_basis, workspace.gram[index]);
                    }
                    for rhs in 0..rhs_count {
                        let index = left_channel * rhs_count + rhs;
                        workspace.right_hand_sides[index] = (weight * left_basis).mul_add(
                            values[(*observation, rhs)],
                            workspace.right_hand_sides[index],
                        );
                    }
                }
            }

            solve_linear_system(
                &mut workspace.gram[..gram_length],
                &mut workspace.right_hand_sides[..rhs_length],
                channels,
                rhs_count,
                1e-12,
            )?;

            for observation in effect.level_index().members(level) {
                for rhs in 0..rhs_count {
                    let mut fitted = 0.0;
                    for channel in 0..channels {
                        fitted = effect.basis_value(channel, *observation).mul_add(
                            workspace.right_hand_sides[channel * rhs_count + rhs],
                            fitted,
                        );
                    }
                    values[(*observation, rhs)] -= fitted;
                }
            }
        }
        Ok(())
    }

    #[test]
    fn removes_weighted_group_intercepts_and_slopes() {
        let effect = EffectTerm::new(
            vec![0, 0, 0, 1, 1, 1],
            2,
            true,
            vec![vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0]],
        )
        .expect("effect");
        let weights = vec![1.0, 2.0, 1.0, 1.0, 1.0, 2.0];
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 3.0, 5.0, 2.0, 1.0, 0.0]]).expect("matrix");
        let mut workspace = ProjectionWorkspace::new(2, 1).expect("workspace");
        project_effect_in_place(&effect, &weights, &mut values, &mut workspace)
            .expect("projection");
        assert!(values.as_slice().iter().all(|value| value.abs() < 1e-10));
    }
}
