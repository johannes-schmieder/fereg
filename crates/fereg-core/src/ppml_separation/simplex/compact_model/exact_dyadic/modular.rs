//! Modular arithmetic for the exact dyadic certificates: 62-bit prime fields
//! with Barrett reduction, sparse elimination over a fixed pivot sequence and
//! Chinese remaindering of a nonsingular integer system.
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::super::{fallible_filled, reserve};
use super::Big;
use crate::error::Result;

/// A prime field `Z/p` with `2^61 < p < 2^62`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Field {
    pub(super) p: u64,
    /// floor(2^124 / p).
    mu: u128,
    /// 2^64 mod p.
    radix: u64,
}

impl Field {
    pub(super) fn new(p: u64) -> Self {
        debug_assert!(p > 1 << 61 && p < 1 << 62);
        let mu = (1u128 << 124) / u128::from(p);
        let radix = ((1u128 << 64) % u128::from(p)) as u64;
        Self { p, mu, radix }
    }

    /// `x mod p` for `x < p^2` (Barrett: the quotient estimate is short by
    /// at most two).
    fn reduce(&self, x: u128) -> u64 {
        let q = ((x >> 60) * self.mu) >> 64;
        let mut r = (x - q * u128::from(self.p)) as u64;
        while r >= self.p {
            r -= self.p;
        }
        r
    }

    pub(super) fn mul(&self, a: u64, b: u64) -> u64 {
        self.reduce(u128::from(a) * u128::from(b))
    }

    pub(super) fn add(&self, a: u64, b: u64) -> u64 {
        let s = a + b;
        if s >= self.p {
            s - self.p
        } else {
            s
        }
    }

    pub(super) fn sub(&self, a: u64, b: u64) -> u64 {
        if a >= b {
            a - b
        } else {
            a + self.p - b
        }
    }

    pub(super) fn pow(&self, mut base: u64, mut exponent: u64) -> u64 {
        let mut result = 1;
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = self.mul(result, base);
            }
            base = self.mul(base, base);
            exponent >>= 1;
        }
        result
    }

    pub(super) fn inverse(&self, a: u64) -> u64 {
        self.pow(a, self.p - 2)
    }

    /// The residue of a signed 64-bit integer.
    pub(super) fn signed(&self, m: i64) -> u64 {
        let r = m.unsigned_abs() % self.p;
        if m < 0 && r != 0 {
            self.p - r
        } else {
            r
        }
    }

    /// The residue of a signed big integer.
    pub(super) fn big(&self, value: &Big) -> u64 {
        let mut r = 0;
        for &limb in value.limbs().iter().rev() {
            r = self.add(self.mul(r, self.radix), limb % self.p);
        }
        if value.is_negative() && r != 0 {
            self.p - r
        } else {
            r
        }
    }
}

