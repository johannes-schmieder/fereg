//! Setup attribution diagnostics for private instrumented copies only.
//!
//! Added by `bench/cmg_setup_attribution.py`; never part of a release. All
//! state is process-global scalar storage: fixed atomic tables and per-thread
//! counter cells. Nothing here allocates on a hot path, sorts or copies a
//! graph, changes a comparison, adds or removes a cancellation query, or
//! changes meter arithmetic. Exactly one of [`COUNTS`] and [`TIMERS`] is set
//! by the applier, so the other dimension compiles away.

// The counter kinds of the introsort are unused in the accepted heapsort arm.
#![allow(dead_code)]

use core::cell::Cell;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

/// Count-only build: operation counters, no timers. Set by the applier.
pub(crate) const COUNTS: bool = false;
/// Timer-only build: scalar timers, no operation counters. Set by the applier.
pub(crate) const TIMERS: bool = false;
/// The applied mode, reported in the trace.
const MODE: &str = "unset";

/// Comparator calls made by the canonical raw-edge sort.
pub(crate) const COMPARISONS: usize = 0;
/// Partition calls (introsort only).
pub(crate) const PARTITIONS: usize = 1;
/// Records classified by partition loops: one per loop iteration.
pub(crate) const PARTITION_STEPS: usize = 2;
/// `slice::swap` calls inside partitions.
pub(crate) const PARTITION_SWAPS: usize = 3;
/// Insertion-sort leaves.
pub(crate) const INSERTION_LEAVES: usize = 4;
/// Records in insertion-sort leaves.
pub(crate) const INSERTION_RECORDS: usize = 5;
/// Adjacent `slice::swap` calls inside insertion leaves.
pub(crate) const INSERTION_SWAPS: usize = 6;
/// Heapsort runs: the whole sort (heapsort) or depth fallbacks (introsort).
pub(crate) const HEAPSORTS: usize = 7;
/// Records in heapsort runs.
pub(crate) const HEAPSORT_RECORDS: usize = 8;
/// Sift-loop iterations inside heapsort runs.
pub(crate) const HEAP_SIFT_STEPS: usize = 9;
/// `slice::swap` calls inside heapsort runs (sifts and extractions).
pub(crate) const HEAP_SWAPS: usize = 10;
/// Units counted by the caller's `CancelMeter` during the sort.
pub(crate) const METER_UNITS: usize = 11;
/// Cancellation queries made by the caller's `CancelMeter` during the sort.
pub(crate) const QUERIES: usize = 12;
const KINDS: usize = 13;
const KIND_NAMES: [&str; KINDS] = [
    "cmp",
    "partitions",
    "partition_steps",
    "partition_swaps",
    "insertion_leaves",
    "insertion_records",
    "insertion_swaps",
    "heapsorts",
    "heapsort_records",
    "heap_sift_steps",
    "heap_swaps",
    "meter_units",
    "queries",
];

const SORT_SLOTS: usize = 64;
// site, level, path, input, output, sort_ns, canonical_ns, complete, then kinds.
const SORT_FIELDS: usize = 8 + KINDS;
const LEVELS: usize = 64;
const PHASES: [&str; 18] = [
    "option_validation",
    "graph_clone_reference_setup",
    "direct_terminal_check",
    "forest_select_split_low_degree_and_label",
    "aggregation_construction",
    "hierarchy_bookkeeping_and_fill_checks",
    "coarse_edge_map_sort_merge_and_graph_finalization",
    "coarse_isolate_pruning",
    "repeat_count_initialization",
    "inverse_diagonal_and_level_finalization",
    "hierarchy_report_bookkeeping",
    "hierarchy_total",
    "finest_components",
    "coarse_centering",
    "terminal_factor",
    "finalize_total",
    "build_with_total",
    "unused",
];

static SORTS: [[AtomicU64; SORT_FIELDS]; SORT_SLOTS] =
    [const { [const { AtomicU64::new(0) }; SORT_FIELDS] }; SORT_SLOTS];
