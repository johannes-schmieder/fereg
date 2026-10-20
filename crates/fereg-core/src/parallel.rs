use crate::error::{ErrorCode, FeregError, Result};
use crate::options::ThreadConfig;
use rayon::prelude::*;
use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// Scheduling grain in scalar operations, independent of estimator and model.
/// Arithmetic leaves and their ordered merges remain owned by each kernel.
const MIN_LANE_WORK: u128 = 16_384;

#[derive(Clone, Copy, Debug)]
pub struct StagePlan {
    /// Ceiling on concurrently executable tasks; kernels may expose fewer.
    pub lanes: usize,
}

impl StagePlan {
    pub fn new(work: u128, tasks: usize, budget: usize) -> Self {
        let useful = work.div_ceil(MIN_LANE_WORK).min(usize::MAX as u128) as usize;
        Self {
            lanes: budget.min(tasks).min(useful).max(1),
        }
    }

    pub(crate) fn current(work: u128, tasks: usize) -> Self {
        Self::new(work, tasks, rayon::current_num_threads())
    }

    pub fn chunk(self, items: usize) -> usize {
        items.div_ceil(self.lanes).max(1)
    }

    pub(crate) fn groups_mut<T: Send>(
        self,
        values: &mut [T],
        stride: usize,
        offsets: &[usize],
        function: impl Fn(usize, &mut [T]) -> Result<()> + Sync,
    ) -> Result<()> {
        let mut contexts = vec![(); self.lanes];
        group_blocks(
            GroupBuffers {
                first: values,
                first_stride: stride,
                second: &mut [] as &mut [()],
                second_stride: 0,
                scratch: &mut contexts,
            },
            offsets,
            0,
            &|first, values, _, _| function(first, values),
        )
    }

    pub(crate) fn trace(self, stage: &str, tasks: usize) {
        if crate::diagnostics::solver_trace_enabled() {
            eprintln!("FEREG_SOLVER_TRACE|phase=stage_plan|stage={stage}|budget={}|lane_limit={}|independent_items={tasks}", rayon::current_num_threads(), self.lanes);
        }
    }
}

fn reserved_initialization_plan(elements: usize) -> StagePlan {
    let workers = rayon::current_thread_index().map_or(1, |_| rayon::current_num_threads());
    StagePlan::new(elements as u128, elements, workers)
}

/// Scan pure row predicates on an installed team without creating a global
/// pool. A failed shortcut leaves its caller's ordered diagnostics authoritative.
pub(crate) fn all_row_chunks(
    rows: usize,
    work: u128,
    predicate: impl Fn(std::ops::Range<usize>) -> bool + Sync,
) -> bool {
    let workers = rayon::current_thread_index().map_or(1, |_| rayon::current_num_threads());
    let plan = StagePlan::new(work, rows, workers);
    if plan.lanes <= 1 {
        return predicate(0..rows);
    }
    let chunk = plan.chunk(rows);
    (0..rows.div_ceil(chunk)).into_par_iter().all(|block| {
        let start = block * chunk;
        predicate(start..(start + chunk).min(rows))
    })
}

/// Initialize an empty, already reserved numeric bank without reallocating.
/// The owner retains its fallible reservation and admission checks. Parallel
/// first-touch uses only an installed team; no global pool is initialized.
pub(crate) fn initialize_reserved<T: Copy + Send + Sync>(
    output: &mut Vec<T>,
    elements: usize,
    value: T,
) {
    debug_assert!(output.is_empty() && output.capacity() >= elements);
    let plan = reserved_initialization_plan(elements);
    if plan.lanes > 1 {
        // Indexed collection owns initialization safely. The minimum leaf
        // length caps concurrent leaves at the admitted stage lanes, though
        // binary splitting can expose fewer lanes for awkward lengths.
        (0..elements)
            .into_par_iter()
            .with_min_len(plan.chunk(elements))
            .map(|_| value)
            .collect_into_vec(output);
    } else {
        output.resize(elements, value);
    }
}

/// Fixed leaves determine numerical order; the partial target bounds their
/// count but is not hard admission. Existing kernels always allow one partial.
pub(crate) fn reduction_blocks(
    items: usize,
    grain: usize,
    maximum: usize,
    stride: usize,
    target_bytes: usize,
) -> Result<usize> {
    if maximum == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "reduction block policy must allow at least one block",
        ));
    }
    let bytes = stride
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "reduction partial bytes overflow")
        })?;
    Ok(items
        .div_ceil(grain)
        .clamp(1, maximum)
        .min((target_bytes / bytes.max(1)).max(1)))
}

