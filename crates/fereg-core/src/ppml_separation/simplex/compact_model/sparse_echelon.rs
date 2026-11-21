//! Sparse row echelon form of the positive rows.
//!
//! The dense positive elimination costs `positive × width × min(positive,
//! width)` operations, cubic once absorbed levels grow with the data. Factor
//! and indicator rows are sparse, so this elimination works on sparse rows and
//! always pivots on a column with the fewest remaining entries (dynamic
//! minimum count, threshold partial pivoting). Its cost follows the entries
//! and the fill it creates, both counted as they arise: storage is requested
//! at its measured size and every operation is charged to the stage work.
//!
//! Columns are scaled by powers of two, so binary and small-integer rows are
//! eliminated without rounding whenever the pivots are units. The echelon
//! form yields the null-space basis by back-substitution and, through its
//! recorded multipliers, the positive weights of a physical Farkas dual by two
//! triangular solves. Every result is only a proposal: rays are lifted and
//! replayed exactly and duals pass the IR certificate.
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::{fallible_filled, reserve};
use crate::error::{allocation, Result};

/// Row visitor: emits `(column, value)` for one row.
pub(super) type Reader<'r> = &'r mut dyn FnMut(usize, &mut dyn FnMut(usize, f64)) -> Result<()>;

/// Threshold for partial pivoting relative to the largest entry in the column.
const PIVOT_THRESHOLD: f64 = 0.1;
/// Words per stored entry (column index and value) and per column-list entry.
const ENTRY_WORDS: usize = 2;
/// Grant storage in steps of at least this many words, doubling.
const GRANT_STEP: usize = 1 << 16;

/// Why the elimination stopped without an echelon form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Refusal {
    pub(super) stage: &'static str,
    pub(super) needed: usize,
    pub(super) available: usize,
}

struct Pivot {
    /// Position of the row in the caller's row list.
    row: usize,
    column: usize,
    value: f64,
    /// Range of the other entries in `frozen`.
    start: usize,
    end: usize,
    /// Range of the multipliers this pivot applied, in `multipliers`.
    applied: (usize, usize),
}

pub(super) struct Echelon {
    width: usize,
    scales: Vec<f64>,
    pivots: Vec<Pivot>,
    frozen: Vec<(u32, f64)>,
    /// (updated row position, multiplier), grouped by pivot.
    multipliers: Vec<(u32, f64)>,
    /// Pivot index of each row position, or `u32::MAX`.
    pivot_of: Vec<u32>,
    free: Vec<usize>,
    /// Operations spent, in the stage-work unit.
    pub(super) work: usize,
    /// Peak words held while eliminating.
    pub(super) words: usize,
}

struct Budget<'g> {
    work: usize,
    limit: usize,
    granted: usize,
    grow: &'g mut dyn FnMut(usize) -> bool,
}

impl Budget<'_> {
    fn charge(&mut self, work: usize) -> std::result::Result<(), Refusal> {
        self.work = self.work.saturating_add(work);
        if self.work > self.limit {
            return Err(Refusal {
                stage: "sparse_echelon_work",
                needed: self.work,
                available: self.limit,
            });
        }
        Ok(())
    }

    fn hold(&mut self, words: usize) -> std::result::Result<(), Refusal> {
        if words <= self.granted {
            return Ok(());
        }
        let request = words.max(self.granted.saturating_mul(2)).max(GRANT_STEP);
        if (self.grow)(request) {
            self.granted = request;
            return Ok(());
        }
        if request > words && (self.grow)(words) {
            self.granted = words;
            return Ok(());
        }
        Err(Refusal {
            stage: "sparse_echelon_cells",
            needed: words,
            available: self.granted,
        })
    }
}

/// Words held for `entries` stored entries over `rows` rows and `width`
/// columns (entries, column lists, heap, per-column and per-row state).
fn held_words(entries: usize, heap: usize, rows: usize, width: usize) -> usize {
    entries
        .saturating_mul(ENTRY_WORDS + 1)
        .saturating_add(heap)
        .saturating_add(width.saturating_mul(6))
        .saturating_add(rows.saturating_mul(6))
}

/// A refusal is `Ok(Err(_))`; allocation and interrupt errors are `Err(_)`.
type Step<T> = Result<std::result::Result<T, Refusal>>;

