//! Construction stages of the bounded Schur graph adapter that run on the
//! borrowed host team: the cached census, sampled-row preparation and hosted
//! edge emission. Each stage partitions by item counts, never by the worker
//! count, and admits its scratch before dispatch (the serial stage runs
//! otherwise). Census and preparation lanes count every unit and report the
//! earliest failing worker after the join; emission chunks have a declared
//! per-edge bound. Results equal the serial stage's exactly.

use cmg::CmgError;

use super::*;

/// Items below which a team stage runs serially; independent of the lanes.
pub(super) const TEAM_STAGE_ITEMS: usize = 65_536;
/// Raw cells per logical census task (a longer row forms its own task).
pub(super) const CENSUS_TASK_CELLS: usize = 4096;

/// One team census task: consecutive workers from `first`, their raw cells in
/// the cache bank and their (merged length, compensated total) slots.
struct CensusTask<'a> {
    first: usize,
    cells: &'a mut [(u32, f64)],
    rows: &'a mut [(usize, f64)],
    failure: Option<(usize, FeregError)>,
}

/// One team preparation task: consecutive workers from `first`, their merged
/// cache rows and their suffix ranges (each row's length plus one).
pub(super) struct PrepareTask<'a> {
    first: usize,
    cells: &'a mut [(u32, f64)],
    suffix: &'a mut [f64],
    failure: Option<(usize, FeregError)>,
}

/// Why a team lane stopped: the host's cancellation, or an ordinary fereg
/// error that the coordinator reports in worker order after the join.
pub(super) enum LaneStop {
    Cancelled,
    Fatal(FeregError),
}

impl From<FeregError> for LaneStop {
    fn from(error: FeregError) -> Self {
        Self::Fatal(error)
    }
}

/// One team lane's counter: the serial units and cadence, polling the host's
/// sticky cancellation query whenever the lane's own count crosses
/// `POLL_UNITS`, also inside a single long row.
pub(super) struct LaneWork<'a> {
    pending: u32,
    cancelled: cmg::Cancellation<'a>,
}

impl<'a> LaneWork<'a> {
    pub(super) fn new(cancelled: cmg::Cancellation<'a>) -> Self {
        Self {
            pending: 0,
            cancelled,
        }
    }
}

impl Counter for LaneWork<'_> {
    type Error = LaneStop;

    #[inline(always)]
    fn step(&mut self, units: u16) -> std::result::Result<(), LaneStop> {
        if units == 0 || units > 1024 {
            return Err(invalid_data("invalid bounded CMG work chunk").into());
        }
        self.pending += u32::from(units);
        if self.pending >= POLL_UNITS {
            self.pending = 0;
            if (self.cancelled)() {
                return Err(LaneStop::Cancelled);
            }
        }
        Ok(())
    }
}

/// Name the team stage a joined cancellation interrupted; other errors pass.
fn stage_phase(error: CmgError, phase: &'static str) -> CmgError {
    match error {
        CmgError::Cancelled { .. } => CmgError::Cancelled { phase },
        other => other,
    }
}

