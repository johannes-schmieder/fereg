//! Interpolate a recession direction on independent raw rows. Absorbed
//! coefficients precede shared finite columns, so disconnected FE components
//! can eliminate their own levels before contributing global constraints.
//! A single prime proposes a vector. Its exact dyadic score decides every row.
use super::*;
use crate::memory::fallible_empty;
use crate::ppml::modular_rank::{inverse, residue, PRIME};
use crate::ppml::prepared_separation::exact_arithmetic::{gcd, rational_candidate, ExactSum};

fn raw_descriptor_words(model: &PreparedPpmlModelGeneration) -> usize {
    model
        .fit_view()
        .effect_specs()
        .iter()
        .fold(0usize, |sum, s| {
            sum.saturating_add(8)
                .saturating_add(s.slopes.len().saturating_mul(2))
        })
}

/// One absorbed intercept effect: its factor codes, the first raw coordinate
/// of its levels and the number of levels.
pub(in crate::ppml::prepared_separation) type InterceptBlock<'a> = (&'a [u32], usize, usize);

impl<'a> RawRows<'a> {
    /// Factor codes of every absorbed effect with the first raw coordinate of
    /// its levels, or `None` when any effect carries a slope channel.
    pub(in crate::ppml::prepared_separation) fn intercept_blocks(
        &self,
    ) -> Result<Option<Vec<InterceptBlock<'a>>>> {
        if self
            .blocks
            .iter()
            .any(|b| !b.intercept || !b.slopes.is_empty())
        {
            return Ok(None);
        }
        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(self.blocks.len())
            .map_err(|_| allocation("unable to reserve streamed factor views"))?;
        let mut base = 0usize;
        for block in &self.blocks {
            blocks.push((block.codes, base, block.levels));
            base = base.saturating_add(block.levels);
        }
        Ok(Some(blocks))
    }
}

impl RawRows<'_> {
    /// Binding owns the coefficient witness, the N-score direction, the
    /// N-capacity compact drop bank, and up to N stable-row IDs. Replay may
    /// hold the direction and both row maps while checking the certificate.
    /// Include raw-view descriptors and the exact dyadic accumulator. Elimination storage is already released.
    pub(in crate::ppml::prepared_separation) fn sparse_replay_words(
        model: &PreparedPpmlModelGeneration,
        coefficient_capacity: usize,
    ) -> usize {
        raw_descriptor_words(model)
            .saturating_add(std::mem::size_of::<ExactSum>().div_ceil(8))
            .saturating_add(coefficient_capacity)
            .saturating_add(model.outcome().len().saturating_mul(3))
    }

    /// Coordinate visits in a full raw row, including all components of a
    /// generated interaction whose indicator does not short circuit.
    pub(in crate::ppml::prepared_separation) fn sparse_row_visits(&self) -> usize {
        use crate::ols::PreparedOlsColumnKind;
        let finite = self.model.model().columns.iter().fold(0usize, |cost, c| {
            cost.saturating_add(match &c.kind {
                PreparedOlsColumnKind::CategoricalInteraction { components } => {
                    components.len().max(1)
                }
                _ => 1,
            })
        });
        self.blocks.iter().fold(finite, |cost, b| {
            cost.saturating_add(1 + usize::from(b.intercept) + b.slopes.len())
        })
    }

    /// Sparse raw design, in elimination order. Unmentioned level coordinates
    /// are identically zero. This same ordering defines the stored witness.
    pub(in crate::ppml::prepared_separation) fn terms(
        &self,
        row: usize,
        mut take: impl FnMut(usize, f64),
    ) -> Result<()> {
        let mut base = 0;
        for (i, block) in self.blocks.iter().enumerate() {
            crate::interrupt::row_checkpoint(i + 1)?;
            let channels = usize::from(block.intercept) + block.slopes.len();
            let mut index = base + channels * block.codes[row] as usize;
            if block.intercept {
                take(index, 1.);
                index += 1;
            }
            for (j, slope) in block.slopes.iter().enumerate() {
                crate::interrupt::row_checkpoint(j + 1)?;
                take(index, slope[row]);
                index += 1;
            }
            base += channels * block.levels;
        }
        let fit = self.model.fit_view();
        for i in 0..self.model.model().columns.len() {
            crate::interrupt::row_checkpoint(i + 1)?;
            take(base + i, fit.column_value(i, row)?);
        }
        Ok(())
    }
}

