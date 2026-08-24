//! Allocation-bounded certified MAP for intercept-plus-one-slope effects.
//!
//! The finalized categorical topology remains the sole owner of compact codes
//! and membership.  Continuous slope columns are borrowed from the prepared
//! numeric sample.  All numerical workspace is reserved before the first
//! projection; iterations and final certificates allocate nothing.

use std::time::Instant;

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};

use super::{
    BorrowedMapWeights, CertifiedBorrowedMap, FeCertificate, FeSolverKind,
    FinalizedCategoricalTopology, MapConfig, SolveReport, SolveStopReason,
};

#[derive(Debug, Clone, Copy)]
struct ValidatedWeights<'a> {
    values: Option<&'a [f64]>,
    raw_scale: f64,
}

impl ValidatedWeights<'_> {
    fn value(self, row: usize) -> f64 {
        self.values
            .map_or(1.0, |values| values[row] / self.raw_scale)
    }
}

trait SlopeWorkspaceReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        elements: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemSlopeWorkspaceReservation;

impl SlopeWorkspaceReservation for SystemSlopeWorkspaceReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        elements: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(elements)
            .map_err(|_| allocation(label))
    }
}

struct SlopeWorkspace {
    reference_norms: Vec<f64>,
    residual_norms: Vec<f64>,
    rhs_scales: Vec<f64>,
    value_scales: Vec<f64>,
    channel_scales: Vec<f64>,
    gram: Vec<f64>,
    gram_corrections: Vec<f64>,
    cross: Vec<f64>,
    cross_corrections: Vec<f64>,
    coefficients: Vec<f64>,
}

impl SlopeWorkspace {
    fn new<R: SlopeWorkspaceReservation>(rhs: usize, reservation: &mut R) -> Result<Self> {
        let two_rhs = rhs
            .checked_mul(2)
            .ok_or_else(|| allocation("intercept-slope MAP RHS workspace overflows"))?;
        Ok(Self {
            reference_norms: reserved_zeros(
                rhs,
                reservation,
                "unable to reserve intercept-slope MAP reference norms",
            )?,
            residual_norms: reserved_zeros(
                rhs,
                reservation,
                "unable to reserve intercept-slope MAP residual norms",
            )?,
            rhs_scales: reserved_zeros(
                rhs,
                reservation,
                "unable to reserve intercept-slope MAP RHS scales",
            )?,
            value_scales: reserved_zeros(
                rhs,
                reservation,
                "unable to reserve intercept-slope MAP value scales",
            )?,
            channel_scales: reserved_zeros(
                2,
                reservation,
                "unable to reserve intercept-slope MAP channel scales",
            )?,
            gram: reserved_zeros(
                4,
                reservation,
                "unable to reserve intercept-slope MAP Gram matrix",
            )?,
            gram_corrections: reserved_zeros(
                4,
                reservation,
                "unable to reserve intercept-slope MAP Gram corrections",
            )?,
            cross: reserved_zeros(
                two_rhs,
                reservation,
                "unable to reserve intercept-slope MAP cross-products",
            )?,
            cross_corrections: reserved_zeros(
                two_rhs,
                reservation,
                "unable to reserve intercept-slope MAP cross-product corrections",
            )?,
            coefficients: reserved_zeros(
                two_rhs,
                reservation,
                "unable to reserve intercept-slope MAP coefficients",
            )?,
        })
    }
}

/// Residualize finalized categorical effects in which every term contains an
/// intercept and the complete request contains at most one slope channel.
pub fn absorb_finalized_categorical_intercept_slope_map_certified(
    topology: &FinalizedCategoricalTopology,
    slopes: &[Option<&[f64]>],
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemSlopeWorkspaceReservation;
    absorb_with_reservation(topology, slopes, weights, values, config, &mut reservation)
}

