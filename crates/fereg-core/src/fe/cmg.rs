//! Certified CMG-PCG absorption for sparse AKM-style categorical designs.
//!
//! Two high-cardinality intercept effects define a weighted bipartite graph.
//! Additional small intercept effects are eliminated through a dense Schur
//! complement. One graph hierarchy and workspace pool are retained for the
//! complete prepared OLS fit, including low-memory blocks and residual polish.

use std::time::Instant;

use cmg::{
    CmgMemoryEstimate, CmgOptions, CmgPreconditioner, CmgProblemSize, Laplacian, ParallelExecutor,
    ParallelOptions, ParallelPcgPolicy, ParallelPcgSolver, ParallelPcgWorkspace, PcgOptions,
};

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::memory::{estimate_fe_solver_memory, FeSolverMemoryBudget};
use crate::options::ThreadConfig;

use super::{
    absorb_borrowed_intercept_map_certified,
    absorb_borrowed_intercept_map_with_structural_constant_certified,
    borrowed_map::polish_borrowed_intercept_cg_with_reference_norms_certified,
    certify_borrowed_intercept_candidate, BorrowedInterceptFeDesign, BorrowedMapWeights,
    CertifiedBorrowedMap, FeCoefficientBatch, FeSolverKind, MapConfig, SolveReport,
    SolveStopReason,
};

const MAX_NUISANCE_COLUMNS: usize = 64;
// CMG's graph solve feeds the OLS normal equations, whose score certificate is
// more sensitive than the observation-space FE certificate. Keep the public
// tolerance unchanged while solving the reusable graph system two orders
// tighter; this avoids a separate post-QR correction and preserves the fitted
// model's RSS/covariance identities.
const PCG_TOLERANCE_MULTIPLIER: f64 = 0.01;
const PCG_TOLERANCE_FLOOR: f64 = 32.0 * f64::EPSILON;
/// The 2026-09-01 development calibration targets AKM-like panels: CMG is the
/// default only when the large worker axis is observed at least three times,
/// the firm axis is at least ten times smaller, and any third FE contributes
/// at most nine non-base nuisance columns. Other ordinary categorical designs
/// use direct Schwarz-LSMR and retain certified CG as a safety continuation.
const AUTO_MIN_OBSERVATIONS_PER_CORE_LEVEL: usize = 3;
const AUTO_MIN_CORE_CARDINALITY_RATIO: usize = 10;
const AUTO_MAX_NUISANCE_COLUMNS: usize = 9;

/// Internal execution policy for one prepared CMG absorption session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CmgConfig {
    pub map: MapConfig,
    pub threads: ThreadConfig,
    pub batch_size: usize,
    pub retain_coefficients: bool,
    pub requested_solver: FeSolverKind,
    pub solver_memory_budget: Option<FeSolverMemoryBudget>,
}

impl CmgConfig {
    fn pcg_options(self) -> PcgOptions {
        PcgOptions {
            relative_tolerance: (self.map.tolerance * PCG_TOLERANCE_MULTIPLIER)
                .max(PCG_TOLERANCE_FLOOR),
            max_iterations: self.map.max_iterations,
            ..PcgOptions::default()
        }
    }
}

/// Observation-pass-free topology summary used by the automatic CMG route.
/// The canonical-edge count deliberately remains the safe input-edge upper
/// bound: computing it exactly would add an avoidable O(N) pass before graph
/// construction and would move runtime even when the route is rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CmgTopologyProfile {
    effect_order: Vec<usize>,
    core_effects: Option<[usize; 2]>,
    vertices: usize,
    nuisance_columns: usize,
    observations: usize,
    highest_cardinality_levels: usize,
    second_cardinality_levels: usize,
}

impl CmgTopologyProfile {
    pub(crate) fn analyze(design: &BorrowedInterceptFeDesign<'_>) -> Result<Self> {
        let mut effect_order: Vec<usize> = (0..design.effects().len()).collect();
        effect_order.sort_by(|left, right| {
            design.effects()[*right]
                .n_levels()
                .cmp(&design.effects()[*left].n_levels())
                .then_with(|| left.cmp(right))
        });
        let core_effects = (effect_order.len() >= 2).then(|| [effect_order[0], effect_order[1]]);
        let highest_cardinality_levels = core_effects
            .map(|core| design.effects()[core[0]].n_levels())
            .unwrap_or(0);
        let second_cardinality_levels = core_effects
            .map(|core| design.effects()[core[1]].n_levels())
            .unwrap_or(0);
        let vertices = if let Some(core) = core_effects {
            design.effects()[core[0]]
                .n_levels()
                .checked_add(design.effects()[core[1]].n_levels())
                .ok_or_else(|| allocation("CMG graph vertex count overflows"))?
        } else {
            0
        };
        let nuisance_columns = effect_order
            .iter()
            .skip(2)
            .try_fold(0_usize, |total, effect| {
                total
                    .checked_add(design.effects()[*effect].n_levels().saturating_sub(1))
                    .ok_or_else(|| allocation("CMG nuisance dimension overflows"))
            })?;
        Ok(Self {
            effect_order,
            core_effects,
            vertices,
            nuisance_columns,
            observations: design.nobs(),
            highest_cardinality_levels,
            second_cardinality_levels,
        })
    }

    #[must_use]
    pub(crate) fn auto_favorable(&self) -> bool {
        self.core_effects.is_some()
            && self.nuisance_columns <= AUTO_MAX_NUISANCE_COLUMNS
            && self
                .highest_cardinality_levels
                .checked_mul(AUTO_MIN_OBSERVATIONS_PER_CORE_LEVEL)
                .is_some_and(|minimum| self.observations >= minimum)
            && self
                .second_cardinality_levels
                .checked_mul(AUTO_MIN_CORE_CARDINALITY_RATIO)
                .is_some_and(|minimum| self.highest_cardinality_levels >= minimum)
    }

    #[cfg(test)]
    const fn nuisance_columns(&self) -> usize {
        self.nuisance_columns
    }
}

/// Fit-lifetime CMG state. The borrowed design owns only small term views;
/// observation codes and membership remain in prepared data.
pub(crate) struct CmgAbsorber<'a> {
    design: BorrowedInterceptFeDesign<'a>,
    weights: BorrowedMapWeights<'a>,
    weight_scale: f64,
    config: CmgConfig,
    route: CmgRoute,
    pending_setup_seconds: f64,
    pending_setup_metrics: CmgSolveMetrics,
    structural_constant_if_empty: bool,
}

// The graph state is fit-lifetime and constructed once. Keeping it inline
// avoids a second infallible heap allocation at the fail-closed setup boundary;
// the roughly 800-byte session value is deliberately bounded.
#[allow(clippy::large_enum_variant)]
enum CmgRoute {
    Direct,
    Graph(GraphCmg),
    Failed(String),
}

struct GraphCmg {
    core_effects: [usize; 2],
    nuisance: Vec<NuisanceColumn>,
    nuisance_factor: PivotedPsdFactor,
    solver: ParallelPcgSolver,
    workspace: ParallelPcgWorkspace,
    setup_iterations: usize,
    setup_backward_error: f64,
}

#[derive(Debug, Clone, Copy)]
struct NuisanceColumn {
    effect: usize,
    level: u32,
}

#[derive(Debug, Default, Clone, Copy)]
struct CmgSolveMetrics {
    iterations: usize,
    backward_error: f64,
}

impl CmgSolveMetrics {
    fn include(&mut self, iterations: usize, backward_error: f64) {
        self.iterations = self.iterations.max(iterations);
        self.backward_error = self.backward_error.max(backward_error);
    }
}

impl<'a> CmgAbsorber<'a> {
    pub(crate) fn build(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'a>,
        config: CmgConfig,
        structural_constant_if_empty: bool,
    ) -> Result<Self> {
        let profile = CmgTopologyProfile::analyze(&design)?;
        Self::build_profiled(
            design,
            weights,
            config,
            structural_constant_if_empty,
            profile,
        )
    }