/// Partition indexed work into the planned lanes. Rayon's minimum-length hint
/// is not a partition: binary splitting can leave half the planned lanes idle.
/// Kernels retain their item indices, arithmetic leaves and ordered merges.
pub(crate) fn stage_try_for_each<I, F>(iterator: I, work: u128, function: F) -> Result<()>
where
    I: IndexedParallelIterator,
    F: Fn(I::Item) -> Result<()> + Sync,
{
    let items = iterator.len();
    let lanes = StagePlan::current(work, items).lanes;
    iterator.with_producer(StageCallback {
        items,
        lanes,
        function,
    })
}

struct StageCallback<F> {
    items: usize,
    lanes: usize,
    function: F,
}

impl<T, F> rayon::iter::plumbing::ProducerCallback<T> for StageCallback<F>
where
    F: Fn(T) -> Result<()> + Sync,
{
    type Output = Result<()>;

    fn callback<P>(self, producer: P) -> Result<()>
    where
        P: rayon::iter::plumbing::Producer<Item = T>,
    {
        stage_producer(producer, self.items, self.lanes, &self.function)
    }
}

fn stage_producer<P, F>(producer: P, items: usize, lanes: usize, function: &F) -> Result<()>
where
    P: rayon::iter::plumbing::Producer,
    F: Fn(P::Item) -> Result<()> + Sync,
{
    if items == 0 {
        return Ok(());
    }
    if lanes <= 1 || items <= 1 {
        return producer.into_iter().try_for_each(function);
    }
    let left_lanes = lanes / 2;
    let middle = ((items as u128 * left_lanes as u128) / lanes as u128) as usize;
    let (left, right) = producer.split_at(middle);
    let (first, second) = rayon::join(
        || stage_producer(left, middle, left_lanes, function),
        || stage_producer(right, items - middle, lanes - left_lanes, function),
    );
    // PPML objective evaluation requires the first error in physical item order.
    first?;
    second
}

/// Two disjoint output banks and one already admitted scratch owner per lane.
/// Empty banks have stride zero. All splitting is allocation-free.
pub(crate) struct GroupBuffers<'a, A, B, C> {
    pub(crate) first: &'a mut [A],
    pub(crate) first_stride: usize,
    pub(crate) second: &'a mut [B],
    pub(crate) second_stride: usize,
    pub(crate) scratch: &'a mut [C],
}

pub(crate) fn group_blocks<A: Send, B: Send, C: Send, F>(
    buffers: GroupBuffers<'_, A, B, C>,
    offsets: &[usize],
    first: usize,
    function: &F,
) -> Result<()>
where
    F: Fn(usize, &mut [A], &mut [B], &mut C) -> Result<()> + Sync,
{
    let levels = offsets.len().saturating_sub(1);
    if levels == 0 {
        return Ok(());
    }
    debug_assert!(!buffers.scratch.is_empty());
    if buffers.scratch.len() <= 1 || levels == 1 {
        return function(
            first,
            buffers.first,
            buffers.second,
            &mut buffers.scratch[0],
        );
    }
    let lanes = buffers.scratch.len().min(levels);
    let left_lanes = lanes / 2;
    let right_lanes = lanes - left_lanes;
    let work = offsets[levels] - offsets[0];
    let target = offsets[0] + ((work as u128 * left_lanes as u128) / lanes as u128) as usize;
    let middle = offsets
        .partition_point(|&x| x < target)
        .clamp(left_lanes, levels - right_lanes);
    let (a, b) = buffers.first.split_at_mut(middle * buffers.first_stride);
    let (c, d) = buffers.second.split_at_mut(middle * buffers.second_stride);
    let (left, right) = buffers.scratch.split_at_mut(left_lanes);
    let (x, y) = rayon::join(
        || {
            group_blocks(
                GroupBuffers {
                    first: a,
                    first_stride: buffers.first_stride,
                    second: c,
                    second_stride: buffers.second_stride,
                    scratch: left,
                },
                &offsets[..=middle],
                first,
                function,
            )
        },
        || {
            group_blocks(
                GroupBuffers {
                    first: b,
                    first_stride: buffers.first_stride,
                    second: d,
                    second_stride: buffers.second_stride,
                    scratch: right,
                },
                &offsets[middle..],
                first + middle,
                function,
            )
        },
    );
    x?;
    y
}

