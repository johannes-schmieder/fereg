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
    retained_effect_order_by_levels, BorrowedInterceptFeDesign, BorrowedMapWeights, CmgAbsorber,
    CmgConfig, CmgControlSeed, CmgTopologyProfile, CmgUnpolishedProjectionReport, FeDesign,
    FeSolverKind, MapConfig, PreparedCmgTopology, ValidatedBorrowedInterceptSource,
    CMG_MAX_NUISANCE_COLUMNS,
};
use crate::memory::{FeSolverMemoryBudget, SolverWorkspacePeak};
use crate::options::ThreadConfig;
use crate::parallel::{InstalledThreadTeam, ThreadTeam};
use crate::prepared_fit_view::PreparedOlsFitView;

use super::categorical_lsmr::{emit_solver_trace_line, JointLsmrCertificate};
use super::PpmlIterationWorkspace;

// Fixed partitions preserve reduction order across thread counts. The wider
// ordinary frames need more than two tasks at N=50,000 to use the admitted team.
// Cap partial K-by-K matrices so very large N cannot multiply retained storage.
const MOMENT_BLOCK_ROWS: usize = 4_096;
const MOMENT_MAX_BLOCKS: usize = 64;

fn moment_block_rows(rows: usize) -> usize {
    MOMENT_BLOCK_ROWS.max(rows.div_ceil(MOMENT_MAX_BLOCKS))
}

pub(crate) fn fused_cmg_batch_size(poolsize: usize, rhs: usize, nuisance: usize) -> usize {
    // Nuisance Schur columns are independent current-weight graph solves too.
    // Capping their batch at K+1 serializes setup on narrow finite designs.
    // Admission and execution must charge the same pool-bounded capacity.
    poolsize.min(rhs.max(nuisance)).max(1)
}

fn redundant_nuisance_effect(design: &BorrowedInterceptFeDesign<'_>) -> Result<Option<usize>> {
    if design.effects().len() != 3 {
        return Ok(None);
    }
    let order = retained_effect_order_by_levels(3, |effect| design.effects()[effect].n_levels())?;
    let nuisance = order[2];
    let codes = design.effects()[nuisance].codes();
    for &core in &order[..2] {
        let effect = &design.effects()[core];
        // Exact membership containment proves span equality for every positive
        // Hessian weight. It needs no numerical rank threshold or scratch map.
        if (0..effect.n_levels()).all(|level| {
            let members = effect.level_index().members(level);
            members
                .first()
                .is_some_and(|&first| members.iter().all(|&row| codes[row] == codes[first]))
        }) {
            return Ok(Some(nuisance));
        }
    }
    Ok(None)
}

fn projection_design(
    design: BorrowedInterceptFeDesign<'_>,
    redundant: Option<usize>,
) -> Result<BorrowedInterceptFeDesign<'_>> {
    let Some(redundant) = redundant else {
        return Ok(design);
    };
    let mut effects = fallible_empty(2, "unable to reserve PPML CMG core views")?;
    for (index, effect) in design.effects().iter().enumerate() {
        if index != redundant {
            effects.push(
                ValidatedBorrowedInterceptSource::new(
                    effect.codes(),
                    effect.n_levels(),
                    effect.level_index(),
                )?
                .effect(),
            );
        }
    }
    BorrowedInterceptFeDesign::new(design.nobs(), effects)
}

