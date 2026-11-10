//! Norms as `m · 2^e`, for comparing gradients of problems whose scales share no range.
//
// Copyright (c) 2026 within contributors
// Copyright (c) 2026 appliedAI Institute for Europe gGmbH
// MIT License; see the notice in `crates/fedemean/src/lsmr.rs`.

use std::ops::{Div, Mul};

/// A nonnegative `m · 2^e`, `m ∈ [1, 2)`, zero or non-finite, for expressions a few factors deep.
#[derive(Clone, Copy)]
pub(super) struct Magnitude {
    m: f64,
    e: i32,
}

impl Magnitude {
    /// `a · b`, exact in exponent where the `f64` product would over- or underflow.
    pub(super) fn product(a: f64, b: f64) -> Self {
        Self::from(a) * Self::from(b)
    }

    /// The nearest `f64`, saturating to 0 or ∞; a subnormal result rounds a second time.
    pub(super) fn to_f64(self) -> f64 {
        if !self.m.is_normal() {
            return self.m;
        }
        ldexp(self.m, self.e)
    }

    /// Neither zero, infinite nor NaN.
    pub(super) fn is_normal(self) -> bool {
        self.m.is_normal()
    }

    fn normalized(m: f64, e: i32) -> Self {
        if !m.is_normal() {
            return Self { m, e: 0 };
        }
        let Self { m, e: shift } = Self::from(m);
        Self { m, e: e + shift }
    }
}

impl From<f64> for Magnitude {
    fn from(x: f64) -> Self {
        if !x.is_finite() || x == 0.0 {
            return Self { m: x, e: 0 };
        }
        // A subnormal carries its leading bit below the mantissa field; 2^64 lifts it exactly.
        let (x, bias) = if x.is_normal() {
            (x, 0)
        } else {
            (x * pow2(64), -64)
        };
        debug_assert!(x > 0.0, "a norm, got {x:e}");
        let bits = x.to_bits();
        let e = ((bits >> 52) & 0x7ff) as i32 - 1023;
        let m = f64::from_bits((bits & ((1 << 52) - 1)) | (1023 << 52));
        Self { m, e: e + bias }
    }
}

impl Mul for Magnitude {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::normalized(self.m * rhs.m, self.e + rhs.e)
    }
}

impl Div for Magnitude {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self::normalized(self.m / rhs.m, self.e - rhs.e)
    }
}

/// `⌊log₂ x⌋` of a positive finite `x`, subnormal included.
pub(super) fn exponent(x: f64) -> i32 {
    Magnitude::from(x).e
}

/// `x · 2^k`, saturating; in two steps since `2^k` alone may not be a double.
pub(super) fn ldexp(x: f64, k: i32) -> f64 {
    let k = k.clamp(-2044, 2044);
    x * pow2(k / 2) * pow2(k - k / 2)
}

/// `2^e` for a normal exponent, built from its bits since `powi`'s precision is unspecified.
fn pow2(e: i32) -> f64 {
    debug_assert!((f64::MIN_EXP - 1..f64::MAX_EXP).contains(&e));
    f64::from_bits(((e + 1023) as u64) << 52)
}

#[cfg(test)]
mod tests {
    use super::Magnitude;
    #[test]
    fn a_float_round_trips() {
        a_float_round_trips_case(1.0); // one
        a_float_round_trips_case(3.7e300); // large
        a_float_round_trips_case(f64::MIN_POSITIVE); // smallest_normal
        a_float_round_trips_case(1.3e-310); // subnormal
        a_float_round_trips_case(f64::from_bits(1)); // smallest_subnormal
        a_float_round_trips_case(f64::MAX); // largest
        a_float_round_trips_case(0.0); // zero
        a_float_round_trips_case(f64::INFINITY); // infinity
    }

    fn a_float_round_trips_case(x: f64) {
        assert_eq!(Magnitude::from(x).to_f64().to_bits(), x.to_bits());
    }

    /// Where the `f64` arithmetic stays in range the two agree bitwise.
    #[test]
    fn in_range_arithmetic_is_the_float_arithmetic() {
        in_range_arithmetic_is_the_float_arithmetic_case(3.0, 7.0, 5.0); // unit
        in_range_arithmetic_is_the_float_arithmetic_case(3e150, 7e150, 5e-7); // large
        in_range_arithmetic_is_the_float_arithmetic_case(3e-150, 7e-150, 5e7); // small
    }

    fn in_range_arithmetic_is_the_float_arithmetic_case(a: f64, b: f64, c: f64) {
        let exact = Magnitude::product(a, b) / Magnitude::from(c);
        assert_eq!(exact.to_f64().to_bits(), (a * b / c).to_bits());
    }

    /// A quotient back in range survives a product that is not.
    #[test]
    fn out_of_range_products_divide_back() {
        out_of_range_products_divide_back_case(1e300, 3e300, 1e300, 3e300); // overflowing_product
        out_of_range_products_divide_back_case(1e-300, 3e-300, 1e-300, 3e-300); // underflowing_product
        out_of_range_products_divide_back_case(1e-300, 3e-10, 1.0, 3e-310); // subnormal_quotient
    }

    fn out_of_range_products_divide_back_case(a: f64, b: f64, c: f64, expected: f64) {
        let got = (Magnitude::product(a, b) / Magnitude::from(c)).to_f64();
        assert!(
            (got / expected - 1.0).abs() < 1e-9,
            "{got:e} vs {expected:e}"
        );
    }

    #[test]
    fn an_out_of_range_value_saturates() {
        assert!(Magnitude::product(1e-300, 1e-300).is_normal());
        assert_eq!(Magnitude::product(1e-300, 1e-300).to_f64(), 0.0);
        assert_eq!(Magnitude::product(1e300, 1e300).to_f64(), f64::INFINITY);
        assert!(Magnitude::from(f64::NAN).to_f64().is_nan());
    }
}