#[derive(Debug)]
struct CachedThreadPool {
    threads: usize,
    pool: Arc<OwnedThreadPool>,
}

/// Own worker handles as well as the Rayon registry. Rayon alone signals
/// termination on drop, so an evicted team could overlap the next Stata call.
#[derive(Debug)]
struct OwnedThreadPool {
    pool: Option<rayon::ThreadPool>,
    workers: JoinedWorkers,
    context: Arc<WorkerContext>,
}

#[derive(Debug, Default)]
struct ActionAdmission {
    shared: usize,
}

/// Installed on every worker, so stolen tasks see the authority of the actual
/// registry. Action admission covers synchronous/scoped work, as do the native
/// transaction boundaries; detached user Rayon jobs are outside that contract.
#[derive(Debug, Default)]
struct WorkerContext {
    owner: OnceLock<Weak<OwnedThreadPool>>,
    admission: Mutex<ActionAdmission>,
}

thread_local! {
    static WORKER_CONTEXT: RefCell<Option<Arc<WorkerContext>>> = const { RefCell::new(None) };
}

fn current_worker_context() -> Option<Arc<WorkerContext>> {
    WORKER_CONTEXT.with(|current| current.borrow().clone())
}

/// Lend the actual admitted registry to one synchronous CMG operation as a
/// borrowed CMG team: CMG runs on fereg's workers and never creates a nested
/// pool. The worker ceiling is the action's thread count and the parallel
/// policy is CMG's default, matching the executors explicit routes created
/// before. Outside fereg's team (including a foreign Rayon pool), the callback
/// receives `None` and CMG runs serially. The team borrow cannot escape.
pub(crate) fn with_cmg_execution<T>(
    operation: impl FnOnce(Option<cmg::Team<'_>>) -> T,
) -> Result<T> {
    let Some(context) = current_worker_context() else {
        return Ok(operation(None));
    };
    let owner =
        context.owner.get().and_then(Weak::upgrade).ok_or_else(|| {
            FeregError::new(ErrorCode::Internal, "CMG host team has no live owner")
        })?;
    let pool = owner.pool.as_ref().expect("live worker team");
    if context.state().shared == 0 {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "CMG host team requires a live action admission",
        ));
    }
    let ceiling = pool.current_num_threads();
    let team = cmg::Team::new(pool, ceiling, cmg::ParallelOptions::default())
        .map_err(|error| FeregError::new(ErrorCode::Internal, error.to_string()))?;
    Ok(operation(Some(team)))
}

