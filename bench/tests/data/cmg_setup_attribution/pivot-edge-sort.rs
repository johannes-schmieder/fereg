//! The one canonical edge sort: an in-place introsort by the total raw-edge
//! order, counted and polled, with no allocation. Serial construction sorts
//! on the caller's meter; with a team, partitions fork within the worker
//! ceiling and each lane polls independently.

use super::{Edge, compare_raw_edges};
use crate::CmgError;
#[cfg(feature = "parallel")]
use crate::Team;
#[cfg(feature = "parallel")]
use crate::cancel::LanePoll;
use crate::cancel::{Meter, Progress, heapsort};
#[cfg(feature = "parallel")]
use crate::execution::Lanes;
use core::cmp::Ordering;

/// Ranges at most this long are insertion-sorted.
const SMALL: usize = 16;
/// Partitions longer than this take Tukey's ninther as pivot (Bentley and
/// McIlroy, "Engineering a Sort Function", 1993); shorter ones take the
/// median of their first, middle and last records.
const NINTHER_MIN: usize = 41;
/// Counted units of pivot selection: 16 per comparison (as a partition
/// step), at most 3 comparisons for a median of three and 12 for a ninther.
const MEDIAN_UNITS: usize = 3 * 16;
const NINTHER_UNITS: usize = 4 * MEDIAN_UNITS;
/// Partitions shorter than this are not forked.
#[cfg(feature = "parallel")]
const FORK_MIN: usize = 1 << 14;
#[cfg(feature = "parallel")]
const PHASE: &str = "team edge sort";

/// Test-only probe: the partition this thread is running, as (classified
/// records, length), so a witness can fire cancellation at a poll inside
/// actual partition work and see what that partition leaves untouched.
#[cfg(all(test, feature = "parallel"))]
mod probe {
    use core::cell::Cell;

    std::thread_local! {
        static PARTITION: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
    }

    pub(super) struct Partitioning((usize, usize));

    pub(super) fn enter(len: usize) -> Partitioning {
        Partitioning(PARTITION.with(|state| state.replace((0, len))))
    }

    impl Drop for Partitioning {
        fn drop(&mut self) {
            PARTITION.with(|state| state.set(self.0));
        }
    }

    pub(super) fn step() {
        PARTITION.with(|state| state.set((state.get().0 + 1, state.get().1)));
    }

    /// Records the running partition has not classified yet, if any runs.
    pub(super) fn remaining() -> Option<usize> {
        let (classified, len) = PARTITION.with(Cell::get);
        (len > 0).then(|| len - classified)
    }
}

/// Test-only count of depth-limit heapsort fallbacks and their records on
/// this thread (serial sorts), for operation-count regression witnesses.
#[cfg(test)]
mod fallbacks {
    use core::cell::Cell;

    std::thread_local! {
        static FALLBACKS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
    }

    pub(super) fn record(len: usize) {
        FALLBACKS.with(|state| state.set((state.get().0 + 1, state.get().1 + len)));
    }

    /// (fallbacks, records) since the last call, then reset.
    pub(super) fn take() -> (usize, usize) {
        FALLBACKS.with(|state| state.replace((0, 0)))
    }
}

fn depth_limit(len: usize) -> usize {
    2 * (usize::BITS - len.leading_zeros()) as usize
}

/// Sort `values` by the total raw-edge order in place on the calling lane: an
/// introsort with three-way partitions around a ninther pivot above 40
/// records and a median-of-three pivot below (see [`pivot`]), a heapsort
/// fallback at depth `2 log2(n)` and insertion sort below 17 records. Every
/// pivot selection, partition step, insertion step and heap step is counted on
/// `meter`: pivot selection after it runs (at most 192 units), a partition
/// every 64 steps (1,024 units), and the
/// meter queries every 65,536 units (about 4,096 steps), inside a partition
/// too. The comparator is total and equal records are identical, so the order
/// equals every other sort's, independent of pivots. Nothing is allocated.
pub(crate) fn sort_serial<M: Meter>(
    values: &mut [Edge],
    meter: &mut M,
    phase: &'static str,
) -> Result<(), CmgError> {
    serial_range(
        values,
        depth_limit(values.len()),
        meter,
        phase,
        compare_raw_edges,
    )
}

/// The serial introsort over any total order on edges. Production passes the
/// canonical comparator; tests also drive the identical algorithm with an
/// adaptive adversary to construct inputs that defeat its pivot rule.
fn serial_range<P: Progress, C: Fn(&Edge, &Edge) -> Ordering + Copy>(
    mut values: &mut [Edge],
    mut depth: usize,
    poll: &mut P,
    phase: &'static str,
    compare: C,
) -> Result<(), CmgError> {
    loop {
        if values.len() <= SMALL {
            return insertion(values, poll, phase, compare);
        }
        if depth == 0 {
            #[cfg(test)]
            fallbacks::record(values.len());
            return heapsort(poll, values, compare, phase);
        }
        depth -= 1;
        let (less, greater) = partition(values, poll, phase, compare)?;
        let (left, rest) = values.split_at_mut(less);
        let right = &mut rest[greater - less..];
        // Recurse into the shorter side and continue with the longer one.
        if left.len() < right.len() {
            serial_range(left, depth, poll, phase, compare)?;
            values = right;
        } else {
            serial_range(right, depth, poll, phase, compare)?;
            values = left;
        }
    }
}

