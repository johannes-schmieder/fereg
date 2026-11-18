//! Validate an implicitly defined rational direction by exact signs of
//! integer determinants. This covers categorical designs whose fitted
//! coefficients have denominators too large for short-rational recovery.
use super::{charge, trace_cell_proof, ExactNullspace, Pattern};
use crate::error::Result;
use crate::memory::fallible_filled;
use std::cmp::Ordering;

// Distinct 31-bit primes. Each contributes more than 30 guaranteed bits.
const PRIMES: [u64; 32] = [
    2147483647, 2147483629, 2147483587, 2147483579, 2147483563, 2147483549, 2147483543, 2147483497,
    2147483489, 2147483477, 2147483423, 2147483399, 2147483353, 2147483323, 2147483269, 2147483249,
    2147483237, 2147483179, 2147483171, 2147483137, 2147483123, 2147483077, 2147483069, 2147483059,
    2147483053, 2147483033, 2147483029, 2147482951, 2147482949, 2147482943, 2147482937, 2147482921,
];

#[derive(Clone, Default)]
struct Natural([u32; 32]);

impl Natural {
    fn residue(&self, prime: u64) -> u64 {
        self.0
            .iter()
            .rev()
            .fold(0, |r, &x| ((r << 32) + u64::from(x)) % prime)
    }

    /// If x lies in [0,M), x+M·factor lies in [0,M·prime). The complete
    /// product of the 32 primes has fewer than 992 bits.
    fn add_multiple(&mut self, modulus: &Self, factor: u64) {
        let mut carry = 0;
        for (x, &m) in self.0.iter_mut().zip(&modulus.0) {
            let sum = u64::from(*x) + u64::from(m) * factor + carry;
            *x = sum as u32;
            carry = sum >> 32;
        }
        debug_assert_eq!(carry, 0);
    }

