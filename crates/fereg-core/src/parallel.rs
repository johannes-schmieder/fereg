use crate::error::{ErrorCode, FeregError, Result};
use crate::options::ThreadConfig;
use rayon::prelude::*;
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

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
        if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
            eprintln!("FEREG_SOLVER_TRACE|phase=stage_plan|stage={stage}|budget={}|lane_limit={}|independent_items={tasks}", rayon::current_num_threads(), self.lanes);
        }
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

/// Bound executable tasks while retaining each caller's arithmetic leaves.
pub(crate) fn stage_items_mut<T: Send>(
    values: &mut [T],
    work: u128,
) -> impl IndexedParallelIterator<Item = &mut T> {
    let grain = StagePlan::current(work, values.len()).chunk(values.len());
    values.par_iter_mut().with_min_len(grain)
}

pub(crate) fn stage_rows_mut<T: Send>(
    values: &mut [T],
    width: usize,
    work: u128,
) -> impl IndexedParallelIterator<Item = &mut [T]> {
    let grain =
        StagePlan::current(work, values.len().div_ceil(width)).chunk(values.len().div_ceil(width));
    values.par_chunks_mut(width).with_min_len(grain)
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
    fn new(threads: usize) -> Result<Self> {
        let mut handles = Vec::new();
        handles.try_reserve_exact(threads).map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve worker-team handles",
            )
        })?;
        let mut workers = JoinedWorkers(handles);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .spawn_handler(|thread| {
                let mut builder = std::thread::Builder::new();
                if let Some(name) = thread.name() {
                    builder = builder.name(name.to_owned());
                }
                if let Some(size) = thread.stack_size() {
                    builder = builder.stack_size(size);
                }
                workers.0.push(builder.spawn(|| thread.run())?);
                Ok(())
            })
            .build()
            .map_err(|error| {
                FeregError::new(
                    ErrorCode::Allocation,
                    format!("unable to construct fereg worker team: {error}"),
                )
            })?;
        Ok(Self {
            pool: Some(pool),
            workers,
        })
    }

    fn install<T: Send>(&self, function: impl FnOnce() -> T + Send) -> T {
        self.pool
            .as_ref()
            .expect("live worker team")
            .install(function)
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

/// A successfully acquired cached Rayon team. Acquisition owns every
/// fallible lock/build step; installing work afterward is infallible.
#[derive(Debug, Clone)]
pub(crate) struct InstalledThreadTeam {
    pool: Arc<OwnedThreadPool>,
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
        Ok(team
            .pool
            .pool
            .as_ref()
            .expect("live worker team")
            .in_place_scope(function))
    }

    /// Resolve/cache the requested pool before entering a transaction that
    /// cannot tolerate another fallible exit.
    pub(crate) fn acquire(self) -> Result<InstalledThreadTeam> {
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
                    pool: Arc::new(pool),
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
        Ok(InstalledThreadTeam { pool })
    }

    #[must_use]
    pub fn ranges(self, len: usize, minimum_chunk: usize) -> Vec<Range<usize>> {
        if len == 0 {
            return Vec::new();
        }
        let minimum_chunk = minimum_chunk.max(1);
        let useful_workers = len.div_ceil(minimum_chunk).max(1);
        let workers = self.threads.min(useful_workers).min(len).max(1);
        let base = len / workers;
        let remainder = len % workers;
        let mut start = 0usize;
        (0..workers)
            .map(|worker| {
                let width = base + usize::from(worker < remainder);
                let range = start..start + width;
                start += width;
                range
            })
            .collect()
    }

    pub fn map_ranges<T, F>(self, len: usize, minimum_chunk: usize, function: F) -> Vec<T>
    where
        T: Send,
        F: Fn(usize, Range<usize>) -> T + Sync,
    {
        let ranges = self.ranges(len, minimum_chunk);
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(ranges.len());
            for (worker, range) in ranges.into_iter().enumerate() {
                let function = &function;
                handles.push(scope.spawn(move || function(worker, range)));
            }
            handles
                .into_iter()
                .map(|handle| handle.join().expect("fereg worker thread panicked"))
                .collect()
        })
    }

    #[must_use]
    pub fn xor_checksum(self, values: &[u64]) -> u64 {
        let partials = self.map_ranges(values.len(), 1_024, |worker, range| {
            let mut state = 0x9e37_79b9_7f4a_7c15_u64 ^ worker as u64;
            let start = range.start;
            for (index, value) in values[range].iter().enumerate() {
                let absolute = start + index;
                state ^= value
                    .wrapping_add((absolute as u64).rotate_left((absolute % 63) as u32))
                    .rotate_left((absolute % 61) as u32);
            }
            (worker, state)
        });
        partials.into_iter().fold(0_u64, |state, (worker, value)| {
            state ^ value.rotate_left((worker % 63) as u32)
        })
    }
}

#[cfg(test)]
mod tests {
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
    fn owned_pool_joins_on_calling_thread_and_handles_nested_teams() {
        for threads in [1, 4, 1] {
            let mut pool = OwnedThreadPool::new(threads).unwrap();
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
        let pool = Arc::new(OwnedThreadPool::new(2).unwrap());
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
    fn ranges_cover_exactly_once() {
        let team = ThreadTeam::new(ThreadConfig::from_requested(32, true).expect("config"));
        let ranges = team.ranges(101, 1);
        assert_eq!(ranges.first().map(|range| range.start), Some(0));
        assert_eq!(ranges.last().map(|range| range.end), Some(101));
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
    }

    #[test]
    fn requested_32_threads_are_exercised_on_large_work() {
        let team = ThreadTeam::new(ThreadConfig::from_requested(32, true).expect("config"));
        assert_eq!(team.ranges(100_000, 1_024).len(), 32);
    }

    #[test]
    fn map_preserves_worker_order() {
        let team = ThreadTeam::new(ThreadConfig::from_requested(8, true).expect("config"));
        let result = team.map_ranges(80, 10, |worker, range| (worker, range));
        for (expected, (worker, _)) in result.into_iter().enumerate() {
            assert_eq!(expected, worker);
        }
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
}
