//! An exact rational Farkas dual on the raw design of a valued compact search.
//!
//! Every binary64 value is a dyadic rational `m 2^e`, so the raw design has
//! exact rational entries and an exact no-separation dual exists whenever the
//! projected cone is infeasible. Starting from the numerical Farkas weights,
//! every boundary row but a basis set keeps a positive integer weight; the
//! weights of independent positive rows and of a boundary basis completing
//! the row space are solved exactly. The basis is selected by sparse
//! elimination modulo a prime, pivoting on the coordinate touched by the
//! fewest rows; the square system is then factored modulo further primes in
//! that fixed pivot sequence and the determinant and Cramer numerators are
//! recovered by Chinese remaindering, with the prime count fixed by a
//! Hadamard bound. Cost follows the entries and fill of the elimination, the
//! storage is requested at its measured size, and every step is charged to
//! the stage work. The dual is accepted only if every basis weight is
//! positive and the caller's exact check finds every model moment exactly
//! zero; no tolerance, rank threshold or conditioning assumption enters.
mod big;
mod modular;
pub(crate) use big::Big;

use modular::{crt_solve, primes, Budget, Elimination, Field, System};

use super::{fallible_filled, reserve};
use crate::error::Result;

/// Raw reader: emits `(coordinate, value)` for one row.
pub(crate) type Reader<'r> = &'r dyn Fn(usize, &mut dyn FnMut(usize, f64)) -> Result<()>;

/// Bits of precision kept by the largest rounded boundary weight.
const WEIGHT_BITS: i32 = 52;
/// Largest power of two in an entry of the scaled integer system.
const TOP_EXPONENT: u32 = 1 << 14;

/// An exact Farkas dual in compact form: weight `scale * k` on each
/// `integers` row, an explicit weight on each `basis` row, zero elsewhere.
#[derive(Debug)]
pub(crate) struct ExactDual {
    pub(crate) scale: Big,
    pub(crate) integers: Vec<(usize, i64)>,
    pub(crate) basis: Vec<(usize, Big)>,
}

/// A finite binary64 as `m 2^e` with `m` odd, or `(0, 0)`.
fn dyadic(x: f64) -> (i64, i32) {
    if x == 0. {
        return (0, 0);
    }
    let bits = x.to_bits();
    let negative = bits >> 63 == 1;
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    let (mut m, mut e) = if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), exponent - 1075)
    };
    let shift = m.trailing_zeros();
    m >>= shift;
    e += shift as i32;
    let m = m as i64;
    (if negative { -m } else { m }, e)
}

#[cfg(test)]
pub(super) fn dyadic_for_tests(x: f64) -> (i64, i32) {
    dyadic(x)
}

/// One raw entry `m 2^e`, `m` odd.
#[derive(Clone, Copy, Debug)]
struct Term {
    column: u32,
    mantissa: i64,
    exponent: i32,
}

/// Exact sum of dyadic values as `m 2^e` with odd `m`, when it fits.
fn merge(values: &[(usize, f64)]) -> Option<(i64, i32)> {
    let low = values.iter().map(|&(_, v)| dyadic(v).1).min()?;
    let mut sum = 0i128;
    for &(_, v) in values {
        let (m, e) = dyadic(v);
        let shift = u32::try_from(e - low).ok().filter(|&s| s <= 70)?;
        sum = sum.checked_add(i128::from(m).checked_mul(1i128 << shift)?)?;
    }
    if sum == 0 {
        return Some((0, 0));
    }
    let zeros = sum.trailing_zeros();
    i64::try_from(sum >> zeros)
        .ok()
        .map(|m| (m, low + zeros as i32))
}

