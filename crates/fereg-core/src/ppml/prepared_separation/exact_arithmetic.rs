//! Generate small rational null-vector candidates over one prime and check
//! their scores over the actual dyadic design. Only the latter check proves
//! a coefficient vector satisfies an equality or sign constraint.

use std::cmp::Ordering;

use crate::error::{allocation, invalid_argument, Result};
use crate::memory::fallible_filled;
use crate::ppml::modular_rank::{insert, residue, PRIME};

/// The caller admits width² + width eight-byte words for the persistent
/// echelon and work row, plus three width-sized vectors during reconstruction.
pub(crate) struct ExactNullspace {
    echelon: Vec<u64>,
    work: Vec<u64>,
    rank: usize,
}

impl ExactNullspace {
    pub(crate) fn new(width: usize) -> Result<Self> {
        let cells = width
            .checked_mul(width)
            .ok_or_else(|| allocation("exact separation echelon dimensions overflow"))?;
        Ok(Self {
            echelon: fallible_filled(cells, 0, "unable to reserve exact separation echelon")?,
            work: fallible_filled(width, 0, "unable to reserve exact separation work row")?,
            rank: 0,
        })
    }

    pub(crate) fn insert(&mut self, row: &[f64]) -> Result<()> {
        let width = self.work.len();
        if row.len() != width {
            return Err(invalid_argument("misaligned exact separation row"));
        }
        if self.rank == width {
            return Ok(());
        }
        for (column, (&value, word)) in row.iter().zip(&mut self.work).enumerate() {
            crate::interrupt::row_checkpoint(column)?;
            if !value.is_finite() {
                return Err(invalid_argument("nonfinite exact separation design value"));
            }
            *word = residue(value);
        }
        self.rank += usize::from(insert(&mut self.echelon, &mut self.work));
        Ok(())
    }

    pub(crate) fn pivot(&self, column: usize) -> bool {
        self.echelon[column * self.work.len() + column] != 0
    }

    pub(crate) fn rank(&self) -> usize {
        self.rank
    }

    pub(super) fn insert_integers(&mut self, row: &[i64]) -> Result<()> {
        if row.len() != self.work.len() {
            return Err(invalid_argument("misaligned exact integer constraint"));
        }
        if self.rank < row.len() {
            for (&coefficient, dest) in row.iter().zip(&mut self.work) {
                *dest = coefficient.rem_euclid(PRIME as i64) as u64;
            }
            self.rank += usize::from(insert(&mut self.echelon, &mut self.work));
        }
        Ok(())
    }

    /// Store each proposed integer relation in its *vacant* echelon row. The
    /// diagonal must stay zero until all reconstructions finish; save the
    /// positive free coefficient separately in the old modular work vector.
    pub(super) fn into_relations(mut self) -> Result<Option<CellRelations>> {
        let width = self.work.len();
        self.work.fill(0);
        let mut free = fallible_filled(width, 0i64, "unable to reserve free cell coordinates")?;
        for i in 0..width {
            if self.pivot(i) {
                continue;
            }
            free[i] = 1;
            let Some(relation) = self.reconstruct(&free)? else {
                return Ok(None);
            };
            free[i] = 0;
            let stored = &mut self.echelon[i * width..(i + 1) * width];
            for (dest, &value) in stored.iter_mut().zip(&relation) {
                *dest = value as u64;
            }
            self.work[i] = stored[i];
            stored[i] = 0;
        }
        Ok(Some(CellRelations {
            bank: self.echelon,
            diagonal: self.work,
        }))
    }