fn absorb_with_reservation<R: SlopeWorkspaceReservation>(
    topology: &FinalizedCategoricalTopology,
    slopes: &[Option<&[f64]>],
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config)?;
    topology.validate()?;
    let nobs = topology.retained_rows().len();
    if values.nrows() != nobs || slopes.len() != topology.effect_to_topology().len() {
        return Err(invalid_argument(
            "intercept-slope MAP topology, slope, and value dimensions differ",
        ));
    }
    if nobs == 0 || values.ncols() == 0 {
        return Err(invalid_data(
            "intercept-slope MAP requires nonempty observation and RHS axes",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("intercept-slope MAP values must be finite"));
    }
    let mut slope_count = 0_usize;
    for slope in slopes.iter().flatten() {
        slope_count = slope_count
            .checked_add(1)
            .ok_or_else(|| allocation("intercept-slope MAP slope count overflows"))?;
        if slope.len() != nobs || slope.iter().any(|value| !value.is_finite()) {
            return Err(invalid_data(
                "intercept-slope MAP slope values are not finite and aligned",
            ));
        }
    }
    if slope_count > 1 {
        return Err(unsupported(
            "intercept-slope MAP currently supports at most one slope channel",
        ));
    }
    let weights = validate_weights(weights, nobs)?;

    let setup_started = Instant::now();
    let mut workspace = SlopeWorkspace::new(values.ncols(), reservation)?;
    weighted_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.coefficients[..values.ncols()],
    )?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();

    let initial = certify(
        topology,
        slopes,
        weights,
        &values,
        config.tolerance,
        &mut workspace,
    )?;
    if initial.passed {
        return finish(
            values,
            0,
            initial,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    for iteration in 1..=config.max_iterations {
        for (effect, slope) in slopes.iter().copied().enumerate() {
            project_effect(
                topology,
                effect,
                slope,
                weights,
                &mut values,
                &mut workspace,
            )?;
        }
        if config.symmetric && slopes.len() > 1 {
            for (effect, slope) in slopes[..slopes.len() - 1].iter().copied().enumerate().rev() {
                project_effect(
                    topology,
                    effect,
                    slope,
                    weights,
                    &mut values,
                    &mut workspace,
                )?;
            }
        }
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            let certificate = certify(
                topology,
                slopes,
                weights,
                &values,
                config.tolerance,
                &mut workspace,
            )?;
            if certificate.passed {
                return finish(
                    values,
                    iteration,
                    certificate,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                );
            }
        }
    }
    Err(invalid_data(
        "intercept-slope MAP exhausted its iteration budget without certification",
    ))
}

fn project_effect(
    topology: &FinalizedCategoricalTopology,
    effect: usize,
    slope: Option<&[f64]>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut SlopeWorkspace,
) -> Result<()> {
    let source =
        topology
            .registry()
            .source(*topology.effect_to_topology().get(effect).ok_or_else(|| {
                invalid_argument("intercept-slope MAP effect index is out of range")
            })?)?;
    for level in 0..source.n_levels() {
        let members = source.members(level)?;
        group_moments(members, slope, weights, values, workspace)?;
        solve_group(slope.is_some(), values.ncols(), workspace)?;
        for &row in members {
            for rhs in 0..values.ncols() {
                let fitted = workspace.coefficients[rhs]
                    + slope.map_or(0.0, |slope| {
                        (slope[row] / workspace.channel_scales[1])
                            * workspace.coefficients[values.ncols() + rhs]
                    });
                let residual = values[(row, rhs)] - fitted;
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "intercept-slope MAP projection produced a nonfinite residual",
                    ));
                }
                values[(row, rhs)] = residual;
            }
        }
    }
    Ok(())
}

