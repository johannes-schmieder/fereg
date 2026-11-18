//! A two-double expansion for a bounded independent guard. Inputs and outputs
//! remain ordinary doubles. FMA supplies the exact product's rounding tail.
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Wide(f64, f64);

impl Wide {
    pub(super) const ZERO: Self = Self(0.0, 0.0);
    pub(super) const ONE: Self = Self(1.0, 0.0);

    fn sum(a: f64, b: f64) -> Self {
        let s = a + b;
        let back = s - a;
        Self(s, (a - (s - back)) + (b - back))
    }

    pub(super) fn value(self) -> f64 {
        self.0 + self.1
    }

    pub(super) fn exp(self) -> Option<Self> {
        // Cody-Waite reduction. Both pieces of log(2) are required here
        // because the starting binary64 mean is precisely what is audited.
        const LN_2: Wide = Wide(std::f64::consts::LN_2, 2.3190468138462996e-17);
        let n = (self.0 * std::f64::consts::LOG2_E).round();
        if !(-1022.0..=1023.0).contains(&n) {
            return None;
        }
        let r = self - LN_2 * Self::from(n);
        let mut term = Self::ONE;
        let mut sum = Self::ONE;
        for j in 1..=32 {
            term = (term * r) / Self::from(j as f64);
            sum += term;
            if term.0.abs() < 1e-35 {
                break;
            }
        }
        let res = sum * Self::from(2f64.powi(n as i32));
        (res.0.is_finite() && res.0 > 0.0).then_some(res)
    }
}
impl From<f64> for Wide {
    fn from(value: f64) -> Self {
        Self(value, 0.0)
    }
}
impl Add for Wide {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        let h = Self::sum(self.0, rhs.0);
        let l = Self::sum(self.1, rhs.1);
        let v = Self::sum(h.0, h.1 + l.0);
        Self::sum(v.0, v.1 + l.1)
    }
}
impl Sub for Wide {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self + Self(-rhs.0, -rhs.1)
    }
}
impl AddAssign for Wide {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for Wide {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl Mul for Wide {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        let p = self.0 * rhs.0;
        let e = self.0.mul_add(rhs.0, -p);
        Self::sum(p, e + (self.0 * rhs.1 + self.1 * rhs.0))
    }
}
impl Div for Wide {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let q = Self::from(self.0 / rhs.0);
        let r = self - q * rhs;
        let d = Self::from(r.0 / rhs.0);
        let tail = r - d * rhs;
        q + (d + Self::from(tail.0 / rhs.0))
    }
}

#[cfg(test)]
mod tests {
    use super::Wide;
    #[test]
    fn cancellation_and_reduced_exponential_retain_the_low_component() {
        let tiny = Wide::from(2f64.powi(-85));
        assert_eq!((Wide::ONE + tiny) - Wide::ONE, tiny);
        let log2 = Wide(std::f64::consts::LN_2, 2.3190468138462996e-17);
        assert_eq!(log2.exp(), Some(Wide::from(2.)));
        assert!(
            ((log2 + tiny).exp().unwrap() - Wide::from(2.) - tiny * Wide::from(2.))
                .value()
                .abs()
                < 1e-30
        );
    }
}
