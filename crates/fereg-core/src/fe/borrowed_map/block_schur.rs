//! Exact global elimination of the largest categorical effect.
//!
//! An intercept-only design with effects `E_0..E_k` partitions its weighted
//! normal matrix as `[[D, C], [C', A]]`, with `D` the diagonal mass of the
//! effect that has the most levels. Those coefficients are eliminated exactly:
//! the remaining coefficients solve `S b = g` with `S = A - C' D^-1 C`, which
//! is applied implicitly. Each eliminated group gathers the remaining
//! coefficients of its distinct remaining-code cells, subtracts their weighted
//! group mean and scatters the weighted deviations back. Up to eight
//! right-hand sides advance in lockstep through one traversal. The eliminated
//! coefficients follow by back-substitution, and one observation pass forms
//! the residual while it recomputes every remaining level's true weighted
//! moment.
//!
//! Scope: an exact alternative solver for the same projection. The recurrence
//! stops at a tenth of the requested tolerance on the reduced residual, which
//! is the remaining blocks' moment vector, relative to the column's norm
//! about its eliminated group means. Acceptance uses the moments recomputed
//! from the formed residual with the usual certificate scale. Moments of the
//! eliminated levels vanish by construction up to rounding and are not
//! recomputed. A stall, an admission refusal or a numerical breakdown before
//! the residual pass returns the untouched input to the caller's route; a
//! failed certificate after it retries in place, since projecting a partially
//! residualized column yields the same residual.
//!
//! Memory: every allocation is fallible. The cache keeps the geometry, the
//! weight state and one workspace per batch width, so repeated projections of
//! one width with changing weights allocate nothing (unless the caller asks
//! for the residual norms attached to each result).

use super::*;
use crate::fe::LevelIndex;
use std::ops::Range;

/// Developer switch: `FEREG_BLOCK_SCHUR=0` keeps the established routes.
pub(crate) fn enabled() -> bool {
    #[cfg(test)]
    if TEST_DISABLED.load(std::sync::atomic::Ordering::SeqCst) > 0 {
        return false;
    }
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("FEREG_BLOCK_SCHUR").map_or(true, |v| v != "0"))
}

#[cfg(test)]
static TEST_DISABLED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Tests of the established routes' own machinery disable the elimination
/// while the guard lives. The switch is process-wide, so the guard also holds
/// an exclusive lock that tests comparing repeated runs share
/// ([`stable_route_for_test`]); their runs never straddle a switch.
#[cfg(test)]
pub(crate) struct DisabledForTest(#[allow(dead_code)] std::sync::RwLockWriteGuard<'static, ()>);

#[cfg(test)]
static TEST_ROUTE_LOCK: std::sync::RwLock<()> = std::sync::RwLock::new(());

#[cfg(test)]
pub(crate) fn disable_for_test() -> DisabledForTest {
    let lock = TEST_ROUTE_LOCK
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    TEST_DISABLED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    DisabledForTest(lock)
}

#[cfg(test)]
impl Drop for DisabledForTest {
    fn drop(&mut self) {
        TEST_DISABLED.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Shared side of the route lock for tests that compare repeated runs.
#[cfg(test)]
pub(crate) fn stable_route_for_test() -> std::sync::RwLockReadGuard<'static, ()> {
    TEST_ROUTE_LOCK
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Retained (non-eliminated) blocks with specialized kernels.
pub(crate) const MAX_BLOCKS: usize = 4;
/// Right-hand sides per lockstep batch.
pub(crate) const BATCH: usize = 8;
/// Below this many scalar updates a stage stays on the calling thread.
const PARALLEL_WORK: usize = 1 << 15;
/// Fixed reduction chunk for reduced-space vector operations, independent of
/// the worker count.
const CHUNK: usize = 2048;
/// Iteration ceiling before the route yields to the caller's solver.
const MAX_ITERATIONS: usize = 1500;
/// A column must halve its stopping measure every this many iterations ...
const STALL_WINDOW: usize = 40;
/// ... and every this many once it meets the acceptance rule.
const KEPT_WINDOW: usize = 8;
/// The recurrence stops at this fraction of the requested tolerance on the
/// column's within norm (see [`run`]).
const STOP_FRACTION: f64 = 0.1;
/// Columns whose within norm is below this fraction of the certificate scale
/// lie inside the eliminated span.
const WITHIN_FLOOR: f64 = 1.0 / (1_u64 << 30) as f64;
/// Acceptance rule: this fraction of the tolerance at the certificate scale
/// (where a column may stop when its recurrence reaches its rounding floor).
const ACCEPT_FRACTION: f64 = 0.5;

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| allocation("unable to reserve block-elimination workspace"))
}

fn with_capacity<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    reserve(&mut values, count)?;
    Ok(values)
}

fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>> {
    let mut values = with_capacity(count)?;
    values.resize(count, value);
    Ok(values)
}

/// `values` becomes `count` copies of `value`, keeping its allocation.
fn refill<T: Clone>(values: &mut Vec<T>, count: usize, value: T) -> Result<()> {
    values.clear();
    reserve(values, count)?;
    values.resize(count, value);
    Ok(())
}

/// `values` becomes a copy of `source`, keeping its allocation.
fn refill_from<T: Clone>(values: &mut Vec<T>, source: &[T]) -> Result<()> {
    values.clear();
    reserve(values, source.len())?;
    values.extend_from_slice(source);
    Ok(())
}

/// Run `body(lane, part, item)` for every lane, in parallel by recursive
/// halving and without allocation: lane `l` owns the elements
/// `(bounds[l] - bounds[0]) * stride..(bounds[l + 1] - bounds[0]) * stride`
/// of `data` and its own state `items[l]`.
fn each_lane<T, S, F>(
    first: usize,
    bounds: &[usize],
    stride: usize,
    data: &mut [T],
    items: &mut [S],
    body: &F,
) where
    T: Send,
    S: Send,
    F: Fn(usize, &mut [T], &mut S) + Sync,
{
    if items.len() <= 1 {
        if let Some(item) = items.first_mut() {
            body(first, data, item);
        }
        return;
    }
    let mid = items.len() / 2;
    let (left, right) = data.split_at_mut((bounds[mid] - bounds[0]) * stride);
    let (head, tail) = items.split_at_mut(mid);
    rayon::join(
        || each_lane(first, &bounds[..=mid], stride, left, head, body),
        || each_lane(first + mid, &bounds[mid..], stride, right, tail, body),
    );
}

/// [`each_lane`] over two arrays with their own bounds.
fn each_lane2<A, B, S, F>(
    first: usize,
    (a_bounds, a): (&[usize], &mut [A]),
    (b_bounds, b): (&[usize], &mut [B]),
    items: &mut [S],
    body: &F,
) where
    A: Send,
    B: Send,
    S: Send,
    F: Fn(usize, &mut [A], &mut [B], &mut S) + Sync,
{
    if items.len() <= 1 {
        if let Some(item) = items.first_mut() {
            body(first, a, b, item);
        }
        return;
    }
    let mid = items.len() / 2;
    let (a_left, a_right) = a.split_at_mut(a_bounds[mid] - a_bounds[0]);
    let (b_left, b_right) = b.split_at_mut(b_bounds[mid] - b_bounds[0]);
    let (head, tail) = items.split_at_mut(mid);
    rayon::join(
        || {
            each_lane2(
                first,
                (&a_bounds[..=mid], a_left),
                (&b_bounds[..=mid], b_left),
                head,
                body,
            )
        },
        || {
            each_lane2(
                first + mid,
                (&a_bounds[mid..], a_right),
                (&b_bounds[mid..], b_right),
                tail,
                body,
            )
        },
    );
}

/// `parts` consecutive ranges of `0..offsets.len()-1` with balanced totals.
fn partition(offsets: &[usize], parts: usize) -> Result<Vec<Range<usize>>> {
    let items = offsets.len() - 1;
    let total = offsets[items] - offsets[0];
    let parts = parts.clamp(1, items.max(1));
    let mut ranges = with_capacity(parts)?;
    let mut start = 0;
    for part in 1..=parts {
        let goal = offsets[0] + (total as u128 * part as u128 / parts as u128) as usize;
        let mut end = if part == parts {
            items
        } else {
            offsets.partition_point(|&o| o <= goal).saturating_sub(1)
        };
        end = end.clamp(start, items);
        ranges.push(start..end);
        start = end;
    }
    ranges.retain(|range| !range.is_empty());
    if ranges.is_empty() {
        ranges.push(0..items);
    }
    Ok(ranges)
}

/// Lane boundaries `map(start_0), .., map(start_last), map(end_last)`.
fn bounds_of(ranges: &[Range<usize>], map: impl Fn(usize) -> usize) -> Result<Vec<usize>> {
    let mut bounds = with_capacity(ranges.len() + 1)?;
    for range in ranges {
        bounds.push(map(range.start));
    }
    bounds.push(map(ranges.last().map_or(0, |range| range.end)));
    Ok(bounds)
}

fn lane_count(work: usize) -> usize {
    if work < PARALLEL_WORK {
        1
    } else {
        rayon::current_num_threads().clamp(1, 64)
    }
}

#[derive(Clone, Debug)]
struct Lane {
    segments: Range<usize>,
    cells: Range<usize>,
}

/// Immutable cell geometry of one design; independent of weights and RHS.
pub(crate) struct Structure {
    eliminated: usize,
    blocks: Vec<usize>,
    offsets: Vec<u32>,
    reduced: usize,
    segments: usize,
    seg_cells: Vec<usize>,
    codes: Vec<u32>,
    /// Rows per cell when any cell repeats a row combination; else empty.
    multiplicity: Vec<f64>,
    /// Segment-local cell of each eliminated-effect membership entry.
    member_cell: Vec<u32>,
    lanes: Vec<Lane>,
    /// Segment and cell boundaries of `lanes`, and zeros of the same length.
    seg_bounds: Vec<usize>,
    cell_bounds: Vec<usize>,
    zero_bounds: Vec<usize>,
    /// Cells of the largest segment.
    max_seg_cells: usize,
    /// Segments with at least two cells. A one-cell segment's deviation from
    /// its mean is zero, so only these enter the reduced operator.
    active: Vec<u32>,
    /// Operator lanes: ranges of `active` balanced by their cells, with the
    /// largest segment of each.
    active_lanes: Vec<(Range<usize>, usize)>,
    rows: Vec<Range<usize>>,
    row_bounds: Vec<usize>,
    nobs: usize,
    bytes: u128,
}

struct LaneBuild {
    codes: Vec<u32>,
    counts: Vec<u32>,
    seg_cells: Vec<u32>,
}

impl Structure {
    pub(crate) fn cells(&self) -> usize {
        self.seg_cells[self.segments]
    }

