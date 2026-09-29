//! CMG requested-layout admission plus the explicitly estimated runtime policy.
//! Runtime allowances are engineering headroom, not a Rayon or process bound.

use super::*;

pub(super) fn parallel(config: CmgConfig) -> ParallelOptions {
    ParallelOptions {
        threads: config.threads.resolved(),
        ..ParallelOptions::default()
    }
}

pub(super) fn runtime(config: CmgConfig) -> Result<ParallelRuntimeEstimate> {
    ParallelRuntimeAllowance::default()
        .estimate(parallel(config))
        .map_err(cmg_error)
}

pub(super) fn runtime_extra(estimate: ParallelRuntimeEstimate) -> u128 {
    estimate.runtime_heap_allowance_bytes() as u128
        + estimate.worker_stack_allowance_bytes() as u128
}

pub(super) fn storage(roots: &[&dyn CmgStorage]) -> Result<u128> {
    CmgStorageReport::measure(roots)
        .and_then(CmgStorageReport::total_bytes)
        .map(|bytes| bytes as u128)
        .map_err(cmg_error)
}

/// Prepared parents are already charged by PPML's execution budget. Include
/// them in the union before subtracting that parent union, so shared graph
/// component/lineage Arcs cannot be counted again as a new numeric frame.
pub(super) fn frame_storage(
    roots: &[&dyn CmgStorage],
    prepared: Option<&PreparedCmgTopology>,
) -> Result<u128> {
    let Some(prepared) = prepared else {
        return storage(roots);
    };
    let empty = Vec::<f64>::new();
    let mut all: [&dyn CmgStorage; 20] = [&empty; 20];
    if roots.len() + 4 > all.len() {
        return Err(allocation("CMG ownership root capacity exceeded"));
    }
    all[..roots.len()].copy_from_slice(roots);
    let parents: [&dyn CmgStorage; 4] = [
        &prepared.topology,
        &prepared.assembly_workspace,
        &prepared.numeric_weights,
        &prepared.nuisance_seed,
    ];
    all[roots.len()..roots.len() + 4].copy_from_slice(&parents);
    storage(&all[..roots.len() + 4])?
        .checked_sub(storage(&parents)?)
        .ok_or_else(|| allocation("CMG shared-parent union underflows"))
}

/// Record attempted stage costs, then admit before construction. The selected
/// execution plan (including PPML's old owners) remains outside this increment.
pub(super) fn admit(
    config: CmgConfig,
    parents: u128,
    incremental: usize,
    plan: &mut CmgWorkspacePlan,
) -> Result<usize> {
    let peak = parents
        .checked_add(incremental as u128)
        .ok_or_else(|| allocation("CMG action stage overflows"))?;
    // Admission reservations must not overwrite the independently composed
    // heap forecast with max-level/fill/runtime allowances.
    let observed = SolverWorkspacePeak::from_bytes(peak)?.with_expected_heap(Some(0))?;
    plan.construction = plan.construction.max(observed);
    plan.projection = plan.projection.max(observed);
    remaining(config, parents, incremental)
}

pub(super) fn remaining(config: CmgConfig, parents: u128, required: usize) -> Result<usize> {
    let remaining = match config.solver_memory_budget {
        Some(budget) => budget
            .usable_bytes
            .checked_sub(budget.execution_bytes)
            .and_then(|bytes| bytes.checked_sub(parents))
            .ok_or_else(|| allocation("CMG live parents exhaust the joint memory budget"))?,
        None => usize::MAX as u128,
    };
    if remaining < required as u128 {
        return Err(allocation(format!(
            "CMG stage requires {required} bytes after live parents; {remaining} bytes remain"
        )));
    }
    Ok(remaining.min(usize::MAX as u128) as usize)
}

pub(super) fn component_options() -> ComponentBuildOptions {
    ComponentBuildOptions {
        prune_coarse_isolates: true,
        preserve_unpruned_stopping: true,
        factor_terminal_components: true,
    }
}

pub(super) fn construction_bound(
    vertices: usize,
    edges: usize,
    options: CmgOptions,
    config: CmgConfig,
) -> Result<usize> {
    if config.components {
        CmgPreconditioner::component_executor_construction_cmg_allocation_bound(
            vertices,
            edges,
            options,
            component_options(),
        )
    } else {
        CmgPreconditioner::executor_construction_cmg_allocation_bound(vertices, edges, options)
    }
    .map_err(cmg_error)
}

