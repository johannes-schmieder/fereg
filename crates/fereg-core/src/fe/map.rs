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
use rayon::prelude::*;
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
    certificate_groups: Vec<f64>,
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

impl ReusableMapCapacities {
    pub(crate) fn element_capacities(self) -> [usize; 24] {
        [
            self.rhs,
            self.max_channels,
            self.max_level_members,
            self.reference_norms,
            self.residual_norms,
            self.rhs_scales,
            self.norm_scales,
            self.norm_sums,
            self.group_sums,
            self.group_corrections,
            self.projection_gram,
            self.projection_rhs,
            self.projection_workers,
            self.certificate_columns,
            self.certificate_basis,
            self.certificate_geometry,
            self.certificate_ranks,
            self.accelerator_state,
            self.topology_effects,
            self.topology_codes,
            self.topology_slope_vectors,
            self.topology_slope_values,
            self.topology_level_offsets,
            self.topology_level_observations,
        ]
    }
}

impl ReusableMapAbsorber {
    // Read only immediately after successful certification of the returned RHS.
    pub(crate) fn certified_rhs_norms(&self, rhs: usize) -> &[f64] {
        &self.residual_norms[..rhs]
    }

    /// Serial base owner, excluding the separately admitted design and optional
    /// geometry/solver banks. One-shot owners use the same allocation layout.
    pub(crate) fn base_workspace_bytes(design: &FeDesign, rhs: usize) -> Result<u128> {
        let channels = design.max_channels().max(1) as u128;
        let members = design
            .effects()
            .iter()
            .flat_map(|effect| {
                (0..effect.n_levels()).map(|level| effect.level_index().members(level).len())
            })
            .max()
            .unwrap_or(1) as u128;
        let cells = super::MapCapacity::new(channels, members, rhs as u128)?.cells()?;
        cells
            .checked_mul(8)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() as u128))
            .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "reusable MAP bytes overflow"))
    }

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
        let plan = super::MapCapacity::new(
            max_channels as u128,
            max_level_members as u128,
            max_rhs as u128,
        )?;
        plan.certificate
            .len("reusable MAP certificate dimensions overflow")?;
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
            certificate_columns: plan
                .certificate
                .filled(0.0, "reusable MAP certificate columns")?,
            certificate_geometry: None,
            certificate_groups: Vec::new(),
            coefficient_solver: None,
            certificate_basis: plan
                .certificate
                .filled(0.0, "reusable MAP certificate basis")?,
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

    pub(crate) fn coefficient_coupling_bytes(
        design: &FeDesign,
        rhs: usize,
    ) -> Result<Option<u128>> {
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

    pub(crate) fn certificate_workspace_bytes(design: &FeDesign, rhs: usize) -> Result<u128> {
        let groups = design
            .effects()
            .iter()
            .try_fold(0_usize, |n, effect| n.checked_add(effect.n_levels()));
        groups
            .and_then(|n| rhs.checked_add(1)?.checked_mul(n))
            .map(|n| n as u128 * 8)
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "group certificate dimensions overflow",
                )
            })
    }

    pub(crate) fn enable_certificate_workspace(&mut self) -> Result<()> {
        let bytes = Self::certificate_workspace_bytes(&self.design, self.max_rhs)?;
        let length = usize::try_from(bytes / 8).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "group certificate dimensions overflow",
            )
        })?;
        self.certificate_groups = reserved_zeroes(length, "independent group certificates")?;
        Ok(())
    }

    pub(crate) fn joint_slope_bytes(design: &FeDesign, controls: usize) -> Result<u128> {
        super::slope_coefficients::joint::JointFiniteWorkspace::bytes(design, controls)
    }

    #[cfg(test)]
    pub(crate) fn enable_joint_slopes(&mut self) -> Result<()> {
        if let Some(solver) = self.coefficient_solver.as_mut() {
            solver.enable_joint(&self.design, self.max_rhs.saturating_sub(1))?;
        }
        Ok(())
    }

    pub(crate) fn enable_joint_slopes_for_columns(&mut self, columns: usize) -> Result<()> {
        if let Some(solver) = self.coefficient_solver.as_mut() {
            solver.enable_joint(&self.design, columns)?;
        }
        Ok(())
    }

    pub(crate) fn joint_slopes(
        &self,
    ) -> Option<&super::slope_coefficients::joint::JointFiniteWorkspace> {
        self.coefficient_solver
            .as_ref()
            .and_then(|s| s.joint.as_deref())
    }

    /// The owning session proved raw weight bits and every scale field equal.
    /// Only surviving authority on that exact frame can follow its new label.
    pub(crate) fn rebind_identical_weight_generation(
        &mut self,
        prior: MapGeometryGeneration,
        next: MapGeometryGeneration,
    ) {
        if prior.sample == next.sample && prior.topology == next.topology {
            if let Some(geometry) = self.certificate_geometry.as_mut() {
                if geometry.generation == Some(prior) {
                    geometry.generation = Some(next);
                }
            }
        }
    }

    pub(crate) fn invalidate_joint_slopes(&mut self) {
        if let Some(geometry) = self.certificate_geometry.as_mut() {
            geometry.generation = None;
        }
        if let Some(joint) = self
            .coefficient_solver
            .as_mut()
            .and_then(|s| s.joint.as_deref_mut())
        {
            joint.report = None;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn solve_joint_slopes(
        &mut self,
        weights: &[f64],
        controls: &DenseMatrix,
        selected: &[usize],
        response: &[f64],
        config: MapConfig,
        generation: MapGeometryGeneration,
    ) -> Result<()> {
        self.solve_joint_slopes_with(
            weights,
            response,
            config,
            generation,
            |joint, owner, design, geometry, reusable| {
                joint.solve(
                    owner, design, geometry, weights, controls, selected, response, config,
                    reusable,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn solve_joint_slopes_streamed<
        C: super::slope_coefficients::joint::JointFiniteDesign,
    >(
        &mut self,
        weights: &[f64],
        controls: &C,
        selected: &[usize],
        response: &[f64],
        config: MapConfig,
        generation: MapGeometryGeneration,
    ) -> Result<()> {
        self.solve_joint_slopes_with(
            weights,
            response,
            config,
            generation,
            |joint, owner, design, geometry, reusable| {
                joint.solve_streamed(
                    owner, design, geometry, weights, controls, selected, response, config,
                    reusable,
                )
            },
        )
    }

    fn solve_joint_slopes_with(
        &mut self,
        weights: &[f64],
        response: &[f64],
        config: MapConfig,
        generation: MapGeometryGeneration,
        solve: impl FnOnce(
            &mut super::slope_coefficients::joint::JointFiniteWorkspace,
            &mut super::slope_coefficients::SlopeCoefficientWorkspace,
            &FeDesign,
            &ReusableCertificateGeometry,
            bool,
        ) -> Result<usize>,
    ) -> Result<()> {
        let previous = self
            .certificate_geometry
            .as_mut()
            .and_then(|g| g.generation.take());
        let reusable = previous == Some(generation);
        let mut joint = self
            .coefficient_solver
            .as_mut()
            .and_then(|s| s.joint.take())
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::InvalidArgument,
                    "joint slope workspace is not admitted",
                )
            })?;
        joint.report = None;
        let result = (|| {
            validate_config(config)?;
            if weights.len() != self.design.nobs()
                || response.len() != self.design.nobs()
                || weights.iter().any(|w| !w.is_finite() || *w <= 0.)
                || response.iter().any(|y| !y.is_finite())
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "joint slope frame is invalid",
                ));
            }
            let started = Instant::now();
            let reference = response
                .iter()
                .zip(weights)
                .fold(0.0_f64, |m, (&y, &w)| m.max((y * w.sqrt()).abs()));
            let norm = if reference == 0. {
                0.
            } else {
                reference
                    * response
                        .iter()
                        .zip(weights)
                        .fold(0., |s, (&y, &w)| {
                            let v = y * w.sqrt() / reference;
                            v.mul_add(v, s)
                        })
                        .sqrt()
            };
            if !norm.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "joint slope reference norm is not finite",
                ));
            }
            self.reference_norms[0] = norm;
            if !reusable {
                self.prepare_coefficient_geometry(weights)?;
            }
            let setup_seconds = started.elapsed().as_secs_f64();
            let solving = Instant::now();
            let iterations = solve(
                &mut joint,
                self.coefficient_solver.as_mut().unwrap(),
                &self.design,
                self.certificate_geometry.as_ref().unwrap(),
                reusable,
            )?;
            let mut valid = true;
            let certificate =
                self.certify(&joint.residuals, weights, 1, config.tolerance, &mut valid)?;
            if !certificate.passed {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "joint slope independent FE certificate failed",
                ));
            }
            joint.report = Some(reusable_map_report(
                SolveStopReason::Certified,
                iterations,
                certificate,
                setup_seconds,
                solving.elapsed().as_secs_f64(),
            ));
            self.certificate_geometry.as_mut().unwrap().generation = Some(generation);
            Ok(())
        })();
        self.coefficient_solver.as_mut().unwrap().joint = Some(joint);
        result
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
            accelerator_state: self.certificate_groups.capacity()
                + self.coefficient_solver.as_ref().map_or(0, |s| s.elements()),
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

    /// Hand off statistics from this completed coefficient projection only.
    /// Both the consumer and solver use the supplied normalized weights. No
    /// statistic from a previous RHS, weight frame or failed call is exposed.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn absorb_coefficient_with_selected_norms(
        &mut self,
        weights: &[f64],
        values: DenseMatrix,
        config: MapConfig,
        generation: MapGeometryGeneration,
        positions: impl ExactSizeIterator<Item = usize> + Clone,
        reference: Option<&mut [f64]>,
        residual: &mut [f64],
    ) -> Result<CertifiedBorrowedMap> {
        let previous = self
            .certificate_geometry
            .as_mut()
            .and_then(|geometry| geometry.generation.take());
        if self.coefficient_solver.is_none()
            || self.design.effects().is_empty()
            || positions.len() != residual.len()
            || reference
                .as_ref()
                .is_some_and(|norms| norms.len() != residual.len())
            || positions.clone().any(|position| position >= values.ncols())
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "coefficient projection norm handoff is not aligned",
            ));
        }
        // Restore only at the delegated entry, which immediately takes this
        // authority itself. Selection validation and output publication keep it
        // revoked, including if an iterator panics.
        self.certificate_geometry
            .as_mut()
            .expect("coefficient geometry")
            .generation = previous;
        let result = self.absorb_in_generation(weights, values, config, false, Some(generation))?;
        let completed = self
            .certificate_geometry
            .as_mut()
            .expect("coefficient geometry")
            .generation
            .take();
        // That route always computes input norms before solving and residual
        // norms in its mandatory independent final certificate. Validate the
        // whole selection before publishing either caller-owned output.
        if positions.clone().any(|column| {
            !self.reference_norms[column].is_finite() || !self.residual_norms[column].is_finite()
        }) {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "coefficient projection norms are not finite",
            ));
        }
        if let Some(reference) = reference {
            for (out, column) in reference.iter_mut().zip(positions.clone()) {
                *out = self.reference_norms[column];
            }
        }
        for (out, column) in residual.iter_mut().zip(positions) {
            *out = self.residual_norms[column];
        }
        self.certificate_geometry
            .as_mut()
            .expect("coefficient geometry")
            .generation = completed;
        Ok(result)
    }

    pub(crate) fn absorb_in_generation(
        &mut self,
        weights: &[f64],
        values: DenseMatrix,
        config: MapConfig,
        structural_constant_if_empty: bool,
        generation: Option<MapGeometryGeneration>,
    ) -> Result<CertifiedBorrowedMap> {
        self.absorb_with_residual_gate(
            weights,
            values,
            config,
            structural_constant_if_empty,
            generation,
            false,
        )
    }

    /// Canonical finite residuals are assembled from projected columns. A fresh
    /// full certificate can avoid correction; a refusal keeps the normal solve.
    pub(crate) fn absorb_coefficient_residual(
        &mut self,
        weights: &[f64],
        values: DenseMatrix,
        config: MapConfig,
        generation: MapGeometryGeneration,
    ) -> Result<CertifiedBorrowedMap> {
        self.absorb_with_residual_gate(weights, values, config, false, Some(generation), true)
    }

    fn absorb_with_residual_gate(
        &mut self,
        weights: &[f64],
        values: DenseMatrix,
        config: MapConfig,
        structural_constant_if_empty: bool,
        generation: Option<MapGeometryGeneration>,
        certify_residual: bool,
    ) -> Result<CertifiedBorrowedMap> {
        // Revoke before fallible work; only a freshly certified result can
        // restore unchanged sample/topology/weight geometry authority.
        let previous = self
            .certificate_geometry
            .as_mut()
            .and_then(|g| g.generation.take());
        let reusable_geometry = generation.is_some() && previous == generation;
        let result = self.absorb_current_geometry(
            weights,
            values,
            config,
            structural_constant_if_empty,
            reusable_geometry,
            certify_residual,
        );
        if result.is_ok() {
            if let Some(geometry) = self.certificate_geometry.as_mut() {
                geometry.generation = generation;
            }
        }
        result
    }

    fn absorb_current_geometry(
        &mut self,
        weights: &[f64],
        mut values: DenseMatrix,
        config: MapConfig,
        structural_constant_if_empty: bool,
        reusable_geometry: bool,
        certify_residual: bool,
    ) -> Result<CertifiedBorrowedMap> {
        validate_config(config)?;
        let rhs = values.ncols();
        if values.nrows() != self.design.nobs()
            || weights.len() != values.nrows()
            || rhs == 0
            || rhs > self.max_rhs
            || (certify_residual && (rhs != 1 || self.coefficient_solver.is_none()))
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
        // The initial certificate sees this exact input and weight frame.
        self.residual_norms[..rhs].copy_from_slice(&self.reference_norms[..rhs]);
        let setup_seconds = setup_started.elapsed().as_secs_f64();
        let solve_started = Instant::now();

        if self.design.effects().is_empty() {
            return self.absorb_empty(
                values,
                weights,
                rhs,
                config,
                structural_constant_if_empty,
                setup_seconds,
                solve_started,
            );
        }
        if self.coefficient_solver.is_some() {
            return self.absorb_coefficients(
                values,
                weights,
                rhs,
                config,
                certify_residual,
                reusable_geometry,
                setup_seconds,
                solve_started,
            );
        }
        let mut geometry_valid = reusable_geometry;
        let initial = self.certify_with_current_norms(
            &values,
            weights,
            rhs,
            config.tolerance,
            &mut geometry_valid,
        )?;
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

    #[allow(clippy::too_many_arguments)]
    fn absorb_empty(
        &mut self,
        mut values: DenseMatrix,
        weights: &[f64],
        rhs: usize,
        config: MapConfig,
        structural_constant_if_empty: bool,
        setup_seconds: f64,
        solve_started: Instant,
    ) -> Result<CertifiedBorrowedMap> {
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
        let initial = self.certify_structural_constant(&values, weights, rhs, config.tolerance)?;
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
        CertifiedBorrowedMap::from_certified_parts(
            values,
            reusable_map_report(
                SolveStopReason::Certified,
                1,
                certificate,
                setup_seconds,
                solve_started.elapsed().as_secs_f64(),
            ),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn absorb_coefficients(
        &mut self,
        mut values: DenseMatrix,
        weights: &[f64],
        rhs: usize,
        config: MapConfig,
        certify_residual: bool,
        reusable_geometry: bool,
        setup_seconds: f64,
        solve_started: Instant,
    ) -> Result<CertifiedBorrowedMap> {
        // Generation authority covers both QR geometry and coupling. If
        // either must be prepared, keep the normal solve so an early return
        // cannot authorize reuse of an unprepared current-weight operator.
        if certify_residual && reusable_geometry {
            let certificate = self.certify(&values, weights, rhs, config.tolerance, &mut true)?;
            if certificate.passed {
                let result = CertifiedBorrowedMap::from_certified_parts(
                    values,
                    reusable_map_report(
                        SolveStopReason::Certified,
                        0,
                        certificate,
                        setup_seconds,
                        solve_started.elapsed().as_secs_f64(),
                    ),
                )?;
                return Ok(result);
            }
        }
        if !reusable_geometry {
            self.prepare_coefficient_geometry(weights)?;
        }
        let mut geometry_valid = true;
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
        Ok(result)
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
        if let Some(solver) = self.coefficient_solver.as_mut() {
            if solver.prepare_geometry(&self.design, weights, geometry)? {
                return Ok(());
            }
        }
        // Independent certification banks are idle during geometry setup.
        // Lend their admitted columns storage; geometry owns the disjoint output.
        let mut scratch = [
            self.certificate_columns.as_mut_slice(),
            self.certificate_basis.as_mut_slice(),
        ];
        if !prepare_geometry_with_scratch(&self.design, weights, geometry, &mut scratch)? {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "admitted geometry scratch is smaller than its design",
            ));
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
        self.certify_with_current_norms(residuals, weights, rhs, tolerance, geometry_valid)
    }

    fn certify_with_current_norms(
        &mut self,
        residuals: &DenseMatrix,
        weights: &[f64],
        rhs: usize,
        tolerance: f64,
        geometry_valid: &mut bool,
    ) -> Result<FeCertificate> {
        for column in 0..rhs {
            self.rhs_scales[column] = self.reference_norms[column].max(self.residual_norms[column]);
        }
        if self.certificate_geometry.is_some() && !self.certificate_groups.is_empty() {
            if !*geometry_valid {
                self.prepare_coefficient_geometry(weights)?;
                *geometry_valid = true;
            }
            return Ok(self.certify_groups(residuals, weights, rhs, tolerance));
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
    fn certify_groups(
        &mut self,
        residuals: &DenseMatrix,
        weights: &[f64],
        rhs: usize,
        tolerance: f64,
    ) -> FeCertificate {
        let stride = self.max_rhs + 1;
        let geometry = self
            .certificate_geometry
            .as_ref()
            .expect("admitted geometry");
        let mut first_group = 0;
        let mut first_basis = 0;
        for effect in self.design.effects() {
            let groups = effect.n_levels();
            let bank =
                &mut self.certificate_groups[first_group * stride..(first_group + groups) * stride];
            crate::parallel::stage_try_for_each(
                bank.par_chunks_mut(stride).enumerate(),
                self.design.nobs() as u128 * rhs as u128 * effect.channels() as u128,
                |(level, result)| {
                    let members = effect.level_index().members(level);
                    let rank = geometry.ranks[first_group + level];
                    let start = first_basis
                        + effect.channels() * effect.level_index().membership_offsets()[level];
                    let basis = &geometry.basis[start..start + members.len() * rank];
                    certify_reusable_group(
                        effect,
                        members,
                        weights,
                        residuals,
                        &self.rhs_scales[..rhs],
                        basis,
                        rank,
                        &mut result[..rhs + 1],
                    );

                    Ok(())
                },
            )
            .expect("infallible FE certificate stage");
            first_group += groups;
            first_basis += effect.channels() * self.design.nobs();
        }
        let mut absolute = 0.0_f64;
        let mut maximum = 0.0_f64;
        let mut square = 0.0_f64;
        for result in self.certificate_groups.chunks(stride) {
            absolute = absolute.max(result[0]);
            for &relative in &result[1..rhs + 1] {
                maximum = maximum.max(relative);
                square = relative.mul_add(relative, square);
            }
        }
        let count = first_group * rhs;
        FeCertificate {
            absolute_infinity_norm: absolute,
            relative_infinity_norm: maximum,
            relative_two_norm: if count == 0 {
                0.0
            } else {
                (square / count as f64).sqrt()
            },
            passed: maximum <= tolerance,
        }
    }
}

/// Prepare independent local bases using already admitted scratch owners.
/// Geometry preparation never grants generation or RHS certificate authority.
pub(super) fn prepare_geometry_with_scratch<S: AsMut<[f64]> + Send>(
    design: &FeDesign,
    weights: &[f64],
    geometry: &mut ReusableCertificateGeometry,
    scratch: &mut [S],
) -> Result<bool> {
    let available = scratch
        .iter_mut()
        .map(|s| s.as_mut().len())
        .min()
        .unwrap_or(0);
    if design.effects().iter().any(|e| {
        (0..e.n_levels())
            .any(|level| e.level_index().members(level).len() > available / e.channels())
    }) {
        return Ok(false);
    }
    let (mut first_basis, mut first_group) = (0, 0);
    for effect in design.effects() {
        let size = effect.channels() * design.nobs();
        let groups = effect.n_levels();
        let work = design.nobs() as u128 * effect.channels() as u128 * effect.channels() as u128;
        let lanes = crate::parallel::StagePlan::new(
            work,
            groups,
            scratch.len().min(rayon::current_num_threads()),
        )
        .lanes;
        prepare_geometry_blocks(
            effect,
            weights,
            0,
            &mut scratch[..lanes],
            &mut geometry.basis[first_basis..first_basis + size],
            &mut geometry.ranks[first_group..first_group + groups],
        )?;
        first_basis += size;
        first_group += groups;
    }
    Ok(true)
}

fn prepare_geometry_blocks<S: AsMut<[f64]> + Send>(
    effect: &super::EffectTerm,
    weights: &[f64],
    first: usize,
    scratch: &mut [S],
    basis: &mut [f64],
    ranks: &mut [usize],
) -> Result<()> {
    if ranks.is_empty() {
        return Ok(());
    }
    let offsets = effect.level_index().membership_offsets();
    let base = offsets[first];
    if scratch.len() == 1 {
        let columns = scratch[0].as_mut();
        for (offset, rank) in ranks.iter_mut().enumerate() {
            let level = first + offset;
            let members = effect.level_index().members(level);
            let start = (offsets[level] - base) * effect.channels();
            let size = members.len() * effect.channels();
            *rank = prepare_reusable_group_basis(
                effect,
                members,
                weights,
                members.len(),
                &mut columns[..size],
                &mut basis[start..start + size],
            )?;
        }
        return Ok(());
    }
    let left_lanes = scratch.len() / 2;
    let right_lanes = scratch.len() - left_lanes;
    let work = offsets[first + ranks.len()] - base;
    let target = base + (work as u128 * left_lanes as u128 / scratch.len() as u128) as usize;
    let middle = offsets[first..=first + ranks.len()]
        .partition_point(|&v| v < target)
        .clamp(left_lanes, ranks.len() - right_lanes);
    let split = (offsets[first + middle] - base) * effect.channels();
    let (left_basis, right_basis) = basis.split_at_mut(split);
    let (left_ranks, right_ranks) = ranks.split_at_mut(middle);
    let (left, right) = scratch.split_at_mut(left_lanes);
    let (a, b) = rayon::join(
        || prepare_geometry_blocks(effect, weights, first, left, left_basis, left_ranks),
        || {
            prepare_geometry_blocks(
                effect,
                weights,
                first + middle,
                right,
                right_basis,
                right_ranks,
            )
        },
    );
    a?;
    b
}

// Preserve every scalar entry's observation order while sharing row loads,
// weight roots and raw-channel products across a bounded RHS tile.
// Keep the legacy workspace and ownership boundaries explicit during migration.
#[allow(clippy::too_many_arguments)]
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

pub(super) fn prepare_reusable_group_basis(
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
        basis[basis_start..basis_start + member_count]
            .copy_from_slice(&columns[candidate_start..candidate_start + member_count]);
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
    crate::memory::Capacity::new(elements as u128).filled(0.0, label)
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
    super::norms::column_norms(
        values,
        |row| Ok(weights[row].sqrt()),
        output,
        scales,
        sums,
        "reusable MAP weighted norm is not finite",
        "reusable MAP weighted norm is not finite",
    )
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

pub(super) fn reusable_norm(values: &[f64]) -> f64 {
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
    fn joint_slope_directions_match_analytic_weighted_oracle_and_revoke_failed_frames() {
        let mut f = Vec::new();
        let mut g = Vec::new();
        let mut slope = Vec::new();
        let mut raw = Vec::new();
        let mut cells = Vec::new();
        for component in 0..2 {
            for a in 0..3 {
                for b in 0..4 {
                    for r in 0..5 {
                        let t = r as f64 - 2.;
                        f.push((component * 3 + a) as u32);
                        g.push((component * 4 + b) as u32);
                        slope.push(t);
                        raw.extend_from_slice(&[t * t * t * 1e5, 1., t * t]);
                        cells.push((a, b, r));
                    }
                }
            }
        }
        let n = f.len();
        let controls = DenseMatrix::from_row_major(n, 3, raw).unwrap();
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(
                    f,
                    6,
                    true,
                    vec![slope.clone(), slope.iter().map(|v| 2. * v).collect()],
                )
                .unwrap(),
                EffectTerm::new(g, 8, true, vec![slope.clone()]).unwrap(),
            ],
        )
        .unwrap();
        for threads in [1, 2, 4] {
            for coupling in [0, 1, 2] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let mut owner = ReusableMapAbsorber::new(design.clone(), 4).unwrap();
                owner.enable_certificate_geometry().unwrap();
                owner.enable_coefficient_solver(threads).unwrap();
                owner.enable_certificate_workspace().unwrap();
                if coupling != 0 {
                    owner.enable_coefficient_coupling().unwrap();
                    if coupling == 2 {
                        owner
                            .coefficient_solver
                            .as_mut()
                            .unwrap()
                            .force_compact_coupling_for_test();
                    }
                }
                owner.enable_joint_slopes().unwrap();
                let capacity = owner.capacities();
                let charged = ReusableMapAbsorber::joint_slope_bytes(&design, 3).unwrap();
                let joint = owner.joint_slopes().unwrap();
                assert!(
                    charged >= 8 * joint.elements() as u128 + std::mem::size_of_val(joint) as u128
                );
                let config = MapConfig {
                    tolerance: 1e-12,
                    ..MapConfig::default()
                };
                for frame in 0..4 {
                    let weights: Vec<_> = cells
                        .iter()
                        .map(|&(a, b, r)| {
                            (1. + a as f64 + 2. * b as f64)
                                * (1. + frame as f64 * 0.25 + r as f64 * r as f64 * 0.1)
                        })
                        .collect();
                    let residual: Vec<_> = cells
                        .iter()
                        .zip(&weights)
                        .map(|(&(_, _, r), &w)| [1., -4., 6., -4., 1.][r] * 0.01 / w)
                        .collect();
                    let y: Vec<_> = cells
                        .iter()
                        .enumerate()
                        .map(|(i, &(a, b, _))| {
                            0.7 * controls[(i, 2)] - 3e-6 * controls[(i, 0)] + a as f64 * 0.2
                                - b as f64 * 0.1
                                + (a + b) as f64 * slope[i] * 0.05
                                + residual[i]
                        })
                        .collect();
                    let generation = MapGeometryGeneration {
                        sample: 1,
                        topology: 1,
                        weights: frame + 1,
                    };
                    pool.install(|| {
                        owner.solve_joint_slopes(
                            &weights,
                            &controls,
                            &[2, 0],
                            &y,
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                    let out = owner.joint_slopes().unwrap();
                    assert!(out.report.as_ref().unwrap().certified());
                    assert!((out.beta()[0] - 0.7).abs() < 1e-10);
                    assert!((out.beta()[1] + 3e-6).abs() < 1e-13);
                    for (&a, &b) in out.residuals.as_slice().iter().zip(&residual) {
                        assert!((a - b).abs() < 1e-10, "{a} != {b}");
                    }
                    assert_eq!(capacity, owner.capacities());
                    let changed: Vec<_> = y.iter().map(|v| 2. * v).collect();
                    pool.install(|| {
                        owner.solve_joint_slopes(
                            &weights,
                            &controls,
                            &[2, 0],
                            &changed,
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                    assert!((owner.joint_slopes().unwrap().beta()[0] - 1.4).abs() < 1e-10);
                    let zero = vec![0.; y.len()];
                    pool.install(|| {
                        owner.solve_joint_slopes(
                            &weights,
                            &controls,
                            &[2, 0],
                            &zero,
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                    assert!(owner
                        .joint_slopes()
                        .unwrap()
                        .beta()
                        .iter()
                        .all(|&v| v == 0.));
                    assert!(owner
                        .joint_slopes()
                        .unwrap()
                        .residuals
                        .as_slice()
                        .iter()
                        .all(|&v| v == 0.));
                    let short = MapConfig {
                        max_iterations: 1,
                        ..config
                    };
                    let short_error = pool
                        .install(|| {
                            owner.solve_joint_slopes(
                                &weights,
                                &controls,
                                &[2, 0],
                                &y,
                                short,
                                generation,
                            )
                        })
                        .unwrap_err();
                    assert_eq!(short_error.code, ErrorCode::NonConvergence);
                    assert!(owner.joint_slopes().unwrap().report.is_none());

                    let error = pool
                        .install(|| {
                            owner.solve_joint_slopes(
                                &weights,
                                &controls,
                                &[2, 2],
                                &y,
                                config,
                                generation,
                            )
                        })
                        .unwrap_err();
                    assert_eq!(error.code, ErrorCode::NonConvergence);
                    assert!(owner.joint_slopes().unwrap().report.is_none());
                    assert!(owner
                        .certificate_geometry
                        .as_ref()
                        .unwrap()
                        .generation
                        .is_none());
                    pool.install(|| {
                        owner.solve_joint_slopes(
                            &weights,
                            &controls,
                            &[2, 0],
                            &y,
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                    let mut bad = weights.clone();
                    bad[0] = f64::NAN;
                    assert!(owner
                        .solve_joint_slopes(&bad, &controls, &[2, 0], &y, config, generation)
                        .is_err());
                    assert!(owner.joint_slopes().unwrap().report.is_none());
                    assert!(owner
                        .certificate_geometry
                        .as_ref()
                        .unwrap()
                        .generation
                        .is_none());
                }
            }
        }
        assert!(ReusableMapAbsorber::joint_slope_bytes(&design, usize::MAX).is_err());
    }

    #[test]
    fn streamed_joint_slope_directions_match_analytic_weighted_oracle_and_revoke_failed_frames() {
        check_streamed_joint_slope_directions::<false>();
    }

    #[test]
    fn column_oriented_joint_slope_directions_match_analytic_weighted_oracle_and_revoke_failed_frames(
    ) {
        check_streamed_joint_slope_directions::<true>();
    }

    fn check_streamed_joint_slope_directions<const COLUMNS: bool>() {
        use super::super::slope_coefficients::joint::JointFiniteDesign;
        struct Layout<'a, const COLUMNS: bool>(&'a DenseMatrix);
        impl<const COLUMNS: bool> JointFiniteDesign for Layout<'_, COLUMNS> {
            fn prefers_column_scans(&self) -> bool {
                COLUMNS
            }
            fn nrows(&self) -> usize {
                self.0.nrows()
            }
            fn ncols(&self) -> usize {
                self.0.ncols()
            }
            fn visit_column_range(
                &self,
                column: usize,
                range: std::ops::Range<usize>,
                visit: impl FnMut(usize, f64) -> Result<()>,
            ) -> Result<()> {
                self.0.visit_column_range(column, range, visit)
            }
            fn visit_rows(
                &self,
                scratch: &mut [f64],
                visit: impl FnMut(usize, &[f64]) -> Result<()>,
            ) -> Result<()> {
                self.0.visit_rows(scratch, visit)
            }
        }
        let mut f = Vec::new();
        let mut g = Vec::new();
        let mut slope = Vec::new();
        let mut raw = Vec::new();
        let mut cells = Vec::new();
        for component in 0..2 {
            for a in 0..3 {
                for b in 0..4 {
                    for r in 0..5 {
                        let t = r as f64 - 2.;
                        f.push((component * 3 + a) as u32);
                        g.push((component * 4 + b) as u32);
                        slope.push(t);
                        raw.extend_from_slice(&[t * t * t * 1e5, 1., t * t]);
                        cells.push((a, b, r));
                    }
                }
            }
        }
        let n = f.len();
        let controls = DenseMatrix::from_row_major(n, 3, raw).unwrap();
        let streamed = Layout::<COLUMNS>(&controls);
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(
                    f,
                    6,
                    true,
                    vec![slope.clone(), slope.iter().map(|v| 2. * v).collect()],
                )
                .unwrap(),
                EffectTerm::new(g, 8, true, vec![slope.clone()]).unwrap(),
            ],
        )
        .unwrap();
        for threads in [1, 2, 4] {
            for coupling in [0, 1, 2] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let mut owner =
                    ReusableMapAbsorber::new(design.clone(), if coupling == 2 { 4 } else { 1 })
                        .unwrap();
                owner.enable_certificate_geometry().unwrap();
                owner.enable_coefficient_solver(threads).unwrap();
                owner.enable_certificate_workspace().unwrap();
                if coupling != 0 {
                    owner.enable_coefficient_coupling().unwrap();
                    if coupling == 2 {
                        owner
                            .coefficient_solver
                            .as_mut()
                            .unwrap()
                            .force_compact_coupling_for_test();
                    }
                }
                owner.enable_joint_slopes_for_columns(3).unwrap();
                let capacity = owner.capacities();
                let charged = ReusableMapAbsorber::joint_slope_bytes(&design, 3).unwrap();
                let joint = owner.joint_slopes().unwrap();
                assert!(
                    charged >= 8 * joint.elements() as u128 + std::mem::size_of_val(joint) as u128
                );
                let config = MapConfig {
                    tolerance: 1e-12,
                    ..MapConfig::default()
                };
                for frame in 0..4 {
                    let weights: Vec<_> = cells
                        .iter()
                        .map(|&(a, b, r)| {
                            (1. + a as f64 + 2. * b as f64)
                                * (1. + frame as f64 * 0.25 + r as f64 * r as f64 * 0.1)
                        })
                        .collect();
                    let residual: Vec<_> = cells
                        .iter()
                        .zip(&weights)
                        .map(|(&(_, _, r), &w)| [1., -4., 6., -4., 1.][r] * 0.01 / w)
                        .collect();
                    let y: Vec<_> = cells
                        .iter()
                        .enumerate()
                        .map(|(i, &(a, b, _))| {
                            0.7 * controls[(i, 2)] - 3e-6 * controls[(i, 0)] + a as f64 * 0.2
                                - b as f64 * 0.1
                                + (a + b) as f64 * slope[i] * 0.05
                                + residual[i]
                        })
                        .collect();
                    let generation = MapGeometryGeneration {
                        sample: 1,
                        topology: 1,
                        weights: frame + 1,
                    };
                    pool.install(|| {
                        owner.solve_joint_slopes_streamed(
                            &weights,
                            &streamed,
                            &[2, 0],
                            &y,
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                    let out = owner.joint_slopes().unwrap();
                    assert!(out.report.as_ref().unwrap().certified());
                    for (&column, &norm) in [2, 0].iter().zip(out.input_norms().unwrap()) {
                        let squared: f64 = weights
                            .iter()
                            .enumerate()
                            .map(|(row, &weight)| weight * controls[(row, column)].powi(2))
                            .sum();
                        assert!((norm / squared.sqrt() - 1.).abs() < 2e-14);
                    }
                    assert!((out.beta()[0] - 0.7).abs() < 1e-10);
                    assert!((out.beta()[1] + 3e-6).abs() < 1e-13);
                    for (&a, &b) in out.residuals.as_slice().iter().zip(&residual) {
                        assert!((a - b).abs() < 1e-10, "{a} != {b}");
                    }
                    assert_eq!(capacity, owner.capacities());
                    let changed: Vec<_> = y.iter().map(|v| 2. * v).collect();
                    pool.install(|| {
                        owner.solve_joint_slopes_streamed(
                            &weights,
                            &streamed,
                            &[2, 0],
                            &changed,
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                    assert!((owner.joint_slopes().unwrap().beta()[0] - 1.4).abs() < 1e-10);
                    let zero = vec![0.; y.len()];
                    pool.install(|| {
                        owner.solve_joint_slopes_streamed(
                            &weights,
                            &streamed,
                            &[2, 0],
                            &zero,
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                    assert!(owner
                        .joint_slopes()
                        .unwrap()
                        .beta()
                        .iter()
                        .all(|&v| v == 0.));
                    assert!(owner
                        .joint_slopes()
                        .unwrap()
                        .residuals
                        .as_slice()
                        .iter()
                        .all(|&v| v == 0.));
                    let short = MapConfig {
                        max_iterations: 1,
                        ..config
                    };
                    let short_error = pool
                        .install(|| {
                            owner.solve_joint_slopes_streamed(
                                &weights,
                                &streamed,
                                &[2, 0],
                                &y,
                                short,
                                generation,
                            )
                        })
                        .unwrap_err();
                    assert_eq!(short_error.code, ErrorCode::NonConvergence);
                    assert!(owner.joint_slopes().unwrap().report.is_none());
                    assert!(owner.joint_slopes().unwrap().input_norms().is_none());

                    let error = pool
                        .install(|| {
                            owner.solve_joint_slopes_streamed(
                                &weights,
                                &streamed,
                                &[2, 2],
                                &y,
                                config,
                                generation,
                            )
                        })
                        .unwrap_err();
                    assert_eq!(error.code, ErrorCode::NonConvergence);
                    assert!(owner.joint_slopes().unwrap().report.is_none());
                    assert!(owner.joint_slopes().unwrap().input_norms().is_none());
                    assert!(owner
                        .certificate_geometry
                        .as_ref()
                        .unwrap()
                        .generation
                        .is_none());
                    pool.install(|| {
                        owner.solve_joint_slopes_streamed(
                            &weights,
                            &streamed,
                            &[2, 0],
                            &y,
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                    let mut bad = weights.clone();
                    bad[0] = f64::NAN;
                    assert!(owner
                        .solve_joint_slopes_streamed(
                            &bad,
                            &streamed,
                            &[2, 0],
                            &y,
                            config,
                            generation
                        )
                        .is_err());
                    assert!(owner.joint_slopes().unwrap().report.is_none());
                    assert!(owner.joint_slopes().unwrap().input_norms().is_none());
                    assert!(owner
                        .certificate_geometry
                        .as_ref()
                        .unwrap()
                        .generation
                        .is_none());
                }
            }
        }
        assert!(ReusableMapAbsorber::joint_slope_bytes(&design, usize::MAX).is_err());
    }

    #[test]
    fn common_geometry_without_coefficient_solver_preserves_frozen_active_basis() {
        let rows = 32_771;
        for skewed in [false, true] {
            let design = FeDesign::new(
                rows,
                vec![EffectTerm::new(
                    (0..rows)
                        .map(|r| {
                            if skewed && r < 30_000 {
                                0
                            } else {
                                (2 * (r % 17)) as u32
                            }
                        })
                        .collect(),
                    39,
                    true,
                    vec![
                        (0..rows)
                            .map(|r| ((r * 37 % 173) as f64 - 86.0) / 91.0)
                            .collect(),
                        (0..rows)
                            .map(|r| 2.0 * ((r * 37 % 173) as f64 - 86.0) / 91.0)
                            .collect(),
                        vec![0.; rows],
                    ],
                )
                .unwrap()],
            )
            .unwrap();
            for threads in [1, 2, 4, 7] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let mut owner = ReusableMapAbsorber::new(design.clone(), 3).unwrap();
                owner.enable_certificate_geometry().unwrap();
                assert!(owner.coefficient_solver.is_none());
                let capacity = owner.capacities();
                for scale in [1., 1e-80, 1e80, 1.] {
                    let weights: Vec<_> = (0..rows)
                        .map(|r| scale * (0.2 + (r * 13 % 17) as f64))
                        .collect();
                    let mut expected = Vec::new();
                    let mut expected_ranks = Vec::new();
                    let mut columns = vec![0.; owner.certificate_columns.len()];
                    let mut basis = vec![0.; owner.certificate_basis.len()];
                    // Frozen former padded-stride, serial traversal.
                    for effect in design.effects() {
                        for level in 0..effect.n_levels() {
                            let members = effect.level_index().members(level);
                            let rank = prepare_reusable_group_basis(
                                effect,
                                members,
                                &weights,
                                owner.max_level_members,
                                &mut columns,
                                &mut basis,
                            )
                            .unwrap();
                            expected_ranks.push(rank);
                            let mut active = Vec::new();
                            for channel in 0..rank {
                                let start = channel * owner.max_level_members;
                                active.extend_from_slice(&basis[start..start + members.len()]);
                            }
                            expected.push(active);
                        }
                    }
                    pool.install(|| owner.prepare_coefficient_geometry(&weights))
                        .unwrap();
                    assert_eq!(owner.capacities(), capacity);
                    let actual = owner.certificate_geometry.as_ref().unwrap();
                    assert_eq!(actual.ranks, expected_ranks);
                    assert!(actual.generation.is_none());
                    let (mut offset, mut group) = (0, 0);
                    for effect in design.effects() {
                        for level in 0..effect.n_levels() {
                            let members = effect.level_index().members(level).len();
                            let end = offset + members * actual.ranks[group];
                            assert_eq!(
                                actual.basis[offset..end]
                                    .iter()
                                    .map(|v| v.to_bits())
                                    .collect::<Vec<_>>(),
                                expected[group]
                                    .iter()
                                    .map(|v| v.to_bits())
                                    .collect::<Vec<_>>()
                            );
                            offset += members * effect.channels();
                            group += 1;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn parallel_slope_geometry_preserves_active_basis_and_scratch_refusal() {
        // Empty declared levels stress disjoint zero-length slices. Duplicate
        // and zero slopes stress inactive padding, which has no rank authority.
        let rows = 192;
        for oversized in [false, true] {
            let z: Vec<_> = (0..rows).map(|i| ((i / 8) as f64).sin()).collect();
            let design = FeDesign::new(
                rows,
                vec![
                    EffectTerm::new(
                        (0..rows)
                            .map(|i| if oversized { 0 } else { (2 * (i % 8)) as u32 })
                            .collect(),
                        19,
                        true,
                        vec![
                            z.clone(),
                            z.iter().map(|v| v * 2.).collect(),
                            vec![0.; rows],
                        ],
                    )
                    .unwrap(),
                    EffectTerm::new(
                        (0..rows).map(|i| (i % 11) as u32).collect(),
                        13,
                        false,
                        vec![(0..rows).map(|i| 1e120 * ((i * 7) as f64).cos()).collect()],
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
            let mut serial = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
            serial.enable_certificate_geometry().unwrap();
            for threads in [1, 2, 4, 7] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let mut candidate = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
                candidate.enable_certificate_geometry().unwrap();
                candidate.enable_coefficient_solver(threads).unwrap();
                let capacities = candidate.capacities();
                for (generation, scale) in [1., 1e-80, 1e80, 1.].into_iter().enumerate() {
                    let weights: Vec<_> = (0..rows)
                        .map(|i| scale * (0.2 + ((i * (generation + 3)) % 17) as f64))
                        .collect();
                    serial.prepare_coefficient_geometry(&weights).unwrap();
                    let before = candidate
                        .certificate_geometry
                        .as_ref()
                        .unwrap()
                        .basis
                        .clone();
                    let prepared = pool
                        .install(|| {
                            candidate
                                .coefficient_solver
                                .as_mut()
                                .unwrap()
                                .prepare_geometry(
                                    &design,
                                    &weights,
                                    candidate.certificate_geometry.as_mut().unwrap(),
                                )
                        })
                        .unwrap();
                    assert_eq!(prepared, threads > 1 && !oversized);
                    if !prepared {
                        assert_eq!(
                            candidate.certificate_geometry.as_ref().unwrap().basis,
                            before
                        );
                        pool.install(|| candidate.prepare_coefficient_geometry(&weights))
                            .unwrap();
                    }
                    let expected = serial.certificate_geometry.as_ref().unwrap();
                    let actual = candidate.certificate_geometry.as_ref().unwrap();
                    assert_eq!(actual.ranks, expected.ranks);
                    let mut group = 0;
                    let mut offset = 0;
                    for effect in design.effects() {
                        for level in 0..effect.n_levels() {
                            let members = effect.level_index().members(level).len();
                            let end = offset + members * expected.ranks[group];
                            assert_eq!(&actual.basis[offset..end], &expected.basis[offset..end]);
                            offset += members * effect.channels();
                            group += 1;
                        }
                    }
                    assert_eq!(candidate.capacities(), capacities);
                }
            }
        }
    }

    #[test]
    fn coefficient_residual_gate_checks_truth_refusal_and_weight_authority() {
        let n = 6 * 4 * 3;
        let ci: Vec<_> = (0..n).map(|r| (r / 12) as u32).collect();
        let cj: Vec<_> = (0..n).map(|r| ((r / 3) % 4) as u32).collect();
        let t: Vec<_> = (0..n).map(|r| (r % 3) as f64 - 1.).collect();
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(
                    ci.clone(),
                    6,
                    true,
                    vec![t.clone(), t.iter().map(|v| 2. * v).collect(), vec![0.; n]],
                )
                .unwrap(),
                EffectTerm::new(
                    cj.clone(),
                    4,
                    true,
                    vec![ci.iter().map(|&i| i as f64).collect()],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let config = MapConfig {
            tolerance: 1e-12,
            ..MapConfig::default()
        };
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut owner = ReusableMapAbsorber::new(design.clone(), 2).unwrap();
            owner.enable_certificate_geometry().unwrap();
            owner.enable_coefficient_solver(threads).unwrap();
            owner.enable_coefficient_coupling().unwrap();
            owner.enable_certificate_workspace().unwrap();
            let capacity = owner.capacities();
            for (frame, scale) in [1., 1e-60, 1e60].into_iter().enumerate() {
                let generation = MapGeometryGeneration {
                    sample: 1,
                    topology: 1,
                    weights: frame as u64 + 1,
                };
                let jw: Vec<_> = (0..4).map(|j| (j + frame + 1) as f64).collect();
                let mean = (0..4).map(|j| jw[j] * j as f64).sum::<f64>() / jw.iter().sum::<f64>();
                let weights: Vec<_> = (0..n)
                    .map(|r| scale * (1. + ci[r] as f64) * jw[cj[r] as usize])
                    .collect();
                // Weighted complete-product contrasts are orthogonal to every
                // declared channel, including the locally redundant slopes.
                let truth: Vec<_> = (0..n)
                    .map(|r| (ci[r] as f64 + 1.).sin() * (cj[r] as f64 - mean) * t[r])
                    .collect();
                let raw: Vec<_> = (0..n)
                    .map(|r| truth[r] + (ci[r] as f64).cos() * t[r] + (cj[r] as f64).sin())
                    .collect();
                let matrix = |x: &[f64]| DenseMatrix::from_row_major(n, 1, x.to_vec()).unwrap();
                let preparations = owner.certificate_geometry.as_ref().unwrap().preparations;
                let first = pool
                    .install(|| {
                        owner.absorb_coefficient_residual(
                            &weights,
                            matrix(&raw),
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                assert!(first.report().iterations > 0);
                assert_eq!(
                    owner.certificate_geometry.as_ref().unwrap().preparations,
                    preparations + 1
                );
                for (&a, &b) in first.residualized().as_slice().iter().zip(&truth) {
                    assert!((a - b).abs() < 1e-10, "{a} != {b}");
                }
                for rhs_scale in [1., -3., 1e-60, 1e60] {
                    let residual: Vec<_> = truth.iter().map(|v| v * rhs_scale).collect();
                    let accepted = pool
                        .install(|| {
                            owner.absorb_coefficient_residual(
                                &weights,
                                matrix(&residual),
                                config,
                                generation,
                            )
                        })
                        .unwrap();
                    assert!(accepted.report().certified());
                    assert_eq!(accepted.report().iterations, 0);
                    assert_eq!(accepted.residualized().as_slice(), residual.as_slice());
                }
                // Identical geometry does not authorize an untested RHS. A
                // material FE component must trigger the original correction.
                let corrected = pool
                    .install(|| {
                        owner.absorb_coefficient_residual(
                            &weights,
                            matrix(&raw),
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                assert!(corrected.report().iterations > 0);
                for (&a, &b) in corrected.residualized().as_slice().iter().zip(&truth) {
                    assert!((a - b).abs() < 1e-10);
                }
                let zero = vec![0.; n];
                let accepted = pool
                    .install(|| {
                        owner.absorb_coefficient_residual(
                            &weights,
                            matrix(&zero),
                            config,
                            generation,
                        )
                    })
                    .unwrap();
                assert_eq!(accepted.report().iterations, 0);
                assert_eq!(accepted.residualized().as_slice(), zero.as_slice());
                for failure in 0..3 {
                    let mut invalid = truth.clone();
                    if failure == 0 {
                        invalid[0] = f64::NAN;
                    }
                    let values = if failure == 1 {
                        DenseMatrix::zeros(n, 2).unwrap()
                    } else {
                        matrix(&invalid)
                    };
                    let cfg = if failure == 2 {
                        MapConfig {
                            max_iterations: 0,
                            ..config
                        }
                    } else {
                        config
                    };
                    assert!(pool
                        .install(
                            || owner.absorb_coefficient_residual(&weights, values, cfg, generation)
                        )
                        .is_err());
                    assert!(owner
                        .certificate_geometry
                        .as_ref()
                        .unwrap()
                        .generation
                        .is_none());
                    let before = owner.certificate_geometry.as_ref().unwrap().preparations;
                    let recovered = pool
                        .install(|| {
                            owner.absorb_coefficient_residual(
                                &weights,
                                matrix(&raw),
                                config,
                                generation,
                            )
                        })
                        .unwrap();
                    assert!(recovered.report().certified());
                    assert_eq!(
                        owner.certificate_geometry.as_ref().unwrap().preparations,
                        before + 1
                    );
                }
                assert_eq!(owner.capacities(), capacity);
            }
        }
    }

    #[test]
    fn coefficient_norm_handoff_tracks_current_columns_weights_and_failures() {
        let n = 6 * 4 * 3;
        let codes_i: Vec<_> = (0..n).map(|row| (row / 12) as u32).collect();
        let codes_j: Vec<_> = (0..n).map(|row| ((row / 3) % 4) as u32).collect();
        let t: Vec<_> = (0..n).map(|row| (row % 3) as f64 - 1.).collect();
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(
                    codes_i.clone(),
                    6,
                    true,
                    vec![t.clone(), t.iter().map(|v| 2. * v).collect(), vec![0.; n]],
                )
                .unwrap(),
                EffectTerm::new(
                    codes_j.clone(),
                    4,
                    true,
                    vec![codes_i.iter().map(|&i| i as f64).collect()],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let config = MapConfig {
            tolerance: 1e-12,
            ..MapConfig::default()
        };
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut owner = ReusableMapAbsorber::new(design.clone(), 4).unwrap();
            owner.enable_certificate_geometry().unwrap();
            owner.enable_coefficient_solver(threads).unwrap();
            owner.enable_coefficient_coupling().unwrap();
            let capacities = owner.capacities();
            for (frame, scale) in [1., 1e-120, 1e120].into_iter().enumerate() {
                let generation = MapGeometryGeneration {
                    sample: 1,
                    topology: 1,
                    weights: frame as u64 + 1,
                };
                let jw: Vec<_> = (0..4).map(|j| 0.4 + j as f64 + frame as f64).collect();
                let mean_j = (0..4).map(|j| jw[j] * j as f64).sum::<f64>() / jw.iter().sum::<f64>();
                let weights: Vec<_> = (0..n)
                    .map(|row| scale * (1. + codes_i[row] as f64) * jw[codes_j[row] as usize])
                    .collect();
                let mut last_reference = 0.;
                for rhs_frame in 0..2 {
                    let mut raw = DenseMatrix::zeros(n, 4).unwrap();
                    let mut expected = DenseMatrix::zeros(n, 4).unwrap();
                    let value_scale = [1., 1e-60, 1e60][frame] * (rhs_frame + 1) as f64;
                    for row in 0..n {
                        let i = codes_i[row] as f64;
                        let j = codes_j[row] as f64;
                        let remainder = (i + 1.).sin() * (j - mean_j) * t[row];
                        let fitted = i.sin() + i.cos() * t[row] + j.cos() + j.sin() * i;
                        for col in 0..3 {
                            let c = (col + 1) as f64;
                            raw[(row, col)] = value_scale * (c * fitted + remainder / c);
                            expected[(row, col)] = value_scale * remainder / c;
                        }
                    }
                    let selected = [2, 0, 3, 2];
                    let input_norm: Vec<_> = selected
                        .iter()
                        .map(|&col| {
                            (0..n)
                                .map(|row| weights[row] * raw[(row, col)] * raw[(row, col)])
                                .sum::<f64>()
                                .sqrt()
                        })
                        .collect();
                    let mut references = [f64::NAN; 4];
                    let mut residuals = [f64::NAN; 4];
                    let actual = pool
                        .install(|| {
                            owner.absorb_coefficient_with_selected_norms(
                                &weights,
                                raw.clone(),
                                config,
                                generation,
                                selected.into_iter(),
                                Some(&mut references),
                                &mut residuals,
                            )
                        })
                        .unwrap();
                    assert!(actual.report().certified());
                    for (a, e) in actual
                        .residualized()
                        .as_slice()
                        .iter()
                        .zip(expected.as_slice())
                    {
                        assert!((a - e).abs() <= 1e-9 * value_scale);
                    }
                    for (out, &col) in selected.iter().enumerate() {
                        // Direct sums use an independent arithmetic path, within
                        // ranges that cannot overflow or underflow in this oracle.
                        let norm = (0..n)
                            .map(|row| weights[row] * actual.residualized()[(row, col)].powi(2))
                            .sum::<f64>()
                            .sqrt();
                        assert!(
                            (references[out] - input_norm[out]).abs() <= 2e-14 * input_norm[out]
                        );
                        assert!((residuals[out] - norm).abs() <= 2e-14 * norm);
                    }
                    if rhs_frame == 1 {
                        assert!(references[1] > last_reference);
                    }
                    last_reference = references[1];
                    let preparations = owner.certificate_geometry.as_ref().unwrap().preparations;
                    for failure in 0..3 {
                        references.fill(-1.);
                        residuals.fill(-2.);
                        let mut bad_weights = weights.clone();
                        if failure == 1 {
                            bad_weights[0] = f64::NAN;
                        }
                        let positions = if failure == 0 { [2, 0, 4, 2] } else { selected };
                        let budget = if failure == 2 {
                            MapConfig {
                                max_iterations: 0,
                                ..config
                            }
                        } else {
                            config
                        };
                        assert!(pool
                            .install(|| owner.absorb_coefficient_with_selected_norms(
                                &bad_weights,
                                raw.clone(),
                                budget,
                                generation,
                                positions.into_iter(),
                                Some(&mut references),
                                &mut residuals,
                            ))
                            .is_err());
                        assert_eq!(references, [-1.; 4]);
                        assert_eq!(residuals, [-2.; 4]);
                        assert!(owner
                            .certificate_geometry
                            .as_ref()
                            .unwrap()
                            .generation
                            .is_none());
                    }
                    pool.install(|| {
                        owner.absorb_coefficient_with_selected_norms(
                            &weights,
                            raw,
                            config,
                            generation,
                            selected.into_iter(),
                            None,
                            &mut residuals,
                        )
                    })
                    .unwrap();
                    assert_eq!(
                        owner.certificate_geometry.as_ref().unwrap().preparations,
                        preparations + 1
                    );
                    assert_eq!(owner.capacities(), capacities);
                    if frame == 2 && rhs_frame == 1 {
                        let values = DenseMatrix::zeros(n, 4).unwrap();
                        let failure =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                let positions = selected
                                    .into_iter()
                                    .map(|_| panic!("injected selection unwind"));
                                owner.absorb_coefficient_with_selected_norms(
                                    &weights,
                                    values,
                                    config,
                                    generation,
                                    positions,
                                    Some(&mut references),
                                    &mut residuals,
                                )
                            }));
                        assert!(failure.is_err());
                        assert!(owner
                            .certificate_geometry
                            .as_ref()
                            .unwrap()
                            .generation
                            .is_none());
                    }
                }
            }
        }
    }

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
                for coupling_kind in 0..3 {
                    let prepared_coupling = coupling_kind != 0;
                    let mut reference: Option<DenseMatrix> = None;
                    for threads in [1, 4, 8] {
                        let pool = rayon::ThreadPoolBuilder::new()
                            .num_threads(threads)
                            .build()
                            .unwrap();
                        let mut solver = ReusableMapAbsorber::new(design.clone(), 7).unwrap();
                        solver.enable_certificate_geometry().unwrap();
                        solver.enable_coefficient_solver(threads).unwrap();
                        if prepared_coupling {
                            solver.enable_coefficient_coupling().unwrap();
                            if coupling_kind == 2 {
                                solver
                                    .coefficient_solver
                                    .as_mut()
                                    .unwrap()
                                    .force_compact_coupling_for_test();
                            }
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
                                    // Keep the legacy matrix indices and numerical traversal order explicit.
                                    #[allow(clippy::needless_range_loop)]
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
                                                (i as f64 + 1.).sin() * (j as f64 - mean) * sign
                                                    / tw;
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
            absorber.enable_certificate_workspace().unwrap();
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
        for coefficients in [false, true] {
            let mut cached = ReusableMapAbsorber::new(design.clone(), 3).unwrap();
            cached.enable_certificate_geometry().unwrap();
            if coefficients {
                cached.enable_coefficient_solver(1).unwrap();
            }
            cached.enable_certificate_workspace().unwrap();
            if coefficients {
                cached.enable_coefficient_coupling().unwrap();
            }
            let mut fresh = ReusableMapAbsorber::new(design.clone(), 3).unwrap();
            fresh.enable_certificate_geometry().unwrap();
            if coefficients {
                fresh.enable_coefficient_solver(1).unwrap();
            }
            if coefficients {
                fresh.enable_coefficient_coupling().unwrap();
            }
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
                assert_eq!(
                    cached
                        .coefficient_solver
                        .as_ref()
                        .and_then(|s| s.coupling_preparations()),
                    coefficients.then_some(step / 2 + 1)
                );
                if step == 1 {
                    let wrong_sample = MapGeometryGeneration {
                        sample: generation.sample + 1,
                        ..generation
                    };
                    cached.rebind_identical_weight_generation(generation, wrong_sample);
                    assert_eq!(
                        cached.certificate_geometry.as_ref().unwrap().generation,
                        Some(generation)
                    );
                    let next = MapGeometryGeneration {
                        weights: generation.weights + 100,
                        ..generation
                    };
                    cached.rebind_identical_weight_generation(generation, next);
                    let repeated = cached
                        .absorb_in_generation(
                            &weights,
                            values.clone(),
                            MapConfig::default(),
                            false,
                            Some(next),
                        )
                        .unwrap();
                    assert_eq!(repeated.residualized(), expected.residualized());
                    assert_eq!(
                        cached.certificate_geometry.as_ref().unwrap().preparations,
                        1
                    );
                    assert_eq!(
                        cached
                            .coefficient_solver
                            .as_ref()
                            .and_then(|s| s.coupling_preparations()),
                        coefficients.then_some(1)
                    );
                    generation = next;
                }
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
                    cached.rebind_identical_weight_generation(
                        generation,
                        MapGeometryGeneration {
                            weights: generation.weights + 1,
                            ..generation
                        },
                    );
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
                    assert_eq!(
                        cached
                            .coefficient_solver
                            .as_ref()
                            .and_then(|s| s.coupling_preparations()),
                        coefficients.then_some(4)
                    );
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
                    assert_eq!(
                        cached
                            .coefficient_solver
                            .as_ref()
                            .and_then(|s| s.coupling_preparations()),
                        coefficients.then_some(5)
                    );
                }
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
            assert!(tiled.coefficient_solver.is_none());
            let before = tiled.capacities().accelerator_state;
            tiled.enable_certificate_workspace().unwrap();
            assert_eq!(
                (tiled.capacities().accelerator_state - before) as u128 * 8,
                ReusableMapAbsorber::certificate_workspace_bytes(&design, 33).unwrap(),
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
            (rows * 5 * 8
                + 10 * std::mem::size_of::<usize>()
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
        parallel.enable_certificate_workspace().unwrap();
        assert!(parallel.coefficient_solver.is_none());
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