    /// Build the geometry, or `None` when the design is outside the route.
    pub(crate) fn build(design: &BorrowedInterceptFeDesign<'_>) -> Result<Option<Self>> {
        let k = design.effects().len();
        let n = design.nobs();
        if !(2..=MAX_BLOCKS + 1).contains(&k) || n == 0 || n >= u32::MAX as usize {
            return Ok(None);
        }
        let levels = |e: usize| design.effects()[e].n_levels();
        let eliminated = (0..k)
            .max_by_key(|&e| (levels(e), std::cmp::Reverse(e)))
            .expect("nonempty design");
        let mut blocks = with_capacity(k - 1)?;
        blocks.extend((0..k).filter(|&e| e != eliminated));
        let mut offsets = with_capacity(blocks.len())?;
        let mut reduced = 0_usize;
        for &b in &blocks {
            let Ok(offset) = u32::try_from(reduced) else {
                return Ok(None);
            };
            offsets.push(offset);
            reduced = reduced.saturating_add(levels(b));
        }
        if reduced == 0 || reduced >= u32::MAX as usize {
            return Ok(None);
        }
        match blocks.len() {
            1 => Self::build_nb::<1>(design, eliminated, blocks, offsets, reduced),
            2 => Self::build_nb::<2>(design, eliminated, blocks, offsets, reduced),
            3 => Self::build_nb::<3>(design, eliminated, blocks, offsets, reduced),
            4 => Self::build_nb::<4>(design, eliminated, blocks, offsets, reduced),
            _ => Ok(None),
        }
    }

    fn build_nb<const NB: usize>(
        design: &BorrowedInterceptFeDesign<'_>,
        eliminated: usize,
        blocks: Vec<usize>,
        offsets: Vec<u32>,
        reduced: usize,
    ) -> Result<Option<Self>> {
        let n = design.nobs();
        let effect = &design.effects()[eliminated];
        let index = effect.level_index();
        let member_offsets = index.membership_offsets();
        let segments = effect.n_levels();
        if member_offsets.len() != segments + 1 || member_offsets[segments] != n {
            return Ok(None);
        }
        let codes_b: [&[u32]; NB] = std::array::from_fn(|b| design.effects()[blocks[b]].codes());
        let offs: [u32; NB] = std::array::from_fn(|b| offsets[b]);
        let ranges = partition(member_offsets, lane_count(n * NB))?;
        // Merging repeated combinations pays only when they are common.
        let merge = sampled_repeat_rate::<NB>(index, segments, &codes_b)? >= 0.02;
        let mut member_cell = filled(n, 0_u32)?;
        let member_bounds = bounds_of(&ranges, |s| member_offsets[s])?;
        let mut built: Vec<Option<Result<LaneBuild>>> = with_capacity(ranges.len())?;
        built.resize_with(ranges.len(), || None);
        each_lane(
            0,
            &member_bounds,
            1,
            &mut member_cell,
            &mut built,
            &|lane, part, slot| {
                *slot = Some(build_lane::<NB>(
                    index,
                    ranges[lane].clone(),
                    &codes_b,
                    &offs,
                    part,
                    merge,
                ));
            },
        );
        let mut lanes_built = with_capacity(built.len())?;
        for lane in built {
            lanes_built.push(lane.expect("every lane ran")?);
        }
        crate::interrupt::checkpoint()?;
        let cells: usize = lanes_built.iter().map(|l| l.counts.len()).sum();
        let mut seg_cells = filled(segments + 1, 0_usize)?;
        let mut codes = with_capacity(cells * NB)?;
        let repeated = lanes_built
            .iter()
            .any(|l| l.counts.iter().any(|&count| count > 1));
        let mut multiplicity = with_capacity(if repeated { cells } else { 0 })?;
        let mut s = 0;
        for lane in &lanes_built {
            for &count in &lane.seg_cells {
                if count == 0 {
                    return Ok(None);
                }
                seg_cells[s + 1] = seg_cells[s] + count as usize;
                s += 1;
            }
            codes.extend_from_slice(&lane.codes);
            if repeated {
                multiplicity.extend(lane.counts.iter().map(|&c| f64::from(c)));
            }
        }
        drop(lanes_built);
        if s != segments {
            return Err(invalid_argument("block-elimination segment count differs"));
        }
        let lane_ranges = partition(&seg_cells, lane_count(cells * NB * BATCH))?;
        let mut lanes = with_capacity(lane_ranges.len())?;
        lanes.extend(lane_ranges.iter().map(|segments| Lane {
            cells: seg_cells[segments.start]..seg_cells[segments.end],
            segments: segments.clone(),
        }));
        let seg_bounds = bounds_of(&lane_ranges, |s| s)?;
        let cell_bounds = bounds_of(&lane_ranges, |s| seg_cells[s])?;
        let zero_bounds = filled(lanes.len() + 1, 0_usize)?;
        drop(lane_ranges);
        let mut active_count = 0;
        let mut max_seg_cells = 0;
        for s in 0..segments {
            let count = seg_cells[s + 1] - seg_cells[s];
            max_seg_cells = max_seg_cells.max(count);
            active_count += usize::from(count >= 2);
        }
        let mut active = with_capacity(active_count)?;
        let mut active_offsets = with_capacity(active_count + 1)?;
        active_offsets.push(0_usize);
        for s in 0..segments {
            let count = seg_cells[s + 1] - seg_cells[s];
            if count >= 2 {
                active.push(s as u32);
                active_offsets.push(active_offsets[active.len() - 1] + count);
            }
        }
        let active_cells = active_offsets[active.len()];
        let active_ranges = partition(&active_offsets, lane_count(active_cells * NB * BATCH))?;
        drop(active_offsets);
        let mut active_lanes = with_capacity(active_ranges.len())?;
        for range in active_ranges {
            let max_cells = active[range.clone()]
                .iter()
                .map(|&s| seg_cells[s as usize + 1] - seg_cells[s as usize])
                .max()
                .unwrap_or(0);
            active_lanes.push((range, max_cells));
        }
        let row_lanes = lane_count(n * (NB + 1) * BATCH);
        let per = n.div_ceil(row_lanes);
        let mut rows = with_capacity(row_lanes)?;
        rows.extend(
            (0..row_lanes)
                .map(|l| (l * per).min(n)..((l + 1) * per).min(n))
                .filter(|r| !r.is_empty()),
        );
        let row_bounds = bounds_of(&rows, |row| row)?;
        let bytes = (codes.capacity() as u128) * 4
            + (multiplicity.capacity() as u128) * 8
            + (member_cell.capacity() as u128) * 4
            + (active.capacity() as u128) * 4
            + (seg_cells.capacity() as u128) * 8;
        Ok(Some(Self {
            eliminated,
            blocks,
            offsets,
            reduced,
            segments,
            seg_cells,
            codes,
            multiplicity,
            member_cell,
            lanes,
            seg_bounds,
            cell_bounds,
            zero_bounds,
            max_seg_cells,
            active,
            active_lanes,
            rows,
            row_bounds,
            nobs: n,
            bytes,
        }))
    }
}

/// Fraction of rows that repeat a remaining-code combination within their
/// eliminated group, estimated on evenly spaced segments.
fn sampled_repeat_rate<const NB: usize>(
    index: &LevelIndex,
    segments: usize,
    codes_b: &[&[u32]; NB],
) -> Result<f64> {
    let step = (segments / 512).max(1);
    let (mut rows, mut repeats) = (0_usize, 0_usize);
    let mut local: Vec<[u32; NB]> = Vec::new();
    for s in (0..segments).step_by(step) {
        let members = index.members(s);
        local.clear();
        reserve(&mut local, members.len())?;
        local.extend(
            members
                .iter()
                .map(|&row| std::array::from_fn(|b| codes_b[b][row])),
        );
        local.sort_unstable();
        rows += local.len();
        repeats += local.windows(2).filter(|pair| pair[0] == pair[1]).count();
    }
    Ok(if rows == 0 {
        0.0
    } else {
        repeats as f64 / rows as f64
    })
}

/// One lane's cells. With `merge`, rows sharing a remaining-code combination
/// within a segment form one cell with their total weight; without it every
/// row is its own cell, which defines the same Schur operator.
fn build_lane<const NB: usize>(
    index: &LevelIndex,
    segs: Range<usize>,
    codes_b: &[&[u32]; NB],
    offs: &[u32; NB],
    member_cell: &mut [u32],
    merge: bool,
) -> Result<LaneBuild> {
    let mut out = LaneBuild {
        codes: with_capacity(member_cell.len() * NB)?,
        counts: with_capacity(member_cell.len())?,
        seg_cells: with_capacity(segs.len())?,
    };
    let mut local: Vec<([u32; NB], u32)> = Vec::new();
    let mut cursor = 0_usize;
    for (step, s) in segs.enumerate() {
        if step % 4096 == 0 {
            crate::interrupt::checkpoint()?;
        }
        let members = index.members(s);
        if !merge {
            for (k, &row) in members.iter().enumerate() {
                for b in 0..NB {
                    out.codes.push(codes_b[b][row] + offs[b]);
                }
                out.counts.push(1);
                member_cell[cursor + k] = k as u32;
            }
            cursor += members.len();
            out.seg_cells.push(members.len() as u32);
            continue;
        }
        local.clear();
        reserve(&mut local, members.len())?;
        for (k, &row) in members.iter().enumerate() {
            let key: [u32; NB] = std::array::from_fn(|b| codes_b[b][row] + offs[b]);
            local.push((key, k as u32));
        }
        if local.len() > 1 {
            local.sort_unstable();
        }
        let mut cells = 0_u32;
        let mut previous: Option<[u32; NB]> = None;
        for &(key, k) in &local {
            if previous != Some(key) {
                out.codes.extend_from_slice(&key);
                out.counts.push(0);
                cells += 1;
                previous = Some(key);
            }
            *out.counts.last_mut().expect("open cell") += 1;
            member_cell[cursor + k as usize] = cells - 1;
        }
        cursor += members.len();
        out.seg_cells.push(cells);
    }
    Ok(out)
}

/// Current-weight cell masses, segment inverses and the Schur diagonal.
pub(crate) struct Weights {
    /// Cell weights; empty when every cell has unit weight.
    cell: Vec<f64>,
    seg_inverse: Vec<f64>,
    mass: Vec<f64>,
    /// One retained block: each level's mass in the active segments.
    active_mass: Vec<f64>,
    inverse_diagonal: Vec<f64>,
    inverse_root_mass: Vec<f64>,
    raw_scale: f64,
    /// Total normalized weight of the sample.
    total_mass: f64,
    /// Per-lane accumulators of the preparation pass.
    lanes: Vec<WeightLane>,
    bytes: u128,
}

