//! Floating-point helpers whose results do not depend on the platform's math library.
//!
//! The C library's `hypot` is not correctly rounded and differs between
//! platforms in the last bit for a sizeable share of arguments. A recurrence
//! or a factorization that accumulates it (Givens rotations, running norms)
//! then follows a platform-dependent path, and a discrete decision taken on
//! those values (a stopping test, an iteration count, a pivot) can differ
//! between platforms. [`hypot`] is built from IEEE 754 operations only, so it
//! returns the same bits on every platform this crate supports.

/// `√(x² + y²)` without undue overflow or underflow, with the same bits on every platform.
///
/// Built only from IEEE 754 binary64 operations in round-to-nearest: exact
/// power-of-two scaling, additions, multiplications, one division and the
/// correctly rounded square root. It calls no math library and needs no fused
/// multiply-add (Rust never contracts `a * b + c` into one), so the result
/// does not depend on the platform, the processor's features or the
/// optimization level.
///
/// - **Accuracy.** A normal result is the correctly rounded `√(x² + y²)`
///   unless the exact value lies within a relative distance of about `2⁻¹⁰⁰`
///   of a rounding boundary, where it may be a neighbour of it; a subnormal
///   result is rounded a second time on the subnormal grid. Either way it is
///   never more than one ulp from correct rounding, which the unit tests
///   check by exact integer arithmetic on random and adversarial pairs.
///   Results that overflow are `+∞`.
/// - **Special values.** Symmetric in its arguments and their signs.
///   `hypot(±∞, y)` is `+∞` even for a NaN `y`; otherwise a NaN argument
///   gives the canonical NaN; `hypot(x, ±0)` is `|x|`.
/// - **Method.** After exact scaling of the larger magnitude `a` into `[1, 2)`,
///   `a² + b²` is formed to about twice the working precision as a
///   double-length sum (Dekker's exact products with Veltkamp's splitting),
///   its rounded square root `r` is
///   corrected once by `(a² + b² − r²) / 2r` with `r²` formed exactly too,
///   and the result is scaled back. When `b ≤ 2⁻²⁸ a` the correctly rounded
///   result is `a` and is returned directly.
#[must_use]
pub fn hypot(x: f64, y: f64) -> f64 {
    let (x, y) = (x.abs(), y.abs());
    if x == f64::INFINITY || y == f64::INFINITY {
        return f64::INFINITY;
    }
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    let (mut a, mut b) = if x < y { (y, x) } else { (x, y) };
    if b == 0.0 {
        return a;
    }
    // Both subnormal: lift them exactly into the normal range; the result is
    // scaled back by one multiplication at the end.
    let mut unscale = 0;
    if a < f64::MIN_POSITIVE {
        a *= pow2(SUBNORMAL_LIFT);
        b *= pow2(SUBNORMAL_LIFT);
        unscale = -SUBNORMAL_LIFT;
    }
    let ea = biased_exponent(a) - EXPONENT_BIAS;
    // A subnormal `b` reads as 2^-1023, at least its own magnitude, so the
    // gap below can only be underestimated, which keeps the shortcut sound.
    let eb = biased_exponent(b) - EXPONENT_BIAS;
    if ea - eb > NEGLIGIBLE_GAP {
        // b < 2^(eb+1) ≤ 2^-28 a, so √(a² + b²) < a (1 + 2^-57): `a` is the
        // correctly rounded result.
        return a * pow2(unscale);
    }
    // Exact: `a · 2^-ea ∈ [1, 2)`, and `b · 2^-ea` stays normal (at least
    // 2^-28, or 2^-79 for a subnormal `b`).
    let down = pow2(-ea);
    let r = scaled_hypot(a * down, b * down);
    // `r · 2^ea` is exact unless it overflows; a lifted subnormal pair rounds
    // once more here.
    r * pow2(ea) * pow2(unscale)
}

/// `b ≤ 2^-28 a` once the binary exponents of `a` and `b` differ by more than this.
const NEGLIGIBLE_GAP: i32 = 28;

/// Lifts a subnormal pair into the normal range without reaching overflow.
const SUBNORMAL_LIFT: i32 = 54;

const EXPONENT_BIAS: i32 = 1023;

/// Veltkamp's splitting constant `2^27 + 1` for binary64.
const SPLITTER: f64 = 134_217_729.0;

/// `√(a² + b²)` for `1 ≤ a < 2` and `2^-80 ≤ b ≤ a`, within `≈ 2^-103 a` before the final rounding.
fn scaled_hypot(a: f64, b: f64) -> f64 {
    let (a2, a2_error) = exact_square(a);
    let (b2, b2_error) = exact_square(b);
    let sum = a2 + b2;
    // `a2 ≥ b2`, so `(a2 − sum) + b2` is the sum's rounding error exactly.
    let tail = ((a2 - sum) + b2) + (a2_error + b2_error);
    let root = sum.sqrt();
    let (root2, root2_error) = exact_square(root);
    // `root²` is within a factor of two of `sum`, so `sum − root2` is exact.
    let residual = ((sum - root2) - root2_error) + tail;
    root + residual / (2.0 * root)
}

/// `(p, e)` with `p = fl(x²)` and `p + e = x²` exactly (Dekker), for `x` far from over- and underflow.
fn exact_square(x: f64) -> (f64, f64) {
    let product = x * x;
    let scaled = SPLITTER * x;
    let high = scaled - (scaled - x);
    let low = x - high;
    let error = (((high * high - product) + high * low) + high * low) + low * low;
    (product, error)
}

fn biased_exponent(x: f64) -> i32 {
    ((x.to_bits() >> 52) & 0x7ff) as i32
}

/// `2^e` for `-1074 ≤ e ≤ 1023`, from its bits (subnormal powers included).
fn pow2(e: i32) -> f64 {
    debug_assert!((-1074..=1023).contains(&e));
    if e >= -1022 {
        f64::from_bits(((e + EXPONENT_BIAS) as u64) << 52)
    } else {
        f64::from_bits(1_u64 << (e + 1074))
    }
}

#[cfg(test)]
mod tests;
