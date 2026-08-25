use std::time::Instant;

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    BorrowedInterceptFeDesign, FeCertificate, FeSolverKind, MapConfig, SolveReport, SolveStopReason,
};
use rayon::prelude::*;

const PARALLEL_PROJECTION_MIN_ROWS: usize = 262_144;

/// Objective weights for the strict borrowed-intercept MAP path.
///
/// Unit weights remain structural: the solver never materializes an
/// observation-scale vector of ones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorrowedMapWeights<'a> {
    Unit,
    Provided(&'a [f64]),
}

/// A borrowed-intercept MAP result whose construction certifies the returned
/// residualized values.
#[derive(Debug, PartialEq)]
pub struct CertifiedBorrowedMap {
    residualized: DenseMatrix,
    report: SolveReport,
}

impl CertifiedBorrowedMap {
    #[must_use]
    pub const fn residualized(&self) -> &DenseMatrix {
        &self.residualized
    }

    #[must_use]
    pub const fn report(&self) -> &SolveReport {
        &self.report
    }

    #[must_use]
    pub fn into_parts(self) -> (DenseMatrix, SolveReport) {
        (self.residualized, self.report)
    }

    pub(crate) fn from_certified_parts(
        residualized: DenseMatrix,
        report: SolveReport,
    ) -> Result<Self> {
        if !report.certified() {
            return Err(invalid_data(
                "borrowed MAP attempted to construct an uncertified result",
            ));
        }
        Ok(Self {
            residualized,
            report,
        })
    }
}

trait WorkspaceReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemWorkspaceReservation;

impl WorkspaceReservation for SystemWorkspaceReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

struct BorrowedMapWorkspace {
    reference_norms: Vec<f64>,
    residual_norms: Vec<f64>,
    rhs_scales: Vec<f64>,
    value_scales: Vec<f64>,
    group_sums: Vec<f64>,
    group_corrections: Vec<f64>,
    parallel_group_means: Vec<f64>,
}

impl BorrowedMapWorkspace {
    fn new<R: WorkspaceReservation>(
        design: &BorrowedInterceptFeDesign<'_>,
        observation_count: usize,
        rhs_count: usize,
        reservation: &mut R,
    ) -> Result<Self> {
        let parallel_group_mean_count = if observation_count >= PARALLEL_PROJECTION_MIN_ROWS
            && rhs_count > 1
            && rayon::current_num_threads() > 1
        {
            design
                .effects()
                .iter()
                .map(crate::fe::BorrowedInterceptEffect::n_levels)
                .max()
                .unwrap_or(0)
                .checked_mul(rhs_count)
                .ok_or_else(|| allocation("borrowed MAP parallel group-mean dimensions overflow"))?
        } else {
            0
        };
        Ok(Self {
            reference_norms: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP reference norms",
            )?,
            residual_norms: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP residual norms",
            )?,
            rhs_scales: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP RHS scales",
            )?,
            value_scales: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP value scales",
            )?,
            group_sums: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP group sums",
            )?,
            group_corrections: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP group corrections",
            )?,
            parallel_group_means: if parallel_group_mean_count == 0 {
                Vec::new()
            } else {
                reserved_zeros(
                    parallel_group_mean_count,
                    reservation,
                    "unable to reserve borrowed MAP parallel group means",
                )?
            },
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct ValidatedWeights<'a> {
    values: Option<&'a [f64]>,
    raw_scale: f64,
}

impl ValidatedWeights<'_> {
    #[must_use]
    fn normalized(self, observation: usize) -> f64 {
        self.values
            .map_or(1.0, |values| values[observation] / self.raw_scale)
    }
}

/// Residualize a nonempty dense matrix against a validated borrowed,
/// intercept-only FE design.
///
/// The matrix is consumed so a failure cannot expose partially residualized
/// caller state. Every numerical workspace is fallibly allocated before the
/// first projection. A successful return always has a passing independently
/// recomputed fixed-effect certificate.
pub fn absorb_borrowed_intercept_map_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_with_reservation(design, weights, values, config, false, &mut reservation)
}

/// Prepared OLS compatibility path: an empty explicit FE design still owns a
/// structural one-level constant. The global projection uses only the existing
/// dense RHS plus O(K) workspace; it never materializes N constant codes.
pub(crate) fn absorb_borrowed_intercept_map_with_structural_constant_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_with_reservation(design, weights, values, config, true, &mut reservation)
}

