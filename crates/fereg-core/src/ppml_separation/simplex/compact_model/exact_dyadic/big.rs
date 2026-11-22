//! Minimal signed arbitrary-precision integers for exact dyadic certificates:
//! sums of products of integers with binary64 values, Chinese remaindering
//! and sign tests. Every allocation is fallible.
use std::cmp::Ordering;

use crate::error::{allocation, Result};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Big {
    negative: bool,
    /// Little-endian magnitude without high zero limbs; zero is empty.
    limbs: Vec<u64>,
}

fn reserve(limbs: &mut Vec<u64>, extra: usize) -> Result<()> {
    limbs
        .try_reserve(extra)
        .map_err(|_| allocation("unable to reserve exact dyadic integer"))
}

fn trim(limbs: &mut Vec<u64>) {
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
}

fn cmp_magnitude(a: &[u64], b: &[u64]) -> Ordering {
    a.len().cmp(&b.len()).then_with(|| {
        for (x, y) in a.iter().rev().zip(b.iter().rev()) {
            match x.cmp(y) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        Ordering::Equal
    })
}

/// a + b on magnitudes.
fn add_magnitude(a: &[u64], b: &[u64]) -> Result<Vec<u64>> {
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let mut out = Vec::new();
    reserve(&mut out, long.len() + 1)?;
    let mut carry = 0u64;
    for (i, &x) in long.iter().enumerate() {
        let y = short.get(i).copied().unwrap_or(0);
        let (s1, c1) = x.overflowing_add(y);
        let (s2, c2) = s1.overflowing_add(carry);
        out.push(s2);
        carry = u64::from(c1) + u64::from(c2);
    }
    if carry != 0 {
        out.push(carry);
    }
    Ok(out)
}

/// a - b on magnitudes with a >= b.
fn sub_magnitude(a: &[u64], b: &[u64]) -> Result<Vec<u64>> {
    let mut out = Vec::new();
    reserve(&mut out, a.len())?;
    let mut borrow = 0u64;
    for (i, &x) in a.iter().enumerate() {
        let y = b.get(i).copied().unwrap_or(0);
        let (d1, b1) = x.overflowing_sub(y);
        let (d2, b2) = d1.overflowing_sub(borrow);
        out.push(d2);
        borrow = u64::from(b1) + u64::from(b2);
    }
    trim(&mut out);
    Ok(out)
}

impl Big {
    pub(crate) fn zero() -> Self {
        Self::default()
    }

    pub(crate) fn from_i128(value: i128) -> Result<Self> {
        let negative = value < 0;
        let mut magnitude = value.unsigned_abs();
        let mut limbs = Vec::new();
        reserve(&mut limbs, 2)?;
        while magnitude != 0 {
            limbs.push(magnitude as u64);
            magnitude >>= 64;
        }
        Ok(Self {
            negative: negative && !limbs.is_empty(),
            limbs,
        })
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub(crate) fn signum(&self) -> i32 {
        if self.limbs.is_empty() {
            0
        } else if self.negative {
            -1
        } else {
            1
        }
    }

    pub(crate) fn negated(&self) -> Result<Self> {
        let mut limbs = Vec::new();
        reserve(&mut limbs, self.limbs.len())?;
        limbs.extend_from_slice(&self.limbs);
        Ok(Self {
            negative: !self.negative && !limbs.is_empty(),
            limbs,
        })
    }

    pub(crate) fn is_negative(&self) -> bool {
        self.negative
    }

    /// Little-endian magnitude limbs.
    pub(crate) fn limbs(&self) -> &[u64] {
        &self.limbs
    }

    /// self * other (schoolbook).
    pub(crate) fn mul(&self, other: &Self) -> Result<Self> {
        if self.is_zero() || other.is_zero() {
            return Ok(Self::zero());
        }
        let mut out = Vec::new();
        reserve(&mut out, self.limbs.len() + other.limbs.len())?;
        out.resize(self.limbs.len() + other.limbs.len(), 0);
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u128;
            for (j, &b) in other.limbs.iter().enumerate() {
                let t = u128::from(a) * u128::from(b) + u128::from(out[i + j]) + carry;
                out[i + j] = t as u64;
                carry = t >> 64;
            }
            out[i + other.limbs.len()] = carry as u64;
        }
        trim(&mut out);
        Ok(Self::signed(self.negative != other.negative, out))
    }

    pub(crate) fn bits(&self) -> u64 {
        match self.limbs.last() {
            None => 0,
            Some(&top) => (self.limbs.len() as u64 - 1) * 64 + u64::from(64 - top.leading_zeros()),
        }
    }

    fn signed(negative: bool, limbs: Vec<u64>) -> Self {
        Self {
            negative: negative && !limbs.is_empty(),
            limbs,
        }
    }

    pub(crate) fn add(&self, other: &Self) -> Result<Self> {
        if self.negative == other.negative {
            return Ok(Self::signed(
                self.negative,
                add_magnitude(&self.limbs, &other.limbs)?,
            ));
        }
        match cmp_magnitude(&self.limbs, &other.limbs) {
            Ordering::Equal => Ok(Self::zero()),
            Ordering::Greater => Ok(Self::signed(
                self.negative,
                sub_magnitude(&self.limbs, &other.limbs)?,
            )),
            Ordering::Less => Ok(Self::signed(
                other.negative,
                sub_magnitude(&other.limbs, &self.limbs)?,
            )),
        }
    }

    pub(crate) fn sub(&self, other: &Self) -> Result<Self> {
        self.add(&other.negated()?)
    }

    /// self * factor for a signed 64-bit factor.
    pub(crate) fn mul_i64(&self, factor: i64) -> Result<Self> {
        let magnitude = factor.unsigned_abs();
        if magnitude == 0 || self.is_zero() {
            return Ok(Self::zero());
        }
        let mut out = Vec::new();
        reserve(&mut out, self.limbs.len() + 1)?;
        let mut carry = 0u128;
        for &limb in &self.limbs {
            let product = u128::from(limb) * u128::from(magnitude) + carry;
            out.push(product as u64);
            carry = product >> 64;
        }
        if carry != 0 {
            out.push(carry as u64);
        }
        Ok(Self::signed(self.negative != (factor < 0), out))
    }

    /// self * 2^bits.
    pub(crate) fn shl(&self, bits: u32) -> Result<Self> {
        if self.is_zero() {
            return Ok(Self::zero());
        }
        let (words, offset) = ((bits / 64) as usize, bits % 64);
        let mut out = Vec::new();
        reserve(&mut out, self.limbs.len() + words + 1)?;
        out.resize(words, 0);
        if offset == 0 {
            out.extend_from_slice(&self.limbs);
        } else {
            let mut carry = 0u64;
            for &limb in &self.limbs {
                out.push((limb << offset) | carry);
                carry = limb >> (64 - offset);
            }
            if carry != 0 {
                out.push(carry);
            }
        }
        Ok(Self::signed(self.negative, out))
    }

    /// self / 2, rounding the magnitude toward zero.
    pub(crate) fn half(&self) -> Result<Self> {
        let mut out = Vec::new();
        reserve(&mut out, self.limbs.len())?;
        for i in 0..self.limbs.len() {
            let high = self.limbs.get(i + 1).copied().unwrap_or(0);
            out.push((self.limbs[i] >> 1) | (high << 63));
        }
        trim(&mut out);
        Ok(Self::signed(self.negative, out))
    }

    pub(crate) fn cmp(&self, other: &Self) -> Ordering {
        match (self.signum(), other.signum()) {
            (a, b) if a != b => a.cmp(&b),
            (0, _) => Ordering::Equal,
            (1, _) => cmp_magnitude(&self.limbs, &other.limbs),
            _ => cmp_magnitude(&other.limbs, &self.limbs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(v: i128) -> Big {
        Big::from_i128(v).unwrap()
    }

    #[test]
    fn signed_arithmetic_matches_i128_including_carries() {
        let values = [
            0i128,
            1,
            -1,
            u64::MAX as i128,
            -(u64::MAX as i128),
            (u64::MAX as i128) * 3 + 7,
            -(1i128 << 100),
            (1i128 << 90) - 12345,
        ];
        for &a in &values {
            for &b in &values {
                assert_eq!(big(a).add(&big(b)).unwrap(), big(a + b), "{a}+{b}");
                assert_eq!(big(a).sub(&big(b)).unwrap(), big(a - b), "{a}-{b}");
                assert_eq!(big(a).cmp(&big(b)), a.cmp(&b));
            }
            for m in [0i64, 1, -1, 3, -7, i64::MAX >> 3] {
                if let Some(p) = a.checked_mul(i128::from(m)) {
                    assert_eq!(big(a).mul_i64(m).unwrap(), big(p));
                }
            }
            if a.unsigned_abs() < (1u128 << 120) {
                assert_eq!(big(a).shl(3).unwrap(), big(a << 3));
            }
            assert_eq!(big(a).half().unwrap(), big(a / 2));
            for &b in &values {
                if let Some(p) = a.checked_mul(b) {
                    assert_eq!(big(a).mul(&big(b)).unwrap(), big(p), "{a}*{b}");
                }
            }
        }
        assert_eq!(big(1).shl(200).unwrap().bits(), 201);
    }
}
