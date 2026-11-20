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

struct Echelon {
    pivots: Vec<Pivot>,
    entries: Vec<Entry>,
    limit: usize,
    work_left: usize,
}

impl Echelon {
    /// Consume one sparse row. The last column is the augmented coordinate.
    /// A dependent row with conflicting target is omitted from interpolation.
    fn insert(&mut self, row: &mut Vec<Entry>, merge: &mut Vec<Entry>) -> Result<bool> {
        while let Some(&head) = row.first() {
            if head.column as usize == self.pivots.len() {
                break;
            }
            crate::interrupt::row_checkpoint(head.column as usize)?;
            let (start, end) = self.pivots[head.column as usize];
            let existing = &self.entries[start as usize..end as usize];
            let cost = row.len().saturating_add(existing.len());
            if cost > self.work_left {
                return Ok(false);
            }
            self.work_left -= cost;
            if existing.is_empty() {
                let end = self.entries.len().saturating_add(row.len());
                if end > self.limit {
                    return Ok(false);
                }
                if self.entries.capacity() < end {
                    // An allocator can move the pool when it grows. Keep both
                    // the existing allocation and its replacement in budget.
                    let remaining = self.limit - self.entries.capacity();
                    if end > remaining {
                        return Ok(false);
                    }
                    let cap = end
                        .max(self.entries.capacity().saturating_mul(2))
                        .min(remaining);
                    self.entries
                        .try_reserve_exact(cap - self.entries.len())
                        .map_err(|_| allocation("unable to store sparse pivot entries"))?;
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
                break;
            }
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
    budget: ProofBudget,
) -> Result<Option<Witness>> {
    let specs = model.fit_view().effect_specs();
    // Use the sparse interpolation route for level-specific continuous slopes.
    if !specs.iter().any(|s| !s.slopes.is_empty()) {
        return Ok(None);
    }
    let descriptor_words = raw_descriptor_words(model);
    if descriptor_words >= budget.words {
        return Ok(None);
    }
    let design = RawRows::new(model, true)?;
    let width = design.width;
    if RawRows::sparse_replay_words(model, width) > budget.words {
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
    let limit = budget.words.saturating_sub(fixed).min(u32::MAX as usize);
    if limit == 0 || width >= u32::MAX as usize {
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
    let mut work_left = ProofBudget::MAX_SPARSE_WORK;
    // Each retry treats blockers found on the preceding raw replay as exact
    // equalities. This can recover valid rays on other components and costs
    // at most four interpolation/replay passes.
    for _ in 0..4 {
        let Some(remaining) = scan_left.checked_sub(pass.saturating_mul(2)) else {
            break;
        };
        scan_left = remaining;
        let Some(values) = interpolate(&design, &binding, limit, &mut work_left)? else {
            return Ok(None);
        };
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
            return Ok(Some(Witness::Sparse(values)));
        }
        if !blocked {
            break;
        }
    }
    Ok(None)
}

fn interpolate(
    design: &RawRows<'_>,
    binding: &[u64],
    limit: usize,
    work_left: &mut usize,
) -> Result<Option<Vec<i64>>> {
    let width = design.width;
    let mut basis = Echelon {
        pivots: fallible_filled(width, (0, 0), "unable to reserve sparse ray pivots")?,
        entries: Vec::new(),
        limit,
        work_left: *work_left,
    };
    let mut row = fallible_empty(width + 1, "unable to reserve sparse raw equation")?;
    let mut merge = fallible_empty(width + 1, "unable to reserve sparse elimination row")?;
    let mut positive_entries = 0;
    for boundary in [false, true] {
        for (r, &y) in design.model.outcome().iter().enumerate() {
            crate::interrupt::row_checkpoint(r)?;
            if (y == 0. && binding[r / 64] & (1u64 << (r % 64)) == 0) != boundary {
                continue;
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
            if !basis.insert(&mut row, &mut merge)? {
                return Ok(None);
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
    let Some(remaining) = basis.work_left.checked_sub(basis.entries.len()) else {
        return Ok(None);
    };
    *work_left = remaining;
    basis.solution()
}
