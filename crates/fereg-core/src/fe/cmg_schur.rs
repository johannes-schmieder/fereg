//! Fixed CMG cycles on an exact partial worker Schur complement.
//!
//! Firms precede auxiliary high-degree workers. Low-degree worker stars become
//! weighted firm cliques; repeated worker/firm observations are aggregated first.
//! The outer solver still applies its implicit, fully eliminated Schur operator.

use std::time::Instant;

use cmg::{
    CmgOptions, CmgPreconditioner, CmgStorage, CmgStorageReport, CmgWorkspace,
    ComponentBuildOptions, Laplacian, ParallelCmgPlan, ParallelExecutor, ParallelOptions,
    ParallelRuntimeAllowance, PcgWorkspace,
};

use crate::error::{allocation, invalid_argument, invalid_data, FeregError, Result};
use crate::linalg::neumaier_add;
use crate::memory::{vec_capacity_bytes, FeSolverMemoryBudget, SolverWorkspacePeak};

use super::cmg::cmg_error;
use super::{BorrowedInterceptFeDesign, BorrowedMapWeights, LevelIndex};

pub(super) struct CmgSchurPreconditioner {
    preconditioner: CmgPreconditioner,
    plan: ParallelCmgPlan,
    executor: ParallelExecutor,
    workspace: CmgWorkspace,
    rhs: Vec<f64>,
    solution: Vec<f64>,
    components: FirmComponents,
    construction: SolverWorkspacePeak,
}