    /// The free coordinates are supplied in original column order. Their
    /// relative signs and ratios survive both primitive normalization and
    /// clearing the positive common denominator. A bad prime or an inaccurate
    /// reconstruction is harmless only after checking every raw row's score.
    pub(crate) fn reconstruct(&self, free_values: &[i64]) -> Result<Option<Vec<i64>>> {
        let width = self.work.len();
        if free_values.len() != width {
            return Err(invalid_argument("misaligned separation free coordinates"));
        }
        let divisor = (0..width)
            .filter(|&column| !self.pivot(column))
            .map(|column| free_values[column].unsigned_abs())
            .fold(0, gcd);
        if divisor == 0 {
            return Ok(None);
        }
        let free = |column| (i128::from(free_values[column]) / i128::from(divisor)) as i64;
        let mut field = fallible_filled(width, 0u64, "unable to reserve modular solution")?;
        for column in (0..width).rev() {
            crate::interrupt::checkpoint()?;
            if self.pivot(column) {
                let pivot = &self.echelon[column * width..(column + 1) * width];
                // Fewer than 2^64 terms, each below 2^62.
                let sum: u128 = pivot[column + 1..]
                    .iter()
                    .zip(&field[column + 1..])
                    .map(|(&a, &b)| u128::from(a) * u128::from(b))
                    .sum();
                field[column] = (PRIME - (sum % u128::from(PRIME)) as u64) % PRIME;
            } else {
                field[column] = free(column).rem_euclid(PRIME as i64) as u64;
            }
        }
        let mut numerator = fallible_filled(width, 0i64, "unable to reserve rational numerators")?;
        let mut denominator =
            fallible_filled(width, 1i64, "unable to reserve rational denominators")?;
        let mut common = 1i64;
        for column in 0..width {
            if self.pivot(column) {
                let (n, d) = rational_candidate(field[column]);
                numerator[column] = n;
                denominator[column] = d;
                let g = gcd(common as u64, d as u64) as i64;
                let Some(next) = (common / g).checked_mul(d) else {
                    return Ok(None);
                };
                common = next;
            } else {
                numerator[column] = free(column);
            }
        }
        for (n, d) in numerator.iter_mut().zip(denominator) {
            let Some(scaled) = n.checked_mul(common / d) else {
                return Ok(None);
            };
            *n = scaled;
        }
        Ok(Some(numerator))
    }
}

pub(super) struct CellRelations {
    bank: Vec<u64>,
    diagonal: Vec<u64>,
}

