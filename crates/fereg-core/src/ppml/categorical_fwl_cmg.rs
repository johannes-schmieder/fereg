//! PPML-owned fused FWL path over the unpolished CMG graph projection.
//!
//! Changing IRLS weights still requires one upstream hierarchy per ordinary
//! frame. The speedup in this tranche comes from projecting the frozen active
//! controls and working response together, omitting generic observation-space
//! CG polish, retrying only uncertified right-hand sides, and retaining every
//! fereg-owned dense workspace across frames.

use std::time::Instant;

#[cfg(test)]
use std::cell::Cell;

use rayon::prelude::*;

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    BorrowedMapWeights, CmgAbsorber, CmgConfig, CmgTopologyProfile, CmgUnpolishedProjectionReport,
    FeDesign, FeSolverKind, MapConfig, PreparedCmgTopology, CMG_MAX_NUISANCE_COLUMNS,
};
use crate::memory::FeSolverMemoryBudget;
use crate::options::ThreadConfig;
use crate::parallel::{InstalledThreadTeam, ThreadTeam};
use crate::prepared_fit_view::PreparedOlsFitView;

use super::categorical_fwl_lsmr::pivoted_cholesky_solve;
use super::categorical_lsmr::JointLsmrCertificate;
use super::PpmlIterationWorkspace;

const MOMENT_BLOCK_ROWS: usize = 32_768;

