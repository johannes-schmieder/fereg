use crate::data::{DenseMatrix, PreparedWeights};
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::{
    absorb_borrowed_intercept_map_with_structural_constant_certified, BorrowedMapWeights, MapConfig,
};

use super::prepared_unadjusted::{compensated_add, relative_difference, ReferenceWeightView};
use super::{CertifiedPreparedOlsFit, PreparedOlsBreadCoordinateKind, PreparedOlsData};

pub(super) trait PreparedInferenceReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;
}

pub(super) struct SystemPreparedInferenceReservation;

impl PreparedInferenceReservation for SystemPreparedInferenceReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PreparedOlsScoreDesignCertificate {
    pub left_inverse_relative_infinity_norm: f64,
    pub right_inverse_relative_infinity_norm: f64,
    pub intercept_coordinate_relative_infinity_norm: f64,
    pub passed: bool,
}

/// Ephemeral, kept-coordinate score design. It is deliberately consumed by
/// robust inference and is never retained in the final certified result.
#[derive(Debug, PartialEq)]
pub(super) struct CertifiedPreparedOlsScoreDesign {
    pub(super) within: DenseMatrix,
    means: Vec<f64>,
    finite_positions: Vec<usize>,
    normalized_intercept_position: Option<usize>,
    pub(super) certificate: PreparedOlsScoreDesignCertificate,
}

impl CertifiedPreparedOlsScoreDesign {
    pub(super) const fn nrows(&self) -> usize {
        self.within.nrows()
    }

    pub(super) const fn finite_rank(&self) -> usize {
        self.within.ncols()
    }

    pub(super) fn finite_value(&self, row: usize, finite: usize) -> f64 {
        self.within[(row, finite)]
    }

    pub(super) fn reported_value(&self, row: usize, position: usize) -> Result<f64> {
        if self.normalized_intercept_position == Some(position) {
            return Ok(1.0);
        }
        let finite = self
            .finite_positions
            .iter()
            .position(|candidate| *candidate == position)
            .ok_or_else(|| invalid_argument("prepared robust score coordinate is absent"))?;
        let value = self.finite_value(row, finite)
            + if self.normalized_intercept_position.is_some() {
                self.means[finite]
            } else {
                0.0
            };
        if value.is_finite() {
            Ok(value)
        } else {
            Err(invalid_data(
                "prepared robust reported score coordinate is not finite",
            ))
        }
    }

    pub(super) const fn certificate(&self) -> PreparedOlsScoreDesignCertificate {
        self.certificate
    }
}