    pub(crate) fn build_profiled(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'a>,
        config: CmgConfig,
        structural_constant_if_empty: bool,
        profile: CmgTopologyProfile,
    ) -> Result<Self> {
        validate_config(config)?;
        let weight_scale = validate_weights(weights, design.nobs())?;
        let setup_started = Instant::now();
        let route = if design.effects().len() < 2 {
            CmgRoute::Direct
        } else {
            validate_nuisance_limit(&profile)?;
            let graph = preflight_cmg_memory(&design, &profile, config)
                .and_then(|()| build_graph_route(&design, weights, weight_scale, config, &profile));
            match graph {
                Ok(graph) => CmgRoute::Graph(graph),
                Err(error) if config.requested_solver == FeSolverKind::Auto => return Err(error),
                Err(error) => CmgRoute::Failed(error.to_string()),
            }
        };
        let pending_setup_metrics = match &route {
            CmgRoute::Graph(graph) => CmgSolveMetrics {
                iterations: graph.setup_iterations,
                backward_error: graph.setup_backward_error,
            },
            CmgRoute::Direct | CmgRoute::Failed(_) => CmgSolveMetrics::default(),
        };
        Ok(Self {
            design,
            weights,
            weight_scale,
            config,
            route,
            pending_setup_seconds: setup_started.elapsed().as_secs_f64(),
            pending_setup_metrics,
            structural_constant_if_empty,
        })
    }

    /// Residualize one owned matrix. Failure never exposes a partially updated
    /// matrix to the caller; a valid partial CMG projection can still seed the
    /// existing certified CG fallback.
    pub(crate) fn absorb(&mut self, values: DenseMatrix) -> Result<CertifiedBorrowedMap> {
        if values.nrows() != self.design.nobs() || values.ncols() == 0 {
            return Err(invalid_argument(
                "CMG design and matrix dimensions are inconsistent",
            ));
        }
        if values.as_slice().iter().any(|value| !value.is_finite()) {
            return Err(invalid_data("CMG input values must be finite"));
        }
        let reference_norms = weighted_column_norms(&values, self.weights, self.weight_scale)?;
        let setup_seconds = std::mem::take(&mut self.pending_setup_seconds);
        let setup_metrics = std::mem::take(&mut self.pending_setup_metrics);

        if matches!(&self.route, CmgRoute::Direct) {
            let direct = if self.structural_constant_if_empty {
                absorb_borrowed_intercept_map_with_structural_constant_certified(
                    &self.design,
                    self.weights,
                    values,
                    self.config.map,
                )?
            } else {
                absorb_borrowed_intercept_map_certified(
                    &self.design,
                    self.weights,
                    values,
                    self.config.map,
                )?
            };
            let (residualized, mut report, coefficients) = direct.into_full_parts();
            report.requested_solver = self.config.requested_solver;
            report.setup_seconds += setup_seconds;
            return if let Some(coefficients) = coefficients {
                CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                    residualized,
                    report,
                    coefficients,
                )
            } else {
                CertifiedBorrowedMap::from_certified_parts(residualized, report)
            };
        }

        if let CmgRoute::Failed(message) = &self.route {
            ensure_cg_fallback_fits(&self.design, values.ncols(), self.config)?;
            let fallback = polish_borrowed_intercept_cg_with_reference_norms_certified(
                &self.design,
                self.weights,
                values,
                self.config.map,
                &reference_norms,
            )?;
            let (residualized, mut report) = fallback.into_parts();
            report.requested_solver = self.config.requested_solver;
            report.used_solver = FeSolverKind::Cg;
            report.fallback_used = true;
            report.setup_seconds += setup_seconds;
            report.warnings.insert(
                0,
                format!("CMG setup failed ({message}); certified with CG"),
            );
            return CertifiedBorrowedMap::from_certified_parts(residualized, report);
        }

        let solve_started = Instant::now();
        let mut candidate = values;
        let bulk = match &mut self.route {
            CmgRoute::Graph(graph) => graph.project_matrix(
                &self.design,
                self.weights,
                self.weight_scale,
                &mut candidate,
                self.config,
            ),
            CmgRoute::Direct => unreachable!("direct CMG route returned above"),
            CmgRoute::Failed(_) => unreachable!("failed CMG route returned above"),
        };
        let bulk_seconds = solve_started.elapsed().as_secs_f64();

        match bulk {
            Ok((mut metrics, coefficients)) => {
                metrics.include_from(setup_metrics);
                let polished = polish_borrowed_intercept_cg_with_reference_norms_certified(
                    &self.design,
                    self.weights,
                    candidate,
                    self.config.map,
                    &reference_norms,
                )?;
                let (residualized, mut report) = polished.into_parts();
                let polish_iterations = report.iterations;
                report.requested_solver = self.config.requested_solver;
                report.used_solver = FeSolverKind::Cmg;
                report.iterations = metrics.iterations.saturating_add(polish_iterations);
                report.normal_equation_residual =
                    metrics.backward_error.max(report.normal_equation_residual);
                report.setup_seconds = setup_seconds;
                report.solve_seconds += bulk_seconds;
                if polish_iterations > 0 {
                    report.warnings.push(format!(
                        "CMG bulk projection required {polish_iterations} certified CG polishing iterations"
                    ));
                }
                if polish_iterations == 0 {
                    if let Some(coefficients) = coefficients {
                        return CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                            residualized,
                            report,
                            coefficients,
                        );
                    }
                }
                CertifiedBorrowedMap::from_certified_parts(residualized, report)
            }
            Err(error) => {
                ensure_cg_fallback_fits(&self.design, candidate.ncols(), self.config)?;
                let failure = error.to_string();
                self.route = CmgRoute::Failed(failure.clone());
                let fallback = polish_borrowed_intercept_cg_with_reference_norms_certified(
                    &self.design,
                    self.weights,
                    candidate,
                    self.config.map,
                    &reference_norms,
                )?;
                let (residualized, mut report) = fallback.into_parts();
                report.requested_solver = self.config.requested_solver;
                report.used_solver = FeSolverKind::Cg;
                report.fallback_used = true;
                report.setup_seconds += setup_seconds;
                report.solve_seconds += bulk_seconds;
                report
                    .warnings
                    .insert(0, format!("CMG path failed ({failure}); certified with CG"));
                CertifiedBorrowedMap::from_certified_parts(residualized, report)
            }
        }
    }

    /// Final OLS residuals have already been formed from certified within
    /// coordinates and are normally FE-orthogonal. Running graph PCG on that
    /// near-zero right-hand side can exhaust its relative iteration budget.
    /// Certify first and perform a CG correction only when one is actually
    /// required; the retained CMG hierarchy remains available for all earlier
    /// outcome and regressor blocks.
    pub(crate) fn absorb_final_residual(
        &mut self,
        values: DenseMatrix,
    ) -> Result<CertifiedBorrowedMap> {
        if values.nrows() != self.design.nobs() || values.ncols() == 0 {
            return Err(invalid_argument(
                "CMG final-residual design and matrix dimensions are inconsistent",
            ));
        }
        let certificate = certify_borrowed_intercept_candidate(
            &self.design,
            self.weights,
            &values,
            self.config.map,
        )?;
        if !certificate.passed {
            return self.absorb(values);
        }
        let normal_equation_residual = certificate.relative_two_norm;
        CertifiedBorrowedMap::from_certified_parts(
            values,
            SolveReport {
                requested_solver: self.config.requested_solver,
                used_solver: FeSolverKind::Cmg,
                iterations: 0,
                stop_reason: SolveStopReason::Certified,
                certificate,
                normal_equation_residual,
                setup_seconds: 0.0,
                solve_seconds: 0.0,
                fallback_used: false,
                requested_preconditioner: None,
                used_preconditioner: None,
                pilot_iterations: 0,
                pilot_contraction: 0.0,
                warnings: Vec::new(),
            },
        )
    }
}