    fn sign(&self, modulus: &Self) -> Ordering {
        if self.0.iter().all(|&x| x == 0) {
            return Ordering::Equal;
        }
        let mut twice = self.clone();
        twice.add_multiple(self, 1);
        if twice.0.iter().rev().cmp(modulus.0.iter().rev()) == Ordering::Greater {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }
}

fn inverse(x: u64, p: u64) -> u64 {
    let (mut value, mut power, mut result) = (x, p - 2, 1);
    while power > 0 {
        if power & 1 != 0 {
            result = result * value % p;
        }
        value = value * value % p;
        power >>= 1;
    }
    result
}

fn add_residue(acc: &mut Natural, residue: u64, modulus: &Natural, reciprocal: u64, prime: u64) {
    let factor = ((residue + prime - acc.residue(prime)) % prime) * reciprocal % prime;
    acc.add_multiple(modulus, factor);
}

fn ceil_log2(x: u64) -> u32 {
    u64::BITS - x.saturating_sub(1).leading_zeros()
}

/// With B the independent square minor and s the desired anchor scores,
/// det(B)·x_i B^-1 s is an integer bordered determinant (up to sign). A
/// Hadamard bound and a sufficiently large CRT modulus uniquely recover
/// its sign, including equality on every retained row.
pub(super) fn certify(
    patterns: &[Pattern],
    width: usize,
    rank: usize,
    mut value: impl FnMut(usize, usize) -> Result<f64>,
    hint: Option<&[f64]>,
    limits: (usize, &mut usize),
) -> Result<bool> {
    let (words, work) = limits;
    let starting_work = *work;
    let cells = patterns.len();
    let (k, m, r) = (width as u128, cells as u128, rank as u128);
    // First the row echelon, flags, raw row and two pivot index lists.
    // Then the restricted design, modular square, 1024-bit residues
    // (including det B), two pivot lists, anchor scores and solution.
    let setup = (k * k + 3 * k + 2 * r).max(m * r + r * r + 16 * (m + 1) + 4 * r);
    if rank == 0 || setup > words as u128 || !charge(work, m * k * (k + 1)) {
        trace_cell_proof(cells, width, rank, "determinant_admission");
        return Ok(false);
    }
    let Some((anchors, columns)) = independent_minor(patterns, width, rank, &mut value)? else {
        trace_cell_proof(cells, width, rank, "determinant_basis");
        return Ok(false);
    };
    let mut design = fallible_filled(
        cells * rank,
        0i64,
        "unable to reserve exact integer cell columns",
    )?;
    let mut maximum = 1u64;
    for (cell, row) in patterns.iter().zip(design.chunks_exact_mut(rank)) {
        for (&j, dest) in columns.iter().zip(row) {
            let x = value(cell.row, j)?;
            if !x.is_finite() || x.abs() > (1u64 << 53) as f64 || x.trunc() != x {
                return Ok(false);
            }
            *dest = x as i64;
            maximum = maximum.max(dest.unsigned_abs());
        }
    }
    let target = anchor_scores(patterns, &anchors, hint)?;
    let target_max = target.iter().map(|x| x.unsigned_abs()).max().unwrap_or(0);
    if target_max == 0 {
        trace_cell_proof(cells, width, rank, "determinant_zero_anchors");
        return Ok(false);
    }
    let twice_log = (rank as u128 + 1) * u128::from(ceil_log2(rank as u64 + 1))
        + 2 * r * u128::from(ceil_log2(maximum))
        + 2 * u128::from(ceil_log2(target_max));
    let twice_log = squared_column_bound(&design, &anchors, &target)
        .map_or(twice_log, |tight| tight.min(twice_log));
    let bits = 1 + twice_log.div_ceil(2);
    let primes_needed = bits.div_ceil(30) as usize;
    if primes_needed > PRIMES.len() {
        trace_cell_proof(cells, width, rank, "determinant_height");
        return Ok(false);
    }
    let mut residues = fallible_filled(
        cells + 1,
        Natural::default(),
        "unable to reserve determinant residues",
    )?;
    let mut modulus = Natural::default();
    modulus.0[0] = 1;
    let mut square = fallible_filled(rank * rank, 0u64, "unable to reserve determinant factor")?;
    let mut solution = fallible_filled(rank, 0u64, "unable to reserve modular anchor solution")?;
    let mut accepted = 0;
    for prime in PRIMES {
        if !charge(work, 4 * r * r + m * (r + 65) + 64 * r) {
            trace_cell_proof(cells, width, rank, "determinant_work");
            return Ok(false);
        }
        for (dest, &i) in square.chunks_exact_mut(rank).zip(&anchors) {
            for (dest, &x) in dest.iter_mut().zip(&design[i * rank..(i + 1) * rank]) {
                *dest = x.rem_euclid(prime as i64) as u64;
            }
        }
        for (v, &t) in solution.iter_mut().zip(&target) {
            *v = t.rem_euclid(prime as i64) as u64;
        }
        let Some(determinant) = solve(&mut square, &mut solution, prime, work)? else {
            continue;
        };
        let reciprocal = inverse(modulus.residue(prime), prime);
        add_residue(
            &mut residues[cells],
            determinant,
            &modulus,
            reciprocal,
            prime,
        );
        for ((acc, row), cell) in residues[..cells]
            .iter_mut()
            .zip(design.chunks_exact(rank))
            .zip(patterns)
        {
            let dot = row.iter().zip(&solution).fold(0u128, |s, (&x, &b)| {
                s + u128::from(x.rem_euclid(prime as i64) as u64) * u128::from(b)
            });
            let score = (dot % u128::from(prime)) as u64 * determinant % prime;
            if !cell.selected && score != 0 {
                trace_cell_proof(cells, width, rank, "determinant_equality");
                return Ok(false);
            }
            add_residue(acc, score, &modulus, reciprocal, prime);
        }
        let old = modulus.clone();
        modulus.add_multiple(&old, prime - 1);
        accepted += 1;
        if accepted == primes_needed {
            let denominator = residues[cells].sign(&modulus);
            if denominator == Ordering::Equal {
                return Ok(false);
            }
            let verified = residues[..cells].iter().zip(patterns).all(|(x, p)| {
                let sign = x.sign(&modulus);
                if p.selected {
                    sign == denominator.reverse()
                } else {
                    sign == Ordering::Equal
                }
            });
            trace_cell_proof(
                cells,
                width,
                rank,
                if verified {
                    "determinant_verified"
                } else {
                    "determinant_sign"
                },
            );
            if crate::diagnostics::solver_trace_enabled() {
                crate::diagnostics::emit_solver_trace(&format!("FEREG_SOLVER_TRACE|phase=separation_determinant_cost|primes={accepted}|work_units={}|workspace_words={setup}",starting_work-*work));
            }
            return Ok(verified);
        }
    }
    Ok(false)
}

fn anchor_scores(
    patterns: &[Pattern],
    anchors: &[usize],
    hint: Option<&[f64]>,
) -> Result<Vec<i64>> {
    let mut target = fallible_filled(
        anchors.len(),
        0i64,
        "unable to reserve integer anchor scores",
    )?;
    let scale = hint.map_or(0., |h| {
        patterns
            .iter()
            .filter(|p| p.selected)
            .map(|p| -h[p.row])
            .fold(0., f64::max)
    });
    for (&i, s) in anchors.iter().zip(&mut target) {
        if patterns[i].selected {
            *s = if let Some(h) = hint.filter(|_| scale > 0. && scale.is_finite()) {
                -((-h[patterns[i].row] / scale * (1u64 << 40) as f64)
                    .round()
                    .clamp(1., (1u64 << 40) as f64) as i64)
            } else {
                -1
            };
        }
    }
    Ok(target)
}

/// Per-column Hadamard bound for every bordered determinant. A border
/// contributes at most the largest square in its column. Sparse dummy
/// columns can reduce the required prime count substantially.
fn squared_column_bound(design: &[i64], anchors: &[usize], target: &[i64]) -> Option<u128> {
    let rank = anchors.len();
    let square = |x: i64| u128::from(x.unsigned_abs()).pow(2);
    let bits = |sum: u128| u128::from(u128::BITS - sum.saturating_sub(1).leading_zeros());
    let target_norm = target
        .iter()
        .try_fold(0u128, |sum, &x| sum.checked_add(square(x)))?;
    let mut height = bits(target_norm);
    for j in 0..rank {
        let border = design.chunks_exact(rank).map(|r| square(r[j])).max()?;
        let norm = anchors.iter().try_fold(border, |sum, &i| {
            sum.checked_add(square(design[i * rank + j]))
        })?;
        height += bits(norm);
    }
    Some(height)
}

fn independent_minor(
    patterns: &[Pattern],
    width: usize,
    rank: usize,
    value: &mut impl FnMut(usize, usize) -> Result<f64>,
) -> Result<Option<(Vec<usize>, Vec<usize>)>> {
    let mut anchors = fallible_filled(rank, 0usize, "unable to reserve integer cell anchors")?;
    let mut columns = fallible_filled(rank, 0usize, "unable to reserve integer cell coordinates")?;
    {
        let mut basis = ExactNullspace::new(width)?;
        let mut used =
            fallible_filled(width, false, "unable to reserve integer cell column flags")?;
        let mut row = fallible_filled(width, 0., "unable to reserve integer cell design row")?;
        for (i, cell) in patterns
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.selected)
            .chain(patterns.iter().enumerate().filter(|(_, p)| p.selected))
        {
            crate::interrupt::checkpoint()?;
            for (j, x) in row.iter_mut().enumerate() {
                *x = value(cell.row, j)?;
            }
            let before = basis.rank();
            basis.insert(&row)?;
            if basis.rank() > before {
                let Some(j) = (0..width).find(|&j| !used[j] && basis.pivot(j)) else {
                    return Ok(None);
                };
                anchors[before] = i;
                columns[before] = j;
                used[j] = true;
                if basis.rank() == rank {
                    break;
                }
            }
        }
        if basis.rank() != rank {
            return Ok(None);
        }
    }
    Ok(Some((anchors, columns)))
}