pub(super) fn build_prepared_ols_score_design<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    weights: ReferenceWeightView<'_>,
    map: MapConfig,
    tolerance: f64,
    reservation: &mut R,
) -> Result<CertifiedPreparedOlsScoreDesign> {
    let nrows = prepared.sample.n_active();
    let rank = fit.finite_rank();
    let dimension = fit.bread().dimension();
    if nrows == 0 || !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
        return Err(invalid_argument(
            "prepared robust score design has invalid sample or tolerance",
        ));
    }

    let mut generated_columns = reserved_filled(
        reservation,
        rank,
        0_usize,
        "unable to reserve prepared robust kept score identities",
    )?;
    let mut finite_positions = reserved_filled(
        reservation,
        rank,
        0_usize,
        "unable to reserve prepared robust finite score positions",
    )?;
    let mut normalized_intercept_position = None;
    let mut finite = 0_usize;
    for (position, coordinate) in fit.bread().coordinates().iter().enumerate() {
        match coordinate.kind {
            PreparedOlsBreadCoordinateKind::FiniteRegressor => {
                if finite >= rank
                    || generated_columns[..finite]
                        .iter()
                        .any(|column| *column == coordinate.generated_column_index)
                {
                    return Err(invalid_argument(
                        "prepared robust finite score coordinate is duplicated or excessive",
                    ));
                }
                generated_columns[finite] = coordinate.generated_column_index;
                finite_positions[finite] = position;
                finite = finite
                    .checked_add(1)
                    .ok_or_else(|| allocation("prepared robust finite score rank overflows"))?;
            }
            PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept => {
                if normalized_intercept_position.replace(position).is_some() {
                    return Err(invalid_argument(
                        "prepared robust score design has multiple normalized intercepts",
                    ));
                }
            }
        }
    }
    if finite != rank || dimension != rank + usize::from(normalized_intercept_position.is_some()) {
        return Err(invalid_argument(
            "prepared robust score design rank and bread coordinates disagree",
        ));
    }

    let elements = nrows
        .checked_mul(rank)
        .ok_or_else(|| allocation("prepared robust score design dimensions overflow usize"))?;
    let mut raw_values = reserved_filled(
        reservation,
        elements,
        0.0_f64,
        "unable to reserve prepared robust kept score design",
    )?;
    for row in 0..nrows {
        for column in 0..rank {
            let value = prepared.column_value(generated_columns[column], row)?;
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared robust raw score design contains a nonfinite value",
                ));
            }
            raw_values[row * rank + column] = value;
        }
    }

    let mut means = reserved_filled(
        reservation,
        rank,
        0.0_f64,
        "unable to reserve prepared robust score means",
    )?;
    for (column, mean) in means.iter_mut().enumerate() {
        *mean = weighted_matrix_column_mean(&raw_values, nrows, rank, column, weights)?;
    }

    let raw = DenseMatrix::from_row_major(nrows, rank, raw_values)?;
    let within = if rank == 0 {
        raw
    } else {
        let design = prepared.borrowed_intercept_fe_design()?;
        let map_weights = match &prepared.sample.weights {
            PreparedWeights::None => BorrowedMapWeights::Unit,
            PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
        };
        let absorbed = absorb_borrowed_intercept_map_with_structural_constant_certified(
            &design,
            map_weights,
            raw,
            map,
        )?;
        if !absorbed.report().certified() {
            return Err(invalid_data(
                "prepared robust score-design absorption did not certify",
            ));
        }
        absorbed.into_parts().0
    };

    let mut certificate = certify_score_design(
        &within,
        &means,
        &finite_positions,
        normalized_intercept_position,
        fit,
        weights,
        reservation,
    )?;
    certificate.passed = certificate.left_inverse_relative_infinity_norm <= tolerance
        && certificate.right_inverse_relative_infinity_norm <= tolerance
        && certificate.intercept_coordinate_relative_infinity_norm <= tolerance;
    if !certificate.passed {
        return Err(invalid_data(
            "prepared robust score-design certificate did not pass",
        ));
    }
    Ok(CertifiedPreparedOlsScoreDesign {
        within,
        means,
        finite_positions,
        normalized_intercept_position,
        certificate,
    })
}