impl GraphCmg {
    fn project_matrix(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        weight_scale: f64,
        values: &mut DenseMatrix,
        config: CmgConfig,
    ) -> Result<(CmgSolveMetrics, Option<FeCoefficientBatch>)> {
        let mut metrics = CmgSolveMetrics::default();
        let mut coefficients = config
            .retain_coefficients
            .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
            .transpose()?;
        let core_metrics = self.project_core_matrix(
            design,
            weights,
            weight_scale,
            values,
            CoreProjectionRequest {
                config,
                coefficients: coefficients.as_mut(),
                destination_rhs_start: 0,
                coefficient_scale: 1.0,
            },
        )?;
        metrics.include_from(core_metrics);
        if self.nuisance.is_empty() || self.nuisance_factor.rank == 0 {
            return Ok((metrics, coefficients));
        }

        let columns = values.ncols();
        for start in (0..columns).step_by(config.batch_size) {
            let end = start.saturating_add(config.batch_size).min(columns);
            let width = end - start;
            let mut gamma = Vec::new();
            gamma
                .try_reserve_exact(width)
                .map_err(|_| allocation("unable to reserve CMG nuisance coefficients"))?;
            for column in start..end {
                let moments = nuisance_moments(
                    design,
                    &self.nuisance,
                    weights,
                    weight_scale,
                    values,
                    column,
                )?;
                gamma.push(self.nuisance_factor.solve(&moments)?);
            }
            if let Some(batch) = coefficients.as_mut() {
                for (local, nuisance_coefficients) in gamma.iter().enumerate() {
                    for (index, nuisance_column) in self.nuisance.iter().enumerate() {
                        batch.add_intercept(
                            nuisance_column.effect,
                            nuisance_column.level as usize,
                            start + local,
                            nuisance_coefficients[index],
                        )?;
                    }
                }
            }
            let mut correction = DenseMatrix::zeros(values.nrows(), width)?;
            for row in 0..values.nrows() {
                for (local, coefficients) in gamma.iter().enumerate() {
                    correction[(row, local)] =
                        nuisance_value(design, &self.nuisance, coefficients, row)?;
                }
            }
            let correction_metrics = self.project_core_matrix(
                design,
                weights,
                weight_scale,
                &mut correction,
                CoreProjectionRequest {
                    config,
                    coefficients: coefficients.as_mut(),
                    destination_rhs_start: start,
                    coefficient_scale: -1.0,
                },
            )?;
            metrics.include_from(correction_metrics);
            for row in 0..values.nrows() {
                for local in 0..width {
                    let updated = values[(row, start + local)] - correction[(row, local)];
                    if !updated.is_finite() {
                        return Err(invalid_data(
                            "CMG nuisance projection produced a nonfinite residual",
                        ));
                    }
                    values[(row, start + local)] = updated;
                }
            }
        }
        Ok((metrics, coefficients))
    }

    fn project_core_matrix(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        weight_scale: f64,
        values: &mut DenseMatrix,
        request: CoreProjectionRequest<'_>,
    ) -> Result<CmgSolveMetrics> {
        let CoreProjectionRequest {
            config,
            mut coefficients,
            destination_rhs_start,
            coefficient_scale,
        } = request;
        let mut metrics = CmgSolveMetrics::default();
        for start in (0..values.ncols()).step_by(config.batch_size) {
            let end = start.saturating_add(config.batch_size).min(values.ncols());
            let right_hand_sides = build_value_rhs_batch(
                design,
                self.core_effects,
                weights,
                weight_scale,
                values,
                start,
                end,
            )?;
            let solved = self
                .solver
                .solve_batch_with_workspace(
                    &right_hand_sides,
                    config.pcg_options(),
                    &mut self.workspace,
                )
                .map_err(cmg_error)?;
            for (local, solution) in solved.results().iter().enumerate() {
                metrics.include(solution.iterations(), solution.backward_error());
                if let Some(batch) = coefficients.as_deref_mut() {
                    record_core_solution(
                        design,
                        self.core_effects,
                        batch,
                        destination_rhs_start + start + local,
                        coefficient_scale,
                        solution.solution(),
                    )?;
                }
                subtract_core_fit(
                    design,
                    self.core_effects,
                    values,
                    start + local,
                    solution.solution(),
                )?;
            }
        }
        Ok(metrics)
    }
}

struct CoreProjectionRequest<'a> {
    config: CmgConfig,
    coefficients: Option<&'a mut FeCoefficientBatch>,
    destination_rhs_start: usize,
    coefficient_scale: f64,
}

fn record_core_solution(
    design: &BorrowedInterceptFeDesign<'_>,
    core: [usize; 2],
    coefficients: &mut FeCoefficientBatch,
    rhs: usize,
    scale: f64,
    solution: &[f64],
) -> Result<()> {
    let left_levels = design.effects()[core[0]].n_levels();
    let right_levels = design.effects()[core[1]].n_levels();
    if solution.len() != left_levels.saturating_add(right_levels) || !scale.is_finite() {
        return Err(invalid_argument(
            "CMG solution and retained coefficient dimensions are inconsistent",
        ));
    }
    for (level, value) in solution[..left_levels].iter().copied().enumerate() {
        coefficients.add_intercept(core[0], level, rhs, scale * value)?;
    }
    for (level, value) in solution[left_levels..].iter().copied().enumerate() {
        coefficients.add_intercept(core[1], level, rhs, -scale * value)?;
    }
    Ok(())
}

impl CmgSolveMetrics {
    fn include_from(&mut self, other: Self) {
        self.include(other.iterations, other.backward_error);
    }
}

fn build_graph_route(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    config: CmgConfig,
    profile: &CmgTopologyProfile,
) -> Result<GraphCmg> {
    let core_effects = profile
        .core_effects
        .ok_or_else(|| invalid_argument("CMG graph route requires two absorbed effects"))?;
    let nuisance = build_nuisance_columns(design, &profile.effect_order[2..])?;

    let left_levels = design.effects()[core_effects[0]].n_levels();
    let right_levels = design.effects()[core_effects[1]].n_levels();
    let vertices = left_levels
        .checked_add(right_levels)
        .ok_or_else(|| allocation("CMG graph vertex count overflows"))?;
    let parallel = ParallelOptions {
        threads: config.threads.resolved(),
        ..ParallelOptions::default()
    };
    let executor = ParallelExecutor::new(parallel).map_err(cmg_error)?;
    let left_codes = design.effects()[core_effects[0]].codes();
    let right_codes = design.effects()[core_effects[1]].codes();
    let edges = (0..design.nobs()).map(|row| {
        (
            left_codes[row] as usize,
            left_levels + right_codes[row] as usize,
            normalized_weight(weights, weight_scale, row),
        )
    });
    let graph =
        Laplacian::from_edges_with_executor(vertices, edges, &executor).map_err(cmg_error)?;
    let preconditioner =
        CmgPreconditioner::build_with_executor(&graph, CmgOptions::default(), &executor)
            .map_err(cmg_error)?;
    let solver = ParallelPcgSolver::from_preconditioner(
        preconditioner,
        executor,
        ParallelPcgPolicy::default(),
    )
    .map_err(cmg_error)?;
    let workspace = solver.workspace();
    let mut route = GraphCmg {
        core_effects,
        nuisance,
        nuisance_factor: PivotedPsdFactor::empty(),
        solver,
        workspace,
        setup_iterations: 0,
        setup_backward_error: 0.0,
    };
    let (factor, metrics) =
        build_nuisance_factor(design, weights, weight_scale, &mut route, config)?;
    route.nuisance_factor = factor;
    ensure_cmg_exact_retained_memory_fits(&route, config)?;
    route.setup_iterations = metrics.iterations;
    route.setup_backward_error = metrics.backward_error;
    Ok(route)
}

fn build_nuisance_columns(
    design: &BorrowedInterceptFeDesign<'_>,
    effect_order: &[usize],
) -> Result<Vec<NuisanceColumn>> {
    let count = effect_order.iter().try_fold(0_usize, |total, effect| {
        total
            .checked_add(design.effects()[*effect].n_levels().saturating_sub(1))
            .ok_or_else(|| allocation("CMG nuisance dimension overflows"))
    })?;
    if count > MAX_NUISANCE_COLUMNS {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            format!(
                "solver(cmg) supports at most {MAX_NUISANCE_COLUMNS} non-base levels across additional absorbed effects; found {count}"
            ),
        ));
    }
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(count)
        .map_err(|_| allocation("unable to reserve CMG nuisance column map"))?;
    for effect in effect_order {
        for level in 1..design.effects()[*effect].n_levels() {
            columns.push(NuisanceColumn {
                effect: *effect,
                level: u32::try_from(level)
                    .map_err(|_| allocation("CMG nuisance level exceeds u32"))?,
            });
        }
    }
    Ok(columns)
}

