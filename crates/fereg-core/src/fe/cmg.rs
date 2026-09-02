//! Certified CMG-PCG absorption for sparse AKM-style categorical designs.
//!
//! Two high-cardinality intercept effects define a weighted bipartite graph.
//! Additional small intercept effects are eliminated through a dense Schur
//! complement. One graph hierarchy and workspace pool are retained for the
//! complete prepared OLS fit, including low-memory blocks and residual polish.

use std::time::Instant;

use cmg::{
    solve_pcg_batch_into_with_executor, solve_pcg_batch_into_with_plan_and_workspace,
    solve_pcg_batch_into_with_workspace, CmgMemoryEstimate, CmgOptions, CmgPreconditioner,
    CmgProblemSize, Components, Laplacian, ParallelExecutor, ParallelOptions, ParallelPcgExecution,
    ParallelPcgPolicy, ParallelPcgSolver, ParallelPcgWorkspace, PcgBatchMut, PcgBatchRef,
    PcgBatchWorkspace, PcgDiagnostics, PcgOptions, PreparedLaplacianTopology,
    PreparedLaplacianWorkspace, RepeatedPcgMemoryEstimate, RepeatedPcgMemoryReport, TerminalReason,
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

pub(crate) const MAX_NUISANCE_COLUMNS: usize = 64;
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
/// Fresh component-mixture, RHS-width, and thread holdouts put every measured
/// iterative-terminal CMG win at or below 2.30 finest-level passes and the
/// first loss at 5.68. Four is a conservative development safety boundary;
/// direct terminals are deliberately outside this hierarchy-only veto.
const AUTO_MAX_ITERATIVE_HIERARCHY_WORK_OVER_FINEST: f64 = 4.0;
/// A final direct-terminal calibration found a conservative subset where the
/// already-built CMG hierarchy predicts a reliable direct-CG win. These work
/// units are deterministic hierarchy dimensions, not elapsed-time probes.
const AUTO_DIRECT_MAX_DEPTH_WORK_PER_OBSERVATION: u128 = 100;
const AUTO_DIRECT_MAX_NUISANCE_WORK: u128 = 300_000;
const AUTO_DIRECT_MAX_EFFECTIVE_RHS_BATCHES: usize = 5;
const AUTO_DIRECT_MIN_WIDE_FACTOR_RHS_BATCHES: usize = 2;
const AUTO_DIRECT_MAX_WIDE_FACTOR_WORK: u128 = 750_000;
const AUTO_DIRECT_CG_MIN_DEPTH_WORK_PER_OBSERVATION: u128 = 200;
const AUTO_DIRECT_CG_MIN_NUISANCE_COLUMNS: usize = 9;

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
        self.pcg_options_at(
            (self.map.tolerance * PCG_TOLERANCE_MULTIPLIER).max(PCG_TOLERANCE_FLOOR),
        )
    }

    fn pcg_options_at(self, relative_tolerance: f64) -> PcgOptions {
        PcgOptions {
            relative_tolerance,
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

    pub(crate) const fn nuisance_columns(&self) -> usize {
        self.nuisance_columns
    }
}

/// Diagnostics from the CMG graph projection before any observation-space CG
/// polish. This seam is deliberately crate-private: PPML uses it only after
/// independently certifying every projected right-hand side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CmgUnpolishedProjectionReport {
    pub(crate) iterations: usize,
    pub(crate) backward_error: f64,
    pub(crate) setup_seconds: f64,
    pub(crate) solve_seconds: f64,
    pub(crate) right_hand_sides: usize,
    pub(crate) batches: usize,
    pub(crate) nuisance_columns: usize,
}