fn certify_score_design<R: PreparedInferenceReservation>(
    within: &DenseMatrix,
    means: &[f64],
    finite_positions: &[usize],
    normalized_intercept_position: Option<usize>,
    fit: &CertifiedPreparedOlsFit,
    weights: ReferenceWeightView<'_>,
    reservation: &mut R,
) -> Result<PreparedOlsScoreDesignCertificate> {
    let nrows = within.nrows();
    let rank = within.ncols();
    let dimension = fit.bread().dimension();
    let square = rank
        .checked_mul(rank)
        .ok_or_else(|| allocation("prepared robust score Gram dimensions overflow usize"))?;
    if means.len() != rank || finite_positions.len() != rank {
        return Err(invalid_argument(
            "prepared robust score certificate dimensions are inconsistent",
        ));
    }
    let mut gram = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust score Gram",
    )?;
    let mut corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared robust score Gram corrections",
    )?;
    for row in 0..nrows {
        let weight = weights.value(row)?;
        for left in 0..rank {
            for right in 0..rank {
                let term = weight * within[(row, left)] * within[(row, right)];
                let position = left * rank + right;
                compensated_add(term, &mut gram[position], &mut corrections[position])?;
            }
        }
    }
    for (value, correction) in gram.iter_mut().zip(corrections) {
        *value += correction;
        if !value.is_finite() {
            return Err(invalid_data(
                "prepared robust score Gram contains a nonfinite value",
            ));
        }
    }

    let mut left_error = 0.0_f64;
    let mut right_error = 0.0_f64;
    for row in 0..rank {
        let mut left_row_error = 0.0_f64;
        let mut right_row_error = 0.0_f64;
        let mut left_scale = 1.0_f64;
        let mut right_scale = 1.0_f64;
        for column in 0..rank {
            let mut left_product = 0.0_f64;
            let mut left_correction = 0.0_f64;
            let mut right_product = 0.0_f64;
            let mut right_correction = 0.0_f64;
            for inner in 0..rank {
                let bread_left = fit.bread().values()
                    [finite_positions[row] * dimension + finite_positions[inner]];
                let bread_right = fit.bread().values()
                    [finite_positions[inner] * dimension + finite_positions[column]];
                compensated_add(
                    bread_left * gram[inner * rank + column],
                    &mut left_product,
                    &mut left_correction,
                )?;
                compensated_add(
                    gram[row * rank + inner] * bread_right,
                    &mut right_product,
                    &mut right_correction,
                )?;
            }
            left_product += left_correction;
            right_product += right_correction;
            let identity = if row == column { 1.0 } else { 0.0 };
            left_row_error += (left_product - identity).abs();
            right_row_error += (right_product - identity).abs();
            left_scale += left_product.abs();
            right_scale += right_product.abs();
        }
        left_error = left_error.max(left_row_error / left_scale);
        right_error = right_error.max(right_row_error / right_scale);
    }

    let mut intercept_error = 0.0_f64;
    if let Some(intercept) = normalized_intercept_position {
        let effective_n = fit.bread().weight_scale().reference_weight_sum;
        if !effective_n.is_finite() || effective_n <= 0.0 {
            return Err(invalid_data(
                "prepared robust score intercept has invalid effective N",
            ));
        }
        let mut quadratic = 0.0_f64;
        let mut quadratic_correction = 0.0_f64;
        for row in 0..rank {
            let mut expected_cross = 0.0_f64;
            let mut expected_correction = 0.0_f64;
            for column in 0..rank {
                let bread = fit.bread().values()
                    [finite_positions[row] * dimension + finite_positions[column]];
                compensated_add(
                    -bread * means[column],
                    &mut expected_cross,
                    &mut expected_correction,
                )?;
            }
            expected_cross += expected_correction;
            let actual_left = fit.bread().values()[finite_positions[row] * dimension + intercept];
            let actual_right = fit.bread().values()[intercept * dimension + finite_positions[row]];
            let scale = expected_cross
                .abs()
                .max(actual_left.abs())
                .max(actual_right.abs())
                .max(1.0);
            intercept_error =
                intercept_error.max(relative_difference(actual_left, expected_cross, scale)?);
            intercept_error =
                intercept_error.max(relative_difference(actual_right, expected_cross, scale)?);
            compensated_add(
                -means[row] * expected_cross,
                &mut quadratic,
                &mut quadratic_correction,
            )?;
        }
        let expected_corner = 1.0 / effective_n + quadratic + quadratic_correction;
        let actual_corner = fit.bread().values()[intercept * dimension + intercept];
        intercept_error = intercept_error.max(relative_difference(
            actual_corner,
            expected_corner,
            actual_corner.abs().max(expected_corner.abs()).max(1.0),
        )?);
    } else if dimension != rank {
        return Err(invalid_argument(
            "prepared robust score certificate has an unexplained bread coordinate",
        ));
    }
    Ok(PreparedOlsScoreDesignCertificate {
        left_inverse_relative_infinity_norm: left_error,
        right_inverse_relative_infinity_norm: right_error,
        intercept_coordinate_relative_infinity_norm: intercept_error,
        passed: false,
    })
}

fn weighted_matrix_column_mean(
    values: &[f64],
    nrows: usize,
    ncols: usize,
    column: usize,
    weights: ReferenceWeightView<'_>,
) -> Result<f64> {
    if nrows == 0 || column >= ncols || values.len() != nrows * ncols {
        return Err(invalid_argument(
            "prepared robust score mean dimensions are inconsistent",
        ));
    }
    let mut scale = 0.0_f64;
    for row in 0..nrows {
        scale = scale.max(values[row * ncols + column].abs());
    }
    if scale == 0.0 {
        return Ok(0.0);
    }
    let mut numerator = 0.0_f64;
    let mut numerator_correction = 0.0_f64;
    let mut denominator = 0.0_f64;
    let mut denominator_correction = 0.0_f64;
    for row in 0..nrows {
        let weight = weights.value(row)?;
        compensated_add(
            weight * (values[row * ncols + column] / scale),
            &mut numerator,
            &mut numerator_correction,
        )?;
        compensated_add(weight, &mut denominator, &mut denominator_correction)?;
    }
    let denominator = denominator + denominator_correction;
    let mean = scale * ((numerator + numerator_correction) / denominator);
    if mean.is_finite() && denominator.is_finite() && denominator > 0.0 {
        Ok(mean)
    } else {
        Err(invalid_data("prepared robust score mean is not finite"))
    }
}

pub(super) fn reserved_filled<T: Clone, R: PreparedInferenceReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "prepared robust reservation returned insufficient capacity",
        ));
    }
    output.resize(elements, value);
    Ok(output)
}

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidData, message)
}