fn validate_nuisance_limit(profile: &CmgTopologyProfile) -> Result<()> {
    if profile.nuisance_columns > MAX_NUISANCE_COLUMNS {
        Err(FeregError::new(
            ErrorCode::Unsupported,
            format!(
                "solver(cmg) supports at most {MAX_NUISANCE_COLUMNS} non-base levels across additional absorbed effects; found {}",
                profile.nuisance_columns
            ),
        ))
    } else {
        Ok(())
    }
}

fn preflight_cmg_memory(
    design: &BorrowedInterceptFeDesign<'_>,
    profile: &CmgTopologyProfile,
    config: CmgConfig,
) -> Result<()> {
    let Some(budget) = config.solver_memory_budget else {
        return Ok(());
    };
    let parallel = ParallelOptions {
        threads: config.threads.resolved(),
        ..ParallelOptions::default()
    };
    let estimate = CmgMemoryEstimate::conservative(
        CmgProblemSize {
            vertices: profile.vertices,
            input_edges: profile.observations,
            // Safe deterministic upper bound; exact aggregation would add an
            // avoidable O(N) pass before construction.
            canonical_edges: profile.observations,
            right_hand_sides: config.batch_size,
        },
        CmgOptions::default(),
        parallel,
    )
    .map_err(cmg_error)?;
    // fereg retains the input and solved graph vectors together at batch
    // return, outside the upstream solver workspace model.
    let batch_vectors = profile
        .vertices
        .checked_mul(config.batch_size)
        .and_then(|value| value.checked_mul(16))
        .ok_or_else(|| allocation("CMG batched graph workspace estimate overflows"))?;
    // Schur matrix, compensation, factor, and factor work at the 64-column cap.
    let nuisance_square = profile
        .nuisance_columns
        .checked_mul(profile.nuisance_columns)
        .and_then(|value| value.checked_mul(32))
        .ok_or_else(|| allocation("CMG nuisance workspace estimate overflows"))?;
    let retained_coefficients = if config.retain_coefficients {
        design
            .total_term_slots()
            .checked_mul(config.batch_size)
            .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
            .ok_or_else(|| allocation("CMG retained coefficient estimate overflows"))?
    } else {
        0
    };
    let required = (estimate.build_peak_bytes() as u128)
        .checked_add(batch_vectors as u128)
        .and_then(|value| value.checked_add(nuisance_square as u128))
        .and_then(|value| value.checked_add(retained_coefficients as u128))
        .ok_or_else(|| allocation("CMG complete memory estimate overflows"))?;
    if budget.fits(required) {
        Ok(())
    } else {
        Err(allocation(format!(
            "CMG requires {required} incremental bytes and does not fit the joint execution/solver memory plan"
        )))
    }
}

fn ensure_cg_fallback_fits(
    design: &BorrowedInterceptFeDesign<'_>,
    right_hand_sides: usize,
    config: CmgConfig,
) -> Result<()> {
    let Some(budget) = config.solver_memory_budget else {
        return Ok(());
    };
    let levels = design
        .effects()
        .iter()
        .map(super::BorrowedInterceptEffect::n_levels)
        .collect::<Vec<_>>();
    let estimate = estimate_fe_solver_memory(
        design.nobs(),
        right_hand_sides,
        &levels,
        config.threads.resolved(),
        None,
    )?;
    if budget.fits(estimate.cg_bytes) {
        Ok(())
    } else {
        Err(allocation(format!(
            "certified CG fallback requires {} incremental bytes and does not fit the joint execution/solver memory plan",
            estimate.cg_bytes
        )))
    }
}

fn ensure_cmg_exact_retained_memory_fits(graph: &GraphCmg, config: CmgConfig) -> Result<()> {
    let Some(budget) = config.solver_memory_budget else {
        return Ok(());
    };
    let upstream = graph.solver.memory_report(&graph.workspace);
    let nuisance_bytes = graph
        .nuisance
        .capacity()
        .checked_mul(std::mem::size_of::<NuisanceColumn>())
        .and_then(|value| value.checked_add(graph.nuisance_factor.retained_bytes()))
        .ok_or_else(|| allocation("CMG exact retained-memory report overflows"))?;
    let required = (upstream.total_retained_bytes() as u128)
        .checked_add(nuisance_bytes as u128)
        .ok_or_else(|| allocation("CMG exact retained-memory report overflows"))?;
    if budget.fits(required) {
        Ok(())
    } else {
        Err(allocation(format!(
            "CMG retained hierarchy and workspace require {required} incremental bytes and do not fit the joint execution/solver memory plan"
        )))
    }
}

fn build_nuisance_factor(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    graph: &mut GraphCmg,
    config: CmgConfig,
) -> Result<(PivotedPsdFactor, CmgSolveMetrics)> {
    let dimension = graph.nuisance.len();
    if dimension == 0 {
        return Ok((PivotedPsdFactor::empty(), CmgSolveMetrics::default()));
    }
    let square = dimension
        .checked_mul(dimension)
        .ok_or_else(|| allocation("CMG nuisance Schur dimensions overflow"))?;
    let mut schur = zero_vec(square, "unable to reserve CMG nuisance Schur matrix")?;
    let mut corrections = zero_vec(square, "unable to reserve CMG nuisance Schur corrections")?;
    let mut metrics = CmgSolveMetrics::default();

    for start in (0..dimension).step_by(config.batch_size) {
        let end = start.saturating_add(config.batch_size).min(dimension);
        let right_hand_sides = build_nuisance_rhs_batch(
            design,
            graph.core_effects,
            &graph.nuisance,
            weights,
            weight_scale,
            start,
            end,
        )?;
        let solved = graph
            .solver
            .solve_batch_with_workspace(
                &right_hand_sides,
                config.pcg_options(),
                &mut graph.workspace,
            )
            .map_err(cmg_error)?;
        for (local, solution) in solved.results().iter().enumerate() {
            metrics.include(solution.iterations(), solution.backward_error());
            let column = start + local;
            accumulate_schur_column(
                design,
                graph.core_effects,
                &graph.nuisance,
                weights,
                weight_scale,
                column,
                solution.solution(),
                &mut schur,
                &mut corrections,
            )?;
        }
    }
    for left in 0..dimension {
        for right in left..dimension {
            let forward = schur[left * dimension + right] + corrections[left * dimension + right];
            let reverse = schur[right * dimension + left] + corrections[right * dimension + left];
            let symmetric = 0.5 * (forward + reverse);
            if !symmetric.is_finite() {
                return Err(invalid_data("CMG nuisance Schur matrix is nonfinite"));
            }
            schur[left * dimension + right] = symmetric;
            schur[right * dimension + left] = symmetric;
        }
    }
    Ok((
        PivotedPsdFactor::factor(schur, dimension, config.map.tolerance)?,
        metrics,
    ))
}

fn build_value_rhs_batch(
    design: &BorrowedInterceptFeDesign<'_>,
    core: [usize; 2],
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    values: &DenseMatrix,
    start: usize,
    end: usize,
) -> Result<Vec<Vec<f64>>> {
    let vertices = core_vertices(design, core)?;
    let mut batch = zero_matrix_vectors(end - start, vertices, "unable to reserve CMG RHS batch")?;
    let left = design.effects()[core[0]].codes();
    let right = design.effects()[core[1]].codes();
    let right_offset = design.effects()[core[0]].n_levels();
    for row in 0..values.nrows() {
        let weight = normalized_weight(weights, weight_scale, row);
        for (local, column) in (start..end).enumerate() {
            let contribution = weight * values[(row, column)];
            batch[local][left[row] as usize] += contribution;
            batch[local][right_offset + right[row] as usize] -= contribution;
        }
    }
    validate_finite_vectors(&batch, "CMG RHS accumulation is nonfinite")?;
    Ok(batch)
}

fn build_nuisance_rhs_batch(
    design: &BorrowedInterceptFeDesign<'_>,
    core: [usize; 2],
    nuisance: &[NuisanceColumn],
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    start: usize,
    end: usize,
) -> Result<Vec<Vec<f64>>> {
    let vertices = core_vertices(design, core)?;
    let mut batch = zero_matrix_vectors(
        end - start,
        vertices,
        "unable to reserve CMG nuisance RHS batch",
    )?;
    let left = design.effects()[core[0]].codes();
    let right = design.effects()[core[1]].codes();
    let right_offset = design.effects()[core[0]].n_levels();
    for row in 0..design.nobs() {
        let weight = normalized_weight(weights, weight_scale, row);
        for (local, column) in nuisance[start..end].iter().enumerate() {
            if design.effects()[column.effect].codes()[row] == column.level {
                batch[local][left[row] as usize] += weight;
                batch[local][right_offset + right[row] as usize] -= weight;
            }
        }
    }
    validate_finite_vectors(&batch, "CMG nuisance RHS accumulation is nonfinite")?;
    Ok(batch)
}