/// The row's terms with duplicate coordinates merged exactly, or `None` for
/// a non-finite value or a merge beyond 64-bit mantissas.
fn read_terms(read: Reader<'_>, row: usize, width: usize) -> Result<Option<Vec<Term>>> {
    let mut raw: Vec<(usize, f64)> = Vec::new();
    let (mut invalid, mut failed) = (false, false);
    read(row, &mut |j, v| {
        if j >= width || !v.is_finite() {
            invalid = true;
        } else if v != 0. {
            if raw.try_reserve(1).is_ok() {
                raw.push((j, v));
            } else {
                failed = true;
            }
        }
    })?;
    if failed {
        return Err(crate::error::allocation(
            "unable to read an exact dyadic row",
        ));
    }
    if invalid || width > u32::MAX as usize {
        return Ok(None);
    }
    raw.sort_by_key(|&(j, _)| j);
    let mut terms = Vec::new();
    reserve(
        &mut terms,
        raw.len(),
        "unable to reserve exact dyadic terms",
    )?;
    let mut i = 0;
    while i < raw.len() {
        let j = raw[i].0;
        let end = i + raw[i..].iter().take_while(|&&(c, _)| c == j).count();
        let parts = if end == i + 1 {
            dyadic(raw[i].1)
        } else {
            match merge(&raw[i..end]) {
                Some(parts) => parts,
                None => return Ok(None),
            }
        };
        if parts.0 != 0 {
            terms.push(Term {
                column: j as u32,
                mantissa: parts.0,
                exponent: parts.1,
            });
        }
        i = end;
    }
    Ok(Some(terms))
}

/// `m 2^k` modulo the field's prime.
fn residue(field: Field, m: i64, k: u32) -> u64 {
    field.mul(field.signed(m), field.pow(2, u64::from(k)))
}

/// The rows of a nonsingular system: independent positive pivots and a
/// boundary basis, with one pivot coordinate each, selected modulo a prime.
struct Selection {
    rows: Vec<usize>,
    columns: Vec<u32>,
    boundary: Vec<bool>,
    terms: Vec<Vec<Term>>,
    scanned: usize,
}

impl Selection {
    fn push(&mut self, row: usize, column: u32, is_boundary: bool, terms: Vec<Term>) -> Result<()> {
        let message = "unable to reserve exact selection";
        reserve(&mut self.rows, 1, message)?;
        reserve(&mut self.columns, 1, message)?;
        reserve(&mut self.boundary, 1, message)?;
        reserve(&mut self.terms, 1, message)?;
        self.rows.push(row);
        self.columns.push(column);
        self.boundary.push(is_boundary);
        self.terms.push(terms);
        Ok(())
    }
}

/// The coordinates an elimination works on: all raw coordinates, or the
/// ones `map` numbers `0..local` (others `u32::MAX`, ignored).
#[derive(Clone, Copy)]
struct Columns<'m> {
    width: usize,
    map: Option<&'m [u32]>,
    local: usize,
}

impl Columns<'_> {
    fn all(width: usize) -> Self {
        Self {
            width,
            map: None,
            local: width,
        }
    }

    fn local(&self, column: usize) -> Option<u32> {
        match self.map {
            None => (column < self.width).then_some(column as u32),
            Some(map) => map.get(column).copied().filter(|&c| c != u32::MAX),
        }
    }

    /// The row's terms on these coordinates, numbered locally.
    fn terms(&self, read: Reader<'_>, row: usize) -> Result<Option<Vec<Term>>> {
        let Some(mut terms) = read_terms(read, row, self.width)? else {
            return Ok(None);
        };
        if self.map.is_some() {
            terms.retain_mut(|t| match self.local(t.column as usize) {
                Some(c) => {
                    t.column = c;
                    true
                }
                None => false,
            });
        }
        Ok(Some(terms))
    }
}