impl CmgSchurPreconditioner {
    pub(super) fn build(
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        left_effect: usize,
        degree: usize,
        threads: usize,
        budget: Option<FeSolverMemoryBudget>,
    ) -> Result<Self> {
        Self::build_with_options(
            design,
            weights,
            left_effect,
            degree,
            threads,
            budget,
            CmgOptions::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_with_options(
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        left_effect: usize,
        degree: usize,
        threads: usize,
        budget: Option<FeSolverMemoryBudget>,
        options: CmgOptions,
    ) -> Result<Self> {
        if design.effects().len() != 2 || left_effect > 1 || !(4..=6).contains(&degree) {
            return Err(invalid_argument(
                "CMG Schur requires two intercept effects and degree 4, 5 or 6",
            ));
        }
        if let BorrowedMapWeights::Provided(values) = weights {
            if values.len() != design.nobs() {
                return Err(invalid_argument("CMG Schur weight count is inconsistent"));
            }
        }
        crate::interrupt::checkpoint()?;
        let started = Instant::now();
        let parallel = ParallelOptions {
            threads,
            ..ParallelOptions::default()
        };
        let mut memory = Admission {
            budget,
            peak: SolverWorkspacePeak::ZERO,
        };
        let runtime = ParallelRuntimeAllowance::default()
            .estimate(parallel)
            .map_err(cmg_error)?;
        let runtime_budget = memory.admit(0, runtime.total_bytes())?;
        let (executor, runtime) = ParallelExecutor::new_with_estimated_memory_budget(
            parallel,
            ParallelRuntimeAllowance::default(),
            runtime_budget,
        )
        .map_err(cmg_error)?;
        let runtime_extra = runtime.runtime_heap_allowance_bytes() as u128
            + runtime.worker_stack_allowance_bytes() as u128;
        memory.forecast(storage(&[&executor])?)?;
        let (graph, components, retained_workers, input_edges) = build_hybrid_graph(
            design,
            weights,
            left_effect,
            degree,
            &executor,
            runtime_extra,
            &mut memory,
        )?;
        let graph_seconds = started.elapsed().as_secs_f64();
        let hierarchy_started = Instant::now();
        let component_options = ComponentBuildOptions {
            prune_coarse_isolates: true,
            preserve_unpruned_stopping: true,
            factor_terminal_components: true,
        };
        let bound = CmgPreconditioner::component_executor_construction_cmg_allocation_bound(
            graph.vertex_count(),
            graph.edge_count(),
            options,
            component_options,
        )
        .map_err(cmg_error)?;
        let parents = storage(&[&graph, &executor])? + components.bytes();
        let admitted = memory.admit(parents + runtime_extra, bound)?;
        let preconditioner =
            CmgPreconditioner::build_component_experiment_with_executor_and_cmg_budget(
                &graph,
                options,
                component_options,
                &executor,
                admitted,
            )
            .map_err(cmg_error)?;
        crate::interrupt::checkpoint()?;
        memory.forecast(
            super::cmg::forecast::hierarchy_peak(&graph, &executor, &preconditioner, None, true)?
                + components.bytes(),
        )?;
        let hierarchy_seconds = hierarchy_started.elapsed().as_secs_f64();
        let bound = ParallelCmgPlan::construction_allocation_bound(&preconditioner, &executor)
            .map_err(cmg_error)?;
        let parents = storage(&[&graph, &executor, &preconditioner])? + components.bytes();
        let admitted = memory.admit(parents + runtime_extra, bound)?;
        memory.forecast(parents + bound as u128)?;
        let plan = ParallelCmgPlan::build_with_budget(&preconditioner, &executor, admitted)
            .map_err(cmg_error)?;

        // The public PCG allocation bound includes the complete CMG workspace
        // and additional vectors we do not need. Use its conservative coverage
        // rather than the older, explicitly incomplete principal-array estimate.
        let bound = PcgWorkspace::allocation_bytes(&preconditioner)
            .map_err(cmg_error)?
            .checked_add(bytes::<[f64; 2]>(graph.vertex_count())?)
            .ok_or_else(|| allocation("CMG Schur apply workspace overflows"))?;
        let parents = storage(&[&graph, &executor, &preconditioner, &plan])? + components.bytes();
        memory.admit(parents + runtime_extra, bound)?;
        let workspace = preconditioner.try_workspace().map_err(cmg_error)?;
        let rhs = filled(graph.vertex_count(), 0.0)?;
        let solution = filled(graph.vertex_count(), 0.0)?;
        let actual = storage(&[&workspace, &rhs, &solution])?;
        if actual > bound as u128 {
            return Err(allocation("CMG Schur apply storage exceeds admission"));
        }
        // The caller retains this relatively large owner behind a Box. Admit
        // that final allocation as well as the heap banks reachable from it.
        memory.admit(
            parents + actual + runtime_extra,
            std::mem::size_of::<Self>(),
        )?;
        memory.forecast(parents + actual + std::mem::size_of::<Self>() as u128)?;
        if crate::diagnostics::solver_trace_enabled() {
            let report = preconditioner.hierarchy().report();
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=cmgschur_setup|degree={degree}|workers={}|retained_workers={retained_workers}|firms={}|vertices={}|input_edges={input_edges}|edges={}|components={}|threads={}|parallel_operators={}|levels={}|hierarchy_vertices={:?}|hierarchy_nonzeros={:?}|repeats={:?}|graph_seconds={graph_seconds:.9}|hierarchy_seconds={hierarchy_seconds:.9}|setup_seconds={:.9}",
                design.effects()[left_effect].n_levels(), components.labels.len(),
                graph.vertex_count(), graph.edge_count(), components.sizes.len(),
                executor.thread_count(), plan.operator_count(), report.vertex_counts().len(),
                report.vertex_counts(), report.matrix_nonzeros(), preconditioner.repeat_counts(),
                started.elapsed().as_secs_f64(),
            ));
        }
        Ok(Self {
            preconditioner,
            plan,
            executor,
            workspace,
            rhs,
            solution,
            components,
            construction: memory.peak,
        })
    }

    pub(super) fn retained_bytes(&self) -> Result<u128> {
        Ok(storage(&[
            &self.preconditioner,
            &self.plan,
            &self.executor,
            &self.workspace,
            &self.rhs,
            &self.solution,
        ])? + self.components.bytes()
            + std::mem::size_of::<Self>() as u128)
    }

    pub(super) fn construction_peak(&self) -> SolverWorkspacePeak {
        self.construction
    }