fn absorb_with_reservation<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    empty_design_is_structural_constant: bool,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config)?;
    if design.nobs() != values.nrows() {
        return Err(invalid_argument(
            "borrowed MAP design and matrix observation counts differ",
        ));
    }
    if values.nrows() == 0 || values.ncols() == 0 {
        return Err(invalid_data(
            "borrowed MAP requires nonempty observation and RHS axes",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("borrowed MAP values must be finite"));
    }
    let weights = validate_weights(weights, values.nrows())?;

    let setup_started = Instant::now();
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), reservation)?;
    weighted_column_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();

    if design.effects().is_empty() && !empty_design_is_structural_constant {
        return certified_result(
            values,
            SolveStopReason::NoFixedEffects,
            0,
            zero_certificate(),
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    let initial_certificate = if design.effects().is_empty() {
        certify_structural_constant(weights, &values, config.tolerance, &mut workspace)?
    } else {
        certify(design, weights, &values, config.tolerance, &mut workspace)?
    };
    if initial_certificate.passed {
        return certified_result(
            values,
            SolveStopReason::Certified,
            0,
            initial_certificate,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    if design.effects().is_empty() {
        project_structural_constant(weights, &mut values, &mut workspace)?;
        let certificate =
            certify_structural_constant(weights, &values, config.tolerance, &mut workspace)?;
        if !certificate.passed {
            return Err(invalid_data(
                "borrowed MAP structural-constant projection failed certification",
            ));
        }
        return certified_result(
            values,
            SolveStopReason::Certified,
            1,
            certificate,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    for iteration in 1..=config.max_iterations {
        for effect in design.effects() {
            project_effect(effect, weights, &mut values, &mut workspace)?;
        }
        if config.symmetric && design.effects().len() > 1 {
            for effect in design.effects().iter().rev().skip(1) {
                project_effect(effect, weights, &mut values, &mut workspace)?;
            }
        }
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            let certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
            if certificate.passed {
                return certified_result(
                    values,
                    SolveStopReason::Certified,
                    iteration,
                    certificate,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                );
            }
        }
    }

    Err(invalid_data(
        "borrowed MAP exhausted its iteration budget without a passing final certificate",
    ))
}

fn validate_config(config: MapConfig) -> Result<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(invalid_argument(
            "borrowed MAP tolerance must be finite and positive",
        ));
    }
    if config.max_iterations == 0 || config.check_every == 0 {
        return Err(invalid_argument(
            "borrowed MAP iteration and check budgets must be positive",
        ));
    }
    Ok(())
}

fn validate_weights(weights: BorrowedMapWeights<'_>, nrows: usize) -> Result<ValidatedWeights<'_>> {
    match weights {
        BorrowedMapWeights::Unit => Ok(ValidatedWeights {
            values: None,
            raw_scale: 1.0,
        }),
        BorrowedMapWeights::Provided(values) => {
            if values.len() != nrows {
                return Err(invalid_argument(
                    "borrowed MAP weight and observation counts differ",
                ));
            }
            if values
                .iter()
                .any(|weight| !weight.is_finite() || *weight <= 0.0)
            {
                return Err(invalid_data(
                    "borrowed MAP weights must be finite and positive",
                ));
            }
            let raw_scale = values.iter().copied().fold(0.0_f64, f64::max);
            if values.iter().any(|weight| *weight / raw_scale == 0.0) {
                return Err(invalid_data(
                    "borrowed MAP normalized weight underflowed to zero",
                ));
            }
            Ok(ValidatedWeights {
                values: Some(values),
                raw_scale,
            })
        }
    }
}

fn project_structural_constant(
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    structural_constant_means(weights, values, workspace)?;
    for observation in 0..values.nrows() {
        for rhs in 0..values.ncols() {
            let residual = values[(observation, rhs)] - workspace.group_sums[rhs];
            if !residual.is_finite() {
                return Err(invalid_data(
                    "borrowed MAP structural-constant projection produced a nonfinite residual",
                ));
            }
            values[(observation, rhs)] = residual;
        }
    }
    Ok(())
}

fn structural_constant_means(
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<f64> {
    workspace.value_scales.fill(0.0);
    workspace.group_sums.fill(0.0);
    workspace.group_corrections.fill(0.0);
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for observation in 0..values.nrows() {
        compensated_add(
            weights.normalized(observation),
            &mut weight_sum,
            &mut weight_correction,
        )?;
        for rhs in 0..values.ncols() {
            workspace.value_scales[rhs] =
                workspace.value_scales[rhs].max(values[(observation, rhs)].abs());
        }
    }
    let total_weight = weight_sum + weight_correction;
    if !total_weight.is_finite() || total_weight <= 0.0 {
        return Err(invalid_data(
            "borrowed MAP structural-constant weight sum is invalid",
        ));
    }
    for observation in 0..values.nrows() {
        let weight = weights.normalized(observation);
        for rhs in 0..values.ncols() {
            let scale = workspace.value_scales[rhs];
            if scale != 0.0 {
                compensated_add(
                    weight * (values[(observation, rhs)] / scale),
                    &mut workspace.group_sums[rhs],
                    &mut workspace.group_corrections[rhs],
                )?;
            }
        }
    }
    for rhs in 0..values.ncols() {
        let scale = workspace.value_scales[rhs];
        let mean = if scale == 0.0 {
            0.0
        } else {
            scale * ((workspace.group_sums[rhs] + workspace.group_corrections[rhs]) / total_weight)
        };
        if !mean.is_finite() {
            return Err(invalid_data(
                "borrowed MAP structural-constant mean is not finite",
            ));
        }
        workspace.group_sums[rhs] = mean;
    }
    Ok(total_weight)
}

fn certify_structural_constant(
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    weighted_column_norms(
        residuals,
        weights,
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    for rhs in 0..residuals.ncols() {
        let scale = workspace.reference_norms[rhs].max(workspace.residual_norms[rhs]);
        if !scale.is_finite() || scale < 0.0 {
            return Err(invalid_data(
                "borrowed MAP structural-constant RHS scale is invalid",
            ));
        }
        workspace.rhs_scales[rhs] = scale;
    }
    let group_weight = structural_constant_means(weights, residuals, workspace)?;
    let root_group_weight = group_weight.sqrt();
    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    for rhs in 0..residuals.ncols() {
        let mean = workspace.group_sums[rhs];
        let denominator = workspace.rhs_scales[rhs];
        let relative = if denominator == 0.0 {
            if mean == 0.0 {
                0.0
            } else {
                return Err(invalid_data(
                    "borrowed MAP structural-constant zero-scale certificate failed",
                ));
            }
        } else {
            (mean.abs() / denominator) * root_group_weight
        };
        if !relative.is_finite() {
            return Err(invalid_data(
                "borrowed MAP structural-constant certificate ratio is not finite",
            ));
        }
        relative_infinity_norm = relative_infinity_norm.max(relative);
        relative_square_sum = relative.mul_add(relative, relative_square_sum);
        let normalized_moment = mean.abs() * group_weight;
        let raw_moment = normalized_moment * weights.raw_scale;
        absolute_infinity_norm = absolute_infinity_norm.max(if raw_moment.is_finite() {
            raw_moment
        } else if normalized_moment == 0.0 {
            0.0
        } else {
            f64::INFINITY
        });
    }
    let relative_two_norm = (relative_square_sum / residuals.ncols() as f64).sqrt();
    if !relative_two_norm.is_finite() {
        return Err(invalid_data(
            "borrowed MAP structural-constant certificate norm is not finite",
        ));
    }
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance,
    })
}

fn project_effect(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    if effect.codes().len() != values.nrows() {
        return Err(invalid_argument(
            "borrowed MAP effect and matrix observation counts differ",
        ));
    }
    let parallel_mean_count = effect
        .n_levels()
        .checked_mul(values.ncols())
        .ok_or_else(|| allocation("borrowed MAP parallel group-mean dimensions overflow"))?;
    if values.ncols() > 1
        && rayon::current_num_threads() > 1
        && workspace.parallel_group_means.len() >= parallel_mean_count
    {
        return project_effect_parallel(effect, weights, values, workspace, parallel_mean_count);
    }
    project_effect_serial(effect, weights, values, workspace)
}

fn project_effect_serial(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    for level in 0..effect.n_levels() {
        let members = effect.level_index().checked_members(level)?;
        if members.is_empty() {
            return Err(invalid_argument(
                "borrowed MAP effect contains an empty final level",
            ));
        }
        group_means(members, weights, values, workspace)?;
        for &observation in members {
            if observation >= values.nrows() {
                return Err(invalid_argument(
                    "borrowed MAP membership exceeds the observation axis",
                ));
            }
            for rhs in 0..values.ncols() {
                let residual = values[(observation, rhs)] - workspace.group_sums[rhs];
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP projection produced a nonfinite residual",
                    ));
                }
                values[(observation, rhs)] = residual;
            }
        }
    }
    Ok(())
}