pub(super) fn visit(
    model: &PreparedPpmlModelGeneration,
    coefficients: &[i64],
    inspect: impl FnMut(usize, Ordering, f64) -> bool,
) -> Result<bool> {
    let design = RawRows::new(model, true)?;
    visit_design(&design, coefficients, inspect)
}

fn visit_design(
    design: &RawRows<'_>,
    coefficients: &[i64],
    mut inspect: impl FnMut(usize, Ordering, f64) -> bool,
) -> Result<bool> {
    if coefficients.len() != design.width {
        return Ok(false);
    }
    for r in 0..design.len() {
        crate::interrupt::row_checkpoint(r)?;
        let mut sum = ExactSum::new();
        let mut valid = true;
        design.terms(r, |c, x| valid &= sum.add(x, coefficients[c]))?;
        let (sign, score) = sum.score();
        if !valid || !inspect(r, sign, score) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Clone, Copy)]
struct Entry {
    column: u32,
    value: u32,
}

type Pivot = (u32, u32);

/// The first half of the exploratory allowance establishes the observed
/// touches and fill per independent pivot. Beyond that allowance each further
/// half-window must still be productive, both absolutely (relative to a raw
/// row) and relative to the preceding window. Dependent rows earn no credit.
struct EliminationWork {
    spent: usize,
    limit: usize,
    window: usize,
    row_visits: usize,
    sampled_work: usize,
    credited_rank: usize,
    sampled_entries: usize,
    // Touches and entries per pivot in the preceding sample.
    previous: Option<(usize, usize)>,
    extensions: usize,
    plateau: bool,
}

impl EliminationWork {
    fn new(initial: usize, row_visits: usize, adaptive: bool) -> Self {
        let window = if adaptive { initial / 2 } else { 0 };
        Self {
            spent: 0,
            limit: initial - window,
            window,
            row_visits: row_visits.max(1),
            sampled_work: 0,
            credited_rank: 0,
            sampled_entries: 0,
            previous: None,
            extensions: 0,
            plateau: false,
        }
    }

    fn start_basis(&mut self) {
        // Rebuilding after an adverse zero has a new entry pool. Keep the
        // cumulative work/deadline and never credit the same rank twice.
        self.sampled_entries = 0;
    }

    fn inserted_pivot(&mut self, rank: usize, entries: usize) {
        if rank == self.credited_rank {
            // This new basis caught up to the previously sampled rank. Its
            // own entry count defines marginal fill for subsequent pivots.
            self.sampled_entries = entries;
        }
    }

    fn charge(&mut self, cost: usize, rank: usize, entries: usize) -> Result<bool> {
        let Some(next) = self.spent.checked_add(cost) else {
            return Ok(false);
        };
        if next > self.limit && !self.next_window(next, rank, entries)? {
            return Ok(false);
        }
        self.spent = next;
        Ok(true)
    }

    fn seed_after_plateau(&mut self) {
        // Stop normal elimination. A short, fixed prefix of zero equations
        // can still expose a candidate from these completed positive pivots.
        // Its work credit is 1/64 of the original exploration (1M touches).
        self.limit = self.spent.saturating_add(self.window.div_ceil(32));
        self.window = 0;
    }

