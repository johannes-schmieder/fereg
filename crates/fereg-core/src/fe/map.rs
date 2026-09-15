use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::ProjectionWorkspace;
use crate::fe::{
    certify_residuals_with_reference_norms, project_effect_in_place, weighted_column_norms,
    CertifiedBorrowedMap, FeCertificate, FeDesign, FeSolverKind, SolveReport, SolveStopReason,
};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapConfig {
    pub tolerance: f64,
    pub max_iterations: usize,
    pub check_every: usize,
    pub symmetric: bool,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 16_000,
            check_every: 4,
            symmetric: true,
        }
    }
}

/// Residualize one or more right-hand sides with weighted symmetric MAP.
pub fn absorb_map(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: MapConfig,
) -> Result<SolveReport> {
    validate_config(config)?;
    let started = Instant::now();
    if design.nobs() != values.nrows() || weights.len() != values.nrows() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "MAP inputs have inconsistent observation counts",
        ));
    }
    let reference_norms = weighted_column_norms(values, weights)?;
    if design.effects().is_empty() {
        let certificate = certify_residuals_with_reference_norms(
            design,
            weights,
            values,
            &reference_norms,
            config.tolerance,
        )?;
        return Ok(SolveReport {
            checkpoint_retries: 0,
            solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
            requested_solver: FeSolverKind::Map,
            used_solver: FeSolverKind::Map,
            iterations: 0,
            stop_reason: SolveStopReason::NoFixedEffects,
            certificate,
            normal_equation_residual: 0.0,
            setup_seconds: 0.0,
            solve_seconds: started.elapsed().as_secs_f64(),
            certification_seconds: 0.0,
            fallback_used: false,
            requested_preconditioner: None,
            used_preconditioner: None,
            pilot_iterations: 0,
            pilot_contraction: 0.0,
            warnings: Vec::new(),
        });
    }
    let mut workspace = ProjectionWorkspace::new(design.max_channels(), values.ncols())?;
    let mut final_certificate = certify_residuals_with_reference_norms(
        design,
        weights,
        values,
        &reference_norms,
        config.tolerance,
    )?;

    for iteration in 1..=config.max_iterations {
        for effect in design.effects() {
            project_effect_in_place(effect, weights, values, &mut workspace)?;
        }
        if config.symmetric && design.effects().len() > 1 {
            for effect in design.effects().iter().rev().skip(1) {
                project_effect_in_place(effect, weights, values, &mut workspace)?;
            }
        }
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            final_certificate = certify_residuals_with_reference_norms(
                design,
                weights,
                values,
                &reference_norms,
                config.tolerance,
            )?;
            if final_certificate.passed {
                return Ok(SolveReport {
                    checkpoint_retries: 0,
                    solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
                    requested_solver: FeSolverKind::Map,
                    used_solver: FeSolverKind::Map,
                    iterations: iteration,
                    stop_reason: SolveStopReason::Certified,
                    normal_equation_residual: final_certificate.relative_two_norm,
                    certificate: final_certificate,
                    setup_seconds: 0.0,
                    solve_seconds: started.elapsed().as_secs_f64(),
                    certification_seconds: 0.0,
                    fallback_used: false,
                    requested_preconditioner: None,
                    used_preconditioner: None,
                    pilot_iterations: 0,
                    pilot_contraction: 0.0,
                    warnings: Vec::new(),
                });
            }
        }
    }

    Ok(SolveReport {
        checkpoint_retries: 0,
        solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
        requested_solver: FeSolverKind::Map,
        used_solver: FeSolverKind::Map,
        iterations: config.max_iterations,
        stop_reason: SolveStopReason::MaximumIterations,
        normal_equation_residual: final_certificate.relative_two_norm,
        certificate: final_certificate,
        setup_seconds: 0.0,
        solve_seconds: started.elapsed().as_secs_f64(),
        certification_seconds: 0.0,
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: Vec::new(),
    })
}

