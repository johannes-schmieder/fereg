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
//! stops at half the requested tolerance on the reduced residual, which is the
//! remaining blocks' moment vector. Acceptance uses the moments recomputed
//! from the formed residual with the usual certificate scale. Moments of the
//! eliminated levels vanish by construction up to rounding and are not
//! recomputed. A stall, an admission refusal or a numerical breakdown before
//! the residual pass returns the untouched input to the caller's route; a
//! failed certificate after it retries in place, since projecting a partially
//! residualized column yields the same residual.

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
/// while the guard lives (process-wide; other tests only see a fallback).
#[cfg(test)]
pub(crate) struct DisabledForTest;

#[cfg(test)]
pub(crate) fn disable_for_test() -> DisabledForTest {
    TEST_DISABLED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    DisabledForTest
}

#[cfg(test)]
impl Drop for DisabledForTest {
    fn drop(&mut self) {
        TEST_DISABLED.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
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

fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| allocation("unable to reserve block-elimination workspace"))?;
    values.resize(count, value);
    Ok(values)
}

/// Split `values` into consecutive parts of the given lengths.
fn split_parts<'s, T>(mut values: &'s mut [T], lengths: &[usize]) -> Vec<&'s mut [T]> {
    let mut parts = Vec::with_capacity(lengths.len());
    for &length in lengths {
        let (head, tail) = std::mem::take(&mut values).split_at_mut(length);
        parts.push(head);
        values = tail;
    }
    parts
}

/// `parts` consecutive ranges of `0..offsets.len()-1` with balanced totals.
fn partition(offsets: &[usize], parts: usize) -> Vec<Range<usize>> {
    let items = offsets.len() - 1;
    let total = offsets[items] - offsets[0];
    let parts = parts.clamp(1, items.max(1));
    let mut ranges = Vec::with_capacity(parts);
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
    ranges
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
    max_cells: usize,
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
    rows: Vec<Range<usize>>,
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
        let levels: Vec<usize> = design.effects().iter().map(|e| e.n_levels()).collect();
        let eliminated = (0..k)
            .max_by_key(|&e| (levels[e], std::cmp::Reverse(e)))
            .expect("nonempty design");
        let blocks: Vec<usize> = (0..k).filter(|&e| e != eliminated).collect();
        let mut offsets = Vec::with_capacity(blocks.len());
        let mut reduced = 0_usize;
        for &b in &blocks {
            let Ok(offset) = u32::try_from(reduced) else {
                return Ok(None);
            };
            offsets.push(offset);
            reduced = reduced.saturating_add(levels[b]);
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
        let ranges = partition(member_offsets, lane_count(n * NB));
        let mut member_cell = filled(n, 0_u32)?;
        let lengths: Vec<usize> = ranges
            .iter()
            .map(|r| member_offsets[r.end] - member_offsets[r.start])
            .collect();
        let parts = split_parts(&mut member_cell, &lengths);
        let built: Vec<Result<LaneBuild>> = parts
            .into_par_iter()
            .zip(ranges.par_iter())
            .map(|(part, segs)| build_lane::<NB>(index, segs.clone(), &codes_b, &offs, part))
            .collect();
        let mut lanes_built = Vec::with_capacity(built.len());
        for lane in built {
            lanes_built.push(lane?);
        }
        crate::interrupt::checkpoint()?;
        let cells: usize = lanes_built.iter().map(|l| l.counts.len()).sum();
        let mut seg_cells = filled(segments + 1, 0_usize)?;
        let mut codes = Vec::new();
        codes
            .try_reserve_exact(cells * NB)
            .map_err(|_| allocation("unable to reserve block-elimination cells"))?;
        let repeated = lanes_built
            .iter()
            .any(|l| l.counts.iter().any(|&count| count > 1));
        let mut multiplicity = Vec::new();
        if repeated {
            multiplicity
                .try_reserve_exact(cells)
                .map_err(|_| allocation("unable to reserve block-elimination cells"))?;
        }
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
        let lanes = partition(&seg_cells, lane_count(cells * NB * BATCH))
            .into_iter()
            .map(|segments| {
                let max_cells = segments
                    .clone()
                    .map(|s| seg_cells[s + 1] - seg_cells[s])
                    .max()
                    .unwrap_or(0);
                Lane {
                    cells: seg_cells[segments.start]..seg_cells[segments.end],
                    segments,
                    max_cells,
                }
            })
            .collect::<Vec<_>>();
        let row_lanes = lane_count(n * (NB + 1) * BATCH);
        let per = n.div_ceil(row_lanes);
        let rows = (0..row_lanes)
            .map(|l| (l * per).min(n)..((l + 1) * per).min(n))
            .filter(|r| !r.is_empty())
            .collect::<Vec<_>>();
        let bytes = (codes.capacity() as u128) * 4
            + (multiplicity.capacity() as u128) * 8
            + (member_cell.capacity() as u128) * 4
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
            rows,
            nobs: n,
            bytes,
        }))
    }
}