    fn charge_solution(&mut self, rank: usize, entries: usize) -> Result<bool> {
        let Some(total) = self.spent.checked_add(entries) else {
            return Ok(false);
        };
        // Reconstruction visits only the already stored entries, once.
        // At a tranche boundary assess the producing echelon normally, then
        // allow this interruptible linear pass to finish. Charge all touches,
        // while leaving only the regular grant available to a blocker retry.
        let review = if self.plateau {
            // The single new boundary pivot is already present. This known
            // linear pass earns no credit and a partial basis is never retried.
            0
        } else if self.window == 0 {
            entries
        } else {
            entries.min(self.limit.saturating_sub(self.spent).saturating_add(1))
        };
        if !self.charge(review, rank, entries)? {
            return Ok(false);
        }
        self.spent = total;
        Ok(true)
    }

    fn next_window(&mut self, next: usize, rank: usize, entries: usize) -> Result<bool> {
        let limit = self.limit.saturating_add(self.window);
        if next > limit {
            return Ok(false);
        }
        crate::interrupt::checkpoint()?;
        // A blocker retry starts again at rank zero without resetting work.
        // It cannot earn a window merely by reconstructing the old rank.
        let pivots = rank.saturating_sub(self.credited_rank);
        let rates = if pivots == 0 {
            (usize::MAX, usize::MAX)
        } else {
            (
                (self.spent - self.sampled_work).div_ceil(pivots),
                entries
                    .saturating_sub(self.sampled_entries)
                    .div_ceil(pivots),
            )
        };
        if let Some((last_touches, last_entries)) = self.previous {
            let max_touches = self
                .row_visits
                .saturating_mul(256)
                .min(last_touches.saturating_mul(4));
            let max_entries = self
                .row_visits
                .saturating_mul(8)
                .min(last_entries.saturating_mul(4));
            let switch = if pivots == 0 {
                Some("rank_stall")
            } else if rates.0 > max_touches {
                Some("pivot_cost")
            } else if rates.1 > max_entries {
                Some("pivot_fill")
            } else {
                None
            };
            if let Some(reason) = switch {
                self.plateau = true;
                if crate::diagnostics::solver_trace_enabled() {
                    crate::diagnostics::emit_solver_trace(&format!(
                        "FEREG_SOLVER_TRACE|phase=separation_sparse_elimination_switch|reason={reason}|work={}|previous_rank={}|rank={rank}|added_pivots={pivots}|window_work={}|entries={entries}|touches_per_pivot={}|entries_per_pivot={}",
                        self.spent, self.credited_rank, self.spent - self.sampled_work, rates.0, rates.1
                    ));
                }
                return Ok(false);
            }
            self.extensions += 1;
        }
        self.limit = limit;
        self.sampled_work = self.spent;
        self.credited_rank = rank;
        self.sampled_entries = entries;
        self.previous = Some(rates);
        Ok(true)
    }
}

struct Echelon<'a, 'plan> {
    pivots: Vec<Pivot>,
    entries: Vec<Entry>,
    rank: usize,
    fixed: usize,
    admission: &'a mut SparseProofAdmission<'plan>,
}

impl Echelon<'_, '_> {
    fn reserve_entries(&mut self, end: usize) -> Result<bool> {
        let old = self.entries.capacity();
        if end <= old {
            return Ok(true);
        }
        // Both allocations may coexist while the allocator moves the pool.
        // Ask only after the actual new pivot no longer fits. The admission
        // rounds up in bounded increments and may deny growth.
        let base = self.fixed.saturating_add(old);
        if end > u32::MAX as usize || !self.admission.admit(base.saturating_add(end)) {
            return Ok(false);
        }
        let capacity = old
            .saturating_mul(2)
            .max(end)
            .min(self.admission.words() - base)
            .min(u32::MAX as usize);
        self.entries
            .try_reserve_exact(capacity - self.entries.len())
            .map_err(|_| allocation("unable to store sparse pivot entries"))?;
        Ok(true)
    }