/// Deterministic Miller-Rabin for 64-bit integers.
pub(super) fn is_prime(n: u64) -> bool {
    let mul = |a: u64, b: u64| ((u128::from(a) * u128::from(b)) % u128::from(n)) as u64;
    if n < 2 {
        return false;
    }
    for p in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if n % p == 0 {
            return n == p;
        }
    }
    let (mut d, mut s) = (n - 1, 0);
    while d % 2 == 0 {
        d /= 2;
        s += 1;
    }
    'witness: for a in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let (mut x, mut base, mut e) = (1u64, a, d);
        while e > 0 {
            if e & 1 == 1 {
                x = mul(x, base);
            }
            base = mul(base, base);
            e >>= 1;
        }
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..s {
            x = mul(x, x);
            if x == n - 1 {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

/// Primes below 2^62 in decreasing order (all above 2^61 for any count the
/// certificates request).
pub(super) fn primes() -> impl Iterator<Item = Field> {
    let mut candidate = (1u64 << 62) - 1;
    std::iter::from_fn(move || {
        while candidate > (1 << 61) + 1 {
            let n = candidate;
            candidate -= 2;
            if is_prime(n) {
                return Some(Field::new(n));
            }
        }
        None
    })
}

/// Sparse Gaussian elimination modulo a prime. Pivot rows are normalized and
/// stored without their pivot entry; a reduced row has every pivot column
/// eliminated, in pivot order, so the pivot rows are unit upper triangular
/// in insertion order.
pub(super) struct Elimination {
    field: Field,
    pivot_of: Vec<u32>,
    pub(super) columns: Vec<u32>,
    pub(super) rows: Vec<Vec<(u32, u64)>>,
    acc: Vec<u64>,
    mark: Vec<u32>,
    stamp: u32,
    touched: Vec<u32>,
    heap: BinaryHeap<Reverse<u32>>,
    /// Stored entries of the pivot rows.
    pub(super) entries: usize,
}

impl Elimination {
    pub(super) fn new(width: usize, field: Field) -> Result<Self> {
        let mut touched = Vec::new();
        reserve(&mut touched, width, "unable to reserve exact elimination")?;
        let mut heap = BinaryHeap::new();
        heap.try_reserve(width)
            .map_err(|_| crate::error::allocation("unable to reserve exact elimination"))?;
        Ok(Self {
            field,
            pivot_of: fallible_filled(width, u32::MAX, "unable to reserve exact pivots")?,
            columns: Vec::new(),
            rows: Vec::new(),
            acc: fallible_filled(width, 0, "unable to reserve exact accumulator")?,
            mark: fallible_filled(width, 0, "unable to reserve exact marks")?,
            stamp: 0,
            touched,
            heap,
            entries: 0,
        })
    }

    pub(super) fn rank(&self) -> usize {
        self.columns.len()
    }

    fn touch(&mut self, c: u32) {
        let i = c as usize;
        if self.mark[i] != self.stamp {
            self.mark[i] = self.stamp;
            self.acc[i] = 0;
            self.touched.push(c);
            if self.pivot_of[i] != u32::MAX {
                self.heap.push(Reverse(self.pivot_of[i]));
            }
        }
    }

    /// Accumulate `row` and eliminate every pivot column in pivot order;
    /// `multipliers` receives `(pivot, factor)`. False when `work` runs out.
    pub(super) fn reduce(
        &mut self,
        row: &[(u32, u64)],
        mut multipliers: Option<&mut Vec<(u32, u64)>>,
        work: &mut usize,
    ) -> Result<bool> {
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.mark.fill(0);
            self.stamp = 1;
        }
        self.touched.clear();
        self.heap.clear();
        let field = self.field;
        for &(c, v) in row {
            self.touch(c);
            self.acc[c as usize] = field.add(self.acc[c as usize], v);
        }
        while let Some(Reverse(j)) = self.heap.pop() {
            let column = self.columns[j as usize] as usize;
            let f = self.acc[column];
            if f == 0 {
                continue;
            }
            self.acc[column] = 0;
            let len = self.rows[j as usize].len();
            let Some(left) = work.checked_sub(len + 1) else {
                *work = 0;
                return Ok(false);
            };
            *work = left;
            for t in 0..len {
                let (c, v) = self.rows[j as usize][t];
                self.touch(c);
                self.acc[c as usize] = field.sub(self.acc[c as usize], field.mul(f, v));
            }
            if let Some(m) = multipliers.as_deref_mut() {
                reserve(m, 1, "unable to reserve exact multipliers")?;
                m.push((j, f));
            }
        }
        Ok(true)
    }

    /// Nonzero non-pivot entries of the last reduced row.
    pub(super) fn remainder(&self) -> impl Iterator<Item = (u32, u64)> + '_ {
        self.touched
            .iter()
            .filter(|&&c| self.pivot_of[c as usize] == u32::MAX && self.acc[c as usize] != 0)
            .map(|&c| (c, self.acc[c as usize]))
    }

    /// The reduced row's value at `column` (zero when untouched).
    pub(super) fn value(&self, column: u32) -> u64 {
        let i = column as usize;
        if self.mark[i] == self.stamp && self.pivot_of[i] == u32::MAX {
            self.acc[i]
        } else {
            0
        }
    }

    /// Install the last reduced row with pivot `column`; returns its pivot
    /// value before normalization.
    pub(super) fn install(&mut self, column: u32) -> Result<u64> {
        let value = self.value(column);
        debug_assert!(value != 0);
        let inverse = self.field.inverse(value);
        let mut rest = Vec::new();
        reserve(
            &mut rest,
            self.remainder().count().saturating_sub(1),
            "unable to reserve exact pivot row",
        )?;
        let field = self.field;
        for (c, v) in self.remainder() {
            if c != column {
                rest.push((c, field.mul(v, inverse)));
            }
        }
        self.entries += rest.len();
        self.pivot_of[column as usize] = self.columns.len() as u32;
        reserve(&mut self.columns, 1, "unable to reserve exact pivots")?;
        self.columns.push(column);
        reserve(&mut self.rows, 1, "unable to reserve exact pivot rows")?;
        self.rows.push(rest);
        Ok(value)
    }
}