#[derive(Default)]
struct WeightLane {
    mass: Vec<f64>,
    diag: Vec<f64>,
    active: Vec<f64>,
    /// Stamped per-segment level sums (two or more retained blocks).
    sums: Vec<f64>,
    stamp: Vec<u32>,
    touched: Vec<u32>,
    bad: bool,
}

impl Weights {
    fn empty() -> Self {
        Self {
            cell: Vec::new(),
            seg_inverse: Vec::new(),
            mass: Vec::new(),
            active_mass: Vec::new(),
            inverse_diagonal: Vec::new(),
            inverse_root_mass: Vec::new(),
            raw_scale: 1.0,
            total_mass: 0.0,
            lanes: Vec::new(),
            bytes: 0,
        }
    }

    /// Prepare the state of `weights`, reusing the buffers of `recycled` (a
    /// previous state of the same structure) when given; then repeated
    /// preparations allocate nothing.
    fn prepare(
        st: &Structure,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: ValidatedWeights<'_>,
        recycled: Option<Self>,
    ) -> Result<Option<Self>> {
        let mut state = recycled.unwrap_or_else(Self::empty);
        Ok(state.fill(st, design, weights)?.then_some(state))
    }

    /// Form the state in place; `false` when a mass is not positive.
    fn fill(
        &mut self,
        st: &Structure,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: ValidatedWeights<'_>,
    ) -> Result<bool> {
        let cells = st.cells();
        let index = design.effects()[st.eliminated].level_index();
        let member_offsets = index.membership_offsets();
        let m = st.reduced;
        let nb = st.blocks.len();
        let lanes = st.lanes.len();
        // Every weighted cell is re-formed by the lane pass below.
        match weights.values {
            None if st.multiplicity.is_empty() => self.cell.clear(),
            None => refill_from(&mut self.cell, &st.multiplicity)?,
            Some(_) if self.cell.len() == cells => {}
            Some(_) => refill(&mut self.cell, cells, 0.0)?,
        }
        let unit = self.cell.is_empty();
        if self.seg_inverse.len() != st.segments {
            refill(&mut self.seg_inverse, st.segments, 0.0)?;
        }
        if self.lanes.len() != lanes {
            self.lanes.clear();
            reserve(&mut self.lanes, lanes)?;
            self.lanes.resize_with(lanes, WeightLane::default);
        }
        for lane in &mut self.lanes {
            refill(&mut lane.mass, m, 0.0)?;
            refill(&mut lane.diag, m, 0.0)?;
            if nb == 1 {
                refill(&mut lane.active, m, 0.0)?;
            } else {
                if lane.sums.len() != m {
                    refill(&mut lane.sums, m, 0.0)?;
                }
                refill(&mut lane.stamp, m, u32::MAX)?;
                lane.touched.clear();
                reserve(&mut lane.touched, st.max_seg_cells * nb)?;
            }
            lane.bad = false;
        }
        let values = weights.values;
        let scale = weights.raw_scale;
        let cell_bounds = if unit {
            &st.zero_bounds
        } else {
            &st.cell_bounds
        };
        each_lane2(
            0,
            (&st.seg_bounds, &mut self.seg_inverse),
            (cell_bounds, &mut self.cell),
            &mut self.lanes,
            &|l, inverse: &mut [f64], part: &mut [f64], acc: &mut WeightLane| {
                // One pass per segment: its cell weights (summed from the
                // member rows), then the per-segment level sums `a_j` in a
                // stamped lane accumulator; the Schur diagonal is
                // `mass_j - sum_s a_sj^2 / W_s`.
                let lane = &st.lanes[l];
                let WeightLane {
                    mass,
                    diag,
                    active,
                    sums,
                    stamp,
                    touched,
                    bad,
                } = acc;
                let mut cursor = member_offsets[lane.segments.start];
                for (s, inv) in lane.segments.clone().zip(inverse.iter_mut()) {
                    let (c0, c1) = (st.seg_cells[s], st.seg_cells[s + 1]);
                    let cw: &[f64] = if unit {
                        &[]
                    } else {
                        let local = &mut part[c0 - lane.cells.start..c1 - lane.cells.start];
                        if let Some(values) = values {
                            local.fill(0.0);
                            for &row in index.members(s) {
                                local[st.member_cell[cursor] as usize] += values[row] / scale;
                                cursor += 1;
                            }
                        }
                        local
                    };
                    if nb == 1 {
                        // One retained block: every cell of a segment has a
                        // distinct code, so `a_sj` is that cell's weight.
                        let codes = &st.codes[c0..c1];
                        let total: f64 = if unit {
                            (c1 - c0) as f64
                        } else {
                            cw.iter().sum()
                        };
                        if !(total.is_finite() && total > 0.0) {
                            *bad = true;
                            continue;
                        }
                        let inverse_total = 1.0 / total;
                        *inv = inverse_total;
                        if c1 - c0 == 1 {
                            // A one-cell segment adds mass but no curvature.
                            mass[codes[0] as usize] += total;
                            continue;
                        }
                        for (k, &code) in codes.iter().enumerate() {
                            let w = if unit { 1.0 } else { cw[k] };
                            let j = code as usize;
                            mass[j] += w;
                            active[j] += w;
                            diag[j] += w - w * w * inverse_total;
                        }
                        continue;
                    }
                    let mut total = 0.0;
                    touched.clear();
                    let tag = s as u32;
                    for (k, cc) in st.codes[c0 * nb..c1 * nb].chunks_exact(nb).enumerate() {
                        let w = if unit { 1.0 } else { cw[k] };
                        total += w;
                        for &code in cc {
                            let j = code as usize;
                            mass[j] += w;
                            if stamp[j] == tag {
                                sums[j] += w;
                            } else {
                                stamp[j] = tag;
                                sums[j] = w;
                                touched.push(code);
                            }
                        }
                    }
                    if !(total.is_finite() && total > 0.0) {
                        *bad = true;
                        continue;
                    }
                    let inverse_total = 1.0 / total;
                    *inv = inverse_total;
                    if c1 - c0 == 1 {
                        continue;
                    }
                    for &code in touched.iter() {
                        let j = code as usize;
                        let a = sums[j];
                        diag[j] += a - a * a * inverse_total;
                    }
                }
            },
        );
        if self.lanes.iter().any(|lane| lane.bad) {
            return Ok(false);
        }
        // Lane sums in lane order.
        let (first, rest) = self.lanes.split_first().expect("one lane");
        refill_from(&mut self.mass, &first.mass)?;
        refill_from(&mut self.inverse_diagonal, &first.diag)?;
        refill_from(&mut self.active_mass, &first.active)?;
        for other in rest {
            for (a, b) in self.mass.iter_mut().zip(&other.mass) {
                *a += b;
            }
            for (a, b) in self.inverse_diagonal.iter_mut().zip(&other.diag) {
                *a += b;
            }
            for (a, b) in self.active_mass.iter_mut().zip(&other.active) {
                *a += b;
            }
        }
        refill(&mut self.inverse_root_mass, m, 0.0)?;
        for j in 0..m {
            let total = self.mass[j];
            if !(total.is_finite() && total > 0.0) {
                return Ok(false);
            }
            let d = self.inverse_diagonal[j];
            self.inverse_diagonal[j] = if d.is_finite() && d > 1e-13 * total {
                1.0 / d
            } else {
                0.0
            };
            self.inverse_root_mass[j] = 1.0 / total.sqrt();
        }
        let lane_bytes: u128 = self
            .lanes
            .iter()
            .map(|lane| {
                (lane.mass.capacity()
                    + lane.diag.capacity()
                    + lane.active.capacity()
                    + lane.sums.capacity()) as u128
                    * 8
                    + (lane.stamp.capacity() + lane.touched.capacity()) as u128 * 4
            })
            .sum();
        self.bytes = (self.cell.capacity()
            + self.seg_inverse.capacity()
            + self.mass.capacity()
            + self.active_mass.capacity()
            + self.inverse_diagonal.capacity()
            + self.inverse_root_mass.capacity()) as u128
            * 8
            + lane_bytes;
        self.total_mass = self.seg_inverse.iter().map(|inverse| 1.0 / inverse).sum();
        self.raw_scale = weights.raw_scale;
        Ok(true)
    }
}

/// Caller options of one projection.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Request {
    /// Also return the FE coefficients whose fit forms the residual.
    pub(crate) retain_coefficients: bool,
    /// Also bound every level's Jacobi coefficient-error estimate (callers
    /// that check fitted predictors row by row, such as PPML).
    pub(crate) coefficient_accuracy: bool,
}

/// Raw-weighted Gram `Σ_i w_i r_i r_i'` of a certified residual matrix:
/// row-major and symmetric, `columns x columns`. Plain sums within each row
/// lane, combined in lane order (deterministic at a fixed worker count).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ResidualGram {
    pub(crate) columns: usize,
    pub(crate) values: Vec<f64>,
}

/// Outcome of one projection request.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Outcome<V> {
    Done(CertifiedBorrowedMap),
    /// The route declined; see [`project`] for the state of the input.
    Declined(V),
}

/// Identity of a weight vector: unit, or its slice address, length and a
/// digest of its bits (callers may refill one buffer between frames).
type WeightKey = Option<(usize, usize, u64)>;

fn weight_key(values: Option<&[f64]>) -> Result<WeightKey> {
    fn mix(h: u64, bits: u64) -> u64 {
        let h = (h ^ bits).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        h ^ (h >> 31)
    }
    let Some(values) = values else {
        return Ok(None);
    };
    // Fixed leaves and tree: the digest depends only on the bits.
    let digest = crate::parallel::fixed_tree_reduce(
        values.len(),
        1 << 16,
        &|range: Range<usize>| {
            let chunk = &values[range];
            // Four interleaved chains keep the multiplications independent.
            let mut h = [
                0x243f_6a88_85a3_08d3_u64,
                0x1319_8a2e_0370_7344,
                0xa409_3822_299f_31d0,
                0x082e_fa98_ec4e_6c89,
            ];
            let mut quads = chunk.chunks_exact(4);
            for quad in &mut quads {
                for lane in 0..4 {
                    h[lane] = mix(h[lane], quad[lane].to_bits());
                }
            }
            for (lane, v) in quads.remainder().iter().enumerate() {
                h[lane] = mix(h[lane], v.to_bits());
            }
            Ok(h.iter()
                .fold(chunk.len() as u64, |acc, &lane| mix(acc, lane)))
        },
        &|left, right| mix(mix(0xcbf2_9ce4_8422_2325, left), right),
    )?;
    Ok(Some((values.as_ptr() as usize, values.len(), digest)))
}

