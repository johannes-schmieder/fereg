//! Allocation helpers, lane runners and the immutable cell geometry.

use super::*;

pub(super) fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| allocation("unable to reserve block-elimination workspace"))
}

pub(super) fn with_capacity<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    reserve(&mut values, count)?;
    Ok(values)
}

pub(super) fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>> {
    let mut values = with_capacity(count)?;
    values.resize(count, value);
    Ok(values)
}

/// `values` becomes `count` copies of `value`, keeping its allocation.
pub(super) fn refill<T: Clone>(values: &mut Vec<T>, count: usize, value: T) -> Result<()> {
    values.clear();
    reserve(values, count)?;
    values.resize(count, value);
    Ok(())
}

/// `values` becomes a copy of `source`, keeping its allocation.
pub(super) fn refill_from<T: Clone>(values: &mut Vec<T>, source: &[T]) -> Result<()> {
    values.clear();
    reserve(values, source.len())?;
    values.extend_from_slice(source);
    Ok(())
}

/// Run `body(lane, part, item)` for every lane, in parallel by recursive
/// halving and without allocation: lane `l` owns the elements
/// `(bounds[l] - bounds[0]) * stride..(bounds[l + 1] - bounds[0]) * stride`
/// of `data` and its own state `items[l]`.
pub(super) fn each_lane<T, S, F>(
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
pub(super) fn each_lane2<A, B, S, F>(
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
pub(super) fn partition(offsets: &[usize], parts: usize) -> Result<Vec<Range<usize>>> {
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
pub(super) fn bounds_of(
    ranges: &[Range<usize>],
    map: impl Fn(usize) -> usize,
) -> Result<Vec<usize>> {
    let mut bounds = with_capacity(ranges.len() + 1)?;
    for range in ranges {
        bounds.push(map(range.start));
    }
    bounds.push(map(ranges.last().map_or(0, |range| range.end)));
    Ok(bounds)
}

pub(super) fn lane_count(work: usize) -> usize {
    if work < PARALLEL_WORK {
        1
    } else {
        rayon::current_num_threads().clamp(1, 64)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Lane {
    pub(super) segments: Range<usize>,
    pub(super) cells: Range<usize>,
}

/// Immutable cell geometry of one design; independent of weights and RHS.
pub(crate) struct Structure {
    pub(super) eliminated: usize,
    pub(super) blocks: Vec<usize>,
    pub(super) offsets: Vec<u32>,
    pub(super) reduced: usize,
    pub(super) segments: usize,
    pub(super) seg_cells: Vec<usize>,
    pub(super) codes: Vec<u32>,
    /// Rows per cell when any cell repeats a row combination; else empty.
    pub(super) multiplicity: Vec<f64>,
    /// Segment-local cell of each eliminated-effect membership entry.
    pub(super) member_cell: Vec<u32>,
    pub(super) lanes: Vec<Lane>,
    /// Segment and cell boundaries of `lanes`, and zeros of the same length.
    pub(super) seg_bounds: Vec<usize>,
    pub(super) cell_bounds: Vec<usize>,
    pub(super) zero_bounds: Vec<usize>,
    /// Cells of the largest segment.
    pub(super) max_seg_cells: usize,
    /// Segments with at least two cells. A one-cell segment's deviation from
    /// its mean is zero, so only these enter the reduced operator.
    pub(super) active: Vec<u32>,
    /// Operator lanes: ranges of `active` balanced by their cells, with the
    /// largest segment of each.
    pub(super) active_lanes: Vec<(Range<usize>, usize)>,
    pub(super) rows: Vec<Range<usize>>,
    pub(super) row_bounds: Vec<usize>,
    pub(super) nobs: usize,
    pub(super) bytes: u128,
}

pub(super) struct LaneBuild {
    pub(super) codes: Vec<u32>,
    pub(super) counts: Vec<u32>,
    pub(super) seg_cells: Vec<u32>,
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

    pub(super) fn build_nb<const NB: usize>(
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
pub(super) fn sampled_repeat_rate<const NB: usize>(
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
pub(super) fn build_lane<const NB: usize>(
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
