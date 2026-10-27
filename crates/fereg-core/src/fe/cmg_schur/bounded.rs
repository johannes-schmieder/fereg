//! Optional construction. Every loop, comparison and move is counted toward a
//! fixed cancellation cadence; memory is admitted before each allocation and a
//! memory refusal leaves all incumbent solver owners untouched.

use std::cmp::Ordering;
use std::time::Instant;

use cmg::CmgError;

use crate::error::FeregError;

use super::*;

mod install;
mod team;
use install::{install_plan, ApplyShape};
use team::{PrepareTask, CENSUS_TASK_CELLS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fe) enum BuildRefusal {
    MemoryBudget,
}

/// How an automatic build applies its hierarchy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fe) enum AutoApplication {
    /// One team-free scalar cycle per lane (PPML reuse and reduced pairs).
    Scalar,
    /// The row-operator plan and interleaved lanes on the host team, each
    /// installed when admitted, with the scalar application otherwise.
    Team,
}

pub(in crate::fe) struct BoundedCmgBuild {
    pub(in crate::fe) preconditioner: Option<CmgSchurPreconditioner>,
    pub(in crate::fe) refusal: Option<BuildRefusal>,
    pub(in crate::fe) peak: SolverWorkspacePeak,
}

/// How a built Schur preconditioner is installed: the only difference between
/// the automatic upgrade and an explicit `cmgschur` request. Both share the
/// census, edge emission, initial graph, hierarchy options, terminal factor and
/// cancellation cadence; every CMG stage polls the host's cancellation flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fe) enum Install {
    /// Automatic upgrade: the hierarchy builds on the host team when its
    /// parallel bound is admitted (serially otherwise, with the same result).
    /// Without `team_lanes` each lane runs one scalar team-free cycle with no
    /// row operators (PPML reuse and reduced pairs share such a hierarchy).
    /// With `team_lanes`, the row-operator plan and up to that many
    /// interleaved lanes are installed when their bounds are admitted and the
    /// cycle borrows whichever host team is installed; a plan or batch
    /// workspace that does not fit keeps the scalar application. The planned
    /// arithmetic is the scalar cycle's (CMG's fixed-chunk centering of one
    /// coarse component of at least 131,072 entries aside).
    Auto { team_lanes: Option<usize> },
    /// Explicit `cmgschur`: large levels contract on the action's host team of
    /// `threads` workers, a row-operator plan selects that team's eligible
    /// levels, and up to `columns` lanes share one interleaved cycle.
    Explicit { columns: usize, threads: usize },
}

impl Install {
    /// The team-free scalar automatic installation.
    pub(in crate::fe) const AUTO: Self = Self::Auto { team_lanes: None };

    fn columns(self) -> usize {
        match self {
            Self::Auto { team_lanes } => team_lanes.unwrap_or(1),
            Self::Explicit { columns, .. } => columns,
        }
    }

    const fn explicit(self) -> bool {
        matches!(self, Self::Explicit { .. })
    }

    /// Whether a row-operator plan is requested (always built for explicit).
    const fn planned(self) -> bool {
        matches!(
            self,
            Self::Explicit { .. }
                | Self::Auto {
                    team_lanes: Some(_)
                }
        )
    }
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
    /// Test-only record of the team stages taken (see `TEAM_*` bits).
    #[cfg(test)]
    team_stages: u8,
}

#[cfg(test)]
const TEAM_CENSUS: u8 = 1;
#[cfg(test)]
const TEAM_PREPARE: u8 = 2;
#[cfg(test)]
const TEAM_HIERARCHY: u8 = 4;

