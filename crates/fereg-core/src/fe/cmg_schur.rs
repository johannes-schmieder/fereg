//! Fixed CMG cycles on an exact partial worker Schur complement.
//!
//! Firms precede auxiliary high-degree workers. Low-degree worker stars become
//! weighted firm cliques; repeated worker/firm observations are aggregated first.
//! The outer solver still applies its implicit, fully eliminated Schur operator.
//! Automatic upgrades and explicit `cmgschur` requests share one bounded graph
//! adapter and one hierarchy pipeline; they differ only in how the built
//! hierarchy is installed (see `bounded::Install`).

use cmg::{
    CmgBatchWorkspace, CmgOptions, CmgPreconditioner, CmgStorage, CmgStorageReport, CmgWorkspace,
    ComponentBuildOptions, Laplacian, ParallelCmgPlan,
};

use crate::error::{allocation, invalid_argument, invalid_data, Result};
use crate::linalg::neumaier_add;
use crate::memory::{vec_capacity_bytes, FeSolverMemoryBudget, SolverWorkspacePeak};

use super::cmg::{cmg_error, format_usize_series};
use super::{BorrowedInterceptFeDesign, BorrowedMapWeights, LevelIndex};

mod bounded;
mod sampling;
use bounded::Install;
use sampling::SampledTrees;
pub(super) use sampling::Sampling;

const MAX_BATCH_COLUMNS: usize = 4;

pub(super) struct CmgSchurPreconditioner {
    preconditioner: CmgPreconditioner,
    install: Install,
    /// Row operators for the host team's eligible levels (explicit only).
    plan: Option<ParallelCmgPlan>,
    scalar_workspace: CmgWorkspace,
    /// Interleaved lanes of a wider explicit installation.
    batch_workspace: Option<CmgBatchWorkspace>,
    rhs: Vec<f64>,
    solution: Vec<f64>,
    components: FirmComponents,
    batch_columns: usize,
    construction: SolverWorkspacePeak,
    retained_bound: Option<u128>,
}

impl CmgSchurPreconditioner {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build(
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        left_effect: usize,
        degree: usize,
        sampling: Sampling,
        rhs_capacity: usize,
        threads: usize,
        budget: Option<FeSolverMemoryBudget>,
    ) -> Result<Self> {
        Self::build_with_options(
            design,
            weights,
            left_effect,
            degree,
            sampling,
            rhs_capacity,
            threads,
            budget,
            CmgOptions::default(),
        )
    }

    /// Explicit `cmgschur`: the bounded adapter's census, graph and hierarchy,
    /// installed on the action's host team with row operators for its eligible
    /// levels and up to four interleaved lanes. A memory refusal is an
    /// allocation failure; an explicit request has no incumbent to resume.
    #[allow(clippy::too_many_arguments)]
    fn build_with_options(
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        left_effect: usize,
        degree: usize,
        sampling: Sampling,
        rhs_capacity: usize,
        threads: usize,
        budget: Option<FeSolverMemoryBudget>,
        options: CmgOptions,
    ) -> Result<Self> {
        sampling.validate()?;
        if design.effects().len() != 2
            || left_effect > 1
            || !(4..=6).contains(&degree)
            || rhs_capacity == 0
        {
            return Err(invalid_argument(
                "CMG Schur requires two intercept effects and degree 4, 5 or 6",
            ));
        }
        if let BorrowedMapWeights::Provided(values) = weights {
            if values.len() != design.nobs() {
                return Err(invalid_argument("CMG Schur weight count is inconsistent"));
            }
        }
        let install = Install::Explicit {
            columns: rhs_capacity.min(MAX_BATCH_COLUMNS),
            threads,
        };
        // Every CMG stage of the build borrows this same host team.
        let built = crate::parallel::with_cmg_team(threads, |_| {
            Self::build_observations_bounded(
                design.effects()[left_effect].level_index(),
                design.effects()[1 - left_effect].codes(),
                design.effects()[1 - left_effect].n_levels(),
                weights,
                degree,
                sampling,
                budget,
                false,
                install,
                options,
                false,
            )
        })?;
        built
            .preconditioner
            .ok_or_else(|| allocation("CMG Schur construction exceeds the joint memory budget"))
    }

    pub(super) fn retained_bytes(&self) -> Result<u128> {
        if let Some(bound) = self.retained_bound {
            return Ok(bound);
        }
        Ok(storage(&[
            &self.preconditioner,
            &self.plan,
            &self.scalar_workspace,
            &self.batch_workspace,
            &self.rhs,
            &self.solution,
        ])? + self.components.bytes()
            + std::mem::size_of::<Self>() as u128)
    }

    pub(super) fn construction_peak(&self) -> SolverWorkspacePeak {
        self.construction
    }

    /// Solver-trace fields for the installed hierarchy, read from metadata the
    /// build already stored: no graph, factor or workspace payload is visited.
    pub(super) fn trace_hierarchy(&self) -> String {
        let report = self.preconditioner.hierarchy().report();
        let factor = self.preconditioner.terminal_factor();
        format!(
            "terminal={:?}|levels={}|hierarchy_vertices={}|hierarchy_nonzeros={}|repeats={}|terminal_factor={}|terminal_factor_dimension={}|terminal_factor_nonzeros={}|application={}|parallel_operators={}|batch_columns={}",
            report.terminal_reason(),
            report.vertex_counts().len(),
            format_usize_series(report.vertex_counts()),
            format_usize_series(report.matrix_nonzeros()),
            format_usize_series(self.preconditioner.repeat_counts()),
            u8::from(factor.is_some()),
            factor.map_or(0, cmg::GroundedLdl::active_dimension),
            factor.map_or(0, cmg::GroundedLdl::factor_nonzeros),
            match self.install {
                Install::Auto => "scalar",
                Install::Explicit { .. } => "parallel",
            },
            self.plan.as_ref().map_or(0, ParallelCmgPlan::operator_count),
            self.batch_columns,
        )
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
        active: Option<&[f64]>,
    ) -> Result<()> {
        let firms = self.components.labels.len();
        let elements = firms
            .checked_mul(stride)
            .ok_or_else(|| invalid_argument("CMG Schur batch dimensions overflow"))?;
        if rhs_count > stride
            || residual.len() != elements
            || output.len() != elements
            || component_scales.is_some_and(|scales| scales.len() != firms)
            || active.is_some_and(|states| states.len() != rhs_count)
        {
            return Err(invalid_argument(
                "CMG Schur batch dimensions are inconsistent",
            ));
        }
        if active.is_some_and(|states| states.iter().any(|value| !value.is_finite())) {
            return Err(invalid_data("CMG Schur activity state is nonfinite"));
        }
        // Negative activity is an underflow sentinel and remains active. Zero
        // alone marks completion; stale operator values must not survive there.
        if let Some(states) = active.filter(|states| states.contains(&0.0)) {
            for (firm, row) in output.chunks_exact_mut(stride).enumerate() {
                crate::interrupt::row_checkpoint(firm)?;
                for (value, &state) in row.iter_mut().zip(states) {
                    if state == 0.0 {
                        *value = 0.0;
                    }
                }
            }
        }
        let vertices = self.rhs.len() / self.batch_columns;
        let mut next_rhs = 0;
        while next_rhs < rhs_count {
            crate::interrupt::checkpoint()?;
            let mut lanes = [0; MAX_BATCH_COLUMNS];
            let mut columns = 0;
            while next_rhs < rhs_count && columns < self.batch_columns {
                if active.is_none_or(|states| states[next_rhs] != 0.0) {
                    lanes[columns] = next_rhs;
                    columns += 1;
                }
                next_rhs += 1;
            }
            if columns == 0 {
                break;
            }
            let nonzero = self.components.gather(
                residual,
                stride,
                &lanes[..columns],
                component_scales,
                &mut self.rhs[..firms * columns],
            )?;
            // A zero (including component-constant) submitted residual has an
            // exactly zero projected image. Remove it before traversing CMG.
            let live_columns = nonzero[..columns].iter().filter(|value| **value).count();
            if live_columns < columns {
                for firm in 0..firms {
                    crate::interrupt::row_checkpoint(firm)?;
                    let mut live = 0;
                    for lane in 0..columns {
                        if nonzero[lane] {
                            self.rhs[firm * live_columns + live] = self.rhs[firm * columns + lane];
                            live += 1;
                        } else {
                            output[firm * stride + lanes[lane]] = 0.0;
                        }
                    }
                }
                let mut live = 0;
                for lane in 0..columns {
                    if nonzero[lane] {
                        lanes[live] = lanes[lane];
                        live += 1;
                    }
                }
                columns = live_columns;
            }
            if columns == 0 {
                continue;
            }
            // Zero auxiliary loads in bounded segments between host polls.
            for (segment, values) in self.rhs[firms * columns..vertices * columns]
                .chunks_mut(crate::interrupt::CHECKPOINT_ROW_STRIDE)
                .enumerate()
            {
                crate::interrupt::row_checkpoint(
                    segment * crate::interrupt::CHECKPOINT_ROW_STRIDE,
                )?;
                values.fill(0.0);
            }
            self.cycle(vertices, columns, rhs_count == 1)?;
            self.components.scatter(
                &self.solution[..firms * columns],
                output,
                stride,
                &lanes[..columns],
            )?;
        }
        // A cancellation during the final scatter is never reported as success.
        crate::interrupt::checkpoint()
    }