/// PPML-owned structural graph state reused while IRLS changes only weights.
/// OLS deliberately does not use this seam, so its established construction
/// and owned-result solve behavior remain unchanged.
pub(crate) struct PreparedCmgTopology {
    profile: CmgTopologyProfile,
    topology: PreparedLaplacianTopology,
    assembly_workspace: PreparedLaplacianWorkspace,
    numeric_weights: Vec<f64>,
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

/// Private outcome of post-topology automatic CMG setup. A rejected hierarchy
/// is a planned route decision, not a numerical fallback or setup failure.
pub(crate) enum CmgAutoBuildOutcome<'a> {
    Ready(CmgAbsorber<'a>),
    HierarchyRejected { setup_seconds: f64 },
    DirectCgPreferred { setup_seconds: f64 },
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

enum GraphBuildOutcome {
    Ready(GraphCmg),
    HierarchyRejected,
    DirectCgPreferred,
}

struct GraphCmg {
    core_effects: [usize; 2],
    nuisance: Vec<NuisanceColumn>,
    nuisance_factor: PivotedPsdFactor,
    solver: ParallelPcgSolver,
    workspace: GraphCmgWorkspace,
    setup_iterations: usize,
    setup_backward_error: f64,
    setup_right_hand_sides: usize,
    setup_batches: usize,
}

enum GraphCmgWorkspace {
    OwnedResults(ParallelPcgWorkspace),
    CallerBuffers(CallerPcgBuffers),
}

struct CallerPcgBuffers {
    right_hand_sides: Vec<f64>,
    solutions: Vec<f64>,
    diagnostics: Vec<PcgDiagnostics>,
    workspaces: Vec<PcgBatchWorkspace>,
    planned_used: bool,
}

impl CallerPcgBuffers {
    fn new(solver: &ParallelPcgSolver, batch_size: usize) -> Result<Self> {
        let vertices = solver.graph().vertex_count();
        let elements = vertices
            .checked_mul(batch_size)
            .ok_or_else(|| allocation("prepared CMG caller-buffer dimensions overflow"))?;
        let report = solver
            .select_batch_execution(batch_size)
            .map_err(cmg_error)?;
        let workspace_count = report.concurrency().max(1);
        let mut diagnostics = Vec::new();
        diagnostics
            .try_reserve_exact(batch_size)
            .map_err(|_| allocation("unable to reserve prepared CMG diagnostics"))?;
        diagnostics.resize(batch_size, PcgDiagnostics::default());
        let mut workspaces = Vec::new();
        workspaces
            .try_reserve_exact(workspace_count)
            .map_err(|_| allocation("unable to reserve prepared CMG PCG workspace pool"))?;
        for _ in 0..workspace_count {
            workspaces.push(PcgBatchWorkspace::new(solver.preconditioner()).map_err(cmg_error)?);
        }
        Ok(Self {
            right_hand_sides: zero_vec(
                elements,
                "unable to reserve prepared CMG right-hand-side buffers",
            )?,
            solutions: zero_vec(elements, "unable to reserve prepared CMG solution buffers")?,
            diagnostics,
            workspaces,
            planned_used: false,
        })
    }

    fn solve(
        &mut self,
        solver: &ParallelPcgSolver,
        rhs_count: usize,
        options: PcgOptions,
    ) -> Result<()> {
        let vertices = solver.graph().vertex_count();
        let elements = vertices
            .checked_mul(rhs_count)
            .ok_or_else(|| allocation("prepared CMG solve dimensions overflow"))?;
        if elements > self.right_hand_sides.len()
            || elements > self.solutions.len()
            || rhs_count > self.diagnostics.len()
        {
            return Err(invalid_argument(
                "prepared CMG solve exceeds its caller-buffer capacity",
            ));
        }
        let report = solver
            .select_batch_execution(rhs_count)
            .map_err(cmg_error)?;
        if report.concurrency() > self.workspaces.len() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "prepared CMG solve exceeds its fixed workspace pool",
            ));
        }
        let rhs = PcgBatchRef::contiguous(&self.right_hand_sides[..elements], rhs_count, vertices)
            .map_err(cmg_error)?;
        let solutions =
            PcgBatchMut::contiguous(&mut self.solutions[..elements], rhs_count, vertices)
                .map_err(cmg_error)?;
        let diagnostics = &mut self.diagnostics[..rhs_count];
        match report.execution() {
            ParallelPcgExecution::Serial => solve_pcg_batch_into_with_workspace(
                solver.graph(),
                solver.preconditioner(),
                rhs,
                None,
                solutions,
                diagnostics,
                options,
                &mut self.workspaces[0],
            ),
            ParallelPcgExecution::Planned => {
                self.planned_used = true;
                solve_pcg_batch_into_with_plan_and_workspace(
                    solver.graph(),
                    solver.preconditioner(),
                    solver.try_plan().map_err(cmg_error)?,
                    rhs,
                    None,
                    solutions,
                    diagnostics,
                    options,
                    &mut self.workspaces[0],
                    solver.executor(),
                )
            }
            ParallelPcgExecution::AcrossRightHandSides => solve_pcg_batch_into_with_executor(
                solver.graph(),
                solver.preconditioner(),
                rhs,
                None,
                solutions,
                diagnostics,
                options,
                &mut self.workspaces[..report.concurrency()],
                solver.executor(),
            ),
        }
        .map_err(cmg_solve_error)
    }

    fn solution(&self, rhs: usize, vertices: usize) -> Result<&[f64]> {
        let start = rhs
            .checked_mul(vertices)
            .ok_or_else(|| allocation("prepared CMG solution offset overflows"))?;
        let end = start
            .checked_add(vertices)
            .ok_or_else(|| allocation("prepared CMG solution range overflows"))?;
        self.solutions
            .get(start..end)
            .ok_or_else(|| invalid_argument("prepared CMG solution is out of bounds"))
    }

    fn caller_retained_bytes(&self) -> usize {
        self.right_hand_sides
            .capacity()
            .saturating_add(self.solutions.capacity())
            .saturating_mul(std::mem::size_of::<f64>())
            .saturating_add(
                self.diagnostics
                    .capacity()
                    .saturating_mul(std::mem::size_of::<PcgDiagnostics>()),
            )
    }
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
    right_hand_sides: usize,
    batches: usize,
}

/// Private deterministic cost summary available immediately after CMG
/// hierarchy construction. It deliberately depends only on retained hierarchy
/// dimensions and effective recursive repeat counts, so the automatic safety
/// gate can inspect it before nuisance-factor or PCG work begins.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CmgHierarchyWork {
    estimated_recursive_work: u128,
    finest_work: u128,
    work_over_finest: f64,
    deepest_visits: u128,
    maximum_repeat: usize,
}

impl CmgHierarchyWork {
    fn from_levels(
        vertex_counts: &[usize],
        matrix_nonzeros: &[usize],
        repeat_counts: &[usize],
    ) -> Self {
        debug_assert_eq!(vertex_counts.len(), matrix_nonzeros.len());
        debug_assert_eq!(vertex_counts.len(), repeat_counts.len());

        let finest_work = vertex_counts
            .first()
            .copied()
            .unwrap_or(0)
            .saturating_add(matrix_nonzeros.first().copied().unwrap_or(0))
            as u128;
        let mut visits = 1_u128;
        let mut deepest_visits = visits;
        let mut estimated_recursive_work = 0_u128;
        let mut maximum_repeat = 0_usize;
        for (level, (&vertices, &nonzeros)) in vertex_counts.iter().zip(matrix_nonzeros).enumerate()
        {
            let level_work = vertices.saturating_add(nonzeros) as u128;
            estimated_recursive_work =
                estimated_recursive_work.saturating_add(visits.saturating_mul(level_work));
            if level + 1 < vertex_counts.len() {
                let repeat = repeat_counts.get(level).copied().unwrap_or(1).max(1);
                maximum_repeat = maximum_repeat.max(repeat);
                visits = visits.saturating_mul(repeat as u128);
                deepest_visits = deepest_visits.max(visits);
            }
        }
        let work_over_finest = if finest_work == 0 {
            f64::INFINITY
        } else {
            estimated_recursive_work as f64 / finest_work as f64
        };
        Self {
            estimated_recursive_work,
            finest_work,
            work_over_finest,
            deepest_visits,
            maximum_repeat,
        }
    }
}

impl CmgSolveMetrics {
    fn include(&mut self, iterations: usize, backward_error: f64) {
        self.iterations = self.iterations.max(iterations);
        self.backward_error = self.backward_error.max(backward_error);
    }

    fn include_batch(&mut self, right_hand_sides: usize) {
        self.right_hand_sides = self.right_hand_sides.saturating_add(right_hand_sides);
        self.batches = self.batches.saturating_add(1);
    }
}

impl PreparedCmgTopology {
    pub(crate) fn prepare(design: &BorrowedInterceptFeDesign<'_>) -> Result<Self> {
        let profile = CmgTopologyProfile::analyze(design)?;
        validate_nuisance_limit(&profile)?;
        let core = profile.core_effects.ok_or_else(|| {
            invalid_argument("prepared CMG topology requires two absorbed effects")
        })?;
        let left_levels = design.effects()[core[0]].n_levels();
        let vertices = left_levels
            .checked_add(design.effects()[core[1]].n_levels())
            .ok_or_else(|| allocation("prepared CMG graph vertex count overflows"))?;
        let left_codes = design.effects()[core[0]].codes();
        let right_codes = design.effects()[core[1]].codes();
        let endpoints = (0..design.nobs()).map(|row| {
            (
                left_codes[row] as usize,
                left_levels + right_codes[row] as usize,
            )
        });
        let topology =
            PreparedLaplacianTopology::prepare(vertices, endpoints).map_err(cmg_error)?;
        let assembly_workspace = topology.workspace().map_err(cmg_error)?;
        let numeric_weights = zero_vec(
            design.nobs(),
            "unable to reserve prepared CMG numeric weights",
        )?;
        Ok(Self {
            profile,
            topology,
            assembly_workspace,
            numeric_weights,
        })
    }