#[allow(clippy::too_many_arguments)]
fn accumulate_schur_column(
    design: &BorrowedInterceptFeDesign<'_>,
    core: [usize; 2],
    nuisance: &[NuisanceColumn],
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    column: usize,
    solution: &[f64],
    schur: &mut [f64],
    corrections: &mut [f64],
) -> Result<()> {
    let dimension = nuisance.len();
    let left = design.effects()[core[0]].codes();
    let right = design.effects()[core[1]].codes();
    let right_offset = design.effects()[core[0]].n_levels();
    for row in 0..design.nobs() {
        let indicator =
            if design.effects()[nuisance[column].effect].codes()[row] == nuisance[column].level {
                1.0
            } else {
                0.0
            };
        let within = indicator
            - (solution[left[row] as usize] - solution[right_offset + right[row] as usize]);
        let weighted = normalized_weight(weights, weight_scale, row) * within;
        if !weighted.is_finite() {
            return Err(invalid_data("CMG nuisance Schur contribution is nonfinite"));
        }
        for (other, nuisance_column) in nuisance.iter().enumerate() {
            if design.effects()[nuisance_column.effect].codes()[row] == nuisance_column.level {
                compensated_add(
                    weighted,
                    &mut schur[other * dimension + column],
                    &mut corrections[other * dimension + column],
                )?;
            }
        }
    }
    Ok(())
}

fn nuisance_moments(
    design: &BorrowedInterceptFeDesign<'_>,
    nuisance: &[NuisanceColumn],
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    values: &DenseMatrix,
    column: usize,
) -> Result<Vec<f64>> {
    let mut moments = zero_vec(
        nuisance.len(),
        "unable to reserve CMG nuisance moment vector",
    )?;
    let mut corrections = zero_vec(
        nuisance.len(),
        "unable to reserve CMG nuisance moment corrections",
    )?;
    for row in 0..values.nrows() {
        let weighted = normalized_weight(weights, weight_scale, row) * values[(row, column)];
        for (index, nuisance_column) in nuisance.iter().enumerate() {
            if design.effects()[nuisance_column.effect].codes()[row] == nuisance_column.level {
                compensated_add(weighted, &mut moments[index], &mut corrections[index])?;
            }
        }
    }
    for (moment, correction) in moments.iter_mut().zip(corrections) {
        *moment += correction;
        if !moment.is_finite() {
            return Err(invalid_data("CMG nuisance moment is nonfinite"));
        }
    }
    Ok(moments)
}

fn nuisance_value(
    design: &BorrowedInterceptFeDesign<'_>,
    nuisance: &[NuisanceColumn],
    coefficients: &[f64],
    row: usize,
) -> Result<f64> {
    if coefficients.len() != nuisance.len() {
        return Err(invalid_argument(
            "CMG nuisance coefficient dimensions are inconsistent",
        ));
    }
    let mut value = 0.0;
    for (index, column) in nuisance.iter().enumerate() {
        if design.effects()[column.effect].codes()[row] == column.level {
            value += coefficients[index];
        }
    }
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid_data("CMG nuisance fitted value is nonfinite"))
    }
}

fn subtract_core_fit(
    design: &BorrowedInterceptFeDesign<'_>,
    core: [usize; 2],
    values: &mut DenseMatrix,
    column: usize,
    solution: &[f64],
) -> Result<()> {
    let vertices = core_vertices(design, core)?;
    if solution.len() != vertices {
        return Err(invalid_argument(
            "CMG solution and graph dimensions are inconsistent",
        ));
    }
    let left = design.effects()[core[0]].codes();
    let right = design.effects()[core[1]].codes();
    let right_offset = design.effects()[core[0]].n_levels();
    for row in 0..values.nrows() {
        let fitted = solution[left[row] as usize] - solution[right_offset + right[row] as usize];
        let residual = values[(row, column)] - fitted;
        if !residual.is_finite() {
            return Err(invalid_data("CMG projection produced a nonfinite residual"));
        }
        values[(row, column)] = residual;
    }
    Ok(())
}

fn core_vertices(design: &BorrowedInterceptFeDesign<'_>, core: [usize; 2]) -> Result<usize> {
    design.effects()[core[0]]
        .n_levels()
        .checked_add(design.effects()[core[1]].n_levels())
        .ok_or_else(|| allocation("CMG core vertex count overflows"))
}

fn validate_config(config: CmgConfig) -> Result<()> {
    if config.batch_size == 0
        || !config.map.tolerance.is_finite()
        || config.map.tolerance <= 0.0
        || config.map.tolerance >= 1.0
        || config.map.max_iterations == 0
    {
        return Err(invalid_argument("CMG numerical policy is invalid"));
    }
    Ok(())
}

fn validate_weights(weights: BorrowedMapWeights<'_>, nobs: usize) -> Result<f64> {
    match weights {
        BorrowedMapWeights::Unit => Ok(1.0),
        BorrowedMapWeights::Provided(values) => {
            if values.len() != nobs {
                return Err(invalid_argument("CMG weight and observation counts differ"));
            }
            if values
                .iter()
                .any(|weight| !weight.is_finite() || *weight <= 0.0)
            {
                return Err(invalid_data("CMG weights must be finite and positive"));
            }
            let scale = values.iter().copied().fold(0.0_f64, f64::max);
            if !scale.is_finite()
                || scale <= 0.0
                || values.iter().any(|weight| *weight / scale == 0.0)
            {
                return Err(invalid_data("CMG weight normalization failed"));
            }
            Ok(scale)
        }
    }
}

fn normalized_weight(weights: BorrowedMapWeights<'_>, scale: f64, row: usize) -> f64 {
    match weights {
        BorrowedMapWeights::Unit => 1.0,
        BorrowedMapWeights::Provided(values) => values[row] / scale,
    }
}

fn weighted_column_norms(
    values: &DenseMatrix,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
) -> Result<Vec<f64>> {
    let mut scales = zero_vec(values.ncols(), "unable to reserve CMG column scales")?;
    for row in 0..values.nrows() {
        for column in 0..values.ncols() {
            scales[column] = scales[column].max(values[(row, column)].abs());
        }
    }
    let mut sums = zero_vec(values.ncols(), "unable to reserve CMG column norm sums")?;
    let mut corrections = zero_vec(
        values.ncols(),
        "unable to reserve CMG column norm corrections",
    )?;
    for row in 0..values.nrows() {
        let weight = normalized_weight(weights, weight_scale, row);
        for column in 0..values.ncols() {
            if scales[column] != 0.0 {
                let scaled = values[(row, column)] / scales[column];
                compensated_add(
                    weight * scaled * scaled,
                    &mut sums[column],
                    &mut corrections[column],
                )?;
            }
        }
    }
    for column in 0..values.ncols() {
        sums[column] = scales[column] * (sums[column] + corrections[column]).sqrt();
        if !sums[column].is_finite() {
            return Err(invalid_data("CMG reference norm is nonfinite"));
        }
    }
    Ok(sums)
}

struct PivotedPsdFactor {
    dimension: usize,
    rank: usize,
    permutation: Vec<usize>,
    scales: Vec<f64>,
    lower: Vec<f64>,
}

impl PivotedPsdFactor {
    fn empty() -> Self {
        Self {
            dimension: 0,
            rank: 0,
            permutation: Vec::new(),
            scales: Vec::new(),
            lower: Vec::new(),
        }
    }

    fn retained_bytes(&self) -> usize {
        self.permutation
            .capacity()
            .saturating_mul(std::mem::size_of::<usize>())
            .saturating_add(
                self.scales
                    .capacity()
                    .saturating_mul(std::mem::size_of::<f64>()),
            )
            .saturating_add(
                self.lower
                    .capacity()
                    .saturating_mul(std::mem::size_of::<f64>()),
            )
    }