/// `A = L' U` for a square system in a fixed pivot sequence: row `k` has
/// pivot column `k`, `U` is unit upper triangular, `L'` lower triangular
/// with diagonal `diagonal`.
pub(super) struct Factor {
    field: Field,
    upper: Vec<Vec<(u32, u64)>>,
    lower: Vec<Vec<(u32, u64)>>,
    diagonal: Vec<u64>,
}

impl Factor {
    /// Factor rows whose entries are `(column, residue)`, pivoting row `k`
    /// on column `k`; `None` when the prime divides a pivot.
    pub(super) fn new(
        rows: &[Vec<(u32, u64)>],
        field: Field,
        work: &mut usize,
    ) -> Result<Option<Self>> {
        let r = rows.len();
        let mut elimination = Elimination::new(r, field)?;
        let mut lower = Vec::new();
        reserve(&mut lower, r, "unable to reserve exact lower factor")?;
        let mut diagonal = Vec::new();
        reserve(&mut diagonal, r, "unable to reserve exact diagonal")?;
        for (k, row) in rows.iter().enumerate() {
            let mut multipliers = Vec::new();
            if !elimination.reduce(row, Some(&mut multipliers), work)? {
                return Ok(None);
            }
            if elimination.value(k as u32) == 0 {
                return Ok(None);
            }
            diagonal.push(elimination.install(k as u32)?);
            lower.push(multipliers);
        }
        Ok(Some(Self {
            field,
            upper: elimination.rows,
            lower,
            diagonal,
        }))
    }

    pub(super) fn determinant(&self) -> u64 {
        self.diagonal.iter().fold(1, |d, &v| self.field.mul(d, v))
    }

    /// Solve `A^T u = t` in place.
    pub(super) fn solve_transposed(&self, t: &mut [u64]) {
        let f = self.field;
        for j in 0..t.len() {
            let y = t[j];
            if y != 0 {
                for &(c, v) in &self.upper[j] {
                    t[c as usize] = f.sub(t[c as usize], f.mul(v, y));
                }
            }
        }
        for k in (0..t.len()).rev() {
            let u = f.mul(t[k], f.inverse(self.diagonal[k]));
            t[k] = u;
            for &(j, m) in &self.lower[k] {
                t[j as usize] = f.sub(t[j as usize], f.mul(m, u));
            }
        }
    }

    /// Solve `A x = t` in place.
    pub(super) fn solve(&self, t: &mut [u64]) {
        let f = self.field;
        for k in 0..t.len() {
            let mut s = t[k];
            for &(j, m) in &self.lower[k] {
                s = f.sub(s, f.mul(m, t[j as usize]));
            }
            t[k] = f.mul(s, f.inverse(self.diagonal[k]));
        }
        for k in (0..t.len()).rev() {
            let mut s = t[k];
            for &(c, v) in &self.upper[k] {
                s = f.sub(s, f.mul(v, t[c as usize]));
            }
            t[k] = s;
        }
    }