/// Session-owned MAP state for changing-weight iterative estimators.
///
/// This deliberately does not replace the public one-shot MAP path. It owns
/// every RHS-, effect-channel-, and largest-level-sized numerical buffer so a
/// fixed-topology caller can residualize changing matrices after one warm-up
/// without another size-dependent allocation.
pub(crate) struct ReusableMapAbsorber {
    design: FeDesign,
    projection: ProjectionWorkspace,
    max_rhs: usize,
    max_level_members: usize,
    max_channels: usize,
    reference_norms: Vec<f64>,
    residual_norms: Vec<f64>,
    rhs_scales: Vec<f64>,
    norm_scales: Vec<f64>,
    norm_sums: Vec<f64>,
    group_sums: Vec<f64>,
    group_corrections: Vec<f64>,
    certificate_columns: Vec<f64>,
    certificate_basis: Vec<f64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ReusableMapCapacities {
    pub(crate) rhs: usize,
    pub(crate) max_channels: usize,
    pub(crate) max_level_members: usize,
    pub(crate) reference_norms: usize,
    pub(crate) residual_norms: usize,
    pub(crate) rhs_scales: usize,
    pub(crate) norm_scales: usize,
    pub(crate) norm_sums: usize,
    pub(crate) group_sums: usize,
    pub(crate) group_corrections: usize,
    pub(crate) projection_gram: usize,
    pub(crate) projection_rhs: usize,
    pub(crate) certificate_columns: usize,
    pub(crate) certificate_basis: usize,
    pub(crate) accelerator_state: usize,
    pub(crate) topology_effects: usize,
    pub(crate) topology_codes: usize,
    pub(crate) topology_slope_vectors: usize,
    pub(crate) topology_slope_values: usize,
    pub(crate) topology_level_offsets: usize,
    pub(crate) topology_level_observations: usize,
    pub(crate) topology_resident_bytes: u128,
}

impl ReusableMapAbsorber {
    pub(crate) fn new(design: FeDesign, max_rhs: usize) -> Result<Self> {
        if max_rhs == 0 {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "reusable MAP requires a positive RHS capacity",
            ));
        }
        let max_channels = design.max_channels().max(1);
        let max_level_members = design
            .effects()
            .iter()
            .flat_map(|effect| {
                (0..effect.n_levels()).map(|level| effect.level_index().members(level).len())
            })
            .max()
            .unwrap_or(1);
        let certificate_elements =
            max_channels.checked_mul(max_level_members).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "reusable MAP certificate dimensions overflow",
                )
            })?;
        Ok(Self {
            projection: ProjectionWorkspace::new(max_channels, max_rhs)?,
            design,
            max_rhs,
            max_level_members,
            max_channels,
            reference_norms: reserved_zeroes(max_rhs, "reusable MAP reference norms")?,
            residual_norms: reserved_zeroes(max_rhs, "reusable MAP residual norms")?,
            rhs_scales: reserved_zeroes(max_rhs, "reusable MAP RHS scales")?,
            norm_scales: reserved_zeroes(max_rhs, "reusable MAP norm scales")?,
            norm_sums: reserved_zeroes(max_rhs, "reusable MAP norm sums")?,
            group_sums: reserved_zeroes(max_rhs, "reusable MAP group sums")?,
            group_corrections: reserved_zeroes(max_rhs, "reusable MAP group corrections")?,
            certificate_columns: reserved_zeroes(
                certificate_elements,
                "reusable MAP certificate columns",
            )?,
            certificate_basis: reserved_zeroes(
                certificate_elements,
                "reusable MAP certificate basis",
            )?,
        })
    }

    pub(crate) const fn max_rhs(&self) -> usize {
        self.max_rhs
    }

    pub(crate) const fn design(&self) -> &FeDesign {
        &self.design
    }

    pub(crate) fn capacities(&self) -> ReusableMapCapacities {
        let (projection_gram, projection_rhs) = self.projection.storage_capacities();
        let topology = self.design.storage_capacities();
        ReusableMapCapacities {
            rhs: self.reference_norms.capacity(),
            max_channels: self.max_channels,
            max_level_members: self.max_level_members,
            reference_norms: self.reference_norms.capacity(),
            residual_norms: self.residual_norms.capacity(),
            rhs_scales: self.rhs_scales.capacity(),
            norm_scales: self.norm_scales.capacity(),
            norm_sums: self.norm_sums.capacity(),
            group_sums: self.group_sums.capacity(),
            group_corrections: self.group_corrections.capacity(),
            projection_gram,
            projection_rhs,
            certificate_columns: self.certificate_columns.capacity(),
            certificate_basis: self.certificate_basis.capacity(),
            accelerator_state: 0,
            topology_effects: topology.effects,
            topology_codes: topology.codes,
            topology_slope_vectors: topology.slope_vectors,
            topology_slope_values: topology.slope_values,
            topology_level_offsets: topology.level_offsets,
            topology_level_observations: topology.level_observations,
            topology_resident_bytes: topology.resident_bytes,
        }
    }

    pub(crate) fn absorb(
        &mut self,
        weights: &[f64],
        mut values: DenseMatrix,
        config: MapConfig,
        structural_constant_if_empty: bool,
    ) -> Result<CertifiedBorrowedMap> {
        validate_config(config)?;
        let rhs = values.ncols();
        if values.nrows() != self.design.nobs()
            || weights.len() != values.nrows()
            || rhs == 0
            || rhs > self.max_rhs
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "reusable MAP inputs exceed their fixed session dimensions",
            ));
        }
        if weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
            || values.as_slice().iter().any(|value| !value.is_finite())
        {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "reusable MAP inputs must be finite with positive weights",
            ));
        }

        let setup_started = Instant::now();
        reusable_weighted_norms(
            &values,
            weights,
            &mut self.reference_norms[..rhs],
            &mut self.norm_scales[..rhs],
            &mut self.norm_sums[..rhs],
        )?;
        let setup_seconds = setup_started.elapsed().as_secs_f64();
        let solve_started = Instant::now();

        if self.design.effects().is_empty() {
            if !structural_constant_if_empty {
                return CertifiedBorrowedMap::from_certified_parts(
                    values,
                    reusable_map_report(
                        SolveStopReason::NoFixedEffects,
                        0,
                        zero_reusable_certificate(),
                        setup_seconds,
                        solve_started.elapsed().as_secs_f64(),
                    ),
                );
            }
            let initial =
                self.certify_structural_constant(&values, weights, rhs, config.tolerance)?;
            if initial.passed {
                return CertifiedBorrowedMap::from_certified_parts(
                    values,
                    reusable_map_report(
                        SolveStopReason::Certified,
                        0,
                        initial,
                        setup_seconds,
                        solve_started.elapsed().as_secs_f64(),
                    ),
                );
            }
            self.project_structural_constant(&mut values, weights, rhs)?;
            let certificate =
                self.certify_structural_constant(&values, weights, rhs, config.tolerance)?;
            if !certificate.passed {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "reusable MAP structural-constant projection did not certify",
                ));
            }
            return CertifiedBorrowedMap::from_certified_parts(
                values,
                reusable_map_report(
                    SolveStopReason::Certified,
                    1,
                    certificate,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                ),
            );
        }

        let initial = self.certify(&values, weights, rhs, config.tolerance)?;
        if initial.passed {
            return CertifiedBorrowedMap::from_certified_parts(
                values,
                reusable_map_report(
                    SolveStopReason::Certified,
                    0,
                    initial,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                ),
            );
        }
        for iteration in 1..=config.max_iterations {
            for effect in self.design.effects() {
                project_effect_in_place(effect, weights, &mut values, &mut self.projection)?;
            }
            if config.symmetric && self.design.effects().len() > 1 {
                for effect in self.design.effects().iter().rev().skip(1) {
                    project_effect_in_place(effect, weights, &mut values, &mut self.projection)?;
                }
            }
            if self.design.effects().len() <= 1
                || iteration % config.check_every == 0
                || iteration == config.max_iterations
            {
                let certificate = self.certify(&values, weights, rhs, config.tolerance)?;
                if certificate.passed {
                    return CertifiedBorrowedMap::from_certified_parts(
                        values,
                        reusable_map_report(
                            SolveStopReason::Certified,
                            iteration,
                            certificate,
                            setup_seconds,
                            solve_started.elapsed().as_secs_f64(),
                        ),
                    );
                }
            }
        }
        Err(FeregError::new(
            ErrorCode::InvalidData,
            "reusable MAP exhausted its iteration budget without certification",
        ))
    }

    fn project_structural_constant(
        &mut self,
        values: &mut DenseMatrix,
        weights: &[f64],
        rhs: usize,
    ) -> Result<()> {
        self.group_sums[..rhs].fill(0.0);
        self.group_corrections[..rhs].fill(0.0);
        let mut weight_sum = 0.0_f64;
        let mut weight_correction = 0.0_f64;
        for row in 0..values.nrows() {
            reusable_compensated_add(weights[row], &mut weight_sum, &mut weight_correction)?;
            for column in 0..rhs {
                reusable_compensated_add(
                    weights[row] * values[(row, column)],
                    &mut self.group_sums[column],
                    &mut self.group_corrections[column],
                )?;
            }
        }
        let total_weight = weight_sum + weight_correction;
        if !total_weight.is_finite() || total_weight <= 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "reusable MAP structural-constant weight sum is invalid",
            ));
        }
        for column in 0..rhs {
            self.group_sums[column] =
                (self.group_sums[column] + self.group_corrections[column]) / total_weight;
        }
        for row in 0..values.nrows() {
            for column in 0..rhs {
                values[(row, column)] -= self.group_sums[column];
            }
        }
        Ok(())
    }

    fn certify_structural_constant(
        &mut self,
        values: &DenseMatrix,
        weights: &[f64],
        rhs: usize,
        tolerance: f64,
    ) -> Result<FeCertificate> {
        reusable_weighted_norms(
            values,
            weights,
            &mut self.residual_norms[..rhs],
            &mut self.norm_scales[..rhs],
            &mut self.norm_sums[..rhs],
        )?;
        self.group_sums[..rhs].fill(0.0);
        self.group_corrections[..rhs].fill(0.0);
        let weight_sum = weights.iter().copied().sum::<f64>();
        let root_weight_sum = weight_sum.sqrt();
        let mut absolute = 0.0_f64;
        let mut relative_max = 0.0_f64;
        let mut relative_square = 0.0_f64;
        for row in 0..values.nrows() {
            for column in 0..rhs {
                reusable_compensated_add(
                    weights[row] * values[(row, column)],
                    &mut self.group_sums[column],
                    &mut self.group_corrections[column],
                )?;
            }
        }
        for column in 0..rhs {
            let moment = self.group_sums[column] + self.group_corrections[column];
            absolute = absolute.max(moment.abs());
            let scale = self.reference_norms[column].max(self.residual_norms[column]);
            let relative = if scale == 0.0 {
                0.0
            } else {
                (moment / (scale * root_weight_sum)).abs()
            };
            relative_max = relative_max.max(relative);
            relative_square = relative.mul_add(relative, relative_square);
        }
        Ok(FeCertificate {
            absolute_infinity_norm: absolute,
            relative_infinity_norm: relative_max,
            relative_two_norm: (relative_square / rhs as f64).sqrt(),
            passed: relative_max <= tolerance,
        })
    }

    fn certify(
        &mut self,
        residuals: &DenseMatrix,
        weights: &[f64],
        rhs: usize,
        tolerance: f64,
    ) -> Result<FeCertificate> {
        reusable_weighted_norms(
            residuals,
            weights,
            &mut self.residual_norms[..rhs],
            &mut self.norm_scales[..rhs],
            &mut self.norm_sums[..rhs],
        )?;
        for column in 0..rhs {
            self.rhs_scales[column] = self.reference_norms[column].max(self.residual_norms[column]);
        }
        let mut absolute = 0.0_f64;
        let mut relative_max = 0.0_f64;
        let mut relative_square = 0.0_f64;
        let mut relative_count = 0_usize;
        for effect in self.design.effects() {
            let channels = effect.channels();
            for level in 0..effect.n_levels() {
                let members = effect.level_index().members(level);
                let member_count = members.len();
                debug_assert!(member_count <= self.max_level_members);
                let mut active = 0_usize;
                for channel in 0..channels {
                    let target = active * self.max_level_members;
                    let mut scale = 0.0_f64;
                    for (local, &row) in members.iter().enumerate() {
                        let value = weights[row].sqrt() * effect.basis_value(channel, row);
                        self.certificate_columns[target + local] = value;
                        scale = scale.max(value.abs());
                    }
                    if scale == 0.0 {
                        continue;
                    }
                    for local in 0..member_count {
                        self.certificate_columns[target + local] /= scale;
                    }
                    let norm =
                        reusable_norm(&self.certificate_columns[target..target + member_count]);
                    if norm == 0.0 || !norm.is_finite() {
                        return Err(FeregError::new(
                            ErrorCode::InvalidData,
                            "reusable MAP basis normalization failed",
                        ));
                    }
                    for local in 0..member_count {
                        self.certificate_columns[target + local] /= norm;
                    }
                    active += 1;
                }
                let mut basis_count = 0_usize;
                let rank_threshold = f64::EPSILON * member_count.max(active).max(1) as f64 * 8.0;
                while active > 0 && basis_count < member_count {
                    let mut pivot = 0_usize;
                    let mut pivot_norm = 0.0_f64;
                    for column in 0..active {
                        let start = column * self.max_level_members;
                        let norm =
                            reusable_norm(&self.certificate_columns[start..start + member_count]);
                        if norm > pivot_norm {
                            pivot = column;
                            pivot_norm = norm;
                        }
                    }
                    if pivot_norm <= rank_threshold {
                        break;
                    }
                    let last = active - 1;
                    if pivot != last {
                        for local in 0..member_count {
                            self.certificate_columns.swap(
                                pivot * self.max_level_members + local,
                                last * self.max_level_members + local,
                            );
                        }
                    }
                    let candidate_start = last * self.max_level_members;
                    let basis_start = basis_count * self.max_level_members;
                    for local in 0..member_count {
                        self.certificate_basis[basis_start + local] =
                            self.certificate_columns[candidate_start + local];
                    }
                    active -= 1;
                    for _ in 0..2 {
                        for prior in 0..basis_count {
                            reusable_subtract_projection(
                                &mut self.certificate_basis,
                                basis_start,
                                prior * self.max_level_members,
                                member_count,
                            );
                        }
                    }
                    let norm = reusable_norm(
                        &self.certificate_basis[basis_start..basis_start + member_count],
                    );
                    if norm <= rank_threshold {
                        continue;
                    }
                    for local in 0..member_count {
                        self.certificate_basis[basis_start + local] /= norm;
                    }
                    for column in 0..active {
                        let column_start = column * self.max_level_members;
                        for _ in 0..2 {
                            reusable_subtract_cross_projection(
                                &mut self.certificate_columns,
                                column_start,
                                &self.certificate_basis,
                                basis_start,
                                member_count,
                            );
                        }
                    }
                    basis_count += 1;
                }
                for column in 0..rhs {
                    let denominator = self.rhs_scales[column];
                    let mut projected_square = 0.0_f64;
                    if denominator > 0.0 {
                        for basis in 0..basis_count {
                            let start = basis * self.max_level_members;
                            let mut coefficient = 0.0_f64;
                            for (local, &row) in members.iter().enumerate() {
                                let scaled =
                                    weights[row].sqrt() * residuals[(row, column)] / denominator;
                                coefficient = self.certificate_basis[start + local]
                                    .mul_add(scaled, coefficient);
                            }
                            projected_square = coefficient.mul_add(coefficient, projected_square);
                        }
                    }
                    let relative = projected_square.sqrt();
                    relative_max = relative_max.max(relative);
                    relative_square = relative.mul_add(relative, relative_square);
                    relative_count += 1;
                }
                for channel in 0..channels {
                    for column in 0..rhs {
                        let moment = reusable_scaled_moment(members, |row| {
                            weights[row]
                                * effect.basis_value(channel, row)
                                * residuals[(row, column)]
                        });
                        absolute = absolute.max(moment.abs());
                    }
                }
            }
        }
        let relative_two = if relative_count == 0 {
            0.0
        } else {
            (relative_square / relative_count as f64).sqrt()
        };
        Ok(FeCertificate {
            absolute_infinity_norm: absolute,
            relative_infinity_norm: relative_max,
            relative_two_norm: relative_two,
            passed: relative_max <= tolerance,
        })
    }
}

