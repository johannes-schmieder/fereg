//! Outward binary64 intervals. Exact point arithmetic stays a point so
//! cancellation in a genuinely orthogonal design does not manufacture noise.
use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct I {
    pub lo: f64,
    pub hi: f64,
}
impl I {
    pub const ZERO: Self = Self::point(0.);
    pub const ONE: Self = Self::point(1.);
    pub const fn point(x: f64) -> Self {
        Self { lo: x, hi: x }
    }
    fn whole() -> Self {
        Self {
            lo: f64::NEG_INFINITY,
            hi: f64::INFINITY,
        }
    }
    fn hull(a: Self, b: Self) -> Self {
        Self {
            lo: a.lo.min(b.lo),
            hi: a.hi.max(b.hi),
        }
    }
    pub fn sqrt(self) -> Self {
        if self.hi < 0. || !self.hi.is_finite() {
            return Self::whole();
        }
        let low = self.lo.max(0.).sqrt();
        let high = self.hi.sqrt();
        Self {
            lo: if low == 0. { 0. } else { -up(-low) },
            hi: up(high),
        }
    }
    pub fn away(self) -> f64 {
        self.lo.max(-self.hi).max(0.)
    }
    pub fn close(self, value: f64, rel: f64, abs: f64) -> bool {
        self.hi.is_finite()
            && self.lo.is_finite()
            && value.is_finite()
            && (value - self.lo).abs().max((value - self.hi).abs()) <= abs + rel * self.away()
    }
}
fn up(x: f64) -> f64 {
    if x == 0. {
        f64::from_bits(1)
    } else if x > 0. {
        f64::from_bits(x.to_bits() + 1)
    } else {
        f64::from_bits(x.to_bits() - 1)
    }
}
fn around(value: f64, err: f64, tiny_product: bool) -> I {
    if !value.is_finite() || !err.is_finite() {
        return I::whole();
    }
    I {
        lo: if err < 0. || tiny_product {
            -up(-value)
        } else {
            value
        },
        hi: if err > 0. || tiny_product {
            up(value)
        } else {
            value
        },
    }
}
fn add(a: f64, b: f64) -> I {
    let sum = a + b;
    let t = sum - a;
    around(sum, (a - (sum - t)) + (b - t), false)
}
fn prod(a: f64, b: f64) -> I {
    let p = a * b;
    // A residual below the subnormal floor may round to zero in FMA.
    around(
        p,
        a.mul_add(b, -p),
        a != 0. && b != 0. && p.abs() < f64::MIN_POSITIVE * 2_f64.powi(54),
    )
}
fn quot(a: f64, b: f64) -> I {
    let q = a / b;
    around(
        q,
        (-q).mul_add(b, a) * b.signum(),
        a != 0. && (a.abs() < f64::MIN_POSITIVE * 2_f64.powi(54) || q.abs() < f64::MIN_POSITIVE),
    )
}
impl Add for I {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self {
            lo: add(self.lo, b.lo).lo,
            hi: add(self.hi, b.hi).hi,
        }
    }
}
impl Neg for I {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            lo: -self.hi,
            hi: -self.lo,
        }
    }
}
impl Sub for I {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        self + (-b)
    }
}
impl Mul for I {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        let mut c = prod(self.lo, b.lo);
        if self.lo != self.hi {
            c = Self::hull(c, prod(self.hi, b.lo));
        }
        if b.lo != b.hi {
            c = Self::hull(c, prod(self.lo, b.hi));
            if self.lo != self.hi {
                c = Self::hull(c, prod(self.hi, b.hi));
            }
        }
        c
    }
}
impl Div for I {
    type Output = Self;
    fn div(self, b: Self) -> Self {
        if b.away() == 0. {
            return Self::whole();
        }
        let mut c = quot(self.lo, b.lo);
        if self.lo != self.hi {
            c = Self::hull(c, quot(self.hi, b.lo));
        }
        if b.lo != b.hi {
            c = Self::hull(c, quot(self.lo, b.hi));
            if self.lo != self.hi {
                c = Self::hull(c, quot(self.hi, b.hi));
            }
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::I;

    #[test]
    fn exact_binary_arithmetic_and_rounded_endpoints() {
        let dyadic = I::point(0.5) * I::point(0.25) - I::point(0.25) / I::point(2.);
        assert_eq!((dyadic.lo, dyadic.hi), (0., 0.));
        let third = I::ONE / I::point(3.);
        assert_eq!(third.lo, 1. / 3.);
        assert_eq!(third.hi.to_bits(), third.lo.to_bits() + 1);
        let unity = I::point(3.) * third;
        assert!(unity.lo <= 1. && unity.hi >= 1.);
        let zero = I::point(f64::from_bits(1)) * I::point(0.5);
        assert!(zero.lo <= 0. && zero.hi >= f64::from_bits(1));
        assert!(!(I::ONE / I::ZERO).close(0., 1e-7, 1e-10));
    }
}