fn solve(square: &mut [u64], rhs: &mut [u64], p: u64, work: &mut usize) -> Result<Option<u64>> {
    let k = rhs.len();
    let mut determinant = 1;
    for j in 0..k {
        crate::interrupt::checkpoint()?;
        let Some(candidate) = (j..k).find(|&i| square[i * k + j] != 0) else {
            return Ok(None);
        };
        if candidate != j {
            for c in j..k {
                square.swap(j * k + c, candidate * k + c);
            }
            rhs.swap(j, candidate);
            determinant = p - determinant;
        }
        let pivot = square[j * k + j];
        determinant = determinant * pivot % p;
        let reciprocal = inverse(pivot, p);
        for x in &mut square[j * k + j..(j + 1) * k] {
            *x = *x * reciprocal % p;
        }
        rhs[j] = rhs[j] * reciprocal % p;
        for i in j + 1..k {
            let factor = square[i * k + j];
            if factor == 0 {
                continue;
            }
            if !charge(work, (k - j + 2) as u128) {
                *work = 0;
                return Ok(None);
            }
            for c in j + 1..k {
                square[i * k + c] = (square[i * k + c] + p - factor * square[j * k + c] % p) % p;
            }
            rhs[i] = (rhs[i] + p - factor * rhs[j] % p) % p;
        }
    }
    for i in (0..k).rev() {
        for c in i + 1..k {
            rhs[i] = (rhs[i] + p - square[i * k + c] * rhs[c] % p) % p;
        }
    }
    Ok(Some(determinant))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crt_moduli_are_distinct_large_primes() {
        for (i, &p) in PRIMES.iter().enumerate() {
            assert!(p > (1 << 30) && !PRIMES[..i].contains(&p));
            assert!((2..=p.isqrt()).all(|d| p % d != 0));
        }
    }

    #[test]
    fn determinant_signs_resolve_unequal_scores_and_opposite_constraints() {
        let big = (1u64 << 30) as f64 + 3.;
        let patterns: Vec<_> = (0..4)
            .map(|i| Pattern {
                hash: 0,
                row: i,
                selected: i >= 2,
            })
            .collect();
        for sign in [-1., 1.] {
            // |det B| exceeds 2^90 and the last score has twice the
            // magnitude of the preceding one. The first pivot gives a
            // negative anchoring determinant.
            let data = [
                [0., -big, 0.],
                [big, 0., 0.],
                [0., 0., big],
                [big, big, sign * 2. * big],
            ];
            let mut work = 1_000_000;
            assert_eq!(
                certify(
                    &patterns,
                    3,
                    3,
                    |i, j| Ok(data[i][j]),
                    None,
                    (256, &mut work)
                )
                .unwrap(),
                sign > 0.
            );
        }
    }

    #[test]
    fn a_nonzero_retained_score_divisible_by_the_first_prime_is_rejected() {
        let patterns: Vec<_> = (0..3)
            .map(|i| Pattern {
                hash: 0,
                row: i,
                selected: i == 2,
            })
            .collect();
        let data = [[1., 0.], [1., PRIMES[0] as f64], [0., 1.]];
        let mut work = 1_000_000;
        assert!(!certify(
            &patterns,
            2,
            2,
            |i, j| Ok(data[i][j]),
            None,
            (256, &mut work)
        )
        .unwrap());
    }

    #[test]
    fn a_dense_indicator_minor_beyond_i128_replays_strict_signs() {
        // Turning Sylvester H_64 into (H+1)/2 preserves its first all-one
        // row and halves each of the other 63 row differences.
        // Thus |det B| = 64^32 / 2^63 = 2^129.
        let bit = |i: usize, j: usize| f64::from(((i & j).count_ones() % 2 == 0) as u8);
        let patterns: Vec<_> = (0..65)
            .map(|i| Pattern {
                hash: 0,
                row: i,
                selected: i >= 63,
            })
            .collect();
        for scale in [2., -2.] {
            let value = |i, j| {
                Ok(if i == 64 {
                    bit(0, j) + scale * bit(63, j)
                } else {
                    bit(i, j)
                })
            };
            let mut work = 20_000_000;
            assert_eq!(
                certify(&patterns, 64, 64, value, None, (20_000, &mut work)).unwrap(),
                scale > 0.
            );
        }
    }
}