macro_rules! refused {
    ($step:expr) => {
        match $step {
            Ok(value) => value,
            Err(refusal) => return Ok(Err(refusal)),
        }
    };
}

/// Per-column scratch for one row at a time.
struct Scratch {
    accumulator: Vec<f64>,
    marked: Vec<bool>,
    touched: Vec<usize>,
}

impl Scratch {
    fn new(width: usize) -> Result<Self> {
        let mut touched = Vec::new();
        reserve(
            &mut touched,
            width,
            "unable to reserve sparse echelon touches",
        )?;
        Ok(Self {
            accumulator: fallible_filled(width, 0., "unable to reserve sparse echelon row")?,
            marked: fallible_filled(width, false, "unable to reserve sparse echelon marks")?,
            touched,
        })
    }
}

/// Rows with repeated coordinates summed and zeros dropped, their entry
/// count and the column sums of squares.
struct Loaded {
    active: Vec<Vec<(u32, f64)>>,
    squares: Vec<f64>,
    entries: usize,
}

/// Eliminate the rows `rows` (row IDs passed to `read`). `grow(words)` admits
/// the elimination's total storage by its measured size; `limit` bounds the
/// operations. `Ok(Err(_))` names the bound that stopped it.
pub(super) fn eliminate(
    rows: &[usize],
    width: usize,
    read: Reader<'_>,
    tolerance: f64,
    limit: usize,
    grow: &mut dyn FnMut(usize) -> bool,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Step<Echelon> {
    let mut budget = Budget {
        work: 0,
        limit,
        granted: 0,
        grow,
    };
    if u32::try_from(width).is_err() || u32::try_from(rows.len()).is_err() {
        return Ok(Err(Refusal {
            stage: "sparse_echelon_index",
            needed: width.max(rows.len()),
            available: u32::MAX as usize,
        }));
    }
    refused!(budget.hold(held_words(0, 0, rows.len(), width)));
    let mut scratch = Scratch::new(width)?;
    let loaded = refused!(load(
        rows,
        width,
        read,
        &mut budget,
        &mut scratch,
        interrupt
    )?);
    let scales = power_of_two_scales(&loaded.squares)?;
    let mut state = State::new(loaded, &scales, tolerance, scratch)?;
    let mut steps = 0usize;
    while let Some(Reverse((n, column))) = state.heap.pop() {
        let column = column as usize;
        if state.done[column] || n != state.count[column] {
            continue;
        }
        steps += 1;
        if steps % 256 == 0 {
            interrupt()?;
        }
        refused!(state.step(column, &mut budget)?);
        let stored = state.entries + state.frozen.len() + state.multipliers.len();
        refused!(budget.hold(held_words(stored, state.heap.len(), rows.len(), width)));
    }
    state.free.sort_unstable();
    Ok(Ok(Echelon {
        width,
        scales,
        pivots: state.pivots,
        frozen: state.frozen,
        multipliers: state.multipliers,
        pivot_of: state.pivot_of,
        free: state.free,
        work: budget.work,
        words: budget.granted,
    }))
}

fn load(
    rows: &[usize],
    width: usize,
    read: Reader<'_>,
    budget: &mut Budget<'_>,
    scratch: &mut Scratch,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Step<Loaded> {
    let mut active = Vec::new();
    reserve(
        &mut active,
        rows.len(),
        "unable to reserve sparse echelon rows",
    )?;
    let mut squares = fallible_filled(width, 0., "unable to reserve sparse echelon norms")?;
    let mut entries = 0usize;
    for (position, &row) in rows.iter().enumerate() {
        if position % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        let mut invalid = false;
        let Scratch {
            accumulator,
            marked,
            touched,
        } = &mut *scratch;
        read(row, &mut |column, value| {
            if column >= width || !value.is_finite() {
                invalid = true;
                return;
            }
            if !marked[column] {
                marked[column] = true;
                touched.push(column);
            }
            accumulator[column] += value;
        })?;
        if invalid {
            return Ok(Err(Refusal {
                stage: "sparse_echelon_input",
                needed: row,
                available: width,
            }));
        }
        let mut stored = Vec::new();
        reserve(
            &mut stored,
            touched.len(),
            "unable to reserve sparse echelon entries",
        )?;
        for &column in touched.iter() {
            let value = accumulator[column];
            accumulator[column] = 0.;
            marked[column] = false;
            if value != 0. {
                squares[column] += value * value;
                stored.push((column as u32, value));
            }
        }
        touched.clear();
        entries += stored.len();
        refused!(budget.charge(stored.len().saturating_add(1)));
        refused!(budget.hold(held_words(entries, entries, rows.len(), width)));
        active.push(stored);
    }
    Ok(Ok(Loaded {
        active,
        squares,
        entries,
    }))
}

/// Power-of-two column scales near each column norm keep integer rows exact.
fn power_of_two_scales(squares: &[f64]) -> Result<Vec<f64>> {
    let mut scales = fallible_filled(squares.len(), 1., "unable to reserve sparse echelon scales")?;
    for (scale, &square) in scales.iter_mut().zip(squares) {
        let norm = square.sqrt();
        if norm > 1. && norm.is_finite() {
            *scale = 2f64.powi(norm.log2().round() as i32).max(1.);
        }
    }
    Ok(scales)
}

/// Mutable state of one elimination: active rows, their column lists and
/// exact live counts, and the minimum-count order with lazy deletion.
struct State {
    threshold: f64,
    active: Vec<Vec<(u32, f64)>>,
    columns: Vec<Vec<u32>>,
    count: Vec<u32>,
    heap: BinaryHeap<Reverse<(u32, u32)>>,
    done: Vec<bool>,
    seen: Vec<bool>,
    pivot_of: Vec<u32>,
    pivots: Vec<Pivot>,
    frozen: Vec<(u32, f64)>,
    multipliers: Vec<(u32, f64)>,
    free: Vec<usize>,
    scratch: Scratch,
    entries: usize,
}

impl State {
    fn new(loaded: Loaded, scales: &[f64], tolerance: f64, scratch: Scratch) -> Result<Self> {
        let Loaded {
            mut active,
            entries,
            ..
        } = loaded;
        let width = scales.len();
        let rows = active.len();
        let mut largest = 0f64;
        for row in &mut active {
            for (column, value) in row.iter_mut() {
                *value /= scales[*column as usize];
                largest = largest.max(value.abs());
            }
        }
        let mut columns: Vec<Vec<u32>> = Vec::new();
        reserve(
            &mut columns,
            width,
            "unable to reserve sparse echelon columns",
        )?;
        columns.resize_with(width, Vec::new);
        let mut count = fallible_filled(width, 0u32, "unable to reserve sparse echelon counts")?;
        for (position, row) in active.iter().enumerate() {
            for &(column, _) in row {
                let column = column as usize;
                columns[column]
                    .try_reserve(1)
                    .map_err(|_| allocation("unable to reserve sparse echelon column list"))?;
                columns[column].push(position as u32);
                count[column] += 1;
            }
        }
        let mut heap = BinaryHeap::new();
        heap.try_reserve(width)
            .map_err(|_| allocation("unable to reserve sparse echelon order"))?;
        let mut done = fallible_filled(width, false, "unable to reserve sparse echelon state")?;
        let mut free = Vec::new();
        reserve(
            &mut free,
            width,
            "unable to reserve sparse echelon free columns",
        )?;
        for (column, &n) in count.iter().enumerate() {
            if n == 0 {
                done[column] = true;
                free.push(column);
            } else {
                heap.push(Reverse((n, column as u32)));
            }
        }
        let mut pivots = Vec::new();
        reserve(
            &mut pivots,
            rows.min(width),
            "unable to reserve sparse echelon pivot list",
        )?;
        Ok(Self {
            threshold: tolerance * largest.max(1.),
            active,
            columns,
            count,
            heap,
            done,
            seen: fallible_filled(rows, false, "unable to reserve sparse echelon row marks")?,
            pivot_of: fallible_filled(rows, u32::MAX, "unable to reserve sparse echelon pivots")?,
            pivots,
            frozen: Vec::new(),
            multipliers: Vec::new(),
            free,
            scratch,
            entries,
        })
    }

    fn requeue(&mut self, column: usize) -> Result<()> {
        self.heap
            .try_reserve(1)
            .map_err(|_| allocation("unable to reserve sparse echelon order"))?;
        self.heap.push(Reverse((self.count[column], column as u32)));
        Ok(())
    }

    /// Active rows that still hold `column`, each once, with their entry.
    fn collect(&mut self, column: usize, budget: &mut Budget<'_>) -> Step<Vec<(u32, f64)>> {
        let mut candidates = Vec::new();
        reserve(
            &mut candidates,
            self.count[column] as usize,
            "unable to reserve sparse echelon candidates",
        )?;
        let mut scanned = 0usize;
        for position in std::mem::take(&mut self.columns[column]) {
            let p = position as usize;
            if self.pivot_of[p] != u32::MAX || self.seen[p] {
                continue;
            }
            scanned += self.active[p].len();
            if let Some(&(_, value)) = self.active[p].iter().find(|&&(c, _)| c as usize == column) {
                self.seen[p] = true;
                candidates
                    .try_reserve(1)
                    .map_err(|_| allocation("unable to reserve sparse echelon candidates"))?;
                candidates.push((position, value));
            }
        }
        for &(position, _) in &candidates {
            self.seen[position as usize] = false;
        }
        refused!(budget.charge(scanned.saturating_add(candidates.len())));
        Ok(Ok(candidates))
    }

    /// Pivot on `column`, or record it as free when it is numerically empty.
    fn step(&mut self, column: usize, budget: &mut Budget<'_>) -> Step<()> {
        let candidates = refused!(self.collect(column, budget)?);
        self.done[column] = true;
        self.count[column] = 0;
        let largest = candidates.iter().fold(0f64, |m, &(_, v)| m.max(v.abs()));
        if largest <= self.threshold {
            for &(position, _) in &candidates {
                self.active[position as usize].retain(|&(c, _)| c as usize != column);
            }
            self.entries -= candidates.len();
            self.free.push(column);
            return Ok(Ok(()));
        }
        let floor = PIVOT_THRESHOLD * largest;
        let &(chosen, pivot_value) = candidates
            .iter()
            .filter(|&&(_, v)| v.abs() >= floor)
            .min_by_key(|&&(position, _)| (self.active[position as usize].len(), position))
            .expect("the largest candidate passes the threshold");
        let (start, end) = self.freeze(chosen as usize, column)?;
        self.pivot_of[chosen as usize] = self.pivots.len() as u32;
        let applied = self.multipliers.len();
        for &(position, value) in &candidates {
            if position == chosen {
                continue;
            }
            let alpha = value / pivot_value;
            self.multipliers
                .try_reserve(1)
                .map_err(|_| allocation("unable to reserve sparse echelon multipliers"))?;
            self.multipliers.push((position, alpha));
            let row = self.active[position as usize].len();
            refused!(budget.charge(row.saturating_add(end - start)));
            self.update(position as usize, alpha, (start, end), column)?;
        }
        self.pivots.push(Pivot {
            row: chosen as usize,
            column,
            value: pivot_value,
            start,
            end,
            applied: (applied, self.multipliers.len()),
        });
        Ok(Ok(()))
    }

    /// Move the pivot row out of the active set; its other entries are frozen.
    fn freeze(&mut self, chosen: usize, column: usize) -> Result<(usize, usize)> {
        let pivot_row = std::mem::take(&mut self.active[chosen]);
        self.entries -= pivot_row.len();
        let start = self.frozen.len();
        self.frozen
            .try_reserve(pivot_row.len())
            .map_err(|_| allocation("unable to reserve sparse echelon pivot row"))?;
        for &(c, v) in &pivot_row {
            let c = c as usize;
            if c == column {
                continue;
            }
            self.frozen.push((c as u32, v));
            if !self.done[c] {
                self.count[c] -= 1;
                self.requeue(c)?;
            }
        }
        Ok((start, self.frozen.len()))
    }

    /// Row `p` minus `alpha` times the frozen pivot row: the pivot column
    /// leaves the row, fill enters it, and every count follows.
    fn update(
        &mut self,
        p: usize,
        alpha: f64,
        (start, end): (usize, usize),
        column: usize,
    ) -> Result<()> {
        let row = std::mem::take(&mut self.active[p]);
        let before = row.len();
        for &(c, v) in &row {
            let c = c as usize;
            if c != column {
                self.scratch.marked[c] = true;
                self.scratch.accumulator[c] = v;
                self.scratch.touched.push(c);
            }
        }
        drop(row);
        let original = self.scratch.touched.len();
        for k in start..end {
            let (c, v) = self.frozen[k];
            let c = c as usize;
            if !self.scratch.marked[c] {
                self.scratch.marked[c] = true;
                self.scratch.accumulator[c] = 0.;
                self.scratch.touched.push(c);
            }
            self.scratch.accumulator[c] -= alpha * v;
        }
        let mut updated = Vec::new();
        reserve(
            &mut updated,
            self.scratch.touched.len(),
            "unable to reserve sparse echelon update",
        )?;
        for slot in 0..self.scratch.touched.len() {
            let c = self.scratch.touched[slot];
            let value = self.scratch.accumulator[c];
            self.scratch.accumulator[c] = 0.;
            self.scratch.marked[c] = false;
            let kept = value.abs() > self.threshold;
            if kept {
                updated.push((c as u32, value));
            }
            if self.done[c] {
                continue;
            }
            match (slot < original, kept) {
                (true, false) => {
                    self.count[c] -= 1;
                    self.requeue(c)?;
                }
                (false, true) => {
                    self.columns[c]
                        .try_reserve(1)
                        .map_err(|_| allocation("unable to reserve sparse echelon fill"))?;
                    self.columns[c].push(p as u32);
                    self.count[c] += 1;
                    self.requeue(c)?;
                }
                _ => {}
            }
        }
        self.scratch.touched.clear();
        self.entries = self.entries - before + updated.len();
        self.active[p] = updated;
        Ok(())
    }
}

impl Echelon {
    pub(super) fn rank(&self) -> usize {
        self.pivots.len()
    }

    pub(super) fn nullity(&self) -> usize {
        self.free.len()
    }

    /// Row IDs of the independent pivot rows, in elimination order.
    pub(super) fn pivot_rows(&self, rows: &[usize]) -> Result<Vec<usize>> {
        let mut ids = Vec::new();
        reserve(
            &mut ids,
            self.pivots.len(),
            "unable to reserve sparse echelon pivot rows",
        )?;
        ids.extend(self.pivots.iter().map(|pivot| rows[pivot.row]));
        Ok(ids)
    }

    /// Operations of one `combine`: both triangular passes.
    pub(super) fn solve_work(&self) -> usize {
        self.frozen
            .len()
            .saturating_add(self.multipliers.len())
            .saturating_add(self.pivots.len())
            .saturating_add(self.width)
    }

    /// Operations a null-space basis costs: one back-substitution per free column.
    pub(super) fn basis_work(&self) -> usize {
        self.free
            .len()
            .saturating_mul(self.frozen.len().saturating_add(self.pivots.len()))
    }

    /// One unit-norm null-space vector per free column, in original
    /// coordinates, by back-substitution through the pivots in reverse order.
    pub(super) fn basis(&self, interrupt: &mut dyn FnMut() -> Result<()>) -> Result<Vec<Vec<f64>>> {
        let mut basis = Vec::new();
        reserve(
            &mut basis,
            self.free.len(),
            "unable to reserve sparse null-space basis",
        )?;
        for &free in &self.free {
            interrupt()?;
            let mut x =
                fallible_filled(self.width, 0., "unable to reserve sparse null-space vector")?;
            x[free] = 1.;
            for pivot in self.pivots.iter().rev() {
                let mut sum = 0.;
                for &(c, v) in &self.frozen[pivot.start..pivot.end] {
                    sum += v * x[c as usize];
                }
                x[pivot.column] = -sum / pivot.value;
            }
            let mut norm = 0f64;
            for (value, &scale) in x.iter_mut().zip(&self.scales) {
                *value /= scale;
                norm = value.mul_add(*value, norm);
            }
            let norm = norm.sqrt();
            if !norm.is_finite() || norm <= f64::MIN_POSITIVE {
                return Err(crate::error::FeregError::new(
                    crate::error::ErrorCode::NonConvergence,
                    "sparse null-space basis is nonfinite or zero",
                ));
            }
            x.iter_mut().for_each(|value| *value /= norm);
            basis.push(x);
        }
        Ok(basis)
    }

    /// Weights `v` on the pivot rows with `sum_q v_q a_q = target` in
    /// original coordinates: a forward solve through the echelon rows and a
    /// backward solve through the recorded multipliers. The target's
    /// component outside the positive row span is left as residual.
    pub(super) fn combine(&self, target: &[f64]) -> Result<Vec<f64>> {
        let mut residual =
            fallible_filled(self.width, 0., "unable to reserve sparse dual residual")?;
        for ((value, &t), &scale) in residual.iter_mut().zip(target).zip(&self.scales) {
            *value = t / scale;
        }
        let mut weights = fallible_filled(
            self.pivots.len(),
            0.,
            "unable to reserve sparse dual weights",
        )?;
        for (q, pivot) in self.pivots.iter().enumerate() {
            let u = residual[pivot.column] / pivot.value;
            weights[q] = u;
            residual[pivot.column] = 0.;
            for &(c, v) in &self.frozen[pivot.start..pivot.end] {
                residual[c as usize] -= u * v;
            }
        }
        for (p, pivot) in self.pivots.iter().enumerate().rev() {
            let mut value = weights[p];
            for &(position, alpha) in &self.multipliers[pivot.applied.0..pivot.applied.1] {
                let q = self.pivot_of[position as usize];
                if q != u32::MAX {
                    value -= alpha * weights[q as usize];
                }
            }
            weights[p] = value;
        }
        Ok(weights)
    }
}

/// Lift a numerical Farkas vector on the boundary rows to every observation.
/// The boundary keeps its strictly positive weights; the independent pivot
/// rows receive the weights that balance the boundary total, from zero and
/// then refined twice from compensated residuals, each step by the echelon's
/// two triangular passes. Every other positive row has weight zero. For
/// binary rows every product is an exact small integer. The result is only a
/// proposal for the IR certificate.
pub(super) fn physical_dual(
    (each, width, row_terms): (Reader<'_>, usize, usize),
    outcome: &[f64],
    (boundary, weights, positive): (&[usize], &[f64], &[usize]),
    echelon: &Echelon,
    stage_work: usize,
) -> Result<Option<Vec<f64>>> {
    let k = positive.len();
    let cost = boundary
        .len()
        .saturating_add(k.saturating_mul(3))
        .saturating_mul(row_terms)
        .saturating_add(echelon.solve_work().saturating_mul(3));
    if cost > stage_work {
        super::trace_refusal("physical_dual_work", cost, stage_work);
        return Ok(None);
    }
    if k == 0 || k != echelon.rank() || boundary.len() != weights.len() || width != echelon.width {
        return Ok(None);
    }
    let mut total = fallible_filled(width, 0., "unable to reserve boundary dual total")?;
    let mut correction = fallible_filled(width, 0., "unable to reserve boundary dual correction")?;
    for (position, (&i, &w)) in boundary.iter().zip(weights).enumerate() {
        crate::interrupt::row_checkpoint(position)?;
        if !(w.is_finite() && w > 0.) {
            return Ok(None);
        }
        each(i, &mut |j, v| {
            if j < width {
                crate::linalg::neumaier_add(w * v, &mut total[j], &mut correction[j]);
            }
        })?;
    }
    for (value, extra) in total.iter_mut().zip(&correction) {
        *value += extra;
    }
    let mut solution = fallible_filled(k, 0., "unable to reserve positive dual weights")?;
    let mut residual = fallible_filled(width, 0., "unable to reserve positive dual residual")?;
    for _ in 0..3 {
        crate::interrupt::checkpoint()?;
        // Residual of the boundary balance, t + A'p, with compensated sums;
        // the step solves A'step = -residual through the echelon form.
        residual.copy_from_slice(&total);
        correction.fill(0.);
        for (&i, &value) in positive.iter().zip(&solution) {
            each(i, &mut |j, v| {
                if j < width {
                    crate::linalg::neumaier_add(value * v, &mut residual[j], &mut correction[j]);
                }
            })?;
        }
        for (value, extra) in residual.iter_mut().zip(&correction) {
            *value = -(*value + extra);
        }
        let step = echelon.combine(&residual)?;
        for (value, delta) in solution.iter_mut().zip(&step) {
            *value += delta;
        }
    }
    if solution.iter().any(|value| !value.is_finite()) {
        return Ok(None);
    }
    let mut dual = fallible_filled(outcome.len(), 0., "unable to reserve physical dual")?;
    for (&i, &w) in boundary.iter().zip(weights) {
        dual[i] = w;
    }
    for (&i, &value) in positive.iter().zip(&solution) {
        dual[i] = value;
    }
    Ok(Some(dual))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(rows: &[Vec<(usize, f64)>], width: usize) -> Echelon {
        let ids: Vec<usize> = (0..rows.len()).collect();
        let mut read = |i: usize, f: &mut dyn FnMut(usize, f64)| {
            for &(c, v) in &rows[i] {
                f(c, v);
            }
            Ok(())
        };
        eliminate(
            &ids,
            width,
            &mut read,
            1e-10,
            usize::MAX,
            &mut |_| true,
            &mut || Ok(()),
        )
        .unwrap()
        .unwrap()
    }

    fn product(row: &[(usize, f64)], x: &[f64]) -> f64 {
        row.iter().map(|&(c, v)| v * x[c]).sum()
    }

    #[test]
    fn two_factor_path_has_the_component_nullity_and_no_fill_beyond_the_path() {
        // Rows are edges u_a - v_b of a path plus an intercept column 0.
        let levels = 200;
        let mut rows = Vec::new();
        for a in 0..levels {
            rows.push(vec![(0, 1.), (1 + a, 1.), (1 + levels + a, 1.)]);
            if a + 1 < levels {
                rows.push(vec![(0, 1.), (1 + a + 1, 1.), (1 + levels + a, 1.)]);
            }
        }
        let width = 1 + 2 * levels;
        let echelon = run(&rows, width);
        // Connected bipartite graph with an intercept: two free directions.
        assert_eq!(echelon.nullity(), 2);
        assert_eq!(echelon.rank(), rows.len());
        let basis = echelon.basis(&mut || Ok(())).unwrap();
        for x in &basis {
            for row in &rows {
                assert!(product(row, x).abs() < 1e-12);
            }
        }
        // Minimum-count pivoting peels the path: linear work.
        assert!(echelon.work < 40 * rows.len(), "{}", echelon.work);
    }

    #[test]
    fn dependent_rows_and_values_are_detected_and_combined_exactly() {
        let rows = vec![
            vec![(0, 1.), (2, 3.)],
            vec![(1, 2.), (2, -1.)],
            vec![(0, 2.), (1, 4.), (2, 4.)],
            vec![(3, 0.5)],
        ];
        let echelon = run(&rows, 5);
        assert_eq!(echelon.rank(), 3);
        assert_eq!(echelon.nullity(), 2);
        let pivots = echelon.pivot_rows(&[0, 1, 2, 3]).unwrap();
        assert_eq!(pivots.len(), 3);
        // A target in the span: 2 * row0 - row1 + 4 * row3.
        let target = [2., -2., 7., 2., 0.];
        let weights = echelon.combine(&target).unwrap();
        let mut sum = [0f64; 5];
        for (&row, &w) in pivots.iter().zip(&weights) {
            for &(c, v) in &rows[row] {
                sum[c] += w * v;
            }
        }
        for (a, b) in sum.iter().zip(target) {
            assert!((a - b).abs() < 1e-12, "{sum:?}");
        }
        for x in echelon.basis(&mut || Ok(())).unwrap() {
            for row in &rows {
                assert!(product(row, &x).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn work_and_storage_bounds_name_their_stage() {
        let rows: Vec<Vec<(usize, f64)>> = (0..50)
            .map(|i| {
                (0..20)
                    .map(|j| ((i * 7 + j * 3) % 40, 1. + j as f64))
                    .collect()
            })
            .collect();
        let ids: Vec<usize> = (0..rows.len()).collect();
        let mut read = |i: usize, f: &mut dyn FnMut(usize, f64)| {
            for &(c, v) in &rows[i] {
                f(c, v);
            }
            Ok(())
        };
        let work = eliminate(&ids, 40, &mut read, 1e-10, 500, &mut |_| true, &mut || {
            Ok(())
        })
        .unwrap()
        .err()
        .unwrap();
        assert_eq!(work.stage, "sparse_echelon_work");
        let cells = eliminate(
            &ids,
            40,
            &mut read,
            1e-10,
            usize::MAX,
            &mut |_| false,
            &mut || Ok(()),
        )
        .unwrap()
        .err()
        .unwrap();
        assert_eq!(cells.stage, "sparse_echelon_cells");
    }
}
