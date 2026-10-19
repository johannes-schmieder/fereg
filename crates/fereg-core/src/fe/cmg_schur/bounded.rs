//! Optional construction. Every loop, comparison and move is counted toward a
//! fixed cancellation cadence; memory is admitted before each allocation and a
//! memory refusal leaves all incumbent solver owners untouched.

use std::cmp::Ordering;

use cmg::CmgError;

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fe) enum BuildRefusal {
    MemoryBudget,
}

pub(in crate::fe) struct BoundedCmgBuild {
    pub(in crate::fe) preconditioner: Option<CmgSchurPreconditioner>,
    pub(in crate::fe) refusal: Option<BuildRefusal>,
    pub(in crate::fe) peak: SolverWorkspacePeak,
}

#[derive(Debug)]
pub(super) enum AttemptError {
    Refused(BuildRefusal),
    Fatal(FeregError),
}

impl From<FeregError> for AttemptError {
    fn from(error: FeregError) -> Self {
        Self::Fatal(error)
    }
}

pub(super) type Attempt<T> = std::result::Result<T, AttemptError>;

/// Counted primitive units between two cancellation polls.
const POLL_UNITS: u32 = 1 << 16;

/// Cancellation cadence for optional construction. Each bounded operation of
/// 1..=1024 conservative scalar arithmetic/access units is counted before it
/// runs; the host's sticky cancellation flag is polled whenever the running
/// count crosses `POLL_UNITS`, regardless of row or level boundaries, and
/// before every allocation. Work between polls is therefore bounded; allocator
/// and destructor latency are not. Units are a cadence, never an allowance.
pub(super) struct Work {
    pending: u32,
    polls: u64,
    executed: bool,
    break_builder: bool,
}

impl Work {
    pub(super) fn new(break_builder: bool) -> Self {
        Self {
            pending: 0,
            polls: 0,
            executed: false,
            break_builder,
        }
    }

    fn poll(&mut self) -> Attempt<()> {
        self.pending = 0;
        if self.break_builder && self.executed {
            // Developer builder Break: at least one counted operation has
            // already executed. Park here until the host requests cancellation.
            self.break_builder = false;
            crate::fe::routing_break::park_until_cancelled(self.polls);
        }
        self.polls += 1;
        Ok(crate::interrupt::checkpoint()?)
    }

    /// Count one bounded operation of 1..=1024 primitive units.
    #[inline(always)]
    pub(super) fn step(&mut self, units: u16) -> Attempt<()> {
        if units == 0 || units > 1024 {
            return Err(invalid_data("invalid bounded CMG work chunk").into());
        }
        self.executed = true;
        self.pending += u32::from(units);
        if self.pending >= POLL_UNITS {
            self.poll()
        } else {
            Ok(())
        }
    }

    /// Run one CMG construction stage on the borrowed host team, if any. CMG
    /// polls the same host cancellation flag. Cancellation always propagates as
    /// an interrupted estimation; only memory refusal is a candidate refusal.
    fn cmg<T>(
        &mut self,
        build: impl FnOnce(
            Option<&cmg::InstalledTeam<'_>>,
            cmg::Cancellation<'_>,
        ) -> std::result::Result<T, CmgError>,
    ) -> Attempt<T> {
        self.poll()?;
        let cancelled = crate::interrupt::cancellation_requested;
        let result = crate::parallel::with_cmg_execution(|team| build(team, &cancelled))
            .map_err(AttemptError::Fatal)?;
        result.map_err(|error| match error {
            CmgError::Cancelled { .. } => AttemptError::Fatal(FeregError::new(
                crate::error::ErrorCode::Interrupted,
                "bounded CMG construction was interrupted",
            )),
            CmgError::MemoryBudgetExceeded { .. } => {
                AttemptError::Refused(BuildRefusal::MemoryBudget)
            }
            other => AttemptError::Fatal(cmg_error(other)),
        })
    }

    fn capacity<T>(&mut self, count: usize) -> Attempt<Vec<T>> {
        self.step(32)?;
        bytes::<T>(count)?;
        self.poll()?;
        Ok(super::capacity(count)?)
    }

    pub(super) fn filled<T: Copy>(&mut self, count: usize, value: T) -> Attempt<Vec<T>> {
        let mut result = self.capacity(count)?;
        for _ in 0..count {
            self.step(8)?;
            result.push(value);
        }
        Ok(result)
    }

    // Unlike standard-library sorting, this sorter can return before the next
    // bounded group of comparisons/moves. Equal keys have identical value bits.
    pub(super) fn sort<T: Copy>(
        &mut self,
        values: &mut [T],
        compare: impl Fn(&T, &T) -> Ordering,
    ) -> Attempt<()> {
        fn sift<T: Copy>(
            work: &mut Work,
            values: &mut [T],
            mut root: usize,
            end: usize,
            compare: &impl Fn(&T, &T) -> Ordering,
        ) -> Attempt<()> {
            loop {
                work.step(64)?;
                let child = root * 2 + 1;
                if child >= end {
                    return Ok(());
                }
                let mut largest = child;
                if child + 1 < end && compare(&values[child], &values[child + 1]).is_lt() {
                    largest += 1;
                }
                if !compare(&values[root], &values[largest]).is_lt() {
                    return Ok(());
                }
                values.swap(root, largest);
                root = largest;
            }
        }
        self.step(16)?;
        for root in (0..values.len() / 2).rev() {
            sift(self, values, root, values.len(), &compare)?;
        }
        for end in (1..values.len()).rev() {
            self.step(32)?;
            values.swap(0, end);
            sift(self, values, 0, end, &compare)?;
        }
        Ok(())
    }
}

fn admit(
    memory: &mut Admission,
    work: &mut Work,
    parents: u128,
    additional: usize,
) -> Attempt<usize> {
    work.step(128)?;
    parents
        .checked_add(additional as u128)
        .ok_or_else(|| allocation("bounded CMG admission overflows"))?;
    if let Some(budget) = memory.budget {
        let available = budget
            .usable_bytes
            .checked_sub(budget.execution_bytes)
            .and_then(|bytes| bytes.checked_sub(parents));
        if available.is_none_or(|available| available < additional as u128) {
            return Err(AttemptError::Refused(BuildRefusal::MemoryBudget));
        }
    }
    Ok(memory.admit(parents, additional)?)
}

