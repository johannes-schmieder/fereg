//! Exact two-column changes of coordinates on the supplied binary64 design.
use crate::error::{allocation, Result};
use crate::{interrupt::row_checkpoint, linalg::neumaier_add};

#[derive(Clone, Copy)]
pub(super) struct PairwiseDifference {
    pub(super) coefficient: f64,
    pub(super) scale: f64,
    source_lattice_width: i32,
    source_bits_sum: u128,
}

// Below this scale, squaring a condition number to form the covariance can
// lose at least half the available significand: eps^(1/4) for binary64.
pub(super) const WEAK_RATIO: f64 = 1. / 8192.;

/// Certify replacing z by scale * (z - coefficient * x), keeping x.
/// Four linear passes require constant scratch. The chosen orientation
/// subsequently materializes its exact replacement in one additional pass.
/// The estimator still establishes rank on the full current sample and model.
pub(super) fn discover(
    predictor: &[f64],
    target: &[f64],
    centered: Option<([f64; 2], [f64; 2])>,
) -> Result<Option<PairwiseDifference>> {
    if predictor.len() != target.len() || predictor.len() < 2 {
        return Ok(None);
    }
    let (mut anchor_x, mut anchor_z, mut max_z) = (0_f64, 0_f64, 0_f64);
    let (mut finest_bit, mut source_bits_sum) = (i32::MAX, 0_u128);
    for (row, (&x, &z)) in predictor.iter().zip(target).enumerate() {
        row_checkpoint(row)?;
        if !x.is_finite() || !z.is_finite() {
            return Ok(None);
        }
        if x != 0. {
            finest_bit = finest_bit.min(least_bit(x));
            source_bits_sum += u128::from(x.to_bits());
        }
        max_z = max_z.max(z.abs());
        // Equal magnitudes use the smaller signed target in positive-x
        // coordinates, so a row permutation cannot change the reference.
        if x.abs() > anchor_x.abs()
            || (x.abs() == anchor_x.abs() && z * x.signum() < anchor_z * anchor_x.signum())
        {
            (anchor_x, anchor_z) = (x, z);
        }
    }
    let ratio = anchor_z / anchor_x;
    if !ratio.is_finite() || ratio == 0. {
        return Ok(None);
    }

    let mut departure = 0_f64;
    let mut resolved = false;
    for (row, (&x, &z)) in predictor.iter().zip(target).enumerate() {
        row_checkpoint(row)?;
        let d = (-ratio).mul_add(x, z).abs();
        if d > WEAK_RATIO * max_z {
            return Ok(None);
        }
        departure = departure.max(d);
        // The reference ratio involves one division. This margin also
        // excludes columns differing only by rounding of a common multiple.
        let raw = z.abs().max(ratio.abs() * x.abs());
        resolved |= d > 8. * f64::EPSILON * raw + 8. * f64::from_bits(1);
    }
    if !resolved {
        return Ok(None);
    }

    let coefficient = simple_coefficient(ratio, anchor_x.abs(), departure);
    let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
    let coefficient_bit = least_bit(coefficient);
    for (row, (&x, &z)) in predictor.iter().zip(target).enumerate() {
        row_checkpoint(row)?;
        let d = (-coefficient).mul_add(x, z);
        if !d.is_finite() || !exact_contrast(coefficient_bit, x, z, d) {
            return Ok(None);
        }
        low = low.min(d);
        high = high.max(d);
    }
    if let Some(([xmin, xmax], [zmin, zmax])) = centered {
        // With a constant, only the spreads identify either slope. A large
        // common location can make independent columns look proportional in
        // the raw norm. Since z = a*x + d exactly, range(z) - range(d) is
        // also a lower bound for |a|*range(x).
        let spread = zmax - zmin;
        let limit = if spread.is_finite() {
            WEAK_RATIO * spread
        } else {
            WEAK_RATIO * zmax - WEAK_RATIO * zmin
        };
        if xmin >= xmax || high <= low || high - low > limit {
            return Ok(None);
        }
    }
    let largest = low.abs().max(high.abs());
    if largest == 0. {
        return Ok(None);
    }
    // Normalizing the largest value into [1, 2) must itself have a
    // representable multiplier in the coefficient-publication map.
    let exponent = top_bit(largest);
    if exponent < -1023 {
        return Ok(None);
    }
    let scale = if exponent == 1023 {
        f64::from_bits(1_u64 << 51) // 2^-1023 is subnormal
    } else {
        f64::from_bits(((1023 - exponent) as u64) << 52)
    };
    let (mut xx, mut xr, mut rr) = (0., 0., 0.);
    let (mut xx_low, mut xr_low, mut rr_low) = (0., 0., 0.);
    for (row, (&x, &z)) in predictor.iter().zip(target).enumerate() {
        row_checkpoint(row)?;
        let d = (-coefficient).mul_add(x, z);
        let r = d * scale;
        if r / scale != d {
            return Ok(None);
        }
        let (u, r) = if let Some(([xmin, xmax], _)) = centered {
            // A residual proportional to x plus a constant is also an alias.
            // Midranges are independent of row order, and both resulting
            // coordinates are in [-1, 1] even at the exponent extremes.
            (
                midrange_coordinate(x, xmin, xmax),
                midrange_coordinate(d, low, high),
            )
        } else {
            (x / anchor_x.abs(), r)
        };
        neumaier_add(u * u, &mut xx, &mut xx_low);
        neumaier_add(u * r, &mut xr, &mut xr_low);
        neumaier_add(r * r, &mut rr, &mut rr_low);
    }
    // Both columns are bounded. Even a perfect arithmetic identity is a
    // poor basis if its normalized Gram matrix is still nearly singular.
    let correlation = (xr + xr_low).abs() / ((xx + xx_low).sqrt() * (rr + rr_low).sqrt());
    if !correlation.is_finite() || correlation >= 1. - 1. / 1024. {
        return Ok(None);
    }
    Ok(Some(PairwiseDifference {
        coefficient,
        scale,
        source_lattice_width: top_bit(anchor_x) - finest_bit,
        source_bits_sum,
    }))
}