    /// Consume one sparse row. The last column is the augmented coordinate.
    /// A dependent row with conflicting target is omitted from interpolation.
    fn insert(
        &mut self,
        row: &mut Vec<Entry>,
        merge: &mut Vec<Entry>,
        work: &mut EliminationWork,
    ) -> Result<bool> {
        while let Some(&head) = row.first() {
            if head.column as usize == self.pivots.len() {
                break;
            }
            crate::interrupt::row_checkpoint(head.column as usize)?;
            let (start, end) = self.pivots[head.column as usize];
            let cost = row.len().saturating_add((end - start) as usize);
            if !work.charge(cost, self.rank, self.entries.len())? {
                return Ok(false);
            }
            if start == end {
                let end = self.entries.len().saturating_add(row.len());
                if !self.reserve_entries(end)? {
                    return Ok(false);
                }
                let start = self.entries.len();
                let scale = inverse(u64::from(head.value));
                for (j, entry) in row.iter().enumerate() {
                    crate::interrupt::row_checkpoint(j + 1)?;
                    self.entries.push(Entry {
                        column: entry.column,
                        value: (u64::from(entry.value) * scale % PRIME) as u32,
                    });
                }
                self.pivots[head.column as usize] = (start as u32, end as u32);
                self.rank += 1;
                work.inserted_pivot(self.rank, end);
                break;
            }
            let existing = &self.entries[start as usize..end as usize];
            subtract(row, existing, u64::from(head.value), merge)?;
            std::mem::swap(row, merge);
        }
        Ok(true)
    }

    /// Set nonpivot coordinates to zero and the augmented coordinate to one.
    fn solution(&self) -> Result<Option<Vec<i64>>> {
        let width = self.pivots.len();
        let mut words = fallible_filled(width + 1, 0u64, "unable to reconstruct sparse ray")?;
        words[width] = 1;
        let mut common = 1i64;
        for (i, &(start, end)) in self.pivots.iter().enumerate().rev() {
            crate::interrupt::row_checkpoint(i)?;
            if start == end {
                continue;
            }
            let tail = &self.entries[start as usize + 1..end as usize];
            let mut sum = 0u128;
            for (j, e) in tail.iter().enumerate() {
                crate::interrupt::row_checkpoint(j + 1)?;
                sum += u128::from(e.value) * u128::from(words[e.column as usize]);
            }
            words[i] = (PRIME - (sum % u128::from(PRIME)) as u64) % PRIME;
            let (_, denominator) = rational_candidate(words[i]);
            let divisor = gcd(common as u64, denominator as u64) as i64;
            let Some(next) = (common / divisor).checked_mul(denominator) else {
                return Ok(None);
            };
            common = next;
        }
        let mut values = fallible_empty(width, "unable to store sparse ray coefficients")?;
        for (i, &word) in words[..width].iter().enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            let (numerator, denominator) = rational_candidate(word);
            let Some(value) = numerator.checked_mul(common / denominator) else {
                return Ok(None);
            };
            values.push(value);
        }
        Ok(Some(values))
    }
}

/// Merge ordered supports. Both pivots cancel and can be skipped.
fn subtract(row: &[Entry], pivot: &[Entry], scale: u64, out: &mut Vec<Entry>) -> Result<()> {
    out.clear();
    let (mut i, mut j) = (1, 1);
    let mut steps = 0;
    while i < row.len() || j < pivot.len() {
        steps += 1;
        crate::interrupt::row_checkpoint(steps)?;
        let first = row.get(i).map_or(u32::MAX, |e| e.column);
        let second = pivot.get(j).map_or(u32::MAX, |e| e.column);
        let column = first.min(second);
        let mut value = 0;
        if first == column {
            value = u64::from(row[i].value);
            i += 1;
        }
        if second == column {
            value = (value + PRIME - scale * u64::from(pivot[j].value) % PRIME) % PRIME;
            j += 1;
        }
        if value != 0 {
            out.push(Entry {
                column,
                value: value as u32,
            });
        }
    }
    Ok(())
}

