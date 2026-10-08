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
    CmgProblemSize, CmgStorage, CmgStorageReport, ComponentBuildOptions, Components, Laplacian,
    ParallelExecutor, ParallelOptions, ParallelPcgExecution, ParallelPcgPolicy, ParallelPcgSolver,
    ParallelRuntimeAllowance, ParallelRuntimeEstimate, PcgBatchMut, PcgBatchRef, PcgBatchWorkspace,
    PcgDiagnostics, PcgOptions, PreparedLaplacianTopology, PreparedLaplacianWorkspace,
    RepeatedPcgMemoryEstimate,
};

use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, invalid_data, ErrorCode, FeregError, Result};
use crate::linalg::neumaier_add;
use crate::memory::{estimate_fe_solver_memory, FeSolverMemoryBudget, SolverWorkspacePeak};
use crate::options::ThreadConfig;

use super::{
    absorb_borrowed_intercept_map_certified,
    absorb_borrowed_intercept_map_with_structural_constant_certified,
    borrowed_map::polish_borrowed_intercept_cg_with_products_certified,
    borrowed_map::polish_borrowed_intercept_cg_with_reference_norms_certified,
    retained_effect_order_by_levels, BorrowedInterceptFeDesign, BorrowedMapWeights,
    CertifiedBorrowedMap, FeCoefficientBatch, FeSolverKind, MapConfig, SolveReport,
    SolveStopReason,
};

#[path = "cmg_admission.rs"]
mod admission;
#[path = "cmg_forecast.rs"]
mod forecast;

pub(crate) const MAX_NUISANCE_COLUMNS: usize = 64;
// CMG's graph solve feeds the OLS normal equations, whose score certificate is
// more sensitive than the observation-space FE certificate. Keep the public
// tolerance unchanged while solving the reusable graph system two orders
// tighter; this avoids a separate post-QR correction and preserves the fitted
// model's RSS/covariance identities.
const PCG_TOLERANCE_MULTIPLIER: f64 = 0.01;
const PCG_TOLERANCE_FLOOR: f64 = 32.0 * f64::EPSILON;
/// Internal execution policy for one prepared CMG absorption session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CmgConfig {
    pub map: MapConfig,
    pub threads: ThreadConfig,
    pub batch_size: usize,
    pub retain_coefficients: bool,
    pub requested_solver: FeSolverKind,
    /// Component policy for explicit CMG; automatic CMG remains ordinary.
    pub components: bool,
    pub solver_memory_budget: Option<FeSolverMemoryBudget>,
}

/// Private comparison seam; production explicit-CMG uses the component engine.
/// Automatic routing must remain independent of both this policy and the seam.
pub(crate) fn explicit_cmg_components(requested: FeSolverKind) -> bool {
    component_policy(requested, std::env::var_os("FEREG_CMG_ORDINARY").is_some())
}