fn reusable_map_report(
    stop_reason: SolveStopReason,
    iterations: usize,
    certificate: FeCertificate,
    setup_seconds: f64,
    solve_seconds: f64,
) -> SolveReport {
    SolveReport {
        checkpoint_retries: 0,
        solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
        requested_solver: FeSolverKind::Map,
        used_solver: FeSolverKind::Map,
        iterations,
        stop_reason,
        normal_equation_residual: certificate.relative_two_norm,
        certificate,
        setup_seconds,
        solve_seconds,
        certification_seconds: 0.0,
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: Vec::new(),
    }
}

fn zero_reusable_certificate() -> FeCertificate {
    FeCertificate {
        absolute_infinity_norm: 0.0,
        relative_infinity_norm: 0.0,
        relative_two_norm: 0.0,
        passed: true,
    }
}

fn reserved_zeroes(elements: usize, label: &'static str) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(elements)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, label))?;
    values.resize(elements, 0.0);
    Ok(values)
}

fn reusable_weighted_norms(
    values: &DenseMatrix,
    weights: &[f64],
    output: &mut [f64],
    scales: &mut [f64],
    sums: &mut [f64],
) -> Result<()> {
    if output.len() != values.ncols()
        || scales.len() != values.ncols()
        || sums.len() != values.ncols()
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "reusable MAP norm workspace dimensions differ",
        ));
    }
    scales.fill(0.0);
    sums.fill(1.0);
    for row in 0..values.nrows() {
        let root = weights[row].sqrt();
        for column in 0..values.ncols() {
            let value = root * values[(row, column)];
            let absolute = value.abs();
            if absolute == 0.0 {
                continue;
            }
            if scales[column] < absolute {
                let ratio = scales[column] / absolute;
                sums[column] = 1.0 + sums[column] * ratio * ratio;
                scales[column] = absolute;
            } else {
                let ratio = absolute / scales[column];
                sums[column] += ratio * ratio;
            }
        }
    }
    for column in 0..values.ncols() {
        output[column] = if scales[column] == 0.0 {
            0.0
        } else {
            scales[column] * sums[column].sqrt()
        };
        if !output[column].is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "reusable MAP weighted norm is not finite",
            ));
        }
    }
    Ok(())
}