fn group_moments(
    members: &[usize],
    slope: Option<&[f64]>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    workspace: &mut SlopeWorkspace,
) -> Result<()> {
    if members.is_empty() {
        return Err(invalid_argument(
            "intercept-slope MAP contains an empty final level",
        ));
    }
    workspace.gram.fill(0.0);
    workspace.gram_corrections.fill(0.0);
    workspace.cross.fill(0.0);
    workspace.cross_corrections.fill(0.0);
    workspace.value_scales.fill(0.0);
    workspace.channel_scales[0] = 1.0;
    workspace.channel_scales[1] = slope.map_or(1.0, |values| {
        members
            .iter()
            .map(|row| values[*row].abs())
            .fold(0.0_f64, f64::max)
    });
    if slope.is_some() && workspace.channel_scales[1] == 0.0 {
        return Err(invalid_data(
            "intercept-slope MAP slope is zero throughout a final level",
        ));
    }
    for &row in members {
        if row >= values.nrows() {
            return Err(invalid_argument(
                "intercept-slope MAP membership exceeds the observation axis",
            ));
        }
        for rhs in 0..values.ncols() {
            workspace.value_scales[rhs] = workspace.value_scales[rhs].max(values[(row, rhs)].abs());
        }
    }
    let channels = 1 + usize::from(slope.is_some());
    for &row in members {
        let weight = weights.value(row);
        let basis = [
            1.0,
            slope.map_or(0.0, |values| values[row] / workspace.channel_scales[1]),
        ];
        for left in 0..channels {
            for right in 0..channels {
                compensated_add(
                    weight * basis[left] * basis[right],
                    &mut workspace.gram[left * 2 + right],
                    &mut workspace.gram_corrections[left * 2 + right],
                )?;
            }
            for rhs in 0..values.ncols() {
                let scale = workspace.value_scales[rhs];
                if scale != 0.0 {
                    compensated_add(
                        weight * basis[left] * (values[(row, rhs)] / scale),
                        &mut workspace.cross[left * values.ncols() + rhs],
                        &mut workspace.cross_corrections[left * values.ncols() + rhs],
                    )?;
                }
            }
        }
    }
    for index in 0..4 {
        workspace.gram[index] += workspace.gram_corrections[index];
    }
    for index in 0..channels * values.ncols() {
        workspace.cross[index] += workspace.cross_corrections[index];
    }
    Ok(())
}

fn solve_group(has_slope: bool, rhs_count: usize, workspace: &mut SlopeWorkspace) -> Result<()> {
    let g00 = workspace.gram[0];
    if !g00.is_finite() || g00 <= 0.0 {
        return Err(invalid_data(
            "intercept-slope MAP group Gram intercept is invalid",
        ));
    }
    if !has_slope {
        for rhs in 0..rhs_count {
            workspace.coefficients[rhs] = workspace.value_scales[rhs] * workspace.cross[rhs] / g00;
        }
        return Ok(());
    }
    let g01 = 0.5 * (workspace.gram[1] + workspace.gram[2]);
    let g11 = workspace.gram[3];
    let determinant = g00.mul_add(g11, -(g01 * g01));
    let scale = g00.abs().max(g01.abs()).max(g11.abs()).max(1.0);
    if !determinant.is_finite() || determinant <= f64::EPSILON * scale * scale * 32.0 {
        return Err(invalid_data(
            "intercept-slope MAP group basis is numerically rank deficient",
        ));
    }
    for rhs in 0..rhs_count {
        let left = workspace.cross[rhs];
        let right = workspace.cross[rhs_count + rhs];
        let value_scale = workspace.value_scales[rhs];
        workspace.coefficients[rhs] = value_scale * (g11 * left - g01 * right) / determinant;
        workspace.coefficients[rhs_count + rhs] =
            value_scale * (g00 * right - g01 * left) / determinant;
    }
    Ok(())
}

