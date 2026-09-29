//! Harvested from fereg-testkit/src/explicit_ols.rs at legacy-v0.5.0-beta.
//! Independent dense oracle; no production numerical implementation is called.

use super::*;

#[derive(Debug, Clone)]
struct Reflector {
    start: usize,
    direction: Vec<f64>,
}

#[derive(Debug, Clone)]
pub(super) struct PivotedQr {
    columns: Vec<Vec<f64>>,
    permutation: Vec<usize>,
    scales: Vec<f64>,
    reflectors: Vec<Reflector>,
    rank: usize,
    nrows: usize,
}

impl PivotedQr {
    pub(super) fn factor(
        columns: Vec<Vec<f64>>,
        nrows: usize,
        tolerance: f64,
    ) -> Result<Self, OracleError> {
        let scales = columns
            .iter()
            .map(|column| stable_norm(column.iter().copied()))
            .collect::<Vec<_>>();
        Self::factor_with_scales(columns, nrows, tolerance, scales)
    }

    /// Factor residualized columns relative to their scale before projection.
    /// Using their post-projection norm would inflate roundoff left behind by
    /// an exact FE-collinear column into a full-strength regressor.
    pub(super) fn factor_with_scales(
        mut columns: Vec<Vec<f64>>,
        nrows: usize,
        tolerance: f64,
        mut scales: Vec<f64>,
    ) -> Result<Self, OracleError> {
        if columns.iter().any(|column| column.len() != nrows) {
            return Err(OracleError::InvalidInput(
                "QR columns have inconsistent lengths".to_owned(),
            ));
        }
        if scales.len() != columns.len()
            || scales
                .iter()
                .any(|scale| !scale.is_finite() || *scale < 0.0)
        {
            return Err(OracleError::NumericalFailure(
                "a design-column reference norm is invalid".to_owned(),
            ));
        }
        for (column, &scale) in columns.iter_mut().zip(&scales) {
            if scale == 0.0 {
                if column.iter().any(|value| *value != 0.0) {
                    return Err(OracleError::NumericalFailure(
                        "a zero-scale design column contains a nonzero value".to_owned(),
                    ));
                }
                continue;
            }
            for value in column {
                *value /= scale;
                if !value.is_finite() {
                    return Err(OracleError::NumericalFailure(
                        "design-column scaling produced a nonfinite value".to_owned(),
                    ));
                }
            }
        }
        let mut permutation = (0..columns.len()).collect::<Vec<_>>();
        let mut reflectors = Vec::new();
        let mut rank = 0_usize;

        for step in 0..nrows.min(columns.len()) {
            let mut pivot = step;
            let mut pivot_norm = -1.0_f64;
            for (column, values) in columns.iter().enumerate().skip(step) {
                let norm = stable_norm(values[step..].iter().copied());
                if norm > pivot_norm {
                    pivot = column;
                    pivot_norm = norm;
                }
            }
            if pivot_norm <= tolerance {
                break;
            }
            columns.swap(step, pivot);
            permutation.swap(step, pivot);
            scales.swap(step, pivot);

            let alpha = if columns[step][step].is_sign_negative() {
                pivot_norm
            } else {
                -pivot_norm
            };
            let mut direction = columns[step][step..].to_vec();
            direction[0] -= alpha;
            let direction_norm = stable_norm(direction.iter().copied());
            if direction_norm <= tolerance || !direction_norm.is_finite() {
                return Err(OracleError::NumericalFailure(
                    "Householder direction is numerically unresolved".to_owned(),
                ));
            }
            for value in &mut direction {
                *value /= direction_norm;
            }
            for column in &mut columns[step..] {
                apply_reflector(&direction, &mut column[step..]);
            }
            reflectors.push(Reflector {
                start: step,
                direction,
            });
            rank += 1;
        }

        Ok(Self {
            columns,
            permutation,
            scales,
            reflectors,
            rank,
            nrows,
        })
    }