/// A session's retained geometry, its current-weight state and the reusable
/// workspaces of its projections.
#[derive(Default)]
pub(crate) struct Cache {
    refused: bool,
    structure: Option<Structure>,
    /// Weights keyed by the borrowed weight slice (address, length); `None`
    /// key is unit weights.
    weights: Option<(WeightKey, Weights)>,
    work: WorkSlots,
    /// Accumulate the residual Gram in later projections' residual pass.
    gram_requested: bool,
    gram: ResidualGram,
    gram_ready: bool,
    /// Residual norms of the last certified projection.
    norms: Vec<f64>,
    /// Keep the norms here instead of attaching a copy to each result.
    detach_norms: bool,
}

impl Cache {
    /// Also accumulate the residual Gram `Σ w r r'` at the caller's raw
    /// weights (unit weights: 1) in the residual pass of later projections
    /// with at most [`BATCH`] columns; see [`Cache::residual_gram`].
    #[allow(dead_code)] // For the PPML pre-solve's normal equations.
    pub(crate) fn request_residual_gram(&mut self, on: bool) {
        self.gram_requested = on;
    }

    /// The residual Gram of the last [`Cache::project`] call, when it was
    /// requested and the projection certified.
    #[allow(dead_code)] // For the PPML pre-solve's normal equations.
    pub(crate) fn residual_gram(&self) -> Option<&ResidualGram> {
        self.gram_ready.then_some(&self.gram)
    }

    /// Keep the residual norms of later certified projections only in the
    /// cache ([`Cache::residual_norms`]) instead of attaching a copy to each
    /// result, so repeated projections allocate nothing.
    #[allow(dead_code)] // For allocation-free PPML iterations.
    pub(crate) fn detach_residual_norms(&mut self, on: bool) {
        self.detach_norms = on;
    }

    /// `sqrt(Σ (w / max w) r^2)` per column of the last certified projection
    /// (empty after a decline).
    #[allow(dead_code)] // For allocation-free PPML iterations.
    pub(crate) fn residual_norms(&self) -> &[f64] {
        &self.norms
    }

    /// Project with the retained geometry, building it on first use. Any
    /// refusal is remembered so later calls go straight to the caller's route.
    pub(crate) fn project(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        map: MapConfig,
        budget: Option<FeSolverMemoryBudget>,
        request: Request,
    ) -> Result<Outcome<DenseMatrix>> {
        self.gram_ready = false;
        self.norms.clear();
        if self.refused || values.nrows() != design.nobs() {
            return Ok(Outcome::Declined(values));
        }
        let ledger = crate::diagnostics::phase_ledger::PhaseLedger::new(
            "block_schur_setup",
            ["structure", "weights"],
        );
        if self.structure.is_none() {
            self.structure = Structure::build(design)?;
            if self.structure.is_none() {
                self.refused = true;
                return Ok(Outcome::Declined(values));
            }
        }
        ledger.lap(0);
        let st = self.structure.as_ref().expect("structure");
        let validated = validate_weights(weights, values.nrows())?;
        let key = weight_key(validated.values)?;
        if budget.is_some_and(|b| !b.fits(admission_bytes(st, None, values.ncols()))) {
            return Ok(Outcome::Declined(values));
        }
        if self.weights.as_ref().is_none_or(|(k, _)| *k != key) {
            let recycled = self.weights.take().map(|(_, w)| w);
            match Weights::prepare(st, design, validated, recycled)? {
                Some(w) => self.weights = Some((key, w)),
                None => return Ok(Outcome::Declined(values)),
            }
        }
        ledger.lap(1);
        let w = &self.weights.as_ref().expect("weights").1;
        let mut bytes = admission_bytes(st, Some(w), values.ncols());
        if request.retain_coefficients {
            bytes += (st.reduced + st.segments) as u128 * values.ncols() as u128 * 8;
        }
        let outputs = Outputs {
            slots: &mut self.work,
            gram: self.gram_requested.then_some(&mut self.gram),
            norms: &mut self.norms,
            detach_norms: self.detach_norms,
        };
        let (outcome, gram) = project(
            st, w, design, validated, values, map, bytes, request, outputs,
        )?;
        match outcome {
            Outcome::Done(_) => self.gram_ready = gram,
            Outcome::Declined(_) => self.norms.clear(),
        }
        Ok(outcome)
    }

    /// Recompute every level's weighted moment of one residual column in one
    /// observation pass and certify it at `amplification` times its weighted
    /// norm (the derived scale of SPEC §16). `None` when this cache did not
    /// project with these weights.
    pub(crate) fn certify_derived(
        &self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        values: &DenseMatrix,
        tolerance: f64,
        amplification: f64,
    ) -> Result<Option<FeCertificate>> {
        let (Some(st), Some((key, w))) = (self.structure.as_ref(), self.weights.as_ref()) else {
            return Ok(None);
        };
        if values.ncols() != 1 || values.nrows() != st.nobs || design.nobs() != st.nobs {
            return Ok(None);
        }
        let validated = validate_weights(weights, values.nrows())?;
        if *key != weight_key(validated.values)? {
            return Ok(None);
        }
        if !(amplification.is_finite() && amplification >= 1.0) {
            return Err(invalid_argument(
                "derived residual amplification is invalid",
            ));
        }
        let elim_codes = design.effects()[st.eliminated].codes();
        let mut block_codes: Vec<&[u32]> = with_capacity(st.blocks.len())?;
        block_codes.extend(st.blocks.iter().map(|&b| design.effects()[b].codes()));
        let data = values.as_slice();
        let raw = validated.values;
        let scale = validated.raw_scale;
        struct Part {
            remaining: Vec<f64>,
            eliminated: Vec<f64>,
            sq: f64,
            finite: bool,
        }
        let mut parts: Vec<Part> = with_capacity(st.rows.len())?;
        for _ in &st.rows {
            parts.push(Part {
                remaining: filled(st.reduced, 0.0)?,
                eliminated: filled(st.segments, 0.0)?,
                sq: 0.0,
                finite: true,
            });
        }
        parts
            .par_iter_mut()
            .zip(st.rows.par_iter())
            .for_each(|(part, rows)| {
                for row in rows.clone() {
                    let r = data[row];
                    part.finite &= r.is_finite();
                    let wr = raw.map_or(r, |v| v[row] / scale * r);
                    part.sq += wr * r;
                    part.eliminated[elim_codes[row] as usize] += wr;
                    for (codes, &offset) in block_codes.iter().zip(&st.offsets) {
                        part.remaining[(codes[row] + offset) as usize] += wr;
                    }
                }
            });
        let (total, rest) = parts.split_first_mut().expect("one row lane");
        for part in rest.iter() {
            for (a, b) in total.remaining.iter_mut().zip(&part.remaining) {
                *a += b;
            }
            for (a, b) in total.eliminated.iter_mut().zip(&part.eliminated) {
                *a += b;
            }
            total.sq += part.sq;
            total.finite &= part.finite;
        }
        if !total.finite || !total.sq.is_finite() {
            return Err(invalid_data("derived residual values must be finite"));
        }
        let reference = amplification * total.sq.sqrt();
        if !reference.is_finite() {
            return Err(invalid_data("derived residual scale is not finite"));
        }
        let scales = [reference];
        let mut rhs_inf = [0.0];
        let mut fold = CertificateFold::default();
        for (s, &moment) in total.eliminated.iter().enumerate() {
            let mass = 1.0 / w.seg_inverse[s];
            fold.level(mass, &[moment / mass], &scales, &mut rhs_inf, w.raw_scale)?;
        }
        for (level, &moment) in total.remaining.iter().enumerate() {
            let mass = w.mass[level];
            fold.level(mass, &[moment / mass], &scales, &mut rhs_inf, w.raw_scale)?;
        }
        fold.finish(tolerance).map(Some)
    }
}

/// Bytes the solve adds to the retained structure and weights.
fn solve_bytes(st: &Structure, nrhs: usize) -> u128 {
    let r = nrhs.min(BATCH) as u128;
    let m = st.reduced as u128;
    let lanes = st.lanes.len().max(st.rows.len()).max(st.active_lanes.len()) as u128;
    let scratch: u128 = st.active_lanes.iter().map(|&(_, c)| c as u128).sum();
    let partials = 3 * m.div_ceil(CHUNK as u128).max(1);
    8 * r * (6 * m + lanes * m + scratch + st.segments as u128 + partials)
}

pub(crate) fn admission_bytes(st: &Structure, weights: Option<&Weights>, nrhs: usize) -> u128 {
    let weight_bytes = weights.map_or_else(
        || {
            let m = st.reduced as u128;
            let per_lane = if st.blocks.len() == 1 {
                3 * m * 8
            } else {
                3 * m * 8 + m * 4 + (st.max_seg_cells * st.blocks.len()) as u128 * 4
            };
            (st.cells() as u128 + st.segments as u128 + 5 * m) * 8
                + st.lanes.len() as u128 * per_lane
        },
        |w| w.bytes,
    );
    st.bytes + weight_bytes + solve_bytes(st, nrhs)
}