#[cfg(test)]
thread_local! {
    static FORCE_NUMERICAL_FAILURE_ONCE: Cell<bool> = const { Cell::new(false) };
    static FORCE_POST_PROJECTION_NUMERICAL_FAILURE_ONCE: Cell<bool> = const { Cell::new(false) };
    static FORCE_PROJECTION_CERTIFICATE_FAILURE_ONCE: Cell<Option<usize>> = const { Cell::new(None) };
    static TOTAL_RETRIED_RIGHT_HAND_SIDES: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn inject_numerical_failure_once() {
    FORCE_NUMERICAL_FAILURE_ONCE.set(true);
}

#[cfg(test)]
pub(crate) fn inject_post_projection_numerical_failure_once() {
    FORCE_POST_PROJECTION_NUMERICAL_FAILURE_ONCE.set(true);
}

#[cfg(test)]
pub(crate) fn inject_projection_certificate_failure_once(rhs: usize) {
    TOTAL_RETRIED_RIGHT_HAND_SIDES.set(0);
    FORCE_PROJECTION_CERTIFICATE_FAILURE_ONCE.set(Some(rhs));
}

#[cfg(test)]
pub(crate) fn take_total_retried_right_hand_sides() -> usize {
    TOTAL_RETRIED_RIGHT_HAND_SIDES.replace(0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ProjectionColumnCertificate {
    absolute_infinity_norm: f64,
    relative_infinity_norm: f64,
    relative_two_norm: f64,
    passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FusedFwlCmgReport {
    pub(crate) iterations: usize,
    pub(crate) effective_tolerance: f64,
    pub(crate) setup_seconds: f64,
    pub(crate) solve_seconds: f64,
    pub(crate) finish_seconds: f64,
    pub(crate) backward_error: f64,
    pub(crate) graph_right_hand_sides: usize,
    pub(crate) graph_batches: usize,
    pub(crate) nuisance_columns: usize,
    pub(crate) failed_right_hand_sides: usize,
    pub(crate) retried_right_hand_sides: usize,
    pub(crate) continued_after_certificate: bool,
    pub(crate) projection_absolute_infinity_norm: f64,
    pub(crate) projection_relative_infinity_norm: f64,
    pub(crate) projection_relative_two_norm: f64,
    pub(crate) certificate: JointLsmrCertificate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FusedFwlCmgFailureReport {
    pub(crate) iterations: usize,
    pub(crate) retries: usize,
    pub(crate) setup_seconds: f64,
    pub(crate) solve_seconds: f64,
    pub(crate) certification_seconds: f64,
    pub(crate) absolute_infinity_norm: Option<f64>,
    pub(crate) relative_infinity_norm: Option<f64>,
    pub(crate) relative_two_norm: Option<f64>,
    pub(crate) projection_absolute_infinity_norm: Option<f64>,
    pub(crate) backward_error: Option<f64>,
}

#[derive(Debug)]
pub(crate) enum FusedFwlCmgSolveOutcome {
    Certified(FusedFwlCmgReport),
    // Only typed numerical failure crosses this seam. Allocation, data,
    // interruption, and internal errors remain `Err` and cannot trigger MAP.
    NumericalFailure(FusedFwlCmgFailureReport),
}

#[derive(Debug, Default)]
struct FusedFwlCmgAttemptDiagnostics {
    iterations: usize,
    retries: usize,
    setup_seconds: f64,
    solve_seconds: f64,
    certification_seconds: f64,
    absolute_infinity_norm: Option<f64>,
    relative_infinity_norm: Option<f64>,
    relative_two_norm: Option<f64>,
    projection_absolute_infinity_norm: Option<f64>,
    backward_error: Option<f64>,
}

impl FusedFwlCmgAttemptDiagnostics {
    fn observe_projection(&mut self, report: CmgUnpolishedProjectionReport) {
        self.iterations = self.iterations.saturating_add(report.iterations);
        self.setup_seconds += report.setup_seconds;
        self.solve_seconds += report.solve_seconds;
        update_optional_max(&mut self.backward_error, report.backward_error);
    }

    fn observe_projection_certificates(&mut self, certificates: &[ProjectionColumnCertificate]) {
        for certificate in certificates {
            update_optional_max(
                &mut self.absolute_infinity_norm,
                certificate.absolute_infinity_norm,
            );
            update_optional_max(
                &mut self.relative_infinity_norm,
                certificate.relative_infinity_norm,
            );
            update_optional_max(&mut self.relative_two_norm, certificate.relative_two_norm);
            update_optional_max(
                &mut self.projection_absolute_infinity_norm,
                certificate.absolute_infinity_norm,
            );
        }
    }

    fn observe_final_certificate(&mut self, certificate: JointLsmrCertificate) {
        update_optional_max(
            &mut self.absolute_infinity_norm,
            certificate
                .finite_absolute_infinity_norm
                .max(certificate.fixed_effect_absolute_infinity_norm),
        );
        update_optional_max(
            &mut self.relative_infinity_norm,
            certificate.relative_infinity_norm,
        );
        update_optional_max(&mut self.relative_two_norm, certificate.relative_two_norm);
    }

    fn into_failure_report(mut self, elapsed_seconds: f64) -> FusedFwlCmgFailureReport {
        let accounted = self.setup_seconds + self.solve_seconds + self.certification_seconds;
        // A failed lower-level solve may not return its phase report. Preserve
        // that wall time as solve work instead of silently dropping the attempt.
        if elapsed_seconds > accounted {
            self.solve_seconds += elapsed_seconds - accounted;
        }
        FusedFwlCmgFailureReport {
            iterations: self.iterations,
            retries: self.retries,
            setup_seconds: self.setup_seconds,
            solve_seconds: self.solve_seconds,
            certification_seconds: self.certification_seconds,
            absolute_infinity_norm: self.absolute_infinity_norm,
            relative_infinity_norm: self.relative_infinity_norm,
            relative_two_norm: self.relative_two_norm,
            projection_absolute_infinity_norm: self.projection_absolute_infinity_norm,
            backward_error: self.backward_error,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FusedFwlCmgConfig {
    pub(crate) map: MapConfig,
    pub(crate) threads: ThreadConfig,
    pub(crate) batch_size: usize,
    pub(crate) solver_memory_budget: FeSolverMemoryBudget,
}

pub(crate) struct ReusableCategoricalFwlCmg {
    rows: usize,
    active_columns: usize,
    prepared_topology: PreparedCmgTopology,
    active_values: Vec<f64>,
    active_slots: Vec<usize>,
    projected: DenseMatrix,
    retry: DenseMatrix,
    projection_certificates: Vec<ProjectionColumnCertificate>,
    failed_right_hand_sides: Vec<usize>,
    gram: Vec<f64>,
    rhs: Vec<f64>,
    factor: Vec<f64>,
    forward: Vec<f64>,
    standardized_solution: Vec<f64>,
    scales: Vec<f64>,
    coefficients: Vec<f64>,
    permutation: Vec<usize>,
    moment_partials: Vec<f64>,
    residual: Vec<f64>,
    fitted_without_offset: Vec<f64>,
    team: InstalledThreadTeam,
    config: FusedFwlCmgConfig,
}

impl ReusableCategoricalFwlCmg {
    pub(crate) fn new<P: PreparedOlsFitView + Sync + ?Sized>(
        prepared: &P,
        active_generated: &[usize],
        fe_design: &FeDesign,
        config: FusedFwlCmgConfig,
    ) -> Result<Self> {
        let rows = fe_design.nobs();
        if rows == 0 || rows != prepared.fit_sample().n_active() {
            return Err(invalid_argument(
                "fused PPML CMG requires a nonempty aligned fit sample",
            ));
        }
        if config.batch_size == 0 {
            return Err(invalid_argument(
                "fused PPML CMG batch size must be positive",
            ));
        }
        if !(2..=3).contains(&fe_design.effects().len())
            || fe_design
                .effects()
                .iter()
                .any(|effect| !effect.has_intercept() || !effect.slopes().is_empty())
        {
            return Err(unsupported(
                "fused PPML CMG requires two or three aligned categorical intercept-only effects",
            ));
        }
        let borrowed = prepared.fit_borrowed_cmg_design()?;
        let profile = CmgTopologyProfile::analyze(&borrowed)?;
        if profile.nuisance_columns() > CMG_MAX_NUISANCE_COLUMNS {
            return Err(unsupported(format!(
                "fused PPML CMG supports at most {CMG_MAX_NUISANCE_COLUMNS} non-base nuisance columns; found {}",
                profile.nuisance_columns()
            )));
        }
        let fused_workspace_bytes = estimate_fused_cmg_workspace_bytes(
            rows,
            active_generated.len(),
            config.threads.resolved(),
        )?;
        let topology_prepare_peak =
            PreparedCmgTopology::conservative_prepare_peak_bytes(&borrowed)?;
        let preparation_peak = config
            .solver_memory_budget
            .execution_bytes
            .checked_add(fused_workspace_bytes)
            .and_then(|value| value.checked_add(topology_prepare_peak))
            .ok_or_else(|| allocation("fused PPML CMG preparation memory overflows"))?;
        if preparation_peak > config.solver_memory_budget.usable_bytes {
            return Err(allocation(format!(
                "fused PPML CMG workspaces and prepared-topology construction require {} bytes beyond the selected PPML execution plan",
                fused_workspace_bytes + topology_prepare_peak
            )));
        }
        let prepared_topology = PreparedCmgTopology::prepare(&borrowed)?;
        let adjusted_execution = config
            .solver_memory_budget
            .execution_bytes
            .checked_add(fused_workspace_bytes)
            .and_then(|value| value.checked_add(prepared_topology.retained_bytes() as u128))
            .ok_or_else(|| allocation("fused PPML CMG execution memory overflows"))?;
        if adjusted_execution > config.solver_memory_budget.usable_bytes {
            return Err(allocation(format!(
                "fused PPML CMG workspaces and prepared topology require {} bytes beyond the selected PPML execution plan",
                fused_workspace_bytes + prepared_topology.retained_bytes() as u128
            )));
        }
        let config = FusedFwlCmgConfig {
            solver_memory_budget: FeSolverMemoryBudget {
                execution_bytes: adjusted_execution,
                usable_bytes: config.solver_memory_budget.usable_bytes,
            },
            ..config
        };

        let active_columns = active_generated.len();
        let right_hand_sides = active_columns
            .checked_add(1)
            .ok_or_else(|| allocation("fused PPML CMG RHS width overflows"))?;
        let active_elements = rows
            .checked_mul(active_columns)
            .ok_or_else(|| allocation("fused PPML CMG active-control dimensions overflow"))?;
        let square = active_columns
            .checked_mul(active_columns)
            .ok_or_else(|| allocation("fused PPML CMG finite dimensions overflow"))?;
        let moment_stride = square
            .checked_add(active_columns)
            .ok_or_else(|| allocation("fused PPML CMG moment dimensions overflow"))?;
        let moment_blocks = rows.div_ceil(MOMENT_BLOCK_ROWS);
        let moment_elements = moment_blocks
            .checked_mul(moment_stride)
            .ok_or_else(|| allocation("fused PPML CMG moment workspace overflows"))?;

        let mut active_values = fallible_filled(
            active_elements,
            0.0,
            "unable to reserve fused PPML CMG active controls",
        )?;
        let mut active_slots = Vec::new();
        active_slots
            .try_reserve_exact(active_columns)
            .map_err(|_| allocation("unable to reserve fused PPML CMG coefficient slots"))?;
        for (column, &generated) in active_generated.iter().enumerate() {
            for row in 0..rows {
                let value = prepared.fit_column_value(generated, row)?;
                if !value.is_finite() {
                    return Err(invalid_data(
                        "fused PPML CMG active control contains a nonfinite value",
                    ));
                }
                active_values[row * active_columns + column] = value;
            }
            let slot = prepared
                .fit_model()
                .coefficients
                .iter()
                .position(|coefficient| coefficient.generated_column_index == Some(generated))
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "fused PPML CMG active column has no coefficient slot",
                    )
                })?;
            active_slots.push(slot);
        }
        let team = ThreadTeam::new(config.threads).acquire()?;
        Ok(Self {
            rows,
            active_columns,
            prepared_topology,
            active_values,
            active_slots,
            projected: DenseMatrix::zeros(rows, right_hand_sides)?,
            retry: DenseMatrix::zeros(rows, right_hand_sides)?,
            projection_certificates: fallible_filled(
                right_hand_sides,
                ProjectionColumnCertificate {
                    absolute_infinity_norm: f64::INFINITY,
                    relative_infinity_norm: f64::INFINITY,
                    relative_two_norm: f64::INFINITY,
                    passed: false,
                },
                "unable to reserve fused PPML CMG projection certificates",
            )?,
            failed_right_hand_sides: fallible_empty(
                right_hand_sides,
                "unable to reserve fused PPML CMG failed RHS list",
            )?,
            gram: fallible_filled(square, 0.0, "fused PPML CMG finite Gram")?,
            rhs: fallible_filled(active_columns, 0.0, "fused PPML CMG finite RHS")?,
            factor: fallible_filled(square, 0.0, "fused PPML CMG finite factor")?,
            forward: fallible_filled(active_columns, 0.0, "fused PPML CMG finite forward solve")?,
            standardized_solution: fallible_filled(
                active_columns,
                0.0,
                "fused PPML CMG standardized coefficients",
            )?,
            scales: fallible_filled(active_columns, 0.0, "fused PPML CMG finite scales")?,
            coefficients: fallible_filled(
                active_columns,
                0.0,
                "fused PPML CMG finite coefficients",
            )?,
            permutation: fallible_filled(active_columns, 0, "fused PPML CMG finite permutation")?,
            moment_partials: fallible_filled(
                moment_elements,
                0.0,
                "fused PPML CMG finite moment partials",
            )?,
            residual: fallible_filled(rows, 0.0, "fused PPML CMG residual")?,
            fitted_without_offset: fallible_filled(rows, 0.0, "fused PPML CMG fitted predictor")?,
            team,
            config,
        })
    }

    pub(crate) fn solve<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        prepared: &P,
        fe_design: &FeDesign,
        working_response: &[f64],
        weights: &[f64],
        tolerance: f64,
    ) -> Result<FusedFwlCmgSolveOutcome> {
        self.validate_frame(fe_design, working_response, weights, tolerance)?;
        let attempt_started = Instant::now();
        let mut diagnostics = FusedFwlCmgAttemptDiagnostics::default();
        match self.solve_certified(
            prepared,
            fe_design,
            working_response,
            weights,
            tolerance,
            &mut diagnostics,
        ) {
            Ok(report) => Ok(FusedFwlCmgSolveOutcome::Certified(report)),
            Err(error) if error.code == ErrorCode::NonConvergence => {
                Ok(FusedFwlCmgSolveOutcome::NumericalFailure(
                    diagnostics.into_failure_report(attempt_started.elapsed().as_secs_f64()),
                ))
            }
            Err(error) => Err(error),
        }
    }

    fn solve_certified<P: PreparedOlsFitView + Sync + ?Sized>(
        &mut self,
        prepared: &P,
        fe_design: &FeDesign,
        working_response: &[f64],
        weights: &[f64],
        tolerance: f64,
        diagnostics: &mut FusedFwlCmgAttemptDiagnostics,
    ) -> Result<FusedFwlCmgReport> {
        for certificate in &mut self.projection_certificates {
            *certificate = ProjectionColumnCertificate {
                absolute_infinity_norm: f64::INFINITY,
                relative_infinity_norm: f64::INFINITY,
                relative_two_norm: f64::INFINITY,
                passed: false,
            };
        }
        #[cfg(test)]
        {
            if FORCE_NUMERICAL_FAILURE_ONCE.replace(false) {
                return Err(nonconvergence("injected fused PPML CMG numerical failure"));
            }
        }
        self.fill_full_projection_input(working_response);
        let mut map = self.config.map;
        map.tolerance = tolerance;
        let cmg_config = self.cmg_config(map);
        let borrowed = prepared.fit_borrowed_cmg_design()?;
        let mut absorber = CmgAbsorber::build_unpolished_prepared(
            borrowed,
            BorrowedMapWeights::Provided(weights),
            cmg_config,
            &mut self.prepared_topology,
        )?;
        let first = absorber.project_unpolished(&mut self.projected, tolerance)?;
        diagnostics.observe_projection(first);
        let certification_started = Instant::now();
        let projection_certificate =
            self.certify_projected_columns(fe_design, working_response, weights, tolerance, false);
        diagnostics.certification_seconds += certification_started.elapsed().as_secs_f64();
        diagnostics.observe_projection_certificates(&self.projection_certificates);
        projection_certificate?;
        #[cfg(test)]
        if let Some(rhs) = FORCE_PROJECTION_CERTIFICATE_FAILURE_ONCE.replace(None) {
            let certificate = self.projection_certificates.get_mut(rhs).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "injected fused PPML CMG certificate RHS is out of bounds",
                )
            })?;
            certificate.passed = false;
        }
        self.failed_right_hand_sides.clear();
        for (rhs, certificate) in self.projection_certificates.iter().enumerate() {
            if !certificate.passed {
                self.failed_right_hand_sides.push(rhs);
            }
        }
        let failed = self.failed_right_hand_sides.len();
        #[cfg(test)]
        TOTAL_RETRIED_RIGHT_HAND_SIDES
            .set(TOTAL_RETRIED_RIGHT_HAND_SIDES.get().saturating_add(failed));
        let mut retry_report = None;
        let effective_tolerance = (tolerance * 0.01).max(1e-14);
        if failed > 0 {
            diagnostics.retries = diagnostics.retries.saturating_add(failed);
            self.fill_retry_projection_input(working_response)?;
            let report = if fe_design.effects().len() == 2 {
                absorber.project_unpolished(&mut self.retry, effective_tolerance)?
            } else {
                drop(absorber);
                let mut retry_map = self.config.map;
                retry_map.tolerance = effective_tolerance;
                let retry_borrowed = prepared.fit_borrowed_cmg_design()?;
                let retry_config = self.cmg_config(retry_map);
                let mut retry_absorber = CmgAbsorber::build_unpolished_prepared(
                    retry_borrowed,
                    BorrowedMapWeights::Provided(weights),
                    retry_config,
                    &mut self.prepared_topology,
                )?;
                retry_absorber.project_unpolished(&mut self.retry, effective_tolerance)?
            };
            diagnostics.observe_projection(report);
            let certification_started = Instant::now();
            let retry_certificate = self.certify_projected_columns(
                fe_design,
                working_response,
                weights,
                tolerance,
                true,
            );
            diagnostics.certification_seconds += certification_started.elapsed().as_secs_f64();
            diagnostics.observe_projection_certificates(&self.projection_certificates);
            retry_certificate?;
            for (local, &rhs) in self.failed_right_hand_sides.iter().enumerate() {
                if !self.projection_certificates[rhs].passed {
                    return Err(nonconvergence(
                        "fused PPML CMG projection did not pass its per-RHS FE certificate after one tighter retry",
                    ));
                }
                for row in 0..self.rows {
                    self.projected[(row, rhs)] = self.retry[(row, local)];
                }
            }
            retry_report = Some(report);
        }

        #[cfg(test)]
        if FORCE_POST_PROJECTION_NUMERICAL_FAILURE_ONCE.replace(false) {
            return Err(nonconvergence(
                "injected fused PPML CMG post-projection numerical failure",
            ));
        }

        let finish_started = Instant::now();
        if let Err(error) = self.solve_finite_system(weights, tolerance) {
            diagnostics.certification_seconds += finish_started.elapsed().as_secs_f64();
            return Err(error);
        }
        let active_columns = self.active_columns;
        let coefficients = &self.coefficients;
        let projected = &self.projected;
        self.team.install(|| {
            self.residual
                .par_iter_mut()
                .zip(self.fitted_without_offset.par_iter_mut())
                .enumerate()
                .for_each(|(row, (residual, fitted))| {
                    let mut value = projected[(row, active_columns)];
                    for column in 0..active_columns {
                        value -= projected[(row, column)] * coefficients[column];
                    }
                    *residual = value;
                    *fitted = working_response[row] - value;
                });
        });
        if self
            .residual
            .iter()
            .chain(&self.fitted_without_offset)
            .any(|value| !value.is_finite())
        {
            diagnostics.certification_seconds += finish_started.elapsed().as_secs_f64();
            return Err(nonconvergence(
                "fused PPML CMG produced a nonfinite residual or predictor",
            ));
        }
        let certificate = match self.certify_fwl(fe_design, working_response, weights, tolerance) {
            Ok(certificate) => certificate,
            Err(error) => {
                diagnostics.certification_seconds += finish_started.elapsed().as_secs_f64();
                return Err(error);
            }
        };
        diagnostics.observe_final_certificate(certificate);
        if !certificate.passed {
            diagnostics.certification_seconds += finish_started.elapsed().as_secs_f64();
            return Err(nonconvergence(
                "fused PPML CMG did not pass its exact combined normal-equation certificate",
            ));
        }
        let finish_seconds = finish_started.elapsed().as_secs_f64();
        let projection_absolute_infinity_norm = self
            .projection_certificates
            .iter()
            .map(|value| value.absolute_infinity_norm)
            .fold(0.0_f64, f64::max);
        let projection_relative_infinity_norm = self
            .projection_certificates
            .iter()
            .map(|value| value.relative_infinity_norm)
            .fold(0.0_f64, f64::max);
        let projection_relative_two_norm = self
            .projection_certificates
            .iter()
            .map(|value| value.relative_two_norm)
            .fold(0.0_f64, f64::max);
        let (iterations, setup_seconds, solve_seconds, backward_error, projected_rhs, batches) =
            combine_reports(first, retry_report);
        Ok(FusedFwlCmgReport {
            iterations,
            effective_tolerance: if failed == 0 {
                tolerance
            } else {
                effective_tolerance
            },
            setup_seconds,
            solve_seconds,
            finish_seconds,
            backward_error,
            graph_right_hand_sides: projected_rhs,
            graph_batches: batches,
            nuisance_columns: first.nuisance_columns,
            failed_right_hand_sides: failed,
            retried_right_hand_sides: failed,
            continued_after_certificate: failed > 0,
            projection_absolute_infinity_norm,
            projection_relative_infinity_norm,
            projection_relative_two_norm,
            certificate,
        })
    }

    pub(crate) fn fill_candidate(
        &self,
        workspace: &mut PpmlIterationWorkspace,
        offset: &[f64],
    ) -> Result<()> {
        workspace.fill_absolute_candidate(
            &self.fitted_without_offset,
            offset,
            &self.active_slots,
            &self.coefficients,
        )
    }

    pub(crate) fn active_rank(&self) -> usize {
        self.active_slots.len()
    }

    pub(crate) fn active_slots(&self) -> &[usize] {
        &self.active_slots
    }

    fn cmg_config(&self, map: MapConfig) -> CmgConfig {
        CmgConfig {
            map,
            threads: self.config.threads,
            batch_size: self.config.batch_size,
            retain_coefficients: false,
            requested_solver: FeSolverKind::Cmg,
            solver_memory_budget: Some(self.config.solver_memory_budget),
        }
    }

    fn validate_frame(
        &self,
        fe_design: &FeDesign,
        working_response: &[f64],
        weights: &[f64],
        tolerance: f64,
    ) -> Result<()> {
        if fe_design.nobs() != self.rows
            || working_response.len() != self.rows
            || weights.len() != self.rows
            || !tolerance.is_finite()
            || tolerance <= 0.0
        {
            return Err(invalid_argument(
                "fused PPML CMG frame dimensions or tolerance are invalid",
            ));
        }
        if working_response.iter().any(|value| !value.is_finite())
            || weights
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(invalid_data(
                "fused PPML CMG frame contains a nonfinite response or invalid weight",
            ));
        }
        Ok(())
    }

    fn fill_full_projection_input(&mut self, working_response: &[f64]) {
        let columns = self.active_columns;
        let width = columns + 1;
        let active = &self.active_values;
        self.team.install(|| {
            self.projected
                .as_mut_slice()
                .par_chunks_mut(width)
                .enumerate()
                .for_each(|(row, output)| {
                    output[..columns].copy_from_slice(&active[row * columns..(row + 1) * columns]);
                    output[columns] = working_response[row];
                });
        });
    }

    fn fill_retry_projection_input(&mut self, working_response: &[f64]) -> Result<()> {
        let failed = &self.failed_right_hand_sides;
        self.retry
            .resize_zeroed_within_capacity(self.rows, failed.len())?;
        let active_columns = self.active_columns;
        let active = &self.active_values;
        self.team.install(|| {
            self.retry
                .as_mut_slice()
                .par_chunks_mut(failed.len())
                .enumerate()
                .for_each(|(row, output)| {
                    for (local, &rhs) in failed.iter().enumerate() {
                        output[local] = if rhs == active_columns {
                            working_response[row]
                        } else {
                            active[row * active_columns + rhs]
                        };
                    }
                });
        });
        Ok(())
    }

    fn certify_projected_columns(
        &mut self,
        design: &FeDesign,
        working_response: &[f64],
        weights: &[f64],
        tolerance: f64,
        retry_selected: bool,
    ) -> Result<()> {
        let active_columns = self.active_columns;
        let active = &self.active_values;
        let projected = if retry_selected {
            &self.retry
        } else {
            &self.projected
        };
        let targets = &self.failed_right_hand_sides;
        let certificates = &mut self.projection_certificates;
        let calculate = |local: usize, rhs: usize| {
            projection_column_certificate(
                design,
                weights,
                tolerance,
                |row| {
                    if rhs == active_columns {
                        working_response[row]
                    } else {
                        active[row * active_columns + rhs]
                    }
                },
                |row| projected[(row, local)],
            )
        };
        if retry_selected {
            // Retry is exceptional and normally narrow. Updating the retained
            // certificate array serially avoids a fresh result allocation.
            for (local, &rhs) in targets.iter().enumerate() {
                certificates[rhs] = calculate(local, rhs)?;
            }
        } else {
            self.team.install(|| {
                certificates
                    .par_iter_mut()
                    .enumerate()
                    .try_for_each(|(rhs, output)| {
                        *output = calculate(rhs, rhs)?;
                        Ok(())
                    })
            })?;
        }
        Ok(())
    }

    fn solve_finite_system(&mut self, weights: &[f64], tolerance: f64) -> Result<()> {
        let columns = self.active_columns;
        if columns == 0 {
            return Ok(());
        }
        let square = columns * columns;
        let stride = square + columns;
        self.moment_partials.fill(0.0);
        let projected = &self.projected;
        self.team.install(|| {
            self.moment_partials
                .par_chunks_mut(stride)
                .enumerate()
                .for_each(|(block, partial)| {
                    let start = block * MOMENT_BLOCK_ROWS;
                    let end = (start + MOMENT_BLOCK_ROWS).min(weights.len());
                    for row in start..end {
                        let weight = weights[row];
                        let response = projected[(row, columns)];
                        for left in 0..columns {
                            let left_value = projected[(row, left)];
                            partial[square + left] =
                                (weight * left_value).mul_add(response, partial[square + left]);
                            for right in left..columns {
                                partial[left * columns + right] = (weight * left_value).mul_add(
                                    projected[(row, right)],
                                    partial[left * columns + right],
                                );
                            }
                        }
                    }
                });
        });
        self.gram.fill(0.0);
        self.rhs.fill(0.0);
        self.factor.fill(0.0);
        self.forward.fill(0.0);
        for partial in self.moment_partials.chunks(stride) {
            for left in 0..columns {
                add_compensated_scalar(
                    &mut self.rhs[left],
                    &mut self.forward[left],
                    partial[square + left],
                )?;
                for right in left..columns {
                    let slot = left * columns + right;
                    add_compensated_scalar(
                        &mut self.gram[slot],
                        &mut self.factor[slot],
                        partial[slot],
                    )?;
                }
            }
        }
        for left in 0..columns {
            self.rhs[left] += self.forward[left];
            if !self.rhs[left].is_finite() {
                return Err(nonconvergence(
                    "fused PPML CMG finite RHS accumulation is nonfinite",
                ));
            }
            for right in left..columns {
                let slot = left * columns + right;
                self.gram[slot] += self.factor[slot];
                if !self.gram[slot].is_finite() {
                    return Err(nonconvergence(
                        "fused PPML CMG finite Gram accumulation is nonfinite",
                    ));
                }
            }
            for right in (left + 1)..columns {
                self.gram[right * columns + left] = self.gram[left * columns + right];
            }
            self.scales[left] = self.gram[left * columns + left].sqrt();
            if !self.scales[left].is_finite() || self.scales[left] <= 0.0 {
                return Err(nonconvergence(
                    "fused PPML CMG active finite basis lost positive weighted norm",
                ));
            }
        }
        for row in 0..columns {
            self.rhs[row] /= self.scales[row];
            for column in 0..columns {
                self.gram[row * columns + column] /= self.scales[row] * self.scales[column];
            }
        }
        pivoted_cholesky_solve(
            &mut self.gram,
            &mut self.rhs,
            &mut self.factor,
            &mut self.forward,
            &mut self.standardized_solution,
            &mut self.permutation,
            tolerance.max(64.0 * f64::EPSILON).powi(2),
        )?;
        self.coefficients.fill(0.0);
        for position in 0..columns {
            let original = self.permutation[position];
            self.coefficients[original] =
                self.standardized_solution[position] / self.scales[original];
        }
        if self.coefficients.iter().any(|value| !value.is_finite()) {
            return Err(nonconvergence(
                "fused PPML CMG finite coefficients are nonfinite",
            ));
        }
        Ok(())
    }

    fn certify_fwl(
        &self,
        design: &FeDesign,
        target: &[f64],
        weights: &[f64],
        tolerance: f64,
    ) -> Result<JointLsmrCertificate> {
        let reference =
            weighted_norm(target, weights)?.max(weighted_norm(&self.residual, weights)?);
        let mut finite_absolute = 0.0_f64;
        let mut fe_absolute = 0.0_f64;
        let mut relative_infinity = 0.0_f64;
        let mut relative_square = 0.0_f64;
        let mut count = 0_usize;
        for column in 0..self.active_columns {
            let mut moment = CompensatedSum::default();
            let mut basis_square = CompensatedSum::default();
            for (row, &weight) in weights.iter().enumerate().take(self.rows) {
                let value = self.active_values[row * self.active_columns + column];
                moment.add(weight * self.residual[row] * value)?;
                basis_square.add(weight * value * value)?;
            }
            let moment = moment.total();
            finite_absolute = finite_absolute.max(moment.abs());
            add_relative(
                moment,
                basis_square.total().max(0.0).sqrt(),
                reference,
                &mut relative_infinity,
                &mut relative_square,
                &mut count,
            );
        }
        for effect in design.effects() {
            for level in 0..effect.n_levels() {
                let mut moment = CompensatedSum::default();
                let mut basis_square = CompensatedSum::default();
                for &row in effect.level_index().members(level) {
                    moment.add(weights[row] * self.residual[row])?;
                    basis_square.add(weights[row])?;
                }
                let moment = moment.total();
                fe_absolute = fe_absolute.max(moment.abs());
                add_relative(
                    moment,
                    basis_square.total().max(0.0).sqrt(),
                    reference,
                    &mut relative_infinity,
                    &mut relative_square,
                    &mut count,
                );
            }
        }
        let relative_two = if count == 0 {
            0.0
        } else {
            (relative_square / count as f64).sqrt()
        };
        if !finite_absolute.is_finite()
            || !fe_absolute.is_finite()
            || !relative_infinity.is_finite()
            || !relative_two.is_finite()
        {
            return Err(nonconvergence(
                "fused PPML CMG exact certificate is nonfinite",
            ));
        }
        Ok(JointLsmrCertificate {
            finite_absolute_infinity_norm: finite_absolute,
            fixed_effect_absolute_infinity_norm: fe_absolute,
            relative_infinity_norm: relative_infinity,
            relative_two_norm: relative_two,
            passed: relative_infinity <= tolerance && relative_two <= tolerance,
        })
    }
}