impl CmgSchurPreconditioner {
    /// Build one connected, aggregated bipartite CSR component for within's
    /// reduced-inverse adapter. The returned scale is the graph's global weight
    /// normalization: applying the inverse of the original Schur requires
    /// restoring the inverse scale around the fixed cycle.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::fe) fn try_build_reduced_bounded(
        n_eliminated: usize,
        n_retained: usize,
        row_ptr: &[u32],
        columns: &[u32],
        weights: &[f64],
        degree: usize,
        sampling: Sampling,
        budget: Option<FeSolverMemoryBudget>,
    ) -> Result<(BoundedCmgBuild, f64)> {
        sampling.validate()?;
        if n_retained == 0
            || n_eliminated == 0
            || !(4..=6).contains(&degree)
            || n_eliminated.checked_add(1) != Some(row_ptr.len())
            || row_ptr.first() != Some(&0)
            || row_ptr.last().copied().map(|n| n as usize) != Some(columns.len())
            || columns.len() != weights.len()
        {
            return Err(invalid_argument(
                "CMG reduced component CSR dimensions are inconsistent",
            ));
        }
        crate::interrupt::checkpoint()?;
        let mut work = Work::new(false);
        let mut memory = Admission {
            budget,
            peak: SolverWorkspacePeak::ZERO,
        };
        let mut weight_scale = 1.0;
        let attempt = (|| {
            for pair in row_ptr.windows(2) {
                work.step(16)?;
                if pair[0] >= pair[1] || pair[1] as usize > columns.len() {
                    return Err(AttemptError::Fatal(invalid_data(
                        "CMG reduced component CSR has an empty or invalid row",
                    )));
                }
            }
            let source = CellSource::Csr {
                retained: n_retained,
                row_ptr,
                columns,
                weights,
            };
            let (graph, components, scale, cache_bytes) =
                graph(source, degree, sampling, false, &mut work, &mut memory)?;
            weight_scale = scale;
            if components.sizes.len() != 1 {
                return Err(AttemptError::Fatal(invalid_data(
                    "CMG reduced component is disconnected",
                )));
            }
            finish(graph, components, cache_bytes, &mut work, &mut memory)
        })();
        let (preconditioner, refusal) = match attempt {
            Ok(candidate) => (Some(candidate), None),
            Err(AttemptError::Refused(reason)) => (None, Some(reason)),
            Err(AttemptError::Fatal(error)) => return Err(error),
        };
        Ok((
            BoundedCmgBuild {
                preconditioner,
                refusal,
                peak: memory.peak,
            },
            weight_scale,
        ))
    }

    /// Workspace-backed fixed reduced action. A factory may synchronize this
    /// owner to implement an immutable shared inverse without apply allocations.
    pub(in crate::fe) fn apply_reduced_in_place(
        &mut self,
        rhs: &mut [f64],
        scratch: &mut [f64],
        weight_scale: f64,
    ) -> Result<()> {
        if self.parallel.is_some()
            || rhs.len() != self.components.labels.len()
            || scratch.len() < rhs.len()
            || !weight_scale.is_finite()
            || weight_scale <= 0.0
        {
            return Err(invalid_argument(
                "CMG reduced application dimensions or scale are inconsistent",
            ));
        }
        // Normalize homogeneously before the fixed linear action, then restore
        // the full scale only at the end. Dividing rhs by the graph scale first
        // can destroy a tiny residual even when the final image is representable.
        let mut rhs_scale = 0.0_f64;
        for (row, &value) in rhs.iter().enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            if !value.is_finite() {
                return Err(invalid_data("CMG reduced residual is nonfinite"));
            }
            rhs_scale = rhs_scale.max(value.abs());
        }
        if rhs_scale == 0.0 {
            return Ok(());
        }
        for (row, value) in rhs.iter_mut().enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            let scaled = *value / rhs_scale;
            if *value != 0.0 && scaled == 0.0 {
                return Err(invalid_data(
                    "CMG reduced residual normalization underflows",
                ));
            }
            *value = scaled;
        }
        self.apply_batch(rhs, &mut scratch[..rhs.len()], 1, 1, None, None)?;
        for (row, (value, &image)) in rhs.iter_mut().zip(scratch.iter()).enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            *value = restore_scale(image, rhs_scale, weight_scale)?;
        }
        Ok(())
    }

    /// Candidate-only builder with admitted initial-graph stages. `budget`
    /// includes the caller's live incumbent owners in execution_bytes. No solver
    /// state is changed by a memory refusal. Cancellation and actual allocation
    /// or numerical failures remain fatal. The canonical census cache is used
    /// whenever its complete peak envelope is admitted. `break_builder` arms
    /// the developer builder-Break point (see `routing_break`).
    #[allow(clippy::too_many_arguments)]
    pub(in crate::fe) fn try_build_bounded(
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        left_effect: usize,
        degree: usize,
        sampling: Sampling,
        rhs_capacity: usize,
        budget: Option<FeSolverMemoryBudget>,
        break_builder: bool,
    ) -> Result<BoundedCmgBuild> {
        sampling.validate()?;
        if design.effects().len() != 2 || left_effect > 1 || rhs_capacity == 0 {
            return Err(invalid_argument(
                "CMG Schur requires two intercept effects and positive RHS capacity",
            ));
        }
        Self::build_observations_bounded(
            design.effects()[left_effect].level_index(),
            design.effects()[1 - left_effect].codes(),
            design.effects()[1 - left_effect].n_levels(),
            weights,
            degree,
            sampling,
            budget,
            true,
            break_builder,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_observations_bounded(
        membership: &LevelIndex,
        firms: &[u32],
        retained: usize,
        weights: BorrowedMapWeights<'_>,
        degree: usize,
        sampling: Sampling,
        budget: Option<FeSolverMemoryBudget>,
        cache_census: bool,
        break_builder: bool,
    ) -> Result<BoundedCmgBuild> {
        sampling.validate()?;
        if !(4..=6).contains(&degree)
            || firms.len() != membership.nobs()
            || matches!(weights, BorrowedMapWeights::Provided(w) if w.len() != firms.len())
        {
            return Err(invalid_argument(
                "CMG Schur observation source dimensions or cutoff are invalid",
            ));
        }
        crate::interrupt::checkpoint()?;
        let mut work = Work::new(break_builder);
        let mut memory = Admission {
            budget,
            peak: SolverWorkspacePeak::ZERO,
        };
        let source = CellSource::Observations {
            membership,
            firms,
            retained,
            weights,
        };
        let attempt = graph(
            source,
            degree,
            sampling,
            cache_census,
            &mut work,
            &mut memory,
        )
        .and_then(|(graph, components, _, cache_bytes)| {
            finish(graph, components, cache_bytes, &mut work, &mut memory)
        });
        let (preconditioner, refusal) = match attempt {
            Ok(preconditioner) => (Some(preconditioner), None),
            Err(AttemptError::Refused(reason)) => (None, Some(reason)),
            Err(AttemptError::Fatal(error)) => return Err(error),
        };
        Ok(BoundedCmgBuild {
            preconditioner,
            refusal,
            peak: memory.peak,
        })
    }
}

fn finish(
    graph: Laplacian,
    components: FirmComponents,
    cache_bytes: u128,
    work: &mut Work,
    memory: &mut Admission,
) -> Attempt<CmgSchurPreconditioner> {
    let options = CmgOptions::default();
    let experiment = component_options();
    work.step(1024)?;
    let bound = CmgPreconditioner::component_serial_construction_allocation_bound(
        graph.vertex_count(),
        graph.edge_count(),
        options,
        experiment,
    )
    .map_err(cmg_error)?;
    // The synchronous cache stays reserved even after graph emission drops it.
    let parents = graph_storage_bound(&graph)? + components.bytes() + cache_bytes;
    let admitted = admit(memory, work, parents, bound)?;
    // The terminal factor follows the hierarchy options and memory admission.
    let preconditioner = work.cmg(|_, cancelled| {
        CmgPreconditioner::build_component_experiment_serial_controlled(
            &graph, options, experiment, admitted, cancelled,
        )
    })?;
    // Keep the already-admitted conservative hierarchy bound. Exact storage
    // reporting recursively deduplicates allocation owners and is quadratic.
    // The workspace layout scan is O(levels) and the zeroed workspace is O(n).
    work.step(128)?;
    let parents = parents + bound as u128;
    let workspace_bytes = preconditioner
        .workspace_allocation_bytes()
        .map_err(|error| AttemptError::Fatal(cmg_error(error)))?;
    let bank_bytes = bytes::<[f64; 2]>(graph.vertex_count())?;
    let bound = workspace_bytes
        .checked_add(bank_bytes)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<CmgSchurPreconditioner>()))
        .ok_or_else(|| allocation("bounded CMG apply storage overflows"))?;
    admit(memory, work, parents, bound)?;
    work.poll()?;
    let scalar_workspace = preconditioner
        .try_workspace()
        .map_err(|error| AttemptError::Fatal(cmg_error(error)))?;
    let rhs = work.filled(graph.vertex_count(), 0.0)?;
    let solution = work.filled(graph.vertex_count(), 0.0)?;
    work.step(256)?;
    Ok(CmgSchurPreconditioner {
        preconditioner,
        parallel: None,
        scalar_workspace: Some(scalar_workspace),
        rhs,
        solution,
        components,
        batch_columns: 1,
        construction: memory.peak,
        retained_bound: Some(parents + bound as u128),
    })
}

// Evaluate value*numerator/denominator without overflowing or underflowing
// either intermediate ratio. Mantissas remain in [1/2,4); exact powers of two
// carry the exponent. A lost nonzero image is an error, never a zero inverse.
fn restore_scale(value: f64, numerator: f64, denominator: f64) -> Result<f64> {
    fn parts(mut value: f64) -> (f64, i32) {
        let mut correction = 0;
        if value < f64::MIN_POSITIVE {
            value *= 4_503_599_627_370_496.0;
            correction = -52;
        }
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023 + correction;
        (
            f64::from_bits((bits & ((1_u64 << 52) - 1)) | (1023_u64 << 52)),
            exponent,
        )
    }
    if value == 0.0 {
        return Ok(value);
    }
    if !value.is_finite() {
        return Err(invalid_data("CMG reduced image is nonfinite"));
    }
    let (v, ve) = parts(value.abs());
    let (n, ne) = parts(numerator);
    let (d, de) = parts(denominator);
    let mut result = v * n / d;
    let mut exponent = ve + ne - de;
    while exponent > 1023 {
        result *= f64::from_bits(2046_u64 << 52);
        exponent -= 1023;
    }
    while exponent < -1022 {
        result *= f64::MIN_POSITIVE;
        exponent += 1022;
    }
    result *= f64::from_bits(((exponent + 1023) as u64) << 52);
    result = result.copysign(value);
    if !result.is_finite() || result == 0.0 {
        return Err(invalid_data("CMG reduced image scaling is unrepresentable"));
    }
    Ok(result)
}

// Retained graph payload plus the pinned three Arc layouts. This is constant
// work and intentionally includes a few spare metadata bytes.
fn graph_storage_bound(graph: &Laplacian) -> Result<u128> {
    bytes::<[u64; 2]>(graph.edge_count())?
        .checked_add(bytes::<f64>(graph.vertex_count())?)
        .and_then(|bytes| bytes.checked_add(128))
        .map(|bytes| bytes as u128)
        .ok_or_else(|| allocation("bounded CMG graph storage overflows"))
}

fn find_root(labels: &mut [usize], mut vertex: usize, work: &mut Work) -> Attempt<usize> {
    loop {
        work.step(32)?;
        if labels[vertex] == vertex {
            return Ok(vertex);
        }
        labels[vertex] = labels[labels[vertex]];
        vertex = labels[vertex];
    }
}