    fn factor(mut gram: Vec<f64>, dimension: usize, tolerance: f64) -> Result<Self> {
        if gram.len() != dimension.saturating_mul(dimension) {
            return Err(invalid_argument(
                "CMG nuisance factor dimensions are inconsistent",
            ));
        }
        let mut scales = zero_vec(dimension, "unable to reserve CMG nuisance scales")?;
        for index in 0..dimension {
            let diagonal = gram[index * dimension + index];
            if !diagonal.is_finite() {
                return Err(invalid_data("CMG nuisance diagonal is nonfinite"));
            }
            scales[index] = diagonal.max(0.0).sqrt();
        }
        for row in 0..dimension {
            for column in 0..dimension {
                gram[row * dimension + column] = if scales[row] == 0.0 || scales[column] == 0.0 {
                    0.0
                } else {
                    gram[row * dimension + column] / (scales[row] * scales[column])
                };
            }
        }
        let mut permutation: Vec<usize> = (0..dimension).collect();
        let mut lower = zero_vec(
            dimension.saturating_mul(dimension),
            "unable to reserve CMG nuisance factor",
        )?;
        let initial_pivot = (0..dimension)
            .map(|index| gram[index * dimension + index])
            .fold(0.0_f64, f64::max);
        let threshold = initial_pivot * tolerance.max(f64::EPSILON);
        let mut rank = 0;
        for step in 0..dimension {
            let mut pivot = step;
            let mut pivot_value = gram[step * dimension + step];
            for candidate in (step + 1)..dimension {
                let value = gram[candidate * dimension + candidate];
                if value > pivot_value {
                    pivot = candidate;
                    pivot_value = value;
                }
            }
            if pivot_value < -threshold {
                return Err(invalid_data(
                    "CMG nuisance Schur complement is materially indefinite",
                ));
            }
            if pivot_value <= threshold {
                break;
            }
            if pivot != step {
                swap_symmetric(&mut gram, dimension, step, pivot);
                permutation.swap(step, pivot);
                for column in 0..step {
                    lower.swap(step * dimension + column, pivot * dimension + column);
                }
            }
            let diagonal = gram[step * dimension + step].sqrt();
            if !diagonal.is_finite() || diagonal <= 0.0 {
                return Err(invalid_data("CMG nuisance factor pivot is invalid"));
            }
            lower[step * dimension + step] = diagonal;
            for row in (step + 1)..dimension {
                lower[row * dimension + step] = gram[row * dimension + step] / diagonal;
            }
            for row in (step + 1)..dimension {
                for column in row..dimension {
                    let updated = gram[column * dimension + row]
                        - lower[row * dimension + step] * lower[column * dimension + step];
                    gram[column * dimension + row] = updated;
                    gram[row * dimension + column] = updated;
                }
            }
            rank += 1;
        }
        Ok(Self {
            dimension,
            rank,
            permutation,
            scales,
            lower,
        })
    }

    fn solve(&self, rhs: &[f64]) -> Result<Vec<f64>> {
        if rhs.len() != self.dimension {
            return Err(invalid_argument(
                "CMG nuisance solve dimensions are inconsistent",
            ));
        }
        let mut output = zero_vec(self.dimension, "unable to reserve CMG nuisance solution")?;
        if self.rank == 0 {
            return Ok(output);
        }
        let mut work = zero_vec(self.rank, "unable to reserve CMG nuisance solve work")?;
        for row in 0..self.rank {
            let original = self.permutation[row];
            let scale = self.scales[original];
            if scale == 0.0 {
                return Err(invalid_data(
                    "CMG retained nuisance direction has zero scale",
                ));
            }
            let mut value = rhs[original] / scale;
            for (column, work_value) in work[..row].iter().copied().enumerate() {
                value -= self.lower[row * self.dimension + column] * work_value;
            }
            work[row] = value / self.lower[row * self.dimension + row];
        }
        for row in (0..self.rank).rev() {
            let mut value = work[row];
            for (column, work_value) in work
                .iter()
                .copied()
                .enumerate()
                .take(self.rank)
                .skip(row + 1)
            {
                value -= self.lower[column * self.dimension + row] * work_value;
            }
            work[row] = value / self.lower[row * self.dimension + row];
        }
        for (position, work_value) in work.iter().copied().enumerate().take(self.rank) {
            let original = self.permutation[position];
            output[original] = work_value / self.scales[original];
            if !output[original].is_finite() {
                return Err(invalid_data("CMG nuisance solution is nonfinite"));
            }
        }
        Ok(output)
    }
}

fn swap_symmetric(matrix: &mut [f64], dimension: usize, left: usize, right: usize) {
    if left == right {
        return;
    }
    for column in 0..dimension {
        matrix.swap(left * dimension + column, right * dimension + column);
    }
    for row in 0..dimension {
        matrix.swap(row * dimension + left, row * dimension + right);
    }
}

fn zero_vec(elements: usize, label: &'static str) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(elements)
        .map_err(|_| allocation(label))?;
    if values.capacity() < elements {
        return Err(allocation(label));
    }
    values.resize(elements, 0.0);
    Ok(values)
}

fn zero_matrix_vectors(rows: usize, columns: usize, label: &'static str) -> Result<Vec<Vec<f64>>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(rows)
        .map_err(|_| allocation(label))?;
    for _ in 0..rows {
        output.push(zero_vec(columns, label)?);
    }
    Ok(output)
}

fn validate_finite_vectors(values: &[Vec<f64>], message: &'static str) -> Result<()> {
    if values
        .iter()
        .flat_map(|value| value.iter())
        .any(|value| !value.is_finite())
    {
        Err(invalid_data(message))
    } else {
        Ok(())
    }
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
    if sum.is_finite() && correction.is_finite() {
        Ok(())
    } else {
        Err(invalid_data("CMG compensated accumulation is nonfinite"))
    }
}

