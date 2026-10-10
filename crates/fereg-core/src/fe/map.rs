use super::certificate::CertificateContext;
use super::projection::{AutoProjectionWorkspace, CheckedWeights, ParallelProjectionPlan};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
#[cfg(test)]
use crate::fe::certify_residuals_with_reference_norms;
#[cfg(test)]
use crate::fe::ProjectionWorkspace;
use crate::fe::{
    weighted_column_norms, CertifiedBorrowedMap, FeCertificate, FeDesign, FeSolverKind,
    SolveReport, SolveStopReason,
};
use crate::linalg::neumaier_add;
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
            solve_seconds: started.elapsed().as_secs_f64(),
            ..SolveReport::single_route(
                FeSolverKind::Map,
                SolveStopReason::NoFixedEffects,
                certificate,
            )
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
    let checked_weights = CheckedWeights::new(weights)?;

    for iteration in 1..=config.max_iterations {
        crate::interrupt::checkpoint()?;
        for effect in design.effects() {
            workspace.project(effect, checked_weights, values)?;
        }
        if config.symmetric && design.effects().len() > 1 {
            for effect in design.effects().iter().rev().skip(1) {
                workspace.project(effect, checked_weights, values)?;
            }
        }
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            let certificate = context.certify(values, &reference_norms, config.tolerance)?;
            if certificate.passed {
                return Ok(SolveReport {
                    iterations: iteration,
                    normal_equation_residual: certificate.relative_two_norm,
                    solve_seconds: started.elapsed().as_secs_f64(),
                    ..SolveReport::single_route(
                        FeSolverKind::Map,
                        SolveStopReason::Certified,
                        certificate,
                    )
                });
            }
            final_certificate = Some(certificate);
        }
    }
    let final_certificate =
        final_certificate.expect("positive MAP budget always checks the final allowed iteration");

    Ok(SolveReport {
        iterations: config.max_iterations,
        normal_equation_residual: final_certificate.relative_two_norm,
        solve_seconds: started.elapsed().as_secs_f64(),
        ..SolveReport::single_route(
            FeSolverKind::Map,
            SolveStopReason::MaximumIterations,
            final_certificate,
        )
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
        let checked_weights = CheckedWeights::new(weights)?;
        for iteration in 1..=config.max_iterations {
            crate::interrupt::checkpoint()?;
            for effect in self.design.effects() {
                self.projection
                    .project(effect, checked_weights, &mut values)?;
            }
            if config.symmetric && self.design.effects().len() > 1 {
                for effect in self.design.effects().iter().rev().skip(1) {
                    self.projection
                        .project(effect, checked_weights, &mut values)?;
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
                    neumaier_add(scaled, &mut sums[column], &mut corrections[column]);
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
        iterations,
        normal_equation_residual: certificate.relative_two_norm,
        setup_seconds,
        solve_seconds,
        ..SolveReport::single_route(FeSolverKind::Map, stop_reason, certificate)
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
    neumaier_add(value, sum, correction);
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
        neumaier_add(scaled, &mut sum, &mut correction);
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
mod tests;