fn component_policy(requested: FeSolverKind, ordinary_override: bool) -> bool {
    requested == FeSolverKind::Cmg && !ordinary_override
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
        let effect_order = retained_effect_order_by_levels(design.effects().len(), |effect| {
            design.effects()[effect].n_levels()
        })?;
        // The Laplacian is algebraically invariant to swapping its bipartition;
        // core[0] is nevertheless fixed as the larger retained dimension so
        // unequal-cardinality vertex numbering and hierarchy construction do
        // not depend on absorb syntax order. Equal sizes use original position.
        // Coefficients are always scattered by original index.
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
    pub(crate) const fn nuisance_columns(&self) -> usize {
        self.nuisance_columns
    }

    pub(crate) const fn vertices(&self) -> usize {
        self.vertices
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
    pub(crate) nuisance_reused_right_hand_sides: usize,
    pub(crate) pcg_batch_width: usize,
    pub(crate) nuisance_setup: CmgNuisanceSetup,
    pub(crate) projection_work: CmgProjectionWork,
    #[cfg(test)]
    pub(crate) executor_threads: usize,
}

/// PPML-owned structural graph state reused while IRLS changes only weights.
/// OLS keeps its ordinary endpoint construction; both callers use fixed
/// solve banks, while only PPML retains changing-weight topology and seeds.
pub(crate) struct PreparedCmgTopology {
    profile: CmgTopologyProfile,
    design_identity: PreparedCmgDesignIdentity,
    topology: PreparedLaplacianTopology,
    assembly_workspace: PreparedLaplacianWorkspace,
    numeric_weights: Vec<f64>,
    nuisance_seed: Vec<f64>,
    seed_valid: bool,
    seed_for_frame: bool,
    weight_ratio: f64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct CmgNuisanceSetup {
    pub(crate) warm_right_hand_sides: usize,
    pub(crate) iterations: usize,
    pub(crate) restarts: usize,
    pub(crate) extra_restarts: usize,
    pub(crate) initial_relative_residual: f64,
    pub(crate) weight_ratio: f64,
    pub(crate) pcg_seconds: f64,
    pub(crate) rhs_seconds: f64,
    pub(crate) schur_seconds: f64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct CmgProjectionWork {
    pub(crate) warm_right_hand_sides: usize,
    pub(crate) warm_iterations: usize,
    pub(crate) iterations: usize,
    pub(crate) restarts: usize,
    pub(crate) extra_restarts: usize,
    pub(crate) initial_relative_residual: f64,
    pub(crate) pcg_seconds: f64,
    pub(crate) rhs_seconds: f64,
}

/// Owned by the frozen PPML finite basis, never by the reusable OLS absorber.
/// Each control stores the raw core solve, before the nuisance correction.
/// The changing working response remains cold: seeding it caused a measured
/// per-RHS certificate retry and expensive three-FE nuisance-factor rebuild.
pub(crate) struct CmgControlSeed {
    potentials: Vec<f64>,
    valid: bool,
}

impl CmgControlSeed {
    pub(crate) fn estimated_bytes(vertices: usize, columns: usize) -> Result<u128> {
        (vertices as u128)
            .checked_mul(columns as u128)
            .and_then(|value| value.checked_mul(8))
            .and_then(|value| value.checked_add(std::mem::size_of::<Self>() as u128))
            .ok_or_else(|| allocation("CMG control seed estimate overflows"))
    }

    pub(crate) fn new(vertices: usize, columns: usize) -> Result<Self> {
        let elements = vertices
            .checked_mul(columns)
            .ok_or_else(|| allocation("CMG control seed dimensions overflow"))?;
        let potentials = zero_vec(elements, "unable to reserve CMG control seed")?;
        // The fit's joint budget includes this exact retained capacity before
        // any ordinary graph is built; a larger reservation cannot escape it.
        if potentials.capacity() != elements {
            return Err(allocation("CMG control seed exceeds its admitted capacity"));
        }
        Ok(Self {
            potentials,
            valid: false,
        })
    }

    pub(crate) fn reset(&mut self) {
        self.valid = false;
    }

    fn consume(&mut self, weight_ratio: f64) -> bool {
        std::mem::replace(&mut self.valid, false)
            && (1.0..=2.0).contains(&weight_ratio)
            && self.potentials.iter().all(|value| value.is_finite())
    }

    /// Only the owner of all per-RHS and combined PPML certificates may call
    /// this. Promotion only marks the buffer written while invalid: it cannot
    /// copy or allocate. No stale residual is retained or promoted.
    pub(crate) fn retain_certified(&mut self, work: CmgProjectionWork) {
        self.valid = work.extra_restarts == 0
            && (work.warm_right_hand_sides == 0 || work.initial_relative_residual < 0.5)
            && self.potentials.iter().all(|value| value.is_finite());
    }

    #[cfg(test)]
    pub(crate) fn ready(&self) -> bool {
        self.valid
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedCmgDesignIdentity {
    observations: usize,
    effects: Vec<PreparedCmgEffectIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreparedCmgEffectIdentity {
    codes_address: usize,
    codes_len: usize,
    levels: usize,
    level_index_address: usize,
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
    memory: CmgWorkspacePlan,
}

// The projection envelope excludes the variable-width coefficient output;
// construction retains the exact preflight quantity used for admission.
#[derive(Clone, Copy)]
struct CmgWorkspacePlan {
    construction: SolverWorkspacePeak,
    projection: SolverWorkspacePeak,
}

impl CmgWorkspacePlan {
    const EMPTY: Self = Self {
        construction: SolverWorkspacePeak::ZERO,
        projection: SolverWorkspacePeak::ZERO,
    };

    fn record_expected_heap(&mut self, bytes: u128) -> Result<()> {
        let expected = SolverWorkspacePeak::ZERO.with_expected_heap(Some(bytes))?;
        self.construction = self.construction.max(expected);
        self.projection = self.projection.max(expected);
        Ok(())
    }
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
    nuisance_effects: Vec<NuisanceEffect>,
    nuisance_factor: PivotedPsdFactor,
    solver: ParallelPcgSolver,
    runtime_estimate: ParallelRuntimeEstimate,
    workspace: GraphCmgWorkspace,
    setup_iterations: usize,
    setup_backward_error: f64,
    setup_right_hand_sides: usize,
    setup_batches: usize,
    nuisance_setup: CmgNuisanceSetup,
    projection_work: CmgProjectionWork,
}

enum GraphCmgWorkspace {
    OlsBuffers(CallerPcgBuffers),
    CallerBuffers(CallerPcgBuffers),
}

struct CallerPcgBuffers {
    right_hand_sides: Vec<f64>,
    solutions: Vec<f64>,
    nuisance_potentials: Vec<f64>,
    diagnostics: Vec<PcgDiagnostics>,
    workspaces: Vec<PcgBatchWorkspace>,
    planned_used: bool,
}

impl CallerPcgBuffers {
    #[allow(clippy::too_many_arguments)]
    fn new(
        solver: &ParallelPcgSolver,
        batch_size: usize,
        nuisance: usize,
        config: CmgConfig,
        runtime: ParallelRuntimeEstimate,
        prepared: Option<&PreparedCmgTopology>,
        memory: &mut CmgWorkspacePlan,
    ) -> Result<Self> {
        let vertices = solver.graph().vertex_count();
        let elements = vertices
            .checked_mul(batch_size)
            .ok_or_else(|| allocation("prepared CMG caller-buffer dimensions overflow"))?;
        let nuisance_elements = vertices
            .checked_mul(nuisance)
            .ok_or_else(|| allocation("prepared CMG nuisance potential dimensions overflow"))?;
        let report = solver
            .select_batch_execution(batch_size)
            .map_err(cmg_error)?;
        let workspace_count = report.concurrency().max(1);
        let bank_bytes =
            PcgBatchWorkspace::bank_allocation_bytes(solver.preconditioner(), workspace_count)
                .map_err(cmg_error)?;
        let caller_bytes = elements
            .checked_mul(2)
            .and_then(|v| v.checked_add(nuisance_elements))
            .and_then(|v| v.checked_mul(8))
            .and_then(|v| {
                v.checked_add(batch_size.checked_mul(std::mem::size_of::<PcgDiagnostics>())?)
            })
            .ok_or_else(|| allocation("CMG fixed caller banks overflow"))?;
        let total = bank_bytes
            .checked_add(caller_bytes)
            .ok_or_else(|| allocation("CMG fixed banks overflow"))?;
        let parents =
            admission::frame_storage(&[solver], prepared)? + admission::runtime_extra(runtime);
        admission::admit(config, parents, total, memory)?;
        memory.record_expected_heap(parents - admission::runtime_extra(runtime) + total as u128)?;
        let workspaces = PcgBatchWorkspace::new_bank_with_budget(
            solver.preconditioner(),
            workspace_count,
            bank_bytes,
        )
        .map_err(cmg_error)?;
        let mut diagnostics = Vec::new();
        diagnostics
            .try_reserve_exact(batch_size)
            .map_err(|_| allocation("unable to reserve prepared CMG diagnostics"))?;
        diagnostics.resize(batch_size, PcgDiagnostics::default());
        Ok(Self {
            right_hand_sides: zero_vec(
                elements,
                "unable to reserve prepared CMG right-hand-side buffers",
            )?,
            solutions: zero_vec(elements, "unable to reserve prepared CMG solution buffers")?,
            nuisance_potentials: zero_vec(
                nuisance_elements,
                "unable to reserve prepared CMG nuisance potentials",
            )?,
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
        initial_guesses: Option<&[f64]>,
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
        let guesses = initial_guesses
            .map(|values| PcgBatchRef::contiguous(values, rhs_count, vertices))
            .transpose()
            .map_err(cmg_error)?;
        let diagnostics = &mut self.diagnostics[..rhs_count];
        match report.execution() {
            ParallelPcgExecution::Serial => solve_pcg_batch_into_with_workspace(
                solver.graph(),
                solver.preconditioner(),
                rhs,
                guesses,
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
                    solver.try_plan_with_budget(0).map_err(cmg_error)?,
                    rhs,
                    guesses,
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
                guesses,
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
}

#[derive(Debug, Clone, Copy)]
struct NuisanceColumn {
    effect: usize,
    level: u32,
}

#[derive(Debug, Clone, Copy)]
struct NuisanceEffect {
    effect: usize,
    first_column: usize,
    columns: usize,
}

impl NuisanceEffect {
    fn column_at_row(self, design: &BorrowedInterceptFeDesign<'_>, row: usize) -> Option<usize> {
        let level = design.effects()[self.effect].codes()[row] as usize;
        (level > 0 && level <= self.columns).then(|| self.first_column + level - 1)
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct CmgSolveMetrics {
    iterations: usize,
    backward_error: f64,
    right_hand_sides: usize,
    batches: usize,
    nuisance_reused_right_hand_sides: usize,
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
    /// Conservative peak required to prepare fixed endpoint topology, its
    /// changing-weight assembly scratch, and fereg's normalized-weight buffer.
    /// This bound is available before any observation-scale CMG allocation.
    pub(crate) fn conservative_prepare_peak_bytes(
        design: &BorrowedInterceptFeDesign<'_>,
    ) -> Result<u128> {
        let profile = CmgTopologyProfile::analyze(design)?;
        validate_nuisance_limit(&profile)?;
        let topology = PreparedLaplacianTopology::preparation_allocation_bound(
            profile.vertices,
            design.nobs(),
        )
        .map_err(cmg_error)? as u128;
        // The admitted upstream boundary takes a slice, so its caller endpoint
        // bank remains live through topology construction. Later scratch/weight
        // banks are sequential; including both here is a conservative reserve.
        let caller = (design.nobs() as u128)
            .checked_mul((2 * std::mem::size_of::<usize>() + 16) as u128)
            .and_then(|bytes| {
                bytes.checked_add(
                    (design.effects().len() * 5 * std::mem::size_of::<usize>()) as u128,
                )
            })
            .and_then(|bytes| bytes.checked_add(nuisance_seed_bytes(&profile).ok()?))
            .ok_or_else(|| allocation("prepared CMG caller preparation overflows"))?;
        topology
            .checked_add(caller)
            .ok_or_else(|| allocation("prepared CMG preparation overflows"))
    }

    /// Conservative retained topology plus one complete changing-weight CMG
    /// frame. This is available before topology preparation so PPML can admit
    /// the full route, rather than only the endpoint-sorting allocation.
    pub(crate) fn conservative_retained_frame_peak_bytes(
        design: &BorrowedInterceptFeDesign<'_>,
        config: CmgConfig,
    ) -> Result<u128> {
        validate_config(config)?;
        let profile = CmgTopologyProfile::analyze(design)?;
        validate_nuisance_limit(&profile)?;
        let problem = conservative_prepared_problem(&profile, config.batch_size);
        let retained_topology = PreparedCmgTopology::conservative_prepare_peak_bytes(design)?;
        let frame = conservative_prepared_cmg_frame_bytes(design, &profile, config, problem)?;
        retained_topology
            .checked_add(frame)
            .ok_or_else(|| allocation("prepared CMG retained-frame peak estimate overflows"))
    }

    pub(crate) fn prepare(design: &BorrowedInterceptFeDesign<'_>) -> Result<Self> {
        let profile = CmgTopologyProfile::analyze(design)?;
        validate_nuisance_limit(&profile)?;
        let design_identity = PreparedCmgDesignIdentity::new(design)?;
        let core = profile.core_effects.ok_or_else(|| {
            invalid_argument("prepared CMG topology requires two absorbed effects")
        })?;
        let left_levels = design.effects()[core[0]].n_levels();
        let vertices = left_levels
            .checked_add(design.effects()[core[1]].n_levels())
            .ok_or_else(|| allocation("prepared CMG graph vertex count overflows"))?;
        let left_codes = design.effects()[core[0]].codes();
        let right_codes = design.effects()[core[1]].codes();
        let topology_budget =
            PreparedLaplacianTopology::preparation_allocation_bound(vertices, design.nobs())
                .map_err(cmg_error)?;
        let mut endpoints = Vec::new();
        endpoints
            .try_reserve_exact(design.nobs())
            .map_err(|_| allocation("unable to reserve prepared CMG endpoints"))?;
        endpoints.extend((0..design.nobs()).map(|row| {
            (
                left_codes[row] as usize,
                left_levels + right_codes[row] as usize,
            )
        }));
        let topology =
            PreparedLaplacianTopology::prepare_with_budget(vertices, &endpoints, topology_budget)
                .map_err(cmg_error)?;
        drop(endpoints);
        let scratch_bytes =
            PreparedLaplacianWorkspace::allocation_bytes(&topology).map_err(cmg_error)?;
        let assembly_workspace =
            PreparedLaplacianWorkspace::new_with_budget(&topology, scratch_bytes)
                .map_err(cmg_error)?;
        let numeric_weights = zero_vec(
            design.nobs(),
            "unable to reserve prepared CMG numeric weights",
        )?;
        let nuisance_seed = zero_vec(
            profile
                .vertices
                .checked_mul(profile.nuisance_columns)
                .ok_or_else(|| allocation("prepared CMG seed dimensions overflow"))?,
            "unable to reserve prepared CMG nuisance seed",
        )?;
        Ok(Self {
            profile,
            design_identity,
            topology,
            assembly_workspace,
            numeric_weights,
            nuisance_seed,
            seed_valid: false,
            seed_for_frame: false,
            weight_ratio: f64::INFINITY,
        })
    }

    fn assemble(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        profile: &CmgTopologyProfile,
        weights: BorrowedMapWeights<'_>,
        weight_scale: f64,
    ) -> Result<Laplacian> {
        self.ensure_matches(design, profile)?;
        let was_valid = std::mem::replace(&mut self.seed_valid, false);
        self.weight_ratio = 1.0;
        for (row, numeric_weight) in self.numeric_weights.iter_mut().enumerate() {
            let current = normalized_weight(weights, weight_scale, row);
            self.weight_ratio = if current > 0.0 && *numeric_weight > 0.0 {
                self.weight_ratio
                    .max(current / *numeric_weight)
                    .max(*numeric_weight / current)
            } else {
                f64::INFINITY
            };
            *numeric_weight = current;
        }
        // This bounds the normalized graph's quadratic-form change in either
        // direction by two. It is only a seed-admission rule: the upstream
        // solve recomputes b - L_current*x and uses its unchanged certificate.
        self.seed_for_frame = was_valid
            && self.weight_ratio <= 2.0
            && self.nuisance_seed.iter().all(|value| value.is_finite());
        self.topology
            .assemble_with_workspace_budgeted(
                &self.numeric_weights,
                &mut self.assembly_workspace,
                self.topology
                    .numeric_allocation_bytes()
                    .map_err(cmg_error)?,
            )
            .map_err(cmg_error)
    }

    fn ensure_matches(
        &self,
        design: &BorrowedInterceptFeDesign<'_>,
        profile: &CmgTopologyProfile,
    ) -> Result<()> {
        if profile != &self.profile
            || !self.design_identity.matches(design)
            || self.numeric_weights.len() != profile.observations
        {
            return Err(invalid_argument(
                "prepared CMG topology does not match the current absorbed design",
            ));
        }
        Ok(())
    }

    pub(crate) fn retained_bytes(&self) -> Result<usize> {
        let roots: [&dyn CmgStorage; 4] = [
            &self.topology,
            &self.assembly_workspace,
            &self.numeric_weights,
            &self.nuisance_seed,
        ];
        let bytes = admission::storage(&roots)?
            .checked_add(
                (self.profile.effect_order.capacity() * std::mem::size_of::<usize>()) as u128,
            )
            .and_then(|bytes| {
                bytes.checked_add(
                    (self.design_identity.effects.capacity()
                        * std::mem::size_of::<PreparedCmgEffectIdentity>())
                        as u128,
                )
            })
            .ok_or_else(|| allocation("prepared CMG retained owner overflows"))?;
        usize::try_from(bytes)
            .map_err(|_| allocation("prepared CMG retained owner exceeds address space"))
    }

    pub(crate) fn reset_warm_start(&mut self) {
        self.seed_valid = false;
        self.seed_for_frame = false;
    }

    #[cfg(test)]
    pub(crate) fn warm_seed_ready(&self) -> bool {
        self.seed_valid
    }

    /// Called only after the full PPML per-RHS and combined certificates.
    /// Swapping preflighted buffers cannot allocate or change accepted state.
    pub(crate) fn retain_certified_seed(&mut self, mut absorber: CmgAbsorber<'_>) {
        self.reset_warm_start();
        let CmgRoute::Graph(graph) = &mut absorber.route else {
            return;
        };
        let GraphCmgWorkspace::CallerBuffers(caller) = &mut graph.workspace else {
            return;
        };
        if !caller.nuisance_potentials.is_empty()
            && caller.nuisance_potentials.len() == self.nuisance_seed.len()
            && caller
                .nuisance_potentials
                .iter()
                .all(|value| value.is_finite())
            && graph.nuisance_setup.extra_restarts == 0
            && (graph.nuisance_setup.warm_right_hand_sides == 0
                || graph.nuisance_setup.initial_relative_residual < 0.5)
        {
            std::mem::swap(&mut self.nuisance_seed, &mut caller.nuisance_potentials);
            self.seed_valid = true;
        }
    }
}

impl PreparedCmgDesignIdentity {
    fn new(design: &BorrowedInterceptFeDesign<'_>) -> Result<Self> {
        let mut effects = Vec::new();
        effects
            .try_reserve_exact(design.effects().len())
            .map_err(|_| allocation("unable to reserve prepared CMG design identity"))?;
        effects.extend(design.effects().iter().map(|effect| {
            let codes = effect.codes();
            PreparedCmgEffectIdentity {
                codes_address: codes.as_ptr() as usize,
                codes_len: codes.len(),
                levels: effect.n_levels(),
                level_index_address: effect.level_index() as *const _ as usize,
            }
        }));
        Ok(Self {
            observations: design.nobs(),
            effects,
        })
    }

    fn matches(&self, design: &BorrowedInterceptFeDesign<'_>) -> bool {
        self.observations == design.nobs()
            && self.effects.len() == design.effects().len()
            && self
                .effects
                .iter()
                .zip(design.effects())
                .all(|(identity, effect)| {
                    let codes = effect.codes();
                    identity.codes_address == codes.as_ptr() as usize
                        && identity.codes_len == codes.len()
                        && identity.levels == effect.n_levels()
                        && identity.level_index_address == effect.level_index() as *const _ as usize
                })
    }
}

impl<'a> CmgAbsorber<'a> {
    pub(crate) fn build(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'a>,
        config: CmgConfig,
        structural_constant_if_empty: bool,
    ) -> Result<Self> {
        let mut attempted_peak = SolverWorkspacePeak::ZERO;
        Self::build_with_workspace(
            design,
            weights,
            config,
            structural_constant_if_empty,
            &mut attempted_peak,
        )
    }

    pub(crate) fn build_with_workspace(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'a>,
        config: CmgConfig,
        structural_constant_if_empty: bool,
        attempted_peak: &mut SolverWorkspacePeak,
    ) -> Result<Self> {
        let profile = CmgTopologyProfile::analyze(&design)?;
        Self::build_profiled_internal(
            design,
            weights,
            config,
            structural_constant_if_empty,
            profile,
            true,
            None,
            attempted_peak,
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
        let mut attempted_peak = SolverWorkspacePeak::ZERO;
        Self::build_profiled_internal(
            design,
            weights,
            config,
            false,
            profile,
            false,
            Some(prepared),
            &mut attempted_peak,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_profiled_internal(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'a>,
        config: CmgConfig,
        structural_constant_if_empty: bool,
        profile: CmgTopologyProfile,
        retain_setup_failure: bool,
        prepared: Option<&mut PreparedCmgTopology>,
        attempted_peak: &mut SolverWorkspacePeak,
    ) -> Result<Self> {
        validate_config(config)?;
        if config.components && config.requested_solver != FeSolverKind::Cmg {
            return Err(invalid_argument(
                "component CMG requires an explicit CMG request",
            ));
        }
        let weight_scale = validate_weights(weights, design.nobs())?;
        let setup_started = Instant::now();
        let mut memory = CmgWorkspacePlan::EMPTY;
        let route = if design.effects().len() < 2 {
            CmgRoute::Direct
        } else {
            validate_nuisance_limit(&profile)?;
            let graph = if let Some(prepared) = prepared {
                preflight_prepared_cmg_memory(&design, &profile, config, prepared).and_then(
                    |plan| {
                        memory = plan;
                        *attempted_peak = attempted_peak.max(plan.construction);
                        build_prepared_graph_route(
                            &design,
                            weights,
                            weight_scale,
                            config,
                            &profile,
                            prepared,
                            &mut memory,
                        )
                    },
                )
            } else {
                preflight_cmg_memory(&design, &profile, config).and_then(|plan| {
                    memory = plan;
                    *attempted_peak = attempted_peak.max(plan.construction);
                    build_graph_route(
                        &design,
                        weights,
                        weight_scale,
                        config,
                        &profile,
                        &mut memory,
                    )
                })
            };
            if graph.is_err() && memory.construction != SolverWorkspacePeak::ZERO {
                // A failed opaque constructor can have released temporary
                // owners before returning. Do not claim a complete forecast.
                memory.construction = memory.construction.with_expected_heap(None)?;
                memory.projection = memory.projection.with_expected_heap(None)?;
            }
            *attempted_peak = attempted_peak.max(memory.construction);
            match graph {
                Ok(graph) => CmgRoute::Graph(graph),
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
                nuisance_reused_right_hand_sides: 0,
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
            memory,
        })
    }

    pub(crate) fn project_unpolished(
        &mut self,
        values: &mut DenseMatrix,
        relative_tolerance: f64,
    ) -> Result<CmgUnpolishedProjectionReport> {
        self.project_unpolished_with_seed(values, relative_tolerance, None)
    }

    /// Only the frozen-control PPML layout may use a seed: controls first,
    /// followed by one working response. Selective retries use the cold API.
    pub(crate) fn project_unpolished_with_seed(
        &mut self,
        values: &mut DenseMatrix,
        relative_tolerance: f64,
        mut seed: Option<&mut CmgControlSeed>,
    ) -> Result<CmgUnpolishedProjectionReport> {
        let use_seed = seed.as_mut().is_some_and(|seed| {
            seed.consume(match &self.route {
                CmgRoute::Graph(graph) => graph.nuisance_setup.weight_ratio,
                _ => f64::INFINITY,
            })
        });
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
        let (nuisance_columns, pcg_batch_width) = match &self.route {
            CmgRoute::Graph(graph) => (
                graph.nuisance.len(),
                graph.pcg_batch_width(self.config.batch_size),
            ),
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
        #[cfg(test)]
        let executor_threads = match &self.route {
            CmgRoute::Graph(graph) => graph.solver.executor().thread_count(),
            CmgRoute::Direct | CmgRoute::Failed(_) => unreachable!("CMG route checked above"),
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
                seed.map(|seed| (seed, use_seed)),
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
            nuisance_reused_right_hand_sides: metrics.nuisance_reused_right_hand_sides,
            pcg_batch_width,
            nuisance_setup: match &self.route {
                CmgRoute::Graph(graph) => graph.nuisance_setup,
                _ => unreachable!("CMG route checked above"),
            },
            projection_work: match &self.route {
                CmgRoute::Graph(graph) => graph.projection_work,
                _ => unreachable!("CMG route checked above"),
            },
            #[cfg(test)]
            executor_threads,
        })
    }

    pub(crate) fn expected_heap_peak(&self) -> SolverWorkspacePeak {
        self.memory.construction
    }

    fn retained_workspace_peak(&self) -> Result<SolverWorkspacePeak> {
        match &self.route {
            CmgRoute::Graph(graph)
                if matches!(graph.workspace, GraphCmgWorkspace::OlsBuffers(_)) =>
            {
                let admitted = cmg_retained_workspace_bound_bytes(graph, None)?;
                SolverWorkspacePeak::from_bytes(admitted)?.with_expected_heap(Some(
                    admitted - admission::runtime_extra(graph.runtime_estimate),
                ))
            }
            CmgRoute::Direct | CmgRoute::Failed(_) => Ok(SolverWorkspacePeak::ZERO),
            // Prepared PPML topology and caller buffers require their parent
            // action carrier; they do not enter owned OLS absorption.
            _ => Ok(SolverWorkspacePeak::UNMODELED),
        }
    }

    /// Residualize one owned matrix. Failure never exposes a partially updated
    /// matrix to the caller; a valid partial CMG projection can still seed the
    /// existing certified CG fallback.
    pub(crate) fn absorb(&mut self, values: DenseMatrix) -> Result<CertifiedBorrowedMap> {
        let phase_started = crate::diagnostics::phase_timing_enabled().then(Instant::now);
        if values.nrows() != self.design.nobs() || values.ncols() == 0 {
            return Err(invalid_argument(
                "CMG design and matrix dimensions are inconsistent",
            ));
        }
        if values.as_slice().iter().any(|value| !value.is_finite()) {
            return Err(invalid_data("CMG input values must be finite"));
        }
        let (reference_norms, norm_workspace) =
            weighted_column_norms(&values, self.weights, self.weight_scale)?;
        let norm_peak = norm_workspace.with_retained_peak(self.retained_workspace_peak()?)?;
        let norm_bytes = reference_norms.capacity() as u128 * 8;
        let norm_seconds = phase_started.map_or(0.0, |started| started.elapsed().as_secs_f64());
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
            report.solver_workspace_peak = self
                .memory
                .construction
                .max(report.solver_workspace_peak.with_retained(norm_bytes)?)
                .max(norm_peak);
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
            report.solver_workspace_peak = self
                .memory
                .construction
                .max(report.solver_workspace_peak.with_retained(norm_bytes)?)
                .max(norm_peak);
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
        let pcg_options = self.config.pcg_options();
        let bulk = match &mut self.route {
            CmgRoute::Graph(graph) => graph.project_matrix_with_options(
                &self.design,
                self.weights,
                self.weight_scale,
                &mut candidate,
                self.config,
                pcg_options,
                None,
            ),
            CmgRoute::Direct => unreachable!("direct CMG route returned above"),
            CmgRoute::Failed(_) => unreachable!("failed CMG route returned above"),
        };
        let bulk_seconds = solve_started.elapsed().as_secs_f64();

        self.certify_graph_projection(
            candidate,
            bulk,
            reference_norms,
            norm_peak,
            setup_metrics,
            setup_seconds,
            solve_started,
            bulk_seconds,
            norm_seconds,
            phase_started.is_some(),
        )
    }

    // A graph projection has a distinct certification/fallback boundary. The
    // raw norms survive it; no speculative result or temporary array escapes.
    #[allow(clippy::too_many_arguments)]
    fn certify_graph_projection(
        &mut self,
        candidate: DenseMatrix,
        bulk: Result<(CmgSolveMetrics, Option<FeCoefficientBatch>)>,
        reference_norms: Vec<f64>,
        norm_peak: SolverWorkspacePeak,
        setup_metrics: CmgSolveMetrics,
        setup_seconds: f64,
        solve_started: Instant,
        bulk_seconds: f64,
        norm_seconds: f64,
        trace: bool,
    ) -> Result<CertifiedBorrowedMap> {
        let norm_bytes = reference_norms.capacity() as u128 * 8;
        match bulk {
            Ok((mut metrics, coefficients)) => {
                let coefficient_bytes = coefficients
                    .as_ref()
                    .map_or(0, FeCoefficientBatch::retained_bytes);
                let graph_retained = self.retained_workspace_peak()?;
                let bulk_peak = self.memory.projection.with_retained(coefficient_bytes)?;
                metrics.include_from(setup_metrics);
                let polished = polish_borrowed_intercept_cg_with_products_certified(
                    &self.design,
                    self.weights,
                    candidate,
                    self.config.map,
                    &reference_norms,
                    false,
                )?;
                let (residualized, mut report, correction) = polished.into_full_parts();
                if trace {
                    eprintln!(
                        "FEREG_PHASE|cmg|norms={norm_seconds:.9}|bulk={bulk_seconds:.9}|polish={:.9}",
                        solve_started.elapsed().as_secs_f64() - bulk_seconds,
                    );
                }
                let polish_peak = report
                    .solver_workspace_peak
                    .with_retained_peak(graph_retained)?
                    .with_retained(coefficient_bytes)?;
                report.solver_workspace_peak = self
                    .memory
                    .construction
                    .max(bulk_peak.max(polish_peak).with_retained(norm_bytes)?)
                    .max(norm_peak);
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
                if polish_iterations == 0 || correction.is_some() {
                    if let Some(mut coefficients) = coefficients {
                        if let Some(correction) = correction {
                            coefficients.add_assign(&correction, 1.0)?;
                        }
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
                // The failed bulk has released its temporary arrays before
                // CG. Conservatively retain its admitted projection envelope.
                let bulk_peak = if self.config.retain_coefficients {
                    SolverWorkspacePeak::UNMODELED
                } else {
                    self.memory.projection
                };
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
                report.solver_workspace_peak = self
                    .memory
                    .construction
                    .max(
                        bulk_peak
                            .max(report.solver_workspace_peak)
                            .with_retained(norm_bytes)?,
                    )
                    .max(norm_peak);
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

    /// Low-storage OLS reconstructs its residual from raw columns, so the
    /// full FE projection must precede the final, stricter certificate.
    pub(crate) fn absorb_raw_final_residual(
        mut self,
        values: DenseMatrix,
        tolerance: f64,
    ) -> Result<CertifiedBorrowedMap> {
        self.config.map.tolerance = self.config.map.tolerance.min(tolerance);
        self.absorb(values)
    }

    /// Final OLS residuals have already been formed from certified within
    /// coordinates and are normally FE-orthogonal. Running graph PCG on that
    /// near-zero right-hand side can exhaust its relative iteration budget.
    /// Certify first and perform a CG correction only when one is actually
    /// required. Release the hierarchy before reserving the admitted
    /// correction workspace; final certification remains independent.
    pub(crate) fn absorb_final_residual(
        self,
        values: DenseMatrix,
        tolerance: f64,
    ) -> Result<CertifiedBorrowedMap> {
        if values.nrows() != self.design.nobs() || values.ncols() == 0 {
            return Err(invalid_argument(
                "CMG final-residual design and matrix dimensions are inconsistent",
            ));
        }
        let mut config = self.config;
        config.map.tolerance = config.map.tolerance.min(tolerance);
        let retained = self.retained_workspace_peak()?;
        let (certificate, certificate_workspace) =
            super::borrowed_map::certify_borrowed_intercept_candidate_with_workspace(
                &self.design,
                self.weights,
                &values,
                config.map,
            )?;
        let certificate_peak = certificate_workspace.with_retained_peak(retained)?;
        if !certificate.passed {
            drop(self.route);
            ensure_cg_fallback_fits(&self.design, values.ncols(), config)?;
            let (reference_norms, norm_peak) =
                weighted_column_norms(&values, self.weights, self.weight_scale)?;
            let corrected = polish_borrowed_intercept_cg_with_products_certified(
                &self.design,
                self.weights,
                values,
                config.map,
                &reference_norms,
                false,
            )?;
            let (residualized, mut report, mut coefficients) = corrected.into_full_parts();
            if !config.retain_coefficients {
                coefficients = None;
            }
            let correction_peak = report
                .solver_workspace_peak
                .with_retained(reference_norms.capacity() as u128 * 8)?;
            report.solver_workspace_peak = self
                .memory
                .construction
                .max(certificate_peak)
                .max(correction_peak)
                .max(norm_peak);
            report.requested_solver = config.requested_solver;
            report.used_solver = FeSolverKind::Cg;
            report.fallback_used = true;
            report.setup_seconds += self.pending_setup_seconds;
            report
                .warnings
                .push("final CMG residual required a certified CG correction".to_owned());
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
        let normal_equation_residual = certificate.relative_two_norm;
        CertifiedBorrowedMap::from_certified_parts(
            values,
            SolveReport {
                checkpoint_retries: 0,
                solver_workspace_peak: self.memory.construction.max(certificate_peak),
                requested_solver: self.config.requested_solver,
                used_solver: FeSolverKind::Cmg,
                iterations: 0,
                stop_reason: SolveStopReason::Certified,
                certificate,
                normal_equation_residual,
                setup_seconds: 0.0,
                solve_seconds: 0.0,
                certification_seconds: 0.0,
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
    fn pcg_batch_width(&self, capacity: usize) -> usize {
        let concurrency = match &self.workspace {
            GraphCmgWorkspace::CallerBuffers(caller) => caller.workspaces.len(),
            GraphCmgWorkspace::OlsBuffers(_) => 1,
        };
        // Upstream executes each caller batch in workspace-sized waves. Avoid
        // a partial wave at every capacity boundary (e.g. 7+3 for capacity 10)
        // without enlarging the admitted RHS or workspace pool. OLS's owned
        // route and a serial pool retain their original batch boundaries.
        (capacity / concurrency) * concurrency
    }

    #[allow(clippy::too_many_arguments)]
    fn project_matrix_with_options(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        weight_scale: f64,
        values: &mut DenseMatrix,
        config: CmgConfig,
        pcg_options: PcgOptions,
        seed: Option<(&mut CmgControlSeed, bool)>,
    ) -> Result<(CmgSolveMetrics, Option<FeCoefficientBatch>)> {
        let mut metrics = CmgSolveMetrics::default();
        self.projection_work = CmgProjectionWork::default();
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
                seed,
            },
        )?;
        metrics.include_from(core_metrics);
        if self.nuisance.is_empty() || self.nuisance_factor.rank == 0 {
            return Ok((metrics, coefficients));
        }

        let columns = values.ncols();
        let nuisance_columns = self.nuisance.len();
        let workspace_width = config.batch_size.min(columns);
        let gamma_elements = workspace_width
            .checked_mul(nuisance_columns)
            .ok_or_else(|| allocation("CMG nuisance coefficient workspace overflows"))?;
        let mut gamma = zero_vec(
            gamma_elements,
            "unable to reserve CMG nuisance coefficient workspace",
        )?;
        let mut moments = zero_vec(
            nuisance_columns,
            "unable to reserve CMG nuisance moment workspace",
        )?;
        let mut moment_corrections = zero_vec(
            nuisance_columns,
            "unable to reserve CMG nuisance moment corrections",
        )?;
        let mut factor_work = zero_vec(
            nuisance_columns,
            "unable to reserve CMG nuisance factor work",
        )?;
        let mut correction = DenseMatrix::zeros(values.nrows(), workspace_width)?;
        for start in (0..columns).step_by(config.batch_size) {
            let end = start.saturating_add(config.batch_size).min(columns);
            let width = end - start;
            correction.resize_zeroed_within_capacity(values.nrows(), width)?;
            for (local, column) in (start..end).enumerate() {
                nuisance_moments_into(
                    design,
                    &self.nuisance_effects,
                    weights,
                    weight_scale,
                    values,
                    column,
                    (&mut moments, &mut moment_corrections),
                )?;
                let gamma_start = local * nuisance_columns;
                self.nuisance_factor.solve_into(
                    &moments,
                    &mut gamma[gamma_start..gamma_start + nuisance_columns],
                    &mut factor_work,
                )?;
            }
            if let Some(batch) = coefficients.as_mut() {
                for local in 0..width {
                    let nuisance_coefficients =
                        &gamma[local * nuisance_columns..(local + 1) * nuisance_columns];
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
            for row in 0..values.nrows() {
                for local in 0..width {
                    let nuisance_coefficients =
                        &gamma[local * nuisance_columns..(local + 1) * nuisance_columns];
                    correction[(row, local)] =
                        nuisance_value(design, &self.nuisance_effects, nuisance_coefficients, row)?;
                }
            }
            if let GraphCmgWorkspace::CallerBuffers(caller) = &mut self.workspace {
                // These potentials and the Schur factor belong to this exact
                // current-weight operator. Linearity replaces only a duplicate
                // solve of U*gamma; PPML still certifies every completed RHS.
                let vertices = self.solver.graph().vertex_count();
                metrics.nuisance_reused_right_hand_sides = metrics
                    .nuisance_reused_right_hand_sides
                    .saturating_add(width);
                for local in 0..width {
                    let solution = &mut caller.solutions[local * vertices..(local + 1) * vertices];
                    solution.fill(0.0);
                    for (direction, &coefficient) in gamma
                        [local * nuisance_columns..(local + 1) * nuisance_columns]
                        .iter()
                        .enumerate()
                    {
                        let potential = &caller.nuisance_potentials
                            [direction * vertices..(direction + 1) * vertices];
                        for (value, &basis) in solution.iter_mut().zip(potential) {
                            *value += coefficient * basis;
                        }
                    }
                    if let Some(batch) = coefficients.as_mut() {
                        record_core_solution(
                            design,
                            self.core_effects,
                            batch,
                            start + local,
                            -1.0,
                            solution,
                        )?;
                    }
                    subtract_core_fit(design, self.core_effects, &mut correction, local, solution)?;
                }
            } else {
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
                        seed: None,
                    },
                )?;
                metrics.include_from(correction_metrics);
            }
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
            mut seed,
        } = request;
        if let Some((seed, _)) = &seed {
            if !matches!(self.workspace, GraphCmgWorkspace::CallerBuffers(_))
                || Some(seed.potentials.len())
                    != self
                        .solver
                        .graph()
                        .vertex_count()
                        .checked_mul(values.ncols() - 1)
            {
                return Err(invalid_argument(
                    "CMG control seed and frozen RHS dimensions differ",
                ));
            }
        }
        let mut metrics = CmgSolveMetrics::default();
        let batch_width = self.pcg_batch_width(config.batch_size);
        let trace_ols = matches!(self.workspace, GraphCmgWorkspace::OlsBuffers(_))
            && crate::diagnostics::phase_timing_enabled();
        let mut start = 0;
        while start < values.ncols() {
            let seed_end = if seed.as_ref().is_some_and(|(_, enabled)| *enabled)
                && start < values.ncols() - 1
            {
                values.ncols() - 1
            } else {
                values.ncols()
            };
            let end = start.saturating_add(batch_width).min(seed_end);
            match &mut self.workspace {
                GraphCmgWorkspace::OlsBuffers(caller)
                | GraphCmgWorkspace::CallerBuffers(caller) => {
                    let width = end - start;
                    let vertices = self.solver.graph().vertex_count();
                    let rhs_started = Instant::now();
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
                    let rhs_seconds = rhs_started.elapsed().as_secs_f64();
                    self.projection_work.rhs_seconds += rhs_seconds;
                    let mut batch_seed = seed.as_mut().filter(|_| start < values.ncols() - 1);
                    let guesses = batch_seed
                        .as_ref()
                        .filter(|(_, use_seed)| *use_seed)
                        .map(|(seed, _)| &seed.potentials[start * vertices..end * vertices]);
                    let solve_started = Instant::now();
                    caller.solve(&self.solver, width, pcg_options, guesses)?;
                    let solve_seconds = solve_started.elapsed().as_secs_f64();
                    self.projection_work.pcg_seconds += solve_seconds;
                    let subtract_started = trace_ols.then(Instant::now);
                    let was_seeded = guesses.is_some();
                    if let Some((seed, _)) = &mut batch_seed {
                        let retained_end = end.min(values.ncols() - 1);
                        seed.potentials[start * vertices..retained_end * vertices].copy_from_slice(
                            &caller.solutions[..(retained_end - start) * vertices],
                        );
                    }
                    metrics.include_batch(width);
                    for local in 0..width {
                        let diagnostic = caller.diagnostics[local];
                        self.projection_work.iterations += diagnostic.iterations();
                        self.projection_work.restarts += diagnostic.restarts();
                        self.projection_work.extra_restarts += diagnostic
                            .restarts()
                            .saturating_sub(usize::from(diagnostic.iterations() > 0));
                        if was_seeded {
                            self.projection_work.warm_right_hand_sides += 1;
                            self.projection_work.warm_iterations += diagnostic.iterations();
                            let rhs_norm = caller.right_hand_sides
                                [local * vertices..(local + 1) * vertices]
                                .iter()
                                .map(|value| value * value)
                                .sum::<f64>()
                                .sqrt();
                            let relative = diagnostic.initial_residual_norm()
                                / rhs_norm.max(f64::MIN_POSITIVE);
                            self.projection_work.initial_relative_residual = if relative.is_finite()
                            {
                                self.projection_work.initial_relative_residual.max(relative)
                            } else {
                                f64::INFINITY
                            };
                        }
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
                    if let Some(started) = subtract_started {
                        eprintln!("FEREG_PHASE|cmg_batch|rhs={rhs_seconds:.9}|solve={solve_seconds:.9}|subtract={:.9}|width={width}|route={:?}",
                            started.elapsed().as_secs_f64(), self.solver.select_batch_execution(width).map_err(cmg_error)?.execution());
                    }
                }
            }
            start = end;
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
    seed: Option<(&'a mut CmgControlSeed, bool)>,
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
        self.nuisance_reused_right_hand_sides = self
            .nuisance_reused_right_hand_sides
            .saturating_add(other.nuisance_reused_right_hand_sides);
    }
}

// Keep the legacy workspace and ownership boundaries explicit during migration.
#[allow(clippy::too_many_arguments)]
fn build_graph_route(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    config: CmgConfig,
    profile: &CmgTopologyProfile,
    memory: &mut CmgWorkspacePlan,
) -> Result<GraphCmg> {
    let trace_enabled = crate::diagnostics::solver_trace_enabled();
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
    let estimate = admission::runtime(config)?;
    let runtime_budget = admission::admit(config, 0, estimate.total_bytes(), memory)?;
    memory.record_expected_heap(estimate.worker_handles_bytes() as u128)?;
    let (executor, runtime_estimate) = ParallelExecutor::new_with_estimated_memory_budget(
        parallel,
        ParallelRuntimeAllowance::default(),
        runtime_budget,
    )
    .map_err(cmg_error)?;
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
    // Duplicate aggregation compacts in place: retained edge capacity still
    // covers the raw observation bank. Sorting is in-place in the pinned CMG.
    memory.record_expected_heap(admission::storage(&[&graph, &executor])?)?;
    let graph_seconds = graph_started.map_or(0.0, |started| started.elapsed().as_secs_f64());
    build_graph_route_from_numeric(
        design,
        weights,
        weight_scale,
        config,
        profile,
        graph,
        executor,
        None,
        trace_enabled,
        graph_seconds,
        runtime_estimate,
        memory,
    )
}

fn build_prepared_graph_route(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    config: CmgConfig,
    profile: &CmgTopologyProfile,
    prepared: &mut PreparedCmgTopology,
    memory: &mut CmgWorkspacePlan,
) -> Result<GraphCmg> {
    let trace_enabled = crate::diagnostics::solver_trace_enabled();
    let parallel = ParallelOptions {
        threads: config.threads.resolved(),
        ..ParallelOptions::default()
    };
    let estimate = admission::runtime(config)?;
    let runtime_budget = admission::admit(config, 0, estimate.total_bytes(), memory)?;
    memory.record_expected_heap(estimate.worker_handles_bytes() as u128)?;
    let (executor, runtime_estimate) = ParallelExecutor::new_with_estimated_memory_budget(
        parallel,
        ParallelRuntimeAllowance::default(),
        runtime_budget,
    )
    .map_err(cmg_error)?;
    let numeric_bytes = prepared
        .topology
        .numeric_allocation_bytes()
        .map_err(cmg_error)?;
    admission::admit(
        config,
        runtime_estimate.total_bytes() as u128,
        numeric_bytes,
        memory,
    )?;
    memory.record_expected_heap(estimate.worker_handles_bytes() as u128 + numeric_bytes as u128)?;
    let graph_started = trace_enabled.then(Instant::now);
    let graph = prepared.assemble(design, profile, weights, weight_scale)?;
    let graph_seconds = graph_started.map_or(0.0, |started| started.elapsed().as_secs_f64());
    build_graph_route_from_numeric(
        design,
        weights,
        weight_scale,
        config,
        profile,
        graph,
        executor,
        Some(prepared),
        trace_enabled,
        graph_seconds,
        runtime_estimate,
        memory,
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
    prepared: Option<&PreparedCmgTopology>,
    trace_enabled: bool,
    graph_seconds: f64,
    runtime_estimate: ParallelRuntimeEstimate,
    memory: &mut CmgWorkspacePlan,
) -> Result<GraphCmg> {
    let core_effects = profile
        .core_effects
        .ok_or_else(|| invalid_argument("CMG graph route requires two absorbed effects"))?;
    let hierarchy_started = trace_enabled.then(Instant::now);
    let hierarchy_options = prepared.map_or_else(CmgOptions::default, |_| {
        prepared_cmg_options(profile, &graph)
    });
    let bound = admission::construction_bound(
        graph.vertex_count(),
        graph.edge_count(),
        hierarchy_options,
        config,
    )?;
    let parents = admission::frame_storage(&[&graph, &executor], prepared)?
        + admission::runtime_extra(runtime_estimate);
    let construction_budget = admission::admit(config, parents, bound, memory)?;
    let preconditioner = if config.components {
        CmgPreconditioner::build_component_experiment_with_executor_and_cmg_budget(
            &graph,
            hierarchy_options,
            admission::component_options(),
            &executor,
            construction_budget,
        )
    } else {
        CmgPreconditioner::build_with_executor_and_cmg_budget(
            &graph,
            hierarchy_options,
            &executor,
            construction_budget,
        )
    }
    .map_err(cmg_error)?;
    memory.record_expected_heap(forecast::hierarchy_peak(
        &graph,
        &executor,
        &preconditioner,
        prepared,
        config.components,
    )?)?;
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
    let solver_started = trace_enabled.then(Instant::now);
    let solver = ParallelPcgSolver::from_preconditioner(
        preconditioner,
        executor,
        ParallelPcgPolicy::default(),
    )
    .map_err(cmg_error)?;
    if admission::may_need_plan(&solver) {
        let required = solver
            .plan_construction_allocation_bound()
            .map_err(cmg_error)?;
        let parents = admission::frame_storage(&[&solver], prepared)?
            + admission::runtime_extra(runtime_estimate);
        let budget = admission::admit(config, parents, required, memory)?;
        memory.record_expected_heap(
            parents - admission::runtime_extra(runtime_estimate) + required as u128,
        )?;
        solver.try_plan_with_budget(budget).map_err(cmg_error)?;
    }
    let caller = CallerPcgBuffers::new(
        &solver,
        config.batch_size,
        if prepared.is_some() {
            profile.nuisance_columns
        } else {
            0
        },
        config,
        runtime_estimate,
        prepared,
        memory,
    )?;
    let workspace = if prepared.is_some() {
        GraphCmgWorkspace::CallerBuffers(caller)
    } else {
        GraphCmgWorkspace::OlsBuffers(caller)
    };
    let solver_seconds = solver_started.map_or(0.0, |started| started.elapsed().as_secs_f64());
    let metadata_bytes = profile
        .nuisance_columns
        .checked_mul(std::mem::size_of::<NuisanceColumn>())
        .and_then(|bytes| {
            bytes.checked_add(
                profile
                    .effect_order
                    .len()
                    .checked_mul(std::mem::size_of::<NuisanceEffect>())?,
            )
        })
        .ok_or_else(|| allocation("CMG nuisance metadata overflows"))?;
    let caller = match &workspace {
        GraphCmgWorkspace::OlsBuffers(caller) | GraphCmgWorkspace::CallerBuffers(caller) => caller,
    };
    let parents = admission::frame_storage(
        &[
            &solver,
            &caller.workspaces,
            &caller.right_hand_sides,
            &caller.solutions,
            &caller.nuisance_potentials,
        ],
        prepared,
    )? + admission::runtime_extra(runtime_estimate)
        + (caller.diagnostics.capacity() * std::mem::size_of::<PcgDiagnostics>()) as u128;
    admission::admit(config, parents, metadata_bytes, memory)?;
    memory.record_expected_heap(
        parents - admission::runtime_extra(runtime_estimate) + metadata_bytes as u128,
    )?;
    let nuisance = build_nuisance_columns(design, &profile.effect_order[2..])?;
    let nuisance_effects = build_nuisance_effects(design, &profile.effect_order[2..])?;
    let mut route = GraphCmg {
        core_effects,
        nuisance,
        nuisance_effects,
        nuisance_factor: PivotedPsdFactor::empty(),
        solver,
        runtime_estimate,
        workspace,
        setup_iterations: 0,
        setup_backward_error: 0.0,
        setup_right_hand_sides: 0,
        setup_batches: 0,
        nuisance_setup: CmgNuisanceSetup {
            weight_ratio: prepared.map_or(0.0, |state| state.weight_ratio),
            ..CmgNuisanceSetup::default()
        },
        projection_work: CmgProjectionWork::default(),
    };
    let nuisance_bound = profile
        .nuisance_columns
        .checked_mul(profile.nuisance_columns)
        .and_then(|cells| cells.checked_mul(32))
        .ok_or_else(|| allocation("CMG nuisance factor overflows"))?;
    let parents = cmg_retained_workspace_bound_bytes(&route, prepared)?;
    admission::admit(config, parents, nuisance_bound, memory)?;
    memory.record_expected_heap(
        parents - admission::runtime_extra(runtime_estimate) + nuisance_bound as u128,
    )?;
    let nuisance_started = trace_enabled.then(Instant::now);
    let (factor, metrics) = build_nuisance_factor(
        design,
        weights,
        weight_scale,
        &mut route,
        config,
        prepared
            .filter(|state| state.seed_for_frame)
            .map(|state| state.nuisance_seed.as_slice()),
    )?;
    let nuisance_seconds = nuisance_started.map_or(0.0, |started| started.elapsed().as_secs_f64());
    route.nuisance_factor = factor;
    ensure_cmg_exact_retained_memory_fits(&route, config, prepared)?;
    let retained = cmg_retained_workspace_bound_bytes(&route, prepared)?;
    let projection = nuisance_projection_workspace_bytes(
        design.nobs(),
        profile.nuisance_columns,
        config.batch_size,
    )?;
    admission::admit(config, retained, projection, memory)?;
    memory.record_expected_heap(
        retained - admission::runtime_extra(runtime_estimate) + projection as u128,
    )?;
    if trace_enabled {
        eprintln!("FEREG_CMG_MEMORY|policy=estimated-v1|threads={}|runtime_heap_allowance={}|worker_stack_allowance={}|runtime_heap_unbounded=1|retained_with_allowance={retained}|components={}",
            runtime_estimate.threads(), runtime_estimate.runtime_heap_allowance_bytes(), runtime_estimate.worker_stack_allowance_bytes(), config.components);
    }
    route.setup_iterations = metrics.iterations;
    route.setup_backward_error = metrics.backward_error;
    route.setup_right_hand_sides = metrics.right_hand_sides;
    route.setup_batches = metrics.batches;
    if trace_enabled {
        eprintln!(
            "FEREG_CMG_SETUP|graph={graph_seconds:.9}|hierarchy={hierarchy_seconds:.9}|solver_workspace={solver_seconds:.9}|nuisance={nuisance_seconds:.9}|nuisance_columns={}|nuisance_iterations={}|low_effective_degree_threshold={:.17e}",
            route.nuisance.len(),
            metrics.iterations,
            hierarchy_options.low_effective_degree_threshold,
        );
    }
    Ok(route)
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

fn build_nuisance_effects(
    design: &BorrowedInterceptFeDesign<'_>,
    effect_order: &[usize],
) -> Result<Vec<NuisanceEffect>> {
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(effect_order.len())
        .map_err(|_| allocation("unable to reserve CMG nuisance effect map"))?;
    let mut first_column = 0_usize;
    for &effect in effect_order {
        let columns = design.effects()[effect].n_levels().saturating_sub(1);
        if columns > 0 {
            effects.push(NuisanceEffect {
                effect,
                first_column,
                columns,
            });
        }
        first_column = first_column
            .checked_add(columns)
            .ok_or_else(|| allocation("CMG nuisance effect map overflows"))?;
    }
    Ok(effects)
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
) -> Result<CmgWorkspacePlan> {
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
    let nuisance_projection = nuisance_projection_workspace_bytes(
        design.nobs(),
        profile.nuisance_columns,
        config.batch_size,
    )?;
    let retained_coefficients = if config.retain_coefficients {
        design
            .total_term_slots()
            .checked_mul(config.batch_size)
            .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
            .ok_or_else(|| allocation("CMG retained coefficient estimate overflows"))?
    } else {
        0
    };
    let runtime = admission::runtime(config)?.total_bytes() as u128;
    let required = (estimate.build_peak_bytes() as u128)
        .checked_add(runtime)
        .and_then(|value| value.checked_add(batch_vectors as u128))
        .and_then(|value| value.checked_add(nuisance_square as u128))
        .and_then(|value| value.checked_add(nuisance_projection as u128))
        .and_then(|value| value.checked_add(retained_coefficients as u128))
        .ok_or_else(|| allocation("CMG complete memory estimate overflows"))?;
    if config
        .solver_memory_budget
        .is_none_or(|budget| budget.fits(required))
    {
        Ok(CmgWorkspacePlan {
            construction: SolverWorkspacePeak::from_bytes(required)?.with_expected_heap(Some(0))?,
            projection: SolverWorkspacePeak::from_bytes(required - retained_coefficients as u128)?
                .with_expected_heap(Some(0))?,
        })
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
) -> Result<CmgWorkspacePlan> {
    prepared.ensure_matches(design, profile)?;
    let problem = CmgProblemSize {
        vertices: prepared.topology.vertex_count(),
        input_edges: prepared.topology.input_edge_count(),
        canonical_edges: prepared.topology.canonical_edge_count(),
        right_hand_sides: config.batch_size,
    };
    let required = conservative_prepared_cmg_frame_bytes(design, profile, config, problem)?;
    if config
        .solver_memory_budget
        .is_none_or(|budget| budget.fits(required))
    {
        // Prepared PPML does not retain an OLS coefficient output. Its
        // separately owned topology remains charged by the PPML caller.
        let peak = SolverWorkspacePeak::from_bytes(required)?.with_expected_heap(Some(0))?;
        Ok(CmgWorkspacePlan {
            construction: peak,
            projection: peak,
        })
    } else {
        Err(allocation(format!(
            "prepared CMG requires {required} incremental bytes and does not fit the joint execution/solver memory plan"
        )))
    }
}

fn conservative_prepared_problem(
    profile: &CmgTopologyProfile,
    right_hand_sides: usize,
) -> CmgProblemSize {
    CmgProblemSize {
        vertices: profile.vertices,
        input_edges: profile.observations,
        // Canonical edges cannot exceed submitted observations. Computing the
        // exact count would itself require the preparation being preflighted.
        canonical_edges: profile.observations,
        right_hand_sides,
    }
}

fn nuisance_seed_bytes(profile: &CmgTopologyProfile) -> Result<u128> {
    (profile.vertices as u128)
        .checked_mul(profile.nuisance_columns as u128)
        .and_then(|value| value.checked_mul(8))
        .and_then(|value| value.checked_add((std::mem::size_of::<Vec<f64>>() + 16) as u128))
        .ok_or_else(|| allocation("prepared CMG nuisance seed estimate overflows"))
}

fn prepared_cmg_options(profile: &CmgTopologyProfile, graph: &Laplacian) -> CmgOptions {
    let mut options = CmgOptions::default();
    if profile.nuisance_columns > 0
        && !graph.edges().is_empty()
        && graph.edges().iter().all(|edge| {
            edge.weight() / graph.diagonal()[edge.u()] < options.low_effective_degree_threshold
                && edge.weight() / graph.diagonal()[edge.v()]
                    < options.low_effective_degree_threshold
        })
    {
        // Low-effective-degree pruning can leave almost full-sized coarse
        // graphs when every edge is weak under CMG's existing criterion.
        // Mixed/strong-edge graphs keep pruning: unpruned grouping regressed
        // the measured mixed sparse case. This allocation-free current-weight
        // test changes only the preconditioner, never residual/certificate rules.
        options.low_effective_degree_threshold = 0.0;
    }
    options
}

fn conservative_prepared_cmg_frame_bytes(
    design: &BorrowedInterceptFeDesign<'_>,
    profile: &CmgTopologyProfile,
    config: CmgConfig,
    problem: CmgProblemSize,
) -> Result<u128> {
    let parallel = ParallelOptions {
        threads: config.threads.resolved(),
        ..ParallelOptions::default()
    };
    let estimate = RepeatedPcgMemoryEstimate::conservative(
        problem,
        // Grouping does not alter the default factor, fill or depth bounds.
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
    let legacy_build_peak =
        CmgMemoryEstimate::conservative(problem, CmgOptions::default(), parallel)
            .map_err(cmg_error)?
            .build_peak_bytes();
    let nuisance_square = profile
        .nuisance_columns
        .checked_mul(profile.nuisance_columns)
        .and_then(|value| value.checked_mul(32))
        .ok_or_else(|| allocation("prepared CMG nuisance workspace estimate overflows"))?;
    let nuisance_projection = nuisance_projection_workspace_bytes(
        design.nobs(),
        profile.nuisance_columns,
        config.batch_size,
    )?;
    let retained_coefficients = if config.retain_coefficients {
        design
            .total_term_slots()
            .checked_mul(config.batch_size)
            .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
            .ok_or_else(|| allocation("prepared CMG retained coefficient estimate overflows"))?
    } else {
        0
    };
    let runtime = admission::runtime(config)?.total_bytes() as u128;
    let required = (repeated_frame_bytes.max(legacy_build_peak) as u128)
        .checked_add(runtime)
        .and_then(|value| value.checked_add(estimate.caller_logical_bytes() as u128))
        .and_then(|value| value.checked_add(std::mem::size_of::<Vec<f64>>() as u128))
        .and_then(|value| {
            (profile.vertices as u128)
                .checked_mul(profile.nuisance_columns as u128)
                .and_then(|cells| cells.checked_mul(std::mem::size_of::<f64>() as u128))
                .and_then(|bytes| value.checked_add(bytes))
        })
        .and_then(|value| value.checked_add(nuisance_square as u128))
        .and_then(|value| value.checked_add(nuisance_projection as u128))
        .and_then(|value| value.checked_add(retained_coefficients as u128))
        .ok_or_else(|| allocation("prepared CMG complete memory estimate overflows"))?;
    Ok(required)
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
    let required = cmg_retained_workspace_bound_bytes(graph, prepared)?;
    if budget.fits(required) {
        Ok(())
    } else {
        Err(allocation(format!(
            "CMG retained hierarchy and workspace require {required} incremental bytes and do not fit the joint execution/solver memory plan"
        )))
    }
}

fn cmg_retained_workspace_bound_bytes(
    graph: &GraphCmg,
    prepared: Option<&PreparedCmgTopology>,
) -> Result<u128> {
    let caller = match &graph.workspace {
        GraphCmgWorkspace::OlsBuffers(caller) | GraphCmgWorkspace::CallerBuffers(caller) => caller,
    };
    let upstream_bytes = admission::frame_storage(
        &[
            &graph.solver,
            &caller.workspaces,
            &caller.right_hand_sides,
            &caller.solutions,
            &caller.nuisance_potentials,
        ],
        prepared,
    )? + admission::runtime_extra(graph.runtime_estimate);
    let diagnostics_bytes = caller
        .diagnostics
        .capacity()
        .checked_mul(std::mem::size_of::<PcgDiagnostics>())
        .ok_or_else(|| allocation("CMG diagnostic capacities overflow"))?;
    let nuisance_bytes = graph
        .nuisance
        .capacity()
        .checked_mul(std::mem::size_of::<NuisanceColumn>())
        .and_then(|value| {
            value.checked_add(
                graph
                    .nuisance_effects
                    .capacity()
                    .saturating_mul(std::mem::size_of::<NuisanceEffect>()),
            )
        })
        .and_then(|value| value.checked_add(graph.nuisance_factor.retained_bytes()))
        .ok_or_else(|| allocation("CMG exact retained-memory report overflows"))?;
    let required = upstream_bytes
        .checked_add(diagnostics_bytes as u128)
        .and_then(|value| value.checked_add(nuisance_bytes as u128))
        .ok_or_else(|| allocation("CMG retained storage report overflows"))?;
    Ok(required)
}

fn nuisance_projection_workspace_bytes(
    observations: usize,
    nuisance_columns: usize,
    batch_size: usize,
) -> Result<usize> {
    if nuisance_columns == 0 {
        return Ok(0);
    }
    let correction = observations
        .checked_mul(batch_size)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| allocation("CMG nuisance correction workspace estimate overflows"))?;
    let solve_vectors = nuisance_columns
        .checked_mul(3)
        .ok_or_else(|| allocation("CMG nuisance solve workspace estimate overflows"))?;
    let small_values = batch_size
        .checked_mul(nuisance_columns)
        .and_then(|value| value.checked_add(solve_vectors))
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| allocation("CMG nuisance projection workspace estimate overflows"))?;
    correction
        .checked_add(small_values)
        .ok_or_else(|| allocation("CMG nuisance projection workspace estimate overflows"))
}

fn build_nuisance_factor(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    graph: &mut GraphCmg,
    config: CmgConfig,
    initial_guesses: Option<&[f64]>,
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

    let batch_size = graph.pcg_batch_width(config.batch_size);
    for start in (0..dimension).step_by(batch_size) {
        let end = start.saturating_add(batch_size).min(dimension);
        match &mut graph.workspace {
            GraphCmgWorkspace::OlsBuffers(caller) | GraphCmgWorkspace::CallerBuffers(caller) => {
                let rhs_started = Instant::now();
                let width = end - start;
                let vertices = graph.solver.graph().vertex_count();
                fill_nuisance_rhs_batch_into(
                    &mut caller.right_hand_sides,
                    vertices,
                    design,
                    graph.core_effects,
                    &graph.nuisance_effects,
                    weights,
                    weight_scale,
                    start,
                    end,
                )?;
                graph.nuisance_setup.rhs_seconds += rhs_started.elapsed().as_secs_f64();
                let solve_started = Instant::now();
                let guesses =
                    initial_guesses.map(|values| &values[start * vertices..end * vertices]);
                caller.solve(&graph.solver, width, config.pcg_options(), guesses)?;
                graph.nuisance_setup.pcg_seconds += solve_started.elapsed().as_secs_f64();
                graph.nuisance_setup.warm_right_hand_sides +=
                    if guesses.is_some() { width } else { 0 };
                let schur_started = Instant::now();
                metrics.include_batch(width);
                if !caller.nuisance_potentials.is_empty() {
                    caller.nuisance_potentials[start * vertices..end * vertices]
                        .copy_from_slice(&caller.solutions[..width * vertices]);
                }
                for local in 0..width {
                    let diagnostic = caller.diagnostics[local];
                    graph.nuisance_setup.iterations += diagnostic.iterations();
                    graph.nuisance_setup.restarts += diagnostic.restarts();
                    // CMG counts the mandatory fresh terminal residual as a
                    // restart, even when it immediately accepts the solution.
                    graph.nuisance_setup.extra_restarts += diagnostic
                        .restarts()
                        .saturating_sub(usize::from(diagnostic.iterations() > 0));
                    if guesses.is_some() {
                        let rhs_norm = caller.right_hand_sides
                            [local * vertices..(local + 1) * vertices]
                            .iter()
                            .map(|value| value * value)
                            .sum::<f64>()
                            .sqrt();
                        let relative =
                            diagnostic.initial_residual_norm() / rhs_norm.max(f64::MIN_POSITIVE);
                        graph.nuisance_setup.initial_relative_residual = graph
                            .nuisance_setup
                            .initial_relative_residual
                            .max(if relative.is_finite() {
                                relative
                            } else {
                                f64::INFINITY
                            });
                    }
                    metrics.include(diagnostic.iterations(), diagnostic.backward_error());
                    let column = start + local;
                    accumulate_schur_column(
                        design,
                        graph.core_effects,
                        &graph.nuisance,
                        &graph.nuisance_effects,
                        weights,
                        weight_scale,
                        column,
                        caller.solution(local, vertices)?,
                        &mut schur,
                        &mut corrections,
                    )?;
                }
                graph.nuisance_setup.schur_seconds += schur_started.elapsed().as_secs_f64();
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

#[allow(clippy::too_many_arguments)]
fn fill_nuisance_rhs_batch_into(
    output: &mut [f64],
    vertices: usize,
    design: &BorrowedInterceptFeDesign<'_>,
    core: [usize; 2],
    nuisance_effects: &[NuisanceEffect],
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
        for nuisance_effect in nuisance_effects {
            if let Some(column) = nuisance_effect.column_at_row(design, row) {
                if column < start || column >= end {
                    continue;
                }
                let local = column - start;
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
    nuisance_effects: &[NuisanceEffect],
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
        for nuisance_effect in nuisance_effects {
            if let Some(other) = nuisance_effect.column_at_row(design, row) {
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

fn nuisance_moments_into(
    design: &BorrowedInterceptFeDesign<'_>,
    nuisance_effects: &[NuisanceEffect],
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    values: &DenseMatrix,
    column: usize,
    workspace: (&mut [f64], &mut [f64]),
) -> Result<()> {
    let (moments, corrections) = workspace;
    if moments.len() != corrections.len() {
        return Err(invalid_argument(
            "CMG nuisance moment workspace dimensions are inconsistent",
        ));
    }
    moments.fill(0.0);
    corrections.fill(0.0);
    for row in 0..values.nrows() {
        let weighted = normalized_weight(weights, weight_scale, row) * values[(row, column)];
        for nuisance_effect in nuisance_effects {
            if let Some(index) = nuisance_effect.column_at_row(design, row) {
                if index >= moments.len() {
                    return Err(FeregError::new(
                        ErrorCode::Internal,
                        "CMG nuisance effect map exceeds its moment workspace",
                    ));
                }
                compensated_add(weighted, &mut moments[index], &mut corrections[index])?;
            }
        }
    }
    for (moment, correction) in moments.iter_mut().zip(corrections.iter()) {
        *moment += *correction;
        if !moment.is_finite() {
            return Err(invalid_data("CMG nuisance moment is nonfinite"));
        }
    }
    Ok(())
}

fn nuisance_value(
    design: &BorrowedInterceptFeDesign<'_>,
    nuisance_effects: &[NuisanceEffect],
    coefficients: &[f64],
    row: usize,
) -> Result<f64> {
    let mut value = 0.0;
    for nuisance_effect in nuisance_effects {
        if let Some(index) = nuisance_effect.column_at_row(design, row) {
            let coefficient = coefficients.get(index).ok_or_else(|| {
                invalid_argument("CMG nuisance coefficient dimensions are inconsistent")
            })?;
            value += coefficient;
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
) -> Result<(Vec<f64>, SolverWorkspacePeak)> {
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
    let peak = SolverWorkspacePeak::from_bytes(
        (scales.capacity() as u128 + sums.capacity() as u128 + corrections.capacity() as u128) * 8,
    )?;
    Ok((sums, peak))
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

    fn solve_into(&self, rhs: &[f64], output: &mut [f64], work: &mut [f64]) -> Result<()> {
        if rhs.len() != self.dimension || output.len() != self.dimension || work.len() < self.rank {
            return Err(invalid_argument(
                "CMG nuisance solve dimensions are inconsistent",
            ));
        }
        output.fill(0.0);
        if self.rank == 0 {
            return Ok(());
        }
        work[..self.rank].fill(0.0);
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
        Ok(())
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

fn validate_finite_slice(values: &[f64], message: &'static str) -> Result<()> {
    if values.iter().any(|value| !value.is_finite()) {
        Err(invalid_data(message))
    } else {
        Ok(())
    }
}

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    neumaier_add(value, sum, correction);
    if sum.is_finite() && correction.is_finite() {
        Ok(())
    } else {
        Err(invalid_data("CMG compensated accumulation is nonfinite"))
    }
}

fn cmg_error(error: cmg::CmgError) -> FeregError {
    FeregError::new(cmg_error_code(&error), format!("CMG: {error}"))
}

fn cmg_solve_error(error: cmg::CmgError) -> FeregError {
    FeregError::new(cmg_error_code(&error), format!("CMG solve: {error}"))
}

fn cmg_error_code(error: &cmg::CmgError) -> ErrorCode {
    match error {
        cmg::CmgError::PcgBreakdown { .. }
        | cmg::CmgError::MaximumIterations { .. }
        | cmg::CmgError::ResidualVerificationFailed { .. }
        | cmg::CmgError::IncompatibleLaplacianRhs { .. }
        | cmg::CmgError::NonPositivePivot { .. } => ErrorCode::NonConvergence,
        cmg::CmgError::MemoryBudgetExceeded { .. }
        | cmg::CmgError::AllocationFailed { .. }
        | cmg::CmgError::VertexIndexTooWide { .. } => ErrorCode::Allocation,
        cmg::CmgError::ParallelRuntime { .. } | cmg::CmgError::InvalidHierarchy { .. } => {
            ErrorCode::Internal
        }
        cmg::CmgError::DimensionMismatch { .. } | cmg::CmgError::InvalidOption { .. } => {
            ErrorCode::InvalidArgument
        }
        cmg::CmgError::VertexOutOfBounds { .. }
        | cmg::CmgError::SelfLoop { .. }
        | cmg::CmgError::InvalidEdgeWeight { .. }
        | cmg::CmgError::NonFiniteMatrixValue { .. }
        | cmg::CmgError::NotSymmetric { .. }
        | cmg::CmgError::PositiveOffDiagonal { .. }
        | cmg::CmgError::NegativeDiagonal { .. }
        | cmg::CmgError::NotDiagonallyDominant { .. } => ErrorCode::InvalidData,
        // CmgError is non-exhaustive. A new upstream variant must fail closed
        // until fereg assigns it an explicit public error class.
        _ => ErrorCode::Internal,
    }
}

fn nonconvergence(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

#[cfg(test)]
mod tests {
    use super::super::certify_borrowed_intercept_candidate;
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
    fn prepared_nuisance_grouping_preserves_memory_caps_and_zero_nuisance_policy() {
        let mut profile = CmgTopologyProfile {
            effect_order: vec![0, 1, 2],
            core_effects: Some([0, 1]),
            vertices: 1_200,
            nuisance_columns: 0,
            observations: 25_000,
            highest_cardinality_levels: 1_000,
            second_cardinality_levels: 200,
        };
        let edges = (0..9)
            .flat_map(|left| (9..18).map(move |right| (left, right, 1.0)))
            .collect::<Vec<_>>();
        let graph = Laplacian::from_edges(18, edges).unwrap();
        assert_eq!(
            prepared_cmg_options(&profile, &graph),
            CmgOptions::default()
        );
        for width in [1, 32, 64] {
            profile.nuisance_columns = width;
            let options = prepared_cmg_options(&profile, &graph);
            assert_eq!(options.low_effective_degree_threshold, 0.0);
            assert_eq!(
                CmgOptions {
                    low_effective_degree_threshold: CmgOptions::default()
                        .low_effective_degree_threshold,
                    ..options
                },
                CmgOptions::default(),
                "direct factor, fill and depth bounds cannot grow"
            );
            let problem = CmgProblemSize {
                vertices: profile.vertices,
                input_edges: profile.observations,
                canonical_edges: profile.observations,
                right_hand_sides: 10,
            };
            for threads in [1, 7] {
                let parallel = ParallelOptions {
                    threads,
                    ..ParallelOptions::default()
                };
                assert_eq!(
                    RepeatedPcgMemoryEstimate::conservative(
                        problem,
                        options,
                        parallel,
                        threads > 1,
                        false
                    )
                    .unwrap(),
                    RepeatedPcgMemoryEstimate::conservative(
                        problem,
                        CmgOptions::default(),
                        parallel,
                        threads > 1,
                        false
                    )
                    .unwrap()
                );
            }
        }
        let boundary = Laplacian::from_edges(
            16,
            (0..8).flat_map(|left| (8..16).map(move |right| (left, right, 1.0))),
        )
        .unwrap();
        assert_eq!(
            prepared_cmg_options(&profile, &boundary),
            CmgOptions::default(),
            "equality is not weak"
        );
        let mixed = Laplacian::from_edges(
            18,
            (0..9).flat_map(|left| {
                (9..18).map(move |right| {
                    (
                        left,
                        right,
                        if left == 0 && right == 9 { 100.0 } else { 1.0 },
                    )
                })
            }),
        )
        .unwrap();
        assert_eq!(
            prepared_cmg_options(&profile, &mixed),
            CmgOptions::default(),
            "one strong edge is sufficient; do not use an average"
        );
        assert_eq!(
            prepared_cmg_options(&profile, &graph).low_effective_degree_threshold,
            0.0,
            "recompute eligibility from the current graph, without retained policy state"
        );
    }

    #[test]
    fn prepared_coarsening_certifies_large_permuted_weighted_three_fe_graphs() {
        let rows = 24_000;
        let levels = [800, 160, 5];
        let codes = [
            (0..rows)
                .map(|row| (row % levels[0]) as u32)
                .collect::<Vec<_>>(),
            (0..rows)
                .map(|row| ((row * 17 + (row / levels[0]) * 7) % levels[1]) as u32)
                .collect(),
            (0..rows)
                .map(|row| ((row / 37) % levels[2]) as u32)
                .collect(),
        ];
        let indexes = codes
            .iter()
            .zip(levels)
            .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).unwrap())
            .collect::<Vec<_>>();
        let input = DenseMatrix::from_columns(&[
            (0..rows).map(|row| (row as f64 * 0.31).sin()).collect(),
            (0..rows)
                .map(|row| (row as f64 * 0.17).cos() + codes[2][row] as f64)
                .collect(),
        ])
        .unwrap();
        let mut references: [Option<DenseMatrix>; 3] = Default::default();
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let design = || {
                BorrowedInterceptFeDesign::new(
                    rows,
                    order
                        .iter()
                        .map(|&i| effect(&codes[i], levels[i], &indexes[i]))
                        .collect(),
                )
                .unwrap()
            };
            let mut topology = PreparedCmgTopology::prepare(&design()).unwrap();
            let mut control_seed = CmgControlSeed::new(960, 1).unwrap();
            for (frame, reference) in references.iter_mut().enumerate() {
                let weights = (0..rows)
                    .map(|row| {
                        let value = 0.7 + (row % 11) as f64 * 0.13;
                        if frame == 0 {
                            value
                        } else if frame == 1 {
                            value * (1.0 + (row % 7) as f64 * 0.01)
                        } else if row % 3 == 0 {
                            value * 8.0
                        } else {
                            value
                        }
                    })
                    .collect::<Vec<_>>();
                let mut options = config(4);
                options.map.tolerance = 1e-10;
                options.threads =
                    ThreadConfig::from_requested(if frame == 1 { 7 } else { 1 }, true).unwrap();
                let mut absorber = CmgAbsorber::build_unpolished_prepared(
                    design(),
                    BorrowedMapWeights::Provided(&weights),
                    options,
                    &mut topology,
                )
                .unwrap();
                let CmgRoute::Graph(graph) = &absorber.route else {
                    panic!("graph")
                };
                let hierarchy = graph.solver.preconditioner().hierarchy();
                assert_eq!(hierarchy.report().vertex_counts()[0], 960);
                assert!(
                    hierarchy.report().vertex_counts().len() >= 2,
                    "exercise coarsening, not direct-only setup"
                );
                if order == [0, 1, 2] && frame == 0 {
                    let owned = CmgAbsorber::build(
                        design(),
                        BorrowedMapWeights::Provided(&weights),
                        options,
                        false,
                    )
                    .unwrap();
                    let CmgRoute::Graph(owned) = &owned.route else {
                        panic!("owned graph")
                    };
                    assert!(
                        owned
                            .solver
                            .preconditioner()
                            .hierarchy()
                            .report()
                            .vertex_counts()[1]
                            > hierarchy.report().vertex_counts()[1],
                        "the OLS owned route must retain default pruning"
                    );
                }
                let mut candidate = input.clone();
                let report = absorber
                    .project_unpolished_with_seed(&mut candidate, 1e-12, Some(&mut control_seed))
                    .unwrap();
                assert!(!control_seed.ready());
                assert_eq!(
                    report.projection_work.warm_right_hand_sides,
                    usize::from(frame == 1)
                );
                assert_eq!(
                    report.nuisance_setup.warm_right_hand_sides,
                    if frame == 1 { 4 } else { 0 }
                );
                let oracle = absorb_borrowed_intercept_cg_certified(
                    &design(),
                    BorrowedMapWeights::Provided(&weights),
                    input.clone(),
                    options.map,
                )
                .unwrap();
                assert_matrix_close(&candidate, oracle.residualized(), 1e-8);
                assert!(
                    certify_borrowed_intercept_candidate(
                        &design(),
                        BorrowedMapWeights::Provided(&weights),
                        &candidate,
                        options.map
                    )
                    .unwrap()
                    .passed
                );
                if let Some(reference) = reference {
                    assert_eq!(
                        &candidate, reference,
                        "warm projections must preserve permutation determinism"
                    );
                } else {
                    *reference = Some(candidate);
                }
                control_seed.retain_certified(report.projection_work);
                topology.retain_certified_seed(absorber);
            }
        }
    }

    #[test]
    fn component_quantiles_use_stable_nearest_lower_order_statistic() {
        let sizes = [1, 2, 3, 4, 100];
        assert_eq!(component_quantile(&sizes, 50), 3);
        assert_eq!(component_quantile(&sizes, 90), 4);
        assert_eq!(component_quantile(&[], 50), 0);
    }

    #[test]
    fn component_default_is_explicit_only_and_has_an_ordinary_comparison_seam() {
        for solver in [
            FeSolverKind::Auto,
            FeSolverKind::Map,
            FeSolverKind::Lsmr,
            FeSolverKind::Cg,
        ] {
            assert!(!component_policy(solver, false));
            assert!(!component_policy(solver, true));
        }
        assert!(component_policy(FeSolverKind::Cmg, false));
        assert!(!component_policy(FeSolverKind::Cmg, true));
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
            components: false,
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

    #[test]
    fn prepared_topology_rejects_a_different_design_with_the_same_profile() {
        let worker_a = [0, 0, 1, 1, 2, 2, 3, 3];
        let firm_a = [0, 1, 0, 1, 0, 1, 0, 1];
        let worker_b = [0, 1, 1, 2, 2, 3, 3, 0];
        let firm_b = [0, 0, 1, 1, 0, 0, 1, 1];
        let worker_a_index = LevelIndex::from_codes(&worker_a, 4).expect("worker A index");
        let firm_a_index = LevelIndex::from_codes(&firm_a, 2).expect("firm A index");
        let worker_b_index = LevelIndex::from_codes(&worker_b, 4).expect("worker B index");
        let firm_b_index = LevelIndex::from_codes(&firm_b, 2).expect("firm B index");
        let design_a = BorrowedInterceptFeDesign::new(
            worker_a.len(),
            vec![
                effect(&worker_a, 4, &worker_a_index),
                effect(&firm_a, 2, &firm_a_index),
            ],
        )
        .expect("design A");
        let design_b = BorrowedInterceptFeDesign::new(
            worker_b.len(),
            vec![
                effect(&worker_b, 4, &worker_b_index),
                effect(&firm_b, 2, &firm_b_index),
            ],
        )
        .expect("design B");
        assert_eq!(
            CmgTopologyProfile::analyze(&design_a).expect("profile A"),
            CmgTopologyProfile::analyze(&design_b).expect("profile B")
        );

        let prepare_peak =
            PreparedCmgTopology::conservative_prepare_peak_bytes(&design_a).expect("peak");
        let mut prepared = PreparedCmgTopology::prepare(&design_a).expect("prepared topology");
        assert!(prepare_peak >= prepared.retained_bytes().unwrap() as u128);
        let error = match CmgAbsorber::build_unpolished_prepared(
            design_b,
            BorrowedMapWeights::Unit,
            config(1),
            &mut prepared,
        ) {
            Ok(_) => panic!("prepared topology accepted a different design"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(error.message.contains("does not match"));
    }

    #[test]
    fn early_prepared_memory_bound_covers_exact_topology_and_frame() {
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
        let cmg_config = config(2);
        let early =
            PreparedCmgTopology::conservative_retained_frame_peak_bytes(&design, cmg_config)
                .expect("early retained-frame peak");
        let prepared = PreparedCmgTopology::prepare(&design).expect("prepared topology");
        let profile = CmgTopologyProfile::analyze(&design).expect("profile");
        let exact_problem = CmgProblemSize {
            vertices: prepared.topology.vertex_count(),
            input_edges: prepared.topology.input_edge_count(),
            canonical_edges: prepared.topology.canonical_edge_count(),
            right_hand_sides: cmg_config.batch_size,
        };
        let exact_frame =
            conservative_prepared_cmg_frame_bytes(&design, &profile, cmg_config, exact_problem)
                .expect("exact prepared frame peak");
        let exact_combined = (prepared.retained_bytes().unwrap() as u128)
            .checked_add(exact_frame)
            .expect("exact retained-frame total");
        assert!(early >= exact_combined);
    }

    #[test]
    fn prepared_cmg_seed_weight_boundary_and_memory_are_explicit() {
        let codes = [
            [0, 0, 1, 1, 2, 2, 3, 3],
            [0, 1, 2, 0, 1, 2, 0, 1],
            [0, 1, 1, 0, 0, 1, 1, 0],
        ];
        let levels = [4, 3, 2];
        let indexes = codes
            .iter()
            .zip(levels)
            .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).unwrap())
            .collect::<Vec<_>>();
        let design = BorrowedInterceptFeDesign::new(
            8,
            (0..3)
                .map(|i| effect(&codes[i], levels[i], &indexes[i]))
                .collect(),
        )
        .unwrap();
        let mut topology = PreparedCmgTopology::prepare(&design).unwrap();
        let profile = CmgTopologyProfile::analyze(&design).unwrap();
        assert_eq!(topology.nuisance_seed.len(), 7);
        assert_eq!(nuisance_seed_bytes(&profile).unwrap(), 7 * 8 + 40);
        let early = PreparedCmgTopology::conservative_retained_frame_peak_bytes(&design, config(2))
            .unwrap();
        let frame = conservative_prepared_cmg_frame_bytes(
            &design,
            &profile,
            config(2),
            conservative_prepared_problem(&profile, 2),
        )
        .unwrap();
        assert!(early >= topology.retained_bytes().unwrap() as u128 + frame);
        for (ratio, eligible) in [
            (2.0, true),
            (0.5, true),
            (2.0 + 1e-12, false),
            (0.5 - 1e-12, false),
        ] {
            topology.numeric_weights.fill(1.0);
            topology.seed_valid = true;
            let mut weights = [1.0; 8];
            weights[0] = ratio;
            topology
                .assemble(
                    &design,
                    &profile,
                    BorrowedMapWeights::Provided(&weights),
                    1.0,
                )
                .unwrap();
            assert_eq!(topology.seed_for_frame, eligible);
            assert!(!topology.seed_valid);
        }
    }

    #[test]
    fn control_seed_admission_consumes_only_finite_certified_evidence() {
        let mut seed = CmgControlSeed::new(6, 3).unwrap();
        assert!(!seed.ready());
        assert!(!seed.consume(1.0));
        let capacity = seed.potentials.capacity();
        assert_eq!(
            CmgControlSeed::estimated_bytes(6, 3).unwrap(),
            (18 * 8 + std::mem::size_of::<CmgControlSeed>()) as u128
        );
        assert!(CmgControlSeed::estimated_bytes(usize::MAX, usize::MAX).is_err());
        for (ratio, admitted) in [
            (0.5, false),
            (1.0, true),
            (2.0, true),
            (2.0000001, false),
            (f64::INFINITY, false),
            (f64::NAN, false),
        ] {
            seed.retain_certified(CmgProjectionWork::default());
            assert_eq!(seed.consume(ratio), admitted);
            assert!(!seed.ready());
            assert!(
                !seed.consume(1.0),
                "a partial/failed solve cannot reuse the buffer"
            );
        }
        for (residual, extra_restarts, retained) in [
            (0.499, 0, true),
            (0.5, 0, false),
            (0.01, 1, false),
            (f64::NAN, 0, false),
        ] {
            seed.retain_certified(CmgProjectionWork {
                warm_right_hand_sides: 3,
                initial_relative_residual: residual,
                extra_restarts,
                ..CmgProjectionWork::default()
            });
            assert_eq!(seed.ready(), retained);
        }
        seed.retain_certified(CmgProjectionWork::default());
        seed.potentials[2] = f64::NAN;
        assert!(!seed.consume(1.0));
        seed.retain_certified(CmgProjectionWork::default());
        assert!(!seed.ready());
        seed.potentials.fill(0.0);
        seed.retain_certified(CmgProjectionWork::default());
        seed.reset();
        assert!(!seed.ready());
        assert_eq!(
            capacity,
            seed.potentials.capacity(),
            "all state transitions must be allocation-free"
        );
    }

    #[test]
    fn prepared_cmg_warm_seed_is_consumed_recertified_and_invalidated() {
        let rows = 1_200;
        let codes = [
            (0..rows).map(|row| (row % 40) as u32).collect::<Vec<_>>(),
            (0..rows)
                .map(|row| ((row / 40 + 7 * row) % 20) as u32)
                .collect(),
            (0..rows)
                .map(|row| ((row / 200 + row / 7) % 5) as u32)
                .collect(),
        ];
        let levels = [40, 20, 5];
        let indexes = codes
            .iter()
            .zip(levels)
            .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).unwrap())
            .collect::<Vec<_>>();
        let design = || {
            BorrowedInterceptFeDesign::new(
                rows,
                (0..3)
                    .map(|i| effect(&codes[i], levels[i], &indexes[i]))
                    .collect(),
            )
            .unwrap()
        };
        let input = DenseMatrix::from_columns(&[
            (0..rows).map(|row| (row as f64 * 0.31).sin()).collect(),
            (0..rows).map(|row| (row as f64 * 0.17).cos()).collect(),
        ])
        .unwrap();
        let mut topology = PreparedCmgTopology::prepare(&design()).unwrap();
        let mut control_seed = CmgControlSeed::new(60, 1).unwrap();
        assert!(!topology.seed_valid);
        let retained = topology.retained_bytes();
        let mut options = config(2);
        options.map.tolerance = 1e-10;
        for frame in 0..6 {
            let weights = (0..rows)
                .map(|row| {
                    let base = 0.7 + (row % 11) as f64 * 0.13;
                    if frame >= 2 && row % 3 == 0 {
                        base * 5.0
                    } else {
                        base
                    }
                })
                .collect::<Vec<_>>();
            if frame == 3 {
                topology.reset_warm_start();
                control_seed.reset();
            }
            if frame == 4 {
                topology.nuisance_seed[0] = f64::NAN;
                control_seed.potentials[0] = f64::NAN;
            }
            let mut absorber = CmgAbsorber::build_unpolished_prepared(
                design(),
                BorrowedMapWeights::Provided(&weights),
                options,
                &mut topology,
            )
            .unwrap();
            assert!(
                !topology.seed_valid,
                "a failed frame cannot leave a usable seed"
            );
            let mut candidate = input.clone();
            let report = absorber
                .project_unpolished_with_seed(&mut candidate, 1e-11, Some(&mut control_seed))
                .unwrap();
            assert!(!control_seed.ready());
            assert_eq!(
                report.projection_work.warm_right_hand_sides,
                usize::from(frame == 1 || frame == 5)
            );
            if frame == 1 || frame == 5 {
                assert_eq!(report.projection_work.warm_iterations, 0);
                assert!(
                    report.projection_work.iterations > 0,
                    "response must stay cold"
                );
            }
            assert_eq!(
                report.nuisance_setup.warm_right_hand_sides,
                if frame == 1 || frame == 5 { 4 } else { 0 }
            );
            if frame == 1 || frame == 5 {
                assert_eq!(
                    report.nuisance_setup.iterations, 0,
                    "same-operator guess must freshly certify"
                );
            }
            let oracle = absorb_borrowed_intercept_cg_certified(
                &design(),
                BorrowedMapWeights::Provided(&weights),
                input.clone(),
                options.map,
            )
            .unwrap();
            assert_matrix_close(&candidate, oracle.residualized(), 1e-8);
            assert!(
                certify_borrowed_intercept_candidate(
                    &design(),
                    BorrowedMapWeights::Provided(&weights),
                    &candidate,
                    options.map
                )
                .unwrap()
                .passed
            );
            control_seed.retain_certified(report.projection_work);
            assert!(control_seed.ready());
            topology.retain_certified_seed(absorber);
            assert!(
                topology.seed_valid,
                "frame {frame}: {:?}",
                report.nuisance_setup
            );
            assert_eq!(
                retained,
                topology.retained_bytes(),
                "seed commit cannot allocate"
            );
        }
    }

    #[test]
    fn prepared_nuisance_potentials_are_current_weight_and_remove_only_duplicate_solves() {
        let rows = 1_200;
        let codes = [
            (0..rows).map(|row| (row % 40) as u32).collect::<Vec<_>>(),
            (0..rows)
                .map(|row| ((row / 40 + 7 * row) % 20) as u32)
                .collect(),
            (0..rows)
                .map(|row| ((row / 200 + row / 7) % 5) as u32)
                .collect(),
        ];
        let levels = [40, 20, 5];
        let indexes = codes
            .iter()
            .zip(levels)
            .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).expect("index"))
            .collect::<Vec<_>>();
        let input = DenseMatrix::from_columns(
            &(0..3)
                .map(|column| {
                    (0..rows)
                        .map(|row| {
                            ((row + column * 19) as f64 * 0.31).sin() + codes[2][row] as f64 * 0.2
                        })
                        .collect()
                })
                .collect::<Vec<_>>(),
        )
        .expect("input");
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let design = || {
                BorrowedInterceptFeDesign::new(
                    rows,
                    order
                        .iter()
                        .map(|&index| effect(&codes[index], levels[index], &indexes[index]))
                        .collect(),
                )
                .expect("design")
            };
            let mut topology = PreparedCmgTopology::prepare(&design()).expect("topology");
            let mut previous_potentials = None;
            for frame in 0..2 {
                let weights = (0..rows)
                    .map(|row| 0.7 + ((row * (frame + 1)) % 11) as f64 * 0.13)
                    .collect::<Vec<_>>();
                let mut options = config(2);
                options.threads =
                    ThreadConfig::from_requested(1 + 3 * frame, true).expect("threads");
                let mut prepared = CmgAbsorber::build_unpolished_prepared(
                    design(),
                    BorrowedMapWeights::Provided(&weights),
                    options,
                    &mut topology,
                )
                .expect("prepared frame");
                let mut owned = CmgAbsorber::build(
                    design(),
                    BorrowedMapWeights::Provided(&weights),
                    options,
                    false,
                )
                .expect("owned frame");
                let CmgRoute::Graph(graph) = &prepared.route else {
                    panic!("graph")
                };
                let GraphCmgWorkspace::CallerBuffers(caller) = &graph.workspace else {
                    panic!("caller")
                };
                assert_eq!(caller.nuisance_potentials.len(), 60 * 4);
                if let Some(previous) =
                    previous_potentials.replace(caller.nuisance_potentials.clone())
                {
                    assert_ne!(previous, caller.nuisance_potentials);
                }
                let mut previous_values = None;
                for call in 0..2 {
                    let mut candidate = input.clone();
                    let mut reference = input.clone();
                    let first = prepared
                        .project_unpolished(&mut candidate, 1e-11)
                        .expect("prepared projection");
                    let old = owned
                        .project_unpolished(&mut reference, 1e-11)
                        .expect("owned projection");
                    assert_eq!(first.right_hand_sides, if call == 0 { 4 + 3 } else { 3 });
                    assert_eq!(old.right_hand_sides - first.right_hand_sides, 3);
                    assert_eq!(first.nuisance_reused_right_hand_sides, 3);
                    assert_eq!(old.nuisance_reused_right_hand_sides, 0);
                    assert_matrix_close(&candidate, &reference, 5e-9);
                    if let Some(previous) = previous_values.replace(candidate.clone()) {
                        assert_eq!(previous.as_slice(), candidate.as_slice());
                    }
                    let mut certificate_config = options.map;
                    certificate_config.tolerance = 1e-9;
                    assert!(
                        certify_borrowed_intercept_candidate(
                            &design(),
                            BorrowedMapWeights::Provided(&weights),
                            &candidate,
                            certificate_config,
                        )
                        .expect("fresh full-model certificate")
                        .passed
                    );
                }
                // A reused potential is not evidence: a corrupted basis must
                // still be rejected by the independent full-FE certificate.
                let CmgRoute::Graph(graph) = &mut prepared.route else {
                    panic!("graph")
                };
                let GraphCmgWorkspace::CallerBuffers(caller) = &mut graph.workspace else {
                    panic!("caller")
                };
                caller.nuisance_potentials[0] += 10.0;
                let mut corrupt = input.clone();
                prepared
                    .project_unpolished(&mut corrupt, 1e-11)
                    .expect("uncertified projection");
                let mut certificate_config = options.map;
                certificate_config.tolerance = 1e-9;
                assert!(
                    !certify_borrowed_intercept_candidate(
                        &design(),
                        BorrowedMapWeights::Provided(&weights),
                        &corrupt,
                        certificate_config,
                    )
                    .expect("independent corrupted-basis certificate")
                    .passed
                );
            }
        }
    }

    #[test]
    fn prepared_batches_align_with_the_admitted_pool_without_changing_solutions() {
        let rows = 4_200;
        let levels = [80, 70, 65];
        let codes = [
            (0..rows).map(|row| (row % 80) as u32).collect::<Vec<_>>(),
            (0..rows)
                .map(|row| ((row / 80 + 7 * row) % 70) as u32)
                .collect(),
            (0..rows)
                .map(|row| ((row / 200 + row / 7) % 65) as u32)
                .collect(),
        ];
        let indexes = codes
            .iter()
            .zip(levels)
            .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).expect("index"))
            .collect::<Vec<_>>();
        let design = || {
            BorrowedInterceptFeDesign::new(
                rows,
                (0..3)
                    .map(|index| effect(&codes[index], levels[index], &indexes[index]))
                    .collect(),
            )
            .expect("design")
        };
        let weights = (0..rows)
            .map(|row| 0.7 + (row % 11) as f64 * 0.13)
            .collect::<Vec<_>>();
        let input = DenseMatrix::from_columns(
            &(0..11)
                .map(|column| {
                    (0..rows)
                        .map(|row| ((row + column * 19) as f64 * 0.31).sin())
                        .collect()
                })
                .collect::<Vec<_>>(),
        )
        .expect("input");
        let reference = absorb_borrowed_intercept_cg_certified(
            &design(),
            BorrowedMapWeights::Provided(&weights),
            input.clone(),
            config(10).map,
        )
        .expect("independent CG oracle");
        let mut previous = None;
        for (threads, capacity, width) in [
            (1, 10, 10),
            (7, 1, 1),
            (7, 6, 6),
            (7, 7, 7),
            (7, 10, 7),
            (7, 14, 14),
            (14, 10, 10),
        ] {
            let mut options = config(capacity);
            options.threads = ThreadConfig::from_requested(threads, true).expect("threads");
            let mut topology = PreparedCmgTopology::prepare(&design()).expect("topology");
            let mut absorber = CmgAbsorber::build_unpolished_prepared(
                design(),
                BorrowedMapWeights::Provided(&weights),
                options,
                &mut topology,
            )
            .expect("prepared setup");
            let CmgRoute::Graph(graph) = &absorber.route else {
                panic!("graph")
            };
            assert_eq!(graph.pcg_batch_width(capacity), width);
            assert_eq!(graph.setup_batches, 64_usize.div_ceil(width));
            let GraphCmgWorkspace::CallerBuffers(caller) = &graph.workspace else {
                panic!("caller")
            };
            assert_eq!(caller.right_hand_sides.len(), 150 * capacity);
            assert_eq!(caller.workspaces.len(), threads.min(capacity));
            let mut projected = input.clone();
            let report = absorber
                .project_unpolished(&mut projected, 1e-11)
                .expect("projection");
            assert_eq!(report.pcg_batch_width, width);
            assert_eq!(
                report.batches,
                64_usize.div_ceil(width) + 11_usize.div_ceil(width)
            );
            assert_eq!(report.right_hand_sides, 64 + 11);
            assert_matrix_close(&projected, reference.residualized(), 5e-9);
            assert!(
                certify_borrowed_intercept_candidate(
                    &design(),
                    BorrowedMapWeights::Provided(&weights),
                    &projected,
                    options.map,
                )
                .expect("fresh full-FE certificate")
                .passed
            );
            if let Some(previous) = previous.replace(projected.clone()) {
                assert_eq!(previous.as_slice(), projected.as_slice());
            }
            let owned = CmgAbsorber::build(
                design(),
                BorrowedMapWeights::Provided(&weights),
                options,
                false,
            )
            .expect("OLS setup");
            let CmgRoute::Graph(graph) = &owned.route else {
                panic!("OLS graph")
            };
            assert_eq!(graph.pcg_batch_width(capacity), capacity);
            assert_eq!(graph.setup_batches, 64_usize.div_ceil(capacity));
        }
    }

    #[test]
    fn prepared_cmg_tracks_executor_width_across_one_four_one() {
        let worker = [0, 0, 1, 1, 2, 2, 0, 1, 2, 0, 1, 2];
        let firm = [0, 1, 0, 2, 1, 2, 2, 1, 0, 0, 1, 2];
        let worker_index = LevelIndex::from_codes(&worker, 3).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm, 3).expect("firm index");
        let make_design = || {
            BorrowedInterceptFeDesign::new(
                worker.len(),
                vec![
                    effect(&worker, 3, &worker_index),
                    effect(&firm, 3, &firm_index),
                ],
            )
            .expect("design")
        };
        let mut prepared = PreparedCmgTopology::prepare(&make_design()).expect("prepared topology");

        for requested in [1, 4, 1] {
            let mut threaded_config = config(2);
            threaded_config.threads =
                ThreadConfig::from_requested(requested, true).expect("thread config");
            let mut absorber = CmgAbsorber::build_unpolished_prepared(
                make_design(),
                BorrowedMapWeights::Unit,
                threaded_config,
                &mut prepared,
            )
            .expect("prepared CMG setup");
            let mut values = DenseMatrix::from_columns(&[
                (0..worker.len())
                    .map(|row| (row as f64 * 0.37).sin())
                    .collect(),
                (0..worker.len())
                    .map(|row| (row as f64 * 0.23).cos())
                    .collect(),
            ])
            .expect("projection values");
            let report = absorber
                .project_unpolished(&mut values, 1e-10)
                .expect("prepared CMG projection");
            assert_eq!(report.executor_threads, requested);
        }
    }

    #[test]
    fn upstream_cmg_errors_have_an_explicit_fail_closed_public_class() {
        let cases = vec![
            (
                cmg::CmgError::DimensionMismatch {
                    context: "test",
                    expected: 2,
                    actual: 1,
                },
                ErrorCode::InvalidArgument,
            ),
            (
                cmg::CmgError::VertexOutOfBounds {
                    vertex: 2,
                    vertex_count: 2,
                },
                ErrorCode::InvalidData,
            ),
            (
                cmg::CmgError::VertexIndexTooWide {
                    vertex: usize::MAX,
                    maximum: u32::MAX as usize,
                },
                ErrorCode::Allocation,
            ),
            (
                cmg::CmgError::SelfLoop { vertex: 1 },
                ErrorCode::InvalidData,
            ),
            (
                cmg::CmgError::InvalidEdgeWeight {
                    u: 0,
                    v: 1,
                    weight: 0.0,
                },
                ErrorCode::InvalidData,
            ),
            (
                cmg::CmgError::NonFiniteMatrixValue {
                    row: 0,
                    column: 0,
                    value: f64::NAN,
                },
                ErrorCode::InvalidData,
            ),
            (
                cmg::CmgError::NotSymmetric {
                    row: 0,
                    column: 1,
                    forward: 1.0,
                    reverse: 2.0,
                },
                ErrorCode::InvalidData,
            ),
            (
                cmg::CmgError::PositiveOffDiagonal {
                    row: 0,
                    column: 1,
                    value: 1.0,
                },
                ErrorCode::InvalidData,
            ),
            (
                cmg::CmgError::NegativeDiagonal {
                    index: 0,
                    value: -1.0,
                },
                ErrorCode::InvalidData,
            ),
            (
                cmg::CmgError::NotDiagonallyDominant {
                    row: 0,
                    diagonal: 1.0,
                    off_diagonal_sum: 2.0,
                },
                ErrorCode::InvalidData,
            ),
            (
                cmg::CmgError::IncompatibleLaplacianRhs {
                    component: 0,
                    sum: 1.0,
                    tolerance: 0.1,
                },
                ErrorCode::NonConvergence,
            ),
            (
                cmg::CmgError::NonPositivePivot {
                    vertex: 0,
                    value: 0.0,
                },
                ErrorCode::NonConvergence,
            ),
            (
                cmg::CmgError::InvalidHierarchy { context: "test" },
                ErrorCode::Internal,
            ),
            (
                cmg::CmgError::PcgBreakdown {
                    iteration: 1,
                    quantity: "test",
                    value: 0.0,
                },
                ErrorCode::NonConvergence,
            ),
            (
                cmg::CmgError::MaximumIterations {
                    iterations: 10,
                    residual_norm: 1.0,
                    tolerance: 0.1,
                },
                ErrorCode::NonConvergence,
            ),
            (
                cmg::CmgError::ResidualVerificationFailed {
                    iteration: 10,
                    residual_norm: 1.0,
                    tolerance: 0.1,
                },
                ErrorCode::NonConvergence,
            ),
            (
                cmg::CmgError::ParallelRuntime {
                    message: "test".to_owned(),
                },
                ErrorCode::Internal,
            ),
            (
                cmg::CmgError::MemoryBudgetExceeded {
                    required_bytes: 2,
                    budget_bytes: 1,
                },
                ErrorCode::Allocation,
            ),
            (
                cmg::CmgError::AllocationFailed { context: "test" },
                ErrorCode::Allocation,
            ),
            (
                cmg::CmgError::InvalidOption {
                    name: "test",
                    value: 0.0,
                },
                ErrorCode::InvalidArgument,
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(cmg_error_code(&error), expected, "{error}");
            assert_eq!(cmg_error(error.clone()).code, expected);
            assert_eq!(cmg_solve_error(error).code, expected);
        }
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
    fn swapping_cmg_bipartition_orientation_preserves_the_certified_projection() {
        let rows = 240;
        let large = (0..rows).map(|row| (row % 24) as u32).collect::<Vec<_>>();
        let medium = (0..rows)
            .map(|row| ((row * 5 + row / 24) % 9) as u32)
            .collect::<Vec<_>>();
        let nuisance = (0..rows)
            .map(|row| ((row * 7 + row / 11) % 4) as u32)
            .collect::<Vec<_>>();
        let large_index = LevelIndex::from_codes(&large, 24).expect("large index");
        let medium_index = LevelIndex::from_codes(&medium, 9).expect("medium index");
        let nuisance_index = LevelIndex::from_codes(&nuisance, 4).expect("nuisance index");
        let make_design = || {
            BorrowedInterceptFeDesign::new(
                rows,
                vec![
                    effect(&large, 24, &large_index),
                    effect(&medium, 9, &medium_index),
                    effect(&nuisance, 4, &nuisance_index),
                ],
            )
            .expect("orientation design")
        };
        let canonical_profile =
            CmgTopologyProfile::analyze(&make_design()).expect("canonical profile");
        assert_eq!(canonical_profile.core_effects, Some([0, 1]));
        let mut reversed_profile = canonical_profile.clone();
        reversed_profile.effect_order.swap(0, 1);
        reversed_profile.core_effects = Some([1, 0]);
        std::mem::swap(
            &mut reversed_profile.highest_cardinality_levels,
            &mut reversed_profile.second_cardinality_levels,
        );

        let values = DenseMatrix::from_columns(&[
            (0..rows).map(|row| (row as f64 * 0.071).sin()).collect(),
            (0..rows).map(|row| (row as f64 * 0.113).cos()).collect(),
        ])
        .expect("orientation values");
        let mut canonical = CmgAbsorber::build_profiled_internal(
            make_design(),
            BorrowedMapWeights::Unit,
            config(2),
            false,
            canonical_profile,
            true,
            None,
            &mut SolverWorkspacePeak::from_bytes(0).unwrap(),
        )
        .expect("canonical orientation");
        let mut reversed = CmgAbsorber::build_profiled_internal(
            make_design(),
            BorrowedMapWeights::Unit,
            config(2),
            false,
            reversed_profile,
            true,
            None,
            &mut SolverWorkspacePeak::from_bytes(0).unwrap(),
        )
        .expect("reversed orientation");
        let canonical = canonical.absorb(values.clone()).expect("canonical solve");
        let reversed = reversed.absorb(values).expect("reversed solve");
        assert!(canonical.report().certified());
        assert!(reversed.report().certified());
        assert_matrix_close(canonical.residualized(), reversed.residualized(), 5e-10);
        assert!(canonical.report().certificate.relative_infinity_norm <= 1e-10);
        assert!(reversed.report().certificate.relative_infinity_norm <= 1e-10);
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
        let mut auto_config = config(3);
        auto_config.requested_solver = FeSolverKind::Auto;
        let mut session = CmgAbsorber::build_profiled_internal(
            design,
            BorrowedMapWeights::Unit,
            auto_config,
            false,
            profile,
            true,
            None,
            &mut SolverWorkspacePeak::from_bytes(0).unwrap(),
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
        let error = match CmgAbsorber::build_profiled_internal(
            design,
            BorrowedMapWeights::Unit,
            constrained,
            false,
            profile,
            true,
            None,
            &mut SolverWorkspacePeak::from_bytes(0).unwrap(),
        ) {
            Ok(_) => panic!("CMG memory preflight unexpectedly fit"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::Allocation);
        assert!(error.message.contains("joint execution/solver memory plan"));
    }

    #[test]
    fn cmg_heap_forecast_uses_actual_small_component_storage_without_runtime_allowances() {
        for components in [false, true] {
            for groups in [1, 256] {
                let n = groups * 8;
                let left = (0..n)
                    .map(|i| ((i / 8) * 2 + i % 2) as u32)
                    .collect::<Vec<_>>();
                let right = (0..n)
                    .map(|i| ((i / 8) * 2 + (i / 2) % 2) as u32)
                    .collect::<Vec<_>>();
                let li = LevelIndex::from_codes(&left, groups * 2).unwrap();
                let ri = LevelIndex::from_codes(&right, groups * 2).unwrap();
                let design = BorrowedInterceptFeDesign::new(
                    n,
                    vec![
                        effect(&left, groups * 2, &li),
                        effect(&right, groups * 2, &ri),
                    ],
                )
                .unwrap();
                let mut options = config(2);
                options.components = components;
                options.threads = ThreadConfig::from_requested(2, true).unwrap();
                let profile = CmgTopologyProfile::analyze(&design).unwrap();
                let admission = preflight_cmg_memory(&design, &profile, options).unwrap();
                let mut session = CmgAbsorber::build_profiled_internal(
                    design,
                    BorrowedMapWeights::Unit,
                    options,
                    false,
                    profile,
                    true,
                    None,
                    &mut SolverWorkspacePeak::from_bytes(0).unwrap(),
                )
                .unwrap();
                let retained = session.retained_workspace_peak().unwrap();
                let expected = session.expected_heap_peak().expected_heap_bytes().unwrap();
                assert!(expected >= retained.expected_heap_bytes().unwrap());
                assert!(expected < admission.construction.bytes().unwrap() / 4);
                let input = (0..n)
                    .map(|i| if i % 8 < 4 { -1.0 } else { 1.0 })
                    .collect::<Vec<_>>();
                let projected = session
                    .absorb(DenseMatrix::from_columns(&[input.clone()]).unwrap())
                    .unwrap();
                assert!(projected.report().certified());
                for (&actual, &reference) in projected.residualized().as_slice().iter().zip(&input)
                {
                    assert!((actual - reference).abs() < 1e-10);
                }
                let peak = projected.report().solver_workspace_peak;
                assert!(peak.bytes().unwrap() >= admission.construction.bytes().unwrap());
                assert!(peak.expected_heap_bytes().unwrap() < peak.bytes().unwrap() / 4);
            }
        }
    }

    #[test]
    fn cmg_workspace_failed_nuisance_construction_retains_its_admitted_attempt() {
        let n = 6000;
        let worker = (0..n).map(|row| (row / 3) as u32).collect::<Vec<_>>();
        let firm = (0..n)
            .map(|row| ((row / 3 * 7 + row % 3 * 13) % 200) as u32)
            .collect::<Vec<_>>();
        let nuisance = (0..n)
            .map(|row| ((row / 7 + row % 3) % 3) as u32)
            .collect::<Vec<_>>();
        let wi = LevelIndex::from_codes(&worker, 2000).unwrap();
        let fi = LevelIndex::from_codes(&firm, 200).unwrap();
        let ni = LevelIndex::from_codes(&nuisance, 3).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            n,
            vec![
                effect(&worker, 2000, &wi),
                effect(&firm, 200, &fi),
                effect(&nuisance, 3, &ni),
            ],
        )
        .unwrap();
        let profile = CmgTopologyProfile::analyze(&design).unwrap();
        let mut options = config(2);
        options.map.max_iterations = 1;
        options.requested_solver = FeSolverKind::Auto;
        let unbounded = preflight_cmg_memory(&design, &profile, options).unwrap();
        options.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 512,
            usable_bytes: 1 << 30,
        });
        let expected = preflight_cmg_memory(&design, &profile, options)
            .unwrap()
            .construction;
        assert_eq!(unbounded.construction, expected);
        assert!(unbounded.projection.bytes().is_some());
        let mut attempted = SolverWorkspacePeak::ZERO;
        let result = CmgAbsorber::build_profiled_internal(
            design,
            BorrowedMapWeights::Unit,
            options,
            false,
            profile,
            false,
            None,
            &mut attempted,
        );
        match result {
            Err(error) => assert_eq!(error.code, ErrorCode::NonConvergence, "{}", error.message),
            Ok(_) => panic!("one-iteration nuisance construction unexpectedly succeeded"),
        }
        assert!(attempted.bytes().unwrap() >= expected.bytes().unwrap());
        assert!(attempted.bytes().unwrap() > 0);
        assert_eq!(
            attempted.expected_heap_bytes(),
            None,
            "failed opaque setup is not a zero-cost forecast"
        );
    }

    #[test]
    fn cmg_workspace_report_observes_fixed_pool_reuse() {
        let n = 400;
        let left = (0..n).map(|row| (row % 20) as u32).collect::<Vec<_>>();
        let right = (0..n)
            .map(|row| ((row / 20) % 10) as u32)
            .collect::<Vec<_>>();
        let li = LevelIndex::from_codes(&left, 20).unwrap();
        let ri = LevelIndex::from_codes(&right, 10).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            n,
            vec![effect(&left, 20, &li), effect(&right, 10, &ri)],
        )
        .unwrap();
        let mut options = config(4);
        options.threads = ThreadConfig::from_requested(4, true).unwrap();
        options.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 1 << 30,
        });
        let mut session =
            CmgAbsorber::build(design, BorrowedMapWeights::Unit, options, false).unwrap();
        let before = session.retained_workspace_peak().unwrap().bytes().unwrap();
        match &session.route {
            CmgRoute::Graph(graph) => match &graph.workspace {
                GraphCmgWorkspace::OlsBuffers(pool) => assert_eq!(pool.workspaces.len(), 4),
                _ => panic!("expected owned workspace pool"),
            },
            _ => panic!("expected CMG graph"),
        }
        let values = DenseMatrix::from_columns(
            &(0..4)
                .map(|rhs| {
                    (0..n)
                        .map(|row| {
                            (rhs + 1) as f64
                                * ((row % 20) as f64 + if row < 200 { -1.0 } else { 1.0 })
                        })
                        .collect()
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let result = session.absorb(values).unwrap();
        let after = session.retained_workspace_peak().unwrap().bytes().unwrap();
        assert_eq!(after, before, "admitted solve banks cannot grow lazily");
        match &session.route {
            CmgRoute::Graph(graph) => match &graph.workspace {
                GraphCmgWorkspace::OlsBuffers(pool) => assert_eq!(pool.workspaces.len(), 4),
                _ => panic!("expected owned workspace pool"),
            },
            _ => panic!("expected CMG graph"),
        }
        assert!(result.report().solver_workspace_peak.bytes().unwrap() >= after);
        for row in 0..n {
            for rhs in 0..4 {
                assert!(
                    (result.residualized()[(row, rhs)]
                        - (rhs + 1) as f64 * if row < 200 { -1.0 } else { 1.0 })
                    .abs()
                        < 1e-10
                );
            }
        }
    }

    #[test]
    fn cmg_container_admission_accepts_exact_budget_and_rejects_one_byte_short() {
        let left = [0, 0, 1, 1];
        let right = [0, 1, 0, 1];
        let li = LevelIndex::from_codes(&left, 2).unwrap();
        let ri = LevelIndex::from_codes(&right, 2).unwrap();
        let design =
            BorrowedInterceptFeDesign::new(4, vec![effect(&left, 2, &li), effect(&right, 2, &ri)])
                .unwrap();
        let profile = CmgTopologyProfile::analyze(&design).unwrap();
        for threads in [1, 4] {
            let mut options = config(3);
            options.threads = ThreadConfig::from_requested(threads, true).unwrap();
            options.solver_memory_budget = None;
            let expected = preflight_cmg_memory(&design, &profile, options).unwrap();
            let bytes = u128::from(expected.construction.bytes().unwrap());
            options.solver_memory_budget = Some(FeSolverMemoryBudget {
                execution_bytes: 197,
                usable_bytes: 197 + bytes,
            });
            assert_eq!(
                preflight_cmg_memory(&design, &profile, options)
                    .unwrap()
                    .construction,
                expected.construction
            );
            options.solver_memory_budget.as_mut().unwrap().usable_bytes -= 1;
            assert_eq!(
                preflight_cmg_memory(&design, &profile, options)
                    .err()
                    .unwrap()
                    .code,
                ErrorCode::Allocation
            );
            let prepared = PreparedCmgTopology::prepare(&design).unwrap();
            options.solver_memory_budget = None;
            let expected =
                preflight_prepared_cmg_memory(&design, &profile, options, &prepared).unwrap();
            let bytes = u128::from(expected.construction.bytes().unwrap());
            options.solver_memory_budget = Some(FeSolverMemoryBudget {
                execution_bytes: 197,
                usable_bytes: 197 + bytes,
            });
            assert_eq!(
                preflight_prepared_cmg_memory(&design, &profile, options, &prepared)
                    .unwrap()
                    .construction,
                expected.construction
            );
            options.solver_memory_budget.as_mut().unwrap().usable_bytes -= 1;
            assert_eq!(
                preflight_prepared_cmg_memory(&design, &profile, options, &prepared)
                    .err()
                    .unwrap()
                    .code,
                ErrorCode::Allocation
            );
        }
    }

    #[test]
    fn cmg_workspace_preflight_rejection_does_not_charge_an_unallocated_attempt() {
        let left = [0, 0, 1, 1];
        let right = [0, 1, 0, 1];
        let li = LevelIndex::from_codes(&left, 2).unwrap();
        let ri = LevelIndex::from_codes(&right, 2).unwrap();
        let design =
            BorrowedInterceptFeDesign::new(4, vec![effect(&left, 2, &li), effect(&right, 2, &ri)])
                .unwrap();
        let mut options = config(1);
        options.requested_solver = FeSolverKind::Auto;
        options.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 512,
            usable_bytes: 513,
        });
        let mut attempted = SolverWorkspacePeak::ZERO;
        let result = CmgAbsorber::build_with_workspace(
            design,
            BorrowedMapWeights::Unit,
            options,
            false,
            &mut attempted,
        );
        assert!(matches!(
            result,
            Err(FeregError {
                code: ErrorCode::Allocation,
                ..
            })
        ));
        assert_eq!(attempted, SolverWorkspacePeak::ZERO);
    }

    #[test]
    fn cmg_workspace_retained_polish_and_released_final_correction_have_different_peaks() {
        let n = 4 * 3 * 128;
        let left = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
        let right = (0..n).map(|row| ((row / 4) % 3) as u32).collect::<Vec<_>>();
        let li = LevelIndex::from_codes(&left, 4).unwrap();
        let ri = LevelIndex::from_codes(&right, 3).unwrap();
        let design = || {
            BorrowedInterceptFeDesign::new(n, vec![effect(&left, 4, &li), effect(&right, 3, &ri)])
                .unwrap()
        };
        let noise = (0..n)
            .map(|row| if (row / 12) % 2 == 0 { -1.0 } else { 1.0 })
            .collect::<Vec<_>>();
        let mut options = config(2);
        options.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 512,
            usable_bytes: 1 << 30,
        });
        options.retain_coefficients = true;
        let input = DenseMatrix::from_columns(
            &(0..5)
                .map(|rhs| {
                    (0..n)
                        .map(|row| {
                            (rhs + 1) as f64
                                * (noise[row] + left[row] as f64 + 0.5 * right[row] as f64)
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let mut session =
            CmgAbsorber::build(design(), BorrowedMapWeights::Unit, options, false).unwrap();
        let construction = session.memory.construction.bytes().unwrap();
        let before = session.retained_workspace_peak().unwrap().bytes().unwrap();
        let result = session.absorb(input).unwrap();
        assert_eq!(result.report().used_solver, FeSolverKind::Cmg);
        assert!(result.report().certified());
        let after = session.retained_workspace_peak().unwrap().bytes().unwrap();
        assert!(after >= before);
        let (within, report, coefficients) = result.into_full_parts();
        let coefficients = coefficients.expect("exact crossed projection retains coefficients");
        assert_eq!(coefficients.rhs_count(), 5); // Wider than the admitted CMG batch.
        let (norms, _) = weighted_column_norms(&within, BorrowedMapWeights::Unit, 1.0).unwrap();
        let polish = polish_borrowed_intercept_cg_with_reference_norms_certified(
            &design(),
            BorrowedMapWeights::Unit,
            within.clone(),
            options.map,
            &norms,
        )
        .unwrap();
        assert_eq!(polish.report().iterations, 0);
        let retained_polish = after
            + coefficients.retained_bytes() as u64
            + 5 * 8
            + polish.report().solver_workspace_peak.bytes().unwrap();
        assert!(report.solver_workspace_peak.bytes().unwrap() >= construction.max(retained_polish));
        assert_coefficients_reconstruct(
            &DenseMatrix::from_columns(
                &(0..5)
                    .map(|rhs| {
                        (0..n)
                            .map(|row| {
                                (rhs + 1) as f64
                                    * (noise[row] + left[row] as f64 + 0.5 * right[row] as f64)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
            &within,
            &[&left, &right],
            &coefficients,
            1e-10,
        );

        let mut terminal =
            CmgAbsorber::build(design(), BorrowedMapWeights::Unit, options, false).unwrap();
        terminal.memory = CmgWorkspacePlan::EMPTY;
        let terminal_retained = terminal.retained_workspace_peak().unwrap().bytes().unwrap();
        let (certificate, workspace) =
            super::super::borrowed_map::certify_borrowed_intercept_candidate_with_workspace(
                &design(),
                BorrowedMapWeights::Unit,
                &within,
                options.map,
            )
            .unwrap();
        assert!(certificate.passed);
        let terminal_result = terminal
            .absorb_final_residual(within.clone(), options.map.tolerance)
            .unwrap();
        assert_eq!(
            terminal_result.report().solver_workspace_peak.bytes(),
            Some(terminal_retained + workspace.bytes().unwrap())
        );
        assert_eq!(terminal_result.residualized(), &within);
        assert_eq!(terminal_result.report().iterations, 0);
        assert!(!terminal_result.report().fallback_used);

        // Isolate the final phase after construction has already been reported.
        // This test does not change the production admission decision.
        session.memory = CmgWorkspacePlan::EMPTY;
        let input = DenseMatrix::from_columns(&[(0..n)
            .map(|row| noise[row] + if left[row] == 0 { 2e-9 } else { 0.0 })
            .collect()])
        .unwrap();
        let mut strict = options.map;
        strict.tolerance = 1e-12;
        let (cert, cert_work) =
            super::super::borrowed_map::certify_borrowed_intercept_candidate_with_workspace(
                &design(),
                BorrowedMapWeights::Unit,
                &input,
                strict,
            )
            .unwrap();
        assert!(!cert.passed);
        let (norms, norm_work) =
            weighted_column_norms(&input, BorrowedMapWeights::Unit, 1.0).unwrap();
        let reference = polish_borrowed_intercept_cg_with_reference_norms_certified(
            &design(),
            BorrowedMapWeights::Unit,
            input.clone(),
            strict,
            &norms,
        )
        .unwrap();
        assert!(reference.report().iterations > 0);
        let correction =
            reference.report().solver_workspace_peak.bytes().unwrap() + norms.capacity() as u64 * 8;
        let expected = (after + cert_work.bytes().unwrap())
            .max(correction)
            .max(norm_work.bytes().unwrap());
        assert!(
            expected < after + correction,
            "the released hierarchy must not be added to CG"
        );
        let result = session.absorb_final_residual(input, 1e-12).unwrap();
        assert_eq!(
            result.report().solver_workspace_peak.bytes(),
            Some(expected)
        );
        assert!(result.report().fallback_used);
        for (residual, expected) in result.residualized().as_slice().iter().zip(noise) {
            assert!((residual - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn near_within_cmg_hadamard_batch_preserves_forward_accuracy() {
        let rows = 48;
        let left: Vec<_> = (0..rows).map(|row| (row % 4) as u32).collect();
        let right: Vec<_> = (0..rows).map(|row| ((row / 4) % 3) as u32).collect();
        let li = LevelIndex::from_codes(&left, 4).unwrap();
        let ri = LevelIndex::from_codes(&right, 3).unwrap();
        let u: Vec<_> = (0..rows)
            .map(|row| if row / 12 < 2 { 1.0 } else { -1.0 })
            .collect();
        let v: Vec<_> = (0..rows)
            .map(|row| if (row / 12) % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let x1: Vec<_> = (0..rows)
            .map(|row| u[row] + 2e-9 * (left[row] + 1) as f64)
            .collect();
        let x2: Vec<_> = (0..rows)
            .map(|row| v[row] + 1e-9 * (right[row] + 1) as f64)
            .collect();
        let y: Vec<_> = (0..rows)
            .map(|row| {
                0.7 * x1[row] - 0.4 * x2[row]
                    + 2e-9 * (left[row] + 1) as f64
                    + 1e-9 * (right[row] + 1) as f64
                    + 4.0 * u[row] * v[row]
            })
            .collect();
        let input = DenseMatrix::from_columns(&[y, x1, x2, vec![1.0; rows]]).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            rows,
            vec![effect(&left, 4, &li), effect(&right, 3, &ri)],
        )
        .unwrap();
        let mut options = config(4);
        options.map.tolerance = 1e-14;
        options.retain_coefficients = true;
        let mut session =
            CmgAbsorber::build(design, BorrowedMapWeights::Unit, options, false).unwrap();
        let result = session
            .absorb(input.clone())
            .expect("well-conditioned Hadamard design must certify");
        let (within, report, coefficients) = result.into_full_parts();
        assert!(report.certified());
        for row in 0..rows {
            assert!(
                (within[(row, 0)] - (0.7 * u[row] - 0.4 * v[row] + 4.0 * u[row] * v[row])).abs()
                    < 1e-12,
                "row={row} value={} expected={}",
                within[(row, 0)],
                0.7 * u[row] - 0.4 * v[row] + 4.0 * u[row] * v[row]
            );
            assert!((within[(row, 1)] - u[row]).abs() < 1e-12);
            assert!((within[(row, 2)] - v[row]).abs() < 1e-12);
            assert!(within[(row, 3)].abs() < 1e-12);
        }
        if let Some(coefficients) = coefficients {
            assert_coefficients_reconstruct(
                &input,
                &within,
                &[&left, &right],
                &coefficients,
                1e-12,
            );
        }
    }

    #[test]
    fn terminal_residual_correction_checks_its_cg_budget() {
        let n = 4 * 3 * 4;
        let left = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
        let right = (0..n).map(|row| ((row / 4) % 3) as u32).collect::<Vec<_>>();
        let li = LevelIndex::from_codes(&left, 4).unwrap();
        let ri = LevelIndex::from_codes(&right, 3).unwrap();
        let noise = (0..n)
            .map(|row| if (row / 12) % 2 == 0 { -1.0 } else { 1.0 })
            .collect::<Vec<_>>();
        let input = || {
            DenseMatrix::from_columns(&[(0..n)
                .map(|row| noise[row] + if left[row] == 0 { 2e-9 } else { 0.0 })
                .collect()])
            .unwrap()
        };
        let incremental = estimate_fe_solver_memory(n, 1, &[4, 3], 1, None)
            .unwrap()
            .cg_bytes;
        for short in [true, false] {
            let design = BorrowedInterceptFeDesign::new(
                n,
                vec![effect(&left, 4, &li), effect(&right, 3, &ri)],
            )
            .unwrap();
            let mut options = config(1);
            options.map.tolerance = 1e-8;
            let mut session =
                CmgAbsorber::build(design, BorrowedMapWeights::Unit, options, false).unwrap();
            assert!(matches!(&session.route, CmgRoute::Graph(_)));
            // Isolate admission of the terminal correction after construction.
            // Its hierarchy is released before this independent reserve.
            session.config.solver_memory_budget = Some(FeSolverMemoryBudget {
                execution_bytes: 512,
                usable_bytes: 512 + incremental - u128::from(short),
            });
            let result = session.absorb_final_residual(input(), 1e-12);
            if short {
                let error = result.unwrap_err();
                assert_eq!(error.code, ErrorCode::Allocation);
                assert!(error.message.contains("certified CG fallback requires"));
            } else {
                let result = result.unwrap();
                assert!(result.report().certified());
                assert_eq!(result.report().requested_solver, FeSolverKind::Cmg);
                assert_eq!(result.report().used_solver, FeSolverKind::Cg);
                assert!(result.report().fallback_used);
                assert_eq!(result.report().checkpoint_retries, 0);
                let (residual, _) = result.into_parts();
                assert!(residual
                    .as_slice()
                    .iter()
                    .zip(&noise)
                    .all(|(a, b)| (a - b).abs() < 1e-12));
            }
        }
    }

    #[test]
    fn admitted_components_and_ordinary_projection_share_the_independent_oracle() {
        for disconnected in [false, true] {
            let left = [0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
            let right = if disconnected {
                [0, 1, 0, 1, 2, 3, 2, 3, 0, 1, 2, 3]
            } else {
                [0, 1, 1, 2, 2, 3, 3, 0, 0, 1, 2, 3]
            };
            let li = LevelIndex::from_codes(&left, 4).unwrap();
            let ri = LevelIndex::from_codes(&right, 4).unwrap();
            let design = || {
                BorrowedInterceptFeDesign::new(
                    left.len(),
                    vec![effect(&left, 4, &li), effect(&right, 4, &ri)],
                )
                .unwrap()
            };
            let weights = [1.0, 1e-10, 2.0, 3.0, 1.0, 2.0, 1.0, 4.0, 2.0, 1.0, 3.0, 2.0];
            let values = DenseMatrix::from_columns(&[
                (0..left.len()).map(|r| (r as f64 * 0.3).sin()).collect(),
                (0..left.len()).map(|r| (r as f64 * 0.2).cos()).collect(),
            ])
            .unwrap();
            let reference = absorb_borrowed_intercept_cg_certified(
                &design(),
                BorrowedMapWeights::Provided(&weights),
                values.clone(),
                config(2).map,
            )
            .unwrap();
            for components in [false, true] {
                let mut options = config(2);
                options.components = components;
                options.retain_coefficients = true;
                let mut session = CmgAbsorber::build(
                    design(),
                    BorrowedMapWeights::Provided(&weights),
                    options,
                    false,
                )
                .unwrap();
                let outcome = session.absorb(values.clone()).unwrap();
                assert!(outcome.report().certified());
                assert_eq!(outcome.report().used_solver, FeSolverKind::Cmg);
                assert_matrix_close(outcome.residualized(), reference.residualized(), 5e-9);
                let (residual, _, coefficients) = outcome.into_full_parts();
                assert_coefficients_reconstruct(
                    &values,
                    &residual,
                    &[&left, &right],
                    &coefficients.expect("retained component coefficients"),
                    5e-9,
                );
            }
        }
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
    fn multiple_nuisance_effects_agree_with_full_multi_effect_cg() {
        let worker = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 0, 1, 2, 3, 4, 0, 2, 4];
        let firm = [0, 1, 0, 2, 1, 3, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 1, 0];
        let year = [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2];
        let region = [0, 0, 1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0];
        let wi = LevelIndex::from_codes(&worker, 5).expect("worker index");
        let fi = LevelIndex::from_codes(&firm, 4).expect("firm index");
        let yi = LevelIndex::from_codes(&year, 3).expect("year index");
        let ri = LevelIndex::from_codes(&region, 2).expect("region index");
        let make_design = || {
            BorrowedInterceptFeDesign::new(
                worker.len(),
                vec![
                    effect(&worker, 5, &wi),
                    effect(&firm, 4, &fi),
                    effect(&year, 3, &yi),
                    effect(&region, 2, &ri),
                ],
            )
            .expect("design")
        };
        let weights = (0..worker.len())
            .map(|row| 0.8 + (row % 5) as f64 * 0.17)
            .collect::<Vec<_>>();
        let values = DenseMatrix::from_columns(&[
            (0..worker.len())
                .map(|row| (row as f64 * 0.37).sin() + row as f64 * 0.02)
                .collect(),
            (0..worker.len())
                .map(|row| (row as f64 * 0.19).cos() - row as f64 * 0.01)
                .collect(),
        ])
        .expect("values");
        let reference = absorb_borrowed_intercept_cg_certified(
            &make_design(),
            BorrowedMapWeights::Provided(&weights),
            values.clone(),
            config(2).map,
        )
        .expect("CG reference");
        let mut session = CmgAbsorber::build(
            make_design(),
            BorrowedMapWeights::Provided(&weights),
            config(2),
            false,
        )
        .expect("CMG setup");
        let result = session.absorb(values).expect("CMG projection");
        let (result_values, report) = result.into_parts();
        let (reference_values, _) = reference.into_parts();
        assert!(report.certified());
        assert_matrix_close(&result_values, &reference_values, 5e-9);
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