    /// Bytes of one [`SharedScratch`] for [`Self::apply_shared`].
    pub(super) fn shared_scratch_bytes(&self) -> Result<u128> {
        let cancelled = crate::interrupt::cancellation_requested;
        let workspace = self
            .preconditioner
            .workspace_allocation_bytes_controlled(&cancelled)
            .map_err(cmg_error)? as u128;
        let vertices = self.vertex_count() as u128;
        vertices
            .checked_mul(2 * std::mem::size_of::<f64>() as u128)
            .and_then(|bytes| bytes.checked_add(workspace))
            .ok_or_else(|| allocation("shared CMG scratch dimensions overflow"))
    }

    pub(super) fn shared_scratch(&self) -> Result<SharedScratch> {
        let cancelled = crate::interrupt::cancellation_requested;
        let vertices = self.vertex_count();
        let message = "unable to reserve shared CMG scratch";
        let mut rhs = Vec::new();
        rhs.try_reserve_exact(vertices)
            .map_err(|_| allocation(message))?;
        rhs.resize(vertices, 0.0);
        let mut solution = Vec::new();
        solution
            .try_reserve_exact(vertices)
            .map_err(|_| allocation(message))?;
        solution.resize(vertices, 0.0);
        Ok(SharedScratch {
            rhs,
            solution,
            workspace: self
                .preconditioner
                .try_workspace_controlled(&cancelled)
                .map_err(cmg_error)?,
        })
    }

    /// `R B_h E` for a firm vector already compatible on its components: the
    /// one fixed cycle of [`Self::apply_batch`] without its component
    /// projections, on caller scratch, so workers can share one hierarchy.
    /// The output's component constants are arbitrary.
    pub(super) fn apply_shared(
        &self,
        input: &[f64],
        output: &mut [f64],
        scratch: &mut SharedScratch,
    ) -> Result<()> {
        let firms = self.components.labels.len();
        if input.len() != firms || output.len() != firms || scratch.rhs.len() < firms {
            return Err(invalid_argument("shared CMG dimensions are inconsistent"));
        }
        scratch.rhs[..firms].copy_from_slice(input);
        scratch.rhs[firms..].fill(0.0);
        let cancelled = crate::interrupt::cancellation_requested;
        self.preconditioner
            .apply_compatible_into_with(
                &scratch.rhs,
                &mut scratch.solution,
                &mut scratch.workspace,
                cmg::ApplyContext {
                    plan: None,
                    team: None,
                    cancel: Some(&cancelled),
                },
            )
            .map_err(cmg_error)?;
        output.copy_from_slice(&scratch.solution[..firms]);
        Ok(())
    }

    pub(super) fn firm_count(&self) -> usize {
        self.components.labels.len()
    }

    fn vertex_count(&self) -> usize {
        self.rhs.len() / self.batch_columns
    }

    /// One fixed cycle on the packed lanes. The projection's original width
    /// fixes the arithmetic path: a single-RHS projection uses the scalar
    /// cycle, while a compacted final active lane or one-column tile remainder
    /// of a multi-RHS projection still uses the interleaved cycle. CMG polls
    /// the host's cancellation flag at its bounded application cadence.
    fn cycle(&mut self, vertices: usize, columns: usize, single_rhs: bool) -> Result<()> {
        let cancelled = crate::interrupt::cancellation_requested;
        let rhs = &self.rhs[..vertices * columns];
        let solution = &mut self.solution[..vertices * columns];
        let preconditioner = &self.preconditioner;
        let plan = self.plan.as_ref();
        let scalar = &mut self.scalar_workspace;
        let batch = self.batch_workspace.as_mut().filter(|_| !single_rhs);
        let run = |team: Option<cmg::Team<'_>>| {
            let context = cmg::ApplyContext {
                plan,
                team,
                cancel: Some(&cancelled),
            };
            match batch {
                Some(workspace) => preconditioner
                    .apply_compatible_batch_into(rhs, solution, columns, workspace, context),
                None => preconditioner.apply_compatible_into_with(rhs, solution, scalar, context),
            }
            .map_err(cmg_error)
        };
        match self.install {
            // The automatic route keeps its team-free scalar application.
            Install::Auto => run(None),
            Install::Explicit { threads, .. } => {
                crate::parallel::with_cmg_team(threads, |team| run(Some(team)))
            }
        }
    }
}

/// One worker's storage for [`CmgSchurPreconditioner::apply_shared`].
pub(crate) struct SharedScratch {
    rhs: Vec<f64>,
    solution: Vec<f64>,
    workspace: CmgWorkspace,
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

    /// Clear the component accumulators in bounded segments between host polls.
    fn clear(&mut self, columns: usize) -> Result<()> {
        let elements = self.sizes.len() * columns;
        for (segment, (sums, corrections)) in self.sums[..elements]
            .chunks_mut(crate::interrupt::CHECKPOINT_ROW_STRIDE)
            .zip(self.corrections[..elements].chunks_mut(crate::interrupt::CHECKPOINT_ROW_STRIDE))
            .enumerate()
        {
            crate::interrupt::row_checkpoint(segment * crate::interrupt::CHECKPOINT_ROW_STRIDE)?;
            sums.fill(0.0);
            corrections.fill(0.0);
        }
        Ok(())
    }

    fn means(&mut self, columns: usize) -> Result<()> {
        for (component, &size) in self.sizes.iter().enumerate() {
            crate::interrupt::row_checkpoint(component)?;
            for lane in 0..columns {
                let index = component * columns + lane;
                self.sums[index] = (self.sums[index] + self.corrections[index]) / size as f64;
                if !self.sums[index].is_finite() {
                    return Err(invalid_data("CMG Schur component mean is nonfinite"));
                }
            }
        }
        Ok(())
    }