pub(super) fn find(
    model: &PreparedPpmlModelGeneration,
    mut admission: SparseProofAdmission<'_>,
    initial_work: usize,
) -> Result<Option<Witness>> {
    let specs = model.fit_view().effect_specs();
    // Use the sparse interpolation route for level-specific continuous slopes.
    if !specs.iter().any(|s| !s.slopes.is_empty()) {
        return Ok(None);
    }
    let descriptor_words = raw_descriptor_words(model);
    if !admission.admit(descriptor_words) {
        return Ok(None);
    }
    let design = RawRows::new(model, true)?;
    let width = design.width;
    if !admission.adaptive() && RawRows::sparse_replay_words(model, width) > admission.words() {
        return Ok(None);
    }
    // One range per column, two width+1 merge buffers, and packed pivot
    // entries. During back substitution the two buffers are released before
    // the field and coefficient vectors are allocated.
    let flags = design.len().div_ceil(64);
    let fixed = descriptor_words
        .saturating_add(flags)
        .saturating_add(width.saturating_mul(3))
        .saturating_add(2);
    if width >= u32::MAX as usize || !admission.admit(fixed.saturating_add(1)) {
        return Ok(None);
    }
    let mut binding = fallible_filled(flags, 0u64, "unable to store sparse binding rows")?;
    // All finite column recipes are visited, including ones that return zero.
    // An interpolation scans the outcome twice and evaluates one raw equation
    // per row. Exact replay then visits every raw equation once more.
    let pass = design
        .len()
        .saturating_mul(design.sparse_row_visits().saturating_add(2));
    let mut scan_left = ProofBudget::sparse_work(pass, 8);
    let mut work = EliminationWork::new(
        initial_work,
        design.sparse_row_visits(),
        admission.adaptive(),
    );
    // Each retry treats blockers found on the preceding raw replay as exact
    // equalities. This can recover valid rays on other components and costs
    // at most four interpolation/replay passes.
    for _ in 0..4 {
        let Some(remaining) = scan_left.checked_sub(pass.saturating_mul(2)) else {
            break;
        };
        scan_left = remaining;
        let Some(values) = interpolate(&design, &binding, fixed, &mut admission, &mut work)? else {
            return Ok(None);
        };
        // During this first exact score pass, the retry bitset remains live.
        let check_words = descriptor_words
            .saturating_add(flags)
            .saturating_add(values.capacity())
            .saturating_add(std::mem::size_of::<ExactSum>().div_ceil(8));
        if !admission.admit(check_words) {
            return Ok(None);
        }
        let mut strict = false;
        let mut blocked = false;
        let mut valid = true;
        let complete = visit_design(&design, &values, |r, sign, score| {
            if model.outcome()[r] == 0. {
                strict |= sign == Ordering::Less;
                if sign == Ordering::Greater {
                    let bit = 1u64 << (r % 64);
                    blocked |= binding[r / 64] & bit == 0;
                    binding[r / 64] |= bit;
                    valid = false;
                }
            } else {
                valid &= sign == Ordering::Equal;
            }
            valid &= score.is_finite();
            true
        })?;
        if complete && valid && strict {
            if !admission.admit(RawRows::sparse_replay_words(model, values.capacity())) {
                return Ok(None);
            }
            if crate::diagnostics::solver_trace_enabled() {
                crate::diagnostics::emit_solver_trace(&format!(
                    "FEREG_SOLVER_TRACE|phase=separation_sparse_elimination|generation={}|work={}|extensions={}|words={}|partial={}",
                    model.generation(), work.spent, work.extensions, admission.words(), work.plateau
                ));
            }
            return Ok(Some(Witness::Sparse(values)));
        }
        if !blocked || work.plateau {
            break;
        }
    }
    Ok(None)
}