struct Ctx<'d, 'w, const NB: usize> {
    st: &'d Structure,
    w: &'d Weights,
    index: &'d LevelIndex,
    elim_codes: &'d [u32],
    codes_b: [&'d [u32]; NB],
    offs: [u32; NB],
    weights: ValidatedWeights<'w>,
    coefficient_accuracy: bool,
}

#[inline(always)]
fn gather<const R: usize>(cc: &[u32], p: &[[f64; R]]) -> [f64; R] {
    let mut g = p[cc[0] as usize];
    for &c in &cc[1..] {
        let q = &p[c as usize];
        for j in 0..R {
            g[j] += q[j];
        }
    }
    g
}

#[inline(always)]
fn scatter<const R: usize>(cc: &[u32], z: &[f64; R], acc: &mut [[f64; R]]) {
    for &c in cc {
        let a = &mut acc[c as usize];
        for j in 0..R {
            a[j] += z[j];
        }
    }
}

/// One lane of `acc = S p` restricted to its active segments (before the
/// merge).
fn apply_lane<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    p: &[[f64; R]],
    segments: &[u32],
    acc: &mut [[f64; R]],
    u: &mut [[f64; R]],
) {
    acc.fill([0.0; R]);
    if NB == 1 {
        // One retained block: every cell has one code, so
        // `S p = M_a p - sum_c w_c t_s(c) e_j(c)` with `t_s` the segment's
        // weighted mean of `p` and `M_a` the diagonal active mass; this lane
        // accumulates the second term ([`apply`] forms the difference).
        for &s in segments {
            let s = s as usize;
            let (c0, c1) = (st.seg_cells[s], st.seg_cells[s + 1]);
            let codes = &st.codes[c0..c1];
            let mut t = [0.0; R];
            if UNIT {
                for &c in codes {
                    let g = &p[c as usize];
                    for j in 0..R {
                        t[j] += g[j];
                    }
                }
            } else {
                for (&c, &wc) in codes.iter().zip(&w.cell[c0..c1]) {
                    let g = &p[c as usize];
                    for j in 0..R {
                        t[j] += wc * g[j];
                    }
                }
            }
            let inv = w.seg_inverse[s];
            for v in &mut t {
                *v *= inv;
            }
            if UNIT {
                for &c in codes {
                    let a = &mut acc[c as usize];
                    for j in 0..R {
                        a[j] += t[j];
                    }
                }
            } else {
                for (&c, &wc) in codes.iter().zip(&w.cell[c0..c1]) {
                    let a = &mut acc[c as usize];
                    for j in 0..R {
                        a[j] += wc * t[j];
                    }
                }
            }
        }
        return;
    }
    for &s in segments {
        let s = s as usize;
        let (c0, c1) = (st.seg_cells[s], st.seg_cells[s + 1]);
        let codes = &st.codes[c0 * NB..c1 * NB];
        let u = &mut u[..c1 - c0];
        let mut t = [0.0; R];
        if UNIT {
            for (cc, uc) in codes.chunks_exact(NB).zip(u.iter_mut()) {
                let g = gather::<R>(cc, p);
                for j in 0..R {
                    t[j] += g[j];
                }
                *uc = g;
            }
        } else {
            let cw = &w.cell[c0..c1];
            for ((cc, uc), &wc) in codes.chunks_exact(NB).zip(u.iter_mut()).zip(cw) {
                let g = gather::<R>(cc, p);
                for j in 0..R {
                    t[j] += wc * g[j];
                }
                *uc = g;
            }
        }
        let inv = w.seg_inverse[s];
        for v in &mut t {
            *v *= inv;
        }
        if UNIT {
            for (cc, uc) in codes.chunks_exact(NB).zip(u.iter()) {
                let mut z = *uc;
                for j in 0..R {
                    z[j] -= t[j];
                }
                scatter::<R>(cc, &z, acc);
            }
        } else {
            let cw = &w.cell[c0..c1];
            for ((cc, uc), &wc) in codes.chunks_exact(NB).zip(u.iter()).zip(cw) {
                let mut z = *uc;
                for j in 0..R {
                    z[j] = wc * (z[j] - t[j]);
                }
                scatter::<R>(cc, &z, acc);
            }
        }
    }
}

/// One lane's accumulator, cell scratch and pass results.
struct LaneWork<const R: usize> {
    acc: Vec<[f64; R]>,
    u: Vec<[f64; R]>,
    /// RHS pass: `Σ w v^2`, `max |v|`, centered `Σ w v^2`; residual pass:
    /// `Σ w r^2` in the first row.
    stats: [[f64; R]; 3],
    /// Lower triangle of the residual Gram.
    lower: [[f64; R]; R],
}

/// `lanes[0].acc += lanes[1].acc + ... + lanes[L-1].acc`, in lane order.
fn merge<const R: usize>(lanes: &mut [LaneWork<R>]) {
    let Some((first, rest)) = lanes.split_first_mut() else {
        return;
    };
    if rest.is_empty() {
        return;
    }
    let rest: &[LaneWork<R>] = rest;
    let add = |base: usize, chunk: &mut [[f64; R]]| {
        for other in rest {
            let len = chunk.len();
            for (v, o) in chunk.iter_mut().zip(&other.acc[base..base + len]) {
                for j in 0..R {
                    v[j] += o[j];
                }
            }
        }
    };
    if first.acc.len() * R * rest.len() < PARALLEL_WORK {
        add(0, &mut first.acc);
    } else {
        first
            .acc
            .par_chunks_mut(CHUNK)
            .enumerate()
            .for_each(|(i, chunk)| add(i * CHUNK, chunk));
    }
}

/// The solve workspace of one batch width; every buffer is overwritten
/// before it is read, so a retained workspace serves later solves as is.
struct Work<const R: usize> {
    x: Vec<[f64; R]>,
    /// Per column, the best iterate that met the acceptance rule.
    xs: Vec<[f64; R]>,
    r: Vec<[f64; R]>,
    z: Vec<[f64; R]>,
    p: Vec<[f64; R]>,
    lanes: Vec<LaneWork<R>>,
    seg: Vec<[f64; R]>,
    /// Partial sums of the reduced-space reductions, one per `CHUNK`.
    partials: Vec<[[f64; R]; 3]>,
}

impl<const R: usize> Work<R> {
    fn lane_count(st: &Structure) -> usize {
        st.lanes.len().max(st.rows.len()).max(st.active_lanes.len())
    }

    fn new(st: &Structure) -> Result<Self> {
        let m = st.reduced;
        let count = Self::lane_count(st);
        let mut lanes = with_capacity(count)?;
        for l in 0..count {
            let cells = st.active_lanes.get(l).map_or(0, |&(_, cells)| cells);
            lanes.push(LaneWork {
                acc: filled(m, [0.0; R])?,
                u: filled(cells, [0.0; R])?,
                stats: [[0.0; R]; 3],
                lower: [[0.0; R]; R],
            });
        }
        Ok(Self {
            x: filled(m, [0.0; R])?,
            xs: filled(m, [0.0; R])?,
            r: filled(m, [0.0; R])?,
            z: filled(m, [0.0; R])?,
            p: filled(m, [0.0; R])?,
            lanes,
            seg: filled(st.segments, [0.0; R])?,
            partials: filled(m.div_ceil(CHUNK).max(1), [[0.0; R]; 3])?,
        })
    }

    fn fits(&self, st: &Structure) -> bool {
        self.x.len() == st.reduced
            && self.seg.len() == st.segments
            && self.lanes.len() == Self::lane_count(st)
    }
}

/// One retained workspace per batch width.
#[derive(Default)]
struct WorkSlots {
    w1: Option<Work<1>>,
    w2: Option<Work<2>>,
    w3: Option<Work<3>>,
    w4: Option<Work<4>>,
    w5: Option<Work<5>>,
    w6: Option<Work<6>>,
    w7: Option<Work<7>>,
    w8: Option<Work<8>>,
}

trait Slotted: Sized {
    fn slot(slots: &mut WorkSlots) -> &mut Option<Self>;
}

macro_rules! slotted {
    ($($r:literal => $field:ident),*) => {
        $(impl Slotted for Work<$r> {
            fn slot(slots: &mut WorkSlots) -> &mut Option<Self> {
                &mut slots.$field
            }
        })*
    };
}
slotted!(1 => w1, 2 => w2, 3 => w3, 4 => w4, 5 => w5, 6 => w6, 7 => w7, 8 => w8);

fn apply<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    p: &[[f64; R]],
    lanes: &mut [LaneWork<R>],
) {
    let count = st.active_lanes.len();
    if count == 1 {
        let segments = &st.active[st.active_lanes[0].0.clone()];
        let lane = &mut lanes[0];
        apply_lane::<R, NB, UNIT>(st, w, p, segments, &mut lane.acc, &mut lane.u);
    } else {
        lanes[..count]
            .par_iter_mut()
            .zip(st.active_lanes.par_iter())
            .for_each(|(lane, (range, _))| {
                let segments = &st.active[range.clone()];
                apply_lane::<R, NB, UNIT>(st, w, p, segments, &mut lane.acc, &mut lane.u);
            });
        merge(&mut lanes[..count]);
    }
    if NB == 1 {
        for ((q, p), &mass) in lanes[0].acc.iter_mut().zip(p).zip(&w.active_mass) {
            for j in 0..R {
                q[j] = mass * p[j] - q[j];
            }
        }
    }
}

/// Per-RHS `a . b`, as plain sums over fixed chunks combined in chunk order.
fn dot<const R: usize>(a: &[[f64; R]], b: &[[f64; R]], partials: &mut [[[f64; R]; 3]]) -> [f64; R] {
    let body = |a: &[[f64; R]], b: &[[f64; R]]| {
        let mut s = [0.0; R];
        for (x, y) in a.iter().zip(b) {
            for j in 0..R {
                s[j] += x[j] * y[j];
            }
        }
        s
    };
    let m = a.len();
    let mut total = [0.0; R];
    if m * R < PARALLEL_WORK {
        let s = body(a, b);
        for j in 0..R {
            total[j] += s[j];
        }
    } else {
        a.par_chunks(CHUNK)
            .zip(b.par_chunks(CHUNK))
            .zip(partials.par_iter_mut())
            .for_each(|((a, b), slot)| slot[0] = body(a, b));
        for slot in &partials[..m.div_ceil(CHUNK)] {
            for (t, s) in total.iter_mut().zip(&slot[0]) {
                *t += s;
            }
        }
    }
    total
}

/// Column statistics of one batch from the RHS pass.
struct RhsNorms<const R: usize> {
    /// `Σ w v^2`.
    sq: [f64; R],
    /// `max |v|`.
    big: [f64; R],
    /// `Σ w (v - m_s)^2` about the eliminated groups' weighted means `m_s`.
    centered: [f64; R],
}

/// Reduced RHS: `g = A-part of D'Wv` minus the eliminated groups' means, the
/// segment sums `Σ w v` (kept for back-substitution) and the column norms.
fn rhs<const R: usize, const NB: usize, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    values: &DenseMatrix,
    first: usize,
    work: &mut Work<R>,
) -> RhsNorms<R> {
    let st = ctx.st;
    let stride = values.ncols();
    let data = values.as_slice();
    let count = st.lanes.len();
    let weights = ctx.weights.values.unwrap_or(&[]);
    let scale = ctx.weights.raw_scale;
    let Work { seg, lanes, .. } = work;
    each_lane(
        0,
        &st.seg_bounds,
        1,
        seg,
        &mut lanes[..count],
        &|l, seg: &mut [[f64; R]], lane: &mut LaneWork<R>| {
            let acc = &mut lane.acc;
            acc.fill([0.0; R]);
            let mut sq = [0.0; R];
            let mut big = [0.0_f64; R];
            let mut centered = [0.0; R];
            for (s, out) in st.lanes[l].segments.clone().zip(seg.iter_mut()) {
                let members = ctx.index.members(s);
                let mut sum = [0.0; R];
                for &row in members {
                    let v = &data[row * stride + first..row * stride + first + R];
                    let w = if WEIGHTED { weights[row] / scale } else { 1.0 };
                    for j in 0..R {
                        let wv = w * v[j];
                        sum[j] += wv;
                        sq[j] += wv * v[j];
                        big[j] = big[j].max(v[j].abs());
                    }
                }
                *out = sum;
                // Every row of a one-cell segment shares its remaining codes,
                // so their deviations from the segment mean add nothing to
                // `g`.
                let single = st.seg_cells[s + 1] - st.seg_cells[s] == 1;
                let inv = ctx.w.seg_inverse[s];
                let mut mean = sum;
                for value in &mut mean {
                    *value *= inv;
                }
                for &row in members {
                    let v = &data[row * stride + first..row * stride + first + R];
                    let w = if WEIGHTED { weights[row] / scale } else { 1.0 };
                    let mut z = [0.0; R];
                    for j in 0..R {
                        let d = v[j] - mean[j];
                        z[j] = w * d;
                        centered[j] += z[j] * d;
                    }
                    if single {
                        continue;
                    }
                    for b in 0..NB {
                        let a = &mut acc[(ctx.codes_b[b][row] + ctx.offs[b]) as usize];
                        for j in 0..R {
                            a[j] += z[j];
                        }
                    }
                }
            }
            lane.stats = [sq, big, centered];
        },
    );
    merge(&mut lanes[..count]);
    let mut norms = RhsNorms {
        sq: [0.0; R],
        big: [0.0; R],
        centered: [0.0; R],
    };
    for lane in &lanes[..count] {
        let [s, b, c] = lane.stats;
        for j in 0..R {
            norms.sq[j] += s[j];
            norms.big[j] = norms.big[j].max(b[j]);
            norms.centered[j] += c[j];
        }
    }
    norms
}