fn reusable_compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    let updated = *sum + value;
    *correction += if sum.abs() >= value.abs() {
        (*sum - updated) + value
    } else {
        (value - updated) + *sum
    };
    *sum = updated;
    if sum.is_finite() && correction.is_finite() {
        Ok(())
    } else {
        Err(FeregError::new(
            ErrorCode::InvalidData,
            "reusable MAP compensated sum is not finite",
        ))
    }
}

fn reusable_norm(values: &[f64]) -> f64 {
    let mut scale = 0.0_f64;
    let mut sum = 1.0_f64;
    for &value in values {
        let absolute = value.abs();
        if absolute == 0.0 {
            continue;
        }
        if scale < absolute {
            let ratio = scale / absolute;
            sum = 1.0 + sum * ratio * ratio;
            scale = absolute;
        } else {
            let ratio = absolute / scale;
            sum += ratio * ratio;
        }
    }
    if scale == 0.0 {
        0.0
    } else {
        scale * sum.sqrt()
    }
}

fn reusable_subtract_projection(
    values: &mut [f64],
    target: usize,
    direction: usize,
    length: usize,
) {
    let mut coefficient = 0.0_f64;
    for local in 0..length {
        coefficient = values[target + local].mul_add(values[direction + local], coefficient);
    }
    for local in 0..length {
        values[target + local] =
            (-coefficient).mul_add(values[direction + local], values[target + local]);
    }
}