/// How many candidate rows touch each coordinate.
fn column_counts(
    read: Reader<'_>,
    columns: Columns<'_>,
    candidates: &[(usize, bool)],
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Vec<u32>> {
    let mut count = fallible_filled(columns.local, 0u32, "unable to reserve exact column counts")?;
    for (n, &(row, _)) in candidates.iter().enumerate() {
        if n % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        read(row, &mut |j, v| {
            if let Some(c) = columns.local(j).filter(|_| v != 0.) {
                count[c as usize] = count[c as usize].saturating_add(1);
            }
        })?;
    }
    Ok(count)
}

/// Independent candidate rows modulo a prime, in candidate order, until
/// `target` or the candidates run out. Each pivot is the remaining
/// coordinate that the fewest candidate rows touch.
fn select(
    read: Reader<'_>,
    columns: Columns<'_>,
    candidates: &[(usize, bool)],
    target: usize,
    budget: &mut Budget<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<std::result::Result<Selection, &'static str>> {
    let field = primes().next().expect("a 62-bit prime exists");
    if !budget.hold(columns.local.saturating_mul(4)) {
        return Ok(Err("exact_storage"));
    }
    if !budget.charge(candidates.len()) {
        return Ok(Err("exact_selection_work"));
    }
    let count = column_counts(read, columns, candidates, interrupt)?;
    let mut elimination = Elimination::new(columns.local, field)?;
    let mut selection = Selection {
        rows: Vec::new(),
        columns: Vec::new(),
        boundary: Vec::new(),
        terms: Vec::new(),
        scanned: 0,
    };
    let (mut held, mut stored) = (0usize, 0usize);
    let mut residues: Vec<(u32, u64)> = Vec::new();
    for (n, &(row, is_boundary)) in candidates.iter().enumerate() {
        if elimination.rank() == target {
            break;
        }
        if n % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        selection.scanned = n + 1;
        let Some(terms) = columns.terms(read, row)? else {
            return Ok(Err("exact_entry"));
        };
        residues.clear();
        reserve(&mut residues, terms.len(), "unable to reserve exact row")?;
        for t in &terms {
            // A common factor 2^1100 makes every entry an integer.
            let k = (t.exponent + 1100) as u32;
            residues.push((t.column, residue(field, t.mantissa, k)));
        }
        if !budget.charge(terms.len().saturating_mul(12))
            || !elimination.reduce(&residues, None, &mut budget.work)?
        {
            return Ok(Err("exact_selection_work"));
        }
        let pivot = elimination
            .remainder()
            .min_by_key(|&(c, _)| (count[c as usize], c))
            .map(|(c, _)| c);
        let Some(column) = pivot else {
            continue;
        };
        elimination.install(column)?;
        stored += terms.len();
        let live = elimination.entries.saturating_add(stored).saturating_mul(2);
        if live > held {
            let step = (live - held).max(1 << 12);
            if !budget.hold(step) {
                return Ok(Err("exact_storage"));
            }
            held += step;
        }
        selection.push(row, column, is_boundary, terms)?;
    }
    if selection.rows.is_empty() {
        return Ok(Err("exact_selection_empty"));
    }
    Ok(Ok(selection))
}

/// An exact accumulator `value 2^exponent`.
struct Dyadic {
    value: Big,
    exponent: i32,
}

impl Dyadic {
    fn add(&mut self, m: i128, e: i32) -> Result<()> {
        if m == 0 {
            return Ok(());
        }
        if self.value.is_zero() {
            self.exponent = e;
        } else if e < self.exponent {
            self.value = self.value.shl((self.exponent - e) as u32)?;
            self.exponent = e;
        }
        let term = Big::from_i128(m)?.shl((e - self.exponent) as u32)?;
        self.value = self.value.add(&term)?;
        Ok(())
    }
}

/// log2 of the Euclidean norm of entries `m 2^k`, rounded up.
fn log_norm(entries: impl Iterator<Item = (i64, u32)> + Clone) -> f64 {
    let log = |(m, k): (i64, u32)| (m.unsigned_abs() as f64).log2() + f64::from(k);
    let top = entries.clone().map(log).fold(f64::NEG_INFINITY, f64::max);
    if !top.is_finite() {
        return 0.;
    }
    let sum: f64 = entries.map(|e| 4f64.powf(log(e) - top)).sum();
    (top + 0.5 * sum.log2()) * (1. + 1e-12) + 1e-9
}

/// The integer weights `round(w 2^s)` of the boundary rows outside the
/// selection, with `s` keeping `WEIGHT_BITS` bits of the largest weight.
fn integer_weights(
    boundary: &[usize],
    weights: &[f64],
    selected: &[usize],
) -> Result<std::result::Result<Vec<(usize, i64)>, &'static str>> {
    let top = weights.iter().copied().fold(0f64, f64::max);
    if !(top.is_finite() && top > 0.) {
        return Ok(Err("exact_weight_range"));
    }
    let bits = WEIGHT_BITS - top.log2().ceil() as i32;
    if bits.abs() > 1000 {
        return Ok(Err("exact_weight_range"));
    }
    let unit = 2f64.powi(bits);
    let mut integers = Vec::new();
    reserve(
        &mut integers,
        boundary.len(),
        "unable to reserve exact weights",
    )?;
    for (&row, &w) in boundary.iter().zip(weights) {
        if selected.binary_search(&row).is_ok() {
            continue;
        }
        let k = (w * unit).round();
        if !(k >= 1. && k < (1u64 << 62) as f64) {
            return Ok(Err("exact_weight_range"));
        }
        integers.push((row, k as i64));
    }
    Ok(Ok(integers))
}

