//! Optional, serial construction. Every loop, comparison and move is admitted
//! before it runs; refusal leaves all incumbent solver owners untouched.

use std::cmp::Ordering;

use cmg::{BuildDecision, BuildOperation, BuildStopReason, CmgError};

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fe) enum BuildRefusal {
    WorkBudget,
    MemoryBudget,
    Unused,
}

pub(in crate::fe) struct BoundedCmgBuild {
    pub(in crate::fe) preconditioner: Option<CmgSchurPreconditioner>,
    pub(in crate::fe) work_used: u64,
    pub(in crate::fe) refusal: Option<BuildRefusal>,
    pub(in crate::fe) peak: SolverWorkspacePeak,
}

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

/// Scalar arithmetic/access allowance. Allocator and destructor latency are
/// outside this computational bound. Allocations are checked before they begin.
pub(super) struct Work<'a> {
    budget: u64,
    used: u64,
    ordinal: u64,
    control: Option<&'a mut dyn cmg::BuildControl>,
}

impl Work<'_> {
    #[inline(always)]
    fn gate(&mut self, phase: &'static str, operation: BuildOperation) -> Attempt<()> {
        crate::interrupt::checkpoint()?;
        let units = match operation {
            BuildOperation::Work { units } if (1..=1024).contains(&units) => u64::from(units),
            BuildOperation::Work { .. } => {
                return Err(invalid_data("invalid bounded CMG work chunk").into())
            }
            BuildOperation::Allocation { .. } => 0,
        };
        if units > self.budget - self.used {
            return Err(AttemptError::Refused(BuildRefusal::WorkBudget));
        }
        let ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or_else(|| invalid_data("bounded CMG operation ordinal overflows"))?;
        if let Some(control) = &mut self.control {
            match control.checkpoint(cmg::BuildBoundary {
                phase,
                chunk_id: self.ordinal,
                charged_work: self.used,
                operation,
            }) {
                BuildDecision::Continue => {}
                BuildDecision::StopUnused => {
                    return Err(AttemptError::Refused(BuildRefusal::Unused))
                }
                BuildDecision::Interrupted => {
                    return Err(FeregError::new(
                        crate::error::ErrorCode::Interrupted,
                        "bounded CMG construction was interrupted",
                    )
                    .into())
                }
            }
        }
        // Any external controller has admitted this complete chunk before charging it.
        self.used += units;
        self.ordinal = ordinal;
        Ok(())
    }

    #[inline(always)]
    pub(super) fn charge(&mut self, units: u16) -> Attempt<()> {
        self.gate("fereg.cmg.prepare", BuildOperation::Work { units })
    }

    fn cmg<T>(
        &mut self,
        build: impl FnOnce(&mut dyn cmg::BuildControl) -> std::result::Result<T, CmgError>,
    ) -> Attempt<T> {
        let mut failure = None;
        let result = build(&mut |boundary: cmg::BuildBoundary| match self
            .gate(boundary.phase, boundary.operation)
        {
            Ok(()) => BuildDecision::Continue,
            Err(error) => {
                let decision = if matches!(&error, AttemptError::Fatal(e)
                        if e.code == crate::error::ErrorCode::Interrupted)
                {
                    BuildDecision::Interrupted
                } else {
                    BuildDecision::StopUnused
                };
                failure = Some(error);
                decision
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        result.map_err(|error| match error {
            CmgError::BuildStopped {
                reason:
                    BuildStopReason::StopUnused
                    | BuildStopReason::WorkBudgetExhausted
                    | BuildStopReason::CounterOverflow,
                ..
            } => AttemptError::Refused(BuildRefusal::WorkBudget),
            CmgError::BuildStopped {
                reason: BuildStopReason::Interrupted,
                ..
            } => AttemptError::Fatal(FeregError::new(
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
        self.charge(32)?;
        self.gate(
            "fereg.cmg.storage",
            BuildOperation::Allocation {
                bytes: bytes::<T>(count)?,
            },
        )?;
        Ok(super::capacity(count)?)
    }

    pub(super) fn filled<T: Copy>(&mut self, count: usize, value: T) -> Attempt<Vec<T>> {
        let mut result = self.capacity(count)?;
        for _ in 0..count {
            self.charge(8)?;
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
            work: &mut Work<'_>,
            values: &mut [T],
            mut root: usize,
            end: usize,
            compare: &impl Fn(&T, &T) -> Ordering,
        ) -> Attempt<()> {
            loop {
                work.charge(64)?;
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
        self.charge(16)?;
        for root in (0..values.len() / 2).rev() {
            sift(self, values, root, values.len(), &compare)?;
        }
        for end in (1..values.len()).rev() {
            self.charge(32)?;
            values.swap(0, end);
            sift(self, values, 0, end, &compare)?;
        }
        Ok(())
    }
}

fn admit(
    memory: &mut Admission,
    work: &mut Work<'_>,
    parents: u128,
    additional: usize,
) -> Attempt<usize> {
    work.charge(128)?;
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
        work_budget: u64,
    ) -> Result<(BoundedCmgBuild, f64)> {
        Self::try_build_reduced_bounded_controlled(
            n_eliminated,
            n_retained,
            row_ptr,
            columns,
            weights,
            degree,
            sampling,
            budget,
            work_budget,
            &mut |_| BuildDecision::Continue,
        )
    }

    /// The callback gates preprocessing, allocation and every upstream operation.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::fe) fn try_build_reduced_bounded_controlled(
        n_eliminated: usize,
        n_retained: usize,
        row_ptr: &[u32],
        columns: &[u32],
        weights: &[f64],
        degree: usize,
        sampling: Sampling,
        budget: Option<FeSolverMemoryBudget>,
        work_budget: u64,
        control: &mut dyn cmg::BuildControl,
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
        let mut work = Work {
            budget: work_budget,
            used: 0,
            ordinal: 0,
            control: Some(control),
        };
        let mut memory = Admission {
            budget,
            peak: SolverWorkspacePeak::ZERO,
        };
        let mut weight_scale = 1.0;
        let attempt = (|| {
            for pair in row_ptr.windows(2) {
                work.charge(16)?;
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
            finish(
                graph,
                components,
                cache_bytes,
                false,
                &mut work,
                &mut memory,
            )
        })();
        let (preconditioner, refusal) = match attempt {
            Ok(candidate) => (Some(candidate), None),
            Err(AttemptError::Refused(reason)) => (None, Some(reason)),
            Err(AttemptError::Fatal(error)) => return Err(error),
        };
        Ok((
            BoundedCmgBuild {
                preconditioner,
                work_used: work.used,
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

    /// Candidate-only serial builder. `budget` includes the caller's live
    /// incumbent owners in execution_bytes. `work_budget` excludes the caller's
    /// reserved residual-recompute/restart allowance. No solver state is changed
    /// by a refusal. Actual allocation and numerical failures remain fatal.
    /// Only synchronous presets may cache the census; an overlap preset's
    /// one-worker fallback keeps the original two-pass work and allocation gates.
    /// `bounded_terminal` is the separate, portfolio-only factor-preflight policy.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::fe) fn try_build_bounded(
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        left_effect: usize,
        degree: usize,
        sampling: Sampling,
        rhs_capacity: usize,
        budget: Option<FeSolverMemoryBudget>,
        work_budget: u64,
        cache_census: bool,
        bounded_terminal: bool,
    ) -> Result<BoundedCmgBuild> {
        sampling.validate()?;
        if design.effects().len() != 2 || left_effect > 1 || rhs_capacity == 0 {
            return Err(invalid_argument(
                "CMG Schur requires two intercept effects and positive RHS capacity",
            ));
        }
        // This flag identifies synchronous presets even when cache admission
        // declines. Overlap's uncached fallback retains its original controller.
        let mut continue_build = |_| BuildDecision::Continue;
        let control: Option<&mut dyn cmg::BuildControl> = if cache_census {
            None
        } else {
            Some(&mut continue_build)
        };
        Self::build_observations_bounded(
            design.effects()[left_effect].level_index(),
            design.effects()[1 - left_effect].codes(),
            design.effects()[1 - left_effect].n_levels(),
            weights,
            degree,
            sampling,
            budget,
            work_budget,
            cache_census,
            bounded_terminal,
            control,
        )
    }

    /// Borrow an owned service's immutable observation banks without rebuilding a design.
    /// All bounded work and allocation gates use the same external controller.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::fe) fn try_build_observations_bounded_controlled(
        membership: &LevelIndex,
        firms: &[u32],
        retained: usize,
        weights: BorrowedMapWeights<'_>,
        degree: usize,
        sampling: Sampling,
        budget: Option<FeSolverMemoryBudget>,
        work_budget: u64,
        control: &mut dyn cmg::BuildControl,
    ) -> Result<BoundedCmgBuild> {
        Self::build_observations_bounded(
            membership,
            firms,
            retained,
            weights,
            degree,
            sampling,
            budget,
            work_budget,
            false,
            false,
            Some(control),
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
        work_budget: u64,
        cache_census: bool,
        bounded_terminal: bool,
        control: Option<&mut dyn cmg::BuildControl>,
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
        let mut work = Work {
            budget: work_budget,
            used: 0,
            ordinal: 0,
            control,
        };
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
            finish(
                graph,
                components,
                cache_bytes,
                bounded_terminal,
                &mut work,
                &mut memory,
            )
        });
        let (preconditioner, refusal) = match attempt {
            Ok(preconditioner) => (Some(preconditioner), None),
            Err(AttemptError::Refused(reason)) => (None, Some(reason)),
            Err(AttemptError::Fatal(error)) => return Err(error),
        };
        Ok(BoundedCmgBuild {
            preconditioner,
            work_used: work.used,
            refusal,
            peak: memory.peak,
        })
    }
}

fn finish(
    graph: Laplacian,
    components: FirmComponents,
    cache_bytes: u128,
    bounded_terminal: bool,
    work: &mut Work<'_>,
    memory: &mut Admission,
) -> Attempt<CmgSchurPreconditioner> {
    let options = CmgOptions::default();
    let experiment = component_options();
    work.charge(1024)?;
    let bound = CmgPreconditioner::component_serial_construction_allocation_bound(
        graph.vertex_count(),
        graph.edge_count(),
        options,
        experiment,
    )
    .map_err(cmg_error)?;
    // The synchronous cache stays reserved even after graph emission drops it.
    // Controlled builders pass zero and preserve their existing admission order.
    let parents = graph_storage_bound(&graph)? + components.bytes() + cache_bytes;
    let admitted = admit(memory, work, parents, bound)?;
    // The finite upstream meter shadows callback-admitted work; callbacks alone
    // debit this owner. Capture the remainder after the existing admission work.
    let remaining_work = if bounded_terminal {
        work.budget - work.used
    } else {
        u64::MAX
    };
    let preconditioner = work.cmg(|control| {
        if bounded_terminal {
            CmgPreconditioner::build_component_experiment_serial_controlled_with_factor_preflight(
                &graph,
                options,
                experiment,
                admitted,
                remaining_work,
                control,
            )
        } else {
            CmgPreconditioner::build_component_experiment_serial_controlled(
                &graph,
                options,
                experiment,
                admitted,
                u64::MAX,
                control,
            )
        }
    })?;
    // Keep the already-admitted conservative hierarchy bound. Exact storage
    // reporting recursively deduplicates allocation owners and is quadratic;
    // it must never become an ungated operation in optional construction.
    work.charge(128)?;
    let parents = parents + bound as u128;
    let workspace_bytes = work
        .cmg(|control| preconditioner.workspace_allocation_bytes_controlled(u64::MAX, control))?;
    let cycle_work =
        work.cmg(|control| preconditioner.scalar_cycle_work_bound_controlled(u64::MAX, control))?;
    work.charge(128)?;
    let startup_bound = (cycle_work as u128)
        .checked_add(
            (components.labels.len() as u128)
                .checked_mul(384)
                .ok_or_else(|| invalid_data("CMG startup work overflows"))?,
        )
        .and_then(|units| units.checked_add((components.sizes.len() as u128) * 64))
        .and_then(|units| units.checked_add((graph.vertex_count() as u128) * 4))
        .and_then(|units| units.checked_add(256))
        .and_then(|units| u64::try_from(units).ok())
        .ok_or_else(|| invalid_data("CMG startup work overflows"))?;
    let bank_bytes = bytes::<[f64; 2]>(graph.vertex_count())?;
    let bound = workspace_bytes
        .checked_add(bank_bytes)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<CmgSchurPreconditioner>()))
        .ok_or_else(|| allocation("bounded CMG apply storage overflows"))?;
    admit(memory, work, parents, bound)?;
    let scalar_workspace =
        work.cmg(|control| preconditioner.try_workspace_controlled(u64::MAX, control))?;
    let rhs = work.filled(graph.vertex_count(), 0.0)?;
    let solution = work.filled(graph.vertex_count(), 0.0)?;
    work.charge(256)?;
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
        startup_bound: Some(startup_bound),
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

fn find_root(labels: &mut [usize], mut vertex: usize, work: &mut Work<'_>) -> Attempt<usize> {
    loop {
        work.charge(32)?;
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

impl CanonicalCells {
    fn new(observations: usize, workers: usize, work: &mut Work<'_>) -> Attempt<Self> {
        Ok(Self {
            cells: work.capacity(observations)?,
            rows: work.capacity(workers)?,
        })
    }

    fn store(&mut self, cells: &[(u32, f64)], total: f64, work: &mut Work<'_>) -> Attempt<()> {
        work.charge(32)?;
        self.rows.push((self.cells.len(), total));
        for cell in cells {
            work.charge(16)?;
            self.cells.push(*cell);
        }
        Ok(())
    }

    fn load(
        &self,
        worker: usize,
        cells: &mut Vec<(u32, f64)>,
        work: &mut Work<'_>,
    ) -> Attempt<f64> {
        work.charge(32)?;
        let (start, total) = self.rows[worker];
        let end = self
            .rows
            .get(worker + 1)
            .map_or(self.cells.len(), |row| row.0);
        cells.clear();
        for cell in &self.cells[start..end] {
            work.charge(16)?;
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
    fn load_bounded(&mut self, worker: usize, work: &mut Work<'_>) -> Attempt<f64> {
        work.charge(16)?;
        self.cells.clear();
        for position in 0..self.source.row_count(worker) {
            work.charge(64)?;
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
            work.charge(32)?;
            let firm = self.cells[read].0;
            let mut sum = 0.0;
            let mut correction = 0.0;
            while read < self.cells.len() && self.cells[read].0 == firm {
                work.charge(32)?;
                neumaier_add(self.cells[read].1, &mut sum, &mut correction);
                read += 1;
            }
            work.charge(32)?;
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
        work: &mut Work<'_>,
    ) -> Attempt<GraphCounts> {
        let mut counts = GraphCounts::default();
        for worker in 0..self.source.worker_count() {
            let total = self.load_bounded(worker, work)?;
            if let Some(cache) = &mut self.cache {
                cache.store(&self.cells, total, work)?;
            }
            work.charge(64)?;
            let count = self.cells.len();
            if let Some(&(first, _)) = self.cells.first() {
                for &(firm, _) in self.cells.iter().skip(1) {
                    work.charge(16)?;
                    let first = find_root(labels, first as usize, work)?;
                    let other = find_root(labels, firm as usize, work)?;
                    labels[first.max(other)] = first.min(other);
                }
            }
            work.charge(128)?;
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

    fn emit_edges_bounded(
        &mut self,
        sampler: &mut Option<SampledTrees>,
        cutoff: usize,
        edges: &mut Vec<(usize, usize, f64)>,
        work: &mut Work<'_>,
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
                        work.charge(8)?;
                        edges.push(edge);
                    }
                } else {
                    for &(firm, weight) in &self.cells {
                        work.charge(16)?;
                        edges.push((firm as usize, auxiliary, weight));
                    }
                    auxiliary += 1;
                }
            } else {
                for first in 0..self.cells.len() {
                    for second in first + 1..self.cells.len() {
                        work.charge(64)?;
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

fn graph(
    source: CellSource<'_>,
    cutoff: usize,
    sampling: Sampling,
    cache_census: bool,
    work: &mut Work<'_>,
    memory: &mut Admission,
) -> Attempt<(Laplacian, FirmComponents, f64, u128)> {
    work.charge(64)?;
    let firms = source.firm_count();
    let weights = source.weights();
    let mut weight_scale = 1.0_f64;
    if let BorrowedMapWeights::Provided(values) = weights {
        weight_scale = 0.0;
        for &weight in values {
            work.charge(16)?;
            if !weight.is_finite() || weight <= 0.0 {
                return Err(invalid_data("CMG Schur requires positive finite weights").into());
            }
            weight_scale = weight_scale.max(weight);
        }
    }
    let mut max_rows = 0;
    for worker in 0..source.worker_count() {
        work.charge(16)?;
        max_rows = max_rows.max(source.row_count(worker));
    }
    work.charge(128)?;
    let cache_bytes = if cache_census {
        // Retain this dimension/helper allowance even when the cache declines:
        // the original two-pass construction receives only the remaining work.
        work.charge(1024)?;
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
        work.charge(8)?;
        labels.push(firm);
    }
    let mut sizes = work.filled(firms, usize::MAX)?;
    let counts = workers.census_bounded(cutoff, sampling, &mut labels, work)?;
    for firm in 0..firms {
        work.charge(8)?;
        labels[firm] = find_root(&mut labels, firm, work)?;
    }
    let mut component_count = 0;
    for label in &mut labels {
        work.charge(32)?;
        if sizes[*label] == usize::MAX {
            sizes[*label] = component_count;
            component_count += 1;
        }
        *label = sizes[*label];
    }
    sizes.truncate(component_count);
    for size in &mut sizes {
        work.charge(8)?;
        *size = 0;
    }
    for &label in &labels {
        work.charge(8)?;
        sizes[label] += 1;
    }
    work.charge(128)?;
    let vertices = firms
        .checked_add(counts.retained_workers)
        .filter(|vertices| *vertices > 0 && *vertices as u128 <= u32::MAX as u128 + 1)
        .ok_or_else(|| allocation("CMG Schur graph vertex count is not supported"))?;
    let scratch = vec_capacity_bytes(&workers.cells)
        + cache_bytes as u128
        + sampler.as_ref().map_or(0, SampledTrees::bytes)
        + vec_capacity_bytes(&labels)
        + vec_capacity_bytes(&sizes);
    let edge_bytes = bytes::<(usize, usize, f64)>(counts.input_edges)?;
    admit(memory, work, scratch, edge_bytes)?;
    let mut edges = work.capacity(counts.input_edges)?;
    workers.emit_edges_bounded(&mut sampler, cutoff, &mut edges, work)?;
    work.charge(128)?;
    if edges.len() != counts.input_edges {
        return Err(invalid_data("CMG Schur edge census changed during construction").into());
    }
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
    let graph = work.cmg(|control| {
        Laplacian::from_edges_controlled(vertices, &edges, admitted, u64::MAX, control)
    })?;
    drop(edges);
    drop(workers);
    drop(sampler);
    work.charge(256)?;
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

    #[test]
    fn gate_rejections_preserve_state_and_admission_order() {
        let calls = std::cell::Cell::new(0);
        let decision = std::cell::Cell::new(BuildDecision::Continue);
        let mut control = |boundary: cmg::BuildBoundary| {
            calls.set(calls.get() + 1);
            assert_eq!(boundary.chunk_id, 7);
            assert_eq!(boundary.charged_work, 2);
            decision.get()
        };
        let mut work = Work {
            budget: 4,
            used: 2,
            ordinal: u64::MAX,
            control: Some(&mut control),
        };
        for units in [0, 1025] {
            assert!(matches!(work.charge(units), Err(AttemptError::Fatal(error))
                if error.code == crate::error::ErrorCode::InvalidData
                    && error.message == "invalid bounded CMG work chunk"));
        }
        // Budget refusal precedes ordinal overflow, without reaching the controller.
        assert!(matches!(
            work.charge(3),
            Err(AttemptError::Refused(BuildRefusal::WorkBudget))
        ));
        assert!(matches!(work.charge(1), Err(AttemptError::Fatal(error))
            if error.message == "bounded CMG operation ordinal overflows"));
        assert_eq!((work.used, work.ordinal, calls.get()), (2, u64::MAX, 0));

        work.ordinal = 7;
        decision.set(BuildDecision::StopUnused);
        assert!(matches!(
            work.charge(2),
            Err(AttemptError::Refused(BuildRefusal::Unused))
        ));
        decision.set(BuildDecision::Interrupted);
        assert!(matches!(work.charge(2), Err(AttemptError::Fatal(error))
            if error.code == crate::error::ErrorCode::Interrupted));
        assert_eq!((work.used, work.ordinal, calls.get()), (2, 7, 2));
        decision.set(BuildDecision::Continue);
        assert!(work.charge(2).is_ok());
        assert_eq!((work.used, work.ordinal, calls.get()), (4, 8, 3));
        assert!(matches!(
            work.charge(1),
            Err(AttemptError::Refused(BuildRefusal::WorkBudget))
        ));
        assert_eq!((work.used, work.ordinal, calls.get()), (4, 8, 3));
    }

    #[test]
    fn gate_cancellation_precedence_isolated() {
        const CHILD: &str = "FEREG_CMG_GATE_CANCELLATION_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD, "1")
                .args([
                    "fe::cmg_schur::bounded::control_tests::gate_cancellation_precedence_isolated",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let calls = std::cell::Cell::new(0);
        let mut control = |_| {
            calls.set(calls.get() + 1);
            crate::interrupt::request_cancellation();
            BuildDecision::Continue
        };
        let mut work = Work {
            budget: 0,
            used: 0,
            ordinal: u64::MAX,
            control: Some(&mut control),
        };
        crate::interrupt::request_cancellation();
        for units in [0, 1, 1025] {
            assert!(matches!(work.charge(units), Err(AttemptError::Fatal(error))
                if error.code == crate::error::ErrorCode::Interrupted));
        }
        assert_eq!((work.used, work.ordinal, calls.get()), (0, u64::MAX, 0));
        crate::interrupt::reset_cancellation();
        work.budget = 1;
        work.ordinal = 0;
        // A controller-triggered cancellation does not retroactively reject an
        // admitted operation. The next original checkpoint observes it first.
        assert!(work.charge(1).is_ok());
        assert_eq!((work.used, work.ordinal, calls.get()), (1, 1, 1));
        assert!(matches!(work.charge(1), Err(AttemptError::Fatal(error))
            if error.code == crate::error::ErrorCode::Interrupted));
        assert_eq!((work.used, work.ordinal, calls.get()), (1, 1, 1));
        crate::interrupt::reset_cancellation();
    }

    fn candidate(control: &mut dyn cmg::BuildControl) -> Result<(BoundedCmgBuild, f64)> {
        CmgSchurPreconditioner::try_build_reduced_bounded_controlled(
            2,
            2,
            &[0, 2, 4],
            &[0, 1, 0, 1],
            &[2.0, 3.0, 2.0, 3.0],
            5,
            Sampling::default(),
            None,
            u64::MAX,
            control,
        )
    }

    #[test]
    fn external_control_covers_preparation_allocations_and_upstream_without_changing_work() {
        let mut trace = Vec::new();
        let (controlled, scale) = candidate(&mut |boundary| {
            trace.push(boundary);
            BuildDecision::Continue
        })
        .unwrap();
        let (serial, old_scale) = CmgSchurPreconditioner::try_build_reduced_bounded(
            2,
            2,
            &[0, 2, 4],
            &[0, 1, 0, 1],
            &[2.0, 3.0, 2.0, 3.0],
            5,
            Sampling::default(),
            None,
            u64::MAX,
        )
        .unwrap();
        assert_eq!(controlled.work_used, serial.work_used);
        assert_eq!(scale, old_scale);
        let old = serial.preconditioner.unwrap();
        let new = controlled.preconditioner.unwrap();
        assert_eq!(
            new.preconditioner.hierarchy(),
            old.preconditioner.hierarchy()
        );
        assert_eq!(
            new.preconditioner.apply(&[1.0, -1.0]).unwrap(),
            old.preconditioner.apply(&[1.0, -1.0]).unwrap()
        );
        let mut charged = 0;
        for (ordinal, boundary) in trace.iter().enumerate() {
            assert_eq!(boundary.chunk_id, ordinal as u64);
            assert_eq!(boundary.charged_work, charged);
            if let BuildOperation::Work { units } = boundary.operation {
                assert!((1..=1024).contains(&units));
                charged += u64::from(units);
            }
        }
        assert_eq!(charged, controlled.work_used);
        assert!(trace.iter().any(|b| b.phase == "fereg.cmg.prepare"));
        assert!(trace.iter().any(|b| !b.phase.starts_with("fereg.")));
        let allocation = trace
            .iter()
            .position(|b| matches!(b.operation, BuildOperation::Allocation { .. }))
            .unwrap();
        let upstream = trace
            .iter()
            .position(|b| !b.phase.starts_with("fereg."))
            .unwrap();
        for stop in [0, allocation, upstream, trace.len() - 1] {
            for decision in [BuildDecision::StopUnused, BuildDecision::Interrupted] {
                let mut seen = 0;
                let result = candidate(&mut |boundary| {
                    assert_eq!(boundary, trace[seen]);
                    let answer = if seen == stop {
                        decision
                    } else {
                        BuildDecision::Continue
                    };
                    seen += 1;
                    answer
                });
                assert_eq!(seen, stop + 1);
                match decision {
                    BuildDecision::StopUnused => {
                        let (stopped, _) = result.unwrap();
                        assert!(stopped.preconditioner.is_none());
                        assert_eq!(stopped.refusal, Some(BuildRefusal::Unused));
                        assert_eq!(stopped.work_used, trace[stop].charged_work);
                    }
                    BuildDecision::Interrupted => assert_eq!(
                        result.err().unwrap().code,
                        crate::error::ErrorCode::Interrupted
                    ),
                    BuildDecision::Continue => unreachable!(),
                }
            }
        }
        let mut replay = Vec::new();
        candidate(&mut |boundary| {
            replay.push(boundary);
            BuildDecision::Continue
        })
        .unwrap();
        assert_eq!(trace, replay);
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
        assert_eq!(actual.work_used, expected.work_used);
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
                assert_eq!(actual.startup_work(2), expected.startup_work(2));
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
            _ => panic!("controller presence changed construction success"),
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
            work: u64,
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
                    work,
                    true,
                    false,
                )
            } else {
                CmgSchurPreconditioner::try_build_observations_bounded_controlled(
                    &self.membership,
                    &self.firms,
                    self.firm_index.n_levels(),
                    BorrowedMapWeights::Provided(&self.weights),
                    5,
                    sampling,
                    budget,
                    work,
                    &mut |_| BuildDecision::Continue,
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

        fn build_terminal(
            &self,
            enabled: bool,
            allowance: u64,
            control: &mut dyn cmg::BuildControl,
        ) -> Result<BoundedCmgBuild> {
            CmgSchurPreconditioner::build_observations_bounded(
                &self.membership,
                &self.firms,
                self.firm_index.n_levels(),
                BorrowedMapWeights::Provided(&self.weights),
                5,
                Sampling {
                    samples: 1,
                    seed: 0,
                },
                None,
                allowance,
                false,
                enabled,
                Some(control),
            )
        }
    }

    #[test]
    fn synchronous_controller_elision_preserves_cached_and_uncached_builds() {
        let fixture = Fixture::new();
        for cached in [false, true] {
            for samples in [0, 1, 2] {
                let sampling = Sampling { samples, seed: 0 };
                let budget = Some(FeSolverMemoryBudget {
                    usable_bytes: fixture.envelope(sampling).peak_bytes as u128 + 913,
                    execution_bytes: 913,
                });
                let build =
                    |allowance, bounded_terminal, control: Option<&mut dyn cmg::BuildControl>| {
                        CmgSchurPreconditioner::build_observations_bounded(
                            &fixture.membership,
                            &fixture.firms,
                            fixture.firm_index.n_levels(),
                            BorrowedMapWeights::Provided(&fixture.weights),
                            5,
                            sampling,
                            budget,
                            allowance,
                            cached,
                            bounded_terminal,
                            control,
                        )
                        .unwrap()
                    };
                let synchronous = build(u64::MAX, false, None);
                assert!(synchronous.preconditioner.is_some());
                let used = synchronous.work_used;
                let mut continue_build = |_| BuildDecision::Continue;
                assert_same_build(
                    synchronous,
                    build(u64::MAX, false, Some(&mut continue_build)),
                );
                assert_same_build(
                    build(u64::MAX, true, None),
                    build(u64::MAX, true, Some(&mut continue_build)),
                );
                for allowance in [used, used - 1, 0] {
                    let synchronous = build(allowance, false, None);
                    assert_eq!(synchronous.preconditioner.is_some(), allowance == used);
                    assert_eq!(
                        synchronous.refusal,
                        (allowance != used).then_some(BuildRefusal::WorkBudget),
                    );
                    assert!(synchronous.work_used <= allowance);
                    assert_same_build(
                        synchronous,
                        build(allowance, false, Some(&mut continue_build)),
                    );
                }
            }
        }
    }

    #[test]
    fn canonical_cells_and_compensated_totals_survive_sampler_scratch_mutation() {
        let fixture = Fixture::new();
        let mut control = |_| BuildDecision::Continue;
        let mut work = Work {
            budget: u64::MAX,
            used: 0,
            ordinal: 0,
            control: Some(&mut control),
        };
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
    fn terminal_preflight_is_opt_in_and_charges_each_callback_once() {
        let fixture = Fixture::new();
        let sampling = Sampling {
            samples: 1,
            seed: 0,
        };
        let mut old_trace = Vec::new();
        let old = fixture
            .build_terminal(false, u64::MAX, &mut |boundary| {
                old_trace.push(boundary);
                BuildDecision::Continue
            })
            .unwrap();
        let controlled = fixture.build(sampling, None, u64::MAX, false);
        assert_eq!(old.work_used, controlled.work_used);
        assert_eq!(old.peak, controlled.peak);
        let mut trace = Vec::new();
        let enabled = fixture
            .build_terminal(true, u64::MAX, &mut |boundary| {
                trace.push(boundary);
                BuildDecision::Continue
            })
            .unwrap();
        let preflight = trace
            .iter()
            .position(|boundary| boundary.phase == "terminal factor preflight")
            .unwrap();
        assert_eq!(
            trace[preflight].operation,
            BuildOperation::Work { units: 1024 }
        );
        assert_eq!(enabled.work_used, old.work_used + 1024);
        assert_eq!(enabled.peak, old.peak);
        assert_eq!(
            trace
                .iter()
                .filter(|boundary| boundary.phase != "terminal factor preflight")
                .map(|boundary| (boundary.phase, boundary.operation))
                .collect::<Vec<_>>(),
            old_trace
                .iter()
                .map(|boundary| (boundary.phase, boundary.operation))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            enabled.preconditioner.unwrap().preconditioner.hierarchy(),
            old.preconditioner.unwrap().preconditioner.hierarchy()
        );
        let mut charged = 0;
        for (ordinal, boundary) in trace.iter().enumerate() {
            assert_eq!(boundary.chunk_id, ordinal as u64);
            assert_eq!(boundary.charged_work, charged);
            if let BuildOperation::Work { units } = boundary.operation {
                charged += u64::from(units);
            }
        }
        assert_eq!(charged, enabled.work_used);
    }

    #[test]
    fn terminal_preflight_preserves_stop_and_exhaustion_semantics() {
        let fixture = Fixture::new();
        let mut trace = Vec::new();
        fixture
            .build_terminal(true, u64::MAX, &mut |boundary| {
                trace.push(boundary);
                BuildDecision::Continue
            })
            .unwrap();
        let preflight = trace
            .iter()
            .position(|boundary| boundary.phase == "terminal factor preflight")
            .unwrap();
        for decision in [BuildDecision::StopUnused, BuildDecision::Interrupted] {
            let mut seen = 0;
            let result = fixture.build_terminal(true, u64::MAX, &mut |boundary| {
                assert_eq!(boundary, trace[seen]);
                let answer = if seen == preflight {
                    decision
                } else {
                    BuildDecision::Continue
                };
                seen += 1;
                answer
            });
            assert_eq!(seen, preflight + 1);
            if decision == BuildDecision::StopUnused {
                let stopped = result.unwrap();
                assert_eq!(stopped.refusal, Some(BuildRefusal::Unused));
                assert!(stopped.preconditioner.is_none());
                assert_eq!(stopped.work_used, trace[preflight].charged_work);
            } else {
                assert_eq!(
                    result.err().unwrap().code,
                    crate::error::ErrorCode::Interrupted
                );
            }
        }
        // Exhaustion immediately after the new gate must stop before another
        // callback; the upstream meter never adds its charged total a second time.
        let allowance = trace[preflight].charged_work + 1024;
        let mut limited_trace = Vec::new();
        let limited = fixture
            .build_terminal(true, allowance, &mut |boundary| {
                limited_trace.push(boundary);
                BuildDecision::Continue
            })
            .unwrap();
        assert_eq!(limited.refusal, Some(BuildRefusal::WorkBudget));
        assert!(limited.preconditioner.is_none());
        assert_eq!(limited.work_used, allowance);
        assert_eq!(limited_trace, trace[..=preflight]);
    }

    #[test]
    fn cache_preserves_full_graph_hierarchy_and_application_with_less_charged_work() {
        let fixture = Fixture::new();
        for samples in [0, 1, 2] {
            let sampling = Sampling { samples, seed: 0 };
            let envelope = fixture.envelope(sampling);
            let budget = Some(FeSolverMemoryBudget {
                usable_bytes: envelope.peak_bytes as u128 + 913,
                execution_bytes: 913,
            });
            let old = fixture.build(sampling, budget, u64::MAX, false);
            let new = fixture.build(sampling, budget, u64::MAX, true);
            assert!(new.work_used < old.work_used);
            let used = new.work_used;
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
            let exact = fixture.build(sampling, budget, used, true);
            assert!(exact.preconditioner.is_some());
            assert_eq!(exact.work_used, used);
            let short = fixture.build(sampling, budget, used - 1, true);
            assert!(short.preconditioner.is_none());
            assert_eq!(short.refusal, Some(BuildRefusal::WorkBudget));
            assert!(short.work_used < used);
        }
    }

    #[test]
    fn complete_peak_decline_keeps_the_original_usable_two_pass_path() {
        let fixture = Fixture::new();
        let sampling = Sampling {
            samples: 2,
            seed: 0,
        };
        let original = fixture.build(sampling, None, u64::MAX, false);
        let needed = original.peak.bytes().unwrap() as u128;
        assert!(needed < fixture.envelope(sampling).peak_bytes as u128);
        let budget = Some(FeSolverMemoryBudget {
            usable_bytes: needed + 123,
            execution_bytes: 123,
        });
        assert!(admitted_cache_bytes(fixture.source(), 72, 5, sampling, budget).is_none());
        let declined = fixture.build(sampling, budget, u64::MAX, true);
        assert_eq!(declined.refusal, None);
        assert_eq!(declined.work_used, original.work_used + 1024);
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
            let built = fixture.build(sampling, budget, u64::MAX, true);
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
            let mut control = |_| BuildDecision::Continue;
            let mut work = Work {
                budget: u64::MAX,
                used: 0,
                ordinal: 0,
                control: Some(&mut control),
            };
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
