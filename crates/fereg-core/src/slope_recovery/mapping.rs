//! Original-unit publication for a fit in coordinates (x, (z - a*x)*s).
//! The reference fit, including its covariance, is already certified. A
//! congruence in real arithmetic does not certify the *posted doubles*.

use crate::error::{invalid_argument, ErrorCode, FeregError, Result};
use crate::interrupt;
use crate::memory::Capacity;

/// Positions on the published coefficient/covariance axis, including the constant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairMap {
    pub source: usize,
    pub target: usize,
    pub multiple: f64,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct PredictionCheck {
    map: PairMap,
    stable: [f64; 2],
    posted: [f64; 2],
}

const COEFFICIENT_RTOL: f64 = 1e-7;
const COVARIANCE_RTOL: f64 = 1e-6;
const ESTIMATE_ATOL: f64 = 1e-10;

fn unavailable(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, format!("{message}. Refit using an explicitly scaled difference between the nearly collinear variables."))
}

fn close(actual: f64, reference: f64, relative: f64, absolute: f64) -> bool {
    actual.is_finite()
        && reference.is_finite()
        && (actual - reference).abs() <= absolute + relative * reference.abs()
}

pub(crate) fn check_predictor_values(raw: f64, stable: f64) -> Result<()> {
    if close(raw, stable, COEFFICIENT_RTOL, 1e-9) {
        Ok(())
    } else {
        Err(unavailable(
            "original-unit slopes cannot reproduce the certified predictor",
        ))
    }
}

impl PredictionCheck {
    /// The fitted coefficient in the conditioned design, using the same slots.
    pub fn stable_coefficient(&self, slot: usize, other_value: f64) -> f64 {
        if slot == self.map.source {
            self.stable[0]
        } else if slot == self.map.target {
            self.stable[1]
        } else {
            other_value
        }
    }

    /// The coefficient to use with the original design, in its native slot order.
    pub fn posted_coefficient(&self, slot: usize, other_value: f64) -> f64 {
        if slot == self.map.source {
            self.posted[0]
        } else if slot == self.map.target {
            self.posted[1]
        } else {
            other_value
        }
    }

    /// Replay the represented design identity and both finite-slope products
    /// on an actual retained row. Use ordinary products and additions for the
    /// posted predictor, as consumers need not use fused arithmetic.
    pub fn check_row(&self, source: f64, target: f64, residualized: f64) -> Result<()> {
        let residual = (-self.map.multiple).mul_add(source, target) * self.map.scale;
        let stable = source * self.stable[0] + residualized * self.stable[1];
        let original = source * self.posted[0] + target * self.posted[1];
        if residual != residualized {
            return Err(unavailable(
                "original-unit slope design failed the retained-row identity",
            ));
        }
        check_predictor_values(original, stable)
    }
}

impl PairMap {
    /// One original-unit entry for comparison with an independent influence
    /// calculation before publication. All indices share the certified axis.
    pub(crate) fn projected_covariance(
        self,
        stable: &[f64],
        dimension: usize,
        row: usize,
        column: usize,
    ) -> f64 {
        CovarianceMap::new(self, stable, dimension).posted(row, column)
    }

    /// Check the two posted coefficients and return a row-prediction witness.
    pub fn prediction_check(self, coefficients: &[f64]) -> Result<PredictionCheck> {
        let n = coefficients.len();
        if self.source >= n
            || self.target >= n
            || self.source == self.target
            || !self.multiple.is_finite()
            || !self.scale.is_finite()
            || self.scale <= 0.0
        {
            return Err(invalid_argument("invalid original-unit slope mapping"));
        }
        let stable = [coefficients[self.source], coefficients[self.target]];
        let z = stable[1] * self.scale;
        let x = (-self.multiple).mul_add(z, stable[0]);
        if !close(
            x + self.multiple * z,
            stable[0],
            COEFFICIENT_RTOL,
            ESTIMATE_ATOL,
        ) || !close(
            self.multiple.mul_add(z, x),
            stable[0],
            COEFFICIENT_RTOL,
            ESTIMATE_ATOL,
        ) || !close(z / self.scale, stable[1], COEFFICIENT_RTOL, ESTIMATE_ATOL)
        {
            return Err(unavailable(
                "original-unit coefficients lose an identified contrast",
            ));
        }
        Ok(PredictionCheck {
            map: self,
            stable,
            posted: [x, z],
        })
    }