fn certify(
    topology: &FinalizedCategoricalTopology,
    slopes: &[Option<&[f64]>],
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut SlopeWorkspace,
) -> Result<FeCertificate> {
    weighted_norms(
        residuals,
        weights,
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.coefficients[..residuals.ncols()],
    )?;
    for rhs in 0..residuals.ncols() {
        workspace.rhs_scales[rhs] =
            workspace.reference_norms[rhs].max(workspace.residual_norms[rhs]);
    }
    let mut absolute = 0.0_f64;
    let mut relative_infinity = 0.0_f64;
    let mut relative_squares = 0.0_f64;
    let mut count = 0_usize;
    for (effect, slope) in slopes.iter().copied().enumerate() {
        let source = topology
            .registry()
            .source(topology.effect_to_topology()[effect])?;
        for level in 0..source.n_levels() {
            group_moments(source.members(level)?, slope, weights, residuals, workspace)?;
            solve_group(slope.is_some(), residuals.ncols(), workspace)?;
            let channels = 1 + usize::from(slope.is_some());
            for rhs in 0..residuals.ncols() {
                let mut projected_square = 0.0_f64;
                for left in 0..channels {
                    for right in 0..channels {
                        projected_square = workspace.coefficients[left * residuals.ncols() + rhs]
                            .mul_add(
                                workspace.gram[left * 2 + right]
                                    * workspace.coefficients[right * residuals.ncols() + rhs],
                                projected_square,
                            );
                    }
                }
                let projected = projected_square.max(0.0).sqrt();
                let denominator = workspace.rhs_scales[rhs];
                let relative = if denominator == 0.0 {
                    if projected == 0.0 {
                        0.0
                    } else {
                        f64::INFINITY
                    }
                } else {
                    projected / denominator
                };
                if !relative.is_finite() {
                    return Err(invalid_data(
                        "intercept-slope MAP certificate ratio is not finite",
                    ));
                }
                relative_infinity = relative_infinity.max(relative);
                relative_squares = relative.mul_add(relative, relative_squares);
                count = count
                    .checked_add(1)
                    .ok_or_else(|| allocation("intercept-slope MAP certificate count overflows"))?;
                for channel in 0..channels {
                    let raw = workspace.cross[channel * residuals.ncols() + rhs].abs()
                        * workspace.value_scales[rhs]
                        * workspace.channel_scales[channel]
                        * weights.raw_scale;
                    absolute = absolute.max(if raw.is_finite() { raw } else { f64::INFINITY });
                }
            }
        }
    }
    let relative_two = if count == 0 {
        0.0
    } else {
        (relative_squares / count as f64).sqrt()
    };
    Ok(FeCertificate {
        absolute_infinity_norm: absolute,
        relative_infinity_norm: relative_infinity,
        relative_two_norm: relative_two,
        passed: relative_infinity <= tolerance,
    })
}

fn weighted_norms(
    values: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    output: &mut [f64],
    scales: &mut [f64],
    sums: &mut [f64],
) -> Result<()> {
    if output.len() != values.ncols()
        || scales.len() != values.ncols()
        || sums.len() != values.ncols()
    {
        return Err(invalid_argument(
            "intercept-slope MAP norm workspace dimensions differ",
        ));
    }
    scales.fill(0.0);
    sums.fill(1.0);
    for row in 0..values.nrows() {
        let root_weight = weights.value(row).sqrt();
        for rhs in 0..values.ncols() {
            let value = root_weight * values[(row, rhs)];
            if !value.is_finite() {
                return Err(invalid_data(
                    "intercept-slope MAP weighted value is not finite",
                ));
            }
            update_scaled_sum(value, &mut scales[rhs], &mut sums[rhs]);
        }
    }
    for rhs in 0..values.ncols() {
        output[rhs] = if scales[rhs] == 0.0 {
            0.0
        } else {
            scales[rhs] * sums[rhs].sqrt()
        };
        if !output[rhs].is_finite() {
            return Err(invalid_data(
                "intercept-slope MAP weighted norm is not finite",
            ));
        }
    }
    Ok(())
}