impl Work {
    pub(super) fn new(break_builder: bool) -> Self {
        Self {
            pending: 0,
            polls: 0,
            executed: false,
            break_builder,
            #[cfg(test)]
            team_stages: 0,
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
            Option<cmg::Team<'_>>,
            cmg::Cancellation<'_>,
        ) -> std::result::Result<T, CmgError>,
    ) -> Attempt<T> {
        self.poll()?;
        let cancelled = crate::interrupt::cancellation_requested;
        let result = crate::parallel::with_cmg_execution(|team| build(team, &cancelled))
            .map_err(AttemptError::Fatal)?;
        result.map_err(|error| match error {
            CmgError::Cancelled { phase } => AttemptError::Fatal(FeregError::new(
                crate::error::ErrorCode::Interrupted,
                format!("bounded CMG construction was cancelled during {phase}"),
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
}

/// Bounded-work counting shared by the serial builder and team lanes: one
/// operation of 1..=1024 conservative units at a time, with the same cadence.
pub(super) trait Counter {
    type Error: From<FeregError>;

    fn step(&mut self, units: u16) -> std::result::Result<(), Self::Error>;
}

impl Counter for Work {
    type Error = AttemptError;

    #[inline(always)]
    fn step(&mut self, units: u16) -> Attempt<()> {
        Work::step(self, units)
    }
}

/// Counted, allocation-free heapsort with a total comparator: every sift step
/// and extraction is one bounded operation, so it can stop between them.
pub(super) fn counted_sort<C: Counter, T: Copy>(
    counter: &mut C,
    values: &mut [T],
    compare: impl Fn(&T, &T) -> Ordering,
) -> std::result::Result<(), C::Error> {
    fn sift<C: Counter, T: Copy>(
        counter: &mut C,
        values: &mut [T],
        mut root: usize,
        end: usize,
        compare: &impl Fn(&T, &T) -> Ordering,
    ) -> std::result::Result<(), C::Error> {
        loop {
            counter.step(64)?;
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
    counter.step(16)?;
    for root in (0..values.len() / 2).rev() {
        sift(counter, values, root, values.len(), &compare)?;
    }
    for end in (1..values.len()).rev() {
        counter.step(32)?;
        values.swap(0, end);
        sift(counter, values, 0, end, &compare)?;
    }
    Ok(())
}

/// Whether optional scratch of `additional` bytes fits after `parents`
/// without recording anything; a refusal selects the serial path instead.
fn fits(memory: &Admission, parents: u128, additional: usize) -> bool {
    memory.budget.is_none_or(|budget| {
        budget
            .usable_bytes
            .checked_sub(budget.execution_bytes)
            .and_then(|bytes| bytes.checked_sub(parents))
            .is_some_and(|available| available >= additional as u128)
    })
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
            let (graph, components, scale, cache_bytes, _) = graph(
                source,
                degree,
                sampling,
                false,
                Install::AUTO,
                &mut work,
                &mut memory,
            )?;
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
                Install::AUTO,
                CmgOptions::default(),
                &mut work,
                &mut memory,
            )
            .map(|(preconditioner, _)| preconditioner)
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
        if self.install != Install::AUTO
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
        crate::interrupt::checkpoint()
    }

    /// Candidate-only builder with admitted initial-graph stages. `budget`
    /// includes the caller's live incumbent owners in execution_bytes. No solver
    /// state is changed by a memory refusal. Cancellation and actual allocation
    /// or numerical failures remain fatal. The canonical census cache is used
    /// whenever its complete peak envelope is admitted. `break_builder` arms
    /// the developer builder-Break point (see `routing_break`).
    /// A team application installs its row-operator plan and up to
    /// [`MAX_BATCH_COLUMNS`] interleaved lanes of `rhs_capacity` only when
    /// their bounds are admitted; otherwise it keeps the scalar application of
    /// the same hierarchy. Neither choice can refuse the build.
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
        application: AutoApplication,
        options: CmgOptions,
    ) -> Result<BoundedCmgBuild> {
        sampling.validate()?;
        if design.effects().len() != 2 || left_effect > 1 || rhs_capacity == 0 {
            return Err(invalid_argument(
                "CMG Schur requires two intercept effects and positive RHS capacity",
            ));
        }
        let install = match application {
            AutoApplication::Scalar => Install::AUTO,
            AutoApplication::Team => Install::Auto {
                team_lanes: Some(rhs_capacity.min(MAX_BATCH_COLUMNS)),
            },
        };
        Self::build_observations_bounded(
            design.effects()[left_effect].level_index(),
            design.effects()[1 - left_effect].codes(),
            design.effects()[1 - left_effect].n_levels(),
            weights,
            degree,
            sampling,
            budget,
            true,
            install,
            options,
            break_builder,
        )
    }

    /// The one Schur graph adapter and hierarchy pipeline. With `cache_census`
    /// (the automatic route) canonical census cells are cached whenever their
    /// complete peak is admitted; otherwise (the explicit route) the census
    /// streams the observations twice like the original explicit builder.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_observations_bounded(
        membership: &LevelIndex,
        firms: &[u32],
        retained: usize,
        weights: BorrowedMapWeights<'_>,
        degree: usize,
        sampling: Sampling,
        budget: Option<FeSolverMemoryBudget>,
        cache_census: bool,
        install: Install,
        options: CmgOptions,
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
        let started = Instant::now();
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
            install,
            &mut work,
            &mut memory,
        )
        .and_then(|(graph, components, _, cache_bytes, counts)| {
            let graph_seconds = started.elapsed().as_secs_f64();
            let shape = (graph.vertex_count(), graph.edge_count());
            let (preconditioner, hierarchy_seconds) = finish(
                graph,
                components,
                cache_bytes,
                install,
                options,
                &mut work,
                &mut memory,
            )?;
            if let Install::Explicit { threads, .. } = install {
                if crate::diagnostics::solver_trace_enabled() {
                    let report = preconditioner.preconditioner.hierarchy().report();
                    crate::diagnostics::emit_solver_trace(&format!(
                        "FEREG_SOLVER_TRACE|phase=cmgschur_setup|degree={degree}|samples={}|seed={}|workers={}|retained_workers={}|sampled_workers={}|firms={}|vertices={}|input_edges={}|edges={}|components={}|threads={threads}|parallel_operators={}|batch_columns={}|levels={}|hierarchy_vertices={:?}|hierarchy_nonzeros={:?}|repeats={:?}|graph_seconds={graph_seconds:.9}|hierarchy_seconds={hierarchy_seconds:.9}|setup_seconds={:.9}",
                        sampling.samples, sampling.seed, membership.n_levels(), counts.retained_workers, counts.sampled_workers, retained,
                        shape.0, counts.input_edges, shape.1, preconditioner.components.sizes.len(),
                        preconditioner.plan.as_ref().map_or(0, cmg::ParallelCmgPlan::operator_count),
                        preconditioner.batch_columns, report.vertex_counts().len(),
                        report.vertex_counts(), report.matrix_nonzeros(), preconditioner.preconditioner.repeat_counts(),
                        started.elapsed().as_secs_f64(),
                    ));
                }
            }
            Ok(preconditioner)
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
    mut components: FirmComponents,
    cache_bytes: u128,
    install: Install,
    options: CmgOptions,
    work: &mut Work,
    memory: &mut Admission,
) -> Attempt<(CmgSchurPreconditioner, f64)> {
    let explicit = install.explicit();
    let options = cmg::BuildOptions {
        hierarchy: options,
        components: component_options(),
    };
    work.step(1024)?;
    let bound_for = |parallel| {
        CmgPreconditioner::construction_allocation_bound(
            graph.vertex_count(),
            graph.edge_count(),
            options,
            parallel,
        )
        .map_err(cmg_error)
    };
    // The synchronous cache stays reserved even after graph emission drops it.
    let mut parents = graph_storage_bound(&graph)? + components.bytes() + cache_bytes;
    // An explicit build always contracts on its host team, as before. The
    // automatic build uses the team when the parallel bound is admitted and
    // otherwise builds serially under the serial bound; both build the same
    // hierarchy, factor and repeats.
    let serial = bound_for(false)?;
    let team_build = explicit || {
        let parallel = bound_for(true)?;
        work.cmg(|team, _| Ok(team.is_some()))? && fits(memory, parents, parallel)
    };
    let bound = if team_build { bound_for(true)? } else { serial };
    let admitted = admit(memory, work, parents, bound)?;
    #[cfg(test)]
    if team_build && work.cmg(|team, _| Ok(team.is_some()))? {
        work.team_stages |= TEAM_HIERARCHY;
    }
    // The terminal factor follows the hierarchy options and memory admission.
    let started = Instant::now();
    let preconditioner = work.cmg(|team, cancelled| {
        CmgPreconditioner::build_with(
            &graph,
            options,
            cmg::BuildContext {
                team: team.filter(|_| team_build),
                cancel: Some(cancelled),
                budget_bytes: Some(admitted),
            },
        )
    })?;
    let hierarchy_seconds = started.elapsed().as_secs_f64();
    // Keep an already-admitted conservative hierarchy bound. Exact storage
    // reporting recursively deduplicates allocation owners and is quadratic.
    // The parallel bound's extra contraction slot bank is transient: it is
    // released by the joined build, so the automatic route keeps the serial
    // bound of the identical hierarchy (its accepted retained charge) and the
    // admitted peak keeps the parallel one. Explicit builds keep their bound.
    parents += if explicit { bound } else { serial } as u128;
    let component_count = components.sizes.len();
    let shape = ApplyShape {
        vertices: graph.vertex_count(),
        components: component_count,
        explicit,
    };
    let plan = install_plan(&preconditioner, install, shape, &mut parents, work, memory)?;
    // Layout scans are O(levels) and the zeroed workspaces O(n * columns);
    // all poll cancellation inside CMG.
    work.step(128)?;
    let scalar_bytes =
        work.cmg(|_, cancelled| preconditioner.workspace_allocation_bytes_controlled(cancelled))?;
    let mut columns = install.columns();
    let (mut batch_bytes, mut elements, mut bound) =
        shape.bound(&preconditioner, work, scalar_bytes, columns)?;
    if !explicit && columns > 1 && !fits(memory, parents, bound) {
        // Optional interleaved lanes of an automatic installation.
        columns = 1;
        (batch_bytes, elements, bound) =
            shape.bound(&preconditioner, work, scalar_bytes, columns)?;
    }
    admit(memory, work, parents, bound)?;
    if !explicit && columns > 1 {
        // The one-lane scratch stays charged in `parents`; the retained bound
        // keeps both, overstating it by one lane of component sums.
        components.sums = work.filled(component_count * columns, 0.0)?;
        components.corrections = work.filled(component_count * columns, 0.0)?;
    }
    let scalar_workspace =
        work.cmg(|_, cancelled| preconditioner.try_workspace_controlled(cancelled))?;
    let batch_workspace = if columns > 1 {
        Some(work.cmg(|_, cancelled| {
            CmgBatchWorkspace::try_new_controlled(&preconditioner, columns, batch_bytes, cancelled)
        })?)
    } else {
        None
    };
    let rhs = work.filled(elements, 0.0)?;
    let solution = work.filled(elements, 0.0)?;
    work.step(256)?;
    Ok((
        CmgSchurPreconditioner {
            preconditioner,
            install,
            plan,
            scalar_workspace,
            batch_workspace,
            rhs,
            solution,
            components,
            batch_columns: columns,
            construction: memory.peak,
            // Explicit installations report their exact retained storage.
            retained_bound: (!explicit).then_some(parents + bound as u128),
        },
        hierarchy_seconds,
    ))
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
    // Per worker: its first edge and its auxiliary vertex; one more entry
    // holds the total. Edge chunks locate their worker by binary search.
    offsets: Vec<(usize, usize)>,
    // Logical preparation tasks for sampled rows (zero without sampling).
    tasks: usize,
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
        // Locating an edge's worker probes one offset, then binary-searches
        // at most all worker offsets (a run of edge-less workers).
        let worker_steps = 1 + usize::BITS - cache.rows.len().leading_zeros();
        // 192 covers metadata, draw permutation, clamp, weight and validation;
        // 16 per possible binary-search step (row draw and worker lookup) and
        // clique-index step covers all bounded loop bodies. Include the cutoff
        // even outside public 4..=6. Workers poll cancellation before every
        // chunk of at most 1024 units.
        let Some(edge_work) = cutoff
            .checked_mul(16)
            .and_then(|units| units.checked_add(192 + 16 * (search_steps + worker_steps) as usize))
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
        let workers_count = cache.rows.len();
        let tasks: usize = if sampling.samples > 0 {
            let mut tasks = 0;
            let mut pending = 0;
            for worker in 0..workers_count {
                work.step(16)?;
                pending += cache.row(worker).len();
                if pending >= CENSUS_TASK_CELLS || worker + 1 == workers_count {
                    tasks += 1;
                    pending = 0;
                }
            }
            tasks
        } else {
            0
        };
        let Some(extra) = suffix_len
            .checked_mul(std::mem::size_of::<f64>())
            .and_then(|bytes| {
                bytes.checked_add(
                    workers_count
                        .checked_add(1)?
                        .checked_mul(std::mem::size_of::<(usize, usize)>())?,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(tasks.checked_mul(std::mem::size_of::<PrepareTask<'_>>())?)
            })
        else {
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
        #[cfg(test)]
        emission_probes::CHUNK.store(1024 / edge_work, std::sync::atomic::Ordering::SeqCst);
        Ok(Some(Self {
            suffix: work.filled(suffix_len, 0.0)?,
            offsets: work.capacity(workers_count + 1)?,
            tasks,
            chunk_len: 1024 / edge_work,
        }))
    }
}

/// Test-only record of offset probes per emitted edge; each edge counts its
/// probes plus one evaluation as instrumented work.
#[cfg(test)]
pub(super) mod emission_probes {
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub(super) static MAX: AtomicUsize = AtomicUsize::new(0);
    /// Edges per chunk of the most recently selected emission plan.
    pub(super) static CHUNK: AtomicUsize = AtomicUsize::new(0);

    pub(super) fn record(probes: usize) {
        MAX.fetch_max(probes, Ordering::SeqCst);
        crate::interrupt::test_polls::work(probes + 1);
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

    fn row(&self, worker: usize) -> &[(u32, f64)] {
        let start = self.rows[worker].0;
        let end = self
            .rows
            .get(worker + 1)
            .map_or(self.cells.len(), |row| row.0);
        &self.cells[start..end]
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
    let hierarchy = CmgPreconditioner::construction_allocation_bound(
        vertices,
        edges,
        cmg::BuildOptions {
            hierarchy: options,
            components: component_options(),
        },
        false,
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
        let count = self.source.row_count(worker);
        self.cells.clear();
        while self.cells.len() < count {
            let next = (count - self.cells.len()).min(128);
            work.step(8 * next as u16)?;
            self.cells.resize(self.cells.len() + next, (0, 0.0));
        }
        let (kept, total) = load_row(
            self.source,
            self.weight_scale,
            worker,
            &mut self.cells,
            work,
        )?;
        self.cells.truncate(kept);
        Ok(total)
    }

    #[allow(clippy::too_many_arguments)]
    fn census_bounded(
        &mut self,
        cutoff: usize,
        sampling: Sampling,
        labels: &mut [usize],
        work: &mut Work,
        memory: &mut Admission,
        parents: u128,
    ) -> Attempt<GraphCounts> {
        // With an admitted cache and the host team, rows load on the team; the
        // earliest failing worker's error is reported here in worker order.
        let mut team_rows = self.load_rows_on_team(work, memory, parents)?;
        let mut counts = GraphCounts::default();
        for worker in 0..self.source.worker_count() {
            match &mut team_rows {
                Some(failure) => {
                    if failure
                        .as_ref()
                        .is_some_and(|(failed, _)| *failed == worker)
                    {
                        let (_, error) = failure.take().expect("checked failure");
                        return Err(error.into());
                    }
                    work.step(32)?;
                }
                None => {
                    let total = self.load_bounded(worker, work)?;
                    if let Some(cache) = &mut self.cache {
                        cache.store(&self.cells, total, work)?;
                    }
                }
            }
            let cells = match (&team_rows, &self.cache) {
                (Some(_), Some(cache)) => cache.row(worker),
                _ => &self.cells[..],
            };
            work.step(64)?;
            let count = cells.len();
            if let Some(&(first, _)) = cells.first() {
                for &(firm, _) in cells.iter().skip(1) {
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
                    sampler.prepare(&mut self.cells, worker, work)?;
                    while let Some(edge) = sampler.next(&self.cells, work)? {
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

/// Load one worker's raw cells into `cells` (exactly its row length), sort them
/// by firm and weight, and merge each firm's weights with compensated sums in
/// that order. Returns the merged length and the worker's compensated total.
/// The serial census and team lanes share this one implementation.
fn load_row<C: Counter>(
    source: CellSource<'_>,
    weight_scale: f64,
    worker: usize,
    cells: &mut [(u32, f64)],
    counter: &mut C,
) -> std::result::Result<(usize, f64), C::Error> {
    counter.step(16)?;
    for (position, cell) in cells.iter_mut().enumerate() {
        counter.step(64)?;
        let (firm, raw_weight) = source.entry(worker, position);
        let weight = raw_weight / weight_scale;
        if firm as usize >= source.firm_count() {
            return Err(invalid_data("CMG Schur firm code is out of bounds").into());
        }
        if !weight.is_finite() || weight <= 0.0 {
            return Err(invalid_data("CMG Schur requires positive finite weights").into());
        }
        *cell = (firm, weight);
    }
    counted_sort(counter, cells, |left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
    })?;
    let mut read = 0;
    let mut write = 0;
    let mut total = 0.0;
    let mut total_correction = 0.0;
    while read < cells.len() {
        counter.step(32)?;
        let firm = cells[read].0;
        let mut sum = 0.0;
        let mut correction = 0.0;
        while read < cells.len() && cells[read].0 == firm {
            counter.step(32)?;
            neumaier_add(cells[read].1, &mut sum, &mut correction);
            read += 1;
        }
        counter.step(32)?;
        let weight = sum + correction;
        cells[write] = (firm, weight);
        neumaier_add(weight, &mut total, &mut total_correction);
        write += 1;
    }
    let total = total + total_correction;
    if !total.is_finite() || total <= 0.0 {
        return Err(invalid_data("CMG Schur worker weight total is invalid").into());
    }
    Ok((write, total))
}

/// Borrow the installed team for initial graph construction; above CMG's
/// hosted threshold the edges map and sort in fixed chunks on fereg's workers.
fn initial_laplacian_bounded(
    vertices: usize,
    edges: &[(usize, usize, f64)],
    admitted: usize,
    work: &mut Work,
) -> Attempt<Laplacian> {
    work.cmg(|team, cancelled| {
        Laplacian::build(
            vertices,
            edges,
            cmg::BuildContext {
                team,
                cancel: Some(cancelled),
                budget_bytes: Some(admitted),
            },
        )
    })
}

/// Replace union-find parents by dense component labels in first-appearance
/// order and count each component's firms. `sizes` enters as `usize::MAX`.
fn relabel_components(
    labels: &mut [usize],
    sizes: &mut Vec<usize>,
    work: &mut Work,
) -> Attempt<usize> {
    for firm in 0..labels.len() {
        work.step(8)?;
        labels[firm] = find_root(labels, firm, work)?;
    }
    let mut component_count = 0;
    for label in labels.iter_mut() {
        work.step(32)?;
        if sizes[*label] == usize::MAX {
            sizes[*label] = component_count;
            component_count += 1;
        }
        *label = sizes[*label];
    }
    sizes.truncate(component_count);
    for size in sizes.iter_mut() {
        work.step(8)?;
        *size = 0;
    }
    for &label in labels.iter() {
        work.step(8)?;
        sizes[label] += 1;
    }
    Ok(component_count)
}

#[allow(clippy::type_complexity)]
fn graph(
    source: CellSource<'_>,
    cutoff: usize,
    sampling: Sampling,
    cache_census: bool,
    install: Install,
    work: &mut Work,
    memory: &mut Admission,
) -> Attempt<(Laplacian, FirmComponents, f64, u128, GraphCounts)> {
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
        Some(SampledTrees::new(sampling, max_rows, work)?)
    } else {
        None
    };
    let mut labels = work.capacity(firms)?;
    for firm in 0..firms {
        work.step(8)?;
        labels.push(firm);
    }
    let mut sizes = work.filled(firms, usize::MAX)?;
    let census_parents = vec_capacity_bytes(&workers.cells)
        + cache_bytes as u128
        + sampler.as_ref().map_or(0, SampledTrees::bytes)
        + vec_capacity_bytes(&labels)
        + vec_capacity_bytes(&sizes);
    let counts =
        workers.census_bounded(cutoff, sampling, &mut labels, work, memory, census_parents)?;
    let component_count = relabel_components(&mut labels, &mut sizes, work)?;
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
    // An automatic installation widens its centering scratch only once its
    // optional interleaved lanes are admitted (see `finish`).
    let lanes = if install.explicit() {
        install.columns()
    } else {
        1
    };
    let component_elements = component_count
        .checked_mul(lanes)
        .ok_or_else(|| allocation("CMG Schur component batch dimensions overflow"))?;
    admit(
        memory,
        work,
        parents,
        bytes::<[f64; 2]>(component_elements)?,
    )?;
    let components = FirmComponents {
        labels,
        sizes,
        sums: work.filled(component_elements, 0.0)?,
        corrections: work.filled(component_elements, 0.0)?,
    };
    Ok((graph, components, weight_scale, cache_bytes as u128, counts))
}

/// The automatic route's hierarchy build, exactly as `finish` runs it.
#[cfg(test)]
fn automatic_hierarchy(
    graph: &Laplacian,
    cancel: cmg::Cancellation<'_>,
) -> std::result::Result<CmgPreconditioner, CmgError> {
    CmgPreconditioner::build_with(
        graph,
        cmg::BuildOptions {
            hierarchy: CmgOptions::default(),
            components: component_options(),
        },
        cmg::BuildContext {
            team: None,
            cancel: Some(cancel),
            budget_bytes: Some(usize::MAX),
        },
    )
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
                        .census_bounded(
                            5,
                            sampling,
                            &mut labels,
                            &mut work,
                            &mut Admission {
                                budget: None,
                                peak: SolverWorkspacePeak::ZERO,
                            },
                            0,
                        )
                        .unwrap();
                    assert!(count.input_edges >= HOSTED_EDGE_THRESHOLD);
                    let mut sampler = if samples > 0 {
                        Some(SampledTrees::new(sampling, 34, &mut work).unwrap())
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
                    let build = |graph: &Laplacian| automatic_hierarchy(graph, &never).unwrap();
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
                        .census_bounded(
                            5,
                            sampling,
                            &mut labels,
                            &mut work,
                            &mut Admission {
                                budget: None,
                                peak: SolverWorkspacePeak::ZERO,
                            },
                            0,
                        )
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
                    let (graph, components, scale, cache, _) = graph(
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
                        Install::AUTO,
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
                        .cmg(|_, cancelled| automatic_hierarchy(&graph, cancelled))
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
    fn trace_hierarchy_reports_stored_metadata_of_the_installed_build() {
        let (built, _) = candidate().unwrap();
        let installed = built.preconditioner.unwrap();
        let cmg = &installed.preconditioner;
        let report = cmg.hierarchy().report();
        let factor = cmg.terminal_factor().unwrap();
        assert_eq!(report.terminal_reason(), cmg::TerminalReason::Direct);
        let join = |values: &[usize]| {
            values
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(";")
        };
        assert_eq!(
            installed.trace_hierarchy(),
            format!(
                "terminal=Direct|levels={}|hierarchy_vertices={}|hierarchy_nonzeros={}|repeats={}|terminal_factor=1|terminal_factor_dimension={}|terminal_factor_nonzeros={}|application=scalar|parallel_operators=0|batch_columns=1",
                cmg.hierarchy().levels().len(),
                join(report.vertex_counts()),
                join(report.matrix_nonzeros()),
                join(cmg.repeat_counts()),
                factor.active_dimension(),
                factor.factor_nonzeros(),
            )
        );
        assert_eq!(report.vertex_counts().len(), cmg.hierarchy().levels().len());
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
            automatic_hierarchy(&graph, &crate::interrupt::cancellation_requested)
        });
        assert!(interrupted(result.map(drop)));
        assert_eq!(
            candidate().err().map(|error| error.code),
            Some(crate::error::ErrorCode::Interrupted)
        );
        crate::interrupt::reset_cancellation();

        // Scalar workspace preparation polls inside CMG's initialization of
        // its banks. Request host cancellation from inside that preparation,
        // at polls between its allocations, and map it exactly like a build.
        let reference = {
            let (built, scale) = candidate().unwrap();
            let mut built = built.preconditioner.unwrap();
            let mut rhs = [1.0, -1.0];
            built
                .apply_reduced_in_place(&mut rhs, &mut [0.0; 2], scale)
                .unwrap();
            rhs.map(f64::to_bits)
        };
        // 200,000 isolates: one level whose nine banks initialize 1.8 million
        // values at CMG's fixed counted cadence.
        let isolated = Laplacian::from_edges(200_000, []).unwrap();
        let large = CmgPreconditioner::build(&isolated, CmgOptions::default()).unwrap();
        let polls = std::sync::atomic::AtomicUsize::new(0);
        let prepare = |stop: Option<usize>| {
            polls.store(0, std::sync::atomic::Ordering::Relaxed);
            Work::new(false).cmg(|_, cancelled| {
                large.try_workspace_controlled(&|| {
                    let poll = polls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if stop == Some(poll) {
                        crate::interrupt::request_cancellation();
                    }
                    cancelled()
                })
            })
        };
        let complete = prepare(None).unwrap_or_else(|_| panic!("uncancelled workspace"));
        let total = polls.load(std::sync::atomic::Ordering::Relaxed);
        assert!(total >= 9 * 200_000 * 8 / 65_536);
        for stop in [1, total / 2, total - 2] {
            assert!(interrupted(prepare(Some(stop)).map(drop)));
            assert_eq!(polls.load(std::sync::atomic::Ordering::Relaxed), stop + 1);
            crate::interrupt::reset_cancellation();
        }
        let result = Work::new(false).cmg(|_, cancelled| {
            crate::interrupt::request_cancellation();
            large.workspace_allocation_bytes_controlled(cancelled)
        });
        assert!(interrupted(result.map(drop)));
        crate::interrupt::reset_cancellation();
        // After the interrupted attempts a fresh preparation and a fresh
        // bounded candidate (whose finish prepares its workspace the same
        // way) succeed and apply identical bits.
        let again = prepare(None).unwrap_or_else(|_| panic!("rebuilt workspace"));
        assert_eq!(again.byte_len(), complete.byte_len());
        assert_eq!(again.dimensions(), complete.dimensions());
        let (built, scale) = candidate().unwrap();
        let mut built = built.preconditioner.unwrap();
        let mut rhs = [1.0, -1.0];
        built
            .apply_reduced_in_place(&mut rhs, &mut [0.0; 2], scale)
            .unwrap();
        assert_eq!(rhs.map(f64::to_bits), reference);

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
                    AutoApplication::Scalar,
                    CmgOptions::default(),
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
                    Install::AUTO,
                    CmgOptions::default(),
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
        let mut sampler = completed(SampledTrees::new(sampling, 72, &mut work));
        for (worker, (canonical, total)) in expected.iter().enumerate() {
            assert_eq!(
                completed(cache.load(worker, &mut rows.cells, &mut work)).to_bits(),
                *total
            );
            assert_eq!(&rows.cells, canonical);
            if canonical.len() > 5 {
                completed(sampler.prepare(&mut rows.cells, worker, &mut work));
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
                Install::AUTO,
                &mut work,
                &mut memory,
            ))
        };
        let (old, old_components, old_scale, _, _) = build(false);
        let (new, new_components, new_scale, cache_bytes, _) = build(true);
        assert_eq!(old, new);
        assert_eq!(old_components.labels, new_components.labels);
        assert_eq!(old_components.sizes, new_components.sizes);
        assert_eq!(old_scale.to_bits(), new_scale.to_bits());
        assert!(cache_bytes > 0);
    }
}

/// Team construction stages (census rows, sampled-row preparation, edge
/// offsets and the automatic hierarchy) reproduce the serial graph, cache,
/// hierarchy and applications bit for bit at every worker count; lanes count
/// inner work inside one long row; the earliest failing worker's error is the
/// serial error; and refused optional scratch selects the serial path.
#[cfg(test)]
mod team_stage_tests {
    use super::team::{LaneStop, LaneWork, TEAM_STAGE_ITEMS};
    use super::*;
    use crate::{options::ThreadConfig, parallel::ThreadTeam};

    /// About 100,000 observations: varied rows, duplicate cells, one 60,000
    /// cell worker and enough sampled rows for the hosted emission plan.
    struct Fixture {
        workers: Vec<u32>,
        firms: Vec<u32>,
        weights: Vec<f64>,
        membership: LevelIndex,
    }

    impl Fixture {
        fn new(bad: &[(usize, f64)]) -> Self {
            let (count, firms_total) = (3000, 500);
            let mut workers = Vec::new();
            let mut firms = Vec::new();
            let mut weights = Vec::new();
            for worker in 0..count {
                let row = if worker == 7 {
                    60_000
                } else {
                    [3, 5, 6, 17, 40][worker % 5]
                };
                for position in 0..row {
                    workers.push(worker as u32);
                    firms.push(((worker * 13 + position * 7 + position / 3) % firms_total) as u32);
                    weights.push([1.0, 0.5, 2.0, 1.0e-3][(position + worker) % 4]);
                }
            }
            for &(observation, value) in bad {
                weights[observation] = value;
            }
            let membership = LevelIndex::from_codes(&workers, count).unwrap();
            Self {
                workers,
                firms,
                weights,
                membership,
            }
        }

        /// 70,000 emitting workers (two firms, one clique edge each) around a
        /// run of 100,000 duplicate-only workers (two observations at one
        /// firm: canonical degree one, no edges). Edges 34,999 and 35,000
        /// are adjacent in emission order but 100,000 workers apart.
        fn gap() -> Self {
            let (before, gap, after) = (35_000, 100_000, 35_000);
            let count = before + gap + after;
            let mut workers = Vec::new();
            let mut firms = Vec::new();
            let mut weights = Vec::new();
            for worker in 0..count {
                let emitting = worker < before || worker >= before + gap;
                for position in 0..2 {
                    workers.push(worker as u32);
                    let firm = worker * 7 + usize::from(emitting) * position;
                    firms.push((firm % 500) as u32);
                    weights.push([1.0, 0.5][position]);
                }
            }
            let membership = LevelIndex::from_codes(&workers, count).unwrap();
            Self {
                workers,
                firms,
                weights,
                membership,
            }
        }

        fn source(&self) -> CellSource<'_> {
            CellSource::Observations {
                membership: &self.membership,
                firms: &self.firms,
                retained: 500,
                weights: BorrowedMapWeights::Provided(&self.weights),
            }
        }

        fn first_observation_of(&self, worker: u32) -> usize {
            self.workers.iter().position(|&w| w == worker).unwrap()
        }
    }

    fn on(lanes: Option<usize>, operation: impl FnOnce() -> GraphResult + Send) -> GraphResult {
        match lanes {
            Some(lanes) => ThreadTeam::new(ThreadConfig::from_requested(lanes, true).unwrap())
                .install(operation)
                .unwrap(),
            None => operation(),
        }
    }

    type GraphResult = std::result::Result<GraphBits, String>;

    #[derive(Debug)]
    struct GraphBits {
        /// The team stages taken: a path, not part of the result.
        stages: u8,
        edges: Vec<(usize, usize, u64)>,
        diagonal: Vec<u64>,
        labels: Vec<usize>,
        sizes: Vec<usize>,
        counts: (usize, usize, usize),
        cache: u128,
    }

    impl PartialEq for GraphBits {
        fn eq(&self, other: &Self) -> bool {
            (
                &self.edges,
                &self.diagonal,
                &self.labels,
                &self.sizes,
                self.counts,
                self.cache,
            ) == (
                &other.edges,
                &other.diagonal,
                &other.labels,
                &other.sizes,
                other.counts,
                other.cache,
            )
        }
    }

    fn graph_bits(
        fixture: &Fixture,
        sampling: Sampling,
        budget: Option<FeSolverMemoryBudget>,
    ) -> GraphResult {
        let mut work = Work::new(false);
        let mut memory = Admission {
            budget,
            peak: SolverWorkspacePeak::ZERO,
        };
        match graph(
            fixture.source(),
            5,
            sampling,
            true,
            Install::AUTO,
            &mut work,
            &mut memory,
        ) {
            Ok((graph, components, _, cache, counts)) => Ok(GraphBits {
                stages: work.team_stages,
                edges: graph
                    .edges()
                    .iter()
                    .map(|edge| (edge.u(), edge.v(), edge.weight().to_bits()))
                    .collect(),
                diagonal: graph.diagonal().iter().map(|d| d.to_bits()).collect(),
                labels: components.labels,
                sizes: components.sizes,
                counts: (
                    counts.input_edges,
                    counts.sampled_workers,
                    counts.retained_workers,
                ),
                cache,
            }),
            Err(AttemptError::Fatal(error)) => Err(error.message),
            Err(AttemptError::Refused(refusal)) => Err(format!("{refusal:?}")),
        }
    }

    #[test]
    fn team_census_and_emission_match_the_serial_graph_at_every_worker_count() {
        let fixture = Fixture::new(&[]);
        assert!(fixture.membership.nobs() >= TEAM_STAGE_ITEMS);
        for samples in [0, 1, 3] {
            let sampling = Sampling { samples, seed: 0 };
            let serial = on(None, || graph_bits(&fixture, sampling, None)).unwrap();
            assert_eq!(serial.stages, 0);
            assert!(serial.cache > 0, "the canonical cache is admitted");
            let hosted = serial.counts.0 >= HOSTED_EDGE_THRESHOLD;
            assert_eq!(hosted, samples == 3, "{:?}", serial.counts);
            for lanes in [1, 2, 4, 8] {
                let team = on(Some(lanes), || graph_bits(&fixture, sampling, None)).unwrap();
                assert_eq!(team, serial, "samples {samples}, lanes {lanes}");
                let prepared = if hosted { TEAM_PREPARE } else { 0 };
                assert_eq!(team.stages, TEAM_CENSUS | prepared, "lanes {lanes}");
            }
        }
    }

    #[test]
    fn earliest_failing_worker_error_matches_the_serial_census() {
        let probe = Fixture::new(&[]);
        let late = probe.first_observation_of(2900) + 1;
        let early = probe.first_observation_of(40) + 2;
        let giant = probe.first_observation_of(7) + 59_000;
        // The prologue rejects nonpositive or nonfinite raw weights before
        // the census. The census itself rejects an out-of-range firm and a
        // positive weight that underflows after scaling (the largest weight
        // is 2). Workers in different census tasks fail with these different
        // messages, so only the earliest failing worker reproduces the serial
        // error.
        let tiny = f64::from_bits(1);
        for (bad, firm) in [
            (vec![(late, f64::NAN), (early, -1.0)], None),
            (vec![(late, tiny)], Some(early)),
            (vec![(early, tiny)], Some(late)),
            (vec![(late, tiny)], Some(giant)),
            (vec![(giant, tiny)], Some(late)),
        ] {
            let mut fixture = Fixture::new(&bad);
            if let Some(observation) = firm {
                fixture.firms[observation] = 500;
            }
            let sampling = Sampling {
                samples: 1,
                seed: 0,
            };
            let serial = on(None, || graph_bits(&fixture, sampling, None)).unwrap_err();
            if firm.is_some() {
                let expected = if bad[0].0 < firm.unwrap() {
                    "positive finite weights"
                } else {
                    "firm code is out of bounds"
                };
                assert!(serial.contains(expected), "{serial}");
            }
            for lanes in [1, 2, 4] {
                let team = on(Some(lanes), || graph_bits(&fixture, sampling, None)).unwrap_err();
                assert_eq!(team, serial, "lanes {lanes}");
            }
        }
    }

    #[test]
    fn lane_counters_poll_inside_one_long_row_and_stop_there() {
        let fixture = Fixture::new(&[]);
        let source = fixture.source();
        let count = source.row_count(7);
        assert_eq!(count, 60_000);
        // 64 units per loaded cell: one poll interval covers 1,024 cells.
        for stop in [1, 2, 10] {
            let calls = std::sync::atomic::AtomicUsize::new(0);
            let query = || calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 >= stop;
            let mut cells = vec![(u32::MAX, f64::NAN); count];
            let mut lane = LaneWork::new(&query);
            let result = load_row(source, 1.0, 7, &mut cells, &mut lane);
            assert!(matches!(result, Err(LaneStop::Cancelled)));
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), stop);
            let loaded = cells.iter().take_while(|cell| cell.0 != u32::MAX).count();
            // The lane stopped inside the row, within one interval of its poll.
            assert!(
                loaded <= stop * 1024,
                "{loaded} cells loaded by poll {stop}"
            );
            assert!(loaded + 1024 > stop * 1024 - 1024, "{loaded}");
        }
        let mut cells = vec![(0, 0.0); count];
        let mut serial = vec![(0, 0.0); count];
        let never = || false;
        let team = load_row(source, 1.0, 7, &mut cells, &mut LaneWork::new(&never)).ok();
        let expected = load_row(source, 1.0, 7, &mut serial, &mut Work::new(false)).ok();
        assert_eq!(
            team.map(|(n, t)| (n, t.to_bits())),
            expected.map(|(n, t)| (n, t.to_bits()))
        );
        assert_eq!(cells, serial);
    }

    /// Optional team scratch is admitted per stage. A budget of exactly the
    /// cache envelope admits the cache (and with it the small census task
    /// table, which the complete envelope covers) but not the emission plan's
    /// extra offsets and suffix banks, so emission runs serially; a budget
    /// below the envelope admits no cache, so the census runs serially too.
    /// Every path builds the same graph.
    #[test]
    fn refused_optional_scratch_selects_the_serial_stage_with_identical_bits() {
        let fixture = Fixture::new(&[]);
        let sampling = Sampling {
            samples: 3,
            seed: 0,
        };
        let serial = on(None, || graph_bits(&fixture, sampling, None)).unwrap();
        let envelope =
            cache_envelope(fixture.membership.nobs(), 3000, 500, 60_000, 5, sampling).unwrap();
        let budget = |bytes: u128| {
            Some(FeSolverMemoryBudget {
                usable_bytes: bytes + 913,
                execution_bytes: 913,
            })
        };
        let exact = budget(envelope.peak_bytes as u128);
        let tight = on(Some(4), || graph_bits(&fixture, sampling, exact)).unwrap();
        assert_eq!(tight, serial);
        assert_eq!(tight.stages, TEAM_CENSUS, "emission scratch is refused");
        let below = budget(envelope.peak_bytes as u128 - 1);
        let uncached_serial = on(None, || graph_bits(&fixture, sampling, below)).unwrap();
        let uncached = on(Some(4), || graph_bits(&fixture, sampling, below)).unwrap();
        assert_eq!(uncached.cache, 0, "no cache is admitted");
        assert_eq!(uncached, uncached_serial);
        assert_eq!(uncached.stages, 0, "without a cache every stage is serial");
        assert_eq!(
            (&uncached.edges, &uncached.diagonal, &uncached.labels),
            (&serial.edges, &serial.diagonal, &serial.labels)
        );
    }

    /// The automatic route builds its hierarchy on the team when the parallel
    /// bound is admitted, and serially when only the serial bound is: same
    /// hierarchy, factor, repeats and applications on every path.
    #[test]
    fn automatic_hierarchy_on_the_team_equals_the_serial_hierarchy() {
        let fixture = Fixture::new(&[]);
        let sampling = Sampling {
            samples: 1,
            seed: 0,
        };
        type Built = (CmgSchurPreconditioner, u8);
        let build = |lanes: Option<usize>, tight: bool| -> Built {
            let run = || {
                let mut work = Work::new(false);
                let mut memory = Admission {
                    budget: None,
                    peak: SolverWorkspacePeak::ZERO,
                };
                let (graph, components, _, cache, _) = graph(
                    fixture.source(),
                    5,
                    sampling,
                    true,
                    Install::AUTO,
                    &mut work,
                    &mut memory,
                )
                .unwrap();
                let options = cmg::BuildOptions {
                    hierarchy: CmgOptions::default(),
                    components: component_options(),
                };
                let parents = graph_storage_bound(&graph).unwrap() + components.bytes() + cache;
                let serial = CmgPreconditioner::construction_allocation_bound(
                    graph.vertex_count(),
                    graph.edge_count(),
                    options,
                    false,
                )
                .unwrap();
                let parallel = CmgPreconditioner::construction_allocation_bound(
                    graph.vertex_count(),
                    graph.edge_count(),
                    options,
                    true,
                )
                .unwrap();
                assert!(parallel > serial);
                // Admit the serial hierarchy bound but not the parallel one.
                // Later workspaces are admitted separately against parents.
                let mut memory = Admission {
                    budget: tight.then(|| FeSolverMemoryBudget {
                        usable_bytes: parents + serial as u128 + 913 + (1 << 30),
                        execution_bytes: 913 + (1 << 30) - (parallel - serial) as u128 / 2,
                    }),
                    peak: SolverWorkspacePeak::ZERO,
                };
                let stages = work.team_stages & !TEAM_CENSUS & !TEAM_PREPARE;
                let (built, _) = finish(
                    graph,
                    components,
                    cache,
                    Install::AUTO,
                    CmgOptions::default(),
                    &mut work,
                    &mut memory,
                )
                .unwrap();
                (built, work.team_stages & TEAM_HIERARCHY | stages)
            };
            match lanes {
                Some(lanes) => ThreadTeam::new(ThreadConfig::from_requested(lanes, true).unwrap())
                    .install(run)
                    .unwrap(),
                None => run(),
            }
        };
        let (mut serial, stages) = build(None, false);
        assert_eq!(stages, 0);
        let firms = serial.components.labels.len();
        let rhs: Vec<f64> = (0..firms).map(|i| (i % 11) as f64 - 5.0).collect();
        let mut expected = vec![0.0; firms];
        serial
            .apply_batch(&rhs, &mut expected, 1, 1, None, None)
            .unwrap();
        for (lanes, tight) in [(1, false), (2, false), (4, false), (4, true)] {
            let (mut team, stages) = build(Some(lanes), tight);
            let hierarchy = if tight { 0 } else { TEAM_HIERARCHY };
            assert_eq!(stages, hierarchy, "lanes {lanes}, tight {tight}");
            assert_eq!(
                team.preconditioner.hierarchy(),
                serial.preconditioner.hierarchy()
            );
            assert_eq!(
                team.preconditioner.repeat_counts(),
                serial.preconditioner.repeat_counts()
            );
            assert_eq!(team.components.labels, serial.components.labels);
            let mut actual = vec![0.0; firms];
            team.apply_batch(&rhs, &mut actual, 1, 1, None, None)
                .unwrap();
            assert_eq!(
                actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                "lanes {lanes}, tight {tight}"
            );
        }
    }

    /// The automatic route's persistent charge after a team hierarchy build is
    /// the accepted serial one. The parallel bound's extra contraction slot
    /// bank (D) is transient: it bounds the construction peak only. With live
    /// parents P, serial bound S and completion storage W (derived from the
    /// accepted serial result), a budget of P + S + max(D, W) admits the team
    /// build and its completion. Both sides of the parallel admission boundary
    /// install with identical retained accounting, and budgets below P + S + W
    /// or P + S are refused at every worker count. Hierarchies, repeats and
    /// application bits are identical throughout.
    #[test]
    fn automatic_completion_keeps_the_serial_retained_charge_after_a_team_build() {
        let fixture = Fixture::new(&[]);
        let sampling = Sampling {
            samples: 1,
            seed: 0,
        };
        type Built = (CmgSchurPreconditioner, u8, u128, u128, u128);
        // Returns the build, its hierarchy stage, and (P, S, D).
        let build = |lanes: Option<usize>, available: Option<u128>| -> Attempt<Built> {
            let run = || {
                let mut work = Work::new(false);
                let mut memory = Admission {
                    budget: None,
                    peak: SolverWorkspacePeak::ZERO,
                };
                let (graph, components, _, cache, _) = graph(
                    fixture.source(),
                    5,
                    sampling,
                    true,
                    Install::AUTO,
                    &mut work,
                    &mut memory,
                )?;
                let options = cmg::BuildOptions {
                    hierarchy: CmgOptions::default(),
                    components: component_options(),
                };
                let parents = graph_storage_bound(&graph)? + components.bytes() + cache;
                let bound = |parallel| {
                    CmgPreconditioner::construction_allocation_bound(
                        graph.vertex_count(),
                        graph.edge_count(),
                        options,
                        parallel,
                    )
                    .unwrap() as u128
                };
                let (serial, parallel) = (bound(false), bound(true));
                let execution = 913 + (1 << 30);
                let mut memory = Admission {
                    budget: available.map(|available| FeSolverMemoryBudget {
                        usable_bytes: available + execution,
                        execution_bytes: execution,
                    }),
                    peak: SolverWorkspacePeak::ZERO,
                };
                work.team_stages = 0;
                let (built, _) = finish(
                    graph,
                    components,
                    cache,
                    Install::AUTO,
                    CmgOptions::default(),
                    &mut work,
                    &mut memory,
                )?;
                Ok((
                    built,
                    work.team_stages & TEAM_HIERARCHY,
                    parents,
                    serial,
                    parallel - serial,
                ))
            };
            match lanes {
                Some(lanes) => ThreadTeam::new(ThreadConfig::from_requested(lanes, true).unwrap())
                    .install(run)
                    .unwrap(),
                None => run(),
            }
        };
        let peak = |bytes: u128| {
            SolverWorkspacePeak::from_bytes(bytes)
                .unwrap()
                .with_expected_heap(Some(0))
                .unwrap()
        };
        let Ok((mut accepted, stages, p, s, d)) = build(None, None) else {
            panic!("the unlimited serial build installs");
        };
        assert_eq!(stages, 0);
        let retained = accepted.retained_bound.expect("automatic retained charge");
        let w = retained - p - s;
        assert!(d > 0 && w > 0, "D {d}, W {w}");
        let firms = accepted.components.labels.len();
        let rhs: Vec<f64> = (0..firms).map(|i| (i % 11) as f64 - 5.0).collect();
        let mut expected = vec![0.0; firms];
        accepted
            .apply_batch(&rhs, &mut expected, 1, 1, None, None)
            .unwrap();
        let expected: Vec<u64> = expected.iter().map(|x| x.to_bits()).collect();
        // (available, whether the team hierarchy is selected with a team).
        // When W >= D, P + S + D - 1 is below P + S + W: a true refusal below.
        let mut installs = vec![(None, true), (Some(p + s + d.max(w)), true)];
        if w < d {
            installs.push((Some(p + s + d), true));
            installs.push((Some(p + s + d - 1), false));
        }
        for (available, parallel) in installs {
            for lanes in [None, Some(1), Some(4)] {
                let label = format!("available {available:?}, lanes {lanes:?}");
                let (mut built, stages, ..) = match build(lanes, available) {
                    Ok(built) => built,
                    Err(error) => panic!("{label}: {error:?}"),
                };
                let team = lanes.is_some() && parallel;
                assert_eq!(stages, if team { TEAM_HIERARCHY } else { 0 }, "{label}");
                assert_eq!(built.retained_bound, Some(retained), "{label}");
                // The admitted construction peak keeps the real parallel peak.
                let top = if team { p + s + d.max(w) } else { p + s + w };
                assert_eq!(built.construction, peak(top), "{label}");
                assert_eq!(
                    built.preconditioner.hierarchy(),
                    accepted.preconditioner.hierarchy(),
                    "{label}"
                );
                assert_eq!(
                    built.preconditioner.repeat_counts(),
                    accepted.preconditioner.repeat_counts(),
                    "{label}"
                );
                let mut actual = vec![0.0; firms];
                built
                    .apply_batch(&rhs, &mut actual, 1, 1, None, None)
                    .unwrap();
                let actual: Vec<u64> = actual.iter().map(|x| x.to_bits()).collect();
                assert_eq!(actual, expected, "{label}");
            }
        }
        for available in [p + s + w - 1, p + s - 1] {
            for lanes in [None, Some(1), Some(4)] {
                assert!(
                    matches!(
                        build(lanes, Some(available)),
                        Err(AttemptError::Refused(BuildRefusal::MemoryBudget))
                    ),
                    "available {available}, lanes {lanes:?}"
                );
            }
        }
    }

    /// Host cancellation raised at sampled polls of a teamed graph build (a
    /// child process: the poll seam is process-global) is interrupted from
    /// inside the team census and the team row preparation; every lane has
    /// joined when the build returns, and a fresh build repeats the bits.
    #[test]
    fn team_stages_cancel_inside_their_lanes_and_join() {
        const CHILD: &str = "FEREG_TEAM_STAGE_CANCEL_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD, "1")
                .args([
                    "fe::cmg_schur::bounded::team_stage_tests::team_stages_cancel_inside_their_lanes_and_join",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        use crate::interrupt::test_polls;
        let fixture = Fixture::new(&[]);
        let sampling = Sampling {
            samples: 3,
            seed: 0,
        };
        let team = ThreadTeam::new(ThreadConfig::from_requested(4, true).unwrap());
        let run = || {
            team.install(|| graph_bits(&fixture, sampling, None))
                .unwrap()
        };
        test_polls::cancel_at(None);
        let reference = run().unwrap();
        assert_eq!(reference.stages, TEAM_CENSUS | TEAM_PREPARE);
        let total = test_polls::polls();
        assert!(total > 200, "{total} host polls");
        let mut phases = std::collections::BTreeSet::new();
        // The team census and row preparation come first: sample their
        // window densely, then the rest of the build sparsely.
        let early = (0..total.min(3000)).step_by(10);
        for stop in early.chain((0..total).step_by(total / 150)) {
            test_polls::cancel_at(Some(stop));
            match run() {
                Err(message) => {
                    assert!(
                        message.contains("interrupt") || message.contains("cancelled"),
                        "{message}"
                    );
                    phases.insert(
                        message
                            .split_once("cancelled during ")
                            .map_or("fereg", |(_, phase)| phase)
                            .to_string(),
                    );
                }
                Ok(_) => assert!(test_polls::polls() <= stop, "poll {stop} ignored"),
            }
            let settled = test_polls::polls();
            std::thread::sleep(std::time::Duration::from_millis(5));
            assert_eq!(
                test_polls::polls(),
                settled,
                "a lane polled after the build returned"
            );
        }
        for phase in ["CMG Schur team census", "CMG Schur sampled-row preparation"] {
            assert!(phases.contains(phase), "{phase} not in {phases:?}");
        }
        test_polls::cancel_at(None);
        assert_eq!(run().unwrap(), reference);
    }
    /// Hosted emission across a long run of zero-edge workers (a child
    /// process: the poll and work seams are process-global). Each edge's
    /// worker lookup probes at most one offset plus a binary search over the
    /// workers. Emission is the only instrumented stage, so a stop whose
    /// recorded work lies strictly inside it was observed by the emission
    /// stage. After that observation each lane finishes at most its current
    /// chunk, untouched edges stay unevaluated, every lane has joined when the
    /// build returns, and a fresh build repeats the bits. No team, one and
    /// four lanes emit identical graphs.
    #[test]
    fn emission_lookup_across_zero_edge_workers_is_bounded_and_cancellable() {
        const CHILD: &str = "FEREG_EMISSION_GAP_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD, "1")
                .args([
                    "fe::cmg_schur::bounded::team_stage_tests::emission_lookup_across_zero_edge_workers_is_bounded_and_cancellable",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        use super::emission_probes;
        use crate::interrupt::test_polls;
        use std::sync::atomic::Ordering;
        // Leading, internal and trailing zero-edge workers, plus several
        // edges per emitting worker: fixed expected owners independent of the
        // lookup implementation, starting a chunk at every edge.
        let offsets = [0, 0, 2, 2, 2, 5, 5, 7, 7].map(|start| (start, 0));
        let mut previous = 0;
        for (edge, expected) in [1, 1, 4, 4, 4, 6, 6].into_iter().enumerate() {
            assert_eq!(super::team::edge_worker(&offsets, 0, edge), expected);
            previous = super::team::edge_worker(&offsets, previous, edge);
            assert_eq!(previous, expected);
        }
        let mut gap_offsets = vec![(1, 0); 100_003];
        gap_offsets[0].0 = 0;
        gap_offsets.last_mut().unwrap().0 = 2;
        emission_probes::MAX.store(0, Ordering::SeqCst);
        assert_eq!(super::team::edge_worker(&gap_offsets, 0, 1), 100_001);
        assert!(emission_probes::MAX.load(Ordering::SeqCst) <= 18);
        let fixture = Fixture::gap();
        let workers = fixture.membership.n_levels();
        // One fast-path probe plus a binary search over at most `workers`
        // offsets.
        let probe_bound = 1 + (usize::BITS - workers.leading_zeros()) as usize;
        let sampling = Sampling {
            samples: 0,
            seed: 0,
        };
        let serial = on(None, || graph_bits(&fixture, sampling, None)).unwrap();
        assert!(
            serial.counts.0 >= HOSTED_EDGE_THRESHOLD,
            "{:?}",
            serial.counts
        );
        for lanes in [1, 4] {
            test_polls::cancel_at(None);
            emission_probes::MAX.store(0, Ordering::SeqCst);
            let team = on(Some(lanes), || graph_bits(&fixture, sampling, None)).unwrap();
            assert_eq!(team, serial, "lanes {lanes}");
            assert_eq!(team.stages, TEAM_CENSUS, "lanes {lanes}");
            let probes = emission_probes::MAX.load(Ordering::SeqCst);
            assert!(probes > 1, "the gap fixture must exercise worker bisection");
            assert!(
                probes <= probe_bound,
                "lanes {lanes}: one edge probed {probes} worker offsets (bound {probe_bound})"
            );
            // The declared chunk covers its actual worst edge: 192 fixed
            // units, 16 per clique step (cutoff 5), per row-draw step (rows of
            // two: one) and per probed worker offset.
            let chunk = emission_probes::CHUNK.load(Ordering::SeqCst);
            let worst = chunk * (192 + 16 * 5 + 16 * (1 + probes));
            assert!(
                chunk > 0 && worst <= 1024,
                "lanes {lanes}: {chunk} edges per chunk need {worst} units"
            );
        }
        let lanes = 4;
        let team = ThreadTeam::new(ThreadConfig::from_requested(lanes, true).unwrap());
        let run = || {
            team.install(|| graph_bits(&fixture, sampling, None))
                .unwrap()
        };
        test_polls::cancel_at(None);
        let reference = run().unwrap();
        let total_polls = test_polls::polls();
        let (total_work, _) = test_polls::work_done();
        assert!(total_work > serial.counts.0, "{total_work} units");
        // A chunk holds at most 1024 / 288 = 3 edges (the smallest declared
        // per-edge work), each one evaluation plus its probes.
        let chunk_work = 3 * (1 + probe_bound);
        let mut inside = 0;
        for stop in (0..total_polls).step_by((total_polls / 300).max(1)) {
            test_polls::cancel_at(Some(stop));
            let result = run();
            let (work, at) = test_polls::work_done();
            let settled = test_polls::polls();
            std::thread::sleep(std::time::Duration::from_millis(5));
            assert_eq!(test_polls::polls(), settled, "a lane polled after return");
            let Some(at) = at.filter(|&at| at > 0 && at < total_work) else {
                continue;
            };
            inside += 1;
            let message = result.unwrap_err();
            assert!(message.contains("cancelled"), "{message}");
            assert!(
                work - at <= lanes * chunk_work,
                "stop {stop}: {} units after the observation",
                work - at
            );
            assert!(work < total_work, "stop {stop}: every edge evaluated");
        }
        assert!(inside >= 10, "only {inside} stops inside emission");
        test_polls::cancel_at(None);
        assert_eq!(run().unwrap(), reference);
    }
}
