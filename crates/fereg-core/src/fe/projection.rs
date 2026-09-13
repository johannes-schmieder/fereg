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

#[cfg(test)]
mod tests {
    use super::*;

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