enum Solve {
    /// Iterations, the sum over iterations of the active columns, and the
    /// columns stopped at the acceptance rule after a breakdown.
    Converged(usize, usize, usize),
    Declined(&'static str),
}

/// `value / target` with an exact zero target accepting only exact zeros.
fn ratio(value: f64, target: f64) -> f64 {
    if target > 0.0 {
        value / target
    } else if value == 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}

/// Stopping thresholds of one batch: `goal` is where a column stops;
/// `accept` (the certificate-scale rule) is where a column may stop when its
/// recurrence breaks down near the rounding floor.
struct Targets<const R: usize> {
    goal: [f64; R],
    goal_coefficient: Option<[f64; R]>,
    accept: [f64; R],
    accept_coefficient: Option<[f64; R]>,
}

impl<const R: usize> Targets<R> {
    fn measure(
        target: &[f64; R],
        coefficient: Option<&[f64; R]>,
        inf: f64,
        zinf: f64,
        c: usize,
    ) -> f64 {
        ratio(inf, target[c]).max(coefficient.map_or(0.0, |t| ratio(zinf, t[c])))
    }

    fn goal(&self, inf: f64, zinf: f64, c: usize) -> f64 {
        Self::measure(&self.goal, self.goal_coefficient.as_ref(), inf, zinf, c)
    }

    fn accepted(&self, inf: f64, zinf: f64, c: usize) -> bool {
        Self::measure(&self.accept, self.accept_coefficient.as_ref(), inf, zinf, c) <= 1.0
    }
}

/// Lockstep Jacobi-preconditioned CG on `S x = r0` (r0 already in `work.r`).
/// A column stops when its largest scaled moment `|r_j| / sqrt(m_j)` is at
/// most its goal and, when a coefficient goal is given, its largest Jacobi
/// coefficient-error estimate `|r_j| / d_j` is at most that goal. A column
/// whose curvature vanishes, whose progress stalls or that reaches the
/// iteration ceiling returns to its best iterate that met the acceptance
/// rule; without one the route declines.
fn pcg<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    work: &mut Work<R>,
    targets: &Targets<R>,
    cap: usize,
) -> Result<Solve> {
    let m = st.reduced;
    let serial = m * R < PARALLEL_WORK;
    let inv_diag = &w.inverse_diagonal;
    let inv_root = &w.inverse_root_mass;
    // z = M^-1 r, rz, inf over the initial residual; x = 0, p = z.
    let mut rz = [0.0; R];
    let mut inf = [0.0_f64; R];
    let mut zinf = [0.0_f64; R];
    {
        let (r, z) = (&work.r, &mut work.z);
        for j in 0..m {
            for c in 0..R {
                z[j][c] = inv_diag[j] * r[j][c];
                rz[c] += r[j][c] * z[j][c];
                inf[c] = inf[c].max(r[j][c].abs() * inv_root[j]);
                zinf[c] = zinf[c].max(z[j][c].abs());
            }
        }
    }
    work.x.fill([0.0; R]);
    let mut active = [false; R];
    let mut progress = [0.0_f64; R];
    for c in 0..R {
        if !inf[c].is_finite() || !rz[c].is_finite() || !zinf[c].is_finite() {
            return Ok(Solve::Declined("nonfinite initial reduced residual"));
        }
        progress[c] = targets.goal(inf[c], zinf[c], c);
        active[c] = progress[c] > 1.0;
    }
    for (p, z) in work.p.iter_mut().zip(&work.z) {
        for c in 0..R {
            p[c] = if active[c] { z[c] } else { 0.0 };
        }
    }
    // The best iterate meeting the acceptance rule, per column (`xs`).
    let mut kept = [false; R];
    let mut kept_measure = [f64::INFINITY; R];
    work.xs.fill([0.0; R]);
    for c in 0..R {
        if active[c] && targets.accepted(inf[c], zinf[c], c) {
            kept[c] = true;
            kept_measure[c] = progress[c];
        }
    }
    let mut best = progress;
    let mut checkpoint_best = progress;
    let mut kept_checkpoint = progress;
    let mut iterations = 0;
    let mut column_iterations = 0;
    let mut accepted = 0;
    // Return the active columns of `mask` to their kept iterates, or decline.
    macro_rules! fall_back {
        ($mask:expr, $reason:literal) => {{
            let mask: [bool; R] = $mask;
            for c in 0..R {
                if mask[c] && !kept[c] {
                    return Ok(Solve::Declined($reason));
                }
            }
            if mask.contains(&true) {
                copy_columns(&mut work.x, &work.xs, mask);
                for c in 0..R {
                    if mask[c] {
                        active[c] = false;
                        accepted += 1;
                    }
                }
            }
        }};
    }
    while active.contains(&true) {
        if iterations >= cap {
            fall_back!(active, "iteration ceiling");
            break;
        }
        column_iterations += active.iter().filter(|&&on| on).count();
        crate::interrupt::checkpoint()?;
        apply::<R, NB, UNIT>(st, w, &work.p, &mut work.lanes);
        let pq = dot::<R>(&work.p, &work.lanes[0].acc, &mut work.partials);
        let mut alpha = [0.0; R];
        let mut flat = [false; R];
        for c in 0..R {
            if active[c] {
                if pq[c].is_finite() && pq[c] > 0.0 {
                    alpha[c] = rz[c] / pq[c];
                } else {
                    // No curvature left along the direction: the recurrence
                    // has reached its rounding floor.
                    flat[c] = true;
                }
            }
        }
        fall_back!(flat, "nonpositive reduced curvature");
        if !active.contains(&true) {
            break;
        }
        let mut next = [0.0; R];
        let mut inf = [0.0_f64; R];
        let mut zinf = [0.0_f64; R];
        {
            let Work {
                x,
                r,
                z,
                p,
                lanes,
                partials,
                ..
            } = &mut *work;
            let q = &lanes[0].acc;
            let body = |range: Range<usize>,
                        x: &mut [[f64; R]],
                        r: &mut [[f64; R]],
                        z: &mut [[f64; R]]| {
                let mut s = [0.0; R];
                let mut h = [0.0_f64; R];
                let mut hz = [0.0_f64; R];
                for (k, j) in range.enumerate() {
                    for c in 0..R {
                        x[k][c] += alpha[c] * p[j][c];
                        r[k][c] -= alpha[c] * q[j][c];
                        z[k][c] = inv_diag[j] * r[k][c];
                        s[c] += r[k][c] * z[k][c];
                        h[c] = h[c].max(r[k][c].abs() * inv_root[j]);
                        hz[c] = hz[c].max(z[k][c].abs());
                    }
                }
                [s, h, hz]
            };
            let mut fold = |[s, h, hz]: [[f64; R]; 3]| {
                for c in 0..R {
                    next[c] += s[c];
                    inf[c] = inf[c].max(h[c]);
                    zinf[c] = zinf[c].max(hz[c]);
                }
            };
            if serial {
                fold(body(0..m, x, r, z));
            } else {
                x.par_chunks_mut(CHUNK)
                    .zip(r.par_chunks_mut(CHUNK))
                    .zip(z.par_chunks_mut(CHUNK))
                    .zip(partials.par_iter_mut())
                    .enumerate()
                    .for_each(|(i, (((x, r), z), slot))| {
                        let start = i * CHUNK;
                        *slot = body(start..start + x.len(), x, r, z);
                    });
                for &slot in &partials[..m.div_ceil(CHUNK)] {
                    fold(slot);
                }
            }
        }
        iterations += 1;
        let mut beta = [0.0; R];
        let mut save = [false; R];
        for c in 0..R {
            if !active[c] {
                continue;
            }
            if !(inf[c].is_finite() && next[c].is_finite() && zinf[c].is_finite()) {
                return Ok(Solve::Declined("nonfinite reduced residual"));
            }
            let current = targets.goal(inf[c], zinf[c], c);
            best[c] = best[c].min(current);
            if current <= 1.0 {
                active[c] = false;
            } else {
                beta[c] = next[c] / rz[c];
                if current < kept_measure[c] && targets.accepted(inf[c], zinf[c], c) {
                    save[c] = true;
                    kept[c] = true;
                    kept_measure[c] = current;
                }
            }
            rz[c] = next[c];
        }
        if save.contains(&true) {
            copy_columns(&mut work.xs, &work.x, save);
        }
        if iterations % KEPT_WINDOW == 0 {
            // Little progress over the last window: fall back to the kept
            // iterate (short window), or leave hard graphs to the caller's
            // preconditioned solvers (long window).
            let long = iterations % STALL_WINDOW == 0;
            let mut stalled = [false; R];
            for c in 0..R {
                let reference = if kept[c] {
                    kept_checkpoint[c]
                } else {
                    checkpoint_best[c]
                };
                stalled[c] = active[c] && (kept[c] || long) && best[c] > 0.5 * reference;
            }
            fall_back!(stalled, "stalled reduced recurrence");
            kept_checkpoint = best;
            if long {
                checkpoint_best = best;
            }
        }
        let mask = active;
        let z = &work.z;
        let update = |p: &mut [[f64; R]], z: &[[f64; R]]| {
            for (p, z) in p.iter_mut().zip(z) {
                for c in 0..R {
                    p[c] = if mask[c] { z[c] + beta[c] * p[c] } else { 0.0 };
                }
            }
        };
        if serial {
            update(&mut work.p, z);
        } else {
            work.p
                .par_chunks_mut(CHUNK)
                .zip(z.par_chunks(CHUNK))
                .for_each(|(p, z)| update(p, z));
        }
    }
    Ok(Solve::Converged(iterations, column_iterations, accepted))
}