fn project_effect_parallel(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    parallel_mean_count: usize,
) -> Result<()> {
    let levels = effect.n_levels();
    let columns = values.ncols();
    let values_read = &*values;
    workspace.parallel_group_means[..parallel_mean_count]
        .par_chunks_mut(levels)
        .enumerate()
        .try_for_each(|(rhs, means)| -> Result<()> {
            for (level, mean) in means.iter_mut().enumerate() {
                let members = effect.level_index().checked_members(level)?;
                if members.is_empty() {
                    return Err(invalid_argument(
                        "borrowed MAP effect contains an empty final level",
                    ));
                }
                *mean = group_mean_for_column(members, rhs, weights, values_read)?;
            }
            Ok(())
        })?;

    let means = &workspace.parallel_group_means[..parallel_mean_count];
    let codes = effect.codes();
    values
        .as_mut_slice()
        .par_chunks_mut(columns)
        .enumerate()
        .try_for_each(|(observation, row)| -> Result<()> {
            let level = codes[observation] as usize;
            if level >= levels {
                return Err(invalid_argument(
                    "borrowed MAP code exceeds the finalized level count",
                ));
            }
            for (rhs, value) in row.iter_mut().enumerate() {
                let residual = *value - means[rhs * levels + level];
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP projection produced a nonfinite residual",
                    ));
                }
                *value = residual;
            }
            Ok(())
        })?;
    Ok(())
}

