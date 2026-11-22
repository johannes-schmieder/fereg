//! Exact replay of a recession ray whose integer coefficients exceed 64 bits.
//!
//! Each raw value is `m 2^e`; a row's score is the exact sum of
//! `values[k] m 2^(e - shift)` and is rounded once, correctly, to binary64.
use std::cmp::Ordering;

use super::RawRows;
use crate::error::Result;
use crate::ppml_separation::{Big, WideRay};

/// `v = m 2^e` with odd `m`, for finite nonzero `v`.
fn dyadic(v: f64) -> (i64, i32) {
    let bits = v.to_bits();
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
    (if bits >> 63 == 1 { -m } else { m }, e)
}

/// `x 2^k` by exact power-of-two steps (rounded only on overflow or
/// underflow). `|x| < 2^65`, so beyond `|k| = 2300` the result is already
/// infinite or zero and `k` saturates there.
fn scale(mut x: f64, k: i64) -> f64 {
    let mut k = k.clamp(-2300, 2300) as i32;
    while k > 1000 {
        x *= 2f64.powi(1000);
        k -= 1000;
    }
    while k < -1000 {
        x *= 2f64.powi(-1000);
        k += 1000;
    }
    x * 2f64.powi(k)
}

/// `value 2^exponent` rounded to nearest binary64: the top 64 bits carry a
/// sticky bit for every discarded nonzero bit, so the one u64 conversion
/// rounds correctly.
fn rounded(value: &Big, exponent: i64) -> f64 {
    let limbs = value.limbs();
    let bits = value.bits();
    if bits == 0 {
        return 0.;
    }
    let drop = bits.saturating_sub(64);
    let (word, offset) = ((drop / 64) as usize, (drop % 64) as u32);
    let mut top = limbs[word] >> offset;
    if offset != 0 && word + 1 < limbs.len() {
        top |= limbs[word + 1] << (64 - offset);
    }
    let sticky = limbs[..word].iter().any(|&l| l != 0)
        || (offset != 0 && limbs[word] & ((1u64 << offset) - 1) != 0);
    let magnitude = scale(
        (top | u64::from(sticky)) as f64,
        exponent.saturating_add(drop as i64),
    );
    if value.is_negative() {
        -magnitude
    } else {
        magnitude
    }
}

/// Visit every raw row's exact sign and rounded score.
pub(super) fn visit(
    model: &super::PreparedPpmlModelGeneration,
    ray: &WideRay,
    mut inspect: impl FnMut(usize, Ordering, f64) -> bool,
) -> Result<bool> {
    let design = RawRows::new(model, true)?;
    if ray.columns.len() != ray.values.len() {
        return Ok(false);
    }
    let mut slot =
        super::fallible_filled(design.width, u32::MAX, "unable to reserve exact ray slots")?;
    for (k, &c) in ray.columns.iter().enumerate() {
        match slot.get_mut(c as usize) {
            Some(s) if *s == u32::MAX => *s = k as u32,
            _ => return Ok(false),
        }
    }
    let mut terms: Vec<(u32, i64, i32)> = Vec::new();
    for r in 0..design.len() {
        crate::interrupt::row_checkpoint(r)?;
        terms.clear();
        let (mut valid, mut failed) = (true, false);
        design.terms(r, |c, x| {
            if !x.is_finite() {
                valid = false;
            } else if x != 0. && slot[c] != u32::MAX {
                if terms.try_reserve(1).is_ok() {
                    let (m, e) = dyadic(x);
                    terms.push((slot[c], m, e));
                } else {
                    failed = true;
                }
            }
        })?;
        if failed {
            return Err(crate::error::allocation(
                "unable to reserve exact ray terms",
            ));
        }
        if !valid {
            return Ok(false);
        }
        let (sign, score) = score(ray, &terms)?;
        if !inspect(r, sign, score) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn score(ray: &WideRay, terms: &[(u32, i64, i32)]) -> Result<(Ordering, f64)> {
    let Some(low) = terms.iter().map(|&(_, _, e)| e).min() else {
        return Ok((Ordering::Equal, 0.));
    };
    let mut sum = Big::zero();
    for &(k, m, e) in terms {
        let term = ray.values[k as usize].mul_i64(m)?.shl((e - low) as u32)?;
        sum = sum.add(&term)?;
    }
    let sign = sum.signum().cmp(&0);
    let exponent = i64::from(low) - i64::from(ray.shift);
    Ok((sign, rounded(&sum, exponent)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_is_correct_with_sticky_bits() {
        let big = |v: i128| Big::from_i128(v).unwrap();
        assert_eq!(rounded(&big(3), -1), 1.5);
        assert_eq!(rounded(&big(-5), 2), -20.);
        // 2^53 + 1 ties to even (2^53); with a sticky bit below it rounds up.
        let tie = big((1i128 << 53) + 1);
        assert_eq!(rounded(&tie, 0), 9007199254740992.);
        let above = big((((1i128 << 53) + 1) << 70) + 1);
        assert_eq!(rounded(&above, -70), 9007199254740994.);
        assert_eq!(rounded(&big(1), -1074), f64::from_bits(1));
    }

    #[test]
    fn scores_stay_exact_for_very_wide_coefficients() {
        // Coefficients 2^(bits-1) and 2^(bits-3) with shift = bits: rows
        // x0 = 1, x1 = 0.3 and x1 = 1 score 0.5, 0.0375 and 0.125 at any width.
        for bits in [64u32, 99_940, 99_990, 100_030, 120_000, 1 << 20] {
            let one = Big::from_i128(1).unwrap();
            let ray = WideRay {
                columns: vec![0, 1],
                values: vec![one.shl(bits - 1).unwrap(), one.shl(bits - 3).unwrap()],
                shift: bits,
            };
            let term = |k: u32, x: f64| {
                let (m, e) = dyadic(x);
                (k, m, e)
            };
            for (terms, want) in [
                (vec![term(0, 1.)], 0.5),
                (vec![term(1, 0.3)], 0.0375),
                (vec![term(1, 1.)], 0.125),
            ] {
                let (sign, got) = score(&ray, &terms).unwrap();
                assert_eq!(sign, Ordering::Greater);
                assert!(
                    (got - want).abs() <= f64::EPSILON * want,
                    "{bits}: {got} vs {want}"
                );
            }
        }
    }
}