    fn assemble(
        &mut self,
        profile: &CmgTopologyProfile,
        weights: BorrowedMapWeights<'_>,
        weight_scale: f64,
    ) -> Result<Laplacian> {
        if profile != &self.profile || self.numeric_weights.len() != profile.observations {
            return Err(invalid_argument(
                "prepared CMG topology does not match the current absorbed design",
            ));
        }
        for (row, numeric_weight) in self.numeric_weights.iter_mut().enumerate() {
            *numeric_weight = normalized_weight(weights, weight_scale, row);
        }
        self.topology
            .assemble_with_workspace(&self.numeric_weights, &mut self.assembly_workspace)
            .map_err(cmg_error)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.topology
            .retained_bytes()
            .saturating_add(self.assembly_workspace.byte_len())
            .saturating_add(
                self.numeric_weights
                    .capacity()
                    .saturating_mul(std::mem::size_of::<f64>()),
            )
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

    /// Build the PPML-only unpolished session from a topology prepared before
    /// the first IRLS frame. Current weights still rebuild the numeric graph
    /// and hierarchy, while endpoint sorting and component discovery are
    /// reused. Caller-owned batch buffers avoid owned CMG result allocation.
    pub(crate) fn build_unpolished_prepared(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'a>,
        config: CmgConfig,
        prepared: &mut PreparedCmgTopology,
    ) -> Result<Self> {
        let profile = CmgTopologyProfile::analyze(&design)?;
        match Self::build_profiled_internal(
            design,
            weights,
            config,
            false,
            profile,
            false,
            None,
            false,
            Some(prepared),
        )? {
            CmgAutoBuildOutcome::Ready(session) => Ok(session),
            CmgAutoBuildOutcome::HierarchyRejected { .. }
            | CmgAutoBuildOutcome::DirectCgPreferred { .. } => {
                unreachable!("unguarded prepared CMG setup cannot reject a hierarchy")
            }
        }
    }

    pub(crate) fn build_profiled(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'a>,
        config: CmgConfig,
        structural_constant_if_empty: bool,
        profile: CmgTopologyProfile,
    ) -> Result<Self> {
        match Self::build_profiled_internal(
            design,
            weights,
            config,
            structural_constant_if_empty,
            profile,
            false,
            None,
            true,
            None,
        )? {
            CmgAutoBuildOutcome::Ready(session) => Ok(session),
            CmgAutoBuildOutcome::HierarchyRejected { .. }
            | CmgAutoBuildOutcome::DirectCgPreferred { .. } => {
                unreachable!("unguarded CMG setup cannot reject a hierarchy")
            }
        }
    }

    pub(crate) fn build_profiled_auto_guarded(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'a>,
        config: CmgConfig,
        structural_constant_if_empty: bool,
        profile: CmgTopologyProfile,
        initial_rhs_columns: usize,
    ) -> Result<CmgAutoBuildOutcome<'a>> {
        if config.requested_solver != FeSolverKind::Auto {
            return Err(invalid_argument(
                "automatic CMG hierarchy guard requires solver(auto)",
            ));
        }
        if initial_rhs_columns == 0 {
            return Err(invalid_argument(
                "automatic CMG hierarchy guard requires at least one right-hand side",
            ));
        }
        Self::build_profiled_internal(
            design,
            weights,
            config,
            structural_constant_if_empty,
            profile,
            true,
            Some(initial_rhs_columns),
            true,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_profiled_internal(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'a>,
        config: CmgConfig,
        structural_constant_if_empty: bool,
        profile: CmgTopologyProfile,
        apply_auto_hierarchy_guard: bool,
        initial_rhs_columns: Option<usize>,
        retain_setup_failure: bool,
        prepared: Option<&mut PreparedCmgTopology>,
    ) -> Result<CmgAutoBuildOutcome<'a>> {
        validate_config(config)?;
        let weight_scale = validate_weights(weights, design.nobs())?;
        let setup_started = Instant::now();
        let route = if design.effects().len() < 2 {
            CmgRoute::Direct
        } else {
            validate_nuisance_limit(&profile)?;
            let graph = if let Some(prepared) = prepared {
                if apply_auto_hierarchy_guard || initial_rhs_columns.is_some() {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "prepared PPML CMG cannot use the OLS automatic hierarchy guard",
                    ));
                }
                preflight_prepared_cmg_memory(&design, &profile, config, prepared).and_then(|()| {
                    build_prepared_graph_route(
                        &design,
                        weights,
                        weight_scale,
                        config,
                        &profile,
                        prepared,
                    )
                })
            } else {
                preflight_cmg_memory(&design, &profile, config).and_then(|()| {
                    build_graph_route(
                        &design,
                        weights,
                        weight_scale,
                        config,
                        &profile,
                        apply_auto_hierarchy_guard,
                        initial_rhs_columns,
                    )
                })
            };
            match graph {
                Ok(GraphBuildOutcome::Ready(graph)) => CmgRoute::Graph(graph),
                Ok(GraphBuildOutcome::HierarchyRejected) => {
                    return Ok(CmgAutoBuildOutcome::HierarchyRejected {
                        setup_seconds: setup_started.elapsed().as_secs_f64(),
                    });
                }
                Ok(GraphBuildOutcome::DirectCgPreferred) => {
                    return Ok(CmgAutoBuildOutcome::DirectCgPreferred {
                        setup_seconds: setup_started.elapsed().as_secs_f64(),
                    });
                }
                Err(error)
                    if config.requested_solver == FeSolverKind::Auto || !retain_setup_failure =>
                {
                    return Err(error);
                }
                Err(error) => CmgRoute::Failed(error.to_string()),
            }
        };
        let pending_setup_metrics = match &route {
            CmgRoute::Graph(graph) => CmgSolveMetrics {
                iterations: graph.setup_iterations,
                backward_error: graph.setup_backward_error,
                right_hand_sides: graph.setup_right_hand_sides,
                batches: graph.setup_batches,
            },
            CmgRoute::Direct | CmgRoute::Failed(_) => CmgSolveMetrics::default(),
        };
        Ok(CmgAutoBuildOutcome::Ready(Self {
            design,
            weights,
            weight_scale,
            config,
            route,
            pending_setup_seconds: setup_started.elapsed().as_secs_f64(),
            pending_setup_metrics,
            structural_constant_if_empty,
        }))
    }

    /// Apply only the graph/Schur projection. No observation-space CG polish
    /// or route fallback is permitted here; callers must independently
    /// certify the returned residualized columns before using them.
    pub(crate) fn project_unpolished(
        &mut self,
        values: &mut DenseMatrix,
        relative_tolerance: f64,
    ) -> Result<CmgUnpolishedProjectionReport> {
        if values.nrows() != self.design.nobs() || values.ncols() == 0 {
            return Err(invalid_argument(
                "CMG unpolished design and matrix dimensions are inconsistent",
            ));
        }
        if values.as_slice().iter().any(|value| !value.is_finite()) {
            return Err(invalid_data("CMG unpolished input values must be finite"));
        }
        if !relative_tolerance.is_finite() || relative_tolerance <= 0.0 {
            return Err(invalid_argument(
                "CMG unpolished tolerance must be finite and positive",
            ));
        }
        let setup_seconds = std::mem::take(&mut self.pending_setup_seconds);
        let mut metrics = std::mem::take(&mut self.pending_setup_metrics);
        let nuisance_columns = match &self.route {
            CmgRoute::Graph(graph) => graph.nuisance.len(),
            CmgRoute::Direct => {
                return Err(FeregError::new(
                    ErrorCode::Unsupported,
                    "CMG unpolished projection requires at least two absorbed effects",
                ));
            }
            CmgRoute::Failed(message) => {
                return Err(nonconvergence(format!("CMG setup failed: {message}")));
            }
        };
        let solve_started = Instant::now();
        let (projected, _) = match &mut self.route {
            CmgRoute::Graph(graph) => graph.project_matrix_with_options(
                &self.design,
                self.weights,
                self.weight_scale,
                values,
                self.config,
                self.config.pcg_options_at(relative_tolerance),
            ),
            CmgRoute::Direct | CmgRoute::Failed(_) => unreachable!("CMG route checked above"),
        }?;
        metrics.include_from(projected);
        Ok(CmgUnpolishedProjectionReport {
            iterations: metrics.iterations,
            backward_error: metrics.backward_error,
            setup_seconds,
            solve_seconds: solve_started.elapsed().as_secs_f64(),
            right_hand_sides: metrics.right_hand_sides,
            batches: metrics.batches,
            nuisance_columns,
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
        self.project_matrix_with_options(
            design,
            weights,
            weight_scale,
            values,
            config,
            config.pcg_options(),
        )
    }

    fn project_matrix_with_options(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        weight_scale: f64,
        values: &mut DenseMatrix,
        config: CmgConfig,
        pcg_options: PcgOptions,
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
                pcg_options,
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
                    pcg_options,
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
            pcg_options,
        } = request;
        let mut metrics = CmgSolveMetrics::default();
        for start in (0..values.ncols()).step_by(config.batch_size) {
            let end = start.saturating_add(config.batch_size).min(values.ncols());
            match &mut self.workspace {
                GraphCmgWorkspace::OwnedResults(workspace) => {
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
                        .solve_batch_with_workspace(&right_hand_sides, pcg_options, workspace)
                        .map_err(cmg_solve_error)?;
                    metrics.include_batch(solved.results().len());
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
                GraphCmgWorkspace::CallerBuffers(caller) => {
                    let width = end - start;
                    let vertices = self.solver.graph().vertex_count();
                    fill_value_rhs_batch_into(
                        &mut caller.right_hand_sides,
                        vertices,
                        design,
                        self.core_effects,
                        weights,
                        weight_scale,
                        values,
                        start,
                        end,
                    )?;
                    caller.solve(&self.solver, width, pcg_options)?;
                    metrics.include_batch(width);
                    for local in 0..width {
                        let diagnostic = caller.diagnostics[local];
                        metrics.include(diagnostic.iterations(), diagnostic.backward_error());
                        let solution = caller.solution(local, vertices)?;
                        if let Some(batch) = coefficients.as_deref_mut() {
                            record_core_solution(
                                design,
                                self.core_effects,
                                batch,
                                destination_rhs_start + start + local,
                                coefficient_scale,
                                solution,
                            )?;
                        }
                        subtract_core_fit(
                            design,
                            self.core_effects,
                            values,
                            start + local,
                            solution,
                        )?;
                    }
                }
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
    pcg_options: PcgOptions,
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
        self.right_hand_sides = self.right_hand_sides.saturating_add(other.right_hand_sides);
        self.batches = self.batches.saturating_add(other.batches);
    }
}

fn build_graph_route(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    config: CmgConfig,
    profile: &CmgTopologyProfile,
    apply_auto_hierarchy_guard: bool,
    initial_rhs_columns: Option<usize>,
) -> Result<GraphBuildOutcome> {
    let trace_enabled = std::env::var_os("FEREG_SOLVER_TRACE").is_some();
    let core_effects = profile
        .core_effects
        .ok_or_else(|| invalid_argument("CMG graph route requires two absorbed effects"))?;
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
    let graph_started = trace_enabled.then(Instant::now);
    let graph =
        Laplacian::from_edges_with_executor(vertices, edges, &executor).map_err(cmg_error)?;
    let graph_seconds = graph_started.map_or(0.0, |started| started.elapsed().as_secs_f64());
    build_graph_route_from_numeric(
        design,
        weights,
        weight_scale,
        config,
        profile,
        graph,
        executor,
        apply_auto_hierarchy_guard,
        initial_rhs_columns,
        None,
        trace_enabled,
        graph_seconds,
    )
}

fn build_prepared_graph_route(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    config: CmgConfig,
    profile: &CmgTopologyProfile,
    prepared: &mut PreparedCmgTopology,
) -> Result<GraphBuildOutcome> {
    let trace_enabled = std::env::var_os("FEREG_SOLVER_TRACE").is_some();
    let parallel = ParallelOptions {
        threads: config.threads.resolved(),
        ..ParallelOptions::default()
    };
    let executor = ParallelExecutor::new(parallel).map_err(cmg_error)?;
    let graph_started = trace_enabled.then(Instant::now);
    let graph = prepared.assemble(profile, weights, weight_scale)?;
    let graph_seconds = graph_started.map_or(0.0, |started| started.elapsed().as_secs_f64());
    build_graph_route_from_numeric(
        design,
        weights,
        weight_scale,
        config,
        profile,
        graph,
        executor,
        false,
        None,
        Some(prepared),
        trace_enabled,
        graph_seconds,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_graph_route_from_numeric(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    config: CmgConfig,
    profile: &CmgTopologyProfile,
    graph: Laplacian,
    executor: ParallelExecutor,
    apply_auto_hierarchy_guard: bool,
    initial_rhs_columns: Option<usize>,
    prepared: Option<&PreparedCmgTopology>,
    trace_enabled: bool,
    graph_seconds: f64,
) -> Result<GraphBuildOutcome> {
    let core_effects = profile
        .core_effects
        .ok_or_else(|| invalid_argument("CMG graph route requires two absorbed effects"))?;
    let hierarchy_started = trace_enabled.then(Instant::now);
    let preconditioner =
        CmgPreconditioner::build_with_executor(&graph, CmgOptions::default(), &executor)
            .map_err(cmg_error)?;
    let hierarchy_seconds =
        hierarchy_started.map_or(0.0, |started| started.elapsed().as_secs_f64());
    let hierarchy = preconditioner.hierarchy();
    let report = hierarchy.report();
    let work = CmgHierarchyWork::from_levels(
        report.vertex_counts(),
        report.matrix_nonzeros(),
        preconditioner.repeat_counts(),
    );
    if trace_enabled {
        trace_cmg_hierarchy(
            &graph,
            &preconditioner,
            work,
            graph_seconds,
            hierarchy_seconds,
        );
    }
    if apply_auto_hierarchy_guard
        && auto_hierarchy_guard_rejects(report.terminal_reason(), work.work_over_finest)
    {
        if trace_enabled {
            eprintln!(
                "FEREG_CMG_GUARD|decision=reject|terminal={:?}|work_over_finest={:.17e}|threshold={:.17e}",
                report.terminal_reason(),
                work.work_over_finest,
                AUTO_MAX_ITERATIVE_HIERARCHY_WORK_OVER_FINEST,
            );
        }
        return Ok(GraphBuildOutcome::HierarchyRejected);
    }
    let terminal_factor_dimension = preconditioner
        .terminal_factor()
        .map_or(0, cmg::GroundedLdl::active_dimension);
    if apply_auto_hierarchy_guard
        && auto_direct_terminal_cg_preferred(
            report.terminal_reason(),
            work,
            report.vertex_counts().len(),
            design.nobs(),
            profile.nuisance_columns,
            initial_rhs_columns.expect("guarded CMG setup has an initial RHS width"),
            config.threads.resolved(),
            terminal_factor_dimension,
        )
    {
        if trace_enabled {
            eprintln!(
                "FEREG_CMG_GUARD|decision=direct_cg|terminal={:?}|levels={}|recursive_work={}|observations={}|nuisance_columns={}|rhs={}|threads={}|terminal_factor_dimension={terminal_factor_dimension}",
                report.terminal_reason(),
                report.vertex_counts().len(),
                work.estimated_recursive_work,
                design.nobs(),
                profile.nuisance_columns,
                initial_rhs_columns.expect("guarded CMG setup has an initial RHS width"),
                config.threads.resolved(),
            );
        }
        return Ok(GraphBuildOutcome::DirectCgPreferred);
    }
    let nuisance = build_nuisance_columns(design, &profile.effect_order[2..])?;
    let solver_started = trace_enabled.then(Instant::now);
    let solver = ParallelPcgSolver::from_preconditioner(
        preconditioner,
        executor,
        ParallelPcgPolicy::default(),
    )
    .map_err(cmg_error)?;
    let workspace = if prepared.is_some() {
        GraphCmgWorkspace::CallerBuffers(CallerPcgBuffers::new(&solver, config.batch_size)?)
    } else {
        GraphCmgWorkspace::OwnedResults(solver.workspace())
    };
    let solver_seconds = solver_started.map_or(0.0, |started| started.elapsed().as_secs_f64());
    let mut route = GraphCmg {
        core_effects,
        nuisance,
        nuisance_factor: PivotedPsdFactor::empty(),
        solver,
        workspace,
        setup_iterations: 0,
        setup_backward_error: 0.0,
        setup_right_hand_sides: 0,
        setup_batches: 0,
    };
    let nuisance_started = trace_enabled.then(Instant::now);
    let (factor, metrics) =
        build_nuisance_factor(design, weights, weight_scale, &mut route, config)?;
    let nuisance_seconds = nuisance_started.map_or(0.0, |started| started.elapsed().as_secs_f64());
    route.nuisance_factor = factor;
    ensure_cmg_exact_retained_memory_fits(&route, config, prepared)?;
    route.setup_iterations = metrics.iterations;
    route.setup_backward_error = metrics.backward_error;
    route.setup_right_hand_sides = metrics.right_hand_sides;
    route.setup_batches = metrics.batches;
    if trace_enabled {
        eprintln!(
            "FEREG_CMG_SETUP|graph={graph_seconds:.9}|hierarchy={hierarchy_seconds:.9}|solver_workspace={solver_seconds:.9}|nuisance={nuisance_seconds:.9}|nuisance_columns={}|nuisance_iterations={}",
            route.nuisance.len(),
            metrics.iterations,
        );
    }
    Ok(GraphBuildOutcome::Ready(route))
}

fn auto_hierarchy_guard_rejects(terminal: TerminalReason, work_over_finest: f64) -> bool {
    terminal.is_iterative() && !(work_over_finest <= AUTO_MAX_ITERATIVE_HIERARCHY_WORK_OVER_FINEST)
}

fn auto_direct_terminal_cg_preferred(
    terminal: TerminalReason,
    work: CmgHierarchyWork,
    hierarchy_levels: usize,
    observations: usize,
    nuisance_columns: usize,
    rhs_columns: usize,
    threads: usize,
    terminal_factor_dimension: usize,
) -> bool {
    if terminal != TerminalReason::Direct || observations == 0 || rhs_columns == 0 || threads == 0 {
        return false;
    }

    let levels = hierarchy_levels as u128;
    let observations = observations as u128;
    let rhs = rhs_columns as u128;
    let effective_rhs_width = rhs_columns.min(threads) as u128;
    let nuisance = nuisance_columns as u128;
    let effective_nuisance_width = nuisance_columns.min(threads) as u128;
    let recursive_work = work.estimated_recursive_work;
    let depth_work_numerator = levels.saturating_mul(recursive_work);
    let cost_guard_rejects = depth_work_numerator
        > AUTO_DIRECT_MAX_DEPTH_WORK_PER_OBSERVATION.saturating_mul(observations)
        || (nuisance_columns > 0
            && recursive_work.saturating_mul(nuisance)
                > AUTO_DIRECT_MAX_NUISANCE_WORK.saturating_mul(effective_nuisance_width))
        || rhs
            >= (AUTO_DIRECT_MAX_EFFECTIVE_RHS_BATCHES as u128).saturating_mul(effective_rhs_width)
        || (terminal_factor_dimension > 0
            && rhs
                >= (AUTO_DIRECT_MIN_WIDE_FACTOR_RHS_BATCHES as u128)
                    .saturating_mul(effective_rhs_width)
            && recursive_work.saturating_mul(rhs)
                > AUTO_DIRECT_MAX_WIDE_FACTOR_WORK.saturating_mul(effective_rhs_width));
    let calibrated_cg_region = depth_work_numerator
        > AUTO_DIRECT_CG_MIN_DEPTH_WORK_PER_OBSERVATION.saturating_mul(observations)
        || nuisance_columns >= AUTO_DIRECT_CG_MIN_NUISANCE_COLUMNS;
    let one_effective_rhs_batch = threads == 1 || rhs_columns <= threads;

    cost_guard_rejects && calibrated_cg_region && one_effective_rhs_batch
}

fn trace_cmg_hierarchy(
    graph: &Laplacian,
    preconditioner: &CmgPreconditioner,
    work: CmgHierarchyWork,
    graph_seconds: f64,
    hierarchy_seconds: f64,
) {
    let hierarchy = preconditioner.hierarchy();
    let report = hierarchy.report();
    let repeats = preconditioner.repeat_counts();
    let components = Components::from_laplacian(graph);
    let mut component_sizes = components.sizes().to_vec();
    component_sizes.sort_unstable();
    let smallest_component = component_sizes.first().copied().unwrap_or(0);
    let median_component = component_quantile(&component_sizes, 50);
    let p90_component = component_quantile(&component_sizes, 90);
    let largest_component = component_sizes.last().copied().unwrap_or(0);
    let largest_component_share = if graph.vertex_count() == 0 {
        0.0
    } else {
        largest_component as f64 / graph.vertex_count() as f64
    };
    let terminal_factor_dimension = preconditioner
        .terminal_factor()
        .map_or(0, cmg::GroundedLdl::active_dimension);
    let terminal_factor_nonzeros = preconditioner
        .terminal_factor()
        .map_or(0, cmg::GroundedLdl::factor_nonzeros);
    eprintln!(
        "FEREG_CMG_HIERARCHY|components={}|component_min={smallest_component}|component_p50={median_component}|component_p90={p90_component}|component_max={largest_component}|component_max_share={largest_component_share:.17e}|terminal={:?}|levels={}|vertices={}|nonzeros={}|repeats={}|recursive_work={}|finest_work={}|work_over_finest={:.17e}|deepest_visits={}|max_repeat={}|terminal_factor_dimension={terminal_factor_dimension}|terminal_factor_nonzeros={terminal_factor_nonzeros}|graph_seconds={graph_seconds:.9}|hierarchy_seconds={hierarchy_seconds:.9}",
        components.count(),
        report.terminal_reason(),
        report.vertex_counts().len(),
        format_usize_series(report.vertex_counts()),
        format_usize_series(report.matrix_nonzeros()),
        format_usize_series(repeats),
        work.estimated_recursive_work,
        work.finest_work,
        work.work_over_finest,
        work.deepest_visits,
        work.maximum_repeat,
    );
}

fn component_quantile(sorted_sizes: &[usize], percentile: usize) -> usize {
    if sorted_sizes.is_empty() {
        return 0;
    }
    let index = (sorted_sizes.len() - 1).saturating_mul(percentile) / 100;
    sorted_sizes[index]
}

fn format_usize_series(values: &[usize]) -> String {
    values
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(";")
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

fn preflight_prepared_cmg_memory(
    design: &BorrowedInterceptFeDesign<'_>,
    profile: &CmgTopologyProfile,
    config: CmgConfig,
    prepared: &PreparedCmgTopology,
) -> Result<()> {
    let Some(budget) = config.solver_memory_budget else {
        return Ok(());
    };
    if &prepared.profile != profile {
        return Err(invalid_argument(
            "prepared CMG topology does not match the current absorbed design",
        ));
    }
    let parallel = ParallelOptions {
        threads: config.threads.resolved(),
        ..ParallelOptions::default()
    };
    let estimate = RepeatedPcgMemoryEstimate::conservative(
        CmgProblemSize {
            vertices: prepared.topology.vertex_count(),
            input_edges: prepared.topology.input_edge_count(),
            canonical_edges: prepared.topology.canonical_edge_count(),
            right_hand_sides: config.batch_size,
        },
        CmgOptions::default(),
        parallel,
        config.threads.resolved() > 1,
        false,
    )
    .map_err(cmg_error)?;
    let repeated_frame_bytes = estimate
        .total_solver_retained_bytes()
        .checked_sub(estimate.prepared_topology_bytes())
        .and_then(|value| value.checked_sub(estimate.numeric_assembly_scratch_bytes()))
        .ok_or_else(|| allocation("prepared CMG repeated memory estimate underflows"))?;
    let legacy_build_peak = CmgMemoryEstimate::conservative(
        CmgProblemSize {
            vertices: prepared.topology.vertex_count(),
            input_edges: prepared.topology.input_edge_count(),
            canonical_edges: prepared.topology.canonical_edge_count(),
            right_hand_sides: config.batch_size,
        },
        CmgOptions::default(),
        parallel,
    )
    .map_err(cmg_error)?
    .build_peak_bytes();
    let nuisance_square = profile
        .nuisance_columns
        .checked_mul(profile.nuisance_columns)
        .and_then(|value| value.checked_mul(32))
        .ok_or_else(|| allocation("prepared CMG nuisance workspace estimate overflows"))?;
    let retained_coefficients = if config.retain_coefficients {
        design
            .total_term_slots()
            .checked_mul(config.batch_size)
            .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
            .ok_or_else(|| allocation("prepared CMG retained coefficient estimate overflows"))?
    } else {
        0
    };
    let required = (repeated_frame_bytes.max(legacy_build_peak) as u128)
        .checked_add(estimate.caller_logical_bytes() as u128)
        .and_then(|value| value.checked_add(nuisance_square as u128))
        .and_then(|value| value.checked_add(retained_coefficients as u128))
        .ok_or_else(|| allocation("prepared CMG complete memory estimate overflows"))?;
    if budget.fits(required) {
        Ok(())
    } else {
        Err(allocation(format!(
            "prepared CMG requires {required} incremental bytes and does not fit the joint execution/solver memory plan"
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

fn ensure_cmg_exact_retained_memory_fits(
    graph: &GraphCmg,
    config: CmgConfig,
    prepared: Option<&PreparedCmgTopology>,
) -> Result<()> {
    let Some(budget) = config.solver_memory_budget else {
        return Ok(());
    };
    let upstream_bytes = match &graph.workspace {
        GraphCmgWorkspace::OwnedResults(workspace) => {
            graph.solver.memory_report(workspace).total_retained_bytes() as u128
        }
        GraphCmgWorkspace::CallerBuffers(caller) => {
            let prepared = prepared.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "prepared CMG caller buffers have no prepared topology",
                )
            })?;
            let plan = if caller.planned_used {
                Some(graph.solver.try_plan().map_err(cmg_error)?)
            } else {
                None
            };
            let report = RepeatedPcgMemoryReport::with_parallel_plan(
                &prepared.topology,
                &prepared.assembly_workspace,
                graph.solver.graph(),
                graph.solver.preconditioner(),
                plan,
                &caller.workspaces,
                config.batch_size,
                false,
            )
            .map_err(cmg_error)?;
            let repeated_frame_bytes = report
                .total_solver_retained_bytes()
                .checked_sub(report.prepared_topology_bytes())
                .and_then(|value| value.checked_sub(report.numeric_assembly_scratch_bytes()))
                .ok_or_else(|| allocation("prepared CMG exact memory report underflows"))?;
            (repeated_frame_bytes as u128)
                .checked_add(caller.caller_retained_bytes() as u128)
                .ok_or_else(|| allocation("prepared CMG exact retained-memory report overflows"))?
        }
    };
    let nuisance_bytes = graph
        .nuisance
        .capacity()
        .checked_mul(std::mem::size_of::<NuisanceColumn>())
        .and_then(|value| value.checked_add(graph.nuisance_factor.retained_bytes()))
        .ok_or_else(|| allocation("CMG exact retained-memory report overflows"))?;
    let required = upstream_bytes
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
        match &mut graph.workspace {
            GraphCmgWorkspace::OwnedResults(workspace) => {
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
                    .solve_batch_with_workspace(&right_hand_sides, config.pcg_options(), workspace)
                    .map_err(cmg_solve_error)?;
                metrics.include_batch(solved.results().len());
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
            GraphCmgWorkspace::CallerBuffers(caller) => {
                let width = end - start;
                let vertices = graph.solver.graph().vertex_count();
                fill_nuisance_rhs_batch_into(
                    &mut caller.right_hand_sides,
                    vertices,
                    design,
                    graph.core_effects,
                    &graph.nuisance,
                    weights,
                    weight_scale,
                    start,
                    end,
                )?;
                caller.solve(&graph.solver, width, config.pcg_options())?;
                metrics.include_batch(width);
                for local in 0..width {
                    let diagnostic = caller.diagnostics[local];
                    metrics.include(diagnostic.iterations(), diagnostic.backward_error());
                    let column = start + local;
                    accumulate_schur_column(
                        design,
                        graph.core_effects,
                        &graph.nuisance,
                        weights,
                        weight_scale,
                        column,
                        caller.solution(local, vertices)?,
                        &mut schur,
                        &mut corrections,
                    )?;
                }
            }
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

#[allow(clippy::too_many_arguments)]
fn fill_value_rhs_batch_into(
    output: &mut [f64],
    vertices: usize,
    design: &BorrowedInterceptFeDesign<'_>,
    core: [usize; 2],
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    values: &DenseMatrix,
    start: usize,
    end: usize,
) -> Result<()> {
    let width = end
        .checked_sub(start)
        .ok_or_else(|| invalid_argument("prepared CMG value batch range is invalid"))?;
    let elements = width
        .checked_mul(vertices)
        .ok_or_else(|| allocation("prepared CMG value RHS dimensions overflow"))?;
    let output = output
        .get_mut(..elements)
        .ok_or_else(|| invalid_argument("prepared CMG value RHS buffer is too small"))?;
    output.fill(0.0);
    let left = design.effects()[core[0]].codes();
    let right = design.effects()[core[1]].codes();
    let right_offset = design.effects()[core[0]].n_levels();
    for row in 0..values.nrows() {
        let weight = normalized_weight(weights, weight_scale, row);
        for (local, column) in (start..end).enumerate() {
            let contribution = weight * values[(row, column)];
            let base = local * vertices;
            output[base + left[row] as usize] += contribution;
            output[base + right_offset + right[row] as usize] -= contribution;
        }
    }
    validate_finite_slice(output, "prepared CMG RHS accumulation is nonfinite")
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
fn fill_nuisance_rhs_batch_into(
    output: &mut [f64],
    vertices: usize,
    design: &BorrowedInterceptFeDesign<'_>,
    core: [usize; 2],
    nuisance: &[NuisanceColumn],
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    start: usize,
    end: usize,
) -> Result<()> {
    let width = end
        .checked_sub(start)
        .ok_or_else(|| invalid_argument("prepared CMG nuisance batch range is invalid"))?;
    let elements = width
        .checked_mul(vertices)
        .ok_or_else(|| allocation("prepared CMG nuisance RHS dimensions overflow"))?;
    let output = output
        .get_mut(..elements)
        .ok_or_else(|| invalid_argument("prepared CMG nuisance RHS buffer is too small"))?;
    output.fill(0.0);
    let left = design.effects()[core[0]].codes();
    let right = design.effects()[core[1]].codes();
    let right_offset = design.effects()[core[0]].n_levels();
    for row in 0..design.nobs() {
        let weight = normalized_weight(weights, weight_scale, row);
        for (local, column) in nuisance[start..end].iter().enumerate() {
            if design.effects()[column.effect].codes()[row] == column.level {
                let base = local * vertices;
                output[base + left[row] as usize] += weight;
                output[base + right_offset + right[row] as usize] -= weight;
            }
        }
    }
    validate_finite_slice(
        output,
        "prepared CMG nuisance RHS accumulation is nonfinite",
    )
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

fn validate_finite_slice(values: &[f64], message: &'static str) -> Result<()> {
    if values.iter().any(|value| !value.is_finite()) {
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

fn cmg_solve_error(error: cmg::CmgError) -> FeregError {
    let code = match &error {
        cmg::CmgError::PcgBreakdown { .. }
        | cmg::CmgError::MaximumIterations { .. }
        | cmg::CmgError::ResidualVerificationFailed { .. }
        | cmg::CmgError::IncompatibleLaplacianRhs { .. } => ErrorCode::NonConvergence,
        cmg::CmgError::MemoryBudgetExceeded { .. } => ErrorCode::Allocation,
        cmg::CmgError::ParallelRuntime { .. } => ErrorCode::Internal,
        cmg::CmgError::InvalidOption { .. } => ErrorCode::InvalidArgument,
        _ => ErrorCode::InvalidData,
    };
    FeregError::new(code, format!("CMG solve: {error}"))
}

fn nonconvergence(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
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

    #[test]
    fn hierarchy_work_counts_recursive_level_visits() {
        let work = CmgHierarchyWork::from_levels(&[100, 20, 5], &[300, 40, 0], &[4, 3, 0]);
        assert_eq!(work.finest_work, 400);
        assert_eq!(work.estimated_recursive_work, 700);
        assert_eq!(work.deepest_visits, 12);
        assert_eq!(work.maximum_repeat, 4);
        assert!((work.work_over_finest - 1.75).abs() <= f64::EPSILON);
    }

    #[test]
    fn component_quantiles_use_stable_nearest_lower_order_statistic() {
        let sizes = [1, 2, 3, 4, 100];
        assert_eq!(component_quantile(&sizes, 50), 3);
        assert_eq!(component_quantile(&sizes, 90), 4);
        assert_eq!(component_quantile(&[], 50), 0);
    }

    #[test]
    fn auto_hierarchy_guard_applies_only_to_costly_iterative_terminals() {
        assert!(!auto_hierarchy_guard_rejects(TerminalReason::Direct, 100.0));
        assert!(!auto_hierarchy_guard_rejects(
            TerminalReason::StagnatedVertexReduction,
            2.30,
        ));
        assert!(!auto_hierarchy_guard_rejects(
            TerminalReason::MaximumLevels,
            AUTO_MAX_ITERATIVE_HIERARCHY_WORK_OVER_FINEST,
        ));
        assert!(auto_hierarchy_guard_rejects(
            TerminalReason::StagnatedFill,
            5.68,
        ));
        assert!(auto_hierarchy_guard_rejects(
            TerminalReason::FullContraction,
            f64::NAN,
        ));
    }

    #[test]
    fn auto_direct_terminal_cg_guard_matches_locked_primary_boundaries() {
        let work = |recursive_work| CmgHierarchyWork {
            estimated_recursive_work: recursive_work,
            finest_work: recursive_work,
            work_over_finest: 1.0,
            deepest_visits: 1,
            maximum_repeat: 1,
        };

        assert!(!auto_direct_terminal_cg_preferred(
            TerminalReason::StagnatedFill,
            work(20_001),
            10,
            1_000,
            0,
            5,
            8,
            0,
        ));
        assert!(!auto_direct_terminal_cg_preferred(
            TerminalReason::Direct,
            work(20_000),
            10,
            1_000,
            0,
            5,
            8,
            0,
        ));
        assert!(auto_direct_terminal_cg_preferred(
            TerminalReason::Direct,
            work(20_001),
            10,
            1_000,
            0,
            5,
            8,
            0,
        ));
        assert!(auto_direct_terminal_cg_preferred(
            TerminalReason::Direct,
            work(300_001),
            1,
            1_000_000,
            9,
            5,
            8,
            0,
        ));
        assert!(!auto_direct_terminal_cg_preferred(
            TerminalReason::Direct,
            work(20_001),
            10,
            1_000,
            0,
            9,
            8,
            100,
        ));
        assert!(auto_direct_terminal_cg_preferred(
            TerminalReason::Direct,
            work(20_001),
            10,
            1_000,
            0,
            17,
            1,
            100,
        ));
    }

    #[test]
    fn auto_hierarchy_guard_rejects_mixed_cliff_but_retains_all_tiny_graph() {
        let periods = 3;
        let workers = 33_000;
        let firms = 3_300;
        let rows = periods * workers;

        let build_codes = |components: usize, balanced: bool| {
            let tiny_components = if balanced { components } else { components - 1 };
            let small_workers = tiny_components * 30;
            let small_firms = tiny_components * 3;
            let remaining_workers = workers - small_workers;
            let remaining_firms = firms - small_firms;
            let worker_codes: Vec<u32> = (0..rows).map(|row| (row / periods) as u32).collect();
            let firm_codes = (0..rows)
                .map(|row| {
                    let worker = row / periods;
                    let period = row % periods;
                    if worker < small_workers {
                        (worker / 30) * 3 + ((worker % 3) + period) % 3
                    } else {
                        small_firms
                            + ((worker - small_workers) % remaining_firms + period)
                                % remaining_firms
                    }
                })
                .map(|firm| firm as u32)
                .collect::<Vec<_>>();
            debug_assert!(balanced || remaining_workers == 9_030);
            (worker_codes, firm_codes)
        };

        let (mixed_worker, mixed_firm) = build_codes(800, false);
        let mixed_worker_index =
            LevelIndex::from_codes(&mixed_worker, workers).expect("mixed worker index");
        let mixed_firm_index =
            LevelIndex::from_codes(&mixed_firm, firms).expect("mixed firm index");
        let mixed_design = BorrowedInterceptFeDesign::new(
            rows,
            vec![
                effect(&mixed_worker, workers, &mixed_worker_index),
                effect(&mixed_firm, firms, &mixed_firm_index),
            ],
        )
        .expect("mixed design");
        let mixed_profile = CmgTopologyProfile::analyze(&mixed_design).expect("mixed profile");
        assert!(mixed_profile.auto_favorable());
        let mut auto_config = config(5);
        auto_config.requested_solver = FeSolverKind::Auto;
        assert!(matches!(
            CmgAbsorber::build_profiled_auto_guarded(
                mixed_design,
                BorrowedMapWeights::Unit,
                auto_config,
                false,
                mixed_profile,
                5,
            )
            .expect("guarded mixed setup"),
            CmgAutoBuildOutcome::HierarchyRejected { .. }
        ));

        let (tiny_worker, tiny_firm) = build_codes(1_100, true);
        let tiny_worker_index =
            LevelIndex::from_codes(&tiny_worker, workers).expect("tiny worker index");
        let tiny_firm_index = LevelIndex::from_codes(&tiny_firm, firms).expect("tiny firm index");
        let tiny_design = BorrowedInterceptFeDesign::new(
            rows,
            vec![
                effect(&tiny_worker, workers, &tiny_worker_index),
                effect(&tiny_firm, firms, &tiny_firm_index),
            ],
        )
        .expect("tiny design");
        let tiny_profile = CmgTopologyProfile::analyze(&tiny_design).expect("tiny profile");
        assert!(matches!(
            CmgAbsorber::build_profiled_auto_guarded(
                tiny_design,
                BorrowedMapWeights::Unit,
                auto_config,
                false,
                tiny_profile,
                5,
            )
            .expect("guarded all-tiny setup"),
            CmgAutoBuildOutcome::Ready(_)
        ));
    }

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