static NEXT_SORT: AtomicUsize = AtomicUsize::new(0);
static TIMER_NS: [[AtomicU64; LEVELS]; PHASES.len()] =
    [const { [const { AtomicU64::new(0) }; LEVELS] }; PHASES.len()];
static TIMER_CALLS: [[AtomicU64; LEVELS]; PHASES.len()] =
    [const { [const { AtomicU64::new(0) }; LEVELS] }; PHASES.len()];
static UNKNOWN_PHASES: AtomicU64 = AtomicU64::new(0);
static LEVEL_OVERFLOW: AtomicU64 = AtomicU64::new(0);

std::thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static COUNTERS: [Cell<u64>; KINDS] = const { [const { Cell::new(0) }; KINDS] };
    // Contraction level whose canonical sort runs on this thread, if any.
    static SITE: Cell<Option<usize>> = const { Cell::new(None) };
}

/// Count `amount` operations of `kind` while a counted canonical sort is
/// active on this thread. Compiles to nothing in a timer-only build.
#[inline(always)]
pub(crate) fn count(kind: usize, amount: u64) {
    if COUNTS {
        ACTIVE.with(|active| {
            if active.get() {
                COUNTERS.with(|counters| counters[kind].set(counters[kind].get() + amount));
            }
        });
    }
}

/// Clear every record before an attempt.
#[doc(hidden)]
pub fn reset() {
    for slot in &SORTS {
        for field in slot {
            field.store(0, Relaxed);
        }
    }
    NEXT_SORT.store(0, Relaxed);
    for row in TIMER_NS.iter().chain(TIMER_CALLS.iter()) {
        for cell in row {
            cell.store(0, Relaxed);
        }
    }
    UNKNOWN_PHASES.store(0, Relaxed);
    LEVEL_OVERFLOW.store(0, Relaxed);
}

/// A running scalar timer (timer-only builds).
pub(crate) struct Timer(Option<Instant>);

impl Timer {
    #[inline(always)]
    pub(crate) fn start() -> Self {
        Self(TIMERS.then(Instant::now))
    }

    /// Add the elapsed time to `phase` at `level`.
    #[inline(always)]
    pub(crate) fn stop_phase(self, phase: &'static str, level: usize) {
        if let Some(start) = self.0 {
            let elapsed = start.elapsed().as_nanos() as u64;
            let Some(index) = PHASES.iter().position(|name| *name == phase) else {
                UNKNOWN_PHASES.fetch_add(1, Relaxed);
                return;
            };
            if level >= LEVELS {
                LEVEL_OVERFLOW.fetch_add(1, Relaxed);
                return;
            }
            TIMER_NS[index][level].fetch_add(elapsed, Relaxed);
            TIMER_CALLS[index][level].fetch_add(1, Relaxed);
        }
    }
}

/// Time `operation` as `phase` at `level` (timer-only builds).
#[inline(always)]
pub(crate) fn timed_phase<Output>(
    phase: &'static str,
    level: usize,
    operation: impl FnOnce() -> Output,
) -> Output {
    let timer = Timer::start();
    let output = operation();
    timer.stop_phase(phase, level);
    output
}

/// Marks this thread's canonical sorts as the contraction of `level`.
pub(crate) struct ContractionSite(Option<usize>);

impl Drop for ContractionSite {
    fn drop(&mut self) {
        SITE.with(|site| site.set(self.0));
    }
}

pub(crate) fn contraction_site(level: usize) -> ContractionSite {
    ContractionSite(SITE.with(|site| site.replace(Some(level))))
}

/// Path of a canonical raw-edge sort.
pub(crate) const PATH_SERIAL: u64 = 0;
/// Team sort inside `Laplacian::canonical_parallel` (lanes not counted).
pub(crate) const PATH_TEAM: u64 = 1;
/// Team sort inside the hosted initial graph (lanes not counted).
pub(crate) const PATH_HOSTED: u64 = 2;

/// One canonical raw-edge sort and its duplicate reduction.
pub(crate) struct SortScope {
    slot: Option<usize>,
    started: Option<Instant>,
    sort_ns: u64,
    counted: bool,
}