    /// B = P_f R B_h E P_f, where E zero-pads auxiliary workers and R restricts
    /// to firms. One fixed symmetric CMG cycle is used for every application.
    /// A component-scaled outer operator S/c requires c B, hence input scaling.
    pub(super) fn apply_batch(
        &mut self,
        residual: &[f64],
        output: &mut [f64],
        stride: usize,
        rhs_count: usize,
        component_scales: Option<&[f64]>,
    ) -> Result<()> {
        let firms = self.components.labels.len();
        let elements = firms
            .checked_mul(stride)
            .ok_or_else(|| invalid_argument("CMG Schur batch dimensions overflow"))?;
        if rhs_count > stride
            || residual.len() != elements
            || output.len() != elements
            || component_scales.is_some_and(|scales| scales.len() != firms)
        {
            return Err(invalid_argument(
                "CMG Schur batch dimensions are inconsistent",
            ));
        }
        for rhs in 0..rhs_count {
            crate::interrupt::checkpoint()?;
            for firm in 0..firms {
                let scale = component_scales.map_or(1.0, |scales| scales[firm]);
                if !scale.is_finite() || scale <= 0.0 {
                    return Err(invalid_data("CMG Schur component scale is invalid"));
                }
                self.rhs[firm] = residual[firm * stride + rhs] * scale;
                if !self.rhs[firm].is_finite() {
                    return Err(invalid_data("CMG Schur scaled residual is nonfinite"));
                }
            }
            self.components.center(&mut self.rhs[..firms])?;
            self.rhs[firms..].fill(0.0);
            self.plan
                .apply_compatible_into(
                    &self.preconditioner,
                    &self.rhs,
                    &mut self.solution,
                    &mut self.workspace,
                    &self.executor,
                )
                .map_err(cmg_error)?;
            self.components.center(&mut self.solution[..firms])?;
            for firm in 0..firms {
                output[firm * stride + rhs] = self.solution[firm];
            }
        }
        Ok(())
    }
}

struct Admission {
    budget: Option<FeSolverMemoryBudget>,
    peak: SolverWorkspacePeak,
}

impl Admission {
    fn admit(&mut self, parents: u128, additional: usize) -> Result<usize> {
        let stage = parents
            .checked_add(additional as u128)
            .ok_or_else(|| allocation("CMG Schur admission overflows"))?;
        self.peak = self
            .peak
            .max(SolverWorkspacePeak::from_bytes(stage)?.with_expected_heap(Some(0))?);
        let remaining = match self.budget {
            Some(budget) => budget
                .usable_bytes
                .checked_sub(budget.execution_bytes)
                .and_then(|bytes| bytes.checked_sub(parents))
                .ok_or_else(|| allocation("CMG Schur parents exhaust the joint memory budget"))?,
            None => usize::MAX as u128,
        };
        if remaining < additional as u128 {
            return Err(allocation(format!(
                "CMG Schur stage requires {additional} bytes after live parents; {remaining} remain"
            )));
        }
        Ok(remaining.min(usize::MAX as u128) as usize)
    }

    fn forecast(&mut self, heap: u128) -> Result<()> {
        self.peak = self
            .peak
            .max(SolverWorkspacePeak::ZERO.with_expected_heap(Some(heap))?);
        Ok(())
    }
}

fn bytes<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| allocation("CMG Schur allocation dimensions overflow"))
}

fn capacity<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| allocation("unable to reserve CMG Schur storage"))?;
    if values.capacity() != count {
        return Err(allocation("CMG Schur reservation exceeds admission"));
    }
    Ok(values)
}

fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>> {
    let mut values = capacity(count)?;
    values.resize(count, value);
    Ok(values)
}

fn storage(roots: &[&dyn CmgStorage]) -> Result<u128> {
    CmgStorageReport::measure(roots)
        .and_then(CmgStorageReport::total_bytes)
        .map(|value| value as u128)
        .map_err(cmg_error)
}

struct FirmComponents {
    labels: Vec<usize>,
    sizes: Vec<usize>,
    sums: Vec<f64>,
    corrections: Vec<f64>,
}