fn group_mean_for_column(
    members: &[usize],
    rhs: usize,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
) -> Result<f64> {
    let mut scale = 0.0_f64;
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= values.nrows() {
            return Err(invalid_argument(
                "borrowed MAP membership exceeds the observation axis",
            ));
        }
        compensated_add(
            weights.normalized(observation),
            &mut weight_sum,
            &mut weight_correction,
        )?;
        scale = scale.max(values[(observation, rhs)].abs());
    }
    let total_weight = weight_sum + weight_correction;
    if !total_weight.is_finite() || total_weight <= 0.0 {
        return Err(invalid_data(
            "borrowed MAP group weight sum is not finite and positive",
        ));
    }

    let mut normalized_sum = 0.0_f64;
    let mut correction = 0.0_f64;
    if scale != 0.0 {
        for &observation in members {
            compensated_add(
                weights.normalized(observation) * (values[(observation, rhs)] / scale),
                &mut normalized_sum,
                &mut correction,
            )?;
        }
    }
    let mean = if scale == 0.0 {
        0.0
    } else {
        scale * ((normalized_sum + correction) / total_weight)
    };
    if !mean.is_finite() {
        return Err(invalid_data("borrowed MAP group mean is not finite"));
    }
    Ok(mean)
}

fn group_means(
    members: &[usize],
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<f64> {
    workspace.value_scales.fill(0.0);
    workspace.group_sums.fill(0.0);
    workspace.group_corrections.fill(0.0);

    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= values.nrows() {
            return Err(invalid_argument(
                "borrowed MAP membership exceeds the observation axis",
            ));
        }
        compensated_add(
            weights.normalized(observation),
            &mut weight_sum,
            &mut weight_correction,
        )?;
        for rhs in 0..values.ncols() {
            workspace.value_scales[rhs] =
                workspace.value_scales[rhs].max(values[(observation, rhs)].abs());
        }
    }
    let total_weight = weight_sum + weight_correction;
    if !total_weight.is_finite() || total_weight <= 0.0 {
        return Err(invalid_data(
            "borrowed MAP group weight sum is not finite and positive",
        ));
    }

    for &observation in members {
        let weight = weights.normalized(observation);
        for rhs in 0..values.ncols() {
            let scale = workspace.value_scales[rhs];
            if scale == 0.0 {
                continue;
            }
            let term = weight * (values[(observation, rhs)] / scale);
            compensated_add(
                term,
                &mut workspace.group_sums[rhs],
                &mut workspace.group_corrections[rhs],
            )?;
        }
    }

    for rhs in 0..values.ncols() {
        let scale = workspace.value_scales[rhs];
        let normalized_sum = workspace.group_sums[rhs] + workspace.group_corrections[rhs];
        let mean = if scale == 0.0 {
            0.0
        } else {
            scale * (normalized_sum / total_weight)
        };
        if !mean.is_finite() {
            return Err(invalid_data("borrowed MAP group mean is not finite"));
        }
        workspace.group_sums[rhs] = mean;
    }
    Ok(total_weight)
}

