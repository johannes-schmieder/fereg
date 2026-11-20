//! Inside a joint categorical cell every FE indicator is constant and each
//! varying-slope column reduces to its original numeric value. Strictly
//! positive boundary multipliers in *each* cell therefore suffice, even when
//! the full design couples different cells through ordinary regressors.
use super::*;

fn gcd(mut a: i128, mut b: i128) -> i128 {
    debug_assert!(a >= 0 && b >= 0);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn dyadic(value: f64) -> Option<(i128, i32)> {
    if !value.is_finite() {
        return None;
    }
    if value == 0. {
        return Some((0, 0));
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 2047) as i32;
    let m = (bits & ((1 << 52) - 1)) | (u64::from(exponent != 0) << 52);
    let trailing = m.trailing_zeros();
    let odd = i128::from(m >> trailing);
    Some((
        if value.is_sign_negative() { -odd } else { odd },
        exponent.max(1) - 1075 + trailing as i32,
    ))
}

/// Clear the one common power-of-two denominator without a rounded floating
/// subtraction. A large exponent spread can exceed the bounded integer bank.
fn integral(values: &[f64], row: &mut [i128]) -> Option<()> {
    let mut low = i32::MAX;
    for &v in values {
        let (m, e) = dyadic(v)?;
        if m != 0 {
            low = low.min(e);
        }
    }
    for (&v, dst) in values.iter().zip(row) {
        let (m, e) = dyadic(v)?;
        *dst = if m == 0 {
            0
        } else {
            let shift = (e - low) as u32;
            if shift >= 127 {
                return None;
            }
            m.checked_mul(1i128 << shift)?
        };
        dst.checked_abs()?;
    }
    Some(())
}

/// Normalize immediately after each fraction-free pivot to limit height.
/// A zero diagonal marks unused storage; only the upper triangle is read.
fn insert(row: &mut [i128], basis: &mut [i128]) -> Result<bool> {
    for (i, b) in basis.chunks_exact_mut(row.len()).enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        if row[i] == 0 {
            continue;
        }
        if b[i] == 0 {
            b[i..].copy_from_slice(&row[i..]);
            break;
        }
        crate::interrupt::checkpoint()?;
        if eliminate(&mut row[i..], &b[i..]).is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn eliminate(row: &mut [i128], pivot: &[i128]) -> Option<()> {
    let d = gcd(row[0].checked_abs()?, pivot[0].checked_abs()?);
    let a = pivot[0] / d;
    let c = row[0] / d;
    let mut divisor = 0;
    for (v, &known) in row.iter_mut().zip(pivot) {
        *v = a.checked_mul(*v)?.checked_sub(c.checked_mul(known)?)?;
        divisor = gcd(divisor, v.checked_abs()?);
    }
    if divisor != 0 {
        for v in row {
            *v /= divisor;
        }
    }
    Some(())
}

fn solution(basis: &[i128], w: &mut [i128], positive: usize) -> Result<bool> {
    let width = w.len();
    w.fill(0);
    // Free positive coordinates may be zero. Try unit free boundary weights.
    w[positive..].fill(1);
    for i in (0..width).rev() {
        crate::interrupt::checkpoint()?;
        let b = &basis[i * width..(i + 1) * width];
        if b[i] != 0 && substitute(b, w, i).is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn substitute(b: &[i128], w: &mut [i128], i: usize) -> Option<()> {
    let mut sum = 0i128;
    for (&v, &a) in w[i + 1..].iter().zip(&b[i + 1..]) {
        sum = sum.checked_add(v.checked_mul(a)?)?;
    }
    let g = gcd(sum.checked_abs()?, b[i].checked_abs()?);
    let scale = b[i].checked_abs()? / g;
    for v in w.iter_mut() {
        *v = v.checked_mul(scale)?;
    }
    w[i] = (sum / g).checked_mul(-b[i].signum())?;
    Some(())
}

struct CellScratch {
    ids: Vec<usize>,
    column: Vec<f64>,
    row: Vec<i128>,
    basis: Vec<i128>,
}

impl CellScratch {
    fn new(width: usize) -> Result<Self> {
        let basis = width
            .checked_mul(width)
            .ok_or_else(|| allocation("sparse cell echelon overflow"))?;
        Ok(Self {
            ids: fallible_filled(width, 0, "unable to reserve sparse cell rows")?,
            column: fallible_filled(width, 0., "unable to reserve sparse cell column")?,
            row: fallible_filled(width, 0, "unable to reserve sparse cell equation")?,
            basis: fallible_filled(basis, 0, "unable to reserve sparse cell pivots")?,
        })
    }

    fn verify(
        &mut self,
        model: &PreparedPpmlModelGeneration,
        columns: &[Family<'_>],
        m: usize,
        positive: usize,
        norm2: &mut f64,
        margin: &mut i64,
    ) -> Result<bool> {
        let ids = &self.ids[..m];
        let column = &mut self.column[..m];
        let row = &mut self.row[..m];
        let basis = &mut self.basis[..m * m];
        basis.fill(0);
        // An extra constant is valid even for a model with no intercept.
        // The source cells agree on all group codes by construction.
        let numeric = || columns.iter().filter(|f| !matches!(f.values, Values::One));
        row.fill(1);
        if !insert(row, basis)? {
            return Ok(false);
        }
        for f in numeric() {
            crate::interrupt::checkpoint()?;
            for (v, &r) in column.iter_mut().zip(ids) {
                *v = f.value(model, r)?;
            }
            if integral(column, row).is_none() || !insert(row, basis)? {
                return Ok(false);
            }
        }
        if !solution(basis, row, positive)?
            || row[positive..].iter().any(|&w| w <= 0)
            || row.iter().any(|&w| i64::try_from(w).is_err())
        {
            return Ok(false);
        }
        let divisor = row.iter().map(|w| w.abs()).fold(0, gcd);
        for w in row.iter_mut() {
            *w /= divisor;
        }
        *margin = (*margin).min(row[positive..].iter().copied().min().unwrap() as i64);
        for &w in &*row {
            *norm2 += (w as f64).powi(2);
        }
        let mut constant = ExactSum::new();
        for &w in &*row {
            constant.add(1., w as i64);
        }
        if !constant.zero() {
            return Ok(false);
        }
        for f in numeric() {
            crate::interrupt::checkpoint()?;
            let mut sum = ExactSum::new();
            for (&r, &w) in ids.iter().zip(&*row) {
                if !sum.add(f.value(model, r)?, w as i64) {
                    return Ok(false);
                }
            }
            if !sum.zero() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn take(work: &mut usize, cost: usize) -> bool {
    if cost > *work {
        return false;
    }
    *work -= cost;
    true
}

pub(super) fn prove(
    model: &PreparedPpmlModelGeneration,
    design: &Design<'_>,
    limit: usize,
    work: &mut usize,
    tolerance: f64,
) -> Result<bool> {
    let y = model.outcome();
    let n = y.len();
    let codes = || design.families.iter().filter_map(|f| f.codes);
    let axes = codes().count();
    let Some(buckets) = n.checked_mul(2) else {
        return Ok(false);
    };
    let groups = n.saturating_add(buckets);
    if axes == 0 || groups >= limit || !take(work, design.pass) {
        return Ok(false);
    }
    let mut heads = fallible_filled(buckets, usize::MAX, "unable to reserve sparse cell buckets")?;
    let mut next = fallible_filled(n, usize::MAX, "unable to reserve sparse cell row chains")?;
    for (r, link) in next.iter_mut().enumerate() {
        crate::interrupt::row_checkpoint(r)?;
        let h = codes().fold(0xcbf29ce484222325u64, |h, c| {
            (h ^ u64::from(c[r])).wrapping_mul(0x100000001b3)
        });
        let mut slot = (h % buckets as u64) as usize;
        loop {
            let other = heads[slot];
            if !take(work, axes) {
                return Ok(false);
            }
            if other == usize::MAX || codes().all(|c| c[r] == c[other]) {
                *link = std::mem::replace(&mut heads[slot], r);
                break;
            }
            crate::interrupt::row_checkpoint(slot)?;
            slot = (slot + 1) % buckets;
        }
    }
    let rows = |head: usize| {
        std::iter::successors((head != usize::MAX).then_some(head), |&r| {
            (next[r] != usize::MAX).then_some(next[r])
        })
    };
    let mut width = 0;
    for (slot, &head) in heads.iter().enumerate() {
        crate::interrupt::row_checkpoint(slot)?;
        let mut len = 0;
        let mut boundary = false;
        for r in rows(head) {
            crate::interrupt::row_checkpoint(len)?;
            len += 1;
            boundary |= y[r] == 0.;
        }
        if boundary {
            width = width.max(len);
        }
    }
    let words = width.saturating_mul(width.saturating_mul(2).saturating_add(4));
    if words > limit - groups || width.saturating_mul(width) > *work {
        return Ok(false);
    }
    let mut scratch = CellScratch::new(width)?;
    let mut norm2 = 0.;
    let mut margin = i64::MAX;
    for (slot, &head) in heads.iter().enumerate() {
        crate::interrupt::row_checkpoint(slot)?;
        if head == usize::MAX {
            continue;
        }
        let mut positive = 0;
        let mut m = 0;
        for r in rows(head) {
            crate::interrupt::row_checkpoint(m)?;
            positive += usize::from(y[r] > 0.);
            m += 1;
        }
        if positive == m {
            continue;
        }
        let cost = m
            .saturating_mul(m)
            .saturating_mul(design.families.len().saturating_add(3));
        if positive == 0 || !take(work, cost) {
            return Ok(false);
        }
        let (mut p, mut z) = (0, positive);
        for r in rows(head) {
            let cursor = if y[r] > 0. { &mut p } else { &mut z };
            scratch.ids[*cursor] = r;
            *cursor += 1;
        }
        if !scratch.verify(
            model,
            &design.families,
            m,
            positive,
            &mut norm2,
            &mut margin,
        )? {
            return Ok(false);
        }
    }
    Ok(margin as f64 > tolerance * norm2.sqrt())
}