impl PairwiseDifference {
    // Prefer a simple shear, then an unchanged source on a coarser relative
    // lattice. Remaining ties use the smaller multiplier, then an exact
    // order-independent sum of the source bit patterns (signed zero agrees).
    // An unresolved numerical tie then follows ordinary requested ordering.
    pub(super) fn priority(&self) -> (i32, i32, u64, u128) {
        (
            top_bit(self.coefficient) - least_bit(self.coefficient),
            self.source_lattice_width,
            self.coefficient.abs().to_bits(),
            self.source_bits_sum,
        )
    }

    pub(super) fn materialize(
        &self,
        predictor: &[f64],
        target: &[f64],
        column_fits: bool,
    ) -> Result<Vec<f64>> {
        if !column_fits {
            return Err(allocation(
                "insufficient memory for the slope recovery column",
            ));
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(target.len())
            .map_err(|_| allocation("cannot allocate the slope recovery column"))?;
        for (row, (&x, &z)) in predictor.iter().zip(target).enumerate() {
            row_checkpoint(row)?;
            values.push((-self.coefficient).mul_add(x, z) * self.scale);
        }
        Ok(values)
    }
}

fn midrange_coordinate(value: f64, low: f64, high: f64) -> f64 {
    let span = high - low;
    let position = if span.is_finite() {
        (value - low) / span
    } else {
        (value * 0.5 - low * 0.5) / (high * 0.5 - low * 0.5)
    };
    2. * position - 1.
}

// Pick the shortest significand compatible with the observed departure.
// This recovers e.g. a coefficient of exactly 3 even when the reference
// row has its own weak contrast. A rounded residual alone is never a proof.
fn simple_coefficient(ratio: f64, max_x: f64, departure: f64) -> f64 {
    let bits = ratio.abs().to_bits();
    for drop in (1..=52).rev() {
        let unit = 1_u64 << drop;
        let rounded = (bits + unit / 2) & !(unit - 1);
        let candidate = f64::from_bits(rounded).copysign(ratio);
        if candidate != 0. && (candidate - ratio).abs() * max_x <= 4. * departure {
            return candidate;
        }
    }
    ratio
}

// For finite nonzero x, return the exponent of its highest and lowest set
// bits. Include subnormals, whose significand does not have a hidden bit.
fn top_bit(x: f64) -> i32 {
    let bits = x.abs().to_bits();
    let exponent = (bits >> 52) as i32;
    if exponent == 0 {
        63 - bits.leading_zeros() as i32 - 1074
    } else {
        exponent - 1023
    }
}

fn least_bit(x: f64) -> i32 {
    let bits = x.abs().to_bits();
    let exponent = (bits >> 52) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    if exponent == 0 {
        fraction.trailing_zeros() as i32 - 1074
    } else {
        ((1_u64 << 52) | fraction).trailing_zeros() as i32 + exponent - 1075
    }
}

fn exact_contrast(coefficient_bit: i32, x: f64, z: f64, difference: f64) -> bool {
    if x == 0. {
        return true;
    }
    let product_bit = coefficient_bit + least_bit(x);
    let grid = if z == 0. {
        product_bit
    } else {
        product_bit.min(least_bit(z))
    };
    // The exact value lies on this binary lattice. If the greatest adjacent
    // floating spacing at the single-rounded FMA result is at most the
    // lattice spacing, that result is exact (including a result of zero).
    let spacing = (top_bit(difference) - 52).max(-1074);
    grid >= spacing
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Materialized {
        coefficient: f64,
        scale: f64,
        values: Vec<f64>,
    }

    fn pairwise_difference(
        x: &[f64],
        z: &[f64],
        centered: Option<([f64; 2], [f64; 2])>,
        fits: bool,
    ) -> Result<Option<Materialized>> {
        discover(x, z, centered)?
            .map(|basis| {
                Ok(Materialized {
                    coefficient: basis.coefficient,
                    scale: basis.scale,
                    values: basis.materialize(x, z, fits)?,
                })
            })
            .transpose()
    }

    #[test]
    fn both_orders_and_row_permutations_have_an_exact_weak_design() {
        let x = [0., 1., -2., 1., 2.];
        let v = [1., -2., 1., 0., -1.];
        for depth in [18, 36, 44] {
            let step = 2_f64.powi(-depth);
            let z: [f64; 5] = std::array::from_fn(|i| x[i] + step * v[i]);
            for (left, right) in [(&x, &z), (&z, &x)] {
                let result = pairwise_difference(left, right, None, true)
                    .unwrap()
                    .unwrap();
                assert_eq!(result.coefficient, 1.);
                for (i, &value) in result.values.iter().enumerate() {
                    assert_eq!(value / result.scale, right[i] - left[i]);
                }
                let mut perm_x = *left;
                let mut perm_z = *right;
                perm_x.reverse();
                perm_z.reverse();
                let reversed = pairwise_difference(&perm_x, &perm_z, None, true)
                    .unwrap()
                    .unwrap();
                assert_eq!(reversed.coefficient, result.coefficient);
                assert_eq!(reversed.scale, result.scale);
                assert!(reversed.values.iter().rev().eq(&result.values));
            }
        }
    }

    #[test]
    fn exact_aliases_and_rounding_of_a_common_multiple_decline() {
        for (x, z) in [
            ([3., -6., 0., 9.], [1., -2., -0., 3.]),
            (
                [0.1_f64, 0.2, 0.3, 0.4],
                [0.1_f64, 0.2, 0.3, 0.4].map(|x| 3. * x),
            ),
            ([0., 1., 2., 4.], [3., 0., 2., 1.]),
        ] {
            assert!(pairwise_difference(&x, &z, None, true).unwrap().is_none());
            assert!(pairwise_difference(&z, &x, None, true).unwrap().is_none());
        }
    }

    #[test]
    fn a_tight_memory_limit_only_refuses_an_established_recovery_column() {
        let x = [1., 2., 3.];
        let alias = [3., 6., 9.];
        assert!(pairwise_difference(&x, &alias, None, false)
            .unwrap()
            .is_none());
        let identified = [3., 6. + 2_f64.powi(-30), 9.];
        let error = pairwise_difference(&x, &identified, None, false)
            .err()
            .unwrap();
        assert_eq!(error.code, crate::error::ErrorCode::Allocation);
    }

    #[test]
    fn independent_translated_slopes_only_need_recovery_without_a_constant() {
        let x = [0., 1., 2., 0., 1., 2.].map(|v| v + 2_f64.powi(24));
        let z = [0., 0., 0., 1., 1., 1.].map(|v| v - 2_f64.powi(22));
        let (xs, zs) = ([x[0], x[2]], [z[0], z[3]]);
        for (a, b, centered) in [(&x, &z, (xs, zs)), (&z, &x, (zs, xs))] {
            assert!(pairwise_difference(a, b, Some(centered), false)
                .unwrap()
                .is_none());
            assert!(pairwise_difference(a, b, None, true).unwrap().is_some());
        }
        let w = std::array::from_fn::<_, 6, _>(|i| x[i] + (i / 3) as f64 * 2_f64.powi(-18));
        let p = pairwise_difference(&x, &w, Some((xs, [w[0], w[5]])), true)
            .unwrap()
            .unwrap();
        assert_eq!(p.coefficient, 1.);
        assert_eq!(p.values, [0., 0., 0., 1., 1., 1.]);
        assert_eq!(midrange_coordinate(0., -f64::MAX, f64::MAX), 0.);
    }

    #[test]
    fn rounded_product_can_have_an_exact_fused_difference() {
        let x = [0.1_f64, 0.2, 0.3, 0.4];
        let z = std::array::from_fn::<_, 4, _>(|i| 3. * x[i] + 2_f64.powi(-30) * (i % 2) as f64);
        let p = pairwise_difference(&x, &z, None, true).unwrap().unwrap();
        assert_eq!(p.coefficient, 3.);
        assert_ne!(p.values[0], 0.); // 3 * x[0] rounded up in the raw target.
        assert_eq!(p.values[0] / p.scale, 2_f64.powi(-55));
        assert!(!exact_contrast(least_bit(1. + f64::EPSILON), 1.1, 0., 1.1));
        assert!(!exact_contrast(-1, f64::from_bits(1), 0., 0.));
    }
}