fn validate_weights(weights: BorrowedMapWeights<'_>, nobs: usize) -> Result<ValidatedWeights<'_>> {
    match weights {
        BorrowedMapWeights::Unit => Ok(ValidatedWeights {
            values: None,
            raw_scale: 1.0,
        }),
        BorrowedMapWeights::Provided(values) => {
            if values.len() != nobs
                || values
                    .iter()
                    .any(|value| !value.is_finite() || *value <= 0.0)
            {
                return Err(invalid_data(
                    "intercept-slope MAP weights must be positive, finite, and aligned",
                ));
            }
            let raw_scale = values.iter().copied().fold(0.0_f64, f64::max);
            if values.iter().any(|value| *value / raw_scale == 0.0) {
                return Err(invalid_data(
                    "intercept-slope MAP normalized weight underflowed to zero",
                ));
            }
            Ok(ValidatedWeights {
                values: Some(values),
                raw_scale,
            })
        }
    }
}

fn validate_config(config: MapConfig) -> Result<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(invalid_argument(
            "intercept-slope MAP tolerance must be finite and positive",
        ));
    }
    if config.max_iterations == 0 || config.check_every == 0 {
        return Err(invalid_argument(
            "intercept-slope MAP iteration budgets must be positive",
        ));
    }
    Ok(())
}

fn finish(
    values: DenseMatrix,
    iterations: usize,
    certificate: FeCertificate,
    setup_seconds: f64,
    solve_seconds: f64,
) -> Result<CertifiedBorrowedMap> {
    CertifiedBorrowedMap::from_certified_parts(
        values,
        SolveReport {
            requested_solver: FeSolverKind::Map,
            used_solver: FeSolverKind::Map,
            iterations,
            stop_reason: SolveStopReason::Certified,
            normal_equation_residual: certificate.relative_two_norm,
            certificate,
            setup_seconds,
            solve_seconds,
            fallback_used: false,
            warnings: Vec::new(),
        },
    )
}

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    let updated = *sum + value;
    let adjustment = if sum.abs() >= value.abs() {
        (*sum - updated) + value
    } else {
        (value - updated) + *sum
    };
    *sum = updated;
    *correction += adjustment;
    if !sum.is_finite() || !correction.is_finite() {
        return Err(invalid_data(
            "intercept-slope MAP compensated accumulation is not finite",
        ));
    }
    Ok(())
}

fn update_scaled_sum(value: f64, scale: &mut f64, sum: &mut f64) {
    let absolute = value.abs();
    if absolute == 0.0 {
        return;
    }
    if *scale < absolute {
        let ratio = *scale / absolute;
        *sum = 1.0 + *sum * ratio * ratio;
        *scale = absolute;
    } else {
        let ratio = absolute / *scale;
        *sum += ratio * ratio;
    }
}

fn reserved_zeros<R: SlopeWorkspaceReservation>(
    elements: usize,
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<f64>> {
    let mut output = Vec::new();
    reservation.reserve_f64(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "intercept-slope MAP workspace reservation returned insufficient capacity",
        ));
    }
    output.resize(elements, 0.0);
    Ok(output)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