#[cfg(test)]
thread_local! {
    static FORCE_NUMERICAL_FAILURE_ONCE: Cell<bool> = const { Cell::new(false) };
    static FORCE_FATAL_FAILURE_ONCE: Cell<Option<ErrorCode>> = const { Cell::new(None) };
    static FORCE_POST_PROJECTION_NUMERICAL_FAILURE_ONCE: Cell<bool> = const { Cell::new(false) };
    static FORCE_PROJECTION_CERTIFICATE_FAILURE_ONCE: Cell<Option<usize>> = const { Cell::new(None) };
    static TOTAL_RETRIED_RIGHT_HAND_SIDES: Cell<usize> = const { Cell::new(0) };
    static ROUTE_OBSERVATION_PREPARATIONS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn inject_numerical_failure_once() {
    FORCE_NUMERICAL_FAILURE_ONCE.set(true);
}

#[cfg(test)]
pub(crate) fn inject_fatal_failure_once(code: ErrorCode) {
    assert_ne!(code, ErrorCode::NonConvergence);
    FORCE_FATAL_FAILURE_ONCE.set(Some(code));
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

#[cfg(test)]
pub(crate) fn take_route_observation_preparations() -> usize {
    ROUTE_OBSERVATION_PREPARATIONS.replace(0)
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
    expected_heap_peak: SolverWorkspacePeak,
    expected_parent_bytes: u128,
    rows: usize,
    active_columns: usize,
    certificate_topology: FusedCmgCertificateTopology,
    prepared_topology: PreparedCmgTopology,
    control_seed: CmgControlSeed,
    redundant_nuisance: Option<usize>,
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
    /// Ratio learned from the preceding independent projection certificate.
    /// This only targets the CMG recurrence; certification always uses the
    /// caller's requested tolerance.
    projection_tolerance_scale: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FusedCmgCertificateTopology {
    rows: usize,
    effect_count: usize,
    effects: [FusedCmgCertificateEffect; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FusedCmgCertificateEffect {
    codes_address: usize,
    codes_len: usize,
    levels: usize,
}

impl FusedCmgCertificateTopology {
    const EMPTY_EFFECT: FusedCmgCertificateEffect = FusedCmgCertificateEffect {
        codes_address: 0,
        codes_len: 0,
        levels: 0,
    };

    fn bind(design: &FeDesign) -> Result<Self> {
        if !(2..=3).contains(&design.effects().len()) {
            return Err(unsupported(
                "fused PPML CMG certificate topology requires two or three effects",
            ));
        }
        let mut effects = [Self::EMPTY_EFFECT; 3];
        for (slot, effect) in effects.iter_mut().zip(design.effects()) {
            if !effect.has_intercept() || !effect.slopes().is_empty() {
                return Err(unsupported(
                    "fused PPML CMG certificate topology requires categorical intercept-only effects",
                ));
            }
            let codes = effect.codes();
            *slot = FusedCmgCertificateEffect {
                codes_address: codes.as_ptr() as usize,
                codes_len: codes.len(),
                levels: effect.n_levels(),
            };
        }
        Ok(Self {
            rows: design.nobs(),
            effect_count: design.effects().len(),
            effects,
        })
    }

    fn matches(self, design: &FeDesign) -> bool {
        self.rows == design.nobs()
            && self.effect_count == design.effects().len()
            && self.effects[..self.effect_count]
                .iter()
                .zip(design.effects())
                .all(|(identity, effect)| {
                    let codes = effect.codes();
                    effect.has_intercept()
                        && effect.slopes().is_empty()
                        && identity.codes_address == codes.as_ptr() as usize
                        && identity.codes_len == codes.len()
                        && identity.levels == effect.n_levels()
                })
    }
}

impl ReusableCategoricalFwlCmg {
    /// The persistent prepared/fused parent also remains live during canonical
    /// checkpoint solves; hierarchy frames and checkpoints are sequential.
    pub(crate) fn expected_heap_peak(&self, checkpoint_bytes: u128) -> Result<SolverWorkspacePeak> {
        Ok(self.expected_heap_peak.max(
            SolverWorkspacePeak::from_bytes(checkpoint_bytes)?
                .with_retained(self.expected_parent_bytes)?,
        ))
    }

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
        let certificate_topology = FusedCmgCertificateTopology::bind(fe_design)?;
        let borrowed = prepared.fit_borrowed_cmg_design()?;
        let profile = CmgTopologyProfile::analyze(&borrowed)?;
        if profile.nuisance_columns() > CMG_MAX_NUISANCE_COLUMNS {
            return Err(unsupported(format!(
                "fused PPML CMG supports at most {CMG_MAX_NUISANCE_COLUMNS} non-base nuisance columns; found {}",
                profile.nuisance_columns()
            )));
        }
        // Keep admission on the full declared topology, including its original
        // nuisance envelope. Only the PPML projection recipe is reduced; the
        // canonical model and every independent FE certificate stay full width.
        let redundant_nuisance = redundant_nuisance_effect(&borrowed)?;
        let borrowed = projection_design(borrowed, redundant_nuisance)?;
        let fused_workspace_bytes = estimate_fused_cmg_workspace_bytes(
            rows,
            active_generated.len(),
            config.threads.resolved(),
        )?
        .checked_add(CmgControlSeed::estimated_bytes(
            profile.vertices(),
            active_generated.len(),
        )?)
        .ok_or_else(|| allocation("fused PPML CMG seed workspace overflows"))?;
        let topology_prepare_peak =
            PreparedCmgTopology::conservative_prepare_peak_bytes(&borrowed)?;
        let preparation_peak = config
            .solver_memory_budget
            .execution_bytes
            .checked_add(topology_prepare_peak)
            .ok_or_else(|| allocation("fused PPML CMG preparation memory overflows"))?;
        if preparation_peak > config.solver_memory_budget.usable_bytes {
            return Err(allocation(format!(
                "fused PPML CMG prepared-topology construction requires {topology_prepare_peak} bytes beyond the selected PPML execution plan"
            )));
        }
        #[cfg(test)]
        ROUTE_OBSERVATION_PREPARATIONS.set(ROUTE_OBSERVATION_PREPARATIONS.get().saturating_add(1));
        let prepared_topology = PreparedCmgTopology::prepare(&borrowed)?;
        let prepared_retained = prepared_topology.retained_bytes()? as u128;
        let adjusted_execution = config
            .solver_memory_budget
            .execution_bytes
            .checked_add(fused_workspace_bytes)
            .and_then(|value| value.checked_add(prepared_retained))
            .ok_or_else(|| allocation("fused PPML CMG execution memory overflows"))?;
        if adjusted_execution > config.solver_memory_budget.usable_bytes {
            return Err(allocation(format!(
                "fused PPML CMG workspaces and prepared topology require {} bytes beyond the selected PPML execution plan",
                fused_workspace_bytes + prepared_retained
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
        let moment_blocks = rows.div_ceil(moment_block_rows(rows));
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
            expected_heap_peak: SolverWorkspacePeak::from_bytes(
                topology_prepare_peak.max(fused_workspace_bytes + prepared_retained),
            )?,
            expected_parent_bytes: fused_workspace_bytes + prepared_retained,
            rows,
            active_columns,
            certificate_topology,
            prepared_topology,
            control_seed: CmgControlSeed::new(profile.vertices(), active_columns)?,
            redundant_nuisance,
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
            projection_tolerance_scale: 1.0,
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
        if let Err(error) = self.validate_frame(fe_design, working_response, weights, tolerance) {
            self.reset_warm_start();
            return Err(error);
        }
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
                self.reset_warm_start();
                Ok(FusedFwlCmgSolveOutcome::NumericalFailure(
                    diagnostics.into_failure_report(attempt_started.elapsed().as_secs_f64()),
                ))
            }
            Err(error) => {
                self.reset_warm_start();
                Err(error)
            }
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
        let frame_started = Instant::now();
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
            if let Some(code) = FORCE_FATAL_FAILURE_ONCE.replace(None) {
                return Err(FeregError::new(
                    code,
                    "injected fused PPML CMG fatal failure",
                ));
            }
            if FORCE_NUMERICAL_FAILURE_ONCE.replace(false) {
                return Err(nonconvergence("injected fused PPML CMG numerical failure"));
            }
        }
        self.fill_full_projection_input(working_response);
        let first_tolerance = (tolerance * self.projection_tolerance_scale).max(1e-14);
        let mut map = self.config.map;
        map.tolerance = first_tolerance;
        let cmg_config = self.cmg_config(map);
        let borrowed =
            projection_design(prepared.fit_borrowed_cmg_design()?, self.redundant_nuisance)?;
        let attempt = CmgAbsorber::build_unpolished_prepared(
            borrowed,
            BorrowedMapWeights::Provided(weights),
            cmg_config,
            &mut self.prepared_topology,
        );
        if attempt.is_err() {
            self.expected_heap_peak = SolverWorkspacePeak::UNMODELED;
        }
        let mut absorber = attempt?;
        self.expected_heap_peak = self.expected_heap_peak.max(
            absorber
                .expected_heap_peak()
                .with_retained(self.expected_parent_bytes)?,
        );
        let first = absorber.project_unpolished_with_seed(
            &mut self.projected,
            first_tolerance,
            (self.active_columns > 0).then_some(&mut self.control_seed),
        )?;
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
        let retry_tolerance = (first_tolerance * 0.01).max(1e-14);
        if failed > 0 {
            // The independent per-RHS certificate has demonstrated that the
            // recurrence's stopping scale was insufficient on this changing-
            // weight topology. Reuse that information on the next frame so
            // the routine does not systematically solve every RHS twice.
            self.projection_tolerance_scale = (self.projection_tolerance_scale * 0.01).max(1e-8);
            diagnostics.retries = diagnostics.retries.saturating_add(failed);
            self.fill_retry_projection_input(working_response)?;
            let report = if first.nuisance_columns == 0 {
                absorber.project_unpolished(&mut self.retry, retry_tolerance)?
            } else {
                drop(absorber);
                let mut retry_map = self.config.map;
                retry_map.tolerance = retry_tolerance;
                let retry_borrowed = projection_design(
                    prepared.fit_borrowed_cmg_design()?,
                    self.redundant_nuisance,
                )?;
                let retry_config = self.cmg_config(retry_map);
                self.reset_warm_start();
                let attempt = CmgAbsorber::build_unpolished_prepared(
                    retry_borrowed,
                    BorrowedMapWeights::Provided(weights),
                    retry_config,
                    &mut self.prepared_topology,
                );
                if attempt.is_err() {
                    self.expected_heap_peak = SolverWorkspacePeak::UNMODELED;
                }
                absorber = attempt?;
                self.expected_heap_peak = self.expected_heap_peak.max(
                    absorber
                        .expected_heap_peak()
                        .with_retained(self.expected_parent_bytes)?,
                );
                absorber.project_unpolished(&mut self.retry, retry_tolerance)?
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
        let finite_seconds = finish_started.elapsed().as_secs_f64();
        let reconstruction_started = Instant::now();
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
        let reconstruction_seconds = reconstruction_started.elapsed().as_secs_f64();
        let joint_certificate_started = Instant::now();
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
        let joint_certificate_seconds = joint_certificate_started.elapsed().as_secs_f64();
        let finish_seconds =
            diagnostics.certification_seconds + finish_started.elapsed().as_secs_f64();
        if failed == 0 {
            self.control_seed.retain_certified(first.projection_work);
            self.prepared_topology.retain_certified_seed(absorber);
        } else {
            self.reset_warm_start();
        }
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
        // Per-RHS certificates also run on successful frames. Charge them
        // alongside final certification, and retain fereg-owned preparation
        // outside the lower-level graph report in setup rather than losing it.
        let setup_seconds = setup_seconds
            + (frame_started.elapsed().as_secs_f64()
                - setup_seconds
                - solve_seconds
                - finish_seconds)
                .max(0.0);
        if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
            for (attempt, work) in std::iter::once(("first", first.projection_work))
                .chain(retry_report.map(|report| ("retry", report.projection_work)))
            {
                emit_solver_trace_line(&format!(
                    "FEREG_SOLVER_TRACE|phase=cmg_projection|attempt={attempt}|warm_rhs={}|warm_iterations_sum={}|iterations_sum={}|restarts={}|extra_restarts={}|initial_relative_residual={:.17e}|pcg_seconds={:.17e}|rhs_seconds={:.17e}",
                    work.warm_right_hand_sides, work.warm_iterations, work.iterations, work.restarts, work.extra_restarts,
                    work.initial_relative_residual, work.pcg_seconds, work.rhs_seconds,
                ));
            }
            for (attempt, setup) in std::iter::once(("first", first.nuisance_setup))
                .chain(retry_report.map(|report| ("retry", report.nuisance_setup)))
            {
                emit_solver_trace_line(&format!(
                    "FEREG_SOLVER_TRACE|phase=cmg_nuisance|attempt={attempt}|warm_rhs={}|iterations_sum={}|restarts={}|extra_restarts={}|initial_relative_residual={:.17e}|weight_ratio={:.17e}|pcg_seconds={:.17e}|rhs_seconds={:.17e}|schur_seconds={:.17e}",
                    setup.warm_right_hand_sides, setup.iterations, setup.restarts, setup.extra_restarts,
                    setup.initial_relative_residual, setup.weight_ratio,
                    setup.pcg_seconds, setup.rhs_seconds, setup.schur_seconds,
                ));
            }
            emit_solver_trace_line(&format!(
                "FEREG_SOLVER_TRACE|phase=cmg_frame|rhs={}|nuisance={}|redundant_effect={}|batch_capacity={}|pcg_batch_width={}|scheduled={:.17e}|first_tolerance={:.17e}|effective={:.17e}|first_iterations={}|continuation_iterations={}|retried_rhs={}|graph_rhs={}|graph_batches={}|nuisance_reused_rhs={}|setup_seconds={:.17e}|solve_seconds={:.17e}|projection_certificate_seconds={:.17e}|finite_seconds={:.17e}|reconstruction_seconds={:.17e}|joint_certificate_seconds={:.17e}",
                self.active_columns + 1, first.nuisance_columns,
                self.redundant_nuisance.map_or(0, |effect| effect + 1),
                self.config.batch_size, first.pcg_batch_width, tolerance, first_tolerance,
                if failed == 0 { first_tolerance } else { retry_tolerance },
                first.iterations, retry_report.map_or(0, |report| report.iterations), failed,
                projected_rhs, batches, first.nuisance_reused_right_hand_sides
                    + retry_report.map_or(0, |report| report.nuisance_reused_right_hand_sides),
                setup_seconds, solve_seconds, diagnostics.certification_seconds, finite_seconds,
                reconstruction_seconds, joint_certificate_seconds,
            ));
        }
        Ok(FusedFwlCmgReport {
            iterations,
            effective_tolerance: if failed == 0 {
                first_tolerance
            } else {
                retry_tolerance
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

    pub(crate) fn reset_warm_start(&mut self) {
        self.control_seed.reset();
        self.prepared_topology.reset_warm_start();
    }

    #[cfg(test)]
    pub(crate) fn warm_seed_ready(&self) -> (bool, bool) {
        (
            self.prepared_topology.warm_seed_ready(),
            self.control_seed.ready(),
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
            components: crate::fe::explicit_cmg_components(FeSolverKind::Cmg),
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
        if !self.certificate_topology.matches(fe_design) {
            return Err(invalid_argument(
                "fused PPML CMG certificate topology changed during the reusable fit",
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
        let block_rows = moment_block_rows(weights.len());
        self.team.install(|| {
            self.moment_partials
                .par_chunks_mut(stride)
                .enumerate()
                .for_each(|(block, partial)| {
                    let start = block * block_rows;
                    let end = start.saturating_add(block_rows).min(weights.len());
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
        &mut self,
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
        // The finite solve has finished with these K-wide buffers. Reuse them
        // for fresh raw-design moments, not the projected Gram used by the
        // solver. Each direction retains its original compensated row order.
        self.team.install(|| {
            fill_finite_certificate_moments(
                &self.active_values,
                &self.residual,
                weights,
                &mut self.rhs,
                &mut self.scales,
            )
        })?;
        for column in 0..self.active_columns {
            let moment = self.rhs[column];
            finite_absolute = finite_absolute.max(moment.abs());
            add_relative(
                moment,
                self.scales[column].max(0.0).sqrt(),
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

fn fill_finite_certificate_moments(
    values: &[f64],
    residual: &[f64],
    weights: &[f64],
    moments: &mut [f64],
    basis_squares: &mut [f64],
) -> Result<()> {
    let columns = moments.len();
    if basis_squares.len() != columns
        || residual.len() != weights.len()
        || residual.len().checked_mul(columns) != Some(values.len())
    {
        return Err(invalid_argument("CMG finite certificate dimensions differ"));
    }
    moments
        .par_iter_mut()
        .zip(basis_squares.par_iter_mut())
        .enumerate()
        .try_for_each(|(column, (moment, basis_square))| -> Result<()> {
            let mut sum = CompensatedSum::default();
            let mut square = CompensatedSum::default();
            for (row, &weight) in weights.iter().enumerate() {
                let value = values[row * columns + column];
                sum.add(weight * residual[row] * value)?;
                square.add(weight * value * value)?;
            }
            *moment = sum.total();
            *basis_square = square.total();
            Ok(())
        })
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
        // The reduced recipe borrows codes/memberships; only two term views,
        // role indices, and the fit-owned redundant-effect marker are new.
        .and_then(|value| {
            value.checked_add(
                (2 * std::mem::size_of::<crate::fe::BorrowedInterceptEffect<'_>>()
                    + 3 * std::mem::size_of::<usize>()
                    + std::mem::size_of::<Option<usize>>()) as u128,
            )
        })
        .ok_or_else(|| allocation("fused PPML CMG small workspace bytes overflow"))?;
    let blocks = rows.div_ceil(moment_block_rows(rows)) as u128;
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

#[allow(clippy::too_many_arguments)]
fn pivoted_cholesky_solve(
    gram: &mut [f64],
    rhs: &mut [f64],
    factor: &mut [f64],
    forward: &mut [f64],
    solution: &mut [f64],
    permutation: &mut [usize],
    pivot_guard: f64,
) -> Result<()> {
    let dimension = rhs.len();
    if gram.len() != dimension.saturating_mul(dimension)
        || factor.len() != gram.len()
        || forward.len() != dimension
        || solution.len() != dimension
        || permutation.len() != dimension
    {
        return Err(invalid_argument(
            "fused PPML CMG finite workspace dimensions are inconsistent",
        ));
    }
    factor.fill(0.0);
    for (position, value) in permutation.iter_mut().enumerate() {
        *value = position;
    }
    for step in 0..dimension {
        let mut pivot = step;
        for candidate in (step + 1)..dimension {
            if gram[candidate * dimension + candidate] > gram[pivot * dimension + pivot] {
                pivot = candidate;
            }
        }
        if pivot != step {
            for column in 0..dimension {
                gram.swap(step * dimension + column, pivot * dimension + column);
            }
            for row in 0..dimension {
                gram.swap(row * dimension + step, row * dimension + pivot);
            }
            rhs.swap(step, pivot);
            permutation.swap(step, pivot);
            for column in 0..step {
                factor.swap(step * dimension + column, pivot * dimension + column);
            }
        }
        let pivot_value = gram[step * dimension + step];
        if !pivot_value.is_finite() || pivot_value <= pivot_guard {
            return Err(nonconvergence(
                "fused PPML CMG active finite basis lost rank",
            ));
        }
        let diagonal = pivot_value.sqrt();
        factor[step * dimension + step] = diagonal;
        for row in (step + 1)..dimension {
            factor[row * dimension + step] = gram[row * dimension + step] / diagonal;
        }
        for row in (step + 1)..dimension {
            for column in row..dimension {
                let value = gram[column * dimension + row]
                    - factor[row * dimension + step] * factor[column * dimension + step];
                if !value.is_finite() {
                    return Err(nonconvergence(
                        "fused PPML CMG Cholesky update is nonfinite",
                    ));
                }
                gram[column * dimension + row] = value;
                gram[row * dimension + column] = value;
            }
        }
    }
    for row in 0..dimension {
        let mut value = rhs[row];
        for column in 0..row {
            value -= factor[row * dimension + column] * forward[column];
        }
        forward[row] = value / factor[row * dimension + row];
    }
    for row in (0..dimension).rev() {
        let mut value = forward[row];
        for column in (row + 1)..dimension {
            value -= factor[column * dimension + row] * solution[column];
        }
        solution[row] = value / factor[row * dimension + row];
    }
    if forward
        .iter()
        .chain(solution.iter())
        .any(|value| !value.is_finite())
    {
        return Err(nonconvergence("fused PPML CMG Cholesky solve is nonfinite"));
    }
    Ok(())
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
    use crate::fe::EffectTerm;

    #[test]
    fn nuisance_reduction_requires_exact_nesting_in_a_selected_core() {
        for core in [0, 1] {
            let first = (0..192).map(|row| (row % 12) as u32).collect::<Vec<_>>();
            let second = (0..192)
                .map(|row| ((7 * row + row / 12) % 8) as u32)
                .collect::<Vec<_>>();
            let nuisance = if core == 0 { &first } else { &second }
                .iter()
                .map(|code| code / if core == 0 { 3 } else { 2 })
                .collect::<Vec<_>>();
            for near_nested in [false, true] {
                let mut nuisance = nuisance.clone();
                if near_nested {
                    nuisance[191] = (nuisance[191] + 1) % 4;
                }
                let codes = [&first, &second, &nuisance];
                let levels = [12, 8, 4];
                let indices = std::array::from_fn::<_, 3, _>(|index| {
                    crate::fe::LevelIndex::from_codes(codes[index], levels[index]).unwrap()
                });
                for order in [
                    [0, 1, 2],
                    [0, 2, 1],
                    [1, 0, 2],
                    [1, 2, 0],
                    [2, 0, 1],
                    [2, 1, 0],
                ] {
                    let design = BorrowedInterceptFeDesign::new(
                        192,
                        order
                            .iter()
                            .map(|&index| {
                                crate::fe::BorrowedInterceptEffect::new(
                                    codes[index],
                                    levels[index],
                                    &indices[index],
                                )
                                .unwrap()
                            })
                            .collect(),
                    )
                    .unwrap();
                    let redundant = redundant_nuisance_effect(&design).unwrap();
                    assert_eq!(
                        redundant.map(|index| order[index]),
                        (!near_nested).then_some(2)
                    );
                    let projected = projection_design(design, redundant).unwrap();
                    assert_eq!(projected.effects().len(), if near_nested { 3 } else { 2 });
                    for (effect, &index) in projected
                        .effects()
                        .iter()
                        .zip(order.iter().filter(|&&index| near_nested || index != 2))
                    {
                        assert_eq!(effect.codes().as_ptr(), codes[index].as_ptr());
                        assert!(std::ptr::eq(effect.level_index(), &indices[index]));
                    }
                    if !near_nested {
                        assert_eq!(redundant_nuisance_effect(&projected).unwrap(), None);
                    }
                }
            }
        }
    }

    #[test]
    fn graph_batch_capacity_covers_nuisance_without_exceeding_the_pool() {
        for pool in [1, 2, 7, 10, 64] {
            for rhs in [1, 3, 11, 51] {
                for nuisance in [0, 1, 8, 32, 63, 64] {
                    let batch = fused_cmg_batch_size(pool, rhs, nuisance);
                    assert!(batch > 0 && batch <= pool);
                    assert_eq!(batch, pool.min(rhs.max(nuisance)));
                    if nuisance <= rhs {
                        assert_eq!(batch, pool.min(rhs));
                    }
                }
            }
        }
        assert_eq!(fused_cmg_batch_size(10, 1, 64), 10);
        assert_eq!(fused_cmg_batch_size(10, 1, 8), 8);
        assert_eq!(fused_cmg_batch_size(10, 1, 0), 1);
    }

    #[test]
    fn pivoted_cholesky_recovers_a_permuted_spd_system() {
        let mut gram = vec![1.0, 0.5, 0.5, 4.0];
        let mut rhs = vec![2.0, 9.0];
        let mut factor = vec![0.0; 4];
        let mut forward = vec![0.0; 2];
        let mut solution = vec![0.0; 2];
        let mut permutation = vec![0; 2];
        pivoted_cholesky_solve(
            &mut gram,
            &mut rhs,
            &mut factor,
            &mut forward,
            &mut solution,
            &mut permutation,
            1e-14,
        )
        .expect("solve");
        let mut canonical = [0.0; 2];
        for position in 0..2 {
            canonical[permutation[position]] = solution[position];
        }
        assert!((canonical[0] - 14.0 / 15.0).abs() <= 1e-12);
        assert!((canonical[1] - 32.0 / 15.0).abs() <= 1e-12);
    }

    fn certificate_design(first: &[u32], second: &[u32]) -> FeDesign {
        FeDesign::new(
            first.len(),
            vec![
                EffectTerm::new(first.to_vec(), 2, true, Vec::new()).expect("first effect"),
                EffectTerm::new(second.to_vec(), 2, true, Vec::new()).expect("second effect"),
            ],
        )
        .expect("certificate design")
    }

    #[test]
    fn fused_workspace_estimate_is_monotone() {
        let narrow = estimate_fused_cmg_workspace_bytes(100, 2, 1).expect("narrow estimate");
        let wide = estimate_fused_cmg_workspace_bytes(100, 8, 1).expect("wide estimate");
        let long = estimate_fused_cmg_workspace_bytes(1_000, 2, 1).expect("long estimate");
        assert!(wide > narrow);
        assert!(long > narrow);
        let before = estimate_fused_cmg_workspace_bytes(MOMENT_BLOCK_ROWS - 1, 50, 7).unwrap();
        let at = estimate_fused_cmg_workspace_bytes(MOMENT_BLOCK_ROWS, 50, 7).unwrap();
        let after = estimate_fused_cmg_workspace_bytes(MOMENT_BLOCK_ROWS + 1, 50, 7).unwrap();
        assert_eq!(after - at - (at - before), (50 * 50 + 50) * 8);
        for rows in [0, 1, 4_096, 50_000, 262_144, 262_145, 1_000_000, usize::MAX] {
            assert!(rows.div_ceil(moment_block_rows(rows)) <= MOMENT_MAX_BLOCKS);
        }
    }

    #[test]
    fn finite_certificate_preserves_literal_moments_across_thread_counts() {
        let rows = 4097;
        let columns = 50;
        let values = (0..rows * columns)
            .map(|cell| (cell as f64 / 17.0).sin())
            .collect::<Vec<_>>();
        let weights = (0..rows)
            .map(|row| 2.0_f64.powi((row % 17) as i32 - 8))
            .collect::<Vec<_>>();
        let residual = (0..rows)
            .map(|row| (row as f64 / 23.0).cos())
            .collect::<Vec<_>>();
        let mut expected = vec![0.0; columns];
        let mut expected_square = vec![0.0; columns];
        for column in 0..columns {
            let mut moment = CompensatedSum::default();
            let mut square = CompensatedSum::default();
            for row in 0..rows {
                let value = values[row * columns + column];
                moment.add(weights[row] * residual[row] * value).unwrap();
                square.add(weights[row] * value * value).unwrap();
            }
            expected[column] = moment.total();
            expected_square[column] = square.total();
        }
        for threads in [1, 2, 7] {
            let team = ThreadTeam::new(ThreadConfig::from_requested(threads, true).unwrap())
                .acquire()
                .unwrap();
            let mut moments = vec![f64::NAN; columns];
            let mut squares = vec![f64::NAN; columns];
            team.install(|| {
                fill_finite_certificate_moments(
                    &values,
                    &residual,
                    &weights,
                    &mut moments,
                    &mut squares,
                )
            })
            .unwrap();
            assert_eq!(moments, expected);
            assert_eq!(squares, expected_square);
            team.install(|| {
                fill_finite_certificate_moments(&[], &residual, &weights, &mut [], &mut [])
            })
            .unwrap();
        }
        assert!(
            fill_finite_certificate_moments(&values, &residual, &weights, &mut [0.0], &mut [])
                .is_err()
        );
    }

    #[test]
    fn certificate_topology_is_bound_to_the_fit_lifetime_design() {
        let first = [0, 0, 1, 1];
        let second = [0, 1, 0, 1];
        let design = certificate_design(&first, &second);
        let identity = FusedCmgCertificateTopology::bind(&design).expect("topology identity");
        assert!(identity.matches(&design));

        let same_values_in_distinct_storage = certificate_design(&first, &second);
        assert!(!identity.matches(&same_values_in_distinct_storage));

        let changed = certificate_design(&first, &[0, 1, 1, 0]);
        assert!(!identity.matches(&changed));
    }
}