#[derive(Clone, Copy)]
enum CellSource<'a> {
    Observations {
        membership: &'a LevelIndex,
        firms: &'a [u32],
        retained: usize,
        weights: BorrowedMapWeights<'a>,
    },
    Csr {
        retained: usize,
        row_ptr: &'a [u32],
        columns: &'a [u32],
        weights: &'a [f64],
    },
}

impl<'a> CellSource<'a> {
    fn worker_count(self) -> usize {
        match self {
            Self::Observations { membership, .. } => membership.n_levels(),
            Self::Csr { row_ptr, .. } => row_ptr.len() - 1,
        }
    }
    fn firm_count(self) -> usize {
        match self {
            Self::Observations { retained, .. } | Self::Csr { retained, .. } => retained,
        }
    }
    fn row_count(self, worker: usize) -> usize {
        match self {
            Self::Observations { membership, .. } => membership.members(worker).len(),
            Self::Csr { row_ptr, .. } => (row_ptr[worker + 1] - row_ptr[worker]) as usize,
        }
    }
    fn entry(self, worker: usize, position: usize) -> (u32, f64) {
        match self {
            Self::Observations {
                membership,
                firms,
                weights,
                ..
            } => {
                let row = membership.members(worker)[position];
                (
                    firms[row],
                    match weights {
                        BorrowedMapWeights::Unit => 1.0,
                        BorrowedMapWeights::Provided(values) => values[row],
                    },
                )
            }
            Self::Csr {
                row_ptr,
                columns,
                weights,
                ..
            } => {
                let row = row_ptr[worker] as usize + position;
                (columns[row], weights[row])
            }
        }
    }
    fn weights(self) -> BorrowedMapWeights<'a> {
        match self {
            Self::Observations { weights, .. } => weights,
            Self::Csr { weights, .. } => BorrowedMapWeights::Provided(weights),
        }
    }
}

fn component_options() -> ComponentBuildOptions {
    ComponentBuildOptions {
        prune_coarse_isolates: true,
        preserve_unpruned_stopping: true,
        factor_terminal_components: true,
    }
}

struct CanonicalCells {
    cells: Vec<(u32, f64)>,
    // One start offset and the original compensated total per worker. The next
    // start (or cells.len() for the last worker) is the current row's end.
    rows: Vec<(usize, f64)>,
}

// A fixed logical threshold, independent of the admitted physical lane count.
const HOSTED_EDGE_THRESHOLD: usize = 65_536;

struct EdgeEmissionPlan {
    suffix: Vec<f64>,
    chunk_len: usize,
}