/// The integer system of the selected rows on the pivot coordinates, each
/// equation (coordinate) scaled by the power of two that makes every entry
/// of the selected and weighted rows an integer, and its right side
/// `-sum_i k_i x_i` over the weighted rows.
fn transposed_system(
    read: Reader<'_>,
    width: usize,
    selection: &Selection,
    integers: &[(usize, i64)],
    budget: &mut Budget<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<std::result::Result<(System, Vec<Big>), &'static str>> {
    let r = selection.rows.len();
    let mut column_of = fallible_filled(width, u32::MAX, "unable to reserve exact coordinates")?;
    for (k, &c) in selection.columns.iter().enumerate() {
        column_of[c as usize] = k as u32;
    }
    let mut low = fallible_filled(r, i32::MAX, "unable to reserve exact scales")?;
    for t in selection.terms.iter().flatten() {
        let j = column_of[t.column as usize];
        if j != u32::MAX {
            low[j as usize] = low[j as usize].min(t.exponent);
        }
    }
    let mut sums: Vec<Dyadic> = Vec::new();
    reserve(&mut sums, r, "unable to reserve exact right side")?;
    sums.extend((0..r).map(|_| Dyadic {
        value: Big::zero(),
        exponent: 0,
    }));
    for (n, &(row, k)) in integers.iter().enumerate() {
        if n % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        let Some(terms) = read_terms(read, row, width)? else {
            return Ok(Err("exact_entry"));
        };
        if !budget.charge(terms.len().saturating_mul(8).saturating_add(1)) {
            return Ok(Err("exact_rhs_work"));
        }
        for t in terms {
            let j = column_of[t.column as usize];
            if j != u32::MAX {
                low[j as usize] = low[j as usize].min(t.exponent);
                sums[j as usize].add(i128::from(k) * i128::from(t.mantissa), t.exponent)?;
            }
        }
    }
    let mut rows = Vec::new();
    reserve(&mut rows, r, "unable to reserve exact system")?;
    let mut top = 0u32;
    for terms in &selection.terms {
        let mut row = Vec::new();
        reserve(&mut row, terms.len(), "unable to reserve exact system row")?;
        for t in terms {
            let j = column_of[t.column as usize];
            if j != u32::MAX {
                let k = (t.exponent - low[j as usize]) as u32;
                top = top.max(k);
                row.push((j, t.mantissa, k));
            }
        }
        rows.push(row);
    }
    let mut rhs = Vec::new();
    reserve(&mut rhs, r, "unable to reserve exact right side")?;
    for (sum, &e) in sums.into_iter().zip(&low) {
        let k = if sum.value.is_zero() {
            0
        } else {
            (sum.exponent - e) as u32
        };
        if k > TOP_EXPONENT {
            return Ok(Err("exact_exponent_range"));
        }
        rhs.push(sum.value.shl(k)?.negated()?);
    }
    if top > TOP_EXPONENT {
        return Ok(Err("exact_exponent_range"));
    }
    Ok(Ok((System { rows, top }, rhs)))
}