/// `to[.][c] = from[.][c]` for the columns of `mask`.
fn copy_columns<const R: usize>(to: &mut [[f64; R]], from: &[[f64; R]], mask: [bool; R]) {
    let body = |to: &mut [[f64; R]], from: &[[f64; R]]| {
        for (t, f) in to.iter_mut().zip(from) {
            for c in 0..R {
                if mask[c] {
                    t[c] = f[c];
                }
            }
        }
    };
    if to.len() * R < PARALLEL_WORK {
        body(to, from);
    } else {
        to.par_chunks_mut(CHUNK)
            .zip(from.par_chunks(CHUNK))
            .for_each(|(to, from)| body(to, from));
    }
}

/// `seg[s] = (Σ_s w v - Σ_c w_c (A x)_c) / W_s`, the eliminated coefficients.
fn recover<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    work: &mut Work<R>,
) {
    let count = st.lanes.len();
    let Work { x, seg, lanes, .. } = work;
    let x = &*x;
    each_lane(
        0,
        &st.seg_bounds,
        1,
        seg,
        &mut lanes[..count],
        &|l, seg: &mut [[f64; R]], _: &mut LaneWork<R>| {
            for (s, out) in st.lanes[l].segments.clone().zip(seg.iter_mut()) {
                let (c0, c1) = (st.seg_cells[s], st.seg_cells[s + 1]);
                let codes = &st.codes[c0 * NB..c1 * NB];
                let mut t = [0.0; R];
                if UNIT {
                    for cc in codes.chunks_exact(NB) {
                        let g = gather::<R>(cc, x);
                        for j in 0..R {
                            t[j] += g[j];
                        }
                    }
                } else {
                    for (cc, &wc) in codes.chunks_exact(NB).zip(&w.cell[c0..c1]) {
                        let g = gather::<R>(cc, x);
                        for j in 0..R {
                            t[j] += wc * g[j];
                        }
                    }
                }
                let inv = w.seg_inverse[s];
                for j in 0..R {
                    out[j] = (out[j] - t[j]) * inv;
                }
            }
        },
    );
}

/// Form the residual in place and accumulate its remaining-level moments into
/// the first lane's accumulator; returns `Σ w r^2` per RHS. With `gram`, also
/// forms the raw-weighted residual Gram `Σ w r r'` (plain sums per row lane,
/// combined in lane order).
fn residualize<const R: usize, const NB: usize, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    values: &mut DenseMatrix,
    first: usize,
    work: &mut Work<R>,
    gram: Option<&mut ResidualGram>,
) -> Result<[f64; R]> {
    let st = ctx.st;
    let stride = values.ncols();
    let weights = ctx.weights.values.unwrap_or(&[]);
    let scale = ctx.weights.raw_scale;
    let count = st.rows.len();
    let with_gram = gram.is_some();
    let Work { x, seg, lanes, .. } = work;
    let (x, alpha) = (&*x, &*seg);
    each_lane(
        0,
        &st.row_bounds,
        stride,
        values.as_mut_slice(),
        &mut lanes[..count],
        &|l, data: &mut [f64], lane: &mut LaneWork<R>| {
            let acc = &mut lane.acc;
            acc.fill([0.0; R]);
            let mut sq = [0.0; R];
            let mut lower = [[0.0; R]; R];
            for (row, values) in st.rows[l].clone().zip(data.chunks_exact_mut(stride)) {
                let mut f = alpha[ctx.elim_codes[row] as usize];
                for b in 0..NB {
                    let q = &x[(ctx.codes_b[b][row] + ctx.offs[b]) as usize];
                    for j in 0..R {
                        f[j] += q[j];
                    }
                }
                let w = if WEIGHTED { weights[row] / scale } else { 1.0 };
                let v = &mut values[first..first + R];
                let mut wr = [0.0; R];
                for j in 0..R {
                    let r = v[j] - f[j];
                    v[j] = r;
                    wr[j] = w * r;
                    sq[j] += wr[j] * r;
                }
                for b in 0..NB {
                    let a = &mut acc[(ctx.codes_b[b][row] + ctx.offs[b]) as usize];
                    for j in 0..R {
                        a[j] += wr[j];
                    }
                }
                if with_gram {
                    let raw = if WEIGHTED { weights[row] } else { 1.0 };
                    for (a, lower_row) in lower.iter_mut().enumerate() {
                        let wa = raw * v[a];
                        for (entry, &vb) in lower_row[..=a].iter_mut().zip(&v[..=a]) {
                            *entry += wa * vb;
                        }
                    }
                }
            }
            lane.stats[0] = sq;
            lane.lower = lower;
        },
    );
    merge(&mut lanes[..count]);
    let mut sq = [0.0; R];
    for lane in &lanes[..count] {
        for (total, part) in sq.iter_mut().zip(&lane.stats[0]) {
            *total += part;
        }
    }
    if let Some(gram) = gram {
        let mut lower = [[0.0; R]; R];
        for lane in &lanes[..count] {
            for (total, part) in lower.iter_mut().zip(&lane.lower) {
                for (a, b) in total.iter_mut().zip(part) {
                    *a += b;
                }
            }
        }
        refill(&mut gram.values, R * R, 0.0)?;
        gram.columns = R;
        for (a, lower_row) in lower.iter().enumerate() {
            for (b, &value) in lower_row[..=a].iter().enumerate() {
                gram.values[a * R + b] = value;
                gram.values[b * R + a] = value;
            }
        }
    }
    Ok(sq)
}

/// Per-call timing and iteration facts (trace only).
#[derive(Default, Clone, Copy)]
struct Facts {
    iterations: usize,
    /// Sum over batches and iterations of the active columns.
    column_iterations: usize,
    /// Columns stopped at the acceptance rule after a breakdown.
    accepted: usize,
    rhs_seconds: f64,
    pcg_seconds: f64,
    residual_seconds: f64,
}

enum Step {
    /// Declined before any residual was formed.
    Declined(&'static str),
    /// The residual of this batch was formed in place.
    Formed {
        fold: Option<CertificateFold>,
        norms: [f64; BATCH],
        reference: [f64; BATCH],
        /// Whether the residual Gram was formed.
        gram: bool,
    },
}

/// One batch `first..first+R`: RHS and recurrence read `values`; only a
/// converged recurrence forms the residual in place.
#[allow(clippy::too_many_arguments)]
fn run<const R: usize, const NB: usize, const UNIT: bool, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    values: &mut DenseMatrix,
    first: usize,
    reference: Option<&[f64]>,
    tolerance: f64,
    cap: usize,
    facts: &mut Facts,
    coefficients: Option<&mut FeCoefficientBatch>,
    slots: &mut WorkSlots,
    gram: Option<&mut ResidualGram>,
) -> Result<Step>
where
    Work<R>: Slotted,
{
    let st = ctx.st;
    let slot = <Work<R> as Slotted>::slot(slots);
    if slot.as_ref().is_none_or(|work| !work.fits(st)) {
        *slot = None;
        *slot = Some(Work::<R>::new(st)?);
    }
    let work = slot.as_mut().expect("workspace");
    let started = Instant::now();
    let norms = rhs::<R, NB, WEIGHTED>(ctx, values, first, work);
    std::mem::swap(&mut work.r, &mut work.lanes[0].acc);
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    let mut scale = [0.0; R];
    let mut within = [0.0; R];
    for j in 0..R {
        let norm = norms.sq[j].sqrt();
        if !norm.is_finite() || (norms.big[j] > 0.0 && !(1e-150..=1e150).contains(&norm)) {
            return Ok(Step::Declined("RHS scale outside the plain-sum range"));
        }
        scale[j] = reference.map_or(norm, |r| r[j]);
        // The reduced problem sees only the column without its eliminated
        // group means, so its accuracy is set at that norm (a large common
        // level would otherwise loosen it). A column inside the eliminated
        // span has only rounding left there and keeps the acceptance rule.
        let centered = norms.centered[j].max(0.0).sqrt().min(scale[j]);
        within[j] = if centered >= WITHIN_FLOOR * scale[j] {
            centered
        } else {
            f64::NAN
        };
    }
    // Coefficient accuracy: Jacobi error estimates within the tolerance of
    // the column's root-mean-square magnitude.
    let root_total = ctx.w.total_mass.sqrt();
    let mut targets = Targets {
        goal: [0.0; R],
        goal_coefficient: ctx.coefficient_accuracy.then_some([0.0; R]),
        accept: [0.0; R],
        accept_coefficient: ctx.coefficient_accuracy.then_some([0.0; R]),
    };
    for j in 0..R {
        targets.accept[j] = ACCEPT_FRACTION * tolerance * scale[j];
        targets.goal[j] = if within[j].is_nan() {
            targets.accept[j]
        } else {
            STOP_FRACTION * tolerance * within[j]
        };
        if let Some(t) = targets.accept_coefficient.as_mut() {
            t[j] = targets.accept[j] / root_total;
        }
        if let Some(t) = targets.goal_coefficient.as_mut() {
            t[j] = targets.goal[j] / root_total;
        }
    }
    let started = Instant::now();
    let solved = pcg::<R, NB, UNIT>(st, ctx.w, work, &targets, cap)?;
    facts.pcg_seconds += started.elapsed().as_secs_f64();
    let iterations = match solved {
        Solve::Converged(iterations, columns, accepted) => {
            facts.column_iterations += columns;
            facts.accepted += accepted;
            iterations
        }
        Solve::Declined(reason) => return Ok(Step::Declined(reason)),
    };
    facts.iterations = facts.iterations.max(iterations);
    let started = Instant::now();
    recover::<R, NB, UNIT>(st, ctx.w, work);
    if let Some(batch) = coefficients {
        // Corrections add their increments to the coefficients of the
        // preceding solve, so the sum fits the original columns.
        let per = batch.coefficients_per_rhs;
        for j in 0..R {
            let base = (first + j) * per;
            let start = base + batch.effect_offsets[st.eliminated];
            for (value, alpha) in batch.values[start..start + st.segments]
                .iter_mut()
                .zip(&work.seg)
            {
                *value += alpha[j];
            }
            for (b, &effect) in st.blocks.iter().enumerate() {
                let start = base + batch.effect_offsets[effect];
                let levels = batch.level_counts[effect];
                let offset = st.offsets[b] as usize;
                for (value, x) in batch.values[start..start + levels]
                    .iter_mut()
                    .zip(&work.x[offset..offset + levels])
                {
                    *value += x[j];
                }
            }
        }
    }
    let formed = gram.is_some();
    let sq = residualize::<R, NB, WEIGHTED>(ctx, values, first, work, gram)?;
    let (fold, norms) = certify_batch::<R>(ctx.w, &work.lanes[0].acc, sq, scale, tolerance);
    facts.residual_seconds += started.elapsed().as_secs_f64();
    let mut padded_norms = [0.0; BATCH];
    padded_norms[..R].copy_from_slice(&norms);
    let mut padded_scale = [0.0; BATCH];
    padded_scale[..R].copy_from_slice(&scale);
    Ok(Step::Formed {
        fold,
        norms: padded_norms,
        reference: padded_scale,
        gram: formed,
    })
}