    pub(super) const fn rank(&self) -> usize {
        self.rank
    }

    pub(super) fn kept_original_indices(&self) -> &[usize] {
        &self.permutation[..self.rank]
    }

    pub(super) fn transform(&self, values: &[f64]) -> Vec<f64> {
        let mut transformed = values.to_vec();
        for reflector in &self.reflectors {
            apply_reflector(&reflector.direction, &mut transformed[reflector.start..]);
        }
        transformed
    }

    pub(super) fn inverse_transform(&self, values: &[f64]) -> Vec<f64> {
        let mut transformed = values.to_vec();
        for reflector in self.reflectors.iter().rev() {
            apply_reflector(&reflector.direction, &mut transformed[reflector.start..]);
        }
        transformed
    }

    pub(super) fn residualize(&self, values: &[f64]) -> Vec<f64> {
        debug_assert_eq!(values.len(), self.nrows);
        let mut transformed = self.transform(values);
        transformed[..self.rank].fill(0.0);
        self.inverse_transform(&transformed)
    }

    pub(super) fn solve(&self, right_hand_side: &[f64]) -> Result<Vec<f64>, OracleError> {
        if right_hand_side.len() != self.nrows {
            return Err(OracleError::InvalidInput(
                "QR right-hand-side length differs from the design".to_owned(),
            ));
        }
        let transformed = self.transform(right_hand_side);
        let mut scaled_solution = vec![0.0; self.rank];
        for row in (0..self.rank).rev() {
            let mut value = transformed[row];
            for (column, coefficient) in scaled_solution
                .iter()
                .enumerate()
                .take(self.rank)
                .skip(row + 1)
            {
                value = (-self.columns[column][row]).mul_add(*coefficient, value);
            }
            let diagonal = self.columns[row][row];
            if diagonal == 0.0 || !diagonal.is_finite() {
                return Err(OracleError::NumericalFailure(
                    "identified QR block is singular".to_owned(),
                ));
            }
            scaled_solution[row] = value / diagonal;
        }
        Ok(scaled_solution
            .iter()
            .zip(&self.scales)
            .map(|(coefficient, scale)| coefficient / scale)
            .collect())
    }

    pub(super) fn bread(&self) -> Result<OracleMatrix, OracleError> {
        let mut inverse_r = OracleMatrix::zeros(self.rank, self.rank);
        for right_hand_side in 0..self.rank {
            for row in (0..self.rank).rev() {
                let mut value = f64::from(row == right_hand_side);
                for column in row + 1..self.rank {
                    value = (-self.columns[column][row])
                        .mul_add(inverse_r.data[column * self.rank + right_hand_side], value);
                }
                let diagonal = self.columns[row][row];
                if diagonal == 0.0 || !diagonal.is_finite() {
                    return Err(OracleError::NumericalFailure(
                        "identified QR block has no finite inverse".to_owned(),
                    ));
                }
                inverse_r.data[row * self.rank + right_hand_side] = value / diagonal;
            }
        }
        let mut bread = OracleMatrix::zeros(self.rank, self.rank);
        for row in 0..self.rank {
            for column in 0..self.rank {
                let value = (0..self.rank).fold(0.0, |sum, inner| {
                    inverse_r.data[row * self.rank + inner]
                        .mul_add(inverse_r.data[column * self.rank + inner], sum)
                });
                bread.data[row * self.rank + column] =
                    value / (self.scales[row] * self.scales[column]);
            }
        }
        Ok(bread)
    }
}

pub(super) fn apply_reflector(direction: &[f64], values: &mut [f64]) {
    let inner = direction
        .iter()
        .zip(values.iter())
        .fold(0.0, |sum, (&left, &right)| left.mul_add(right, sum));
    for (&component, value) in direction.iter().zip(values) {
        *value = (-2.0 * inner).mul_add(component, *value);
    }
}

pub(super) fn stable_norm(values: impl Iterator<Item = f64>) -> f64 {
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