pub(crate) fn estimate_fused_cmg_workspace_bytes(
    rows: usize,
    active_columns: usize,
    threads: usize,
) -> Result<u128> {
    if rows == 0 || threads == 0 {
        return Err(invalid_argument(
            "fused PPML CMG memory estimate requires positive rows and threads",
        ));
    }
    let n = rows as u128;
    let k = active_columns as u128;
    let rhs = k
        .checked_add(1)
        .ok_or_else(|| allocation("fused PPML CMG memory RHS width overflows"))?;
    let observation_values = n
        .checked_mul(
            k.checked_add(
                rhs.checked_mul(2)
                    .ok_or_else(|| allocation("fused PPML CMG projected buffer count overflows"))?,
            )
            .and_then(|value| value.checked_add(2))
            .ok_or_else(|| allocation("fused PPML CMG observation buffer count overflows"))?,
        )
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(|| allocation("fused PPML CMG observation bytes overflow"))?;
    let square = k
        .checked_mul(k)
        .ok_or_else(|| allocation("fused PPML CMG square workspace overflows"))?;
    let float_workspace = square
        .checked_mul(2)
        .and_then(|value| value.checked_add(k.checked_mul(5)?))
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>() as u128))
        .ok_or_else(|| allocation("fused PPML CMG float workspace bytes overflow"))?;
    let index_workspace = k
        .checked_mul(2)
        .and_then(|value| value.checked_add(rhs))
        .and_then(|value| value.checked_mul(std::mem::size_of::<usize>() as u128))
        .ok_or_else(|| allocation("fused PPML CMG index workspace bytes overflow"))?;
    let certificate_workspace = rhs
        .checked_mul(std::mem::size_of::<ProjectionColumnCertificate>() as u128)
        .ok_or_else(|| allocation("fused PPML CMG certificate bytes overflow"))?;
    let small_values = float_workspace
        .checked_add(index_workspace)
        .and_then(|value| value.checked_add(certificate_workspace))
        .ok_or_else(|| allocation("fused PPML CMG small workspace bytes overflow"))?;
    let blocks = rows.div_ceil(MOMENT_BLOCK_ROWS) as u128;
    let partials = blocks
        .checked_mul(
            square
                .checked_add(k)
                .ok_or_else(|| allocation("fused PPML CMG moment stride overflows"))?,
        )
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(|| allocation("fused PPML CMG moment partial bytes overflow"))?;
    observation_values
        .checked_add(small_values)
        .and_then(|value| value.checked_add(partials))
        .and_then(|value| value.checked_add((threads as u128).checked_mul(256)?))
        .ok_or_else(|| allocation("fused PPML CMG total workspace bytes overflow"))
}