impl EdgeEmissionPlan {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit(
        &mut self,
        cache: &mut CanonicalCells,
        sampler: &mut Option<SampledTrees>,
        cutoff: usize,
        firms: usize,
        sampling: Sampling,
        edges: &mut Vec<(usize, usize, f64)>,
        work: &mut Work,
    ) -> Attempt<()> {
        let workers = cache.rows.len();
        // Edge offsets and auxiliary vertices by prefix over workers; the
        // sampled rows are prepared on the team below.
        let mut auxiliary = firms;
        let mut start = 0_usize;
        for worker in 0..workers {
            work.step(32)?;
            let count = cache.row(worker).len();
            let edge_count = if count > cutoff {
                if sampling.samples > 0 && sampler.is_some() {
                    (count - 1) * sampling.samples as usize
                } else {
                    count
                }
            } else {
                count * count.saturating_sub(1) / 2
            };
            self.offsets.push((start, auxiliary));
            start = start
                .checked_add(edge_count)
                .ok_or_else(|| allocation("CMG Schur edge count overflows"))?;
            if count > cutoff && sampler.is_none() {
                auxiliary += 1;
            }
        }
        self.offsets.push((start, auxiliary));
        // The admitted output bank never grows; the caller checks the count.
        if start > edges.capacity() {
            return Err(invalid_data("CMG Schur edge census changed during construction").into());
        }
        if sampler.is_some() {
            self.prepare_rows(cache, cutoff, work)?;
        }
        // The output bank becomes the edge slots in counted segments.
        while edges.len() < start {
            let next = (start - edges.len()).min(128);
            work.step(8 * next as u16)?;
            edges.resize(edges.len() + next, (0, 0, 0.0));
        }
        let chunk_len = self.chunk_len;
        let offsets = &self.offsets;
        let suffix = &self.suffix;
        let cache = &*cache;
        work.cmg(|team, cancelled| {
            let stage = cmg::ControlledStage::admit(edges.len(), chunk_len)?;
            stage.run(
                team.expect("emission plan retains the installed action"),
                edges,
                cancelled,
                |chunk_index, chunk| {
                    let first = chunk_index * chunk_len;
                    let mut worker = offsets.partition_point(|offset| offset.0 <= first) - 1;
                    for (position, edge) in chunk.iter_mut().enumerate() {
                        let index = first + position;
                        worker = edge_worker(offsets, worker, index);
                        let (edge_start, auxiliary) = offsets[worker];
                        let ordinal = index - edge_start;
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
                                &suffix[start + worker..=end + worker],
                                ordinal,
                            )
                        } else if cells.len() > cutoff {
                            (cells[ordinal].0 as usize, auxiliary, cells[ordinal].1)
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

    /// Sort each sampled row by weight and form its suffix masses on the team,
    /// in place in the cache and in that row's range of the suffix bank. Tasks
    /// group consecutive workers by merged cells; lanes count every unit and
    /// the earliest failing worker's error is returned after the join.
    pub(super) fn prepare_rows(
        &mut self,
        cache: &mut CanonicalCells,
        cutoff: usize,
        work: &mut Work,
    ) -> Attempt<()> {
        let workers = cache.rows.len();
        let mut table: Vec<PrepareTask<'_>> = work.capacity(self.tasks)?;
        {
            let mut cells = &mut cache.cells[..];
            let mut suffix = &mut self.suffix[..];
            let mut first = 0;
            let mut length = 0;
            let mut consumed = 0;
            for worker in 0..workers {
                work.step(16)?;
                let end = cache
                    .rows
                    .get(worker + 1)
                    .map_or(cells.len() + consumed, |row| row.0);
                length = end - consumed;
                if length >= CENSUS_TASK_CELLS || worker + 1 == workers {
                    let rows = worker + 1 - first;
                    let (task_cells, rest_cells) = std::mem::take(&mut cells).split_at_mut(length);
                    let (task_suffix, rest_suffix) =
                        std::mem::take(&mut suffix).split_at_mut(length + rows);
                    table.push(PrepareTask {
                        first,
                        cells: task_cells,
                        suffix: task_suffix,
                        failure: None,
                    });
                    cells = rest_cells;
                    suffix = rest_suffix;
                    consumed = end;
                    first = worker + 1;
                    length = 0;
                }
            }
            let _ = length;
        }
        let rows = &cache.rows;
        work.cmg(|team, cancelled| {
            let stage = cmg::ControlledStage::admit(table.len(), 1)?;
            stage
                .run(
                    team.expect("emission plan retains the installed action"),
                    &mut table,
                    cancelled,
                    |_, tasks| {
                        let mut lane = LaneWork::new(cancelled);
                        for task in tasks {
                            let base = rows[task.first].0;
                            let mut cell = 0;
                            let mut slot = 0;
                            let workers = task.suffix.len() - task.cells.len();
                            for offset in 0..workers {
                                let worker = task.first + offset;
                                let end = rows
                                    .get(worker + 1)
                                    .map_or(base + task.cells.len(), |row| row.0);
                                let count = end - base - cell;
                                if count > cutoff {
                                    let result = super::sampling::prepare_row(
                                        &mut task.cells[cell..cell + count],
                                        &mut task.suffix[slot..=slot + count],
                                        &mut lane,
                                    );
                                    match result {
                                        Ok(()) => {}
                                        Err(LaneStop::Cancelled) => {
                                            return Err(CmgError::Cancelled {
                                                phase: "CMG Schur sampled-row preparation",
                                            });
                                        }
                                        Err(LaneStop::Fatal(error)) => {
                                            task.failure = Some((worker, error));
                                            return Ok(());
                                        }
                                    }
                                } else if let Err(stop) = lane.step(16) {
                                    match stop {
                                        LaneStop::Cancelled => {
                                            return Err(CmgError::Cancelled {
                                                phase: "CMG Schur sampled-row preparation",
                                            });
                                        }
                                        LaneStop::Fatal(error) => {
                                            task.failure = Some((worker, error));
                                            return Ok(());
                                        }
                                    }
                                }
                                cell += count;
                                slot += count + 1;
                            }
                        }
                        Ok(())
                    },
                )
                .map_err(|error| stage_phase(error, "CMG Schur sampled-row preparation"))
        })?;
        #[cfg(test)]
        {
            work.team_stages |= TEAM_PREPARE;
        }
        for task in &mut table {
            work.step(16)?;
            if let Some((_, error)) = task.failure.take() {
                return Err(error.into());
            }
        }
        Ok(())
    }
}

impl BoundedCells<'_> {
    /// Load, sort and merge every worker's row on the host team directly into
    /// the admitted canonical cache. Workers are grouped into logical tasks of
    /// at least `CENSUS_TASK_CELLS` raw cells (a longer row is its own task);
    /// each task owns its raw range of the cache bank, so no lane shares
    /// scratch, and lanes count every unit, inside a single long row too. The
    /// merged rows are then compacted in worker order into exactly the serial
    /// census's cache layout. The decision depends on item counts, never on the
    /// worker count. Returns `None` (the serial census runs) without a cache,
    /// a team or enough cells, or when the task table is
    /// not admitted; otherwise the earliest failing worker and its error.
    pub(super) fn load_rows_on_team(
        &mut self,
        work: &mut Work,
        memory: &mut Admission,
        parents: u128,
    ) -> Attempt<Option<Option<(usize, FeregError)>>> {
        let source = self.source;
        let weight_scale = self.weight_scale;
        let Some(cache) = self.cache.as_mut() else {
            return Ok(None);
        };
        let CellSource::Observations { membership, .. } = source else {
            return Ok(None);
        };
        let cells_total = membership.nobs();
        if cells_total < TEAM_STAGE_ITEMS || !work.cmg(|team, _| Ok(team.is_some()))? {
            return Ok(None);
        }
        let workers = source.worker_count();
        let mut tasks = 0;
        let mut pending = 0;
        for worker in 0..workers {
            work.step(16)?;
            pending += source.row_count(worker);
            if pending >= CENSUS_TASK_CELLS || worker + 1 == workers {
                tasks += 1;
                pending = 0;
            }
        }
        let table = bytes::<CensusTask<'_>>(tasks)?;
        if !fits(memory, parents, table) {
            return Ok(None);
        }
        admit(memory, work, parents, table)?;
        while cache.cells.len() < cells_total {
            let next = (cells_total - cache.cells.len()).min(128);
            work.step(8 * next as u16)?;
            cache.cells.resize(cache.cells.len() + next, (0, 0.0));
        }
        while cache.rows.len() < workers {
            let next = (workers - cache.rows.len()).min(128);
            work.step(8 * next as u16)?;
            cache.rows.resize(cache.rows.len() + next, (0, 0.0));
        }
        let mut table: Vec<CensusTask<'_>> = work.capacity(tasks)?;
        {
            let mut cells = &mut cache.cells[..];
            let mut rows = &mut cache.rows[..];
            let mut first = 0;
            let mut length = 0;
            for worker in 0..workers {
                work.step(16)?;
                length += source.row_count(worker);
                if length >= CENSUS_TASK_CELLS || worker + 1 == workers {
                    let (task_cells, rest_cells) = std::mem::take(&mut cells).split_at_mut(length);
                    let (task_rows, rest_rows) =
                        std::mem::take(&mut rows).split_at_mut(worker + 1 - first);
                    table.push(CensusTask {
                        first,
                        cells: task_cells,
                        rows: task_rows,
                        failure: None,
                    });
                    cells = rest_cells;
                    rows = rest_rows;
                    first = worker + 1;
                    length = 0;
                }
            }
        }
        work.cmg(|team, cancelled| {
            let stage = cmg::ControlledStage::admit(table.len(), 1)?;
            stage
                .run(
                    team.expect("checked team"),
                    &mut table,
                    cancelled,
                    |_, tasks| {
                        let mut lane = LaneWork::new(cancelled);
                        for task in tasks {
                            let mut start = 0;
                            for (offset, row) in task.rows.iter_mut().enumerate() {
                                let worker = task.first + offset;
                                let count = source.row_count(worker);
                                let cells = &mut task.cells[start..start + count];
                                match load_row(source, weight_scale, worker, cells, &mut lane) {
                                    Ok(loaded) => *row = loaded,
                                    Err(LaneStop::Cancelled) => {
                                        return Err(CmgError::Cancelled {
                                            phase: "CMG Schur team census",
                                        });
                                    }
                                    Err(LaneStop::Fatal(error)) => {
                                        task.failure = Some((worker, error));
                                        return Ok(());
                                    }
                                }
                                start += count;
                            }
                        }
                        Ok(())
                    },
                )
                .map_err(|error| stage_phase(error, "CMG Schur team census"))
        })?;
        #[cfg(test)]
        {
            work.team_stages |= TEAM_CENSUS;
        }
        let mut failure = None;
        for task in &mut table {
            work.step(16)?;
            if let Some(found) = task.failure.take() {
                failure = Some(found);
                break;
            }
        }
        drop(table);
        // Compact the merged rows in worker order: row starts and the cells
        // between them equal the serial census's cache exactly. The census
        // reads the rows before a failing worker, so that worker's start
        // closes the last of them.
        let mut raw = 0;
        let mut write = 0;
        for worker in 0..workers {
            if failure
                .as_ref()
                .is_some_and(|(failed, _)| *failed == worker)
            {
                cache.rows[worker].0 = write;
                break;
            }
            let (kept, total) = cache.rows[worker];
            let mut moved = 0;
            while moved < kept {
                let next = (kept - moved).min(64);
                work.step(16 * next as u16)?;
                cache
                    .cells
                    .copy_within(raw + moved..raw + moved + next, write + moved);
                moved += next;
            }
            cache.rows[worker] = (write, total);
            write += kept;
            raw += source.row_count(worker);
        }
        cache.cells.truncate(write);
        Ok(Some(failure))
    }
}

/// The worker whose edge range holds `index`, starting from the worker of
/// an earlier edge in the same chunk. Workers without edges share their
/// successor's start, so a run of them is crossed by one bisection: at most
/// one probe plus the bit length of the worker count, the steps that the
/// declared per-edge work includes.
#[inline]
fn edge_worker(offsets: &[(usize, usize)], worker: usize, index: usize) -> usize {
    #[cfg(test)]
    let mut probes = 1;
    let mut lower = worker + 1;
    if offsets[lower].0 > index {
        #[cfg(test)]
        emission_probes::record(probes);
        return worker;
    }
    // offsets[lower].0 <= index < offsets[upper].0 (the last entry is the
    // edge total), and upper - lower < the worker count.
    let mut upper = offsets.len() - 1;
    while upper - lower > 1 {
        #[cfg(test)]
        {
            probes += 1;
        }
        let middle = lower + (upper - lower) / 2;
        if offsets[middle].0 <= index {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    #[cfg(test)]
    emission_probes::record(probes);
    lower
}