/// Candidate rows for the basis: positive pivots, then boundary rows by
/// decreasing weight.
fn candidates(boundary: &[usize], weights: &[f64], pivots: &[usize]) -> Result<Vec<(usize, bool)>> {
    let mut order = fallible_filled(boundary.len(), 0usize, "unable to reserve exact order")?;
    for (i, slot) in order.iter_mut().enumerate() {
        *slot = i;
    }
    order.sort_by(|&a, &b| weights[b].total_cmp(&weights[a]).then(a.cmp(&b)));
    let mut candidates = Vec::new();
    reserve(
        &mut candidates,
        pivots.len() + boundary.len(),
        "unable to reserve exact candidates",
    )?;
    candidates.extend(pivots.iter().map(|&r| (r, false)));
    candidates.extend(order.iter().map(|&i| (boundary[i], true)));
    Ok(candidates)
}

/// An exact integer dual for every boundary row and the selected positive
/// rows, or the reason it was not formed.
pub(crate) fn exact_dual(
    read: Reader<'_>,
    width: usize,
    (boundary, weights, pivots): (&[usize], &[f64], &[usize]),
    target: usize,
    stage_work: usize,
    admit: &mut dyn FnMut(usize) -> bool,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<std::result::Result<ExactDual, &'static str>> {
    if target == 0 || boundary.is_empty() || boundary.len() != weights.len() {
        return Ok(Err("exact_rank_limit"));
    }
    let mut budget = Budget {
        work: stage_work,
        held: 0,
        admit,
    };
    let listed = boundary.len().saturating_add(pivots.len());
    if !budget.hold(listed.saturating_mul(4)) {
        return Ok(Err("exact_storage"));
    }
    let candidates = candidates(boundary, weights, pivots)?;
    let columns = Columns::all(width);
    let selection = match select(read, columns, &candidates, target, &mut budget, interrupt)? {
        Ok(selection) => selection,
        Err(reason) => return Ok(Err(reason)),
    };
    drop(candidates);
    let mut selected = Vec::new();
    reserve(
        &mut selected,
        selection.rows.len(),
        "unable to reserve exact marks",
    )?;
    selected.extend_from_slice(&selection.rows);
    selected.sort_unstable();
    let integers = match integer_weights(boundary, weights, &selected)? {
        Ok(integers) => integers,
        Err(reason) => return Ok(Err(reason)),
    };
    let built = transposed_system(read, width, &selection, &integers, &mut budget, interrupt)?;
    let (system, rhs) = match built {
        Ok(built) => built,
        Err(reason) => return Ok(Err(reason)),
    };
    let r = selection.rows.len();
    let rows_bits: f64 = system
        .rows
        .iter()
        .map(|row| log_norm(row.iter().map(|&(_, m, k)| (m, k))).max(0.))
        .sum();
    let rhs_top = rhs.iter().map(Big::bits).max().unwrap_or(0) as f64;
    let bits = rows_bits + rhs_top + 0.5 * (r as f64).log2() + 4.;
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=exact_dyadic_system|rank={r}|target={target}|scanned={}|bits={bits:.0}|work_left={}",
            selection.scanned, budget.work
        );
    }
    let (det, numerators) = match crt_solve(&system, &rhs, true, bits, &mut budget, interrupt)? {
        Ok(solved) => solved,
        Err(reason) => return Ok(Err(reason)),
    };
    drop((system, rhs));
    let mut basis = Vec::new();
    reserve(&mut basis, r, "unable to reserve exact basis weights")?;
    let flagged = selection.rows.iter().zip(&selection.boundary);
    for ((&row, &is_boundary), value) in flagged.zip(numerators) {
        if is_boundary && value.signum() <= 0 {
            return Ok(Err("exact_boundary_weight"));
        }
        basis.push((row, value));
    }
    Ok(Ok(ExactDual {
        scale: det,
        integers,
        basis,
    }))
}