fn interpolate(
    design: &RawRows<'_>,
    binding: &[u64],
    fixed: usize,
    admission: &mut SparseProofAdmission<'_>,
    work: &mut EliminationWork,
) -> Result<Option<Vec<i64>>> {
    let width = design.width;
    work.start_basis();
    let mut basis = Echelon {
        pivots: fallible_filled(width, (0, 0), "unable to reserve sparse ray pivots")?,
        entries: Vec::new(),
        rank: 0,
        fixed,
        admission,
    };
    let mut row = fallible_empty(width + 1, "unable to reserve sparse raw equation")?;
    let mut merge = fallible_empty(width + 1, "unable to reserve sparse elimination row")?;
    let mut positive_entries = 0;
    let mut seeds = 0;
    for boundary in [false, true] {
        for (r, &y) in design.model.outcome().iter().enumerate() {
            crate::interrupt::row_checkpoint(r)?;
            if (y == 0. && binding[r / 64] & (1u64 << (r % 64)) == 0) != boundary {
                continue;
            }
            if work.plateau {
                if seeds == 64 {
                    break;
                }
                seeds += 1;
            }
            row.clear();
            design.terms(r, |c, x| {
                let value = residue(x) as u32;
                if value != 0 {
                    row.push(Entry {
                        column: c as u32,
                        value,
                    });
                }
            })?;
            if boundary {
                row.push(Entry {
                    column: width as u32,
                    value: 1,
                });
            }
            if !basis.insert(&mut row, &mut merge, work)? {
                if !boundary && work.plateau {
                    // The interrupted row has changed only the merge buffers.
                    work.seed_after_plateau();
                    break;
                }
                return Ok(None);
            }
            if work.plateau && basis.entries.len() > positive_entries {
                break;
            }
        }
        if !boundary {
            positive_entries = basis.entries.len();
        }
    }
    if basis.entries.len() == positive_entries {
        return Ok(None);
    }
    drop(row);
    drop(merge);
    if !work.charge_solution(basis.rank, basis.entries.len())? {
        return Ok(None);
    }
    basis.solution()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elimination_windows_require_affordable_new_pivots() {
        // Each initial half-window has ten touches and one entry per pivot.
        let started = || {
            let mut work = EliminationWork::new(100, 1, true);
            assert!(work.charge(50, 0, 0).unwrap());
            assert!(work.charge(50, 5, 5).unwrap());
            work
        };
        let mut productive = started();
        assert!(productive.charge(50, 10, 10).unwrap());
        assert_eq!(productive.extensions, 1);
        assert!(productive.charge(50, 15, 15).unwrap());
        assert_eq!(productive.spent, 200);

        // The same work got no new independent row, only one costly new
        // pivot, or five pivots with sharply more entries per pivot.
        for (rank, entries) in [(5, 5), (6, 6), (10, 30)] {
            let mut work = started();
            assert!(!work.charge(1, rank, entries).unwrap());
            assert_eq!(work.spent, 100);
        }
        // A blocker retry spends its remaining allowance rebuilding a much
        // denser basis. Only fresh pivots beyond the paid rank may earn the
        // next window, at their actual local fill and cumulative touch cost.
        let retry = || {
            let mut work = started();
            assert!(work.charge(30, 10, 10).unwrap());
            work.start_basis();
            assert!(work.charge(20, 5, 40).unwrap());
            work.inserted_pivot(10, 80);
            work
        };
        let mut repeated = retry();
        assert!(!repeated.charge(1, 10, 80).unwrap());
        assert_eq!(repeated.spent, 150);
        assert!(retry().charge(1, 12, 82).unwrap());

        // A measured linear reconstruction can straddle a full extra
        // window. Its touches remain spent if exact replay asks for a retry.
        let mut finishing = EliminationWork::new(100, 4, true);
        assert!(finishing.charge(50, 0, 0).unwrap());
        assert!(finishing.charge(50, 10, 30).unwrap());
        assert!(finishing.charge_solution(20, 60).unwrap());
        assert_eq!((finishing.spent, finishing.limit), (160, 150));
        finishing.start_basis();
        assert!(!finishing.charge(1, 0, 0).unwrap());
        let mut strict = EliminationWork::new(100, 1, false);
        assert!(strict.charge(100, 0, 0).unwrap());
        assert!(!strict.charge(1, 100, 100).unwrap());
        assert!(!strict.charge_solution(20, 60).unwrap());
    }
}