    pub(super) fn entries(&self) -> usize {
        self.upper.iter().chain(&self.lower).map(Vec::len).sum()
    }
}

/// A square integer system: row `k` lists `(column, m, k)` for the entry
/// `m 2^k`, `k >= 0`.
pub(super) struct System {
    pub(super) rows: Vec<Vec<(u32, i64, u32)>>,
    pub(super) top: u32,
}

/// Storage and work of one exact certificate stage.
pub(super) struct Budget<'a> {
    pub(super) work: usize,
    pub(super) held: usize,
    pub(super) admit: &'a mut dyn FnMut(usize) -> bool,
}

impl Budget<'_> {
    pub(super) fn charge(&mut self, work: usize) -> bool {
        match self.work.checked_sub(work) {
            Some(left) => {
                self.work = left;
                true
            }
            None => false,
        }
    }

    pub(super) fn hold(&mut self, words: usize) -> bool {
        let total = self.held.saturating_add(words);
        if (self.admit)(total) {
            self.held = total;
            true
        } else {
            false
        }
    }
}

/// The determinant `d > 0` and `d * x` over the integers for `A^T x = b`
/// (`transposed`) or `A x = b`, by Chinese remaindering with enough primes
/// for `bits` (a bound on log2 of the determinant and every Cramer
/// numerator).
pub(super) fn crt_solve(
    system: &System,
    rhs: &[Big],
    transposed: bool,
    bits: f64,
    budget: &mut Budget<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<std::result::Result<(Big, Vec<Big>), &'static str>> {
    let r = system.rows.len();
    if !bits.is_finite() || bits > 1e7 {
        return Ok(Err("exact_bits_limit"));
    }
    let needed = ((bits + 2.) / 61.).ceil() as usize;
    // Residue rows, final values and the modulus are held at once.
    let entries: usize = system.rows.iter().map(Vec::len).sum();
    let words = (r + 1)
        .saturating_mul(needed + 1)
        .saturating_add(entries.saturating_mul(4))
        .saturating_add((system.top as usize).saturating_add(4 * r));
    if !budget.hold(words) {
        return Ok(Err("exact_storage"));
    }
    let mut residues: Vec<Vec<(u32, u64)>> = Vec::new();
    reserve(&mut residues, r, "unable to reserve exact residues")?;
    for row in &system.rows {
        residues.push(fallible_filled(
            row.len(),
            (0u32, 0u64),
            "unable to reserve exact residues",
        )?);
    }
    let mut powers = fallible_filled(
        system.top as usize + 1,
        0u64,
        "unable to reserve exact powers",
    )?;
    let mut values = fallible_filled(r + 1, Big::zero(), "unable to reserve exact values")?;
    let mut modulus = Big::from_i128(1)?;
    let mut t = fallible_filled(r, 0u64, "unable to reserve exact right side")?;
    let (mut used, mut skipped) = (0usize, 0usize);
    for field in primes() {
        if used == needed {
            break;
        }
        // A nonzero determinant has at most `needed` prime factors here.
        if skipped > needed {
            return Ok(Err("exact_singular"));
        }
        interrupt()?;
        powers[0] = 1;
        for k in 1..powers.len() {
            powers[k] = field.add(powers[k - 1], powers[k - 1]);
        }
        for (out, row) in residues.iter_mut().zip(&system.rows) {
            for (slot, &(c, m, k)) in out.iter_mut().zip(row) {
                *slot = (c, field.mul(field.signed(m), powers[k as usize]));
            }
        }
        if !budget.charge(entries.saturating_mul(2).saturating_add(powers.len())) {
            return Ok(Err("exact_solve_work"));
        }
        let Some(factor) = Factor::new(&residues, field, &mut budget.work)? else {
            if budget.work == 0 {
                return Ok(Err("exact_solve_work"));
            }
            skipped += 1;
            continue;
        };
        for (slot, b) in t.iter_mut().zip(rhs) {
            *slot = field.big(b);
        }
        if transposed {
            factor.solve_transposed(&mut t);
        } else {
            factor.solve(&mut t);
        }
        let det = factor.determinant();
        let limbs = modulus.limbs().len() + 1;
        if !budget.charge(
            factor
                .entries()
                .saturating_add((r + 1).saturating_mul(limbs).saturating_mul(3)),
        ) {
            return Ok(Err("exact_solve_work"));
        }
        let inverse = field.inverse(field.big(&modulus));
        for (k, value) in values.iter_mut().enumerate() {
            let target = if k < r { field.mul(det, t[k]) } else { det };
            let step = field.mul(field.sub(target, field.big(value)), inverse);
            if step != 0 {
                *value = value.add(&modulus.mul_i64(step as i64)?)?;
            }
        }
        modulus = modulus.mul_i64(field.p as i64)?;
        used += 1;
    }
    if used < needed {
        return Ok(Err("exact_primes_exhausted"));
    }
    let half = modulus.half()?;
    for value in &mut values {
        if value.cmp(&half) == std::cmp::Ordering::Greater {
            *value = value.sub(&modulus)?;
        }
    }
    let mut det = values.pop().expect("determinant value");
    if det.is_zero() {
        return Ok(Err("exact_singular"));
    }
    if det.signum() < 0 {
        det = det.negated()?;
        for value in &mut values {
            *value = value.negated()?;
        }
    }
    Ok(Ok((det, values)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barrett_matches_wide_remainder() {
        let field = primes().next().unwrap();
        let p = field.p;
        for &(a, b) in &[
            (0, 0),
            (1, p - 1),
            (p - 1, p - 1),
            (p / 3, p / 7 + 5),
            (12345, 678910),
        ] {
            let want = ((u128::from(a) * u128::from(b)) % u128::from(p)) as u64;
            assert_eq!(field.mul(a, b), want);
        }
        assert_eq!(field.mul(field.inverse(987654321), 987654321), 1);
        let big = Big::from_i128(-(1i128 << 100) - 17).unwrap();
        let want = (-(1i128 << 100) - 17).rem_euclid(i128::from(p)) as u64;
        assert_eq!(field.big(&big), want);
    }

    #[test]
    fn sparse_factor_solves_both_systems() {
        // A = [[2,1,0],[0,3,1],[1,0,4]], b = [1,2,3].
        let system = System {
            rows: vec![
                vec![(0, 1, 1), (1, 1, 0)],
                vec![(1, 3, 0), (2, 1, 0)],
                vec![(0, 1, 0), (2, 1, 2)],
            ],
            top: 2,
        };
        let rhs: Vec<Big> = [1, 2, 3]
            .iter()
            .map(|&v| Big::from_i128(v).unwrap())
            .collect();
        let mut admit = |_| true;
        let mut budget = Budget {
            work: usize::MAX,
            held: 0,
            admit: &mut admit,
        };
        let dense = [[2i128, 1, 0], [0, 3, 1], [1, 0, 4]];
        for transposed in [false, true] {
            let (det, x) = crt_solve(&system, &rhs, transposed, 64., &mut budget, &mut || Ok(()))
                .unwrap()
                .unwrap();
            assert_eq!(det, Big::from_i128(25).unwrap());
            for i in 0..3 {
                let mut sum = Big::zero();
                for j in 0..3 {
                    let a = if transposed { dense[j][i] } else { dense[i][j] };
                    sum = sum.add(&x[j].mul_i64(a as i64).unwrap()).unwrap();
                }
                assert_eq!(sum, rhs[i].mul(&det).unwrap(), "row {i}");
            }
        }
    }
}