    /// Gather and the first centering pass share one traversal. Each column's
    /// Neumaier additions retain the original ascending-firm order.
    fn gather(
        &mut self,
        residual: &[f64],
        stride: usize,
        lanes: &[usize],
        scales: Option<&[f64]>,
        target: &mut [f64],
    ) -> Result<[bool; MAX_BATCH_COLUMNS]> {
        let columns = lanes.len();
        self.clear(columns)?;
        for (firm, &label) in self.labels.iter().enumerate() {
            crate::interrupt::row_checkpoint(firm)?;
            let scale = scales.map_or(1.0, |scales| scales[firm]);
            if !scale.is_finite() || scale <= 0.0 {
                return Err(invalid_data("CMG Schur component scale is invalid"));
            }
            for (lane, &source) in lanes.iter().enumerate() {
                let value = residual[firm * stride + source] * scale;
                if !value.is_finite() {
                    return Err(invalid_data("CMG Schur scaled residual is nonfinite"));
                }
                target[firm * columns + lane] = value;
                let index = label * columns + lane;
                neumaier_add(value, &mut self.sums[index], &mut self.corrections[index]);
            }
        }
        self.means(columns)?;
        let mut nonzero = [false; MAX_BATCH_COLUMNS];
        for (firm, &label) in self.labels.iter().enumerate() {
            crate::interrupt::row_checkpoint(firm)?;
            for lane in 0..columns {
                let value = &mut target[firm * columns + lane];
                *value -= self.sums[label * columns + lane];
                if !value.is_finite() {
                    return Err(invalid_data("CMG Schur centered value is nonfinite"));
                }
                nonzero[lane] |= *value != 0.0;
            }
        }
        Ok(nonzero)
    }

