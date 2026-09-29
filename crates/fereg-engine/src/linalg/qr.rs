//! Rectangular, multi-RHS adaptation of legacy finite_wls_kernel's pivoted QR.
//! Local FE blocks and finite WLS use the same factorization primitive.
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    threads::compensated_sum,
};

#[derive(Debug)]
pub(crate) struct WeightedQr {
    qr: DenseMatrix,
    permutation: Vec<usize>,
    maxima: Vec<f64>,
    norms: Vec<f64>,
    roots: Vec<f64>,
    root_scale: f64,
    taus: Vec<f64>,
}
impl WeightedQr {
    pub(crate) fn factor(mut matrix: DenseMatrix, weights: &[f64], threshold: f64) -> Result<Self> {
        if weights.len() != matrix.nrows() || !threshold.is_finite() || threshold <= 0. {
            return Err(invalid("QR weights or rank threshold are invalid"));
        }
        if weights.iter().any(|w| !w.is_finite() || *w <= 0.)
            || matrix.as_slice().iter().any(|v| !v.is_finite())
        {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "nonfinite QR input or invalid weight",
            ));
        }
        // A common weight scale preserves the objective and prevents root-weight overflow.
        let largest_root = weights.iter().copied().fold(0., f64::max).sqrt();
        let mut roots = reserved(weights.len())?;
        roots.extend(weights.iter().map(|w| w.sqrt() / largest_root));
        let mut maxima = reserved(matrix.ncols())?;
        let mut norms = reserved(matrix.ncols())?;
        for column in 0..matrix.ncols() {
            let maximum = (0..matrix.nrows())
                .map(|r| matrix[(r, column)].abs())
                .fold(0., f64::max);
            maxima.push(maximum);
            for row in 0..matrix.nrows() {
                let original = matrix[(row, column)];
                let value = if maximum == 0. {
                    0.
                } else {
                    original / maximum * roots[row]
                };
                if original != 0. && value == 0. {
                    return Err(numerical("weighted QR column scale underflow"));
                }
                matrix[(row, column)] = value;
            }
            let norm = column_norm(&matrix, column, 0);
            norms.push(norm);
            if norm != 0. {
                for row in 0..matrix.nrows() {
                    matrix[(row, column)] /= norm;
                }
            }
        }
        let mut permutation = reserved(matrix.ncols())?;
        permutation.extend(0..matrix.ncols());
        let mut factor = Self {
            taus: reserved(matrix.nrows().min(matrix.ncols()))?,
            qr: matrix,
            permutation,
            maxima,
            norms,
            roots,
            root_scale: largest_root,
        };
        factor.factor_scaled(threshold)?;
        Ok(factor)
    }
    fn factor_scaled(&mut self, threshold: f64) -> Result<()> {
        let initial = (0..self.qr.ncols())
            .map(|c| column_norm(&self.qr, c, 0))
            .fold(0., f64::max);
        for step in 0..self.qr.nrows().min(self.qr.ncols()) {
            let mut pivot = step;
            let mut pivot_norm = column_norm(&self.qr, self.permutation[step], step);
            for candidate in step + 1..self.qr.ncols() {
                let norm = column_norm(&self.qr, self.permutation[candidate], step);
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
                self.qr[(row, column)] /= denominator;
            }
            for target in step + 1..self.qr.ncols() {
                let target = self.permutation[target];
                let projection = tau
                    * compensated_sum(
                        std::iter::once(self.qr[(step, target)]).chain(
                            (step + 1..self.qr.nrows())
                                .map(|row| self.qr[(row, column)] * self.qr[(row, target)]),
                        ),
                    );
                self.qr[(step, target)] -= projection;
                for row in step + 1..self.qr.nrows() {
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
    pub(crate) fn solve_normal(&self, rhs: &DenseMatrix) -> Result<DenseMatrix> {
        if rhs.nrows() != self.qr.ncols() {
            return Err(invalid("QR normal RHS row count differs"));
        }
        let mut result = DenseMatrix::zeros(rhs.nrows(), rhs.ncols())?;
        for step in 0..self.rank() {
            let basis = self.permutation[step];
            for column in 0..rhs.ncols() {
                let value = scale_ratio(
                    rhs[(basis, column)],
                    1.,
                    self.maxima[basis],
                    self.norms[basis],
                );
                let value = scale_ratio(value, 1., self.root_scale, 1.);
                let tail = compensated_sum((0..step).map(|previous| {
                    self.qr[(previous, basis)] * result[(self.permutation[previous], column)]
                }));
                result[(basis, column)] = (value - tail) / self.qr[(step, basis)];
            }
        }
        for step in (0..self.rank()).rev() {
            let basis = self.permutation[step];
            for column in 0..rhs.ncols() {
                let tail = compensated_sum((step + 1..self.rank()).map(|next| {
                    self.qr[(step, self.permutation[next])]
                        * result[(self.permutation[next], column)]
                }));
                result[(basis, column)] = (result[(basis, column)] - tail) / self.qr[(step, basis)];
            }
        }
        for &basis in &self.permutation[..self.rank()] {
            for column in 0..rhs.ncols() {
                let value = scale_ratio(
                    result[(basis, column)],
                    1.,
                    self.maxima[basis],
                    self.norms[basis],
                );
                result[(basis, column)] = scale_ratio(value, 1., self.root_scale, 1.);
            }
        }
        if result.as_slice().iter().any(|value| !value.is_finite()) {
            return Err(numerical("QR normal solve is not representable"));
        }
        Ok(result)
    }
    pub(crate) fn rank(&self) -> usize {
        self.taus.len()
    }
    pub(crate) fn solve(&self, mut rhs: DenseMatrix) -> Result<DenseMatrix> {
        if rhs.nrows() != self.qr.nrows() {
            return Err(invalid("QR RHS row count differs"));
        }
        if rhs.as_slice().iter().any(|v| !v.is_finite()) {
            return Err(FeregError::new(ErrorCode::InvalidData, "nonfinite QR RHS"));
        }
        let mut scales = reserved(rhs.ncols())?;
        for column in 0..rhs.ncols() {
            let scale = (0..rhs.nrows())
                .map(|r| rhs[(r, column)].abs())
                .fold(0., f64::max);
            scales.push(scale);
            for row in 0..rhs.nrows() {
                rhs[(row, column)] = if scale == 0. {
                    0.
                } else {
                    rhs[(row, column)] / scale * self.roots[row]
                };
            }
        }
        for (step, &tau) in self.taus.iter().enumerate() {
            let basis = self.permutation[step];
            for column in 0..rhs.ncols() {
                let projection = tau
                    * compensated_sum(
                        std::iter::once(rhs[(step, column)]).chain(
                            (step + 1..rhs.nrows())
                                .map(|row| self.qr[(row, basis)] * rhs[(row, column)]),
                        ),
                    );
                rhs[(step, column)] -= projection;
                for row in step + 1..rhs.nrows() {
                    rhs[(row, column)] -= self.qr[(row, basis)] * projection;
                }
            }
        }
        let mut result = DenseMatrix::zeros(self.qr.ncols(), rhs.ncols())?;
        for step in (0..self.rank()).rev() {
            let basis = self.permutation[step];
            for column in 0..rhs.ncols() {
                let tail = compensated_sum((step + 1..self.rank()).map(|next| {
                    self.qr[(step, self.permutation[next])]
                        * result[(self.permutation[next], column)]
                }));
                result[(basis, column)] = (rhs[(step, column)] - tail) / self.qr[(step, basis)];
            }
        }
        for &basis in &self.permutation[..self.rank()] {
            for column in 0..rhs.ncols() {
                result[(basis, column)] = scale_ratio(
                    result[(basis, column)],
                    scales[column],
                    self.maxima[basis],
                    self.norms[basis],
                );
            }
        }
        if result.as_slice().iter().any(|v| !v.is_finite()) {
            return Err(numerical("QR coefficients are not representable"));
        }
        Ok(result)
    }
}
fn column_norm(matrix: &DenseMatrix, column: usize, start: usize) -> f64 {
    (start..matrix.nrows()).fold(0., |norm, row| norm.hypot(matrix[(row, column)]))
}
// Split exponents before rescaling: neither an intermediate overflow nor an
// intermediate underflow may discard an otherwise representable coefficient.
fn scale_ratio(value: f64, numerator: f64, denominator: f64, norm: f64) -> f64 {
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
mod tests {
    use super::scale_ratio;
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