fn certify(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    weighted_column_norms(
        residuals,
        weights,
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    for rhs in 0..residuals.ncols() {
        let scale = workspace.reference_norms[rhs].max(workspace.residual_norms[rhs]);
        if !scale.is_finite() || scale < 0.0 {
            return Err(invalid_data("borrowed MAP RHS scale is invalid"));
        }
        workspace.rhs_scales[rhs] = scale;
    }

    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0_usize;
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let members = effect.level_index().checked_members(level)?;
            if members.is_empty() {
                return Err(invalid_argument(
                    "borrowed MAP certificate contains an empty final level",
                ));
            }
            let group_weight = group_means(members, weights, residuals, workspace)?;
            let root_group_weight = group_weight.sqrt();
            if !root_group_weight.is_finite() || root_group_weight <= 0.0 {
                return Err(invalid_data(
                    "borrowed MAP certificate group weight is invalid",
                ));
            }
            for rhs in 0..residuals.ncols() {
                let mean = workspace.group_sums[rhs];
                let denominator = workspace.rhs_scales[rhs];
                let relative = if denominator == 0.0 {
                    if mean == 0.0 {
                        0.0
                    } else {
                        return Err(invalid_data(
                            "borrowed MAP zero-scale certificate has a nonzero moment",
                        ));
                    }
                } else {
                    (mean.abs() / denominator) * root_group_weight
                };
                if !relative.is_finite() {
                    return Err(invalid_data("borrowed MAP certificate ratio is not finite"));
                }
                relative_infinity_norm = relative_infinity_norm.max(relative);
                relative_square_sum = relative.mul_add(relative, relative_square_sum);
                if !relative_square_sum.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP certificate two-norm accumulation is not finite",
                    ));
                }
                relative_count = relative_count
                    .checked_add(1)
                    .ok_or_else(|| allocation("borrowed MAP certificate moment count overflows"))?;

                let normalized_moment = mean.abs() * group_weight;
                let raw_moment = normalized_moment * weights.raw_scale;
                absolute_infinity_norm = absolute_infinity_norm.max(if raw_moment.is_finite() {
                    raw_moment
                } else if normalized_moment == 0.0 {
                    0.0
                } else {
                    f64::INFINITY
                });
            }
        }
    }

    let relative_two_norm = if relative_count == 0 {
        0.0
    } else {
        (relative_square_sum / relative_count as f64).sqrt()
    };
    if !relative_two_norm.is_finite() {
        return Err(invalid_data(
            "borrowed MAP certificate two norm is not finite",
        ));
    }
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance,
    })
}

fn weighted_column_norms(
    values: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    output: &mut [f64],
    scales: &mut [f64],
    scaled_square_sums: &mut [f64],
) -> Result<()> {
    if output.len() != values.ncols()
        || scales.len() != values.ncols()
        || scaled_square_sums.len() != values.ncols()
    {
        return Err(invalid_argument(
            "borrowed MAP norm workspace dimensions differ",
        ));
    }
    scales.fill(0.0);
    scaled_square_sums.fill(1.0);
    for row in 0..values.nrows() {
        let root_weight = weights.normalized(row).sqrt();
        if !root_weight.is_finite() || root_weight <= 0.0 {
            return Err(invalid_data(
                "borrowed MAP normalized weight is not finite and positive",
            ));
        }
        for rhs in 0..values.ncols() {
            let value = root_weight * values[(row, rhs)];
            if !value.is_finite() {
                return Err(invalid_data("borrowed MAP weighted value is not finite"));
            }
            update_scaled_sum_of_squares(value, &mut scales[rhs], &mut scaled_square_sums[rhs]);
        }
    }
    for rhs in 0..values.ncols() {
        let norm = if scales[rhs] == 0.0 {
            0.0
        } else {
            scales[rhs] * scaled_square_sums[rhs].sqrt()
        };
        if !norm.is_finite() {
            return Err(invalid_data(
                "borrowed MAP weighted column norm is not finite",
            ));
        }
        output[rhs] = norm;
    }
    Ok(())
}