/// Fold the recomputed remaining-level moments at `max(reference, ||r||)`.
fn certify_batch<const R: usize>(
    w: &Weights,
    moments: &[[f64; R]],
    sq: [f64; R],
    reference: [f64; R],
    tolerance: f64,
) -> (Option<CertificateFold>, [f64; R]) {
    let mut fold = CertificateFold::default();
    let mut scales = [0.0; R];
    let mut norms = [0.0; R];
    for j in 0..R {
        norms[j] = sq[j].sqrt();
        scales[j] = reference[j].max(norms[j]);
        if !scales[j].is_finite() {
            return (None, norms);
        }
    }
    let mut rhs_inf = [0.0; R];
    let mut means = [0.0; R];
    for (level, moment) in moments.iter().enumerate() {
        let mass = w.mass[level];
        let inverse = 1.0 / mass;
        for j in 0..R {
            means[j] = moment[j] * inverse;
        }
        if fold
            .level(mass, &means, &scales, &mut rhs_inf, w.raw_scale)
            .is_err()
        {
            return (None, norms);
        }
    }
    let passed = fold.relative_infinity_norm <= tolerance;
    (passed.then_some(fold), norms)
}

fn merge_fold(total: &mut CertificateFold, part: &CertificateFold) {
    total.absolute_infinity_norm = total
        .absolute_infinity_norm
        .max(part.absolute_infinity_norm);
    total.relative_infinity_norm = total
        .relative_infinity_norm
        .max(part.relative_infinity_norm);
    total.relative_square_sum += part.relative_square_sum;
    total.relative_count += part.relative_count;
}

#[allow(clippy::too_many_arguments)]
fn dispatch<const NB: usize>(
    ctx: &Ctx<'_, '_, NB>,
    values: &mut DenseMatrix,
    first: usize,
    width: usize,
    reference: Option<&[f64]>,
    tolerance: f64,
    cap: usize,
    facts: &mut Facts,
    coefficients: Option<&mut FeCoefficientBatch>,
    slots: &mut WorkSlots,
    gram: Option<&mut ResidualGram>,
) -> Result<Step> {
    let unit = ctx.w.cell.is_empty();
    let weighted = ctx.weights.values.is_some();
    macro_rules! go {
        ($r:literal) => {
            match (unit, weighted) {
                (true, false) => run::<$r, NB, true, false>(
                    ctx,
                    values,
                    first,
                    reference,
                    tolerance,
                    cap,
                    facts,
                    coefficients,
                    slots,
                    gram,
                ),
                (true, true) => run::<$r, NB, true, true>(
                    ctx,
                    values,
                    first,
                    reference,
                    tolerance,
                    cap,
                    facts,
                    coefficients,
                    slots,
                    gram,
                ),
                (false, false) => run::<$r, NB, false, false>(
                    ctx,
                    values,
                    first,
                    reference,
                    tolerance,
                    cap,
                    facts,
                    coefficients,
                    slots,
                    gram,
                ),
                (false, true) => run::<$r, NB, false, true>(
                    ctx,
                    values,
                    first,
                    reference,
                    tolerance,
                    cap,
                    facts,
                    coefficients,
                    slots,
                    gram,
                ),
            }
        };
    }
    match width {
        1 => go!(1),
        2 => go!(2),
        3 => go!(3),
        4 => go!(4),
        5 => go!(5),
        6 => go!(6),
        7 => go!(7),
        8 => go!(8),
        _ => Err(invalid_argument("block-elimination batch width is invalid")),
    }
}

/// The cache's reusable outputs and workspaces lent to one projection.
struct Outputs<'c> {
    slots: &'c mut WorkSlots,
    /// Requested residual Gram (formed only for at most [`BATCH`] columns).
    gram: Option<&'c mut ResidualGram>,
    /// Residual norms, one per column.
    norms: &'c mut Vec<f64>,
    detach_norms: bool,
}

/// Project the row-major RHS `values` against the design. A decline returns
/// the matrix: untouched, unless a later batch or a correction declined after
/// earlier residuals were formed, whose projection is then still the request.
/// Also reports whether the residual Gram was formed.
#[allow(clippy::too_many_arguments)]
fn project(
    st: &Structure,
    w: &Weights,
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: DenseMatrix,
    map: MapConfig,
    bytes: u128,
    request: Request,
    outputs: Outputs<'_>,
) -> Result<(Outcome<DenseMatrix>, bool)> {
    if values.nrows() != st.nobs || values.ncols() == 0 || design.nobs() != st.nobs {
        return Ok((Outcome::Declined(values), false));
    }
    match st.blocks.len() {
        1 => project_nb::<1>(st, w, design, weights, values, map, bytes, request, outputs),
        2 => project_nb::<2>(st, w, design, weights, values, map, bytes, request, outputs),
        3 => project_nb::<3>(st, w, design, weights, values, map, bytes, request, outputs),
        4 => project_nb::<4>(st, w, design, weights, values, map, bytes, request, outputs),
        _ => Ok((Outcome::Declined(values), false)),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_nb<const NB: usize>(
    st: &Structure,
    w: &Weights,
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    mut values: DenseMatrix,
    map: MapConfig,
    bytes: u128,
    request: Request,
    outputs: Outputs<'_>,
) -> Result<(Outcome<DenseMatrix>, bool)> {
    let mut ledger = crate::diagnostics::phase_ledger::PhaseLedger::new("block_schur", ["solve"]);
    let ncols = values.ncols();
    let Outputs {
        slots,
        gram,
        norms: residual_norms,
        detach_norms,
    } = outputs;
    let mut gram = gram.filter(|_| ncols <= BATCH);
    let ctx = Ctx::<NB> {
        st,
        w,
        index: design.effects()[st.eliminated].level_index(),
        elim_codes: design.effects()[st.eliminated].codes(),
        codes_b: std::array::from_fn(|b| design.effects()[st.blocks[b]].codes()),
        offs: std::array::from_fn(|b| st.offsets[b]),
        weights,
        coefficient_accuracy: request.coefficient_accuracy,
    };
    let tolerance = map.tolerance;
    let cap = map.max_iterations.clamp(1, MAX_ITERATIONS);
    let started = Instant::now();
    let mut facts = Facts::default();
    let mut fold = CertificateFold::default();
    residual_norms.clear();
    reserve(residual_norms, ncols)?;
    let mut corrections = 0;
    let mut gram_formed = false;
    let mut coefficients = if request.retain_coefficients {
        Some(FeCoefficientBatch::intercept_zeros(design, ncols)?)
    } else {
        None
    };
    let mut start = 0;
    while start < ncols {
        let width = (ncols - start).min(BATCH);
        let mut reference: Option<[f64; BATCH]> = None;
        let mut attempts = 0;
        loop {
            let step = dispatch::<NB>(
                &ctx,
                &mut values,
                start,
                width,
                reference.as_ref().map(|r| &r[..width]),
                tolerance,
                cap,
                &mut facts,
                coefficients.as_mut(),
                slots,
                gram.as_deref_mut(),
            )?;
            match step {
                Step::Declined(reason) => {
                    trace_decline(reason, facts);
                    return Ok((Outcome::Declined(values), false));
                }
                Step::Formed {
                    fold: Some(part),
                    norms,
                    gram: formed,
                    ..
                } => {
                    merge_fold(&mut fold, &part);
                    residual_norms.extend_from_slice(&norms[..width]);
                    gram_formed = formed;
                    break;
                }
                Step::Formed {
                    fold: None,
                    reference: scale,
                    ..
                } => {
                    // The residual is a valid input with the same projection;
                    // correct it in place at the original scale.
                    if attempts == 2 {
                        trace_decline("certificate failed after corrections", facts);
                        return Ok((Outcome::Declined(values), false));
                    }
                    attempts += 1;
                    corrections += 1;
                    reference = Some(scale);
                }
            }
        }
        start += width;
    }
    ledger.lap(0);
    ledger.facts(&[
        ("iterations", facts.iterations as u64),
        ("column_iterations", facts.column_iterations as u64),
        ("accepted_columns", facts.accepted as u64),
        ("columns", ncols as u64),
        ("cells", st.cells() as u64),
        ("active_segments", st.active.len() as u64),
        ("reduced", st.reduced as u64),
        ("lanes", st.lanes.len() as u64),
        ("corrections", corrections as u64),
        ("rhs_us", (facts.rhs_seconds * 1e6) as u64),
        ("pcg_us", (facts.pcg_seconds * 1e6) as u64),
        ("residual_us", (facts.residual_seconds * 1e6) as u64),
    ]);
    let certificate = fold.finish(tolerance)?;
    if !certificate.passed {
        trace_decline("combined certificate failed", facts);
        return Ok((Outcome::Declined(values), false));
    }
    let report = SolveReport {
        requested_solver: FeSolverKind::Auto,
        iterations: facts.iterations,
        solver_workspace_peak: SolverWorkspacePeak::from_bytes(bytes)?,
        // The certificate is fused into the residual pass, so its time is
        // part of the solve.
        solve_seconds: started.elapsed().as_secs_f64(),
        certification_seconds: 0.0,
        ..SolveReport::single_route(FeSolverKind::Cg, SolveStopReason::Certified, certificate)
    };
    let result = match coefficients {
        Some(coefficients) => CertifiedBorrowedMap::from_certified_parts_with_coefficients(
            values,
            report,
            coefficients,
        ),
        None => CertifiedBorrowedMap::from_certified_parts(values, report),
    }?;
    let result = if detach_norms {
        result
    } else {
        let mut attached = with_capacity(ncols)?;
        attached.extend_from_slice(residual_norms);
        result.with_residual_norms(attached)?
    };
    Ok((Outcome::Done(result), gram_formed))
}

fn trace_decline(reason: &str, facts: Facts) {
    if crate::diagnostics::solver_trace_enabled() || crate::diagnostics::phase_timing_enabled() {
        eprintln!(
            "FEREG_PHASE|block_schur_decline|reason={reason}|iterations={}|pcg_us={}",
            facts.iterations,
            (facts.pcg_seconds * 1e6) as u64
        );
    }
}

#[cfg(test)]
mod tests;