fn projection_column_certificate(
    design: &FeDesign,
    weights: &[f64],
    tolerance: f64,
    target: impl Fn(usize) -> f64,
    residual: impl Fn(usize) -> f64,
) -> Result<ProjectionColumnCertificate> {
    let mut target_scale = ScaledSquareSum::default();
    let mut residual_scale = ScaledSquareSum::default();
    for (row, &weight) in weights.iter().enumerate().take(design.nobs()) {
        let root_weight = weight.sqrt();
        target_scale.add(root_weight * target(row))?;
        residual_scale.add(root_weight * residual(row))?;
    }
    let reference = target_scale.norm().max(residual_scale.norm());
    let mut absolute = 0.0_f64;
    let mut relative_infinity = 0.0_f64;
    let mut relative_square = 0.0_f64;
    let mut count = 0_usize;
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let mut moment = CompensatedSum::default();
            let mut basis_square = CompensatedSum::default();
            for &row in effect.level_index().members(level) {
                moment.add(weights[row] * residual(row))?;
                basis_square.add(weights[row])?;
            }
            let moment = moment.total();
            absolute = absolute.max(moment.abs());
            add_relative(
                moment,
                basis_square.total().max(0.0).sqrt(),
                reference,
                &mut relative_infinity,
                &mut relative_square,
                &mut count,
            );
        }
    }
    let relative_two = if count == 0 {
        0.0
    } else {
        (relative_square / count as f64).sqrt()
    };
    if !absolute.is_finite() || !relative_infinity.is_finite() || !relative_two.is_finite() {
        return Err(nonconvergence(
            "fused PPML CMG projection certificate is nonfinite",
        ));
    }
    Ok(ProjectionColumnCertificate {
        absolute_infinity_norm: absolute,
        relative_infinity_norm: relative_infinity,
        relative_two_norm: relative_two,
        passed: relative_infinity <= tolerance && relative_two <= tolerance,
    })
}