fn unsupported(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Unsupported, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{finalize_categorical_topology, CategoricalCodeView};

    fn config() -> MapConfig {
        MapConfig {
            tolerance: 1e-11,
            max_iterations: 20_000,
            check_every: 1,
            symmetric: true,
        }
    }

    #[derive(Default)]
    struct InjectedReservation {
        calls: usize,
        fail_at: Option<usize>,
        undersize_at: Option<usize>,
    }

    impl SlopeWorkspaceReservation for InjectedReservation {
        fn reserve_f64(
            &mut self,
            target: &mut Vec<f64>,
            elements: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected intercept-slope MAP allocation failure",
                ));
            }
            if self.undersize_at != Some(call) {
                target
                    .try_reserve_exact(elements)
                    .map_err(|_| allocation("test intercept-slope MAP reservation failed"))?;
            }
            Ok(())
        }
    }

    #[test]
    fn intercept_plus_slope_and_second_intercept_effect_match_explicit_span() {
        let group = [0, 0, 0, 1, 1, 1, 2, 2, 2];
        let second = [0, 1, 2, 0, 1, 2, 0, 1, 2];
        let sources = [
            CategoricalCodeView::new(&group, 3),
            CategoricalCodeView::new(&second, 3),
        ];
        let topology =
            finalize_categorical_topology(&sources, &[&[0_usize][..], &[1_usize][..]], &[true; 9])
                .expect("topology");
        let slope = [0.0, 1.0, 2.0, 0.0, 2.0, 3.0, 1.0, 2.0, 4.0];
        let values = DenseMatrix::from_columns(&[
            (0..9)
                .map(|row| {
                    3.0 + 0.5 * group[row] as f64
                        + (group[row] as f64 + 1.0) * slope[row]
                        + 0.25 * second[row] as f64
                })
                .collect::<Vec<_>>(),
            (0..9).map(|row| slope[row] + second[row] as f64).collect(),
        ])
        .expect("values");
        let result = absorb_finalized_categorical_intercept_slope_map_certified(
            &topology,
            &[Some(&slope), None],
            BorrowedMapWeights::Unit,
            values,
            config(),
        )
        .expect("certified MAP");
        assert!(result.report().certified());
        assert!(result
            .residualized()
            .as_slice()
            .iter()
            .all(|value| value.abs() < 1e-9));
    }

    #[test]
    fn zero_within_level_slope_and_multiple_slope_channels_fail_closed() {
        let group = [0, 0, 1, 1];
        let sources = [CategoricalCodeView::new(&group, 2)];
        let topology =
            finalize_categorical_topology(&sources, &[&[0]], &[true; 4]).expect("topology");
        let values = DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("values");
        let zero = [0.0; 4];
        let error = absorb_finalized_categorical_intercept_slope_map_certified(
            &topology,
            &[Some(&zero)],
            BorrowedMapWeights::Unit,
            values.clone(),
            config(),
        )
        .expect_err("zero slope");
        assert_eq!(error.code, ErrorCode::InvalidData);

        let repeated = finalize_categorical_topology(&sources, &[&[0], &[0]], &[true; 4])
            .expect("repeated topology");
        let slope = [0.0, 1.0, 0.0, 1.0];
        let error = absorb_finalized_categorical_intercept_slope_map_certified(
            &repeated,
            &[Some(&slope), Some(&slope)],
            BorrowedMapWeights::Unit,
            values,
            config(),
        )
        .expect_err("multiple slopes");
        assert_eq!(error.code, ErrorCode::Unsupported);
    }

    #[test]
    fn every_workspace_reservation_and_capacity_guard_fails_closed() {
        let group = [0, 0, 0, 1, 1, 1];
        let topology = finalize_categorical_topology(
            &[CategoricalCodeView::new(&group, 2)],
            &[&[0]],
            &[true; 6],
        )
        .expect("topology");
        let slope = [0.0, 1.0, 2.0, 0.0, 1.5, 3.0];
        let values =
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 4.0, 3.0, 5.0, 8.0]]).expect("values");
        let mut successful = InjectedReservation::default();
        absorb_with_reservation(
            &topology,
            &[Some(&slope)],
            BorrowedMapWeights::Unit,
            values.clone(),
            config(),
            &mut successful,
        )
        .expect("allocation audit baseline");
        assert!(successful.calls > 0);
        for call in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(call),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &topology,
                &[Some(&slope)],
                BorrowedMapWeights::Unit,
                values.clone(),
                config(),
                &mut injected,
            )
            .expect_err("injected allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");

            let mut undersized = InjectedReservation {
                undersize_at: Some(call),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &topology,
                &[Some(&slope)],
                BorrowedMapWeights::Unit,
                values.clone(),
                config(),
                &mut undersized,
            )
            .expect_err("undersized allocation");
            assert_eq!(error.code, ErrorCode::Allocation, "call {call}");
        }
    }
}