impl FirmComponents {
    fn bytes(&self) -> u128 {
        vec_capacity_bytes(&self.labels)
            + vec_capacity_bytes(&self.sizes)
            + vec_capacity_bytes(&self.sums)
            + vec_capacity_bytes(&self.corrections)
    }

    fn center(&mut self, values: &mut [f64]) -> Result<()> {
        self.sums.fill(0.0);
        self.corrections.fill(0.0);
        for (&label, &value) in self.labels.iter().zip(values.iter()) {
            neumaier_add(value, &mut self.sums[label], &mut self.corrections[label]);
        }
        for component in 0..self.sizes.len() {
            self.sums[component] =
                (self.sums[component] + self.corrections[component]) / self.sizes[component] as f64;
            if !self.sums[component].is_finite() {
                return Err(invalid_data("CMG Schur component mean is nonfinite"));
            }
        }
        for (&label, value) in self.labels.iter().zip(values.iter_mut()) {
            *value -= self.sums[label];
            if !value.is_finite() {
                return Err(invalid_data("CMG Schur centered value is nonfinite"));
            }
        }
        Ok(())
    }
}

struct WorkerCells<'a> {
    membership: &'a LevelIndex,
    firms: &'a [u32],
    weights: BorrowedMapWeights<'a>,
    weight_scale: f64,
    cells: Vec<(u32, f64)>,
}

impl WorkerCells<'_> {
    fn load(&mut self, worker: usize) -> Result<f64> {
        self.cells.clear();
        for (position, &row) in self.membership.members(worker).iter().enumerate() {
            if position % 4096 == 0 {
                crate::interrupt::checkpoint()?;
            }
            let weight = match self.weights {
                BorrowedMapWeights::Unit => 1.0,
                BorrowedMapWeights::Provided(values) => values[row] / self.weight_scale,
            };
            if !weight.is_finite() || weight <= 0.0 {
                return Err(invalid_data("CMG Schur requires positive finite weights"));
            }
            self.cells.push((self.firms[row], weight));
        }
        self.cells.sort_unstable_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
        });
        let mut read = 0;
        let mut write = 0;
        let mut total = 0.0;
        let mut total_correction = 0.0;
        while read < self.cells.len() {
            let firm = self.cells[read].0;
            let mut sum = 0.0;
            let mut correction = 0.0;
            while read < self.cells.len() && self.cells[read].0 == firm {
                neumaier_add(self.cells[read].1, &mut sum, &mut correction);
                read += 1;
            }
            let weight = sum + correction;
            self.cells[write] = (firm, weight);
            neumaier_add(weight, &mut total, &mut total_correction);
            write += 1;
        }
        self.cells.truncate(write);
        let total = total + total_correction;
        if !total.is_finite() || total <= 0.0 {
            return Err(invalid_data("CMG Schur worker weight total is invalid"));
        }
        Ok(total)
    }
}

// Stream cliques directly into CMG's canonical edge bank. The exact census
// supplies a size hint, avoiding a second observation- or edge-scale staging bank.
struct HybridEdges<'a> {
    workers: WorkerCells<'a>,
    cutoff: usize,
    next_worker: usize,
    next_auxiliary: usize,
    remaining: usize,
    first: usize,
    second: usize,
    total: f64,
    failure: Option<FeregError>,
}

