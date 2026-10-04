use super::{normalized_solver_weight, RetainedWithinStorage};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::BorrowedMapWeights;
use crate::ols::PreparedOlsBreadWeightScale;

pub(super) struct RefinementFrame<'a> {
    pub storage: &'a RetainedWithinStorage,
    pub active_positions: &'a [usize],
    pub reference_norms: &'a [f64],
    pub response: &'a [f64],
    pub weights: BorrowedMapWeights<'a>,
    pub weight_scale: PreparedOlsBreadWeightScale,
}

pub(super) struct RefinementWorkspace<'a> {
    pub coefficients: &'a mut [f64],
    pub bread: &'a [f64],
    pub row: &'a mut [f64],
    pub scores: &'a mut [f64],
    pub corrections: &'a mut [f64],
}

pub(super) struct RefinementOutput<'a> {
    pub coefficients: &'a mut [f64],
    pub residuals: &'a mut [f64],
    pub weighted_residual_square: &'a mut f64,
}

impl RefinementOutput<'_> {
    fn begin_pass(&mut self, frame: &RefinementFrame<'_>, coefficients: &[f64]) {
        for (column, &position) in frame.active_positions.iter().enumerate() {
            self.coefficients[column] = coefficients[column] / frame.reference_norms[position];
        }
        *self.weighted_residual_square = 0.0;
    }

    fn observe(&mut self, frame: &RefinementFrame<'_>, row: usize, values: &[f64], weight: f64) {
        // Output arithmetic has its own rounding order; the independent score
        // residual above must never be substituted for this fitted-value sum.
        let mut fitted = 0.0_f64;
        for (column, &position) in frame.active_positions.iter().enumerate() {
            fitted = self.coefficients[column].mul_add(values[position], fitted);
        }
        let residual = frame.response[row] - fitted;
        self.residuals[row] = residual;
        *self.weighted_residual_square =
            (weight * residual).mul_add(residual, *self.weighted_residual_square);
    }
}

pub(crate) fn coefficient_accuracy_scale(left: f64, right: f64) -> f64 {
    let magnitude = left.abs().max(right.abs());
    (1e-3 + magnitude).min(magnitude.max(1.0))
}