fn cmg_error(error: cmg::CmgError) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, format!("CMG: {error}"))
}

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{absorb_borrowed_intercept_cg_certified, BorrowedInterceptEffect, LevelIndex};

    fn config(batch_size: usize) -> CmgConfig {
        CmgConfig {
            map: MapConfig {
                tolerance: 1e-10,
                max_iterations: 2_000,
                check_every: 2,
                symmetric: true,
            },
            threads: ThreadConfig::from_requested(1, true).expect("threads"),
            batch_size,
            retain_coefficients: false,
            requested_solver: FeSolverKind::Cmg,
            solver_memory_budget: None,
        }
    }

    fn effect<'a>(
        codes: &'a [u32],
        levels: usize,
        index: &'a LevelIndex,
    ) -> BorrowedInterceptEffect<'a> {
        BorrowedInterceptEffect::new(codes, levels, index).expect("borrowed effect")
    }

    fn assert_matrix_close(left: &DenseMatrix, right: &DenseMatrix, tolerance: f64) {
        assert_eq!((left.nrows(), left.ncols()), (right.nrows(), right.ncols()));
        let maximum = left
            .as_slice()
            .iter()
            .zip(right.as_slice())
            .map(|(left, right)| (left - right).abs())
            .fold(0.0_f64, f64::max);
        assert!(maximum <= tolerance, "maximum difference {maximum:e}");
    }

    fn assert_coefficients_reconstruct(
        input: &DenseMatrix,
        residualized: &DenseMatrix,
        effects: &[&[u32]],
        coefficients: &FeCoefficientBatch,
        tolerance: f64,
    ) {
        for rhs in 0..input.ncols() {
            let coefficient_rhs = coefficients.rhs(rhs).expect("coefficient RHS");
            for row in 0..input.nrows() {
                let fitted = effects
                    .iter()
                    .enumerate()
                    .map(|(effect, codes)| {
                        coefficient_rhs[coefficients.effect_offsets()[effect] + codes[row] as usize]
                    })
                    .sum::<f64>();
                let expected = input[(row, rhs)] - residualized[(row, rhs)];
                assert!(
                    (fitted - expected).abs() <= tolerance,
                    "retained CMG coefficients differ by {:e}",
                    (fitted - expected).abs()
                );
            }
        }
    }

    fn calibrated_panel_profile(
        periods: usize,
        workers: usize,
        firms: usize,
        nuisance_levels: Option<usize>,
        reverse_core_order: bool,
    ) -> CmgTopologyProfile {
        let rows = periods * workers;
        let worker: Vec<u32> = (0..rows).map(|row| (row / periods) as u32).collect();
        let firm: Vec<u32> = (0..rows)
            .map(|row| ((row / periods + row % periods) % firms) as u32)
            .collect();
        let worker_index = LevelIndex::from_codes(&worker, workers).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm, firms).expect("firm index");
        let mut effects = if reverse_core_order {
            vec![
                effect(&firm, firms, &firm_index),
                effect(&worker, workers, &worker_index),
            ]
        } else {
            vec![
                effect(&worker, workers, &worker_index),
                effect(&firm, firms, &firm_index),
            ]
        };
        let nuisance;
        let nuisance_index;
        if let Some(levels) = nuisance_levels {
            nuisance = (0..rows)
                .map(|row| (row % levels) as u32)
                .collect::<Vec<_>>();
            nuisance_index = LevelIndex::from_codes(&nuisance, levels).expect("nuisance index");
            effects.push(effect(&nuisance, levels, &nuisance_index));
        }
        let design = BorrowedInterceptFeDesign::new(rows, effects).expect("profile design");
        CmgTopologyProfile::analyze(&design).expect("CMG topology profile")
    }

    #[test]
    fn auto_route_matches_calibrated_akm_shape_boundaries() {
        let favorable = calibrated_panel_profile(3, 100, 10, Some(10), false);
        assert!(favorable.auto_favorable());
        assert_eq!(favorable.nuisance_columns(), 9);

        let short = calibrated_panel_profile(2, 100, 10, None, false);
        assert!(!short.auto_favorable());

        let balanced = calibrated_panel_profile(10, 100, 11, None, false);
        assert!(!balanced.auto_favorable());

        let nuisance = calibrated_panel_profile(10, 1_000, 100, Some(11), false);
        assert_eq!(nuisance.nuisance_columns(), 10);
        assert!(!nuisance.auto_favorable());
    }

    #[test]
    fn auto_route_is_invariant_to_core_effect_order() {
        let forward = calibrated_panel_profile(10, 100, 5, None, false);
        let reverse = calibrated_panel_profile(10, 100, 5, None, true);
        assert_eq!(forward.auto_favorable(), reverse.auto_favorable());
        assert_eq!(forward.vertices, reverse.vertices);
        assert_eq!(
            forward.highest_cardinality_levels,
            reverse.highest_cardinality_levels
        );
        assert_eq!(
            forward.second_cardinality_levels,
            reverse.second_cardinality_levels
        );
    }

    #[test]
    fn three_way_profile_uses_two_largest_effects_as_core_in_every_order() {
        let rows = 2_000;
        let worker_levels = 400;
        let firm_levels = 80;
        let year_levels = 5;
        let worker: Vec<u32> = (0..rows)
            .map(|row| (row / (rows / worker_levels)) as u32)
            .collect();
        let firm: Vec<u32> = (0..rows)
            .map(|row| ((row / 5 + row % 5) % firm_levels) as u32)
            .collect();
        let year: Vec<u32> = (0..rows).map(|row| (row % year_levels) as u32).collect();
        let wi = LevelIndex::from_codes(&worker, worker_levels).expect("worker index");
        let fi = LevelIndex::from_codes(&firm, firm_levels).expect("firm index");
        let yi = LevelIndex::from_codes(&year, year_levels).expect("year index");
        let sources = [
            (&worker, worker_levels, &wi),
            (&firm, firm_levels, &fi),
            (&year, year_levels, &yi),
        ];

        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let effects = order
                .iter()
                .map(|index| {
                    let (codes, levels, level_index) = sources[*index];
                    effect(codes, levels, level_index)
                })
                .collect();
            let design = BorrowedInterceptFeDesign::new(rows, effects).expect("profile design");
            let profile = CmgTopologyProfile::analyze(&design).expect("profile");
            assert_eq!(profile.highest_cardinality_levels, worker_levels);
            assert_eq!(profile.second_cardinality_levels, firm_levels);
            assert_eq!(profile.vertices, worker_levels + firm_levels);
            assert_eq!(profile.nuisance_columns(), year_levels - 1);
            let selected_original_effects = profile
                .core_effects
                .expect("two core effects")
                .map(|position| order[position]);
            assert_eq!(selected_original_effects, [0, 1]);
            assert_eq!(order[profile.effect_order[2]], 2);
        }
    }

    #[test]
    fn auto_requested_cmg_skips_map_pilot_and_certifies() {
        let periods = 8;
        let workers = 100;
        let firms = 10;
        let rows = periods * workers;
        let worker: Vec<u32> = (0..rows).map(|row| (row / periods) as u32).collect();
        let firm: Vec<u32> = (0..rows)
            .map(|row| ((row / periods + row % periods) % firms) as u32)
            .collect();
        let worker_index = LevelIndex::from_codes(&worker, workers).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm, firms).expect("firm index");
        let design = BorrowedInterceptFeDesign::new(
            rows,
            vec![
                effect(&worker, workers, &worker_index),
                effect(&firm, firms, &firm_index),
            ],
        )
        .expect("design");
        let profile = CmgTopologyProfile::analyze(&design).expect("profile");
        assert!(profile.auto_favorable());
        let mut auto_config = config(3);
        auto_config.requested_solver = FeSolverKind::Auto;
        let mut session = CmgAbsorber::build_profiled(
            design,
            BorrowedMapWeights::Unit,
            auto_config,
            false,
            profile,
        )
        .expect("auto CMG setup");
        let values = DenseMatrix::from_columns(&[(0..rows)
            .map(|row| (row as f64 * 0.031).sin() + (row as f64 * 0.017).cos())
            .collect()])
        .expect("values");
        let result = session.absorb(values).expect("auto CMG projection");
        assert!(result.report().certified());
        assert_eq!(result.report().requested_solver, FeSolverKind::Auto);
        assert_eq!(result.report().used_solver, FeSolverKind::Cmg);
        assert_eq!(result.report().pilot_iterations, 0);
    }

    #[test]
    fn auto_cmg_memory_preflight_rejects_before_graph_construction() {
        let worker = [0, 0, 1, 1, 2, 2, 3, 3];
        let firm = [0, 1, 0, 1, 0, 1, 0, 1];
        let worker_index = LevelIndex::from_codes(&worker, 4).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm, 2).expect("firm index");
        let design = BorrowedInterceptFeDesign::new(
            worker.len(),
            vec![
                effect(&worker, 4, &worker_index),
                effect(&firm, 2, &firm_index),
            ],
        )
        .expect("design");
        let profile = CmgTopologyProfile::analyze(&design).expect("profile");
        let mut constrained = config(1);
        constrained.requested_solver = FeSolverKind::Auto;
        constrained.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 1,
        });
        let error = match CmgAbsorber::build_profiled(
            design,
            BorrowedMapWeights::Unit,
            constrained,
            false,
            profile,
        ) {
            Ok(_) => panic!("CMG memory preflight unexpectedly fit"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::Allocation);
        assert!(error.message.contains("joint execution/solver memory plan"));
    }

    #[test]
    fn weighted_two_way_projection_agrees_with_certified_cg() {
        let worker = [0, 0, 1, 1, 2, 2, 0, 1, 2, 0, 1, 2];
        let firm = [0, 1, 0, 2, 1, 2, 2, 1, 0, 0, 1, 2];
        let worker_index = LevelIndex::from_codes(&worker, 3).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm, 3).expect("firm index");
        let design = BorrowedInterceptFeDesign::new(
            worker.len(),
            vec![
                effect(&worker, 3, &worker_index),
                effect(&firm, 3, &firm_index),
            ],
        )
        .expect("design");
        let reference_design = BorrowedInterceptFeDesign::new(
            worker.len(),
            vec![
                effect(&worker, 3, &worker_index),
                effect(&firm, 3, &firm_index),
            ],
        )
        .expect("reference design");
        let weights = [1.0, 1.5, 0.8, 2.0, 1.2, 0.7, 1.8, 1.1, 0.9, 1.4, 1.6, 1.3];
        let values = DenseMatrix::from_columns(&[
            (0..worker.len())
                .map(|row| (row as f64 * 0.37).sin())
                .collect(),
            (0..worker.len())
                .map(|row| (row as f64 * 0.23).cos())
                .collect(),
        ])
        .expect("values");
        let reference = absorb_borrowed_intercept_cg_certified(
            &reference_design,
            BorrowedMapWeights::Provided(&weights),
            values.clone(),
            config(2).map,
        )
        .expect("CG reference");
        let mut session = CmgAbsorber::build(
            design,
            BorrowedMapWeights::Provided(&weights),
            config(2),
            false,
        )
        .expect("CMG setup");
        let result = session.absorb(values).expect("CMG projection");
        let (result_values, report) = result.into_parts();
        let (reference_values, _) = reference.into_parts();
        assert_eq!(report.requested_solver, FeSolverKind::Cmg);
        assert_eq!(report.used_solver, FeSolverKind::Cmg);
        assert!(report.certified());
        assert_matrix_close(&result_values, &reference_values, 5e-9);
    }

    #[test]
    fn duplicate_matches_and_disconnected_components_are_supported() {
        let worker = [0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
        let firm = [0, 0, 1, 1, 2, 2, 3, 3, 1, 0, 3, 2];
        let worker_index = LevelIndex::from_codes(&worker, 4).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm, 4).expect("firm index");
        let design = BorrowedInterceptFeDesign::new(
            worker.len(),
            vec![
                effect(&worker, 4, &worker_index),
                effect(&firm, 4, &firm_index),
            ],
        )
        .expect("design");
        let values =
            DenseMatrix::from_columns(&[(0..worker.len()).map(|row| row as f64 - 3.0).collect()])
                .expect("values");
        let mut session = CmgAbsorber::build(design, BorrowedMapWeights::Unit, config(1), false)
            .expect("CMG setup");
        let result = session.absorb(values).expect("CMG projection");
        assert!(result.report().certified());
        assert_eq!(result.report().used_solver, FeSolverKind::Cmg);
    }

    #[test]
    fn multiple_rhs_calls_reuse_one_hierarchy() {
        let worker = [0, 0, 1, 1, 2, 2, 0, 1, 2];
        let firm = [0, 1, 0, 1, 1, 2, 2, 2, 0];
        let worker_index = LevelIndex::from_codes(&worker, 3).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm, 3).expect("firm index");
        let design = BorrowedInterceptFeDesign::new(
            worker.len(),
            vec![
                effect(&worker, 3, &worker_index),
                effect(&firm, 3, &firm_index),
            ],
        )
        .expect("design");
        let mut session = CmgAbsorber::build(design, BorrowedMapWeights::Unit, config(2), false)
            .expect("CMG setup");
        let first = session
            .absorb(
                DenseMatrix::from_columns(&[vec![1.0, 0.2, -0.4, 1.3, 2.1, -0.7, 0.9, 1.7, -1.2]])
                    .expect("first values"),
            )
            .expect("first projection");
        let second = session
            .absorb(
                DenseMatrix::from_columns(&[vec![-0.5, 0.1, 0.6, -1.0, 0.8, 1.4, -0.2, 0.7, 1.1]])
                    .expect("second values"),
            )
            .expect("second projection");
        assert!(first.report().setup_seconds >= 0.0);
        assert_eq!(second.report().setup_seconds, 0.0);
        assert!(first.report().certified() && second.report().certified());
    }

    #[test]
    fn one_effect_uses_exact_direct_projection_but_retains_cmg_request() {
        let group = [0, 0, 1, 1, 2, 2];
        let index = LevelIndex::from_codes(&group, 3).expect("group index");
        let design = BorrowedInterceptFeDesign::new(group.len(), vec![effect(&group, 3, &index)])
            .expect("design");
        let mut session = CmgAbsorber::build(design, BorrowedMapWeights::Unit, config(2), false)
            .expect("direct setup");
        let result = session
            .absorb(
                DenseMatrix::from_columns(&[vec![1.0, 3.0, 2.0, 6.0, -1.0, 5.0]]).expect("values"),
            )
            .expect("direct projection");
        let (values, report) = result.into_parts();
        assert_eq!(report.requested_solver, FeSolverKind::Cmg);
        assert_eq!(report.used_solver, FeSolverKind::Map);
        assert!(report.certified());
        assert_eq!(values.as_slice(), &[-1.0, 1.0, -2.0, 2.0, -3.0, 3.0]);
    }

    #[test]
    fn absorbed_nuisance_effect_agrees_with_full_multi_effect_cg() {
        let worker = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 0, 1, 2, 3, 4];
        let firm = [0, 1, 0, 2, 1, 3, 2, 3, 0, 1, 2, 3, 0, 1, 2];
        let year = [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2];
        let wi = LevelIndex::from_codes(&worker, 5).expect("worker index");
        let fi = LevelIndex::from_codes(&firm, 4).expect("firm index");
        let yi = LevelIndex::from_codes(&year, 3).expect("year index");
        let make_design = || {
            BorrowedInterceptFeDesign::new(
                worker.len(),
                vec![
                    effect(&worker, 5, &wi),
                    effect(&firm, 4, &fi),
                    effect(&year, 3, &yi),
                ],
            )
            .expect("design")
        };
        let values = DenseMatrix::from_columns(&[
            (0..worker.len())
                .map(|row| (row as f64 * 0.41).sin() + row as f64 * 0.03)
                .collect(),
            (0..worker.len())
                .map(|row| (row as f64 * 0.17).cos())
                .collect(),
        ])
        .expect("values");
        let reference = absorb_borrowed_intercept_cg_certified(
            &make_design(),
            BorrowedMapWeights::Unit,
            values.clone(),
            config(2).map,
        )
        .expect("CG reference");
        let mut retaining_config = config(2);
        retaining_config.retain_coefficients = true;
        let mut session = CmgAbsorber::build(
            make_design(),
            BorrowedMapWeights::Unit,
            retaining_config,
            false,
        )
        .expect("CMG setup");
        let result = session.absorb(values.clone()).expect("CMG projection");
        let (result_values, report, coefficients) = result.into_full_parts();
        let (reference_values, _) = reference.into_parts();
        assert!(report.certified());
        assert_matrix_close(&result_values, &reference_values, 5e-9);
        let coefficients = coefficients.expect("certified CMG coefficient product");
        assert_coefficients_reconstruct(
            &values,
            &result_values,
            &[&worker, &firm, &year],
            &coefficients,
            5e-9,
        );
    }

    #[test]
    fn nested_rank_deficient_nuisance_effect_is_handled() {
        let worker = [0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
        let firm = [0, 1, 0, 2, 1, 2, 0, 1, 2, 2, 0, 1];
        let nested = [0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1];
        let wi = LevelIndex::from_codes(&worker, 4).expect("worker index");
        let fi = LevelIndex::from_codes(&firm, 3).expect("firm index");
        let ni = LevelIndex::from_codes(&nested, 2).expect("nested index");
        let design = BorrowedInterceptFeDesign::new(
            worker.len(),
            vec![
                effect(&worker, 4, &wi),
                effect(&firm, 3, &fi),
                effect(&nested, 2, &ni),
            ],
        )
        .expect("design");
        let values = DenseMatrix::from_columns(&[(0..worker.len())
            .map(|row| (row as f64 * 0.29).sin())
            .collect()])
        .expect("values");
        let mut session = CmgAbsorber::build(design, BorrowedMapWeights::Unit, config(1), false)
            .expect("CMG setup");
        let result = session.absorb(values).expect("CMG projection");
        assert!(result.report().certified());
    }

    #[test]
    fn more_than_sixty_four_nuisance_columns_are_rejected() {
        let rows = 134;
        let worker: Vec<u32> = (0..rows).map(|row| (row % 67) as u32).collect();
        let firm: Vec<u32> = (0..rows)
            .map(|row| ((row + row / 67) % 67) as u32)
            .collect();
        let nuisance: Vec<u32> = (0..rows).map(|row| (row % 66) as u32).collect();
        let wi = LevelIndex::from_codes(&worker, 67).expect("worker index");
        let fi = LevelIndex::from_codes(&firm, 67).expect("firm index");
        let ni = LevelIndex::from_codes(&nuisance, 66).expect("nuisance index");
        let design = BorrowedInterceptFeDesign::new(
            rows,
            vec![
                effect(&worker, 67, &wi),
                effect(&firm, 67, &fi),
                effect(&nuisance, 66, &ni),
            ],
        )
        .expect("design");
        let error = match CmgAbsorber::build(design, BorrowedMapWeights::Unit, config(1), false) {
            Ok(_) => panic!("nuisance limit was not enforced"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(error.message.contains("at most 64"));
    }
}