/// The same sort on a team: partitions of at least 16,384 records fork while
/// lanes remain, so at most the worker ceiling of lanes run, and every started
/// lane joins before the meter's final poll. Each logical node of the
/// partition tree (a partition, an insertion leaf or a heapsort fallback)
/// queries once on entry with its own lane counter and then at the fixed
/// cadence (every 65,536 counted units, about 4,096 partition steps). The tree
/// depends only on the input, so a successful run's query total and each
/// node's logical checkpoints are the same at every worker ceiling, and work
/// between two queries of a lane stays bounded. The chronological order of
/// queries across lanes, and where a cancellation stops, are not fixed.
#[cfg(feature = "parallel")]
pub(crate) fn sort<M: Meter>(
    values: &mut [Edge],
    team: Team<'_>,
    meter: &mut M,
) -> Result<(), CmgError> {
    let query = meter.query();
    let depth = depth_limit(values.len());
    let sorted = team.with_lanes(|lanes| team_range(values, depth, lanes, query));
    meter.joined(PHASE)?;
    sorted
}

#[cfg(feature = "parallel")]
fn team_range(
    mut values: &mut [Edge],
    mut depth: usize,
    lanes: Lanes<'_>,
    query: Option<crate::Cancellation<'_>>,
) -> Result<(), CmgError> {
    loop {
        let mut node = LanePoll::new(query);
        node.advance(PHASE, crate::cancel::POLL_UNITS as usize)?;
        if values.len() <= SMALL {
            return insertion(values, &mut node, PHASE, compare_raw_edges);
        }
        if depth == 0 {
            return heapsort(&mut node, values, compare_raw_edges, PHASE);
        }
        depth -= 1;
        let (less, greater) = partition(values, &mut node, PHASE, compare_raw_edges)?;
        let total = values.len();
        let (left, rest) = values.split_at_mut(less);
        let right = &mut rest[greater - less..];
        if lanes.count() > 1 && total >= FORK_MIN {
            let (a, b) = lanes.join(
                |lanes| team_range(left, depth, lanes, query),
                |lanes| team_range(right, depth, lanes, query),
            );
            return a.and(b);
        }
        // Recurse into the shorter side and continue with the longer one.
        if left.len() < right.len() {
            team_range(left, depth, lanes.single(), query)?;
            values = right;
        } else {
            team_range(right, depth, lanes.single(), query)?;
            values = left;
        }
    }
}

fn insertion<P: Progress, C: Fn(&Edge, &Edge) -> Ordering>(
    values: &mut [Edge],
    poll: &mut P,
    phase: &'static str,
    compare: C,
) -> Result<(), CmgError> {
    for index in 1..values.len() {
        let mut current = index;
        while current > 0 && compare(&values[current], &values[current - 1]).is_lt() {
            values.swap(current, current - 1);
            current -= 1;
        }
        poll.advance(phase, 16 * (index - current + 1))?;
    }
    Ok(())
}

/// The median of three records under a total order, in at most three
/// comparisons. Equal records are identical, so the value is unique.
fn median_of_three(a: Edge, b: Edge, c: Edge, compare: impl Fn(&Edge, &Edge) -> Ordering) -> Edge {
    if compare(&a, &b).is_lt() {
        if compare(&b, &c).is_lt() {
            b
        } else if compare(&a, &c).is_lt() {
            c
        } else {
            a
        }
    } else if compare(&a, &c).is_lt() {
        a
    } else if compare(&b, &c).is_lt() {
        c
    } else {
        b
    }
}

/// The pivot of a partition of at least 17 records and its counted units.
/// Up to 40 records: the median of the first, middle and last records. Above:
/// Tukey's ninther, the median of the medians of three evenly spaced triples
/// around the first, middle and last records, with spacing `len / 8`.
fn pivot(values: &[Edge], compare: impl Fn(&Edge, &Edge) -> Ordering + Copy) -> (Edge, usize) {
    let last = values.len() - 1;
    let middle = last / 2;
    if values.len() < NINTHER_MIN {
        let pivot = median_of_three(values[0], values[middle], values[last], compare);
        return (pivot, MEDIAN_UNITS);
    }
    // len >= 41 gives s >= 5 and nine distinct ordered positions.
    let s = values.len() / 8;
    let at =
        |a: usize, b: usize, c: usize| median_of_three(values[a], values[b], values[c], compare);
    let pivot = median_of_three(
        at(0, s, 2 * s),
        at(middle - s, middle, middle + s),
        at(last - 2 * s, last - s, last),
        compare,
    );
    (pivot, NINTHER_UNITS)
}

/// Dijkstra three-way partition around [`pivot`]. Pivot selection is counted
/// after it runs (at most 12 comparisons, 192 units), then every 64
/// classified records. Returns `(less, greater)`: `values[..less] < pivot`,
/// `values[less..greater] == pivot` and `values[greater..] > pivot`.
fn partition<P: Progress, C: Fn(&Edge, &Edge) -> Ordering + Copy>(
    values: &mut [Edge],
    poll: &mut P,
    phase: &'static str,
    compare: C,
) -> Result<(usize, usize), CmgError> {
    #[cfg(all(test, feature = "parallel"))]
    let _probe = probe::enter(values.len());
    let (pivot, units) = pivot(values, compare);
    poll.advance(phase, units)?;
    let (mut less, mut index, mut greater) = (0, 0, values.len());
    let mut steps = 0;
    while index < greater {
        #[cfg(all(test, feature = "parallel"))]
        {
            crate::execution::visits::visit_within(core::ptr::from_ref(&values[index]).cast());
            probe::step();
        }
        match compare(&values[index], &pivot) {
            Ordering::Less => {
                values.swap(less, index);
                less += 1;
                index += 1;
            }
            Ordering::Greater => {
                greater -= 1;
                values.swap(index, greater);
            }
            Ordering::Equal => index += 1,
        }
        steps += 1;
        if steps == 64 {
            poll.advance(phase, 64 * 16)?;
            steps = 0;
        }
    }
    poll.advance(phase, 16 * steps.max(1))?;
    Ok((less, greater))
}