    /// Restore both slopes and all their covariance rows, atomically. On
    /// success the caller must still replay its retained prediction rows.
    pub fn restore(
        self,
        coefficients: &mut [f64],
        covariance: &mut [f64],
    ) -> Result<PredictionCheck> {
        self.restore_with_rank_policy(coefficients, covariance, true)
    }

    /// Translation along an unrestricted explicit intercept leaves the model
    /// slope restrictions unchanged. Check mathematical rank and every contrast
    /// of the posted doubles here; the PPML caller separately certifies Stata's
    /// model Wald test on the original slopes.
    pub(crate) fn restore_intercept(
        self,
        coefficients: &mut [f64],
        covariance: &mut [f64],
    ) -> Result<PredictionCheck> {
        self.restore_with_rank_policy(coefficients, covariance, false)
    }

    fn restore_with_rank_policy(
        self,
        coefficients: &mut [f64],
        covariance: &mut [f64],
        native_joint_rank: bool,
    ) -> Result<PredictionCheck> {
        let n = coefficients.len();
        if n.checked_mul(n) != Some(covariance.len()) {
            return Err(invalid_argument(
                "invalid original-unit covariance dimensions",
            ));
        }
        let prediction = self.prediction_check(coefficients)?;
        let pair = CovarianceMap::new(self, covariance, n);
        pair.certify(native_joint_rank)?;

        // No fallible operation after this point. Complete every cross with
        // an unaffected regressor (including _cons) before replacing the block.
        let [xx, xz, zz] = pair.posted;
        for j in 0..n {
            if j != self.source && j != self.target {
                let target = self.scale * covariance[self.target * n + j];
                let source = (-self.multiple).mul_add(target, covariance[self.source * n + j]);
                covariance[self.source * n + j] = source;
                covariance[j * n + self.source] = source;
                covariance[self.target * n + j] = target;
                covariance[j * n + self.target] = target;
            }
        }
        covariance[self.source * n + self.source] = xx;
        covariance[self.source * n + self.target] = xz;
        covariance[self.target * n + self.source] = xz;
        covariance[self.target * n + self.target] = zz;
        coefficients[self.source] = prediction.posted[0];
        coefficients[self.target] = prediction.posted[1];
        Ok(prediction)
    }
}

struct CovarianceMap<'a> {
    map: PairMap,
    stable: &'a [f64],
    n: usize,
    posted: [f64; 3],
}

impl<'a> CovarianceMap<'a> {
    fn new(map: PairMap, stable: &'a [f64], n: usize) -> Self {
        let (x, r, s, a) = (map.source, map.target, map.scale, map.multiple);
        let zz = stable[r * n + r] * s * s;
        let uz = stable[x * n + r] * s;
        let xz = (-a).mul_add(zz, uz);
        let xx = (-a).mul_add(xz, (-a).mul_add(uz, stable[x * n + x]));
        Self {
            map,
            stable,
            n,
            posted: [xx, xz, zz],
        }
    }

    fn stable(&self, i: usize, j: usize) -> f64 {
        self.stable[i * self.n + j]
    }

    fn posted(&self, i: usize, j: usize) -> f64 {
        let PairMap {
            source: x,
            target: z,
            multiple: a,
            scale: s,
        } = self.map;
        let [xx, xz, zz] = self.posted;
        if i == x || j == x {
            let k = if i == x { j } else { i };
            if k == x {
                return xx;
            }
            if k == z {
                return xz;
            }
            return (-a).mul_add(s * self.stable(z, k), self.stable(x, k));
        }
        if i == z || j == z {
            let k = if i == z { j } else { i };
            return if k == z { zz } else { s * self.stable(z, k) };
        }
        self.stable(i, j)
    }