/// Run one explicit CMG operation on a borrowed team of exactly the requested
/// width. Inside a native action this is the action's own team; a team of a
/// different width is an internal error rather than a nested pool. Outside any
/// action (standalone tools and tests) the operation runs on fereg's
/// process-local team for `threads`, never on a CMG-owned pool.
pub(crate) fn with_cmg_team<T: Send>(
    threads: usize,
    operation: impl FnOnce(cmg::Team<'_>) -> Result<T> + Send,
) -> Result<T> {
    let installed = move || -> Result<T> {
        with_cmg_execution(|team| match team {
            Some(team) if team.worker_ceiling() == threads => operation(team),
            Some(team) => Err(FeregError::new(
                ErrorCode::Internal,
                format!(
                    "explicit CMG requested {threads} workers but its host team has {}",
                    team.worker_ceiling()
                ),
            )),
            None => Err(FeregError::new(
                ErrorCode::Internal,
                "explicit CMG requires a host team",
            )),
        })?
    };
    if current_worker_context().is_some() {
        installed()
    } else {
        ThreadTeam::new(crate::options::ThreadConfig::from_requested(threads, true)?)
            .install(installed)?
    }
}

impl WorkerContext {
    fn state(&self) -> std::sync::MutexGuard<'_, ActionAdmission> {
        // No user work runs under this lock. Cleanup remains infallible even
        // after an unrelated panic propagates through an installed action.
        self.admission
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

#[derive(Debug)]
struct ActionLease {
    context: Arc<WorkerContext>,
}

impl ActionLease {
    fn acquire(context: &Arc<WorkerContext>) -> Result<Self> {
        let mut state = context.state();
        state.shared = state.shared.checked_add(1).ok_or_else(|| {
            FeregError::new(ErrorCode::Internal, "worker-team action count overflows")
        })?;
        Ok(Self {
            context: Arc::clone(context),
        })
    }
}

impl Clone for ActionLease {
    fn clone(&self) -> Self {
        // Each lease owns a context Arc. Its reference-count limit bounds the
        // live lease count before the counter can overflow. Cloning the value
        // guard allocates nothing, including in warmed PPML iterations.
        let context = Arc::clone(&self.context);
        context.state().shared += 1;
        Self { context }
    }
}

impl Drop for ActionLease {
    fn drop(&mut self) {
        self.context.state().shared -= 1;
    }
}

#[derive(Debug)]
struct JoinedWorkers(Vec<std::thread::JoinHandle<()>>);

impl JoinedWorkers {
    fn join(&mut self) {
        for handle in self.0.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Drop for JoinedWorkers {
    fn drop(&mut self) {
        self.join();
    }
}

impl OwnedThreadPool {
    fn new(threads: usize) -> Result<Arc<Self>> {
        let mut handles = Vec::new();
        handles.try_reserve_exact(threads).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve worker-team handles",
            )
        })?;
        let mut workers = JoinedWorkers(handles);
        let context = Arc::new(WorkerContext::default());
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .spawn_handler(|thread| {
                let context = Arc::clone(&context);
                let mut builder = std::thread::Builder::new();
                if let Some(name) = thread.name() {
                    builder = builder.name(name.to_owned());
                }
                if let Some(size) = thread.stack_size() {
                    builder = builder.stack_size(size);
                }
                workers.0.push(builder.spawn(move || {
                    WORKER_CONTEXT.with(|current| *current.borrow_mut() = Some(context));
                    thread.run();
                    WORKER_CONTEXT.with(|current| *current.borrow_mut() = None);
                })?);
                Ok(())
            })
            .build()
            .map_err(|error| {
                FeregError::new(
                    ErrorCode::Allocation,
                    format!("unable to construct fereg worker team: {error}"),
                )
            })?;
        let owner = Arc::new(Self {
            pool: Some(pool),
            workers,
            context,
        });
        owner
            .context
            .owner
            .set(Arc::downgrade(&owner))
            .expect("new team owner");
        Ok(owner)
    }

    fn install<T: Send>(&self, function: impl FnOnce() -> T + Send) -> T {
        self.pool
            .as_ref()
            .expect("live worker team")
            .install(function)
    }

    fn acquire(self: &Arc<Self>) -> Result<InstalledThreadTeam> {
        let lease = ActionLease::acquire(&self.context)?;
        Ok(InstalledThreadTeam {
            pool: Arc::clone(self),
            _lease: lease,
        })
    }
}

impl Drop for OwnedThreadPool {
    fn drop(&mut self) {
        let on_own_worker = self
            .pool
            .as_ref()
            .is_some_and(|pool| pool.current_thread_index().is_some());
        drop(self.pool.take());
        if on_own_worker {
            // The public Rust API can be called by external asynchronous Rayon
            // work. Joining ourselves (or peers waiting for us) would deadlock.
            // Stata actions use only scoped work and release their teams on the
            // calling thread; this escape is outside that action lifetime proof.
            self.workers.0.clear();
        } else {
            self.workers.join();
        }
    }
}

static THREAD_POOL: OnceLock<Mutex<Option<CachedThreadPool>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadTeam {
    threads: usize,
    deterministic: bool,
}

/// A successfully acquired cached Rayon team. Acquisition owns every fallible
/// lock/build/admission step; installing work afterward is infallible. Clones
/// retain the admission lease, including across accepted-state transactions.
#[derive(Debug, Clone)]
pub(crate) struct InstalledThreadTeam {
    pool: Arc<OwnedThreadPool>,
    _lease: ActionLease,
}

impl InstalledThreadTeam {
    pub(crate) fn threads(&self) -> usize {
        self.pool
            .pool
            .as_ref()
            .expect("live worker team")
            .current_num_threads()
    }

    pub(crate) fn install<T, F>(&self, function: F) -> T
    where
        T: Send,
        F: FnOnce() -> T + Send,
    {
        self.pool.install(function)
    }

    fn in_place_scope<'scope, T>(&self, function: impl FnOnce(&rayon::Scope<'scope>) -> T) -> T {
        self.pool
            .pool
            .as_ref()
            .expect("live worker team")
            .in_place_scope(function)
    }
}