fn combine_reports(
    first: CmgUnpolishedProjectionReport,
    retry: Option<CmgUnpolishedProjectionReport>,
) -> (usize, f64, f64, f64, usize, usize) {
    retry.map_or(
        (
            first.iterations,
            first.setup_seconds,
            first.solve_seconds,
            first.backward_error,
            first.right_hand_sides,
            first.batches,
        ),
        |retry| {
            (
                first.iterations.saturating_add(retry.iterations),
                first.setup_seconds + retry.setup_seconds,
                first.solve_seconds + retry.solve_seconds,
                first.backward_error.max(retry.backward_error),
                first
                    .right_hand_sides
                    .saturating_add(retry.right_hand_sides),
                first.batches.saturating_add(retry.batches),
            )
        },
    )
}

fn update_optional_max(target: &mut Option<f64>, value: f64) {
    if value.is_finite() && value >= 0.0 {
        *target = Some(target.as_ref().map_or(value, |current| current.max(value)));
    }
}

fn weighted_norm(values: &[f64], weights: &[f64]) -> Result<f64> {
    let mut sum = ScaledSquareSum::default();
    for (&value, &weight) in values.iter().zip(weights) {
        sum.add(weight.sqrt() * value)?;
    }
    Ok(sum.norm())
}

fn add_relative(
    moment: f64,
    basis_norm: f64,
    reference: f64,
    infinity: &mut f64,
    square: &mut f64,
    count: &mut usize,
) {
    let denominator = basis_norm * reference;
    let relative = if denominator > 0.0 {
        moment.abs() / denominator
    } else if moment == 0.0 {
        0.0
    } else {
        f64::INFINITY
    };
    *infinity = (*infinity).max(relative);
    *square = relative.mul_add(relative, *square);
    *count = count.saturating_add(1);
}