/// An exact recession ray with arbitrary-precision integer coefficients:
/// raw coordinate `columns[k]` has coefficient `values[k] 2^-shift`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WideRay {
    pub(crate) columns: Vec<u32>,
    pub(crate) values: Vec<Big>,
    pub(crate) shift: u32,
}

/// The integer system `A_J x = -A_F t` of the selected binding rows on the
/// pivot coordinates `J`, each row scaled to integers, with the free
/// coordinates `F` at the integers `t`.
fn direct_system(
    selection: &Selection,
    pivot: &[u32],
    free: &[i64],
) -> Result<std::result::Result<(System, Vec<Big>), &'static str>> {
    let r = selection.rows.len();
    let mut rows = Vec::new();
    reserve(&mut rows, r, "unable to reserve exact ray system")?;
    let mut rhs = Vec::new();
    reserve(&mut rhs, r, "unable to reserve exact ray right side")?;
    let mut top = 0u32;
    for terms in &selection.terms {
        let low = terms.iter().map(|t| t.exponent).min().unwrap_or(0);
        let mut row = Vec::new();
        reserve(&mut row, terms.len(), "unable to reserve exact ray row")?;
        let mut sum = Big::zero();
        for t in terms {
            let k = (t.exponent - low) as u32;
            if k > TOP_EXPONENT {
                return Ok(Err("exact_exponent_range"));
            }
            top = top.max(k);
            let c = t.column as usize;
            if pivot[c] != u32::MAX {
                row.push((pivot[c], t.mantissa, k));
            } else if free[c] != 0 {
                let product = i128::from(t.mantissa) * i128::from(free[c]);
                sum = sum.add(&Big::from_i128(product)?.shl(k)?)?;
            }
        }
        rows.push(row);
        rhs.push(sum.negated()?);
    }
    Ok(Ok((System { rows, top }, rhs)))
}