fn update_scaled_sum_of_squares(value: f64, scale: &mut f64, sum: &mut f64) {
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

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    let updated = *sum + value;
    let adjustment = if sum.abs() >= value.abs() {
        (*sum - updated) + value
    } else {
        (value - updated) + *sum
    };
    *correction += adjustment;
    *sum = updated;
    if !sum.is_finite() || !correction.is_finite() {
        return Err(invalid_data(
            "borrowed MAP compensated accumulation is not finite",
        ));
    }
    Ok(())
}

fn certified_result(
    residualized: DenseMatrix,
    stop_reason: SolveStopReason,
    iterations: usize,
    certificate: FeCertificate,
    setup_seconds: f64,
    solve_seconds: f64,
) -> Result<CertifiedBorrowedMap> {
    let report = SolveReport {
        requested_solver: FeSolverKind::Map,
        used_solver: FeSolverKind::Map,
        iterations,
        stop_reason,
        normal_equation_residual: certificate.relative_two_norm,
        certificate,
        setup_seconds,
        solve_seconds,
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: Vec::new(),
    };
    if !report.certified() {
        return Err(invalid_data(
            "borrowed MAP attempted to return an uncertified result",
        ));
    }
    Ok(CertifiedBorrowedMap {
        residualized,
        report,
    })
}

const fn zero_certificate() -> FeCertificate {
    FeCertificate {
        absolute_infinity_norm: 0.0,
        relative_infinity_norm: 0.0,
        relative_two_norm: 0.0,
        passed: true,
    }
}

