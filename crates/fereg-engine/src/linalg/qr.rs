//! Rectangular, multi-RHS adaptation of legacy finite_wls_kernel's pivoted QR.
//! Local FE blocks and finite WLS use the same factorization primitive.
use super::scaling::{ColumnScaling, ScaledDesign};
use crate::{
    data::{DenseMatrix, Weights},
    error::{reserved, ErrorCode, FeregError, Result},
    execution::{self, Poll},
};

#[derive(Debug)]
pub(crate) struct WeightedQr {
    qr: DenseMatrix,
    permutation: Vec<usize>,
    scaling: ColumnScaling,
    taus: Vec<f64>,
}
impl WeightedQr {
    pub(crate) fn factor(
        matrix: DenseMatrix,
        weights: Weights<'_>,
        threshold: f64,
        poll: Poll<'_>,
    ) -> Result<Self> {
        Self::from_scaled(ScaledDesign::new(matrix, weights, poll)?, threshold, poll)
    }
    pub(crate) fn from_scaled(
        scaled: ScaledDesign,
        threshold: f64,
        poll: Poll<'_>,
    ) -> Result<Self> {
        Self::factor_with_reference(scaled, threshold, 0., poll)
    }
    /// Projected columns keep their original unit reference even if every
    /// surviving column is tiny. Renormalizing the rank test to projection
    /// noise would manufacture identified directions in a nested effect.
    pub(crate) fn from_reference_scaled(
        scaled: ScaledDesign,
        threshold: f64,
        poll: Poll<'_>,
    ) -> Result<Self> {
        Self::factor_with_reference(scaled, threshold, 1., poll)
    }
    fn factor_with_reference(
        scaled: ScaledDesign,
        threshold: f64,
        reference: f64,
        poll: Poll<'_>,
    ) -> Result<Self> {
        execution::check(poll)?;
        if !threshold.is_finite() || threshold <= 0. {
            return Err(invalid("invalid QR rank threshold"));
        }
        let mut permutation = reserved(scaled.matrix.ncols())?;
        permutation.extend(0..scaled.matrix.ncols());
        let mut factor = Self {
            taus: reserved(scaled.matrix.nrows().min(scaled.matrix.ncols()))?,
            qr: scaled.matrix,
            scaling: scaled.scaling,
            permutation,
        };
        factor.factor_scaled(threshold, reference, poll)?;
        Ok(factor)
    }
    fn factor_scaled(&mut self, threshold: f64, reference: f64, poll: Poll<'_>) -> Result<()> {
        let mut initial = reference;
        for column in 0..self.qr.ncols() {
            initial = initial.max(column_norm(&self.qr, column, 0, poll)?);
        }
        for step in 0..self.qr.nrows().min(self.qr.ncols()) {
            execution::check(poll)?;
            let mut pivot = step;
            let mut pivot_norm = column_norm(&self.qr, self.permutation[step], step, poll)?;
            for candidate in step + 1..self.qr.ncols() {
                let norm = column_norm(&self.qr, self.permutation[candidate], step, poll)?;
                let tied = (norm - pivot_norm).abs() <= 64. * f64::EPSILON * norm.max(pivot_norm);
                if norm > pivot_norm && !tied
                    || tied && self.permutation[candidate] < self.permutation[pivot]
                {
                    pivot = candidate;
                    pivot_norm = norm;
                }
            }
            if pivot_norm <= initial * threshold {
                break;
            }
            self.permutation.swap(step, pivot);
            let column = self.permutation[step];
            let first = self.qr[(step, column)];
            let beta = -pivot_norm.copysign(first);
            let denominator = first - beta;
            let tau = (beta - first) / beta;
            self.qr[(step, column)] = beta;
            for row in step + 1..self.qr.nrows() {
                execution::check_chunk(poll, row - step)?;
                self.qr[(row, column)] /= denominator;
            }
            for target in step + 1..self.qr.ncols() {
                let target = self.permutation[target];
                let projection = tau
                    * execution::sum(
                        std::iter::once(self.qr[(step, target)]).chain(
                            (step + 1..self.qr.nrows())
                                .map(|row| self.qr[(row, column)] * self.qr[(row, target)]),
                        ),
                        poll,
                    )?;
                self.qr[(step, target)] -= projection;
                for row in step + 1..self.qr.nrows() {
                    execution::check_chunk(poll, row - step)?;
                    self.qr[(row, target)] -= self.qr[(row, column)] * projection;
                }
            }
            self.taus.push(tau);
        }
        if self.qr.as_slice().iter().any(|v| !v.is_finite()) {
            return Err(numerical("QR factor is not finite"));
        }
        Ok(())
    }
    /// Identified principal-block inverse of X'WX. Dropped coordinates remain
    /// zero; compatible normal RHSs yield the same weighted projection as QR.
    pub(crate) fn solve_normal(&self, rhs: &DenseMatrix, poll: Poll<'_>) -> Result<DenseMatrix> {
        execution::check(poll)?;
        if rhs.nrows() != self.qr.ncols() {
            return Err(invalid("QR normal RHS row count differs"));
        }
        let mut result = DenseMatrix::zeros(rhs.nrows(), rhs.ncols())?;
        for step in 0..self.rank() {
            execution::check(poll)?;
            let basis = self.permutation[step];
            for column in 0..rhs.ncols() {
                let value = self.scaling.normal_coordinate(rhs[(basis, column)], basis);
                let tail = execution::sum(
                    (0..step).map(|previous| {
                        self.qr[(previous, basis)] * result[(self.permutation[previous], column)]
                    }),
                    poll,
                )?;
                result[(basis, column)] = (value - tail) / self.qr[(step, basis)];
            }
        }
        for step in (0..self.rank()).rev() {
            execution::check(poll)?;
            let basis = self.permutation[step];
            for column in 0..rhs.ncols() {
                let tail = execution::sum(
                    (step + 1..self.rank()).map(|next| {
                        self.qr[(step, self.permutation[next])]
                            * result[(self.permutation[next], column)]
                    }),
                    poll,
                )?;
                result[(basis, column)] = (result[(basis, column)] - tail) / self.qr[(step, basis)];
            }
        }
        for &basis in &self.permutation[..self.rank()] {
            for column in 0..rhs.ncols() {
                result[(basis, column)] = self
                    .scaling
                    .normal_coordinate(result[(basis, column)], basis);
            }
        }
        if result.as_slice().iter().any(|value| !value.is_finite()) {
            return Err(numerical("QR normal solve is not representable"));
        }
        Ok(result)
    }
    pub(crate) fn kept(&self) -> &[usize] {
        &self.permutation[..self.rank()]
    }
    pub(crate) fn scaling(&self) -> &ColumnScaling {
        &self.scaling
    }
    pub(crate) fn scaled_inverse(&self, poll: Poll<'_>) -> Result<DenseMatrix> {
        execution::check(poll)?;
        let p = self.qr.ncols();
        let mut inverse = DenseMatrix::zeros(p, p)?;
        for column in 0..self.rank() {
            execution::check(poll)?;
            for row in (0..=column).rev() {
                let tail = execution::sum(
                    (row + 1..=column).map(|next| {
                        self.qr[(row, self.permutation[next])]
                            * inverse[(self.permutation[next], self.permutation[column])]
                    }),
                    poll,
                )?;
                inverse[(self.permutation[row], self.permutation[column])] =
                    ((if row == column { 1. } else { 0. }) - tail)
                        / self.qr[(row, self.permutation[row])];
            }
        }
        let mut bread = DenseMatrix::zeros(p, p)?;
        for &left in self.kept() {
            execution::check(poll)?;
            for &right in self.kept() {
                bread[(left, right)] = execution::sum(
                    self.kept()
                        .iter()
                        .map(|&column| inverse[(left, column)] * inverse[(right, column)]),
                    poll,
                )?;
            }
        }
        Ok(bread)
    }
    pub(crate) fn rank(&self) -> usize {
        self.taus.len()
    }
    pub(crate) fn solve(&self, rhs: DenseMatrix, poll: Poll<'_>) -> Result<DenseMatrix> {
        let (mut rhs, scales) = self.scaling.rhs(rhs, poll)?;
        for (step, &tau) in self.taus.iter().enumerate() {
            execution::check(poll)?;
            let basis = self.permutation[step];
            for column in 0..rhs.ncols() {
                let projection = tau
                    * execution::sum(
                        std::iter::once(rhs[(step, column)]).chain(
                            (step + 1..rhs.nrows())
                                .map(|row| self.qr[(row, basis)] * rhs[(row, column)]),
                        ),
                        poll,
                    )?;
                rhs[(step, column)] -= projection;
                for row in step + 1..rhs.nrows() {
                    execution::check_chunk(poll, row - step)?;
                    rhs[(row, column)] -= self.qr[(row, basis)] * projection;
                }
            }
        }
        let mut result = DenseMatrix::zeros(self.qr.ncols(), rhs.ncols())?;
        for step in (0..self.rank()).rev() {
            execution::check(poll)?;
            let basis = self.permutation[step];
            for column in 0..rhs.ncols() {
                let tail = execution::sum(
                    (step + 1..self.rank()).map(|next| {
                        self.qr[(step, self.permutation[next])]
                            * result[(self.permutation[next], column)]
                    }),
                    poll,
                )?;
                result[(basis, column)] = (rhs[(step, column)] - tail) / self.qr[(step, basis)];
            }
        }
        for &basis in &self.permutation[..self.rank()] {
            for column in 0..rhs.ncols() {
                result[(basis, column)] =
                    self.scaling
                        .coefficient(result[(basis, column)], basis, scales[column]);
            }
        }
        if result.as_slice().iter().any(|v| !v.is_finite()) {
            return Err(numerical("QR coefficients are not representable"));
        }
        Ok(result)
    }
}
fn column_norm(matrix: &DenseMatrix, column: usize, start: usize, poll: Poll<'_>) -> Result<f64> {
    execution::fold(start..matrix.nrows(), 0_f64, poll, |norm, row| {
        norm.hypot(matrix[(row, column)])
    })
}
// Split exponents before rescaling: neither an intermediate overflow nor an
// intermediate underflow may discard an otherwise representable coefficient.
pub(crate) fn scale_ratio(value: f64, numerator: f64, denominator: f64, norm: f64) -> f64 {
    if value == 0. || numerator == 0. {
        return 0.;
    }
    if !value.is_finite() {
        return value;
    }
    let (v, ve) = parts(value);
    let (n, ne) = parts(numerator);
    let (d, de) = parts(denominator);
    let (q, qe) = parts(norm);
    let mut mantissa = v * n / d / q;
    let mut exponent = ve + ne - de - qe;
    while mantissa.abs() >= 2. {
        mantissa *= 0.5;
        exponent += 1;
    }
    while mantissa.abs() < 1. {
        mantissa *= 2.;
        exponent -= 1;
    }
    // At most one multiplication reaches the subnormal range.
    if exponent < -1022 {
        (mantissa * 2f64.powi(exponent + 1022)) * f64::MIN_POSITIVE
    } else if exponent > 1023 {
        (mantissa * 2f64.powi(exponent - 1023)) * 2f64.powi(1023)
    } else {
        mantissa * 2f64.powi(exponent)
    }
}
fn parts(mut value: f64) -> (f64, i32) {
    let mut adjustment = 0;
    if value.abs() < f64::MIN_POSITIVE {
        value *= 2f64.powi(54);
        adjustment = -54;
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023 + adjustment;
    (
        f64::from_bits((bits & ((1u64 << 63) | ((1u64 << 52) - 1))) | (1023u64 << 52)),
        exponent,
    )
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

#[cfg(test)]
#[path = "interrupt_tests.rs"]
mod interrupt_tests;
#[cfg(test)]
mod tests {
    use super::{scale_ratio, ScaledDesign, WeightedQr};
    use crate::data::{DenseMatrix, Weights};
    #[test]
    fn projected_rank_keeps_original_scale_when_every_column_is_small() {
        let scaled = |size: f64| {
            let values = DenseMatrix::from_columns(&[vec![size, -size, size, -size]]).unwrap();
            let mut scaled = ScaledDesign::new(values, Weights::Unit, None).unwrap();
            scaled.reference_scales(&[2.], None).unwrap();
            scaled
        };
        // Relative-only pivoting promotes an all-noise projected bank to rank one.
        assert_eq!(
            WeightedQr::from_scaled(scaled(1e-16), 1e-12, None)
                .unwrap()
                .rank(),
            1
        );
        assert_eq!(
            WeightedQr::from_reference_scaled(scaled(1e-16), 1e-12, None)
                .unwrap()
                .rank(),
            0
        );
        let factor = WeightedQr::from_reference_scaled(scaled(1e-8), 1e-12, None).unwrap();
        assert_eq!(factor.rank(), 1);
        let rhs = DenseMatrix::from_columns(&[vec![1., -1., 1., -1.]]).unwrap();
        assert!((factor.solve(rhs, None).unwrap()[(0, 0)] - 1e8).abs() < 1e-7);
    }
    #[test]
    fn exponent_rescaling_does_not_lose_representable_values() {
        assert_eq!(scale_ratio(1e-200, 1e200, 1e-200, 1.), 1e200);
        assert_eq!(scale_ratio(1e200, 1e200, 1e200, 1.), 1e200);
        assert_eq!(
            scale_ratio(1., f64::from_bits(1), 1., 1.),
            f64::from_bits(1)
        );
        assert_eq!(scale_ratio(-1., f64::MAX, f64::MAX, 1.), -1.);
        assert_eq!(scale_ratio(2., f64::MAX, 1., 1.), f64::INFINITY);
    }
}