impl SortScope {
    /// Claim a record. Only serial sorts are counted, and only on this
    /// thread; team lanes run elsewhere (or inline) and stay uncounted.
    pub(crate) fn begin(path: u64, input: usize) -> Self {
        let index = NEXT_SORT.fetch_add(1, Relaxed);
        let slot = (index < SORT_SLOTS).then_some(index);
        if let Some(slot) = slot {
            let record = &SORTS[slot];
            let site = SITE.with(Cell::get);
            record[0].store(u64::from(site.is_some()), Relaxed);
            record[1].store(site.unwrap_or(0) as u64, Relaxed);
            record[2].store(path, Relaxed);
            record[3].store(input as u64, Relaxed);
        }
        let counted = COUNTS && path == PATH_SERIAL && slot.is_some();
        if counted {
            COUNTERS.with(|counters| counters.iter().for_each(|cell| cell.set(0)));
        }
        ACTIVE.with(|active| active.set(counted));
        Self {
            slot,
            started: TIMERS.then(Instant::now),
            sort_ns: 0,
            counted,
        }
    }

    /// The sort has finished: stop counting and take its time.
    pub(crate) fn sorted(mut self) -> Self {
        ACTIVE.with(|active| active.set(false));
        if let Some(started) = self.started {
            self.sort_ns = started.elapsed().as_nanos() as u64;
        }
        if let (Some(slot), true) = (self.slot, self.counted) {
            COUNTERS.with(|counters| {
                for (kind, cell) in counters.iter().enumerate() {
                    SORTS[slot][8 + kind].store(cell.get(), Relaxed);
                }
            });
        }
        self
    }

    /// Duplicate reduction has finished: record the output and total time.
    pub(crate) fn finish<E>(self, graph: &Result<crate::Laplacian, E>) {
        if let (Some(slot), Ok(graph)) = (self.slot, graph) {
            let record = &SORTS[slot];
            record[4].store(graph.edge_count() as u64, Relaxed);
            record[5].store(self.sort_ns, Relaxed);
            if let Some(started) = self.started {
                record[6].store(started.elapsed().as_nanos() as u64, Relaxed);
            }
            record[7].store(1, Relaxed);
        }
    }
}

impl Drop for SortScope {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(false));
    }
}

/// The trace fields of everything recorded since [`reset`]: one `sortN`
/// field per canonical sort in call order and one `timers` field.
#[doc(hidden)]
#[must_use]
pub fn trace_fields() -> String {
    let sorts = NEXT_SORT.load(Relaxed);
    let mut fields = format!(
        "schema=1|mode={MODE}|sorts={sorts}|sort_overflow={}|unknown_phases={}|level_overflow={}",
        sorts.saturating_sub(SORT_SLOTS),
        UNKNOWN_PHASES.load(Relaxed),
        LEVEL_OVERFLOW.load(Relaxed),
    );
    let mut timers = Vec::new();
    for (index, name) in PHASES.iter().enumerate() {
        for level in 0..LEVELS {
            let calls = TIMER_CALLS[index][level].load(Relaxed);
            if calls > 0 {
                timers.push(format!(
                    "{name}@{level}:{}:{calls}",
                    TIMER_NS[index][level].load(Relaxed)
                ));
            }
        }
    }
    fields.push_str("|timers=");
    fields.push_str(&timers.join(";"));
    for (index, record) in SORTS.iter().enumerate().take(sorts.min(SORT_SLOTS)) {
        let value = |field: usize| record[field].load(Relaxed);
        let site = if value(0) == 1 {
            "contraction"
        } else {
            "outside"
        };
        let path = ["serial", "team", "hosted"]
            .get(value(2) as usize)
            .copied()
            .unwrap_or("invalid");
        fields.push_str(&format!(
            "|sort{index}=site:{site},level:{},path:{path},n:{},out:{},sort_ns:{},canonical_ns:{},complete:{}",
            value(1), value(3), value(4), value(5), value(6), value(7),
        ));
        for (kind, name) in KIND_NAMES.iter().enumerate() {
            fields.push_str(&format!(",{name}:{}", value(8 + kind)));
        }
    }
    fields
}