impl CellRelations {
    /// Verify or consume one signed relation at a time. Each has its own
    /// nonzero free coordinate and is zero on all the other free coordinates.
    pub(super) fn all(
        &self,
        scratch: &mut [i64],
        mut accept: impl FnMut(&[i64]) -> Result<bool>,
    ) -> Result<bool> {
        let width = self.diagonal.len();
        for (i, &diagonal) in self.diagonal.iter().enumerate() {
            if diagonal == 0 {
                continue;
            }
            crate::interrupt::checkpoint()?;
            for (value, &stored) in scratch
                .iter_mut()
                .zip(&self.bank[i * width..(i + 1) * width])
            {
                *value = stored as i64;
            }
            scratch[i] = diagonal as i64;
            if !accept(scratch)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Euclidean reconstruction in the symmetric uniqueness interval
/// 2·|numerator|·denominator < PRIME. Larger integral solutions are also
/// worth trying, since the caller checks the candidate independently.
fn rational_candidate(word: u64) -> (i64, i64) {
    const LIMIT: i64 = 32_767; // floor(sqrt(PRIME / 2))
    let (mut previous, mut current) = (PRIME as i64, word as i64);
    let (mut previous_factor, mut factor) = (0i64, 1i64);
    while current > LIMIT {
        let quotient = previous / current;
        (previous, current) = (current, previous % current);
        (previous_factor, factor) = (factor, previous_factor - quotient * factor);
    }
    if factor.abs() <= LIMIT {
        return (current * factor.signum(), factor.abs());
    }
    let integer = if word > PRIME / 2 {
        word as i64 - PRIME as i64
    } else {
        word as i64
    };
    (integer, 1)
}

/// The largest |binary64 · i64| is below 2^1087. Even a 64-bit usize
/// worth of terms stays below 2^1151. Expressed in units of 2^-1074
/// this needs at most 2225 bits for either unsigned subtotal.
const LIMBS: usize = 35;

/// Sign-only interface for a caller that does not publish score magnitudes.
pub(crate) fn exact_dot(row: &[f64], coefficients: &[i64]) -> Option<Ordering> {
    exact_score(row, coefficients).map(|(sign, _)| sign)
}

/// The exact sign and the score rounded once to binary64, including
/// cancellation after intermediate floating overflow or loss of tiny terms.
/// An unrepresentably large score rounds to signed infinity; its sign still
/// holds. Products use the original mantissa and integer coefficient.
pub(super) fn exact_score(row: &[f64], coefficients: &[i64]) -> Option<(Ordering, f64)> {
    if row.len() != coefficients.len() {
        return None;
    }
    let mut positive = [0u64; LIMBS];
    let mut negative = [0u64; LIMBS];
    for (&value, &coefficient) in row.iter().zip(coefficients) {
        if !value.is_finite() {
            return None;
        }
        if coefficient == 0 || value == 0. {
            continue;
        }
        let bits = value.to_bits();
        let exponent = (bits >> 52) as usize & 0x7ff;
        let fraction = bits & ((1u64 << 52) - 1);
        let mantissa = if exponent == 0 {
            fraction
        } else {
            fraction | (1u64 << 52)
        };
        let product = u128::from(mantissa) * u128::from(coefficient.unsigned_abs());
        let total = if value.is_sign_negative() == coefficient.is_negative() {
            &mut positive
        } else {
            &mut negative
        };
        let shift = exponent.saturating_sub(1);
        let limb = shift / 64;
        let offset = shift % 64;
        let low = product << offset;
        add_word(total, limb, low as u64);
        add_word(total, limb + 1, (low >> 64) as u64);
        if offset != 0 {
            add_word(total, limb + 2, (product >> (128 - offset)) as u64);
        }
    }
    let sign = positive.iter().rev().cmp(negative.iter().rev());
    let value = match sign {
        Ordering::Equal => 0.,
        Ordering::Greater => rounded_difference(&mut positive, &negative),
        Ordering::Less => -rounded_difference(&mut negative, &positive),
    };
    Some((sign, value))
}

fn add_word(total: &mut [u64; LIMBS], mut index: usize, mut addend: u64) {
    while addend != 0 {
        let (sum, carry) = total[index].overflowing_add(addend);
        total[index] = sum;
        addend = u64::from(carry);
        index += 1;
    }
}

/// Subtract unsigned totals and round the resulting positive integer in
/// units of 2^-1074. The first discarded bit and all lower bits decide ties.
fn rounded_difference(big: &mut [u64; LIMBS], small: &[u64; LIMBS]) -> f64 {
    let mut borrow = false;
    for (big, &small) in big.iter_mut().zip(small) {
        let (part, first) = big.overflowing_sub(small);
        let (result, second) = part.overflowing_sub(u64::from(borrow));
        *big = result;
        borrow = first || second;
    }
    let last = big
        .iter()
        .rposition(|&word| word != 0)
        .expect("strict difference");
    let high = last * 64 + 63 - big[last].leading_zeros() as usize;
    if high <= 52 {
        return f64::from_bits(big[0]);
    }
    if high >= 2098 {
        return f64::INFINITY;
    }
    let discarded = high - 52;
    let (index, offset) = (discarded / 64, discarded % 64);
    let mut significant = big[index] >> offset;
    if offset != 0 {
        significant |= big[index + 1] << (64 - offset);
    }
    let guard_index = (discarded - 1) / 64;
    let guard = 1u64 << ((discarded - 1) % 64);
    let sticky =
        (big[guard_index] & (guard - 1)) != 0 || big[..guard_index].iter().any(|&word| word != 0);
    let up = big[guard_index] & guard != 0 && (sticky || significant & 1 != 0);
    // Carry into the exponent naturally handles a power-of-two boundary
    // and the tie between the maximum finite and the overflowing result.
    f64::from_bits(((discarded as u64) << 52) + significant + u64::from(up))
}

#[cfg(test)]
mod tests;