impl Iterator for HybridEdges<'_> {
    type Item = (usize, usize, f64);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let cells = &self.workers.cells;
            if cells.len() > self.cutoff && self.first < cells.len() {
                let (firm, weight) = cells[self.first];
                self.first += 1;
                self.remaining -= 1;
                return Some((firm as usize, self.next_auxiliary - 1, weight));
            }
            if cells.len() <= self.cutoff && self.first + 1 < cells.len() {
                let (left, left_weight) = cells[self.first];
                let (right, right_weight) = cells[self.second];
                let weight =
                    left_weight.min(right_weight) * (left_weight.max(right_weight) / self.total);
                if !weight.is_finite() || weight <= 0.0 {
                    self.failure = Some(invalid_data(
                        "CMG Schur clique weight is nonpositive or nonfinite",
                    ));
                    return None;
                }
                self.second += 1;
                if self.second == cells.len() {
                    self.first += 1;
                    self.second = self.first + 1;
                }
                self.remaining -= 1;
                return Some((left as usize, right as usize, weight));
            }
            if self.next_worker == self.workers.membership.n_levels() || self.failure.is_some() {
                return None;
            }
            match self.workers.load(self.next_worker) {
                Ok(total) => self.total = total,
                Err(error) => {
                    self.failure = Some(error);
                    return None;
                }
            }
            self.next_worker += 1;
            self.first = 0;
            self.second = 1;
            if self.workers.cells.len() > self.cutoff {
                self.next_auxiliary += 1;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

fn root(labels: &mut [usize], mut vertex: usize) -> usize {
    while labels[vertex] != vertex {
        labels[vertex] = labels[labels[vertex]];
        vertex = labels[vertex];
    }
    vertex
}

#[allow(clippy::too_many_arguments)]
fn build_hybrid_graph(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    left_effect: usize,
    cutoff: usize,
    executor: &ParallelExecutor,
    runtime_extra: u128,
    memory: &mut Admission,
) -> Result<(Laplacian, FirmComponents, usize, usize)> {
    let left = &design.effects()[left_effect];
    let right = &design.effects()[1 - left_effect];
    let firms = right.n_levels();
    // Match the implicit Schur operator's global weight normalization. This
    // keeps both graph scales aligned and prevents harmless global AW scaling
    // from changing the preconditioner or overflowing worker totals.
    let mut weight_scale = 1.0_f64;
    if let BorrowedMapWeights::Provided(values) = weights {
        weight_scale = 0.0;
        for (row, &weight) in values.iter().enumerate() {
            if row % 4096 == 0 {
                crate::interrupt::checkpoint()?;
            }
            if !weight.is_finite() || weight <= 0.0 {
                return Err(invalid_data("CMG Schur requires positive finite weights"));
            }
            weight_scale = weight_scale.max(weight);
        }
    }
    let max_rows = (0..left.n_levels())
        .map(|worker| left.level_index().members(worker).len())
        .max()
        .unwrap_or(0);
    let scratch_bytes = bytes::<(u32, f64)>(max_rows)?;
    let component_bytes = bytes::<[usize; 2]>(firms)?;
    let bound = scratch_bytes
        .checked_add(component_bytes)
        .ok_or_else(|| allocation("CMG Schur topology scratch overflows"))?;
    let runtime_heap = storage(&[executor])?;
    memory.admit(runtime_heap + runtime_extra, bound)?;
    memory.forecast(runtime_heap + bound as u128)?;
    let mut workers = WorkerCells {
        membership: left.level_index(),
        firms: right.codes(),
        weights,
        weight_scale,
        cells: capacity(max_rows)?,
    };
    let mut labels = capacity(firms)?;
    labels.extend(0..firms);
    let mut sizes = filled(firms, usize::MAX)?;
    let mut retained_workers = 0_usize;
    let mut input_edges = 0_usize;
    for worker in 0..left.n_levels() {
        workers.load(worker)?;
        let count = workers.cells.len();
        if let Some(&(first, _)) = workers.cells.first() {
            for &(firm, _) in workers.cells.iter().skip(1) {
                let first = root(&mut labels, first as usize);
                let other = root(&mut labels, firm as usize);
                labels[first.max(other)] = first.min(other);
            }
        }
        let edges = if count > cutoff {
            retained_workers = retained_workers
                .checked_add(1)
                .ok_or_else(|| allocation("CMG Schur auxiliary count overflows"))?;
            count
        } else {
            count * count.saturating_sub(1) / 2
        };
        input_edges = input_edges
            .checked_add(edges)
            .ok_or_else(|| allocation("CMG Schur edge count overflows"))?;
    }
    for firm in 0..firms {
        labels[firm] = root(&mut labels, firm);
    }
    let mut component_count = 0;
    for label in &mut labels {
        if sizes[*label] == usize::MAX {
            sizes[*label] = component_count;
            component_count += 1;
        }
        *label = sizes[*label];
    }
    sizes.truncate(component_count);
    sizes.fill(0);
    for &label in &labels {
        sizes[label] += 1;
    }
    let vertices = firms
        .checked_add(retained_workers)
        .filter(|vertices| *vertices > 0 && *vertices as u128 <= u32::MAX as u128 + 1)
        .ok_or_else(|| allocation("CMG Schur graph vertex count is not supported"))?;
    // Pinned Laplacian construction reserves compact Edge (16 bytes), builds
    // one diagonal, and may shrink its merged edge bank. Cover both banks
    // during shrinking plus the three graph Arc allocations and control blocks.
    let graph_bound = bytes::<[u64; 4]>(input_edges)?
        .checked_add(bytes::<f64>(vertices)?)
        .and_then(|value| value.checked_add(128))
        .ok_or_else(|| allocation("CMG Schur graph construction overflows"))?;
    let parents = runtime_heap
        + vec_capacity_bytes(&workers.cells)
        + vec_capacity_bytes(&labels)
        + vec_capacity_bytes(&sizes);
    memory.admit(parents + runtime_extra, graph_bound)?;
    memory.forecast(parents + graph_bound as u128)?;
    workers.cells.clear();
    let mut edges = HybridEdges {
        workers,
        cutoff,
        next_worker: 0,
        next_auxiliary: firms,
        remaining: input_edges,
        first: 0,
        second: 1,
        total: 0.0,
        failure: None,
    };
    let graph_result = Laplacian::from_edges_with_executor(vertices, &mut edges, executor);
    if let Some(error) = edges.failure.take() {
        return Err(error);
    }
    let graph = graph_result.map_err(cmg_error)?;
    if edges.remaining != 0 {
        return Err(invalid_data(
            "CMG Schur edge census changed during construction",
        ));
    }
    drop(edges);
    crate::interrupt::checkpoint()?;
    let parents =
        storage(&[&graph, executor])? + vec_capacity_bytes(&labels) + vec_capacity_bytes(&sizes);
    let bound = bytes::<[f64; 2]>(component_count)?;
    memory.admit(parents + runtime_extra, bound)?;
    memory.forecast(parents + bound as u128)?;
    let components = FirmComponents {
        labels,
        sizes,
        sums: filled(component_count, 0.0)?,
        corrections: filled(component_count, 0.0)?,
    };
    Ok((graph, components, retained_workers, input_edges))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::fe::BorrowedInterceptEffect;

    struct Fixture {
        workers: Vec<u32>,
        firms: Vec<u32>,
        weights: Vec<f64>,
        worker_index: LevelIndex,
        firm_index: LevelIndex,
    }

    impl Fixture {
        fn new(workers: Vec<u32>, firms: Vec<u32>, weights: Vec<f64>) -> Self {
            let worker_levels = workers.iter().copied().max().unwrap() as usize + 1;
            let firm_levels = firms.iter().copied().max().unwrap() as usize + 1;
            let worker_index = LevelIndex::from_codes(&workers, worker_levels).unwrap();
            let firm_index = LevelIndex::from_codes(&firms, firm_levels).unwrap();
            Self {
                workers,
                firms,
                weights,
                worker_index,
                firm_index,
            }
        }

        fn design(&self) -> BorrowedInterceptFeDesign<'_> {
            BorrowedInterceptFeDesign::new(
                self.workers.len(),
                vec![
                    BorrowedInterceptEffect::new(
                        &self.workers,
                        self.worker_index.n_levels(),
                        &self.worker_index,
                    )
                    .unwrap(),
                    BorrowedInterceptEffect::new(
                        &self.firms,
                        self.firm_index.n_levels(),
                        &self.firm_index,
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
        }

        // Seven disconnected components, each containing one worker with a
        // different distinct-firm degree. Duplicate first-firm observations
        // make observation count an incorrect proxy for degree or clique weights.
        fn weighted_stars() -> Self {
            let mut workers = Vec::new();
            let mut firms = Vec::new();
            let mut weights = Vec::new();
            let mut first = 0;
            for degree in 1..=7 {
                for firm in 0..degree {
                    workers.push(degree - 1);
                    firms.push(first + firm);
                    weights.push((firm + 1) as f64);
                }
                workers.push(degree - 1);
                firms.push(first);
                weights.push(2.0);
                first += degree;
            }
            Self::new(workers, firms, weights)
        }
    }

    #[test]
    fn weighted_cliques_auxiliary_stars_and_component_inverse_match_closed_form() {
        let fixture = Fixture::weighted_stars();
        let design = fixture.design();
        for (cutoff, expected_edges) in [(4, 28), (5, 33), (6, 42)] {
            let mut solver = CmgSchurPreconditioner::build(
                &design,
                BorrowedMapWeights::Provided(&fixture.weights),
                0,
                cutoff,
                1,
                None,
            )
            .unwrap();
            let graph = solver.preconditioner.hierarchy().levels()[0].graph();
            assert_eq!(graph.vertex_count(), 28 + 7 - cutoff);
            assert_eq!(graph.edge_count(), expected_edges);
            assert_eq!(solver.components.sizes, vec![1, 2, 3, 4, 5, 6, 7]);
            let mut residual = vec![0.0; 28 * 3];
            let mut expected = vec![0.0; residual.len()];
            let mut scales = vec![0.0; 28];
            let mut first = 0;
            for degree in 1..=7 {
                // For a single weighted worker, S=diag(a)-aa'/sum(a).
                // On sum(r)=0, x_j=r_j/a_j, centered, solves Sx=r exactly.
                let mut mean = 0.0;
                for firm in 0..degree {
                    let value = firm as f64 - (degree - 1) as f64 / 2.0;
                    let weight = if firm == 0 { 3.0 } else { (firm + 1) as f64 };
                    residual[(first + firm) * 3] = value + 17.0;
                    residual[(first + firm) * 3 + 1] = 2.0 * value - 3.0;
                    scales[first + firm] = degree as f64;
                    expected[(first + firm) * 3] = value / weight * degree as f64 * 7.0;
                    mean += expected[(first + firm) * 3] / degree as f64;
                }
                for firm in 0..degree {
                    expected[(first + firm) * 3] -= mean;
                    expected[(first + firm) * 3 + 1] = 2.0 * expected[(first + firm) * 3];
                }
                first += degree;
            }
            let mut output = vec![0.0; residual.len()];
            solver
                .apply_batch(&residual, &mut output, 3, 2, Some(&scales))
                .unwrap();
            for (actual, expected) in output.iter().zip(&expected) {
                assert!(
                    (actual - expected).abs() < 2e-12,
                    "k={cutoff}: {actual} != {expected}"
                );
            }
            let first_output = output.clone();
            solver
                .apply_batch(&residual, &mut output, 3, 2, Some(&scales))
                .unwrap();
            assert_eq!(output, first_output);
            assert!(
                solver.retained_bytes().unwrap()
                    <= solver.construction_peak().bytes().unwrap() as u128
            );
            for global_scale in [1e-250, 1e250] {
                let rescaled: Vec<f64> = fixture
                    .weights
                    .iter()
                    .map(|weight| weight * global_scale)
                    .collect();
                let mut scaled_solver = CmgSchurPreconditioner::build(
                    &design,
                    BorrowedMapWeights::Provided(&rescaled),
                    0,
                    cutoff,
                    1,
                    None,
                )
                .unwrap();
                scaled_solver
                    .apply_batch(&residual, &mut output, 3, 2, Some(&scales))
                    .unwrap();
                for (actual, reference) in output.iter().zip(&first_output) {
                    assert!((actual - reference).abs() < 2e-11);
                }
            }
        }
    }

    #[test]
    fn fixed_multilevel_cycle_is_linear_symmetric_positive_and_repeatable() {
        let mut workers = Vec::new();
        let mut firms = Vec::new();
        let width = 20;
        for row in 0..width {
            for column in 0..width {
                let left = row * width + column;
                for right in [
                    row * width + (column + 1) % width,
                    ((row + 1) % width) * width + column,
                ] {
                    let worker = (workers.len() / 2) as u32;
                    workers.extend([worker, worker]);
                    firms.extend([left as u32, right as u32]);
                }
            }
        }
        let fixture = Fixture::new(workers, firms, Vec::new());
        let design = fixture.design();
        let options = CmgOptions {
            direct_threshold: 8,
            ..CmgOptions::default()
        };
        let mut reference: Option<Vec<f64>> = None;
        for threads in [1, 2] {
            let mut solver = CmgSchurPreconditioner::build_with_options(
                &design,
                BorrowedMapWeights::Unit,
                0,
                4,
                threads,
                None,
                options,
            )
            .unwrap();
            assert!(solver.preconditioner.hierarchy().levels().len() > 1);
            let n = width * width;
            let mut residual = vec![0.0; n * 3];
            for firm in 0..n {
                let x = (firm as f64 * 0.31).sin();
                let y = (firm as f64 * 0.19).cos();
                residual[firm * 3] = x;
                residual[firm * 3 + 1] = y;
                residual[firm * 3 + 2] = 2.0 * x - 3.0 * y;
            }
            let mut output = vec![0.0; residual.len()];
            solver
                .apply_batch(&residual, &mut output, 3, 3, None)
                .unwrap();
            let mut xby = 0.0;
            let mut ybx = 0.0;
            let mut xbx = 0.0;
            for firm in 0..n {
                let position = firm * 3;
                assert!(
                    (output[position + 2] - 2.0 * output[position] + 3.0 * output[position + 1])
                        .abs()
                        < 2e-11
                );
                xby += residual[position] * output[position + 1];
                ybx += residual[position + 1] * output[position];
                xbx += residual[position] * output[position];
            }
            assert!((xby - ybx).abs() < 2e-11 * xby.abs().max(1.0));
            assert!(xbx > 0.0);
            let repeated = output.clone();
            solver
                .apply_batch(&residual, &mut output, 3, 3, None)
                .unwrap();
            assert_eq!(output, repeated);
            if let Some(reference) = &reference {
                for (actual, prior) in output.iter().zip(reference) {
                    assert!((actual - prior).abs() < 2e-11);
                }
            } else {
                reference = Some(output);
            }
        }
    }

    #[test]
    fn memory_admission_and_invalid_requests_fail_closed() {
        let fixture = Fixture::weighted_stars();
        let design = fixture.design();
        let build = |budget| {
            CmgSchurPreconditioner::build(&design, BorrowedMapWeights::Unit, 0, 5, 1, budget)
        };
        let baseline = build(None).unwrap();
        let admitted = baseline.construction_peak().bytes().unwrap() as u128;
        drop(baseline);
        assert!(build(Some(FeSolverMemoryBudget {
            execution_bytes: 123,
            usable_bytes: admitted + 123
        }))
        .is_ok());
        assert!(matches!(
            build(Some(FeSolverMemoryBudget {
                execution_bytes: 123,
                usable_bytes: admitted + 122
            })),
            Err(FeregError {
                code: ErrorCode::Allocation,
                ..
            })
        ));
        assert!(matches!(
            CmgSchurPreconditioner::build(&design, BorrowedMapWeights::Unit, 0, 7, 1, None),
            Err(FeregError {
                code: ErrorCode::InvalidArgument,
                ..
            })
        ));
        let mut solver = build(None).unwrap();
        assert!(solver.apply_batch(&[0.0], &mut [0.0], 1, 1, None).is_err());
        let mut residual = vec![0.0; 28];
        residual[1] = f64::NAN;
        assert!(solver
            .apply_batch(&residual, &mut vec![0.0; 28], 1, 1, None)
            .is_err());
    }
}