pub(super) fn refine_retained_coefficients(
    frame: RefinementFrame<'_>,
    workspace: RefinementWorkspace<'_>,
    tolerance: f64,
    mut output: Option<RefinementOutput<'_>>,
) -> Result<()> {
    let rank = frame.active_positions.len();
    if !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "retained coefficient accuracy tolerance is invalid",
        ));
    }
    // With no finite coefficients there is no correction to estimate, and
    // the session need not materialize a finite-design row bank.
    if rank == 0 {
        return Ok(());
    }
    let stored = frame.storage.columns();
    if frame.response.len() != frame.storage.rows()
        || frame.reference_norms.len() != stored
        || workspace.coefficients.len() != rank
        || rank.checked_mul(rank) != Some(workspace.bread.len())
        || workspace.row.len() != stored
        || workspace.scores.len() != rank
        || workspace.corrections.len() != rank
        || output.as_ref().is_some_and(|output| {
            output.coefficients.len() != rank || output.residuals.len() != frame.response.len()
        })
        || frame.active_positions.iter().any(|&position| {
            position >= stored
                || !frame.reference_norms[position].is_finite()
                || frame.reference_norms[position] <= 0.0
        })
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "retained coefficient refinement inputs are invalid",
        ));
    }
    let mut reader = frame.storage.reader();
    // Recomputed row residuals expose errors hidden by solving and checking
    // the same rounded Gram/RHS. Existing admitted banks hold all scratch.
    for pass in 0..=3 {
        workspace.scores.fill(0.0);
        workspace.corrections.fill(0.0);
        if let Some(output) = output.as_mut() {
            output.begin_pass(&frame, workspace.coefficients);
        }
        for (row, &response) in frame.response.iter().enumerate() {
            reader.fill_row(row, workspace.row)?;
            let mut residual = response;
            for (column, &position) in frame.active_positions.iter().enumerate() {
                let value = workspace.row[position] / frame.reference_norms[position];
                residual = (-workspace.coefficients[column]).mul_add(value, residual);
            }
            let weight = normalized_solver_weight(frame.weights, frame.weight_scale, row)?;
            if let Some(output) = output.as_mut() {
                output.observe(&frame, row, workspace.row, weight);
            }
            for (column, &position) in frame.active_positions.iter().enumerate() {
                let value =
                    weight * (workspace.row[position] / frame.reference_norms[position]) * residual;
                let sum = workspace.scores[column];
                let next = sum + value;
                workspace.corrections[column] += if sum.abs() >= value.abs() {
                    (sum - next) + value
                } else {
                    (value - next) + sum
                };
                workspace.scores[column] = next;
            }
        }
        for (score, &correction) in workspace
            .scores
            .iter_mut()
            .zip(workspace.corrections.iter())
        {
            *score += correction;
        }
        let mut maximum = 0.0_f64;
        for (column, &position) in frame.active_positions.iter().enumerate() {
            let (mut sum, mut correction) = (0.0_f64, 0.0_f64);
            for (entry, &score) in workspace.bread[column * rank..(column + 1) * rank]
                .iter()
                .zip(workspace.scores.iter())
            {
                let value = entry * score;
                let next = sum + value;
                correction += if sum.abs() >= value.abs() {
                    (sum - next) + value
                } else {
                    (value - next) + sum
                };
                sum = next;
            }
            let delta = sum + correction;
            let before = workspace.coefficients[column] / frame.reference_norms[position];
            let after = (workspace.coefficients[column] + delta) / frame.reference_norms[position];
            let relative = (delta / frame.reference_norms[position]).abs()
                / coefficient_accuracy_scale(before, after);
            if !relative.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "retained coefficient refinement is nonfinite",
                ));
            }
            workspace.row[column] = delta;
            maximum = maximum.max(relative);
        }
        if maximum <= tolerance {
            return Ok(());
        }
        if pass == 3 {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "retained coefficient refinement exhausted its accuracy budget",
            ));
        }
        for (coefficient, &delta) in workspace.coefficients.iter_mut().zip(workspace.row.iter()) {
            *coefficient += delta;
        }
    }
    unreachable!("bounded refinement returns at its final pass")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ols::prepared_scratch::PreparedOlsScratch;

    fn exercise_output(
        initial: f64,
        bread_multiplier: f64,
        scratch: bool,
        output: bool,
    ) -> (Result<()>, f64, Vec<f64>, f64, u64) {
        let n = 2051;
        let values: Vec<_> = (0..n)
            .flat_map(|i| [13.0, (i % 71 + 1) as f64, -7.0])
            .collect();
        let response: Vec<_> = (0..n).map(|i| values[3 * i + 1] * 0.125).collect();
        let weights: Vec<_> = (0..n).map(|i| (i % 7 + 1) as f64).collect();
        let storage = if scratch {
            let mut scratch = PreparedOlsScratch::create_row_major(n, 3).unwrap();
            scratch.write_linear(0, &values).unwrap();
            RetainedWithinStorage::Scratch(scratch)
        } else {
            RetainedWithinStorage::Dense(
                crate::DenseMatrix::from_row_major(n, 3, values.clone()).unwrap(),
            )
        };
        let gram: f64 = (0..n)
            .map(|i| weights[i] / 5.0 * (values[3 * i + 1] / 3.0).powi(2))
            .sum();
        let mut coefficients = [initial];
        let mut row = [0.0; 3];
        let mut scores = [0.0];
        let mut corrections = [0.0];
        let mut output_coefficients = [0.0];
        let mut residuals = vec![f64::NAN; n];
        let mut rss = f64::NAN;
        let frame = RefinementFrame {
            storage: &storage,
            active_positions: &[1],
            reference_norms: &[1.0, 3.0, 1.0],
            response: &response,
            weights: BorrowedMapWeights::Provided(&weights),
            weight_scale: PreparedOlsBreadWeightScale {
                weight_kind: None,
                solver_weight_divisor: 5.0,
                reference_weight_multiplier: 1.0,
                reference_weight_sum: n as f64,
            },
        };
        let result = refine_retained_coefficients(
            frame,
            RefinementWorkspace {
                coefficients: &mut coefficients,
                bread: &[bread_multiplier / gram],
                row: &mut row,
                scores: &mut scores,
                corrections: &mut corrections,
            },
            1e-12,
            output.then_some(RefinementOutput {
                coefficients: &mut output_coefficients,
                residuals: &mut residuals,
                weighted_residual_square: &mut rss,
            }),
        );
        let mut reads = match &storage {
            RetainedWithinStorage::Scratch(scratch) => scratch.shared_read_chunks_for_test(),
            _ => 0,
        };
        if result.is_ok() && !output {
            rss = 0.0;
            let mut reader = storage.reader();
            let coefficient = coefficients[0] / 3.0;
            for i in 0..n {
                reader.fill_row(i, &mut row).unwrap();
                let fitted = coefficient.mul_add(row[1], 0.0);
                residuals[i] = response[i] - fitted;
                rss = ((weights[i] / 5.0) * residuals[i]).mul_add(residuals[i], rss);
            }
            reads = match &storage {
                RetainedWithinStorage::Scratch(scratch) => scratch.shared_read_chunks_for_test(),
                _ => 0,
            };
        }
        (result, coefficients[0], residuals, rss, reads)
    }

    #[test]
    fn frozen_output_preserves_arithmetic_after_first_and_corrected_passes() {
        // Exact binary optimum and integer design: independent of either score
        // summation or the published residual calculation.
        assert_ne!(
            (-0.375_f64).mul_add(5.0 / 3.0, 0.625).to_bits(),
            0.0_f64.to_bits()
        );
        for scratch in [false, true] {
            for initial in [0.375, 0.675] {
                let separate = exercise_output(initial, 1.0, scratch, false);
                let fused = exercise_output(initial, 1.0, scratch, true);
                separate.0.unwrap();
                fused.0.unwrap();
                assert!((fused.1 / 3.0 - 0.125).abs() < 1e-14);
                assert_eq!(separate.1.to_bits(), fused.1.to_bits());
                assert!(separate
                    .2
                    .iter()
                    .zip(&fused.2)
                    .all(|(a, b)| a.to_bits() == b.to_bits()));
                assert_eq!(separate.3.to_bits(), fused.3.to_bits());
                if scratch {
                    assert_eq!(separate.4 - fused.4, (2051_u64 * 3).div_ceil(1024));
                }
            }
        }
    }

    #[test]
    fn speculative_output_cannot_hide_exhausted_or_nonfinite_refinement() {
        for multiplier in [0.5, f64::INFINITY] {
            for scratch in [false, true] {
                let separate = exercise_output(0.675, multiplier, scratch, false);
                let fused = exercise_output(0.675, multiplier, scratch, true);
                let before = separate.0.unwrap_err();
                let after = fused.0.unwrap_err();
                assert_eq!(before.code, after.code);
                assert_eq!(before.message, after.message);
                assert_eq!(separate.1.to_bits(), fused.1.to_bits());
                assert_eq!(separate.4, fused.4);
            }
        }
    }

    #[test]
    fn fused_refinement_validates_corrupt_omitted_scratch_values() {
        use std::io::{Seek, SeekFrom, Write};
        let mut scratch = PreparedOlsScratch::create_row_major(2, 2).unwrap();
        scratch.write_linear(0, &[1.0, 2.0, 3.0, 4.0]).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(scratch.identity_path())
            .unwrap();
        file.seek(SeekFrom::Start(8)).unwrap();
        file.write_all(&f64::NAN.to_le_bytes()).unwrap();
        let storage = RetainedWithinStorage::Scratch(scratch);
        for output in [false, true] {
            let mut coefficients = [0.5];
            let mut row = [0.0; 2];
            let mut scores = [0.0];
            let mut corrections = [0.0];
            let mut output_coefficients = [0.0];
            let mut residuals = [0.0; 2];
            let mut rss = 0.0;
            let error = refine_retained_coefficients(
                RefinementFrame {
                    storage: &storage,
                    active_positions: &[0],
                    reference_norms: &[1.0, 1.0],
                    response: &[0.5, 1.5],
                    weights: BorrowedMapWeights::Unit,
                    weight_scale: PreparedOlsBreadWeightScale {
                        weight_kind: None,
                        solver_weight_divisor: 1.0,
                        reference_weight_multiplier: 1.0,
                        reference_weight_sum: 2.0,
                    },
                },
                RefinementWorkspace {
                    coefficients: &mut coefficients,
                    bread: &[0.1],
                    row: &mut row,
                    scores: &mut scores,
                    corrections: &mut corrections,
                },
                1e-12,
                output.then_some(RefinementOutput {
                    coefficients: &mut output_coefficients,
                    residuals: &mut residuals,
                    weighted_residual_square: &mut rss,
                }),
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert_eq!(
                error.message,
                "prepared OLS scratch contains a nonfinite value"
            );
        }
    }
}