    /// Restriction, mean subtraction and scatter share the final traversal;
    /// the intermediate hybrid output never needs an in-place centering pass.
    fn scatter(
        &mut self,
        source: &[f64],
        output: &mut [f64],
        stride: usize,
        lanes: &[usize],
    ) -> Result<()> {
        let columns = lanes.len();
        self.clear(columns)?;
        for (firm, &label) in self.labels.iter().enumerate() {
            crate::interrupt::row_checkpoint(firm)?;
            for lane in 0..columns {
                let index = label * columns + lane;
                neumaier_add(
                    source[firm * columns + lane],
                    &mut self.sums[index],
                    &mut self.corrections[index],
                );
            }
        }
        self.means(columns)?;
        for (firm, &label) in self.labels.iter().enumerate() {
            crate::interrupt::row_checkpoint(firm)?;
            for (lane, &target) in lanes.iter().enumerate() {
                let value = source[firm * columns + lane] - self.sums[label * columns + lane];
                if !value.is_finite() {
                    return Err(invalid_data("CMG Schur centered value is nonfinite"));
                }
                output[firm * stride + target] = value;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct GraphCounts {
    retained_workers: usize,
    sampled_workers: usize,
    input_edges: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorCode, FeregError};
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

        fn torus(width: usize) -> Self {
            let mut workers = Vec::new();
            let mut firms = Vec::new();
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
            Self::new(workers, firms, Vec::new())
        }

        fn sampled_degrees(duplicates: bool, scale: f64) -> Self {
            let mut workers = Vec::new();
            let mut firms = Vec::new();
            let mut weights = Vec::new();
            let mut first = 0;
            for (worker, degree) in [30, 60, 3].into_iter().enumerate() {
                for firm in 0..degree {
                    let copies = if duplicates && firm % 7 == 0 { 2 } else { 1 };
                    for _ in 0..copies {
                        workers.push(worker as u32);
                        firms.push(first + firm);
                        weights.push((1 + firm % 5) as f64 * scale / copies as f64);
                    }
                }
                first += degree;
            }
            Self::new(workers, firms, weights)
        }
    }

    fn edges(solver: &CmgSchurPreconditioner) -> Vec<(usize, usize, u64)> {
        solver.preconditioner.hierarchy().levels()[0]
            .graph()
            .edges()
            .iter()
            .map(|edge| (edge.u(), edge.v(), edge.weight().to_bits()))
            .collect()
    }

    #[test]
    fn bounded_serial_candidates_preserve_graphs_and_fixed_symmetric_cycles() {
        let fixture = Fixture::sampled_degrees(true, 1.0);
        let design = fixture.design();
        for samples in [0, 1, 2] {
            let sampling = Sampling { samples, seed: 0 };
            let explicit = CmgSchurPreconditioner::build(
                &design,
                BorrowedMapWeights::Provided(&fixture.weights),
                0,
                5,
                sampling,
                2,
                1,
                None,
            )
            .unwrap();
            let built = CmgSchurPreconditioner::try_build_bounded(
                &design,
                BorrowedMapWeights::Provided(&fixture.weights),
                0,
                5,
                sampling,
                2,
                None,
                false,
            )
            .unwrap();
            assert_eq!(built.refusal, None);
            let mut candidate = built.preconditioner.unwrap();
            assert_eq!(candidate.install, Install::Auto);
            assert!(candidate.plan.is_none() && candidate.batch_workspace.is_none());
            assert_eq!(edges(&candidate), edges(&explicit));
            assert_eq!(candidate.components.labels, explicit.components.labels);
            let n = fixture.firm_index.n_levels();
            let residual: Vec<_> = (0..n * 2).map(|i| (i as f64 * 0.31).sin()).collect();
            let mut output = vec![0.0; residual.len()];
            candidate
                .apply_batch(&residual, &mut output, 2, 2, None, None)
                .unwrap();
            let reference = output.clone();
            candidate
                .apply_batch(&residual, &mut output, 2, 2, None, None)
                .unwrap();
            assert_eq!(output, reference);
            let xbx: f64 = (0..n).map(|i| residual[2 * i] * output[2 * i]).sum();
            let xby: f64 = (0..n).map(|i| residual[2 * i] * output[2 * i + 1]).sum();
            let ybx: f64 = (0..n).map(|i| residual[2 * i + 1] * output[2 * i]).sum();
            assert!(xbx > 0.0);
            assert!((xby - ybx).abs() < 1e-11 * xby.abs().max(1.0));
            candidate
                .apply_batch(&vec![1.0; residual.len()], &mut output, 2, 2, None, None)
                .unwrap();
            assert!(output.iter().all(|value| *value == 0.0));
            assert!(candidate.retained_bytes().unwrap() <= built.peak.bytes().unwrap() as u128);
        }
    }

    #[test]
    fn bounded_csr_inverse_matches_analytic_schur_across_weight_scales() {
        for exponent in [-500, 0, 500] {
            let scale = 2.0_f64.powi(exponent);
            let weights = [2.0 * scale, 3.0 * scale, 2.0 * scale, 3.0 * scale];
            let (built, normalization) = CmgSchurPreconditioner::try_build_reduced_bounded(
                2,
                2,
                &[0, 2, 4],
                &[0, 1, 0, 1],
                &weights,
                5,
                Sampling::default(),
                None,
            )
            .unwrap();
            let mut candidate = built.preconditioner.unwrap();
            assert_eq!(normalization, 3.0 * scale);
            let mut rhs = [1.0, -1.0];
            candidate
                .apply_reduced_in_place(&mut rhs, &mut [0.0; 2], normalization)
                .unwrap();
            // Two [2,3] rows give S=2.4*[1,-1;-1,1]*scale.
            let expected = 5.0 / 24.0 / scale;
            assert!((rhs[0] / expected - 1.0).abs() < 2e-14);
            assert!((rhs[1] / expected + 1.0).abs() < 2e-14);
            let mut constant = [1.0, 1.0];
            candidate
                .apply_reduced_in_place(&mut constant, &mut [0.0; 2], normalization)
                .unwrap();
            assert_eq!(constant, [0.0; 2]);
        }
        // Relative weight range matters separately from a harmless global
        // scale. Pre-dividing this residual by max(weight) would erase it.
        let (built, normalization) = CmgSchurPreconditioner::try_build_reduced_bounded(
            1,
            2,
            &[0, 2],
            &[0, 1],
            &[1.0, 1e300],
            5,
            Sampling::default(),
            None,
        )
        .unwrap();
        let mut candidate = built.preconditioner.unwrap();
        let mut rhs = [1e-100, -1e-100];
        candidate
            .apply_reduced_in_place(&mut rhs, &mut [0.0; 2], normalization)
            .unwrap();
        assert!((rhs[0] / 5e-101 - 1.0).abs() < 2e-14);
        assert!((rhs[1] / 5e-101 + 1.0).abs() < 2e-14);
        let malformed = CmgSchurPreconditioner::try_build_reduced_bounded(
            2,
            2,
            &[0, 3, 2],
            &[0, 1],
            &[1.0, 1.0],
            5,
            Sampling::default(),
            None,
        )
        .err()
        .unwrap();
        assert_eq!(malformed.code, ErrorCode::InvalidData);
        let disconnected = CmgSchurPreconditioner::try_build_reduced_bounded(
            2,
            2,
            &[0, 1, 2],
            &[0, 1],
            &[1.0, 1.0],
            5,
            Sampling::default(),
            None,
        )
        .err()
        .unwrap();
        assert_eq!(disconnected.code, ErrorCode::InvalidData);
    }

    #[test]
    fn bounded_memory_refusal_does_not_hide_invalid_inputs() {
        let fixture = Fixture::weighted_stars();
        let design = fixture.design();
        let memory = CmgSchurPreconditioner::try_build_bounded(
            &design,
            BorrowedMapWeights::Unit,
            0,
            5,
            Sampling::default(),
            1,
            Some(FeSolverMemoryBudget {
                usable_bytes: 0,
                execution_bytes: 0,
            }),
            false,
        )
        .unwrap();
        assert!(memory.preconditioner.is_none());
        assert_eq!(memory.refusal, Some(bounded::BuildRefusal::MemoryBudget));
        let mut weights = fixture.weights.clone();
        weights[0] = f64::NAN;
        let error = CmgSchurPreconditioner::try_build_bounded(
            &design,
            BorrowedMapWeights::Provided(&weights),
            0,
            5,
            Sampling::default(),
            1,
            None,
            false,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn sampled_degree_30_60_graphs_preserve_components_and_fixed_positive_cycles() {
        let fixture = Fixture::sampled_degrees(true, 1.0);
        let design = fixture.design();
        let exact = CmgSchurPreconditioner::build(
            &design,
            BorrowedMapWeights::Provided(&fixture.weights),
            0,
            4,
            Sampling::default(),
            2,
            1,
            None,
        )
        .unwrap();
        assert_eq!(
            exact.preconditioner.hierarchy().levels()[0]
                .graph()
                .vertex_count(),
            95
        );
        assert_eq!(edges(&exact).len(), 93);
        let exact_low: Vec<_> = edges(&exact)
            .into_iter()
            .filter(|(u, v, _)| *u >= 90 && *v < 93)
            .collect();
        for samples in [1, 4] {
            let mut reference = None;
            for threads in [1, 4] {
                let mut solver = CmgSchurPreconditioner::build_with_options(
                    &design,
                    BorrowedMapWeights::Provided(&fixture.weights),
                    0,
                    4,
                    Sampling { samples, seed: 19 },
                    2,
                    threads,
                    None,
                    CmgOptions {
                        direct_threshold: 8,
                        ..CmgOptions::default()
                    },
                )
                .unwrap();
                let graph = solver.preconditioner.hierarchy().levels()[0].graph();
                assert_eq!(graph.vertex_count(), 93);
                assert!(graph
                    .edges()
                    .iter()
                    .all(|edge| edge.weight().is_finite() && edge.weight() > 0.0));
                let components = cmg::Components::from_laplacian(graph);
                assert_eq!(components.sizes(), &[30, 60, 3]);
                if samples == 1 {
                    assert_eq!(graph.edge_count(), 91);
                }
                let signature = edges(&solver);
                assert_eq!(
                    signature
                        .iter()
                        .copied()
                        .filter(|(u, _, _)| *u >= 90)
                        .collect::<Vec<_>>(),
                    exact_low
                );
                if let Some(prior) = reference.as_ref() {
                    assert_eq!(&signature, prior);
                } else {
                    reference = Some(signature);
                }
                let residual: Vec<_> = (0..186).map(|index| (index as f64 * 0.31).sin()).collect();
                let mut output = vec![0.0; residual.len()];
                solver
                    .apply_batch(&residual, &mut output, 2, 2, None, None)
                    .unwrap();
                let mut xbx = 0.0;
                let mut xby = 0.0;
                let mut ybx = 0.0;
                for firm in 0..93 {
                    xbx += residual[2 * firm] * output[2 * firm];
                    xby += residual[2 * firm] * output[2 * firm + 1];
                    ybx += residual[2 * firm + 1] * output[2 * firm];
                }
                assert!(xbx > 0.0);
                assert!((xby - ybx).abs() < 1e-11 * xby.abs().max(1.0));
                let first = output.clone();
                solver
                    .apply_batch(&residual, &mut output, 2, 2, None, None)
                    .unwrap();
                assert_eq!(output, first);
            }
        }
    }

    #[test]
    fn sampled_graphs_repeat_under_duplicates_binary_weight_scaling_and_fixed_seed() {
        let mut reference = None;
        for (duplicates, scale) in [
            (false, 1.0),
            (true, 1.0),
            (true, 2.0_f64.powi(-100)),
            (true, 2.0_f64.powi(100)),
        ] {
            let fixture = Fixture::sampled_degrees(duplicates, scale);
            let design = fixture.design();
            let solver = CmgSchurPreconditioner::build(
                &design,
                BorrowedMapWeights::Provided(&fixture.weights),
                0,
                5,
                Sampling {
                    samples: 4,
                    seed: 0,
                },
                1,
                1,
                None,
            )
            .unwrap();
            let signature = edges(&solver);
            if let Some(prior) = reference.as_ref() {
                assert_eq!(&signature, prior);
            } else {
                reference = Some(signature);
            }
            let changed = CmgSchurPreconditioner::build(
                &design,
                BorrowedMapWeights::Provided(&fixture.weights),
                0,
                5,
                Sampling {
                    samples: 4,
                    seed: 1,
                },
                1,
                1,
                None,
            )
            .unwrap();
            assert_ne!(edges(&changed), *reference.as_ref().unwrap());
        }
    }

    #[test]
    fn sampled_scratch_and_graph_are_admitted_and_underflow_is_explicit() {
        let mut fixture = Fixture::sampled_degrees(false, 1.0);
        let design = fixture.design();
        let build = |sampling, budget| {
            CmgSchurPreconditioner::build(
                &design,
                BorrowedMapWeights::Provided(&fixture.weights),
                0,
                4,
                sampling,
                2,
                1,
                budget,
            )
        };
        let sampling = Sampling {
            samples: 3,
            seed: 2,
        };
        let solver = build(sampling, None).unwrap();
        let peak = solver.construction_peak().bytes().unwrap() as u128;
        assert!(solver.retained_bytes().unwrap() <= peak);
        drop(solver);
        assert!(build(
            sampling,
            Some(FeSolverMemoryBudget {
                execution_bytes: 11,
                usable_bytes: peak + 11
            })
        )
        .is_ok());
        assert!(matches!(
            build(
                sampling,
                Some(FeSolverMemoryBudget {
                    execution_bytes: 11,
                    usable_bytes: peak + 10
                })
            ),
            Err(FeregError {
                code: ErrorCode::Allocation,
                ..
            })
        ));
        assert!(matches!(
            build(
                Sampling {
                    samples: u32::MAX,
                    seed: 0
                },
                Some(FeSolverMemoryBudget {
                    execution_bytes: 11,
                    usable_bytes: peak + 11
                })
            ),
            Err(FeregError {
                code: ErrorCode::Allocation,
                ..
            })
        ));
        drop(design);
        fixture.weights.fill(1.0);
        fixture.weights[0] = f64::from_bits(1);
        assert!(matches!(
            CmgSchurPreconditioner::build(
                &fixture.design(),
                BorrowedMapWeights::Provided(&fixture.weights),
                0,
                4,
                Sampling {
                    samples: 2,
                    seed: 0
                },
                1,
                1,
                None,
            ),
            Err(FeregError {
                code: ErrorCode::InvalidData,
                ..
            })
        ));
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
                Sampling::default(),
                3,
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
                .apply_batch(&residual, &mut output, 3, 2, Some(&scales), None)
                .unwrap();
            for (actual, expected) in output.iter().zip(&expected) {
                assert!(
                    (actual - expected).abs() < 2e-12,
                    "k={cutoff}: {actual} != {expected}"
                );
            }
            let first_output = output.clone();
            solver
                .apply_batch(&residual, &mut output, 3, 2, Some(&scales), None)
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
                    Sampling::default(),
                    3,
                    1,
                    None,
                )
                .unwrap();
                scaled_solver
                    .apply_batch(&residual, &mut output, 3, 2, Some(&scales), None)
                    .unwrap();
                for (actual, reference) in output.iter().zip(&first_output) {
                    assert!((actual - reference).abs() < 2e-11);
                }
            }
        }
    }

    #[test]
    fn fixed_multilevel_cycle_is_linear_symmetric_positive_and_repeatable() {
        let width = 20;
        let fixture = Fixture::torus(width);
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
                Sampling::default(),
                3,
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
                .apply_batch(&residual, &mut output, 3, 3, None, None)
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
                .apply_batch(&residual, &mut output, 3, 3, None, None)
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
    fn original_single_rhs_reuses_exact_scalar_cycle_in_a_wide_session() {
        let fixture = Fixture::torus(65);
        let design = fixture.design();
        let firms = fixture.firm_index.n_levels();
        let columns = 5;
        let residual: Vec<f64> = (0..firms * columns)
            .map(|index| (index as f64 * 0.17).sin())
            .collect();
        let options = CmgOptions {
            direct_threshold: 16,
            ..CmgOptions::default()
        };
        for threads in [1, 4] {
            let mut solver = CmgSchurPreconditioner::build_with_options(
                &design,
                BorrowedMapWeights::Unit,
                0,
                4,
                Sampling::default(),
                columns,
                threads,
                None,
                options,
            )
            .unwrap();
            assert!(solver.batch_workspace.is_some());
            assert!(solver.preconditioner.hierarchy().levels().len() > 1);
            if threads > 1 {
                assert!(solver.plan.as_ref().unwrap().operator_count() > 0);
            }
            let vertices = solver.rhs.len() / solver.batch_columns;
            let mut scalar_rhs = vec![0.0; vertices];
            let mut scalar_solution = vec![0.0; vertices];
            let mut scalar_workspace = solver.preconditioner.try_workspace().unwrap();
            let mut expected = vec![0.0; residual.len()];
            solver
                .components
                .gather(&residual, columns, &[0], None, &mut scalar_rhs[..firms])
                .unwrap();
            crate::parallel::with_cmg_team(threads, |team| {
                solver
                    .preconditioner
                    .apply_compatible_into_with(
                        &scalar_rhs,
                        &mut scalar_solution,
                        &mut scalar_workspace,
                        cmg::ApplyContext {
                            plan: solver.plan.as_ref(),
                            team: Some(team),
                            cancel: None,
                        },
                    )
                    .map_err(cmg_error)
            })
            .unwrap();
            solver
                .components
                .scatter(&scalar_solution[..firms], &mut expected, columns, &[0])
                .unwrap();
            let mut output = vec![0.0; residual.len()];
            solver
                .apply_batch(&residual, &mut output, columns, 1, None, None)
                .unwrap();
            assert_eq!(output, expected);

            // Preserve the original multi-RHS path for both a one-column tile
            // remainder and a mask leaving only one active lane. A deliberately
            // incompatible scalar workspace would fail if either took it.
            solver
                .apply_batch(&residual, &mut output, columns, columns, None, None)
                .unwrap();
            let batch_expected = output.clone();
            let tiny_graph = Laplacian::from_edges(2, [(0, 1, 1.0)]).unwrap();
            let tiny_cmg = CmgPreconditioner::build(&tiny_graph, CmgOptions::default()).unwrap();
            let retained_scalar = std::mem::replace(
                &mut solver.scalar_workspace,
                tiny_cmg.try_workspace().unwrap(),
            );
            solver
                .apply_batch(&residual, &mut output, columns, columns, None, None)
                .unwrap();
            assert_eq!(output, batch_expected);
            solver
                .apply_batch(
                    &residual,
                    &mut output,
                    columns,
                    columns,
                    None,
                    Some(&[0.0, 0.0, -1.0, 0.0, 0.0]),
                )
                .unwrap();
            for firm in 0..firms {
                for rhs in 0..columns {
                    let reference = if rhs == 2 {
                        batch_expected[firm * columns + rhs]
                    } else {
                        0.0
                    };
                    assert_eq!(output[firm * columns + rhs], reference);
                }
            }
            solver.scalar_workspace = retained_scalar;
            output.fill(0.0);
            solver
                .apply_batch(&residual, &mut output, columns, 1, None, None)
                .unwrap();
            assert_eq!(output, expected);
            assert!(
                solver.retained_bytes().unwrap()
                    <= solver.construction_peak().bytes().unwrap() as u128
            );
        }
    }

    #[test]
    fn compact_batches_match_scalar_cycles_across_masks_zeroes_and_reused_tails() {
        let fixture = Fixture::weighted_stars();
        let design = fixture.design();
        let columns = 7;
        let firms = fixture.firm_index.n_levels();
        for cutoff in 4..=6 {
            let mut solver = CmgSchurPreconditioner::build(
                &design,
                BorrowedMapWeights::Provided(&fixture.weights),
                0,
                cutoff,
                Sampling::default(),
                columns,
                2,
                None,
            )
            .unwrap();
            assert_eq!(solver.batch_columns, MAX_BATCH_COLUMNS);
            let vertices = solver.rhs.len() / solver.batch_columns;
            let mut scalar_workspace = solver.preconditioner.try_workspace().unwrap();
            let mut scalar_rhs = vec![0.0; vertices];
            let mut scalar_solution = vec![0.0; vertices];
            let mut residual = vec![0.0; firms * columns];
            let mut output = vec![123.0; residual.len()];
            let mut expected = output.clone();
            let mut scales = vec![0.0; firms];
            for (firm, scale) in scales.iter_mut().enumerate() {
                *scale = (solver.components.labels[firm] + 1) as f64;
            }
            for submitted in [7, 7, 3, 1, 7] {
                for firm in 0..firms {
                    for rhs in 0..columns {
                        residual[firm * columns + rhs] = ((firm * 3 + rhs) as f64 * 0.17).sin();
                    }
                }
                let mut states = vec![1.0; submitted];
                if submitted > 3 {
                    states[2] = 0.0;
                    states[3] = -1.0;
                    for firm in 0..firms {
                        residual[firm * columns] = 0.0;
                        residual[firm * columns + 2] = f64::NAN;
                        residual[firm * columns + 4] = 0.0;
                    }
                }
                for (rhs, &state) in states.iter().enumerate() {
                    if state == 0.0 {
                        for firm in 0..firms {
                            expected[firm * columns + rhs] = 0.0;
                        }
                        continue;
                    }
                    solver
                        .components
                        .gather(
                            &residual,
                            columns,
                            &[rhs],
                            Some(&scales),
                            &mut scalar_rhs[..firms],
                        )
                        .unwrap();
                    scalar_rhs[firms..].fill(0.0);
                    crate::parallel::with_cmg_team(2, |team| {
                        solver
                            .preconditioner
                            .apply_compatible_into_with(
                                &scalar_rhs,
                                &mut scalar_solution,
                                &mut scalar_workspace,
                                cmg::ApplyContext {
                                    plan: solver.plan.as_ref(),
                                    team: Some(team),
                                    cancel: None,
                                },
                            )
                            .map_err(cmg_error)
                    })
                    .unwrap();
                    solver
                        .components
                        .scatter(&scalar_solution[..firms], &mut expected, columns, &[rhs])
                        .unwrap();
                }
                solver
                    .apply_batch(
                        &residual,
                        &mut output,
                        columns,
                        submitted,
                        Some(&scales),
                        Some(&states),
                    )
                    .unwrap();
                for (actual, reference) in output.iter().zip(&expected) {
                    assert!((actual - reference).abs() < 2e-12);
                }
                let repeated = output.clone();
                solver
                    .apply_batch(
                        &residual,
                        &mut output,
                        columns,
                        submitted,
                        Some(&scales),
                        Some(&states),
                    )
                    .unwrap();
                assert_eq!(output, repeated);
            }
            residual.fill(f64::NAN);
            solver
                .apply_batch(
                    &residual,
                    &mut output,
                    columns,
                    columns,
                    None,
                    Some(&[0.0; 7]),
                )
                .unwrap();
            assert!(output.iter().all(|value| *value == 0.0));
        }
    }

    #[test]
    fn memory_admission_and_invalid_requests_fail_closed() {
        let fixture = Fixture::weighted_stars();
        let design = fixture.design();
        for rhs_capacity in [1, 4] {
            let build = |budget| {
                CmgSchurPreconditioner::build(
                    &design,
                    BorrowedMapWeights::Unit,
                    0,
                    5,
                    Sampling::default(),
                    rhs_capacity,
                    1,
                    budget,
                )
            };
            let baseline = build(None).unwrap();
            assert_eq!(baseline.batch_workspace.is_some(), rhs_capacity > 1);
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
                CmgSchurPreconditioner::build(
                    &design,
                    BorrowedMapWeights::Unit,
                    0,
                    7,
                    Sampling::default(),
                    1,
                    1,
                    None
                ),
                Err(FeregError {
                    code: ErrorCode::InvalidArgument,
                    ..
                })
            ));
            let mut solver = build(None).unwrap();
            assert!(solver
                .apply_batch(&[0.0], &mut [0.0], 1, 1, None, None)
                .is_err());
            let mut residual = vec![0.0; 28];
            residual[1] = f64::NAN;
            assert!(solver
                .apply_batch(&residual, &mut vec![0.0; 28], 1, 1, None, None)
                .is_err());
        }
    }