fn add_compensated_scalar(sum: &mut f64, correction: &mut f64, value: f64) -> Result<()> {
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
        Err(nonconvergence(
            "fused PPML CMG finite moment accumulation is nonfinite",
        ))
    }
}

#[derive(Default)]
struct CompensatedSum {
    sum: f64,
    correction: f64,
}

impl CompensatedSum {
    fn add(&mut self, value: f64) -> Result<()> {
        let updated = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - updated) + value
        } else {
            (value - updated) + self.sum
        };
        self.sum = updated;
        if self.sum.is_finite() && self.correction.is_finite() {
            Ok(())
        } else {
            Err(nonconvergence(
                "fused PPML CMG compensated accumulation is nonfinite",
            ))
        }
    }

    fn total(&self) -> f64 {
        self.sum + self.correction
    }
}

#[derive(Default)]
struct ScaledSquareSum {
    scale: f64,
    square_sum: f64,
}

impl ScaledSquareSum {
    fn add(&mut self, value: f64) -> Result<()> {
        if !value.is_finite() {
            return Err(nonconvergence("fused PPML CMG weighted norm is nonfinite"));
        }
        let absolute = value.abs();
        if absolute == 0.0 {
            return Ok(());
        }
        if self.scale < absolute {
            let ratio = if self.scale == 0.0 {
                0.0
            } else {
                self.scale / absolute
            };
            self.square_sum = 1.0 + self.square_sum * ratio * ratio;
            self.scale = absolute;
        } else {
            let ratio = absolute / self.scale;
            self.square_sum += ratio * ratio;
        }
        Ok(())
    }

    fn norm(&self) -> f64 {
        if self.scale == 0.0 {
            0.0
        } else {
            self.scale * self.square_sum.sqrt()
        }
    }
}

fn fallible_filled<T: Clone>(elements: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(message))?;
    output.resize(elements, value);
    Ok(output)
}

fn fallible_empty<T>(elements: usize, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(message))?;
    Ok(output)
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

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn nonconvergence(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fused_workspace_estimate_is_monotone() {
        let narrow = estimate_fused_cmg_workspace_bytes(100, 2, 1).expect("narrow estimate");
        let wide = estimate_fused_cmg_workspace_bytes(100, 8, 1).expect("wide estimate");
        let long = estimate_fused_cmg_workspace_bytes(1_000, 2, 1).expect("long estimate");
        assert!(wide > narrow);
        assert!(long > narrow);
    }
}