impl ThreadTeam {
    #[must_use]
    pub fn new(config: ThreadConfig) -> Self {
        Self {
            threads: config.resolved(),
            deterministic: config.deterministic,
        }
    }

    #[must_use]
    pub const fn threads(self) -> usize {
        self.threads
    }

    #[must_use]
    pub const fn deterministic(self) -> bool {
        self.deterministic
    }

    /// Run one native action with the resolved public thread count. Actions
    /// reuse one bounded process-local pool and replace it when the requested
    /// count changes, avoiding per-call Rayon registry retention while
    /// preserving safe thread-count changes between Stata calls.
    pub fn install<T, F>(self, function: F) -> Result<T>
    where
        T: Send,
        F: FnOnce() -> T + Send,
    {
        Ok(self.acquire()?.install(function))
    }

    /// Keep the scope body on its caller while lending the bounded team to
    /// Rust-owned work. In particular, a thread-confined input producer must
    /// never be moved into `install` alongside its native consumers.
    pub fn in_place_scope<'scope, T>(
        self,
        function: impl FnOnce(&rayon::Scope<'scope>) -> T,
    ) -> Result<T> {
        let team = self.acquire()?;
        Ok(team.in_place_scope(function))
    }

    /// Resolve/cache the requested pool before entering a transaction that
    /// cannot tolerate another fallible exit.
    pub(crate) fn acquire(self) -> Result<InstalledThreadTeam> {
        if let Some(context) = current_worker_context() {
            let pool = context.owner.get().and_then(Weak::upgrade).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "installed worker team has no live owner",
                )
            })?;
            if pool
                .pool
                .as_ref()
                .expect("live worker team")
                .current_num_threads()
                == self.threads
            {
                return pool.acquire();
            }
        }
        let cache = THREAD_POOL.get_or_init(|| Mutex::new(None));
        let mut retired = None;
        let pool = {
            let mut guard = cache.lock().map_err(|_| {
                FeregError::new(
                    ErrorCode::Internal,
                    "fereg worker-team cache lock is poisoned",
                )
            })?;
            if guard
                .as_ref()
                .is_none_or(|cached| cached.threads != self.threads)
            {
                let pool = OwnedThreadPool::new(self.threads)?;
                retired = guard.replace(CachedThreadPool {
                    threads: self.threads,
                    pool,
                });
            }
            Arc::clone(
                &guard
                    .as_ref()
                    .expect("worker-team cache was populated above")
                    .pool,
            )
        };
        drop(retired);
        pool.acquire()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reserved_initialization_uses_installed_lanes_and_preserves_storage() {
        assert!(rayon::current_thread_index().is_none());
        assert_eq!(super::reserved_initialization_plan(262_147).lanes, 1);
        let check = |workers| {
            for elements in [0, 1, 16_385, 65_536, 262_147] {
                let plan = super::reserved_initialization_plan(elements);
                assert_eq!(
                    plan.lanes,
                    super::StagePlan::new(elements as u128, elements, workers).lanes
                );
                assert!(elements / plan.chunk(elements) <= plan.lanes);
                let mut values = Vec::new();
                values.try_reserve_exact(elements).unwrap();
                let pointer = values.as_ptr();
                let capacity = values.capacity();
                super::initialize_reserved(&mut values, elements, 0.0_f64);
                assert_eq!(values.as_ptr(), pointer);
                assert_eq!(values.capacity(), capacity);
                assert_eq!(values.len(), elements);
                assert!(values.iter().all(|value| value.to_bits() == 0));
                let mut indices = Vec::new();
                indices.try_reserve_exact(elements).unwrap();
                let pointer = indices.as_ptr();
                let capacity = indices.capacity();
                super::initialize_reserved(&mut indices, elements, 0_usize);
                assert_eq!(indices.as_ptr(), pointer);
                assert_eq!(indices.capacity(), capacity);
                assert_eq!(indices.len(), elements);
                assert!(indices.iter().all(|&value| value == 0));
            }
        };
        check(1);
        for workers in [1, 4, 16] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| check(workers));
        }
    }

    struct CountingProducer {
        range: std::ops::Range<usize>,
        leaves: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl rayon::iter::plumbing::Producer for CountingProducer {
        type Item = usize;
        type IntoIter = std::ops::Range<usize>;

        fn into_iter(self) -> Self::IntoIter {
            self.leaves
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.range
        }

        fn split_at(self, index: usize) -> (Self, Self) {
            let middle = self.range.start + index;
            (
                Self {
                    range: self.range.start..middle,
                    leaves: std::sync::Arc::clone(&self.leaves),
                },
                Self {
                    range: middle..self.range.end,
                    leaves: self.leaves,
                },
            )
        }
    }

    #[test]
    fn explicit_partitions_expose_all_planned_lanes_and_visit_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        for items in [0, 1, 5, 9, 10, 17, 137] {
            for budget in [1, 2, 4, 7, 8, 9, 15, 16] {
                let leaves = std::sync::Arc::new(AtomicUsize::new(0));
                let visits: Vec<_> = (0..items).map(|_| AtomicUsize::new(0)).collect();
                let lanes = super::StagePlan::new(u128::MAX, items, budget).lanes;
                super::stage_producer(
                    CountingProducer {
                        range: 0..items,
                        leaves: std::sync::Arc::clone(&leaves),
                    },
                    items,
                    lanes,
                    &|index| {
                        visits[index].fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(
                    leaves.load(Ordering::SeqCst),
                    if items == 0 { 0 } else { lanes }
                );
                assert!(visits.iter().all(|value| value.load(Ordering::SeqCst) == 1));
            }
        }
    }

    #[test]
    fn indexed_stage_preserves_zip_indices_first_error_and_unwinds() {
        use rayon::prelude::*;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        let mut output = [0_usize; 10];
        pool.install(|| {
            super::stage_try_for_each(
                output
                    .par_iter_mut()
                    .zip((0..9).into_par_iter())
                    .enumerate(),
                u128::MAX,
                |(index, (target, source))| {
                    assert_eq!(index, source);
                    *target = index + 1;
                    Ok(())
                },
            )
            .unwrap();
        });
        assert_eq!(output, [1, 2, 3, 4, 5, 6, 7, 8, 9, 0]);
        let error = pool
            .install(|| {
                super::stage_try_for_each((0..10).into_par_iter(), u128::MAX, |index| {
                    if index == 2 || index == 7 {
                        Err(super::FeregError::new(
                            super::ErrorCode::InvalidData,
                            index.to_string(),
                        ))
                    } else {
                        Ok(())
                    }
                })
            })
            .unwrap_err();
        assert_eq!(error.code, super::ErrorCode::InvalidData);
        assert_eq!(error.message, "2");
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pool.install(|| {
                super::stage_try_for_each((0..10).into_par_iter(), u128::MAX, |index| {
                    assert_ne!(index, 7, "injected stage panic");
                    Ok(())
                })
            })))
            .is_err()
        );
    }

    #[test]
    fn stage_limits_follow_work_tasks_and_budget() {
        for budget in [1, 2, 4, 7, 8, 9, 15, 16] {
            for tasks in [0, 1, 5, 8, 31] {
                for work in [0, 16_384, 65_536, u128::MAX] {
                    let plan = super::StagePlan::new(work, tasks, budget);
                    assert!(plan.lanes <= budget && plan.lanes <= tasks.max(1));
                    assert!(tasks.div_ceil(plan.chunk(tasks)) <= plan.lanes);
                }
            }
        }
    }

    #[test]
    fn degree_blocks_cover_skewed_groups_and_preserve_lane_owners() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        for offsets in [
            vec![
                0, 1_000_000, 1_000_001, 1_000_002, 1_000_003, 1_000_004, 1_000_005, 1_000_006,
                1_000_007,
            ],
            vec![0, 1, 2, 3, 4, 5, 6, 7, 1_000_007],
            vec![0, 2, 5, 7, 11, 13, 17, 19, 23],
        ] {
            for lanes in 1..=8 {
                let calls = AtomicUsize::new(0);
                let mut output = vec![usize::MAX; 8];
                super::StagePlan::new(u128::MAX, 8, lanes)
                    .groups_mut(&mut output, 1, &offsets, |first, values| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        for (local, value) in values.iter_mut().enumerate() {
                            *value = first + local;
                        }
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(output, (0..8).collect::<Vec<_>>());
                assert_eq!(calls.load(Ordering::SeqCst), lanes);
            }
        }
    }

    use super::*;
    use crate::options::ThreadConfig;

    #[test]
    fn cmg_borrows_the_actual_admitted_team_only() {
        assert!(with_cmg_execution(|team| team.is_none()).unwrap());
        let foreign = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        foreign.install(|| assert!(with_cmg_execution(|team| team.is_none()).unwrap()));
        for workers in [1, 4] {
            let owner = OwnedThreadPool::new(workers).unwrap();
            // Registry membership alone is not an admitted native action.
            assert!(owner.install(|| with_cmg_execution(|_| ())).is_err());
            let admitted = owner.acquire().unwrap();
            admitted.install(|| {
                let coordinator = std::thread::current().id();
                let local = std::cell::Cell::new(0);
                with_cmg_execution(|team| {
                    assert_eq!(team.unwrap().worker_ceiling(), workers);
                    assert_eq!(std::thread::current().id(), coordinator);
                    local.set(1); // The coordinator and its controller need not be Send.
                })
                .unwrap();
                assert_eq!(local.get(), 1);
                foreign.install(|| assert!(with_cmg_execution(|team| team.is_none()).unwrap()));
            });
        }
    }

    #[test]
    fn owned_pool_joins_on_calling_thread_and_handles_nested_teams() {
        for threads in [1, 4, 1] {
            let mut pool = Arc::try_unwrap(OwnedThreadPool::new(threads).unwrap()).unwrap();
            assert_eq!(pool.install(rayon::current_num_threads), threads);
            assert_eq!(pool.workers.0.len(), threads);
            drop(pool.pool.take());
            for handle in pool.workers.0.drain(..) {
                handle.join().unwrap();
            }
        }
        let outer = ThreadTeam::new(ThreadConfig::from_requested(2, true).unwrap());
        let inner = ThreadTeam::new(ThreadConfig::from_requested(3, true).unwrap());
        assert_eq!(
            outer
                .install(|| inner.install(rayon::current_num_threads).unwrap())
                .unwrap(),
            3
        );
    }

    #[test]
    fn last_owner_on_own_worker_does_not_join_itself() {
        let pool = OwnedThreadPool::new(2).unwrap();
        let owner = Arc::clone(&pool);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (go_tx, go_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        pool.pool.as_ref().unwrap().spawn(move || {
            ready_tx.send(()).unwrap();
            go_rx.recv().unwrap();
            drop(owner);
            done_tx.send(()).unwrap();
        });
        ready_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        drop(pool);
        go_tx.send(()).unwrap();
        done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    fn isolated_pool_uses_the_resolved_thread_count() {
        let team = ThreadTeam::new(ThreadConfig::from_requested(2, true).expect("config"));
        let observed = team
            .install(rayon::current_num_threads)
            .expect("isolated pool");
        assert_eq!(observed, 2);
    }

    #[test]
    fn cached_pool_can_change_counts_and_return_to_one() {
        for threads in [1, 4, 1] {
            let team =
                ThreadTeam::new(ThreadConfig::from_requested(threads, true).expect("config"));
            assert_eq!(
                team.install(rayon::current_num_threads)
                    .expect("cached team"),
                threads,
            );
        }
    }

    #[test]
    fn admission_leases_cover_clones_and_nested_survivors() {
        let pool = OwnedThreadPool::new(2).unwrap();
        let shared = pool.acquire().unwrap();
        let clone = shared.clone();
        assert_eq!(pool.context.state().shared, 2);
        drop(shared);
        assert_eq!(pool.context.state().shared, 1);
        let survivor = clone.install(|| {
            let nested = ThreadTeam::new(ThreadConfig::from_requested(2, true).unwrap())
                .acquire()
                .unwrap();
            assert!(Arc::ptr_eq(&nested.pool, &pool));
            nested
        });
        drop(clone);
        assert_eq!(pool.context.state().shared, 1);
        assert!(survivor.install(|| with_cmg_execution(|team| team.is_some()).unwrap()));
        drop(survivor);
        assert_eq!(pool.context.state().shared, 0);
        // A worker without a live action admission cannot lend its team to CMG.
        assert!(pool.install(|| with_cmg_execution(|_| ())).is_err());
        assert!(with_cmg_execution(|team| team.is_none()).unwrap());
    }

    #[test]
    fn scope_retains_admission_until_spawned_work_finishes() {
        let pool = OwnedThreadPool::new(2).unwrap();
        let team = pool.acquire().unwrap();
        let (body_tx, body_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            team.in_place_scope(|scope| {
                scope.spawn(move |_| {
                    assert!(with_cmg_execution(|team| team.is_some()).unwrap());
                    release_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                });
                body_tx.send(()).unwrap();
            });
        });
        body_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(pool.context.state().shared, 1);
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        assert_eq!(pool.context.state().shared, 0);
    }
}