// CMG's select_batch_execution may initialize its plan. Use the pinned public
// eligibility dimensions here to admit that initialization first. A one-RHS tail
// can choose Planned even when full batches run AcrossRightHandSides. Tests
// compare all actual width decisions with this predicate at the crossovers.
pub(super) fn may_need_plan(solver: &ParallelPcgSolver) -> bool {
    let options = solver.executor().options();
    let graph = solver.graph();
    if solver.executor().thread_count() <= 1
        || graph.edge_count() < solver.policy().min_planned_edges
    {
        return false;
    }
    // The component count is private upstream. Conservatively admit the
    // small plan even for a disconnected vector-eligible graph, instead of
    // allocating another component discovery. Solver routing remains upstream.
    let vector = graph.vertex_count()
        >= options
            .min_parallel_len
            .max(options.reduction_chunk_size.saturating_mul(8));
    vector
        || solver
            .preconditioner()
            .hierarchy()
            .levels()
            .iter()
            .any(|level| {
                let graph = level.graph();
                level.terminal_reason().is_none()
                    && graph.edge_count()
                        >= graph
                            .vertex_count()
                            .saturating_add(graph.vertex_count() / 4)
                    && graph.edge_count() >= options.min_parallel_len
            })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(threads: usize) -> CmgConfig {
        CmgConfig {
            map: MapConfig::default(),
            threads: ThreadConfig::from_requested(threads, true).unwrap(),
            batch_size: 4,
            retain_coefficients: false,
            requested_solver: FeSolverKind::Cmg,
            components: false,
            solver_memory_budget: None,
        }
    }

    #[test]
    fn estimated_runtime_policy_is_explicit_and_admitted_after_parents() {
        let mut config = config(16);
        let estimate = runtime(config).unwrap();
        assert_eq!(estimate.runtime_heap_allowance_bytes(), 64 * 1024 * 1024);
        assert_eq!(estimate.worker_stack_allowance_bytes(), 128 * 1024 * 1024);
        assert_eq!(runtime_extra(estimate), 192 * 1024 * 1024);
        assert_eq!(
            estimate.total_bytes() as u128,
            runtime_extra(estimate) + estimate.worker_handles_bytes() as u128
        );
        let parents = 12345;
        config.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 789,
            usable_bytes: 789 + parents + estimate.total_bytes() as u128,
        });
        assert_eq!(
            remaining(config, parents, estimate.total_bytes()).unwrap(),
            estimate.total_bytes()
        );
        config.solver_memory_budget.as_mut().unwrap().usable_bytes -= 1;
        assert_eq!(
            remaining(config, parents, estimate.total_bytes())
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
        assert_eq!(
            remaining(config, u128::MAX, 1).unwrap_err().code,
            ErrorCode::Allocation
        );
        assert!(ParallelExecutor::runtime_allocation_bound().is_none());
    }

    #[test]
    fn runtime_handles_are_counted_once_and_shutdown_can_repeat() {
        for threads in [1, 4, 1] {
            let config = config(threads);
            let estimate = runtime(config).unwrap();
            let (executor, actual) = ParallelExecutor::new_with_estimated_memory_budget(
                parallel(config),
                ParallelRuntimeAllowance::default(),
                estimate.total_bytes(),
            )
            .unwrap();
            assert_eq!(actual, estimate);
            let report = CmgStorageReport::measure(&[&executor, &executor]).unwrap();
            assert!(report.runtime_heap_unbounded());
            assert_eq!(
                report.total_bytes().unwrap(),
                estimate.worker_handles_bytes()
            );
            assert_eq!(
                storage(&[&executor]).unwrap() + runtime_extra(actual),
                actual.total_bytes() as u128
            );
            drop(executor);
        }
    }

    #[test]
    fn plan_admission_precedes_routing_and_fixed_banks_preserve_decisions() {
        for threads in [1, 4] {
            for components in [false, true] {
                let mut config = config(threads);
                config.components = components;
                let graph = Laplacian::from_edges(
                    80,
                    (0..80).flat_map(|i| (i + 1..80).map(move |j| (i, j, 1.0))),
                )
                .unwrap();
                let options = CmgOptions {
                    direct_threshold: 8,
                    ..CmgOptions::default()
                };
                let par = ParallelOptions {
                    threads,
                    min_parallel_len: 1,
                    reduction_chunk_size: 1,
                    ..ParallelOptions::default()
                };
                let estimate = ParallelRuntimeAllowance::default().estimate(par).unwrap();
                let (executor, _) = ParallelExecutor::new_with_estimated_memory_budget(
                    par,
                    ParallelRuntimeAllowance::default(),
                    estimate.total_bytes(),
                )
                .unwrap();
                let bound = construction_bound(80, graph.edge_count(), options, config).unwrap();
                let preconditioner = if components {
                    CmgPreconditioner::build_component_experiment_with_executor_and_cmg_budget(
                        &graph,
                        options,
                        component_options(),
                        &executor,
                        bound,
                    )
                } else {
                    CmgPreconditioner::build_with_executor_and_cmg_budget(
                        &graph, options, &executor, bound,
                    )
                }
                .unwrap();
                let solver = ParallelPcgSolver::from_preconditioner(
                    preconditioner,
                    executor,
                    ParallelPcgPolicy {
                        min_planned_edges: 1,
                        ..ParallelPcgPolicy::default()
                    },
                )
                .unwrap();
                let needs = may_need_plan(&solver);
                if needs {
                    let bytes = solver.plan_construction_allocation_bound().unwrap();
                    assert!(solver.try_plan_with_budget(bytes - 1).is_err());
                    solver.try_plan_with_budget(bytes).unwrap();
                }
                let before = storage(&[&solver]).unwrap();
                for width in [1, 2, 4] {
                    let report = solver.select_batch_execution(width).unwrap();
                    assert_eq!(
                        width == 1 && needs,
                        width == 1 && report.execution() == ParallelPcgExecution::Planned
                    );
                }
                assert_eq!(
                    before,
                    storage(&[&solver]).unwrap(),
                    "routing must not grow the solver"
                );
                let bytes =
                    PcgBatchWorkspace::bank_allocation_bytes(solver.preconditioner(), threads)
                        .unwrap();
                assert!(PcgBatchWorkspace::new_bank_with_budget(
                    solver.preconditioner(),
                    threads,
                    bytes - 1
                )
                .is_err());
                let old = PcgBatchWorkspace::new_bank_with_budget(
                    solver.preconditioner(),
                    threads,
                    bytes,
                )
                .unwrap();
                let new = PcgBatchWorkspace::new_bank_with_budget(
                    solver.preconditioner(),
                    threads,
                    bytes,
                )
                .unwrap();
                assert_eq!(
                    storage(&[&old, &new]).unwrap(),
                    storage(&[&old]).unwrap() + storage(&[&new]).unwrap()
                );
                assert_eq!(
                    storage(&[&solver, solver.graph()]).unwrap(),
                    storage(&[&solver]).unwrap()
                );
            }
        }
    }
}