fn reusable_subtract_cross_projection(
    values: &mut [f64],
    target: usize,
    directions: &[f64],
    direction: usize,
    length: usize,
) {
    let mut coefficient = 0.0_f64;
    for local in 0..length {
        coefficient = values[target + local].mul_add(directions[direction + local], coefficient);
    }
    for local in 0..length {
        values[target + local] =
            (-coefficient).mul_add(directions[direction + local], values[target + local]);
    }
}

fn reusable_scaled_moment(members: &[usize], value: impl Fn(usize) -> f64) -> f64 {
    let mut scale = 0.0_f64;
    for &row in members {
        scale = scale.max(value(row).abs());
    }
    if scale == 0.0 {
        return 0.0;
    }
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for &row in members {
        let scaled = value(row) / scale;
        let updated = sum + scaled;
        correction += if sum.abs() >= scaled.abs() {
            (sum - updated) + scaled
        } else {
            (scaled - updated) + sum
        };
        sum = updated;
    }
    (sum + correction) * scale
}

fn validate_config(config: MapConfig) -> Result<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "MAP tolerance must be finite and positive",
        ));
    }
    if config.max_iterations == 0 || config.check_every == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "MAP iteration and check budgets must be positive",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn absorbs_two_way_intercepts_and_certifies() {
        let worker = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("worker");
        let firm = EffectTerm::new(vec![0, 1, 0, 1, 0, 1], 2, true, vec![]).expect("firm");
        let design = FeDesign::new(6, vec![worker, firm]).expect("design");
        let weights = vec![1.0; 6];
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 2.0, 3.0, 4.0, 5.0]]).expect("values");
        let report =
            absorb_map(&design, &weights, &mut values, MapConfig::default()).expect("absorb");
        assert_eq!(report.stop_reason, SolveStopReason::Certified);
        assert!(report.certificate.passed);
        assert!(values.as_slice().iter().all(|value| value.abs() < 1e-8));
    }
}