impl EdgeEmissionPlan {
    #[allow(clippy::too_many_arguments)]
    fn select(
        workers: &BoundedCells<'_>,
        edges: usize,
        max_rows: usize,
        cutoff: usize,
        sampling: Sampling,
        work: &mut Work,
        memory: &mut Admission,
    ) -> Attempt<Option<Self>> {
        work.step(64)?;
        let Some(cache) = workers
            .cache
            .as_ref()
            .filter(|_| edges >= HOSTED_EDGE_THRESHOLD)
        else {
            return Ok(None);
        };
        work.step(1024)?;
        // This is one lookup for the entire emission, never one per row.
        if !work.cmg(|team, _| Ok(team.is_some()))? {
            return Ok(None);
        }
        let search_steps = usize::BITS - max_rows.saturating_sub(1).leading_zeros();
        // 192 covers metadata, draw permutation, clamp, weight and validation;
        // 16 per possible binary-search step and clique-index step covers all
        // bounded loop bodies. Include the cutoff even outside public 4..=6.
        // Workers poll cancellation before every chunk of at most 1024 units.
        let Some(edge_work) = cutoff
            .checked_mul(16)
            .and_then(|units| units.checked_add(192 + 16 * search_steps as usize))
        else {
            return Ok(None);
        };
        if edge_work > 1024 {
            return Ok(None);
        }
        let suffix_len = if sampling.samples > 0 {
            cache.cells.len().checked_add(cache.rows.len())
        } else {
            Some(0)
        };
        let Some(suffix_len) = suffix_len else {
            return Ok(None);
        };
        let Some(extra) = suffix_len.checked_mul(std::mem::size_of::<f64>()) else {
            return Ok(None);
        };
        // Select optional scratch against the complete conservative envelope,
        // before any row is reordered or any edge kernel executes.
        let Some(envelope) = cache_envelope(
            cache.cells.capacity(),
            cache.rows.len(),
            workers.source.firm_count(),
            max_rows,
            cutoff,
            sampling,
        ) else {
            return Ok(None);
        };
        let Some(required) = envelope.peak_bytes.checked_add(extra) else {
            return Ok(None);
        };
        if memory.budget.is_some_and(|budget| {
            budget
                .usable_bytes
                .checked_sub(budget.execution_bytes)
                .is_none_or(|available| available < required as u128)
        }) {
            return Ok(None);
        }
        admit(memory, work, envelope.peak_bytes as u128, extra)?;
        Ok(Some(Self {
            suffix: work.filled(suffix_len, 0.0)?,
            chunk_len: 1024 / edge_work,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn emit(
        &mut self,
        cache: &mut CanonicalCells,
        sampler: &mut Option<SampledTrees>,
        cutoff: usize,
        firms: usize,
        sampling: Sampling,
        edges: &mut Vec<(usize, usize, f64)>,
        work: &mut Work,
    ) -> Attempt<()> {
        let mut auxiliary = firms;
        for worker in 0..cache.rows.len() {
            work.step(64)?;
            let start = cache.rows[worker].0;
            let end = cache
                .rows
                .get(worker + 1)
                .map_or(cache.cells.len(), |row| row.0);
            let cells = &mut cache.cells[start..end];
            let count = cells.len();
            let edge_count = if count > cutoff {
                if let Some(sampler) = sampler {
                    // Preserve the original bounded sort and reverse Neumaier pass.
                    sampler.prepare_bounded(cells, worker, work)?;
                    for (output, &value) in self.suffix[start + worker..=end + worker]
                        .iter_mut()
                        .zip(sampler.suffix(count))
                    {
                        work.step(8)?;
                        *output = value;
                    }
                    (count - 1) * sampling.samples as usize
                } else {
                    count
                }
            } else {
                count * count.saturating_sub(1) / 2
            };
            // Reuse the required output bank as private task metadata. No task
            // vector or worker-by-maximum-degree scratch is needed. Auxiliary
            // ids are exact in f64: graph admission already bounds vertices by
            // u32::MAX + 1, so every emitted id is at most u32::MAX.
            for ordinal in 0..edge_count {
                work.step(16)?;
                edges.push((worker, ordinal, auxiliary as f64));
            }
            if count > cutoff && sampler.is_none() {
                auxiliary += 1;
            }
        }
        work.cmg(|team, cancelled| {
            let stage = cmg::ControlledStage::admit(edges.len(), self.chunk_len)?;
            stage.run(
                team.expect("emission plan retains the installed action"),
                edges,
                cancelled,
                |_, chunk| {
                    for edge in chunk {
                        let (worker, ordinal, auxiliary) = *edge;
                        let (start, total) = cache.rows[worker];
                        let end = cache
                            .rows
                            .get(worker + 1)
                            .map_or(cache.cells.len(), |row| row.0);
                        let cells = &cache.cells[start..end];
                        *edge = if cells.len() > cutoff && sampling.samples > 0 {
                            super::sampling::indexed_edge(
                                sampling,
                                worker,
                                cells,
                                &self.suffix[start + worker..=end + worker],
                                ordinal,
                            )
                        } else if cells.len() > cutoff {
                            (
                                cells[ordinal].0 as usize,
                                auxiliary as usize,
                                cells[ordinal].1,
                            )
                        } else {
                            let mut first = 0;
                            let mut remaining = ordinal;
                            while remaining >= cells.len() - first - 1 {
                                remaining -= cells.len() - first - 1;
                                first += 1;
                            }
                            let second = first + 1 + remaining;
                            let (left, left_weight) = cells[first];
                            let (right, right_weight) = cells[second];
                            (
                                left as usize,
                                right as usize,
                                left_weight.min(right_weight)
                                    * (left_weight.max(right_weight) / total),
                            )
                        };
                        if !edge.2.is_finite() || edge.2 <= 0.0 {
                            return Err(CmgError::InvalidEdgeWeight {
                                u: edge.0,
                                v: edge.1,
                                weight: edge.2,
                            });
                        }
                    }
                    Ok(())
                },
            )
        })
    }
}

impl CanonicalCells {
    fn new(observations: usize, workers: usize, work: &mut Work) -> Attempt<Self> {
        Ok(Self {
            cells: work.capacity(observations)?,
            rows: work.capacity(workers)?,
        })
    }

    fn store(&mut self, cells: &[(u32, f64)], total: f64, work: &mut Work) -> Attempt<()> {
        work.step(32)?;
        self.rows.push((self.cells.len(), total));
        for cell in cells {
            work.step(16)?;
            self.cells.push(*cell);
        }
        Ok(())
    }

    fn load(&self, worker: usize, cells: &mut Vec<(u32, f64)>, work: &mut Work) -> Attempt<f64> {
        work.step(32)?;
        let (start, total) = self.rows[worker];
        let end = self
            .rows
            .get(worker + 1)
            .map_or(self.cells.len(), |row| row.0);
        cells.clear();
        for cell in &self.cells[start..end] {
            work.step(16)?;
            cells.push(*cell);
        }
        Ok(total)
    }
}

struct CacheEnvelope {
    cache_bytes: usize,
    peak_bytes: usize,
}

/// Sum phase envelopes rather than reclaiming any stage. This dimension-only
/// proof runs before cache allocation; arithmetic/helper refusal selects the
/// original two-pass builder. It is a peak bound, never a controller allowance.
fn cache_envelope(
    observations: usize,
    workers: usize,
    firms: usize,
    max_rows: usize,
    cutoff: usize,
    sampling: Sampling,
) -> Option<CacheEnvelope> {
    fn bank<T>(count: usize) -> Option<usize> {
        count.checked_mul(std::mem::size_of::<T>())
    }
    // Sampled high-degree rows emit trees on firms; census retains auxiliary
    // worker vertices only when sampling is disabled.
    let vertices = if sampling.samples > 0 {
        firms
    } else {
        firms.checked_add(workers)?
    };
    if vertices == 0 || vertices as u128 > u32::MAX as u128 + 1 {
        return None;
    }
    let edges = cutoff
        .checked_sub(1)?
        .max(sampling.samples as usize)
        .max(1)
        .checked_mul(observations)?;
    let cache_bytes = bank::<(u32, f64)>(observations)?
        .checked_add(bank::<(usize, f64)>(workers)?)?
        .checked_add(std::mem::size_of::<CanonicalCells>())?;
    let sampler = if sampling.samples > 0 {
        SampledTrees::allocation_bytes(max_rows).ok()?
    } else {
        0
    };
    let options = CmgOptions::default();
    let hierarchy = CmgPreconditioner::component_serial_construction_allocation_bound(
        vertices,
        edges,
        options,
        component_options(),
    )
    .ok()?;
    let workspace = CmgPreconditioner::scalar_workspace_allocation_bound(vertices, options).ok()?;
    let owners = std::mem::size_of::<BoundedCells<'_>>()
        + std::mem::size_of::<SampledTrees>()
        + std::mem::size_of::<FirmComponents>()
        + std::mem::size_of::<Vec<(usize, usize, f64)>>()
        + std::mem::size_of::<Laplacian>()
        + std::mem::size_of::<CmgSchurPreconditioner>();
    let peak_bytes = [
        cache_bytes,
        bank::<(u32, f64)>(max_rows)?,
        sampler,
        bank::<[usize; 2]>(firms)?,
        bank::<[f64; 2]>(firms)?,
        bank::<(usize, usize, f64)>(edges)?,
        bank::<[u64; 4]>(edges)?
            .checked_add(bank::<f64>(vertices)?)?
            .checked_add(128)?,
        bank::<[u64; 2]>(edges)?
            .checked_add(bank::<f64>(vertices)?)?
            .checked_add(128)?,
        hierarchy,
        workspace,
        bank::<[f64; 2]>(vertices)?,
        owners,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)?;
    Some(CacheEnvelope {
        cache_bytes,
        peak_bytes,
    })
}

fn admitted_cache_bytes(
    source: CellSource<'_>,
    max_rows: usize,
    cutoff: usize,
    sampling: Sampling,
    budget: Option<FeSolverMemoryBudget>,
) -> Option<usize> {
    let CellSource::Observations { membership, .. } = source else {
        return None;
    };
    let envelope = cache_envelope(
        membership.nobs(),
        source.worker_count(),
        source.firm_count(),
        max_rows,
        cutoff,
        sampling,
    )?;
    let available = budget.map_or(Some(usize::MAX as u128), |budget| {
        budget.usable_bytes.checked_sub(budget.execution_bytes)
    })?;
    (envelope.peak_bytes as u128 <= available).then_some(envelope.cache_bytes)
}

struct BoundedCells<'a> {
    source: CellSource<'a>,
    weight_scale: f64,
    cells: Vec<(u32, f64)>,
    cache: Option<CanonicalCells>,
}

impl BoundedCells<'_> {
    fn load_bounded(&mut self, worker: usize, work: &mut Work) -> Attempt<f64> {
        work.step(16)?;
        self.cells.clear();
        for position in 0..self.source.row_count(worker) {
            work.step(64)?;
            let (firm, raw_weight) = self.source.entry(worker, position);
            let weight = raw_weight / self.weight_scale;
            if firm as usize >= self.source.firm_count() {
                return Err(invalid_data("CMG Schur firm code is out of bounds").into());
            }
            if !weight.is_finite() || weight <= 0.0 {
                return Err(invalid_data("CMG Schur requires positive finite weights").into());
            }
            self.cells.push((firm, weight));
        }
        work.sort(&mut self.cells, |left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
        })?;
        let mut read = 0;
        let mut write = 0;
        let mut total = 0.0;
        let mut total_correction = 0.0;
        while read < self.cells.len() {
            work.step(32)?;
            let firm = self.cells[read].0;
            let mut sum = 0.0;
            let mut correction = 0.0;
            while read < self.cells.len() && self.cells[read].0 == firm {
                work.step(32)?;
                neumaier_add(self.cells[read].1, &mut sum, &mut correction);
                read += 1;
            }
            work.step(32)?;
            let weight = sum + correction;
            self.cells[write] = (firm, weight);
            neumaier_add(weight, &mut total, &mut total_correction);
            write += 1;
        }
        self.cells.truncate(write);
        let total = total + total_correction;
        if !total.is_finite() || total <= 0.0 {
            return Err(invalid_data("CMG Schur worker weight total is invalid").into());
        }
        Ok(total)
    }

    fn census_bounded(
        &mut self,
        cutoff: usize,
        sampling: Sampling,
        labels: &mut [usize],
        work: &mut Work,
    ) -> Attempt<GraphCounts> {
        let mut counts = GraphCounts::default();
        for worker in 0..self.source.worker_count() {
            let total = self.load_bounded(worker, work)?;
            if let Some(cache) = &mut self.cache {
                cache.store(&self.cells, total, work)?;
            }
            work.step(64)?;
            let count = self.cells.len();
            if let Some(&(first, _)) = self.cells.first() {
                for &(firm, _) in self.cells.iter().skip(1) {
                    work.step(16)?;
                    let first = find_root(labels, first as usize, work)?;
                    let other = find_root(labels, firm as usize, work)?;
                    labels[first.max(other)] = first.min(other);
                }
            }
            work.step(128)?;
            let edges = if count > cutoff && sampling.samples > 0 {
                counts.sampled_workers = counts
                    .sampled_workers
                    .checked_add(1)
                    .ok_or_else(|| allocation("CMG sampled worker count overflows"))?;
                (count - 1)
                    .checked_mul(sampling.samples as usize)
                    .ok_or_else(|| allocation("CMG sampled worker edge count overflows"))?
            } else if count > cutoff {
                counts.retained_workers = counts
                    .retained_workers
                    .checked_add(1)
                    .ok_or_else(|| allocation("CMG Schur auxiliary count overflows"))?;
                count
            } else {
                count * count.saturating_sub(1) / 2
            };
            counts.input_edges = counts
                .input_edges
                .checked_add(edges)
                .ok_or_else(|| allocation("CMG Schur edge count overflows"))?;
        }
        Ok(counts)
    }

    /// Admit the output bank and select one complete emission path before
    /// returning private raw edges. Failed stages never fall through to a retry.
    #[allow(clippy::too_many_arguments)]
    fn collect_edges_bounded(
        &mut self,
        sampler: &mut Option<SampledTrees>,
        input_edges: usize,
        max_rows: usize,
        cutoff: usize,
        sampling: Sampling,
        scratch: u128,
        work: &mut Work,
        memory: &mut Admission,
    ) -> Attempt<Vec<(usize, usize, f64)>> {
        let edge_bytes = bytes::<(usize, usize, f64)>(input_edges)?;
        admit(memory, work, scratch, edge_bytes)?;
        let mut edges = work.capacity(input_edges)?;
        if let Some(mut emission) =
            EdgeEmissionPlan::select(self, input_edges, max_rows, cutoff, sampling, work, memory)?
        {
            emission.emit(
                self.cache.as_mut().expect("admitted canonical rows"),
                sampler,
                cutoff,
                self.source.firm_count(),
                sampling,
                &mut edges,
                work,
            )?;
        } else {
            self.emit_edges_bounded(sampler, cutoff, &mut edges, work)?;
        }
        work.step(128)?;
        if edges.len() != input_edges {
            return Err(invalid_data("CMG Schur edge census changed during construction").into());
        }
        Ok(edges)
    }

    fn emit_edges_bounded(
        &mut self,
        sampler: &mut Option<SampledTrees>,
        cutoff: usize,
        edges: &mut Vec<(usize, usize, f64)>,
        work: &mut Work,
    ) -> Attempt<()> {
        let mut auxiliary = self.source.firm_count();
        for worker in 0..self.source.worker_count() {
            let total = if let Some(cache) = &self.cache {
                // Sampling sorts this scratch by weight; the retained canonical
                // row and its compensated total remain immutable.
                cache.load(worker, &mut self.cells, work)?
            } else {
                self.load_bounded(worker, work)?
            };
            if self.cells.len() > cutoff {
                if let Some(sampler) = sampler.as_mut() {
                    sampler.prepare_bounded(&mut self.cells, worker, work)?;
                    while let Some(edge) = sampler.next_bounded(&self.cells, work)? {
                        work.step(8)?;
                        edges.push(edge);
                    }
                } else {
                    for &(firm, weight) in &self.cells {
                        work.step(16)?;
                        edges.push((firm as usize, auxiliary, weight));
                    }
                    auxiliary += 1;
                }
            } else {
                for first in 0..self.cells.len() {
                    for second in first + 1..self.cells.len() {
                        work.step(64)?;
                        let (left, left_weight) = self.cells[first];
                        let (right, right_weight) = self.cells[second];
                        let weight =
                            left_weight.min(right_weight) * (left_weight.max(right_weight) / total);
                        if !weight.is_finite() || weight <= 0.0 {
                            return Err(invalid_data(
                                "CMG Schur clique weight is nonpositive or nonfinite",
                            )
                            .into());
                        }
                        edges.push((left as usize, right as usize, weight));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Borrow the installed team only for initial graph construction; hierarchy
/// construction retains its separate serial admission and execution policy.
fn initial_laplacian_bounded(
    vertices: usize,
    edges: &[(usize, usize, f64)],
    admitted: usize,
    work: &mut Work,
) -> Attempt<Laplacian> {
    work.cmg(|team, cancelled| {
        if let Some(team) = team {
            Laplacian::from_edges_hosted_controlled(vertices, edges, admitted, team, cancelled)
        } else {
            Laplacian::from_edges_controlled(vertices, edges, admitted, cancelled)
        }
    })
}

fn graph(
    source: CellSource<'_>,
    cutoff: usize,
    sampling: Sampling,
    cache_census: bool,
    work: &mut Work,
    memory: &mut Admission,
) -> Attempt<(Laplacian, FirmComponents, f64, u128)> {
    work.step(64)?;
    let firms = source.firm_count();
    let weights = source.weights();
    let mut weight_scale = 1.0_f64;
    if let BorrowedMapWeights::Provided(values) = weights {
        weight_scale = 0.0;
        for &weight in values {
            work.step(16)?;
            if !weight.is_finite() || weight <= 0.0 {
                return Err(invalid_data("CMG Schur requires positive finite weights").into());
            }
            weight_scale = weight_scale.max(weight);
        }
    }
    let mut max_rows = 0;
    for worker in 0..source.worker_count() {
        work.step(16)?;
        max_rows = max_rows.max(source.row_count(worker));
    }
    work.step(128)?;
    let cache_bytes = if cache_census {
        // Dimension-only envelope check before any cache allocation.
        work.step(1024)?;
        admitted_cache_bytes(source, max_rows, cutoff, sampling, memory.budget).unwrap_or(0)
    } else {
        0
    };
    let sampler_bytes = if sampling.samples > 0 {
        SampledTrees::allocation_bytes(max_rows)?
    } else {
        0
    };
    let bound = bytes::<(u32, f64)>(max_rows)?
        .checked_add(sampler_bytes)
        .and_then(|total| total.checked_add(bytes::<[usize; 2]>(firms).ok()?))
        .and_then(|total| total.checked_add(cache_bytes))
        .ok_or_else(|| allocation("bounded CMG topology scratch overflows"))?;
    admit(memory, work, 0, bound)?;
    let mut workers = BoundedCells {
        source,
        weight_scale,
        cells: work.capacity(max_rows)?,
        cache: if cache_bytes > 0 {
            let CellSource::Observations { membership, .. } = source else {
                return Err(invalid_data("CMG canonical cache requires observations").into());
            };
            Some(CanonicalCells::new(
                membership.nobs(),
                source.worker_count(),
                work,
            )?)
        } else {
            None
        },
    };
    let mut sampler = if sampling.samples > 0 {
        Some(SampledTrees::new_bounded(sampling, max_rows, work)?)
    } else {
        None
    };
    let mut labels = work.capacity(firms)?;
    for firm in 0..firms {
        work.step(8)?;
        labels.push(firm);
    }
    let mut sizes = work.filled(firms, usize::MAX)?;
    let counts = workers.census_bounded(cutoff, sampling, &mut labels, work)?;
    for firm in 0..firms {
        work.step(8)?;
        labels[firm] = find_root(&mut labels, firm, work)?;
    }
    let mut component_count = 0;
    for label in &mut labels {
        work.step(32)?;
        if sizes[*label] == usize::MAX {
            sizes[*label] = component_count;
            component_count += 1;
        }
        *label = sizes[*label];
    }
    sizes.truncate(component_count);
    for size in &mut sizes {
        work.step(8)?;
        *size = 0;
    }
    for &label in &labels {
        work.step(8)?;
        sizes[label] += 1;
    }
    work.step(128)?;
    let vertices = firms
        .checked_add(counts.retained_workers)
        .filter(|vertices| *vertices > 0 && *vertices as u128 <= u32::MAX as u128 + 1)
        .ok_or_else(|| allocation("CMG Schur graph vertex count is not supported"))?;
    let scratch = vec_capacity_bytes(&workers.cells)
        + cache_bytes as u128
        + sampler.as_ref().map_or(0, SampledTrees::bytes)
        + vec_capacity_bytes(&labels)
        + vec_capacity_bytes(&sizes);
    let edges = workers.collect_edges_bounded(
        &mut sampler,
        counts.input_edges,
        max_rows,
        cutoff,
        sampling,
        scratch,
        work,
        memory,
    )?;
    let graph_bound = bytes::<[u64; 4]>(counts.input_edges)?
        .checked_add(bytes::<f64>(vertices)?)
        .and_then(|bytes| bytes.checked_add(128))
        .ok_or_else(|| allocation("CMG Schur graph construction overflows"))?;
    let admitted = admit(
        memory,
        work,
        scratch + vec_capacity_bytes(&edges),
        graph_bound,
    )?;
    let graph = initial_laplacian_bounded(vertices, &edges, admitted, work)?;
    drop(edges);
    drop(workers);
    drop(sampler);
    work.step(256)?;
    let parents = graph_storage_bound(&graph)?
        + vec_capacity_bytes(&labels)
        + vec_capacity_bytes(&sizes)
        + cache_bytes as u128;
    admit(memory, work, parents, bytes::<[f64; 2]>(component_count)?)?;
    let components = FirmComponents {
        labels,
        sizes,
        sums: work.filled(component_count, 0.0)?,
        corrections: work.filled(component_count, 0.0)?,
    };
    Ok((graph, components, weight_scale, cache_bytes as u128))
}

#[cfg(test)]
mod control_tests {
    use super::*;

    fn emission_fixture(rows: usize) -> (Vec<u32>, Vec<u32>, Vec<f64>) {
        let mut pointers = vec![0];
        let mut columns = Vec::new();
        let mut weights = Vec::new();
        for worker in 0..rows {
            let count = [3, 5, 6, 17][worker % 4];
            for firm in (0..count).rev() {
                // Ties, duplicate cells, wide weights, and two components.
                for weight in [1.0, [1.0, 1.0e-8, 1.0e8][firm % 3]] {
                    columns.push((firm + 19 * (worker % 2)) as u32);
                    weights.push(weight);
                }
            }
            pointers.push(columns.len() as u32);
        }
        (pointers, columns, weights)
    }

    fn bits(values: &[f64]) -> Vec<u64> {
        values.iter().map(|x| x.to_bits()).collect()
    }

    #[test]
    fn hosted_emission_matches_serial_raw_graph_hierarchy_and_cycle_bits() {
        use crate::{options::ThreadConfig, parallel::ThreadTeam};
        let (pointers, columns, weights) = emission_fixture(8192);
        for samples in [0, 1, 3] {
            let sampling = Sampling { samples, seed: 0 };
            let run = |lanes: Option<usize>| {
                let operation = || {
                    let mut work = Work::new(false);
                    let mut memory = Admission {
                        budget: None,
                        peak: SolverWorkspacePeak::ZERO,
                    };
                    let source = CellSource::Csr {
                        retained: 38,
                        row_ptr: &pointers,
                        columns: &columns,
                        weights: &weights,
                    };
                    let mut cells = BoundedCells {
                        source,
                        weight_scale: 1.0,
                        cells: Vec::with_capacity(34),
                        cache: Some(CanonicalCells::new(columns.len(), 8192, &mut work).unwrap()),
                    };
                    let mut labels: Vec<_> = (0..38).collect();
                    let count = cells
                        .census_bounded(5, sampling, &mut labels, &mut work)
                        .unwrap();
                    assert!(count.input_edges >= HOSTED_EDGE_THRESHOLD);
                    let mut sampler = if samples > 0 {
                        Some(SampledTrees::new_bounded(sampling, 34, &mut work).unwrap())
                    } else {
                        None
                    };
                    let mut edges = Vec::with_capacity(count.input_edges);
                    let plan = EdgeEmissionPlan::select(
                        &cells,
                        count.input_edges,
                        34,
                        5,
                        sampling,
                        &mut work,
                        &mut memory,
                    )
                    .unwrap();
                    assert_eq!(plan.is_some(), lanes.is_some());
                    if let Some(mut plan) = plan {
                        plan.emit(
                            cells.cache.as_mut().unwrap(),
                            &mut sampler,
                            5,
                            38,
                            sampling,
                            &mut edges,
                            &mut work,
                        )
                        .unwrap();
                    } else {
                        cells
                            .emit_edges_bounded(&mut sampler, 5, &mut edges, &mut work)
                            .unwrap();
                    }
                    let vertices = 38 + count.retained_workers;
                    let graph =
                        initial_laplacian_bounded(vertices, &edges, usize::MAX, &mut work).unwrap();
                    (edges, count.retained_workers, work.polls, graph)
                };
                match lanes {
                    Some(lanes) => {
                        ThreadTeam::new(ThreadConfig::from_requested(lanes, true).unwrap())
                            .install(operation)
                            .unwrap()
                    }
                    None => operation(),
                }
            };
            let (expected, retained, _, reference_graph) = run(None);
            let expected_bits: Vec<_> = expected
                .iter()
                .map(|&(a, b, w)| (a, b, w.to_bits()))
                .collect();
            let mut schedule = None;
            for lanes in [1, 2, 4, 4] {
                let (edges, actual_retained, polls, graph) = run(Some(lanes));
                assert_eq!(actual_retained, retained);
                // fereg's own poll schedule is independent of the worker count.
                assert_eq!(polls, *schedule.get_or_insert(polls));
                assert_eq!(
                    edges
                        .iter()
                        .map(|&(a, b, w)| (a, b, w.to_bits()))
                        .collect::<Vec<_>>(),
                    expected_bits
                );
                assert_eq!(graph.edges(), reference_graph.edges());
                assert_eq!(bits(graph.diagonal()), bits(reference_graph.diagonal()));
                if samples > 0 {
                    let never = || false;
                    let build = |graph: &Laplacian| {
                        CmgPreconditioner::build_component_experiment_serial_controlled(
                            graph,
                            CmgOptions::default(),
                            component_options(),
                            usize::MAX,
                            &never,
                        )
                        .unwrap()
                    };
                    let reference = build(&reference_graph);
                    let actual = build(&graph);
                    assert_eq!(actual.hierarchy(), reference.hierarchy());
                    let mut rhs = vec![0.0; 38];
                    rhs[0] = 1.0;
                    rhs[1] = -1.0;
                    assert_eq!(
                        bits(&actual.apply(&rhs).unwrap()),
                        bits(&reference.apply(&rhs).unwrap())
                    );
                }
            }
        }
    }

    #[test]
    fn hosted_emission_scratch_fallback_precedes_kernel_work() {
        use crate::{options::ThreadConfig, parallel::ThreadTeam};
        let (pointers, columns, weights) = emission_fixture(8192);
        let sampling = Sampling {
            samples: 1,
            seed: 0,
        };
        for lanes in [1, 2, 4] {
            ThreadTeam::new(ThreadConfig::from_requested(lanes, true).unwrap())
                .install(|| {
                    let mut work = Work::new(false);
                    let mut memory = Admission {
                        budget: None,
                        peak: SolverWorkspacePeak::ZERO,
                    };
                    let mut cells = BoundedCells {
                        source: CellSource::Csr {
                            retained: 38,
                            row_ptr: &pointers,
                            columns: &columns,
                            weights: &weights,
                        },
                        weight_scale: 1.0,
                        cells: Vec::with_capacity(34),
                        cache: Some(CanonicalCells::new(columns.len(), 8192, &mut work).unwrap()),
                    };
                    let mut labels: Vec<_> = (0..38).collect();
                    let counts = cells
                        .census_bounded(5, sampling, &mut labels, &mut work)
                        .unwrap();
                    let canonical = cells.cache.as_ref().unwrap().cells.clone();
                    assert!(EdgeEmissionPlan::select(
                        &cells,
                        HOSTED_EDGE_THRESHOLD - 1,
                        34,
                        5,
                        sampling,
                        &mut work,
                        &mut memory
                    )
                    .unwrap()
                    .is_none());
                    let envelope =
                        cache_envelope(columns.len(), 8192, 38, 34, 5, sampling).unwrap();
                    memory.budget = Some(FeSolverMemoryBudget {
                        usable_bytes: envelope.peak_bytes as u128,
                        execution_bytes: 0,
                    });
                    assert!(EdgeEmissionPlan::select(
                        &cells,
                        counts.input_edges,
                        34,
                        5,
                        sampling,
                        &mut work,
                        &mut memory
                    )
                    .unwrap()
                    .is_none());
                    assert_eq!(cells.cache.as_ref().unwrap().cells, canonical);
                })
                .unwrap();
        }
    }

    #[test]
    fn observation_graph_uses_hosted_stages_at_every_worker_count() {
        use crate::{options::ThreadConfig, parallel::ThreadTeam};
        for rows in [8, 8192] {
            let (pointers, columns, weights) = emission_fixture(rows);
            let codes: Vec<_> = pointers
                .windows(2)
                .enumerate()
                .flat_map(|(worker, pair)| {
                    std::iter::repeat(worker as u32).take((pair[1] - pair[0]) as usize)
                })
                .collect();
            let membership = LevelIndex::from_codes(&codes, rows).unwrap();
            let run = |lanes: Option<usize>| {
                let operation = || {
                    crate::parallel::with_cmg_execution(|team| {
                        assert_eq!(team.map(|team| team.worker_ceiling()), lanes);
                    })
                    .unwrap();
                    let mut work = Work::new(false);
                    let mut memory = Admission {
                        budget: None,
                        peak: SolverWorkspacePeak::ZERO,
                    };
                    let (graph, components, scale, cache) = graph(
                        CellSource::Observations {
                            membership: &membership,
                            firms: &columns,
                            retained: 38,
                            weights: BorrowedMapWeights::Provided(&weights),
                        },
                        5,
                        Sampling {
                            samples: 1,
                            seed: 0,
                        },
                        true,
                        &mut work,
                        &mut memory,
                    )
                    .unwrap();
                    (
                        (
                            graph
                                .edges()
                                .iter()
                                .map(|edge| (edge.u(), edge.v(), edge.weight().to_bits()))
                                .collect::<Vec<_>>(),
                            bits(graph.diagonal()),
                            components.labels,
                            components.sizes,
                            scale.to_bits(),
                            cache,
                        ),
                        work.polls,
                    )
                };
                if let Some(lanes) = lanes {
                    ThreadTeam::new(ThreadConfig::from_requested(lanes, true).unwrap())
                        .install(operation)
                        .unwrap()
                } else {
                    operation()
                }
            };
            let (expected, _) = run(None);
            // The hosted emission path has its own poll schedule; it is
            // independent of the admitted worker count.
            let mut schedule = None;
            for lanes in [1, 2, 4] {
                let (actual, polls) = run(Some(lanes));
                assert_eq!(actual, expected);
                assert_eq!(polls, *schedule.get_or_insert(polls));
            }
        }
    }

    #[test]
    fn host_stage_dispatch_is_identical_at_every_worker_count() {
        use crate::{options::ThreadConfig, parallel::ThreadTeam};
        for workers in [1, 4] {
            ThreadTeam::new(ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    let mut work = Work::new(false);
                    let mut output = [0_u32; 16]; // Already owned/admitted test storage.
                    work.cmg(|team, cancelled| {
                        cmg::ControlledStage::admit(16, 4)?.run(
                            team.unwrap(),
                            &mut output,
                            cancelled,
                            |index, chunk| {
                                chunk.fill(index as u32 + 1);
                                Ok(())
                            },
                        )
                    })
                    .unwrap();
                    assert_eq!(output, [1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4]);
                    assert_eq!(work.polls, 1);
                })
                .unwrap();
        }
    }

    fn candidate() -> Result<(BoundedCmgBuild, f64)> {
        CmgSchurPreconditioner::try_build_reduced_bounded(
            2,
            2,
            &[0, 2, 4],
            &[0, 1, 0, 1],
            &[2.0, 3.0, 2.0, 3.0],
            5,
            Sampling::default(),
            None,
        )
    }

    #[test]
    fn controlled_hierarchy_and_workspace_outlive_individual_installations() {
        use crate::{options::ThreadConfig, parallel::ThreadTeam};
        let make = |workers| {
            ThreadTeam::new(ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    let (result, scale) = candidate().unwrap();
                    (result.preconditioner.unwrap(), scale)
                })
                .unwrap()
        };
        let (mut one, scale) = make(1);
        let (four, other_scale) = make(4);
        assert_eq!(scale, other_scale);
        assert_eq!(
            one.preconditioner.hierarchy(),
            four.preconditioner.hierarchy()
        );
        let mut expected = None;
        for workers in [4, 1, 4] {
            let mut rhs = [1.0, -1.0];
            ThreadTeam::new(ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    one.apply_reduced_in_place(&mut rhs, &mut [0.0; 2], scale)
                        .unwrap();
                })
                .unwrap();
            let bits = rhs.map(f64::to_bits);
            assert_eq!(bits, *expected.get_or_insert(bits));
        }
    }

    #[test]
    fn terminal_factor_follows_the_hierarchy_options_at_every_worker_count() {
        use crate::{options::ThreadConfig, parallel::ThreadTeam};
        let graph = Laplacian::from_edges(2, [(0, 1, 1.0)]).unwrap();
        let run = |workers| {
            ThreadTeam::new(ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    let mut work = Work::new(false);
                    let candidate = work
                        .cmg(|_, cancelled| {
                            CmgPreconditioner::build_component_experiment_serial_controlled(
                                &graph,
                                CmgOptions::default(),
                                component_options(),
                                usize::MAX,
                                cancelled,
                            )
                        })
                        .unwrap_or_else(|_| panic!("small graph must complete"));
                    (candidate, work.polls)
                })
                .unwrap()
        };
        let (one, polls) = run(1);
        let (four, other_polls) = run(4);
        assert_eq!(polls, other_polls);
        assert_eq!(one.hierarchy(), four.hierarchy());
        for candidate in [one, four] {
            // No work allowance can redirect a direct terminal to coarsening.
            assert!(candidate.terminal_factor().is_some());
            assert_eq!(
                candidate.hierarchy().report().terminal_reason(),
                cmg::TerminalReason::Direct
            );
            // Analytic inverse of [[1,-1],[-1,1]] on the quotient space.
            let image = candidate.apply(&[1.0, -1.0]).unwrap();
            assert!((image[0] - image[1] - 1.0).abs() < 1e-12);
            assert!((image[1] - image[0] + 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn steps_validate_chunks_and_poll_at_the_fixed_cadence() {
        let mut work = Work::new(false);
        for units in [0, 1025] {
            assert!(matches!(work.step(units), Err(AttemptError::Fatal(error))
                if error.code == crate::error::ErrorCode::InvalidData
                    && error.message == "invalid bounded CMG work chunk"));
        }
        for _ in 0..(POLL_UNITS / 1024 - 1) {
            work.step(1024).unwrap();
        }
        assert_eq!(work.polls, 0);
        work.step(1024).unwrap();
        assert_eq!((work.polls, work.pending), (1, 0));
        // Every allocation polls first, independently of the counted cadence.
        let bank: Vec<u8> = work.capacity(3).unwrap();
        assert_eq!((bank.capacity(), work.polls), (3, 2));
        // An identical candidate repeats its complete poll schedule.
        let (first, _) = candidate().unwrap();
        assert!(first.preconditioner.is_some() && first.refusal.is_none());
    }

    #[test]
    fn cancellation_and_builder_break_propagate_as_interruption_isolated() {
        const CHILD: &str = "FEREG_CMG_BOUNDED_CANCELLATION_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD, "1")
                .args([
                    "fe::cmg_schur::bounded::control_tests::cancellation_and_builder_break_propagate_as_interruption_isolated",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let interrupted = |result: Attempt<()>| {
            matches!(result, Err(AttemptError::Fatal(error))
                if error.code == crate::error::ErrorCode::Interrupted)
        };
        crate::interrupt::request_cancellation();
        // A counted step below the cadence does not poll; the next poll does.
        let mut work = Work::new(false);
        assert!(work.step(1).is_ok());
        assert!(interrupted(work.capacity::<u8>(1).map(drop)));
        assert!(interrupted(work.cmg(|_, _| Ok(()))));
        // CMG's own stages observe the same flag and never become a refusal.
        let mut work = Work::new(false);
        work.pending = POLL_UNITS - 1;
        crate::interrupt::reset_cancellation();
        let graph = Laplacian::from_edges(3, [(0, 1, 1.0), (1, 2, 2.0)]).unwrap();
        let result = work.cmg(|_, _| {
            crate::interrupt::request_cancellation();
            CmgPreconditioner::build_component_experiment_serial_controlled(
                &graph,
                CmgOptions::default(),
                component_options(),
                usize::MAX,
                &crate::interrupt::cancellation_requested,
            )
        });
        assert!(interrupted(result.map(drop)));
        assert_eq!(
            candidate().err().map(|error| error.code),
            Some(crate::error::ErrorCode::Interrupted)
        );
        crate::interrupt::reset_cancellation();

        // Developer builder Break: park after executed work, then interrupt.
        let guard = crate::fe::routing_break::DeveloperBreak::claim().unwrap();
        let handle = std::thread::spawn(|| {
            while !crate::fe::routing_builder_break_ready() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            crate::interrupt::request_cancellation();
        });
        let mut work = Work::new(true);
        assert!(work.poll().is_ok(), "no work has executed yet");
        assert!(!crate::fe::routing_builder_break_ready());
        work.step(16).unwrap();
        assert!(interrupted(work.poll()));
        handle.join().unwrap();
        assert!(crate::fe::routing_builder_break_ready());
        drop(guard);
        assert!(!crate::fe::routing_builder_break_ready());
        crate::interrupt::reset_cancellation();
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use crate::fe::BorrowedInterceptEffect;

    fn completed<T>(value: Attempt<T>) -> T {
        match value {
            Ok(value) => value,
            Err(AttemptError::Refused(reason)) => panic!("unexpected refusal: {reason:?}"),
            Err(AttemptError::Fatal(error)) => panic!("unexpected failure: {error}"),
        }
    }

    fn assert_same_build(actual: BoundedCmgBuild, expected: BoundedCmgBuild) {
        assert_eq!(actual.peak, expected.peak);
        assert_eq!(actual.refusal, expected.refusal);
        match (actual.preconditioner, expected.preconditioner) {
            (Some(mut actual), Some(mut expected)) => {
                let left = actual.preconditioner.hierarchy();
                let right = expected.preconditioner.hierarchy();
                assert_eq!(left, right);
                for (left, right) in left.levels().iter().zip(right.levels()) {
                    for (a, b) in left.graph().diagonal().iter().zip(right.graph().diagonal()) {
                        assert_eq!(a.to_bits(), b.to_bits());
                    }
                    for (a, b) in left.graph().edges().iter().zip(right.graph().edges()) {
                        assert_eq!(a.weight().to_bits(), b.weight().to_bits());
                    }
                }
                assert_eq!(actual.components.labels, expected.components.labels);
                assert_eq!(actual.components.sizes, expected.components.sizes);
                assert_eq!(
                    actual.retained_bytes().unwrap(),
                    expected.retained_bytes().unwrap()
                );
                let n = actual.components.labels.len();
                let rhs: Vec<_> = (0..2 * n).map(|i| (i % 7) as f64 - 3.0).collect();
                let mut a = vec![0.0; rhs.len()];
                let mut b = vec![0.0; rhs.len()];
                actual.apply_batch(&rhs, &mut a, 2, 2, None, None).unwrap();
                expected
                    .apply_batch(&rhs, &mut b, 2, 2, None, None)
                    .unwrap();
                for (a, b) in a.iter().zip(&b) {
                    assert_eq!(a.to_bits(), b.to_bits());
                }
            }
            (None, None) => {}
            _ => panic!("repeated construction changed its success"),
        }
    }

    struct Fixture {
        workers: Vec<u32>,
        firms: Vec<u32>,
        weights: Vec<f64>,
        membership: LevelIndex,
        firm_index: LevelIndex,
    }

    impl Fixture {
        fn new() -> Self {
            let mut workers = Vec::new();
            let mut firms = Vec::new();
            let mut weights = Vec::new();
            let mut first = 0;
            for (worker, degree) in [1, 2, 3, 6, 9].into_iter().enumerate() {
                for firm in 0..degree {
                    for duplicate in 0..8 {
                        workers.push(worker as u32);
                        firms.push(first + firm);
                        weights.push(if duplicate == 0 {
                            1.0e12
                        } else {
                            (firm + 1) as f64
                        });
                    }
                }
                first += degree;
            }
            Self {
                membership: LevelIndex::from_codes(&workers, 5).unwrap(),
                firm_index: LevelIndex::from_codes(&firms, first as usize).unwrap(),
                workers,
                firms,
                weights,
            }
        }

        fn design(&self) -> BorrowedInterceptFeDesign<'_> {
            BorrowedInterceptFeDesign::new(
                self.workers.len(),
                vec![
                    BorrowedInterceptEffect::new(
                        &self.workers,
                        self.membership.n_levels(),
                        &self.membership,
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

        fn source(&self) -> CellSource<'_> {
            CellSource::Observations {
                membership: &self.membership,
                firms: &self.firms,
                retained: self.firm_index.n_levels(),
                weights: BorrowedMapWeights::Provided(&self.weights),
            }
        }

        fn build(
            &self,
            sampling: Sampling,
            budget: Option<FeSolverMemoryBudget>,
            cached: bool,
        ) -> BoundedCmgBuild {
            if cached {
                CmgSchurPreconditioner::try_build_bounded(
                    &self.design(),
                    BorrowedMapWeights::Provided(&self.weights),
                    0,
                    5,
                    sampling,
                    1,
                    budget,
                    false,
                )
            } else {
                CmgSchurPreconditioner::build_observations_bounded(
                    &self.membership,
                    &self.firms,
                    self.firm_index.n_levels(),
                    BorrowedMapWeights::Provided(&self.weights),
                    5,
                    sampling,
                    budget,
                    false,
                    false,
                )
            }
            .unwrap()
        }

        fn envelope(&self, sampling: Sampling) -> CacheEnvelope {
            cache_envelope(
                self.workers.len(),
                self.membership.n_levels(),
                self.firm_index.n_levels(),
                9 * 8,
                5,
                sampling,
            )
            .unwrap()
        }
    }

    #[test]
    fn cached_and_uncached_builds_repeat_exactly() {
        let fixture = Fixture::new();
        for cached in [false, true] {
            for samples in [0, 1, 2] {
                let sampling = Sampling { samples, seed: 0 };
                let budget = Some(FeSolverMemoryBudget {
                    usable_bytes: fixture.envelope(sampling).peak_bytes as u128 + 913,
                    execution_bytes: 913,
                });
                let first = fixture.build(sampling, budget, cached);
                assert!(first.preconditioner.is_some());
                assert_same_build(first, fixture.build(sampling, budget, cached));
            }
        }
    }

    #[test]
    fn canonical_cells_and_compensated_totals_survive_sampler_scratch_mutation() {
        let fixture = Fixture::new();
        let mut work = Work::new(false);
        let mut rows = BoundedCells {
            source: fixture.source(),
            weight_scale: 1.0e12,
            cells: completed(work.capacity(72)),
            cache: None,
        };
        let mut cache = completed(CanonicalCells::new(fixture.workers.len(), 5, &mut work));
        let mut expected = Vec::new();
        for worker in 0..5 {
            let total = completed(rows.load_bounded(worker, &mut work));
            expected.push((rows.cells.clone(), total.to_bits()));
            completed(cache.store(&rows.cells, total, &mut work));
        }
        let sampling = Sampling {
            samples: 2,
            seed: 19,
        };
        let mut sampler = completed(SampledTrees::new_bounded(sampling, 72, &mut work));
        for (worker, (canonical, total)) in expected.iter().enumerate() {
            assert_eq!(
                completed(cache.load(worker, &mut rows.cells, &mut work)).to_bits(),
                *total
            );
            assert_eq!(&rows.cells, canonical);
            if canonical.len() > 5 {
                completed(sampler.prepare_bounded(&mut rows.cells, worker, &mut work));
            }
            assert_eq!(
                completed(cache.load(worker, &mut rows.cells, &mut work)).to_bits(),
                *total
            );
            assert_eq!(&rows.cells, canonical);
        }
    }

    #[test]
    fn cache_preserves_full_graph_hierarchy_and_application() {
        let fixture = Fixture::new();
        for samples in [0, 1, 2] {
            let sampling = Sampling { samples, seed: 0 };
            let envelope = fixture.envelope(sampling);
            let budget = Some(FeSolverMemoryBudget {
                usable_bytes: envelope.peak_bytes as u128 + 913,
                execution_bytes: 913,
            });
            let old = fixture.build(sampling, budget, false);
            let new = fixture.build(sampling, budget, true);
            let mut old = old.preconditioner.unwrap();
            let mut new = new.preconditioner.unwrap();
            assert_eq!(
                old.preconditioner.hierarchy(),
                new.preconditioner.hierarchy()
            );
            assert_eq!(old.components.labels, new.components.labels);
            assert_eq!(old.components.sizes, new.components.sizes);
            assert_eq!(
                new.retained_bytes().unwrap(),
                old.retained_bytes().unwrap() + envelope.cache_bytes as u128
            );
            let n = fixture.firm_index.n_levels();
            let rhs: Vec<_> = (0..n).map(|i| (i as f64 * 0.31).sin()).collect();
            let mut expected = vec![0.0; n];
            let mut actual = vec![0.0; n];
            old.apply_batch(&rhs, &mut expected, 1, 1, None, None)
                .unwrap();
            new.apply_batch(&rhs, &mut actual, 1, 1, None, None)
                .unwrap();
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn complete_peak_decline_keeps_the_original_usable_two_pass_path() {
        let fixture = Fixture::new();
        let sampling = Sampling {
            samples: 2,
            seed: 0,
        };
        let original = fixture.build(sampling, None, false);
        let needed = original.peak.bytes().unwrap() as u128;
        assert!(needed < fixture.envelope(sampling).peak_bytes as u128);
        let budget = Some(FeSolverMemoryBudget {
            usable_bytes: needed + 123,
            execution_bytes: 123,
        });
        assert!(admitted_cache_bytes(fixture.source(), 72, 5, sampling, budget).is_none());
        let declined = fixture.build(sampling, budget, true);
        assert_eq!(declined.refusal, None);
        assert_eq!(declined.peak, original.peak);
        let original = original.preconditioner.unwrap();
        let declined = declined.preconditioner.unwrap();
        assert_eq!(
            declined.retained_bytes().unwrap(),
            original.retained_bytes().unwrap()
        );
        assert_eq!(
            declined.preconditioner.hierarchy(),
            original.preconditioner.hierarchy()
        );
        let exhausted = Some(FeSolverMemoryBudget {
            usable_bytes: 1,
            execution_bytes: 2,
        });
        assert!(admitted_cache_bytes(fixture.source(), 72, 5, sampling, exhausted).is_none());
    }

    #[test]
    fn sampled_cache_envelope_omits_worker_vertices_and_bounds_complete_build() {
        let fixture = Fixture::new();
        let workers = fixture.membership.n_levels();
        let firms = fixture.firm_index.n_levels();
        let row_bytes = workers * std::mem::size_of::<(usize, f64)>();
        for samples in [0, 1, 2] {
            let sampling = Sampling { samples, seed: 0 };
            let envelope = fixture.envelope(sampling);
            let without_workers =
                cache_envelope(fixture.workers.len(), 0, firms, 72, 5, sampling).unwrap();
            assert_eq!(
                envelope.cache_bytes - without_workers.cache_bytes,
                row_bytes
            );
            if samples > 0 {
                // Workers still need cached row metadata, but cannot enlarge
                // graph, hierarchy, workspace or RHS/output vertex banks.
                assert_eq!(envelope.peak_bytes - without_workers.peak_bytes, row_bytes);
            } else {
                assert!(envelope.peak_bytes - without_workers.peak_bytes > row_bytes);
            }
            let budget = Some(FeSolverMemoryBudget {
                usable_bytes: envelope.peak_bytes as u128 + 913,
                execution_bytes: 913,
            });
            assert_eq!(
                admitted_cache_bytes(fixture.source(), 72, 5, sampling, budget),
                Some(envelope.cache_bytes)
            );
            let built = fixture.build(sampling, budget, true);
            assert_eq!(built.refusal, None);
            assert!(built.peak.bytes().unwrap() as u128 <= envelope.peak_bytes as u128);
            let preconditioner = built.preconditioner.unwrap();
            assert!(preconditioner.retained_bytes().unwrap() <= envelope.peak_bytes as u128);
            let graph = preconditioner.preconditioner.hierarchy().levels()[0].graph();
            // Exactly the degree-six and degree-nine rows need auxiliary
            // vertices without sampling; both become firm-only sampled trees.
            assert_eq!(
                graph.vertex_count(),
                firms + if samples == 0 { 2 } else { 0 }
            );
        }
    }

    #[test]
    fn checked_envelope_overflow_declines_without_allocation() {
        let sampling = Sampling::default();
        for (observations, workers, firms, max_rows) in [
            (usize::MAX, 1, 1, 1),
            (1, usize::MAX, 1, 1),
            (1, 1, usize::MAX, 1),
            (1, 1, 1, usize::MAX),
        ] {
            assert!(cache_envelope(observations, workers, firms, max_rows, 5, sampling).is_none());
        }
    }

    #[test]
    fn empty_observations_and_isolated_firms_keep_identical_graphs() {
        let membership = LevelIndex::from_codes(&[], 0).unwrap();
        let source = CellSource::Observations {
            membership: &membership,
            firms: &[],
            retained: 3,
            weights: BorrowedMapWeights::Unit,
        };
        let build = |cached| {
            let mut work = Work::new(false);
            let mut memory = Admission {
                budget: None,
                peak: SolverWorkspacePeak::ZERO,
            };
            completed(graph(
                source,
                5,
                Sampling::default(),
                cached,
                &mut work,
                &mut memory,
            ))
        };
        let (old, old_components, old_scale, _) = build(false);
        let (new, new_components, new_scale, cache_bytes) = build(true);
        assert_eq!(old, new);
        assert_eq!(old_components.labels, new_components.labels);
        assert_eq!(old_components.sizes, new_components.sizes);
        assert_eq!(old_scale.to_bits(), new_scale.to_bits());
        assert!(cache_bytes > 0);
    }
}
