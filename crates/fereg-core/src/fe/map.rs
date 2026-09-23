use super::certificate::CertificateContext;
use super::projection::{AutoProjectionWorkspace, ParallelProjectionPlan};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
#[cfg(test)]
use crate::fe::certify_residuals_with_reference_norms;
#[cfg(test)]
use crate::fe::{project_effect_in_place, ProjectionWorkspace};
use crate::fe::{
    weighted_column_norms, CertifiedBorrowedMap, FeCertificate, FeDesign, FeSolverKind,
    SolveReport, SolveStopReason,
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
    absorb_map_with_context(
        &CertificateContext::new(design, weights),
        values,
        config,
        None,
    )
}

pub(super) fn absorb_map_with_context(
    context: &CertificateContext<'_>,
    values: &mut DenseMatrix,
    config: MapConfig,
    projection_plan: Option<ParallelProjectionPlan>,
) -> Result<SolveReport> {
    let (design, weights) = context.inputs();
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
        let certificate = context.certify(values, &reference_norms, config.tolerance)?;
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
    let mut workspace = AutoProjectionWorkspace::new(design, values.ncols(), projection_plan)?;
    // MAP always performs a sweep before its first scheduled certificate.
    // Validate the weighted basis before mutation, but do not construct an
    // unused initial QR/span certificate. Every returned residual is freshly
    // certified at a scheduled check or at the final allowed iteration.
    if values.ncols() > 0 {
        for effect in design.effects() {
            for level in 0..effect.n_levels() {
                for channel in 0..effect.channels() {
                    for &row in effect.level_index().members(level) {
                        if !(weights[row].sqrt() * effect.basis_value(channel, row)).is_finite() {
                            return Err(FeregError::new(
                                ErrorCode::InvalidData,
                                "weighted fixed-effect basis is not finite",
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut final_certificate = None;

    for iteration in 1..=config.max_iterations {
        for effect in design.effects() {
            workspace.project(effect, weights, values)?;
        }
        if config.symmetric && design.effects().len() > 1 {
            for effect in design.effects().iter().rev().skip(1) {
                workspace.project(effect, weights, values)?;
            }
        }
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            let certificate = context.certify(values, &reference_norms, config.tolerance)?;
            if certificate.passed {
                return Ok(SolveReport {
                    checkpoint_retries: 0,
                    solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
                    requested_solver: FeSolverKind::Map,
                    used_solver: FeSolverKind::Map,
                    iterations: iteration,
                    stop_reason: SolveStopReason::Certified,
                    normal_equation_residual: certificate.relative_two_norm,
                    certificate,
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
            final_certificate = Some(certificate);
        }
    }
    let final_certificate =
        final_certificate.expect("positive MAP budget always checks the final allowed iteration");

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
    projection: AutoProjectionWorkspace,
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
    certificate_geometry: Option<ReusableCertificateGeometry>,
    coefficient_solver: Option<Box<super::slope_coefficients::SlopeCoefficientWorkspace>>,
}

/// Supplied only by the linear session that owns all weight mutations. Response
/// and finite-basis changes do not alter local weighted FE geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MapGeometryGeneration {
    pub(crate) sample: u64,
    pub(crate) topology: u64,
    pub(crate) weights: u64,
}

pub(super) struct ReusableCertificateGeometry {
    pub(super) basis: Vec<f64>,
    pub(super) ranks: Vec<usize>,
    generation: Option<MapGeometryGeneration>,
    #[cfg(test)]
    preparations: usize,
}

fn reusable_certificate_dimensions(design: &FeDesign) -> Result<(usize, usize)> {
    design
        .effects()
        .iter()
        .try_fold((0_usize, 0_usize), |(basis, ranks), effect| {
            let basis = design
                .nobs()
                .checked_mul(effect.channels())
                .and_then(|n| basis.checked_add(n));
            let ranks = ranks.checked_add(effect.n_levels());
            basis.zip(ranks).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "reusable certificate geometry dimensions overflow",
                )
            })
        })
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
    pub(crate) projection_workers: usize,
    pub(crate) certificate_columns: usize,
    pub(crate) certificate_basis: usize,
    pub(crate) certificate_geometry: usize,
    pub(crate) certificate_ranks: usize,
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
            projection: AutoProjectionWorkspace::new(&design, max_rhs, None)?,
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
            certificate_geometry: None,
            coefficient_solver: None,
            certificate_basis: reserved_zeroes(
                certificate_elements,
                "reusable MAP certificate basis",
            )?,
        })
    }

    /// Only storage is retained: every call rebuilds group coefficients using
    /// the current weights and RHS. The caller admits this bank for the session.
    pub(crate) fn enable_parallel_projection(
        &mut self,
        plan: ParallelProjectionPlan,
    ) -> Result<()> {
        self.projection.enable_parallel(plan)
    }

    pub(crate) fn certificate_geometry_bytes(design: &FeDesign) -> Result<u128> {
        let (basis, ranks) = reusable_certificate_dimensions(design)?;
        Ok(basis as u128 * 8
            + ranks as u128 * std::mem::size_of::<usize>() as u128
            + std::mem::size_of::<Option<MapGeometryGeneration>>() as u128)
    }

    /// The caller admits this optional bank for the entire session lifetime.
    /// An allocation refusal retains the existing uncached certificate path.
    pub(crate) fn enable_certificate_geometry(&mut self) -> Result<()> {
        let (basis_len, rank_len) = reusable_certificate_dimensions(&self.design)?;
        let basis = reserved_zeroes(basis_len, "reusable certificate geometry")?;
        let mut ranks = Vec::new();
        ranks.try_reserve_exact(rank_len).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve reusable certificate ranks",
            )
        })?;
        ranks.resize(rank_len, 0);
        if basis.capacity() > basis_len || ranks.capacity() > rank_len {
            return Err(FeregError::new(
                ErrorCode::Allocation,
                "reusable certificate capacity exceeds admission",
            ));
        }
        self.certificate_geometry = Some(ReusableCertificateGeometry {
            basis,
            ranks,
            generation: None,
            #[cfg(test)]
            preparations: 0,
        });
        Ok(())
    }

    pub(crate) fn coefficient_solver_bytes(
        design: &FeDesign,
        rhs: usize,
        threads: usize,
    ) -> Result<u128> {
        super::slope_coefficients::SlopeCoefficientWorkspace::bytes(design, rhs, threads)
    }

    pub(crate) fn enable_coefficient_solver(&mut self, threads: usize) -> Result<()> {
        if self.certificate_geometry.is_none() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "coefficient solver requires admitted geometry",
            ));
        }
        self.coefficient_solver = Some(Box::new(
            super::slope_coefficients::SlopeCoefficientWorkspace::new(
                &self.design,
                self.max_rhs,
                threads,
            )?,
        ));
        Ok(())
    }

    pub(crate) fn has_certificate_geometry(&self) -> bool {
        self.certificate_geometry.is_some()
    }

    pub(crate) fn coefficient_coupling_bytes(design: &FeDesign, rhs: usize) -> Result<Option<u128>> {
        super::slope_coefficients::SlopeCoefficientWorkspace::coupling_bytes(design, rhs)
    }

    pub(crate) fn enable_coefficient_coupling(&mut self) -> Result<()> {
        if let Some(solver) = self.coefficient_solver.as_mut() {
            solver.enable_coupling(&self.design)?;
            if let Some(geometry) = self.certificate_geometry.as_mut() {
                geometry.generation = None;
            }
        }
        Ok(())
    }

    pub(crate) fn coefficient_certificate_bytes(design: &FeDesign, rhs: usize) -> Result<u128> {
        super::slope_coefficients::SlopeCoefficientWorkspace::certificate_bytes(design, rhs)
    }

    pub(crate) fn enable_coefficient_certificates(&mut self) -> Result<()> {
        if let Some(solver) = self.coefficient_solver.as_mut() {
            solver.enable_certificate_groups()?;
        }
        Ok(())
    }

    pub(crate) const fn max_rhs(&self) -> usize {
        self.max_rhs
    }

    pub(crate) const fn design(&self) -> &FeDesign {
        &self.design
    }

    pub(crate) fn capacities(&self) -> ReusableMapCapacities {
        let (projection_gram, projection_rhs, projection_workers) =
            self.projection.storage_capacities();
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
            projection_workers,
            certificate_columns: self.certificate_columns.capacity(),
            certificate_basis: self.certificate_basis.capacity(),
            certificate_geometry: self
                .certificate_geometry
                .as_ref()
                .map_or(0, |g| g.basis.capacity()),
            certificate_ranks: self
                .certificate_geometry
                .as_ref()
                .map_or(0, |g| g.ranks.capacity()),
            accelerator_state: self.coefficient_solver.as_ref().map_or(0, |s| s.elements()),
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
        values: DenseMatrix,
        config: MapConfig,
        structural_constant_if_empty: bool,
    ) -> Result<CertifiedBorrowedMap> {
        self.absorb_in_generation(weights, values, config, structural_constant_if_empty, None)
    }

    pub(crate) fn absorb_in_generation(
        &mut self,
        weights: &[f64],
        mut values: DenseMatrix,
        config: MapConfig,
        structural_constant_if_empty: bool,
        generation: Option<MapGeometryGeneration>,
    ) -> Result<CertifiedBorrowedMap> {
        // Take authority before any fallible work. Failure or unwind cannot
        // leave a generation authorized for a later call.
        let previous = self
            .certificate_geometry
            .as_mut()
            .and_then(|g| g.generation.take());
        let reusable_geometry = generation.is_some() && previous == generation;
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

        let mut geometry_valid = false;
        if self.coefficient_solver.is_some() {
            if !reusable_geometry {
                self.prepare_coefficient_geometry(weights)?;
            }
            geometry_valid = true;
            let solver = self
                .coefficient_solver
                .as_mut()
                .expect("admitted coefficient workspace");
            let iterations = solver.solve(
                &self.design,
                self.certificate_geometry
                    .as_ref()
                    .expect("admitted geometry"),
                weights,
                &mut values,
                &self.reference_norms[..rhs],
                config,
                reusable_geometry,
            )?;
            let certificate =
                self.certify(&values, weights, rhs, config.tolerance, &mut geometry_valid)?;
            if !certificate.passed {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "slope coefficient residual failed independent certification",
                ));
            }
            let result = CertifiedBorrowedMap::from_certified_parts(
                values,
                reusable_map_report(
                    SolveStopReason::Certified,
                    iterations,
                    certificate,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                ),
            )?;
            self.certificate_geometry
                .as_mut()
                .expect("admitted geometry")
                .generation = generation;
            return Ok(result);
        }
        let initial = self.certify(&values, weights, rhs, config.tolerance, &mut geometry_valid)?;
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
                self.projection.project(effect, weights, &mut values)?;
            }
            if config.symmetric && self.design.effects().len() > 1 {
                for effect in self.design.effects().iter().rev().skip(1) {
                    self.projection.project(effect, weights, &mut values)?;
                }
            }
            if self.design.effects().len() <= 1
                || iteration % config.check_every == 0
                || iteration == config.max_iterations
            {
                let certificate =
                    self.certify(&values, weights, rhs, config.tolerance, &mut geometry_valid)?;
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

    // Preparation establishes only weighted geometry. It cannot certify a RHS.
    fn prepare_coefficient_geometry(&mut self, weights: &[f64]) -> Result<()> {
        let geometry = self
            .certificate_geometry
            .as_mut()
            .expect("admitted geometry");
        #[cfg(test)]
        {
            geometry.preparations += 1;
        }
        let mut offset = 0;
        let mut group = 0;
        for effect in self.design.effects() {
            for level in 0..effect.n_levels() {
                let members = effect.level_index().members(level);
                let rank = prepare_reusable_group_basis(
                    effect,
                    members,
                    weights,
                    self.max_level_members,
                    &mut self.certificate_columns,
                    &mut self.certificate_basis,
                )?;
                geometry.ranks[group] = rank;
                for channel in 0..rank {
                    let start = channel * self.max_level_members;
                    let target = offset + channel * members.len();
                    geometry.basis[target..target + members.len()]
                        .copy_from_slice(&self.certificate_basis[start..start + members.len()]);
                }
                offset += effect.channels() * members.len();
                group += 1;
            }
        }
        Ok(())
    }

    fn certify(
        &mut self,
        residuals: &DenseMatrix,
        weights: &[f64],
        rhs: usize,
        tolerance: f64,
        geometry_valid: &mut bool,
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
        if *geometry_valid {
            if let Some(solver) = self
                .coefficient_solver
                .as_mut()
                .filter(|s| s.has_certificate_groups())
            {
                return Ok(solver.certify(
                    &self.design,
                    self.certificate_geometry
                        .as_ref()
                        .expect("valid geometry has storage"),
                    weights,
                    residuals,
                    &self.rhs_scales[..rhs],
                    tolerance,
                ));
            }
        }
        let mut absolute = 0.0_f64;
        let mut relative_max = 0.0_f64;
        let mut relative_square = 0.0_f64;
        let mut relative_count = 0_usize;
        let mut geometry_offset = 0_usize;
        let mut group_index = 0_usize;
        for effect in self.design.effects() {
            let channels = effect.channels();
            for level in 0..effect.n_levels() {
                let members = effect.level_index().members(level);
                let member_count = members.len();
                debug_assert!(member_count <= self.max_level_members);
                let basis_count = if *geometry_valid {
                    self.certificate_geometry
                        .as_ref()
                        .expect("valid geometry has storage")
                        .ranks[group_index]
                } else {
                    let basis_count = prepare_reusable_group_basis(
                        effect,
                        members,
                        weights,
                        self.max_level_members,
                        &mut self.certificate_columns,
                        &mut self.certificate_basis,
                    )?;
                    if let Some(geometry) = self.certificate_geometry.as_mut() {
                        geometry.ranks[group_index] = basis_count;
                        for basis in 0..basis_count {
                            let start = basis * self.max_level_members;
                            let target = geometry_offset + basis * member_count;
                            geometry.basis[target..target + member_count].copy_from_slice(
                                &self.certificate_basis[start..start + member_count],
                            );
                        }
                    }
                    basis_count
                };
                let (basis_values, basis_offset, basis_stride) = match self
                    .certificate_geometry
                    .as_ref()
                {
                    Some(geometry) => (geometry.basis.as_slice(), geometry_offset, member_count),
                    None => (self.certificate_basis.as_slice(), 0, self.max_level_members),
                };
                for column in 0..rhs {
                    let denominator = self.rhs_scales[column];
                    let mut projected_square = 0.0_f64;
                    if denominator > 0.0 {
                        for basis in 0..basis_count {
                            let start = basis_offset + basis * basis_stride;
                            let mut coefficient = 0.0_f64;
                            for (local, &row) in members.iter().enumerate() {
                                let scaled =
                                    weights[row].sqrt() * residuals[(row, column)] / denominator;
                                coefficient =
                                    basis_values[start + local].mul_add(scaled, coefficient);
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
                geometry_offset += channels * member_count;
                group_index += 1;
            }
        }
        *geometry_valid = self.certificate_geometry.is_some();
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

// Preserve every scalar entry's observation order while sharing row loads,
// weight roots and raw-channel products across a bounded RHS tile.
pub(super) fn certify_reusable_group(
    effect: &super::EffectTerm,
    members: &[usize],
    weights: &[f64],
    residuals: &DenseMatrix,
    scales: &[f64],
    basis: &[f64],
    rank: usize,
    result: &mut [f64],
) {
    const TILE: usize = 16;
    let rhs = residuals.ncols();
    result.fill(0.0);
    for first in (0..rhs).step_by(TILE) {
        let width = (rhs - first).min(TILE);
        let mut projected = [0.0_f64; TILE];
        for channel in 0..rank {
            let mut coefficients = [0.0_f64; TILE];
            let direction = &basis[channel * members.len()..(channel + 1) * members.len()];
            for (local, &row) in members.iter().enumerate() {
                let root = weights[row].sqrt();
                let values = &residuals.as_slice()[row * rhs + first..row * rhs + first + width];
                for column in 0..width {
                    let denominator = scales[first + column];
                    if denominator > 0.0 {
                        let scaled = root * values[column] / denominator;
                        coefficients[column] =
                            direction[local].mul_add(scaled, coefficients[column]);
                    }
                }
            }
            for column in 0..width {
                projected[column] =
                    coefficients[column].mul_add(coefficients[column], projected[column]);
            }
        }
        for column in 0..width {
            result[1 + first + column] = projected[column].sqrt();
        }
        for channel in 0..effect.channels() {
            let mut raw_scales = [0.0_f64; TILE];
            for &row in members {
                let left = weights[row] * effect.basis_value(channel, row);
                let values = &residuals.as_slice()[row * rhs + first..row * rhs + first + width];
                for column in 0..width {
                    raw_scales[column] = raw_scales[column].max((left * values[column]).abs());
                }
            }
            let mut sums = [0.0_f64; TILE];
            let mut corrections = [0.0_f64; TILE];
            for &row in members {
                let left = weights[row] * effect.basis_value(channel, row);
                let values = &residuals.as_slice()[row * rhs + first..row * rhs + first + width];
                for column in 0..width {
                    if raw_scales[column] == 0.0 {
                        continue;
                    }
                    let scaled = (left * values[column]) / raw_scales[column];
                    let sum = sums[column];
                    let updated = sum + scaled;
                    corrections[column] += if sum.abs() >= scaled.abs() {
                        (sum - updated) + scaled
                    } else {
                        (scaled - updated) + sum
                    };
                    sums[column] = updated;
                }
            }
            for column in 0..width {
                let moment = (sums[column] + corrections[column]) * raw_scales[column];
                result[0] = result[0].max(moment.abs());
            }
        }
    }
}

fn prepare_reusable_group_basis(
    effect: &super::EffectTerm,
    members: &[usize],
    weights: &[f64],
    stride: usize,
    columns: &mut [f64],
    basis: &mut [f64],
) -> Result<usize> {
    let member_count = members.len();
    let channels = effect.channels();
    let mut active = 0_usize;
    for channel in 0..channels {
        let target = active * stride;
        let mut scale = 0.0_f64;
        for (local, &row) in members.iter().enumerate() {
            let value = weights[row].sqrt() * effect.basis_value(channel, row);
            columns[target + local] = value;
            scale = scale.max(value.abs());
        }
        if scale == 0.0 {
            continue;
        }
        for local in 0..member_count {
            columns[target + local] /= scale;
        }
        let norm = reusable_norm(&columns[target..target + member_count]);
        if norm == 0.0 || !norm.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "reusable MAP basis normalization failed",
            ));
        }
        for local in 0..member_count {
            columns[target + local] /= norm;
        }
        active += 1;
    }
    let mut basis_count = 0_usize;
    let rank_threshold = f64::EPSILON * member_count.max(active).max(1) as f64 * 8.0;
    while active > 0 && basis_count < member_count {
        let mut pivot = 0_usize;
        let mut pivot_norm = 0.0_f64;
        for column in 0..active {
            let start = column * stride;
            let norm = reusable_norm(&columns[start..start + member_count]);
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
                columns.swap(pivot * stride + local, last * stride + local);
            }
        }
        let candidate_start = last * stride;
        let basis_start = basis_count * stride;
        for local in 0..member_count {
            basis[basis_start + local] = columns[candidate_start + local];
        }
        active -= 1;
        for _ in 0..2 {
            for prior in 0..basis_count {
                reusable_subtract_projection(basis, basis_start, prior * stride, member_count);
            }
        }
        let norm = reusable_norm(&basis[basis_start..basis_start + member_count]);
        if norm <= rank_threshold {
            continue;
        }
        for local in 0..member_count {
            basis[basis_start + local] /= norm;
        }
        for column in 0..active {
            let column_start = column * stride;
            for _ in 0..2 {
                reusable_subtract_cross_projection(
                    columns,
                    column_start,
                    basis,
                    basis_start,
                    member_count,
                );
            }
        }
        basis_count += 1;
    }
    Ok(basis_count)
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
    fn slope_coefficients_preserve_analytic_projection_across_weights_rank_and_threads() {
        // On this complete product, sum_j w_j*g(j)=0 and sum_t w_t*h(t)=0.
        // Therefore f(i)*g(j)*h(t) is orthogonal to i[t], j[i] and t effects,
        // even with duplicate/zero channels, disconnected copies and no i intercept.
        for third in [false, true] {
            for intercept in [false, true] {
                let rows = 2 * 8 * 6 * 2;
                let mut g1 = Vec::new();
                let mut g2 = Vec::new();
                let mut g3 = Vec::new();
                let mut z1 = Vec::new();
                let mut z2 = Vec::new();
                for component in 0..2 {
                    for i in 0..8 {
                        for j in 0..6 {
                            for t in 0..2 {
                                g1.push((component * 8 + i) as u32);
                                g2.push((component * 6 + j) as u32);
                                g3.push((component * 2 + t) as u32);
                                z1.push(if t == 0 { -1. } else { 1. });
                                z2.push(i as f64);
                            }
                        }
                    }
                }
                let mut effects = vec![
                    EffectTerm::new(
                        g1,
                        16,
                        intercept,
                        vec![
                            z1.clone(),
                            z1.iter().map(|z| 2. * z).collect(),
                            vec![0.; rows],
                        ],
                    )
                    .unwrap(),
                    EffectTerm::new(g2, 12, true, vec![z2]).unwrap(),
                ];
                if third {
                    effects.push(EffectTerm::new(g3, 4, true, vec![]).unwrap());
                }
                let design = FeDesign::new(rows, effects).unwrap();
                for prepared_coupling in [false, true] {
                    let mut reference: Option<DenseMatrix> = None;
                    for threads in [1, 4] {
                        let pool = rayon::ThreadPoolBuilder::new()
                            .num_threads(threads)
                            .build()
                            .unwrap();
                        let mut solver = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
                        solver.enable_certificate_geometry().unwrap();
                        solver.enable_coefficient_solver(threads).unwrap();
                        if prepared_coupling {
                            solver.enable_coefficient_coupling().unwrap();
                        }
                        let capacity = solver.capacities();
                        assert!(ReusableMapAbsorber::coefficient_solver_bytes(
                            &design,
                            usize::MAX,
                            threads
                        )
                        .is_err());
                        for (generation, rhs) in [7, 1, 3, 7].into_iter().enumerate() {
                            let mut values = DenseMatrix::zeros(rows, rhs).unwrap();
                            let mut expected = DenseMatrix::zeros(rows, rhs).unwrap();
                            let mut weights = Vec::new();
                            let jw: Vec<_> = (0..6)
                                .map(|j| 0.4 + j as f64 * 0.3 + generation as f64)
                                .collect();
                            let mean = jw
                                .iter()
                                .enumerate()
                                .map(|(j, w)| j as f64 * w)
                                .sum::<f64>()
                                / jw.iter().sum::<f64>();
                            let scale = [1., 1., 1e-60, 1e60][generation];
                            let mut row = 0;
                            for component in 0..2 {
                                for i in 0..8 {
                                    for j in 0..6 {
                                        for t in 0..2 {
                                            let sign = if t == 0 { -1. } else { 1. };
                                            let tw = if t == 0 { 0.5 } else { 2. };
                                            weights.push(
                                                scale
                                                    * (0.2 + i as f64 + generation as f64)
                                                    * jw[j]
                                                    * tw,
                                            );
                                            let remainder =
                                                (i as f64 + 1.).sin() * (j as f64 - mean) * sign / tw;
                                            let fitted = if intercept {
                                                (i * i + component) as f64
                                            } else {
                                                0.
                                            } + (i as f64).sin() * sign
                                                + (j as f64).cos()
                                                + j as f64 * i as f64 / 8.
                                                + if third { t as f64 * 0.31 } else { 0. };
                                            for col in 0..rhs {
                                                let c = (col + 1) as f64;
                                                expected[(row, col)] = remainder / c;
                                                values[(row, col)] = fitted * c + remainder / c;
                                            }
                                            row += 1;
                                        }
                                    }
                                }
                            }
                            let norms = weighted_column_norms(&values, &weights).unwrap();
                            let result = pool
                                .install(|| {
                                    solver.absorb(
                                        &weights,
                                        values,
                                        MapConfig {
                                            tolerance: 1e-12,
                                            ..MapConfig::default()
                                        },
                                        false,
                                    )
                                })
                                .unwrap();
                            for (&a, &e) in result
                                .residualized()
                                .as_slice()
                                .iter()
                                .zip(expected.as_slice())
                            {
                                assert!(
                                    (a - e).abs() < 2e-8,
                                    "generation={generation}, threads={threads}: {a} != {e}"
                                );
                            }
                            assert!(
                                certify_residuals_with_reference_norms(
                                    &design,
                                    &weights,
                                    result.residualized(),
                                    &norms,
                                    1e-12
                                )
                                .unwrap()
                                .passed
                            );
                            assert_eq!(capacity, solver.capacities());
                            if prepared_coupling {
                                solver
                                    .coefficient_solver
                                    .as_ref()
                                    .unwrap()
                                    .check_coupling_products(
                                        solver.design(),
                                        solver.certificate_geometry.as_ref().unwrap(),
                                    );
                            }
                            if generation == 3 {
                                if let Some(prior) = &reference {
                                    assert_eq!(prior, result.residualized());
                                } else {
                                    reference = Some(result.residualized().clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn slope_coefficients_fail_closed_and_recover_after_a_short_solve() {
        let n = 180;
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(
                    (0..n).map(|i| (i % 15) as u32).collect(),
                    15,
                    true,
                    vec![(0..n).map(|i| (i as f64 * 0.37).sin()).collect()],
                )
                .unwrap(),
                EffectTerm::new(
                    (0..n).map(|i| ((i / 9) % 10) as u32).collect(),
                    10,
                    true,
                    vec![(0..n).map(|i| (i as f64 * 0.19).cos()).collect()],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let values = DenseMatrix::from_row_major(
            n,
            2,
            (0..n * 2).map(|i| (i as f64 * 0.23).sin()).collect(),
        )
        .unwrap();
        let weights: Vec<_> = (0..n).map(|i| 0.4 + (i % 7) as f64).collect();
        let mut owner = ReusableMapAbsorber::new(design, 2).unwrap();
        assert!(owner.enable_coefficient_solver(2).is_err());
        owner.enable_certificate_geometry().unwrap();
        owner.enable_coefficient_solver(2).unwrap();
        owner.enable_coefficient_coupling().unwrap();
        let capacity = owner.capacities();
        assert!(owner
            .absorb(
                &weights,
                values.clone(),
                MapConfig {
                    max_iterations: 1,
                    tolerance: 1e-12,
                    ..MapConfig::default()
                },
                false
            )
            .is_err());
        let mut invalid = values.clone();
        invalid[(0, 0)] = f64::NAN;
        assert!(owner
            .absorb(&weights, invalid, MapConfig::default(), false)
            .is_err());
        let result = owner
            .absorb(
                &weights,
                values,
                MapConfig {
                    tolerance: 1e-12,
                    ..MapConfig::default()
                },
                false,
            )
            .unwrap();
        assert!(result.report().certified());
        assert_eq!(capacity, owner.capacities());
    }

    #[test]
    #[ignore = "bounded frozen-frame development diagnostic; requires FEREG_SLOPE_FRAME"]
    fn slope_coefficient_frozen_frame() {
        let root = std::path::PathBuf::from(std::env::var("FEREG_SLOPE_FRAME").unwrap());
        let meta = std::fs::read_to_string(root.join("frame.json")).unwrap();
        let scalar = |key: &str| -> usize {
            meta.split(&format!("\"{key}\":"))
                .nth(1)
                .unwrap()
                .split(',')
                .next()
                .unwrap()
                .parse()
                .unwrap()
        };
        let list = |key: &str| -> Vec<usize> {
            meta.split(&format!("\"{key}\":["))
                .nth(1)
                .unwrap()
                .split(']')
                .next()
                .unwrap()
                .split(',')
                .map(|v| v.trim().parse().unwrap())
                .collect()
        };
        let n = scalar("rows");
        let rhs = scalar("rhs");
        let levels = list("levels");
        let channels = list("channels");
        let data: Vec<Vec<f64>> = std::fs::read_to_string(root.join("frame.csv"))
            .unwrap()
            .lines()
            .map(|line| line.split(',').map(|x| x.parse().unwrap()).collect())
            .collect();
        let weights: Vec<_> = data.iter().map(|r| r[0]).collect();
        let values = DenseMatrix::from_row_major(
            n,
            rhs,
            data.iter()
                .flat_map(|r| r[1..1 + rhs].iter().copied())
                .collect(),
        )
        .unwrap();
        let mut effects = Vec::new();
        let mut at = 1 + 2 * rhs;
        for (&levels, &channels) in levels.iter().zip(&channels) {
            let codes = data.iter().map(|r| r[at] as u32).collect();
            let slopes = (1..channels)
                .map(|c| data.iter().map(|r| r[at + c]).collect())
                .collect();
            effects.push(EffectTerm::new(codes, levels, true, slopes).unwrap());
            at += channels;
        }
        let design = FeDesign::new(n, effects).unwrap();
        let baseline = std::env::var_os("FEREG_SLOPE_BASELINE").is_some();
        let tolerance = std::env::var("FEREG_SLOPE_TOL")
            .unwrap_or_else(|_| "1e-12".into())
            .parse()
            .unwrap();
        let threads: usize = std::env::var("FEREG_SLOPE_THREADS")
            .unwrap_or_else(|_| "4".into())
            .parse()
            .unwrap();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let started = Instant::now();
        let mut absorber = ReusableMapAbsorber::new(design, rhs).unwrap();
        absorber.enable_certificate_geometry().unwrap();
        if baseline {
            if let Some(plan) =
                ParallelProjectionPlan::for_design_and_threads(absorber.design(), rhs, threads)
                    .unwrap()
            {
                absorber.enable_parallel_projection(plan).unwrap();
            }
        } else {
            absorber.enable_coefficient_solver(threads).unwrap();
            absorber.enable_coefficient_certificates().unwrap();
            if std::env::var_os("FEREG_SLOPE_COUPLING").is_some() {
                absorber.enable_coefficient_coupling().unwrap();
            }
        }
        let construction = started.elapsed().as_secs_f64();
        let started = Instant::now();
        let result = pool
            .install(|| {
                absorber.absorb(
                    &weights,
                    values,
                    MapConfig {
                        tolerance,
                        ..MapConfig::default()
                    },
                    false,
                )
            })
            .unwrap();
        let elapsed = started.elapsed().as_secs_f64();
        println!("SLOPE_FRAME|baseline={baseline}|rows={n}|rhs={rhs}|threads={threads}|construction={construction}|seconds={elapsed}|iterations={}|certificate={}|workspace_bytes={}",result.report().iterations,result.report().certificate.relative_infinity_norm,ReusableMapAbsorber::coefficient_solver_bytes(absorber.design(),rhs,threads).unwrap());
        if let Ok(output) = std::env::var("FEREG_SLOPE_OUTPUT") {
            use std::io::Write;
            let mut out = std::io::BufWriter::new(std::fs::File::create(output).unwrap());
            for row in 0..n {
                for col in 0..rhs {
                    write!(
                        out,
                        "{}{:.17e}",
                        if col == 0 { "" } else { "," },
                        result.residualized()[(row, col)]
                    )
                    .unwrap();
                }
                writeln!(out).unwrap();
            }
        }
    }

    #[test]
    fn slope_geometry_reuses_only_explicit_generations_and_invalidates_before_errors() {
        let rows = 120;
        let z: Vec<_> = (0..rows).map(|i| (i % 7) as f64 - 3.0).collect();
        let design = FeDesign::new(
            rows,
            vec![
                EffectTerm::new(
                    (0..rows).map(|i| (i % 5) as u32).collect(),
                    5,
                    true,
                    vec![z.clone(), z.iter().map(|v| 2.0 * v).collect()],
                )
                .unwrap(),
                EffectTerm::new(
                    (0..rows).map(|i| ((i / 5) % 4) as u32).collect(),
                    4,
                    true,
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let mut cached = ReusableMapAbsorber::new(design.clone(), 3).unwrap();
        cached.enable_certificate_geometry().unwrap();
        cached.enable_coefficient_solver(1).unwrap();
        cached.enable_coefficient_certificates().unwrap();
        cached.enable_coefficient_coupling().unwrap();
        let mut fresh = ReusableMapAbsorber::new(design, 3).unwrap();
        fresh.enable_certificate_geometry().unwrap();
        fresh.enable_coefficient_solver(1).unwrap();
        fresh.enable_coefficient_coupling().unwrap();
        let capacities = cached.capacities();
        let mut generation = MapGeometryGeneration {
            sample: 1,
            topology: 2,
            weights: 3,
        };
        for step in 0..6 {
            if step == 2 {
                generation.weights += 1;
            }
            if step == 4 {
                generation.topology += 1;
            }
            let weights: Vec<_> = (0..rows)
                .map(|i| 0.2 + ((i * generation.weights as usize) % 17) as f64)
                .collect();
            let rhs = if step % 2 == 0 { 3 } else { 1 };
            let mut values = DenseMatrix::zeros(rows, rhs).unwrap();
            for row in 0..rows {
                for col in 0..rhs {
                    values[(row, col)] = ((row * 19 + col * 11 + step) as f64).sin();
                }
            }
            let expected = fresh
                .absorb(&weights, values.clone(), MapConfig::default(), false)
                .unwrap();
            let actual = cached
                .absorb_in_generation(
                    &weights,
                    values.clone(),
                    MapConfig::default(),
                    false,
                    Some(generation),
                )
                .unwrap();
            assert_eq!(actual.residualized(), expected.residualized());
            assert_eq!(actual.report().certificate, expected.report().certificate);
            assert_eq!(cached.capacities(), capacities);
            let geometry = cached.certificate_geometry.as_ref().unwrap();
            assert_eq!(geometry.generation, Some(generation));
            assert_eq!(geometry.preparations, step / 2 + 1);
            assert_eq!(cached.coefficient_solver.as_ref().unwrap().coupling_preparations(), Some(step / 2 + 1));
            if step == 5 {
                values[(0, 0)] = f64::NAN;
                assert!(cached
                    .absorb_in_generation(
                        &weights,
                        values,
                        MapConfig::default(),
                        false,
                        Some(generation)
                    )
                    .is_err());
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().generation,
                    None
                );
                let zeros = DenseMatrix::zeros(rows, 1).unwrap();
                cached
                    .absorb_in_generation(
                        &weights,
                        zeros.clone(),
                        MapConfig::default(),
                        false,
                        Some(generation),
                    )
                    .unwrap();
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().preparations,
                    4
                );
                assert_eq!(cached.coefficient_solver.as_ref().unwrap().coupling_preparations(), Some(4));
                cached
                    .absorb(&weights, zeros, MapConfig::default(), false)
                    .unwrap();
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().generation,
                    None
                );
                assert_eq!(
                    cached.certificate_geometry.as_ref().unwrap().preparations,
                    5
                );
                assert_eq!(cached.coefficient_solver.as_ref().unwrap().coupling_preparations(), Some(5));
            }
        }
    }

    #[test]
    fn reusable_certificate_tiles_match_serial_bits_across_threads_widths_and_scales() {
        let rows = 120;
        let z: Vec<_> = (0..rows).map(|i| (i % 7) as f64 - 3.0).collect();
        let design = FeDesign::new(
            rows,
            vec![
                EffectTerm::new(
                    (0..rows).map(|i| (i % 5) as u32).collect(),
                    6,
                    true,
                    vec![
                        z.clone(),
                        z.iter().map(|v| 2.0 * v).collect(),
                        vec![0.0; rows],
                    ],
                )
                .unwrap(),
                EffectTerm::new(
                    (0..rows).map(|i| ((i / 5) % 4) as u32).collect(),
                    4,
                    false,
                    vec![(0..rows).map(|i| (i % 11) as f64 - 5.0).collect()],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut serial = ReusableMapAbsorber::new(design.clone(), 33).unwrap();
            serial.enable_certificate_geometry().unwrap();
            let mut tiled = ReusableMapAbsorber::new(design.clone(), 33).unwrap();
            tiled.enable_certificate_geometry().unwrap();
            tiled.enable_coefficient_solver(threads).unwrap();
            let before = tiled.capacities().accelerator_state;
            tiled.enable_coefficient_certificates().unwrap();
            assert_eq!(
                (tiled.capacities().accelerator_state - before) as u128 * 8,
                ReusableMapAbsorber::coefficient_certificate_bytes(&design, 33).unwrap(),
            );
            let capacities = tiled.capacities();
            for rhs in [1, 7, 16, 17, 33] {
                for factor in [1e-60, 1.0, 1e60] {
                    let weights: Vec<_> = (0..rows)
                        .map(|i| factor * (0.1 + (i % 13) as f64))
                        .collect();
                    let mut residuals = DenseMatrix::zeros(rows, rhs).unwrap();
                    for row in 0..rows {
                        for column in 1..rhs {
                            residuals[(row, column)] = ((row * 19 + column * 11) as f64).sin()
                                * if column % 2 == 0 { 1e-60 } else { 1e60 };
                        }
                    }
                    serial.prepare_coefficient_geometry(&weights).unwrap();
                    tiled.prepare_coefficient_geometry(&weights).unwrap();
                    let expected = serial
                        .certify(&residuals, &weights, rhs, 1e-12, &mut true)
                        .unwrap();
                    let actual = pool
                        .install(|| tiled.certify(&residuals, &weights, rhs, 1e-12, &mut true))
                        .unwrap();
                    assert_eq!(
                        actual, expected,
                        "threads={threads}, rhs={rhs}, factor={factor}"
                    );
                    assert_eq!(tiled.capacities(), capacities);
                }
            }
        }
    }

    #[test]
    fn reusable_certificate_geometry_is_exact_and_rebuilt_after_every_call() {
        let rows = 120;
        let z: Vec<_> = (0..rows).map(|i| (i % 7) as f64 - 3.0).collect();
        let design = FeDesign::new(
            rows,
            vec![
                EffectTerm::new(
                    (0..rows).map(|i| (i % 5) as u32).collect(),
                    6,
                    true,
                    vec![
                        z.clone(),
                        z.iter().map(|v| 2.0 * v).collect(),
                        vec![0.0; rows],
                    ],
                )
                .unwrap(),
                EffectTerm::new(
                    (0..rows).map(|i| ((i / 5) % 4) as u32).collect(),
                    4,
                    false,
                    vec![(0..rows).map(|i| (i % 11) as f64 - 5.0).collect()],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let mut plain = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
        let mut cached = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
        cached.enable_certificate_geometry().unwrap();
        let capacities = cached.capacities();
        assert_eq!(capacities.certificate_geometry, rows * 5);
        assert_eq!(capacities.certificate_ranks, 10);
        assert_eq!(
            ReusableMapAbsorber::certificate_geometry_bytes(&design).unwrap(),
            (rows * 5 * 8 + 10 * std::mem::size_of::<usize>()
                + std::mem::size_of::<Option<MapGeometryGeneration>>()) as u128
        );
        for (generation, rhs) in [7, 1, 3, 7].into_iter().enumerate() {
            let weights: Vec<_> = (0..rows)
                .map(|i| 0.1 + ((i * (generation + 1)) % 13) as f64)
                .collect();
            let mut values = DenseMatrix::zeros(rows, rhs).unwrap();
            for row in 0..rows {
                for col in 0..rhs {
                    values[(row, col)] = ((row * 19 + col * 11 + generation) % 97) as f64;
                }
            }
            for limit in [1, 16000] {
                let config = MapConfig {
                    max_iterations: limit,
                    ..MapConfig::default()
                };
                let expected = plain.absorb(&weights, values.clone(), config, false);
                let actual = cached.absorb(&weights, values.clone(), config, false);
                match (actual, expected) {
                    (Ok(actual), Ok(expected)) => {
                        assert_eq!(actual.residualized(), expected.residualized());
                        assert_eq!(actual.report().certificate, expected.report().certificate);
                        assert_eq!(actual.report().iterations, expected.report().iterations);
                        let norms = weighted_column_norms(&values, &weights).unwrap();
                        assert!(
                            certify_residuals_with_reference_norms(
                                &design,
                                &weights,
                                actual.residualized(),
                                &norms,
                                1e-8
                            )
                            .unwrap()
                            .passed
                        );
                    }
                    (Err(actual), Err(expected)) => assert_eq!(actual.code, expected.code),
                    _ => panic!("cached and uncached outcomes differ"),
                }
                assert_eq!(cached.capacities(), capacities);
            }
        }
        // Initial zero-residual success must rebuild even when the old basis is
        // finite but has the wrong numerical rank under the new weights.
        let design = FeDesign::new(
            3,
            vec![EffectTerm::new(
                vec![0; 3],
                2,
                true,
                vec![vec![1e150, 2e150, 3e150], vec![0.0; 3]],
            )
            .unwrap()],
        )
        .unwrap();
        let mut cached = ReusableMapAbsorber::new(design, 1).unwrap();
        cached.enable_certificate_geometry().unwrap();
        for (weights, rank) in [
            (vec![1.0; 3], 2),
            (vec![1.0, 1e-60, 1e-60], 1),
            (vec![1.0; 3], 2),
        ] {
            cached
                .absorb(
                    &weights,
                    DenseMatrix::zeros(3, 1).unwrap(),
                    MapConfig::default(),
                    false,
                )
                .unwrap();
            assert_eq!(
                cached.certificate_geometry.as_ref().unwrap().ranks,
                vec![rank, 0]
            );
            assert!(cached
                .absorb(
                    &[f64::NAN; 3],
                    DenseMatrix::zeros(3, 1).unwrap(),
                    MapConfig::default(),
                    false
                )
                .is_err());
        }
    }

    #[test]
    fn reusable_parallel_projection_refreshes_weights_rhs_and_capacity() {
        let rows = 16_384;
        let design = FeDesign::new(
            rows,
            vec![
                EffectTerm::new(
                    (0..rows).map(|i| (i % 64) as u32).collect(),
                    64,
                    true,
                    vec![(0..rows).map(|i| ((i * 13) % 79) as f64 / 79.0).collect()],
                )
                .unwrap(),
                EffectTerm::new(
                    (0..rows).map(|i| ((i / 64) % 32) as u32).collect(),
                    32,
                    true,
                    vec![(0..rows).map(|i| ((i * 17) % 83) as f64 / 83.0).collect()],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let mut serial = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
        let mut parallel = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
        let plan = ParallelProjectionPlan::for_design_and_threads(&design, 7, 4)
            .unwrap()
            .unwrap();
        parallel.enable_parallel_projection(plan).unwrap();
        parallel.enable_certificate_geometry().unwrap();
        let before = parallel.capacities();
        assert_eq!(before.projection_workers, 4);
        assert_eq!(
            before.projection_rhs,
            serial.capacities().projection_rhs + 64 * 2 * 7
        );
        assert_eq!(
            before.projection_gram,
            serial.capacities().projection_gram + 4 * 2 * 2
        );
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        for (generation, rhs) in [7, 1, 3, 7].into_iter().enumerate() {
            let weights: Vec<_> = (0..rows)
                .map(|i| 0.1 + ((i * (generation + 1)) % 11) as f64)
                .collect();
            let mut values = DenseMatrix::zeros(rows, rhs).unwrap();
            for row in 0..rows {
                for col in 0..rhs {
                    values[(row, col)] = ((row * 19 + col * 11 + generation) % 97) as f64;
                }
            }
            let expected = serial
                .absorb(&weights, values.clone(), MapConfig::default(), false)
                .unwrap();
            let actual = pool
                .install(|| parallel.absorb(&weights, values, MapConfig::default(), false))
                .unwrap();
            assert_eq!(actual.residualized(), expected.residualized());
            assert_eq!(actual.report().certificate, expected.report().certificate);
            assert_eq!(actual.report().iterations, expected.report().iterations);
            assert_eq!(parallel.capacities(), before);
        }
        let mut bad_weights = vec![1.0; rows];
        bad_weights[rows - 1] = f64::NAN;
        assert_eq!(
            parallel
                .absorb(
                    &bad_weights,
                    DenseMatrix::zeros(rows, 1).unwrap(),
                    MapConfig::default(),
                    false
                )
                .unwrap_err()
                .code,
            ErrorCode::InvalidData
        );
        assert_eq!(parallel.capacities(), before);
    }

    #[test]
    fn map_preserves_scheduled_certificates_and_iteration_boundaries() {
        let rows = 60;
        let left = EffectTerm::new(
            (0..rows).map(|i| (i % 5) as u32).collect(),
            6,
            true,
            vec![(0..rows).map(|i| (i % 7) as f64 - 3.0).collect()],
        )
        .unwrap();
        let right = EffectTerm::new(
            (0..rows).map(|i| ((i / 5) % 4) as u32).collect(),
            4,
            true,
            vec![(0..rows).map(|i| (i % 11) as f64 - 5.0).collect()],
        )
        .unwrap();
        let design = FeDesign::new(rows, vec![left, right]).unwrap();
        for rhs in [0, 1, 7] {
            for weighted in [false, true] {
                let weights: Vec<_> = (0..rows)
                    .map(|i| if weighted { 0.5 + (i % 7) as f64 } else { 1.0 })
                    .collect();
                for max_iterations in [1, 3, 8, 100] {
                    for check_every in [1, 4, 99] {
                        for symmetric in [false, true] {
                            let mut actual = DenseMatrix::zeros(rows, rhs).unwrap();
                            for row in 0..rows {
                                for col in 0..rhs {
                                    actual[(row, col)] = ((row * 13 + col * 5) % 37) as f64 - 18.0;
                                }
                            }
                            let mut expected = actual.clone();
                            let config = MapConfig {
                                max_iterations,
                                check_every,
                                symmetric,
                                ..MapConfig::default()
                            };
                            let a = absorb_map(&design, &weights, &mut actual, config).unwrap();
                            let b = baseline_absorb_map(&design, &weights, &mut expected, config)
                                .unwrap();
                            assert_eq!(a.iterations, b.iterations);
                            assert_eq!(a.stop_reason, b.stop_reason);
                            assert_eq!(a.certificate, b.certificate);
                            for (x, y) in actual.as_slice().iter().zip(expected.as_slice()) {
                                assert_eq!(x.to_bits(), y.to_bits());
                            }
                        }
                    }
                }
            }
        }
        for check_every in [1, 4, 99] {
            let mut zeros = DenseMatrix::zeros(rows, 1).unwrap();
            let report = absorb_map(
                &design,
                &vec![1.0; rows],
                &mut zeros,
                MapConfig {
                    max_iterations: 8,
                    check_every,
                    ..MapConfig::default()
                },
            )
            .unwrap();
            assert_eq!(report.iterations, check_every.min(8));
            assert!(report.certified());
        }
    }

    #[test]
    fn map_rejects_weighted_basis_overflow_before_mutation() {
        let effect = EffectTerm::new(vec![0; 4], 1, true, vec![vec![f64::MAX / 2.0; 4]]).unwrap();
        let design = FeDesign::new(4, vec![effect]).unwrap();
        let original = DenseMatrix::from_columns(&[vec![0.0, 1.0, -1.0, 0.0]]).unwrap();
        let mut actual = original.clone();
        let mut expected = original.clone();
        let a = absorb_map(&design, &[16.0; 4], &mut actual, MapConfig::default()).unwrap_err();
        let b = baseline_absorb_map(&design, &[16.0; 4], &mut expected, MapConfig::default())
            .unwrap_err();
        assert_eq!(a, b);
        assert_eq!(a.code, ErrorCode::InvalidData);
        assert_eq!(actual, original);
        assert_eq!(expected, original);
    }

    fn baseline_absorb_map(
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