fn reserved_zeros<R: WorkspaceReservation>(
    elements: usize,
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<f64>> {
    let mut output = Vec::new();
    reservation.reserve_f64(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "borrowed MAP workspace reservation returned insufficient capacity",
        ));
    }
    output.resize(elements, 0.0);
    Ok(output)
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

    #[derive(Default)]
    struct InjectedReservation {
        fail_at: Option<usize>,
        skip_at: Option<usize>,
        calls: usize,
        records: Vec<(usize, &'static str)>,
    }

    impl WorkspaceReservation for InjectedReservation {
        fn reserve_f64(
            &mut self,
            target: &mut Vec<f64>,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls = self.calls.checked_add(1).expect("small reservation count");
            self.records.push((additional, label));
            if self.fail_at == Some(call) {
                return Err(allocation("injected borrowed MAP allocation failure"));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test borrowed MAP reservation failed"))
        }
    }

    fn config() -> MapConfig {
        MapConfig {
            tolerance: 1e-11,
            max_iterations: 10_000,
            check_every: 1,
            symmetric: true,
        }
    }

    #[test]
    fn every_workspace_reservation_and_capacity_guard_fails_closed() {
        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
        let columns = [vec![1.0, 2.0, 4.0, 8.0], vec![2.0, 3.0, 1.0, -1.0]];

        let mut successful = InjectedReservation::default();
        absorb_with_reservation(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
            false,
            &mut successful,
        )
        .expect("successful run");
        assert_eq!(successful.calls, 6);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
                false,
                &mut injected,
            )
            .expect_err("injected allocation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        for skip_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
                false,
                &mut injected,
            )
            .expect_err("undersized allocation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }

    #[test]
    fn structural_constant_empty_design_allocations_are_fallible_and_only_rhs_scale() {
        let observations = 4096;
        let rhs_count = 3;
        let design =
            BorrowedInterceptFeDesign::new(observations, Vec::new()).expect("empty design");
        let columns = (0..rhs_count)
            .map(|column| {
                (0..observations)
                    .map(|row| (row as f64).sin() + column as f64)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let labels = [
            "unable to reserve borrowed MAP reference norms",
            "unable to reserve borrowed MAP residual norms",
            "unable to reserve borrowed MAP RHS scales",
            "unable to reserve borrowed MAP value scales",
            "unable to reserve borrowed MAP group sums",
            "unable to reserve borrowed MAP group corrections",
        ];

        let mut successful = InjectedReservation::default();
        let result = absorb_with_reservation(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&columns).expect("structural RHS"),
            config(),
            true,
            &mut successful,
        )
        .expect("structural constant projection");
        assert!(result.report().certified());
        assert_eq!(result.report().iterations, 1);
        assert_eq!(successful.calls, labels.len());
        assert_eq!(
            successful.records,
            labels
                .into_iter()
                .map(|label| (rhs_count, label))
                .collect::<Vec<_>>()
        );
        assert!(successful
            .records
            .iter()
            .all(|(additional, _)| *additional < observations));

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("structural RHS"),
                config(),
                true,
                &mut injected,
            )
            .expect_err("structural constant allocation must fail closed");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }
        for skip_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("structural RHS"),
                config(),
                true,
                &mut injected,
            )
            .expect_err("undersized structural workspace must fail closed");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }

    #[test]
    fn large_parallel_projection_is_bit_stable_and_its_workspace_fails_closed() {
        let observations = PARALLEL_PROJECTION_MIN_ROWS;
        let levels = 4_096;
        let codes = (0..observations)
            .map(|row| (row % levels) as u32)
            .collect::<Vec<_>>();
        let index = LevelIndex::from_codes(&codes, levels).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, levels, &index).expect("source");
        let design =
            BorrowedInterceptFeDesign::new(observations, vec![source.effect()]).expect("design");
        let columns = (0..3)
            .map(|column| {
                (0..observations)
                    .map(|row| {
                        let centered = (row % 97) as f64 - 48.0;
                        centered.mul_add(column as f64 + 0.5, (row % levels) as f64 / 17.0)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let weights = (0..observations)
            .map(|row| 1.0 + (row % 11) as f64 / 13.0)
            .collect::<Vec<_>>();

        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("pool")
                .install(|| {
                    absorb_borrowed_intercept_map_certified(
                        &design,
                        BorrowedMapWeights::Provided(&weights),
                        DenseMatrix::from_columns(&columns).expect("matrix"),
                        config(),
                    )
                    .expect("certified projection")
                })
        };
        let serial = run(1);
        let parallel = run(4);
        assert_eq!(serial.residualized(), parallel.residualized());
        assert_eq!(serial.report().iterations, parallel.report().iterations);
        assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
        assert_eq!(serial.report().certificate, parallel.report().certificate);

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .expect("pool");
        let mut successful = InjectedReservation::default();
        pool.install(|| {
            absorb_with_reservation(
                &design,
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
                false,
                &mut successful,
            )
            .expect("parallel reservation succeeds");
        });
        assert_eq!(successful.calls, 7);
        assert_eq!(
            successful.records.last(),
            Some(&(
                levels * columns.len(),
                "unable to reserve borrowed MAP parallel group means",
            )),
        );

        for skip in [false, true] {
            let mut injected = InjectedReservation {
                fail_at: (!skip).then_some(6),
                skip_at: skip.then_some(6),
                ..InjectedReservation::default()
            };
            let error = pool
                .install(|| {
                    absorb_with_reservation(
                        &design,
                        BorrowedMapWeights::Provided(&weights),
                        DenseMatrix::from_columns(&columns).expect("matrix"),
                        config(),
                        false,
                        &mut injected,
                    )
                })
                .expect_err("parallel workspace allocation must fail closed");
            assert_eq!(error.code, ErrorCode::Allocation);
        }
    }

    #[test]
    fn success_is_always_certified_and_nonconvergence_is_fatal() {
        let first_codes = [0_u32, 0, 0, 1, 1, 1];
        let second_codes = [0_u32, 0, 1, 0, 1, 1];
        let first_index = LevelIndex::from_codes(&first_codes, 2).expect("first index");
        let second_index = LevelIndex::from_codes(&second_codes, 2).expect("second index");
        let first =
            ValidatedBorrowedInterceptSource::new(&first_codes, 2, &first_index).expect("first");
        let second =
            ValidatedBorrowedInterceptSource::new(&second_codes, 2, &second_index).expect("second");
        let design = BorrowedInterceptFeDesign::new(6, vec![first.effect(), second.effect()])
            .expect("design");
        let columns = [vec![1.0, -3.0, 2.0, 5.0, -4.0, 0.5]];

        let success = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0, 2.0, 0.7, 3.0, 1.5, 0.5]),
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
        )
        .expect("certified");
        assert!(success.report().certified());

        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0, 2.0, 0.7, 3.0, 1.5, 0.5]),
            DenseMatrix::from_columns(&columns).expect("matrix"),
            MapConfig {
                tolerance: 1e-16,
                max_iterations: 1,
                check_every: 1,
                symmetric: false,
            },
        )
        .expect_err("uncertified solve must be fatal");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn empty_nonfinite_and_invalid_weight_inputs_fail_before_workspace_allocation() {
        let design = BorrowedInterceptFeDesign::new(0, Vec::new()).expect("empty design");
        let mut reservation = InjectedReservation::default();
        let error = absorb_with_reservation(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::zeros(0, 1).expect("matrix"),
            config(),
            false,
            &mut reservation,
        )
        .expect_err("empty rows");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert_eq!(reservation.calls, 0);

        let codes = [0_u32, 0];
        let index = LevelIndex::from_codes(&codes, 1).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).expect("design");
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let error = absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Provided(&[1.0, bad]),
                DenseMatrix::from_columns(&[vec![1.0, 2.0]]).expect("matrix"),
                config(),
            )
            .expect_err("invalid weight");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[f64::MAX, f64::MIN_POSITIVE]),
            DenseMatrix::from_columns(&[vec![1.0, 2.0]]).expect("matrix"),
            config(),
        )
        .expect_err("normalized underflow");
        assert_eq!(error.code, ErrorCode::InvalidData);

        for bad in [f64::NAN, f64::INFINITY] {
            let error = absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&[vec![1.0, bad]]).expect("matrix"),
                config(),
            )
            .expect_err("invalid value");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn no_effects_and_already_certified_inputs_return_without_iteration() {
        let design = BorrowedInterceptFeDesign::new(3, Vec::new()).expect("design");
        let result = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0]]).expect("matrix"),
            config(),
        )
        .expect("no effects");
        assert_eq!(result.report().stop_reason, SolveStopReason::NoFixedEffects);
        assert_eq!(result.report().iterations, 0);

        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
        let result = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![-1.0, 1.0, -2.0, 2.0]]).expect("matrix"),
            config(),
        )
        .expect("already certified");
        assert_eq!(result.report().iterations, 0);
        assert!(result.report().certified());

        let zero = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![0.0; 4]]).expect("zero matrix"),
            config(),
        )
        .expect("zero RHS");
        assert_eq!(zero.report().iterations, 0);
        assert_eq!(zero.report().certificate.relative_infinity_norm, 0.0);
    }

    #[test]
    fn one_way_projection_is_exact_and_invalid_shapes_and_configurations_fail() {
        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
        let result = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0, 3.0, 2.0, 2.0]),
            DenseMatrix::from_columns(&[vec![1.0, 5.0, 4.0, 8.0]]).expect("matrix"),
            config(),
        )
        .expect("one-way projection");
        assert_eq!(result.report().iterations, 1);
        assert_close(
            result.residualized().as_slice(),
            &[-3.0, 1.0, -2.0, 2.0],
            1e-14,
        );

        let short = BorrowedInterceptFeDesign::new(3, Vec::new()).expect("short design");
        assert_eq!(
            absorb_borrowed_intercept_map_certified(
                &short,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
                config(),
            )
            .expect_err("dimension mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Provided(&[1.0; 3]),
                DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
                config(),
            )
            .expect_err("weight mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::zeros(4, 0).expect("empty RHS matrix"),
                config(),
            )
            .expect_err("empty RHS")
            .code,
            ErrorCode::InvalidData
        );

        for bad in [
            MapConfig {
                tolerance: 0.0,
                ..config()
            },
            MapConfig {
                tolerance: f64::NAN,
                ..config()
            },
            MapConfig {
                max_iterations: 0,
                ..config()
            },
            MapConfig {
                check_every: 0,
                ..config()
            },
        ] {
            assert_eq!(
                absorb_borrowed_intercept_map_certified(
                    &design,
                    BorrowedMapWeights::Unit,
                    DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
                    bad,
                )
                .expect_err("invalid config")
                .code,
                ErrorCode::InvalidArgument
            );
        }
    }

    #[test]
    fn nonrepresentable_weighted_norm_fails_closed() {
        let codes = [0_u32, 0];
        let index = LevelIndex::from_codes(&codes, 1).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).expect("design");
        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![f64::MAX, f64::MAX]]).expect("matrix"),
            config(),
        )
        .expect_err("overflowing weighted norm");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    fn assert_close(left: &[f64], right: &[f64], tolerance: f64) {
        assert_eq!(left.len(), right.len());
        for (left, right) in left.iter().zip(right) {
            assert!((left - right).abs() <= tolerance, "{left} != {right}");
        }
    }
}