fn build_lane<const NB: usize>(
    index: &LevelIndex,
    segs: Range<usize>,
    codes_b: &[&[u32]; NB],
    offs: &[u32; NB],
    member_cell: &mut [u32],
) -> Result<LaneBuild> {
    let mut out = LaneBuild {
        codes: Vec::with_capacity(member_cell.len() * NB),
        counts: Vec::with_capacity(member_cell.len()),
        seg_cells: Vec::with_capacity(segs.len()),
    };
    let mut local: Vec<([u32; NB], u32)> = Vec::new();
    let mut cursor = 0_usize;
    for (step, s) in segs.enumerate() {
        if step % 4096 == 0 {
            crate::interrupt::checkpoint()?;
        }
        let members = index.members(s);
        local.clear();
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
    inverse_diagonal: Vec<f64>,
    inverse_root_mass: Vec<f64>,
    raw_scale: f64,
    /// Total normalized weight of the sample.
    total_mass: f64,
    bytes: u128,
}

impl Weights {
    fn prepare(
        st: &Structure,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: ValidatedWeights<'_>,
    ) -> Result<Option<Self>> {
        let cells = st.cells();
        let index = design.effects()[st.eliminated].level_index();
        let member_offsets = index.membership_offsets();
        let cell = match weights.values {
            None if st.multiplicity.is_empty() => Vec::new(),
            None => {
                let mut cell = Vec::new();
                cell.try_reserve_exact(cells)
                    .map_err(|_| allocation("unable to reserve block-elimination weights"))?;
                cell.extend_from_slice(&st.multiplicity);
                cell
            }
            Some(values) => {
                let mut cell = filled(cells, 0.0_f64)?;
                let lengths: Vec<usize> = st.lanes.iter().map(|l| l.cells.len()).collect();
                let scale = weights.raw_scale;
                split_parts(&mut cell, &lengths)
                    .into_par_iter()
                    .zip(st.lanes.par_iter())
                    .for_each(|(part, lane)| {
                        let base = member_offsets[lane.segments.start];
                        let mut cursor = base;
                        for s in lane.segments.clone() {
                            let first = st.seg_cells[s] - lane.cells.start;
                            for &row in index.members(s) {
                                let local = st.member_cell[cursor] as usize;
                                part[first + local] += values[row] / scale;
                                cursor += 1;
                            }
                        }
                    });
                cell
            }
        };
        let m = st.reduced;
        let nb = st.blocks.len();
        let unit = cell.is_empty();
        let mut seg_inverse = filled(st.segments, 0.0_f64)?;
        let lengths: Vec<usize> = st.lanes.iter().map(|l| l.segments.len()).collect();
        let lanes = st.lanes.len();
        let mut mass_acc: Vec<Vec<f64>> = Vec::with_capacity(lanes);
        let mut diag_acc: Vec<Vec<f64>> = Vec::with_capacity(lanes);
        for _ in 0..lanes {
            mass_acc.push(filled(m, 0.0_f64)?);
            diag_acc.push(filled(m, 0.0_f64)?);
        }
        let failures: Vec<bool> = split_parts(&mut seg_inverse, &lengths)
            .into_par_iter()
            .zip(st.lanes.par_iter())
            .zip(mass_acc.par_iter_mut().zip(diag_acc.par_iter_mut()))
            .map(|((inverse, lane), (mass, diag))| {
                // Per-segment level sums `a_j` use a stamped lane accumulator;
                // the Schur diagonal is `mass_j - sum_s a_sj^2 / W_s`.
                let mut sums = vec![0.0_f64; m];
                let mut stamp = vec![u32::MAX; m];
                let mut touched: Vec<u32> = Vec::new();
                let mut bad = false;
                for (s, inv) in lane.segments.clone().zip(inverse.iter_mut()) {
                    let (c0, c1) = (st.seg_cells[s], st.seg_cells[s + 1]);
                    let mut total = 0.0;
                    touched.clear();
                    let tag = s as u32;
                    for c in c0..c1 {
                        let w = if unit { 1.0 } else { cell[c] };
                        total += w;
                        for &code in &st.codes[c * nb..(c + 1) * nb] {
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
                        bad = true;
                        continue;
                    }
                    let inverse_total = 1.0 / total;
                    *inv = inverse_total;
                    for &code in &touched {
                        let j = code as usize;
                        let a = sums[j];
                        diag[j] += a - a * a * inverse_total;
                    }
                }
                bad
            })
            .collect();
        if failures.into_iter().any(|bad| bad) {
            return Ok(None);
        }
        let (mass, rest) = mass_acc.split_first_mut().expect("one lane");
        let (diag, drest) = diag_acc.split_first_mut().expect("one lane");
        for other in rest.iter() {
            for (a, b) in mass.iter_mut().zip(other) {
                *a += b;
            }
        }
        for other in drest.iter() {
            for (a, b) in diag.iter_mut().zip(other) {
                *a += b;
            }
        }
        let mass = std::mem::take(mass);
        let mut inverse_diagonal = std::mem::take(diag);
        drop(mass_acc);
        drop(diag_acc);
        let mut inverse_root_mass = filled(m, 0.0_f64)?;
        for j in 0..m {
            let total = mass[j];
            if !(total.is_finite() && total > 0.0) {
                return Ok(None);
            }
            let d = inverse_diagonal[j];
            inverse_diagonal[j] = if d.is_finite() && d > 1e-13 * total {
                1.0 / d
            } else {
                0.0
            };
            inverse_root_mass[j] = 1.0 / total.sqrt();
        }
        let bytes = (cell.capacity() + seg_inverse.capacity() + 3 * m) as u128 * 8
            + 2 * (lanes * m) as u128 * 8;
        let total_mass: f64 = seg_inverse.iter().map(|inverse| 1.0 / inverse).sum();
        Ok(Some(Self {
            cell,
            seg_inverse,
            mass,
            inverse_diagonal,
            inverse_root_mass,
            raw_scale: weights.raw_scale,
            total_mass,
            bytes,
        }))
    }
}

/// Caller options of one projection.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Request {
    /// Also return the FE coefficients whose fit forms the residual.
    pub(crate) retain_coefficients: bool,
    /// Recompute the weight state even for the same weight slice (callers
    /// that refill one buffer between frames).
    pub(crate) fresh_weights: bool,
    /// Also bound every level's Jacobi coefficient-error estimate (callers
    /// that check fitted predictors row by row, such as PPML).
    pub(crate) coefficient_accuracy: bool,
}

/// Outcome of one projection request.
pub(crate) enum Outcome<V> {
    Done(CertifiedBorrowedMap),
    /// The route declined; see [`project`] for the state of the input.
    Declined(V),
}

/// A session's retained geometry and its current-weight state.
#[derive(Default)]
pub(crate) struct Cache {
    refused: bool,
    structure: Option<Structure>,
    /// Weights keyed by the borrowed weight slice (address, length); `None`
    /// key is unit weights.
    weights: Option<(Option<(usize, usize)>, Weights)>,
}

impl Cache {
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
        let key = validated.values.map(|v| (v.as_ptr() as usize, v.len()));
        if budget.is_some_and(|b| !b.fits(admission_bytes(st, None, values.ncols()))) {
            return Ok(Outcome::Declined(values));
        }
        if request.fresh_weights || self.weights.as_ref().is_none_or(|(k, _)| *k != key) {
            self.weights = None;
            match Weights::prepare(st, design, validated)? {
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
        project(st, w, design, validated, values, map, bytes, request)
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
        if *key != validated.values.map(|v| (v.as_ptr() as usize, v.len())) {
            return Ok(None);
        }
        if !(amplification.is_finite() && amplification >= 1.0) {
            return Err(invalid_argument(
                "derived residual amplification is invalid",
            ));
        }
        let elim_codes = design.effects()[st.eliminated].codes();
        let block_codes: Vec<&[u32]> = st
            .blocks
            .iter()
            .map(|&b| design.effects()[b].codes())
            .collect();
        let data = values.as_slice();
        let raw = validated.values;
        let scale = validated.raw_scale;
        let parts: Vec<(Vec<f64>, Vec<f64>, f64, bool)> = st
            .rows
            .par_iter()
            .map(|rows| {
                let mut remaining = vec![0.0_f64; st.reduced];
                let mut eliminated = vec![0.0_f64; st.segments];
                let mut sq = 0.0_f64;
                let mut finite = true;
                for row in rows.clone() {
                    let r = data[row];
                    finite &= r.is_finite();
                    let wr = raw.map_or(r, |v| v[row] / scale * r);
                    sq += wr * r;
                    eliminated[elim_codes[row] as usize] += wr;
                    for (codes, &offset) in block_codes.iter().zip(&st.offsets) {
                        remaining[(codes[row] + offset) as usize] += wr;
                    }
                }
                (remaining, eliminated, sq, finite)
            })
            .collect();
        let mut parts = parts.into_iter();
        let (mut remaining, mut eliminated, mut sq, mut finite) =
            parts.next().expect("one row lane");
        for (r, e, s, f) in parts {
            for (a, b) in remaining.iter_mut().zip(&r) {
                *a += b;
            }
            for (a, b) in eliminated.iter_mut().zip(&e) {
                *a += b;
            }
            sq += s;
            finite &= f;
        }
        if !finite || !sq.is_finite() {
            return Err(invalid_data("derived residual values must be finite"));
        }
        let reference = amplification * sq.sqrt();
        if !reference.is_finite() {
            return Err(invalid_data("derived residual scale is not finite"));
        }
        let scales = [reference];
        let mut rhs_inf = [0.0];
        let mut fold = CertificateFold::default();
        for (s, &moment) in eliminated.iter().enumerate() {
            let mass = 1.0 / w.seg_inverse[s];
            fold.level(mass, &[moment / mass], &scales, &mut rhs_inf, w.raw_scale)?;
        }
        for (level, &moment) in remaining.iter().enumerate() {
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
    let lanes = st.lanes.len().max(st.rows.len()) as u128;
    let scratch: u128 = st.lanes.iter().map(|l| l.max_cells as u128).sum();
    8 * r * (5 * m + lanes * m + scratch + st.segments as u128)
}

pub(crate) fn admission_bytes(st: &Structure, weights: Option<&Weights>, nrhs: usize) -> u128 {
    let weight_bytes = weights.map_or_else(
        || {
            (st.cells() as u128 + st.segments as u128 + 3 * st.reduced as u128) * 8
                + 2 * (st.lanes.len() * st.reduced) as u128 * 8
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

/// One lane of `acc = S p` restricted to its segments (before the merge).
fn apply_lane<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    p: &[[f64; R]],
    lane: &Lane,
    acc: &mut [[f64; R]],
    u: &mut [[f64; R]],
) {
    acc.fill([0.0; R]);
    for s in lane.segments.clone() {
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

/// `accs[0] += accs[1] + ... + accs[L-1]`, in lane order.
fn merge<const R: usize>(accs: &mut [Vec<[f64; R]>]) {
    let (first, rest) = accs.split_first_mut().expect("one lane");
    if rest.is_empty() {
        return;
    }
    let rest: &[Vec<[f64; R]>] = rest;
    let add = |base: usize, chunk: &mut [[f64; R]]| {
        for other in rest {
            let len = chunk.len();
            for (v, o) in chunk.iter_mut().zip(&other[base..base + len]) {
                for j in 0..R {
                    v[j] += o[j];
                }
            }
        }
    };
    if first.len() * R * rest.len() < PARALLEL_WORK {
        add(0, first);
    } else {
        first
            .par_chunks_mut(CHUNK)
            .enumerate()
            .for_each(|(i, chunk)| add(i * CHUNK, chunk));
    }
}

struct Work<const R: usize> {
    x: Vec<[f64; R]>,
    r: Vec<[f64; R]>,
    z: Vec<[f64; R]>,
    p: Vec<[f64; R]>,
    accs: Vec<Vec<[f64; R]>>,
    scratch: Vec<Vec<[f64; R]>>,
    seg: Vec<[f64; R]>,
}

impl<const R: usize> Work<R> {
    fn new(st: &Structure) -> Result<Self> {
        let m = st.reduced;
        let lanes = st.lanes.len().max(st.rows.len());
        let mut accs = Vec::with_capacity(lanes);
        for _ in 0..lanes {
            accs.push(filled(m, [0.0; R])?);
        }
        let mut scratch = Vec::with_capacity(st.lanes.len());
        for lane in &st.lanes {
            scratch.push(filled(lane.max_cells, [0.0; R])?);
        }
        Ok(Self {
            x: filled(m, [0.0; R])?,
            r: filled(m, [0.0; R])?,
            z: filled(m, [0.0; R])?,
            p: filled(m, [0.0; R])?,
            accs,
            scratch,
            seg: filled(st.segments, [0.0; R])?,
        })
    }
}

fn apply<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    p: &[[f64; R]],
    accs: &mut [Vec<[f64; R]>],
    scratch: &mut [Vec<[f64; R]>],
) {
    let lanes = st.lanes.len();
    if lanes == 1 {
        apply_lane::<R, NB, UNIT>(st, w, p, &st.lanes[0], &mut accs[0], &mut scratch[0]);
        return;
    }
    accs[..lanes]
        .par_iter_mut()
        .zip(scratch.par_iter_mut())
        .zip(st.lanes.par_iter())
        .for_each(|((acc, u), lane)| apply_lane::<R, NB, UNIT>(st, w, p, lane, acc, u));
    merge(&mut accs[..lanes]);
}

/// Per-RHS sums over fixed chunks, combined in chunk order.
fn chunked<const R: usize, T: Send>(
    m: usize,
    serial: bool,
    body: impl Fn(Range<usize>) -> T + Sync,
    mut fold: impl FnMut(T),
) {
    if serial {
        fold(body(0..m));
    } else {
        let parts: Vec<T> = (0..m.div_ceil(CHUNK))
            .into_par_iter()
            .map(|i| body(i * CHUNK..((i + 1) * CHUNK).min(m)))
            .collect();
        for part in parts {
            fold(part);
        }
    }
}

fn dot<const R: usize>(a: &[[f64; R]], b: &[[f64; R]]) -> [f64; R] {
    let m = a.len();
    let mut total = [0.0; R];
    chunked::<R, [f64; R]>(
        m,
        m * R < PARALLEL_WORK,
        |range| {
            let mut s = [0.0; R];
            for (x, y) in a[range.clone()].iter().zip(&b[range]) {
                for j in 0..R {
                    s[j] += x[j] * y[j];
                }
            }
            s
        },
        |s| {
            for j in 0..R {
                total[j] += s[j];
            }
        },
    );
    total
}

/// Reduced RHS: `g = A-part of D'Wv` minus the eliminated groups' means, the
/// segment sums `Σ w v` (kept for back-substitution) and `Σ w v^2`, `max|v|`.
fn rhs<const R: usize, const NB: usize, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    values: &DenseMatrix,
    first: usize,
    work: &mut Work<R>,
) -> ([f64; R], [f64; R]) {
    let st = ctx.st;
    let stride = values.ncols();
    let data = values.as_slice();
    let lengths: Vec<usize> = st.lanes.iter().map(|l| l.segments.len()).collect();
    let lanes = st.lanes.len();
    let seg_parts = split_parts(&mut work.seg, &lengths);
    let weights = ctx.weights.values.unwrap_or(&[]);
    let scale = ctx.weights.raw_scale;
    let run = |lane: &Lane, acc: &mut [[f64; R]], seg: &mut [[f64; R]]| {
        acc.fill([0.0; R]);
        let mut sq = [0.0; R];
        let mut big = [0.0_f64; R];
        for (s, out) in lane.segments.clone().zip(seg.iter_mut()) {
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
                    z[j] = w * (v[j] - mean[j]);
                }
                for b in 0..NB {
                    let a = &mut acc[(ctx.codes_b[b][row] + ctx.offs[b]) as usize];
                    for j in 0..R {
                        a[j] += z[j];
                    }
                }
            }
        }
        (sq, big)
    };
    let results: Vec<([f64; R], [f64; R])> = if lanes == 1 {
        let mut parts = seg_parts;
        vec![run(
            &st.lanes[0],
            &mut work.accs[0],
            parts.pop().expect("lane"),
        )]
    } else {
        work.accs[..lanes]
            .par_iter_mut()
            .zip(seg_parts.into_par_iter())
            .zip(st.lanes.par_iter())
            .map(|((acc, seg), lane)| run(lane, acc, seg))
            .collect()
    };
    merge(&mut work.accs[..lanes]);
    let mut sq = [0.0; R];
    let mut big = [0.0_f64; R];
    for (s, b) in results {
        for j in 0..R {
            sq[j] += s[j];
            big[j] = big[j].max(b[j]);
        }
    }
    (sq, big)
}

enum Solve {
    Converged(usize),
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

/// Lockstep Jacobi-preconditioned CG on `S x = r0` (r0 already in `work.r`).
/// A column stops when its largest scaled moment `|r_j| / sqrt(m_j)` is at
/// most `target` and, when `coefficient_target` is given, its largest
/// Jacobi coefficient-error estimate `|r_j| / d_j` is at most that target.
fn pcg<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    work: &mut Work<R>,
    target: [f64; R],
    coefficient_target: Option<[f64; R]>,
    cap: usize,
) -> Result<Solve> {
    let measure = |moment: f64, coefficient: f64, c: usize| {
        ratio(moment, target[c]).max(coefficient_target.map_or(0.0, |t| ratio(coefficient, t[c])))
    };
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
        progress[c] = measure(inf[c], zinf[c], c);
        active[c] = progress[c] > 1.0;
    }
    for (p, z) in work.p.iter_mut().zip(&work.z) {
        for c in 0..R {
            p[c] = if active[c] { z[c] } else { 0.0 };
        }
    }
    let mut best = progress;
    let mut checkpoint_best = progress;
    let mut iterations = 0;
    while active.contains(&true) {
        if iterations >= cap {
            return Ok(Solve::Declined("iteration ceiling"));
        }
        crate::interrupt::checkpoint()?;
        apply::<R, NB, UNIT>(st, w, &work.p, &mut work.accs, &mut work.scratch);
        let pq = dot::<R>(&work.p, &work.accs[0]);
        let mut alpha = [0.0; R];
        for c in 0..R {
            if active[c] {
                if !(pq[c].is_finite() && pq[c] > 0.0) {
                    return Ok(Solve::Declined("nonpositive reduced curvature"));
                }
                alpha[c] = rz[c] / pq[c];
            }
        }
        let mut next = [0.0; R];
        let mut inf = [0.0_f64; R];
        let mut zinf = [0.0_f64; R];
        {
            let Work {
                x, r, z, p, accs, ..
            } = &mut *work;
            let q = &accs[0];
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
                (s, h, hz)
            };
            let parts: Vec<([f64; R], [f64; R], [f64; R])> = if serial {
                vec![body(0..m, x, r, z)]
            } else {
                x.par_chunks_mut(CHUNK)
                    .zip(r.par_chunks_mut(CHUNK))
                    .zip(z.par_chunks_mut(CHUNK))
                    .enumerate()
                    .map(|(i, ((x, r), z))| {
                        let start = i * CHUNK;
                        body(start..start + x.len(), x, r, z)
                    })
                    .collect()
            };
            for (s, h, hz) in parts {
                for c in 0..R {
                    next[c] += s[c];
                    inf[c] = inf[c].max(h[c]);
                    zinf[c] = zinf[c].max(hz[c]);
                }
            }
        }
        iterations += 1;
        let mut beta = [0.0; R];
        for c in 0..R {
            if !active[c] {
                continue;
            }
            if !(inf[c].is_finite() && next[c].is_finite() && zinf[c].is_finite()) {
                return Ok(Solve::Declined("nonfinite reduced residual"));
            }
            let current = measure(inf[c], zinf[c], c);
            best[c] = best[c].min(current);
            if current <= 1.0 {
                active[c] = false;
            } else {
                beta[c] = next[c] / rz[c];
            }
            rz[c] = next[c];
        }
        if iterations % 40 == 0 {
            for c in 0..R {
                // Little progress over the last window: leave hard graphs to
                // the caller's preconditioned solvers.
                if active[c] && best[c] > 0.5 * checkpoint_best[c] {
                    return Ok(Solve::Declined("stalled reduced recurrence"));
                }
            }
            checkpoint_best = best;
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
    Ok(Solve::Converged(iterations))
}

/// `seg[s] = (Σ_s w v - Σ_c w_c (A x)_c) / W_s`, the eliminated coefficients.
fn recover<const R: usize, const NB: usize, const UNIT: bool>(
    st: &Structure,
    w: &Weights,
    work: &mut Work<R>,
) {
    let lengths: Vec<usize> = st.lanes.iter().map(|l| l.segments.len()).collect();
    let x = &work.x;
    let parts = split_parts(&mut work.seg, &lengths);
    let run = |lane: &Lane, seg: &mut [[f64; R]]| {
        for (s, out) in lane.segments.clone().zip(seg.iter_mut()) {
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
    };
    if st.lanes.len() == 1 {
        let mut parts = parts;
        run(&st.lanes[0], parts.pop().expect("lane"));
    } else {
        parts
            .into_par_iter()
            .zip(st.lanes.par_iter())
            .for_each(|(seg, lane)| run(lane, seg));
    }
}

/// Form the residual in place and accumulate its remaining-level moments into
/// `accs[0]`; returns `Σ w r^2` per RHS.
fn residualize<const R: usize, const NB: usize, const WEIGHTED: bool>(
    ctx: &Ctx<'_, '_, NB>,
    values: &mut DenseMatrix,
    first: usize,
    work: &mut Work<R>,
) -> [f64; R] {
    let st = ctx.st;
    let stride = values.ncols();
    let weights = ctx.weights.values.unwrap_or(&[]);
    let scale = ctx.weights.raw_scale;
    let Work { x, seg, accs, .. } = work;
    let (x, alpha) = (&*x, &*seg);
    let lengths: Vec<usize> = st.rows.iter().map(|r| r.len() * stride).collect();
    let lanes = st.rows.len();
    let run = |rows: Range<usize>, data: &mut [f64], acc: &mut [[f64; R]]| {
        acc.fill([0.0; R]);
        let mut sq = [0.0; R];
        for (row, values) in rows.zip(data.chunks_exact_mut(stride)) {
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
        }
        sq
    };
    let parts = split_parts(values.as_mut_slice(), &lengths);
    let results: Vec<[f64; R]> = if lanes == 1 {
        let mut parts = parts;
        vec![run(
            st.rows[0].clone(),
            parts.pop().expect("lane"),
            &mut accs[0],
        )]
    } else {
        parts
            .into_par_iter()
            .zip(st.rows.par_iter())
            .zip(accs[..lanes].par_iter_mut())
            .map(|((data, rows), acc)| run(rows.clone(), data, acc))
            .collect()
    };
    merge(&mut accs[..lanes]);
    let mut sq = [0.0; R];
    for s in results {
        for j in 0..R {
            sq[j] += s[j];
        }
    }
    sq
}

/// Per-call timing and iteration facts (trace only).
#[derive(Default, Clone, Copy)]
struct Facts {
    iterations: usize,
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
        norms: Vec<f64>,
        reference: Vec<f64>,
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
) -> Result<Step> {
    let st = ctx.st;
    let mut work = Work::<R>::new(st)?;
    let started = Instant::now();
    let (sq, big) = rhs::<R, NB, WEIGHTED>(ctx, values, first, &mut work);
    std::mem::swap(&mut work.r, &mut work.accs[0]);
    facts.rhs_seconds += started.elapsed().as_secs_f64();
    let mut scale = [0.0; R];
    for j in 0..R {
        let norm = sq[j].sqrt();
        if !norm.is_finite() || (big[j] > 0.0 && !(1e-150..=1e150).contains(&norm)) {
            return Ok(Step::Declined("RHS scale outside the plain-sum range"));
        }
        scale[j] = reference.map_or(norm, |r| r[j]);
    }
    let mut target = [0.0; R];
    for j in 0..R {
        target[j] = 0.5 * tolerance * scale[j];
    }
    // Coefficient accuracy: Jacobi error estimates within the tolerance of
    // the column's root-mean-square magnitude.
    let coefficient_target = ctx.coefficient_accuracy.then(|| {
        let root_total = ctx.w.total_mass.sqrt();
        let mut t = [0.0; R];
        for j in 0..R {
            t[j] = 0.5 * tolerance * scale[j] / root_total;
        }
        t
    });
    let started = Instant::now();
    let solved = pcg::<R, NB, UNIT>(st, ctx.w, &mut work, target, coefficient_target, cap)?;
    facts.pcg_seconds += started.elapsed().as_secs_f64();
    let iterations = match solved {
        Solve::Converged(iterations) => iterations,
        Solve::Declined(reason) => return Ok(Step::Declined(reason)),
    };
    facts.iterations = facts.iterations.max(iterations);
    let started = Instant::now();
    recover::<R, NB, UNIT>(st, ctx.w, &mut work);
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
    let sq = residualize::<R, NB, WEIGHTED>(ctx, values, first, &mut work);
    let (fold, norms) = certify_batch::<R>(ctx.w, &work.accs[0], sq, scale, tolerance);
    facts.residual_seconds += started.elapsed().as_secs_f64();
    Ok(Step::Formed {
        fold,
        norms,
        reference: scale.to_vec(),
    })
}

/// Fold the recomputed remaining-level moments at `max(reference, ||r||)`.
fn certify_batch<const R: usize>(
    w: &Weights,
    moments: &[[f64; R]],
    sq: [f64; R],
    reference: [f64; R],
    tolerance: f64,
) -> (Option<CertificateFold>, Vec<f64>) {
    let mut fold = CertificateFold::default();
    let mut scales = [0.0; R];
    let mut norms = vec![0.0; R];
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

/// Project the row-major RHS `values` against the design. A decline returns
/// the matrix: untouched, unless a later batch or a correction declined after
/// earlier residuals were formed, whose projection is then still the request.
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
) -> Result<Outcome<DenseMatrix>> {
    if values.nrows() != st.nobs || values.ncols() == 0 || design.nobs() != st.nobs {
        return Ok(Outcome::Declined(values));
    }
    match st.blocks.len() {
        1 => project_nb::<1>(st, w, design, weights, values, map, bytes, request),
        2 => project_nb::<2>(st, w, design, weights, values, map, bytes, request),
        3 => project_nb::<3>(st, w, design, weights, values, map, bytes, request),
        4 => project_nb::<4>(st, w, design, weights, values, map, bytes, request),
        _ => Ok(Outcome::Declined(values)),
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
) -> Result<Outcome<DenseMatrix>> {
    let mut ledger = crate::diagnostics::phase_ledger::PhaseLedger::new("block_schur", ["solve"]);
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
    let ncols = values.ncols();
    let started = Instant::now();
    let mut facts = Facts::default();
    let mut fold = CertificateFold::default();
    let mut residual_norms = Vec::with_capacity(ncols);
    let mut corrections = 0;
    let mut coefficients = if request.retain_coefficients {
        Some(FeCoefficientBatch::intercept_zeros(design, ncols)?)
    } else {
        None
    };
    let mut start = 0;
    while start < ncols {
        let width = (ncols - start).min(BATCH);
        let mut reference: Option<Vec<f64>> = None;
        let mut attempts = 0;
        loop {
            let step = dispatch::<NB>(
                &ctx,
                &mut values,
                start,
                width,
                reference.as_deref(),
                tolerance,
                cap,
                &mut facts,
                coefficients.as_mut(),
            )?;
            match step {
                Step::Declined(reason) => {
                    trace_decline(reason, facts);
                    return Ok(Outcome::Declined(values));
                }
                Step::Formed {
                    fold: Some(part),
                    norms,
                    ..
                } => {
                    merge_fold(&mut fold, &part);
                    residual_norms.extend_from_slice(&norms);
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
                        return Ok(Outcome::Declined(values));
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
        ("cells", st.cells() as u64),
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
        return Ok(Outcome::Declined(values));
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
    match coefficients {
        Some(coefficients) => CertifiedBorrowedMap::from_certified_parts_with_coefficients(
            values,
            report,
            coefficients,
        ),
        None => CertifiedBorrowedMap::from_certified_parts(values, report),
    }?
    .with_residual_norms(residual_norms)
    .map(Outcome::Done)
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