    // Host cancellation raised inside CMG's planned interleaved cycle or a
    // planned PCG solve on the explicit route's host team joins every lane,
    // maps to an interrupted estimation and leaves the installed hierarchy,
    // plan and workspaces reusable with unchanged bits.
    #[test]
    fn explicit_kernels_cancel_inside_application_and_pcg_then_recover() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        const CHILD: &str = "FEREG_CMG_SCHUR_KERNEL_CANCEL_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD, "1")
                .args([
                    "fe::cmg_schur::tests::explicit_kernels_cancel_inside_application_and_pcg_then_recover",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let bits = |values: &[f64]| {
            values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        };
        let fixture = Fixture::torus(65);
        let design = fixture.design();
        let firms = fixture.firm_index.n_levels();
        let columns = 4;
        let residual: Vec<f64> = (0..firms * columns)
            .map(|index| (index as f64 * 0.17).sin())
            .collect();
        let mut solver = CmgSchurPreconditioner::build_with_options(
            &design,
            BorrowedMapWeights::Unit,
            0,
            4,
            Sampling::default(),
            columns,
            4,
            None,
            CmgOptions {
                direct_threshold: 16,
                ..CmgOptions::default()
            },
        )
        .unwrap();
        assert!(solver.plan.as_ref().unwrap().operator_count() > 0);
        let mut expected = vec![0.0; residual.len()];
        solver
            .apply_batch(&residual, &mut expected, columns, columns, None, None)
            .unwrap();
        // The packed, component-centered tile of that projection.
        let vertices = solver.rhs.len() / solver.batch_columns;
        let packed = solver.rhs[..vertices * columns].to_vec();
        let polls = AtomicUsize::new(0);
        let query = |stop: Option<usize>| {
            let polls = &polls;
            move || {
                let poll = polls.fetch_add(1, Ordering::Relaxed);
                if stop == Some(poll) {
                    crate::interrupt::request_cancellation();
                }
                crate::interrupt::cancellation_requested()
            }
        };
        let apply = |solver: &mut CmgSchurPreconditioner, stop: Option<usize>| {
            polls.store(0, Ordering::Relaxed);
            let cancel = query(stop);
            let CmgSchurPreconditioner {
                preconditioner,
                plan,
                batch_workspace,
                solution,
                ..
            } = solver;
            crate::parallel::with_cmg_team(4, |team| {
                preconditioner
                    .apply_compatible_batch_into(
                        &packed,
                        &mut solution[..vertices * columns],
                        columns,
                        batch_workspace.as_mut().unwrap(),
                        cmg::ApplyContext {
                            plan: plan.as_ref(),
                            team: Some(team),
                            cancel: Some(&cancel),
                        },
                    )
                    .map_err(cmg_error)
            })
        };
        apply(&mut solver, None).unwrap();
        let reference = bits(&solver.solution[..vertices * columns]);
        let total = polls.load(Ordering::Relaxed);
        assert!(total >= 3, "{total} application polls");
        // Lane polls depend on chunk claiming, so a late stop may not be
        // reached; a query that answered true never yields success.
        for stop in [0, total / 2, total - 1] {
            match apply(&mut solver, Some(stop)) {
                Err(error) => assert_eq!(error.code, ErrorCode::Interrupted),
                Ok(()) => assert!(polls.load(Ordering::Relaxed) <= stop),
            }
            crate::interrupt::reset_cancellation();
        }
        apply(&mut solver, None).unwrap();
        assert_eq!(bits(&solver.solution[..vertices * columns]), reference);
        let mut output = vec![0.0; residual.len()];
        solver
            .apply_batch(&residual, &mut output, columns, columns, None, None)
            .unwrap();
        assert_eq!(bits(&output), bits(&expected));
        // A pending host cancellation also stops the public application.
        crate::interrupt::request_cancellation();
        assert_eq!(
            solver
                .apply_batch(&residual, &mut output, columns, columns, None, None)
                .unwrap_err()
                .code,
            ErrorCode::Interrupted
        );
        crate::interrupt::reset_cancellation();

        // One planned PCG solve with the same hierarchy, plan and team.
        let graph = solver.preconditioner.hierarchy().levels()[0]
            .graph()
            .clone();
        let n = graph.vertex_count();
        let potentials: Vec<f64> = (0..n).map(|vertex| (vertex as f64 * 0.11).cos()).collect();
        let rhs = graph.matvec(&potentials).unwrap();
        let bytes =
            cmg::PcgBatchWorkspace::bank_allocation_bytes(&solver.preconditioner, 1).unwrap();
        let mut workspaces =
            cmg::PcgBatchWorkspace::new_bank_with_budget(&solver.preconditioner, 1, bytes).unwrap();
        let mut solution = vec![0.0; n];
        let mut diagnostics = [cmg::PcgDiagnostics::default()];
        let mut solve = |stop: Option<usize>| {
            polls.store(0, Ordering::Relaxed);
            let cancel = query(stop);
            solution.fill(0.0);
            crate::parallel::with_cmg_team(4, |team| {
                cmg::solve_pcg_batch_into(
                    &graph,
                    &solver.preconditioner,
                    cmg::PcgBatchRef::contiguous(&rhs, 1, n).unwrap(),
                    None,
                    cmg::PcgBatchMut::contiguous(&mut solution, 1, n).unwrap(),
                    &mut diagnostics,
                    cmg::PcgOptions::default(),
                    &mut workspaces,
                    cmg::SolveContext {
                        plan: solver.plan.as_ref(),
                        team: Some(team),
                        cancel: Some(&cancel),
                        ..cmg::SolveContext::default()
                    },
                )
                .map_err(cmg_error)
            })
            .map(|()| (bits(&solution), diagnostics[0].iterations()))
        };
        let reference = solve(None).unwrap();
        let total = polls.load(Ordering::Relaxed);
        assert!(
            total > reference.1,
            "{total} polls over {} iterations",
            reference.1
        );
        for stop in [1, total / 2, total - 1] {
            match solve(Some(stop)) {
                Err(error) => assert_eq!(error.code, ErrorCode::Interrupted),
                Ok(_) => assert!(polls.load(Ordering::Relaxed) <= stop),
            }
            crate::interrupt::reset_cancellation();
        }
        assert_eq!(solve(None).unwrap(), reference);
    }

    // The explicit adapter's plan, workspaces and application poll the host
    // flag inside their stages on a four-worker team; cancellation at sampled
    // host queries is Interrupted, and a fresh build repeats the same bits.
    #[test]
    fn explicit_build_and_application_poll_the_host_inside_plan_and_workspaces() {
        const CHILD: &str = "FEREG_CMG_SCHUR_BUILD_POLL_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD, "1")
                .args([
                    "fe::cmg_schur::tests::explicit_build_and_application_poll_the_host_inside_plan_and_workspaces",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        use crate::interrupt::test_polls;
        let fixture = Fixture::torus(65);
        let design = fixture.design();
        let firms = fixture.firm_index.n_levels();
        let columns = 4;
        let residual: Vec<f64> = (0..firms * columns)
            .map(|index| (index as f64 * 0.17).sin())
            .collect();
        let run = || -> Result<Vec<u64>> {
            let mut solver = CmgSchurPreconditioner::build_with_options(
                &design,
                BorrowedMapWeights::Unit,
                0,
                4,
                Sampling::default(),
                columns,
                4,
                None,
                CmgOptions {
                    direct_threshold: 16,
                    ..CmgOptions::default()
                },
            )?;
            let mut output = vec![0.0; residual.len()];
            solver.apply_batch(&residual, &mut output, columns, columns, None, None)?;
            Ok(output.iter().map(|value| value.to_bits()).collect())
        };
        test_polls::cancel_at(None);
        let reference = run().unwrap();
        let total = test_polls::polls();
        let mut phases = std::collections::BTreeSet::new();
        let mut cancel = |stop: usize| -> Option<String> {
            test_polls::cancel_at(Some(stop));
            match run() {
                Err(error) => {
                    assert_eq!(error.code, ErrorCode::Interrupted, "{error}");
                    let phase = error
                        .message
                        .split_once("cancelled during ")
                        .map_or_else(|| "fereg".to_string(), |(_, phase)| phase.to_string());
                    phases.insert(phase.clone());
                    Some(phase)
                }
                Ok(_) => {
                    assert!(test_polls::polls() <= stop, "poll {stop} ignored");
                    None
                }
            }
        };
        let stops: Vec<usize> = (0..total)
            .step_by((total / 400).max(1))
            .chain([total - 1])
            .collect();
        let sampled: Vec<Option<String>> = stops.iter().map(|&stop| cancel(stop)).collect();
        // Plan publication is one checkpoint after the plan's CSR polls and
        // before the workspaces, so every stop from the first sampled plan
        // stop to the first sampled workspace stop is visited.
        let phase_at = |prefix: &str, from: usize| {
            (from..sampled.len())
                .find(|&index| {
                    sampled[index]
                        .as_deref()
                        .is_some_and(|p| p.starts_with(prefix))
                })
                .expect("phase not sampled")
        };
        let plan = phase_at("plan ", 0);
        let workspace = phase_at("CMG workspace", plan);
        for stop in stops[plan] + 1..stops[workspace] {
            cancel(stop);
        }
        // The torus has degree four, so its row operators use serial CSR.
        for phase in [
            "CSR scatter",
            "plan publication",
            "CMG batch workspace",
            "fereg",
        ] {
            assert!(phases.contains(phase), "{phase} not in {phases:?}");
        }
        test_polls::cancel_at(None);
        assert_eq!(run().unwrap(), reference);
    }

    // Pre-consolidation witness: digests recorded from the explicit executor
    // builder and bounded automatic builder at fereg e9efda55 / CMG 03a26041,
    // using the dyadic inputs below. These expectations come from the accepted
    // pre-M3b implementation, not the consolidated adapter. The consolidated
    // adapter must reproduce graphs, labels, hierarchy metadata, application
    // policy and every cycle output bit at one and four workers.
    #[test]
    fn explicit_and_automatic_builds_reproduce_frozen_pre_consolidation_bits() {
        fn mix(digest: &mut u64, word: u64) {
            for byte in word.to_le_bytes() {
                *digest ^= u64::from(byte);
                *digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        fn digest(solver: &mut CmgSchurPreconditioner, firms: usize, capacity: usize) -> u64 {
            let mut value = 0xcbf2_9ce4_8422_2325_u64;
            for (u, v, w) in edges(solver) {
                mix(&mut value, u as u64);
                mix(&mut value, v as u64);
                mix(&mut value, w);
            }
            for &label in &solver.components.labels {
                mix(&mut value, label as u64);
            }
            for byte in solver.trace_hierarchy().bytes() {
                mix(&mut value, u64::from(byte));
            }
            let stride = capacity;
            // Small dyadic inputs have identical bits across platform math libraries.
            let residual: Vec<f64> = (0..firms * stride)
                .map(|index| ((index * 37 % 257) as f64 - 128.0) / 64.0 + (index % 3) as f64 * 0.25)
                .collect();
            let scales: Vec<f64> = (0..firms).map(|firm| 1.0 + (firm % 4) as f64).collect();
            let mut mask = vec![1.0; stride];
            mask[0] = 0.0;
            if stride > 2 {
                mask[2] = -1.0;
            }
            let mut output = vec![0.0; residual.len()];
            for rhs_count in [1, stride] {
                for (scaled, masked) in [(false, false), (true, false), (false, true)] {
                    if masked && rhs_count == 1 {
                        continue;
                    }
                    output.fill(0.5);
                    solver
                        .apply_batch(
                            &residual,
                            &mut output,
                            stride,
                            rhs_count,
                            scaled.then_some(&scales[..]),
                            masked.then_some(&mask[..rhs_count]),
                        )
                        .unwrap();
                    for entry in &output {
                        mix(&mut value, entry.to_bits());
                    }
                }
            }
            value
        }
        let torus_options = |threshold| CmgOptions {
            direct_threshold: threshold,
            ..CmgOptions::default()
        };
        let cases: Vec<(&str, Fixture, bool, usize, u32, CmgOptions)> = vec![
            (
                "sampled0",
                Fixture::sampled_degrees(true, 1.0),
                true,
                5,
                0,
                CmgOptions::default(),
            ),
            (
                "sampled1",
                Fixture::sampled_degrees(true, 1.0),
                true,
                5,
                1,
                CmgOptions::default(),
            ),
            (
                "sampled2",
                Fixture::sampled_degrees(true, 1.0),
                true,
                5,
                2,
                CmgOptions::default(),
            ),
            (
                "stars4",
                Fixture::weighted_stars(),
                true,
                4,
                0,
                CmgOptions::default(),
            ),
            (
                "stars6",
                Fixture::weighted_stars(),
                true,
                6,
                0,
                CmgOptions::default(),
            ),
            ("torus20", Fixture::torus(20), false, 4, 0, torus_options(8)),
            (
                "torus65",
                Fixture::torus(65),
                false,
                4,
                0,
                torus_options(16),
            ),
        ];
        let record = std::env::var_os("FEREG_RECORD_FROZEN_CMG_SCHUR").is_some();
        let mut actual = Vec::new();
        for (name, fixture, provided, degree, samples, options) in &cases {
            let design = fixture.design();
            let weights = if *provided {
                BorrowedMapWeights::Provided(&fixture.weights)
            } else {
                BorrowedMapWeights::Unit
            };
            let sampling = Sampling {
                samples: *samples,
                seed: 0,
            };
            let firms = fixture.firm_index.n_levels();
            for threads in [1, 4] {
                for capacity in [1, 2, 5] {
                    let value = crate::ThreadTeam::new(
                        crate::ThreadConfig::from_requested(threads, true).unwrap(),
                    )
                    .install(|| {
                        let mut explicit = CmgSchurPreconditioner::build_with_options(
                            &design, weights, 0, *degree, sampling, capacity, threads, None,
                            *options,
                        )
                        .unwrap();
                        let explicit = digest(&mut explicit, firms, capacity);
                        let automatic = if *options == CmgOptions::default() {
                            let mut automatic = CmgSchurPreconditioner::try_build_bounded(
                                &design, weights, 0, *degree, sampling, capacity, None, false,
                            )
                            .unwrap()
                            .preconditioner
                            .unwrap();
                            digest(&mut automatic, firms, capacity)
                        } else {
                            0
                        };
                        (explicit, automatic)
                    })
                    .unwrap();
                    actual.push((format!("{name}/t{threads}/c{capacity}"), value));
                }
            }
        }
        if record {
            for (key, (explicit, automatic)) in &actual {
                eprintln!("FROZEN|{key}|{explicit:#018x}|{automatic:#018x}");
            }
            return;
        }
        let expected: &[(&str, u64, u64)] = &[
            ("sampled0/t1/c1", 0x40beee6dc6bd41ea, 0x060991e685a537f7),
            ("sampled0/t1/c2", 0xec45ed054d93097e, 0x8c7a45f494247580),
            ("sampled0/t1/c5", 0xbdd14aaa7012574a, 0xf723baa93e6956aa),
            ("sampled0/t4/c1", 0x40beee6dc6bd41ea, 0x060991e685a537f7),
            ("sampled0/t4/c2", 0xec45ed054d93097e, 0x8c7a45f494247580),
            ("sampled0/t4/c5", 0xbdd14aaa7012574a, 0xf723baa93e6956aa),
            ("sampled1/t1/c1", 0x29c9481088727731, 0x6347d246a656dd64),
            ("sampled1/t1/c2", 0x494fdaf4e3d02c73, 0xe65943e3d1bf25c9),
            ("sampled1/t1/c5", 0x21c5d9da3ff35436, 0x7c2ae72dd1e86816),
            ("sampled1/t4/c1", 0x29c9481088727731, 0x6347d246a656dd64),
            ("sampled1/t4/c2", 0x494fdaf4e3d02c73, 0xe65943e3d1bf25c9),
            ("sampled1/t4/c5", 0x21c5d9da3ff35436, 0x7c2ae72dd1e86816),
            ("sampled2/t1/c1", 0x6fb9ace2e79bd4c3, 0x8f0e7d0e077df0a6),
            ("sampled2/t1/c2", 0x6932ef4e465f2ed0, 0xc12238b468600b42),
            ("sampled2/t1/c5", 0xc8903a83f6f56897, 0x55379ef6ef995eb7),
            ("sampled2/t4/c1", 0x6fb9ace2e79bd4c3, 0x8f0e7d0e077df0a6),
            ("sampled2/t4/c2", 0x6932ef4e465f2ed0, 0xc12238b468600b42),
            ("sampled2/t4/c5", 0xc8903a83f6f56897, 0x55379ef6ef995eb7),
            ("stars4/t1/c1", 0x932919e4d1a4d427, 0x47a450334a12f962),
            ("stars4/t1/c2", 0xbd13e8617dc315aa, 0x420cbbebb2cfa480),
            ("stars4/t1/c5", 0x4cd4a3454c9faa34, 0x106c59d795b112d4),
            ("stars4/t4/c1", 0x932919e4d1a4d427, 0x47a450334a12f962),
            ("stars4/t4/c2", 0xbd13e8617dc315aa, 0x420cbbebb2cfa480),
            ("stars4/t4/c5", 0x4cd4a3454c9faa34, 0x106c59d795b112d4),
            ("stars6/t1/c1", 0x1aaf4e774688e9e2, 0x4afb6812eb0140a7),
            ("stars6/t1/c2", 0x923bba1f8c902780, 0xcd52f75d7183bc26),
            ("stars6/t1/c5", 0x6a0e33f2e77d3451, 0x4f0ef63b17c49eb1),
            ("stars6/t4/c1", 0x1aaf4e774688e9e2, 0x4afb6812eb0140a7),
            ("stars6/t4/c2", 0x923bba1f8c902780, 0xcd52f75d7183bc26),
            ("stars6/t4/c5", 0x6a0e33f2e77d3451, 0x4f0ef63b17c49eb1),
            ("torus20/t1/c1", 0xef9541428232ec5e, 0x0000000000000000),
            ("torus20/t1/c2", 0xe531af690d09bb67, 0x0000000000000000),
            ("torus20/t1/c5", 0x49ffc6a279e28594, 0x0000000000000000),
            ("torus20/t4/c1", 0xef9541428232ec5e, 0x0000000000000000),
            ("torus20/t4/c2", 0xe531af690d09bb67, 0x0000000000000000),
            ("torus20/t4/c5", 0x49ffc6a279e28594, 0x0000000000000000),
            ("torus65/t1/c1", 0x90ff271e68e46c62, 0x0000000000000000),
            ("torus65/t1/c2", 0x50ac0449cd803c99, 0x0000000000000000),
            ("torus65/t1/c5", 0x5d9cbd673f7c76c1, 0x0000000000000000),
            ("torus65/t4/c1", 0xc8ee3adf7c3e89f3, 0x0000000000000000),
            ("torus65/t4/c2", 0x82248e5ca0b9e4f8, 0x0000000000000000),
            ("torus65/t4/c5", 0x25caff7f6ecf4d04, 0x0000000000000000),
        ];
        assert_eq!(actual.len(), expected.len());
        for ((key, (explicit, automatic)), (name, frozen_explicit, frozen_automatic)) in
            actual.iter().zip(expected)
        {
            assert_eq!(key, name);
            assert_eq!(explicit, frozen_explicit, "explicit {key}");
            assert_eq!(automatic, frozen_automatic, "automatic {key}");
        }
    }
}