    fn replay(&self, i: usize, j: usize, strong_variance: f64) -> f64 {
        let PairMap {
            source: x,
            target: z,
            multiple: a,
            scale: s,
        } = self.map;
        if i == x || j == x {
            let k = if i == x { j } else { i };
            if k == x {
                return strong_variance;
            }
            let cross = self.posted(x, k) + a * self.posted(z, k);
            return if k == z { cross / s } else { cross };
        }
        let mut value = self.posted(i, j);
        if i == z {
            value /= s;
        }
        if j == z {
            value /= s;
        }
        value
    }

    fn certify(&self, native_joint_rank: bool) -> Result<()> {
        let [xx, xz, zz] = self.posted;
        let a = self.map.multiple;
        // Stata consumers can evaluate the quadratic either by two matrix
        // products or by expanding it. Check both orders as well as the
        // fused dot products; a favorable cancellation cannot authorize a post.
        let twice_cross = 2.0 * (a * xz);
        let last = a * (a * zz);
        let variances = [
            (xx + a * xz) + a * (xz + a * zz),
            (xx + twice_cross) + last,
            (last + twice_cross) + xx,
            a.mul_add(a.mul_add(zz, xz), a.mul_add(xz, xx)),
            accurate_quadratic(xx, xz, zz, a),
        ];
        for variance in variances {
            let expected = self.stable(self.map.source, self.map.source);
            if !close(variance, expected, COVARIANCE_RTOL, ESTIMATE_ATOL)
                || !close(
                    variance.sqrt(),
                    expected.sqrt(),
                    COVARIANCE_RTOL,
                    ESTIMATE_ATOL,
                )
            {
                return Err(unavailable(
                    "original-unit covariance loses the identified slope contrast",
                ));
            }
        }

        let n = self.n;
        for slot in [self.map.source, self.map.target] {
            for j in 0..n {
                let replayed = self.replay(slot, j, variances[0]);
                let expected = self.stable(slot, j);
                if !close(replayed, expected, COVARIANCE_RTOL, ESTIMATE_ATOL)
                    || slot == j
                        && !close(
                            replayed.sqrt(),
                            expected.sqrt(),
                            COVARIANCE_RTOL,
                            ESTIMATE_ATOL,
                        )
                {
                    return Err(unavailable(
                        "original-unit covariance cannot reproduce the certified cross terms",
                    ));
                }
            }
        }
        let mut factor = Capacity::<f64>::new(self.stable.len() as u128).filled(
            0.0,
            "unable to reserve original-unit covariance certificate",
        )?;
        let mut scales = Capacity::<f64>::new(n as u128)
            .filled(0.0, "unable to reserve original-unit covariance scales")?;
        let stable = |i, j| self.stable(i, j);
        let posted = |i, j| self.posted(i, j);
        let stable_rank = covariance_rank(n, &mut factor, &mut scales, stable, false)?;
        let raw_rank = covariance_rank(n, &mut factor, &mut scales, posted, native_joint_rank)?;
        let slots = [self.map.source, self.map.target];
        let pair_rank = |i: usize, j: usize| self.stable(slots[i], slots[j]);
        let original_pair = |i: usize, j: usize| self.posted(slots[i], slots[j]);
        if raw_rank != stable_rank
            || covariance_rank(2, &mut factor, &mut scales, pair_rank, false)?
                != covariance_rank(
                    2,
                    &mut factor,
                    &mut scales,
                    original_pair,
                    native_joint_rank,
                )?
        {
            return Err(unavailable(
                "original-unit covariance loses joint-test rank",
            ));
        }

        let lo = variances.into_iter().fold(f64::INFINITY, f64::min);
        let hi = variances.into_iter().fold(f64::NEG_INFINITY, f64::max);
        // For every contrast c, bound |c'(V_replayed−V)c| by
        // rtol*c'V*c + atol²*||c||². The squared absolute SE allowance
        // protects genuinely zero variances as well as their standard errors.
        // Only the strong diagonal depends on the summation order above.
        covariance_rank(
            n,
            &mut factor,
            &mut scales,
            |i, j| self.replay(i, j, lo),
            false,
        )?;
        for (sign, variance) in [(1.0, lo), (-1.0, hi)] {
            covariance_rank(
                n,
                &mut factor,
                &mut scales,
                |i, j| {
                    let reference = stable(i, j);
                    let error = self.replay(i, j, variance) - reference;
                    let margin = COVARIANCE_RTOL * reference + sign * error;
                    if i == j {
                        margin + ESTIMATE_ATOL.powi(2)
                    } else {
                        margin
                    }
                },
                false,
            )?;
        }
        Ok(())
    }
}