/// An exact integer null vector of the binding rows on the active
/// coordinates, near the numerical `coefficients`: binding rows are reduced
/// modulo a prime until one active coordinate is left free (or they run
/// out), the free coordinates keep the rounded numerical values and the
/// pivot coordinates are solved exactly. The caller replays every raw row.
pub(super) fn exact_ray(
    read: Reader<'_>,
    coefficients: &[f64],
    (active, binding): (&[usize], &[usize]),
    stage_work: usize,
    admit: &mut dyn FnMut(usize) -> bool,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<std::result::Result<WideRay, &'static str>> {
    let (width, k) = (coefficients.len(), active.len());
    if k < 2 || binding.is_empty() || active.iter().any(|&j| j >= width) {
        return Ok(Err("exact_ray_support"));
    }
    let mut budget = Budget {
        work: stage_work,
        held: 0,
        admit,
    };
    let listed = width.saturating_add(binding.len()).saturating_mul(2);
    if !budget.hold(listed) {
        return Ok(Err("exact_storage"));
    }
    let mut map = fallible_filled(width, u32::MAX, "unable to reserve exact ray map")?;
    for (p, &j) in active.iter().enumerate() {
        map[j] = p as u32;
    }
    let mut candidates = Vec::new();
    reserve(
        &mut candidates,
        binding.len(),
        "unable to reserve exact ray rows",
    )?;
    candidates.extend(binding.iter().map(|&r| (r, false)));
    let columns = Columns {
        width,
        map: Some(&map),
        local: k,
    };
    let selection = match select(read, columns, &candidates, k - 1, &mut budget, interrupt)? {
        Ok(selection) => selection,
        Err(reason) => return Ok(Err(reason)),
    };
    drop(candidates);
    let mut pivot = fallible_filled(k, u32::MAX, "unable to reserve exact ray pivots")?;
    for (position, &c) in selection.columns.iter().enumerate() {
        pivot[c as usize] = position as u32;
    }
    let top = (0..k)
        .filter(|&c| pivot[c] == u32::MAX)
        .map(|c| coefficients[active[c]].abs())
        .fold(0f64, f64::max);
    if !(top.is_finite() && top > 0.) {
        return Ok(Err("exact_ray_free"));
    }
    let unit = 2f64.powi((WEIGHT_BITS - top.log2().ceil() as i32).clamp(-1000, 1000));
    let mut free = fallible_filled(k, 0i64, "unable to reserve exact ray values")?;
    for c in (0..k).filter(|&c| pivot[c] == u32::MAX) {
        free[c] = (coefficients[active[c]] * unit).round() as i64;
    }
    let (system, rhs) = match direct_system(&selection, &pivot, &free)? {
        Ok(built) => built,
        Err(reason) => return Ok(Err(reason)),
    };
    let bits: f64 = system
        .rows
        .iter()
        .zip(&rhs)
        .map(|(row, b)| {
            let norm = log_norm(row.iter().map(|&(_, m, k)| (m, k))).max(0.);
            norm.max(b.bits() as f64) + 0.5
        })
        .sum::<f64>()
        + 4.;
    let (det, solved) = match crt_solve(&system, &rhs, false, bits, &mut budget, interrupt)? {
        Ok(solved) => solved,
        Err(reason) => return Ok(Err(reason)),
    };
    let mut entries: Vec<(u32, Big)> = Vec::new();
    reserve(&mut entries, k, "unable to reserve exact ray")?;
    for c in 0..k {
        let value = match pivot[c] {
            u32::MAX => det.mul_i64(free[c])?,
            p => solved[p as usize].clone(),
        };
        if !value.is_zero() {
            entries.push((active[c] as u32, value));
        }
    }
    entries.sort_by_key(|&(c, _)| c);
    let shift = entries.iter().map(|(_, v)| v.bits()).max().unwrap_or(0);
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=exact_dyadic_ray|active={k}|rank={}|scanned={}|bits={bits:.0}|shift={shift}",
            selection.rows.len(),
            selection.scanned
        );
    }
    let (columns, values) = entries.into_iter().unzip();
    Ok(Ok(WideRay {
        columns,
        values,
        shift: u32::try_from(shift).unwrap_or(u32::MAX),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dyadic_decomposition_is_exact() {
        for x in [1., -3., 0.75, 1e-300, -2.5e-320, 123456.789, f64::MAX] {
            let (m, e) = dyadic(x);
            let back = (m as f64) * 2f64.powi(e.max(-1074));
            if e >= -1022 {
                assert_eq!(back, x, "{x}");
            }
            assert!(m % 2 != 0);
        }
        assert_eq!(dyadic(0.), (0, 0));
        assert_eq!(merge(&[(0, 0.5), (0, 0.25)]), Some((3, -2)));
        assert_eq!(merge(&[(0, 0.5), (0, -0.5)]), Some((0, 0)));
    }

    #[test]
    fn primes_are_prime_and_decreasing() {
        let list: Vec<u64> = primes().take(5).map(|f| f.p).collect();
        assert!(list.windows(2).all(|w| w[0] > w[1]));
        assert!(list
            .iter()
            .all(|&p| p < 1 << 62 && p > 1 << 61 && modular::is_prime(p)));
    }

    /// The exact moment of every column: `sum_i d_i x_ij` scaled by 2^1100.
    fn moments(rows: &[Vec<(usize, f64)>], dual: &ExactDual, width: usize) -> Vec<Big> {
        let mut weight = vec![None; rows.len()];
        for &(row, k) in &dual.integers {
            weight[row] = Some(dual.scale.mul_i64(k).unwrap());
        }
        for (row, d) in &dual.basis {
            weight[*row] = Some(d.clone());
        }
        (0..width)
            .map(|column| {
                let mut sum = Big::zero();
                for (i, w) in weight.iter().enumerate() {
                    let Some(w) = w else { continue };
                    for &(j, v) in &rows[i] {
                        if j == column {
                            let (m, e) = dyadic(v);
                            let term = w.mul_i64(m).unwrap().shl((e + 1100) as u32).unwrap();
                            sum = sum.add(&term).unwrap();
                        }
                    }
                }
                sum
            })
            .collect()
    }

    #[test]
    fn balances_a_dyadic_cell_exactly() {
        // Rows: coordinates (intercept, level slope z). The positive row fixes
        // a direction; boundary rows must balance with positive weights.
        let rows: Vec<Vec<(usize, f64)>> = vec![
            vec![(0, 1.), (1, 0.3)],             // positive
            vec![(0, 1.), (1, -0.7)],            // boundary
            vec![(0, 1.), (1, 1.1)],             // boundary
            vec![(0, 1.), (1, 0.25)],            // boundary
            vec![(0, 0.5), (0, 0.5), (1, 1e-3)], // boundary, duplicate coordinate
        ];
        let read = |i: usize, f: &mut dyn FnMut(usize, f64)| -> Result<()> {
            for &(j, v) in &rows[i] {
                f(j, v);
            }
            Ok(())
        };
        let boundary = [1usize, 2, 3, 4];
        let weights = [1.6, 1.0, 1.3, 0.2];
        for target in [2, 3] {
            let dual = exact_dual(
                &read,
                2,
                (&boundary, &weights, &[0]),
                target,
                usize::MAX,
                &mut |_| true,
                &mut || Ok(()),
            )
            .unwrap()
            .unwrap();
            assert_eq!(dual.scale.signum(), 1);
            assert!(dual.integers.iter().all(|&(_, k)| k >= 1));
            for (row, d) in &dual.basis {
                if *row != 0 {
                    assert_eq!(d.signum(), 1);
                }
            }
            let zero = moments(&rows, &dual, 2).iter().all(Big::is_zero);
            assert!(zero, "target {target}");
        }
    }

    #[test]
    fn lifts_a_slope_ray_exactly() {
        // Coordinates (intercept, slope). One positive row at z = 0.3 binds
        // the ray b0 + 0.3 b1 = 0; zero rows above it are strictly negative.
        let rows: Vec<Vec<(usize, f64)>> = vec![
            vec![(0, 1.), (1, 0.3)],
            vec![(0, 1.), (1, 0.9)],
            vec![(0, 1.), (1, 1.7)],
        ];
        let read = |i: usize, f: &mut dyn FnMut(usize, f64)| -> Result<()> {
            for &(j, v) in &rows[i] {
                f(j, v);
            }
            Ok(())
        };
        let ray = exact_ray(
            &read,
            &[0.3000000001, -1.],
            (&[0, 1], &[0]),
            usize::MAX,
            &mut |_| true,
            &mut || Ok(()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(ray.columns, vec![0, 1]);
        // Exact scores: zero on the positive row, negative elsewhere.
        for (i, row) in rows.iter().enumerate() {
            let mut sum = Big::zero();
            for &(j, v) in row {
                let (m, e) = dyadic(v);
                let term = ray.values[j]
                    .mul_i64(m)
                    .unwrap()
                    .shl((e + 1100) as u32)
                    .unwrap();
                sum = sum.add(&term).unwrap();
            }
            assert_eq!(sum.signum(), if i == 0 { 0 } else { -1 }, "row {i}");
        }
    }

    #[test]
    fn storage_and_work_refusals_name_their_stage() {
        let rows: Vec<Vec<(usize, f64)>> = vec![vec![(0, 1.)], vec![(0, 1.)]];
        let read = |i: usize, f: &mut dyn FnMut(usize, f64)| -> Result<()> {
            for &(j, v) in &rows[i] {
                f(j, v);
            }
            Ok(())
        };
        let sets = (&[1usize][..], &[1.][..], &[0usize][..]);
        let refused = exact_dual(&read, 1, sets, 1, 0, &mut |_| true, &mut || Ok(()));
        assert_eq!(refused.unwrap().unwrap_err(), "exact_selection_work");
        let refused = exact_dual(
            &read,
            1,
            sets,
            1,
            usize::MAX,
            &mut |_| false,
            &mut || Ok(()),
        );
        assert_eq!(refused.unwrap().unwrap_err(), "exact_storage");
    }
}