// Independently evaluate the mathematical contrast of the posted doubles.
// The FMA remainders retain product errors even when a is not a power of two.
fn accurate_quadratic(xx: f64, xz: f64, zz: f64, a: f64) -> f64 {
    let cross = a * xz;
    let inner = a * zz;
    let inner_error = a.mul_add(zz, -inner);
    let outer = a * inner;
    let correction = a * inner_error;
    let mut sum = 0.0;
    let mut remainder = 0.0;
    for term in [
        xx,
        2.0 * cross,
        outer,
        2.0 * a.mul_add(xz, -cross),
        a.mul_add(inner, -outer),
        correction,
        a.mul_add(inner_error, -correction),
    ] {
        crate::linalg::neumaier_add(term, &mut sum, &mut remainder);
    }
    sum + remainder
}

/// PSD and symmetric elimination in unit-diagonal coordinates. Zero variance
/// must have a genuine zero row. Postestimation uses an additional rank rule:
/// Stata's symmetric inverse drops a remaining pivot below 1e-9 of the
/// uneliminated variance or a raw variance pivot below 1e-19.
fn covariance_rank(
    n: usize,
    factor: &mut [f64],
    scales: &mut [f64],
    entry: impl Fn(usize, usize) -> f64,
    native_rank: bool,
) -> Result<usize> {
    let invalid = || unavailable("original-unit covariance fails the contrast or PSD certificate");
    let scales = &mut scales[..n];
    for (i, scale) in scales.iter_mut().enumerate() {
        *scale = entry(i, i).sqrt();
        if !scale.is_finite() {
            return Err(invalid());
        }
    }
    for i in 0..n {
        interrupt::row_checkpoint(i)?;
        for j in 0..=i {
            let value = entry(i, j);
            let small = scales[i].min(scales[j]);
            let large = scales[i].max(scales[j]);
            let correlation = if i == j && small > 0.0 {
                1.0
            } else if small > 0.0 {
                (value / large) / small
            } else if value == 0.0 {
                0.0
            } else {
                return Err(invalid());
            };
            if !correlation.is_finite() || value != entry(j, i) {
                return Err(invalid());
            }
            factor[i * n + j] = correlation;
            factor[j * n + i] = correlation;
        }
    }
    let noise = 32.0 * (n as f64) * f64::EPSILON;
    let mut rank = 0;
    for p in 0..n {
        interrupt::checkpoint()?;
        let mut pivot = p;
        for i in p + 1..n {
            if factor[i * n + i] > factor[pivot * n + pivot]
                || (factor[i * n + i] == factor[pivot * n + pivot] && scales[i] > scales[pivot])
            {
                pivot = i;
            }
        }
        let diagonal = factor[pivot * n + pivot];
        if diagonal <= noise {
            for i in p..n {
                for j in p..=i {
                    if factor[i * n + j].abs() > noise {
                        return Err(invalid());
                    }
                }
            }
            break;
        }
        // Allow for normalized elimination roundoff around the relative threshold.
        let lower = (diagonal - noise).max(0.0);
        if !native_rank || lower > 1e-9 && (diagonal * scales[pivot]) * scales[pivot] >= 1e-19 {
            rank += 1;
        }
        if p != pivot {
            for i in 0..n {
                factor.swap(p * n + i, pivot * n + i);
            }
            for i in 0..n {
                factor.swap(i * n + p, i * n + pivot);
            }
            scales.swap(p, pivot);
        }
        for i in p + 1..n {
            let loading = factor[i * n + p] / diagonal;
            for j in p + 1..=i {
                let value = (-loading).mul_add(factor[j * n + p], factor[i * n + j]);
                if !value.is_finite() {
                    return Err(invalid());
                }
                factor[i * n + j] = value;
                factor[j * n + i] = value;
            }
        }
    }
    Ok(rank)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(scale: f64) -> PairMap {
        PairMap {
            source: 0,
            target: 1,
            multiple: 1.,
            scale,
        }
    }

    #[test]
    fn restore_matches_exact_congruence_including_constant_and_omitted_slot() {
        let map = PairMap {
            source: 1,
            target: 0,
            multiple: -0.5,
            scale: 4096.0,
        };
        // Slots are r, x, omitted, constant; non-diagonal certified fit.
        let mut b = [0.5, 0.25, 0., -1.];
        let mut v = [
            2., 0.25, 0., 0.5, 0.25, 4., 0., -0.25, 0., 0., 0., 0., 0.5, -0.25, 0., 2.,
        ];
        let p = map.restore(&mut b, &mut v).unwrap();
        assert_eq!(b, [2048., 1024.25, 0., -1.]);
        assert_eq!(
            v,
            [
                33554432., 16778240., 0., 2048., 16778240., 8389636., 0., 1023.75, 0., 0., 0., 0.,
                2048., 1023.75, 0., 2.
            ]
        );
        p.check_row(4., -2. + 3. / 4096., 3.).unwrap();
        assert!(p.check_row(4., -2. + 3. / 4096., 2.).is_err());
    }

    #[test]
    fn even_exact_replayed_variance_cannot_authorize_lost_joint_test_rank() {
        let mut b = [1., 0.5];
        let mut v = [1., 0., 0., 1.];
        let old = (b, v);
        let error = map(32768.).restore(&mut b, &mut v).unwrap_err();
        assert!(error.message.contains("joint-test rank"), "{error}");
        assert_eq!((b, v), old);
        map(8192.).restore(&mut b, &mut v).unwrap();
    }

    #[test]
    fn tiny_conditioned_variance_restores_to_well_conditioned_public_inference() {
        let m = map(2_f64.powi(36));
        let mut b = [0.5, 0.25];
        let t = m.scale.recip();
        let mut v = [1., 0.1 * t, 0.1 * t, t * t];
        m.restore(&mut b, &mut v).unwrap();
        assert_eq!(v[3], 1.);
        assert!((v[0] - 1.8).abs() < 1e-15);
        assert!((v[1] + 0.9).abs() < 1e-15);
    }

    #[test]
    fn full_rank_certified_matrix_can_lose_the_strong_variance_after_rounding() {
        let mut b = [0., 0.];
        let mut v = [0.00497487173760006, 0., 0., 0.00497487173760006];
        let error = map(2_f64.powi(36)).restore(&mut b, &mut v).unwrap_err();
        assert!(error.message.contains("identified slope contrast"));
    }

    #[test]
    fn genuine_zero_and_singular_covariance_and_unrepresentable_xb() {
        let mut b = [1., 1.];
        let mut v = [0.; 4];
        let p = map(2_f64.powi(45)).restore(&mut b, &mut v).unwrap();
        assert_eq!(v, [0.; 4]);
        assert!(p.check_row(1.03, 1.03, 0.).is_err());
        p.check_row(2., 2., 0.).unwrap();

        let mut b = [2., 0.];
        let mut singular = [0.1, 0., 0., 0.];
        map(2_f64.powi(45)).restore(&mut b, &mut singular).unwrap();
        assert_eq!(singular, [0.1, 0., 0., 0.]);
    }

    #[test]
    fn native_symmetric_inverse_also_drops_tiny_diagonal_variances() {
        let mut work = [0.; 4];
        let mut scales = [0.; 2];
        let raw = [1., 0., 0., 1e-22];
        let rank = covariance_rank(2, &mut work, &mut scales, |i, j| raw[2 * i + j], true).unwrap();
        assert_eq!(rank, 1);
    }

    #[test]
    fn independent_full_matrix_contrast_can_fail_even_when_each_entry_is_accurate() {
        let m = PairMap {
            multiple: 1.1,
            ..map(64.)
        };
        let mut b = [0.; 3];
        // First and third coordinates have identical scores. A small roundoff
        // in x's variance gives their true null contrast a fictitious SE.
        let mut v = [1., 0.1, 1., 0.1, 1., 0.1, 1., 0.1, 1.];
        let error = m.restore(&mut b, &mut v).unwrap_err();
        assert!(error.message.contains("PSD certificate"), "{error}");
    }
}
